//! Where annotations go when a subject leaves it to the sheet.
//!
//! A subject records a balloon or a note with [`Placement::Auto`], and a
//! [`Plan`] decides where it goes in the main view, once for the subject on
//! an output, from the subject drawn at moments across its run. Each
//! annotation either
//!
//! - lines up along an edge of the drawing, in a column beside it or, for a
//!   balloon, a row above or below it, fanned along it in the order of what
//!   it points at so that leaders rise and fall without crossing
//!   (quadrille's `fan`); or
//! - sits just off what it points at, riding it if it moves, at the offset
//!   that keeps clearest of the drawing wherever the part goes;
//!
//! and the ways of choosing between these are weighed for the cheapest.
//! Costs are counted on the pixel grid: the drawing an annotation would
//! hide, the lines its leader would cross, how far it reaches, how it meets
//! the others. One that does not fit wholly inside the view is left out,
//! not cut.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ops::RangeInclusive;
use std::rc::Rc;

use iced_core::{Point, Rectangle};

use super::raster::{self, Inked, LETTERING, Piece, Projection, rect};
use super::{Draft, Ink, Mark, Pass, Placement};

/// How far annotations line up clear of the drawing.
const CLEAR: i32 = 10;
/// How far annotations keep from the edges of the view.
const MARGIN: i32 = 2;
/// The pixels between neighbours along an edge.
const GAP: i32 = 4;

/// What it costs to hide a pixel of the drawing under an annotation...
const HIDES_DRAWING: f32 = 20.0;
/// ...or of other lettering, which no one could then read.
const HIDES_LETTERING: f32 = 400.0;
/// A leader crossing a pixel of the drawing...
const CROSSES_DRAWING: f32 = 1.0;
/// ...or of lettering.
const CROSSES_LETTERING: f32 = 400.0;
/// Two leaders meeting, a pixel at a time.
const CROSSES_LEADER: f32 = 200.0;
/// A pixel of leader: shorter reads better.
const REACH: f32 = 0.4;
/// An annotation not lined up with the others: a drawing reads tidier with
/// them lined up, when that costs little more.
const LOOSE: f32 = 40.0;
/// An annotation left out for want of room.
const LEFT_OUT: f32 = 20_000.0;

/// What covers a pixel of the view: the drawing, or lettering.
const DRAWN: u8 = 1;
const LETTERED: u8 = 2;

/// Where each automatically placed annotation of a main view goes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plan {
    /// The view it places annotations in: `None` for the front view.
    view: Option<usize>,
    spots: HashMap<Id, Spot>,
}

/// Where one annotation goes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Spot {
    /// Its balloon's centre or its note's elbow is at this pixel, wherever
    /// its target is.
    At(Point<i32>),
    /// This far from its target, wherever that goes.
    Off(i32, i32),
}

/// An annotation, as the same one is recorded at every moment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Id {
    Balloon(usize),
    Note(String),
}

impl Id {
    /// The annotation `mark` is, if it is placed automatically.
    fn of(mark: &Mark) -> Option<Self> {
        match &mark.ink {
            Ink::Balloon {
                item,
                offset: Placement::Auto,
                ..
            } => Some(Self::Balloon(*item)),
            Ink::Note {
                text,
                elbow: Placement::Auto,
                ..
            } => Some(Self::Note(text.clone())),
            _ => None,
        }
    }
}

/// Where one annotation goes in a trial of the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    /// In the column or row along this edge of the drawing.
    Beside(Edge),
    /// Just off its target, riding it if it moves.
    Near,
}

/// An edge of the drawing, which annotations line up along.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

impl Edge {
    const ALL: [Self; 4] = [Self::Left, Self::Right, Self::Top, Self::Bottom];

    /// Away from the drawing, along x for a column and y for a row.
    fn sign(self) -> i32 {
        match self {
            Self::Left | Self::Top => -1,
            Self::Right | Self::Bottom => 1,
        }
    }

    /// Whether annotations along it make a row, not a column.
    fn row(self) -> bool {
        matches!(self, Self::Top | Self::Bottom)
    }

    /// A point `distance` out from the drawing across it.
    fn out(self, distance: i32) -> (i32, i32) {
        if self.row() {
            (0, self.sign() * distance)
        } else {
            (self.sign() * distance, 0)
        }
    }
}

/// When there are more ways of placing a view's annotations than this, each
/// is moved in turn to wherever is cheapest, from the likeliest start, until
/// none is better moved.
const TRIALS: usize = 1024;

