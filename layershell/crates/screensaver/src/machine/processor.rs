//! The processor, its cores and caches, and the memory.
use std::collections::{BTreeMap, BTreeSet};

use super::{Tree, cpu_list};

/// The processor.
#[derive(Debug, Clone, PartialEq)]
pub struct Cpu {
    /// Its name: `/proc/cpuinfo`'s model name.
    pub model: Option<String>,
    pub packages: u32,
    pub cores: u32,
    /// Hardware threads: what the kernel calls CPUs.
    pub threads: u32,
    /// The kinds of core of a hybrid processor; empty when they are all
    /// alike.
    pub kinds: Vec<Cores>,
    /// Its caches, by level and kind.
    pub caches: Vec<Cache>,
    /// The guaranteed clock, in MHz, where the driver says.
    pub base_mhz: Option<u32>,
    /// The fastest any core runs.
    pub max_mhz: Option<u32>,
}

/// The cores of one kind on a hybrid processor.
#[derive(Debug, Clone, PartialEq)]
pub struct Cores {
    pub kind: CoreKind,
    pub cores: u32,
    pub threads: u32,
    pub max_mhz: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreKind {
    Performance,
    Efficient,
}

/// Caches of one size at one level: `instances` of them, each shared by
/// the threads of a core, a cluster or the package.
#[derive(Debug, Clone, PartialEq)]
pub struct Cache {
    pub level: u8,
    pub kind: CacheKind,
    pub bytes: u64,
    pub instances: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CacheKind {
    Data,
    Instruction,
    Unified,
}

pub(super) fn cpu(tree: &Tree) -> Option<Cpu> {
    const DIR: &str = "sys/devices/system/cpu";

    let cpus: Vec<u32> = tree
        .entries(DIR)
        .iter()
        .filter_map(|name| name.strip_prefix("cpu")?.parse().ok())
        .collect();
    let info = tree.text("proc/cpuinfo").unwrap_or_default();
    let model = field(&info, "model name")
        .map(|model| model.split_whitespace().collect::<Vec<_>>().join(" "));

    if cpus.is_empty() && model.is_none() {
        return None;
    }

    // The package and core each CPU is on.
    let core: BTreeMap<u32, (u32, u32)> = cpus
        .iter()
        .filter_map(|&cpu| {
            let at = |file: &str| format!("{DIR}/cpu{cpu}/topology/{file}");
            Some((
                cpu,
                (
                    tree.number(at("physical_package_id"))?,
                    tree.number(at("core_id"))?,
                ),
            ))
        })
        .collect();
    let mhz = |cpu: u32, file: &str| {
        tree.number::<u32>(format!("{DIR}/cpu{cpu}/cpufreq/{file}"))
            .map(|khz| khz / 1000)
    };
    let cores_of = |cpus: &[u32]| {
        cpus.iter()
            .filter_map(|cpu| core.get(cpu))
            .collect::<BTreeSet<_>>()
            .len() as u32
    };

    let processors = info
        .lines()
        .filter(|line| line.starts_with("processor"))
        .count() as u32;
    let threads = if cpus.is_empty() {
        processors
    } else {
        cpus.len() as u32
    };
    let packages = match core
        .values()
        .map(|(package, _)| package)
        .collect::<BTreeSet<_>>()
        .len()
    {
        0 => 1,
        n => n as u32,
    };
    let cores = match cores_of(&cpus) {
        0 => field(&info, "cpu cores")
            .and_then(|n| n.parse::<u32>().ok())
            .map_or(threads, |n| n * packages),
        n => n,
    };

    let kinds: Vec<Cores> = [
        ("sys/devices/cpu_core/cpus", CoreKind::Performance),
        ("sys/devices/cpu_atom/cpus", CoreKind::Efficient),
    ]
    .into_iter()
    .filter_map(|(path, kind)| {
        let members = cpu_list(&tree.text(path)?);
        Some(Cores {
            kind,
            cores: cores_of(&members),
            threads: members.len() as u32,
            max_mhz: members
                .iter()
                .filter_map(|&cpu| mhz(cpu, "cpuinfo_max_freq"))
                .max(),
        })
    })
    .collect();

    Some(Cpu {
        model,
        packages,
        cores,
        threads,
        kinds: if kinds.len() > 1 { kinds } else { Vec::new() },
        caches: caches(tree, &cpus),
        base_mhz: cpus.iter().find_map(|&cpu| mhz(cpu, "base_frequency")),
        max_mhz: cpus
            .iter()
            .filter_map(|&cpu| mhz(cpu, "cpuinfo_max_freq"))
            .max(),
    })
}

/// The caches the CPUs have, each counted once however many threads share
/// it.
fn caches(tree: &Tree, cpus: &[u32]) -> Vec<Cache> {
    let mut seen = BTreeSet::new();

    for &cpu in cpus {
        let dir = format!("sys/devices/system/cpu/cpu{cpu}/cache");

        for index in tree
            .entries(&dir)
            .iter()
            .filter(|name| name.starts_with("index"))
        {
            let at = |file: &str| format!("{dir}/{index}/{file}");
            let kind = match tree.text(at("type")).as_deref() {
                Some("Data") => CacheKind::Data,
                Some("Instruction") => CacheKind::Instruction,
                Some("Unified") => CacheKind::Unified,
                _ => continue,
            };
            let (Some(level), Some(bytes)) = (
                tree.number::<u8>(at("level")),
                tree.text(at("size")).as_deref().and_then(size),
            ) else {
                continue;
            };
            let shared = tree
                .text(at("shared_cpu_list"))
                .unwrap_or_else(|| cpu.to_string());

            seen.insert((level, kind, bytes, shared));
        }
    }

    let mut counted: BTreeMap<(u8, CacheKind, u64), u32> = BTreeMap::new();

    for (level, kind, bytes, _) in seen {
        *counted.entry((level, kind, bytes)).or_default() += 1;
    }

    counted
        .into_iter()
        .map(|((level, kind, bytes), instances)| Cache {
            level,
            kind,
            bytes,
            instances,
        })
        .collect()
}

/// A cache size as sysfs writes it: `48K`, `2048K`.
fn size(text: &str) -> Option<u64> {
    let (digits, unit) = text.split_at(
        text.find(|c: char| !c.is_ascii_digit())
            .unwrap_or(text.len()),
    );
    let unit = match unit {
        "" => 1,
        "K" => 1 << 10,
        "M" => 1 << 20,
        "G" => 1 << 30,
        _ => return None,
    };

    Some(digits.parse::<u64>().ok()? * unit)
}

/// The first value of a `/proc/cpuinfo` field.
fn field(info: &str, name: &str) -> Option<String> {
    info.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == name && !value.trim().is_empty()).then(|| value.trim().to_owned())
    })
}

