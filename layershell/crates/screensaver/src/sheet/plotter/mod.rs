//! The plotter: a sheet's drawing turned into the pen's work once, and
//! played back by time.
//!
//! [`Plot::new`] works a sheet's plot out on its first frame. The pieces of
//! its marks become the pen's strokes ([`strokes`]), each pixel drawn by
//! the one stroke that owns it in the finished drawing ([`own`]). The
//! strokes are put in a drafting office's order ([`order`]), and the pen's
//! moves along and between them are timed as the draughtsman's hand moves
//! ([`hand`]) and fitted to the plot's length ([`motion`]). Each frame then
//! looks its moment up ([`Plot::at`]): the strokes done, how far into the
//! next the pen is, and where its head is.
//!
//! The plot is drawn in buckets of consecutive strokes, each kept as a
//! drawing of its own once it is done, so a frame draws again only the
//! bucket in progress, a path for each stretch of a stroke, and the
//! renderer's damage is the stretch the pen is on.
pub mod detail;
pub mod hand;
pub mod motion;
pub mod order;
pub mod own;
pub mod pen;
pub mod strokes;

use std::collections::BTreeMap;
use std::ops::Range;
use std::time::{Duration, Instant};

use iced_core::{Point, Rectangle};
use iced_widget::graphics::geometry;
use quadrille::Palette;
use quadrille::draw::Pen;

use crate::draft::Pass;
use crate::draft::raster::{self, Inked, colour};

use motion::Motions;
use order::Meta;
use own::Owners;
use pen::Head;
use strokes::{Form, Stroke};

/// The longest stretch of a stroke, in pixels, drawn as a path of its own,
/// so the pen's damage is the stretch it is on...
const PLOT_PIECE: usize = 96;
/// ...and the ink of each separately kept bucket of strokes.
const PLOT_BUCKET: usize = 1536;

/// The laptop's sheet height, which the hand's lengths and speeds are for.
const REFERENCE: f64 = 533.0;

/// The least radius of a circle or arc a compass draws, in pixels of the
/// laptop's sheet.
const COMPASS: f64 = 12.0;

/// Whether a compass draws a circle or an arc of `radius` pixels on a
/// sheet `height` virtual pixels high: one too big to draw freehand.
pub fn compasses(radius: i32, height: i32) -> bool {
    f64::from(radius) >= COMPASS * scale(height)
}

/// How much the hand's lengths and speeds are scaled on a sheet `height`
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
    /// The pen's strokes, in the order it draws them, and its work drawing
    /// them, fitted to `length` seconds.
    strokes: Vec<Stroke>,
    motions: Motions,
    length: f64,
    /// The ranges of strokes kept as one drawing once plotted.
    buckets: Vec<Range<usize>>,
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

/// What a plot has done at a moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    /// How many buckets are plotted whole: kept from now on.
    pub kept: usize,
    /// Whether the next is plotted in part.
    pub live: bool,
    pub head: Option<Head>,
    /// How many strokes are done, and the one being drawn with how many of
    /// its pixels the pen has passed.
    done: usize,
    drawing: Option<(usize, usize)>,
}

