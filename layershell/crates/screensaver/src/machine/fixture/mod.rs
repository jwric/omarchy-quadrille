//! Made-up machines: what tests and committed images draw instead of the
//! machine they were made on. Each is a kind of computer the sheets must
//! read well on (a laptop, a desktop tower, a server, a virtual machine),
//! its parts generic and its live values a function of time, so a render
//! of it is the same every time.
//!
//! Each is described once, as a [`Spec`]: the machine the inventory should
//! find, and what its files hold. The tests write that as a sysfs tree and
//! read it back, which must give the fixture.
use std::collections::BTreeMap;
use std::f32::consts::TAU;

use quadrille_desktop::physical::{DisplayInput, Overrides, PhysicalSize};

use super::*;

mod desktop;
mod laptop;
mod server;
mod vm;

/// A made-up machine of a kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fixture {
    /// A laptop with an external display: what committed images show.
    Laptop,
    /// A desktop tower: a graphics card with two displays, a processor
    /// whose cores are all alike, case fans on the board's monitor, three
    /// drives, no battery.
    Desktop,
    /// A two-socket server: many cores, modules and drives, no display and
    /// no fan the kernel sees.
    Server,
    /// A virtual machine, with almost nothing.
    Vm,
}

impl Fixture {
    #[cfg(test)]
    pub const ALL: [Self; 4] = [Self::Laptop, Self::Desktop, Self::Server, Self::Vm];

    pub fn machine(self) -> Machine {
        match self {
            Self::Laptop => laptop::machine(),
            Self::Desktop => desktop::machine(),
            Self::Server => server::machine(),
            Self::Vm => vm::machine(),
        }
    }

    /// Writes the machine as a sysfs tree in `fake`, with the files of
    /// identifiers a real one has beside what is read.
    #[cfg(test)]
    pub fn tree(self, fake: &super::tests::Fake) {
        match self {
            Self::Laptop => laptop::tree(fake),
            Self::Desktop => desktop::spec().tree(fake),
            Self::Server => server::spec().tree(fake),
            Self::Vm => vm::spec().tree(fake),
        }
    }
}

/// Where a PCI function is on domain 0: bus, device and function.
type At = (u8, u8, u8);

const fn pci((bus, device, function): At) -> PciAddress {
    PciAddress::new(0, bus, device, function)
}

/// Who made every part, and the id `pci.ids` has them under.
const GENERIC: u16 = 0x0f0f;

/// What `pci.ids` calls the classes of the fixtures' functions, by base
/// class and subclass.
const CLASSES: [(u16, &str); 13] = [
    (0x0100, "SCSI storage controller"),
    (0x0106, "SATA controller"),
    (0x0107, "Serial Attached SCSI controller"),
    (0x0108, "Non-Volatile memory controller"),
    (0x0200, "Ethernet controller"),
    (0x0280, "Network controller"),
    (0x0300, "VGA compatible controller"),
    (0x0403, "Audio device"),
    (0x0600, "Host bridge"),
    (0x0601, "ISA bridge"),
    (0x0604, "PCI bridge"),
    (0x0c03, "USB controller"),
    (0x0c05, "SMBus"),
];

/// A made-up machine: what the inventory should find in it, and what its
/// files hold where that is more than the inventory keeps.
struct Spec {
    chassis: Chassis,
    /// The SMBIOS chassis type the firmware gives.
    #[cfg_attr(not(test), allow(dead_code, reason = "only the tests write the tree"))]
    smbios: u32,
    cpu: Option<Processor>,
    /// `MemTotal`, in KiB.
    memory_kib: Option<u64>,
    pci: Vec<Pci>,
    /// The USB devices on each host controller, root hubs first, each hub
    /// before what is on it.
    usb: Vec<(At, Vec<Usb>)>,
    /// The graphics cards' outputs, the machine's own panel first.
    screens: Vec<Screen>,
    /// The drives, in the order of their kernel names.
    drives: Vec<Disk>,
    /// The network interfaces, in the order of their names.
    ports: Vec<Port>,
    /// The hardware monitors, `hwmon0` first.
    chips: Vec<Chip>,
}

/// A processor: `packages` alike, each of cores of one kind or more.
struct Processor {
    model: &'static str,
    packages: u32,
    kinds: Vec<Kind>,
    /// The last cache, one a package shared by all its cores, in KiB.
    l3_kib: Option<u64>,
}

/// The cores of one kind in a package: the kind (none on a processor
/// whose cores are all alike), how many and their threads, their clocks
/// in MHz, and the caches they have: level, kind, size in KiB and the
/// cores that share one.
struct Kind {
    kind: Option<CoreKind>,
    cores: u32,
    threads: u32,
    max_mhz: Option<u32>,
    base_mhz: Option<u32>,
    caches: Vec<(u8, CacheKind, u64, u32)>,
}

/// A PCI function: where it is, the bridge it is behind, its class, its
/// device id (functions of one model share it and its name), its name and
/// the driver bound to it.
#[derive(Debug, Clone, Copy)]
struct Pci {
    at: At,
    behind: Option<At>,
    class: u32,
    device: u16,
    name: &'static str,
    driver: Option<&'static str>,
}

impl Pci {
    const fn new(at: At, class: u32, device: u16, name: &'static str) -> Self {
        Self {
            at,
            behind: None,
            class,
            device,
            name,
            driver: None,
        }
    }

    const fn behind(mut self, bridge: At) -> Self {
        self.behind = Some(bridge);
        self
    }

    const fn driver(mut self, driver: &'static str) -> Self {
        self.driver = Some(driver);
        self
    }
}

/// A USB device: its port, its speed in Mbit/s, the USB version it speaks,
/// its class, its product, a hub's ports, and whether it can be unplugged
/// where its port says.
#[derive(Debug, Clone, Copy)]
struct Usb {
    port: &'static str,
    speed: f32,
    version: &'static str,
    class: u8,
    product: Option<&'static str>,
    ports: u32,
    removable: Option<bool>,
}

impl Usb {
    const fn new(
        port: &'static str,
        speed: f32,
        version: &'static str,
        class: u8,
        product: Option<&'static str>,
    ) -> Self {
        Self {
            port,
            speed,
            version,
            class,
            product,
            ports: 0,
            removable: None,
        }
    }

