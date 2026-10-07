//! Where everything goes on a sheet, from the output's size in virtual
//! pixels.
//!
//! The sheet is a drawing frame filling the output: a border with zones,
//! the main view in the middle, and a column on the right with the title
//! block at the foot, the parts list and notes over it, and the detail view
//! and its part's specification at the top. On a wide output the detail
//! leaves the column for the main area, where it can be larger.
use iced_core::Rectangle;

use super::plates::{columns, wrap};
use crate::draft::raster::{LETTERING, rect};
use crate::subjects::Card;

/// Pixels between the output's edge and the sheet's trim line.
pub const MARGIN: i32 = 4;
/// Space between regions.
pub const GUTTER: i32 = 8;
/// One line of lettering.
pub const LINE: i32 = 12;
/// A title-block field: a name over a value, with a pixel each side.
pub const FIELD: i32 = 2 * LINE + 2;
/// The caption strip over a view or plate.
pub const CAPTION: i32 = LINE + 4;

#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// The trim line: the sheet's edge.
    pub trim: Rectangle<i32>,
    /// The border line, inside the zone band.
    pub border: Rectangle<i32>,
    pub columns: u8,
    pub rows: u8,
    /// Whether the output is wide: the detail beside the view, not in the
    /// column.
    pub wide: bool,
    /// The readings, one line over the view.
    pub readings: Rectangle<i32>,
    /// What the main view is fitted into.
    pub view: Rectangle<i32>,
    /// The view's name and scale, under it.
    pub caption: Rectangle<i32>,
    /// The detail view's window, its caption over it.
    pub detail: Rectangle<i32>,
    /// The specification of the part in detail.
    pub spec: Rectangle<i32>,
    pub notes: Rectangle<i32>,
    pub parts: Rectangle<i32>,
    pub title: Rectangle<i32>,
}

/// Rows of the title block.
pub const TITLE_ROWS: i32 = 3;

impl Layout {
    pub fn new(width: i32, height: i32, card: &Card) -> Self {
        let trim = rect(MARGIN, MARGIN, width - 2 * MARGIN, height - 2 * MARGIN);
        let band = i32::from(LETTERING.cap()) + 4;
        let border = inset(trim, band);
        let inner = inset(border, GUTTER);

        let wide = inner.width > 2 * inner.height;
        let column = if wide { 300 } else { 216 }.min(inner.width / 3);

        let main = rect(
            inner.x,
            inner.y,
            inner.width - column - GUTTER,
            inner.height,
        );
        let right = rect(main.x + main.width + GUTTER, inner.y, column, inner.height);

        // The column from the foot up: title block, parts list, notes.
        let title_height = TITLE_ROWS * FIELD + 1;
        let title = rect(
            right.x,
            right.y + right.height - title_height,
            right.width,
            title_height,
        );

        let parts_height = (card.parts.len() as i32 + 1) * (LINE + 1) + 1;
        let parts = rect(
            right.x,
            title.y - parts_height + 1,
            right.width,
            parts_height,
        );

        let note_columns = columns(right.width - 6) - 3;
        let note_lines: usize = card
            .notes
            .iter()
            .map(|note| wrap(note, note_columns).len())
            .sum();
        let notes_height = CAPTION + note_lines as i32 * LINE;
        let notes = rect(
            right.x,
            parts.y - GUTTER - notes_height,
            right.width,
            notes_height,
        );

        let spec_rows = card
            .parts
            .iter()
            .map(|part| part.spec.len())
            .max()
            .unwrap_or(0) as i32;
        let spec_height = CAPTION + spec_rows * LINE + 2;

        let readings = rect(main.x, main.y, main.width, LINE);
        let caption = rect(main.x, main.y + main.height - LINE, main.width, LINE);
        let between = rect(
            main.x,
            readings.y + readings.height + GUTTER,
            main.width,
            caption.y - GUTTER - (readings.y + readings.height + GUTTER),
        );

        let (view, detail, spec) = if wide {
            // The detail beside the view, as tall as the view; the
            // specification heads the column.
            let detail_width = (between.width * 2 / 5).min(between.height * 3 / 2);
            let view = rect(
                between.x,
                between.y,
                between.width - detail_width - GUTTER,
                between.height,
            );
            let detail = rect(
                view.x + view.width + GUTTER,
                between.y,
                detail_width,
                between.height,
            );
            let spec = rect(right.x, right.y, right.width, spec_height);

            (view, detail, spec)
        } else {
            let top = right.y;
            let room = notes.y - GUTTER - top;
            let detail_height = (room - spec_height - GUTTER).max(4 * LINE);
            let detail = rect(right.x, top, right.width, detail_height);
            let spec = rect(
                right.x,
                detail.y + detail.height + GUTTER,
                right.width,
                spec_height,
            );

            (between, detail, spec)
        };

        Self {
            trim,
            border,
            wide,
            columns: (border.width / 110).clamp(2, 16) as u8,
            rows: (border.height / 110).clamp(2, 10) as u8,
            readings,
            view,
            caption,
            detail,
            spec,
            notes,
            parts,
            title,
        }
    }

