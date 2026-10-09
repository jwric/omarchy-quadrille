//! The plotter: a sheet's drawing turned into the pen's work once, and
//! played back by time.
//!
//! [`Plot::new`] works a sheet's plot out on its first frame. The pieces of
//! its marks become the pen's strokes ([`strokes`]), each pixel drawn by
//! the one stroke that owns it in the finished drawing ([`own`]). The
//! strokes are put in the style's order ([`order`]), and the pen's moves
//! along and between them are timed and fitted to the plot's length
//! ([`motion`]). Each frame then looks its moment up ([`Plot::at`]): the
//! strokes done, how far into the next the pen is, and where its head is.
//!
//! The plot is drawn in buckets of consecutive strokes, each kept as a
//! drawing of its own once it is done, so a frame draws again only the
//! bucket in progress, a path for each stretch of a stroke, and the
//! renderer's damage is the stretch the pen is on.
//!
//! The plot as it was, [`Motion::Reveal`], is kept exactly: every piece in
//! the order of its passes, revealed at one rate of pen travel.
pub mod detail;
pub mod motion;
pub mod order;
pub mod own;
pub mod pen;
pub mod strokes;
pub mod style;

use std::collections::BTreeMap;
use std::ops::Range;
use std::time::{Duration, Instant};

use iced_core::{Point, Rectangle};
use iced_widget::graphics::geometry;
use quadrille::Palette;
use quadrille::draw::Pen;

use crate::draft::Pass;
use crate::draft::raster::{self, Inked, colour};

use super::{Paths, draw_pieces};
use motion::Motions;
use order::Meta;
use own::Owners;
use pen::Head;
use strokes::{Form, Making, Stroke};
use style::{Circles, Motion, PlotStyle};

/// The longest piece of the plot as it was, and stretch of a stroke, in
/// pixels of pen travel: each a path of its own, so the pen's damage is the
/// stretch it is on...
const PLOT_PIECE: usize = 96;
/// ...and the pen travel, or ink, of each separately kept bucket of them.
const PLOT_BUCKET: usize = 1536;

/// The laptop's sheet height, which a style's lengths and speeds are for.
const REFERENCE: f64 = 533.0;

/// The least radius of a circle or arc a compass draws, in pixels of the
/// laptop's sheet.
const COMPASS: f64 = 12.0;

/// Whether a compass draws a circle or an arc of `radius` pixels on a
/// sheet `height` virtual pixels high: one too big to draw freehand.
pub fn compasses(radius: i32, height: i32) -> bool {
    f64::from(radius) >= COMPASS * scale(height)
}

/// How much a style's lengths and speeds are scaled on a sheet `height`
/// virtual pixels high, so a plot takes as long on every output.
pub fn scale(height: i32) -> f64 {
    f64::from(height) / REFERENCE
}

/// A mark to plot, as its view projects it.
pub struct Marked {
    pub meta: Meta,
    /// Whether it moves while the subject runs: drawn over what does not.
    pub moving: bool,
    pub pieces: Vec<Inked>,
}

/// What a sheet is plotted on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paper {
    /// The sheet's size, in virtual pixels.
    pub size: (i32, i32),
    /// What the drawing is drawn inside.
    pub clip: Rectangle<i32>,
    /// Where the pen is kept off the drawing.
    pub home: Point<i32>,
    /// Whether the drawing is a diagram, not drawn to scale.
    pub diagram: bool,
}

/// A sheet's plot, worked out once.
pub struct Plot {
    work: Work,
    /// The ranges of pieces, or strokes, kept as one drawing once plotted.
    buckets: Vec<Range<usize>>,
    clip: Rectangle<i32>,
    /// The centre of each mark a compass draws, by its index.
    compasses: Vec<Option<Point<i32>>>,
    pub cues: Cues,
    pub stats: Stats,
}

