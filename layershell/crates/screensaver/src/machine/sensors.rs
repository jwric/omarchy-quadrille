//! What the machine is doing: its hardware monitors' temperatures and fan
//! speeds, its batteries' charge and its I/O.
//!
//! The inventory says what is measured ([`Sensor`]); a [`Sampler`] says
//! what it reads. On the machine a background thread reads every source
//! once a second into a [`Snapshot`] that frames take as it stands, so a
//! slow sysfs read never holds a frame up. A drive's temperature is a
//! command to the drive, which can keep it from its deepest sleep or, a
//! hard disk's, from spinning down: those are read once a minute, and not
//! before the first frame. The fixture's snapshot is a function of time.
use std::fs::File;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::{Duration, Instant};

use super::power::{self, Charge};
use super::{Module, PciAddress, PciDevice, PciKind, Tree};

/// One thing a hardware monitor measures.
#[derive(Debug, Clone, PartialEq)]
pub struct Sensor {
    pub kind: SensorKind,
    /// The monitor's chip, as its driver names it: `coretemp`, `nvme`,
    /// `spd5118`.
    pub chip: String,
    /// The chip's number for it: 2 for `fan2`, as `sensors` and a board's
    /// headers count.
    #[allow(dead_code, reason = "the cooling sheet will name fans by it")]
    pub channel: u32,
    /// What the chip calls it: `Package id 0`, `Composite`.
    pub label: Option<String>,
    /// What it is on.
    pub site: Site,
}

/// What a sensor measures, and the unit its values are in. A monitor's
/// voltages, currents and power are not read: no sheet draws them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorKind {
    /// In °C.
    Temperature,
    /// In revolutions a minute.
    Fan,
}

impl SensorKind {
    /// Whether `value` is one a working sensor of the kind reads. A
    /// monitor's input with nothing wired to it reads what its register
    /// holds: -128 or 127 °C on a board's Super I/O chip, 0 °C on a
    /// channel the firmware never fills. Those are no temperatures.
    pub fn plausible(self, value: f32) -> bool {
        match self {
            Self::Temperature => value > 0.0 && value < 115.0,
            Self::Fan => value >= 0.0,
        }
    }
}

/// What a sensor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Site {
    /// Processor package `n` or one of its cores: the packages in the order
    /// of their monitors (`coretemp.0` and `coretemp.1`, or a `k10temp` on
    /// each package's 18.3 and 19.3 functions).
    Processor(usize),
    /// Memory module `n`, in the order of
    /// [`Memory::modules`](super::Memory::modules).
    Module(usize),
    /// Drive `n`, in the order of [`Machine::drives`](super::Machine::drives):
    /// an NVMe drive's controller, or the disk behind a SATA one.
    Drive(usize),
    /// A PCI device: a graphics card, a network adapter.
    Device(PciAddress),
    /// The board: the embedded controller's fans, the ACPI thermal zone.
    Board,
}

/// The live values, in the order of the machine's lists they belong to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// Each of [`Machine::sensors`](super::Machine::sensors) in its kind's
    /// unit; `None` where it could not be read.
    pub sensors: Vec<Option<f32>>,
    pub batteries: Vec<Charge>,
    /// Whether each charger is plugged in.
    pub chargers: Vec<Option<bool>>,
    /// What each drive reads and writes; `None` until a second sample.
    pub drives: Vec<Option<Transfer>>,
    /// What each interface receives and sends; `None` until a second
    /// sample.
    pub interfaces: Vec<Option<Traffic>>,
}

impl Sensor {
    /// Whether reading it costs a command to a drive: it is read once a
    /// minute.
    pub(super) fn slow(&self) -> bool {
        SLOW_CHIPS.contains(&self.chip.as_str())
    }
}

impl Snapshot {
    /// Sensor `index`'s value.
    pub fn sensor(&self, index: usize) -> Option<f32> {
        self.sensors.get(index).copied().flatten()
    }
}

/// A drive's I/O, in bytes a second.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Transfer {
    pub read: f32,
    pub written: f32,
}

/// An interface's I/O, in bytes a second.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Traffic {
    pub received: f32,
    pub sent: f32,
}

/// What the hardware monitors offer.
pub(super) struct Hwmon {
    pub sensors: Vec<Sensor>,
    /// Each sensor's value file, from the root, what its values are
    /// divided by to be in the sensor's unit, and its kind.
    pub sources: Vec<(String, f64, SensorKind)>,
    pub modules: Vec<Module>,
}

