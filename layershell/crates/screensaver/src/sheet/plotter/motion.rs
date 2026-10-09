//! How the pen moves, and when.
//!
//! The pen's work is a list of moves: carried up from one stroke to the
//! next, drawing along a stroke, touching down to set a letter, or still
//! while it settles or pauses between stages, or at the carousel changing
//! pens. Each is timed as the style's carriage would move
//! ([`Motion::Physical`]: blocks of constant acceleration, slowing for
//! corners by GRBL's junction deviation, a short move stepped rather than
//! ramped) or eased as a whole ([`Motion::Eased`]). Then the moves are
//! fitted to the plot's length: the pauses keep their time, or shrink until
//! they take three tenths of the plot, and the moves are played faster or
//! slower to fill the rest. A moment of the plot is then found by a binary
//! search and worked out in closed form.
use glam::DVec2;
use iced_core::Point;
use mint::Point2;

use crate::draft::Tone;

use super::order::{Gap, pen as position};
use super::pen::{Head, Pose};
use super::strokes::{Form, Stroke};
use super::style::{Carousel, Easing, Glyphs, Motion, Physics, PlotStyle};

/// Seconds at the end of a plot the pen is gone, so its last frames show
/// the drawing as the subject's run begins.
pub const REST: f32 = 0.1;
/// Seconds the pen waits at home once it is parked.
pub const HOLD: f64 = 0.2;
/// The most of a plot its pauses may take.
const PAUSES: f64 = 0.3;
/// How far a stroke's path may stray from its pixels' before its corners
/// count: the stair steps of a pixel line are no corners.
const TOLERANCE: f64 = 0.75;

/// A stretch of a move at a constant acceleration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Block {
    /// When it starts, from the start of the move...
    start: f64,
    time: f64,
    /// ...the speed then, its acceleration, how far along the move it
    /// starts and how far it goes.
    speed: f64,
    accel: f64,
    from: f64,
    length: f64,
}

/// How far along its path a move has gone over time.
#[derive(Debug, Clone, PartialEq)]
pub enum Run {
    /// In blocks of constant acceleration, from rest to rest.
    Blocks(Vec<Block>),
    /// Eased in and out (a smoothstep) over its whole length.
    Eased { length: f64, time: f64 },
}

impl Run {
    pub fn time(&self) -> f64 {
        match self {
            Self::Blocks(blocks) => blocks.last().map_or(0.0, |block| block.start + block.time),
            Self::Eased { time, .. } => *time,
        }
    }

    /// How far along it is `time` seconds in.
    pub fn at(&self, time: f64) -> f64 {
        match self {
            Self::Blocks(blocks) => {
                let index = blocks
                    .partition_point(|block| block.start <= time)
                    .saturating_sub(1);

                blocks.get(index).map_or(0.0, |block| {
                    let t = (time - block.start).clamp(0.0, block.time);

                    block.from
                        + (block.speed * t + 0.5 * block.accel * t * t).clamp(0.0, block.length)
                })
            }
            Self::Eased {
                length,
                time: whole,
            } => {
                let u = if *whole > 0.0 {
                    (time / whole).clamp(0.0, 1.0)
                } else {
                    1.0
                };

                length * u * u * (3.0 - 2.0 * u)
            }
        }
    }
}

