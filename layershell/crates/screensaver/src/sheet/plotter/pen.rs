//! The pen's head, as the sheet shows it.
//!
//! A crosshair in the accent, the palette's colour for the machine at work:
//! close round the pixel just drawn while the pen is down, which shows
//! through its dark centre; open while it is carried up; and with its
//! centre lit for the frame the pen settles after a long move.
use iced_core::Point;
use iced_widget::graphics::geometry;
use quadrille::Palette;
use quadrille::draw::Pen;

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
