//! How large a virtual pixel is on each output, from the compositor's EDID
//! and any measured sizes in `~/.config/quadrille/displays.toml`: the same
//! calibration the wallpaper and the cursor overlay use.
use std::collections::HashMap;
use std::process::Command;

use quadrille_desktop::physical::{DisplayInput, Overrides, PhysicalSize};

use crate::sheet::Display;

/// A virtual pixel when nothing is known: two logical pixels at 96 per inch.
pub const GUESS: Display = Display {
    mm_per_vpx: 2.0 * 25.4 / 96.0,
    estimated: true,
};

/// The displays Hyprland reports, by output name.
pub fn hyprland() -> HashMap<String, Display> {
    let output = match Command::new("hyprctl").args(["-j", "monitors"]).output() {
        Ok(output) if output.status.success() => output.stdout,
        Ok(output) => {
            log::warn!(
                "hyprctl monitors failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return HashMap::new();
        }
        Err(error) => {
            log::warn!("no hyprctl ({error}): drawings are scaled for a guessed pixel size");
            return HashMap::new();
        }
    };

    parse(&output, &Overrides::load_default())
}

/// The displays of a `hyprctl -j monitors` answer.
pub fn parse(json: &[u8], overrides: &Overrides) -> HashMap<String, Display> {
    let Ok(serde_json::Value::Array(monitors)) = serde_json::from_slice(json) else {
        return HashMap::new();
    };

    monitors
        .iter()
        .filter_map(DisplayInput::from_monitor)
        .map(|input| {
            let size = PhysicalSize::resolve(&input, overrides);
            let display = Display {
                mm_per_vpx: (size.mm_per_vpx_x + size.mm_per_vpx_y) / 2.0,
                estimated: size.estimated,
            };

            (input.name, display)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_laptop_panel_is_calibrated_from_its_edid() {
        // The laptop: 2560 × 1600 at 1.666667, EDID 340 × 220 mm, which snaps
        // to a 16.0" panel of 344.6 mm across; three pixels to a virtual one.
        let json =
            br#"[{"name":"eDP-2","make":"AU Optronics","model":"0x07B2","width":2560,"height":1600,
            "physicalWidth":340,"physicalHeight":220,"transform":0,"scale":1.666667}]"#;
        let displays = parse(json, &Overrides::default());
        let laptop = displays["eDP-2"];

        assert!(!laptop.estimated);
        assert!((laptop.mm_per_vpx - 0.404).abs() < 0.001, "{laptop:?}");
    }

    #[test]
    fn nothing_readable_is_no_display() {
        assert!(parse(b"not json", &Overrides::default()).is_empty());
    }
}
