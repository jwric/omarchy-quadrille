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
pub mod schedule;
pub mod timeline;

use std::cell::RefCell;

use iced_core::{Point, Rectangle, Size, mouse};
use iced_widget::canvas::{self, Frame, Geometry};
use iced_widget::graphics::geometry;
use quadrille::canvas::Memo;
use quadrille::draw::{Anchor, Horizontal, Pen, Vertical};
use quadrille::{Palette, Theme};

use crate::draft::letters;
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

/// The longest piece, in pixels of pen travel, the plot is cut into...
const PLOT_PIECE: usize = 96;
/// ...and the pen travel of each separately kept bucket of them.
const PLOT_BUCKET: usize = 1536;
/// The strips the wipe is made of.
const WIPE_STRIPS: i32 = 8;

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

/// What a kept drawing was made for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Key {
    /// The subject, and which showing of it: the same subject comes round
    /// again on a later sheet.
    subject: usize,
    serial: u64,
    focus: Option<usize>,
    /// Whether the detail's circle is drawn on the view yet.
    marked: bool,
    size: (i32, i32),
}

/// What a sheet keeps from frame to frame: each of its parts once it is
/// drawn in, so a frame draws again only what is changing, and the
/// renderer's damage is no larger than that.
pub struct Kept<Renderer: geometry::Renderer> {
    /// The sheet's border: the same on every sheet of the output.
    border: Memo<(i32, i32), Renderer>,
    /// The title block, parts list, notes and caption.
    furniture: Memo<Key, Renderer>,
    /// The main view's still marks.
    view: Memo<Key, Renderer>,
    /// The detail view's still marks, its caption and the part's
    /// specification.
    detail: Memo<Key, Renderer>,
    /// The finished buckets of a plot, and the passed strips of a wipe.
    ///
    /// The renderer counts a kept drawing it has seen before as unchanged,
    /// so these keep a frame's repaint to the bucket the pen is in or the
    /// strip the wipe is crossing. It counts a drawing newly kept as the
    /// whole output changed, so they are few and large: a bucket is about
    /// half a second of plotting, the wipe eight strips.
    pieces: RefCell<Vec<Memo<Piece, Renderer>>>,
}

/// What a kept piece of a plot or a wipe was made for.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    sheet: Key,
    /// A bucket of the plot, or a strip of the wipe.
    wipe: bool,
    index: usize,
}

impl<Renderer: geometry::Renderer> Default for Kept<Renderer> {
    fn default() -> Self {
        Self {
            border: Memo::new(),
            furniture: Memo::new(),
            view: Memo::new(),
            detail: Memo::new(),
            pieces: RefCell::new(Vec::new()),
        }
    }
}

impl<Renderer: geometry::Renderer> Kept<Renderer> {
    /// The kept drawing of piece `key`, drawn by `draw` if it is not kept.
    fn piece(
        &self,
        renderer: &Renderer,
        size: Size,
        key: Piece,
        draw: impl FnOnce(&mut Frame<Renderer>),
    ) -> Geometry<Renderer> {
        let mut pieces = self.pieces.borrow_mut();

        while pieces.len() <= key.index {
            pieces.push(Memo::new());
        }

        pieces[key.index].draw(renderer, size, key, draw)
    }
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
    type State = Kept<Renderer>;

