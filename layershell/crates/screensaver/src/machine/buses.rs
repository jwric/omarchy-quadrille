//! The buses: the PCI devices and the bridges they sit behind, and the tree
//! of USB hubs and devices.
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use super::Tree;

/// Where a PCI device is: domain, bus, device and function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PciAddress {
    pub domain: u32,
    pub bus: u8,
    pub device: u8,
    pub function: u8,
}

impl PciAddress {
    pub const fn new(domain: u32, bus: u8, device: u8, function: u8) -> Self {
        Self {
            domain,
            bus,
            device,
            function,
        }
    }
}

impl FromStr for PciAddress {
    type Err = ();

    /// `0000:01:00.0`; a domain may have more than four digits, as a volume
    /// management device's `10000:e1:00.0` does.
    fn from_str(text: &str) -> Result<Self, ()> {
        let (domain, rest) = text.split_once(':').ok_or(())?;
        let (bus, rest) = rest.split_once(':').ok_or(())?;
        let (device, function) = rest.split_once('.').ok_or(())?;
        let hex = |digits: &str, width: usize| {
            (digits.len() == width && digits.bytes().all(|b| b.is_ascii_hexdigit()))
                .then(|| u32::from_str_radix(digits, 16).ok())
                .flatten()
                .ok_or(())
        };

        if domain.len() < 4 || function.len() != 1 {
            return Err(());
        }

        Ok(Self {
            domain: hex(domain, domain.len())?,
            bus: hex(bus, 2)? as u8,
            device: hex(device, 2)? as u8,
            function: hex(function, 1)? as u8,
        })
    }
}

impl fmt::Display for PciAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04x}:{:02x}:{:02x}.{:x}",
            self.domain, self.bus, self.device, self.function
        )
    }
}

/// A device on the PCI bus.
#[derive(Debug, Clone, PartialEq)]
pub struct PciDevice {
    pub address: PciAddress,
    /// The bridge it sits behind; `None` on a root bus.
    pub parent: Option<PciAddress>,
    /// Its class code: base class, subclass and programming interface.
    pub class: u32,
    pub vendor: u16,
    pub device: u16,
    /// The vendor's and the device's names from `pci.ids`, when it is
    /// installed and knows them.
    pub vendor_name: Option<String>,
    pub name: Option<String>,
    /// What its class is called: `VGA compatible controller`.
    pub class_name: Option<String>,
    /// The kernel driver bound to it: `nvme`, `xhci_hcd`.
    pub driver: Option<String>,
}

/// What a PCI device is for, by its class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PciKind {
    /// A bridge to another bus: what devices sit behind.
    Bridge,
    /// The host bridge and the chipset's other bridges.
    Chipset,
    Display,
    Storage,
    Network,
    Usb,
    Audio,
    Other,
}

impl PciDevice {
    pub fn kind(&self) -> PciKind {
        let (base, sub) = ((self.class >> 16) as u8, (self.class >> 8) as u8);

        match (base, sub) {
            (0x06, 0x04 | 0x09) => PciKind::Bridge,
            (0x06, _) => PciKind::Chipset,
            (0x03, _) => PciKind::Display,
            (0x01, _) => PciKind::Storage,
            (0x02, _) | (0x0d, _) => PciKind::Network,
            (0x0c, 0x03) => PciKind::Usb,
            (0x04, _) => PciKind::Audio,
            _ => PciKind::Other,
        }
    }
}

/// Every PCI device, by address.
pub(super) fn pci(tree: &Tree) -> Vec<PciDevice> {
    let dir = "sys/bus/pci/devices";
    let mut devices: Vec<PciDevice> = tree
        .entries(dir)
        .iter()
        .filter_map(|name| {
            let address: PciAddress = name.parse().ok()?;
            let at = |file: &str| format!("{dir}/{name}/{file}");
            let hex = |file: &str| {
                let text = tree.text(at(file))?;
                u32::from_str_radix(text.trim_start_matches("0x"), 16).ok()
            };
            let path = tree.pci_path(format!("{dir}/{name}"));

            Some(PciDevice {
                address,
                parent: path
                    .iter()
                    .rev()
                    .skip_while(|step| **step != address)
                    .nth(1)
                    .copied(),
                class: hex("class")?,
                vendor: hex("vendor")? as u16,
                device: hex("device")? as u16,
                vendor_name: None,
                name: None,
                class_name: None,
                driver: tree.link(at("driver")),
            })
        })
        .collect();

    devices.sort_by_key(|device| device.address);
    name(tree, &mut devices);
    devices
}

