//! The graphics cards' outputs and the displays on them, measured from
//! their EDID as the sheets' scales are: through `PhysicalSize`.
//!
//! The EDID is read a field at a time, and never where it keeps the
//! display's serial number: not its four bytes at 12, and not a
//! descriptor tagged as a serial string.
use quadrille_desktop::physical::{DisplayInput, Overrides, PhysicalSize};

use super::{PciAddress, Tree, natural};

/// An output of a graphics card.
#[derive(Debug, Clone)]
pub struct Connector {
    /// Its name, as the compositor names the output: `eDP-1`, `HDMI-A-1`.
    pub name: String,
    pub kind: ConnectorKind,
    /// The graphics card it is on.
    pub gpu: Option<PciAddress>,
    /// The display connected to it.
    pub panel: Option<Panel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorKind {
    /// The machine's own panel: eDP, LVDS, DSI.
    Internal,
    DisplayPort,
    Hdmi,
    Dvi,
    Vga,
    Other,
}

/// A display.
#[derive(Debug, Clone)]
pub struct Panel {
    /// The maker's three-letter PNP ID: `BOE`, `DEL`.
    pub maker: Option<String>,
    /// The name it gives itself; laptop panels mostly give none.
    pub name: Option<String>,
    /// Its preferred resolution.
    pub pixels: (u32, u32),
    /// The refresh rate of that mode, in Hz.
    pub refresh: Option<f32>,
    /// The year it was made, or the model year.
    pub year: Option<u16>,
    /// Its physical size, as the sheets' scales resolve it.
    pub size: PhysicalSize,
}

impl Panel {
    /// The diagonal in inches.
    pub fn inches(&self) -> f64 {
        self.size.diagonal_mm / 25.4
    }

    /// The distance between pixels, in millimetres.
    pub fn pitch_mm(&self) -> f64 {
        (self.size.mm_per_pixel_x + self.size.mm_per_pixel_y) / 2.0
    }
}

impl ConnectorKind {
    fn of(name: &str) -> Self {
        let kind = name.split('-').next().unwrap_or_default();

        match kind {
            "eDP" | "LVDS" | "DSI" => Self::Internal,
            "DP" => Self::DisplayPort,
            "HDMI" => Self::Hdmi,
            "DVI" => Self::Dvi,
            "VGA" => Self::Vga,
            _ => Self::Other,
        }
    }
}

/// Every connector, the machine's own panel first.
pub(super) fn connectors(tree: &Tree, overrides: &Overrides) -> Vec<Connector> {
    let mut connectors: Vec<Connector> = tree
        .entries("sys/class/drm")
        .into_iter()
        .filter_map(|entry| {
            // `card1-HDMI-A-1`; the cards themselves and render nodes have
            // no output.
            let (card, name) = entry.split_once('-')?;
            card.strip_prefix("card")?.parse::<u32>().ok()?;

            if name.starts_with("Writeback") {
                return None;
            }

            let at = |file: &str| format!("sys/class/drm/{entry}/{file}");
            let connected = tree.text(at("status")).as_deref() == Some("connected");
            let preferred = tree.text(at("modes")).and_then(|modes| {
                let (width, height) = modes.lines().next()?.split_once('x')?;
                Some((
                    width.parse().ok()?,
                    height.trim_end_matches('i').parse().ok()?,
                ))
            });

            Some(Connector {
                kind: ConnectorKind::of(name),
                gpu: tree.pci_path(at("")).last().copied(),
                panel: connected
                    .then(|| panel(tree, &at("edid"), name, preferred, overrides))
                    .flatten(),
                name: name.to_owned(),
            })
        })
        .collect();

    connectors.sort_by_key(|connector| {
        (
            connector.kind != ConnectorKind::Internal,
            natural(&connector.name),
        )
    });
    connectors
}

/// The display on a connector, from its EDID and the modes the kernel
/// lists; `None` if neither says its resolution.
fn panel(
    tree: &Tree,
    edid: &str,
    connector: &str,
    preferred: Option<(u32, u32)>,
    overrides: &Overrides,
) -> Option<Panel> {
    let edid = Edid::read(tree, edid);
    let pixels = edid.as_ref().and_then(|edid| edid.pixels).or(preferred)?;
    let mm = edid.as_ref().and_then(|edid| edid.mm).unwrap_or((0, 0));
    let input = DisplayInput {
        name: connector.into(),
        make: edid
            .as_ref()
            .and_then(|edid| edid.maker.clone())
            .unwrap_or_default(),
        model: edid
            .as_ref()
            .and_then(|edid| edid.name.clone())
            .unwrap_or_default(),
        width: pixels.0,
        height: pixels.1,
        physical_width: f64::from(mm.0),
        physical_height: f64::from(mm.1),
        transform: 0,
        scale: 1.0,
    };
    let size = PhysicalSize::resolve(&input, overrides);
    let edid = edid.unwrap_or_default();

    Some(Panel {
        maker: edid.maker,
        name: edid.name,
        pixels,
        refresh: edid.refresh,
        year: edid.year,
        size,
    })
}

/// What the sheets need of an EDID.
#[derive(Debug, Clone, Default, PartialEq)]
struct Edid {
    maker: Option<String>,
    name: Option<String>,
    pixels: Option<(u32, u32)>,
    refresh: Option<f32>,
    year: Option<u16>,
    /// The image size in millimetres: the preferred timing's, or the base
    /// block's in centimetres.
    mm: Option<(u32, u32)>,
}

/// The base block's 18-byte descriptors.
const DESCRIPTORS: [u64; 4] = [54, 72, 90, 108];
/// The tag of a descriptor that holds the display's name.
const NAME: u8 = 0xfc;

impl Edid {
    fn read(tree: &Tree, path: &str) -> Option<Self> {
        let mut head = [0; 12];
        tree.bytes(path, 0, &mut head)?;

        if head[..8] != [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00] {
            return None;
        }

        // Bytes 12 to 15 are the serial number: the read skips them.
        let mut made = [0; 2];
        let mut size = [0; 2];
        tree.bytes(path, 16, &mut made)?;
        tree.bytes(path, 21, &mut size)?;

        let mut edid = Self {
            maker: maker(u16::from_be_bytes([head[8], head[9]])),
            year: (made[1] > 0).then(|| 1990 + u16::from(made[1])),
            mm: (size[0] > 0 && size[1] > 0)
                .then(|| (u32::from(size[0]) * 10, u32::from(size[1]) * 10)),
            ..Self::default()
        };

        for offset in DESCRIPTORS {
            // The pixel clock of a timing, or zero and the tag of a display
            // descriptor: the rest is read only for these two.
            let mut lead = [0; 4];
            tree.bytes(path, offset, &mut lead)?;

            let timing = lead[0] != 0 || lead[1] != 0;

            if timing && edid.pixels.is_none() {
                let mut d = [0; 18];
                tree.bytes(path, offset, &mut d)?;
                edid.timing(&d);
            } else if !timing && lead[3] == NAME {
                let mut d = [0; 18];
                tree.bytes(path, offset, &mut d)?;
                edid.name = text(&d[5..]);
            }
        }

        Some(edid)
    }