    /// A bus's root hub, its host controller.
    const fn root(port: &'static str, speed: f32, version: &'static str, ports: u32) -> Self {
        Self::new(port, speed, version, 0x09, Some("xHCI Host Controller")).ports(ports)
    }

    const fn ports(mut self, ports: u32) -> Self {
        self.ports = ports;
        self
    }

    const fn removable(mut self, removable: bool) -> Self {
        self.removable = Some(removable);
        self
    }
}

/// A graphics card's output: its name, the card and the function it is
/// on, and what is plugged into it.
struct Screen {
    name: &'static str,
    #[cfg_attr(not(test), allow(dead_code, reason = "only the tests write the tree"))]
    card: u32,
    gpu: At,
    on: Shown,
}

enum Shown {
    /// Nothing.
    Open,
    /// A display that describes itself in an EDID.
    Monitor(Monitor),
    /// A display that gives no EDID, only its modes: a virtual machine's.
    Modes(u32, u32),
}

/// A display's EDID: the name it gives itself, its preferred mode and
/// refresh rate, its image size in millimetres and the year it was made.
struct Monitor {
    name: Option<&'static str>,
    pixels: (u32, u32),
    refresh: u32,
    mm: (u32, u32),
    year: u16,
}

/// A drive: its kernel name, its model, its size in 512-byte sectors, how
/// it is attached, whether it says it spins, and its partitions.
struct Disk {
    name: String,
    model: Option<&'static str>,
    sectors: u64,
    on: Attached,
    rotational: bool,
    partitions: Vec<Part>,
}