/// The blocks of a move along `points` from rest to rest, at most `top`
/// fast: each segment at the acceleration `accel` gives for its length,
/// each corner turned as fast as cutting it by `deviation` allows.
fn blocks(points: &[DVec2], top: f64, accel: impl Fn(f64) -> f64, deviation: f64) -> Run {
    let segments: Vec<(f64, DVec2, f64)> = points
        .windows(2)
        .filter_map(|pair| {
            let along = pair[1] - pair[0];
            let length = along.length();

            (length > 1e-9).then(|| (length, along / length, accel(length)))
        })
        .collect();
    let count = segments.len();

    // The speed at each corner, first the most it may turn at...
    let mut speed = vec![0.0; count + 1];

    for index in 1..count {
        let (_, before, a) = segments[index - 1];
        let (_, after, b) = segments[index];

        speed[index] = junction(before, after, a.min(b), deviation, top);
    }

    // ...then as fast as it can be reached from the start, and stopped
    // from by the end.
    for index in 0..count {
        let (length, _, accel) = segments[index];
        speed[index + 1] =
            speed[index + 1].min((speed[index].powi(2) + 2.0 * accel * length).sqrt());
    }
    speed[count] = 0.0;

    for index in (0..count).rev() {
        let (length, _, accel) = segments[index];
        speed[index] = speed[index].min((speed[index + 1].powi(2) + 2.0 * accel * length).sqrt());
    }

    let mut blocks = Vec::new();
    let (mut start, mut from) = (0.0, 0.0);

    for (index, &(length, _, accel)) in segments.iter().enumerate() {
        let (enter, leave) = (speed[index], speed[index + 1]);
        // A triangle's peak, or the cruise if it reaches it.
        let peak = (accel * length + (enter * enter + leave * leave) / 2.0)
            .sqrt()
            .min(top);
        let rising = ((peak * peak - enter * enter) / (2.0 * accel)).max(0.0);
        let falling = ((peak * peak - leave * leave) / (2.0 * accel)).max(0.0);
        let cruise = (length - rising - falling).max(0.0);

        for (time, speed, accel, length) in [
            ((peak - enter) / accel, enter, accel, rising),
            (
                if peak > 0.0 { cruise / peak } else { 0.0 },
                peak,
                0.0,
                cruise,
            ),
            ((peak - leave) / accel, peak, -accel, falling),
        ] {
            if time > 1e-12 {
                blocks.push(Block {
                    start,
                    time,
                    speed,
                    accel,
                    from,
                    length,
                });
                start += time;
                from += length;
            }
        }
    }

    Run::Blocks(blocks)
}

/// The fastest a corner from direction `before` to `after` is turned at
/// `accel`, cutting it by no more than `deviation`.
fn junction(before: DVec2, after: DVec2, accel: f64, deviation: f64, top: f64) -> f64 {
    let cos = -before.dot(after);

    if cos > 1.0 - 1e-9 {
        // Straight back.
        return 0.0;
    }

    let sine = ((1.0 - cos) / 2.0).max(0.0).sqrt();

    if sine > 1.0 - 1e-9 {
        // Straight on.
        return top;
    }

    (accel * deviation * sine / (1.0 - sine)).sqrt().min(top)
}

/// The way along a stroke: its path, simplified to its corners for the
/// motion, and how far along its pixels each point of that is.
#[derive(Debug, Clone, PartialEq)]
pub struct Way {
    /// The path's points kept as corners, by index...
    corners: Vec<usize>,
    /// ...and how far along the simplified path each is.
    along: Vec<f64>,
    /// How far along the pixels each point of the path is.
    steps: Vec<f64>,
    /// How many pixels the stroke has.
    pixels: usize,
}

impl Way {
    /// The way along `stroke`, and the points of its simplified path; a
    /// loop goes round back to where it began.
    fn new(stroke: &Stroke) -> (Self, Vec<DVec2>) {
        let mut path: Vec<Point<i32>> = stroke.pixels.clone();

        if stroke.closed() {
            path.push(stroke.pixels[0]);
        }

        let mut steps = Vec::with_capacity(path.len());
        let mut along = 0.0;

        for (index, pixel) in path.iter().enumerate() {
            if index > 0 {
                along += point(path[index - 1]).distance(point(*pixel));
            }

            steps.push(along);
        }

        let mint: Vec<Point2<i32>> = path.iter().map(|p| Point2 { x: p.x, y: p.y }).collect();
        let mut corners = ramer_douglas_peucker::rdp(&mint, TOLERANCE);

        // A closed path is simplified as one that ends where it starts.
        corners.dedup();

        if corners.last() != Some(&(path.len() - 1)) {
            corners.push(path.len() - 1);
        }

        let points: Vec<DVec2> = corners.iter().map(|&index| point(path[index])).collect();
        let mut along = Vec::with_capacity(points.len());
        let mut length = 0.0;

        for (index, at) in points.iter().enumerate() {
            if index > 0 {
                length += points[index - 1].distance(*at);
            }

            along.push(length);
        }

        (
            Self {
                corners,
                along,
                steps,
                pixels: stroke.pixels.len(),
            },
            points,
        )
    }

