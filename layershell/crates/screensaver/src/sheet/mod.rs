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
pub mod plotter;
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
use crate::draft::place::{self, Plan};
use crate::draft::raster::{self, Head, Inked, LETTERING, Projection, colour, rect};
use crate::draft::scale::Ratio;
use crate::draft::v;
use crate::draft::{Draft, Extent, Ink, Line, Mark, Shape, Tone};
use crate::subjects::{Card, Detail, Place, Room, Subject};

use layout::{CAPTION, LINE, Layout};
use plates::Typist;
use plotter::style::{PlotStyle, Wipe};
use plotter::{Marked, Paper, Plot};
use timeline::{Focus, MARK, Phase, Showing};

/// The physical size of a virtual pixel on an output.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub mm_per_vpx: f64,
    /// Whether the size is a guess (no EDID and no measured size).
    pub estimated: bool,
}

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
    /// How the pen plots it.
    pub plot: PlotStyle,
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
    /// The plot of the sheet showing, worked out on its first frame...
    plot: plotter::Kept<PlotFor>,
    /// ...and of the detail showing, on its first.
    detail_plot: plotter::Kept<DetailFor, plotter::detail::Detail>,
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

/// A showing of a subject, by its index and serial, at a size, plotted in a
/// style: frozen for the showing, so a live subject's marks cannot change
/// under the buckets kept of it.
type PlotFor = (usize, u64, (i32, i32), PlotStyle);

/// The detail of a part, by its index, on a showing.
type DetailFor = (PlotFor, usize);

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
            plot: plotter::Kept::default(),
            detail_plot: plotter::Kept::default(),
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
        let (width, height) = (size.width as i32, size.height as i32);
        let mut scene = Scene::new(self, width, height, &kept.plan);
        let palette = theme.palette();

        // A detail the pen plots: its plot, worked out on its first frame
        // from the drawing as it stood when its part was picked out.
        if self.plot.details
            && let Some(view) = scene.detail.as_mut()
            && !view.focus.settled()
        {
            let showing = self.showing;
            let made = (
                (showing.subject, showing.serial, (width, height), self.plot),
                view.focus.part,
            );
            let plan = kept.detail_plot.get(made, || {
                self.detail_plot(view.focus, (width, height), &kept.plan)
            });
            let now = plan.at(view.focus.time);

            view.pen = Some((plan, now));
        }
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

        // The main view: plotted in buckets of strokes, each a drawing of
        // its own once done, so a frame repaints only the bucket the pen is
        // in; once plotted, kept, and what moves drawn each frame.
        let mut head = if let Phase::Plot(share) = moment.phase {
            let (plotted, head) =
                scene.plotted(kept, renderer, bounds.size(), key(false), share, palette);

            layers.extend(plotted);
            head
        } else {
            let marked = scene.detail.as_ref().is_some_and(DetailView::marked);

            layers.push(
                kept.view
                    .draw(renderer, bounds.size(), key(marked), |frame| {
                        scene.view(frame, palette, Layer::Fixed);
                    }),
            );
            scene
                .view(&mut frame, palette, Layer::Moving)
                .map(|Head(at)| plotter::pen::Head::down(at))
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
                head = head.or(scene
                    .detail_view(&mut frame, palette, view, Layer::Moving)
                    .map(|Head(at)| plotter::pen::Head::down(at)));
            } else {
                head = head.or(scene
                    .detail_view(&mut frame, palette, view, Layer::All)
                    .map(|Head(at)| plotter::pen::Head::down(at)));
            }

            // Carried up, or still, where the plan has the pen.
            if let Some((_, now)) = &view.pen {
                head = head.or(now.head);
            }
        }

        scene.readings(&mut Pen::new(&mut frame), palette);

        let head = head.filter(|_| self.plot.head);

        if let Some(head) = head {
            let mut pen = Pen::new(&mut frame);

            plotter::pen::draw(&mut pen, head, palette);

            // The carriage's ticks, which on a gantry plotter ride at the
            // ends of its arm, along which the head shows where it is.
            if self.plot.ticks && !self.plot.gantry {
                let (trim, border) = (scene.layout.trim, scene.layout.border);

                plotter::pen::ticks_across(&mut pen, head.at.x, trim, border, palette);
                plotter::pen::ticks_down(&mut pen, head.at.y, trim, border, palette);
            }
        }

        layers.push(frame.into_geometry());
        layers.extend(scene.wipe(kept, renderer, bounds.size(), key(false), palette));

        // The gantry's arm, over the border and under all else, a layer of
        // its own every frame so that the others pair with the frame
        // before's as they are. It is shown while the pen plots the sheet,
        // not while it plots a detail over the subject's run: crossing what
        // moves, it would have most of the sheet repainted every frame.
        if self.plot.gantry && self.plot.head {
            let mut arm = Frame::new(renderer, bounds.size());

            if let Some(head) = head.filter(|_| matches!(moment.phase, Phase::Plot(_))) {
                let (trim, border) = (scene.layout.trim, scene.layout.border);
                let mut pen = Pen::new(&mut arm);

                plotter::pen::gantry(&mut pen, head.at.x, border, palette);

                if self.plot.ticks {
                    plotter::pen::ticks_across(&mut pen, head.at.x, trim, border, palette);
                }
            }

            layers.insert(1, arm.into_geometry());
        }

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

/// The plot of `sheet` on an output `size` virtual pixels across and down,
/// worked out as its first frame works it out.
pub fn plot_of(sheet: &Sheet<'_>, size: (i32, i32)) -> Plot {
    Scene::new(sheet, size.0, size.1, &Planned::default()).plot(&sheet.plot, size)
}

impl Sheet<'_> {
    /// The pen's plot of the detail in `focus` on a sheet of `size`, from
    /// the drawing as it stood when its part was picked out: the same
    /// whichever frame of the detail works it out.
    fn detail_plot(
        &self,
        focus: Focus,
        size: (i32, i32),
        planned: &Planned,
    ) -> plotter::detail::Detail {
        let moment = self.showing.moment;
        let picked = Sheet {
            subject: self.subject,
            number: self.number,
            of: self.of,
            showing: Showing {
                moment: timeline::Moment {
                    phase: Phase::Run(Some(Focus {
                        part: focus.part,
                        time: 0.0,
                    })),
                    local: moment.local - focus.time,
                    run: timeline::SETTLE + focus.part as f32 * timeline::DETAIL,
                },
                ..self.showing
            },
            display: self.display,
            date: self.date,
            plot: self.plot,
        };
        let scene = Scene::new(&picked, size.0, size.1, planned);
        let view = scene.detail.as_ref().expect("A part picked out");

        plotter::detail::Detail::new(&self.plot, scene.sketch(view), plotter::scale(size.1))
    }
}

/// The sheet worked out for one size: its layout, the subject's marks at
/// the moment, and where the views put them.
struct Scene<'a> {
    sheet: &'a Sheet<'a>,
    /// The subject laid out for its view's room, if it lays itself out
    /// (see [`Subject::fitted`]): what the scene draws.
    fitted: Option<Rc<dyn Subject>>,
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

/// How far past a whole number of pixels to a unit a diagram may fill its
/// area and be drawn at the whole number all the same.
const WHOLE: f64 = 1.15;

