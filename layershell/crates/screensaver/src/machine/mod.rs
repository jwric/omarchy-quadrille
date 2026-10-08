//! The machine the screensaver runs on: what it is, and what it is doing.
//!
//! [`Machine`] is an inventory of the computer's parts, read once at start
//! from `/sys` and `/proc` through a root path (`/` on the machine, a
//! temporary directory in tests), with no root privileges and no
//! subprocesses. Every source is optional: what the machine does not expose
//! is absent. Its live values (temperatures, fan speeds, the battery's
//! charge, I/O rates) come from [`Machine::sample`], which a background
//! thread refreshes once a second so a frame never waits on sysfs.
//!
//! Nothing that identifies the machine or its owner is read: no serial
//! numbers, MAC addresses, UUIDs, host or user names, network names or mount
//! points. The types have no field that could hold them, and every read goes
//! through [`Tree`], which refuses the files that carry them.
//!
//! [`Machine::fixture`] is a made-up laptop with the same shape, whose
//! values are a function of time: what tests and committed images draw.
//! [`Fixture`] has it and the other kinds of machine the sheets are tried
//! on: a desktop tower, a server and a virtual machine.

mod buses;
mod displays;
mod fixture;
mod network;
mod power;
mod processor;
mod sensors;
mod storage;

use std::fs::File;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use quadrille_desktop::physical::Overrides;

pub use buses::{PciAddress, PciDevice, PciKind, UsbDevice};
pub use displays::{Connector, ConnectorKind, Panel};
pub use fixture::Fixture;
pub use network::{Interface, Link};
pub use power::{Battery, Charge, ChargeState, Charger, ChargerKind};
pub use processor::{Cache, CacheKind, CoreKind, Cores, Cpu, Memory, Module};
pub use sensors::{HISTORY, Sampler, Sensor, SensorKind, Site, Snapshot, Traffic, Transfer};
pub use storage::{Drive, DriveKind, Partition};

/// A computer, as far as it says without root.
#[derive(Debug, Clone)]
pub struct Machine {
    pub chassis: Chassis,
    pub cpu: Option<Cpu>,
    pub memory: Option<Memory>,
    pub drives: Vec<Drive>,
    /// Every PCI device, by address: the bridges with what is behind them.
    pub pci: Vec<PciDevice>,
    /// Every USB device, root hubs first, each hub before what is on it.
    pub usb: Vec<UsbDevice>,
    /// The graphics cards' outputs, and the displays on them.
    pub connectors: Vec<Connector>,
    pub batteries: Vec<Battery>,
    pub chargers: Vec<Charger>,
    /// The network interfaces with hardware behind them.
    pub interfaces: Vec<Interface>,
    /// What the hardware monitors measure, whose values [`Self::sample`]
    /// gives in the same order.
    pub sensors: Vec<Sensor>,
    sampler: Sampler,
}

impl Machine {
    /// This machine, with the displays' measured sizes from
    /// `~/.config/quadrille/displays.toml` as the sheets' scales take them.
    pub fn live() -> Self {
        Self::read(Path::new("/"), &Overrides::load_default())
    }

    /// The machine whose `/sys` and `/proc` are under `root`.
    pub fn read(root: &Path, overrides: &Overrides) -> Self {
        let (mut machine, gauges) = Self::inventory(root, overrides);

        machine.sampler = Sampler::live(gauges);
        machine
    }

