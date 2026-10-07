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

use crate::sheet::timeline::{self, Moment, Phase, Showing};
use crate::sheet::{Display, Sheet, schedule::Schedule};
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

/// What a sheet is doing in a frame, as far as its cost goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// The drawing is plotted in.
    Plot,
    /// A part's detail view is drawn in.
    DetailIn,
    /// Nothing is drawn in: only what moves changes.
    Settled,
    Wipe,
}

impl Stage {
    pub fn of(moment: Moment) -> Self {
        match moment.phase {
            Phase::Plot(_) => Self::Plot,
            Phase::Wipe(_) => Self::Wipe,
            Phase::Run(Some(focus)) if !focus.settled() => Self::DetailIn,
            Phase::Run(_) => Self::Settled,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Plot => "plot",
            Self::DetailIn => "detail in",
            Self::Settled => "settled",
            Self::Wipe => "wipe",
        }
    }
}

/// One frame's cost.
#[derive(Debug, Clone, Copy)]
pub struct Timed {
    pub stage: Stage,
    pub draw: Duration,
    /// The share of the output a live surface repaints for the frame: the
    /// damage the software compositor computes against the frame before.
    pub repainted: f32,
}

/// A renderer drawing sheets offscreen for one output, keeping what a live
/// surface keeps between frames.
pub struct Studio<'a> {
    renderer: iced_renderer::Renderer,
    cache: user_interface::Cache,
    /// The layers of the frame drawn before, for the damage between them.
    previous: Option<Vec<iced_tiny_skia::Layer>>,
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
            previous: None,
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
    fn draw(&mut self, first: usize, elapsed: f32) -> Showing {
        // The same seed every time: a moment drawn twice is the same sheet.
        let showing = Schedule::new(
            self.subjects.iter().map(|s| s.card().parts.len()).collect(),
            0,
            Some(first),
        )
        .at(0, elapsed);
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

        showing
    }

    /// The share of the output the frame just drawn damages, as the
    /// software compositor works it out to repaint a live surface.
    fn damage(&mut self) -> f32 {
        let iced_renderer::fallback::Renderer::Secondary(renderer) = &mut self.renderer else {
            return 1.0;
        };
        let (width, height) = self.output.virtual_size();
        let bounds = iced_core::Rectangle::with_size(Size::new(width as f32, height as f32));
        let layers = renderer.layers().to_vec();

        let share = match &self.previous {
            Some(previous) => {
                let damage = iced_graphics::damage::diff(
                    previous,
                    &layers,
                    |layer| vec![layer.bounds],
                    iced_tiny_skia::Layer::damage,
                );

                iced_graphics::damage::group(damage, bounds)
                    .iter()
                    .map(|region| region.width * region.height)
                    .sum::<f32>()
                    / (bounds.width * bounds.height)
            }
            None => 1.0,
        };

        self.previous = Some(layers);
        share.min(1.0)
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

    /// Every frame of subject `first`'s sheet at `fps`, timed: drawing the
    /// sheet (the canvas program, what a live surface runs every frame) and
    /// rasterizing all of it (a live surface rasterizes only what changed,
    /// and its compositor then upscales that; neither is measured here).
    pub fn bench(&mut self, first: usize, fps: f32) -> Vec<Timed> {
        let parts = self.subjects[first].card().parts.len();
        let frames = (timeline::duration(parts) * fps) as usize;

        (0..frames)
            .map(|frame| {
                let started = Instant::now();
                let showing = self.draw(first, frame as f32 / fps);
                let drawn = started.elapsed();
                let repainted = self.damage();
                // What the frame rasterizes to, as a live surface would.
                let _ = self.rasterize();

                Timed {
                    stage: Stage::of(showing.moment),
                    draw: drawn,
                    repainted,
                }
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

            assert!(
                busy.frame(first, 30.0) == fresh.frame(first, 30.0),
                "subject {first}"
            );
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

#[cfg(test)]
mod lettering {
    use super::*;
    use crate::draft::letters;
    use crate::draft::raster::LETTERING;
    use iced_widget::canvas::{Frame, Geometry};
    use quadrille::draw::{Anchor, Pen};

    /// Lines of lettering, set by the renderer's text or as the font's pixels.
    struct Specimen {
        pixels: bool,
        lines: Vec<String>,
    }

    impl canvas::Program<(), Theme, iced_renderer::Renderer> for Specimen {
        type State = ();

        fn draw(
            &self,
            _: &(),
            renderer: &iced_renderer::Renderer,
            _: &Theme,
            bounds: iced_core::Rectangle,
            _: mouse::Cursor,
        ) -> Vec<Geometry<iced_renderer::Renderer>> {
            let mut frame = Frame::new(renderer, bounds.size());
            let mut pen = Pen::new(&mut frame);

            for (i, line) in self.lines.iter().enumerate() {
                // Odd and even columns and rows.
                let at = iced_core::Point::new(3 + i as i32 % 3, 2 + i as i32 * 13 + i as i32 % 2);

                if self.pixels {
                    letters::write(&mut pen, line, at, Theme::TERMINAL.palette().ink);
                } else {
                    pen.text(
                        LETTERING,
                        line.clone(),
                        at,
                        Anchor::TOP_LEFT,
                        Theme::TERMINAL.palette().ink,
                    );
                }
            }

            drop(pen);
            vec![frame.into_geometry()]
        }
    }

    fn render(specimen: Specimen, size: (u32, u32)) -> Vec<u8> {
        let settings = quadrille_desktop::graphics::settings();
        let mut renderer = smol::block_on(<iced_renderer::Renderer as Headless>::new(
            iced_core::renderer::Settings {
                font: settings.font,
                text_size: settings.text_size,
                ..iced_core::renderer::Settings::default()
            },
            false,
            Some("tiny-skia"),
        ))
        .unwrap();
        let element: iced_core::Element<'_, (), Theme, iced_renderer::Renderer> = canvas(specimen)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed();
        let mut interface = UserInterface::build(
            element,
            Size::new(size.0 as f32, size.1 as f32),
            user_interface::Cache::default(),
            &mut renderer,
        );
        let theme = Theme::TERMINAL;
        let palette = theme.palette();

        interface.draw(
            &mut renderer,
            &Theme::TERMINAL,
            &Style {
                text_color: palette.ink,
            },
            mouse::Cursor::Unavailable,
        );

        renderer.screenshot(Size::new(size.0, size.1), 1.0, palette.void)
    }

    /// The font's pixels are the renderer's text, pixel for pixel, for
    /// every character the sheets letter.
    #[test]
    fn the_fonts_pixels_are_the_renderers_text() {
        load_fonts();

        let ascii: String = (32u8..127).map(char::from).collect();
        let lines: Vec<String> = ascii
            .as_bytes()
            .chunks(24)
            .map(|chunk| String::from_utf8(chunk.to_vec()).unwrap())
            .chain([
                "°±²³µ¹×ØΓΔΩαζπ—…⁰⁴⁵⁶⁷⁸⁹−─".to_owned(),
                "1:5×10⁸ Ø40 R43.3 20° 47 µF 10 kΩ".to_owned(),
            ])
            .collect();
        let size = (200, 13 * lines.len() as u32 + 6);
        let text = render(
            Specimen {
                pixels: false,
                lines: lines.clone(),
            },
            size,
        );
        let pixels = render(
            Specimen {
                pixels: true,
                lines,
            },
            size,
        );

        let differing = text
            .chunks_exact(4)
            .zip(pixels.chunks_exact(4))
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| (i as u32 % size.0, i as u32 / size.0))
            .collect::<Vec<_>>();

        let void = Theme::TERMINAL.palette().void.into_rgba8();
        assert!(
            pixels.chunks_exact(4).any(|pixel| pixel != void),
            "nothing was lettered"
        );
        assert!(
            differing.is_empty(),
            "{} pixels differ, first at {:?}",
            differing.len(),
            &differing[..differing.len().min(8)]
        );
    }
}