    fn draw(
        &self,
        kept: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry<Renderer>> {
        let size = Size::new(bounds.width.floor(), bounds.height.floor());
        let scene = Scene::new(self, size.width as i32, size.height as i32);
        let palette = theme.palette();
        let moment = self.showing.moment;
        let key = |marked| Key {
            subject: self.showing.subject,
            serial: self.showing.serial,
            focus: moment.focus().map(|focus| focus.part),
            marked,
            size: (size.width as i32, size.height as i32),
        };

        let mut layers = Vec::with_capacity(4);
        let mut frame = Frame::new(renderer, bounds.size());

        layers.push(
            kept.border
                .draw(renderer, bounds.size(), key(false).size, |frame| {
                    plates::border(&mut Pen::new(frame), &scene.layout, palette);
                }),
        );

        // The furniture, once its lettering has typed itself in.
        if moment.typed() >= 1.0 {
            layers.push(
                kept.furniture
                    .draw(renderer, bounds.size(), key(false), |frame| {
                        scene.furniture(frame, palette);
                    }),
            );
        } else {
            scene.furniture(&mut frame, palette);
        }

        // The main view: plotted in buckets of short pieces, each a drawing
        // of its own, so a frame repaints only the bucket the pen is in;
        // once plotted, kept, and what moves drawn each frame.
        let mut head = if let Phase::Plot(share) = moment.phase {
            let pieces: Vec<Inked> = scene
                .view_pieces()
                .into_iter()
                .flat_map(|inked| {
                    let tone = inked.tone;
                    inked
                        .piece
                        .split(PLOT_PIECE)
                        .into_iter()
                        .map(move |piece| Inked { piece, tone })
                })
                .collect();
            let total: usize = pieces.iter().map(|inked| inked.piece.cost()).sum();
            let budget = (share * total as f32) as usize;
            let clip = scene.layout.drawing();
            let mut spent = 0;
            let mut head = None;

            for (index, bucket) in buckets(&pieces, PLOT_BUCKET).enumerate() {
                let cost: usize = bucket.iter().map(|inked| inked.piece.cost()).sum();

                if spent + cost <= budget {
                    // Plotted whole: kept from now on.
                    let key = Piece {
                        sheet: key(false),
                        wipe: false,
                        index,
                    };

                    layers.push(kept.piece(renderer, bounds.size(), key, |frame| {
                        let mut whole = usize::MAX;
                        let _ =
                            draw_pieces(&mut Pen::new(frame), palette, bucket, clip, &mut whole);
                    }));
                } else {
                    let mut part = Frame::new(renderer, bounds.size());
                    let mut left = budget - spent;

                    head = draw_pieces(&mut Pen::new(&mut part), palette, bucket, clip, &mut left);
                    layers.push(part.into_geometry());
                    break;
                }

                spent += cost;
            }

            head
        } else {
            let marked = scene.detail.as_ref().is_some_and(DetailView::marked);

            layers.push(
                kept.view
                    .draw(renderer, bounds.size(), key(marked), |frame| {
                        scene.view(frame, palette, Layer::Fixed);
                    }),
            );
            scene.view(&mut frame, palette, Layer::Moving)
        };

        // The detail, once it is drawn in.
        if let Some(view) = &scene.detail {
            if view.focus.settled() {
                layers.push(
                    kept.detail
                        .draw(renderer, bounds.size(), key(false), |frame| {
                            scene.detail_view(frame, palette, view, Layer::Fixed);
                        }),
                );
                head = head.or(scene.detail_view(&mut frame, palette, view, Layer::Moving));
            } else {
                head = head.or(scene.detail_view(&mut frame, palette, view, Layer::All));
            }
        }

        scene.readings(&mut Pen::new(&mut frame), palette);

        if let Some(Head(at)) = head {
            Pen::new(&mut frame).crosshair(at, 3, 1, palette.accent);
        }

        layers.push(frame.into_geometry());
        layers.extend(scene.wipe(kept, renderer, bounds.size(), key(false), palette));

        layers
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
    /// Whether its circle on the view is drawn in and stays where it is.
    fn marked(&self) -> bool {
        self.focus.time >= MARK && !self.detail.follows
    }

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

    /// The sheet's furniture: title block, parts list, notes and caption.
    fn furniture<Renderer: geometry::Renderer>(
        &self,
        frame: &mut Frame<Renderer>,
        palette: &Palette,
    ) {
        self.plates(&mut Pen::new(frame), palette);
    }

    /// The main view's marks that `layer` takes, plotted in part while the
    /// plotter is at work, and the circle of the detail in focus.
    fn view<Renderer: geometry::Renderer>(
        &self,
        frame: &mut Frame<Renderer>,
        palette: &Palette,
        layer: Layer,
    ) -> Option<Head> {
        let moment = self.sheet.showing.moment;
        let share = match moment.phase {
            Phase::Plot(share) => Some(share),
            _ => None,
        };
        let marks = self
            .draft
            .marks()
            .iter()
            .filter(|mark| mark.shown_in(true) && layer.takes(mark.moving));
        let mut pen = Pen::new(frame);
        let head = plot(
            &mut pen,
            palette,
            marks,
            &self.main,
            self.layout.drawing(),
            moment.focus().map(|f| f.part),
            share,
        );

        // The circle is still once it is drawn round a still detail.
        match &self.detail {
            Some(view) if layer == Layer::All || (layer == Layer::Fixed) == view.marked() => {
                head.or(self.detail_marker(&mut pen, palette, view))
            }
            _ => head,
        }
    }

    /// The wipe: the sheet's ground painted over what it has passed, in
    /// strips that each stay as they are once passed, and its line.
    fn wipe<Renderer: geometry::Renderer>(
        &self,
        kept: &Kept<Renderer>,
        renderer: &Renderer,
        size: Size,
        key: Key,
        palette: &Palette,
    ) -> Vec<Geometry<Renderer>> {
        let Phase::Wipe(share) = self.sheet.showing.moment.phase else {
            return Vec::new();
        };
        let border = self.layout.border;
        let (left, top, height) = (border.x + 1, border.y + 1, border.height - 2);
        let x = left + ((border.width - 2) as f32 * share) as i32;
        let mut geometries = Vec::new();

        let strip = ((border.width - 2) + WIPE_STRIPS - 1) / WIPE_STRIPS;

        for (index, start) in (left..x).step_by(strip as usize).enumerate() {
            let width = (x - start).min(strip);
            let paint = |frame: &mut Frame<Renderer>| {
                Pen::new(frame).fill(rect(start, top, width, height), palette.void);
            };

            if width == strip {
                let key = Piece {
                    sheet: key,
                    wipe: true,
                    index,
                };

                geometries.push(kept.piece(renderer, size, key, paint));
            } else {
                let mut strip = Frame::new(renderer, size);
                paint(&mut strip);
                geometries.push(strip.into_geometry());
            }
        }

        let mut line = Frame::new(renderer, size);
        Pen::new(&mut line).vline(x, top, top + height - 1, palette.accent);
        geometries.push(line.into_geometry());

        geometries
    }

    /// The main view's pieces in the plotter's order, all of them.
    fn view_pieces(&self) -> Vec<Inked> {
        let moment = self.sheet.showing.moment;
        let marks = self.draft.marks().iter().filter(|mark| mark.shown_in(true));

        pieces(marks, &self.main, moment.focus().map(|focus| focus.part))
    }

    /// The title block, parts list, notes and the view's caption.
    fn plates<Renderer: geometry::Renderer>(&self, pen: &mut Pen<'_, Renderer>, palette: &Palette) {
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
        let circle = raster::Piece::path(
            raster::ordered_circle(centre, radius),
            raster::Stipple::of(crate::draft::Line::Phantom),
        );
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
            letters::set(
                pen,
                &letter(view.focus.part).to_string(),
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
            let circle = raster::Piece::path(
                raster::ordered_circle(centre, radius),
                raster::Stipple::Dash { on: 11, off: 4 },
            );

            circle.draw(&mut pen, palette.faint, palette.void, usize::MAX, inside);
        }

        head
    }
}

/// `pieces` in consecutive buckets of about `cost` pixels of pen travel.
fn buckets(pieces: &[Inked], cost: usize) -> impl Iterator<Item = &[Inked]> {
    let mut rest = pieces;

    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }

        let mut travel = 0;
        let end = rest
            .iter()
            .position(|inked| {
                travel += inked.piece.cost();
                travel >= cost
            })
            .map_or(rest.len(), |last| last + 1);
        let (bucket, after) = rest.split_at(end);

        rest = after;
        Some(bucket)
    })
}

