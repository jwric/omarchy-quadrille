//! Lettering as a pen letters it: each glyph's pixels as the strokes that
//! draw them.
//!
//! A glyph of the pixel font is a little graph: its pixels, each joined to
//! the pixels beside it, and to one diagonally only where neither pixel of
//! the corner between them is lit, so an L's corner is one turn and not a
//! triangle. The graph is covered with as few trails as a greedy walk
//! finds, each going on as straight as it can, which for the capitals and
//! figures is the least there can be: a stroke for each pair of free ends,
//! one for a ring. A trail starts at a free end, the leftmost and then the
//! highest, as a draughtsman letters; a dot is a stroke of its own.
//!
//! The trails are worked out once for each character and kept.
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex, OnceLock};

use iced_core::Point;

use super::letters;
use super::raster::LETTERING;

/// A pixel of a glyph, from its line box's top-left corner.
type Pixel = (i32, i32);

/// A glyph's strokes, each the pixels it passes over in order.
type Trails = Arc<[Vec<Pixel>]>;

/// The strokes of each character of `text`, set with its line box's
/// top-left corner at `top_left` as [`letters::write`] sets it: each the
/// pixels the pen passes over, in order.
pub fn strokes(text: &str, top_left: Point<i32>) -> Vec<Vec<Vec<Point<i32>>>> {
    let advance = i32::from(LETTERING.advance());

    text.chars()
        .enumerate()
        .map(|(index, character)| {
            let left = top_left.x + index as i32 * advance;

            trails(character)
                .iter()
                .map(|trail| {
                    trail
                        .iter()
                        .map(|&(x, y)| Point::new(left + x, top_left.y + y))
                        .collect()
                })
                .collect()
        })
        .collect()
}

/// The trails of `character`, traced the first time it is asked for.
fn trails(character: char) -> Trails {
    static TRAILS: OnceLock<Mutex<HashMap<char, Trails>>> = OnceLock::new();

    let mut traced = TRAILS
        .get_or_init(Default::default)
        .lock()
        .expect("Trail cache");

    traced
        .entry(character)
        .or_insert_with(|| {
            let pixels = letters::pixels(&character.to_string(), Point::new(0, 0));

            trace(pixels.into_iter().flatten().map(|p| (p.x, p.y)).collect()).into()
        })
        .clone()
}