    /// How many of the stroke's pixels the pen has passed `distance` along
    /// the simplified path.
    fn passed(&self, distance: f64) -> usize {
        let last = self.along.len() - 1;

        if last == 0 || distance >= self.along[last] {
            return self.pixels;
        }

        let index = self
            .along
            .partition_point(|along| *along <= distance)
            .saturating_sub(1)
            .min(last - 1);
        let span = self.along[index + 1] - self.along[index];
        let share = if span > 0.0 {
            (distance - self.along[index]) / span
        } else {
            0.0
        };
        let (from, to) = (
            self.steps[self.corners[index]],
            self.steps[self.corners[index + 1]],
        );
        let step = from + share * (to - from);

        self.steps
            .partition_point(|along| *along <= step)
            .clamp(1, self.pixels)
    }
}

fn point(pixel: Point<i32>) -> DVec2 {
    DVec2::new(f64::from(pixel.x), f64::from(pixel.y))
}

fn pixel(point: DVec2) -> Point<i32> {
    Point::new(point.x.round() as i32, point.y.round() as i32)
}

/// What the pen does.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    /// Carried up from one point to another.
    Travel { from: DVec2, to: DVec2, run: Run },
    /// Down along a stroke.
    Draw { stroke: usize, run: Run, way: Way },
    /// Down on a stroke it sets at a touch.
    Touch { stroke: usize },
    /// Still: settling, hovering between stages, parked.
    Still { at: Point<i32>, pose: Pose },
    /// At the carousel: turning it `clicks` places, a jolt each, then
    /// taking up the pen in `tone`, or putting the pen away.
    Change {
        at: Point<i32>,
        tone: Option<Tone>,
        clicks: u8,
        click: f64,
    },
}

impl Act {
    /// Whether it is a move, played faster or slower to fit the plot, and
    /// not a pause.
    fn moves(&self) -> bool {
        !matches!(self, Self::Still { .. } | Self::Change { .. })
    }
}

/// One thing the pen does, and when.
#[derive(Debug, Clone, PartialEq)]
pub struct Op {
    /// When it starts and how long it takes, in seconds of the plot.
    pub start: f64,
    pub time: f64,
    /// How many strokes are done by its start; a touch's own with them.
    pub done: usize,
    pub act: Act,
}

/// The pen's work through a plot.
#[derive(Debug, Clone, PartialEq)]
pub struct Motions {
    pub ops: Vec<Op>,
    /// Seconds its moves and its pauses take as planned...
    pub moving: f64,
    pub pausing: f64,
    /// ...how much faster its moves are played to fit the plot, and its
    /// pauses.
    pub k: f64,
    pub p: f64,
    /// Pixels the pen is carried up.
    pub travel: f64,
}

/// Where the pen is at a moment of the plot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// How many strokes are done...
    pub done: usize,
    /// ...and the one being drawn, with how many of its pixels the pen has
    /// passed.
    pub drawing: Option<(usize, usize)>,
    pub head: Option<Head>,
}

