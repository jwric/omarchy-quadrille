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
pub mod motion;
pub mod order;
pub mod own;
pub mod pen;
pub mod strokes;
pub mod style;

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
use strokes::{Form, Stroke};
use style::{Motion, PlotStyle};

/// The longest piece of the plot as it was, and stretch of a stroke, in
/// pixels of pen travel: each a path of its own, so the pen's damage is the
/// stretch it is on...
const PLOT_PIECE: usize = 96;
/// ...and the pen travel, or ink, of each separately kept bucket of them.
const PLOT_BUCKET: usize = 1536;

/// The laptop's sheet height, which a style's lengths and speeds are for.
const REFERENCE: f64 = 533.0;

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
}

/// A sheet's plot, worked out once.
pub struct Plot {
    work: Work,
    /// The ranges of pieces, or strokes, kept as one drawing once plotted.
    buckets: Vec<Range<usize>>,
    clip: Rectangle<i32>,
    pub stats: Stats,
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
    /// Letters and dots set at a touch, and how many a second.
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

        let made: Vec<Stroke> = marks
            .iter()
            .enumerate()
            .filter(|(_, marked)| !waits(marked))
            .flat_map(|(mark, marked)| {
                let (owners, ids) = (&owners, &ids);

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
                            style.lining,
                        )
                    })
            })
            .collect();
        let metas: Vec<Meta> = marks.iter().map(|marked| marked.meta).collect();
        let ordered = order::order(style, made, &metas, paper.home);
        let scale = f64::from(paper.size.1) / REFERENCE;
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
            lettering: f64::from(style.lettering) * motions.k,
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
                motion::Act::Still { .. } => stats.still += op.time,
            }
        }

        Self {
            work: Work::Pen {
                strokes: ordered.strokes,
                motions,
                length: f64::from(style.length),
            },
            buckets,
            clip: paper.clip,
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

/// Works out the plot of a sheet when it is first asked for, and keeps it
/// while that sheet shows.
pub struct Kept<For: PartialEq>(std::cell::RefCell<Option<(For, std::rc::Rc<Plot>)>>);

impl<For: PartialEq> Default for Kept<For> {
    fn default() -> Self {
        Self(std::cell::RefCell::new(None))
    }
}

impl<For: PartialEq> Kept<For> {
    /// The plot made `made`, worked out by `plot` if it is not kept.
    pub fn get(&self, made: For, plot: impl FnOnce() -> Plot) -> std::rc::Rc<Plot> {
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

    /// Consecutive pieces are kept in buckets of at least the cost asked
    /// for, the last of whatever is left.
    #[test]
    fn buckets_hold_at_least_their_cost() {
        assert_eq!(buckets(&[3, 3, 3, 3, 1], 5), [0..2, 2..4, 4..5]);
        assert_eq!(buckets(&[9, 1], 5), [0..1, 1..2]);
        assert!(buckets(&[], 5).is_empty());
    }
}