    /// The machine under `root`, not yet sampled, and the files its live
    /// values are in.
    fn inventory(root: &Path, overrides: &Overrides) -> (Self, sensors::Gauges) {
        let tree = Tree::new(root);
        let pci = buses::pci(&tree);
        let (drives, drive_gauges): (Vec<Drive>, _) = storage::drives(&tree).into_iter().unzip();
        let names: Vec<String> = drives.iter().map(|drive| drive.name.clone()).collect();
        let hwmon = sensors::hwmon(&tree, &pci, &names);
        let (batteries, battery_gauges) = power::batteries(&tree).into_iter().unzip();
        let (chargers, charger_gauges) = power::chargers(&tree).into_iter().unzip();
        let (interfaces, interface_gauges) = network::interfaces(&tree).into_iter().unzip();
        let gauges = sensors::Gauges {
            sensors: hwmon
                .sources
                .iter()
                .zip(&hwmon.sensors)
                .map(|((path, per_unit, kind), sensor)| {
                    Some(sensors::Channel {
                        file: tree.open(path)?,
                        per_unit: *per_unit,
                        kind: *kind,
                        slow: sensor.slow(),
                    })
                })
                .collect(),
            batteries: battery_gauges,
            chargers: charger_gauges,
            drives: drive_gauges,
            interfaces: interface_gauges,
        };
        let machine = Self {
            chassis: Chassis::read(&tree),
            cpu: processor::cpu(&tree),
            memory: processor::memory(&tree, hwmon.modules),
            drives,
            usb: buses::usb(&tree),
            connectors: displays::connectors(&tree, overrides),
            batteries,
            chargers,
            interfaces,
            sensors: hwmon.sensors,
            pci,
            sampler: Sampler::made(|_| Snapshot::default()),
        };

        (machine, gauges)
    }

    /// The made-up laptop: an external display, two fans, an NVMe drive,
    /// Wi-Fi and ethernet.
    pub fn fixture() -> Self {
        Fixture::Laptop.machine()
    }

    /// What the machine is doing `t` seconds into a sheet: the latest
    /// sample, at most a second old, of this machine; the fixture's values
    /// at `t`.
    pub fn sample(&self, t: f32) -> Arc<Snapshot> {
        self.sampler.at(t)
    }

    /// What the machine has done up to `t`: its samples a second apart,
    /// the oldest first, for up to two minutes (see [`Sampler::history`]).
    pub fn history(&self, t: f32) -> Vec<Arc<Snapshot>> {
        self.sampler.history(t)
    }

    /// The displays connected, and the outputs they are on.
    pub fn displays(&self) -> impl Iterator<Item = (&Connector, &Panel)> {
        self.connectors
            .iter()
            .filter_map(|connector| Some((connector, connector.panel.as_ref()?)))
    }

    /// The PCI device at `address`.
    pub fn pci_device(&self, address: PciAddress) -> Option<&PciDevice> {
        self.pci.iter().find(|device| device.address == address)
    }

    /// The PCI devices directly behind `bridge`, or on the root bus.
    #[allow(dead_code, reason = "only the tests walk the tree with it yet")]
    pub fn pci_behind(&self, bridge: Option<PciAddress>) -> impl Iterator<Item = &PciDevice> {
        self.pci
            .iter()
            .filter(move |device| device.parent == bridge)
    }

    /// The USB devices on the ports of `hub`, or the root hubs.
    pub fn usb_on<'a>(&'a self, hub: Option<&'a str>) -> impl Iterator<Item = &'a UsbDevice> {
        self.usb
            .iter()
            .filter(move |device| device.parent.as_deref() == hub)
    }

    /// The sensors on `site`, with their indices in a [`Snapshot`].
    pub fn sensors_on(&self, site: Site) -> impl Iterator<Item = (usize, &Sensor)> {
        self.sensors
            .iter()
            .enumerate()
            .filter(move |(_, sensor)| sensor.site == site)
    }

    /// The fans, with their indices in a [`Snapshot`].
    pub fn fans(&self) -> impl Iterator<Item = (usize, &Sensor)> {
        self.sensors
            .iter()
            .enumerate()
            .filter(|(_, sensor)| sensor.kind == SensorKind::Fan)
    }
}

/// What the machine is, by its firmware.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chassis {
    pub kind: ChassisKind,
    /// Who made it: `sys_vendor`.
    pub vendor: Option<String>,
    /// What they call it: `product_name`.
    pub product: Option<String>,
    pub board_vendor: Option<String>,
    pub board: Option<String>,
    /// The firmware's version.
    pub bios: Option<String>,
}

/// The shape of a machine, from the SMBIOS chassis type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChassisKind {
    Desktop,
    /// A small box: a mini PC, a lunch box or a stick.
    Mini,
    Laptop,
    Tablet,
    AllInOne,
    Server,
    #[default]
    Other,
}