impl Motions {
    /// The pen's work drawing `strokes`, each after `gaps`, as `style`
    /// moves, fitted to the style's length but its last [`REST`]; home, its
    /// carousel and where it parks, is `home`. Lengths and speeds are scaled
    /// by `scale`.
    pub fn plan(
        style: &PlotStyle,
        strokes: &[Stroke],
        gaps: &[Gap],
        home: Point<i32>,
        scale: f64,
    ) -> Self {
        let mut plan = Planner::new(
            style,
            scale,
            match strokes.first() {
                Some(first) if !style.from_home => first.start(),
                _ => home,
            },
        );
        let home = point(home);
        let mut holding: Option<Tone> = None;

        for (index, stroke) in strokes.iter().enumerate() {
            let beat = match gaps[index] {
                Gap::Stage if index > 0 => style.beats.stage,
                Gap::Cluster => style.beats.cluster,
                _ => 0.0,
            };

            plan.still(Pose::Up, f64::from(beat));

            if let Some(carousel) = style.carousel
                && holding != Some(stroke.tone)
            {
                let clicks = match holding {
                    Some(tone) => position(tone).abs_diff(position(stroke.tone)),
                    None => position(stroke.tone),
                };

                plan.carry(home);
                plan.change(Some(stroke.tone), clicks, carousel);
                holding = Some(stroke.tone);
            }

            // A letter set whole after a letter of the same line is set
            // where the pen steps to.
            let typed = style.glyphs == Glyphs::Touched && index > 0 && {
                let before = &strokes[index - 1];

                matches!(before.form, Form::Touch(_))
                    && matches!(stroke.form, Form::Touch(_))
                    && (before.mark, before.piece) == (stroke.mark, stroke.piece)
            };

            if typed {
                plan.at = point(stroke.start());
            }

            plan.stroke(index, stroke);
        }

        if let Some(carousel) = style.carousel {
            plan.carry(home);
            plan.change(None, 0, carousel);
        }

        if style.park {
            plan.carry(home);
            plan.still(Pose::Up, HOLD);
        }

        plan.fit(f64::from(style.length - REST))
    }

    /// Where the pen is `time` seconds into the plot, drawing `strokes`.
    pub fn at(&self, time: f64, strokes: &[Stroke]) -> Place {
        let done = Place {
            done: strokes.len(),
            drawing: None,
            head: None,
        };
        let index = self
            .ops
            .partition_point(|op| op.start <= time)
            .saturating_sub(1);
        let Some(op) = self.ops.get(index) else {
            return done;
        };

        if index + 1 == self.ops.len() && time >= op.start + op.time {
            return done;
        }

        // Seconds into the op as planned.
        let into = (time - op.start).clamp(0.0, op.time) * self.k;
        let place = |pose, at| Place {
            done: op.done,
            drawing: None,
            head: Some(Head { at, pose }),
        };

        match &op.act {
            Act::Travel { from, to, run } => {
                let length = from.distance(*to);
                let share = if length > 0.0 {
                    run.at(into) / length
                } else {
                    1.0
                };

                place(Pose::Up, pixel(from.lerp(*to, share)))
            }
            Act::Draw { stroke, run, way } => {
                let passed = way.passed(run.at(into));

                Place {
                    done: op.done,
                    drawing: Some((*stroke, passed)),
                    head: Some(Head {
                        at: strokes[*stroke].pixels[passed - 1],
                        pose: Pose::Down,
                    }),
                }
            }
            Act::Touch { stroke } => place(Pose::Down, strokes[*stroke].start()),
            Act::Still { at, pose } => place(*pose, *at),
            Act::Change {
                at,
                tone,
                clicks,
                click,
            } => {
                // Seconds into the change as planned: a pause's are its
                // own, shrunk to fit.
                let into = (time - op.start).clamp(0.0, op.time) / self.p;
                let clicked = into / click;
                // A pixel's jolt for the first half of each click.
                let jolt = i32::from(clicked < f64::from(*clicks) && clicked.fract() < 0.5);
                let pose = match tone {
                    Some(tone) => Pose::Changing(*tone),
                    None => Pose::Up,
                };

                place(pose, Point::new(at.x + jolt, at.y))
            }
        }
    }
}

/// The pen's work as it is planned, before it is fitted to the plot.
pub struct Planner<'a> {
    style: &'a PlotStyle,
    scale: f64,
    /// What the pen does, how long each takes as planned, and how many
    /// strokes are done by its start.
    acts: Vec<(Act, f64, usize)>,
    /// Where the pen is.
    at: DVec2,
    travel: f64,
}