/// When the pen reaches what fills a sheet's form in, in seconds of the
/// plot: what a drafting office lists and signs as its drawing proceeds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cues {
    /// When the annotation is begun.
    pub annotation: Option<f64>,
    /// When each balloon's number is begun, by its item.
    pub balloons: BTreeMap<usize, f64>,
    /// When each view's bodies are done, by its pane.
    pub views: BTreeMap<usize, f64>,
    /// When the last stroke is done.
    pub drawn: f64,
}

#[derive(Debug, PartialEq)]
enum Work {
    /// The plot as it was: pieces revealed by their cost in pen travel.
    Reveal {
        pieces: Vec<Inked>,
        /// The cost plotted by the end of each bucket.
        ends: Vec<usize>,
        total: usize,
    },
    /// The pen's strokes, and its work drawing them.
    Pen {
        strokes: Vec<Stroke>,
        motions: Motions,
        length: f64,
    },
}

/// What a plot has done at a moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    /// How many buckets are plotted whole: kept from now on.
    pub kept: usize,
    /// Whether the next is plotted in part.
    pub live: bool,
    pub head: Option<Head>,
    progress: Progress,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Progress {
    /// How much of the pieces' cost is plotted.
    Reveal(usize),
    /// How many strokes are done, and the one being drawn with how many of
    /// its pixels the pen has passed.
    Pen {
        done: usize,
        drawing: Option<(usize, usize)>,
    },
}

/// What a plot is made of and how its time is spent: what tunes a style.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    /// Seconds the pen's moves and its pauses take as planned...
    pub moving: f64,
    pub pausing: f64,
    /// ...and how much faster than planned its moves are played to fit the
    /// plot (more than 1 is faster), and its pauses.
    pub k: f64,
    pub p: f64,
    /// Seconds of the plot the pen spends drawing, carried up and still.
    pub drawing: f64,
    pub travelling: f64,
    pub still: f64,
    /// Letters and dots set at a touch; and letters lettered a second,
    /// over the time the pen spends lettering.
    pub touches: usize,
    pub lettering: f64,
    pub strokes: usize,
    pub groups: usize,
    /// Pixels inked, and carried over with the pen up.
    pub ink: usize,
    pub travel: f64,
    /// How long working the plot out took.
    pub planned: Duration,
}

impl Plot {
    /// The plot of `marks` on `paper`, as `style` plots it.
    pub fn new(style: &PlotStyle, marks: &[Marked], paper: Paper) -> Self {
        let started = Instant::now();
        let mut plot = match style.motion {
            Motion::Reveal => Self::reveal(marks, paper),
            Motion::Physical(_) | Motion::Eased(_) => Self::pen(style, marks, paper),
        };

        plot.stats.planned = started.elapsed();
        plot
    }

    /// The plot as it was: the pieces of every mark in the order of their
    /// passes, cut short.
    fn reveal(marks: &[Marked], paper: Paper) -> Self {
        let mut passed: Vec<(Pass, &Inked)> = marks
            .iter()
            .flat_map(|marked| marked.pieces.iter().map(|inked| (marked.meta.pass, inked)))
            .collect();

        passed.sort_by_key(|(pass, _)| *pass);

        let pieces: Vec<Inked> = passed
            .into_iter()
            .flat_map(|(_, inked)| {
                let tone = inked.tone;

                inked
                    .piece
                    .clone()
                    .split(PLOT_PIECE)
                    .into_iter()
                    .map(move |piece| Inked { piece, tone })
            })
            .collect();
        let costs: Vec<usize> = pieces.iter().map(|inked| inked.piece.cost()).collect();
        let buckets = buckets(&costs, PLOT_BUCKET);
        let ends = buckets
            .iter()
            .scan(0, |spent, bucket| {
                *spent += costs[bucket.clone()].iter().sum::<usize>();
                Some(*spent)
            })
            .collect();
        let total = costs.iter().sum();

        Self {
            stats: Stats {
                strokes: pieces.len(),
                ink: total,
                k: 1.0,
                p: 1.0,
                ..Stats::default()
            },
            work: Work::Reveal {
                pieces,
                ends,
                total,
            },
            buckets,
            clip: paper.clip,
            compasses: Vec::new(),
            cues: Cues::default(),
        }
    }

