//! One output's drawing sheet, as a canvas program.
//!
//! [`Sheet`] draws a subject at a moment of its [`timeline`]: the sheet's
//! furniture, the main view at a preferred scale (true on a calibrated
//! display), the plotter's progress, a detail view of the part in focus and
//! its specification, the readings, and the wipe between subjects.
//!
//! Once nothing is being drawn in, what does not move is kept in a
//! [`Memo`] and only the moving marks are drawn again each frame, so the
//! renderer's damage is the size of what moves.
pub mod layout;
pub mod plates;
pub mod timeline;

use iced_core::{Point, Rectangle, Size, mouse};
use iced_widget::canvas::{self, Frame, Geometry};
use iced_widget::graphics::geometry;
use quadrille::canvas::Memo;
use quadrille::draw::{Anchor, Horizontal, Pen, Vertical};
use quadrille::{Palette, Theme};

use crate::draft::raster::{self, Head, Inked, LETTERING, Projection, colour, rect};
use crate::draft::scale::Ratio;
use crate::draft::{Draft, Extent, Mark, Tone};
use crate::subjects::{Card, Detail, Subject};

use layout::{CAPTION, LINE, Layout};
use plates::Typist;
use timeline::{Focus, MARK, Phase, Showing};

/// The physical size of a virtual pixel on an output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub mm_per_vpx: f64,
    /// Whether the size is a guess (no EDID and no measured size).
    pub estimated: bool,
}

/// How fast lettering types, in characters a second.
const TYPING_RATE: f32 = 900.0;

/// A sheet on one output, at one moment.
pub struct Sheet<'a> {
    pub subject: &'a dyn Subject,
    /// The subject's sheet number, from one, and how many there are.
    pub number: usize,
    pub of: usize,
    pub showing: Showing,
    pub display: Display,
    /// Today, as the title block writes it.
    pub date: &'a str,
}

/// What the kept drawing was made for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    /// The subject, and which showing of it: the same subject comes round
    /// again on a later sheet.
    subject: usize,
    serial: u64,
    focus: Option<usize>,
    size: (i32, i32),
}

/// Which marks a pass of painting draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer {
    Fixed,
    Moving,
    All,
}

impl Layer {
    fn takes(self, moving: bool) -> bool {
        match self {
            Self::Fixed => !moving,
            Self::Moving => moving,
            Self::All => true,
        }
    }
}

impl<Message, Renderer> canvas::Program<Message, Theme, Renderer> for Sheet<'_>
where
    Renderer: geometry::Renderer,
{
    type State = Memo<Key, Renderer>;

    fn draw(
        &self,
        memo: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let size = Size::new(bounds.width.floor(), bounds.height.floor());
        let scene = Scene::new(self, size.width as i32, size.height as i32);
        let palette = theme.palette();
        let moment = self.showing.moment;

        if moment.settled() {
            let key = Key {
                subject: self.showing.subject,
                serial: self.showing.serial,
                focus: moment.focus().map(|focus| focus.part),
                size: (size.width as i32, size.height as i32),
            };
            let fixed = memo.draw(renderer, bounds.size(), key, |frame| {
                scene.paint(frame, palette, Layer::Fixed);
            });
            let mut frame = Frame::new(renderer, bounds.size());
            scene.paint(&mut frame, palette, Layer::Moving);

            vec![fixed, frame.into_geometry()]
        } else {
            let mut frame = Frame::new(renderer, bounds.size());
            scene.paint(&mut frame, palette, Layer::All);

            vec![frame.into_geometry()]
        }
    }

    fn mouse_interaction(
        &self,
        _memo: &Self::State,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        mouse::Interaction::Hidden
    }
}

/// The sheet worked out for one size: its layout, the subject's marks at
/// the moment, and where the views put them.
struct Scene<'a> {
    sheet: &'a Sheet<'a>,
    card: &'a Card,
    layout: Layout,
    draft: Draft,
    main: Projection,
    main_ratio: Option<Ratio>,
    detail: Option<DetailView>,
}