impl<'a> Planner<'a> {
    /// The work of a pen at `from`, moving as `style` moves, its lengths
    /// and speeds scaled by `scale`.
    pub fn new(style: &'a PlotStyle, scale: f64, from: Point<i32>) -> Self {
        Self {
            style,
            scale,
            acts: Vec::new(),
            at: point(from),
            travel: 0.0,
        }
    }

    fn done(&self) -> usize {
        match self.acts.last() {
            Some((Act::Draw { stroke, .. } | Act::Touch { stroke }, ..)) => stroke + 1,
            Some((_, _, done)) => *done,
            None => 0,
        }
    }

    /// The pen still for `time` seconds, standing as `pose` says.
    pub fn still(&mut self, pose: Pose, time: f64) {
        if time > 0.0 {
            let done = self.done();
            self.acts.push((
                Act::Still {
                    at: pixel(self.at),
                    pose,
                },
                time,
                done,
            ));
        }
    }

    /// Changes pens at the carousel, where the pen is: turns it `clicks`
    /// places to the pen in `tone` and takes it up, or puts the pen away.
    fn change(&mut self, tone: Option<Tone>, clicks: u8, carousel: Carousel) {
        let click = f64::from(carousel.click);
        let done = self.done();

        self.acts.push((
            Act::Change {
                at: pixel(self.at),
                tone,
                clicks,
                click,
            },
            f64::from(carousel.change) + click * f64::from(clicks),
            done,
        ));
    }

    /// Reaches stroke `index`, `stroke`, and draws it: carried up to it
    /// unless it is there, settling after a long move, and for a circle
    /// by way of its centre and back. A pen already at a stroke's start
    /// draws it from there.
    pub fn stroke(&mut self, index: usize, stroke: &Stroke) {
        let mut hop = if self.at != point(stroke.start()) {
            self.carry(point(stroke.start()))
        } else {
            0.0
        };

        // A circle is reached at its centre, and begun from there.
        if let Form::Circle(_) = stroke.form {
            hop = self.carry(point(stroke.pixels[0]));
        }

        if let Some(drop) = self.style.drop
            && hop > f64::from(drop.near) * self.scale
        {
            self.still(Pose::Landing, f64::from(drop.far));
        }

        self.draw(index, stroke);

        if let Form::Circle(centre) = stroke.form {
            self.carry(point(centre));
        }
    }

    /// Carries the pen up to `to`, a pixel.
    pub fn go(&mut self, to: Point<i32>) {
        self.carry(point(to));
    }

    /// Carries the pen up to `to`; returns how far.
    fn carry(&mut self, to: DVec2) -> f64 {
        let from = self.at;
        let length = from.distance(to);

        if length < 0.5 {
            return length;
        }

        let scale = self.scale;
        let run = match self.style.motion {
            Motion::Physical(Physics {
                travel,
                travel_accel,
                step,
                short,
                ..
            }) => blocks(
                &[from, to],
                f64::from(travel) * scale,
                |length| {
                    if length < f64::from(short) * scale {
                        f64::from(step) * scale
                    } else {
                        f64::from(travel_accel) * scale
                    }
                },
                0.0,
            ),
            Motion::Eased(Easing {
                travel,
                least_travel,
                long,
                ..
            }) => {
                let mut time = length / (f64::from(travel) * scale);

                if length > f64::from(long) * scale {
                    time = time.max(f64::from(least_travel));
                }

                Run::Eased { length, time }
            }
            Motion::Reveal => Run::Eased { length, time: 0.0 },
        };
        let time = run.time();
        let done = self.done();

        self.acts.push((Act::Travel { from, to, run }, time, done));
        self.at = to;
        self.travel += length;

        length
    }