impl Plan {
    /// Places the automatic annotations of `view` (`None`: the front view),
    /// from `samples` of the subject drawn across its run, the view drawn
    /// through `projection` inside `clip`.
    pub fn new(
        samples: &[Draft],
        projection: &Projection,
        clip: Rectangle<i32>,
        view: Option<usize>,
    ) -> Self {
        let search = Search::new(samples, projection, clip, view);
        let options = search.options();
        let trials = options
            .iter()
            .try_fold(1_usize, |trials, options| trials.checked_mul(options.len()))
            .filter(|trials| *trials <= TRIALS);

        let choices = match trials {
            Some(trials) => (0..trials)
                .map(|trial| {
                    let mut rest = trial;

                    options
                        .iter()
                        .map(|options| {
                            let choice = options[rest % options.len()];
                            rest /= options.len();
                            choice
                        })
                        .collect::<Vec<_>>()
                })
                .map(|choices| (search.cost(&choices), choices))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, choices)| choices)
                .unwrap_or_default(),
            None => search.descend(&options),
        };

        let spots = search
            .arrange(&choices)
            .into_iter()
            .zip(&search.slots)
            .filter_map(|(spot, slot)| Some((slot.id.clone(), spot?)))
            .collect();

        Self { view, spots }
    }

    /// Places the automatic annotations among `marks`, drawn through
    /// `projection`; one the plan left out stays unplaced, and undrawn.
    ///
    /// A moving one that would cover other lettering at this moment, run its
    /// leader through it or have another's run through it, is left out for
    /// the moment instead.
    pub fn apply(&self, marks: &mut [Mark], projection: &Projection) {
        let mut moving = Vec::new();

        for (index, mark) in marks.iter_mut().enumerate() {
            let Some(spot) = Id::of(mark)
                .filter(|_| mark.view == self.view)
                .and_then(|id| self.spots.get(&id))
            else {
                continue;
            };

            let offset = spot.offset(projection.px(target(mark)));
            place(mark, Placement::Offset(offset.0, offset.1));

            if mark.moving {
                moving.push(index);
            }
        }

        for index in moving {
            let footprint = Footprint::of(&marks[index], projection);
            let covers = marks.iter().enumerate().any(|(other, mark)| {
                if other == index || !mark.shown_in(self.view) || mark.pass() < Pass::Annotation {
                    return false;
                }

                // Either way: its own over the other's lettering, or the
                // other's leader through its own.
                let other = Footprint::of(mark, projection);

                other.boxes.iter().any(|area| footprint.meets(*area))
                    || footprint.boxes.iter().any(|area| other.meets(*area))
            });

            if covers {
                place(&mut marks[index], Placement::Auto);
            }
        }
    }

    /// Whether the plan placed the annotation `mark` is.
    #[cfg(test)]
    pub fn places(&self, mark: &Mark) -> bool {
        Id::of(mark).is_some_and(|id| self.spots.contains_key(&id))
    }
}

impl Spot {
    /// Its offset from a target at `target`.
    fn offset(self, target: Point<i32>) -> (i32, i32) {
        match self {
            Self::At(at) => (at.x - target.x, at.y - target.y),
            Self::Off(x, y) => (x, y),
        }
    }

    fn key(self) -> (bool, i32, i32) {
        match self {
            Self::At(at) => (false, at.x, at.y),
            Self::Off(x, y) => (true, x, y),
        }
    }
}

/// The search for a plan: the view, its annotations, and what placing each
/// of them somewhere has been found to cost.
struct Search<'a> {
    slots: Vec<Slot>,
    ground: Ground,
    projection: &'a Projection,
    /// The offset each annotation would sit at just off its target, on its
    /// own.
    near: Vec<Option<(i32, i32)>>,
    /// How each annotation sits beside each edge.
    reach: Vec<[Reach; 4]>,
    /// Each annotation placed at a spot, and what that costs on the ground,
    /// or `None` if it is not wholly inside the view there.
    placed: RefCell<HashMap<Trial, Rc<Option<Costed>>>>,
    /// What two annotations at two spots cost drawn together.
    clashes: RefCell<HashMap<[Trial; 2], f32>>,
}

/// An annotation, by its index, tried at a spot.
type Trial = (usize, (bool, i32, i32));

/// An annotation placed, and what that costs on the ground.
type Costed = (Placed, f32);

impl<'a> Search<'a> {
    fn new(
        samples: &[Draft],
        projection: &'a Projection,
        clip: Rectangle<i32>,
        view: Option<usize>,
    ) -> Self {
        let slots = Slot::all(samples, view);
        let reach = slots
            .iter()
            .map(|slot| Edge::ALL.map(|edge| slot.reach(edge, projection)))
            .collect();
        let mut search = Self {
            slots,
            ground: Ground::new(samples, projection, clip, view),
            projection,
            near: Vec::new(),
            reach,
            placed: RefCell::default(),
            clashes: RefCell::default(),
        };

        search.near = (0..search.slots.len()).map(|i| search.near(i)).collect();
        search
    }

    /// Where each annotation may go: beside any edge (a note only beside
    /// one at the side, where its shelf runs out), or just off its target.
    fn options(&self) -> Vec<Vec<Choice>> {
        self.slots
            .iter()
            .zip(&self.near)
            .map(|(slot, near)| {
                let note = matches!(slot.id, Id::Note(_));
                let mut options: Vec<Choice> = Edge::ALL
                    .into_iter()
                    .filter(|edge| !(note && edge.row()))
                    .map(Choice::Beside)
                    .collect();

                options.extend(near.map(|_| Choice::Near));
                options
            })
            .collect()
    }