/// Names the devices from `pci.ids`, or by their base class without it.
fn name(tree: &Tree, devices: &mut [PciDevice]) {
    let ids = ["usr/share/hwdata/pci.ids", "usr/share/misc/pci.ids"]
        .iter()
        .find_map(|path| tree.text(path))
        .map(|text| Ids::parse(&text, devices))
        .unwrap_or_default();

    for device in devices {
        let (base, sub) = ((device.class >> 16) as u8, (device.class >> 8) as u8);

        device.vendor_name = ids.vendors.get(&device.vendor).cloned();
        device.name = ids.devices.get(&(device.vendor, device.device)).cloned();
        device.class_name = ids
            .classes
            .get(&(base, Some(sub)))
            .or_else(|| ids.classes.get(&(base, None)))
            .cloned()
            .or_else(|| base_class(base).map(Into::into));
    }
}

/// The names `pci.ids` gives the devices there are.
#[derive(Debug, Default)]
struct Ids {
    vendors: HashMap<u16, String>,
    devices: HashMap<(u16, u16), String>,
    /// Classes, and subclasses of them.
    classes: HashMap<(u8, Option<u8>), String>,
}

impl Ids {
    /// Reads the names of `devices`' vendors, devices and classes: vendors
    /// at the margin, their devices a tab in, subsystems two; then the
    /// classes, `C` and the base class at the margin, subclasses a tab in.
    fn parse(text: &str, devices: &[PciDevice]) -> Self {
        let vendors: HashSet<u16> = devices.iter().map(|d| d.vendor).collect();
        let ids: HashSet<(u16, u16)> = devices.iter().map(|d| (d.vendor, d.device)).collect();
        let mut names = Self::default();
        let mut vendor = None;
        let mut class = None;

        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }

            let depth = line.bytes().take_while(|b| *b == b'\t').count();
            let Some((id, name)) = line.trim_start_matches('\t').split_once("  ") else {
                continue;
            };
            let name = name.trim().to_owned();

            match (depth, id.strip_prefix("C ")) {
                (0, Some(base)) => {
                    vendor = None;
                    class = u8::from_str_radix(base, 16).ok();
                    if let Some(base) = class {
                        names.classes.insert((base, None), name);
                    }
                }
                (0, None) => {
                    class = None;
                    vendor = u16::from_str_radix(id, 16).ok();
                    if let Some(id) = vendor.filter(|id| vendors.contains(id)) {
                        names.vendors.insert(id, name);
                    }
                }
                (1, None) => {
                    if let (Some(vendor), Ok(device)) = (vendor, u16::from_str_radix(id, 16)) {
                        if ids.contains(&(vendor, device)) {
                            names.devices.insert((vendor, device), name);
                        }
                    } else if let (Some(base), Ok(sub)) = (class, u8::from_str_radix(id, 16)) {
                        names.classes.insert((base, Some(sub)), name);
                    }
                }
                _ => {}
            }
        }

        names
    }
}

/// What `pci.ids` calls a base class.
pub(super) fn base_class(code: u8) -> Option<&'static str> {
    Some(match code {
        0x00 => "Unclassified device",
        0x01 => "Mass storage controller",
        0x02 => "Network controller",
        0x03 => "Display controller",
        0x04 => "Multimedia controller",
        0x05 => "Memory controller",
        0x06 => "Bridge",
        0x07 => "Communication controller",
        0x08 => "Generic system peripheral",
        0x09 => "Input device controller",
        0x0a => "Docking station",
        0x0b => "Processor",
        0x0c => "Serial bus controller",
        0x0d => "Wireless controller",
        0x0e => "Intelligent controller",
        0x0f => "Satellite communications controller",
        0x10 => "Encryption controller",
        0x11 => "Signal processing controller",
        0x12 => "Processing accelerators",
        0x13 => "Non-Essential Instrumentation",
        _ => return None,
    })
}

