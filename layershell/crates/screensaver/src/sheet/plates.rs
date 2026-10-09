//! The sheet's furniture: its border and zones, the title block, the parts
//! list, the notes, a part's specification and the readings.
//!
//! Lettering goes through a [`Typist`], which can be given a budget of
//! characters: the title block of a new sheet types itself in. Or the
//! furniture is a printed form a drafting office fills in as its drawing
//! proceeds, an [`Entry`] at a time, each with its own typist ([`Typing`]).
use iced_core::{Color, Point, Rectangle};
use iced_widget::graphics::geometry;
use quadrille::Palette;
use quadrille::draw::{Anchor, Pen};

use super::layout::{CAPTION, FIELD, LINE, Layout};
use crate::draft::letters;
use crate::draft::raster::{LETTERING, rect};
use crate::subjects::Card;

/// Sets text, as much of it as its budget allows.
pub struct Typist {
    /// Characters still to type.
    budget: usize,
    /// Characters it has been given to type, typed or not.
    pub asked: usize,
}

impl Typist {
    /// A typist that has been typing for `seconds` at `per_second`.
    pub fn rate(seconds: f32, per_second: f32) -> Self {
        Self {
            budget: (seconds.max(0.0) * per_second) as usize,
            asked: 0,
        }
    }

    /// Whether everything set so far is typed in, with more to come.
    pub fn caught_up(&self) -> bool {
        self.budget > 0
    }

    pub fn text<Renderer: geometry::Renderer>(
        &mut self,
        pen: &mut Pen<'_, Renderer>,
        text: &str,
        at: Point<i32>,
        anchor: Anchor,
        color: Color,
    ) {
        let count = text.chars().count();
        let shown = count.min(self.budget);
        self.budget -= shown;
        self.asked += count;

        if shown == 0 {
            return;
        }

        // Placed as the whole text would be, so typing does not shift it.
        let corner = crate::draft::raster::place(LETTERING, text, at, anchor);
        let typed: String = text.chars().take(shown).collect();

        letters::write(pen, &typed, corner, color);
    }
}

/// What of a sheet's form fills in at once, as a drafting office fills in
/// its printed form while the drawing proceeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Entry {
    /// The form as printed, with what is known of the drawing before it is
    /// begun: its title, number, domain and unit, and its revisions.
    Form,
    Notes,
    /// A part's row of the parts list.
    Row(usize),
    /// The name under a view other than the front view, by its pane.
    View(usize),
    /// What the draughtsman signs off once the drawing is done: its scale,
    /// the sheet, the date and who drew it, and the front view's name.
    SignOff,
}

/// Which of the furniture is drawn, and who types it.
pub enum Typing {
    /// All of it, by one typist in turn: a new sheet typing itself in.
    InTurn(Typist),
    /// Only one entry, by a typist of its own.
    Only(Entry, Typist),
}

impl Typing {
    /// The typist of `entry`, if it is drawn.
    pub fn typist(&mut self, entry: Entry) -> Option<&mut Typist> {
        match self {
            Self::InTurn(typist) => Some(typist),
            Self::Only(only, typist) => (*only == entry).then_some(typist),
        }
    }

    /// Whether `entry` is drawn.
    pub fn draws(&self, entry: Entry) -> bool {
        match self {
            Self::InTurn(_) => true,
            Self::Only(only, _) => *only == entry,
        }
    }
}

/// `text` broken into lines of at most `columns` characters, at spaces.
pub fn wrap(text: &str, columns: usize) -> Vec<String> {
    textwrap::wrap(text, columns)
        .into_iter()
        .map(|line| line.into_owned())
        .collect()
}

/// How many characters fit across `width` pixels.
pub fn columns(width: i32) -> usize {
    (width.max(0) / i32::from(LETTERING.advance())) as usize
}

