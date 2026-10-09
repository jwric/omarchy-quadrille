//! How the pen moves, and when.
//!
//! The pen's work is a list of moves: carried up from one stroke to the
//! next, drawing along a stroke, touching down to set a dot, or still while
//! it settles or pauses between stages. Each is timed as the draughtsman's
//! [hand](super::hand) moves: in blocks of constant acceleration, slowing
//! for corners by GRBL's junction deviation, a short move stepped rather
//! than ramped. Then the moves are fitted to the plot's length: the pauses
//! keep their time, or shrink until they take three tenths of the plot, and
//! the moves are played faster or slower to fill the rest. A moment of the
//! plot is then found by a binary search and worked out in closed form.
use glam::DVec2;
use iced_core::Point;
use mint::Point2;

use super::hand;
use super::order::Gap;
use super::pen::{Head, Pose};
use super::strokes::{Form, Stroke};

/// Seconds at the end of a plot the pen is gone, so its last frames show
/// the drawing as the subject's run begins: long enough for the ink it
/// leaves wet to dry, with a frame to spare.
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

/// How far along its path a move has gone over time: in blocks of
/// constant acceleration, from rest to rest.
#[derive(Debug, Clone, PartialEq)]
pub struct Run(Vec<Block>);

impl Run {
    pub fn time(&self) -> f64 {
        self.0.last().map_or(0.0, |block| block.start + block.time)
    }

    /// How far along it is `time` seconds in.
    pub fn at(&self, time: f64) -> f64 {
        let index = self
            .0
            .partition_point(|block| block.start <= time)
            .saturating_sub(1);

        self.0.get(index).map_or(0.0, |block| {
            let t = (time - block.start).clamp(0.0, block.time);

            block.from + (block.speed * t + 0.5 * block.accel * t * t).clamp(0.0, block.length)
        })
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

    Run(blocks)
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

/// The pixel nearest `point`.
pub fn pixel(point: DVec2) -> Point<i32> {
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
}

impl Act {
    /// Whether it is a move, played faster or slower to fit the plot, and
    /// not a pause.
    fn moves(&self) -> bool {
        !matches!(self, Self::Still { .. })
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
    /// The pen's work drawing `strokes`, each after `gaps`, from home at
    /// `home` and back, fitted to `length` seconds but its [`REST`].
    /// Lengths and speeds are scaled by `scale`.
    pub fn plan(
        strokes: &[Stroke],
        gaps: &[Gap],
        home: Point<i32>,
        scale: f64,
        length: f32,
    ) -> Self {
        let mut plan = Planner::new(scale, home);

        for (index, stroke) in strokes.iter().enumerate() {
            let beat = match gaps[index] {
                Gap::Stage if index > 0 => hand::STAGE_BEAT,
                Gap::Cluster => hand::CLUSTER_BEAT,
                _ => 0.0,
            };

            plan.still(Pose::Up, f64::from(beat));
            plan.stroke(index, stroke);
        }

        plan.go(home);
        plan.still(Pose::Up, HOLD);
        plan.fit(f64::from(length - REST))
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
        }
    }
}

/// The pen's work as it is planned, before it is fitted to the plot.
pub struct Planner {
    scale: f64,
    /// What the pen does, how long each takes as planned, and how many
    /// strokes are done by its start.
    acts: Vec<(Act, f64, usize)>,
    /// Where the pen is.
    at: DVec2,
    travel: f64,
}

impl Planner {
    /// The work of a pen at `from`, its lengths and speeds scaled by
    /// `scale`.
    pub fn new(scale: f64, from: Point<i32>) -> Self {
        Self {
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

    /// Reaches stroke `index`, `stroke`, and draws it: carried up to it
    /// unless it is there, settling after a long move. A pen already at a
    /// stroke's start draws it from there.
    pub fn stroke(&mut self, index: usize, stroke: &Stroke) {
        let hop = if self.at != point(stroke.start()) {
            self.carry(point(stroke.start()))
        } else {
            0.0
        };

        if hop > f64::from(hand::NEAR) * self.scale {
            self.still(Pose::Landing, f64::from(hand::SETTLING));
        }

        self.draw(index, stroke);
    }

    /// Carries the pen up to `to`, a pixel.
    pub fn go(&mut self, to: Point<i32>) {
        self.carry(point(to));
    }

    /// The acceleration along a line `length` pixels long, at `accel`
    /// unless it is short enough to be stepped.
    fn accel(&self, length: f64, accel: f32) -> f64 {
        if length < f64::from(hand::SHORT) * self.scale {
            f64::from(hand::STEP) * self.scale
        } else {
            f64::from(accel) * self.scale
        }
    }

    /// Carries the pen up to `to`; returns how far.
    fn carry(&mut self, to: DVec2) -> f64 {
        let from = self.at;
        let length = from.distance(to);

        if length < 0.5 {
            return length;
        }

        let run = blocks(
            &[from, to],
            f64::from(hand::TRAVEL) * self.scale,
            |length| self.accel(length, hand::TRAVEL_ACCEL),
            0.0,
        );
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
                1.0 / f64::from(hand::TOUCHES),
                index + 1,
            ));
        } else {
            let (way, points) = Way::new(stroke);
            let run = blocks(
                &points,
                f64::from(hand::speed(stroke.tone)) * self.scale,
                |length| self.accel(length, hand::ACCEL),
                f64::from(hand::DEVIATION) * self.scale,
            );
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

        self.at = point(stroke.end());
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
        assert_eq!(short.0.len(), 2);
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
}