/// A USB hub or device.
#[derive(Debug, Clone, PartialEq)]
pub struct UsbDevice {
    /// Where it is: `usb3` for the third bus's root hub, `3-2.4` for what is
    /// on port 4 of the hub on the root hub's port 2.
    pub port: String,
    /// The hub it is on; `None` for a root hub.
    pub parent: Option<String>,
    /// The host controller its bus is on.
    pub controller: Option<PciAddress>,
    /// Its speed in Mbit/s: 1.5, 12, 480, 5000, 10000, 20000.
    pub speed: Option<f32>,
    /// The USB version it speaks: `2.00`, `3.20`.
    pub version: Option<String>,
    /// Its class, or its first interface's when the device leaves it to
    /// them: 0x09 a hub, 0x03 an input device, 0x08 storage, 0x0e video.
    pub class: u8,
    /// The ports of a hub.
    pub ports: u32,
    pub product: Option<String>,
    /// Who made it; never a root hub's, which is the kernel's release.
    pub maker: Option<String>,
    /// Whether it can be unplugged, where the port says.
    pub removable: Option<bool>,
}

impl UsbDevice {
    pub fn hub(&self) -> bool {
        self.class == 0x09
    }
}

/// Every USB device, root hubs first, each hub before what is on it.
pub(super) fn usb(tree: &Tree) -> Vec<UsbDevice> {
    let dir = "sys/bus/usb/devices";
    let mut devices: Vec<UsbDevice> = tree
        .entries(dir)
        .into_iter()
        // Interfaces, `3-2:1.0`, are a device's own.
        .filter(|port| !port.contains(':'))
        .map(|port| {
            let at = |file: &str| format!("{dir}/{port}/{file}");
            let hex = |path: String| u8::from_str_radix(&tree.text(path)?, 16).ok();
            let mut class = hex(at("bDeviceClass")).unwrap_or(0);

            // Defined by its interfaces, or a composite device of several.
            if (class == 0x00 || class == 0xef)
                && let Some(interface) = tree
                    .entries(format!("{dir}/{port}"))
                    .into_iter()
                    .find(|name| name.starts_with(&format!("{port}:")))
            {
                class = hex(at(&format!("{interface}/bInterfaceClass"))).unwrap_or(class);
            }

            UsbDevice {
                parent: parent(&port),
                controller: tree.pci_path(format!("{dir}/{port}")).last().copied(),
                speed: tree.number(at("speed")),
                version: tree.text(at("version")),
                class,
                ports: tree.number(at("maxchild")).unwrap_or(0),
                product: tree.text(at("product")),
                maker: (!port.starts_with("usb"))
                    .then(|| tree.text(at("manufacturer")))
                    .flatten(),
                removable: match tree.text(at("removable")).as_deref() {
                    Some("removable") => Some(true),
                    Some("fixed") => Some(false),
                    _ => None,
                },
                port,
            }
        })
        .collect();

    // By bus, then port by port down the tree: `usb3`, `3-2`, `3-2.4`,
    // `3-10`.
    devices.sort_by_key(|device| {
        device
            .port
            .trim_start_matches("usb")
            .split(['-', '.'])
            .map(|step| step.parse::<u32>().unwrap_or(u32::MAX))
            .collect::<Vec<_>>()
    });
    devices
}