/// The trim and border lines, the zone divisions in the band between them,
/// their numbers and letters, and a centring mark on each side.
pub fn border<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    layout: &Layout,
    palette: &Palette,
) {
    let (trim, border) = (layout.trim, layout.border);

    pen.outline(trim, palette.edge);
    pen.outline(border, palette.edge);

    let across = |k: i32| border.x + border.width * k / i32::from(layout.columns);
    let down = |k: i32| border.y + border.height * k / i32::from(layout.rows);

    for k in 0..=i32::from(layout.columns) {
        let x = across(k);

        if k > 0 && k < i32::from(layout.columns) {
            pen.vline(x, trim.y, border.y, palette.edge);
            pen.vline(
                x,
                border.y + border.height - 1,
                trim.y + trim.height - 1,
                palette.edge,
            );
        }

        if k < i32::from(layout.columns) {
            let middle = (x + across(k + 1)) / 2;
            let label = (k + 1).to_string();
            let top = (trim.y + border.y) / 2;
            let bottom = (border.y + border.height - 1 + trim.y + trim.height - 1) / 2;

            for y in [top, bottom] {
                letters::set(
                    pen,
                    &label,
                    Point::new(middle, y + 1),
                    Anchor::CENTRE,
                    palette.faint,
                );
            }
        }
    }

    for k in 0..=i32::from(layout.rows) {
        let y = down(k);

        if k > 0 && k < i32::from(layout.rows) {
            pen.hline(trim.x, border.x, y, palette.edge);
            pen.hline(
                border.x + border.width - 1,
                trim.x + trim.width - 1,
                y,
                palette.edge,
            );
        }

        if k < i32::from(layout.rows) {
            let middle = (y + down(k + 1)) / 2;
            let label = char::from(b'A' + k as u8).to_string();
            let left = (trim.x + border.x) / 2;
            let right = (border.x + border.width - 1 + trim.x + trim.width - 1) / 2;

            for x in [left, right] {
                letters::set(
                    pen,
                    &label,
                    Point::new(x + 1, middle + 1),
                    Anchor::CENTRE,
                    palette.faint,
                );
            }
        }
    }

    // Centring marks: a short heavy tick through the band at each side's
    // middle, reaching into the drawing.
    let (cx, cy) = (border.x + border.width / 2, border.y + border.height / 2);
    let reach = 6;

    pen.fill(
        rect(cx, trim.y, 1, border.y - trim.y + reach),
        palette.muted,
    );
    pen.fill(
        rect(
            cx,
            border.y + border.height - reach,
            1,
            trim.y + trim.height - border.y - border.height + reach,
        ),
        palette.muted,
    );
    pen.fill(
        rect(trim.x, cy, border.x - trim.x + reach, 1),
        palette.muted,
    );
    pen.fill(
        rect(
            border.x + border.width - reach,
            cy,
            trim.x + trim.width - border.x - border.width + reach,
            1,
        ),
        palette.muted,
    );
}

/// One cell of a title block: a name over a value.
pub struct Field<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub span: i32,
    pub emphasis: bool,
    /// What its value is filled in with, on a form.
    pub entry: Entry,
}

impl<'a> Field<'a> {
    pub fn new(name: &'a str, value: &'a str, span: i32) -> Self {
        Self {
            name,
            value,
            span,
            emphasis: false,
            entry: Entry::Form,
        }
    }

    pub fn emphasised(self) -> Self {
        Self {
            emphasis: true,
            ..self
        }
    }

    /// The field signed off once the drawing is done.
    pub fn signed(self) -> Self {
        Self {
            entry: Entry::SignOff,
            ..self
        }
    }
}

