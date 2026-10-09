//! What the pen draws: the strokes a sheet's pieces become.
//!
//! A stroke is one motion of the pen: the pixels it passes over in order,
//! and which of them it inks. A dashed line is one stroke, the pen bouncing
//! over its gaps; lining is a stroke along each of its diagonals, back and
//! forth across the area; an area of solid or tint is one stroke back and
//! forth along its rows; a letter is a touch of the pen that sets it whole,
//! or the strokes of its glyph traced in turn; a dot is a touch. Which
//! pixels a stroke inks is worked out once, in the direction it was drawn,
//! and carried with them, so turning a stroke round or starting a loop
//! elsewhere never moves a dash.
use std::collections::BTreeMap;
use std::ops::Range;

use iced_core::{Point, Rectangle};

use crate::draft::raster::{self, Inked, LETTERING, Piece, Texture};
use crate::draft::{Tone, glyphs, letters};

use super::own::{Owners, painted};
use super::style::{Glyphs, Lining};

/// One motion of the pen.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    /// The pixels the pen passes over, in order; a touch's, all at once.
    pub pixels: Vec<Point<i32>>,
    /// Which of them it inks: those its line type lights and it owns.
    pub lit: Vec<bool>,
    pub tone: Tone,
    pub form: Form,
    /// What it draws: the mark, by its index among the plot's, and the
    /// piece of the mark.
    pub mark: usize,
    pub piece: usize,
}

/// How the pen goes along a stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// From its first pixel to its last.
    Line,
    /// Round and back to where it began, which may be anywhere on it.
    Loop,
    /// A circle round this centre as a plotter's circle instruction draws
    /// it: from the centre out to its first pixel, at three o'clock, round
    /// anticlockwise, and back to the centre.
    Circle(Point<i32>),
    /// A stroke of a letter, from its first pixel to its last: the way the
    /// letter is written, never turned round.
    Glyph,
    /// A touch where the pen stands, setting a letter or a dot whole.
    Touch(Point<i32>),
}

impl Stroke {
    /// Where the pen starts it.
    pub fn start(&self) -> Point<i32> {
        match self.form {
            Form::Touch(at) | Form::Circle(at) => at,
            Form::Line | Form::Loop | Form::Glyph => self.pixels[0],
        }
    }

    /// Where the pen leaves it.
    pub fn end(&self) -> Point<i32> {
        match self.form {
            Form::Touch(at) | Form::Circle(at) => at,
            Form::Loop => self.pixels[0],
            Form::Line | Form::Glyph => self.pixels[self.pixels.len() - 1],
        }
    }

    /// Whether the pen goes round it and back to its first pixel.
    pub fn closed(&self) -> bool {
        matches!(self.form, Form::Loop | Form::Circle(_))
    }

    /// How many pixels it inks.
    pub fn ink(&self) -> usize {
        self.lit.iter().filter(|lit| **lit).count()
    }

    /// The pixels it inks among `passed`, those the pen passes over by
    /// their places along it.
    pub fn inked(&self, passed: Range<usize>) -> impl Iterator<Item = Point<i32>> + '_ {
        self.pixels
            .iter()
            .zip(&self.lit)
            .take(passed.end)
            .skip(passed.start)
            .filter(|(_, lit)| **lit)
            .map(|(pixel, _)| *pixel)
    }

    /// The same stroke drawn the other way: a line from its end, a loop
    /// the other way round from the same pixel.
    pub fn reverse(&mut self) {
        match self.form {
            Form::Line => {
                self.pixels.reverse();
                self.lit.reverse();
            }
            Form::Loop => {
                self.pixels[1..].reverse();
                self.lit[1..].reverse();
            }
            Form::Circle(_) | Form::Glyph | Form::Touch(_) => {}
        }
    }

    /// A loop started at its pixel `start`.
    pub fn rotate(&mut self, start: usize) {
        if self.form == Form::Loop {
            self.pixels.rotate_left(start);
            self.lit.rotate_left(start);
        }
    }
}

/// How a sheet's pieces are made strokes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Making {
    pub lining: Lining,
    pub glyphs: Glyphs,
}

