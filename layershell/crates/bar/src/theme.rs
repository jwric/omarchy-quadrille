//! The panels follow the Omarchy theme that is current.
//!
//! `omarchy theme set` replaces `~/.local/state/omarchy/current/theme` (it
//! moves a staged directory into place) and writes the name to
//! `current/theme.name`. This reads both, and watches the directory they live
//! in with inotify, which costs nothing while nothing changes.
//!
//! - `quadrille-<x>` (terminal, paper, phosphor, amber, lcd) is one of
//!   quadrille's own themes, used as it is.
//! - Any other theme gets a [`Palette`] built from its `colors.toml`, by the
//!   mapping documented on [`palette`].
use iced_futures::futures::SinkExt;
use iced_futures::futures::channel::mpsc;
use iced_futures::stream;

use iced_core::Color;
use quadrille::theme::{mix, rgb};
use quadrille::{Palette, Theme};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `~/.local/state/omarchy/current`.
pub fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();

    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".local/state"))
        .join("omarchy/current")
}

/// The theme that is current in `dir`: what `theme.name` names, with the
/// colours of `theme/colors.toml`. Terminal if there is nothing to read.
pub fn current(dir: &Path) -> Theme {
    let name = std::fs::read_to_string(dir.join("theme.name"))
        .map(|name| name.trim().to_owned())
        .unwrap_or_default();

    if let Some(builtin) = quadrille_theme(&name) {
        return builtin;
    }

    match std::fs::read_to_string(dir.join("theme/colors.toml")) {
        Ok(colors) => from_colors(&name, &colors).unwrap_or(Theme::TERMINAL),
        Err(_) => Theme::TERMINAL,
    }
}

/// One of quadrille's themes, by the name of the Omarchy theme made from it.
pub fn quadrille_theme(name: &str) -> Option<Theme> {
    Some(match name.strip_prefix("quadrille-")? {
        "terminal" => Theme::TERMINAL,
        "paper" => Theme::PAPER,
        "phosphor" => Theme::PHOSPHOR,
        "amber" => Theme::AMBER,
        "lcd" => Theme::LCD,
        _ => return None,
    })
}

/// A theme from the text of a `colors.toml`; `None` when it has no
/// `background`, `foreground` or `accent`.
pub fn from_colors(name: &str, colors: &str) -> Option<Theme> {
    let colors = parse(colors);

    Some(Theme::new(format!("omarchy:{name}"), palette(&colors)?))
}

/// The key/value pairs of the flat `key = "#rrggbb"` file Omarchy writes.
pub fn parse(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;

            Some((
                key.trim().to_owned(),
                value.trim().trim_matches('"').to_owned(),
            ))
        })
        .collect()
}

fn hex(value: &str) -> Option<Color> {
    let value = value.strip_prefix('#')?;

    if value.len() != 6 {
        return None;
    }

    let byte = |at: usize| u8::from_str_radix(value.get(at..at + 2)?, 16).ok();

    Some(rgb(byte(0)?, byte(2)?, byte(4)?))
}

fn luminance(color: Color) -> f32 {
    0.2126 * color.r + 0.7152 * color.g + 0.0722 * color.b
}

