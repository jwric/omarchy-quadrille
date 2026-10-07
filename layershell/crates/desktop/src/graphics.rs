//! What `quadrille::settings()` does, split in two so that a host with nothing
//! on screen holds no fonts.
//!
//! `quadrille::settings()` also makes Departure Mono the fallback family of
//! the font database, and to do that it touches the global font system, which
//! reads every font installed on the machine. That is most of what a host with
//! no surface would keep resident, and it is only needed when something is
//! drawn. So [`settings`] has the settings alone, and [`fall_back_to_departure`]
//! the rest, run when the first surface is built.
use iced_core::{Font, PixelScaleMode, Settings};

use quadrille::Face;

use std::sync::Once;

/// The settings of a quadrille application, without touching the font system.
pub fn settings() -> Settings {
    Settings {
        fonts: quadrille::fonts::ALL
            .iter()
            .map(|font| (*font).into())
            .collect(),
        font: Face::BODY.font,
        text_size: f32::from(Face::BODY.size()).into(),
        antialiasing: false,
        pixel_scale: PixelScaleMode::Auto(quadrille::LOGICAL_PER_VIRTUAL),
        ..Settings::default()
    }
}

/// Makes Departure Mono the monospace and sans-serif family of the font
/// database, so that a character no face has is drawn as the face's own
/// missing glyph on every machine. Once.
pub fn fall_back_to_departure() {
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let Font {
            family: iced_core::font::Family::Name(family),
            ..
        } = Face::PROSE.font
        else {
            return;
        };

        let mut font_system = iced_widget::graphics::text::font_system()
            .write()
            .expect("Write to the font system");

        let database = font_system.raw().db_mut();

        database.set_monospace_family(family);
        database.set_sans_serif_family(family);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This stands in for `quadrille::settings()`: it must stay what it is.
    #[test]
    fn the_settings_are_quadrilles() {
        let ours = settings();
        let theirs = quadrille::settings();

        assert_eq!(ours.fonts.len(), theirs.fonts.len());
        assert_eq!(ours.font, theirs.font);
        assert_eq!(ours.text_size, theirs.text_size);
        assert_eq!(ours.antialiasing, theirs.antialiasing);
        assert_eq!(ours.pixel_scale, theirs.pixel_scale);
    }
}
