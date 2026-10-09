//! The pen's head, as the sheet shows it, and its carriage.
//!
//! A crosshair in the accent, the palette's colour for the machine at work:
//! close round the pixel just drawn while the pen is down, which shows
//! through its dark centre; open while it is carried up; with its centre
//! lit for the frame the pen settles after a long move; and round a swatch
//! of the pen it takes up at the carousel.
//!
//! The carriage the head rides on shows as a tick in the sheet's zone bands
//! above and below the head; on a gantry plotter as its arm, a hairline
//! across the sheet under the ink, with a tick at each end.
//!
//! A draughtsman's pen leaves its ink wet behind it for a moment, in the
//! accent until it dries to its tone, a tail that is longer the faster the
//! pen goes; and draws a circle with a compass, whose arm reaches from its
//! point at the centre out to the pen. A quick study's pen leaves a longer
//! tail, a comet, and its hops dotted in it as a plotter's preview shows
//! its moves up.
//!
//! The renderer repaints each place a frame changes, every drawing that
//! crosses it again, so how the ticks are drawn decides what they cost: the
//! two at a gantry's ends as one path, the same column as the arm; with no
//! arm, each on its own, not the height of the sheet between them. There
//! are no ticks either side of the head as well: two more places to repaint
//! every frame would take the plot past its share of the frame.
use iced_core::{Point, Rectangle};
use iced_widget::graphics::geometry;
use quadrille::Palette;
use quadrille::draw::Pen;

use crate::draft::Tone;
use crate::draft::raster::{self, colour, rect};

/// Where the pen's head is, and how it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head {
    pub at: Point<i32>,
    pub pose: Pose,
}

/// How the pen stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pose {
    /// On the paper.
    Down,
    /// Off it: carried, hovering, or at home.
    Up,
    /// Settling onto it after a long move.
    Landing,
    /// At the carousel, taking up the pen in this tone.
    Changing(Tone),
}

impl Head {
    /// The pen down at `at`.
    pub fn down(at: Point<i32>) -> Self {
        Self {
            at,
            pose: Pose::Down,
        }
    }
}

/// Draws the pen's `head`.
pub fn draw<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    head: Head,
    palette: &Palette,
) {
    match head.pose {
        Pose::Down => pen.crosshair(head.at, 3, 1, palette.accent),
        Pose::Up => pen.crosshair(head.at, 2, 3, palette.accent),
        Pose::Landing => {
            pen.crosshair(head.at, 3, 1, palette.accent);
            pen.pixel(head.at, palette.accent);
        }
        Pose::Changing(tone) => {
            pen.crosshair(head.at, 2, 3, palette.accent);
            pen.fill(
                rect(head.at.x - 1, head.at.y - 1, 3, 3),
                colour(palette, tone),
            );
        }
    }
}

/// Draws the ink still wet, `pixels`.
pub fn wet<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    pixels: &[Point<i32>],
    palette: &Palette,
) {
    raster::fill_pixels(pen, pixels, palette.accent);
}

/// Draws the dots of the pen's trail, `stretches` of them, each a path of
/// its own: a stretch that stays the same from one frame to the next is
/// not repainted.
pub fn trail<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    stretches: &[Vec<Point<i32>>],
    palette: &Palette,
) {
    for stretch in stretches {
        raster::fill_pixels(pen, stretch, palette.accent);
        pen.flush();
    }
}

/// Draws a compass drawing round `centre` with the pen at `head`: its arm
/// a faint hairline, its point a dot in the accent.
pub fn compass<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    centre: Point<i32>,
    head: Point<i32>,
    palette: &Palette,
) {
    pen.line(centre, head, palette.faint);
    pen.pixel(centre, palette.accent);
}

/// How long a carriage tick is.
const TICK: i32 = 3;

/// Draws the carriage's ticks above and below a head at `x`: in the zone
/// bands between the sheet's `trim` and its `border`, against the border,
/// as one path, the column a gantry's arm spans.
pub fn ticks_across<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    x: i32,
    trim: Rectangle<i32>,
    border: Rectangle<i32>,
    palette: &Palette,
) {
    let bottom = border.y + border.height - 1;

    if x > trim.x && x < trim.x + trim.width - 1 {
        pen.flush();
        pen.vline(x, border.y - TICK, border.y - 1, palette.accent);
        pen.vline(x, bottom + 1, bottom + TICK, palette.accent);
        pen.flush();
    }
}

/// Draws the carriage's ticks above and below a head at `x` on a plotter
/// with no arm to show where it is: in the zone bands as [`ticks_across`]
/// draws them, each a path of its own, so each is repainted alone and not
/// the height of the sheet between them.
pub fn ticks<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    x: i32,
    trim: Rectangle<i32>,
    border: Rectangle<i32>,
    palette: &Palette,
) {
    let bottom = border.y + border.height - 1;

    if x > trim.x && x < trim.x + trim.width - 1 {
        pen.flush();
        pen.vline(x, border.y - TICK, border.y - 1, palette.accent);
        pen.flush();
        pen.vline(x, bottom + 1, bottom + TICK, palette.accent);
        pen.flush();
    }
}

/// Draws a gantry's arm at `x`: a hairline across the sheet inside its
/// `border`, while it is over the sheet.
pub fn gantry<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    x: i32,
    border: Rectangle<i32>,
    palette: &Palette,
) {
    if x > border.x && x < border.x + border.width - 1 {
        pen.vline(x, border.y + 1, border.y + border.height - 2, palette.edge);
    }
}
