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
use std::rc::Rc;

use iced_core::{Point, Rectangle, Size, mouse};
use iced_widget::canvas::{self, Frame, Geometry};
use iced_widget::graphics::geometry;
use quadrille::canvas::Memo;
use quadrille::draw::{Anchor, Horizontal, Pen, Vertical};
use quadrille::{Palette, Theme};

use crate::draft::letters;
use crate::draft::place::Plan;
use crate::draft::raster::{self, Head, Inked, LETTERING, Projection, colour, rect};
use crate::draft::scale::Ratio;
use crate::draft::v;
use crate::draft::{Draft, Extent, Ink, Line, Mark, Tone};
use crate::subjects::{Card, Detail, Place, Subject};

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
const WIPE_STRIPS: i32 = 32;

/// How fast lettering types, in characters a second.
const TYPING_RATE: f32 = 900.0;

/// The moments across a subject's run its automatic annotations are placed
/// against: where its moving parts go.
const PLACING_SAMPLES: usize = 12;

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
    /// Where the automatic annotations of the subject showing go.
    plan: Planned,
    /// The finished buckets of a plot, and the passed strips of a wipe.
    ///
    /// The renderer counts a kept drawing it has seen before as unchanged,
    /// and one newly kept as changed only where it draws, so these keep a
    /// frame's repaint to the piece the pen is in or the strip the wipe is
    /// crossing.
    pieces: RefCell<Vec<Memo<Piece, Renderer>>>,
}

/// The plan of a subject's automatic annotations at one size, worked out
/// once and kept while the subject shows.
#[derive(Default)]
struct Planned(RefCell<Option<(PlanFor, Rc<Vec<Plan>>)>>);

/// A subject, by its index, at a size.
type PlanFor = (usize, (i32, i32));

impl Planned {
    /// The plans for subject `index` at `size`, a plan a view, worked out by
    /// `plan` if they are not kept.
    fn get(
        &self,
        index: usize,
        size: (i32, i32),
        plan: impl FnOnce() -> Vec<Plan>,
    ) -> Rc<Vec<Plan>> {
        let key = (index, size);
        let mut kept = self.0.borrow_mut();

        match kept.as_ref() {
            Some((made, plan)) if *made == key => plan.clone(),
            _ => {
                let plan = Rc::new(plan());
                *kept = Some((key, plan.clone()));
                plan
            }
        }
    }
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
            plan: Planned::default(),
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
        let scene = Scene::new(self, size.width as i32, size.height as i32, &kept.plan);
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
                        let _ = draw_pieces(
                            &mut Pen::new(frame),
                            palette,
                            bucket,
                            clip,
                            &mut whole,
                            Paths::Apart,
                        );
                    }));
                } else {
                    let mut part = Frame::new(renderer, bounds.size());
                    let mut left = budget - spent;

                    head = draw_pieces(
                        &mut Pen::new(&mut part),
                        palette,
                        bucket,
                        clip,
                        &mut left,
                        Paths::Apart,
                    );
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
    /// The front view's projection...
    main: Projection,
    main_ratio: Option<Ratio>,
    /// ...and every view the sheet shows, the front view first.
    panes: Vec<Pane>,
    detail: Option<DetailView>,
}

/// One of the subject's views on the sheet.
#[derive(Debug, Clone, PartialEq)]
struct Pane {
    /// Which: `None` for the front view.
    view: Option<usize>,
    projection: Projection,
    /// The part of the main area it is drawn and annotated in.
    cell: Rectangle<i32>,
    /// Its name and the top middle of its caption, but for the front view's,
    /// which is the sheet's.
    caption: Option<(String, Point<i32>)>,
}

/// The room between views lined up on a sheet.
const BETWEEN: i32 = 2 * LINE;

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
    fit_span(
        card,
        (extent.width(), extent.height()),
        (0, 0),
        area,
        display,
    )
}

