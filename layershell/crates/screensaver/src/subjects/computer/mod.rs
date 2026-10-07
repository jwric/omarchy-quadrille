//! The sheets of the computer the screensaver runs on.
//!
//! Each is a [`Subject`] like the designed ones, built once from the
//! machine's inventory ([`Machine`]) and drawn from it; what moves on it is
//! what the machine is doing, from [`Machine::sample`]. A sheet the machine
//! says too little for is left out, not drawn empty.
//!
//! Names from the inventory (a processor's model, a device's) are lettered
//! in capitals, as the rest of a sheet is, and never lettered whole where
//! they would not fit: the parts list carries a short name and the
//! specification as much of the full one as its column holds.
use crate::machine::Machine;

use super::Subject;

mod layout;
mod topology;

/// The sheets `machine` has enough to show, in sheet order.
pub fn sheets(machine: &Machine) -> Vec<Box<dyn Subject>> {
    let mut sheets: Vec<Box<dyn Subject>> = Vec::new();

    if let Some(topology) = topology::Topology::new(machine) {
        sheets.push(Box::new(topology));
    }

    sheets
}

/// A size in bytes as memory and caches are sold: `32 GiB`, `24 MiB`,
/// `31.0 GiB`.
fn binary(bytes: u64) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if value.fract() == 0.0 {
        format!("{value} {}", units[unit])
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

/// A size in bytes as drives are sold, in powers of ten: `1 TB`, `512 GB`,
/// `1.9 TB`.
fn decimal(bytes: u64) -> String {
    let units = ["B", "kB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;

    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }

    let tenths = (value * 10.0).round() / 10.0;

    if tenths.fract() == 0.0 || tenths >= 100.0 {
        format!("{} {}", tenths.round(), units[unit])
    } else {
        format!("{tenths:.1} {}", units[unit])
    }
}

/// A link's speed in Mbit/s, as it is rated: `480 MBIT/S`, `10 GBIT/S`.
fn bits(mbit: f32) -> String {
    let (value, unit) = if mbit >= 1000.0 {
        (mbit / 1000.0, "GBIT/S")
    } else {
        (mbit, "MBIT/S")
    };
    let tenths = (value * 10.0).round() / 10.0;

    if tenths.fract() == 0.0 {
        format!("{} {unit}", tenths.round())
    } else {
        format!("{tenths:.1} {unit}")
    }
}

/// A rate of I/O in bytes a second, in megabytes to a tenth, the width of
/// the number kept as it changes.
fn rate(bytes: f32) -> String {
    let megabytes = if bytes.is_finite() {
        bytes.max(0.0)
    } else {
        0.0
    } / 1e6;

    format!("{megabytes:5.1} MB/s")
}

/// A name from the inventory as a sheet letters it: in capitals, without
/// trademark signs, its spaces single.
fn lettered(name: &str) -> String {
    let name = ["(R)", "(r)", "(TM)", "(tm)", "®", "™"]
        .iter()
        .fold(name.to_owned(), |name, mark| name.replace(mark, ""));

    name.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

/// `n` of something, singular or plural: `1 DRIVE`, `2 DRIVES`.
fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `text` cut to `room` characters, with an ellipsis where it is cut.
fn fit(text: &str, room: usize) -> String {
    if text.chars().count() <= room {
        return text.to_owned();
    }

    let mut cut: String = text.chars().take(room.saturating_sub(1)).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_are_written_as_they_are_sold() {
        assert_eq!(binary(32 << 30), "32 GiB");
        assert_eq!(binary(24 << 20), "24 MiB");
        assert_eq!(binary(33_289_584_640), "31.0 GiB");
        assert_eq!(decimal(1_000_204_886_016), "1 TB");
        assert_eq!(decimal(512_110_190_592), "512 GB");
        assert_eq!(decimal(1_920_383_410_176), "1.9 TB");
    }

    #[test]
    fn speeds_and_rates_are_written_plainly() {
        assert_eq!(bits(480.0), "480 MBIT/S");
        assert_eq!(bits(1.5), "1.5 MBIT/S");
        assert_eq!(bits(10000.0), "10 GBIT/S");
        assert_eq!(bits(2500.0), "2.5 GBIT/S");
        assert_eq!(rate(4.0e6), "  4.0 MB/s");
        assert_eq!(rate(f32::NAN), "  0.0 MB/s");
    }

    #[test]
    fn names_are_lettered_in_capitals_and_cut_to_fit() {
        assert_eq!(
            lettered("13th Gen  Intel(R) Core(TM) i7"),
            "13TH GEN INTEL CORE I7"
        );
        assert_eq!(fit("GIGABIT ETHERNET CONTROLLER", 12), "GIGABIT ETH…");
        assert_eq!(fit("WI-FI", 12), "WI-FI");
    }

    #[test]
    fn the_fixture_has_every_sheet() {
        assert_eq!(sheets(&Machine::fixture()).len(), 1);
    }
}