impl Chassis {
    fn read(tree: &Tree) -> Self {
        let dmi = |name: &str| {
            tree.text(format!("sys/class/dmi/id/{name}"))
                .filter(|value| !placeholder(value))
        };

        Self {
            kind: tree
                .number("sys/class/dmi/id/chassis_type")
                .map_or(ChassisKind::Other, ChassisKind::from_smbios),
            vendor: dmi("sys_vendor"),
            product: dmi("product_name"),
            board_vendor: dmi("board_vendor"),
            board: dmi("board_name"),
            bios: dmi("bios_version"),
        }
    }
}

impl ChassisKind {
    /// The kind of an SMBIOS chassis type (DSP0134, 7.4.1).
    fn from_smbios(code: u32) -> Self {
        match code {
            3..=7 | 15 | 24 | 34 => Self::Desktop,
            16 | 35 | 36 => Self::Mini,
            8..=10 | 14 | 31 | 32 => Self::Laptop,
            11 | 30 => Self::Tablet,
            13 => Self::AllInOne,
            17 | 23 | 25 | 28 => Self::Server,
            _ => Self::Other,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Desktop => "DESKTOP",
            Self::Mini => "MINI PC",
            Self::Laptop => "LAPTOP",
            Self::Tablet => "TABLET",
            Self::AllInOne => "ALL-IN-ONE",
            Self::Server => "SERVER",
            Self::Other => "COMPUTER",
        }
    }
}

/// What firmware writes where it has nothing to say.
fn placeholder(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();

    [
        "to be filled by o.e.m.",
        "default string",
        "system product name",
        "system manufacturer",
        "not applicable",
        "none",
        "o.e.m.",
    ]
    .contains(&lower.as_str())
}

/// The files the inventory reads, under a root.
///
/// Every read goes through it, and it reads nothing that names the machine
/// or its owner (see [`private`]): a file of serial numbers, addresses or
/// UUIDs is as good as missing.
#[derive(Debug, Clone)]
struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(root: &Path) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.root.join(relative)
    }

    /// A file's text, trimmed; `None` when it is missing, empty, unreadable
    /// or private.
    fn text(&self, relative: impl AsRef<Path>) -> Option<String> {
        let path = self.path(relative);

        if private(&path) {
            return None;
        }

        let text = std::fs::read(&path).ok()?;
        let text = String::from_utf8_lossy(&text).trim().to_owned();

        (!text.is_empty()).then_some(text)
    }

    /// A file's text as a number.
    fn number<T: FromStr>(&self, relative: impl AsRef<Path>) -> Option<T> {
        self.text(relative)?.parse().ok()
    }

    /// The bytes of a file from `offset`, as many as `into` holds.
    fn bytes(&self, relative: impl AsRef<Path>, offset: u64, into: &mut [u8]) -> Option<()> {
        use std::os::unix::fs::FileExt;

        self.open(relative)?.read_exact_at(into, offset).ok()
    }

    /// The file, open to be read again and again.
    fn open(&self, relative: impl AsRef<Path>) -> Option<File> {
        let path = self.path(relative);

        if private(&path) {
            return None;
        }

        File::open(path).ok()
    }

    fn exists(&self, relative: impl AsRef<Path>) -> bool {
        self.path(relative).exists()
    }

    /// The names in a directory, in order.
    fn entries(&self, relative: impl AsRef<Path>) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(self.path(relative)) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .collect();

        names.sort_by_key(|name| natural(name));
        names
    }

    /// The name of what a link points to: a device's driver, say.
    fn link(&self, relative: impl AsRef<Path>) -> Option<String> {
        let target = std::fs::read_link(self.path(relative)).ok()?;

        Some(target.file_name()?.to_str()?.to_owned())
    }

    /// The PCI devices a sysfs path is reached through, from the root
    /// complex down: a PCI device's own path ends with itself.
    fn pci_path(&self, relative: impl AsRef<Path>) -> Vec<PciAddress> {
        let Ok(path) = std::fs::canonicalize(self.path(relative)) else {
            return Vec::new();
        };

        path.components()
            .filter_map(|part| part.as_os_str().to_str()?.parse().ok())
            .collect()
    }

    /// Whether a sysfs path is reached through a USB bus.
    fn through_usb(&self, relative: impl AsRef<Path>) -> bool {
        std::fs::canonicalize(self.path(relative)).is_ok_and(|path| {
            path.components().any(|part| {
                part.as_os_str()
                    .to_str()
                    .is_some_and(|part| part.starts_with("usb"))
            })
        })
    }
}

