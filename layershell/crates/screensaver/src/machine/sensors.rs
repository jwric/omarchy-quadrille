//! What the machine is doing: its hardware monitors' temperatures, fan
//! speeds and power, its batteries' charge and its I/O.
//!
//! The inventory says what is measured ([`Sensor`]); a [`Sampler`] says
//! what it reads. On the machine a background thread reads every source
//! once a second into a [`Snapshot`] that frames take as it stands, so a
//! slow sysfs read (an NVMe drive's temperature is a command to the drive)
//! never holds a frame up. The fixture's snapshot is a function of time.
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
    /// What the chip calls it: `Package id 0`, `Composite`.
    pub label: Option<String>,
    /// What it is on.
    pub site: Site,
}

/// What a sensor measures, and the unit its values are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensorKind {
    /// In °C.
    Temperature,
    /// In revolutions a minute.
    Fan,
    /// In watts.
    Power,
    /// In volts.
    Voltage,
    /// In amperes.
    Current,
}

/// What a sensor is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Site {
    /// The processor package or one of its cores.
    Processor,
    /// Memory module `n`, in the order of
    /// [`Memory::modules`](super::Memory::modules).
    Module(usize),
    /// A PCI device: a graphics card, a drive's controller, a network
    /// adapter.
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
    /// Each sensor's value file, from the root, and what its values are
    /// divided by to be in the sensor's unit.
    pub sources: Vec<(String, f64)>,
    pub modules: Vec<Module>,
}

/// The chips that measure the processor, and memory modules.
const PROCESSOR_CHIPS: [&str; 4] = ["coretemp", "k10temp", "zenpower", "cpu_thermal"];
const MODULE_CHIPS: [(&str, Option<&str>); 2] = [("spd5118", Some("DDR5")), ("jc42", None)];
/// Wireless adapters whose monitor hangs off a thermal zone, not the
/// adapter.
const WIRELESS_CHIPS: [&str; 4] = ["iwlwifi", "mt79", "ath1", "rtw"];

/// The channels of a hardware monitor: prefix, kind, value suffixes in the
/// order they are preferred, and what a value is in the kind's unit
/// (millidegrees, microwatts, millivolts, milliamperes).
const CHANNELS: [(&str, SensorKind, &[&str], f64); 5] = [
    ("temp", SensorKind::Temperature, &["input"], 1e3),
    ("fan", SensorKind::Fan, &["input"], 1.0),
    ("power", SensorKind::Power, &["average", "input"], 1e6),
    ("in", SensorKind::Voltage, &["input"], 1e3),
    ("curr", SensorKind::Current, &["input"], 1e3),
];

/// Reads what every hardware monitor but the power supplies' measures (the
/// batteries are read as batteries).
pub(super) fn hwmon(tree: &Tree, pci: &[PciDevice]) -> Hwmon {
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
    modules.sort_by_key(|(bus, _)| super::natural(bus));

    let mut sensors = Vec::new();
    let mut sources = Vec::new();

    for (entry, chip, bus, path) in &chips {
        let module = MODULE_CHIPS
            .iter()
            .any(|(name, _)| name == chip)
            .then(|| modules.iter().position(|(on, _)| on == bus))
            .flatten();
        let site = match module {
            Some(module) => Site::Module(module),
            None => site(chip, path, pci),
        };
        let files = tree.entries(format!("{DIR}/{entry}"));

        for (prefix, kind, suffixes, per_unit) in CHANNELS {
            let mut numbers: Vec<u32> = files
                .iter()
                .filter_map(|file| {
                    let (channel, suffix) = file.strip_prefix(prefix)?.split_once('_')?;
                    suffixes
                        .contains(&suffix)
                        .then(|| channel.parse().ok())
                        .flatten()
                })
                .collect();
            numbers.sort();
            numbers.dedup();

            for n in numbers {
                let Some(suffix) = suffixes
                    .iter()
                    .find(|suffix| files.contains(&format!("{prefix}{n}_{suffix}")))
                else {
                    continue;
                };

                sensors.push(Sensor {
                    kind,
                    chip: chip.clone(),
                    label: tree.text(format!("{DIR}/{entry}/{prefix}{n}_label")),
                    site,
                });
                sources.push((format!("{DIR}/{entry}/{prefix}{n}_{suffix}"), per_unit));
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

/// What a chip that is not on a memory module measures: by its name, or
/// by the device it is on.
fn site(chip: &str, path: &[PciAddress], pci: &[PciDevice]) -> Site {
    let on = path
        .last()
        .and_then(|address| pci.iter().find(|device| device.address == *address));

    match on {
        _ if PROCESSOR_CHIPS.contains(&chip) => Site::Processor,
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

/// The open files the live values are read from, in the order of the
/// machine's lists.
pub(super) struct Gauges {
    pub sensors: Vec<Option<(File, f64)>>,
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
    /// one.
    pub(super) fn sample(&self, before: Option<(&Counters, Duration)>) -> (Snapshot, Counters) {
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
                .map(|source| {
                    let (file, per_unit) = source.as_ref()?;
                    Some((number(file)? / per_unit) as f32)
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

/// How often the machine is sampled.
const PERIOD: Duration = Duration::from_secs(1);

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
    /// Samples `gauges` once now, and then once a second from the first
    /// time it is asked, for as long as anything holds it.
    pub(super) fn live(gauges: Gauges) -> Self {
        let (snapshot, counters) = gauges.sample(None);

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

/// Samples once a second until nothing holds the sampler.
fn keep_sampling(
    shared: Weak<Shared>,
    (gauges, mut counters, mut then): (Gauges, Counters, Instant),
) {
    loop {
        std::thread::sleep(PERIOD);

        let now = Instant::now();
        let (snapshot, next) = gauges.sample(Some((&counters, now - then)));
        let Some(shared) = shared.upgrade() else {
            return;
        };

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
                    Site::Processor
                ),
                (
                    SensorKind::Temperature,
                    "nvme",
                    Some("Composite"),
                    Site::Device(PciAddress::new(0, 1, 0, 0))
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
        assert_eq!(machine.sensors_on(Site::Processor).count(), 1);
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
        let (first, counters) = gauges.sample(None);

        assert_eq!(first.drives, [None]);
        assert_eq!(first.interfaces, [None, None]);

        let stat = "sys/devices/pci0000:00/0000:00:06.0/0000:01:00.0/nvme/nvme0/nvme0n1/stat";
        let rx = "sys/devices/pci0000:00/0000:00:1c.0/0000:02:00.0/net/eth0/statistics/rx_bytes";
        fake.file(stat, "100 0 3048 0 50 0 2400 0 0 0 0\n")
            .file(rx, "3000000\n")
            .file("sys/class/hwmon/hwmon1/temp1_input", "61500\n");

        let (second, _) = gauges.sample(Some((&counters, Duration::from_secs(2))));

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