    /// The pen's plot: strokes inking what they own, in the style's order,
    /// timed as it moves.
    fn pen(style: &PlotStyle, marks: &[Marked], paper: Paper) -> Self {
        // Every piece by its number from one, in the order made...
        let ids: Vec<Vec<u32>> = marks
            .iter()
            .scan(0, |count, marked| {
                let ids = (0..marked.pieces.len())
                    .map(|index| *count + index as u32 + 1)
                    .collect();

                *count += marked.pieces.len() as u32;
                Some(ids)
            })
            .collect();

        // ...painted as the subject's run first shows them: what is still,
        // pass by pass, then what moves over it. Traces left for the run
        // are not plotted, and what they would cover is.
        let waits = |marked: &Marked| {
            style.traces_wait && marked.moving && marked.meta.pass == Pass::Traces
        };
        let mut owners = Owners::new(paper.size.0, paper.size.1);
        let mut painting: Vec<usize> = (0..marks.len())
            .filter(|&index| !waits(&marks[index]))
            .collect();

        painting.sort_by_key(|&index| (marks[index].moving, marks[index].meta.pass));

        for index in painting {
            for (piece, inked) in marks[index].pieces.iter().enumerate() {
                owners.paint(ids[index][piece], &inked.piece, paper.clip);
            }
        }

        let making = Making {
            lining: style.lining,
            glyphs: style.glyphs,
        };
        let made: Vec<Stroke> = marks
            .iter()
            .enumerate()
            .filter(|(_, marked)| !waits(marked))
            .flat_map(|(mark, marked)| {
                let (owners, ids) = (&owners, &ids);
                let centre = marked
                    .meta
                    .centre
                    .filter(|_| style.circles == Circles::Centred);

                marked
                    .pieces
                    .iter()
                    .enumerate()
                    .flat_map(move |(piece, inked)| {
                        strokes::strokes(
                            inked,
                            (mark, piece, ids[mark][piece]),
                            owners,
                            paper.clip,
                            making,
                            centre,
                        )
                    })
            })
            .collect();
        let metas: Vec<Meta> = marks.iter().map(|marked| marked.meta).collect();
        // A diagram grows along what touches.
        let touching = (style.grow && paper.diagram).then(|| {
            let whose: Vec<usize> = std::iter::once(0)
                .chain(
                    ids.iter()
                        .enumerate()
                        .flat_map(|(mark, ids)| ids.iter().map(move |_| mark)),
                )
                .collect();

            owners.touching(&made, |id| whose[id as usize])
        });
        let ordered = order::order(style, made, &metas, paper.home, touching.as_ref());
        let scale = scale(paper.size.1);
        let motions = Motions::plan(style, &ordered.strokes, &ordered.gaps, paper.home, scale);
        let inks: Vec<usize> = ordered.strokes.iter().map(Stroke::ink).collect();
        let buckets = buckets(&inks, PLOT_BUCKET);

        let mut stats = Stats {
            moving: motions.moving,
            pausing: motions.pausing,
            k: motions.k,
            p: motions.p,
            touches: ordered
                .strokes
                .iter()
                .filter(|stroke| matches!(stroke.form, Form::Touch(_)))
                .count(),
            lettering: lettering(marks, &ordered.strokes, &motions),
            strokes: ordered.strokes.len(),
            groups: ordered.groups,
            ink: inks.iter().sum(),
            travel: motions.travel,
            ..Stats::default()
        };

        for op in &motions.ops {
            match op.act {
                motion::Act::Draw { .. } | motion::Act::Touch { .. } => stats.drawing += op.time,
                motion::Act::Travel { .. } => stats.travelling += op.time,
                motion::Act::Still { .. } | motion::Act::Change { .. } => stats.still += op.time,
            }
        }

        let compasses = metas
            .iter()
            .map(|meta| {
                meta.compass
                    .filter(|&(_, radius)| compasses(radius, paper.size.1))
                    .map(|(centre, _)| centre)
            })
            .collect();

        Self {
            cues: cues(&ordered, &metas, &motions),
            work: Work::Pen {
                strokes: ordered.strokes,
                motions,
                length: f64::from(style.length),
            },
            buckets,
            clip: paper.clip,
            compasses,
            stats,
        }
    }