/// The title block's fields: what the sheet is, its scale, and when.
pub fn title_block<'a>(
    card: &'a Card,
    scale: &'a str,
    sheet: &'a str,
    date: &'a str,
) -> [Vec<Field<'a>>; 3] {
    [
        vec![
            Field::new("TITLE", &card.title, 3).emphasised(),
            Field::new("DRAWING", &card.number, 2),
        ],
        vec![
            Field::new("DOMAIN", card.domain.label(), 3),
            Field::new("SCALE", scale, 2).signed(),
            // The projection symbol's.
            Field::new("", "", 1),
        ],
        // The unit is a symbol, and the sheet number takes up to `10 OF 10`
        // on the laptop's column.
        vec![
            Field::new("UNIT", card.unit.label(), 3),
            Field::new("SHEET", sheet, 5).signed(),
            Field::new("DATE", date, 6).signed(),
            Field::new("DRAWN", "QUADRILLE", 6).signed(),
        ],
    ]
}

/// The middle of the field the projection symbol takes in a title block at
/// `bounds`: the last of its second row.
pub fn projection_cell(bounds: Rectangle<i32>, rows: &[Vec<Field<'_>>]) -> Point<i32> {
    let (left, right) = cells(bounds, &rows[1])
        .last()
        .copied()
        .unwrap_or((bounds.x, bounds.x + bounds.width - 1));

    Point::new((left + right) / 2, bounds.y + FIELD + FIELD / 2)
}

/// The first-angle projection symbol (ISO 5456-2) round `centre`: a cone's
/// frustum seen from the front, and seen from its large end, drawn on its
/// right as first-angle projection places a view from the left.
pub fn first_angle<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    centre: Point<i32>,
    color: iced_core::Color,
) {
    let at = |x: i32, y: i32| Point::new(centre.x + x, centre.y + y);

    for (from, to) in [
        (at(-11, -4), at(-1, -2)),
        (at(-1, -2), at(-1, 2)),
        (at(-1, 2), at(-11, 4)),
        (at(-11, 4), at(-11, -4)),
    ] {
        pen.line(from, to, color);
    }

    pen.pixels(&quadrille::draw::shape::circle(at(7, 0), 4), color);
    pen.pixels(&quadrille::draw::shape::circle(at(7, 0), 2), color);
}

