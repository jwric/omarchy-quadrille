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
use textwrap::WordSplitter;

use crate::draft::geom::{along, length};
use crate::draft::{Draft, Tone, V2};
use crate::machine::Machine;

use super::Subject;

mod cooling;
mod displays;
mod layout;
mod topology;

/// The specification's rows, at most, and the characters a row holds on
/// the laptop's column: name and value with room between them.
const SPEC_ROWS: usize = 6;
const SPEC_ROOM: usize = 33;

/// The sheets `machine` has enough to show, in sheet order.
pub fn sheets(machine: &Machine) -> Vec<Box<dyn Subject>> {
    let mut sheets: Vec<Box<dyn Subject>> = Vec::new();

    if let Some(topology) = topology::Topology::new(machine) {
        sheets.push(Box::new(topology));
    }

    if let Some(displays) = displays::Displays::new(machine) {
        sheets.push(Box::new(displays));
    }

    if let Some(cooling) = cooling::Cooling::new(machine) {
        sheets.push(Box::new(cooling));
    }

    sheets
}

/// Specification rows of `name` and `value`, the value carried onto a
/// second row if it is too long for one, and cut at the end of that one,
/// with an ellipsis, if it is too long for two.
fn rows(name: &str, value: &str) -> Vec<(String, String)> {
    let room = SPEC_ROOM - name.chars().count() - 2;

    if value.chars().count() <= room {
        return vec![(name.into(), value.into())];
    }

    // Broken only between words: never inside one at its hyphen, which
    // would part HDMI-A-1.
    let options = textwrap::Options::new(room).word_splitter(WordSplitter::NoHyphenation);
    let lines = textwrap::wrap(value, options);
    let rest = lines[1..].join(" ");

    vec![
        (name.to_owned(), fit(&lines[0], room)),
        (String::new(), fit(&rest, room)),
    ]
}

/// Draws `draw` set on the grid by each of `snaps` in turn, the outermost
/// first (see [`Draft::snapped`]).
fn set(d: &mut Draft, snaps: &[V2], draw: impl FnOnce(&mut Draft)) {
    match snaps.split_first() {
        Some((&at, rest)) => d.snapped(at, |d| set(d, rest, draw)),
        None => draw(d),
    }
}

/// Draws `draw` set on the grid by `at`, then `steps` times on by `step`,
/// each by the one before: so each step is the same number of pixels.
fn chain(d: &mut Draft, at: V2, step: V2, steps: usize, draw: impl FnOnce(&mut Draft)) {
    d.snapped(at, |d| match steps {
        0 => draw(d),
        _ => chain(d, at + step, step, steps - 1, draw),
    });
}

/// Something running along a route: dots `spacing` apart that have gone
/// `travelled` along `pieces` (from their end toward their start if
/// `backward`), `share` of them there, each there or not by its own number
/// so a busier route carries more of them and none blinks as it runs. The
/// pieces are one route, broken where it passes through something drawn.
///
/// A dot is three pixels across, so it sits square on the 1 px line it
/// runs along, a pixel either side.
fn flow(
    d: &mut Draft,
    pieces: &[Vec<V2>],
    share: f32,
    spacing: f64,
    travelled: f64,
    backward: bool,
    tone: Tone,
) {
    flow_by(
        d,
        pieces,
        share,
        spacing,
        travelled,
        backward,
        |d, _, at| {
            d.dot(at, 3).tone(tone);
        },
    );
}

