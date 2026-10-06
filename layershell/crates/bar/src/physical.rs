//! Physical calibration shared with the wallpaper's Physical.js.
use std::collections::HashMap;
use std::path::PathBuf;

const PANEL_DIAGONALS: &[f64] = &[
    13.3, 13.5, 14.0, 15.6, 16.0, 17.0, 17.3, 21.5, 23.8, 24.0, 24.5, 25.0, 27.0, 28.0, 31.5, 32.0,
    34.0, 35.0, 38.0, 40.0, 42.0, 43.0, 49.0, 55.0,
];

#[derive(Debug, Clone, Default)]
pub struct DisplayInput {
    pub name: String,
    pub make: String,
    pub model: String,
    pub width: u32,
    pub height: u32,
    pub physical_width: f64,
    pub physical_height: f64,
    pub transform: u32,
    pub scale: f64,
}

impl DisplayInput {
    pub fn from_monitor(value: &serde_json::Value) -> Option<Self> {
        Some(Self {
            name: value["name"].as_str()?.into(),
            make: value["make"].as_str().unwrap_or_default().into(),
            model: value["model"].as_str().unwrap_or_default().into(),
            width: value["width"].as_u64()?.try_into().ok()?,
            height: value["height"].as_u64()?.try_into().ok()?,
            physical_width: value["physicalWidth"].as_f64().unwrap_or_default(),
            physical_height: value["physicalHeight"].as_f64().unwrap_or_default(),
            transform: value["transform"].as_u64().unwrap_or_default() as u32,
            scale: value["scale"].as_f64().unwrap_or(1.0),
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayOverride {
    pub diagonal_inches: Option<f64>,
    pub width_mm: Option<f64>,
    pub height_mm: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub struct Overrides(pub HashMap<String, DisplayOverride>);

impl Overrides {
    pub fn default_path() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_default()
                    .join(".config")
            })
            .join("quadrille/displays.toml")
    }

    pub fn load_default() -> Self {
        let path = Self::default_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                log::warn!("{}: {error}", path.display());
                Self::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!("{}: {error}", path.display());
                Self::default()
            }
        }
    }

    /// The same strict TOML subset as Physical.js: named tables and positive
    /// numeric dimensions, including comments, quoted names and exponents.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut entries = HashMap::<String, HashMap<String, f64>>::new();
        let mut current = None;
        for (index, raw) in text.lines().enumerate() {
            let line = uncomment(raw);
            if line.is_empty() {
                continue;
            }
            if let Some(table) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let name = table_name(table)?;
                if entries.contains_key(&name) {
                    return Err(format!("duplicate display table: {name}"));
                }
                entries.insert(name.clone(), HashMap::new());
                current = Some(name);
                continue;
            }
            let invalid = || format!("invalid display setting at line {}", index + 1);
            let Some((key, number)) = line.split_once('=') else {
                return Err(invalid());
            };
            let key = key.trim();
            if !["diagonal_inches", "width_mm", "height_mm"].contains(&key) {
                return Err(invalid());
            }
            let entry = current
                .as_ref()
                .and_then(|name| entries.get_mut(name))
                .ok_or_else(invalid)?;
            let value = parse_number(number.trim())
                .ok_or_else(|| format!("invalid display number at line {}", index + 1))?;
            if !positive(value) || entry.insert(key.into(), value).is_some() {
                return Err(format!(
                    "invalid or duplicate display dimension at line {}",
                    index + 1
                ));
            }
        }
        let mut result = HashMap::new();
        for (key, values) in entries {
            let entry = DisplayOverride {
                diagonal_inches: values.get("diagonal_inches").copied(),
                width_mm: values.get("width_mm").copied(),
                height_mm: values.get("height_mm").copied(),
            };
            if !(entry.diagonal_inches.is_some() && values.len() == 1
                || entry.width_mm.is_some() && entry.height_mm.is_some() && values.len() == 2)
            {
                return Err(format!("incomplete display dimensions: {key}"));
            }
            result.insert(key, entry);
        }
        Ok(Self(result))
    }

    fn for_display(&self, input: &DisplayInput) -> DisplayOverride {
        self.0
            .get(&input.name)
            .or_else(|| self.0.get(&format!("{} {}", input.make, input.model)))
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy)]
// Keep all derived quantities in sync with Physical.js and the shared vectors.
#[allow(dead_code)]
pub struct PhysicalSize {
    pub width_mm: f64,
    pub height_mm: f64,
    pub diagonal_mm: f64,
    pub width_px: u32,
    pub height_px: u32,
    pub estimated: bool,
    pub source: &'static str,
    pub pixels_per_vpx: u32,
    pub mm_per_pixel_x: f64,
    pub mm_per_pixel_y: f64,
    pub px_per_mm_x: f64,
    pub px_per_mm_y: f64,
    pub mm_per_vpx_x: f64,
    pub mm_per_vpx_y: f64,
    pub mm_per_logical_pixel_x: f64,
    pub mm_per_logical_pixel_y: f64,
}