    /// Annotation `i` at `spot`, and what that costs on the ground.
    fn placed(&self, i: usize, spot: Spot) -> Rc<Option<Costed>> {
        self.placed
            .borrow_mut()
            .entry((i, spot.key()))
            .or_insert_with(|| {
                let placed = self.slots[i].placed(spot, self.projection);

                Rc::new(placed.inside(self.ground.clip).then(|| {
                    let cost = placed.cost(&self.ground);
                    (placed, cost)
                }))
            })
            .clone()
    }

    /// Where each annotation goes for `choices`: lined up beside the
    /// drawing, or just off its target; `None` for one that does not fit
    /// inside the view.
    fn arrange(&self, choices: &[Choice]) -> Vec<Option<Spot>> {
        let clip = self.ground.clip;
        let drawing = self.ground.outline.unwrap_or(clip);
        let mut spots: Vec<Option<Spot>> = choices
            .iter()
            .zip(&self.near)
            .map(|(choice, near)| match choice {
                Choice::Near => near.map(|(x, y)| Spot::Off(x, y)),
                Choice::Beside(_) => None,
            })
            .collect();

        for edge in Edge::ALL {
            let members: Vec<usize> = (0..self.slots.len())
                .filter(|&i| choices[i] == Choice::Beside(edge))
                .collect();

            if members.is_empty() {
                continue;
            }

            let reach: Vec<Reach> = members
                .iter()
                .map(|&i| self.reach[i][edge as usize])
                .collect();
            let widest = reach
                .iter()
                .map(|reach| reach.out + reach.past)
                .max()
                .unwrap_or(0);

            // Where they line up: just clear of the drawing, moved in as
            // far as they must to stay inside the view.
            let (near, low, high) = if edge.row() {
                (
                    if edge == Edge::Top {
                        drawing.y
                    } else {
                        drawing.y + drawing.height - 1
                    },
                    clip.y,
                    clip.y + clip.height - 1,
                )
            } else {
                (
                    if edge == Edge::Left {
                        drawing.x
                    } else {
                        drawing.x + drawing.width - 1
                    },
                    clip.x,
                    clip.x + clip.width - 1,
                )
            };
            let line = match edge.sign() {
                1 => (near + CLEAR).min(high - MARGIN - widest),
                _ => (near - CLEAR).max(low + MARGIN + widest),
            };

            // Along a row, x and y trade places.
            let flip = |point: Point<i32>| {
                if edge.row() {
                    Point::new(point.y, point.x)
                } else {
                    point
                }
            };
            let (along, pivot) = if edge.row() {
                (
                    clip.x + MARGIN..=clip.x + clip.width - 1 - MARGIN,
                    drawing.x + drawing.width / 2,
                )
            } else {
                (
                    clip.y + MARGIN..=clip.y + clip.height - 1 - MARGIN,
                    drawing.y + drawing.height / 2,
                )
            };
            let items: Vec<Item> = members
                .iter()
                .zip(&reach)
                .map(|(&i, reach)| Item {
                    target: flip(self.slots[i].target(self.projection)),
                    above: reach.before,
                    below: reach.after,
                })
                .collect();

            for ((&i, reach), at) in members
                .iter()
                .zip(&reach)
                .zip(fan(&items, line, along, pivot))
            {
                let spot = Spot::At(flip(Point::new(line + edge.sign() * reach.out, at)));

                spots[i] = self.placed(i, spot).is_some().then_some(spot);
            }
        }

        spots
    }

    /// What the annotations cost placed by `choices`.
    fn cost(&self, choices: &[Choice]) -> f32 {
        let spots = self.arrange(choices);
        let mut cost = 0.0;

        for (i, spot) in spots.iter().enumerate() {
            let Some(spot) = spot else {
                cost += LEFT_OUT;
                continue;
            };
            let placed = self.placed(i, *spot);
            let Some((placed, own)) = placed.as_ref() else {
                cost += LEFT_OUT;
                continue;
            };

            cost += own;

            if choices[i] == Choice::Near {
                cost += LOOSE;
            }

            for (j, other) in spots.iter().enumerate().skip(i + 1) {
                let Some(other) = other else {
                    continue;
                };
                let key = [(i, spot.key()), (j, other.key())];

                if let Some(clash) = self.clashes.borrow().get(&key) {
                    cost += clash;
                    continue;
                }

                let clash = match self.placed(j, *other).as_ref() {
                    Some((other, _)) => placed.clash(other),
                    None => 0.0,
                };

                self.clashes.borrow_mut().insert(key, clash);
                cost += clash;
            }
        }

        cost
    }

    /// The likeliest choices: each annotation beside the edge of the
    /// drawing nearest what it points at, or just off it when that costs less
    /// on its own.
    fn start(&self, options: &[Vec<Choice>]) -> Vec<Choice> {
        let drawing = self.ground.outline.unwrap_or(self.ground.clip);

        (0..self.slots.len())
            .map(|i| {
                let target = self.slots[i].target(self.projection);
                let distance = |edge: Edge| match edge {
                    Edge::Left => target.x - drawing.x,
                    Edge::Right => drawing.x + drawing.width - target.x,
                    Edge::Top => target.y - drawing.y,
                    Edge::Bottom => drawing.y + drawing.height - target.y,
                };
                let nearest = options[i]
                    .iter()
                    .filter_map(|choice| match choice {
                        Choice::Beside(edge) => Some(*edge),
                        Choice::Near => None,
                    })
                    .min_by_key(|edge| distance(*edge))
                    .map(Choice::Beside);
                let alone = |choice: Choice| {
                    let mut choices: Vec<Choice> =
                        options.iter().map(|options| options[0]).collect();
                    choices[i] = choice;
                    self.cost(&choices)
                };

                match (nearest, self.near[i]) {
                    (Some(nearest), Some(_)) if alone(Choice::Near) < alone(nearest) => {
                        Choice::Near
                    }
                    (Some(nearest), _) => nearest,
                    (None, _) => Choice::Near,
                }
            })
            .collect()
    }