/// The left and right edges of each field of `row` across `bounds`.
fn cells(bounds: Rectangle<i32>, row: &[Field<'_>]) -> Vec<(i32, i32)> {
    let total: i32 = row.iter().map(|field| field.span).sum::<i32>().max(1);
    let mut used = 0;

    row.iter()
        .map(|field| {
            let left = bounds.x + (bounds.width - 1) * used / total;
            used += field.span;
            (left, bounds.x + (bounds.width - 1) * used / total)
        })
        .collect()
}

/// How many characters each field of `row` has room for across `width`.
pub fn room(width: i32, row: &[Field<'_>]) -> Vec<usize> {
    cells(rect(0, 0, width, FIELD), row)
        .into_iter()
        .map(|(left, right)| columns(right - left - 4))
        .collect()
}

/// A boxed table of fields in rows; each row is divided between its fields
/// by their spans, each field its name over its value.
pub fn fields<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    typing: &mut Typing,
    bounds: Rectangle<i32>,
    rows: &[Vec<Field<'_>>],
    palette: &Palette,
) {
    let form = typing.draws(Entry::Form);

    if form {
        pen.outline(bounds, palette.edge);
    }

    for (r, row) in rows.iter().enumerate() {
        let y = bounds.y + r as i32 * FIELD;

        if r > 0 && form {
            pen.hline(bounds.x, bounds.x + bounds.width - 1, y, palette.edge);
        }

        let rooms = room(bounds.width, row);

        for (f, (field, (x, _))) in row.iter().zip(cells(bounds, row)).enumerate() {
            if f > 0 && form {
                pen.vline(x, y, y + FIELD, palette.edge);
            }

            let value: String = field.value.chars().take(rooms[f]).collect();

            if let Some(typist) = typing.typist(Entry::Form) {
                typist.text(
                    pen,
                    field.name,
                    Point::new(x + 3, y + 1),
                    Anchor::TOP_LEFT,
                    palette.faint,
                );
            }

            if let Some(typist) = typing.typist(field.entry) {
                typist.text(
                    pen,
                    &value,
                    Point::new(x + 3, y + 1 + LINE),
                    Anchor::TOP_LEFT,
                    if field.emphasis {
                        palette.accent
                    } else {
                        palette.ink
                    },
                );
            }
        }
    }
}

/// A caption: a rule broken by a name, `── NAME ──`, the toolkit's group
/// heading.
pub fn caption<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    typist: &mut Typist,
    bounds: Rectangle<i32>,
    name: &str,
    right: &str,
    palette: &Palette,
) {
    let y = bounds.y + i32::from(LETTERING.cap_top()) + i32::from(LETTERING.cap()) / 2;
    let name_width = i32::from(LETTERING.width(name));
    let right_width = i32::from(LETTERING.width(right));
    let (left, end) = (bounds.x, bounds.x + bounds.width - 1);

    pen.hline(left, left + 3, y, palette.edge);
    typist.text(
        pen,
        name,
        Point::new(left + 7, bounds.y),
        Anchor::TOP_LEFT,
        palette.muted,
    );

    let after = left + 7 + name_width + 4;
    let before = if right.is_empty() {
        end
    } else {
        end - right_width - 8
    };

    if before > after {
        pen.hline(after, before, y, palette.edge);
    }
    if !right.is_empty() {
        typist.text(
            pen,
            right,
            Point::new(end - 3, bounds.y),
            Anchor::new(
                quadrille::draw::Horizontal::Right,
                quadrille::draw::Vertical::Top,
            ),
            palette.ink,
        );
        pen.hline(end - 1, end, y, palette.edge);
    }
}

/// The parts list: a row for each part under a header; the part `lit`, if
/// any, shown inverse.
pub fn parts<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    typing: &mut Typing,
    bounds: Rectangle<i32>,
    card: &Card,
    lit: Option<usize>,
    palette: &Palette,
) {
    let header = ["ITEM", "NAME", "QTY", card.domain.material_heading()];
    let rows = card.parts.iter().enumerate().map(|(i, part)| {
        vec![
            (i + 1).to_string(),
            part.name.clone(),
            part.quantity.to_string(),
            part.material.clone(),
        ]
    });

    table(
        pen,
        typing,
        bounds,
        &[4, 12, 3, 9],
        &header,
        rows,
        (lit, Entry::Row),
        palette,
    );
}

/// The revision table: what has changed on the drawing, under a caption.
pub fn revisions<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    typing: &mut Typing,
    bounds: Rectangle<i32>,
    card: &Card,
    palette: &Palette,
) {
    let Some(typist) = typing.typist(Entry::Form) else {
        return;
    };

    caption(
        pen,
        typist,
        rect(bounds.x, bounds.y, bounds.width, CAPTION),
        "REVISIONS",
        "",
        palette,
    );

    let rows = card.revisions.iter().map(|revision| {
        vec![
            revision.mark.to_string(),
            revision.description.clone(),
            revision.date.clone(),
        ]
    });

    table(
        pen,
        typing,
        rect(
            bounds.x,
            bounds.y + CAPTION,
            bounds.width,
            bounds.height - CAPTION,
        ),
        &[3, 15, 8],
        &["REV", "DESCRIPTION", "DATE"],
        rows,
        (None, |_| Entry::Form),
        palette,
    );
}

