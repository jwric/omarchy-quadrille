//! Sheets drawn offscreen to PNG, exactly as an output would show them: the
//! same canvas program, laid out and rasterized in virtual pixels by the
//! software renderer, upscaled nearest-neighbour when asked.
use std::borrow::Cow;
use std::path::Path;
use std::time::{Duration, Instant};

use image::{RgbaImage, imageops};

use iced_core::renderer::{Headless, Style};
use iced_core::{Length, Size, mouse};
use iced_runtime::user_interface::{self, UserInterface};
use iced_widget::{Widget as _, canvas};
use quadrille::Theme;

use crate::sheet::{Display, Sheet, timeline::Programme};
use crate::subjects::Subject;

/// An output to draw for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Output {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    pub display: Display,
}

impl Output {
    /// The laptop: 2560 × 1600 at 1.666667, a 344.6 mm wide panel.
    pub const LAPTOP: Self = Self {
        width: 2560,
        height: 1600,
        scale: 1.666667,
        display: Display {
            mm_per_vpx: 344.6 / 2560.0 * 3.0,
            estimated: false,
        },
    };

    /// The ultrawide: 3440 × 1440 at 1, 796.6 mm across.
    pub const ULTRAWIDE: Self = Self {
        width: 3440,
        height: 1440,
        scale: 1.0,
        display: Display {
            mm_per_vpx: 796.6 / 3440.0 * 2.0,
            estimated: false,
        },
    };

    /// Physical pixels to a virtual pixel, as the fork's `Auto(2)` decides.
    pub fn pixel_scale(self) -> u32 {
        (2.0 * self.scale).round().max(1.0) as u32
    }

    /// The size of the sheet in virtual pixels.
    pub fn virtual_size(self) -> (u32, u32) {
        let ps = self.pixel_scale();

        (self.width / ps, self.height / ps)
    }
}

/// Makes the toolkit's fonts drawable, once.
pub fn load_fonts() {
    let mut fonts = iced_widget::graphics::text::font_system()
        .write()
        .expect("Write to the font system");

    for font in quadrille::fonts::ALL {
        fonts.load_font(Cow::Borrowed(font));
    }

    drop(fonts);
    quadrille_desktop::graphics::fall_back_to_departure();
}

/// A renderer drawing sheets offscreen for one output, keeping what a live
/// surface keeps between frames.
pub struct Studio<'a> {
    renderer: iced_renderer::Renderer,
    cache: user_interface::Cache,
    subjects: &'a [Box<dyn Subject>],
    output: Output,
    theme: &'a Theme,
    date: &'a str,
}

impl<'a> Studio<'a> {
    pub fn new(
        subjects: &'a [Box<dyn Subject>],
        output: Output,
        theme: &'a Theme,
        date: &'a str,
    ) -> Result<Self, String> {
        let settings = quadrille_desktop::graphics::settings();
        let renderer = smol::block_on(<iced_renderer::Renderer as Headless>::new(
            iced_core::renderer::Settings {
                font: settings.font,
                text_size: settings.text_size,
                ..iced_core::renderer::Settings::default()
            },
            false,
            Some("tiny-skia"),
        ))
        .ok_or("no software renderer")?;

        Ok(Self {
            renderer,
            cache: user_interface::Cache::default(),
            subjects,
            output,
            theme,
            date,
        })
    }

    /// The sheets starting with subject `first`, `elapsed` seconds in, as
    /// RGBA in virtual pixels.
    pub fn frame(&mut self, first: usize, elapsed: f32) -> RgbaImage {
        let _ = self.draw(first, elapsed);
        self.rasterize()
    }