/// The pixels per model unit that frame `span` model units across and down,
/// with `gaps` pixels besides, in `area`, as [`fit`] does.
fn fit_span(
    card: &Card,
    span: (f32, f32),
    gaps: (i32, i32),
    area: Rectangle<i32>,
    display: Display,
) -> (f32, Option<Ratio>) {
    let unit = card.unit.millimetres();
    let filling = (f64::from(area.width - gaps.0) / f64::from(span.0))
        .min(f64::from(area.height - gaps.1) / f64::from(span.1));

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

/// The subject's views laid out in the main area, lined up as first-angle
/// projection puts them, all to one scale, and that scale.
///
/// A view beside the front view or under it goes in if there is room for
/// it: on a wide display at whatever scale fits them all, on a narrow one
/// only at the scale the front view has alone.
fn arrange(sheet: &Sheet<'_>, layout: &Layout) -> (Vec<Pane>, Option<Ratio>) {
    let card = sheet.subject.card();
    let area = layout.view;
    let front = sheet.subject.extent();
    let views = sheet.subject.views();
    let alone = fit(card, front, area, sheet.display);
    let only = |scale: f32| {
        vec![Pane {
            view: None,
            projection: Projection::centred(front, area, scale),
            cell: layout.drawing(),
            caption: None,
        }]
    };

    let beside = views.iter().position(|view| view.place == Place::Beside);
    let under = views.iter().position(|view| view.place == Place::Under);

    if beside.is_none() && under.is_none() {
        return (only(alone.0), alone.1);
    }

    // Across: the front view's x and anything under it, then what is beside
    // it; down: the front view's y and anything beside it, then what is
    // under it, and a caption under it all.
    let span = |beside: Option<usize>, under: Option<usize>| {
        let (mut left, mut right) = (front.min.x, front.max.x);
        let (mut low, mut high) = (front.min.y, front.max.y);

        if let Some(under) = under {
            left = left.min(views[under].extent.min.x);
            right = right.max(views[under].extent.max.x);
        }

        if let Some(beside) = beside {
            low = low.min(views[beside].extent.min.y);
            high = high.max(views[beside].extent.max.y);
        }

        (left, right, low, high)
    };

    let trials = [(beside, under), (beside, None), (None, under)];

    for (beside, under) in trials {
        if beside.is_none() && under.is_none() {
            continue;
        }

        let (left, right, low, high) = span(beside, under);
        let across = right - left + beside.map_or(0.0, |i| views[i].extent.width());
        let down = high - low + under.map_or(0.0, |i| views[i].extent.height());
        let gaps = (
            if beside.is_some() { BETWEEN } else { 0 },
            if under.is_some() { BETWEEN } else { 0 } + CAPTION,
        );
        let (scale, ratio) = fit_span(card, (across, down), gaps, area, sheet.display);

        // On a narrow display, never smaller than the front view alone.
        if !layout.wide && scale < alone.0 * 0.999 {
            continue;
        }

        let width = scale * across + gaps.0 as f32;
        let height = scale * down + gaps.1 as f32;
        let x = area.x as f32 + (area.width as f32 - width) / 2.0;
        let y = area.y as f32 + (area.height as f32 - height) / 2.0;
        let front_origin = ((x - scale * left).round(), (y + scale * high).round());
        let drawing = layout.drawing();
        let split_x = (x + scale * (right - left) + BETWEEN as f32 / 2.0).round() as i32;
        let split_y = (y + scale * (high - low) + BETWEEN as f32 / 2.0).round() as i32;
        let bottom = drawing.y + drawing.height;
        let end = drawing.x + drawing.width;

        let mut panes = vec![Pane {
            view: None,
            projection: Projection {
                origin: front_origin,
                scale,
            },
            cell: rect(
                drawing.x,
                drawing.y,
                if beside.is_some() { split_x } else { end } - drawing.x,
                if under.is_some() { split_y } else { bottom } - drawing.y,
            ),
            caption: None,
        }];

        let caption = |view: usize, projection: &Projection| {
            let extent = views[view].extent;
            let middle = projection.px(v(extent.centre().x, extent.min.y));

            Some((views[view].name.clone(), Point::new(middle.x, middle.y + 6)))
        };

        if let Some(beside) = beside {
            let projection = Projection {
                origin: (
                    (x + scale * (right - left) + BETWEEN as f32
                        - scale * views[beside].extent.min.x)
                        .round(),
                    front_origin.1,
                ),
                scale,
            };

            panes.push(Pane {
                view: Some(beside),
                caption: caption(beside, &projection),
                projection,
                cell: rect(
                    split_x,
                    drawing.y,
                    end - split_x,
                    if under.is_some() { split_y } else { bottom } - drawing.y,
                ),
            });
        }

        if let Some(under) = under {
            let projection = Projection {
                origin: (
                    front_origin.0,
                    (y + scale * (high - low) + BETWEEN as f32 + scale * views[under].extent.max.y)
                        .round(),
                ),
                scale,
            };

            panes.push(Pane {
                view: Some(under),
                caption: caption(under, &projection),
                projection,
                cell: rect(
                    drawing.x,
                    split_y,
                    if beside.is_some() { split_x } else { end } - drawing.x,
                    bottom - split_y,
                ),
            });
        }

        return (panes, ratio);
    }

    (only(alone.0), alone.1)
}

impl<'a> Scene<'a> {
    fn new(sheet: &'a Sheet<'a>, width: i32, height: i32, planned: &Planned) -> Self {
        let card = sheet.subject.card();
        let layout = Layout::new(width, height, card);
        let (panes, main_ratio) = arrange(sheet, &layout);
        let main = panes[0].projection;

        let mut draft = Draft::new();
        sheet.subject.draw(&mut draft, sheet.showing.moment.run);

        // Where the automatic annotations go in each view, from where the
        // subject's parts go over its run: the same on every frame.
        let plans = planned.get(sheet.showing.subject, (width, height), || {
            let running = timeline::running(card.parts.len());
            // Spread by the golden ratio, which no cycle of the subject's
            // keeps time with: evenly spaced samples can all catch a part
            // at the same point of its stroke.
            let samples: Vec<Draft> = (0..PLACING_SAMPLES)
                .map(|k| {
                    let mut sample = Draft::new();
                    let t = running * (k as f32 * 0.618_034).fract();

                    sheet.subject.draw(&mut sample, t);

                    // The circles the sheet marks details with, and their
                    // letters, which annotations keep clear of too.
                    for index in 0..card.parts.len() {
                        if let Some(detail) = sheet.subject.detail(index, t) {
                            let corner = main.length(detail.radius).max(4) * 7 / 10;
                            let marker = |d: &mut Draft| {
                                d.circle(detail.centre, detail.radius, Line::Phantom);
                                d.label(detail.centre, letter(index).to_string())
                                    .anchor(Anchor::LEFT)
                                    .nudge(corner + 8, -corner - 6);
                            };

                            if detail.follows {
                                sample.moving(marker);
                            } else {
                                marker(&mut sample);
                            }
                        }
                    }

                    sample
                })
                .collect();

            panes
                .iter()
                .map(|pane| Plan::new(&samples, &pane.projection, pane.cell, pane.view))
                .collect()
        });

        for (plan, pane) in plans.iter().zip(&panes) {
            plan.apply(draft.marks_mut(), &pane.projection);
        }

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
            panes,
            detail,
        }
    }

    /// The marks every view shows, each with its view's projection; a
    /// cutting plane only when the sheet shows its section.
    fn shown(&self) -> impl Iterator<Item = (&Mark, &Projection)> {
        self.panes.iter().flat_map(|pane| {
            self.draft
                .marks()
                .iter()
                .filter(|mark| mark.shown_in(pane.view))
                .filter(|mark| match mark.ink {
                    Ink::Section { view, .. } => {
                        self.panes.iter().any(|pane| pane.view == Some(view))
                    }
                    _ => true,
                })
                .map(|mark| (mark, &pane.projection))
        })
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
        let marks = self.shown().filter(|(mark, _)| layer.takes(mark.moving));
        let mut pen = Pen::new(frame);

        // The circle is still once it is drawn round a still detail. It goes
        // under the view's marks, so their lettering reads across it, and
        // its letter over them.
        let marker = self
            .detail
            .as_ref()
            .filter(|view| layer == Layer::All || (layer == Layer::Fixed) == view.marked());
        let circle = marker.and_then(|view| self.detail_marker(&mut pen, palette, view));
        let head = plot(
            &mut pen,
            palette,
            marks,
            self.layout.drawing(),
            moment.focus().map(|f| f.part),
            share,
        );

        if let Some(view) = marker {
            self.detail_letter(&mut pen, palette, view);
        }

        head.or(circle)
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

    /// The views' pieces in the plotter's order, all of them.
    fn view_pieces(&self) -> Vec<Inked> {
        let moment = self.sheet.showing.moment;

        pieces(self.shown(), moment.focus().map(|focus| focus.part))
    }

    /// The title block, parts list, notes and the view's caption.
    fn plates<Renderer: geometry::Renderer>(&self, pen: &mut Pen<'_, Renderer>, palette: &Palette) {
        let (layout, card, sheet) = (&self.layout, self.card, self.sheet);
        let mut typist = Typist::rate(sheet.showing.moment.local, TYPING_RATE);

        let scale = self.scale_label(self.main_ratio);
        let number = format!("{} OF {}", sheet.number, sheet.of);

        let title_block = plates::title_block(card, &scale, &number, sheet.date);

        plates::fields(pen, &mut typist, layout.title, &title_block, palette);

        // A drawing to scale is a projection, and says which; a diagram is
        // not.
        if card.scaled && typist.caught_up() {
            plates::first_angle(
                pen,
                plates::projection_cell(layout.title, &title_block),
                palette.muted,
            );
        }

        if let Some(bounds) = layout.revisions {
            plates::revisions(pen, &mut typist, bounds, card, palette);
        }

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

        // The other views' names, under them.
        for (name, at) in self.panes.iter().filter_map(|pane| pane.caption.as_ref()) {
            typist.text(pen, name, *at, Anchor::TOP, palette.muted);

            let underline = i32::from(LETTERING.width(name));
            pen.hline(
                at.x - underline / 2,
                at.x - underline / 2 + underline - 1,
                at.y + LINE,
                palette.edge,
            );
        }
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

    /// The circle on the main view round what the detail magnifies, drawn
    /// in as the part is picked out.
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

        circle.draw(
            pen,
            palette.accent,
            palette.void,
            budget,
            self.layout.drawing(),
        )
    }

    /// The letter of the circle on the main view, once it is drawn in.
    fn detail_letter<Renderer: geometry::Renderer>(
        &self,
        pen: &mut Pen<'_, Renderer>,
        palette: &Palette,
        view: &DetailView,
    ) {
        let centre = self.main.px(view.detail.centre);
        let radius = self.main.length(view.detail.radius).max(4);

        if view.focus.time >= MARK {
            let text = letter(view.focus.part).to_string();
            // Up and right, unless what the view draws or letters is in the
            // way there and not at another corner; a circle that follows a
            // moving part keeps to one.
            let (corner, leader, at, anchor) = if view.detail.follows {
                marker_letter(centre, radius, CORNERS[0])
            } else {
                let pieces = pieces(self.shown(), None);
                let drawing = self.layout.drawing();

                CORNERS
                    .iter()
                    .map(|&way| marker_letter(centre, radius, way))
                    .min_by_key(|&(corner, leader, at, anchor)| {
                        let area = letter_area(corner, leader, &text, at, anchor);

                        match raster::intersection(area, drawing) {
                            Some(inside) if inside == area => crowding(&pieces, area),
                            _ => usize::MAX,
                        }
                    })
                    .expect("Four corners")
            };

            // Over line work it cannot keep clear of, on the sheet's ground.
            let top_left = raster::place(LETTERING, &text, at, anchor);
            let cap_top = top_left.y + i32::from(LETTERING.cap_top());

            pen.line(corner, leader, palette.accent);
            pen.fill(
                rect(
                    top_left.x - 1,
                    cap_top - 1,
                    i32::from(LETTERING.width(&text)) + 1,
                    i32::from(LETTERING.cap()) + 2,
                ),
                palette.void,
            );
            letters::set(pen, &text, at, anchor, palette.accent);
        }
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
            .filter(|mark| mark.magnified() && view.takes(layer, mark))
            .map(|mark| (mark, &view.projection));
        let share = (plotted < 1.0).then_some(plotted);

        if view.paints(layer) {
            // The boundary of what the main view's circle marks, under the
            // marks so their lettering reads across it.
            let centre = view.projection.px(view.detail.centre);
            let radius = view.projection.length(view.detail.radius);
            let circle = raster::Piece::path(
                raster::ordered_circle(centre, radius),
                raster::Stipple::Dash { on: 11, off: 4 },
            );

            circle.draw(&mut pen, palette.faint, palette.void, usize::MAX, inside);
        }

        plot(
            &mut pen,
            palette,
            marks,
            inside,
            Some(view.focus.part),
            share,
        )
    }
}

/// The corners of a detail's circle its letter may stand at, as the ways
/// out from its centre, the first preferred.
const CORNERS: [(i32, i32); 4] = [(1, -1), (-1, -1), (1, 1), (-1, 1)];

/// A detail circle's letter at the corner `(x, y)` out from the circle at
/// `centre` of `radius`: where its leader leaves the circle and ends, and
/// where the letter is set from and how.
fn marker_letter(
    centre: Point<i32>,
    radius: i32,
    (x, y): (i32, i32),
) -> (Point<i32>, Point<i32>, Point<i32>, Anchor) {
    let reach = radius * 7 / 10;
    let corner = Point::new(centre.x + x * reach, centre.y + y * reach);
    let leader = Point::new(corner.x + x * 6, corner.y + y * 6);
    let side = if x > 0 {
        Horizontal::Left
    } else {
        Horizontal::Right
    };

    (
        corner,
        leader,
        Point::new(leader.x + x * 2, leader.y),
        Anchor::new(side, Vertical::Middle),
    )
}

/// What a detail circle's letter and its leader take up.
fn letter_area(
    corner: Point<i32>,
    leader: Point<i32>,
    text: &str,
    at: Point<i32>,
    anchor: Anchor,
) -> Rectangle<i32> {
    let top_left = raster::place(LETTERING, text, at, anchor);
    let cap_top = top_left.y + i32::from(LETTERING.cap_top());
    let (left, top) = (
        corner.x.min(leader.x).min(top_left.x - 2),
        corner.y.min(leader.y).min(cap_top - 2),
    );
    let (right, bottom) = (
        corner
            .x
            .max(leader.x)
            .max(top_left.x + i32::from(LETTERING.width(text)) + 1),
        corner
            .y
            .max(leader.y)
            .max(cap_top + i32::from(LETTERING.cap()) + 2),
    );

    rect(left, top, right - left + 1, bottom - top + 1)
}

/// How much of `pieces` is in `area`: a pixel for each pixel of line work
/// or an area, and many for lettering, which nothing is to be set over.
fn crowding(pieces: &[Inked], area: Rectangle<i32>) -> usize {
    let covered = |bounds: Rectangle<i32>| {
        raster::intersection(bounds, area).map_or(0, |part| (part.width * part.height) as usize)
    };

    pieces
        .iter()
        .map(|inked| match &inked.piece {
            raster::Piece::Path { pixels, .. } => pixels
                .iter()
                .filter(|pixel| raster::contains(area, **pixel))
                .count(),
            raster::Piece::Rows { rows, .. } => rows
                .iter()
                .map(|&(y, from, to)| covered(rect(from, y, to - from + 1, 1)))
                .sum(),
            raster::Piece::Block(bounds) => covered(*bounds),
            raster::Piece::Knockout(bounds) => {
                raster::intersection(*bounds, area).map_or(0, |_| LETTERED)
            }
            raster::Piece::Text { .. } => 0,
        })
        .sum()
}

/// What lettering in the way of a detail's letter counts for: more than the
/// line work its box could hold.
const LETTERED: usize = 1000;

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

/// The pieces of `marks`, each through its projection, in the plotter's
/// order, the marks of part `focus` in the accent.
fn pieces<'m>(
    marks: impl Iterator<Item = (&'m Mark, &'m Projection)>,
    focus: Option<usize>,
) -> Vec<Inked> {
    let mut pieces: Vec<(crate::draft::Pass, Inked)> = Vec::new();
    let mut buffer = Vec::new();

    for (mark, projection) in marks {
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

/// How plotted pieces are handed to the renderer, which repaints by path.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Paths {
    /// A path for each piece: a frame that adds to one repaints only it, not
    /// every piece plotted before it in the same colour. For the plot.
    Apart,
    /// The pen's runs of one colour as one path: a drawing that moves as a
    /// whole repaints as a few large regions, not many small ones.
    Batched,
}

/// Draws `pieces` inside `clip` until `budget` (in pixels of pen travel)
/// runs out; returns where the pen stopped if it stopped inside a piece.
fn draw_pieces<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    palette: &Palette,
    pieces: &[Inked],
    clip: Rectangle<i32>,
    budget: &mut usize,
    paths: Paths,
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

        if paths == Paths::Apart {
            pen.flush();
        }

        if *budget <= cost {
            *budget = 0;
            return head;
        }

        *budget -= cost;
    }

    None
}