    /// From the likeliest start, moves each annotation in turn to wherever
    /// costs least, until none is better moved.
    fn descend(&self, options: &[Vec<Choice>]) -> Vec<Choice> {
        let mut choices = self.start(options);
        let mut least = self.cost(&choices);

        for _ in 0..8 {
            let mut moved = false;

            for i in 0..choices.len() {
                for &option in &options[i] {
                    let mut trial = choices.clone();
                    trial[i] = option;

                    let cost = self.cost(&trial);

                    if cost < least {
                        least = cost;
                        choices = trial;
                        moved = true;
                    }
                }
            }

            if !moved {
                break;
            }
        }

        choices
    }

    /// The offset annotation `i` would sit at just off its target, on its
    /// own: the one that costs least (over the samples, when it moves)
    /// among those that keep it inside the view.
    fn near(&self, i: usize) -> Option<(i32, i32)> {
        let mut best: Option<(f32, (i32, i32))> = None;

        for distance in [24, 32, 42, 54, 66] {
            for step in 0..16 {
                let angle = step as f32 * std::f32::consts::TAU / 16.0;
                let offset = (
                    (distance as f32 * angle.cos()).round() as i32,
                    (distance as f32 * angle.sin()).round() as i32,
                );

                if let Some((_, cost)) = &*self.placed(i, Spot::Off(offset.0, offset.1))
                    && best.is_none_or(|(least, _)| *cost < least)
                {
                    best = Some((*cost, offset));
                }
            }
        }

        best.map(|(_, offset)| offset)
    }
}

/// One automatically placed annotation, as each sample recorded it.
struct Slot {
    id: Id,
    /// Its mark in each sample, if the sample has it.
    marks: Vec<Option<Mark>>,
    moving: bool,
}

/// An annotation placed: what it covers in each sample it is drawn in, or
/// once for all of them when it stays put.
struct Placed {
    moving: bool,
    footprints: Vec<(usize, Footprint)>,
}

/// How an annotation sits beside an edge: how far its elbow is out from
/// where it lines up and how far it reaches past that, across the edge;
/// and how far it reaches before and after its elbow, along it.
#[derive(Debug, Clone, Copy)]
struct Reach {
    out: i32,
    past: i32,
    before: i32,
    after: i32,
}

impl Slot {
    /// The automatic annotations of `view` across `samples`.
    fn all(samples: &[Draft], view: Option<usize>) -> Vec<Self> {
        let mut slots: Vec<Self> = Vec::new();

        for (sample, draft) in samples.iter().enumerate() {
            for mark in draft.marks().iter().filter(|mark| mark.shown_in(view)) {
                let Some(id) = Id::of(mark) else {
                    continue;
                };

                let index = match slots.iter().position(|slot| slot.id == id) {
                    Some(index) => index,
                    None => {
                        slots.push(Self {
                            id,
                            marks: vec![None; samples.len()],
                            moving: false,
                        });
                        slots.len() - 1
                    }
                };

                slots[index].moving |= mark.moving;
                slots[index].marks[sample].get_or_insert_with(|| mark.clone());
            }
        }

        slots
    }