impl PhysicalSize {
    pub fn resolve(input: &DisplayInput, overrides: &Overrides) -> Self {
        let mut width_px = input.width.max(1);
        let mut height_px = input.height.max(1);
        let scale = if positive(input.scale) {
            input.scale
        } else {
            1.0
        };
        let pixels_per_vpx = (2.0 * scale).round().max(1.0) as u32;
        let entry = overrides.for_display(input);
        let (mut width_mm, mut height_mm, diagonal_mm, source) =
            match (entry.width_mm, entry.height_mm) {
                (Some(w), Some(h)) if positive(w) && positive(h) => (w, h, w.hypot(h), "override"),
                _ => {
                    let (diagonal_mm, source) = match entry.diagonal_inches {
                        Some(d) if positive(d) => (d * 25.4, "override"),
                        _ if positive(input.physical_width) && positive(input.physical_height) => (
                            diagonal(input.physical_width.hypot(input.physical_height) / 25.4)
                                * 25.4,
                            "edid",
                        ),
                        _ => (
                            f64::from(width_px).hypot(f64::from(height_px)) * 25.4 / 96.0,
                            "estimated",
                        ),
                    };
                    let pixel_diagonal = f64::from(width_px).hypot(f64::from(height_px));
                    (
                        diagonal_mm * f64::from(width_px) / pixel_diagonal,
                        diagonal_mm * f64::from(height_px) / pixel_diagonal,
                        diagonal_mm,
                        source,
                    )
                }
            };
        if input.transform % 2 != 0 {
            std::mem::swap(&mut width_px, &mut height_px);
            std::mem::swap(&mut width_mm, &mut height_mm);
        }
        let mm_per_pixel_x = width_mm / f64::from(width_px);
        let mm_per_pixel_y = height_mm / f64::from(height_px);
        Self {
            width_mm,
            height_mm,
            diagonal_mm,
            width_px,
            height_px,
            estimated: source == "estimated",
            source,
            pixels_per_vpx,
            mm_per_pixel_x,
            mm_per_pixel_y,
            px_per_mm_x: 1.0 / mm_per_pixel_x,
            px_per_mm_y: 1.0 / mm_per_pixel_y,
            mm_per_vpx_x: mm_per_pixel_x * f64::from(pixels_per_vpx),
            mm_per_vpx_y: mm_per_pixel_y * f64::from(pixels_per_vpx),
            mm_per_logical_pixel_x: mm_per_pixel_x * scale,
            mm_per_logical_pixel_y: mm_per_pixel_y * scale,
        }
    }
}

fn positive(value: f64) -> bool {
    value.is_finite() && value > 0.0
}

fn diagonal(inches: f64) -> f64 {
    let nearest = PANEL_DIAGONALS
        .iter()
        .copied()
        .min_by(|a, b| (a - inches).abs().total_cmp(&(b - inches).abs()))
        .unwrap();
    if (nearest - inches).abs() / inches <= 0.03 {
        nearest
    } else {
        (inches * 10.0).round() / 10.0
    }
}

#[cfg(test)]
fn snap_mm(mm: f64, px_per_mm: f64, pixels_per_vpx: u32) -> i32 {
    (mm * px_per_mm / f64::from(pixels_per_vpx)).round() as i32 * pixels_per_vpx as i32
}

fn uncomment(line: &str) -> &str {
    let mut quote = None;
    let mut escaped = false;
    for (index, c) in line.char_indices() {
        if escaped {
            escaped = false;
        } else if quote == Some('"') && c == '\\' {
            escaped = true;
        } else if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '"' || c == '\'' {
            quote = Some(c);
        } else if c == '#' {
            return line[..index].trim();
        }
    }
    line.trim()
}

fn table_name(text: &str) -> Result<String, String> {
    let text = text.trim();
    let text = text.strip_prefix("displays.").unwrap_or(text).trim();
    if text.starts_with('"') {
        return serde_json::from_str::<String>(text)
            .map_err(|_| "invalid display table name".into());
    }
    if let Some(literal) = text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        if !literal.contains('\'') {
            return Ok(literal.into());
        }
    }
    if !text.is_empty()
        && text
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Ok(text.into());
    }
    Err("invalid display table name".into())
}