    /// Draws stroke `index`.
    fn draw(&mut self, index: usize, stroke: &Stroke) {
        let done = self.done();

        if let Form::Touch(_) = stroke.form {
            // Set as the pen touches down: done with those before it.
            self.acts.push((
                Act::Touch { stroke: index },
                1.0 / f64::from(self.style.lettering),
                index + 1,
            ));
        } else {
            let scale = self.scale;
            let (way, points) = Way::new(stroke);
            let run = match self.style.motion {
                Motion::Physical(Physics {
                    speeds,
                    accel,
                    step,
                    short,
                    deviation,
                    ..
                }) => blocks(
                    &points,
                    f64::from(speeds.of(stroke.tone)) * scale,
                    |length| {
                        if length < f64::from(short) * scale {
                            f64::from(step) * scale
                        } else {
                            f64::from(accel) * scale
                        }
                    },
                    f64::from(deviation) * scale,
                ),
                Motion::Eased(Easing {
                    speed, long, least, ..
                }) => {
                    let length = way.along[way.along.len() - 1];
                    let mut time = length / (f64::from(speed) * scale);

                    if length > f64::from(long) * scale {
                        time = time.max(f64::from(least));
                    }

                    Run::Eased { length, time }
                }
                Motion::Reveal => Run::Eased {
                    length: 0.0,
                    time: 0.0,
                },
            };
            let time = run.time();

            self.acts.push((
                Act::Draw {
                    stroke: index,
                    run,
                    way,
                },
                time,
                done,
            ));
        }

        // A circle ends where it began, and the pen goes back to its centre
        // from there.
        self.at = point(match stroke.form {
            Form::Circle(_) => stroke.pixels[0],
            _ => stroke.end(),
        });
    }

