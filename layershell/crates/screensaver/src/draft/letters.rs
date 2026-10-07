//! Lettering drawn as the font's own pixels.
//!
//! Departure Mono is a pixel font: each glyph's outline is a union of
//! squares on a grid that, at the lettering's native size, is the pixel
//! grid. So a glyph is rasterized once, from the font file, by testing each
//! pixel's centre against its outline (the non-zero rule a rasterizer
//! fills by), and lettering is then drawn as runs of pixels like every
//! other mark.
//!
//! That is the same pixels as the renderer's own text, and a test holds it to
//! that. What it changes is the damage: canvas text is recorded with an
//! unbounded height, so text that changes from one frame to the next damages
//! everything from it to the output's far corner. Pixels damage only
//! themselves, and need no shaping on every frame.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use iced_core::{Color, Point, Rectangle};
use iced_widget::graphics::geometry;
use quadrille::draw::Pen;

use super::raster::{LETTERING, rect};

/// A glyph's pixels as runs along its rows, from its cell's left edge and
/// the baseline (rows above it are negative).
type Glyph = Arc<[Rectangle<i32>]>;

/// The font the lettering face is set in.
fn font() -> &'static ttf_parser::Face<'static> {
    static FONT: OnceLock<ttf_parser::Face<'static>> = OnceLock::new();

    FONT.get_or_init(|| {
        ttf_parser::Face::parse(quadrille::fonts::DEPARTURE_MONO_TIGHT, 0)
            .expect("Departure Mono Tight is a font")
    })
}

/// The pixels of `character`, rasterized the first time it is asked for.
fn glyph(character: char) -> Glyph {
    static GLYPHS: OnceLock<Mutex<HashMap<char, Glyph>>> = OnceLock::new();

    let mut glyphs = GLYPHS
        .get_or_init(Default::default)
        .lock()
        .expect("Glyph cache");

    glyphs
        .entry(character)
        .or_insert_with(|| rasterize(character).into())
        .clone()
}

/// An outline as closed polygons, in font units.
#[derive(Default)]
struct Outline {
    contours: Vec<Vec<(f32, f32)>>,
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(vec![(x, y)]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if let Some(contour) = self.contours.last_mut() {
            contour.push((x, y));
        }
    }

    // A pixel font has no curves; were there any, their chords would do.
    fn quad_to(&mut self, _: f32, _: f32, x: f32, y: f32) {
        self.line_to(x, y);
    }

    fn curve_to(&mut self, _: f32, _: f32, _: f32, _: f32, x: f32, y: f32) {
        self.line_to(x, y);
    }

    fn close(&mut self) {}
}

impl Outline {
    /// The non-zero winding number of the outline round `(x, y)`.
    fn winding(&self, x: f32, y: f32) -> i32 {
        let mut winding = 0;

        for contour in &self.contours {
            for (i, &(x0, y0)) in contour.iter().enumerate() {
                let (x1, y1) = contour[(i + 1) % contour.len()];
                let side = (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0);

                if y0 <= y && y < y1 && side > 0.0 {
                    winding += 1;
                } else if y1 <= y && y < y0 && side < 0.0 {
                    winding -= 1;
                }
            }
        }

        winding
    }
}

/// The pixels of `character` at the lettering face's size: every pixel
/// whose centre the outline covers. A character the font lacks is its
/// missing glyph, as the renderer draws it.
fn rasterize(character: char) -> Vec<Rectangle<i32>> {
    let font = font();
    let id = font
        .glyph_index(character)
        .unwrap_or(ttf_parser::GlyphId(0));
    let mut outline = Outline::default();

    let Some(bounds) = font.outline_glyph(id, &mut outline) else {
        return Vec::new();
    };

    // Font units to a pixel.
    let unit = f32::from(font.units_per_em()) / f32::from(LETTERING.size());
    let (left, right) = (
        (f32::from(bounds.x_min) / unit).floor() as i32,
        (f32::from(bounds.x_max) / unit).ceil() as i32,
    );
    let (low, high) = (
        (f32::from(bounds.y_min) / unit).floor() as i32,
        (f32::from(bounds.y_max) / unit).ceil() as i32,
    );
    let mut runs = Vec::new();

    for row in low..high {
        let y = (row as f32 + 0.5) * unit;
        let mut start = None;

        for column in left..=right {
            let lit = column < right && outline.winding((column as f32 + 0.5) * unit, y) != 0;

            match (lit, start) {
                (true, None) => start = Some(column),
                (false, Some(first)) => {
                    // Font rows go up; the sheet's go down from the baseline.
                    runs.push(rect(first, -row - 1, column - first, 1));
                    start = None;
                }
                _ => {}
            }
        }
    }

    runs
}

/// Sets `text` in the lettering face with its line box's top-left corner at
/// `top_left`, as `Pen::text` would place it.
pub fn write<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    text: &str,
    top_left: Point<i32>,
    color: Color,
) {
    let baseline = top_left.y + i32::from(LETTERING.baseline());
    let advance = i32::from(LETTERING.advance());

    for (index, character) in text.chars().enumerate() {
        let left = top_left.x + index as i32 * advance;

        for run in glyph(character).iter() {
            pen.fill(
                rect(left + run.x, baseline + run.y, run.width, run.height),
                color,
            );
        }
    }
}

/// Sets `text` placed by `anchor` at `at`, as `Pen::text` places it.
pub fn set<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    text: &str,
    at: Point<i32>,
    anchor: quadrille::draw::Anchor,
    color: Color,
) {
    write(
        pen,
        text,
        super::raster::place(LETTERING, text, at, anchor),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capital_stands_on_the_baseline_within_its_cell() {
        let runs = rasterize('H');
        let rows: Vec<i32> = runs.iter().map(|run| run.y).collect();

        assert_eq!(
            *rows.iter().max().unwrap(),
            -1,
            "the lowest row is just above the baseline"
        );
        assert_eq!(*rows.iter().min().unwrap(), -i32::from(LETTERING.cap()));
        assert!(
            runs.iter()
                .all(|run| run.x >= 0 && run.x + run.width <= i32::from(LETTERING.advance()))
        );
    }

    #[test]
    fn a_space_is_nothing() {
        assert!(rasterize(' ').is_empty());
    }
}