struct DetailView {
    focus: Focus,
    detail: Detail,
    projection: Projection,
    ratio: Option<Ratio>,
}

impl DetailView {
    /// Whether `layer` paints what the detail shows: every frame while it
    /// is drawn in or when it follows a moving part, and once otherwise.
    fn paints(&self, layer: Layer) -> bool {
        layer == Layer::All || (layer == Layer::Moving) == self.detail.follows
    }

    /// Whether `layer` paints `mark` in the detail view.
    fn takes(&self, layer: Layer, mark: &Mark) -> bool {
        if self.detail.follows {
            self.paints(layer)
        } else {
            layer.takes(mark.moving)
        }
    }
}

/// The pixels per model unit that frame `extent` in `area`: at the largest
/// preferred scale for a drawing to scale, filling it for a diagram.
fn fit(
    card: &Card,
    extent: Extent,
    area: Rectangle<i32>,
    display: Display,
) -> (f32, Option<Ratio>) {
    let unit = card.unit.millimetres();
    let filling = (f64::from(area.width) / f64::from(extent.width()))
        .min(f64::from(area.height) / f64::from(extent.height()));

    if card.scaled {
        let limit = filling * display.mm_per_vpx / unit;

        if let Some(ratio) = Ratio::preferred_at_most(limit) {
            return ((ratio.0 * unit / display.mm_per_vpx) as f32, Some(ratio));
        }
    }

    (filling as f32, None)
}

/// The detail letter of part `index`: B for the first (A names sections).
fn letter(index: usize) -> char {
    char::from(b'B' + (index % 24) as u8)
}

impl<'a> Scene<'a> {
    fn new(sheet: &'a Sheet<'a>, width: i32, height: i32) -> Self {
        let card = sheet.subject.card();
        let layout = Layout::new(width, height, card);
        let extent = sheet.subject.extent();
        let (scale, main_ratio) = fit(card, extent, layout.view, sheet.display);
        let main = Projection::centred(extent, layout.view, scale);

        let mut draft = Draft::new();
        sheet.subject.draw(&mut draft, sheet.showing.moment.run);

        let detail = sheet.showing.moment.focus().and_then(|focus| {
            let detail = sheet.subject.detail(focus.part, sheet.showing.moment.run)?;
            let window = layout::inset(layout.detail_window(), 2);
            // The circle and a margin round it, magnified at least to the
            // next preferred scale past the view's.
            let region = Extent::around(detail.centre, detail.radius * 1.2);
            let (mut scale, mut ratio) = fit(card, region, window, sheet.display);

            if let (Some(fitted), Some(main)) = (ratio, main_ratio) {
                let least = Ratio::preferred_at_most(main.0 * 2.0).unwrap_or(fitted);

                if fitted < least {
                    scale *= (least.0 / fitted.0) as f32;
                    ratio = Some(least);
                }
            } else if ratio.is_none() {
                scale = scale.max(main.scale * 2.0);
            }

            Some(DetailView {
                focus,
                detail,
                projection: Projection::centred(region, window, scale),
                ratio,
            })
        });

        Self {
            sheet,
            card,
            layout,
            draft,
            main,
            main_ratio,
            detail,
        }
    }

    fn scale_label(&self, ratio: Option<Ratio>) -> String {
        match ratio {
            Some(ratio) if self.sheet.display.estimated => format!("~{}", ratio.label()),
            Some(ratio) => ratio.label(),
            None => "NONE".into(),
        }
    }