/// The strokes of `inked`, piece `piece` of mark `mark`, which is piece
/// `id` of the drawing: inking only what it owns in `owners`, inside
/// `clip`, its lining and lettering drawn as `making` says. A circle round
/// `centre`, if one is given, is drawn from it as a plotter draws a circle.
/// A stroke that would ink nothing is left out.
pub fn strokes(
    inked: &Inked,
    (mark, piece, id): (usize, usize, u32),
    owners: &Owners,
    clip: Rectangle<i32>,
    making: Making,
    centre: Option<Point<i32>>,
) -> Vec<Stroke> {
    let owned = |pixel: &Point<i32>| owners.owns(id, *pixel);
    let stroke = |pixels: Vec<Point<i32>>, lit: Vec<bool>, form: Form| Stroke {
        pixels,
        lit,
        tone: inked.tone,
        form,
        mark,
        piece,
    };
    let mut strokes = Vec::new();

    match &inked.piece {
        Piece::Path {
            pixels,
            stipple,
            phase,
        } => {
            // From the first pixel inside the clip to the last: the pen
            // stays on the drawing.
            let inside = |pixel: &Point<i32>| raster::contains(clip, *pixel);

            if let (Some(first), Some(last)) = (
                pixels.iter().position(inside),
                pixels.iter().rposition(inside),
            ) {
                let lit = (first..=last)
                    .map(|index| stipple.lights(phase + index) && owned(&pixels[index]))
                    .collect();
                let made = path(pixels[first..=last].to_vec(), lit, stroke);

                strokes.push(match centre {
                    // A circle the clip leaves whole.
                    Some(centre) if made.form == Form::Loop && last + 1 - first == pixels.len() => {
                        centred(made, centre)
                    }
                    _ => made,
                });
            }
        }
        Piece::Rows { texture, .. } => {
            let diagonal = match (texture, making.lining) {
                (Texture::Rising(_), Lining::Diagonals) => Some(true),
                (Texture::Falling(_), Lining::Diagonals) => Some(false),
                _ => None,
            };

            match diagonal {
                Some(rising) => {
                    let mut lined = Vec::new();
                    painted(&inked.piece, clip, |pixel| lined.push(pixel));

                    for pixels in diagonals(lined, rising) {
                        let lit = pixels.iter().map(owned).collect();
                        strokes.push(path(pixels, lit, stroke));
                    }
                }
                None => {
                    let pixels = serpentine(&inked.piece, clip);

                    if !pixels.is_empty() {
                        let lit = pixels.iter().map(owned).collect();
                        strokes.push(path(pixels, lit, stroke));
                    }
                }
            }
        }
        Piece::Text { at, text } if raster::shows(*at, text, clip) => match making.glyphs {
            Glyphs::Touched => {
                let advance = i32::from(LETTERING.advance());
                let middle = at.y + i32::from(LETTERING.cap_top()) + i32::from(LETTERING.cap()) / 2;

                for (index, glyph) in letters::pixels(text, *at).into_iter().enumerate() {
                    let centre = Point::new(at.x + index as i32 * advance + advance / 2, middle);
                    let lit = glyph.iter().map(owned).collect();

                    strokes.push(stroke(glyph, lit, Form::Touch(centre)));
                }
            }
            Glyphs::Traced => {
                for trail in glyphs::strokes(text, *at).into_iter().flatten() {
                    let lit = trail.iter().map(owned).collect();
                    let form = match trail.as_slice() {
                        [dot] => Form::Touch(*dot),
                        _ => Form::Glyph,
                    };

                    strokes.push(stroke(trail, lit, form));
                }
            }
        },
        // Lettering the clip would cut is left out whole.
        Piece::Text { .. } => {}
        Piece::Block(bounds) => {
            let mut pixels = Vec::new();
            painted(&inked.piece, clip, |pixel| pixels.push(pixel));

            let centre = Point::new(
                bounds.x + (bounds.width - 1) / 2,
                bounds.y + (bounds.height - 1) / 2,
            );
            let lit = pixels.iter().map(owned).collect();

            strokes.push(stroke(pixels, lit, Form::Touch(centre)));
        }
        // The ground it clears is a gap the pen leaves.
        Piece::Knockout(_) => {}
    }

    strokes.retain(|stroke| stroke.lit.iter().any(|lit| *lit));
    strokes
}

