//! Sheets drawn offscreen to PNG, exactly as an output would show them: the
//! same canvas program, laid out and rasterized in virtual pixels by the
//! software renderer, upscaled nearest-neighbour when asked.
use std::borrow::Cow;
use std::path::Path;
use std::time::{Duration, Instant};

use image::{RgbaImage, imageops};

use iced_core::renderer::{Headless, Style};
use iced_core::{Length, Rectangle, Size, mouse};
use iced_graphics::damage;
use iced_runtime::user_interface::{self, UserInterface};
use iced_widget::{Widget as _, canvas};
use quadrille::Theme;

use crate::sheet::plotter::Plot;
use crate::sheet::plotter::style::PlotStyle;
use crate::sheet::timeline::{self, Moment, Phase, Showing};
use crate::sheet::{self, Display, Sheet, schedule::Schedule};
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

    /// What the desk calls it, if it is one of the desk's.
    #[cfg(test)]
    pub fn name(self) -> String {
        if self == Self::LAPTOP {
            "the laptop".into()
        } else if self == Self::ULTRAWIDE {
            "the ultrawide".into()
        } else {
            format!("{} × {}", self.width, self.height)
        }
    }

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

impl std::str::FromStr for Output {
    type Err = String;

    /// An output as `WIDTHxHEIGHT[@SCALE][:MM]`: its mode in pixels, the
    /// scale the compositor gives it (1 when left out) and its panel's
    /// width in millimetres. With no width, a logical pixel is taken to be
    /// a 96th of an inch, as a display with no EDID is.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let wrong = || format!("{text:?} is not WIDTHxHEIGHT[@SCALE][:MM], like 1920x1080@1.25");
        let (mode, mm) = match text.split_once(':') {
            Some((mode, mm)) => (mode, Some(mm.parse::<f64>().map_err(|_| wrong())?)),
            None => (text, None),
        };
        let (pixels, scale) = match mode.split_once('@') {
            Some((pixels, scale)) => (pixels, scale.parse::<f64>().map_err(|_| wrong())?),
            None => (mode, 1.0),
        };
        let (width, height) = pixels.split_once('x').ok_or_else(wrong)?;
        let (width, height): (u32, u32) = (
            width.parse().map_err(|_| wrong())?,
            height.parse().map_err(|_| wrong())?,
        );

        let positive = |value: f64| value.is_finite() && value > 0.0;

        if width == 0 || height == 0 || !positive(scale) || mm.is_some_and(|mm| !positive(mm)) {
            return Err(wrong());
        }

        let mut output = Self {
            width,
            height,
            scale,
            display: Display {
                mm_per_vpx: 0.0,
                estimated: mm.is_none(),
            },
        };
        let pixel = f64::from(output.pixel_scale());

        output.display.mm_per_vpx = match mm {
            Some(mm) => mm / f64::from(width) * pixel,
            None => pixel / scale * 25.4 / 96.0,
        };

        Ok(output)
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
    pub repaint: Repaint,
}

/// A frame presented on a live surface.
#[derive(Debug, Clone, Copy)]
pub struct Repaint {
    /// The share of the output repainted: the damage the software compositor
    /// works out against the frame before.
    pub share: f32,
    /// How long repainting it took.
    pub took: Duration,
}

/// What a live surface keeps between frames: its pixels, the layers they
/// were drawn from, and its clip mask.
struct Surface {
    pixels: Vec<u8>,
    layers: Vec<iced_tiny_skia::Layer>,
    mask: tiny_skia::Mask,
}