/// [`flow`], each dot drawn by `dot` with the piece it is on: set on the
/// grid as that piece is.
fn flow_by(
    d: &mut Draft,
    pieces: &[Vec<V2>],
    share: f32,
    spacing: f64,
    travelled: f64,
    backward: bool,
    mut dot: impl FnMut(&mut Draft, usize, V2),
) {
    if !share.is_finite() || share <= 0.0 {
        return;
    }

    let lengths: Vec<f32> = pieces.iter().map(|piece| length(piece)).collect();
    let total: f32 = lengths.iter().sum();
    let first = ((travelled - f64::from(total)) / spacing).ceil() as i64;
    let last = (travelled / spacing).floor() as i64;
    // The two ways' dots are numbered apart, so they are not all there or
    // not together.
    let offset = if backward { 0.5 } else { 0.0 };

    for number in first..=last {
        let lot = (number as f64 * 0.618_034 + offset).rem_euclid(1.0);

        if lot >= f64::from(share) {
            continue;
        }

        let run = (travelled - number as f64 * spacing) as f32;
        let mut left = if backward { total - run } else { run };

        for (index, (piece, &length)) in pieces.iter().zip(&lengths).enumerate() {
            if left <= length {
                dot(d, index, along(piece, left));
                break;
            }
            left -= length;
        }
    }
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

/// Terms a sheet letters in their own case wherever they are, from the
/// inventory or not.
const TERMS: [(&str, &str); 2] = [("PCIE", "PCIe"), ("NVME", "NVMe")];

/// A name from the inventory as a sheet letters it: in capitals but for
/// [`TERMS`], without trademark signs, its spaces single.
fn lettered(name: &str) -> String {
    let name = ["(R)", "(r)", "(TM)", "(tm)", "®", "™"]
        .iter()
        .fold(name.to_owned(), |name, mark| name.replace(mark, ""));
    let name = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase();

    TERMS
        .iter()
        .fold(name, |name, (capitals, term)| cased(&name, capitals, term))
}

/// A name from `pci.ids` as a sheet letters it (see [`lettered`]), with
/// what reads as noise on a sheet taken out:
///
/// - a chip's code followed by its marketing name in brackets, `GX107M
///   [Generic RTX 400]`, is the marketing name; a name of its own followed
///   by a code name in brackets, `Wi-Fi 6E WX210 2x2 [Generic Peak]`, is
///   the name;
/// - a list of the models that share the device's id, `WX700*/WX710*`,
///   is the first of them, and a model's wildcard star is dropped;
/// - a word run into a bracket is spaced from it;
/// - `PCI Express` is `PCIe`.
fn cleaned(name: &str) -> String {
    let name = match (name.find('['), name.rfind(']')) {
        (Some(open), Some(close)) if open < close => {
            let before = name[..open].trim();
            let inside = name[open + 1..close].trim();

            if before.split_whitespace().count() <= 3 && !inside.is_empty() {
                inside
            } else {
                before
            }
        }
        _ => name,
    };
    // Models in a list each have a number; the first has a letter, which
    // tells a list of models from `10/100/1000` or `802.11a/b/g`.
    let words: Vec<&str> = name
        .split_whitespace()
        .map(|word| {
            let models: Vec<&str> = word.split('/').collect();
            let numbered = |model: &str| model.chars().any(|c| c.is_ascii_digit());

            if models.len() > 1
                && models.iter().all(|model| numbered(model))
                && models[0].chars().any(char::is_alphabetic)
            {
                models[0]
            } else {
                word
            }
        })
        .collect();
    // A word run into the bracket after it, `6E(802.11ax)`, is spaced
    // from it.
    let name = words
        .join(" ")
        .replace('*', "")
        .replace('(', " (")
        .replace("PCI Express", "PCIe");

    lettered(&name)
}

/// `text` with every `capitals` that is a word of its own (not a part of
/// a longer one) written `term`.
fn cased(text: &str, capitals: &str, term: &str) -> String {
    let mut cased = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(at) = rest.find(capitals) {
        let after = &rest[at + capitals.len()..];
        let before = rest[..at].chars().last().or(cased.chars().last());
        let alone = !before.is_some_and(char::is_alphanumeric)
            && !after.chars().next().is_some_and(char::is_alphanumeric);

        cased.push_str(&rest[..at]);
        cased.push_str(if alone { term } else { capitals });
        rest = after;
    }

    cased.push_str(rest);
    cased
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
        assert_eq!(
            lettered("NVMe SSD Controller, PCIe 4.0 (PCIE-X)"),
            "NVMe SSD CONTROLLER, PCIe 4.0 (PCIe-X)"
        );
        assert_eq!(lettered("NVMEXPRESS PCIE4"), "NVMEXPRESS PCIE4");
        assert_eq!(fit("GIGABIT ETHERNET CONTROLLER", 12), "GIGABIT ETH…");
        assert_eq!(fit("WI-FI", 12), "WI-FI");
    }

    /// Names from `pci.ids` lose what reads as noise, and keep what tells
    /// a device from another.
    #[test]
    fn device_names_are_cleaned_of_noise() {
        for (name, cleaned_as) in [
            (
                "GX107M [Generic RTX 400 Max-Q / Mobile]",
                "GENERIC RTX 400 MAX-Q / MOBILE",
            ),
            (
                "Generic Lake-P [Generic Arc Graphics]",
                "GENERIC ARC GRAPHICS",
            ),
            (
                "Wi-Fi 6E(802.11ax) WX210/WX1675* 2x2 [Generic Peak]",
                "WI-FI 6E (802.11AX) WX210 2X2",
            ),
            (
                "Wi-Fi 7(802.11be) WX700*/WX710*/WB20*/WB40 2x2",
                "WI-FI 7 (802.11BE) WX700 2X2",
            ),
            (
                "NVMe SSD Controller SX100/PX100/PX200",
                "NVMe SSD CONTROLLER SX100",
            ),
            (
                "GX8100/8200/8300 PCI Express Gigabit Ethernet Controller",
                "GX8100 PCIe GIGABIT ETHERNET CONTROLLER",
            ),
            // Not lists of models: speeds, standards, code names.
            (
                "10/100/1000 Ethernet Adapter",
                "10/100/1000 ETHERNET ADAPTER",
            ),
            (
                "802.11a/b/g/n Wireless Adapter",
                "802.11A/B/G/N WIRELESS ADAPTER",
            ),
            ("Generic Lake-P/U/H Audio", "GENERIC LAKE-P/U/H AUDIO"),
            ("Ethernet Controller X100-V", "ETHERNET CONTROLLER X100-V"),
        ] {
            assert_eq!(cleaned(name), cleaned_as, "{name}");
        }
    }

    /// A specification's long value is carried onto a second row between
    /// words, never inside a connector's name at its hyphen.
    #[test]
    fn specification_rows_break_between_words() {
        let carried = rows("OUTPUTS", "DP-1, DP-2, DP-3, HDMI-A-1, HDMI-A-2");

        assert_eq!(carried.len(), 2);
        assert!(
            carried
                .iter()
                .all(|(_, value)| value.split(", ").all(|name| name.contains('-'))),
            "{carried:?}"
        );

        // Too long for two rows, it says so where it is cut.
        let cut = rows(
            "SENSORS",
            "SYSTIN, CPUTIN, AUXTIN1, AUXTIN3, PECI AGENT 0 CALIBRATION, ACPI THERMAL ZONE",
        );

        assert_eq!(cut.len(), 2);
        assert!(cut[1].1.ends_with('…'), "{cut:?}");
        assert!(cut[1].1.chars().count() <= SPEC_ROOM - "SENSORS".len() - 2);
    }

    /// A machine has the sheets it has something to show on, and no
    /// others: a server with no display has no displays sheet, a virtual
    /// machine with no monitor no cooling sheet, a display whose size is
    /// not known is not drawn to scale, and a machine of which nothing is
    /// known has none.
    #[test]
    fn each_machine_has_the_sheets_it_has_something_for() {
        use crate::machine::Fixture;
        use crate::machine::tests::Fake;

        for (fixture, expected) in [
            (Fixture::Laptop, &["topology", "displays", "cooling"][..]),
            (Fixture::Desktop, &["topology", "displays", "cooling"]),
            (Fixture::Server, &["topology", "cooling"]),
            (Fixture::Vm, &["topology"]),
        ] {
            let names: Vec<&str> = sheets(&fixture.machine())
                .iter()
                .map(|sheet| sheet.name())
                .collect();

            assert_eq!(names, expected, "{fixture:?}");
        }

        assert!(sheets(&Fake::new().read()).is_empty());
    }

    /// Whatever the machine, its sheets' titles fit the title block, part
    /// names and values the parts list, and the specifications' rows their
    /// column, no more of them than it has room for.
    #[test]
    fn every_machines_cards_fit_their_columns() {
        for fixture in crate::machine::Fixture::ALL {
            for sheet in sheets(&fixture.machine()) {
                let card = sheet.card();
                let at = format!("{fixture:?} {}", sheet.name());

                assert!(card.title.chars().count() <= 24, "{at}: {}", card.title);

                for part in &card.parts {
                    assert!(part.name.chars().count() <= 14, "{at}: {}", part.name);
                    assert!(
                        part.material.chars().count() <= 10,
                        "{at}: {}",
                        part.material
                    );
                    assert!(part.spec.len() <= SPEC_ROWS, "{at}: {}", part.name);

                    for (name, value) in &part.spec {
                        assert!(
                            name.chars().count() + value.chars().count() + 2 <= SPEC_ROOM,
                            "{at}: {}: {name} {value}",
                            part.name
                        );
                    }
                }
            }
        }
    }

    /// A machine's serial numbers, addresses and names are in its tree
    /// beside what is read, and none of them is lettered on a sheet: not in
    /// what is drawn as a sheet runs, its card or its readings. Every kind
    /// of machine is read from its tree, the laptop's with more planted.
    #[test]
    fn no_identifier_reaches_a_sheet() {
        use crate::machine::Fixture;
        use crate::machine::tests::{Fake, IDENTIFIERS, plant};

        for fixture in Fixture::ALL {
            let fake = Fake::new();
            fixture.tree(&fake);

            if fixture == Fixture::Laptop {
                plant(&fake);
            }

            let machine = fake.read();
            let sheets = sheets(&machine);

            assert!(!sheets.is_empty());

            for sheet in &sheets {
                let mut seen = format!("{:?}", sheet.card());

                for t in [0.0, 11.5, 17.0, 24.0, 31.0, 38.0, 45.0, 52.0, 59.0, 66.0] {
                    let mut draft = Draft::new();
                    sheet.draw(&mut draft, t);
                    seen += &format!("{:?}{:?}", draft.marks(), sheet.readings(t));
                }

                for secret in IDENTIFIERS {
                    assert!(
                        !seen.contains(secret),
                        "{secret} is on {fixture:?}'s {}",
                        sheet.name()
                    );
                }
            }
        }
    }
}