    /// Where the plot is with `share` of its time gone.
    pub fn at(&self, share: f32) -> Now {
        match &self.work {
            Work::Reveal {
                pieces,
                ends,
                total,
            } => {
                let budget = (share * *total as f32) as usize;
                let kept = ends.partition_point(|end| *end <= budget);
                let live = kept < self.buckets.len();
                let mut left = budget - if kept > 0 { ends[kept - 1] } else { 0 };
                let mut head = None;

                // Where the pen stopped: in the piece the budget ran out in.
                if live {
                    for inked in &pieces[self.buckets[kept].clone()] {
                        let cost = inked.piece.cost();

                        if left <= cost {
                            head = inked.piece.head(left, self.clip).map(|at| Head::down(at.0));
                            break;
                        }

                        left -= cost;
                    }
                }

                Now {
                    kept,
                    live,
                    head,
                    progress: Progress::Reveal(budget),
                }
            }
            Work::Pen {
                strokes,
                motions,
                length,
            } => {
                let place = motions.at(f64::from(share) * length, strokes);
                let kept = self
                    .buckets
                    .partition_point(|bucket| bucket.end <= place.done);

                Now {
                    kept,
                    live: kept < self.buckets.len(),
                    head: place.head,
                    progress: Progress::Pen {
                        done: place.done,
                        drawing: place.drawing,
                    },
                }
            }
        }
    }

    /// The pixels inked in the last `seconds` before `share` of the plot's
    /// time is gone: the ink still wet.
    pub fn wet(&self, share: f32, seconds: f32) -> Vec<Point<i32>> {
        let Work::Pen {
            strokes,
            motions,
            length,
        } = &self.work
        else {
            return Vec::new();
        };
        let time = f64::from(share) * length;
        // How far through the strokes the pen is: a stroke, and how many of
        // its pixels it has passed.
        let reached = |time: f64| {
            let place = motions.at(time.max(0.0), strokes);

            place.drawing.unwrap_or((place.done, 0))
        };
        let (then, now) = (reached(time - f64::from(seconds)), reached(time));

        (then.0..=now.0.min(strokes.len().saturating_sub(1)))
            .filter(|_| then != now)
            .flat_map(|index| {
                let stroke = &strokes[index];
                let from = if index == then.0 { then.1 } else { 0 };
                let to = if index == now.0 {
                    now.1
                } else {
                    stroke.pixels.len()
                };

                stroke.inked(from..to.max(from))
            })
            .collect()
    }

    /// Where the point of the compass is at `now`, if one is drawing.
    pub fn compass(&self, now: &Now) -> Option<Point<i32>> {
        let (Work::Pen { strokes, .. }, Progress::Pen { drawing, .. }) = (&self.work, now.progress)
        else {
            return None;
        };
        let (index, _) = drawing?;

        self.compasses.get(strokes[index].mark).copied().flatten()
    }

    /// Draws bucket `index` whole.
    pub fn draw_bucket<Renderer: geometry::Renderer>(
        &self,
        pen: &mut Pen<'_, Renderer>,
        palette: &Palette,
        index: usize,
    ) {
        let bucket = self.buckets[index].clone();

        match &self.work {
            Work::Reveal { pieces, .. } => {
                let mut whole = usize::MAX;
                let _ = draw_pieces(
                    pen,
                    palette,
                    &pieces[bucket],
                    self.clip,
                    &mut whole,
                    Paths::Apart,
                );
            }
            Work::Pen { strokes, .. } => {
                for stroke in &strokes[bucket] {
                    draw_stroke(pen, palette, stroke, stroke.pixels.len());
                }
            }
        }
    }