/// The hub a port is on: `3-2` for `3-2.4`, `usb3` for `3-2`.
pub(super) fn parent(port: &str) -> Option<String> {
    if port.starts_with("usb") {
        return None;
    }

    match port.rsplit_once('.') {
        Some((hub, _)) => Some(hub.into()),
        None => Some(format!("usb{}", port.split_once('-')?.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Fake, laptop};
    use super::*;

    #[test]
    fn pci_addresses_read_and_write_as_the_kernel_does() {
        let address: PciAddress = "0000:01:00.0".parse().unwrap();
        assert_eq!(address, PciAddress::new(0, 1, 0, 0));
        assert_eq!(address.to_string(), "0000:01:00.0");

        let behind_vmd: PciAddress = "10000:e1:00.0".parse().unwrap();
        assert_eq!(behind_vmd, PciAddress::new(0x10000, 0xe1, 0, 0));
        assert_eq!(behind_vmd.to_string(), "10000:e1:00.0");

        for not in [
            "pci0000:00",
            "0000:00:1f",
            "0000:00:1f.10",
            "00:1f.0",
            "usb1",
        ] {
            assert!(not.parse::<PciAddress>().is_err(), "{not}");
        }
    }

    #[test]
    fn devices_sit_behind_their_bridges_and_are_named() {
        let machine = laptop().read();
        let nvme = machine
            .pci_device(PciAddress::new(0, 1, 0, 0))
            .expect("The drive's controller");

        assert_eq!(nvme.parent, Some(PciAddress::new(0, 0, 6, 0)));
        assert_eq!(nvme.kind(), PciKind::Storage);
        assert_eq!(nvme.name.as_deref(), Some("NVMe SSD Controller"));
        assert_eq!(
            nvme.class_name.as_deref(),
            Some("Non-Volatile memory controller")
        );
        assert_eq!(nvme.driver.as_deref(), Some("nvme"));

        let root: Vec<_> = machine.pci_behind(None).map(|d| d.address).collect();
        assert!(root.contains(&PciAddress::new(0, 0, 6, 0)));
        assert!(!root.contains(&nvme.address));
        assert_eq!(
            machine
                .pci_device(PciAddress::new(0, 0, 6, 0))
                .map(PciDevice::kind),
            Some(PciKind::Bridge)
        );
    }

    #[test]
    fn without_pci_ids_a_device_is_named_by_its_class() {
        let fake = laptop();
        std::fs::remove_file(fake.root().join("usr/share/hwdata/pci.ids")).unwrap();
        let machine = fake.read();
        let gpu = machine.pci_device(PciAddress::new(0, 0, 2, 0)).unwrap();

        assert_eq!(gpu.name, None);
        assert_eq!(gpu.vendor_name, None);
        assert_eq!(gpu.class_name.as_deref(), Some("Display controller"));
    }

    #[test]
    fn usb_devices_hang_from_their_hubs() {
        let machine = laptop().read();
        let roots: Vec<_> = machine.usb_on(None).map(|d| d.port.as_str()).collect();
        assert_eq!(roots, ["usb1", "usb2"]);

        let hub = machine.usb.iter().find(|d| d.port == "2-1").unwrap();
        assert!(hub.hub());
        assert_eq!(hub.ports, 4);

        let on_hub: Vec<_> = machine.usb_on(Some("2-1")).collect();
        assert_eq!(on_hub.len(), 1);
        // A device that leaves its class to its interfaces has theirs.
        assert_eq!(on_hub[0].class, 0x08);
        assert_eq!(on_hub[0].controller, Some(PciAddress::new(0, 0, 0x0d, 0)));

        // A root hub's maker is the kernel's release, not the hardware's.
        let root = machine.usb.iter().find(|d| d.port == "usb1").unwrap();
        assert_eq!(root.product.as_deref(), Some("xHCI Host Controller"));
        assert_eq!(root.maker, None);

        let camera = machine.usb.iter().find(|d| d.port == "1-6").unwrap();
        assert_eq!(camera.class, 0x0e);
        assert_eq!(camera.speed, Some(480.0));
        assert_eq!(camera.removable, Some(false));
    }

    #[test]
    fn a_port_is_on_the_hub_its_name_says() {
        assert_eq!(parent("usb3"), None);
        assert_eq!(parent("3-2").as_deref(), Some("usb3"));
        assert_eq!(parent("3-2.4").as_deref(), Some("3-2"));
        assert_eq!(parent("3-2.4.1").as_deref(), Some("3-2.4"));
    }

    #[test]
    fn a_device_with_no_class_is_left_out_not_guessed() {
        let fake = Fake::new();
        fake.file("sys/devices/pci0000:00/0000:00:00.0/vendor", "0x0f0f")
            .link(
                "sys/bus/pci/devices/0000:00:00.0",
                "../../../devices/pci0000:00/0000:00:00.0",
            );

        assert!(fake.read().pci.is_empty());
    }
}