/// The chips whose every read is a command to a drive.
const SLOW_CHIPS: [&str; 2] = ["drivetemp", "nvme"];

/// The chips that measure the processor, and memory modules.
const PROCESSOR_CHIPS: [&str; 4] = ["coretemp", "k10temp", "zenpower", "cpu_thermal"];
const MODULE_CHIPS: [(&str, Option<&str>); 2] = [("spd5118", Some("DDR5")), ("jc42", None)];
/// Wireless adapters whose monitor hangs off a thermal zone, not the
/// adapter.
const WIRELESS_CHIPS: [&str; 4] = ["iwlwifi", "mt79", "ath1", "rtw"];

/// The channels of a hardware monitor that are read: prefix, kind, and
/// what a value is in the kind's unit (millidegrees, revolutions a minute).
const CHANNELS: [(&str, SensorKind, f64); 2] = [
    ("temp", SensorKind::Temperature, 1e3),
    ("fan", SensorKind::Fan, 1.0),
];

/// Reads what every hardware monitor but the power supplies' measures (the
/// batteries are read as batteries), placed on the PCI devices, and on the
/// drives (by their kernel names).
///
/// A temperature that reads nonsense when the machine is read is an input
/// with nothing on it, and is left out; a drive's is not read then, as it
/// is a command to the drive.
pub(super) fn hwmon(tree: &Tree, pci: &[PciDevice], drives: &[String]) -> Hwmon {
    const DIR: &str = "sys/class/hwmon";

    // Each chip's directory, name, the device it is on (an I²C address
    // for a memory module's) and the PCI devices that device is behind.
    let chips: Vec<(String, String, String, Vec<PciAddress>)> = tree
        .entries(DIR)
        .into_iter()
        .filter_map(|entry| {
            let chip = tree.text(format!("{DIR}/{entry}/name"))?;
            let device = format!("{DIR}/{entry}/device");
            let path = std::fs::canonicalize(tree.path(&device)).unwrap_or_default();

            if path
                .components()
                .any(|part| part.as_os_str() == "power_supply")
            {
                return None;
            }

            let bus = tree.link(&device).unwrap_or_default();
            Some((entry, chip, bus, tree.pci_path(device)))
        })
        .collect();

    // The memory modules in the order of their bus addresses.
    let mut modules: Vec<(&str, Option<&str>)> = chips
        .iter()
        .filter_map(|(_, chip, bus, _)| {
            let (_, generation) = MODULE_CHIPS.iter().find(|(name, _)| name == chip)?;
            Some((bus.as_str(), *generation))
        })
        .collect();
    modules.sort_by_key(|(bus, _)| (i2c(bus), super::natural(bus)));

    // The processor's packages, by the devices their monitors are on.
    let mut packages: Vec<&str> = chips
        .iter()
        .filter(|(_, chip, ..)| PROCESSOR_CHIPS.contains(&chip.as_str()))
        .map(|(_, _, bus, _)| bus.as_str())
        .collect();
    packages.sort_by_key(|bus| super::natural(bus));
    packages.dedup();

    // What each drive's monitor would be on: an NVMe drive's controller, a
    // SATA disk's SCSI device.
    let devices: Vec<Option<std::path::PathBuf>> = drives
        .iter()
        .map(|name| std::fs::canonicalize(tree.path(format!("sys/block/{name}/device"))).ok())
        .collect();

    let mut sensors = Vec::new();
    let mut sources = Vec::new();

    for (entry, chip, bus, path) in &chips {
        let module = MODULE_CHIPS
            .iter()
            .any(|(name, _)| name == chip)
            .then(|| modules.iter().position(|(on, _)| on == bus))
            .flatten();
        let on = std::fs::canonicalize(tree.path(format!("{DIR}/{entry}/device"))).ok();
        let drive = on.as_ref().and_then(|on| {
            devices
                .iter()
                .position(|device| device.as_ref() == Some(on))
        });
        let site = match (module, drive) {
            (Some(module), _) => Site::Module(module),
            (None, Some(drive)) => Site::Drive(drive),
            _ if PROCESSOR_CHIPS.contains(&chip.as_str()) => Site::Processor(
                packages
                    .iter()
                    .position(|package| package == bus)
                    .unwrap_or(0),
            ),
            _ => site(chip, path, pci),
        };
        let slow = SLOW_CHIPS.contains(&chip.as_str());
        let files = tree.entries(format!("{DIR}/{entry}"));

        for (prefix, kind, per_unit) in CHANNELS {
            let mut numbers: Vec<u32> = files
                .iter()
                .filter_map(|file| {
                    let (channel, suffix) = file.strip_prefix(prefix)?.split_once('_')?;
                    (suffix == "input").then(|| channel.parse().ok()).flatten()
                })
                .collect();
            numbers.sort();
            numbers.dedup();

            for n in numbers {
                let source = format!("{DIR}/{entry}/{prefix}{n}_input");
                let nonsense = kind == SensorKind::Temperature
                    && !slow
                    && tree
                        .number::<f64>(&source)
                        .is_some_and(|value| !kind.plausible((value / per_unit) as f32));

                if nonsense {
                    continue;
                }

                sensors.push(Sensor {
                    kind,
                    chip: chip.clone(),
                    channel: n,
                    label: tree.text(format!("{DIR}/{entry}/{prefix}{n}_label")),
                    site,
                });
                sources.push((source, per_unit, kind));
            }
        }
    }

    Hwmon {
        sensors,
        sources,
        modules: modules
            .into_iter()
            .map(|(_, generation)| Module {
                generation: generation.map(Into::into),
            })
            .collect(),
    }
}