    /// Draws as much of the bucket in progress at `now` as is plotted.
    pub fn draw_live<Renderer: geometry::Renderer>(
        &self,
        pen: &mut Pen<'_, Renderer>,
        palette: &Palette,
        now: &Now,
    ) {
        let Some(bucket) = self.buckets.get(now.kept).cloned() else {
            return;
        };

        match (&self.work, now.progress) {
            (Work::Reveal { pieces, ends, .. }, Progress::Reveal(budget)) => {
                let mut left = budget - if now.kept > 0 { ends[now.kept - 1] } else { 0 };
                let _ = draw_pieces(
                    pen,
                    palette,
                    &pieces[bucket],
                    self.clip,
                    &mut left,
                    Paths::Apart,
                );
            }
            (Work::Pen { strokes, .. }, Progress::Pen { done, drawing }) => {
                for stroke in &strokes[bucket.start..done.max(bucket.start)] {
                    draw_stroke(pen, palette, stroke, stroke.pixels.len());
                }

                if let Some((index, passed)) = drawing {
                    draw_stroke(pen, palette, &strokes[index], passed);
                }
            }
            _ => unreachable!("a plot's moment is its own"),
        }
    }
}

/// When the pen reaches what fills the form in, drawing `ordered`, whose
/// marks `metas` says what they are, as `motions` says.
fn cues(ordered: &order::Ordered, metas: &[Meta], motions: &Motions) -> Cues {
    use order::stage;

    // Each stroke's start and end.
    let mut times = vec![(0.0, 0.0); ordered.strokes.len()];

    for op in &motions.ops {
        if let motion::Act::Draw { stroke, .. } | motion::Act::Touch { stroke } = op.act {
            times[stroke] = (op.start, op.start + op.time);
        }
    }

    // A balloon's number is the last of its pieces.
    let mut numbers: BTreeMap<usize, usize> = BTreeMap::new();

    for stroke in &ordered.strokes {
        let number = numbers.entry(stroke.mark).or_default();
        *number = (*number).max(stroke.piece);
    }

    let mut cues = Cues::default();

    for (index, stroke) in ordered.strokes.iter().enumerate() {
        let (start, end) = times[index];
        let meta = &metas[stroke.mark];

        match ordered.stages[index] {
            stage::ANNOTATION => {
                cues.annotation = Some(cues.annotation.map_or(start, |cue: f64| cue.min(start)));
            }
            stage::BALLOONS if stroke.piece == numbers[&stroke.mark] => {
                if let Some(item) = meta.item {
                    cues.balloons.entry(item).or_insert(start);
                }
            }
            stage::BODIES => {
                let view = cues.views.entry(meta.pane).or_insert(end);
                *view = view.max(end);
            }
            _ => {}
        }

        cues.drawn = cues.drawn.max(end);
    }

    cues
}

/// Letters a second in a plot of `marks` drawing `strokes` as `motions`
/// says: the letters of every line of lettering plotted, over the time the
/// pen spends on them, from its first letter's first stroke to its last's
/// end.
fn lettering(marks: &[Marked], strokes: &[Stroke], motions: &Motions) -> f64 {
    let letters: std::collections::BTreeMap<(usize, usize), usize> = marks
        .iter()
        .enumerate()
        .flat_map(|(mark, marked)| {
            marked
                .pieces
                .iter()
                .enumerate()
                .filter_map(move |(piece, inked)| match &inked.piece {
                    raster::Piece::Text { text, .. } => Some((
                        (mark, piece),
                        text.chars().filter(|c| !c.is_whitespace()).count(),
                    )),
                    _ => None,
                })
        })
        .collect();
    let line = |index: usize| {
        strokes
            .get(index)
            .map(|stroke| (stroke.mark, stroke.piece))
            .filter(|key| letters.contains_key(key))
    };
    let mut lettered = std::collections::BTreeSet::new();
    let mut time = 0.0;

    for op in &motions.ops {
        let on = match op.act {
            motion::Act::Draw { stroke, .. } | motion::Act::Touch { stroke } => line(stroke),
            // Between two strokes of the same line.
            _ => match (op.done.checked_sub(1).and_then(line), line(op.done)) {
                (Some(before), Some(next)) if before == next => Some(next),
                _ => None,
            },
        };

        if let Some(key) = on {
            lettered.insert(key);
            time += op.time;
        }
    }

    let count: usize = lettered.iter().map(|key| letters[key]).sum();

    if time > 0.0 { count as f64 / time } else { 0.0 }
}