    fn paint<Renderer: geometry::Renderer>(
        &self,
        frame: &mut Frame<Renderer>,
        palette: &Palette,
        layer: Layer,
    ) {
        if layer.takes(false) {
            plates::border(&mut Pen::new(frame), &self.layout, palette);
        }

        self.content(frame, palette, layer);

        // The wipe paints the sheet's ground over what it has passed.
        if let Phase::Wipe(share) = self.sheet.showing.moment.phase {
            let border = self.layout.border;
            let x = border.x + 1 + ((border.width - 2) as f32 * share) as i32;
            let mut pen = Pen::new(frame);

            pen.fill(
                rect(
                    border.x + 1,
                    border.y + 1,
                    x - border.x - 1,
                    border.height - 2,
                ),
                palette.void,
            );
            pen.vline(
                x,
                border.y + 1,
                border.y + border.height - 2,
                palette.accent,
            );
        }
    }

    fn content<Renderer: geometry::Renderer>(
        &self,
        frame: &mut Frame<Renderer>,
        palette: &Palette,
        layer: Layer,
    ) {
        let moment = self.sheet.showing.moment;
        let focus = moment.focus();
        let mut pen = Pen::new(frame);

        if layer.takes(false) {
            self.furniture(&mut pen, palette);
        }

        // The main view, plotted in part while the plotter is at work.
        let share = match moment.phase {
            Phase::Plot(share) => Some(share),
            _ => None,
        };
        let marks = self
            .draft
            .marks()
            .iter()
            .filter(|mark| mark.shown_in(true) && layer.takes(mark.moving));
        let mut head = plot(
            &mut pen,
            palette,
            marks,
            &self.main,
            self.layout.drawing(),
            focus.map(|f| f.part),
            share,
        );

        if let Some(view) = &self.detail
            && view.paints(layer)
        {
            head = head.or(self.detail_marker(&mut pen, palette, view));
        }

        if layer.takes(true) {
            self.readings(&mut pen, palette);
        }

        drop(pen);

        if let Some(view) = &self.detail {
            head = head.or(self.detail_view(frame, palette, view, layer));
        }

        if let Some(Head(at)) = head
            && layer.takes(true)
        {
            Pen::new(frame).crosshair(at, 3, 1, palette.accent);
        }
    }

    /// The title block, parts list, notes and the view's caption.
    fn furniture<Renderer: geometry::Renderer>(
        &self,
        pen: &mut Pen<'_, Renderer>,
        palette: &Palette,
    ) {
        let (layout, card, sheet) = (&self.layout, self.card, self.sheet);
        let mut typist = Typist::rate(sheet.showing.moment.local, TYPING_RATE);

        let scale = self.scale_label(self.main_ratio);
        let number = format!("{} OF {}", sheet.number, sheet.of);

        plates::fields(
            pen,
            &mut typist,
            layout.title,
            &plates::title_block(card, &scale, &number, sheet.date),
            palette,
        );

        plates::parts(
            pen,
            &mut typist,
            layout.parts,
            card,
            self.sheet.showing.moment.focus().map(|focus| focus.part),
            palette,
        );

        plates::notes(pen, &mut typist, layout.notes, &card.notes, palette);

        let caption = match self.main_ratio {
            Some(_) => format!("{}   SCALE {}", card.view, scale),
            None => card.view.clone(),
        };
        let middle = layout.caption.x + layout.caption.width / 2;

        typist.text(
            pen,
            &caption,
            Point::new(middle, layout.caption.y),
            Anchor::TOP,
            palette.muted,
        );
        let underline = i32::from(LETTERING.width(&caption));
        pen.hline(
            middle - underline / 2,
            middle - underline / 2 + underline - 1,
            layout.caption.y + LINE,
            palette.edge,
        );
    }

