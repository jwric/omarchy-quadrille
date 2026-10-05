//! Readings of the machine: enough for a bar to have something true to show.
use std::fs;

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuTimes {
    busy: u64,
    total: u64,
}

pub fn cpu_times() -> Option<CpuTimes> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().next()?;

    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse().ok())
        .collect();

    if values.len() < 5 {
        return None;
    }

    let idle = values[3] + values[4];
    let total: u64 = values.iter().take(8).sum();

    Some(CpuTimes {
        busy: total - idle,
        total,
    })
}

/// The share of the time the processors were busy between two readings.
pub fn cpu_percent(previous: CpuTimes, now: CpuTimes) -> u8 {
    let total = now.total.saturating_sub(previous.total);

    if total == 0 {
        return 0;
    }

    (now.busy.saturating_sub(previous.busy) * 100 / total).min(100) as u8
}

pub fn memory_percent() -> u8 {
    let Ok(meminfo) = fs::read_to_string("/proc/meminfo") else {
        return 0;
    };

    let field = |name: &str| -> u64 {
        meminfo
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim_start_matches(':').split_whitespace().next())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    };

    let total = field("MemTotal");

    if total == 0 {
        return 0;
    }

    (100 - field("MemAvailable") * 100 / total) as u8
}

/// Whether any interface but loopback is up.
pub fn network_up() -> bool {
    let Ok(entries) = fs::read_dir("/sys/class/net") else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry.file_name() != "lo"
            && fs::read_to_string(entry.path().join("operstate"))
                .is_ok_and(|state| state.trim() == "up")
    })
}

/// The charge of the first battery, if there is one.
pub fn battery_percent() -> Option<u8> {
    let entries = fs::read_dir("/sys/class/power_supply").ok()?;

    entries.flatten().find_map(|entry| {
        if !entry.file_name().to_string_lossy().starts_with("BAT") {
            return None;
        }

        fs::read_to_string(entry.path().join("capacity"))
            .ok()?
            .trim()
            .parse()
            .ok()
    })
}

/// Local hours, minutes and seconds.
pub fn local_time() -> (u8, u8, u8) {
    // SAFETY: `time` and `localtime_r` are given valid pointers.
    unsafe {
        let now = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();

        let _ = libc::localtime_r(&now, &mut tm);

        (tm.tm_hour as u8, tm.tm_min as u8, tm.tm_sec as u8)
    }
}