/// A palette from the roles of an Omarchy `colors.toml`.
///
/// | palette role | taken from |
/// |---|---|
/// | `void` | `background` |
/// | `ground` | `lighter_background`, else a 5 % step from `background` towards the ink |
/// | `raised`, `hover` | the ground, 8 % and 16 % of the way towards the ink |
/// | `edge` | `selection`, or 22 % towards the ink if that is too close to the ground to be seen |
/// | `ink` | `foreground` |
/// | `muted`, `faint` | the ground, 70 % and 35 % of the way towards the ink |
/// | `line` | the ground, 45 % of the way towards the ink |
/// | `accent` | `accent` |
/// | `on_accent` | `background` or `foreground`, whichever is further from the accent |
/// | `highlight` | the ground, 35 % of the way towards the accent |
/// | `live`, `caution`, `alarm` | `green`, `yellow`, `red` |
///
/// The steps along the surfaces are the ones the quadrille palettes use (a
/// control changes state by stepping towards the ink), and the inks are mixed
/// in the same proportions, so a theme that comes out of
/// `tools/gen_themes.py` and back through this lands on the palette it came
/// from, give or take a shade.
pub fn palette(colors: &HashMap<String, String>) -> Option<Palette> {
    let get = |key: &str| colors.get(key).and_then(|value| hex(value));

    let void = get("background")?;
    let ink = get("foreground")?;
    let accent = get("accent")?;

    let ground = get("lighter_background").unwrap_or_else(|| mix(void, ink, 0.05));

    // A theme made of a single flat colour has nothing to step along.
    let ground = if (luminance(ground) - luminance(void)).abs() < 0.004 {
        mix(void, ink, 0.05)
    } else {
        ground
    };

    let edge = get("selection")
        .filter(|edge| (luminance(*edge) - luminance(ground)).abs() >= 0.06)
        .unwrap_or_else(|| mix(ground, ink, 0.22));

    let on_accent = if (luminance(void) - luminance(accent)).abs()
        >= (luminance(ink) - luminance(accent)).abs()
    {
        void
    } else {
        ink
    };

    Some(Palette {
        void,
        ground,
        raised: mix(ground, ink, 0.08),
        hover: mix(ground, ink, 0.16),
        edge,
        ink,
        muted: mix(ground, ink, 0.70),
        faint: mix(ground, ink, 0.35),
        line: mix(ground, ink, 0.45),
        accent,
        on_accent,
        highlight: mix(ground, accent, 0.35),
        live: get("green").unwrap_or(accent),
        caution: get("yellow").unwrap_or(accent),
        alarm: get("red").unwrap_or(accent),
    })
}

/// A message each time the current theme may have changed, at most one for a
/// burst of changes (`omarchy theme set` removes a directory, moves one in and
/// writes a file).
pub fn changes(dir: PathBuf) -> impl iced_futures::futures::Stream<Item = ()> {
    stream::channel(4, async move |mut output: mpsc::Sender<()>| {
        let (sender, mut receiver) = mpsc::unbounded::<()>();

        let watched = dir.clone();

        let _ = std::thread::Builder::new()
            .name("theme-watch".into())
            .spawn(move || watch(&watched, &sender));

        use iced_futures::futures::StreamExt;

        while receiver.next().await.is_some() {
            let _ = output.send(()).await;
        }
    })
}