/// Whether a file holds what the inventory never reads: serial numbers,
/// hardware addresses, UUIDs and other unique identifiers (an NVMe
/// namespace's EUI and NGUID, its WWID, its subsystem's NQN), host names.
fn private(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    let name = name.to_ascii_lowercase();

    ["serial", "uuid", "address", "hostname", "nodename"]
        .iter()
        .any(|word| name.contains(word))
        || [
            "eui",
            "nguid",
            "wwid",
            "subsysnqn",
            "asset_tag",
            "descriptors",
        ]
        .iter()
        .any(|word| name.starts_with(word))
        || name.ends_with("asset_tag")
}

/// A key that orders names by the numbers in them: `hwmon2` before
/// `hwmon10`, `3-2.4` before `3-10`.
fn natural(name: &str) -> Vec<(String, u64)> {
    let mut key = Vec::new();
    let mut text = String::new();
    let mut digits = String::new();

    for c in name.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            if !digits.is_empty() {
                key.push((std::mem::take(&mut text), digits.parse().unwrap_or(0)));
                digits.clear();
            }
            text.push(c);
        }
    }

    key.push((text, digits.parse().unwrap_or(0)));
    key
}

/// The numbers in a CPU list: `0-3,8,10-11`.
fn cpu_list(text: &str) -> Vec<u32> {
    text.split(',')
        .filter_map(|range| match range.trim().split_once('-') {
            Some((from, to)) => Some(from.parse().ok()?..=to.parse().ok()?),
            None => {
                let one = range.trim().parse().ok()?;
                Some(one..=one)
            }
        })
        .flatten()
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A fake root to read a machine from: files and links in a temporary
    /// directory, gone with it.
    pub struct Fake {
        pub dir: tempfile::TempDir,
    }

    impl Fake {
        pub fn new() -> Self {
            Self {
                dir: tempfile::tempdir().expect("A temporary directory"),
            }
        }

        pub fn root(&self) -> &Path {
            self.dir.path()
        }

        /// Writes `text` to `path`, making the directories it is in.
        pub fn file(&self, path: &str, text: impl AsRef<[u8]>) -> &Self {
            let path = self.root().join(path);

            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
            self
        }

        pub fn dir(&self, path: &str) -> &Self {
            std::fs::create_dir_all(self.root().join(path)).unwrap();
            self
        }

        /// Makes `path` a link to `target`, which is relative to the link.
        pub fn link(&self, path: &str, target: &str) -> &Self {
            let path = self.root().join(path);

            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(target, path).unwrap();
            self
        }

        /// Makes `path` a link to `target`, both from the root, written
        /// relative to the link as sysfs writes its links.
        pub fn points(&self, path: &str, target: &str) -> &Self {
            let from: Vec<&str> = path.split('/').collect();
            let to: Vec<&str> = target.split('/').collect();
            let dir = &from[..from.len() - 1];
            let shared = dir.iter().zip(&to).take_while(|(a, b)| a == b).count();
            let mut steps = vec![".."; dir.len() - shared];

            steps.extend(&to[shared..]);
            self.link(path, &steps.join("/"))
        }

        pub fn read(&self) -> Machine {
            Machine::read(self.root(), &Overrides::default())
        }
    }

    #[test]
    fn nothing_to_read_is_a_machine_with_nothing_known() {
        let fake = Fake::new();
        let machine = fake.read();

        assert_eq!(machine.chassis, Chassis::default());
        assert!(machine.cpu.is_none());
        assert!(machine.memory.is_none());
        assert!(machine.drives.is_empty());
        assert!(machine.pci.is_empty());
        assert!(machine.usb.is_empty());
        assert!(machine.connectors.is_empty());
        assert!(machine.batteries.is_empty() && machine.chargers.is_empty());
        assert!(machine.interfaces.is_empty());
        assert!(machine.sensors.is_empty());

        let snapshot = machine.sample(0.0);
        assert!(snapshot.sensors.is_empty() && snapshot.drives.is_empty());
    }

    #[test]
    fn the_chassis_is_read_from_the_firmware_and_placeholders_are_nothing() {
        let fake = Fake::new();
        fake.file("sys/class/dmi/id/chassis_type", "10\n")
            .file("sys/class/dmi/id/sys_vendor", "Maker\n")
            .file("sys/class/dmi/id/product_name", "Model 14\n")
            .file("sys/class/dmi/id/board_name", "To be filled by O.E.M.\n")
            .file("sys/class/dmi/id/bios_version", "1.07\n");

        let chassis = fake.read().chassis;

        assert_eq!(chassis.kind, ChassisKind::Laptop);
        assert_eq!(chassis.vendor.as_deref(), Some("Maker"));
        assert_eq!(chassis.product.as_deref(), Some("Model 14"));
        assert_eq!(chassis.board, None);
        assert_eq!(chassis.board_vendor, None);
        assert_eq!(chassis.bios.as_deref(), Some("1.07"));
    }

    /// A mini PC, a lunch box or a stick is a small box of its own, not a
    /// desktop: it is cooled as a laptop is, by a blower.
    #[test]
    fn small_boxes_are_mini_pcs() {
        for code in [16, 35, 36] {
            assert_eq!(ChassisKind::from_smbios(code), ChassisKind::Mini);
        }
        for code in [3, 6, 7] {
            assert_eq!(ChassisKind::from_smbios(code), ChassisKind::Desktop);
        }
        assert_eq!(ChassisKind::Mini.label(), "MINI PC");
    }

    /// `machine` with a display `mm` across and down, of `pixels`, on a
    /// further output `name` of the graphics its first display is on: a
    /// desk the fixtures do not have, to try a sheet on.
    pub fn plugged(
        mut machine: Machine,
        name: &str,
        mm: (f64, f64),
        pixels: (u32, u32),
    ) -> Machine {
        let mut connector = machine
            .connectors
            .iter()
            .find(|connector| connector.panel.is_some())
            .cloned()
            .expect("A display to copy");
        let panel = connector.panel.as_mut().expect("A display");

        connector.name = name.into();
        connector.kind = ConnectorKind::of(name);
        panel.pixels = pixels;
        panel.size.width_mm = mm.0;
        panel.size.height_mm = mm.1;
        panel.size.diagonal_mm = mm.0.hypot(mm.1);
        panel.size.mm_per_pixel_x = mm.0 / f64::from(pixels.0);
        panel.size.mm_per_pixel_y = mm.1 / f64::from(pixels.1);
        machine.connectors.push(connector);
        machine
    }

    /// What identifies a machine or its owner, planted in a tree by
    /// [`plant`]; the EDID's serial number and serial string are in its
    /// displays' EDIDs already.
    pub const IDENTIFIERS: [&str; 12] = [
        "SERIAL-DMI-0001",
        "UUID-0000-1111",
        "SERIAL-NVME-0002",
        "EUI-0003",
        "NQN-SERIAL-0004",
        "02:00:5e:10:20:30",
        "02:00:5e:40:50:60",
        "SERIAL-BAT-0005",
        "SERIAL-USB-0006",
        "MACHINE-HOSTNAME",
        "SN-7Y2K4Q9",
        "1234567890",
    ];

    /// Writes [`IDENTIFIERS`] into `fake` where a machine keeps them, beside
    /// what the inventory reads.
    pub fn plant(fake: &Fake) {
        let secrets = IDENTIFIERS;

        fake.file("sys/class/dmi/id/product_serial", secrets[0])
            .file("sys/class/dmi/id/board_serial", secrets[0])
            .file("sys/class/dmi/id/chassis_serial", secrets[0])
            .file("sys/class/dmi/id/product_uuid", secrets[1])
            .file(
                "sys/devices/pci0000:00/0000:00:06.0/0000:01:00.0/nvme/nvme0/serial",
                secrets[2],
            )
            .file(
                "sys/devices/pci0000:00/0000:00:06.0/0000:01:00.0/nvme/nvme0/nvme0n1/eui",
                secrets[3],
            )
            .file(
                "sys/devices/pci0000:00/0000:00:06.0/0000:01:00.0/nvme/nvme0/nvme0n1/wwid",
                secrets[3],
            )
            .file(
                "sys/devices/pci0000:00/0000:00:06.0/0000:01:00.0/nvme/nvme0/subsysnqn",
                secrets[4],
            )
            .file(
                "sys/devices/pci0000:00/0000:00:14.3/net/wlan0/address",
                secrets[5],
            )
            .file(
                "sys/devices/pci0000:00/0000:00:1c.0/0000:02:00.0/net/eth0/address",
                secrets[6],
            )
            .file(
                "sys/devices/pci0000:00/0000:00:1c.0/0000:02:00.0/net/eth0/perm_address",
                secrets[6],
            )
            .file("sys/class/power_supply/BAT0/serial_number", secrets[7])
            .file(
                "sys/devices/pci0000:00/0000:00:0d.0/usb1/1-3/serial",
                secrets[8],
            )
            .file("proc/sys/kernel/hostname", secrets[9])
            .file("etc/hostname", secrets[9]);

        // The monitor's EDID carries both of its serial numbers.
        let edid = std::fs::read(
            fake.root()
                .join("sys/devices/pci0000:00/0000:00:02.0/drm/card0/card0-DP-1/edid"),
        )
        .unwrap();
        assert!(edid.windows(4).any(|w| w == b"SN-7"));
        assert_eq!(edid[12..16], 1_234_567_890u32.to_le_bytes());
    }

    /// Serial numbers, hardware addresses, UUIDs and the like are in the
    /// tree beside what is read, and none of them reaches the inventory or
    /// a sample of it (the sheets' test checks none reaches a sheet).
    #[test]
    fn no_identifier_is_ever_read() {
        let fake = laptop();
        plant(&fake);

        let machine = fake.read();
        let seen = format!("{machine:?}\n{:?}", machine.sample(0.0));

        assert!(!machine.drives.is_empty() && !machine.interfaces.is_empty());
        for secret in IDENTIFIERS {
            assert!(!seen.contains(secret), "{secret} was read");
        }
    }

    #[test]
    fn the_files_of_identifiers_are_private() {
        for name in [
            "serial",
            "product_serial",
            "product_uuid",
            "address",
            "perm_address",
            "eui",
            "nguid",
            "wwid",
            "subsysnqn",
            "serial_number",
            "chassis_asset_tag",
            "hostname",
        ] {
            assert!(private(Path::new(name)), "{name}");
        }

        for name in ["model", "product", "size", "temp1_input", "name"] {
            assert!(!private(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn names_are_ordered_by_their_numbers() {
        let mut names = vec!["hwmon10", "hwmon2", "3-10", "3-2.4", "3-2", "usb3", "usb1"];
        names.sort_by_key(|name| natural(name));

        assert_eq!(
            names,
            ["3-2", "3-2.4", "3-10", "hwmon2", "hwmon10", "usb1", "usb3"]
        );
    }

    #[test]
    fn cpu_lists_are_ranges() {
        assert_eq!(cpu_list("0-3,8,10-11\n"), [0, 1, 2, 3, 8, 10, 11]);
        assert_eq!(cpu_list(""), Vec::<u32>::new());
    }

    /// The machine this runs on, and two samples of it a few seconds apart,
    /// printed to look at; nothing of it belongs in the repository.
    #[test]
    #[ignore = "reads this machine: run with --ignored --nocapture"]
    fn this_machine() {
        let started = std::time::Instant::now();
        let machine = Machine::read(Path::new("/"), &Overrides::default());
        let took = started.elapsed();
        println!("{machine:#?}\n{:#?}", machine.sample(0.0));
        println!("read in {took:?}");

        std::thread::sleep(std::time::Duration::from_millis(2500));
        println!("{:#?}", machine.sample(0.0));
    }

    /// The tree of the fixture laptop: what the parsing tests read.
    pub fn laptop() -> Fake {
        let fake = Fake::new();
        Fixture::Laptop.tree(&fake);
        fake
    }
}
