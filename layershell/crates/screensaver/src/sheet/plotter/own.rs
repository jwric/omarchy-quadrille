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
//!
//! The owners also say which marks touch, which a diagram grows along.
use iced_core::{Point, Rectangle};

use crate::draft::letters;
use crate::draft::raster::{self, Piece};

use super::order::Touching;
use super::strokes::Stroke;

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

    /// The marks that touch: those some pixel of whose `strokes` inks is
    /// beside a pixel another owns. `mark` says whose each piece
    /// is, by its number; the ground a label clears counts, so a legend
    /// set on a wire touches it.
    pub fn touching(&self, strokes: &[Stroke], mark: impl Fn(u32) -> usize) -> Touching {
        let mut touching = Touching::new();

        for stroke in strokes {
            for pixel in stroke.inked(0..stroke.pixels.len()) {
                for (dx, dy) in [
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                    (-1, 0),
                    (1, 0),
                    (-1, 1),
                    (0, 1),
                    (1, 1),
                ] {
                    let beside = Point::new(pixel.x + dx, pixel.y + dy);

                    if let Some(index) = self.index(beside)
                        && self.owner[index] != 0
                    {
                        let other = mark(self.owner[index]);

                        if other != stroke.mark {
                            touching.insert((other.min(stroke.mark), other.max(stroke.mark)));
                        }
                    }
                }
            }
        }

        touching
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
        Piece::Path { pixels, stipple } => {
            for (index, pixel) in pixels.iter().enumerate() {
                if stipple.lights(index) && raster::contains(clip, *pixel) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::Tone;
    use crate::draft::raster::{Stipple, rect};
    use crate::sheet::plotter::strokes::Form;
    use quadrille::draw::shape;

    /// Marks touch where a pixel of one's ink is beside one another owns,
    /// the ground a label clears among them: a wire meets a box, a legend
    /// sits on the wire; a line further off touches nothing.
    #[test]
    fn marks_touch_where_their_pixels_meet() {
        let clip = rect(0, 0, 100, 100);
        let wire = shape::line(Point::new(10, 50), Point::new(60, 50));
        let pieces = [
            Piece::path(wire.clone(), Stipple::Solid),
            Piece::path(
                shape::line(Point::new(61, 40), Point::new(61, 60)),
                Stipple::Solid,
            ),
            Piece::Knockout(rect(20, 48, 10, 5)),
            Piece::path(
                shape::line(Point::new(10, 80), Point::new(60, 80)),
                Stipple::Solid,
            ),
        ];
        let mut owners = Owners::new(100, 100);

        for (index, piece) in pieces.iter().enumerate() {
            owners.paint(index as u32 + 1, piece, clip);
        }

        let strokes: Vec<Stroke> = [
            (0, wire),
            (3, shape::line(Point::new(10, 80), Point::new(60, 80))),
        ]
        .into_iter()
        .map(|(mark, pixels)| Stroke {
            lit: pixels
                .iter()
                .map(|pixel| owners.owns(mark as u32 + 1, *pixel))
                .collect(),
            pixels,
            tone: Tone::Ink,
            form: Form::Line,
            mark,
            piece: 0,
        })
        .collect();

        assert_eq!(
            owners.touching(&strokes, |id| id as usize - 1),
            [(0, 1), (0, 2)].into_iter().collect()
        );
    }
}