/// A run of pixels as the pen draws it: a loop if it ends where it began or
/// beside it, a touch if it is one pixel.
fn path(
    mut pixels: Vec<Point<i32>>,
    mut lit: Vec<bool>,
    stroke: impl Fn(Vec<Point<i32>>, Vec<bool>, Form) -> Stroke,
) -> Stroke {
    let (first, last) = (pixels[0], pixels[pixels.len() - 1]);
    let beside = (first.x - last.x).abs() <= 1 && (first.y - last.y).abs() <= 1;

    if pixels.len() == 1 {
        return stroke(pixels, lit, Form::Touch(first));
    }

    if pixels.len() >= 4 && beside {
        // A loop passes its first pixel once.
        if first == last {
            pixels.pop();
            let closing = lit.pop().unwrap_or(false);
            lit[0] |= closing;
        }

        return stroke(pixels, lit, Form::Loop);
    }

    stroke(pixels, lit, Form::Line)
}

/// `circle`, a loop round `centre`, drawn as a plotter's circle instruction
/// draws it: from its pixel at three o'clock, anticlockwise.
pub fn centred(mut circle: Stroke, centre: Point<i32>) -> Stroke {
    let count = circle.pixels.len();
    // Level with the centre and furthest right of it.
    let start = (0..count)
        .min_by_key(|&index| {
            let pixel = circle.pixels[index];

            ((pixel.y - centre.y).abs(), centre.x - pixel.x, index)
        })
        .unwrap_or(0);

    circle.rotate(start);

    // Anticlockwise on the sheet, whose y grows downwards: upwards first.
    if circle.pixels[1].y > circle.pixels[count - 1].y {
        circle.reverse();
    }

    circle.form = Form::Circle(centre);
    circle
}

/// The pixels of an area's rows inside `clip`, back and forth: the first
/// row left to right, the next right to left.
fn serpentine(piece: &Piece, clip: Rectangle<i32>) -> Vec<Point<i32>> {
    let Piece::Rows { rows, .. } = piece else {
        return Vec::new();
    };
    let (left, right) = (clip.x, clip.x + clip.width - 1);
    let mut pixels = Vec::new();
    let mut forwards = true;

    for &(y, from, to) in rows {
        let (from, to) = (from.max(left), to.min(right));

        if y < clip.y || y >= clip.y + clip.height || from > to {
            continue;
        }

        if forwards {
            pixels.extend((from..=to).map(|x| Point::new(x, y)));
        } else {
            pixels.extend((from..=to).rev().map(|x| Point::new(x, y)));
        }

        forwards = !forwards;
    }

    pixels
}