    /// The title and the instruments, along the top of the main area.
    fn readings<Renderer: geometry::Renderer>(
        &self,
        pen: &mut Pen<'_, Renderer>,
        palette: &Palette,
    ) {
        let bounds = self.layout.readings;
        let mut typist = Typist::rate(self.sheet.showing.moment.local, TYPING_RATE);
        let advance = i32::from(LETTERING.advance());
        let mut x = bounds.x;

        typist.text(
            pen,
            &self.card.title,
            Point::new(x, bounds.y),
            Anchor::TOP_LEFT,
            palette.accent,
        );
        x += (self.card.title.chars().count() as i32 + 4) * advance;

        for reading in self.sheet.subject.readings(self.sheet.showing.moment.run) {
            let width =
                (reading.name.chars().count() + reading.value.chars().count() + 1) as i32 * advance;

            if x + width > bounds.x + bounds.width {
                break;
            }

            typist.text(
                pen,
                reading.name,
                Point::new(x, bounds.y),
                Anchor::TOP_LEFT,
                palette.muted,
            );
            x += (reading.name.chars().count() as i32 + 1) * advance;
            typist.text(
                pen,
                &reading.value,
                Point::new(x, bounds.y),
                Anchor::TOP_LEFT,
                palette.ink,
            );
            x += (reading.value.chars().count() as i32 + 3) * advance;
        }
    }

    /// The circle on the main view round what the detail magnifies, and its
    /// letter; drawn in as the part is picked out.
    fn detail_marker<Renderer: geometry::Renderer>(
        &self,
        pen: &mut Pen<'_, Renderer>,
        palette: &Palette,
        view: &DetailView,
    ) -> Option<Head> {
        let centre = self.main.px(view.detail.centre);
        let radius = self.main.length(view.detail.radius).max(4);
        let share = (view.focus.time / MARK).min(1.0);
        let circle = raster::Piece::Path {
            pixels: raster::ordered_circle(centre, radius),
            stipple: raster::Stipple::of(crate::draft::Line::Phantom),
        };
        let budget = (share * circle.cost() as f32) as usize;
        let head = circle.draw(
            pen,
            palette.accent,
            palette.void,
            budget,
            self.layout.drawing(),
        );

        if share >= 1.0 {
            let corner = Point::new(centre.x + radius * 7 / 10, centre.y - radius * 7 / 10);
            let label = Point::new(corner.x + 6, corner.y - 6);

            pen.line(corner, label, palette.accent);
            pen.text(
                LETTERING,
                letter(view.focus.part).to_string(),
                Point::new(label.x + 2, label.y),
                Anchor::new(Horizontal::Left, Vertical::Middle),
                palette.accent,
            );
        }

        head
    }

    /// The detail view in its window, and the part's specification.
    fn detail_view<Renderer: geometry::Renderer>(
        &self,
        frame: &mut Frame<Renderer>,
        palette: &Palette,
        view: &DetailView,
        layer: Layer,
    ) -> Option<Head> {
        let window = self.layout.detail_window();
        let plotted = view.focus.plotted();
        let since = view.focus.time - MARK;
        let part = &self.card.parts[view.focus.part];

        if layer.takes(false) {
            let mut pen = Pen::new(frame);
            let mut typist = Typist::rate(since, TYPING_RATE / 3.0);
            let title = format!("DETAIL {}", letter(view.focus.part));
            let scale = match view.ratio {
                Some(_) => format!("SCALE {}", self.scale_label(view.ratio)),
                None => "ENLARGED".into(),
            };

            if since >= 0.0 {
                plates::caption(
                    &mut pen,
                    &mut typist,
                    rect(window.x, self.layout.detail.y, window.width, CAPTION),
                    &title,
                    &scale,
                    palette,
                );
                pen.outline(window, palette.edge);
            }

            // The part's specification under its number and name.
            let spec = self.layout.spec;
            let heading = format!("{}  {}", view.focus.part + 1, part.name);

            if since >= 0.0 {
                plates::caption(
                    &mut pen,
                    &mut typist,
                    rect(spec.x, spec.y, spec.width, CAPTION),
                    &heading,
                    &part.material,
                    palette,
                );

                let rows: Vec<_> = part.spec.to_vec();

                plates::readings(
                    &mut pen,
                    &mut typist,
                    rect(spec.x, spec.y + CAPTION, spec.width, spec.height - CAPTION),
                    &rows,
                    palette,
                );
            }
        }

        if since < 0.0 {
            return None;
        }

        let inside = layout::inset(window, 1);
        let mut pen = Pen::new(frame);
        let marks = self
            .draft
            .marks()
            .iter()
            .filter(|mark| mark.shown_in(false) && view.takes(layer, mark));
        let share = (plotted < 1.0).then_some(plotted);

        let head = plot(
            &mut pen,
            palette,
            marks,
            &view.projection,
            inside,
            Some(view.focus.part),
            share,
        );

        if view.paints(layer) {
            // The boundary of what the main view's circle marks.
            let centre = view.projection.px(view.detail.centre);
            let radius = view.projection.length(view.detail.radius);
            let circle = raster::Piece::Path {
                pixels: raster::ordered_circle(centre, radius),
                stipple: raster::Stipple::Dash { on: 11, off: 4 },
            };

            circle.draw(&mut pen, palette.faint, palette.void, usize::MAX, inside);
        }

        head
    }
}