    /// Where the main view may draw: its area and the gutters round it, so
    /// nothing a subject draws reaches the column or the border.
    pub fn drawing(&self) -> Rectangle<i32> {
        let top = self.readings.y + self.readings.height + 1;
        let bottom = self.caption.y - 1;
        let right = if self.detail.x > self.view.x + self.view.width {
            self.detail.x - GUTTER / 2
        } else {
            self.view.x + self.view.width + GUTTER / 2
        };

        rect(
            self.view.x - GUTTER / 2,
            top,
            right - (self.view.x - GUTTER / 2),
            bottom - top,
        )
    }

    /// The detail view's drawing window, under its caption.
    pub fn detail_window(&self) -> Rectangle<i32> {
        rect(
            self.detail.x,
            self.detail.y + CAPTION,
            self.detail.width,
            self.detail.height - CAPTION,
        )
    }
}

/// `bounds` shrunk by `by` on every side.
pub fn inset(bounds: Rectangle<i32>, by: i32) -> Rectangle<i32> {
    rect(
        bounds.x + by,
        bounds.y + by,
        bounds.width - 2 * by,
        bounds.height - 2 * by,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subjects;

    fn inside(inner: Rectangle<i32>, outer: Rectangle<i32>) -> bool {
        inner.x >= outer.x
            && inner.y >= outer.y
            && inner.x + inner.width <= outer.x + outer.width
            && inner.y + inner.height <= outer.y + outer.height
    }

    fn apart(a: Rectangle<i32>, b: Rectangle<i32>) -> bool {
        a.x + a.width <= b.x
            || b.x + b.width <= a.x
            || a.y + a.height <= b.y
            || b.y + b.height <= a.y
    }

    #[test]
    fn regions_stay_on_the_sheet_and_off_each_other_on_both_displays() {
        // The laptop (2560 × 1600 at 3 pixels per virtual pixel) and the
        // ultrawide (3440 × 1440 at 2).
        for (width, height) in [(853, 533), (1720, 720)] {
            for subject in subjects::all() {
                let layout = Layout::new(width, height, subject.card());
                let regions = [
                    layout.readings,
                    layout.view,
                    layout.caption,
                    layout.detail,
                    layout.spec,
                    layout.notes,
                    layout.parts,
                    layout.title,
                ];

                for (i, region) in regions.iter().enumerate() {
                    assert!(
                        inside(*region, layout.border),
                        "{width}: region {i} {region:?}"
                    );
                    assert!(
                        region.width > 0 && region.height > 0,
                        "{width}: region {i} is empty"
                    );

                    for (j, other) in regions.iter().enumerate().skip(i + 1) {
                        // The parts list shares its foot rule with the title block.
                        if (i, j) == (6, 7) {
                            continue;
                        }
                        assert!(apart(*region, *other), "{width}: {i} overlaps {j}");
                    }
                }

                assert!(
                    layout.view.height >= 300,
                    "{width}: the view is {:?}",
                    layout.view
                );
            }
        }
    }
}