/// Draws the pixels `stroke` inks among the first `passed`, a path for each
/// stretch of [`PLOT_PIECE`] pixels: what is drawn on stays the same
/// stretches from frame to frame, so only the stretch the pen is on is
/// repainted.
fn draw_stroke<Renderer: geometry::Renderer>(
    pen: &mut Pen<'_, Renderer>,
    palette: &Palette,
    stroke: &Stroke,
    passed: usize,
) {
    let color = colour(palette, stroke.tone);
    let passed = passed.min(stroke.pixels.len());

    for start in (0..passed).step_by(PLOT_PIECE) {
        let lit: Vec<Point<i32>> = stroke
            .inked(start..(start + PLOT_PIECE).min(passed))
            .collect();

        raster::fill_pixels(pen, &lit, color);
        pen.flush();
    }
}

/// Consecutive ranges of `costs`, each of at least `cost` but the last.
fn buckets(costs: &[usize], cost: usize) -> Vec<Range<usize>> {
    let mut buckets = Vec::new();
    let (mut start, mut spent) = (0, 0);

    for (index, piece) in costs.iter().enumerate() {
        spent += piece;

        if spent >= cost {
            buckets.push(start..index + 1);
            start = index + 1;
            spent = 0;
        }
    }

    if start < costs.len() {
        buckets.push(start..costs.len());
    }

    buckets
}

/// Works out the plot of a sheet, or of a detail, when it is first asked
/// for, and keeps it while that sheet, or detail, shows.
pub struct Kept<For: PartialEq, What = Plot>(std::cell::RefCell<Option<(For, std::rc::Rc<What>)>>);

impl<For: PartialEq, What> Default for Kept<For, What> {
    fn default() -> Self {
        Self(std::cell::RefCell::new(None))
    }
}