/// Draws `marks` through `projection` inside `clip`, the marks of part
/// `focus` in the accent. With a `share`, only that share of the drawing is
/// plotted, in the plotter's order; the pen's position is returned.
fn plot<'m, Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    palette: &Palette,
    marks: impl Iterator<Item = &'m Mark>,
    projection: &Projection,
    clip: Rectangle<i32>,
    focus: Option<usize>,
    share: Option<f32>,
) -> Option<Head> {
    let mut pieces: Vec<(crate::draft::Pass, Inked)> = Vec::new();
    let mut buffer = Vec::new();

    for mark in marks {
        raster::rasterize(mark, projection, &mut buffer);

        let focused = focus.is_some() && mark.part == focus;

        pieces.extend(buffer.drain(..).map(|mut inked| {
            if focused && !matches!(inked.tone, Tone::Live | Tone::Caution) {
                inked.tone = Tone::Accent;
            }

            (mark.pass(), inked)
        }));
    }

    pieces.sort_by_key(|(pass, _)| *pass);

    let total: usize = pieces.iter().map(|(_, inked)| inked.piece.cost()).sum();
    let mut budget = share.map_or(usize::MAX, |share| (share * total as f32) as usize);

    for (_, inked) in &pieces {
        let cost = inked.piece.cost();
        let head = inked.piece.draw(
            pen,
            colour(palette, inked.tone),
            palette.void,
            budget.min(cost),
            clip,
        );

        if budget <= cost {
            return head;
        }

        budget -= cost;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless::Output;
    use crate::subjects;
    use timeline::Programme;

    /// Every value in the title block fits its cell, on both of the desk's
    /// displays, for every subject: a cut value reads as a different value.
    #[test]
    fn the_title_block_never_cuts_a_value() {
        let subjects = subjects::all();

        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            let (width, height) = output.virtual_size();

            for (index, subject) in subjects.iter().enumerate() {
                let programme = Programme {
                    first: index,
                    parts: subjects.iter().map(|s| s.card().parts.len()).collect(),
                };
                let sheet = Sheet {
                    subject: subject.as_ref(),
                    number: index + 1,
                    of: subjects.len(),
                    showing: programme.at(30.0),
                    display: output.display,
                    date: "2026-10-07",
                };
                let scene = Scene::new(&sheet, width as i32, height as i32);
                let scale = scene.scale_label(scene.main_ratio);
                let number = format!("{} OF {}", sheet.number, sheet.of);

                for row in plates::title_block(subject.card(), &scale, &number, sheet.date) {
                    for (field, room) in
                        row.iter().zip(plates::room(scene.layout.title.width, &row))
                    {
                        assert!(
                            field.value.chars().count() <= room,
                            "{}: {} {:?} has room for {room}",
                            subject.name(),
                            field.name,
                            field.value
                        );
                    }
                }
            }
        }
    }
}