/// A partition: where it starts and its size, in sectors; the filesystem
/// mounted from it (or `swap`), and whether that is through the device
/// mapper.
type Part = (u64, u64, Option<&'static str>, bool);

/// How a drive is attached.
#[cfg_attr(not(test), allow(dead_code, reason = "only the tests write the tree"))]
enum Attached {
    /// Through its own NVMe controller, the `n`th.
    Nvme(At, u32),
    /// On port `n` of a SATA controller.
    Sata(At, u32),
    /// As target `n` of a SAS controller.
    Sas(At, u32),
    /// As the `n`th virtio device, on its function.
    Virtio(At, u32),
}

/// A network interface: its name (never read), its link, the function it
/// is on, the speed it negotiated and whether it is up.
struct Port {
    #[cfg_attr(not(test), allow(dead_code, reason = "only the tests write the tree"))]
    name: &'static str,
    link: Link,
    at: At,
    speed: Option<u32>,
    up: bool,
}

/// A hardware monitor: its chip's name, what it is on, where the inventory
/// places what it measures (nowhere for a battery's, which is read as a
/// battery), and its channels.
struct Chip {
    name: &'static str,
    on: On,
    site: Option<Site>,
    channels: Vec<Channel>,
}

/// What a monitor is on.
#[cfg_attr(not(test), allow(dead_code, reason = "only the tests write the tree"))]
enum On {
    /// No device: an ACPI thermal zone, a wireless adapter's monitor.
    Nothing,
    /// A device under `sys/devices`: `platform/coretemp.0`.
    Device(&'static str),
    /// A PCI function.
    Pci(At),
    /// The embedded controller, behind the LPC bridge at `At`.
    Ec(At),
    /// A memory module on bus `bus` of the SMBus controller at `smbus`,
    /// at `address`, and its generation.
    Module {
        smbus: At,
        bus: u8,
        address: u16,
        generation: Option<&'static str>,
    },
    /// Drive `n` of [`Spec::drives`]: its NVMe controller, or its SCSI
    /// device.
    Drive(usize),
    /// The power supply of that name.
    Supply(&'static str),
}

/// One of a monitor's channels: `temp1`, `fan2`, `in0`; what it holds
/// when the tree is read, its label, and whether the inventory reads it
/// (not a voltage, and not an input with nothing wired to it).
struct Channel {
    name: String,
    value: i64,
    label: Option<String>,
    read: bool,
    /// A temperature's high limit, in millidegrees.
    max: Option<i64>,
}

impl Chip {
    fn new(name: &'static str, on: On, site: Site) -> Self {
        Self {
            name,
            on,
            site: Some(site),
            channels: Vec::new(),
        }
    }

    /// A power supply's monitor, which the inventory leaves to the
    /// batteries.
    fn supply(name: &'static str, supply: &'static str) -> Self {
        Self {
            name,
            on: On::Supply(supply),
            site: None,
            channels: Vec::new(),
        }
    }

    fn channel(mut self, name: String, value: i64, label: Option<&str>, read: bool) -> Self {
        self.channels.push(Channel {
            name,
            value,
            label: label.map(Into::into),
            read,
            max: None,
        });
        self
    }

    /// The last channel's high limit, in millidegrees.
    fn max(mut self, millidegrees: i64) -> Self {
        if let Some(channel) = self.channels.last_mut() {
            channel.max = Some(millidegrees);
        }
        self
    }

    /// Temperature `n`, in millidegrees.
    fn temp(self, n: u32, millidegrees: i64, label: Option<&str>) -> Self {
        self.channel(format!("temp{n}"), millidegrees, label, true)
    }

    /// Fan `n`, in revolutions a minute.
    fn fan(self, n: u32, rpm: i64, label: Option<&str>) -> Self {
        self.channel(format!("fan{n}"), rpm, label, true)
    }

    /// A temperature input with nothing wired to it, which reads nonsense
    /// and is left out.
    fn unwired(self, n: u32, millidegrees: i64, label: Option<&str>) -> Self {
        self.channel(format!("temp{n}"), millidegrees, label, false)
    }

    /// A channel that is not read: a voltage, a current, a power.
    fn other(self, name: &str, value: i64) -> Self {
        self.channel(name.into(), value, None, false)
    }

    /// The channels the inventory reads, in its order: temperatures, then
    /// fans, each by number; with their kinds and numbers.
    fn read(&self) -> Vec<(SensorKind, u32, &Channel)> {
        let mut read: Vec<(SensorKind, u32, &Channel)> = self
            .channels
            .iter()
            .filter(|channel| channel.read)
            .filter_map(|channel| {
                let (kind, n) = match channel.name.strip_prefix("temp") {
                    Some(n) => (SensorKind::Temperature, n),
                    None => (SensorKind::Fan, channel.name.strip_prefix("fan")?),
                };
                Some((kind, n.parse().ok()?, channel))
            })
            .collect();

        read.sort_by_key(|&(kind, n, _)| (kind == SensorKind::Fan, n));
        read
    }
}

impl Spec {
    /// The machine, its live values from `sample`.
    fn machine(&self, sample: fn(f32) -> Snapshot) -> Machine {
        let modules: Vec<Module> = self
            .chips
            .iter()
            .filter_map(|chip| match chip.on {
                On::Module { generation, .. } => Some(Module {
                    generation: generation.map(Into::into),
                }),
                _ => None,
            })
            .collect();

        Machine {
            chassis: self.chassis.clone(),
            cpu: self.cpu.as_ref().map(Processor::cpu),
            memory: (self.memory_kib.is_some() || !modules.is_empty()).then(|| Memory {
                bytes: self.memory_kib.map(|kib| kib * 1024),
                modules,
            }),
            drives: self.drives.iter().map(Disk::drive).collect(),
            pci: self.pci.iter().map(Pci::device).collect(),
            usb: self
                .usb
                .iter()
                .flat_map(|&(controller, ref devices)| {
                    devices.iter().map(move |usb| usb.device(controller))
                })
                .collect(),
            connectors: self.screens.iter().map(Screen::connector).collect(),
            batteries: Vec::new(),
            chargers: Vec::new(),
            interfaces: self
                .ports
                .iter()
                .map(|port| Interface {
                    link: port.link,
                    driver: self.function(port.at).driver.map(Into::into),
                    pci: Some(pci(port.at)),
                    usb: false,
                    speed: port.speed,
                    up: port.up,
                })
                .collect(),
            sensors: self
                .chips
                .iter()
                .filter_map(|chip| Some((chip, chip.site?)))
                .flat_map(|(chip, site)| {
                    chip.read()
                        .into_iter()
                        .map(move |(kind, channel, read)| Sensor {
                            kind,
                            chip: chip.name.into(),
                            channel,
                            label: read.label.clone(),
                            limit: read.max.map(|max| max as f32 / 1000.0),
                            site,
                        })
                })
                .collect(),
            sampler: Sampler::made(sample),
        }
    }

    /// Values that move as a machine's do, about what its files held when
    /// it was read: the processor and graphics warm and cool over about
    /// forty seconds with the turning fans following them, the rest drift
    /// a little, and the drives and the network carry bursts.
    fn snapshot(&self, t: f32) -> Snapshot {
        let wave = |period: f32, phase: f32| (TAU * t / period + phase).sin();
        // Up to one, and zero for most of each period.
        let burst = |period: f32, phase: f32| wave(period, phase).max(0.0).powi(4);
        let load = 0.5 + 0.5 * wave(37.0, 0.0);
        let sensors = self
            .chips
            .iter()
            .filter_map(|chip| Some((chip, chip.site?)))
            .flat_map(|(chip, site)| chip.read().into_iter().map(move |read| (site, read)))
            .enumerate()
            .map(|(k, (site, (kind, _, channel)))| {
                let k = k as f32;
                let value = channel.value as f32;
                let busy = match site {
                    Site::Processor(_) => true,
                    Site::Device(at) => self
                        .pci
                        .iter()
                        .any(|function| pci(function.at) == at && function.class >> 16 == 0x03),
                    _ => false,
                };

                Some(match kind {
                    SensorKind::Temperature if busy => {
                        value / 1e3 + 16.0 * load - 6.0 + 1.5 * wave(4.7, k)
                    }
                    SensorKind::Temperature => value / 1e3 + 1.5 * wave(53.0 + 7.0 * k, k),
                    // A fan that stood still stays still: an empty header.
                    SensorKind::Fan => value * (0.8 + 0.4 * load),
                })
            })
            .collect();

        Snapshot {
            sensors,
            batteries: Vec::new(),
            chargers: Vec::new(),
            drives: (0..self.drives.len())
                .map(|k| {
                    let k = k as f32;
                    Some(Transfer {
                        read: 3.0e6 * burst(7.0 + 2.0 * k, k),
                        written: 1.2e6 * burst(11.0 + 3.0 * k, 2.0 + k),
                    })
                })
                .collect(),
            interfaces: self
                .ports
                .iter()
                .enumerate()
                .map(|(k, port)| {
                    let (k, up) = (k as f32, if port.up { 1.0 } else { 0.0 });
                    Some(Traffic {
                        received: up * 2.0e6 * (0.5 + 0.5 * wave(9.0 + k, k)),
                        sent: up * 2.5e5 * (0.5 + 0.5 * wave(5.0 + k, 1.0 + k)),
                    })
                })
                .collect(),
        }
    }

    /// The PCI function at `at`.
    fn function(&self, at: At) -> &Pci {
        self.pci
            .iter()
            .find(|function| function.at == at)
            .expect("A function of the machine")
    }
}

impl Processor {
    fn cpu(&self) -> Cpu {
        let packages = self.packages;
        let mut caches: BTreeMap<(u8, CacheKind, u64), u32> = BTreeMap::new();

        for kind in &self.kinds {
            for &(level, cache, kib, shared) in &kind.caches {
                *caches.entry((level, cache, kib << 10)).or_default() +=
                    packages * kind.cores.div_ceil(shared);
            }
        }
        if let Some(kib) = self.l3_kib {
            *caches
                .entry((3, CacheKind::Unified, kib << 10))
                .or_default() += packages;
        }

        Cpu {
            model: Some(self.model.into()),
            packages,
            cores: packages * self.kinds.iter().map(|kind| kind.cores).sum::<u32>(),
            threads: packages
                * self
                    .kinds
                    .iter()
                    .map(|kind| kind.cores * kind.threads)
                    .sum::<u32>(),
            kinds: self
                .kinds
                .iter()
                .filter_map(|kind| {
                    Some(Cores {
                        kind: kind.kind?,
                        cores: packages * kind.cores,
                        threads: packages * kind.cores * kind.threads,
                        max_mhz: kind.max_mhz,
                    })
                })
                .collect(),
            caches: caches
                .into_iter()
                .map(|((level, kind, bytes), instances)| Cache {
                    level,
                    kind,
                    bytes,
                    instances,
                })
                .collect(),
            base_mhz: self.kinds.first().and_then(|kind| kind.base_mhz),
            max_mhz: self.kinds.iter().filter_map(|kind| kind.max_mhz).max(),
        }
    }
}

impl Pci {
    fn device(&self) -> PciDevice {
        let subclass = (self.class >> 8) as u16;

        PciDevice {
            address: pci(self.at),
            parent: self.behind.map(pci),
            class: self.class,
            vendor: GENERIC,
            device: self.device,
            vendor_name: Some("Generic".into()),
            name: Some(self.name.into()),
            class_name: CLASSES
                .iter()
                .find(|(code, _)| *code == subclass)
                .map(|(_, name)| (*name).into()),
            driver: self.driver.map(Into::into),
        }
    }
}

impl Usb {
    fn device(&self, controller: At) -> UsbDevice {
        UsbDevice {
            port: self.port.into(),
            parent: buses::parent(self.port),
            controller: Some(pci(controller)),
            speed: Some(self.speed),
            version: Some(self.version.into()),
            class: self.class,
            ports: self.ports,
            product: self.product.map(Into::into),
            maker: None,
            removable: self.removable,
        }
    }
}

impl Screen {
    fn connector(&self) -> Connector {
        let size = |make: &str, model: &str, (width, height): (u32, u32), mm: (u32, u32)| {
            PhysicalSize::resolve(
                &DisplayInput {
                    name: self.name.into(),
                    make: make.into(),
                    model: model.into(),
                    width,
                    height,
                    physical_width: f64::from(mm.0),
                    physical_height: f64::from(mm.1),
                    transform: 0,
                    scale: 1.0,
                },
                &Overrides::default(),
            )
        };

        Connector {
            name: self.name.into(),
            kind: ConnectorKind::of(self.name),
            gpu: Some(pci(self.gpu)),
            panel: match &self.on {
                Shown::Open => None,
                Shown::Monitor(monitor) => Some(Panel {
                    maker: Some("GEN".into()),
                    name: monitor.name.map(Into::into),
                    pixels: monitor.pixels,
                    refresh: Some(monitor.refresh as f32),
                    year: Some(monitor.year),
                    size: size(
                        "GEN",
                        monitor.name.unwrap_or_default(),
                        monitor.pixels,
                        monitor.mm,
                    ),
                }),
                &Shown::Modes(width, height) => Some(Panel {
                    maker: None,
                    name: None,
                    pixels: (width, height),
                    refresh: None,
                    year: None,
                    size: size("", "", (width, height), (0, 0)),
                }),
            },
        }
    }
}

impl Disk {
    fn drive(&self) -> Drive {
        let (kind, at) = match self.on {
            Attached::Nvme(at, _) => (DriveKind::Nvme, at),
            Attached::Sata(at, _) | Attached::Sas(at, _) => (DriveKind::Sata, at),
            Attached::Virtio(at, _) => (DriveKind::Virtual, at),
        };

        Drive {
            name: self.name.clone(),
            model: self.model.map(Into::into),
            bytes: self.sectors * 512,
            kind,
            rotational: self.rotational,
            removable: false,
            pci: Some(pci(at)),
            partitions: self
                .partitions
                .iter()
                .enumerate()
                .map(|(k, &(start, sectors, filesystem, mapped))| Partition {
                    number: k as u32 + 1,
                    start: start * 512,
                    bytes: sectors * 512,
                    filesystem: filesystem.map(Into::into),
                    mapped,
                })
                .collect(),
        }
    }
}

/// A GPT disk of `sectors` with an EFI system partition of a gigabyte and
/// the rest `filesystem`.
fn efi_and(sectors: u64, filesystem: &'static str) -> Vec<Part> {
    vec![
        (2048, 2_097_152, Some("vfat"), false),
        (
            2_099_200,
            sectors - 2_099_200 - 2048,
            Some(filesystem),
            false,
        ),
    ]
}

/// A disk of `sectors` with one partition, all of it `filesystem`.
fn whole(sectors: u64, filesystem: Option<&'static str>) -> Vec<Part> {
    vec![(2048, sectors - 4096, filesystem, false)]
}

/// The CPUs `first` to `last` as sysfs lists them: `16-19`, `3`.
#[cfg(test)]
fn span(first: u32, last: u32) -> String {
    if first == last {
        first.to_string()
    } else {
        format!("{first}-{last}")
    }
}

#[cfg(test)]
impl Spec {
    /// Writes the machine as a sysfs tree in `fake`, with the files of
    /// identifiers a real one has beside what is read.
    fn tree(&self, fake: &super::tests::Fake) {
        self.firmware(fake);
        self.processor(fake);
        self.buses(fake);
        self.displays(fake);
        self.storage(fake);
        self.network(fake);
        self.monitors(fake);
    }

    /// Where function `at` is under `sys/devices`: under its root complex,
    /// behind its bridges.
    fn place(&self, at: At) -> String {
        match self.function(at).behind {
            Some(bridge) => format!("{}/{}", self.place(bridge), pci(at)),
            None => format!("pci0000:{:02x}/{}", at.0, pci(at)),
        }
    }

    /// The device a drive's monitor is on: its NVMe controller, its SCSI
    /// device or its virtio device.
    fn drive_device(&self, disk: &Disk) -> String {
        match disk.on {
            Attached::Nvme(at, n) => format!("sys/devices/{}/nvme/nvme{n}", self.place(at)),
            Attached::Sata(at, port) => format!(
                "sys/devices/{}/ata{}/host{port}/target{port}:0:0/{port}:0:0:0",
                self.place(at),
                port + 1
            ),
            Attached::Sas(at, n) => format!(
                "sys/devices/{}/host0/port-0:{n}/end_device-0:{n}/target0:0:{n}/0:0:{n}:0",
                self.place(at)
            ),
            Attached::Virtio(at, n) => format!("sys/devices/{}/virtio{n}", self.place(at)),
        }
    }

    fn firmware(&self, fake: &super::tests::Fake) {
        use super::tests::IDENTIFIERS;

        let dmi = "sys/class/dmi/id";
        let chassis = &self.chassis;

        fake.file(&format!("{dmi}/chassis_type"), format!("{}\n", self.smbios));

        for (name, value) in [
            ("sys_vendor", &chassis.vendor),
            ("product_name", &chassis.product),
            ("board_vendor", &chassis.board_vendor),
            ("board_name", &chassis.board),
            ("bios_version", &chassis.bios),
        ] {
            if let Some(value) = value {
                fake.file(&format!("{dmi}/{name}"), format!("{value}\n"));
            }
        }

        fake.file(&format!("{dmi}/product_serial"), IDENTIFIERS[0])
            .file(&format!("{dmi}/product_uuid"), IDENTIFIERS[1]);
    }

    /// Each package's cores of each kind in turn, a core's threads
    /// numbered together, as a hybrid laptop numbers them.
    fn processor(&self, fake: &super::tests::Fake) {
        let system = "sys/devices/system/cpu";
        let Some(processor) = &self.cpu else {
            return;
        };
        let per_package: u32 = processor
            .kinds
            .iter()
            .map(|kind| kind.cores * kind.threads)
            .sum();
        let mut info = String::new();
        let mut by_kind: Vec<(CoreKind, Vec<String>)> = Vec::new();
        let mut cpu = 0;

        for package in 0..processor.packages {
            let l3 = span(cpu, cpu + per_package - 1);
            let mut core_id = 0;

            for kind in &processor.kinds {
                let first = cpu;
                let last = first + kind.cores * kind.threads - 1;

                if let Some(core_kind) = kind.kind {
                    match by_kind.iter_mut().find(|(k, _)| *k == core_kind) {
                        Some((_, lists)) => lists.push(span(first, last)),
                        None => by_kind.push((core_kind, vec![span(first, last)])),
                    }
                }

                for core in 0..kind.cores {
                    for _ in 0..kind.threads {
                        let at = format!("{system}/cpu{cpu}");
                        let caches = kind
                            .caches
                            .iter()
                            .map(|&(level, cache, kib, shared)| {
                                let group = core / shared;
                                let from = first + group * shared * kind.threads;
                                let to = first
                                    + ((group + 1) * shared).min(kind.cores) * kind.threads
                                    - 1;
                                (level, cache, kib, span(from, to))
                            })
                            .chain(
                                processor
                                    .l3_kib
                                    .map(|kib| (3, CacheKind::Unified, kib, l3.clone())),
                            );

                        fake.file(
                            &format!("{at}/topology/physical_package_id"),
                            format!("{package}\n"),
                        )
                        .file(&format!("{at}/topology/core_id"), format!("{core_id}\n"));

                        if let Some(mhz) = kind.max_mhz {
                            fake.file(
                                &format!("{at}/cpufreq/cpuinfo_max_freq"),
                                format!("{}\n", mhz * 1000),
                            );
                        }
                        if let Some(mhz) = kind.base_mhz {
                            fake.file(
                                &format!("{at}/cpufreq/base_frequency"),
                                format!("{}\n", mhz * 1000),
                            );
                        }

                        for (index, (level, cache, kib, shared)) in caches.enumerate() {
                            let at = format!("{at}/cache/index{index}");
                            let cache = match cache {
                                CacheKind::Data => "Data",
                                CacheKind::Instruction => "Instruction",
                                CacheKind::Unified => "Unified",
                            };

                            fake.file(&format!("{at}/level"), format!("{level}\n"))
                                .file(&format!("{at}/type"), format!("{cache}\n"))
                                .file(&format!("{at}/size"), format!("{kib}K\n"))
                                .file(&format!("{at}/shared_cpu_list"), format!("{shared}\n"));
                        }

                        info +=
                            &format!("processor\t: {cpu}\nmodel name\t: {}\n\n", processor.model);
                        cpu += 1;
                    }

                    core_id += 1;
                }
            }
        }

        for (kind, lists) in by_kind {
            let name = match kind {
                CoreKind::Performance => "cpu_core",
                CoreKind::Efficient => "cpu_atom",
            };
            fake.file(
                &format!("sys/devices/{name}/cpus"),
                format!("{}\n", lists.join(",")),
            );
        }

        fake.file("proc/cpuinfo", info)
            .dir(&format!("{system}/cpufreq"));

        if let Some(kib) = self.memory_kib {
            fake.file(
                "proc/meminfo",
                format!("MemTotal:       {kib} kB\nMemFree:         1000 kB\n"),
            );
        }
    }

    /// The PCI functions at their places under the root complexes, named
    /// in a `pci.ids` of the fixtures' own; the USB devices under their
    /// host controllers.
    fn buses(&self, fake: &super::tests::Fake) {
        use super::tests::IDENTIFIERS;

        let mut ids = format!("# The fixture's pci.ids\n{GENERIC:04x}  Generic\n");
        let mut named = Vec::new();

        for function in &self.pci {
            let at = format!("sys/devices/{}", self.place(function.at));

            fake.file(
                &format!("{at}/class"),
                format!("0x{:06x}\n", function.class),
            )
            .file(&format!("{at}/vendor"), format!("0x{GENERIC:04x}\n"))
            .file(
                &format!("{at}/device"),
                format!("0x{:04x}\n", function.device),
            )
            .points(&format!("sys/bus/pci/devices/{}", pci(function.at)), &at);

            if let Some(driver) = function.driver {
                fake.points(
                    &format!("{at}/driver"),
                    &format!("sys/bus/pci/drivers/{driver}"),
                );
            }
            if !named.contains(&function.device) {
                ids += &format!(
                    "\t{:04x}  {name}\n\t\t{GENERIC:04x} 0001  {name} (subsystem)\n",
                    function.device,
                    name = function.name
                );
                named.push(function.device);
            }
        }

        ids += "0f10  Someone Else\n\t0001  Not This One\n";
        let mut base = None;

        for (code, name) in CLASSES {
            let class = (code >> 8) as u8;

            if base != Some(class) {
                let base_name = buses::base_class(class).expect("A class pci.ids names");
                ids += &format!("C {class:02x}  {base_name}\n");
                base = Some(class);
            }
            ids += &format!("\t{:02x}  {name}\n", code & 0xff);
        }

        fake.file("usr/share/hwdata/pci.ids", ids);

        for &(controller, ref devices) in &self.usb {
            let host = format!("sys/devices/{}", self.place(controller));

            for usb in devices {
                let mut path = Vec::new();
                let mut step = Some(usb.port.to_owned());
                while let Some(port) = step {
                    step = buses::parent(&port);
                    path.insert(0, port);
                }
                let at = format!("{host}/{}", path.join("/"));
                let port = usb.port;
                // A hub and a wireless controller say what they are; other
                // devices leave it to their interfaces.
                let own_class = if matches!(usb.class, 0x09 | 0xe0) {
                    usb.class
                } else {
                    0
                };

                fake.file(&format!("{at}/speed"), format!("{}\n", usb.speed))
                    .file(&format!("{at}/version"), format!(" {}\n", usb.version))
                    .file(&format!("{at}/bDeviceClass"), format!("{own_class:02x}\n"))
                    .file(&format!("{at}/maxchild"), format!("{}\n", usb.ports))
                    .file(
                        &format!("{at}/removable"),
                        match usb.removable {
                            Some(true) => "removable\n",
                            Some(false) => "fixed\n",
                            None => "unknown\n",
                        },
                    )
                    .points(&format!("sys/bus/usb/devices/{port}"), &at);

                if let Some(product) = usb.product {
                    fake.file(&format!("{at}/product"), format!("{product}\n"));
                }
                if port.starts_with("usb") {
                    fake.file(&format!("{at}/manufacturer"), "Linux 6.0.0-host xhci-hcd\n");
                } else if usb.class != 0x09 {
                    fake.file(&format!("{at}/serial"), IDENTIFIERS[8]);
                }
                if own_class == 0 {
                    let interface = format!("{at}/{port}:1.0");

                    fake.file(
                        &format!("{interface}/bInterfaceClass"),
                        format!("{:02x}\n", usb.class),
                    )
                    .points(&format!("sys/bus/usb/devices/{port}:1.0"), &interface);
                }
            }
        }
    }

    /// The graphics cards' outputs, with the EDIDs of what is on them.
    fn displays(&self, fake: &super::tests::Fake) {
        use super::displays::tests::edid;

        let mut cards = Vec::new();

        for screen in &self.screens {
            let card = format!(
                "sys/devices/{}/drm/card{}",
                self.place(screen.gpu),
                screen.card
            );
            let name = format!("card{}-{}", screen.card, screen.name);
            let at = format!("{card}/{name}");

            if !cards.contains(&screen.card) {
                fake.points(&format!("sys/class/drm/card{}", screen.card), &card);
                cards.push(screen.card);
            }

            let (status, modes, edid) = match &screen.on {
                Shown::Open => ("disconnected", String::new(), Vec::new()),
                Shown::Monitor(monitor) => {
                    let (width, height) = monitor.pixels;
                    (
                        "connected",
                        format!("{width}x{height}\n1920x1080\n"),
                        edid(
                            "GEN",
                            monitor.name,
                            (width, height, monitor.refresh),
                            monitor.mm,
                            monitor.year,
                        ),
                    )
                }
                Shown::Modes(width, height) => {
                    ("connected", format!("{width}x{height}\n"), Vec::new())
                }
            };

            fake.file(&format!("{at}/status"), format!("{status}\n"))
                .file(&format!("{at}/modes"), modes)
                .file(&format!("{at}/edid"), edid)
                .points(&format!("sys/class/drm/{name}"), &at);
        }

        if !cards.is_empty() {
            fake.file("sys/class/drm/version", "drm 1.1.0\n");
        }
    }

    /// The drives with their partitions, the volumes mapped on them and
    /// what is mounted from those; a swap device that is no drive.
    fn storage(&self, fake: &super::tests::Fake) {
        use super::tests::IDENTIFIERS;

        let mut mounts = String::new();
        let mut swaps = String::from("Filename\t\t\t\tType\t\tSize\t\tUsed\t\tPriority\n");
        let mut volumes = 0;
        let mut rooted = false;

        for disk in &self.drives {
            let name = &disk.name;
            let device = self.drive_device(disk);
            let at = match disk.on {
                Attached::Nvme(..) => format!("{device}/{name}"),
                _ => format!("{device}/block/{name}"),
            };

            match (&disk.on, disk.model) {
                (Attached::Nvme(..), model) => {
                    fake.file(
                        &format!("{device}/model"),
                        format!("{:<40}\n", model.unwrap_or_default()),
                    )
                    .file(&format!("{device}/serial"), IDENTIFIERS[2])
                    .file(&format!("{device}/subsysnqn"), IDENTIFIERS[4]);
                }
                (_, Some(model)) => {
                    fake.file(&format!("{device}/model"), format!("{model:<16}\n"))
                        .file(&format!("{device}/vpd_pg80"), IDENTIFIERS[2]);
                }
                (_, None) => {
                    fake.dir(&device);
                }
            }

            fake.file(&format!("{at}/size"), format!("{}\n", disk.sectors))
                .file(&format!("{at}/removable"), "0\n")
                .file(
                    &format!("{at}/queue/rotational"),
                    if disk.rotational { "1\n" } else { "0\n" },
                )
                .file(&format!("{at}/stat"), "1000 0 2048 0 500 0 2000 0 0 0 0\n")
                .file(&format!("{at}/eui"), IDENTIFIERS[3])
                .file(&format!("{at}/wwid"), IDENTIFIERS[3])
                .points(&format!("{at}/device"), &device)
                .points(&format!("sys/block/{name}"), &at);

            let separator = if name.ends_with(|c: char| c.is_ascii_digit()) {
                "p"
            } else {
                ""
            };

            for (k, &(start, sectors, filesystem, mapped)) in disk.partitions.iter().enumerate() {
                let partition = format!("{name}{separator}{}", k + 1);
                let part = format!("{at}/{partition}");

                fake.file(&format!("{part}/partition"), format!("{}\n", k + 1))
                    .file(&format!("{part}/start"), format!("{start}\n"))
                    .file(&format!("{part}/size"), format!("{sectors}\n"));

                let source = if mapped {
                    let dm = format!("dm-{volumes}");
                    let volume = format!("sys/devices/virtual/block/{dm}");

                    fake.file(&format!("{volume}/size"), format!("{}\n", sectors - 32_768))
                        .file(&format!("{volume}/dm/name"), format!("volume{volumes}\n"))
                        .file(&format!("{volume}/dm/uuid"), "CRYPT-LUKS2-UUID-0000-1111\n")
                        .points(&format!("sys/block/{dm}"), &volume)
                        .points(&format!("{part}/holders/{dm}"), &volume)
                        .points(&format!("dev/mapper/volume{volumes}"), &format!("dev/{dm}"));
                    volumes += 1;
                    format!("/dev/mapper/volume{}", volumes - 1)
                } else {
                    format!("/dev/{partition}")
                };

                match filesystem {
                    Some("swap") => {
                        swaps += &format!("{source}  partition\t{}\t0\t-2\n", sectors / 2);
                    }
                    Some(filesystem) => {
                        let point = match filesystem {
                            "vfat" => "/boot".to_owned(),
                            _ if !rooted => {
                                rooted = true;
                                "/".to_owned()
                            }
                            _ => format!("/mnt/{partition}"),
                        };
                        mounts += &format!("{source} {point} {filesystem} rw,relatime 0 0\n");
                    }
                    None => {}
                }
            }
        }

        mounts += "tmpfs /tmp tmpfs rw 0 0\nproc /proc proc rw 0 0\n";
        swaps += "/dev/zram0  partition\t8388604\t0\t100\n";
        fake.file("sys/devices/virtual/block/zram0/size", "16777216\n")
            .points("sys/block/zram0", "sys/devices/virtual/block/zram0")
            .file("proc/mounts", mounts)
            .file("proc/swaps", swaps);
    }

    /// The interfaces on their adapters, and two with no hardware: the
    /// loopback and a bridge.
    fn network(&self, fake: &super::tests::Fake) {
        use super::tests::IDENTIFIERS;

        for (k, port) in self.ports.iter().enumerate() {
            let device = format!("sys/devices/{}", self.place(port.at));
            let at = format!("{device}/net/{}", port.name);

            fake.file(&format!("{at}/type"), "1\n")
                .file(
                    &format!("{at}/operstate"),
                    if port.up { "up\n" } else { "down\n" },
                )
                .file(&format!("{at}/statistics/rx_bytes"), "1000000\n")
                .file(&format!("{at}/statistics/tx_bytes"), "200000\n")
                .file(&format!("{at}/address"), IDENTIFIERS[5 + k % 2])
                .points(&format!("{at}/device"), &device)
                .points(&format!("sys/class/net/{}", port.name), &at);

            match (port.link, port.speed) {
                (Link::Wireless, _) => {
                    fake.dir(&format!("{at}/wireless"));
                }
                (_, Some(speed)) => {
                    fake.file(&format!("{at}/speed"), format!("{speed}\n"));
                }
                // Unplugged, or a virtual adapter's.
                (_, None) => {
                    fake.file(&format!("{at}/speed"), "-1\n");
                }
            }
        }

        for (name, kind) in [("lo", "772"), ("docker0", "1")] {
            let at = format!("sys/devices/virtual/net/{name}");

            fake.file(&format!("{at}/type"), format!("{kind}\n"))
                .file(&format!("{at}/operstate"), "unknown\n")
                .points(&format!("sys/class/net/{name}"), &at);
        }
    }

    /// The hardware monitors, each on its device.
    fn monitors(&self, fake: &super::tests::Fake) {
        for (n, chip) in self.chips.iter().enumerate() {
            let at = format!("sys/class/hwmon/hwmon{n}");
            let device = match chip.on {
                On::Nothing => None,
                On::Device(path) => Some(format!("sys/devices/{path}")),
                On::Pci(function) => Some(format!("sys/devices/{}", self.place(function))),
                On::Ec(lpc) => Some(format!("sys/devices/{}/PNP0C09:00", self.place(lpc))),
                On::Module {
                    smbus,
                    bus,
                    address,
                    ..
                } => Some(format!(
                    "sys/devices/{}/i2c-{bus}/{bus}-{address:04x}",
                    self.place(smbus)
                )),
                On::Drive(drive) => Some(self.drive_device(&self.drives[drive])),
                On::Supply(supply) => Some(format!("sys/class/power_supply/{supply}")),
            };

            fake.file(&format!("{at}/name"), format!("{}\n", chip.name));

            if let Some(device) = device {
                fake.dir(&device).points(&format!("{at}/device"), &device);
            }

            for channel in &chip.channels {
                fake.file(
                    &format!("{at}/{}_input", channel.name),
                    format!("{}\n", channel.value),
                );
                if let Some(label) = &channel.label {
                    fake.file(
                        &format!("{at}/{}_label", channel.name),
                        format!("{label}\n"),
                    );
                }
                if let Some(max) = channel.max {
                    fake.file(&format!("{at}/{}_max", channel.name), format!("{max}\n"));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Fake, IDENTIFIERS};
    use super::*;

    /// Each fixture is what reading its tree gives: every part of the
    /// inventory read from a whole machine.
    #[test]
    fn each_fixture_is_its_tree_read() {
        for fixture in Fixture::ALL {
            let fake = Fake::new();
            fixture.tree(&fake);

            let read = format!("{:#?}", fake.read());
            let made = format!("{:#?}", fixture.machine());

            for (line, (read, made)) in read.lines().zip(made.lines()).enumerate() {
                assert_eq!(read, made, "{fixture:?}, line {}", line + 1);
            }
            assert_eq!(read.lines().count(), made.lines().count(), "{fixture:?}");
        }
    }

    /// The files of identifiers each tree has beside what is read reach
    /// neither the inventory nor a sample of it.
    #[test]
    fn no_fixtures_identifiers_are_read() {
        for fixture in Fixture::ALL {
            let fake = Fake::new();
            fixture.tree(&fake);

            let machine = fake.read();
            let seen = format!("{machine:?}\n{:?}", machine.sample(0.0));

            for secret in IDENTIFIERS {
                assert!(!seen.contains(secret), "{fixture:?}: {secret} was read");
            }
        }
    }

    #[test]
    fn the_fixtures_values_are_a_function_of_time_for_each_of_their_parts() {
        for fixture in Fixture::ALL {
            let machine = fixture.machine();

            for t in [0.0, 0.37, 5.0, 61.3, 1234.5] {
                let snapshot = machine.sample(t);

                assert_eq!(snapshot, machine.sample(t));
                assert_eq!(snapshot.sensors.len(), machine.sensors.len());
                assert_eq!(snapshot.batteries.len(), machine.batteries.len());
                assert_eq!(snapshot.chargers.len(), machine.chargers.len());
                assert_eq!(snapshot.drives.len(), machine.drives.len());
                assert_eq!(snapshot.interfaces.len(), machine.interfaces.len());

                for (index, value) in snapshot.sensors.iter().enumerate() {
                    let value = value.expect("Every sensor reads");
                    let sensor = &machine.sensors[index];
                    let fits = match sensor.kind {
                        // Turning, or an empty header.
                        SensorKind::Fan => value == 0.0 || (500.0..4000.0).contains(&value),
                        SensorKind::Temperature => (15.0..95.0).contains(&value),
                    };

                    assert!(
                        fits && sensor.kind.plausible(value),
                        "{fixture:?}: sensor {index} at {t}: {value}"
                    );
                }

                for charge in &snapshot.batteries {
                    assert!((0.05..=0.8).contains(&charge.fraction.unwrap()));
                }
            }

            assert_ne!(machine.sample(0.0), machine.sample(10.0), "{fixture:?}");
        }
    }

    /// Every device a part hangs from is on the bus, and every bridge a
    /// device is behind.
    #[test]
    fn every_fixtures_parts_hang_from_its_bus() {
        for fixture in Fixture::ALL {
            let machine = fixture.machine();

            for address in machine
                .drives
                .iter()
                .filter_map(|d| d.pci)
                .chain(machine.interfaces.iter().filter_map(|i| i.pci))
                .chain(machine.connectors.iter().filter_map(|c| c.gpu))
                .chain(machine.usb.iter().filter_map(|u| u.controller))
                .chain(machine.pci.iter().filter_map(|d| d.parent))
                .chain(machine.sensors.iter().filter_map(|s| match s.site {
                    Site::Device(address) => Some(address),
                    _ => None,
                }))
            {
                assert!(
                    machine.pci_device(address).is_some(),
                    "{fixture:?}: {address}"
                );
            }

            for sensor in &machine.sensors {
                match sensor.site {
                    Site::Drive(drive) => assert!(drive < machine.drives.len()),
                    Site::Module(module) => {
                        assert!(module < machine.memory.as_ref().map_or(0, |m| m.modules.len()))
                    }
                    Site::Processor(package) => {
                        assert!(package < machine.cpu.as_ref().unwrap().packages as usize)
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn the_laptop_has_what_the_sheets_draw() {
        let machine = Machine::fixture();

        assert_eq!(machine.chassis.kind, ChassisKind::Laptop);
        assert_eq!(machine.displays().count(), 2);
        assert_eq!(machine.fans().count(), 2);
        assert_eq!(machine.drives[0].kind, DriveKind::Nvme);

        let links: Vec<_> = machine.interfaces.iter().map(|i| i.link).collect();
        assert_eq!(links, [Link::Ethernet, Link::Wireless]);
    }

    /// The desktop: a processor whose cores are all alike, a graphics card
    /// with a 4K display and another, four case fans turning on the
    /// board's monitor and three headers empty, an NVMe drive, a SATA SSD
    /// and a hard disk, and no battery.
    #[test]
    fn the_desktop_has_what_a_tower_has() {
        let machine = Fixture::Desktop.machine();
        let cpu = machine.cpu.as_ref().unwrap();

        assert_eq!(machine.chassis.kind, ChassisKind::Desktop);
        assert!(cpu.kinds.is_empty() && cpu.packages == 1);

        let displays: Vec<_> = machine.displays().map(|(_, panel)| panel.pixels).collect();
        assert_eq!(displays, [(3840, 2160), (1920, 1080)]);
        assert!(machine.displays().all(|(connector, _)| {
            machine
                .pci_device(connector.gpu.unwrap())
                .unwrap()
                .parent
                .is_some()
        }));

        let first = machine.sample(0.0);
        let board: Vec<f32> = machine
            .fans()
            .filter(|(_, fan)| fan.site == Site::Board)
            .map(|(index, _)| first.sensor(index).unwrap())
            .collect();
        assert_eq!(board.len(), 7);
        assert_eq!(board.iter().filter(|rpm| **rpm > 0.0).count(), 4);

        let kinds: Vec<_> = machine
            .drives
            .iter()
            .map(|d| (d.kind, d.rotational))
            .collect();
        assert_eq!(
            kinds,
            [
                (DriveKind::Nvme, false),
                (DriveKind::Sata, false),
                (DriveKind::Sata, true)
            ]
        );
        assert!(machine.batteries.is_empty());
    }

    /// The server: two packages of many cores, sixteen modules, many
    /// drives, no display connected and no fan.
    #[test]
    fn the_server_has_what_a_rack_server_has() {
        let machine = Fixture::Server.machine();
        let cpu = machine.cpu.as_ref().unwrap();

        assert_eq!(machine.chassis.kind, ChassisKind::Server);
        assert_eq!((cpu.packages, cpu.cores, cpu.threads), (2, 64, 128));
        assert_eq!(machine.memory.as_ref().unwrap().modules.len(), 16);
        assert!(machine.drives.len() >= 16);
        assert_eq!(machine.displays().count(), 0);
        assert!(!machine.connectors.is_empty());
        assert_eq!(machine.fans().count(), 0);
        assert!(
            machine
                .sensors
                .iter()
                .any(|sensor| sensor.site == Site::Processor(1))
        );
    }

    /// The virtual machine: a display with no EDID, a virtio disk, no
    /// monitor and no USB.
    #[test]
    fn the_vm_has_almost_nothing() {
        let machine = Fixture::Vm.machine();

        assert_eq!(machine.chassis.kind, ChassisKind::Other);
        assert!(machine.sensors.is_empty() && machine.usb.is_empty());
        assert!(machine.memory.as_ref().unwrap().modules.is_empty());
        assert_eq!(machine.drives[0].kind, DriveKind::Virtual);

        let (_, panel) = machine.displays().next().unwrap();
        assert!(panel.size.estimated);
    }
}