/// What a plot is made of and how its time is spent: what tunes the hand.
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
    /// Dots and strokes of letters set at a touch; and letters lettered a
    /// second, over the time the pen spends lettering.
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
    /// The pen's plot of `marks` on `paper`, in `length` seconds: strokes
    /// inking what they own, in a drafting office's order, timed as the
    /// hand moves.
    pub fn new(marks: &[Marked], paper: Paper, length: f32) -> Self {
        let started = Instant::now();
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
        // pass by pass, then what moves over it. What moves along the
        // drawing's traces (bus traffic, flow, a signal) waits for the run
        // to appear, the machine switching on, and what it would cover is
        // plotted.
        let waits = |marked: &Marked| marked.moving && marked.meta.pass == Pass::Traces;
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
                        strokes::strokes(inked, (mark, piece, ids[mark][piece]), owners, paper.clip)
                    })
            })
            .collect();
        let metas: Vec<Meta> = marks.iter().map(|marked| marked.meta).collect();
        // A diagram grows along what touches.
        let touching = paper.diagram.then(|| {
            let whose: Vec<usize> = std::iter::once(0)
                .chain(
                    ids.iter()
                        .enumerate()
                        .flat_map(|(mark, ids)| ids.iter().map(move |_| mark)),
                )
                .collect();

            owners.touching(&made, |id| whose[id as usize])
        });
        let ordered = order::order(made, &metas, paper.home, touching.as_ref());
        let motions = Motions::plan(
            &ordered.strokes,
            &ordered.gaps,
            paper.home,
            scale(paper.size.1),
            length,
        );
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
                motion::Act::Still { .. } => stats.still += op.time,
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

        let cues = cues(&ordered, &metas, &motions);

        stats.planned = started.elapsed();

        Self {
            cues,
            strokes: ordered.strokes,
            motions,
            length: f64::from(length),
            buckets,
            compasses,
            stats,
        }
    }

    /// Where the plot is with `share` of its time gone.
    pub fn at(&self, share: f32) -> Now {
        let place = self
            .motions
            .at(f64::from(share) * self.length, &self.strokes);
        let kept = self
            .buckets
            .partition_point(|bucket| bucket.end <= place.done);

        Now {
            kept,
            live: kept < self.buckets.len(),
            head: place.head,
            done: place.done,
            drawing: place.drawing,
        }
    }

    /// The pixels inked in the last `seconds` before `share` of the plot's
    /// time is gone: the ink still wet.
    pub fn wet(&self, share: f32, seconds: f32) -> Vec<Point<i32>> {
        let strokes = &self.strokes;
        let time = f64::from(share) * self.length;
        // How far through the strokes the pen is: a stroke, and how many of
        // its pixels it has passed.
        let reached = |time: f64| {
            let place = self.motions.at(time.max(0.0), strokes);

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
        let (index, _) = now.drawing?;

        self.compasses
            .get(self.strokes[index].mark)
            .copied()
            .flatten()
    }

    /// Draws bucket `index` whole.
    pub fn draw_bucket<Renderer: geometry::Renderer>(
        &self,
        pen: &mut Pen<'_, Renderer>,
        palette: &Palette,
        index: usize,
    ) {
        for stroke in &self.strokes[self.buckets[index].clone()] {
            draw_stroke(pen, palette, stroke, stroke.pixels.len());
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

        for stroke in &self.strokes[bucket.start..now.done.max(bucket.start)] {
            draw_stroke(pen, palette, stroke, stroke.pixels.len());
        }

        if let Some((index, passed)) = now.drawing {
            draw_stroke(pen, palette, &self.strokes[index], passed);
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

    /// Every plot of every sheet on both of the desk's outputs, with the
    /// output and the subject's name and parts.
    fn plots(each: impl Fn(Output, &str, usize, &Plot)) {
        let subjects = crate::subjects::all(&Machine::fixture());
        let theme = quadrille::Theme::TERMINAL;

        for output in [Output::LAPTOP, Output::ULTRAWIDE] {
            let studio = Studio::new(&subjects, output, &theme, "2026-10-07").expect("A studio");

            for (index, subject) in subjects.iter().enumerate() {
                let parts = subject.card().parts.len();

                each(output, subject.name(), parts, &studio.plot(index));
            }
        }
    }

    /// The share of `plot`'s time gone at each frame of it a surface shows
    /// at thirty frames a second on a sheet documenting `parts` parts,
    /// until the pen is put away.
    fn frames(plot: &Plot, parts: usize) -> impl Iterator<Item = (f32, f32)> {
        let length = plot.length as f32;
        let end = timeline::PLOT_START + length - motion::REST;

        (0..)
            .map(|frame| frame as f32 / 30.0)
            .take_while(move |local| *local < end)
            .map(move |local| match Moment::at(local, parts, length).phase {
                Phase::Plot(share) => (local, share),
                phase => unreachable!("{phase:?} at {local}"),
            })
    }

    /// The pen is on every frame of the plot, from its start until it is
    /// put away.
    #[test]
    fn the_pen_is_on_every_plot_frame() {
        plots(|output, name, parts, plot| {
            for (local, share) in frames(plot, parts) {
                assert!(
                    plot.at(share).head.is_some(),
                    "{name} on {}: no pen at {local} s",
                    output.name()
                );
            }
        });
    }

    /// The plot ends on time: every stroke is drawn and every bucket kept by
    /// the last frame of it, with the pen put away and its ink dry, and not
    /// before the last stretch of it.
    #[test]
    fn the_plot_ends_on_time() {
        plots(|output, name, _, plot| {
            let at = |share: f32| plot.at(share);
            let last = 1.0 - 1.0 / (30.0 * plot.length as f32);
            let what = format!("{name} on {}", output.name());

            assert!(plot.wet(last, hand::WET).is_empty(), "{what}");

            for now in [at(1.0), at(last)] {
                assert_eq!(now.kept, plot.buckets.len(), "{what}");
                assert!(!now.live && now.head.is_none(), "{what}");
                assert_eq!(
                    (now.done, now.drawing),
                    (plot.strokes.len(), None),
                    "{what}"
                );
            }

            assert!(at(0.9).kept < plot.buckets.len(), "{what}: done early");
        });
    }

    /// How far each plot's moves are sped up or slowed down to fit it, k,
    /// stays in range: within a factor of a few of the hand's own pace on
    /// every sheet.
    #[test]
    fn the_plots_pace_stays_in_range() {
        plots(|output, name, _, plot| {
            assert!(
                (0.3..=5.0).contains(&plot.stats.k),
                "{name} on {}: k {:.2}",
                output.name(),
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
        let studio =
            Studio::new(&subjects, Output::ULTRAWIDE, &theme, "2026-10-07").expect("A studio");

        for (index, subject) in subjects.iter().enumerate() {
            let (a, b) = (studio.plot(index), studio.plot(index));

            assert!(
                a.strokes == b.strokes && a.motions == b.motions && a.buckets == b.buckets,
                "{}",
                subject.name()
            );
        }
    }

    /// The pixels `plot` has inked with `share` of its time gone.
    fn inked(plot: &Plot, share: f32) -> std::collections::BTreeSet<(i32, i32)> {
        let now = plot.at(share);

        plot.strokes[..now.done]
            .iter()
            .flat_map(|stroke| stroke.inked(0..stroke.pixels.len()))
            .chain(
                now.drawing
                    .into_iter()
                    .flat_map(|(index, passed)| plot.strokes[index].inked(0..passed)),
            )
            .map(|pixel| (pixel.x, pixel.y))
            .collect()
    }

    /// Sheets of each kind plotted on the laptop.
    fn sheets(each: impl Fn(&str, usize, &Plot)) {
        let subjects = crate::subjects::all(&Machine::fixture());
        let theme = quadrille::Theme::TERMINAL;
        let studio =
            Studio::new(&subjects, Output::LAPTOP, &theme, "2026-10-07").expect("A studio");

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
    /// moment before it: all it has newly inked, and nothing it has not
    /// inked (a letter's strokes may cross, inking a pixel again).
    #[test]
    fn the_wet_ink_is_what_was_inked_last() {
        sheets(|name, parts, plot| {
            let what = |local: f32| format!("{name} at {local} s");
            let mut seen = 0;

            for (local, share) in frames(plot, parts) {
                let then = share - hand::WET / plot.length as f32;
                let (now, before) = (inked(plot, share), inked(plot, then.max(0.0)));
                let fresh: Vec<&(i32, i32)> = now.difference(&before).collect();
                let wet: std::collections::BTreeSet<(i32, i32)> = plot
                    .wet(share, hand::WET)
                    .into_iter()
                    .map(|pixel| (pixel.x, pixel.y))
                    .collect();

                assert!(
                    fresh.iter().all(|pixel| wet.contains(pixel)),
                    "{}",
                    what(local)
                );
                assert!(wet.is_subset(&now), "{}", what(local));
                seen += usize::from(!wet.is_empty());
            }

            assert!(seen > 100, "{name}: wet on {seen} frames");
        });
    }

    /// A compass is seen only while the pen draws a circle or an arc, its
    /// point at the centre and the pen out on the circle.
    #[test]
    fn the_compass_is_at_the_centre_of_what_the_pen_draws() {
        sheets(|name, parts, plot| {
            let mut seen = 0;

            for (local, share) in frames(plot, parts) {
                let now = plot.at(share);
                let Some(centre) = plot.compass(&now) else {
                    continue;
                };
                let (Some(head), Some((index, _))) = (now.head, now.drawing) else {
                    panic!("{name} at {local} s: a compass with no pen drawing");
                };
                let reach = |pixel: Point<i32>| {
                    f64::from(pixel.x - centre.x).hypot(f64::from(pixel.y - centre.y))
                };

                assert!(
                    (reach(head.at) - reach(plot.strokes[index].pixels[0])).abs() <= 1.5,
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
        sheets(|name, parts, plot| {
            let cues = &plot.cues;
            let end = plot.length - f64::from(motion::REST);

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