impl<For: PartialEq, What> Kept<For, What> {
    /// The plot made `made`, worked out by `plot` if it is not kept.
    pub fn get(&self, made: For, plot: impl FnOnce() -> What) -> std::rc::Rc<What> {
        let mut kept = self.0.borrow_mut();

        match kept.as_ref() {
            Some((key, plot)) if *key == made => plot.clone(),
            _ => {
                let plot = std::rc::Rc::new(plot());
                *kept = Some((made, plot.clone()));
                plot
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::headless::{Output, Studio};
    use crate::machine::Machine;
    use crate::sheet::timeline::{self, Moment, Phase};

    /// Every plot of every sheet on both of the desk's outputs in the pen's
    /// styles, with the style, the output and the subject's name and parts.
    fn plots(each: impl Fn(&PlotStyle, Output, &str, usize, &Plot)) {
        let subjects = crate::subjects::all(&Machine::fixture());
        let theme = quadrille::Theme::TERMINAL;

        for style in [PlotStyle::CAROUSEL, PlotStyle::DRAFTING, PlotStyle::QUICK] {
            for output in [Output::LAPTOP, Output::ULTRAWIDE] {
                let studio = Studio::new(&subjects, output, &theme, "2026-10-07")
                    .expect("A studio")
                    .plotting(style);

                for (index, subject) in subjects.iter().enumerate() {
                    let parts = subject.card().parts.len();

                    each(&style, output, subject.name(), parts, &studio.plot(index));
                }
            }
        }
    }

    /// The share of the plot gone at each frame of it a surface shows at
    /// thirty frames a second, until the pen is put away.
    fn frames(style: &PlotStyle, parts: usize) -> impl Iterator<Item = (f32, f32)> {
        let end = timeline::PLOT_START + style.length - motion::REST;
        let style = *style;

        (0..)
            .map(|frame| frame as f32 / 30.0)
            .take_while(move |local| *local < end)
            .map(
                move |local| match Moment::at(local, parts, style.length).phase {
                    Phase::Plot(share) => (local, share),
                    phase => unreachable!("{phase:?} at {local}"),
                },
            )
    }

    /// The pen is on every frame of the plot, from its start until it is
    /// put away.
    #[test]
    fn the_pen_is_on_every_plot_frame() {
        plots(|style, output, name, parts, plot| {
            for (local, share) in frames(style, parts) {
                assert!(
                    plot.at(share).head.is_some(),
                    "{name} on {}, {}: no pen at {local} s",
                    output.name(),
                    style.name
                );
            }
        });
    }

    /// The plot ends on time: every stroke is drawn and every bucket kept by
    /// the last frame of it, with the pen put away, and not before the last
    /// stretch of it.
    #[test]
    fn the_plot_ends_on_time() {
        plots(|style, output, name, _, plot| {
            let Work::Pen { strokes, .. } = &plot.work else {
                unreachable!("the pen's plot")
            };
            let at = |share: f32| plot.at(share);
            let last = 1.0 - 1.0 / (30.0 * style.length);
            let what = format!("{name} on {}, {}", output.name(), style.name);

            for now in [at(1.0), at(last)] {
                assert_eq!(now.kept, plot.buckets.len(), "{what}");
                assert!(!now.live && now.head.is_none(), "{what}");
                assert_eq!(
                    now.progress,
                    Progress::Pen {
                        done: strokes.len(),
                        drawing: None
                    },
                    "{what}"
                );
            }

            assert!(at(0.9).kept < plot.buckets.len(), "{what}: done early");
        });
    }

    /// How far each plot's moves are sped up or slowed down to fit it, k,
    /// stays in range: within a factor of a few of each style's own pace on
    /// every sheet. The design aims at 0.75 to 1.5; the sheets differ in
    /// their work more than that range allows one length of plot (see
    /// `plot-stats`), which the styles are to settle.
    #[test]
    fn the_plots_pace_stays_in_range() {
        plots(|style, output, name, _, plot| {
            assert!(
                (0.3..=5.0).contains(&plot.stats.k),
                "{name} on {}, {}: k {:.2}",
                output.name(),
                style.name,
                plot.stats.k
            );
            assert!(plot.stats.p > 0.5, "{name}: p {:.2}", plot.stats.p);
        });
    }

    /// A plot is worked out the same every time.
    #[test]
    fn a_plot_is_worked_out_alike_every_time() {
        let subjects = crate::subjects::all(&Machine::fixture());
        let theme = quadrille::Theme::TERMINAL;

        for style in [PlotStyle::CAROUSEL, PlotStyle::DRAFTING, PlotStyle::QUICK] {
            let studio = Studio::new(&subjects, Output::ULTRAWIDE, &theme, "2026-10-07")
                .expect("A studio")
                .plotting(style);

            for index in 0..subjects.len() {
                let (a, b) = (studio.plot(index), studio.plot(index));

                assert!(a.work == b.work && a.buckets == b.buckets, "{}", style.name);
            }
        }
    }

    /// The pixels `plot` has inked with `share` of its time gone.
    fn inked(plot: &Plot, share: f32) -> std::collections::BTreeSet<(i32, i32)> {
        let Work::Pen { strokes, .. } = &plot.work else {
            unreachable!("the pen's plot")
        };
        let Progress::Pen { done, drawing } = plot.at(share).progress else {
            unreachable!("the pen's plot")
        };

        strokes[..done]
            .iter()
            .flat_map(|stroke| stroke.inked(0..stroke.pixels.len()))
            .chain(
                drawing
                    .into_iter()
                    .flat_map(|(index, passed)| strokes[index].inked(0..passed)),
            )
            .map(|pixel| (pixel.x, pixel.y))
            .collect()
    }

    /// A drafting office's sheets of each kind, plotted on the laptop.
    fn drafted(each: impl Fn(&str, usize, &Plot)) {
        let subjects = crate::subjects::all(&Machine::fixture());
        let theme = quadrille::Theme::TERMINAL;
        let studio = Studio::new(&subjects, Output::LAPTOP, &theme, "2026-10-07")
            .expect("A studio")
            .plotting(PlotStyle::DRAFTING);

        for name in ["gears", "engine", "topology"] {
            let index = crate::subjects::find(&subjects, name).expect("A subject");

            each(
                name,
                subjects[index].card().parts.len(),
                &studio.plot(index),
            );
        }
    }

    /// The ink still wet on each frame is what the pen has inked in the
    /// style's moment before it: all it has newly inked, and nothing it has
    /// not inked (a letter's strokes may cross, inking a pixel again).
    #[test]
    fn the_wet_ink_is_what_was_inked_last() {
        let style = PlotStyle::DRAFTING;

        drafted(|name, parts, plot| {
            let mut seen = 0;

            for (local, share) in frames(&style, parts) {
                let then = share - style.wet / style.length;
                let (now, before) = (inked(plot, share), inked(plot, then.max(0.0)));
                let fresh: Vec<&(i32, i32)> = now.difference(&before).collect();
                let wet: std::collections::BTreeSet<(i32, i32)> = plot
                    .wet(share, style.wet)
                    .into_iter()
                    .map(|pixel| (pixel.x, pixel.y))
                    .collect();

                assert!(
                    fresh.iter().all(|pixel| wet.contains(pixel)),
                    "{name} at {local} s"
                );
                assert!(wet.is_subset(&now), "{name} at {local} s");
                seen += usize::from(!wet.is_empty());
            }

            assert!(seen > 100, "{name}: wet on {seen} frames");
        });
    }

    /// A compass is seen only while the pen draws a circle or an arc, its
    /// point at the centre and the pen out on the circle.
    #[test]
    fn the_compass_is_at_the_centre_of_what_the_pen_draws() {
        let style = PlotStyle::DRAFTING;

        drafted(|name, parts, plot| {
            let Work::Pen { strokes, .. } = &plot.work else {
                unreachable!("the pen's plot")
            };
            let mut seen = 0;

            for (local, share) in frames(&style, parts) {
                let now = plot.at(share);
                let Some(centre) = plot.compass(&now) else {
                    continue;
                };
                let (
                    Some(head),
                    Progress::Pen {
                        drawing: Some((index, _)),
                        ..
                    },
                ) = (now.head, now.progress)
                else {
                    panic!("{name} at {local} s: a compass with no pen drawing");
                };
                let reach = |pixel: Point<i32>| {
                    f64::from(pixel.x - centre.x).hypot(f64::from(pixel.y - centre.y))
                };

                assert!(
                    (reach(head.at) - reach(strokes[index].pixels[0])).abs() <= 1.5,
                    "{name} at {local} s"
                );
                seen += 1;
            }

            if name != "topology" {
                assert!(seen > 3, "{name}: a compass on {seen} frames");
            }
        });
    }

    /// The form's cues come in the order of a drafting office's stages, the
    /// annotation before the balloons, and all of them before the pen is
    /// put away.
    #[test]
    fn the_cues_come_as_the_stages_do() {
        let style = PlotStyle::DRAFTING;

        drafted(|name, parts, plot| {
            let cues = &plot.cues;
            let end = f64::from(style.length - motion::REST);

            assert!(cues.drawn > 0.0 && cues.drawn < end, "{name}");
            assert!(cues.views.contains_key(&0), "{name}");
            assert_eq!(cues.balloons.len(), parts, "{name}");

            for &balloon in cues.balloons.values() {
                assert!(
                    cues.annotation.is_none_or(|notes| notes < balloon),
                    "{name}"
                );
                assert!(balloon < cues.drawn, "{name}");
            }

            for &view in cues.views.values() {
                assert!(view <= cues.drawn, "{name}");
            }
        });
    }

    /// Consecutive pieces are kept in buckets of at least the cost asked
    /// for, the last of whatever is left.
    #[test]
    fn buckets_hold_at_least_their_cost() {
        assert_eq!(buckets(&[3, 3, 3, 3, 1], 5), [0..2, 2..4, 4..5]);
        assert_eq!(buckets(&[9, 1], 5), [0..1, 1..2]);
        assert!(buckets(&[], 5).is_empty());
    }
}
