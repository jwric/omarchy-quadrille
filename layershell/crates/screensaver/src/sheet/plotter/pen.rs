//! The pen's head, as the sheet shows it, and its carriage.
//!
//! A crosshair in the accent, the palette's colour for the machine at work:
//! close round the pixel just drawn while the pen is down, which shows
//! through its dark centre; open while it is carried up; and with its
//! centre lit for the frame the pen settles after a long move.
//!
//! The carriage the head rides on shows as a tick in the sheet's zone bands
//! above and below the head.
//!
//! A draughtsman's pen leaves its ink wet behind it for a moment, in the
//! accent until it dries to its tone, a tail that is longer the faster the
//! pen goes; and draws a circle with a compass, whose arm reaches from its
//! point at the centre out to the pen.
//!
//! The renderer repaints each place a frame changes, every drawing that
//! crosses it again, so how the ticks are drawn decides what they cost:
//! each on its own, not the height of the sheet between them. There are no
//! ticks either side of the head as well: two more places to repaint every
//! frame would take the plot past its share of the frame.
use iced_core::{Point, Rectangle};
use iced_widget::graphics::geometry;
use quadrille::Palette;
use quadrille::draw::Pen;

use crate::draft::raster;

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
/// each a path of its own, so each is repainted alone and not the height
/// of the sheet between them.
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