struct DetailView {
    focus: Focus,
    detail: Detail,
    projection: Projection,
    ratio: Option<Ratio>,
    /// The pen's plot of it and where that is, while the pen plots it.
    pen: Option<(Rc<plotter::detail::Detail>, plotter::detail::Now)>,
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
pub(crate) fn fit(
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

    if !card.scaled {
        // A diagram that would fill the area at a little more than a whole
        // number of pixels to a unit is drawn at the whole number: every
        // point of its layout on a pixel, every block as large as another
        // of its size, for a little of the area.
        let whole = filling.floor();

        if whole >= 1.0 && filling < whole * WHOLE {
            return (whole as f32, None);
        }
    }

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
fn arrange(subject: &dyn Subject, display: Display, layout: &Layout) -> (Vec<Pane>, Option<Ratio>) {
    let card = subject.card();
    let area = layout.view;
    let front = subject.extent();
    let views = subject.views();
    let alone = fit(card, front, area, display);
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
        let (scale, ratio) = fit_span(card, (across, down), gaps, area, display);

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
            projection: Projection::new(front_origin, scale),
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
            let projection = Projection::new(
                (
                    (x + scale * (right - left) + BETWEEN as f32
                        - scale * views[beside].extent.min.x)
                        .round(),
                    front_origin.1,
                ),
                scale,
            );

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
            let projection = Projection::new(
                (
                    front_origin.0,
                    (y + scale * (high - low) + BETWEEN as f32 + scale * views[under].extent.max.y)
                        .round(),
                ),
                scale,
            );

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

/// The subject of `sheet` laid out for the main view of `layout`, if it
/// lays itself out (see [`Subject::fitted`]).
fn fitted(sheet: &Sheet<'_>, layout: &Layout) -> Option<Rc<dyn Subject>> {
    sheet.subject.fitted(Room {
        view: v(layout.view.width as f32, layout.view.height as f32),
        mm_per_vpx: sheet.display.mm_per_vpx,
    })
}

impl<'a> Scene<'a> {
    fn new(sheet: &'a Sheet<'a>, width: i32, height: i32, planned: &Planned) -> Self {
        // The subject's card lays the sheet out, and says the same laid
        // out for any room.
        let layout = Layout::new(width, height, sheet.subject.card());
        let fitted = fitted(sheet, &layout);
        let subject = fitted.as_deref().unwrap_or(sheet.subject);
        let card = subject.card();
        let (mut panes, main_ratio) = arrange(subject, sheet.display, &layout);
        let main = panes[0].projection;

        let mut draft = Draft::new();
        subject.draw(&mut draft, sheet.showing.moment.run);

        // Where the automatic annotations go in each view, from where the
        // subject's parts go over its run: the same on every frame.
        let plans = planned.get(sheet.showing.subject, (width, height), || {
            let running = timeline::running(card.parts.len());
            // Spread by the golden ratio, which no cycle of the subject's
            // keeps time with: evenly spaced samples can all catch a part
            // at the same point of its stroke.
            let mut drawn = Vec::new();
            let samples: Vec<Draft> = (0..PLACING_SAMPLES)
                .map(|k| {
                    let mut sample = Draft::new();
                    let t = running * (k as f32 * 0.618_034).fract();

                    subject.draw(&mut sample, t);
                    drawn.push(sample.marks().len());

                    // The circles the sheet marks details with, and their
                    // letters, which annotations keep clear of too.
                    for index in 0..card.parts.len() {
                        if let Some(detail) = subject.detail(index, t) {
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
                .map(|pane| {
                    let mut plan = Plan::new(&samples, &pane.projection, pane.cell, pane.view);
                    let drawings = samples
                        .iter()
                        .zip(&drawn)
                        .map(|(sample, &drawn)| &sample.marks()[..drawn]);

                    plan.centre(drawings, &pane.projection, pane.cell);
                    plan
                })
                .collect()
        });

        // A view alone is drawn with what is placed round it in its
        // middle, not the room the subject keeps round its drawing: the
        // balloons go wherever they read best, which may be on one side.
        // A diagram's views drawn together are moved together the same way,
        // with their captions.
        let (x, y) = match plans.as_slice() {
            [plan] => plan.shift,
            plans if !card.scaled => plans
                .iter()
                .filter_map(Plan::inked)
                .chain(panes.iter().filter_map(|pane| {
                    let (name, top) = pane.caption.as_ref()?;
                    let width = i32::from(LETTERING.width(name));

                    // Its lettering and the line under it.
                    Some(rect(top.x - width / 2, top.y, width, LINE + 1))
                }))
                .reduce(place::union)
                .map_or((0, 0), |inked| place::middled(inked, layout.drawing())),
            _ => (0, 0),
        };

        for pane in &mut panes {
            pane.projection.origin.0 += x as f32;
            pane.projection.origin.1 += y as f32;

            if let Some((_, top)) = &mut pane.caption {
                *top = Point::new(top.x + x, top.y + y);
            }
        }

        let main = panes[0].projection;

        for (plan, pane) in plans.iter().zip(&panes) {
            plan.apply(draft.marks_mut(), &pane.projection);
        }

        let detail = sheet.showing.moment.focus().and_then(|focus| {
            let detail = subject.detail(focus.part, sheet.showing.moment.run)?;
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
                // Twice the view's scale, or what fits what must be read in
                // full round the circle's middle, if that is less.
                let least = match detail.holds {
                    Some(holds) => {
                        let half = (holds.max - detail.centre).max(detail.centre - holds.min);
                        let held = Extent::new(detail.centre - half, detail.centre + half);

                        (main.scale * 2.0).min(fit(card, held, window, sheet.display).0)
                    }
                    None => main.scale * 2.0,
                };

                scale = scale.max(least);
            }

            Some(DetailView {
                focus,
                detail,
                projection: Projection::centred(region, window, scale),
                ratio,
                pen: None,
            })
        });

        Self {
            sheet,
            fitted,
            layout,
            draft,
            main,
            main_ratio,
            panes,
            detail,
        }
    }

    /// What the scene draws: the sheet's subject, or it laid out for its
    /// view's room.
    fn subject(&self) -> &dyn Subject {
        self.fitted.as_deref().unwrap_or(self.sheet.subject)
    }

    fn card(&self) -> &Card {
        self.subject().card()
    }

    /// The marks every view shows, each with its view's projection; a
    /// cutting plane only when the sheet shows its section.
    fn shown(&self) -> impl Iterator<Item = (&Mark, &Projection)> {
        self.shown_in_panes()
            .map(|(_, mark, projection)| (mark, projection))
    }

    /// [`Self::shown`], each with the index of its view's pane.
    fn shown_in_panes(&self) -> impl Iterator<Item = (usize, &Mark, &Projection)> {
        self.panes.iter().enumerate().flat_map(|(index, pane)| {
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
                .map(move |mark| (index, mark, &pane.projection))
        })
    }

    /// The plot of the views' marks on a sheet of `size`, as `style` plots
    /// it.
    fn plot(&self, style: &PlotStyle, size: (i32, i32)) -> Plot {
        let marks: Vec<Marked> = self
            .shown_in_panes()
            .map(|(pane, mark, projection)| {
                let mut pieces = Vec::new();
                raster::rasterize(mark, projection, &mut pieces);

                Marked {
                    meta: plotter::order::Meta {
                        pass: mark.pass(),
                        line: match mark.ink {
                            Ink::Stroke { line, .. } | Ink::Arrow { line, .. } => Some(line),
                            _ => None,
                        },
                        curved: matches!(
                            mark.ink,
                            Ink::Stroke {
                                shape: Shape::Circle { .. }
                                    | Shape::Arc { .. }
                                    | Shape::Keyhole { .. }
                                    | Shape::Polyline { closed: true, .. },
                                ..
                            }
                        ),
                        part: mark.part,
                        pane,
                        cuts: match mark.ink {
                            Ink::Section { view, .. } => {
                                self.panes.iter().position(|pane| pane.view == Some(view))
                            }
                            _ => None,
                        },
                        item: match mark.ink {
                            Ink::Balloon { item, .. } => Some(item),
                            _ => None,
                        },
                        centre: match mark.ink {
                            Ink::Stroke {
                                shape: Shape::Circle { centre, .. },
                                ..
                            } => Some(raster::through(mark, projection).px(centre)),
                            _ => None,
                        },
                    },
                    moving: mark.moving,
                    pieces,
                }
            })
            .collect();
        Plot::new(
            style,
            &marks,
            Paper {
                size,
                clip: self.layout.drawing(),
                home: self.home(),
            },
        )
    }

    /// Where the pen is kept, and its carousel: the corner of the zone band
    /// at the foot on the left, off the drawing, where a plotter's origin
    /// is.
    fn home(&self) -> Point<i32> {
        let (trim, border) = (self.layout.trim, self.layout.border);

        Point::new(
            (trim.x + border.x) / 2,
            (border.y + border.height - 1 + trim.y + trim.height - 1) / 2,
        )
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

    /// The main view plotted `share` of the way, as the sheet's style plots
    /// it: the buckets plotted whole, each kept, and then the one in
    /// progress; and where the pen's head is.
    fn plotted<Renderer: geometry::Renderer>(
        &self,
        kept: &Kept<Renderer>,
        renderer: &Renderer,
        size: Size,
        sheet: Key,
        share: f32,
        palette: &Palette,
    ) -> (Vec<Geometry<Renderer>>, Option<plotter::pen::Head>) {
        let showing = self.sheet.showing;
        let style = self.sheet.plot;
        let plot = kept
            .plot
            .get((showing.subject, showing.serial, sheet.size, style), || {
                self.plot(&style, sheet.size)
            });
        let now = plot.at(share);
        let mut layers: Vec<Geometry<Renderer>> = (0..now.kept)
            .map(|index| {
                let key = Piece {
                    sheet,
                    wipe: false,
                    index,
                };

                kept.piece(renderer, size, key, |frame| {
                    plot.draw_bucket(&mut Pen::new(frame), palette, index);
                })
            })
            .collect();

        if now.live {
            let mut part = Frame::new(renderer, size);

            plot.draw_live(&mut Pen::new(&mut part), palette, &now);
            layers.push(part.into_geometry());
        }

        (layers, now.head)
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
        let x = match self.sheet.plot.wipe {
            Wipe::Line => left + ((border.width - 2) as f32 * share) as i32,
            // From the parked pen, eased in and out.
            Wipe::Sweep => {
                let from = self.home().x;
                let eased = share * share * (3.0 - 2.0 * share);

                from + ((left + border.width - 2 - from) as f32 * eased) as i32
            }
        };
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
        let mut pen = Pen::new(&mut line);

        pen.vline(x, top, top + height - 1, palette.accent);

        // A gantry's arm clears the sheet it drew, its carriage riding the
        // rails as it goes.
        if self.sheet.plot.gantry && self.sheet.plot.ticks {
            plotter::pen::ticks_across(&mut pen, x, self.layout.trim, border, palette);
        }

        drop(pen);
        geometries.push(line.into_geometry());

        geometries
    }

    /// The title block, parts list, notes and the view's caption.
    fn plates<Renderer: geometry::Renderer>(&self, pen: &mut Pen<'_, Renderer>, palette: &Palette) {
        let (layout, card, sheet) = (&self.layout, self.card(), self.sheet);
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
            &self.card().title,
            Point::new(x, bounds.y),
            Anchor::TOP_LEFT,
            palette.accent,
        );
        x += (self.card().title.chars().count() as i32 + 4) * advance;

        for reading in self.subject().readings(self.sheet.showing.moment.run) {
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

        // Drawn by the pen as far as its plan has drawn it, round the
        // circle as it is now.
        if let Some((_, now)) = &view.pen {
            let ring = self.ring(view);

            return partly(pen, palette, &ring, now.ring, self.layout.drawing())
                .filter(|_| now.drawing == Some(plotter::detail::Drawing::Ring));
        }

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
        if view.focus.time >= MARK {
            let text = letter(view.focus.part).to_string();
            let ((corner, leader, at, anchor), clear) = self.letter_spot(view);

            pen.line(corner, leader, palette.accent);

            // On the sheet's ground over line work it keeps clear of but
            // for the odd pixel, never over an outline it would break.
            if clear {
                pen.fill(knockout(&text, at, anchor), palette.void);
            }

            letters::set(pen, &text, at, anchor, palette.accent);
        }
    }

    /// Where the letter of the circle round the detail on the view goes,
    /// and whether it is clear of the view's outlines: up and right, unless
    /// what the view draws or letters is in the way there and not somewhere
    /// else round the circle. A circle that follows a moving part keeps to
    /// one place.
    fn letter_spot(&self, view: &DetailView) -> (Lettered, bool) {
        let centre = self.main.px(view.detail.centre);
        let radius = self.main.length(view.detail.radius).max(4);

        if view.detail.follows {
            (marker_letter(centre, radius, WAYS[0], 0), true)
        } else {
            let text = letter(view.focus.part).to_string();
            let pieces = pieces(self.shown(), None);
            let edges = outlines(self.shown());

            letter_place(
                centre,
                radius,
                &text,
                &pieces,
                &edges,
                self.layout.drawing(),
            )
        }
    }

    /// The circle round the detail on the view, as the pen draws it.
    fn ring(&self, view: &DetailView) -> plotter::strokes::Stroke {
        plotter::detail::circle(
            self.main.px(view.detail.centre),
            self.main.length(view.detail.radius).max(4),
            raster::Stipple::of(Line::Phantom),
            Tone::Accent,
            &self.sheet.plot,
        )
    }

    /// The detail view's boundary circle, round what the view's circle
    /// marks, as the pen draws it.
    fn window_circle(&self, view: &DetailView) -> plotter::strokes::Stroke {
        plotter::detail::circle(
            view.projection.px(view.detail.centre),
            view.projection.length(view.detail.radius),
            WINDOW_CIRCLE,
            Tone::Faint,
            &self.sheet.plot,
        )
    }

    /// The detail view's pieces, each by what it draws (see
    /// [`plotter::detail::Id`]), in the order they are painted: pass by
    /// pass, the part in focus in the accent.
    fn detail_pieces(&self, view: &DetailView) -> Vec<plotter::detail::Sketched> {
        let mut pieces = Vec::new();
        let mut buffer = Vec::new();
        let mut kinds: std::collections::BTreeMap<plotter::detail::Kind, usize> =
            std::collections::BTreeMap::new();

        for mark in self.draft.marks() {
            if !mark.magnified_for(view.focus.part) {
                continue;
            }

            let kind = (
                mark.pass(),
                mark.part,
                plotter::order::pen(mark.tone),
                kind(&mark.ink),
            );
            let which = kinds.entry(kind).or_default();
            let focused = mark.part == Some(view.focus.part);

            raster::rasterize(mark, &view.projection, &mut buffer);
            pieces.extend(buffer.drain(..).enumerate().map(|(piece, mut inked)| {
                if focused && !matches!(inked.tone, Tone::Live | Tone::Caution) {
                    inked.tone = Tone::Accent;
                }

                (
                    mark.pass(),
                    plotter::detail::Sketched {
                        id: (kind, *which, piece),
                        inked,
                    },
                )
            }));
            *which += 1;
        }

        pieces.sort_by_key(|(pass, _)| *pass);
        pieces.into_iter().map(|(_, sketched)| sketched).collect()
    }

    /// What the pen plots `view` from, as the scene has it.
    fn sketch(&self, view: &DetailView) -> plotter::detail::Sketch {
        let text = letter(view.focus.part).to_string();
        let ((_, _, at, anchor), _) = self.letter_spot(view);
        let top_left = raster::place(LETTERING, &text, at, anchor);

        plotter::detail::Sketch {
            home: self.home(),
            ring: self.ring(view),
            // The middle of the letter.
            letter: Point::new(
                top_left.x + i32::from(LETTERING.width(&text)) / 2,
                top_left.y + i32::from(LETTERING.cap_top()) + i32::from(LETTERING.cap()) / 2,
            ),
            window: self.window_circle(view),
            pieces: self.detail_pieces(view),
            clip: layout::inset(self.layout.detail_window(), 1),
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
        let part = &self.card().parts[view.focus.part];

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

        // Plotted by the pen: each piece as far along as the plan has
        // drawn it, as the piece is now, in the order they are painted.
        if let Some((plan, now)) = &view.pen {
            use plotter::detail::Drawing;

            let circle = self.window_circle(view);
            let mut head = partly(&mut pen, palette, &circle, now.window, inside)
                .filter(|_| now.drawing == Some(Drawing::Window));

            for sketched in self.detail_pieces(view) {
                let piece = &sketched.inked.piece;
                let budget = plotter::detail::budget(piece, inside, plan.drawn(now, sketched.id));

                if budget > 0 {
                    let stopped = sketched.inked.piece.draw(
                        &mut pen,
                        colour(palette, sketched.inked.tone),
                        palette.void,
                        budget,
                        inside,
                    );

                    if now.drawing == Some(Drawing::Piece(sketched.id)) {
                        head = head.or(stopped);
                    }
                }
            }

            return head;
        }

        let marks = self
            .draft
            .marks()
            .iter()
            .filter(|mark| mark.magnified_for(view.focus.part) && view.takes(layer, mark))
            .map(|mark| (mark, &view.projection));
        let share = (plotted < 1.0).then_some(plotted);

        if view.paints(layer) {
            // The boundary of what the main view's circle marks, under the
            // marks so their lettering reads across it.
            let centre = view.projection.px(view.detail.centre);
            let radius = view.projection.length(view.detail.radius);
            let circle = raster::Piece::path(raster::ordered_circle(centre, radius), WINDOW_CIRCLE);

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

/// What a mark is, for telling it from others of its pass and part.
fn kind(ink: &Ink) -> u8 {
    match ink {
        Ink::Stroke { .. } => 0,
        Ink::Arrow { .. } => 1,
        Ink::Area { .. } => 2,
        Ink::Inside { .. } => 3,
        Ink::Label { .. } => 4,
        Ink::Dimension { .. } => 5,
        Ink::Note { .. } => 6,
        Ink::Balloon { .. } => 7,
        Ink::Dot { .. } => 8,
        Ink::Finish { .. } => 9,
        Ink::Datum { .. } => 10,
        Ink::Control { .. } => 11,
        Ink::Section { .. } => 12,
    }
}

/// The dashes of a detail view's boundary circle.
const WINDOW_CIRCLE: raster::Stipple = raster::Stipple::Dash { on: 11, off: 4 };

/// Draws the first `share` of `stroke`'s pixels that it inks, inside
/// `clip`; returns where the pen is if it is part of the way along.
fn partly<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    palette: &Palette,
    stroke: &plotter::strokes::Stroke,
    share: f64,
    clip: Rectangle<i32>,
) -> Option<Head> {
    let count = stroke.pixels.len();
    let passed = ((share * count as f64).round() as usize).min(count);
    let lit: Vec<Point<i32>> = stroke
        .inked(0..passed)
        .filter(|pixel| raster::contains(clip, *pixel))
        .collect();

    raster::fill_pixels(pen, &lit, colour(palette, stroke.tone));

    (passed > 0 && passed < count).then(|| Head(stroke.pixels[passed - 1]))
}

/// The ways out from a detail's circle its letter may stand, the first
/// preferred: its four corners, then its four sides.
const WAYS: [(i32, i32); 8] = [
    (1, -1),
    (-1, -1),
    (1, 1),
    (-1, 1),
    (1, 0),
    (-1, 0),
    (0, -1),
    (0, 1),
];

/// How much further than the least a letter's leader may run, to find a
/// place clear of the view's outlines; how clear of them it keeps, and
/// would rather keep, so it reads as the circle's and not as part of what
/// is drawn beside it.
const FURTHER: [i32; 3] = [0, 6, 12];
const ROOM: i32 = 3;
const ROOMY: i32 = 8;

/// Where a detail circle's letter stands: where its leader leaves the
/// circle and ends, and where the letter is set from and how.
type Lettered = (Point<i32>, Point<i32>, Point<i32>, Anchor);

/// A detail circle's letter the way `(x, y)` out from the circle at
/// `centre` of `radius`, its leader `further` pixels longer than the
/// least.
fn marker_letter(centre: Point<i32>, radius: i32, (x, y): (i32, i32), further: i32) -> Lettered {
    let reach = if x != 0 && y != 0 {
        radius * 7 / 10
    } else {
        radius
    };
    let corner = Point::new(centre.x + x * reach, centre.y + y * reach);
    let leader = Point::new(corner.x + x * (6 + further), corner.y + y * (6 + further));
    let side = match x {
        1 => Horizontal::Left,
        -1 => Horizontal::Right,
        _ => Horizontal::Centre,
    };
    let (at, height) = match y {
        // Over or under the end of a leader up or down.
        _ if x != 0 => (Point::new(leader.x + x * 2, leader.y), Vertical::Middle),
        -1 => (Point::new(leader.x, leader.y - 2), Vertical::Baseline),
        _ => (Point::new(leader.x, leader.y + 2), Vertical::CapTop),
    };

    (corner, leader, at, Anchor::new(side, height))
}

/// Where a detail circle's letter `text` goes round the circle at `centre`
/// of `radius`, given the view's `pieces` and the pixels of its `edges`,
/// inside `drawing`; and whether it is clear of every edge (by [`ROOM`]),
/// so that it can be set on the sheet's ground without breaking one or
/// crowding it.
///
/// Clear of lettering and balloons first, which it would make unreadable,
/// then of the edges, then with room round it, then at a corner on the
/// shortest leader, then where the least line work is, in the order of
/// [`WAYS`].
fn letter_place(
    centre: Point<i32>,
    radius: i32,
    text: &str,
    pieces: &[Inked],
    edges: &std::collections::HashSet<(i32, i32)>,
    drawing: Rectangle<i32>,
) -> (Lettered, bool) {
    let places = FURTHER.iter().flat_map(|&further| {
        WAYS.iter()
            .map(move |&way| (marker_letter(centre, radius, way, further), way, further))
    });

    places
        .filter_map(|(place @ (corner, leader, at, anchor), (x, y), further)| {
            let area = letter_area(corner, leader, text, at, anchor);

            (raster::intersection(area, drawing) == Some(area)).then(|| {
                let under = knockout(text, at, anchor);
                let within = |room: i32| {
                    let around = layout::inset(under, -room);

                    edges
                        .iter()
                        .any(|&(x, y)| raster::contains(around, Point::new(x, y)))
                };
                let crowding = crowding(pieces, area);
                let near = further == 0 && x != 0 && y != 0;

                (
                    place,
                    (
                        crowding >= LETTERED,
                        within(ROOM),
                        within(ROOMY),
                        !near,
                        crowding,
                    ),
                )
            })
        })
        .min_by_key(|(_, key)| *key)
        .map(|(place, (_, blocked, ..))| (place, !blocked))
        .unwrap_or((marker_letter(centre, radius, WAYS[0], 0), false))
}

/// What a detail circle's letter is set on the sheet's ground over: its
/// capitals and a pixel round them.
fn knockout(text: &str, at: Point<i32>, anchor: Anchor) -> Rectangle<i32> {
    let top_left = raster::place(LETTERING, text, at, anchor);
    let cap_top = top_left.y + i32::from(LETTERING.cap_top());

    rect(
        top_left.x - 1,
        cap_top - 1,
        i32::from(LETTERING.width(text)) + 1,
        i32::from(LETTERING.cap()) + 2,
    )
}

/// The pixels of the outlines among `marks`, each through its projection:
/// the visible edges, which a detail's letter is never set over.
fn outlines<'m>(
    marks: impl Iterator<Item = (&'m Mark, &'m Projection)>,
) -> std::collections::HashSet<(i32, i32)> {
    let mut edges = std::collections::HashSet::new();
    let mut buffer = Vec::new();

    for (mark, projection) in marks.filter(|(mark, _)| mark.pass() == crate::draft::Pass::Edges) {
        raster::rasterize(mark, projection, &mut buffer);

        for inked in buffer.drain(..) {
            if let raster::Piece::Path { pixels, .. } = inked.piece {
                edges.extend(pixels.iter().map(|pixel| (pixel.x, pixel.y)));
            }
        }
    }

    edges
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
    use crate::draft::{Measure, Pass, Placement, Scope};
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
        plotted_sheet(subjects, index, output, local, PlotStyle::TODAY)
    }

    /// [`sheet`], plotted in `style`.
    fn plotted_sheet<'a>(
        subjects: &'a [Box<dyn Subject>],
        index: usize,
        output: Output,
        local: f32,
        style: PlotStyle,
    ) -> Sheet<'a> {
        let mut schedule = Schedule::new(
            subjects.iter().map(|s| s.card().parts.len()).collect(),
            0,
            Some(index),
            style.length,
        );

        Sheet {
            subject: subjects[index].as_ref(),
            number: index + 1,
            of: subjects.len(),
            showing: schedule.at(0, local),
            display: output.display,
            date: "2026-10-07",
            plot: style,
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

    /// What is wrong with the annotations the sheet placed itself: one
    /// outside the view, or over another annotation's lettering, or with
    /// its leader across it.
    fn placed_faults(sheet: &Sheet<'_>, scene: &Scene<'_>) -> Vec<String> {
        let mut recorded = Draft::new();
        scene
            .subject()
            .draw(&mut recorded, sheet.showing.moment.run);

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
        let mut faults = Vec::new();

        for (i, (auto, _)) in annotations.iter().enumerate() {
            let (boxes, lines) = &footprints[i];

            if !auto || boxes.is_empty() {
                continue;
            }

            if !(boxes
                .iter()
                .all(|area| raster::intersection(*area, clip) == Some(*area))
                && lines.iter().all(|pixel| raster::contains(clip, *pixel)))
            {
                faults.push(format!("annotation {i} leaves the view"));
            }

            for (j, (others, _)) in footprints.iter().enumerate() {
                if i == j {
                    continue;
                }

                for other in others {
                    if boxes
                        .iter()
                        .any(|area| raster::intersection(*area, *other).is_some())
                    {
                        faults.push(format!("annotation {i} covers annotation {j}"));
                    }
                    if lines.iter().any(|pixel| raster::contains(*other, *pixel)) {
                        faults.push(format!("annotation {i}'s leader crosses annotation {j}"));
                    }
                }
            }
        }

        faults
    }

    /// The box a line of lettering takes.
    fn text_box(at: Point<i32>, text: &str) -> Rectangle<i32> {
        rect(
            at.x,
            at.y,
            i32::from(LETTERING.width(text)),
            i32::from(LETTERING.line()),
        )
    }

    /// What is wrong with the lettering of the sheet's view and detail: a
    /// line of the view's outside it, or over another mark's; a line of
    /// the detail's own across the edge of its window, which cuts it.
    fn lettering_faults(scene: &Scene<'_>) -> Vec<String> {
        let drawing = scene.layout.drawing();
        let mut faults = Vec::new();
        let mut lettered: Vec<(usize, String, Rectangle<i32>)> = Vec::new();

        for (index, mark) in scene.draft.marks().iter().enumerate() {
            if !mark.shown_in(None) {
                continue;
            }

            let mut pieces = Vec::new();
            raster::rasterize(mark, &scene.main, &mut pieces);

            for inked in pieces {
                if let raster::Piece::Text { at, text } = inked.piece {
                    let area = text_box(at, &text);

                    if raster::intersection(area, drawing) != Some(area) {
                        faults.push(format!("{text:?} leaves the view"));
                    }
                    lettered.push((index, text, area));
                }
            }
        }

        for (i, (mark, text, area)) in lettered.iter().enumerate() {
            for (other_mark, other, other_area) in &lettered[i + 1..] {
                if mark != other_mark && raster::intersection(*area, *other_area).is_some() {
                    faults.push(format!("{text:?} is over {other:?}"));
                }
            }
        }

        if let Some(view) = &scene.detail {
            let window = layout::inset(scene.layout.detail_window(), 1);

            for mark in scene.draft.marks() {
                if !matches!(mark.scope, Scope::Detail | Scope::Own)
                    || mark.part != Some(view.focus.part)
                {
                    continue;
                }

                let mut pieces = Vec::new();
                raster::rasterize(mark, &view.projection, &mut pieces);

                for inked in pieces {
                    if let raster::Piece::Text { at, text } = inked.piece {
                        let area = text_box(at, &text);
                        let shown = raster::intersection(area, window);

                        if shown.is_some() && shown != Some(area) {
                            faults.push(format!(
                                "detail {}: {text:?} is cut",
                                letter(view.focus.part)
                            ));
                        }
                    }
                }
            }
        }

        faults
    }

    /// Where lettering breaks an outline: the ground a label clears under
    /// itself taking pixels of a visible edge, in the view or in the
    /// detail, which leaves a gap in the edge.
    fn broken_outlines(scene: &Scene<'_>) -> Vec<String> {
        let mut faults = Vec::new();
        let mut check =
            |marks: Vec<&Mark>, projection: &Projection, clip: Rectangle<i32>, at: &str| {
                let mut edges = Vec::new();
                let mut grounds = Vec::new();

                for mark in marks {
                    let mut pieces = Vec::new();
                    raster::rasterize(mark, projection, &mut pieces);

                    for inked in pieces {
                        match (&mark.ink, inked.piece) {
                            (
                                Ink::Stroke {
                                    line: Line::Outline,
                                    ..
                                },
                                raster::Piece::Path { pixels, .. },
                            ) => edges
                                .extend(pixels.into_iter().filter(|p| raster::contains(clip, *p))),
                            (Ink::Label { text, .. }, raster::Piece::Knockout(area)) => {
                                grounds.push((text.clone(), area));
                            }
                            _ => {}
                        }
                    }
                }

                for (text, area) in grounds {
                    if let Some(pixel) = edges.iter().find(|pixel| raster::contains(area, **pixel))
                    {
                        faults.push(format!("{at}: {text:?} breaks an outline at {pixel:?}"));
                    }
                }
            };

        check(
            scene
                .draft
                .marks()
                .iter()
                .filter(|mark| mark.shown_in(None))
                .collect(),
            &scene.main,
            scene.layout.drawing(),
            "view",
        );

        if let Some(view) = &scene.detail {
            check(
                scene
                    .draft
                    .marks()
                    .iter()
                    .filter(|mark| mark.magnified_for(view.focus.part))
                    .collect(),
                &view.projection,
                layout::inset(scene.layout.detail_window(), 1),
                &format!("detail {}", letter(view.focus.part)),
            );
        }

        faults
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
                    let faults = placed_faults(&sheet, &scene);

                    assert!(
                        faults.is_empty(),
                        "{} on the {} at {local} s: {}",
                        subjects[index].name(),
                        if output == Output::LAPTOP {
                            "laptop"
                        } else {
                            "ultrawide"
                        },
                        faults.join("; ")
                    );
                }
            }
        }
    }

    /// The sheets of the computer read well whatever the computer: for
    /// every kind of machine, on both outputs, at the moment each part is
    /// in detail, the view's lettering is inside it and clear of other
    /// lettering, what the sheet places is clear of it too, and no line of
    /// a detail is cut by its window.
    #[test]
    fn every_machines_sheets_are_lettered_clear_on_both_outputs() {
        use crate::machine::Fixture;

        let mut wrong = Vec::new();

        for fixture in Fixture::ALL {
            let machine = fixture.machine();
            let subjects = subjects::all(&machine);

            for index in 0..subjects.len() {
                if subjects[index].card().domain != subjects::Domain::Computing {
                    continue;
                }

                let parts = subjects[index].card().parts.len();
                let name = subjects[index].name();

                for (output, desk) in [(Output::LAPTOP, "laptop"), (Output::ULTRAWIDE, "ultrawide")]
                {
                    let (width, height) = output.virtual_size();
                    let planned = Planned::default();
                    let mut faults = Vec::new();

                    // Each part in detail, settled.
                    for part in 0..parts {
                        let local = timeline::PLOT_START
                            + timeline::PLOT
                            + timeline::SETTLE
                            + timeline::DETAIL * part as f32
                            + MARK
                            + timeline::DETAIL_PLOT
                            + 1.0;
                        let sheet = sheet(&subjects, index, output, local);
                        let scene = Scene::new(&sheet, width as i32, height as i32, &planned);

                        faults.extend(
                            placed_faults(&sheet, &scene)
                                .into_iter()
                                .chain(lettering_faults(&scene))
                                .chain(broken_outlines(&scene))
                                .map(|fault| format!("at {local:.1} s, {fault}")),
                        );
                    }

                    if !faults.is_empty() {
                        wrong.push(format!(
                            "{fixture:?} {name} on the {desk}: {}",
                            faults[..faults.len().min(4)].join("; ")
                        ));
                    }
                }
            }
        }

        assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    }

    /// A view alone, or a diagram's views together, have as much paper on
    /// either side of what is drawn in them, their annotations and
    /// captions with it, as on the other, a few pixels apart at most for
    /// what moves: for every sheet of every machine, on both outputs. (A
    /// designed subject's moving parts may reach further at one moment
    /// than another, and it is centred on all of their reach.)
    #[test]
    fn the_views_are_drawn_in_the_middle() {
        use crate::machine::Fixture;

        for fixture in Fixture::ALL {
            let subjects = subjects::all(&fixture.machine());

            for (index, subject) in subjects.iter().enumerate() {
                if subject.card().domain != subjects::Domain::Computing {
                    continue;
                }

                for output in [Output::LAPTOP, Output::ULTRAWIDE] {
                    let (width, height) = output.virtual_size();
                    let sheet = sheet(&subjects, index, output, 12.0);
                    let scene =
                        Scene::new(&sheet, width as i32, height as i32, &Planned::default());
                    let view = scene.layout.drawing();
                    let union = |a: Rectangle<i32>, b: Rectangle<i32>| {
                        let (x, y) = (a.x.min(b.x), a.y.min(b.y));
                        let right = (a.x + a.width).max(b.x + b.width);
                        let bottom = (a.y + a.height).max(b.y + b.height);

                        rect(x, y, right - x, bottom - y)
                    };
                    let mut inked: Option<Rectangle<i32>> = scene
                        .panes
                        .iter()
                        .filter_map(|pane| {
                            let (name, top) = pane.caption.as_ref()?;
                            let width = i32::from(LETTERING.width(name));

                            Some(rect(top.x - width / 2, top.y, width, LINE + 1))
                        })
                        .reduce(union);

                    for (mark, projection) in scene.shown() {
                        let mut pieces = Vec::new();
                        raster::rasterize(mark, projection, &mut pieces);

                        for piece in pieces {
                            let area = match piece.piece {
                                raster::Piece::Path { pixels, .. } => pixels
                                    .iter()
                                    .map(|pixel| rect(pixel.x, pixel.y, 1, 1))
                                    .reduce(union),
                                raster::Piece::Text { at, text } => Some(text_box(at, &text)),
                                raster::Piece::Block(area) | raster::Piece::Knockout(area) => {
                                    Some(area)
                                }
                                raster::Piece::Rows { .. } => None,
                            };

                            if let Some(area) = area.and_then(|a| raster::intersection(a, view)) {
                                inked = Some(inked.map_or(area, |inked| union(inked, area)));
                            }
                        }
                    }

                    let inked = inked.expect("Something drawn");
                    let above = inked.y - view.y;
                    let below = view.y + view.height - inked.y - inked.height;
                    let left = inked.x - view.x;
                    let right = view.x + view.width - inked.x - inked.width;

                    assert!(
                        (above - below).abs() <= 8 && (left - right).abs() <= 8,
                        "{fixture:?} {} on {width}: {above} above, {below} below, {left} left, \
                         {right} right",
                        subject.name()
                    );
                }
            }
        }
    }

    /// On a laptop whose sheet is smaller than the desk's laptop's (2256 ×
    /// 1504 at 1.5, 2880 × 1800 at 2), a machine's diagrams fold to fit the
    /// view they have rather than being drawn smaller than their lettering:
    /// the made-up laptop's topology and cooling at a pixel to a unit on
    /// the first, and close to it on the second.
    #[test]
    fn a_smaller_laptop_folds_the_machines_diagrams_to_fit() {
        let subjects = subjects::all(&Machine::fixture());

        for (width, height, scale, least) in [(2256, 1504, 1.5, 1.0), (2880, 1800, 2.0, 0.9)] {
            let output = Output {
                width,
                height,
                scale,
                display: Display {
                    mm_per_vpx: 0.4,
                    estimated: true,
                },
            };
            let (width, height) = output.virtual_size();

            for name in ["topology", "cooling"] {
                let index = subjects::find(&subjects, name).unwrap();
                let sheet = sheet(&subjects, index, output, 30.0);
                let layout = Layout::new(width as i32, height as i32, sheet.subject.card());
                let fitted = fitted(&sheet, &layout);
                let subject = fitted.as_deref().expect("Folded for the smaller view");
                let (panes, _) = arrange(subject, sheet.display, &layout);

                assert!(
                    panes[0].projection.scale >= least,
                    "{name} on {width} × {height}: {}",
                    panes[0].projection.scale
                );
            }
        }
    }

    /// The machine's sheets read as well on the monitors most desks have
    /// as on the laptop: 1920 × 1080 (or 3840 × 2160 at 2), where a diagram
    /// is drawn at a pixel to a unit, and 2560 × 1440, where it is drawn
    /// larger than its details' windows are made for, so they magnify less.
    #[test]
    fn the_machines_sheets_read_well_on_common_monitors() {
        use crate::machine::Fixture;

        for (width, height) in [(1920, 1080), (2560, 1440)] {
            let output = Output {
                width,
                height,
                scale: 1.0,
                display: Display {
                    mm_per_vpx: 0.5,
                    estimated: true,
                },
            };
            let (width, height) = output.virtual_size();

            for (fixture, name) in Fixture::ALL
                .into_iter()
                .flat_map(|fixture| ["topology", "cooling", "displays"].map(|name| (fixture, name)))
            {
                let subjects = subjects::all(&fixture.machine());
                let Some(index) = subjects::find(&subjects, name) else {
                    continue;
                };
                let planned = Planned::default();

                for part in 0..subjects[index].card().parts.len() {
                    let local = timeline::PLOT_START
                        + timeline::PLOT
                        + timeline::SETTLE
                        + timeline::DETAIL * part as f32
                        + MARK
                        + timeline::DETAIL_PLOT
                        + 1.0;
                    let sheet = sheet(&subjects, index, output, local);
                    let scene = Scene::new(&sheet, width as i32, height as i32, &planned);
                    let faults: Vec<String> = placed_faults(&sheet, &scene)
                        .into_iter()
                        .chain(lettering_faults(&scene))
                        .chain(broken_outlines(&scene))
                        .collect();

                    assert!(
                        faults.is_empty(),
                        "{fixture:?} {name} on {width} × {height} at {local:.1} s: {}",
                        faults.join("; ")
                    );
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
                let fitted = fitted(&sheet, &layout);
                let subject = fitted.as_deref().unwrap_or(sheet.subject);
                let (panes, _) = arrange(subject, sheet.display, &layout);
                let views = subject.views();
                let drawing = layout.drawing();
                let name = subjects[index].name();

                assert_eq!(panes[0].view, None);

                for (i, pane) in panes.iter().enumerate() {
                    let extent = match pane.view {
                        None => subject.extent(),
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
    /// displays, for every subject and every kind of machine: a cut value
    /// reads as a different value.
    #[test]
    fn the_title_block_never_cuts_a_value() {
        for fixture in crate::machine::Fixture::ALL {
            the_title_block_never_cuts_a_value_of(&subjects::all(&fixture.machine()));
        }
    }

    fn the_title_block_never_cuts_a_value_of(subjects: &[Box<dyn Subject>]) {
        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            let (width, height) = output.virtual_size();

            for (index, subject) in subjects.iter().enumerate() {
                let mut schedule = Schedule::new(
                    subjects.iter().map(|s| s.card().parts.len()).collect(),
                    0,
                    Some(index),
                    timeline::PLOT,
                );
                let sheet = Sheet {
                    subject: subject.as_ref(),
                    number: index + 1,
                    of: subjects.len(),
                    showing: schedule.at(0, 30.0),
                    display: output.display,
                    date: "2026-10-07",
                    plot: PlotStyle::TODAY,
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
        use std::collections::HashSet;

        let centre = Point::new(200, 200);
        let drawing = rect(0, 0, 400, 400);
        let place = |way: (i32, i32)| marker_letter(centre, 40, way, 0);
        let choose = |pieces: &[Inked], edges: &HashSet<(i32, i32)>| {
            letter_place(centre, 40, "B", pieces, edges, drawing)
        };
        let lettered = |way: (i32, i32)| {
            let (corner, leader, at, anchor) = place(way);
            Inked {
                piece: raster::Piece::Knockout(letter_area(corner, leader, "B", at, anchor)),
                tone: Tone::Ink,
            }
        };
        let none = HashSet::new();
        // An outline across where the letter goes up and right.
        let (_, _, at, _) = place(WAYS[0]);
        let line: HashSet<(i32, i32)> = (150..260).map(|x| (x, at.y)).collect();

        assert_eq!(choose(&[], &none), (place(WAYS[0]), true));
        assert_eq!(choose(&[lettered(WAYS[0])], &none).0, place(WAYS[1]));
        assert_eq!(
            choose(&[lettered(WAYS[0]), lettered(WAYS[1])], &none).0,
            place(WAYS[2])
        );
        // A corner with an outline in the way gives way to a clear one;
        // with every corner taken, a side is.
        assert_eq!(choose(&[lettered(WAYS[1])], &line).0, place(WAYS[2]));
        assert_eq!(
            choose(
                &[lettered(WAYS[1]), lettered(WAYS[2]), lettered(WAYS[3])],
                &line
            ),
            (place(WAYS[4]), true)
        );

        // Where the only place clear of outlines is lettered over (a
        // balloon), it stands by an outline, without the ground behind it,
        // rather than over the lettering.
        let (corner, leader, at, anchor) = place(WAYS[0]);
        let open = layout::inset(letter_area(corner, leader, "B", at, anchor), -ROOMY);
        let crowded: HashSet<(i32, i32)> = (0..400)
            .flat_map(|x| (0..400).map(move |y| (x, y)))
            .filter(|&(x, y)| (x + y) % 3 == 0 && !raster::contains(open, Point::new(x, y)))
            .collect();
        let (chosen, clear) = choose(&[lettered(WAYS[0])], &crowded);

        assert_ne!(chosen, place(WAYS[0]));
        assert!(!clear);
    }

    /// A detail's letter never breaks an outline: with outlines through
    /// every place near the circle it goes further out, and with them
    /// everywhere it is set without the ground behind it.
    #[test]
    fn a_details_letter_never_breaks_an_outline() {
        use std::collections::HashSet;

        let centre = Point::new(200, 200);
        let drawing = rect(0, 0, 400, 400);
        // Outlines round the circle wherever a letter could stand on the
        // shortest leader: rings of pixels from 30 to 60 out.
        let near: HashSet<(i32, i32)> = (100..300_i32)
            .flat_map(|x| (100..300_i32).map(move |y| (x, y)))
            .filter(|&(x, y)| {
                let r = (((x - 200).pow(2) + (y - 200).pow(2)) as f32).sqrt();
                (30.0..60.0).contains(&r) && (x + y) % 3 == 0
            })
            .collect();
        let (place, clear) = letter_place(centre, 40, "B", &[], &near, drawing);
        let (_, _, at, anchor) = place;

        assert!(clear);
        assert!(
            !near
                .iter()
                .any(|&(x, y)| raster::contains(knockout("B", at, anchor), Point::new(x, y)))
        );

        let everywhere: HashSet<(i32, i32)> = (0..400)
            .flat_map(|x| (0..400).map(move |y| (x, y)))
            .filter(|&(x, y)| (x + y) % 3 == 0)
            .collect();

        assert!(!letter_place(centre, 40, "B", &[], &everywhere, drawing).1);
    }

    /// On the displays sheet no dimension's value is lettered over a
    /// display's edge, on either display, whatever the machine's displays.
    #[test]
    fn the_displays_dimensions_clear_their_edges() {
        let mut drawn = 0;

        for fixture in crate::machine::Fixture::ALL {
            let subjects = subjects::all(&fixture.machine());

            if let Some(index) = subjects::find(&subjects, "displays") {
                the_displays_dimensions_clear_their_edges_on(&subjects, index);
                drawn += 1;
            }
        }

        assert!(drawn >= 2);
    }

    fn the_displays_dimensions_clear_their_edges_on(subjects: &[Box<dyn Subject>], index: usize) {
        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            let (width, height) = output.virtual_size();
            let sheet = sheet(subjects, index, output, 12.0);
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

    /// The displays sheet reads well whatever displays a machine has and
    /// wherever it is drawn: a laptop beside a monitor larger than itself,
    /// three monitors and four, on the laptop and the ultrawide, on common
    /// monitors, on a small laptop's screen and on a monitor on its side.
    /// At each display's detail its lettering is clear, its dimensions'
    /// values are off the edges, and a diagonal's value lettered in its
    /// display is a line clear of the display's edges.
    #[test]
    fn the_displays_read_well_on_any_desk() {
        use crate::machine::tests::plugged;
        use crate::machine::{ConnectorKind, Fixture};

        let laptop_and_monitor = {
            let mut machine = Fixture::Laptop.machine();

            for connector in &mut machine.connectors {
                if connector.kind != ConnectorKind::Internal {
                    connector.panel = None;
                }
            }

            plugged(machine, "HDMI-A-1", (797.2, 333.7), (3440, 1440))
        };
        let three = plugged(
            Fixture::Desktop.machine(),
            "DP-2",
            (597.7, 336.2),
            (3840, 2160),
        );
        let four = plugged(three.clone(), "DP-3", (527.0, 296.5), (2560, 1440));
        let monitor = |width, height, mm_per_vpx| Output {
            width,
            height,
            scale: 1.0,
            display: Display {
                mm_per_vpx,
                estimated: false,
            },
        };
        let mut wrong = Vec::new();

        for (desk, machine) in [
            ("laptop", Fixture::Laptop.machine()),
            ("desktop", Fixture::Desktop.machine()),
            ("laptop and monitor", laptop_and_monitor),
            ("three monitors", three),
            ("four monitors", four),
        ] {
            let subjects = subjects::all(&machine);
            let index = subjects::find(&subjects, "displays").expect("A displays sheet");

            for (screen, output) in [
                ("laptop", Output::LAPTOP),
                ("ultrawide", Output::ULTRAWIDE),
                ("1920 × 1080", monitor(1920, 1080, 0.553)),
                ("2560 × 1440", monitor(2560, 1440, 0.467)),
                ("1366 × 768", monitor(1366, 768, 0.504)),
                ("1080 × 1920", monitor(1080, 1920, 0.553)),
            ] {
                let (width, height) = output.virtual_size();
                let planned = Planned::default();

                for part in 0..subjects[index].card().parts.len() {
                    let local = timeline::PLOT_START
                        + timeline::PLOT
                        + timeline::SETTLE
                        + timeline::DETAIL * part as f32
                        + MARK
                        + timeline::DETAIL_PLOT
                        + 1.0;
                    let sheet = sheet(&subjects, index, output, local);
                    let scene = Scene::new(&sheet, width as i32, height as i32, &planned);
                    let faults: Vec<String> = placed_faults(&sheet, &scene)
                        .into_iter()
                        .chain(lettering_faults(&scene))
                        .chain(broken_outlines(&scene))
                        .chain(dimension_faults(&scene))
                        .collect();

                    if !faults.is_empty() {
                        wrong.push(format!(
                            "{desk} on {screen} at {local:.1} s: {}",
                            faults[..faults.len().min(4)].join("; ")
                        ));
                    }
                }
            }
        }

        assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    }

    /// Where a dimension's value is lettered over an edge, or a diagonal's,
    /// lettered in its display, less than a line from one.
    fn dimension_faults(scene: &Scene<'_>) -> Vec<String> {
        let mut values = Vec::new();
        let mut edges = Vec::new();

        for (mark, projection) in scene.shown() {
            let mut pieces = Vec::new();
            raster::rasterize(mark, projection, &mut pieces);

            for inked in pieces {
                match (&mark.ink, inked.piece) {
                    (Ink::Dimension { measure, text }, raster::Piece::Knockout(area)) => {
                        values.push((
                            text.clone(),
                            matches!(measure, Measure::Diagonal { .. }),
                            area,
                        ));
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

        values
            .into_iter()
            .filter_map(|(text, diagonal, area)| {
                let clear = if diagonal { LINE - 1 } else { 0 };
                let gap = |pixel: &Point<i32>| {
                    let across = (area.x - pixel.x).max(pixel.x - (area.x + area.width - 1));
                    let down = (area.y - pixel.y).max(pixel.y - (area.y + area.height - 1));

                    across.max(down)
                };

                edges
                    .iter()
                    .find(|pixel| gap(pixel) <= clear)
                    .map(|pixel| format!("{text:?} is within {clear} of an edge at {pixel:?}"))
            })
            .collect()
    }

    /// The plot as it was drawn before it was worked out once a sheet: the
    /// views' pieces in the order of their passes, cut short, gathered in
    /// buckets by their cost in pen travel and revealed by that cost, all
    /// of it worked out again on every frame; and where the pen stopped.
    fn plotted_as_it_was<Renderer: geometry::Renderer>(
        scene: &Scene<'_>,
        renderer: &Renderer,
        size: Size,
        share: f32,
        palette: &Palette,
    ) -> (Vec<Geometry<Renderer>>, Option<Head>) {
        const PLOT_PIECE: usize = 96;
        const PLOT_BUCKET: usize = 1536;

        let pieces: Vec<Inked> = pieces(scene.shown(), None)
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
        let mut rest = pieces.as_slice();
        let (mut spent, mut head, mut layers) = (0, None, Vec::new());

        while !rest.is_empty() {
            let mut travel = 0;
            let end = rest
                .iter()
                .position(|inked| {
                    travel += inked.piece.cost();
                    travel >= PLOT_BUCKET
                })
                .map_or(rest.len(), |last| last + 1);
            let (bucket, after) = rest.split_at(end);
            let cost: usize = bucket.iter().map(|inked| inked.piece.cost()).sum();
            let mut frame = Frame::new(renderer, size);

            if spent + cost <= budget {
                let mut whole = usize::MAX;
                let _ = draw_pieces(
                    &mut Pen::new(&mut frame),
                    palette,
                    bucket,
                    clip,
                    &mut whole,
                    Paths::Apart,
                );
                layers.push(frame.into_geometry());
            } else {
                let mut left = budget - spent;
                head = draw_pieces(
                    &mut Pen::new(&mut frame),
                    palette,
                    bucket,
                    clip,
                    &mut left,
                    Paths::Apart,
                );
                layers.push(frame.into_geometry());
                break;
            }

            spent += cost;
            rest = after;
        }

        (layers, head)
    }

    /// A sheet's main view plotted to its moment, with the pen's head: as
    /// the sheet plots it, or as it was plotted before.
    struct Plotted<'a> {
        sheet: Sheet<'a>,
        as_it_was: bool,
    }

    impl canvas::Program<(), Theme, iced_renderer::Renderer> for Plotted<'_> {
        type State = Kept<iced_renderer::Renderer>;

        fn draw(
            &self,
            kept: &Self::State,
            renderer: &iced_renderer::Renderer,
            theme: &Theme,
            bounds: Rectangle,
            _cursor: mouse::Cursor,
        ) -> Vec<Geometry<iced_renderer::Renderer>> {
            let (width, height) = (bounds.width as i32, bounds.height as i32);
            let scene = Scene::new(&self.sheet, width, height, &kept.plan);
            let palette = theme.palette();
            let Phase::Plot(share) = self.sheet.showing.moment.phase else {
                unreachable!("a moment of the plot")
            };
            let mut frame = Frame::new(renderer, bounds.size());
            let mut layers = if self.as_it_was {
                let (layers, head) =
                    plotted_as_it_was(&scene, renderer, bounds.size(), share, palette);

                if let Some(Head(at)) = head {
                    Pen::new(&mut frame).crosshair(at, 3, 1, palette.accent);
                }

                layers
            } else {
                let key = Key {
                    subject: self.sheet.showing.subject,
                    serial: self.sheet.showing.serial,
                    focus: None,
                    marked: false,
                    size: (width, height),
                };
                let (layers, head) =
                    scene.plotted(kept, renderer, bounds.size(), key, share, palette);

                if let Some(head) = head {
                    plotter::pen::draw(&mut Pen::new(&mut frame), head, palette);
                }

                layers
            };

            layers.push(frame.into_geometry());
            layers
        }
    }

    /// Fails at the first moment of a sheet of each of `names`, on each of
    /// `outputs`, that today's plot draws differently from the plot as it
    /// was: `moments` moments all through the plot and its last.
    fn plotted_as_it_was_on(names: &[&str], outputs: &[Output], moments: usize) {
        let subjects = subjects::all(&Machine::fixture());
        let theme = Theme::TERMINAL;
        let end = timeline::PLOT_START + timeline::PLOT;

        for name in names {
            let index = subjects::find(&subjects, name).expect("A subject");

            for &output in outputs {
                let size = output.virtual_size();
                let moments = (0..moments)
                    .map(|step| step as f32 * end / moments as f32)
                    .chain([end - 0.001]);

                for local in moments {
                    let drawn = |as_it_was| {
                        crate::headless::screenshot(
                            Plotted {
                                sheet: sheet(&subjects, index, output, local),
                                as_it_was,
                            },
                            size,
                            &theme,
                        )
                    };

                    assert!(
                        drawn(false) == drawn(true),
                        "{name} on {} at {local} s",
                        output.name()
                    );
                }
            }
        }
    }

    /// Today's plot is the plot as it was, pixel for pixel, the pen's head
    /// with it: a sheet on each output at moments all through the plot.
    #[test]
    fn todays_plot_is_the_plot_as_it_was() {
        plotted_as_it_was_on(&["gears"], &[Output::LAPTOP], 8);
        plotted_as_it_was_on(&["topology"], &[Output::ULTRAWIDE], 8);
    }

    /// Several subjects' sheets, on both outputs, at many moments.
    #[test]
    #[ignore = "minutes: run with --release --ignored"]
    fn todays_plot_is_the_plot_as_it_was_at_many_moments() {
        plotted_as_it_was_on(
            &["gears", "engine", "timer", "topology", "cooling"],
            &[Output::LAPTOP, Output::ULTRAWIDE],
            28,
        );
    }

    /// The pixels the traces that wait for the run paint in its first
    /// frame: what moves along the drawing's traces.
    fn waiting_traces(
        sheet: &Sheet<'_>,
        size: (i32, i32),
    ) -> std::collections::HashSet<(u32, u32)> {
        let scene = Scene::new(sheet, size.0, size.1, &Planned::default());
        let clip = scene.layout.drawing();
        let mut pixels = std::collections::HashSet::new();

        for (mark, projection) in scene.shown() {
            if !(mark.moving && mark.pass() == Pass::Traces) {
                continue;
            }

            let mut pieces = Vec::new();
            raster::rasterize(mark, projection, &mut pieces);

            for inked in pieces {
                plotter::own::painted(&inked.piece, clip, |pixel| {
                    pixels.insert((pixel.x as u32, pixel.y as u32));
                });
            }
        }

        pixels
    }

    /// Fails unless, in each of `styles`, the last frame of the plot a
    /// surface shows at thirty frames a second is the run's first, but for
    /// the traces that wait for the run to appear: on every sheet, on both
    /// outputs.
    fn ends_on_the_run(styles: &[PlotStyle]) {
        use crate::headless::Studio;

        let subjects = subjects::all(&Machine::fixture());
        let theme = Theme::TERMINAL;
        let mut wrong = Vec::new();

        for &style in styles {
            for output in [Output::LAPTOP, Output::ULTRAWIDE] {
                let (width, height) = output.virtual_size();
                let mut studio = Studio::new(&subjects, output, &theme, "2026-10-07")
                    .expect("A studio")
                    .plotting(style);
                let end = timeline::PLOT_START + style.length;

                for index in 0..subjects.len() {
                    let last = studio.frame(index, end - 1.0 / 30.0);
                    let first = studio.frame(index, end);
                    let waiting = if style.traces_wait {
                        waiting_traces(
                            &plotted_sheet(&subjects, index, output, end, style),
                            (width as i32, height as i32),
                        )
                    } else {
                        std::collections::HashSet::new()
                    };
                    let differ = last
                        .enumerate_pixels()
                        .filter(|(x, y, pixel)| {
                            *pixel != first.get_pixel(*x, *y) && !waiting.contains(&(*x, *y))
                        })
                        .count();

                    if differ > 0 {
                        wrong.push(format!(
                            "{} on {}, {} (traces wait: {}): {differ} pixels",
                            subjects[index].name(),
                            output.name(),
                            style.name,
                            style.traces_wait
                        ));
                    }
                }
            }
        }

        assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    }

    /// The pen's plot ends on the drawing the subject's run begins with,
    /// exactly, every trace plotted with the rest: no ink is drawn over,
    /// none wiped, on any sheet.
    #[test]
    fn the_last_plot_frame_is_the_first_run_frame() {
        ends_on_the_run(&[PlotStyle {
            traces_wait: false,
            ..PlotStyle::DRAFTING
        }]);
    }

    /// In every style, and with the traces waiting for the run.
    #[test]
    #[ignore = "minutes: run with --release --ignored"]
    fn every_style_ends_on_the_run() {
        ends_on_the_run(&[PlotStyle::CAROUSEL, PlotStyle::DRAFTING, PlotStyle::QUICK]);
    }

    /// Fails at the first frame of a plot in each of `styles`, of each of
    /// `names`, drawn `fps` a second with the pen's head hidden, that does
    /// not draw every pixel the frame before it drew.
    fn only_adds_ink(styles: &[PlotStyle], names: &[&str], fps: f32) {
        use crate::headless::Studio;

        let subjects = subjects::all(&Machine::fixture());
        let theme = Theme::TERMINAL;
        let void = image::Rgba(theme.palette().void.into_rgba8());

        for &style in styles {
            let style = PlotStyle {
                head: false,
                ..style
            };
            let mut studio = Studio::new(&subjects, Output::LAPTOP, &theme, "2026-10-07")
                .expect("A studio")
                .plotting(style);

            for name in names {
                let index = subjects::find(&subjects, name).expect("A subject");
                let frames = ((timeline::PLOT_START + style.length) * fps) as usize;
                let mut before = studio.frame(index, 0.0);

                for frame in 1..=frames {
                    let at = frame as f32 / fps;
                    let now = studio.frame(index, at);
                    let lost = before
                        .enumerate_pixels()
                        .filter(|(x, y, pixel)| **pixel != void && *pixel != now.get_pixel(*x, *y))
                        .count();

                    assert!(lost == 0, "{name}, {}, {at} s: {lost} pixels", style.name);
                    before = now;
                }
            }
        }
    }

    /// The pen's plot only ever adds ink: no frame of it wipes or draws
    /// over a pixel a frame before it drew.
    #[test]
    fn ink_only_accumulates_during_the_plot() {
        only_adds_ink(&[PlotStyle::DRAFTING], &["gears", "cooling"], 2.0);
    }

    /// In every style, on sheets of each kind, four frames a second.
    #[test]
    #[ignore = "minutes: run with --release --ignored"]
    fn ink_only_accumulates_in_every_style() {
        only_adds_ink(
            &[PlotStyle::CAROUSEL, PlotStyle::DRAFTING, PlotStyle::QUICK],
            &["gears", "engine", "timer", "topology", "cooling"],
            4.0,
        );
    }

    /// A plot is the same drawn by a studio that has drawn other sheets'
    /// plots first as by a fresh one, in every style.
    #[test]
    fn a_plot_is_the_same_from_a_fresh_studio() {
        use crate::headless::Studio;

        let subjects = subjects::all(&Machine::fixture());
        let theme = Theme::TERMINAL;

        for style in [PlotStyle::CAROUSEL, PlotStyle::DRAFTING, PlotStyle::QUICK] {
            let studio = || {
                Studio::new(&subjects, Output::LAPTOP, &theme, "2026-10-07")
                    .expect("A studio")
                    .plotting(style)
            };
            let mut busy = studio();

            for first in 0..subjects.len() {
                let _ = busy.frame(first, 4.0);
            }

            for first in [0, subjects.len() - 1] {
                for at in [2.0, 5.5] {
                    assert!(
                        busy.frame(first, at) == studio().frame(first, at),
                        "{}, subject {first} at {at} s",
                        style.name
                    );
                }
            }
        }
    }

    /// A detail the pen plots is worked out from the drawing as it stood
    /// when its part was picked out, so it is drawn the same by a surface
    /// that has shown it from the start as by one that begins halfway
    /// through it, a part moving or not.
    #[test]
    fn a_plotted_detail_is_the_same_from_any_frame() {
        use crate::headless::Studio;

        let subjects = subjects::all(&Machine::fixture());
        let theme = Theme::TERMINAL;
        let style = PlotStyle::CAROUSEL;
        let studio = || {
            Studio::new(&subjects, Output::LAPTOP, &theme, "2026-10-07")
                .expect("A studio")
                .plotting(style)
        };

        for name in ["gears", "engine"] {
            let index = subjects::find(&subjects, name).expect("A subject");
            let picked = timeline::PLOT_START + style.length + timeline::SETTLE;
            let mut watching = studio();

            for frame in 0..=20 {
                let at = picked + frame as f32 / 10.0;
                let watched = watching.frame(index, at);

                if frame % 5 == 3 {
                    assert!(watched == studio().frame(index, at), "{name} at {at} s");
                }
            }
        }
    }

    /// A part's detail drawn on its own, with its circle on the view: as
    /// the pen has plotted it by its sheet's moment, or drawn whole as it
    /// is once it settles.
    struct DetailIn<'a> {
        sheet: Sheet<'a>,
        pen: bool,
    }

    impl canvas::Program<(), Theme, iced_renderer::Renderer> for DetailIn<'_> {
        type State = Kept<iced_renderer::Renderer>;

        fn draw(
            &self,
            kept: &Self::State,
            renderer: &iced_renderer::Renderer,
            theme: &Theme,
            bounds: Rectangle,
            _cursor: mouse::Cursor,
        ) -> Vec<Geometry<iced_renderer::Renderer>> {
            let size = (bounds.width as i32, bounds.height as i32);
            let mut scene = Scene::new(&self.sheet, size.0, size.1, &kept.plan);
            let view = scene.detail.as_mut().expect("A part in detail");

            if self.pen {
                let plan = Rc::new(self.sheet.detail_plot(view.focus, size, &kept.plan));
                let now = plan.at(view.focus.time);

                view.pen = Some((plan, now));
            } else {
                // Settled, its whole drawing drawn.
                view.focus.time = MARK + timeline::DETAIL_PLOT + 0.1;
            }

            let view = scene.detail.as_ref().expect("A part in detail");
            let mut frame = Frame::new(renderer, bounds.size());
            let _ = scene.detail_marker(&mut Pen::new(&mut frame), theme.palette(), view);
            let _ = scene.detail_view(&mut frame, theme.palette(), view, Layer::All);

            vec![frame.into_geometry()]
        }
    }

    /// The pen finishes a detail on what the detail shows once it settles,
    /// exactly, on every sheet and both outputs: nothing pops in, nothing is
    /// left over.
    #[test]
    fn a_plotted_detail_ends_on_the_settled_detail() {
        let subjects = subjects::all(&Machine::fixture());
        let theme = Theme::TERMINAL;
        let style = PlotStyle::CAROUSEL;
        // The pen home and gone, just before the detail settles.
        let local =
            timeline::PLOT_START + style.length + timeline::SETTLE + MARK + timeline::DETAIL_PLOT
                - plotter::motion::REST / 2.0;

        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            for index in 0..subjects.len() {
                let drawn = |pen| {
                    crate::headless::screenshot(
                        DetailIn {
                            sheet: plotted_sheet(&subjects, index, output, local, style),
                            pen,
                        },
                        output.virtual_size(),
                        &theme,
                    )
                };
                let (plotted, settled) = (drawn(true), drawn(false));
                let differ = plotted
                    .chunks_exact(4)
                    .zip(settled.chunks_exact(4))
                    .filter(|(a, b)| a != b)
                    .count();

                assert!(
                    differ == 0,
                    "{} on {}: {differ} pixels",
                    subjects[index].name(),
                    output.name()
                );
            }
        }
    }
}