/// Lining's pixels as the lines they are: each diagonal run of them, top
/// to bottom and bottom to top in turn across the area, so the pen goes
/// back and forth. Lines `rising` to the right join pixels a step left and
/// down from each other, falling ones a step right and down.
fn diagonals(pixels: Vec<Point<i32>>, rising: bool) -> Vec<Vec<Point<i32>>> {
    let mut lines: BTreeMap<i32, Vec<Point<i32>>> = BTreeMap::new();

    for pixel in pixels {
        let line = if rising {
            pixel.x + pixel.y
        } else {
            pixel.x - pixel.y
        };

        lines.entry(line).or_default().push(pixel);
    }

    let mut runs = Vec::new();

    for (index, mut line) in lines.into_values().enumerate() {
        line.sort_by_key(|pixel| pixel.y);
        line.dedup();

        let mut cut: Vec<Vec<Point<i32>>> = Vec::new();

        for pixel in line {
            match cut.last_mut() {
                Some(run) if run[run.len() - 1].y + 1 == pixel.y => run.push(pixel),
                _ => cut.push(vec![pixel]),
            }
        }

        if index % 2 == 1 {
            cut.reverse();

            for run in &mut cut {
                run.reverse();
            }
        }

        runs.extend(cut);
    }

    runs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::Line;
    use crate::draft::raster::{Stipple, rect};
    use quadrille::draw::{Polygon, shape};

    const CLIP: Rectangle<i32> = rect(0, 0, 200, 200);

    /// The strokes of `piece`, alone on a sheet, drawn as `lining` says.
    fn alone(piece: &Piece, lining: Lining) -> Vec<Stroke> {
        made(
            piece,
            Making {
                lining,
                glyphs: Glyphs::Touched,
            },
            None,
        )
    }

    /// The strokes of `piece`, alone on a sheet, made as `making` says,
    /// round `centre` if it is a circle drawn from its centre.
    fn made(piece: &Piece, making: Making, centre: Option<Point<i32>>) -> Vec<Stroke> {
        let mut owners = Owners::new(200, 200);
        owners.paint(1, piece, CLIP);

        let inked = Inked {
            piece: piece.clone(),
            tone: Tone::Ink,
        };

        strokes(&inked, (0, 0, 1), &owners, CLIP, making, centre)
    }

    /// Pixels in an order of their own, each once.
    fn sorted(pixels: impl IntoIterator<Item = Point<i32>>) -> Vec<(i32, i32)> {
        let mut pixels: Vec<(i32, i32)> = pixels.into_iter().map(|p| (p.x, p.y)).collect();
        pixels.sort_unstable();
        pixels.dedup();
        pixels
    }

    /// What `piece` paints whole.
    fn painted_by(piece: &Piece) -> Vec<(i32, i32)> {
        let mut pixels = Vec::new();
        painted(piece, CLIP, |pixel| pixels.push(pixel));
        sorted(pixels)
    }

    /// What `strokes` ink, all the way along.
    fn inked_by(strokes: &[Stroke]) -> Vec<(i32, i32)> {
        sorted(
            strokes
                .iter()
                .flat_map(|stroke| stroke.inked(0..stroke.pixels.len())),
        )
    }

    /// A dashed circle turned round, or started anywhere, inks the pixels
    /// it inked, its dashes where they were.
    #[test]
    fn a_turned_dashed_loop_lights_the_same_pixels() {
        let piece = Piece::path(
            raster::ordered_circle(Point::new(100, 100), 37),
            Stipple::of(Line::Hidden),
        );
        let mut made = alone(&piece, Lining::Rows);

        assert_eq!(made.len(), 1);
        assert_eq!(made[0].form, Form::Loop);
        assert_eq!(inked_by(&made), painted_by(&piece));

        let whole = inked_by(&made);

        made[0].reverse();
        assert_eq!(inked_by(&made), whole);
        made[0].rotate(41);
        assert_eq!(inked_by(&made), whole);
        made[0].reverse();
        made[0].rotate(7);
        assert_eq!(inked_by(&made), whole);
    }

    /// Lining drawn along its diagonals inks exactly the pixels its rows
    /// do, each diagonal a run of touching pixels.
    #[test]
    fn lining_along_its_diagonals_lights_its_rows() {
        let rows = Polygon::new([
            Point::new(20, 30),
            Point::new(150, 18),
            Point::new(170, 120),
            Point::new(90, 80),
            Point::new(30, 150),
        ])
        .rows();

        for texture in [Texture::Rising(4), Texture::Falling(4)] {
            let piece = Piece::Rows {
                rows: rows.clone(),
                texture,
                origin: Point::new(20, 30),
            };
            let made = alone(&piece, Lining::Diagonals);

            assert_eq!(inked_by(&made), painted_by(&piece), "{texture:?}");
            assert!(made.len() > 10);

            for stroke in &made {
                for pair in stroke.pixels.windows(2) {
                    assert_eq!((pair[0].y - pair[1].y).abs(), 1, "{texture:?}");
                    assert_eq!((pair[0].x - pair[1].x).abs(), 1, "{texture:?}");
                }
            }
        }
    }

    /// An area drawn back and forth along its rows inks what it paints,
    /// and so do a line of lettering set a letter at a time, a dot and a
    /// chain line running off the sheet's drawing.
    #[test]
    fn rows_lettering_and_dots_light_what_they_paint() {
        let triangle =
            Polygon::new([Point::new(10, 10), Point::new(60, 14), Point::new(30, 50)]).rows();

        for piece in [
            Piece::Rows {
                rows: triangle.clone(),
                texture: Texture::Bayer(6),
                origin: Point::new(10, 10),
            },
            Piece::Rows {
                rows: triangle,
                texture: Texture::Rising(4),
                origin: Point::new(10, 10),
            },
            Piece::Text {
                at: Point::new(12, 40),
                text: "Ø8 H7/k6".into(),
            },
            Piece::Block(rect(5, 5, 3, 3)),
            Piece::path(
                shape::line(Point::new(-20, 5), Point::new(80, 90)),
                Stipple::of(Line::Centre),
            ),
        ] {
            assert_eq!(
                inked_by(&alone(&piece, Lining::Rows)),
                painted_by(&piece),
                "{piece:?}"
            );
        }
    }

    /// What another piece paints over a stroke is left for it: the ground
    /// under lettering is a gap in the line it crosses.
    #[test]
    fn a_stroke_inks_only_what_it_owns() {
        let line = Piece::path(
            shape::line(Point::new(10, 50), Point::new(150, 50)),
            Stipple::Solid,
        );
        let ground = Piece::Knockout(rect(60, 45, 20, 10));
        let mut owners = Owners::new(200, 200);

        owners.paint(1, &line, CLIP);
        owners.paint(2, &ground, CLIP);

        let made = strokes(
            &Inked {
                piece: line,
                tone: Tone::Line,
            },
            (0, 0, 1),
            &owners,
            CLIP,
            Making {
                lining: Lining::Rows,
                glyphs: Glyphs::Touched,
            },
            None,
        );

        assert_eq!(made.len(), 1);
        assert_eq!(made[0].pixels.len(), 141);
        assert_eq!(made[0].ink(), 141 - 20);
        assert!(inked_by(&made).iter().all(|&(x, _)| !(60..80).contains(&x)));
    }

    /// Lettering traced a stroke at a time inks what it paints, its letters
    /// left to right, each stroke the way it is written.
    #[test]
    fn traced_lettering_lights_what_it_paints() {
        let piece = Piece::Text {
            at: Point::new(12, 40),
            text: "Ø8 H7/k6 R43.3".into(),
        };
        let traced = made(
            &piece,
            Making {
                lining: Lining::Rows,
                glyphs: Glyphs::Traced,
            },
            None,
        );

        let advance = i32::from(LETTERING.advance());
        let letter = |stroke: &Stroke| (stroke.pixels[0].x - 12).div_euclid(advance);

        assert_eq!(inked_by(&traced), painted_by(&piece));
        assert!(traced.len() > 14);
        assert!(
            traced
                .windows(2)
                .all(|pair| letter(&pair[0]) <= letter(&pair[1]))
        );
        assert!(
            traced
                .iter()
                .all(|stroke| matches!(stroke.form, Form::Glyph | Form::Touch(_)))
        );
    }

    /// A circle drawn from its centre starts level with it on the right
    /// and goes up from there, round to where it began, with its dashes
    /// where they were.
    #[test]
    fn a_circle_from_its_centre_starts_at_three_oclock_and_goes_up() {
        let centre = Point::new(100, 100);
        let piece = Piece::path(
            raster::ordered_circle(centre, 37),
            Stipple::of(Line::Phantom),
        );
        let made = made(
            &piece,
            Making {
                lining: Lining::Rows,
                glyphs: Glyphs::Touched,
            },
            Some(centre),
        );

        assert_eq!(made.len(), 1);
        assert_eq!(made[0].form, Form::Circle(centre));
        assert_eq!(made[0].pixels[0], Point::new(137, 100));
        assert_eq!(made[0].pixels[1].y, 99);
        assert_eq!((made[0].start(), made[0].end()), (centre, centre));
        assert_eq!(inked_by(&made), painted_by(&piece));
    }
}