/// Where an I²C device is, `1-001a`, as numbers to order by: its bus, and
/// its address, which is in hex.
fn i2c(name: &str) -> Option<(u32, u32)> {
    let (bus, address) = name.split_once('-')?;

    Some((bus.parse().ok()?, u32::from_str_radix(address, 16).ok()?))
}

/// What a chip that is not on the processor, a memory module or a drive
/// measures: by the device it is on, or by its name.
fn site(chip: &str, path: &[PciAddress], pci: &[PciDevice]) -> Site {
    let on = path
        .last()
        .and_then(|address| pci.iter().find(|device| device.address == *address));

    match on {
        // A chip behind the board's own bridges (the embedded controller
        // behind the LPC bridge, the SMBus) measures the board.
        Some(device)
            if !matches!(device.kind(), PciKind::Chipset | PciKind::Bridge)
                && device.class >> 8 != 0x0c05 =>
        {
            Site::Device(device.address)
        }
        _ if WIRELESS_CHIPS.iter().any(|name| chip.starts_with(name)) => pci
            .iter()
            .find(|device| device.class >> 8 == 0x0280)
            .map_or(Site::Board, |adapter| Site::Device(adapter.address)),
        _ => Site::Board,
    }
}

/// A sensor's open value file.
pub(super) struct Channel {
    pub file: File,
    /// What its values are divided by to be in its kind's unit.
    pub per_unit: f64,
    /// What it measures: a value its kind does not read is no reading.
    pub kind: SensorKind,
    /// Whether it is read only now and then ([`Sensor::slow`]).
    pub slow: bool,
}

/// The open files the live values are read from, in the order of the
/// machine's lists.
pub(super) struct Gauges {
    pub sensors: Vec<Option<Channel>>,
    pub batteries: Vec<power::Gauge>,
    pub chargers: Vec<Option<File>>,
    /// Each drive's `stat`.
    pub drives: Vec<Option<File>>,
    /// Each interface's bytes received and sent.
    pub interfaces: Vec<Option<(File, File)>>,
}

/// The I/O counters of one sample: bytes so far, each way.
#[derive(Debug, Clone, Default)]
pub(super) struct Counters {
    drives: Vec<Option<(u64, u64)>>,
    interfaces: Vec<Option<(u64, u64)>>,
}