    /// The samples it is drawn in: one for all when it stays put.
    fn samples(&self) -> impl Iterator<Item = usize> + '_ {
        self.marks
            .iter()
            .enumerate()
            .filter(|(_, mark)| mark.is_some())
            .map(|(sample, _)| sample)
            .take(if self.moving { usize::MAX } else { 1 })
    }

    /// Where its target is on average.
    fn target(&self, projection: &Projection) -> Point<i32> {
        let targets: Vec<Point<i32>> = self
            .samples()
            .filter_map(|sample| Some(projection.px(target(self.marks[sample].as_ref()?))))
            .collect();
        let count = targets.len().max(1) as i32;

        Point::new(
            targets.iter().map(|point| point.x).sum::<i32>() / count,
            targets.iter().map(|point| point.y).sum::<i32>() / count,
        )
    }

    /// Its mark in `sample`, placed `offset` from its target.
    fn offset(&self, sample: usize, offset: (i32, i32)) -> Option<Mark> {
        let mut mark = self.marks[sample].clone()?;

        match &mut mark.ink {
            Ink::Balloon { offset: old, .. } | Ink::Note { elbow: old, .. } => {
                *old = Placement::Offset(offset.0, offset.1);
            }
            _ => {}
        }

        Some(mark)
    }

    /// What it covers at `spot`.
    fn placed(&self, spot: Spot, projection: &Projection) -> Placed {
        let footprints = self
            .samples()
            .filter_map(|sample| {
                let target = projection.px(target(self.marks[sample].as_ref()?));
                let mark = self.offset(sample, spot.offset(target))?;

                Some((sample, Footprint::of(&mark, projection)))
            })
            .collect();

        Placed {
            moving: self.moving,
            footprints,
        }
    }

    /// How it sits beside `edge`.
    fn reach(&self, edge: Edge, projection: &Projection) -> Reach {
        let none = Reach {
            out: 0,
            past: 0,
            before: 0,
            after: 0,
        };
        let Some(sample) = self.samples().next() else {
            return none;
        };
        let offset = edge.out(200);
        let Some(mark) = self.offset(sample, offset) else {
            return none;
        };
        let Some(bounds) = Footprint::of(&mark, projection).bounds(false) else {
            return none;
        };
        let target = projection.px(target(&mark));
        let elbow = Point::new(target.x + offset.0, target.y + offset.1);

        // Across the edge and along it, as for a column.
        let (elbow, low, high, top, bottom) = if edge.row() {
            (
                Point::new(elbow.y, elbow.x),
                bounds.y,
                bounds.y + bounds.height - 1,
                bounds.x,
                bounds.x + bounds.width - 1,
            )
        } else {
            (
                elbow,
                bounds.x,
                bounds.x + bounds.width - 1,
                bounds.y,
                bounds.y + bounds.height - 1,
            )
        };
        let (near, far) = if edge.sign() > 0 {
            (low, high)
        } else {
            (high, low)
        };

        Reach {
            out: (elbow.x - near) * edge.sign(),
            past: (far - elbow.x) * edge.sign(),
            before: elbow.y - top,
            after: bottom - elbow.y,
        }
    }
}

impl Placed {
    /// Its footprint at `sample`: its only one when it stays put.
    fn at(&self, sample: usize) -> Option<&Footprint> {
        if self.moving {
            self.footprints
                .iter()
                .find(|(at, _)| *at == sample)
                .map(|(_, footprint)| footprint)
        } else {
            self.footprints.first().map(|(_, footprint)| footprint)
        }
    }

    fn inside(&self, clip: Rectangle<i32>) -> bool {
        !self.footprints.is_empty()
            && self
                .footprints
                .iter()
                .all(|(_, footprint)| footprint.inside(clip))
    }

    /// What it costs on `ground`, on average over the samples it is in.
    fn cost(&self, ground: &Ground) -> f32 {
        let total: f32 = self
            .footprints
            .iter()
            .map(|(sample, footprint)| footprint.cost(ground, self.moving.then_some(*sample)))
            .sum();

        total / self.footprints.len().max(1) as f32
    }

    /// What it costs drawn with `other`, on average over the samples.
    fn clash(&self, other: &Self) -> f32 {
        let samples: Vec<usize> = match (self.moving, other.moving) {
            (false, false) => return self.footprints[0].1.clash(&other.footprints[0].1),
            (true, _) => self.footprints.iter().map(|(sample, _)| *sample).collect(),
            (false, true) => other.footprints.iter().map(|(sample, _)| *sample).collect(),
        };
        let costs: Vec<f32> = samples
            .iter()
            .filter_map(|&sample| Some(self.at(sample)?.clash(other.at(sample)?)))
            .collect();

        costs.iter().sum::<f32>() / costs.len().max(1) as f32
    }
}

fn place(mark: &mut Mark, placement: Placement) {
    match &mut mark.ink {
        Ink::Balloon { offset, .. } => *offset = placement,
        Ink::Note { elbow, .. } => *elbow = placement,
        _ => {}
    }
}

fn target(mark: &Mark) -> super::V2 {
    match &mark.ink {
        Ink::Balloon { target, .. } | Ink::Note { target, .. } => *target,
        _ => unreachable!("only balloons and notes are placed"),
    }
}

/// An annotation in a column: what it points at, and the rows it takes
/// above and below its elbow.
#[derive(Debug, Clone, Copy)]
struct Item {
    target: Point<i32>,
    above: i32,
    below: i32,
}