/// Blocks on an inotify watch of `dir`, sending once per burst of events.
fn watch(dir: &Path, sender: &mpsc::UnboundedSender<()>) {
    use std::os::unix::ffi::OsStrExt;

    let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
        return;
    };

    // SAFETY: plain libc calls on a descriptor this function owns.
    unsafe {
        let fd = libc::inotify_init1(libc::IN_CLOEXEC);

        if fd < 0 {
            log::warn!("inotify is not available: no live theme changes");
            return;
        }

        let mask = libc::IN_CREATE
            | libc::IN_MOVED_TO
            | libc::IN_MOVED_FROM
            | libc::IN_DELETE
            | libc::IN_CLOSE_WRITE;

        if libc::inotify_add_watch(fd, path.as_ptr(), mask) < 0 {
            log::warn!("cannot watch {}: no live theme changes", dir.display());
            libc::close(fd);
            return;
        }

        let mut buffer = [0u8; 4096];

        loop {
            // Wait for the first event.
            if libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) <= 0 {
                break;
            }

            // Then for the burst to end: no event for a tenth of a second.
            loop {
                let mut poll = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };

                let settled = Duration::from_millis(120).as_millis() as i32;

                if libc::poll(&mut poll, 1, settled) <= 0 {
                    break;
                }

                if libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) <= 0 {
                    break;
                }
            }

            if sender.unbounded_send(()).is_err() {
                break;
            }
        }

        libc::close(fd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TERMINAL: &str = r##"
mode = "dark"

accent = "#ffa133"
selection = "#484844"
muted = "#8e8e8e"

background = "#1a1a1a"
dark_background = "#1a1a1a"
darker_background = "#101010"
lighter_background = "#222222"

foreground = "#c0c0c0"
dark_foreground = "#8e8e8e"

red = "#e8553e"
yellow = "#e4c64c"
orange = "#ffa133"
green = "#9fc79a"
cyan = "#bccabb"
brown = "#6c6c58"
"##;

    const TOKYO_NIGHT: &str = r##"
mode = "dark"
accent = "#7aa2f7"
selection = "#292e42"
muted = "#414868"
background = "#1a1b26"
lighter_background = "#24283b"
foreground = "#a9b1d6"
red = "#f7768e"
yellow = "#e0af68"
green = "#9ece6a"
"##;

    const LIGHT: &str = r##"
mode = "light"
accent = "#3366cc"
selection = "#dddddd"
background = "#ffffff"
lighter_background = "#f4f4f4"
foreground = "#222222"
red = "#cc0000"
yellow = "#aa8800"
green = "#007700"
"##;

    fn palette_of(text: &str) -> Palette {
        palette(&parse(text)).expect("A palette")
    }

    #[test]
    fn a_theme_generated_from_quadrille_comes_back_to_it() {
        let back = palette_of(TERMINAL);
        let original = Palette::TERMINAL;

        // What colors.toml carries comes back exactly.
        assert_eq!(back.void, original.void);
        assert_eq!(back.ground, original.ground);
        assert_eq!(back.ink, original.ink);
        assert_eq!(back.accent, original.accent);
        assert_eq!(back.edge, original.edge);
        assert_eq!(back.live, original.live);
        assert_eq!(back.caution, original.caution);
        assert_eq!(back.alarm, original.alarm);
        assert_eq!(back.on_accent, original.on_accent);

        // What it does not is built the way quadrille builds it, so it is
        // within a few levels of the original.
        let near = |a: Color, b: Color| {
            (a.r - b.r)
                .abs()
                .max((a.g - b.g).abs())
                .max((a.b - b.b).abs())
                < 12.0 / 255.0
        };

        assert!(near(back.raised, original.raised));
        assert!(near(back.hover, original.hover));
        assert!(near(back.muted, original.muted));
        assert!(near(back.faint, original.faint));
    }

    #[test]
    fn surfaces_step_towards_the_ink() {
        for text in [TERMINAL, TOKYO_NIGHT, LIGHT] {
            let palette = palette_of(text);
            let from_ground = |color: Color| (luminance(color) - luminance(palette.ground)).abs();

            assert!(from_ground(palette.raised) < from_ground(palette.hover));
            assert!(from_ground(palette.faint) < from_ground(palette.muted));
            assert!(from_ground(palette.muted) < from_ground(palette.ink));
            assert!(from_ground(palette.edge) >= 0.05, "an edge must be visible");
        }
    }

    #[test]
    fn a_light_theme_is_light() {
        assert!(palette_of(LIGHT).ground.r > 0.9);
        assert!(!palette_of(LIGHT).is_dark());
        assert!(palette_of(TOKYO_NIGHT).is_dark());
    }

    #[test]
    fn the_accent_carries_readable_text() {
        let palette = palette_of(TOKYO_NIGHT);

        assert_ne!(palette.on_accent, palette.accent);
    }

    #[test]
    fn quadrille_names_are_quadrille_themes() {
        assert_eq!(quadrille_theme("quadrille-paper"), Some(Theme::PAPER));
        assert_eq!(quadrille_theme("quadrille-lcd"), Some(Theme::LCD));
        assert_eq!(quadrille_theme("quadrille-nope"), None);
        assert_eq!(quadrille_theme("tokyo-night"), None);
    }

    #[test]
    fn a_colors_file_without_roles_is_not_a_theme() {
        assert!(palette(&parse("mode = \"dark\"\n")).is_none());
    }

    #[test]
    fn colours_are_read_out_of_quotes_and_hashes() {
        let colors = parse("accent = \"#ffa133\"\nmode = \"dark\"\n");

        assert_eq!(colors["accent"], "#ffa133");
        assert_eq!(hex(&colors["accent"]), Some(rgb(0xff, 0xa1, 0x33)));
    }
}