/// The memory.
#[derive(Debug, Clone, PartialEq)]
pub struct Memory {
    /// What the kernel has to use, a little less than is installed.
    pub bytes: Option<u64>,
    /// The modules whose sensors the kernel shows, one for each; none where
    /// it shows none (soldered memory, or no driver for it).
    pub modules: Vec<Module>,
}

/// A memory module.
#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    /// Its generation, where its sensor tells: `DDR5`.
    pub generation: Option<String>,
}

impl Memory {
    /// The memory installed: the total rounded up to a size memory comes in
    /// (32 GiB from the 30.9 the kernel keeps for itself).
    pub fn installed(&self) -> Option<u64> {
        let bytes = self.bytes?;
        let gib = bytes as f64 / f64::from(1u32 << 30);
        let step = 2f64.powi((gib.log2().floor() as i32 - 2).max(0));

        Some(((gib / step).ceil() * step) as u64 * (1 << 30))
    }
}

pub(super) fn memory(tree: &Tree, modules: Vec<Module>) -> Option<Memory> {
    let info = tree.text("proc/meminfo").unwrap_or_default();
    let bytes = field(&info, "MemTotal").and_then(|total| {
        let kib: u64 = total.strip_suffix("kB")?.trim().parse().ok()?;
        Some(kib * 1024)
    });

    (bytes.is_some() || !modules.is_empty()).then_some(Memory { bytes, modules })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{Fake, laptop};
    use super::*;

    #[test]
    fn a_hybrid_processor_is_counted_by_core_kind() {
        let cpu = laptop().read().cpu.expect("A processor");

        assert_eq!(cpu.model.as_deref(), Some("Generic 16-Core Processor"));
        assert_eq!((cpu.packages, cpu.cores, cpu.threads), (1, 16, 24));
        assert_eq!(
            cpu.kinds,
            [
                Cores {
                    kind: CoreKind::Performance,
                    cores: 8,
                    threads: 16,
                    max_mhz: Some(5000)
                },
                Cores {
                    kind: CoreKind::Efficient,
                    cores: 8,
                    threads: 8,
                    max_mhz: Some(3800)
                },
            ]
        );
        assert_eq!((cpu.base_mhz, cpu.max_mhz), (Some(2200), Some(5000)));

        let l2: Vec<_> = cpu.caches.iter().filter(|c| c.level == 2).collect();
        assert_eq!(l2.len(), 2);
        assert_eq!((l2[0].bytes, l2[0].instances), (2 << 20, 8));
        assert_eq!((l2[1].bytes, l2[1].instances), (4 << 20, 2));

        let l3 = cpu.caches.iter().find(|c| c.level == 3).unwrap();
        assert_eq!((l3.bytes, l3.instances), (24 << 20, 1));
    }

    #[test]
    fn cpuinfo_alone_still_says_what_the_processor_is() {
        let fake = Fake::new();
        fake.file(
            "proc/cpuinfo",
            "processor\t: 0\nmodel name\t: Some   Processor\ncpu cores\t: 2\n\n\
             processor\t: 1\nmodel name\t: Some   Processor\ncpu cores\t: 2\n\n\
             processor\t: 2\nprocessor\t: 3\n",
        );
        let cpu = fake.read().cpu.unwrap();

        assert_eq!(cpu.model.as_deref(), Some("Some Processor"));
        assert_eq!((cpu.packages, cpu.cores, cpu.threads), (1, 2, 4));
        assert!(cpu.kinds.is_empty() && cpu.caches.is_empty());
        assert_eq!(cpu.max_mhz, None);
    }

    #[test]
    fn memory_is_the_total_and_the_modules_with_sensors() {
        let memory = laptop().read().memory.expect("Memory");

        assert_eq!(memory.bytes, Some(32_509_360 * 1024));
        assert_eq!(memory.installed(), Some(32 << 30));
        assert_eq!(memory.modules.len(), 2);
        assert_eq!(memory.modules[0].generation.as_deref(), Some("DDR5"));
    }

    #[test]
    fn installed_memory_rounds_to_the_sizes_it_comes_in() {
        let installed = |kib: u64| {
            Memory {
                bytes: Some(kib * 1024),
                modules: Vec::new(),
            }
            .installed()
            .map(|bytes| bytes >> 30)
        };

        assert_eq!(installed(7_990_000), Some(8));
        assert_eq!(installed(12_050_000), Some(12));
        assert_eq!(installed(16_150_000), Some(16));
        assert_eq!(installed(24_400_000), Some(24));
        assert_eq!(installed(32_385_324), Some(32));
        assert_eq!(installed(65_500_000), Some(64));
    }

    #[test]
    fn cache_sizes_are_in_bytes() {
        assert_eq!(size("48K"), Some(48 << 10));
        assert_eq!(size("24576K"), Some(24 << 20));
        assert_eq!(size("1M"), Some(1 << 20));
        assert_eq!(size("lots"), None);
    }
}