/// The elbow rows of `items` down the column `column`, as quadrille's
/// drafting demo lays out notes.
///
/// Each goes as near as it can to the row where a leader from its target,
/// rising one pixel for every two across, would meet the column: rising
/// for targets above row `pivot`, falling for the rest. They keep that
/// order down the column, stay `GAP` apart, and stay within `rows` when
/// they fit.
fn fan(items: &[Item], column: i32, rows: RangeInclusive<i32>, pivot: i32) -> Vec<i32> {
    let ideal = |item: &Item| {
        let rise = (column - item.target.x).abs() / 2;

        if item.target.y < pivot {
            item.target.y - rise
        } else {
            item.target.y + rise
        }
    };

    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&i| (ideal(&items[i]), items[i].target.y, items[i].target.x));

    // Each run of items packed a gap apart is a cluster, placed where its
    // items want to be on average. Clusters that overlap merge.
    struct Cluster {
        start: usize,
        end: usize,
        top: i32,
        offsets: Vec<i32>,
        height: i32,
    }

    let place = |start: usize, end: usize| {
        let mut offsets = Vec::with_capacity(end - start);
        let mut offset = 0;
        let mut previous: Option<&Item> = None;

        for &i in &order[start..end] {
            let item = &items[i];

            offset += match previous {
                Some(previous) => previous.below + 1 + GAP + item.above,
                None => item.above,
            };
            offsets.push(offset);
            previous = Some(item);
        }

        let height = offset + previous.map_or(0, |item| item.below) + 1;
        let wanted: i64 = order[start..end]
            .iter()
            .zip(&offsets)
            .map(|(&i, offset)| i64::from(ideal(&items[i]) - offset))
            .sum();
        let wanted = wanted.div_euclid((end - start) as i64) as i32;
        let lowest = rows.end() - height + 1;
        let top = wanted.min(lowest).max(*rows.start());

        Cluster {
            start,
            end,
            top,
            offsets,
            height,
        }
    };

    let mut clusters: Vec<Cluster> = Vec::new();

    for k in 0..order.len() {
        let mut cluster = place(k, k + 1);

        while let Some(previous) = clusters.last() {
            if previous.top + previous.height + GAP <= cluster.top {
                break;
            }

            let start = previous.start;
            clusters.pop();
            cluster = place(start, cluster.end);
        }

        clusters.push(cluster);
    }

    let mut elbows = vec![0; items.len()];

    for cluster in clusters {
        for (&i, offset) in order[cluster.start..cluster.end]
            .iter()
            .zip(&cluster.offsets)
        {
            elbows[i] = cluster.top + offset;
        }
    }

    elbows
}

/// What covers the view: what stays put, and what moves at each sample.
struct Ground {
    clip: Rectangle<i32>,
    still: Grid,
    /// What moves, at each sample...
    moving: Vec<Grid>,
    /// ...and everywhere it goes.
    swept: Grid,
    /// The bounds of the drawing's edges and areas.
    outline: Option<Rectangle<i32>>,
}

impl Ground {
    fn new(
        samples: &[Draft],
        projection: &Projection,
        clip: Rectangle<i32>,
        view: Option<usize>,
    ) -> Self {
        let mut ground = Self {
            clip,
            still: Grid::new(clip),
            moving: (0..samples.len()).map(|_| Grid::new(clip)).collect(),
            swept: Grid::new(clip),
            outline: None,
        };

        for (sample, draft) in samples.iter().enumerate() {
            for mark in draft.marks().iter().filter(|mark| mark.shown_in(view)) {
                // What stays put is the same in every sample.
                if sample > 0 && !mark.moving {
                    continue;
                }

                let mut pieces = Vec::new();
                raster::rasterize(mark, projection, &mut pieces);

                let what = if mark.pass() >= Pass::Annotation {
                    LETTERED
                } else {
                    DRAWN
                };
                let outlines = matches!(mark.pass(), Pass::Edges | Pass::Hidden | Pass::Areas);

                for inked in &pieces {
                    if mark.moving {
                        ground.moving[sample].cover(inked, what);
                        ground.swept.cover(inked, what);
                    } else {
                        ground.still.cover(inked, what);
                    }

                    if outlines && let Some(bounds) = extent(&inked.piece) {
                        ground.outline = Some(match ground.outline {
                            Some(outline) => union(outline, bounds),
                            None => bounds,
                        });
                    }
                }
            }
        }

        ground.outline = ground
            .outline
            .and_then(|outline| raster::intersection(outline, clip));
        ground
    }

    /// What covers `pixel`: at `sample`, or at any moment.
    fn at(&self, pixel: Point<i32>, sample: Option<usize>) -> u8 {
        self.still.at(pixel)
            | match sample {
                Some(sample) => self.moving[sample].at(pixel),
                None => self.swept.at(pixel),
            }
    }
}

/// A bit per pixel of the view for each thing that covers it.
struct Grid {
    clip: Rectangle<i32>,
    cells: Vec<u8>,
}

impl Grid {
    fn new(clip: Rectangle<i32>) -> Self {
        Self {
            clip,
            cells: vec![0; clip.width.max(0) as usize * clip.height.max(0) as usize],
        }
    }

    fn index(&self, pixel: Point<i32>) -> Option<usize> {
        raster::contains(self.clip, pixel).then(|| {
            (pixel.y - self.clip.y) as usize * self.clip.width as usize
                + (pixel.x - self.clip.x) as usize
        })
    }

    fn at(&self, pixel: Point<i32>) -> u8 {
        self.index(pixel).map_or(0, |index| self.cells[index])
    }

    fn set(&mut self, pixel: Point<i32>, what: u8) {
        if let Some(index) = self.index(pixel) {
            self.cells[index] |= what;
        }
    }

    fn fill(&mut self, area: Rectangle<i32>, what: u8) {
        if let Some(area) = raster::intersection(area, self.clip) {
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    self.set(Point::new(x, y), what);
                }
            }
        }
    }

    /// Marks what `inked` covers as `what`.
    fn cover(&mut self, inked: &Inked, what: u8) {
        match &inked.piece {
            Piece::Path { pixels, .. } => {
                for pixel in pixels {
                    self.set(*pixel, what);
                }
            }
            Piece::Rows { rows, .. } => {
                for &(y, from, to) in rows {
                    self.fill(rect(from, y, to - from + 1, 1), what);
                }
            }
            Piece::Text { at, text } => self.fill(text_box(*at, text), what),
            Piece::Block(area) | Piece::Knockout(area) => self.fill(*area, what),
        }
    }
}