impl Gauges {
    /// Reads every source, and the I/O rates since `before` when there was
    /// one; the slow sensors too unless their values are `held` from an
    /// earlier snapshot.
    pub(super) fn sample(
        &self,
        before: Option<(&Counters, Duration)>,
        held: Option<&Snapshot>,
    ) -> (Snapshot, Counters) {
        let number = |file: &File| reread(file)?.parse::<f64>().ok();
        let counters = Counters {
            drives: self
                .drives
                .iter()
                .map(|stat| {
                    // Sectors read are the third field, sectors written the
                    // seventh.
                    let stat = reread(stat.as_ref()?)?;
                    let fields: Vec<u64> = stat
                        .split_whitespace()
                        .filter_map(|field| field.parse().ok())
                        .collect();
                    Some((*fields.get(2)? * 512, *fields.get(6)? * 512))
                })
                .collect(),
            interfaces: self
                .interfaces
                .iter()
                .map(|files| {
                    let (rx, tx) = files.as_ref()?;
                    Some((number(rx)? as u64, number(tx)? as u64))
                })
                .collect(),
        };
        let rates =
            |now: &[Option<(u64, u64)>], then: Option<&[Option<(u64, u64)>]>, elapsed: f32| {
                now.iter()
                    .enumerate()
                    .map(|(index, now)| {
                        let (now, then) = (now.as_ref()?, then?.get(index)?.as_ref()?);
                        let rate = |now: u64, then: u64| now.saturating_sub(then) as f32 / elapsed;
                        Some((rate(now.0, then.0), rate(now.1, then.1)))
                    })
                    .collect::<Vec<_>>()
            };
        let elapsed = before.map_or(1.0, |(_, elapsed)| elapsed.as_secs_f32().max(1e-3));
        let before = before.map(|(counters, _)| counters);

        let snapshot = Snapshot {
            sensors: self
                .sensors
                .iter()
                .enumerate()
                .map(|(index, channel)| {
                    let channel = channel.as_ref()?;

                    match held {
                        Some(held) if channel.slow => held.sensor(index),
                        _ => Some((number(&channel.file)? / channel.per_unit) as f32)
                            .filter(|value| channel.kind.plausible(*value)),
                    }
                })
                .collect(),
            batteries: self.batteries.iter().map(power::Gauge::read).collect(),
            chargers: self
                .chargers
                .iter()
                .map(|online| Some(number(online.as_ref()?)? != 0.0))
                .collect(),
            drives: rates(
                &counters.drives,
                before.map(|b| b.drives.as_slice()),
                elapsed,
            )
            .into_iter()
            .map(|rate| rate.map(|(read, written)| Transfer { read, written }))
            .collect(),
            interfaces: rates(
                &counters.interfaces,
                before.map(|b| b.interfaces.as_slice()),
                elapsed,
            )
            .into_iter()
            .map(|rate| rate.map(|(received, sent)| Traffic { received, sent }))
            .collect(),
        };

        (snapshot, counters)
    }
}

/// A sysfs file's value now: read again from its start, as an attribute
/// is made afresh for each read from there.
pub(super) fn reread(file: &File) -> Option<String> {
    use std::os::unix::fs::FileExt;

    let mut buffer = [0; 512];
    let length = file.read_at(&mut buffer, 0).ok()?;
    let text = String::from_utf8_lossy(&buffer[..length]).trim().to_owned();

    (!text.is_empty()).then_some(text)
}

/// How often the machine is sampled, and its slow sensors.
const PERIOD: Duration = Duration::from_secs(1);
const SLOW_PERIOD: Duration = Duration::from_secs(60);

/// Where a machine's live values come from.
#[derive(Clone)]
pub struct Sampler(Source);

#[derive(Clone)]
enum Source {
    /// Read from the machine by a thread of its own.
    Live(Arc<Shared>),
    /// A function of time.
    Made(fn(f32) -> Snapshot),
}

/// What the sampling thread shares with the frames.
struct Shared {
    latest: Mutex<Arc<Snapshot>>,
    /// The gauges, until the thread that reads them is started.
    idle: Mutex<Option<(Gauges, Counters, Instant)>>,
}

impl Sampler {
    /// Samples `gauges` once now but for the slow sensors, and then once a
    /// second from the first time it is asked, for as long as anything
    /// holds it.
    pub(super) fn live(gauges: Gauges) -> Self {
        let (snapshot, counters) = gauges.sample(None, Some(&Snapshot::default()));

        Self(Source::Live(Arc::new(Shared {
            latest: Mutex::new(Arc::new(snapshot)),
            idle: Mutex::new(Some((gauges, counters, Instant::now()))),
        })))
    }

    pub(super) fn made(at: fn(f32) -> Snapshot) -> Self {
        Self(Source::Made(at))
    }

    /// The values at `t`: the latest sample of a live machine.
    pub fn at(&self, t: f32) -> Arc<Snapshot> {
        match &self.0 {
            Source::Made(at) => Arc::new(at(t)),
            Source::Live(shared) => {
                let idle = shared
                    .idle
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .take();

                if let Some(idle) = idle {
                    let shared = Arc::downgrade(shared);
                    let started = std::thread::Builder::new()
                        .name("machine sampler".into())
                        .spawn(move || keep_sampling(shared, idle));

                    if let Err(error) = started {
                        log::warn!("the machine's values stay as first read: {error}");
                    }
                }

                shared
                    .latest
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone()
            }
        }
    }
}