fn parse_number(text: &str) -> Option<f64> {
    let mut chars = text.chars().peekable();
    if matches!(chars.peek(), Some('+' | '-')) {
        chars.next();
    }
    fn digits(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> bool {
        let mut digit = false;
        while let Some(c) = chars.peek() {
            if c.is_ascii_digit() {
                digit = true;
                chars.next();
            } else if *c == '_' && digit {
                chars.next();
                if !chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                    return false;
                }
            } else {
                break;
            }
        }
        digit
    }
    if !digits(&mut chars) {
        return None;
    }
    if chars.peek() == Some(&'.') {
        chars.next();
        if !digits(&mut chars) {
            return None;
        }
    }
    if matches!(chars.peek(), Some('e' | 'E')) {
        chars.next();
        if matches!(chars.peek(), Some('+' | '-')) {
            chars.next();
        }
        if !digits(&mut chars) {
            return None;
        }
    }
    if chars.next().is_some() {
        return None;
    }
    text.replace('_', "").parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn vectors() -> Value {
        serde_json::from_str(include_str!("../../../../docs/physical-vectors.json")).unwrap()
    }

    fn overrides(value: &Value) -> Overrides {
        Overrides(
            value
                .as_object()
                .unwrap()
                .iter()
                .map(|(key, entry)| {
                    (
                        key.clone(),
                        DisplayOverride {
                            diagonal_inches: entry["diagonal_inches"].as_f64(),
                            width_mm: entry["width_mm"].as_f64(),
                            height_mm: entry["height_mm"].as_f64(),
                        },
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn independent_panel_anchors() {
        for anchor in vectors()["anchors"].as_array().unwrap() {
            let input = DisplayInput::from_monitor(&anchor["input"]).unwrap();
            let actual = PhysicalSize::resolve(&input, &Overrides::default());
            let tolerance = anchor["relativeTolerance"].as_f64().unwrap();
            for (axis, found) in [("widthMm", actual.width_mm), ("heightMm", actual.height_mm)] {
                assert!(
                    (found / anchor[axis].as_f64().unwrap() - 1.0).abs() <= tolerance,
                    "{}.{axis}: {found}",
                    anchor["id"]
                );
            }
            assert!(
                (actual.diagonal_mm - anchor["nominalDiagonalMm"].as_f64().unwrap()).abs() < 0.001,
                "{}: nominal family",
                anchor["id"]
            );
        }
    }

    #[test]
    fn shared_physical_vectors() {
        for vector in vectors()["cases"].as_array().unwrap() {
            let input = DisplayInput::from_monitor(&vector["input"]).unwrap();
            let actual = PhysicalSize::resolve(&input, &overrides(&vector["overrides"]));
            let fields = json!({
                "widthMm": actual.width_mm, "heightMm": actual.height_mm,
                "diagonalMm": actual.diagonal_mm, "widthPx": actual.width_px,
                "heightPx": actual.height_px, "estimated": actual.estimated,
                "source": actual.source, "pixelsPerVpx": actual.pixels_per_vpx,
                "mmPerPixelX": actual.mm_per_pixel_x, "mmPerPixelY": actual.mm_per_pixel_y,
                "pxPerMmX": actual.px_per_mm_x, "pxPerMmY": actual.px_per_mm_y,
                "mmPerVpxX": actual.mm_per_vpx_x, "mmPerVpxY": actual.mm_per_vpx_y,
                "mmPerLogicalPixelX": actual.mm_per_logical_pixel_x,
                "mmPerLogicalPixelY": actual.mm_per_logical_pixel_y,
            });
            for (key, expected) in vector["expected"].as_object().unwrap() {
                if let Some(expected) = expected.as_f64() {
                    let found = fields[key].as_f64().unwrap();
                    assert!(
                        (found - expected).abs() <= 1e-9,
                        "{}.{key}: {found} != {expected}",
                        vector["id"]
                    );
                } else {
                    assert_eq!(fields[key], *expected, "{}.{key}", vector["id"]);
                }
            }
            for mm in -100..=100 {
                let px = snap_mm(f64::from(mm), actual.px_per_mm_x, actual.pixels_per_vpx);
                assert_eq!(
                    px % actual.pixels_per_vpx as i32,
                    0,
                    "{}: mark off grid",
                    vector["id"]
                );
                assert!(
                    (f64::from(px) - f64::from(mm) * actual.px_per_mm_x).abs()
                        <= f64::from(actual.pixels_per_vpx) / 2.0 + 1e-9,
                    "{}: mark error exceeds half a virtual pixel",
                    vector["id"]
                );
            }
        }
    }

    #[test]
    fn shared_mark_vectors() {
        for vector in vectors()["marks"].as_array().unwrap() {
            assert_eq!(
                snap_mm(
                    vector["mm"].as_f64().unwrap(),
                    vector["pxPerMm"].as_f64().unwrap(),
                    vector["pixelsPerVpx"].as_u64().unwrap() as u32
                ),
                vector["expected"].as_i64().unwrap() as i32,
                "{}",
                vector["id"]
            );
        }
    }

    #[test]
    fn shared_override_parser_vectors() {
        for vector in vectors()["overrideParsers"].as_array().unwrap() {
            let parsed = Overrides::parse(vector["text"].as_str().unwrap()).unwrap();
            let expected = overrides(&vector["expected"]);
            assert_eq!(parsed.0.len(), expected.0.len(), "{}", vector["id"]);
            for (name, entry) in expected.0 {
                let found = parsed.0.get(&name).unwrap();
                assert_eq!(
                    found.diagonal_inches, entry.diagonal_inches,
                    "{}",
                    vector["id"]
                );
                assert_eq!(found.width_mm, entry.width_mm, "{}", vector["id"]);
                assert_eq!(found.height_mm, entry.height_mm, "{}", vector["id"]);
            }
        }
        for text in vectors()["invalidOverrides"].as_array().unwrap() {
            assert!(Overrides::parse(text.as_str().unwrap()).is_err(), "{text}");
        }
    }
}