    /// Reads a detailed timing descriptor: the first is the preferred mode.
    fn timing(&mut self, d: &[u8; 18]) {
        let twelve = |low: u8, high: u8| u32::from(low) | u32::from(high) << 8;
        let clock = f64::from(u16::from_le_bytes([d[0], d[1]])) * 10_000.0;
        let width = twelve(d[2], d[4] >> 4);
        let blank_x = twelve(d[3], d[4] & 0x0f);
        let height = twelve(d[5], d[7] >> 4);
        let blank_y = twelve(d[6], d[7] & 0x0f);
        let mm = (twelve(d[12], d[14] >> 4), twelve(d[13], d[14] & 0x0f));
        let total = f64::from((width + blank_x) * (height + blank_y));

        if width == 0 || height == 0 {
            return;
        }

        self.pixels = Some((width, height));
        self.refresh = (total > 0.0).then(|| ((clock / total * 100.0).round() / 100.0) as f32);

        if mm.0 > 0 && mm.1 > 0 {
            self.mm = Some(mm);
        }
    }
}

/// A PNP ID: three letters of five bits each, `A` being 1.
fn maker(code: u16) -> Option<String> {
    [10, 5, 0]
        .iter()
        .map(|shift| match (code >> shift) & 0x1f {
            letter @ 1..=26 => Some(char::from(b'A' + letter as u8 - 1)),
            _ => None,
        })
        .collect()
}

/// A descriptor's text: up to 13 characters, ended by a line feed.
fn text(bytes: &[u8]) -> Option<String> {
    let end = bytes
        .iter()
        .position(|b| *b == b'\n')
        .unwrap_or(bytes.len());
    let text: String = String::from_utf8_lossy(&bytes[..end]).trim().into();

    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::tests::laptop;
    use super::*;

    /// An EDID's base block: a display by `maker`, its preferred `mode` and
    /// image size, the year it was made, its name if it gives one, and a
    /// serial number in both places one goes.
    pub fn edid(
        maker: &str,
        name: Option<&str>,
        (width, height, refresh): (u32, u32, u32),
        (width_mm, height_mm): (u32, u32),
        year: u16,
    ) -> Vec<u8> {
        let mut e = vec![0u8; 128];
        e[..8].copy_from_slice(&[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0]);

        let letters: Vec<u16> = maker.bytes().map(|b| u16::from(b - b'A' + 1)).collect();
        e[8..10].copy_from_slice(&(letters[0] << 10 | letters[1] << 5 | letters[2]).to_be_bytes());
        e[12..16].copy_from_slice(&1_234_567_890u32.to_le_bytes());
        e[16] = 0xff;
        e[17] = (year - 1990) as u8;
        e[18] = 1;
        e[19] = 4;
        e[21] = width_mm.div_ceil(10) as u8;
        e[22] = height_mm.div_ceil(10) as u8;

        // The preferred timing, its blanking chosen so the clock is whole.
        let (blank_x, blank_y) = (160u32, if height == 1600 { 50 } else { 62 });
        let clock = (width + blank_x) * (height + blank_y) * refresh / 10_000;
        let d = &mut e[54..72];
        d[..2].copy_from_slice(&(clock as u16).to_le_bytes());
        d[2] = width as u8;
        d[3] = blank_x as u8;
        d[4] = ((width >> 8) << 4 | blank_x >> 8) as u8;
        d[5] = height as u8;
        d[6] = blank_y as u8;
        d[7] = ((height >> 8) << 4 | blank_y >> 8) as u8;
        d[12] = width_mm as u8;
        d[13] = height_mm as u8;
        d[14] = ((width_mm >> 8) << 4 | height_mm >> 8) as u8;

        let mut descriptor = |at: usize, tag: u8, text: &str| {
            e[at + 3] = tag;
            let mut bytes: Vec<u8> = text.bytes().collect();
            bytes.push(b'\n');
            bytes.resize(13, b' ');
            e[at + 5..at + 18].copy_from_slice(&bytes);
        };

        if let Some(name) = name {
            descriptor(72, NAME, name);
        }
        descriptor(90, 0xff, "SN-7Y2K4Q9");
        e[108 + 3] = 0x10;

        let sum = e[..127].iter().fold(0u8, |sum, b| sum.wrapping_add(*b));
        e[127] = 0u8.wrapping_sub(sum);
        e
    }

    #[test]
    fn the_displays_are_measured_from_their_edid() {
        let machine = laptop().read();
        let names: Vec<_> = machine.connectors.iter().map(|c| c.name.as_str()).collect();

        // The machine's own first.
        assert_eq!(names, ["eDP-1", "DP-1", "HDMI-A-1"]);

        let displays: Vec<_> = machine.displays().collect();
        assert_eq!(displays.len(), 2);

        let (own, panel) = displays[0];
        assert_eq!(own.kind, ConnectorKind::Internal);
        assert_eq!(own.gpu, Some(PciAddress::new(0, 0, 2, 0)));
        assert_eq!(panel.maker.as_deref(), Some("GEN"));
        assert_eq!(panel.name, None);
        assert_eq!(panel.pixels, (2560, 1600));
        assert_eq!(panel.refresh, Some(120.0));
        assert_eq!(panel.year, Some(2025));
        assert!(!panel.size.estimated);
        assert!((panel.inches() - 14.0).abs() < 1e-9, "{}", panel.inches());
        assert!(
            (panel.pitch_mm() - 0.1178).abs() < 0.0005,
            "{}",
            panel.pitch_mm()
        );

        let (external, monitor) = displays[1];
        assert_eq!(external.kind, ConnectorKind::DisplayPort);
        assert_eq!(monitor.name.as_deref(), Some("27 Monitor"));
        assert_eq!(monitor.pixels, (3840, 2160));
        assert_eq!(monitor.refresh, Some(60.0));
        assert!(
            (monitor.inches() - 27.0).abs() < 1e-9,
            "{}",
            monitor.inches()
        );

        assert!(machine.connectors[2].panel.is_none());
    }

    #[test]
    fn makers_are_three_letters() {
        assert_eq!(maker(0x10ac).as_deref(), Some("DEL"));
        assert_eq!(maker(0x09e5).as_deref(), Some("BOE"));
        assert_eq!(maker(0).as_deref(), None);
    }

    #[test]
    fn connectors_are_known_by_kind() {
        assert_eq!(ConnectorKind::of("eDP-2"), ConnectorKind::Internal);
        assert_eq!(ConnectorKind::of("HDMI-A-1"), ConnectorKind::Hdmi);
        assert_eq!(ConnectorKind::of("DP-3"), ConnectorKind::DisplayPort);
        assert_eq!(ConnectorKind::of("Virtual-1"), ConnectorKind::Other);
    }
}