impl std::fmt::Debug for Sampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Sampler")
    }
}

/// Samples once a second until nothing holds the sampler, the slow sensors
/// the first time and once a minute after.
fn keep_sampling(
    shared: Weak<Shared>,
    (gauges, mut counters, mut then): (Gauges, Counters, Instant),
) {
    let mut slow: Option<Instant> = None;

    loop {
        std::thread::sleep(PERIOD);

        let Some(shared) = shared.upgrade() else {
            return;
        };
        let now = Instant::now();
        let latest = shared
            .latest
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let due = slow.is_none_or(|read| now - read >= SLOW_PERIOD);
        let (snapshot, next) =
            gauges.sample(Some((&counters, now - then)), (!due).then_some(&*latest));

        if due {
            slow = Some(now);
        }

        *shared.latest.lock().unwrap_or_else(PoisonError::into_inner) = Arc::new(snapshot);
        (counters, then) = (next, now);
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Fake, laptop};
    use super::*;

    #[test]
    fn every_monitor_but_the_batteries_is_read_and_placed() {
        let machine = laptop().read();
        let found: Vec<_> = machine
            .sensors
            .iter()
            .map(|s| (s.kind, s.chip.as_str(), s.label.as_deref(), s.site))
            .collect();

        assert_eq!(
            found,
            [
                (SensorKind::Temperature, "acpitz", None, Site::Board),
                (
                    SensorKind::Temperature,
                    "cpu_thermal",
                    Some("Package"),
                    Site::Processor(0)
                ),
                (
                    SensorKind::Temperature,
                    "nvme",
                    Some("Composite"),
                    Site::Drive(0)
                ),
                (SensorKind::Temperature, "spd5118", None, Site::Module(0)),
                (SensorKind::Temperature, "spd5118", None, Site::Module(1)),
                (SensorKind::Fan, "ec", Some("CPU Fan"), Site::Board),
                (SensorKind::Fan, "ec", Some("System Fan"), Site::Board),
            ]
        );

        let snapshot = machine.sample(0.0);
        assert_eq!(snapshot.sensor(1), Some(52.0));
        assert_eq!(snapshot.sensor(5), Some(2200.0));
        assert_eq!(machine.fans().count(), 2);
        assert_eq!(machine.sensors_on(Site::Processor(0)).count(), 1);
    }

    /// A temperature input with nothing wired to it reads its register's
    /// limits, or the 0 a firmware never filled in: it is left out when
    /// the machine is read, and a reading that turns to nonsense later is
    /// no reading.
    #[test]
    fn an_unwired_input_is_no_temperature() {
        let fake = Fake::new();
        let at = "sys/class/hwmon/hwmon0";
        fake.file(&format!("{at}/name"), "nct6799\n");

        for (n, millidegrees) in [
            (1, "32000"),
            (2, "127000"),
            (3, "-128000"),
            (4, "0"),
            (5, "41500"),
        ] {
            fake.file(&format!("{at}/temp{n}_input"), format!("{millidegrees}\n"));
        }

        let machine = fake.read();
        let channels: Vec<u32> = machine.sensors.iter().map(|s| s.channel).collect();

        assert_eq!(channels, [1, 5]);
        assert_eq!(machine.sample(0.0).sensors, [Some(32.0), Some(41.5)]);

        let (_, gauges) = super::super::Machine::inventory(fake.root(), &Default::default());
        fake.file(&format!("{at}/temp5_input"), "-128000\n");

        assert_eq!(gauges.sample(None, None).0.sensors, [Some(32.0), None]);
    }

    /// Only temperatures and fan speeds are read: no sheet draws a
    /// monitor's voltages, currents or power. A fan keeps the chip's number
    /// for it.
    #[test]
    fn only_temperatures_and_fans_are_read() {
        let fake = Fake::new();
        let at = "sys/class/hwmon/hwmon0";
        fake.file(&format!("{at}/name"), "nct6799\n")
            .file(&format!("{at}/in0_input"), "1368\n")
            .file(&format!("{at}/curr1_input"), "500\n")
            .file(&format!("{at}/power1_average"), "21000000\n")
            .file(&format!("{at}/fan2_input"), "1180\n")
            .file(&format!("{at}/fan7_input"), "0\n")
            .file(&format!("{at}/temp1_input"), "32000\n");

        let machine = fake.read();
        let found: Vec<_> = machine
            .sensors
            .iter()
            .map(|s| (s.kind, s.channel))
            .collect();

        assert_eq!(
            found,
            [
                (SensorKind::Temperature, 1),
                (SensorKind::Fan, 2),
                (SensorKind::Fan, 7)
            ]
        );
        assert_eq!(
            machine.sample(0.0).sensors,
            [Some(32.0), Some(1180.0), Some(0.0)]
        );
    }

    /// Each package of a processor has its monitor, on a device of its
    /// own: `coretemp.0` and `coretemp.1`, or `k10temp` on each package's
    /// function 3 of devices 18 and 19.
    #[test]
    fn each_package_has_its_monitor() {
        let fake = Fake::new();

        for (n, device) in ["platform/coretemp.1", "platform/coretemp.0"]
            .iter()
            .enumerate()
        {
            fake.dir(&format!("sys/devices/{device}"))
                .file(&format!("sys/class/hwmon/hwmon{n}/name"), "coretemp\n")
                .file(&format!("sys/class/hwmon/hwmon{n}/temp1_input"), "50000\n")
                .points(
                    &format!("sys/class/hwmon/hwmon{n}/device"),
                    &format!("sys/devices/{device}"),
                );
        }

        let sites: Vec<Site> = fake.read().sensors.iter().map(|s| s.site).collect();
        assert_eq!(sites, [Site::Processor(1), Site::Processor(0)]);

        let fake = Fake::new();

        for (n, function) in ["0000:00:19.3", "0000:00:18.3"].iter().enumerate() {
            let device = format!("sys/devices/pci0000:00/{function}");
            fake.file(&format!("{device}/class"), "0x060000\n")
                .file(&format!("{device}/vendor"), "0x0f0f\n")
                .file(&format!("{device}/device"), "0x0018\n")
                .points(&format!("sys/bus/pci/devices/{function}"), &device)
                .file(&format!("sys/class/hwmon/hwmon{n}/name"), "k10temp\n")
                .file(&format!("sys/class/hwmon/hwmon{n}/temp1_input"), "50000\n")
                .points(&format!("sys/class/hwmon/hwmon{n}/device"), &device);
        }

        let sites: Vec<Site> = fake.read().sensors.iter().map(|s| s.site).collect();
        assert_eq!(sites, [Site::Processor(1), Site::Processor(0)]);
    }

    /// A drive's monitor is placed on the drive, not on the controller it
    /// is behind: two SATA disks on one controller are two drives.
    #[test]
    fn a_drives_monitor_is_on_its_drive() {
        let fake = Fake::new();
        super::super::Fixture::Desktop.tree(&fake);
        let machine = fake.read();
        let on_drives: Vec<(&str, Site)> = machine
            .sensors
            .iter()
            .filter_map(|s| match s.site {
                Site::Drive(n) => Some((machine.drives[n].name.as_str(), s.site)),
                _ => None,
            })
            .collect();

        assert_eq!(
            on_drives,
            [
                ("nvme0n1", Site::Drive(0)),
                ("nvme0n1", Site::Drive(0)),
                ("nvme0n1", Site::Drive(0)),
                ("sda", Site::Drive(1)),
                ("sdb", Site::Drive(2)),
            ]
        );
    }

    /// Modules are in the order of their addresses, which are in hex:
    /// 0x18 before 0x1a, and bus 0's before bus 1's.
    #[test]
    fn modules_are_in_the_order_of_their_addresses() {
        let fake = Fake::new();
        let smbus = "sys/devices/pci0000:00/0000:00:1f.4";

        for (n, (bus, address)) in [(1, 0x18), (0, 0x1a), (0, 0x18)].iter().enumerate() {
            let device = format!("{smbus}/i2c-{bus}/{bus}-{address:04x}");
            fake.dir(&device)
                .file(&format!("sys/class/hwmon/hwmon{n}/name"), "jc42\n")
                .file(
                    &format!("sys/class/hwmon/hwmon{n}/temp1_input"),
                    format!("{}\n", 40_000 + n),
                )
                .points(&format!("sys/class/hwmon/hwmon{n}/device"), &device);
        }

        let sites: Vec<Site> = fake.read().sensors.iter().map(|s| s.site).collect();

        assert_eq!(sites, [Site::Module(2), Site::Module(1), Site::Module(0)]);
        assert_eq!(i2c("1-001a"), Some((1, 0x1a)));
        assert_eq!(i2c("nvme0"), None);
    }

    #[test]
    fn a_wireless_monitor_on_a_thermal_zone_is_the_adapters() {
        let fake = laptop();
        fake.file("sys/class/hwmon/hwmon9/name", "iwlwifi_1\n")
            .file("sys/class/hwmon/hwmon9/temp1_input", "41000\n");
        let machine = fake.read();
        let wifi = machine.sensors.last().unwrap();

        assert_eq!(wifi.site, Site::Device(PciAddress::new(0, 0, 0x14, 3)));
    }

    #[test]
    fn rates_are_what_the_counters_moved_over_the_time_between() {
        let fake = laptop();
        let (_, gauges) = super::super::Machine::inventory(fake.root(), &Default::default());
        let (first, counters) = gauges.sample(None, None);

        assert_eq!(first.drives, [None]);
        assert_eq!(first.interfaces, [None, None]);

        let stat = "sys/devices/pci0000:00/0000:00:06.0/0000:01:00.0/nvme/nvme0/nvme0n1/stat";
        let rx = "sys/devices/pci0000:00/0000:00:1c.0/0000:02:00.0/net/eth0/statistics/rx_bytes";
        fake.file(stat, "100 0 3048 0 50 0 2400 0 0 0 0\n")
            .file(rx, "3000000\n")
            .file("sys/class/hwmon/hwmon1/temp1_input", "61500\n");

        let (second, _) = gauges.sample(Some((&counters, Duration::from_secs(2))), None);

        assert_eq!(second.sensor(1), Some(61.5));
        assert_eq!(
            second.drives,
            [Some(Transfer {
                read: 1000.0 * 512.0 / 2.0,
                written: 400.0 * 512.0 / 2.0
            })]
        );
        assert_eq!(
            second.interfaces[0],
            Some(Traffic {
                received: 1_000_000.0,
                sent: 0.0
            })
        );
    }

    /// The thread samples the machine afresh once a second; a frame takes
    /// the latest sample.
    #[test]
    fn the_live_sampler_follows_the_machine() {
        let fake = laptop();
        let machine = fake.read();

        assert_eq!(machine.sample(0.0).sensor(1), Some(52.0));

        fake.file("sys/class/hwmon/hwmon1/temp1_input", "70000\n");
        let deadline = Instant::now() + Duration::from_secs(5);

        while machine.sample(0.0).sensor(1) != Some(70.0) {
            assert!(Instant::now() < deadline, "the sampler never read again");
            std::thread::sleep(Duration::from_millis(50));
        }

        assert!(machine.sample(0.0).drives[0].is_some());
    }

    /// A drive's temperature is not read before the first frame, and
    /// when it is read, its value is held between reads.
    #[test]
    fn a_drives_temperature_is_read_now_and_then() {
        let fake = laptop();
        let nvme = "sys/class/hwmon/hwmon2/temp1_input";

        assert!(fake.read().sensors[2].slow());
        assert_eq!(fake.read().sample(0.0).sensor(2), None);

        let (_, gauges) = super::super::Machine::inventory(fake.root(), &Default::default());
        let (read, _) = gauges.sample(None, None);

        fake.file(nvme, "50000\n")
            .file("sys/class/hwmon/hwmon1/temp1_input", "61000\n");

        let (held, _) = gauges.sample(None, Some(&read));
        let (again, _) = gauges.sample(None, None);

        assert_eq!(read.sensor(2), Some(41.85));
        assert_eq!((held.sensor(1), held.sensor(2)), (Some(61.0), Some(41.85)));
        assert_eq!(again.sensor(2), Some(50.0));
    }

    #[test]
    fn a_monitor_that_cannot_be_read_reads_nothing() {
        let fake = Fake::new();
        fake.file("sys/class/hwmon/hwmon0/name", "acpitz\n")
            .file("sys/class/hwmon/hwmon0/temp1_input", "not a number\n");
        let machine = fake.read();

        assert_eq!(machine.sensors.len(), 1);
        assert_eq!(machine.sample(0.0).sensor(0), None);
        assert_eq!(machine.sample(0.0).sensor(7), None);
    }
}