    /// The work fitted to `length` seconds: pauses as planned, or shrunk
    /// to three tenths of it, and the moves played faster or slower to fill
    /// the rest.
    pub fn fit(self, length: f64) -> Motions {
        let (moving, pausing) =
            self.acts
                .iter()
                .fold((0.0, 0.0), |(moving, pausing), (act, time, _)| {
                    if act.moves() {
                        (moving + time, pausing)
                    } else {
                        (moving, pausing + time)
                    }
                });
        let p = if pausing > 0.0 {
            (PAUSES * length / pausing).min(1.0)
        } else {
            1.0
        };
        let k = if moving > 0.0 {
            moving / (length - p * pausing)
        } else {
            1.0
        };
        let mut start = 0.0;
        let ops = self
            .acts
            .into_iter()
            .map(|(act, time, done)| {
                let time = if act.moves() { time / k } else { time * p };
                let op = Op {
                    start,
                    time,
                    done,
                    act,
                };

                start += time;
                op
            })
            .collect();

        Motions {
            ops,
            moving,
            pausing,
            k,
            p,
            travel: self.travel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A long straight move ramps up to its top speed and down again, and
    /// takes as long as that does; a short one never reaches it.
    #[test]
    fn a_move_ramps_up_and_down() {
        let run = blocks(
            &[DVec2::ZERO, DVec2::new(1000.0, 0.0)],
            2000.0,
            |_| 16_000.0,
            3.0,
        );
        // 2000 / 16000 s to reach top speed over 125 px, each way.
        let expected = 2.0 * 0.125 + (1000.0 - 250.0) / 2000.0;

        assert!((run.time() - expected).abs() < 1e-9, "{}", run.time());
        assert!((run.at(run.time()) - 1000.0).abs() < 1e-9);
        assert!((run.at(0.125) - 125.0).abs() < 1e-9);

        let short = blocks(
            &[DVec2::ZERO, DVec2::new(10.0, 0.0)],
            2000.0,
            |_| 16_000.0,
            3.0,
        );
        let Run::Blocks(blocks) = &short else {
            unreachable!()
        };

        assert_eq!(blocks.len(), 2);
        assert!((short.at(short.time()) - 10.0).abs() < 1e-9);
    }

    /// A corner is turned slower the sharper it is, and a turn straight
    /// back stops the pen.
    #[test]
    fn a_sharp_corner_slows_the_pen() {
        let east = DVec2::X;
        // The ink pen's speed, which the design's figures are for.
        let top = 1600.0;
        let turn = |degrees: f64| {
            junction(
                east,
                DVec2::from_angle(degrees.to_radians()),
                16_000.0,
                3.0,
                top,
            )
        };

        assert!(turn(0.0) >= top);
        assert!(turn(5.0) >= top);
        // The design's figures: a right angle keeps a fifth of the speed,
        // thirty degrees three quarters.
        assert!(
            (turn(90.0) / top - 0.21).abs() < 0.02,
            "{}",
            turn(90.0) / top
        );
        assert!(
            (turn(30.0) / top - 0.73).abs() < 0.03,
            "{}",
            turn(30.0) / top
        );
        assert!(turn(150.0) < turn(90.0));
        assert_eq!(turn(180.0), 0.0);
    }

    /// An eased run goes the whole way, slow at its ends and fastest in
    /// its middle.
    #[test]
    fn an_eased_run_eases_in_and_out() {
        let run = Run::Eased {
            length: 100.0,
            time: 1.0,
        };

        assert_eq!(run.at(0.0), 0.0);
        assert_eq!(run.at(1.0), 100.0);
        assert_eq!(run.at(0.5), 50.0);
        assert!(run.at(0.1) < 10.0);
    }

    /// A circle is drawn as a plotter's circle instruction draws it: the
    /// pen carried to its centre, out to its start, round, and back to the
    /// centre.
    #[test]
    fn a_circle_is_drawn_from_its_centre_and_back() {
        let centre = Point::new(100, 100);
        let pixels = crate::draft::raster::ordered_circle(centre, 40);
        let circle = super::super::strokes::centred(
            Stroke {
                lit: vec![true; pixels.len()],
                pixels,
                tone: Tone::Ink,
                form: Form::Loop,
                mark: 0,
                piece: 0,
            },
            centre,
        );
        let style = PlotStyle {
            carousel: None,
            ..PlotStyle::CAROUSEL
        };
        let motions = Motions::plan(&style, &[circle], &[Gap::Stage], Point::new(0, 0), 1.0);
        let acts: Vec<String> = motions
            .ops
            .iter()
            .map(|op| match &op.act {
                Act::Travel { to, .. } => format!("to {},{}", to.x, to.y),
                Act::Draw { .. } => "draw".into(),
                Act::Still { pose, .. } => format!("{pose:?}"),
                act => format!("{act:?}"),
            })
            .collect();

        assert_eq!(
            acts,
            [
                "to 100,100",
                "to 140,100",
                "Landing",
                "draw",
                "to 100,100",
                "to 0,0",
                "Up"
            ]
        );
    }

    /// At the carousel the head jolts a pixel for each place the carousel
    /// turns, and shows the pen it takes up.
    #[test]
    fn the_carousel_clicks_round_to_the_next_pen() {
        let home = Point::new(5, 300);
        let strokes: Vec<Stroke> = [Tone::Faint, Tone::Ink]
            .into_iter()
            .enumerate()
            .map(|(mark, tone)| Stroke {
                pixels: vec![
                    Point::new(100, 100 + mark as i32 * 50),
                    Point::new(160, 100),
                ],
                lit: vec![true; 2],
                tone,
                form: Form::Line,
                mark,
                piece: 0,
            })
            .collect();

        let motions = Motions::plan(
            &PlotStyle::CAROUSEL,
            &strokes,
            &[Gap::Stage, Gap::Stage],
            home,
            1.0,
        );
        let change = motions
            .ops
            .iter()
            .find(|op| {
                matches!(
                    op.act,
                    Act::Change {
                        tone: Some(Tone::Ink),
                        ..
                    }
                )
            })
            .expect("A change to the ink pen");
        let Act::Change { clicks, click, .. } = change.act else {
            unreachable!()
        };
        let at = |planned: f64| {
            motions
                .at(change.start + planned * motions.p, &strokes)
                .head
        };

        // From the faint pen three places round to the ink.
        assert_eq!(clicks, 3);

        for (planned, x) in [
            (0.25 * click, 6),
            (0.75 * click, 5),
            (2.25 * click, 6),
            (3.5 * click, 5),
        ] {
            assert_eq!(
                at(planned),
                Some(Head {
                    at: Point::new(x, 300),
                    pose: Pose::Changing(Tone::Ink)
                }),
                "{planned}"
            );
        }
    }
}