/// The pixels a stroke may go to next from `pixel`: those beside it, and
/// those diagonal to it across a corner neither of whose pixels is lit.
fn neighbours(lit: &BTreeSet<Pixel>, (x, y): Pixel) -> Vec<Pixel> {
    let beside = [(1, 0), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .map(|(dx, dy)| (x + dx, y + dy))
        .filter(|pixel| lit.contains(pixel));
    let diagonal = [(1, 1), (1, -1), (-1, 1), (-1, -1)]
        .into_iter()
        .filter(|&(dx, dy)| !lit.contains(&(x + dx, y)) && !lit.contains(&(x, y + dy)))
        .map(|(dx, dy)| (x + dx, y + dy))
        .filter(|pixel| lit.contains(pixel));

    beside.chain(diagonal).collect()
}

/// A join between two pixels, the same whichever end it is seen from.
fn join(a: Pixel, b: Pixel) -> (Pixel, Pixel) {
    if a < b { (a, b) } else { (b, a) }
}

/// Trails covering every join of the glyph `lit`, and every pixel alone.
fn trace(lit: BTreeSet<Pixel>) -> Vec<Vec<Pixel>> {
    let graph: BTreeMap<Pixel, Vec<Pixel>> = lit
        .iter()
        .map(|&pixel| (pixel, neighbours(&lit, pixel)))
        .collect();
    let mut used: BTreeSet<(Pixel, Pixel)> = BTreeSet::new();
    let free = |pixel: Pixel, used: &BTreeSet<(Pixel, Pixel)>| {
        graph[&pixel]
            .iter()
            .filter(|&&next| !used.contains(&join(pixel, next)))
            .count()
    };
    let mut trails = Vec::new();

    loop {
        // A free end, the leftmost and then the highest; then a pixel that
        // must end a trail, the highest and then the leftmost; then a ring.
        let start = lit
            .iter()
            .filter(|&&pixel| free(pixel, &used) == 1)
            .min_by_key(|&&(x, y)| (x, y))
            .or_else(|| {
                lit.iter()
                    .filter(|&&pixel| free(pixel, &used) % 2 == 1)
                    .min_by_key(|&&(x, y)| (y, x))
            })
            .or_else(|| {
                lit.iter()
                    .filter(|&&pixel| free(pixel, &used) > 0)
                    .min_by_key(|&&(x, y)| (y, x))
            });
        let Some(&start) = start else {
            break;
        };
        let mut trail = vec![start];
        let mut at = start;
        let mut heading: Option<(i32, i32)> = None;

        // On as straight as it can go; from the start, down and then right,
        // as a pen goes.
        while let Some(next) = graph[&at]
            .iter()
            .copied()
            .filter(|&next| !used.contains(&join(at, next)))
            .max_by(|&a, &b| {
                let score = |(x, y): Pixel| {
                    let (dx, dy) = (f64::from(x - at.0), f64::from(y - at.1));
                    let length = dx.hypot(dy);

                    match heading {
                        Some((hx, hy)) => {
                            (dx * f64::from(hx) + dy * f64::from(hy))
                                / (length * f64::from(hx).hypot(f64::from(hy)))
                        }
                        None => (2.0 * dy + dx) / length,
                    }
                };

                score(a).total_cmp(&score(b))
            })
        {
            used.insert(join(at, next));
            heading = Some((next.0 - at.0, next.1 - at.1));
            at = next;
            trail.push(at);
        }

        trails.push(trail);
    }

    // A pixel joined to none: a full stop, the dot of an i.
    trails.extend(
        lit.iter()
            .filter(|pixel| graph[pixel].is_empty())
            .map(|&pixel| vec![pixel]),
    );

    trails
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The characters a sheet letters with most.
    const SET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789.,:/-+%°×()Ø±√";

    /// A glyph's strokes draw its pixels, every one, and nothing else; each
    /// goes from a pixel to one touching it.
    #[test]
    fn a_glyphs_strokes_draw_its_pixels() {
        for character in SET.chars() {
            let at = Point::new(13, 7);
            let text = character.to_string();
            let mut drawn: Vec<(i32, i32)> = strokes(&text, at)[0]
                .iter()
                .flatten()
                .map(|p| (p.x, p.y))
                .collect();
            let mut lit: Vec<(i32, i32)> = letters::pixels(&text, at)[0]
                .iter()
                .map(|p| (p.x, p.y))
                .collect();

            drawn.sort_unstable();
            drawn.dedup();
            lit.sort_unstable();
            assert_eq!(drawn, lit, "{character}");

            for stroke in &strokes(&text, at)[0] {
                for pair in stroke.windows(2) {
                    let (dx, dy) = (pair[1].x - pair[0].x, pair[1].y - pair[0].y);

                    assert!(dx.abs() <= 1 && dy.abs() <= 1 && (dx, dy) != (0, 0));
                }
            }
        }
    }

    /// Letters take few strokes, as a pen letters them: an O one, an H
    /// three, and the set under two a letter.
    #[test]
    fn letters_take_few_strokes() {
        let count = |character: char| trails(character).len();

        assert_eq!(count('O'), 1);
        assert_eq!(count('L'), 1);
        assert_eq!(count('H'), 3);

        let all: usize = SET.chars().map(count).sum();

        assert!(
            (all as f64) < 2.0 * SET.chars().count() as f64,
            "{all} strokes"
        );
    }

    /// A line of lettering is its letters' strokes, each letter a step on.
    #[test]
    fn a_line_is_its_letters_strokes_a_step_apart() {
        let line = strokes("HH", Point::new(0, 0));
        let advance = i32::from(LETTERING.advance());

        assert_eq!(line.len(), 2);

        for (first, second) in line[0].iter().zip(&line[1]) {
            for (a, b) in first.iter().zip(second) {
                assert_eq!((a.x + advance, a.y), (b.x, b.y));
            }
        }
    }
}