/// The pieces of `marks` through `projection` in the plotter's order, the
/// marks of part `focus` in the accent.
fn pieces<'m>(
    marks: impl Iterator<Item = &'m Mark>,
    projection: &Projection,
    focus: Option<usize>,
) -> Vec<Inked> {
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
    pieces.into_iter().map(|(_, inked)| inked).collect()
}

/// Draws `pieces` inside `clip` until `budget` (in pixels of pen travel)
/// runs out; returns where the pen stopped if it stopped inside a piece.
fn draw_pieces<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    palette: &Palette,
    pieces: &[Inked],
    clip: Rectangle<i32>,
    budget: &mut usize,
) -> Option<Head> {
    for inked in pieces {
        let cost = inked.piece.cost();
        let head = inked.piece.draw(
            pen,
            colour(palette, inked.tone),
            palette.void,
            (*budget).min(cost),
            clip,
        );

        if *budget <= cost {
            *budget = 0;
            return head;
        }

        *budget -= cost;
    }

    None
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
    let pieces = pieces(marks, projection, focus);
    let total: usize = pieces.iter().map(|inked| inked.piece.cost()).sum();
    let mut budget = share.map_or(usize::MAX, |share| (share * total as f32) as usize);

    draw_pieces(pen, palette, &pieces, clip, &mut budget)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless::Output;
    use crate::subjects;
    use schedule::Schedule;

    /// Every value in the title block fits its cell, on both of the desk's
    /// displays, for every subject: a cut value reads as a different value.
    #[test]
    fn the_title_block_never_cuts_a_value() {
        let subjects = subjects::all();

        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            let (width, height) = output.virtual_size();

            for (index, subject) in subjects.iter().enumerate() {
                let mut schedule = Schedule::new(
                    subjects.iter().map(|s| s.card().parts.len()).collect(),
                    0,
                    Some(index),
                );
                let sheet = Sheet {
                    subject: subject.as_ref(),
                    number: index + 1,
                    of: subjects.len(),
                    showing: schedule.at(0, 30.0),
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