/// What an annotation covers: the ground it clears for its lettering and
/// circle, and the lines it draws.
#[derive(Debug, Default)]
struct Footprint {
    boxes: Vec<Rectangle<i32>>,
    lines: Vec<Point<i32>>,
}

impl Footprint {
    fn of(mark: &Mark, projection: &Projection) -> Self {
        let mut pieces = Vec::new();
        raster::rasterize(mark, projection, &mut pieces);

        let mut footprint = Self::default();

        for inked in pieces {
            match inked.piece {
                Piece::Knockout(area) => footprint.boxes.push(area),
                Piece::Text { at, text } => footprint.boxes.push(text_box(at, &text)),
                Piece::Path { pixels, .. } => footprint.lines.extend(pixels),
                // The dot on the target, and arrowheads.
                Piece::Block(_) | Piece::Rows { .. } => {}
            }
        }

        // A balloon's circle is inside its box.
        let boxes = footprint.boxes.clone();
        footprint
            .lines
            .retain(|pixel| !boxes.iter().any(|area| raster::contains(*area, *pixel)));

        footprint
    }

    /// The bounds of its boxes, and of its lines too when `lines`.
    fn bounds(&self, lines: bool) -> Option<Rectangle<i32>> {
        let points = self
            .lines
            .iter()
            .filter(|_| lines)
            .map(|pixel| rect(pixel.x, pixel.y, 1, 1));

        self.boxes.iter().copied().chain(points).reduce(union)
    }

    /// Whether it covers any of `area`, or its leader crosses it.
    fn meets(&self, area: Rectangle<i32>) -> bool {
        self.boxes
            .iter()
            .any(|own| raster::intersection(*own, area).is_some())
            || self
                .lines
                .iter()
                .any(|pixel| raster::contains(area, *pixel))
    }

    /// Whether all of it is inside `clip`.
    fn inside(&self, clip: Rectangle<i32>) -> bool {
        self.boxes
            .iter()
            .all(|area| raster::intersection(*area, clip) == Some(*area))
            && self
                .lines
                .iter()
                .all(|pixel| raster::contains(clip, *pixel))
    }

    /// What it costs on `ground`, at `sample` or at any moment.
    fn cost(&self, ground: &Ground, sample: Option<usize>) -> f32 {
        let mut cost = 0.0;

        for area in &self.boxes {
            for y in area.y..area.y + area.height {
                for x in area.x..area.x + area.width {
                    let what = ground.at(Point::new(x, y), sample);

                    if what & LETTERED != 0 {
                        cost += HIDES_LETTERING;
                    } else if what & DRAWN != 0 {
                        cost += HIDES_DRAWING;
                    }
                }
            }
        }

        for pixel in &self.lines {
            let what = ground.at(*pixel, sample);

            cost += REACH;

            if what & LETTERED != 0 {
                cost += CROSSES_LETTERING;
            } else if what & DRAWN != 0 {
                cost += CROSSES_DRAWING;
            }
        }

        cost
    }

    /// What it costs for it and `other` to be drawn together.
    fn clash(&self, other: &Self) -> f32 {
        let mut cost = 0.0;

        for a in &self.boxes {
            for b in &other.boxes {
                if let Some(both) = raster::intersection(*a, *b) {
                    cost += HIDES_LETTERING * (both.width * both.height) as f32;
                }
            }
        }

        let crosses = |lines: &[Point<i32>], boxes: &[Rectangle<i32>]| {
            lines
                .iter()
                .filter(|pixel| boxes.iter().any(|area| raster::contains(*area, **pixel)))
                .count() as f32
        };

        cost += CROSSES_LETTERING * crosses(&self.lines, &other.boxes);
        cost += CROSSES_LETTERING * crosses(&other.lines, &self.boxes);

        let theirs: HashSet<(i32, i32)> = other.lines.iter().map(|p| (p.x, p.y)).collect();
        let met = self
            .lines
            .iter()
            .filter(|pixel| theirs.contains(&(pixel.x, pixel.y)))
            .count();

        cost + CROSSES_LEADER * met as f32
    }
}

/// The box `text` letters, its line box's top-left corner at `at`.
fn text_box(at: Point<i32>, text: &str) -> Rectangle<i32> {
    rect(
        at.x,
        at.y,
        i32::from(LETTERING.width(text)),
        i32::from(LETTERING.line()),
    )
}

fn extent(piece: &Piece) -> Option<Rectangle<i32>> {
    match piece {
        Piece::Path { pixels, .. } => pixels
            .iter()
            .map(|pixel| rect(pixel.x, pixel.y, 1, 1))
            .reduce(union),
        Piece::Rows { rows, .. } => rows
            .iter()
            .map(|&(y, from, to)| rect(from, y, to - from + 1, 1))
            .reduce(union),
        Piece::Text { at, text } => Some(text_box(*at, text)),
        Piece::Block(area) | Piece::Knockout(area) => Some(*area),
    }
}