    /// Lays out and draws the sheet into the renderer's layers.
    fn draw(&mut self, first: usize, elapsed: f32) -> Option<()> {
        let programme = Programme {
            first,
            parts: self.subjects.iter().map(|s| s.card().parts.len()).collect(),
        };
        let showing = programme.at(elapsed);
        let sheet = Sheet {
            subject: self.subjects[showing.subject].as_ref(),
            number: showing.subject + 1,
            of: self.subjects.len(),
            showing,
            display: self.output.display,
            date: self.date,
        };

        let (width, height) = self.output.virtual_size();
        let element: iced_core::Element<'_, (), Theme, iced_renderer::Renderer> = canvas(sheet)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed();
        let mut interface = UserInterface::build(
            element,
            Size::new(width as f32, height as f32),
            std::mem::take(&mut self.cache),
            &mut self.renderer,
        );

        interface.draw(
            &mut self.renderer,
            self.theme,
            &Style {
                text_color: self.theme.palette().ink,
            },
            mouse::Cursor::Unavailable,
        );
        self.cache = interface.into_cache();

        Some(())
    }

    /// The renderer's layers rasterized, every pixel of them.
    fn rasterize(&mut self) -> RgbaImage {
        let (width, height) = self.output.virtual_size();
        let rgba =
            self.renderer
                .screenshot(Size::new(width, height), 1.0, self.theme.palette().void);

        RgbaImage::from_raw(width, height, rgba).expect("A screenshot of the sheet's size")
    }

    /// Draws a frame into a PNG at `path`: at the output's full resolution
    /// when `physical`, one pixel per virtual pixel otherwise.
    pub fn save(
        &mut self,
        first: usize,
        elapsed: f32,
        physical: bool,
        path: &Path,
    ) -> Result<(), String> {
        let mut image = self.frame(first, elapsed);

        if physical {
            let ps = self.output.pixel_scale();
            image = imageops::resize(
                &image,
                image.width() * ps,
                image.height() * ps,
                imageops::FilterType::Nearest,
            );
        }

        image
            .save(path)
            .map_err(|error| format!("{}: {error}", path.display()))
    }

    /// How long each of `count` frames takes at `fps`, from `from` seconds
    /// into subject `first`'s sheet: drawing the sheet (the canvas program,
    /// what a live surface runs every frame), and rasterizing all of it
    /// (a live surface rasterizes only what changed, and its compositor
    /// then upscales that; neither is measured here).
    pub fn bench(
        &mut self,
        first: usize,
        from: f32,
        count: usize,
        fps: f32,
    ) -> Vec<(Duration, Duration)> {
        (0..count)
            .map(|frame| {
                let started = Instant::now();
                let drawn = self.draw(first, from + frame as f32 / fps);
                let rasterizing = Instant::now();
                let _ = self.rasterize();

                (
                    drawn.map_or(Duration::ZERO, |()| rasterizing - started),
                    rasterizing.elapsed(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a surface keeps from one frame to the next never leaks from one
    /// sheet into another: a studio that drew other subjects first draws a
    /// sheet as a fresh one does.
    #[test]
    fn a_kept_drawing_belongs_to_its_sheet() {
        load_fonts();

        let subjects = crate::subjects::all();
        let theme = Theme::TERMINAL;
        let mut busy = Studio::new(&subjects, Output::LAPTOP, &theme, "2026-10-07").unwrap();

        // Settled moments, where the still part of the sheet is kept.
        for first in 0..subjects.len() {
            let _ = busy.frame(first, 30.0);
        }

        for first in [0, 3, subjects.len() - 1] {
            let mut fresh = Studio::new(&subjects, Output::LAPTOP, &theme, "2026-10-07").unwrap();

            assert!(busy.frame(first, 30.0) == fresh.frame(first, 30.0), "subject {first}");
        }
    }

    #[test]
    fn the_presets_are_the_desks_displays() {
        assert_eq!(Output::LAPTOP.pixel_scale(), 3);
        assert_eq!(Output::LAPTOP.virtual_size(), (853, 533));
        assert_eq!(Output::ULTRAWIDE.pixel_scale(), 2);
        assert_eq!(Output::ULTRAWIDE.virtual_size(), (1720, 720));
    }
}