/// Draws `marks`, each through its projection, inside `clip`, the marks of
/// part `focus` in the accent. With a `share`, only that share of the drawing is
/// plotted, in the plotter's order; the pen's position is returned.
fn plot<'m, Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    palette: &Palette,
    marks: impl Iterator<Item = (&'m Mark, &'m Projection)>,
    clip: Rectangle<i32>,
    focus: Option<usize>,
    share: Option<f32>,
) -> Option<Head> {
    let pieces = pieces(marks, focus);
    let total: usize = pieces.iter().map(|inked| inked.piece.cost()).sum();
    let mut budget = share.map_or(usize::MAX, |share| (share * total as f32) as usize);

    draw_pieces(pen, palette, &pieces, clip, &mut budget, Paths::Batched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::{Pass, Placement};
    use crate::headless::Output;
    use crate::machine::Machine;
    use crate::subjects;
    use schedule::Schedule;

    /// A sheet of subject `index` on `output`, `local` seconds in.
    fn sheet<'a>(
        subjects: &'a [Box<dyn Subject>],
        index: usize,
        output: Output,
        local: f32,
    ) -> Sheet<'a> {
        let mut schedule = Schedule::new(
            subjects.iter().map(|s| s.card().parts.len()).collect(),
            0,
            Some(index),
        );

        Sheet {
            subject: subjects[index].as_ref(),
            number: index + 1,
            of: subjects.len(),
            showing: schedule.at(0, local),
            display: output.display,
            date: "2026-10-07",
        }
    }

    /// The boxes an annotation's lettering and circle clear, and the
    /// pixels of its leaders.
    fn footprint(mark: &Mark, projection: &Projection) -> (Vec<Rectangle<i32>>, Vec<Point<i32>>) {
        let mut pieces = Vec::new();
        raster::rasterize(mark, projection, &mut pieces);

        let boxes: Vec<Rectangle<i32>> = pieces
            .iter()
            .filter_map(|inked| match &inked.piece {
                raster::Piece::Knockout(area) => Some(*area),
                _ => None,
            })
            .collect();
        let lines = pieces
            .iter()
            .flat_map(|inked| match &inked.piece {
                raster::Piece::Path { pixels, .. } => pixels.clone(),
                _ => Vec::new(),
            })
            .filter(|pixel| !boxes.iter().any(|area| raster::contains(*area, *pixel)))
            .collect();

        (boxes, lines)
    }

    /// Every annotation the sheet places itself is inside the view, and
    /// neither it nor its leader covers another annotation's lettering, on
    /// both displays, for every subject, while it runs.
    #[test]
    fn placed_annotations_are_in_the_view_and_clear_of_other_lettering() {
        let subjects = subjects::all(&Machine::fixture());

        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            let (width, height) = output.virtual_size();

            for index in 0..subjects.len() {
                let planned = Planned::default();

                for local in [12.0, 20.0, 30.0, 40.0] {
                    let sheet = sheet(&subjects, index, output, local);
                    let scene = Scene::new(&sheet, width as i32, height as i32, &planned);
                    let mut recorded = Draft::new();
                    sheet.subject.draw(&mut recorded, sheet.showing.moment.run);

                    let annotations: Vec<(bool, &Mark)> = recorded
                        .marks()
                        .iter()
                        .zip(scene.draft.marks())
                        .filter(|(mark, _)| mark.shown_in(None) && mark.pass() >= Pass::Annotation)
                        .map(|(mark, placed)| {
                            let auto = matches!(
                                mark.ink,
                                Ink::Balloon {
                                    offset: Placement::Auto,
                                    ..
                                } | Ink::Note {
                                    elbow: Placement::Auto,
                                    ..
                                }
                            );

                            (auto, placed)
                        })
                        .collect();
                    let footprints: Vec<_> = annotations
                        .iter()
                        .map(|(_, mark)| footprint(mark, &scene.main))
                        .collect();
                    let clip = scene.layout.drawing();
                    let display = if output == Output::LAPTOP {
                        "laptop"
                    } else {
                        "ultrawide"
                    };
                    let at = format!("{} on the {display} at {local} s", subjects[index].name());

                    for (i, (auto, _)) in annotations.iter().enumerate() {
                        let (boxes, lines) = &footprints[i];

                        if !auto || boxes.is_empty() {
                            continue;
                        }

                        assert!(
                            boxes
                                .iter()
                                .all(|area| raster::intersection(*area, clip) == Some(*area))
                                && lines.iter().all(|pixel| raster::contains(clip, *pixel)),
                            "{at}: annotation {i} leaves the view"
                        );

                        for (j, (others, _)) in footprints.iter().enumerate() {
                            if i == j {
                                continue;
                            }

                            for other in others {
                                assert!(
                                    boxes
                                        .iter()
                                        .all(|area| raster::intersection(*area, *other).is_none()),
                                    "{at}: annotation {i} covers annotation {j}"
                                );
                                assert!(
                                    !lines.iter().any(|pixel| raster::contains(*other, *pixel)),
                                    "{at}: annotation {i}'s leader crosses annotation {j}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Every view the sheet shows is framed inside a cell of the main area
    /// of its own, on both displays, for every subject; and lined up with
    /// the front view.
    #[test]
    fn views_are_framed_apart_and_lined_up() {
        let subjects = subjects::all(&Machine::fixture());

        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            let (width, height) = output.virtual_size();

            for index in 0..subjects.len() {
                let sheet = sheet(&subjects, index, output, 30.0);
                let layout = Layout::new(width as i32, height as i32, sheet.subject.card());
                let (panes, _) = arrange(&sheet, &layout);
                let views = sheet.subject.views();
                let drawing = layout.drawing();
                let name = subjects[index].name();

                assert_eq!(panes[0].view, None);

                for (i, pane) in panes.iter().enumerate() {
                    let extent = match pane.view {
                        None => sheet.subject.extent(),
                        Some(view) => views[view].extent,
                    };
                    let corner = pane.projection.px(v(extent.min.x, extent.max.y));
                    let far = pane.projection.px(v(extent.max.x, extent.min.y));
                    let framed = rect(corner.x, corner.y, far.x - corner.x, far.y - corner.y);

                    assert!(
                        raster::intersection(pane.cell, drawing) == Some(pane.cell),
                        "{name}: view {i} {:?} leaves the drawing",
                        pane.cell
                    );
                    assert!(
                        raster::intersection(framed, pane.cell) == Some(framed),
                        "{name} on {width}: view {i} {framed:?} leaves its cell {:?}",
                        pane.cell
                    );

                    for other in &panes[i + 1..] {
                        assert!(raster::intersection(pane.cell, other.cell).is_none());
                    }

                    // Beside the front view on its rows, under it on its
                    // columns.
                    match pane.view.map(|view| views[view].place) {
                        Some(Place::Beside) => {
                            assert_eq!(pane.projection.origin.1, panes[0].projection.origin.1);
                        }
                        Some(Place::Under) => {
                            assert_eq!(pane.projection.origin.0, panes[0].projection.origin.0);
                        }
                        None => {}
                    }
                }
            }
        }
    }

    /// Every value in the title block fits its cell, on both of the desk's
    /// displays, for every subject: a cut value reads as a different value.
    #[test]
    fn the_title_block_never_cuts_a_value() {
        let subjects = subjects::all(&Machine::fixture());

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
                let scene = Scene::new(&sheet, width as i32, height as i32, &Planned::default());
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
    /// A detail's letter stands at the first corner of its circle that is
    /// clear of lettering, and not at one that is lettered over.
    #[test]
    fn a_details_letter_keeps_clear_of_lettering() {
        let centre = Point::new(200, 200);
        let choose = |pieces: &[Inked]| {
            CORNERS
                .iter()
                .copied()
                .min_by_key(|&way| {
                    let (corner, leader, at, anchor) = marker_letter(centre, 40, way);
                    crowding(pieces, letter_area(corner, leader, "B", at, anchor))
                })
                .unwrap()
        };
        let lettered = |way: (i32, i32)| {
            let (corner, leader, at, anchor) = marker_letter(centre, 40, way);
            Inked {
                piece: raster::Piece::Knockout(letter_area(corner, leader, "B", at, anchor)),
                tone: Tone::Ink,
            }
        };
        // A line across where the letter goes up and right.
        let (corner, ..) = marker_letter(centre, 40, CORNERS[0]);
        let line = Inked {
            piece: raster::Piece::path(
                (150..260).map(|x| Point::new(x, corner.y - 3)).collect(),
                raster::Stipple::of(Line::Outline),
            ),
            tone: Tone::Line,
        };

        assert_eq!(choose(&[]), CORNERS[0]);
        assert_eq!(choose(&[lettered(CORNERS[0])]), CORNERS[1]);
        assert_eq!(
            choose(&[lettered(CORNERS[0]), lettered(CORNERS[1])]),
            CORNERS[2]
        );
        // A corner with line work in the way gives way to a clear one, and
        // is taken before one with lettering in the way.
        assert_eq!(choose(&[line.clone(), lettered(CORNERS[1])]), CORNERS[2]);
        assert_eq!(
            choose(&[
                line,
                lettered(CORNERS[1]),
                lettered(CORNERS[2]),
                lettered(CORNERS[3])
            ]),
            CORNERS[0]
        );
    }

    /// On the displays sheet no dimension's value is lettered over a
    /// display's edge, on either display.
    #[test]
    fn the_displays_dimensions_clear_their_edges() {
        let subjects = subjects::all(&Machine::fixture());
        let index = subjects::find(&subjects, "displays").expect("The displays sheet");

        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            let (width, height) = output.virtual_size();
            let sheet = sheet(&subjects, index, output, 12.0);
            let scene = Scene::new(&sheet, width as i32, height as i32, &Planned::default());
            let mut values = Vec::new();
            let mut edges = Vec::new();

            for (mark, projection) in scene.shown() {
                let mut pieces = Vec::new();
                raster::rasterize(mark, projection, &mut pieces);

                for inked in pieces {
                    match (&mark.ink, inked.piece) {
                        (Ink::Dimension { .. }, raster::Piece::Knockout(area)) => {
                            values.push(area);
                        }
                        (
                            Ink::Stroke {
                                line: Line::Outline,
                                ..
                            },
                            raster::Piece::Path { pixels, .. },
                        ) => edges.extend(pixels),
                        _ => {}
                    }
                }
            }

            assert!(values.len() >= 4 && !edges.is_empty());

            for area in values {
                assert!(
                    !edges.iter().any(|pixel| raster::contains(area, *pixel)),
                    "{area:?} on {output:?}"
                );
            }
        }
    }
}
