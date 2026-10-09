//! Which piece draws each pixel of the finished drawing.
//!
//! The drawing a sheet shows once its subject runs is its pieces painted
//! one over another: what stays still in the order of its passes, then what
//! moves, over it. [`Owners`] paints them in that order once, keeping for
//! each pixel the piece that painted it last: its owner. A stroke inks only
//! the pixels it owns, so the pen can draw the pieces in any order and
//! finish with exactly that drawing. Nothing it draws is drawn over, and
//! the ground a label is lettered on is a gap the pen leaves, not ink it
//! wipes away.
use iced_core::{Point, Rectangle};

use crate::draft::letters;
use crate::draft::raster::{self, Piece};

/// The owner of every pixel of a sheet.
pub struct Owners {
    width: i32,
    height: i32,
    /// Each pixel's owner, a piece by its number from one; nothing is 0.
    owner: Vec<u32>,
}

impl Owners {
    pub fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            owner: vec![0; (width.max(0) * height.max(0)) as usize],
        }
    }

    /// Paints `piece`, number `id`, over what is painted so far.
    pub fn paint(&mut self, id: u32, piece: &Piece, clip: Rectangle<i32>) {
        painted(piece, clip, |pixel| {
            if let Some(index) = self.index(pixel) {
                self.owner[index] = id;
            }
        });
    }

    /// Whether piece `id` paints `pixel` last.
    pub fn owns(&self, id: u32, pixel: Point<i32>) -> bool {
        self.index(pixel)
            .is_some_and(|index| self.owner[index] == id)
    }

    fn index(&self, pixel: Point<i32>) -> Option<usize> {
        (pixel.x >= 0 && pixel.y >= 0 && pixel.x < self.width && pixel.y < self.height)
            .then(|| (pixel.y * self.width + pixel.x) as usize)
    }
}

/// Every pixel `piece` paints drawn whole inside `clip`, as
/// [`Piece::draw`] paints it; the ground a knockout clears among them.
pub fn painted(piece: &Piece, clip: Rectangle<i32>, mut paint: impl FnMut(Point<i32>)) {
    match piece {
        Piece::Path {
            pixels,
            stipple,
            phase,
        } => {
            for (index, pixel) in pixels.iter().enumerate() {
                if stipple.lights(phase + index) && raster::contains(clip, *pixel) {
                    paint(*pixel);
                }
            }
        }
        Piece::Rows {
            rows,
            texture,
            origin,
        } => {
            let (left, right) = (clip.x, clip.x + clip.width - 1);

            for &(y, from, to) in rows {
                if y < clip.y || y >= clip.y + clip.height {
                    continue;
                }

                for x in from.max(left)..=to.min(right) {
                    if texture.lights(x - origin.x, y - origin.y) {
                        paint(Point::new(x, y));
                    }
                }
            }
        }
        Piece::Text { at, text } => {
            if raster::shows(*at, text, clip) {
                for pixel in letters::pixels(text, *at).into_iter().flatten() {
                    paint(pixel);
                }
            }
        }
        Piece::Block(bounds) | Piece::Knockout(bounds) => {
            if let Some(area) = raster::intersection(*bounds, clip) {
                for y in area.y..area.y + area.height {
                    for x in area.x..area.x + area.width {
                        paint(Point::new(x, y));
                    }
                }
            }
        }
    }
}