/// A table: its columns `widths` shares of the width, a row for each of
/// `rows` under `header`; row `lit`, if any, shown inverse, and each row
/// filled in as the entry its index gives, on a form.
#[allow(clippy::too_many_arguments)]
fn table<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    typing: &mut Typing,
    bounds: Rectangle<i32>,
    widths: &[i32],
    header: &[&str],
    rows: impl Iterator<Item = Vec<String>>,
    (lit, entry): (Option<usize>, impl Fn(usize) -> Entry),
    palette: &Palette,
) {
    let height = LINE + 1;
    let total: i32 = widths.iter().sum::<i32>().max(1);
    let edges: Vec<i32> = std::iter::once(0)
        .chain(widths.iter().scan(0, |sum, width| {
            *sum += width;
            Some(*sum)
        }))
        .map(|share| bounds.x + (bounds.width - 1) * share / total)
        .collect();

    let form = typing.draws(Entry::Form);

    if form {
        pen.outline(bounds, palette.edge);
    }

    for (r, cells) in std::iter::once(header.iter().map(|name| name.to_string()).collect())
        .chain(rows)
        .enumerate()
    {
        let y = bounds.y + r as i32 * height;
        let is_lit = r > 0 && lit == Some(r - 1);
        let row = if r > 0 { entry(r - 1) } else { Entry::Form };

        if r > 0 && form {
            pen.hline(bounds.x, bounds.x + bounds.width - 1, y, palette.edge);
        }
        if is_lit && typing.draws(row) {
            pen.fill(
                rect(bounds.x + 1, y + 1, bounds.width - 2, height - 1),
                palette.accent,
            );
        }

        for (c, cell) in cells.iter().enumerate() {
            let (left, right) = (edges[c], edges[c + 1]);

            if r == 0 && c > 0 && form {
                pen.vline(left, bounds.y, bounds.y + bounds.height - 1, palette.edge);
            }

            let text: String = cell.chars().take(columns(right - left - 4)).collect();
            let color = match (r, is_lit) {
                (0, _) => palette.faint,
                (_, true) => palette.on_accent,
                _ => palette.ink,
            };

            if let Some(typist) = typing.typist(row) {
                typist.text(
                    pen,
                    &text,
                    Point::new(left + 3, y + 1),
                    Anchor::TOP_LEFT,
                    color,
                );
            }
        }
    }
}

/// Numbered notes under a caption, wrapped to the width.
pub fn notes<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    typing: &mut Typing,
    bounds: Rectangle<i32>,
    notes: &[String],
    palette: &Palette,
) {
    if let Some(typist) = typing.typist(Entry::Form) {
        caption(
            pen,
            typist,
            rect(bounds.x, bounds.y, bounds.width, CAPTION),
            "NOTES",
            "",
            palette,
        );
    }

    let Some(typist) = typing.typist(Entry::Notes) else {
        return;
    };
    let mut y = bounds.y + CAPTION;
    let width = columns(bounds.width - 6);

    for (n, note) in notes.iter().enumerate() {
        let number = format!("{}.", n + 1);
        let indent = 3;

        typist.text(
            pen,
            &number,
            Point::new(bounds.x + 3, y),
            Anchor::TOP_LEFT,
            palette.muted,
        );

        for line in wrap(note, width - indent) {
            typist.text(
                pen,
                &line,
                Point::new(
                    bounds.x + 3 + indent as i32 * i32::from(LETTERING.advance()),
                    y,
                ),
                Anchor::TOP_LEFT,
                palette.ink,
            );
            y += LINE;
        }
    }
}

/// Lines of `name … value`, the value right-aligned.
pub fn readings<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    typist: &mut Typist,
    bounds: Rectangle<i32>,
    rows: &[(String, String)],
    palette: &Palette,
) {
    for (r, (name, value)) in rows.iter().enumerate() {
        let y = bounds.y + r as i32 * LINE;

        typist.text(
            pen,
            name,
            Point::new(bounds.x + 3, y),
            Anchor::TOP_LEFT,
            palette.muted,
        );
        typist.text(
            pen,
            value,
            Point::new(bounds.x + bounds.width - 3, y),
            Anchor::new(
                quadrille::draw::Horizontal::Right,
                quadrille::draw::Vertical::Top,
            ),
            palette.ink,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_wrap_at_spaces() {
        assert_eq!(
            wrap("INVOLUTE TEETH, MODULE 2, PRESSURE ANGLE 20°", 20),
            vec!["INVOLUTE TEETH,", "MODULE 2, PRESSURE", "ANGLE 20°"]
        );
    }
}