/// A renderer drawing sheets offscreen for one output, keeping what a live
/// surface keeps between frames.
pub struct Studio<'a> {
    renderer: iced_renderer::Renderer,
    cache: user_interface::Cache,
    /// The surface frames are presented on, once there is one.
    surface: Option<Surface>,
    subjects: &'a [Box<dyn Subject>],
    output: Output,
    theme: &'a Theme,
    date: &'a str,
    plot: PlotStyle,
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
            surface: None,
            subjects,
            output,
            theme,
            date,
            plot: PlotStyle::default(),
        })
    }

    /// The studio plotting its sheets in `style`.
    pub fn plotting(self, style: PlotStyle) -> Self {
        Self {
            plot: style,
            ..self
        }
    }

    /// The sheets starting with subject `first`, `elapsed` seconds in, as
    /// RGBA in virtual pixels.
    pub fn frame(&mut self, first: usize, elapsed: f32) -> RgbaImage {
        let _ = self.draw(first, elapsed);
        self.rasterize()
    }

    /// The sheets starting with subject `first`, `elapsed` seconds in.
    fn sheet(&self, first: usize, elapsed: f32) -> Sheet<'a> {
        // The same seed every time: a moment drawn twice is the same sheet.
        let showing = Schedule::new(
            self.subjects.iter().map(|s| s.card().parts.len()).collect(),
            0,
            Some(first),
            self.plot.length,
        )
        .at(0, elapsed);

        Sheet {
            subject: self.subjects[showing.subject].as_ref(),
            number: showing.subject + 1,
            of: self.subjects.len(),
            showing,
            display: self.output.display,
            date: self.date,
            plot: self.plot,
        }
    }

    /// The plot of subject `first`'s sheet, worked out as its first frame
    /// works it out.
    pub fn plot(&self, first: usize) -> Plot {
        let (width, height) = self.output.virtual_size();

        sheet::plot_of(&self.sheet(first, 0.0), (width as i32, height as i32))
    }

    /// Lays out and draws the sheet into the renderer's layers.
    fn draw(&mut self, first: usize, elapsed: f32) -> Showing {
        let sheet = self.sheet(first, elapsed);
        let showing = sheet.showing;
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

    /// The software renderer, which draws what a live surface shows.
    fn software(&mut self) -> &mut iced_tiny_skia::Renderer {
        match &mut self.renderer {
            iced_renderer::fallback::Renderer::Secondary(renderer) => renderer,
            iced_renderer::fallback::Renderer::Primary(_) => {
                unreachable!("the studio asks for the software renderer")
            }
        }
    }

    /// The output as the software renderer draws to it, and all of it.
    fn viewport(&self) -> (iced_graphics::Viewport, Rectangle) {
        let (width, height) = self.output.virtual_size();
        let viewport = iced_graphics::Viewport::with_physical_size(
            Size::new(width, height),
            iced_core::renderer::Scale {
                window: 1.0,
                application: 1.0,
            },
        );
        let whole = Rectangle::with_size(viewport.logical_size());

        (viewport, whole)
    }

    /// Presents the frame just drawn as a live surface does: repainting only
    /// what the damage since the surface's last frame covers.
    fn present(&mut self) -> Repaint {
        let (width, height) = self.output.virtual_size();
        let (viewport, whole) = self.viewport();
        let background = self.theme.palette().void;
        let layers = self.software().layers().to_vec();

        let damage = match &self.surface {
            Some(surface) => damage::group(
                damage::diff(
                    &surface.layers,
                    &layers,
                    |layer| vec![layer.bounds],
                    iced_tiny_skia::Layer::damage,
                ),
                whole,
            ),
            None => vec![whole],
        };
        let mut surface = self.surface.take().unwrap_or_else(|| Surface {
            pixels: vec![0; width as usize * height as usize * 4],
            layers: Vec::new(),
            mask: tiny_skia::Mask::new(width, height).expect("A clip mask"),
        });

        let started = Instant::now();
        self.software().draw(
            &mut tiny_skia::PixmapMut::from_bytes(&mut surface.pixels, width, height)
                .expect("A pixmap"),
            &mut surface.mask,
            &viewport,
            &damage,
            background,
        );
        let took = started.elapsed();

        surface.layers = layers;
        self.surface = Some(surface);

        Repaint {
            share: (damage.iter().map(Rectangle::area).sum::<f32>() / whole.area()).min(1.0),
            took,
        }
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
    /// repainting what changed of it (what the software compositor does
    /// next, before it upscales that, which is not measured here).
    pub fn bench(&mut self, first: usize, fps: f32) -> Vec<Timed> {
        let parts = self.subjects[first].card().parts.len();
        let frames = (timeline::duration(parts, self.plot.length) * fps) as usize;

        (0..frames)
            .map(|frame| {
                let started = Instant::now();
                let showing = self.draw(first, frame as f32 / fps);
                let drawn = started.elapsed();

                Timed {
                    stage: Stage::of(showing.moment),
                    draw: drawn,
                    repaint: self.present(),
                }
            })
            .collect()
    }
}

/// What `program` draws on a canvas `size` virtual pixels across and
/// down in `theme`, as RGBA: a part of a sheet drawn on its own.
#[cfg(test)]
pub fn screenshot<P>(program: P, size: (u32, u32), theme: &Theme) -> Vec<u8>
where
    P: canvas::Program<(), Theme, iced_renderer::Renderer>,
{
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
    .expect("A software renderer");
    let element: iced_core::Element<'_, (), Theme, iced_renderer::Renderer> = canvas(program)
        .width(Length::Fill)
        .height(Length::Fill)
        .boxed();
    let mut interface = UserInterface::build(
        element,
        Size::new(size.0 as f32, size.1 as f32),
        user_interface::Cache::default(),
        &mut renderer,
    );
    let palette = theme.palette();

    interface.draw(
        &mut renderer,
        theme,
        &Style {
            text_color: palette.ink,
        },
        mouse::Cursor::Unavailable,
    );

    renderer.screenshot(Size::new(size.0, size.1), 1.0, palette.void)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Machine;

    /// Any output can be asked for by its mode: the desk's laptop as it is
    /// written, and a monitor whose size is not given as a guess.
    #[test]
    fn an_output_is_read_from_its_mode() {
        let laptop: Output = "2560x1600@1.666667:344.6".parse().unwrap();

        assert_eq!(laptop.virtual_size(), Output::LAPTOP.virtual_size());
        assert!((laptop.display.mm_per_vpx - Output::LAPTOP.display.mm_per_vpx).abs() < 1e-9);
        assert!(!laptop.display.estimated);

        let monitor: Output = "1366x768".parse().unwrap();

        assert_eq!(monitor.virtual_size(), (683, 384));
        assert!(monitor.display.estimated);
        assert!((monitor.display.mm_per_vpx - 2.0 * 25.4 / 96.0).abs() < 1e-9);

        for wrong in [
            "",
            "1366",
            "1366x",
            "x768",
            "1366x768@",
            "1366x768@0",
            "1366x768:-3",
        ] {
            assert!(wrong.parse::<Output>().is_err(), "{wrong:?}");
        }
    }

    /// What a surface keeps from one frame to the next never leaks from one
    /// sheet into another: a studio that drew other subjects first draws a
    /// sheet as a fresh one does.
    #[test]
    fn a_kept_drawing_belongs_to_its_sheet() {
        load_fonts();

        let subjects = crate::subjects::all(&Machine::fixture());
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

    impl Studio<'_> {
        /// The frame just drawn, drawn whole onto a surface of its own.
        fn whole(&mut self) -> Vec<u8> {
            let (width, height) = self.output.virtual_size();
            let (viewport, whole) = self.viewport();
            let background = self.theme.palette().void;
            let mut pixels = vec![0; width as usize * height as usize * 4];

            self.software().draw(
                &mut tiny_skia::PixmapMut::from_bytes(&mut pixels, width, height)
                    .expect("A pixmap"),
                &mut tiny_skia::Mask::new(width, height).expect("A clip mask"),
                &viewport,
                &[whole],
                background,
            );

            pixels
        }
    }

    /// Repaints a sheet frame by frame at `fps` over `seconds`, and fails at
    /// the first frame that differs from the same frame drawn whole.
    fn repaints_as_drawn(first: usize, output: Output, seconds: std::ops::Range<f32>, fps: f32) {
        repaints_as_drawn_in(PlotStyle::default(), first, output, seconds, fps);
    }

    /// [`repaints_as_drawn`], the sheet plotted in `style`.
    fn repaints_as_drawn_in(
        style: PlotStyle,
        first: usize,
        output: Output,
        seconds: std::ops::Range<f32>,
        fps: f32,
    ) {
        load_fonts();

        let subjects = crate::subjects::all(&Machine::fixture());
        let theme = Theme::TERMINAL;
        let mut studio = Studio::new(&subjects, output, &theme, "2026-10-07")
            .unwrap()
            .plotting(style);
        let frames = ((seconds.end - seconds.start) * fps) as usize;

        for frame in 0..frames {
            let at = seconds.start + frame as f32 / fps;
            let _ = studio.draw(first, at);
            let _ = studio.present();
            let whole = studio.whole();
            let repainted = &studio.surface.as_ref().expect("A surface").pixels;
            let wrong = repainted
                .chunks_exact(4)
                .zip(whole.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count();

            assert!(
                wrong == 0,
                "{}, {}, {at:.2} s in: {wrong} pixels repainted wrong",
                subjects[first].name(),
                style.name
            );
        }
    }

    /// A surface repaints only what changed since its last frame, and shows
    /// what drawing the whole frame would: through a sheet's plot, its
    /// detail coming in, and the wipe to the next.
    #[test]
    fn repainting_the_damage_draws_the_whole_frame() {
        repaints_as_drawn(0, Output::LAPTOP, 0.0..16.0, 4.0);
        repaints_as_drawn(0, Output::LAPTOP, 40.0..48.0, 4.0);
    }

    /// Every sheet, on both outputs, at a live frame rate.
    #[test]
    #[ignore = "minutes: run with --release --ignored"]
    fn every_sheet_repaints_as_drawn() {
        for first in 0..crate::subjects::all(&Machine::fixture()).len() {
            for output in [Output::LAPTOP, Output::ULTRAWIDE] {
                repaints_as_drawn(first, output, 0.0..80.0, 30.0);
            }
        }
    }

    /// The pen's plot repaints as drawn too: its buckets kept, its strokes
    /// drawn a stretch at a time, its head going up and down.
    #[test]
    fn the_pens_plot_repaints_as_drawn() {
        repaints_as_drawn_in(PlotStyle::DRAFTING, 0, Output::LAPTOP, 0.0..12.0, 4.0);
    }

    /// Every sheet's plot in every style, on both outputs, at ten frames a
    /// second.
    #[test]
    #[ignore = "minutes: run with --release --ignored"]
    fn every_plot_repaints_as_drawn() {
        for style in [PlotStyle::CAROUSEL, PlotStyle::DRAFTING, PlotStyle::QUICK] {
            for first in 0..crate::subjects::all(&Machine::fixture()).len() {
                for output in [Output::LAPTOP, Output::ULTRAWIDE] {
                    let end = timeline::PLOT_START + style.length + 0.5;

                    repaints_as_drawn_in(style, first, output, 0.0..end, 10.0);
                }
            }
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

    /// Lines of lettering, set by the renderer's text or as the font's
    /// pixels, or as a sheet's lettering inside a clip.
    struct Specimen {
        pixels: bool,
        lines: Vec<String>,
        clip: Option<iced_core::Rectangle<i32>>,
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

                if let Some(clip) = self.clip {
                    let piece = crate::draft::raster::Piece::Text {
                        at,
                        text: line.clone(),
                    };
                    let theme = Theme::TERMINAL;
                    let palette = theme.palette();

                    let _ = piece.draw(&mut pen, palette.ink, palette.void, usize::MAX, clip);
                } else if self.pixels {
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
        screenshot(specimen, size, &Theme::TERMINAL)
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
                clip: None,
            },
            size,
        );
        let pixels = render(
            Specimen {
                pixels: true,
                lines,
                clip: None,
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

    /// Lettering a clip would cut is left out whole: a value cut short reads
    /// as a different value.
    #[test]
    fn lettering_is_drawn_whole_or_not_at_all() {
        load_fonts();

        let inked = |clip| {
            let background = Theme::TERMINAL.palette().void;
            let void = [
                (background.r * 255.0).round() as u8,
                (background.g * 255.0).round() as u8,
                (background.b * 255.0).round() as u8,
            ];

            render(
                Specimen {
                    pixels: true,
                    lines: vec!["1:5×10⁸".to_owned()],
                    clip: Some(clip),
                },
                (60, 20),
            )
            .chunks_exact(4)
            .filter(|pixel| pixel[..3] != void)
            .count()
        };

        assert!(
            inked(iced_core::Rectangle {
                x: 0,
                y: 0,
                width: 60,
                height: 20
            }) > 0
        );
        assert_eq!(
            inked(iced_core::Rectangle {
                x: 0,
                y: 0,
                width: 30,
                height: 20
            }),
            0
        );
    }
}