fn union(a: Rectangle<i32>, b: Rectangle<i32>) -> Rectangle<i32> {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let right = (a.x + a.width).max(b.x + b.width);
    let bottom = (a.y + a.height).max(b.y + b.height);

    rect(x, y, right - x, bottom - y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::{Line, v};

    fn item(x: i32, y: i32, above: i32, below: i32) -> Item {
        Item {
            target: Point::new(x, y),
            above,
            below,
        }
    }

    #[test]
    fn a_lone_item_sits_where_its_leader_rises_to() {
        assert_eq!(fan(&[item(60, 50, 9, 0)], 100, 0..=200, 100), [30]);
    }

    #[test]
    fn a_fan_keeps_items_apart_in_order_and_in_bounds() {
        let items = [
            item(50, 40, 9, 0),
            item(52, 42, 9, 12),
            item(54, 44, 9, 0),
            item(50, 150, 9, 0),
            item(48, 152, 9, 0),
            item(60, 10, 9, 0),
        ];
        let elbows = fan(&items, 100, 4..=190, 100);

        let mut spans: Vec<(i32, i32)> = items
            .iter()
            .zip(&elbows)
            .map(|(item, y)| (y - item.above, y + item.below))
            .collect();
        spans.sort();

        for pair in spans.windows(2) {
            assert!(pair[0].1 + 1 + GAP <= pair[1].0, "{pair:?}");
        }

        assert!(
            spans
                .iter()
                .all(|(top, bottom)| *top >= 4 && *bottom <= 190)
        );
        assert!(elbows[0] < elbows[1] && elbows[1] < elbows[2]);
        assert!(elbows[3] > items[3].target.y);
    }

    const UNIT: Projection = Projection::new((200.0, 150.0), 1.0);

    const VIEW: Rectangle<i32> = rect(0, 0, 400, 300);

    /// A square part with two balloons on it and a note.
    fn part() -> Draft {
        let mut draft = Draft::new();

        draft.rect(v(-50.0, -50.0), v(50.0, 50.0), Line::Outline);
        draft.balloon(0, v(-40.0, 40.0), Placement::Auto);
        draft.balloon(1, v(40.0, -40.0), Placement::Auto);
        draft.note(v(45.0, 45.0), Placement::Auto, "FACE");
        draft
    }

    #[test]
    fn still_annotations_go_beside_the_drawing_and_clear_of_each_other() {
        let mut draft = part();
        let plan = Plan::new(std::slice::from_ref(&draft), &UNIT, VIEW, None);

        assert!(draft.marks()[1..].iter().all(|mark| plan.places(mark)));
        plan.apply(draft.marks_mut(), &UNIT);

        let footprints: Vec<Footprint> = draft.marks()[1..]
            .iter()
            .map(|mark| Footprint::of(mark, &UNIT))
            .collect();

        for footprint in &footprints {
            let bounds = footprint.bounds(false).expect("Placed");

            assert!(footprint.inside(VIEW));
            // Outside the square, which spans 150 to 250 across.
            assert!(
                bounds.x + bounds.width <= 150 || bounds.x > 250,
                "{bounds:?}"
            );
        }

        for (i, a) in footprints.iter().enumerate() {
            for b in &footprints[i + 1..] {
                assert_eq!(a.clash(b), 0.0);
            }
        }
    }

    #[test]
    fn an_annotation_with_no_room_is_left_out() {
        let mut draft = part();
        // A view no wider than the part.
        let narrow = rect(140, 0, 120, 300);
        let plan = Plan::new(std::slice::from_ref(&draft), &UNIT, narrow, None);

        plan.apply(draft.marks_mut(), &UNIT);

        for mark in &draft.marks()[1..] {
            let footprint = Footprint::of(mark, &UNIT);

            assert!(footprint.boxes.is_empty() || footprint.inside(narrow));
        }
    }

    #[test]
    fn a_moving_annotation_is_clear_of_its_part_wherever_it_goes() {
        let at = |k: usize| -60.0 + 40.0 * k as f32;
        let mut samples: Vec<Draft> = (0..4)
            .map(|k| {
                let mut draft = Draft::new();
                let x = at(k);

                draft.moving(|draft| {
                    draft.rect(v(x - 10.0, -10.0), v(x + 10.0, 10.0), Line::Outline);
                    draft.balloon(0, v(x, 0.0), Placement::Auto);
                });
                draft
            })
            .collect();
        let plan = Plan::new(&samples, &UNIT, VIEW, None);

        assert!(plan.places(&samples[0].marks()[1]));

        for (k, sample) in samples.iter_mut().enumerate() {
            plan.apply(sample.marks_mut(), &UNIT);

            let footprint = Footprint::of(&sample.marks()[1], &UNIT);
            let corner = UNIT.px(v(at(k) - 10.0, 10.0));
            let part = rect(corner.x, corner.y, 21, 21);

            assert!(footprint.inside(VIEW));
            assert!(
                footprint
                    .boxes
                    .iter()
                    .all(|area| raster::intersection(*area, part).is_none()),
                "at {k}: {footprint:?} over {part:?}"
            );
        }
    }
}
