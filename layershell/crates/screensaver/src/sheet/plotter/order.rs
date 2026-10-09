//! The order the pen draws its strokes in: a drafting office's.
//!
//! Strokes go in groups, the strokes of one mark, which keep their order:
//! a dimension's extension lines, then its line, its arrowheads and its
//! value. Each group has a key, its place in the drafting office's
//! [stages](stage): the skeleton of every view, then the bodies view by
//! view and part by part, the lining, the annotation, the traces, the
//! balloons. The groups are drawn in the order of their keys, and those
//! with the same key nearest first from wherever the pen is: a line is
//! turned round, an area's lining run backwards or a loop started where
//! that is nearer. The order is then improved where moving a few groups
//! elsewhere, or turning a stretch of them round, shortens the pen's
//! journey (Or-opt and 2-opt), and each loop started where the pen comes
//! to it and leaves it soonest. The rest of the key keeps clusters apart
//! for the motion's beats: a stage from the next, a part or a view from
//! another.
//!
//! The axes go down longest first, circles round one centre smallest
//! first, a balloon from the dot on its part outwards, and each loop round
//! the way the pen was heading. A diagram grows: from its first part along
//! what touches it, each part drawn whole with its lettering as the pen
//! reaches it, what joins two parts drawn on the way.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use glam::DVec2;
use iced_core::Point;

use crate::draft::{Line, Pass};

use super::strokes::{Form, Stroke};

/// What the order needs to know of a mark.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Meta {
    pub pass: Pass,
    /// The type of line, for a stroke.
    pub line: Option<Line>,
    /// Whether it is a circle, an arc or another closed shape: drawn before
    /// straight lines, as a draughtsman inks them.
    pub curved: bool,
    pub part: Option<usize>,
    /// The view it is drawn in, by its pane's index on the sheet...
    pub pane: usize,
    /// ...and for a cutting plane, the pane of the section it cuts.
    pub cuts: Option<usize>,
    /// A balloon's item number.
    pub item: Option<usize>,
    /// A circle's centre: the circles round one centre are drawn one after
    /// another.
    pub centre: Option<Point<i32>>,
    /// A circle's or an arc's centre and radius, which a compass draws it
    /// round.
    pub compass: Option<(Point<i32>, i32)>,
    /// Whether it is lettering alone: a name, a legend.
    pub label: bool,
}

/// What comes before a stroke in the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gap {
    /// The stroke before it, of the same mark.
    None,
    /// Another mark of the same cluster.
    Group,
    /// Another cluster of the same stage: another part, or view.
    Cluster,
    /// Another stage of the order, or the start.
    Stage,
}

/// Strokes in the order the pen draws them, and what comes between.
pub struct Ordered {
    pub strokes: Vec<Stroke>,
    /// What comes before each stroke.
    pub gaps: Vec<Gap>,
    /// The stage of the order each stroke is drawn in.
    pub stages: Vec<u8>,
    pub groups: usize,
}

/// A drafting office's stages, by their place in the order.
pub mod stage {
    pub const SKELETON: u8 = 0;
    pub const BODIES: u8 = 1;
    pub const LINING: u8 = 2;
    pub const ANNOTATION: u8 = 3;
    pub const TRACES: u8 = 4;
    pub const BALLOONS: u8 = 5;
}

/// Pairs of marks that touch on the sheet, each the lower index first.
pub type Touching = BTreeSet<(usize, usize)>;

/// A group's place in the order: its stage, the cluster within the stage
/// (the first two numbers after it), and what orders the cluster within.
type Key = (u8, u32, u32, u32, u32);

/// The strokes of one mark, in order.
struct Group {
    strokes: Vec<Stroke>,
    key: Key,
    /// Whether it may be drawn backwards whole: an area's lining or rows.
    backwards: bool,
    /// A circle's centre.
    centre: Option<Point<i32>>,
}

/// How far apart, along a loop, the pen may start it.
const SEAMS: usize = 16;

/// The most groups a cluster may have for its order to be polished, and
/// the most rounds of polishing: past these the gain is not worth the time
/// it takes on a sheet's first frame.
const POLISHED: usize = 600;
const ROUNDS: usize = 3;

impl Group {
    fn start(&self) -> Point<i32> {
        self.strokes[0].start()
    }

    fn end(&self) -> Point<i32> {
        self.strokes[self.strokes.len() - 1].end()
    }

    /// Whether it can be drawn the other way round whole: it starts where
    /// it ends, or is one line, or may be drawn backwards.
    fn turns(&self) -> bool {
        self.start() == self.end()
            || self.backwards
            || (self.strokes.len() == 1 && self.strokes[0].form == Form::Line)
    }

    /// Turns it round, if it [`turns`](Self::turns).
    fn turn(&mut self) {
        if self.start() == self.end() {
            return;
        }

        self.strokes.reverse();

        for stroke in &mut self.strokes {
            stroke.reverse();
        }
    }
}

/// `strokes`, in the order made, put in a drafting office's order for a
/// pen starting at `from`; `marks` says what their marks are. A diagram
/// grows along the marks `touching`, if it is given.
pub fn order(
    strokes: Vec<Stroke>,
    marks: &[Meta],
    from: Point<i32>,
    touching: Option<&Touching>,
) -> Ordered {
    let mut groups = group(strokes, marks);

    axes(&mut groups);

    for group in &mut groups {
        if marks[group.strokes[0].mark].pass == Pass::Balloons {
            balloon(&mut group.strokes);
        }
    }

    if let Some(touching) = touching {
        grow(&mut groups, marks, touching);
    }

    // A stable sort: groups with the same key stay in the order made.
    groups.sort_by_key(|group| group.key);

    let count = groups.len();
    let mut ordered = Ordered {
        strokes: Vec::new(),
        gaps: Vec::new(),
        stages: Vec::new(),
        groups: count,
    };
    let mut pen = from;
    let mut last: Option<Key> = None;
    let mut rest = groups.into_iter().peekable();

    while let Some(first) = rest.next() {
        // The cluster: every group with this key.
        let key = first.key;
        let mut cluster = vec![first];

        while let Some(group) = rest.next_if(|group| group.key == key) {
            cluster.push(group);
        }

        let start = pen;

        cluster = nearest(cluster, &mut pen);

        if cluster.len() <= POLISHED {
            cluster = polish(cluster, start);
        }

        cluster = concentric(cluster);
        reseam(&mut cluster, start);
        pen = cluster.last().map_or(pen, Group::end);

        for (index, group) in cluster.into_iter().enumerate() {
            let gap = match last {
                _ if index > 0 => Gap::Group,
                Some(before) if before.0 == key.0 && (before.1, before.2) == (key.1, key.2) => {
                    Gap::Group
                }
                Some(before) if before.0 == key.0 => Gap::Cluster,
                _ => Gap::Stage,
            };

            for (index, stroke) in group.strokes.into_iter().enumerate() {
                pen = stroke.end();
                ordered.gaps.push(if index == 0 { gap } else { Gap::None });
                ordered.stages.push(key.0);
                ordered.strokes.push(stroke);
            }
        }

        last = Some(key);
    }

    onward(&mut ordered.strokes, from);

    ordered
}

/// The strokes in groups, each with its key.
fn group(strokes: Vec<Stroke>, marks: &[Meta]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();

    for stroke in strokes {
        let meta = &marks[stroke.mark];

        match groups
            .last_mut()
            .filter(|group| group.strokes[0].mark == stroke.mark)
        {
            Some(group) => {
                group.backwards &= group.strokes[0].piece == stroke.piece;
                group.strokes.push(stroke);
            }
            None => groups.push(Group {
                key: key(meta),
                backwards: meta.pass == Pass::Areas,
                centre: meta.centre,
                strokes: vec![stroke],
            }),
        }
    }

    groups
}

/// A part's rank in the order: what belongs to none first, then the parts
/// in the order of their items.
fn rank(part: Option<usize>) -> u32 {
    part.map_or(0, |part| part as u32 + 1)
}

/// Whether a mark of `meta` is the skeleton every view is built on: a
/// centre or phantom line, or construction belonging to no part.
fn skeleton(meta: &Meta) -> bool {
    meta.pass == Pass::Construction && !(meta.line == Some(Line::Thin) && meta.part.is_some())
}

/// Where a mark of `meta` goes in the order.
fn key(meta: &Meta) -> Key {
    match meta.pass {
        _ if skeleton(meta) => (stage::SKELETON, 0, 0, 0, 0),
        Pass::Construction | Pass::Edges | Pass::Hidden => (
            stage::BODIES,
            meta.pane as u32,
            rank(meta.part) + 1,
            body(meta),
            0,
        ),
        Pass::Areas => (stage::LINING, rank(meta.part), 0, 0, 0),
        Pass::Annotation => match meta.cuts {
            // A cutting plane before the section it cuts.
            Some(section) => (stage::BODIES, section as u32, 0, 0, 0),
            None => (stage::ANNOTATION, meta.pane as u32, 0, 0, 0),
        },
        Pass::Traces => (stage::TRACES, 0, 0, 0, 0),
        Pass::Balloons => (stage::BALLOONS, meta.item.unwrap_or(0) as u32, 0, 0, 0),
    }
}

/// Where a part's body line of `meta` goes within the part: its circles,
/// arcs and closed outlines first, then its straight lines, then the lines
/// hidden behind it.
fn body(meta: &Meta) -> u32 {
    match meta.pass {
        Pass::Hidden => 3,
        _ if meta.curved => 1,
        _ => 2,
    }
}

/// How many pixels the pen passes over drawing `group`.
fn length(group: &Group) -> usize {
    group.strokes.iter().map(|stroke| stroke.pixels.len()).sum()
}

/// The skeleton's axes first, longest first: its straight lines at least
/// half as long as the longest, each a cluster of its own; the rest of it
/// after them, nearest first.
fn axes(groups: &mut [Group]) {
    let axis = |group: &Group| {
        group.key.0 == stage::SKELETON
            && group.strokes.len() == 1
            && group.strokes[0].form == Form::Line
    };
    let longest = groups
        .iter()
        .filter(|group| axis(group))
        .map(length)
        .max()
        .unwrap_or(0);
    let mut lines: Vec<(usize, usize)> = groups
        .iter()
        .enumerate()
        .filter(|(_, group)| axis(group) && 2 * length(group) >= longest)
        .map(|(index, group)| (index, length(group)))
        .collect();

    lines.sort_by_key(|&(index, length)| (std::cmp::Reverse(length), index));

    for group in groups.iter_mut() {
        if group.key.0 == stage::SKELETON {
            group.key = (stage::SKELETON, 0, 0, 1, 0);
        }
    }

    for (rank, (index, _)) in lines.into_iter().enumerate() {
        groups[index].key = (stage::SKELETON, 0, 0, 0, rank as u32);
    }
}

/// A balloon's strokes as a draughtsman draws them: the dot on the part,
/// the leader out from it, the circle from where the leader meets it, then
/// the number.
fn balloon(strokes: &mut [Stroke]) {
    // Its pieces as they are made: the leader, the dot, the ground the
    // number clears, the circle and the number.
    let rank = |piece: usize| match piece {
        1 => 0,
        0 => 1,
        _ => piece,
    };

    strokes.sort_by_key(|stroke| rank(stroke.piece));

    let Some(dot) = strokes
        .iter()
        .find(|stroke| stroke.piece == 1)
        .map(Stroke::start)
    else {
        return;
    };
    let mut pen = dot;

    for stroke in strokes.iter_mut() {
        match (stroke.piece, stroke.form) {
            (0, Form::Line) => {
                if distance(stroke.end(), pen) < distance(stroke.start(), pen) {
                    stroke.reverse();
                }

                pen = stroke.end();
            }
            (_, Form::Loop) => {
                let seam = (0..stroke.pixels.len())
                    .min_by_key(|&index| (distance(stroke.pixels[index], pen), index))
                    .unwrap_or(0);

                stroke.rotate(seam);
            }
            _ => {}
        }
    }
}

/// `cluster` with the circles round each centre it has more than one round
/// drawn one after another, smallest first, where the first of them was:
/// as a compass is opened out.
fn concentric(cluster: Vec<Group>) -> Vec<Group> {
    let round = |group: &Group| {
        group
            .centre
            .filter(|_| group.strokes[0].closed())
            .map(|centre| (centre.x, centre.y))
    };
    let mut counts: BTreeMap<(i32, i32), usize> = BTreeMap::new();

    for group in &cluster {
        if let Some(centre) = round(group) {
            *counts.entry(centre).or_default() += 1;
        }
    }

    // Each group in its place, or a centre's circles where the first was.
    let mut places: Vec<Result<Group, (i32, i32)>> = Vec::with_capacity(cluster.len());
    let mut sets: BTreeMap<(i32, i32), Vec<Group>> = BTreeMap::new();

    for group in cluster {
        match round(&group).filter(|centre| counts[centre] > 1) {
            Some(centre) => {
                if !sets.contains_key(&centre) {
                    places.push(Err(centre));
                }

                sets.entry(centre).or_default().push(group);
            }
            None => places.push(Ok(group)),
        }
    }

    places
        .into_iter()
        .flat_map(|place| match place {
            Ok(group) => vec![group],
            Err(centre) => {
                let mut set = sets.remove(&centre).unwrap_or_default();
                let at = Point::new(centre.0, centre.1);

                set.sort_by_key(|group| distance(at, group.strokes[0].pixels[0]));
                set
            }
        })
        .collect()
}

/// Turns each loop of `strokes`, drawn in order by a pen starting at
/// `from`, to go round the way the pen was heading as it came to it: as a
/// hand carries a stroke on rather than doubling back.
fn onward(strokes: &mut [Stroke], from: Point<i32>) {
    /// How many pixels along a stroke its heading is taken over.
    const ALONG: usize = 3;

    let towards = |from: Point<i32>, to: Point<i32>| place(to) - place(from);
    let mut pen = from;
    let mut heading = DVec2::ZERO;

    for stroke in strokes.iter_mut() {
        let hop = towards(pen, stroke.start());

        if hop.length() >= 2.0 {
            heading = hop.normalize();
        }

        let count = stroke.pixels.len();

        if stroke.form == Form::Loop && count > 2 * ALONG {
            let ahead = towards(stroke.pixels[0], stroke.pixels[ALONG]);
            let behind = towards(stroke.pixels[0], stroke.pixels[count - ALONG]);

            if heading.dot(behind) > heading.dot(ahead) + 1e-9 {
                stroke.reverse();
            }
        }

        let last = match stroke.form {
            Form::Loop => Some((stroke.pixels[count.saturating_sub(ALONG)], stroke.pixels[0])),
            Form::Line | Form::Glyph if count > 1 => {
                Some((stroke.pixels[count.saturating_sub(1 + ALONG)], stroke.end()))
            }
            _ => None,
        };

        if let Some((before, end)) = last {
            let way = towards(before, end);

            if way.length() > 0.0 {
                heading = way.normalize();
            }
        }

        pen = stroke.end();
    }
}

/// What a diagram's growth walks over: a part, all of it, or a mark
/// belonging to none that joins parts (a wire, a junction) or labels what
/// joins them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Node {
    Part(usize),
    Mark(usize),
}

/// Gives the groups of a diagram the keys of its growth: from its first
/// part outwards along the marks `touching` each other, breadth first. A
/// part is drawn whole as the walk reaches it (its body and the wiring
/// that joins it only to itself, its lining, its lettering), straight after
/// what led to it; a mark of no part (a wire, a junction, a legend on a
/// wire) as it is crossed. Parts are taken by their ranks, then what joins
/// them left to right. What the walk never reaches of the bodies is drawn
/// after it, and the rest keeps its stage.
fn grow(groups: &mut [Group], marks: &[Meta], touching: &Touching) {
    // What each mark is drawn as in the walk, and where in its node: a
    // part's lines, its lining, what it traces still, its lettering; and
    // whether it is drawn if the walk never reaches it.
    let walked = |mark: usize, key: Key| -> Option<(Node, u32, bool)> {
        let meta = &marks[mark];
        let label = meta.pass == Pass::Annotation && meta.label;

        match (meta.part, key.0) {
            (Some(part), stage::BODIES) if meta.cuts.is_none() => {
                Some((Node::Part(part), body(meta), true))
            }
            (Some(part), stage::LINING) => Some((Node::Part(part), 4, true)),
            (Some(part), stage::TRACES) => Some((Node::Part(part), 5, true)),
            (Some(part), stage::ANNOTATION) if label => Some((Node::Part(part), 6, true)),
            (None, stage::BODIES) if meta.cuts.is_none() => {
                Some((Node::Mark(mark), body(meta), true))
            }
            (None, stage::ANNOTATION) if label => Some((Node::Mark(mark), 6, false)),
            (None, stage::TRACES) if meta.line.is_none() => Some((Node::Mark(mark), 5, false)),
            _ => None,
        }
    };
    let mut nodes: BTreeMap<usize, (Node, u32, bool)> = BTreeMap::new();
    // The top left of each node.
    let mut corners: BTreeMap<Node, (i32, i32)> = BTreeMap::new();

    for group in groups.iter() {
        let mark = group.strokes[0].mark;

        if let Some(walk) = walked(mark, group.key) {
            nodes.insert(mark, walk);

            for pixel in group.strokes.iter().flat_map(|stroke| &stroke.pixels) {
                let corner = corners.entry(walk.0).or_insert((pixel.x, pixel.y));

                *corner = (corner.0.min(pixel.x), corner.1.min(pixel.y));
            }
        }
    }

    let mut joins: BTreeMap<Node, BTreeSet<Node>> = BTreeMap::new();

    for &(a, b) in touching {
        if let (Some(&(a, ..)), Some(&(b, ..))) = (nodes.get(&a), nodes.get(&b))
            && a != b
        {
            joins.entry(a).or_default().insert(b);
            joins.entry(b).or_default().insert(a);
        }
    }

    // Parts by rank, then marks of no part left to right.
    let joins: BTreeMap<Node, Vec<Node>> = joins
        .into_iter()
        .map(|(node, next)| {
            let mut next: Vec<Node> = next.into_iter().collect();

            next.sort_by_key(|node| match node {
                Node::Part(part) => (0, *part as i32, 0, 0),
                Node::Mark(mark) => (1, corners[node].0, corners[node].1, *mark as i32),
            });

            (node, next)
        })
        .collect();
    let mut walk = Walk::default();
    let roots: Vec<Node> = corners
        .keys()
        .filter(|node| matches!(node, Node::Part(_)))
        .copied()
        .collect();

    // From the first part, and from any part the walk never reaches.
    for root in roots {
        if walk.seen.contains(&root) {
            continue;
        }

        walk.part(root, &joins);

        while let Some(node) = walk.queue.pop_front() {
            for &next in joins.get(&node).into_iter().flatten() {
                match next {
                    _ if walk.seen.contains(&next) => {}
                    Node::Part(_) => walk.part(next, &joins),
                    Node::Mark(_) => {
                        walk.mark(next, None);

                        // What it leads to, straight after it.
                        for &part in joins.get(&next).into_iter().flatten() {
                            if matches!(part, Node::Part(_)) && !walk.seen.contains(&part) {
                                walk.part(part, &joins);
                            }
                        }
                    }
                }
            }
        }
    }

    // Each part a cluster of the stage, and what leads to it with it.
    let mut places: BTreeMap<Node, (u32, u32)> = BTreeMap::new();
    let mut count = 0;

    for (index, &(node, within)) in walk.order.iter().enumerate() {
        if let Node::Part(_) = node {
            count += 1;
        }

        let place = match (node, within) {
            (Node::Part(_), _) => (count, index as u32),
            // Wiring a part's own, with the part.
            (Node::Mark(_), Some(part)) => places[&part],
            (Node::Mark(_), None) => (count + 1, index as u32),
        };

        places.insert(node, place);
    }

    for group in groups.iter_mut() {
        let Some(&(node, within, always)) = nodes.get(&group.strokes[0].mark) else {
            continue;
        };

        match places.get(&node) {
            Some(&(step, index)) => group.key = (stage::BODIES, 0, step, index, within),
            // A line of no part the walk never reached, after it.
            None if always => group.key = (stage::BODIES, 0, count + 2, 0, within),
            None => {}
        }
    }
}

/// A diagram's growth as it is walked.
#[derive(Default)]
struct Walk {
    seen: BTreeSet<Node>,
    /// What is reached, in order, each mark of no part with the part whose
    /// own wiring it is, if it is.
    order: Vec<(Node, Option<Node>)>,
    /// What is still to be gone on from.
    queue: VecDeque<Node>,
}

impl Walk {
    /// Reaches `node`, a mark of no part, as part `of`'s own wiring if it
    /// is.
    fn mark(&mut self, node: Node, of: Option<Node>) {
        self.seen.insert(node);
        self.order.push((node, of));
        self.queue.push_back(node);
    }

    /// Reaches `part`, and with it the wiring that joins it only to itself
    /// or to what is drawn: every mark of no part it touches from which
    /// marks of no part lead to no part not yet reached.
    fn part(&mut self, part: Node, joins: &BTreeMap<Node, Vec<Node>>) {
        self.seen.insert(part);
        self.order.push((part, None));
        self.queue.push_back(part);

        for &start in joins.get(&part).into_iter().flatten() {
            if !matches!(start, Node::Mark(_)) || self.seen.contains(&start) {
                continue;
            }

            // The wiring from `start`, as far as marks of no part go.
            let mut wiring = vec![start];
            let mut found = BTreeSet::from([start]);
            let mut own = true;
            let mut index = 0;

            while index < wiring.len() {
                for &next in joins.get(&wiring[index]).into_iter().flatten() {
                    match next {
                        Node::Part(_) if next != part && !self.seen.contains(&next) => own = false,
                        Node::Mark(_) if !self.seen.contains(&next) && found.insert(next) => {
                            wiring.push(next);
                        }
                        _ => {}
                    }
                }

                index += 1;
            }

            if own {
                for node in wiring {
                    self.seen.insert(node);
                    self.order.push((node, Some(part)));
                }
            }
        }
    }
}

/// How a group can be started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    Forwards,
    /// Its first stroke turned round.
    FirstTurned,
    /// All of it backwards.
    Backwards,
    /// Its first stroke, a loop, started at this pixel.
    Seam(usize),
}

/// Where a group can be started from, and how.
type Entry = (Point<i32>, Way);

fn entries(group: &Group) -> Vec<Entry> {
    let first = &group.strokes[0];

    match first.form {
        Form::Touch(at) => vec![(at, Way::Forwards)],
        // Lettering is written one way.
        Form::Glyph => vec![(first.start(), Way::Forwards)],
        Form::Loop => (0..first.pixels.len())
            .step_by(SEAMS)
            .map(|index| (first.pixels[index], Way::Seam(index)))
            .collect(),
        Form::Line if group.backwards => {
            let last = &group.strokes[group.strokes.len() - 1];
            let end = match last.form {
                Form::Touch(at) => at,
                _ => last.pixels[last.pixels.len() - 1],
            };

            vec![(first.start(), Way::Forwards), (end, Way::Backwards)]
        }
        Form::Line => vec![
            (first.start(), Way::Forwards),
            (first.end(), Way::FirstTurned),
        ],
    }
}

fn distance(a: Point<i32>, b: Point<i32>) -> i64 {
    let (x, y) = (i64::from(a.x - b.x), i64::from(a.y - b.y));

    x * x + y * y
}

/// `cluster` in the order a pen at `pen` reaches it going always to the
/// nearest group next, each started the nearest way; `pen` ends where the
/// last leaves it. Ties go to the group made first.
fn nearest(cluster: Vec<Group>, pen: &mut Point<i32>) -> Vec<Group> {
    let mut left: Vec<(Group, Vec<Entry>)> = cluster
        .into_iter()
        .map(|group| {
            let entries = entries(&group);
            (group, entries)
        })
        .collect();
    let mut ordered = Vec::with_capacity(left.len());

    while !left.is_empty() {
        let from = *pen;
        let (index, way) = left
            .iter()
            .enumerate()
            .flat_map(|(index, (_, entries))| {
                entries
                    .iter()
                    .map(move |&(at, way)| (distance(from, at), index, way))
            })
            .min_by_key(|&(distance, index, _)| (distance, index))
            .map(|(_, index, way)| (index, way))
            .expect("A group left");
        let (mut group, _) = left.remove(index);

        match way {
            Way::Forwards => {}
            Way::FirstTurned => group.strokes[0].reverse(),
            Way::Backwards => {
                group.strokes.reverse();

                for stroke in &mut group.strokes {
                    stroke.reverse();
                }
            }
            Way::Seam(_) => {
                // The loop's pixel nearest the pen, not only its nearest
                // sample.
                let first = &mut group.strokes[0];
                let start = (0..first.pixels.len())
                    .min_by_key(|&index| (distance(*pen, first.pixels[index]), index))
                    .unwrap_or(0);

                first.rotate(start);
            }
        }

        *pen = group.strokes[group.strokes.len() - 1].end();
        ordered.push(group);
    }

    ordered
}

/// Groups in order, each by its index and whether it is turned round.
type Tour = Vec<(usize, bool)>;

fn place(pixel: Point<i32>) -> DVec2 {
    DVec2::new(f64::from(pixel.x), f64::from(pixel.y))
}

/// `cluster`, drawn in this order by a pen starting at `from`, improved
/// wherever moving a run of one to three groups elsewhere (turned round, if
/// they turn) or turning a stretch of them round shortens the pen's journey
/// between them: a few rounds of each, the first improvement found taken
/// each time.
fn polish(mut cluster: Vec<Group>, from: Point<i32>) -> Vec<Group> {
    let count = cluster.len();
    let ends: Vec<(DVec2, DVec2, bool)> = cluster
        .iter()
        .map(|group| (place(group.start()), place(group.end()), group.turns()))
        .collect();
    let mut tour: Tour = (0..count).map(|index| (index, false)).collect();
    let from = place(from);
    let entry = |&(index, turned): &(usize, bool)| {
        if turned { ends[index].1 } else { ends[index].0 }
    };
    let exit = |&(index, turned): &(usize, bool)| {
        if turned { ends[index].0 } else { ends[index].1 }
    };
    // Where the pen leaves what is before place `at`.
    let before = |tour: &[(usize, bool)], at: usize| {
        if at == 0 { from } else { exit(&tour[at - 1]) }
    };
    // Where the pen goes on to after place `at`.
    let after = |tour: &[(usize, bool)], at: usize| tour.get(at).map(entry);
    let turns = |stretch: &[(usize, bool)]| stretch.iter().all(|&(index, _)| ends[index].2);
    const GAIN: f64 = 1e-6;

    for _ in 0..ROUNDS {
        let mut improved = false;

        // 2-opt: a stretch turned round, if every group in it turns; a
        // group alone, turned where it is.
        for first in 0..count {
            for last in first..count {
                if !turns(&tour[first..=last]) {
                    break;
                }

                let prev = before(&tour, first);
                let next = after(&tour, last + 1);
                let was = prev.distance(entry(&tour[first]))
                    + next.map_or(0.0, |next| exit(&tour[last]).distance(next));
                let will = prev.distance(exit(&tour[last]))
                    + next.map_or(0.0, |next| entry(&tour[first]).distance(next));

                if will < was - GAIN {
                    tour[first..=last].reverse();

                    for step in &mut tour[first..=last] {
                        step.1 = !step.1;
                    }

                    improved = true;
                }
            }
        }

        // Or-opt: a run of one to three moved elsewhere, either way round.
        for length in 1..=3 {
            let mut first = 0;

            while first + length <= count {
                let run: Vec<(usize, bool)> = tour[first..first + length].to_vec();
                let prev = before(&tour, first);
                let next = after(&tour, first + length);
                let taken = prev.distance(entry(&run[0]))
                    + next.map_or(0.0, |next| exit(&run[length - 1]).distance(next))
                    - next.map_or(0.0, |next| prev.distance(next));
                let mut rest = tour.clone();
                rest.drain(first..first + length);

                let turned: Vec<(usize, bool)> = run
                    .iter()
                    .rev()
                    .map(|&(index, turned)| (index, !turned))
                    .collect();
                let ways = if turns(&run) {
                    vec![run.clone(), turned]
                } else {
                    vec![run.clone()]
                };
                let mut best: Option<(f64, usize, Tour)> = None;

                for at in 0..=rest.len() {
                    if at == first {
                        continue;
                    }

                    let prev = before(&rest, at);
                    let next = after(&rest, at);

                    for way in &ways {
                        let added = prev.distance(entry(&way[0]))
                            + next.map_or(0.0, |next| exit(&way[length - 1]).distance(next))
                            - next.map_or(0.0, |next| prev.distance(next));

                        if added < taken - GAIN
                            && best.as_ref().is_none_or(|(cost, ..)| added < *cost)
                        {
                            best = Some((added, at, way.clone()));
                        }
                    }
                }

                match best {
                    Some((_, at, way)) => {
                        rest.splice(at..at, way);
                        tour = rest;
                        improved = true;
                    }
                    None => first += 1,
                }
            }
        }

        if !improved {
            break;
        }
    }

    let mut groups: Vec<Option<Group>> = cluster.drain(..).map(Some).collect();

    tour.into_iter()
        .map(|(index, turned)| {
            let mut group = groups[index].take().expect("Each group once");

            if turned {
                group.turn();
            }

            group
        })
        .collect()
}

/// Starts each loop in `cluster`, drawn by a pen starting at `from`, where
/// the pen comes to it and goes on from it soonest.
fn reseam(cluster: &mut [Group], from: Point<i32>) {
    let mut pen = from;

    for index in 0..cluster.len() {
        let next = match cluster[index].strokes.get(1) {
            Some(stroke) => Some(stroke.start()),
            None => cluster.get(index + 1).map(Group::start),
        };
        let first = &mut cluster[index].strokes[0];

        if first.form == Form::Loop {
            let cost = |pixel: Point<i32>| {
                place(pen).distance(place(pixel))
                    + next.map_or(0.0, |next| place(pixel).distance(place(next)))
            };
            let seam = (0..first.pixels.len())
                .min_by(|&a, &b| {
                    cost(first.pixels[a])
                        .total_cmp(&cost(first.pixels[b]))
                        .then(a.cmp(&b))
                })
                .unwrap_or(0);

            first.rotate(seam);
        }

        pen = cluster[index].end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::Tone;

    fn line(mark: usize, from: (i32, i32), to: (i32, i32)) -> Stroke {
        let pixels =
            quadrille::draw::shape::line(Point::new(from.0, from.1), Point::new(to.0, to.1));

        Stroke {
            lit: vec![true; pixels.len()],
            pixels,
            tone: Tone::Ink,
            form: Form::Line,
            mark,
            piece: 0,
        }
    }

    const EDGE: Meta = Meta {
        pass: Pass::Edges,
        line: Some(Line::Outline),
        curved: false,
        part: None,
        pane: 0,
        cuts: None,
        item: None,
        centre: None,
        compass: None,
        label: false,
    };

    /// The pen goes to the nearest line next, and draws it from its nearer
    /// end.
    #[test]
    fn the_pen_goes_to_the_nearest_line_and_its_nearer_end() {
        let strokes = vec![
            line(0, (100, 0), (100, 50)),
            line(1, (0, 10), (40, 10)),
            line(2, (90, 60), (45, 12)),
        ];
        let ordered = order(strokes, &[EDGE; 3], Point::new(0, 0), None);
        let marks: Vec<usize> = ordered.strokes.iter().map(|stroke| stroke.mark).collect();

        assert_eq!(marks, [1, 2, 0]);
        assert_eq!(ordered.strokes[1].start(), Point::new(45, 12));
        assert_eq!(ordered.strokes[2].start(), Point::new(100, 50));
        assert_eq!(ordered.gaps, [Gap::Stage, Gap::Group, Gap::Group]);
    }

    /// A drafting office's order: the skeleton before the bodies, the
    /// bodies part by part, the balloons last in the order of their items.
    #[test]
    fn a_drafting_office_draws_the_skeleton_then_part_by_part() {
        let meta = |pass, line, part, item| Meta {
            pass,
            line,
            part,
            item,
            ..EDGE
        };
        let marks = [
            meta(Pass::Balloons, None, Some(1), Some(2)),
            meta(Pass::Edges, Some(Line::Outline), Some(1), None),
            meta(Pass::Balloons, None, Some(0), Some(1)),
            meta(Pass::Edges, Some(Line::Outline), Some(0), None),
            meta(Pass::Construction, Some(Line::Centre), Some(1), None),
        ];
        let strokes = (0..marks.len())
            .map(|mark| line(mark, (mark as i32 * 10, 0), (mark as i32 * 10, 5)))
            .collect();
        let ordered = order(strokes, &marks, Point::new(0, 0), None);
        let drawn: Vec<usize> = ordered.strokes.iter().map(|stroke| stroke.mark).collect();

        assert_eq!(drawn, [4, 3, 1, 2, 0]);
        assert_eq!(
            ordered.gaps,
            [
                Gap::Stage,
                Gap::Stage,
                Gap::Cluster,
                Gap::Stage,
                Gap::Cluster
            ]
        );
    }

    /// The pen's journey from `from` through `strokes` and on to `to`.
    fn journey(strokes: &[Stroke], from: Point<i32>, to: Option<Point<i32>>) -> f64 {
        let mut pen = place(from);
        let mut length = 0.0;

        for stroke in strokes {
            length += pen.distance(place(stroke.start()));
            pen = place(stroke.end());
        }

        length + to.map_or(0.0, |to| pen.distance(place(to)))
    }

    /// Lines scattered as a pseudo-random walk would leave them, the same
    /// every time.
    fn scattered(count: usize) -> Vec<Stroke> {
        let mut seed: u64 = 7;
        let mut next = move || {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            ((seed >> 33) % 400) as i32
        };

        (0..count)
            .map(|mark| {
                let from = (next(), next());
                let to = (from.0 + next() / 20 - 10, from.1 + next() / 20 - 10);

                line(mark, from, to)
            })
            .collect()
    }

    /// Polishing the nearest-first order shortens the pen's journey, and
    /// keeps every stroke, each drawn once.
    #[test]
    fn polishing_shortens_the_journey_and_keeps_every_stroke() {
        let home = Point::new(0, 400);
        let mut pen = home;
        let greedy = nearest(group(scattered(120), &[EDGE; 120]), &mut pen);
        let strokes = |groups: &[Group]| -> Vec<Stroke> {
            groups
                .iter()
                .flat_map(|group| group.strokes.iter().cloned())
                .collect()
        };
        let before = journey(&strokes(&greedy), home, None);
        let polished = strokes(&polish(greedy, home));
        let mut marks: Vec<usize> = polished.iter().map(|stroke| stroke.mark).collect();

        marks.sort_unstable();
        assert_eq!(marks, (0..120).collect::<Vec<_>>());
        assert!(
            journey(&polished, home, None) < before * 0.95,
            "{} against {before}",
            journey(&polished, home, None)
        );
    }

    /// A line is drawn from its nearer end, and a stroke of a letter only
    /// the way it is written, however near its end is.
    #[test]
    fn lines_are_turned_round_and_letters_never() {
        for (form, start) in [(Form::Line, (6, 10)), (Form::Glyph, (0, 10))] {
            let mut stroke = line(0, (0, 10), (6, 10));

            stroke.form = form;

            let ordered = order(vec![stroke], &[EDGE], Point::new(7, 10), None);

            assert_eq!(
                ordered.strokes[0].start(),
                Point::new(start.0, start.1),
                "{form:?}"
            );
        }
    }

    /// A closed square loop of side `side` with its top left at `at`.
    fn square(mark: usize, at: (i32, i32), side: i32) -> Stroke {
        let (x, y) = at;
        let mut pixels = quadrille::draw::shape::polyline(&[
            Point::new(x, y),
            Point::new(x + side, y),
            Point::new(x + side, y + side),
            Point::new(x, y + side),
            Point::new(x, y),
        ]);

        pixels.dedup();
        pixels.pop();

        Stroke {
            lit: vec![true; pixels.len()],
            pixels,
            tone: Tone::Ink,
            form: Form::Loop,
            mark,
            piece: 0,
        }
    }

    /// A circle round `centre` of `radius`, a loop from its top.
    fn circle(mark: usize, centre: (i32, i32), radius: i32) -> Stroke {
        let pixels = crate::draft::raster::ordered_circle(Point::new(centre.0, centre.1), radius);

        Stroke {
            lit: vec![true; pixels.len()],
            pixels,
            tone: Tone::Ink,
            form: Form::Loop,
            mark,
            piece: 0,
        }
    }

    /// The marks of `ordered`'s strokes, each once, in the order drawn.
    fn drawn(ordered: &Ordered) -> Vec<usize> {
        let mut marks: Vec<usize> = ordered.strokes.iter().map(|stroke| stroke.mark).collect();
        marks.dedup();
        marks
    }

    /// A drafting office lays down the axes of every view first, the
    /// longest first, then the rest of the skeleton nearest first.
    #[test]
    fn the_axes_go_down_longest_first() {
        let centre = Meta {
            pass: Pass::Construction,
            line: Some(Line::Centre),
            ..EDGE
        };
        let strokes = vec![
            line(0, (10, 10), (30, 10)),
            line(1, (0, 100), (400, 100)),
            line(2, (200, 0), (200, 250)),
            circle(3, (200, 100), 60),
            line(4, (350, 0), (350, 210)),
        ];
        let ordered = order(strokes, &[centre; 5], Point::new(0, 0), None);

        assert_eq!(drawn(&ordered)[..3], [1, 2, 4]);
        assert_eq!(ordered.gaps[1..], [Gap::Group; 4]);
    }

    /// Circles round one centre are drawn one after another, smallest
    /// first, as a compass is opened out; a circle round another centre
    /// keeps its place.
    #[test]
    fn concentric_circles_are_drawn_smallest_first() {
        let round = |centre: (i32, i32)| Meta {
            curved: true,
            part: Some(0),
            centre: Some(Point::new(centre.0, centre.1)),
            ..EDGE
        };
        let strokes = vec![
            circle(0, (100, 100), 40),
            circle(1, (100, 100), 12),
            circle(2, (300, 100), 20),
            circle(3, (100, 100), 25),
        ];
        let marks = [
            round((100, 100)),
            round((100, 100)),
            round((300, 100)),
            round((100, 100)),
        ];
        let ordered = order(strokes, &marks, Point::new(0, 100), None);
        let marks = drawn(&ordered);
        let first = marks
            .iter()
            .position(|&mark| mark == 1)
            .expect("The smallest");

        assert_eq!(marks[first..first + 3], [1, 3, 0]);
    }

    /// A balloon is drawn from the dot on its part: the dot, the leader out
    /// from it, the circle from where the leader meets it, the number.
    #[test]
    fn a_balloon_is_drawn_from_its_dot_outwards() {
        let target = Point::new(40, 40);
        let centre = Point::new(100, 20);
        let mut leader = line(0, (93, 22), (40, 40));
        let mut dot = line(0, (40, 40), (40, 40));
        let mut ring = circle(0, (100, 20), 7);
        let mut number = line(0, (99, 17), (99, 23));

        dot.form = Form::Touch(target);
        dot.pixels = vec![target];
        dot.lit = vec![true];
        number.form = Form::Glyph;
        (leader.piece, dot.piece, ring.piece, number.piece) = (0, 1, 3, 4);

        let balloon = Meta {
            pass: Pass::Balloons,
            item: Some(1),
            ..EDGE
        };
        let ordered = order(
            vec![leader, dot, ring, number],
            &[balloon],
            Point::new(0, 0),
            None,
        );
        let pieces: Vec<usize> = ordered.strokes.iter().map(|stroke| stroke.piece).collect();
        let leader = &ordered.strokes[1];
        let ring = &ordered.strokes[2];

        assert_eq!(pieces, [1, 0, 3, 4]);
        assert_eq!(leader.start(), target);
        assert!(distance(ring.start(), leader.end()) <= 2);
        assert!(distance(ring.start(), centre) > distance(leader.end(), centre) - 2);
    }

    /// A loop goes round the way the pen was heading as it came to it.
    #[test]
    fn a_loop_goes_round_the_way_the_pen_is_heading() {
        for (from, clockwise) in [((0, 60), true), ((200, 60), false)] {
            // Coming to the circle's top from the left, or the right.
            let mut strokes = vec![circle(0, (100, 100), 40)];

            onward(&mut strokes, Point::new(from.0, from.1));

            let second = strokes[0].pixels[3];

            assert_eq!(strokes[0].pixels[0], Point::new(100, 60));
            assert_eq!(second.x > 100, clockwise, "from {from:?}");
        }
    }

    /// A diagram grows from its first part along what touches: each part
    /// drawn whole, its own wiring with its lines and its lettering last,
    /// straight after the wire that leads to it; a beat before each part
    /// and what leads to it; a line nothing reaches after the walk.
    #[test]
    fn a_diagram_grows_from_its_first_part_along_its_wires() {
        let boxed = |part| Meta {
            curved: true,
            part: Some(part),
            ..EDGE
        };
        let label = |part| Meta {
            pass: Pass::Annotation,
            line: None,
            part: Some(part),
            label: true,
            ..EDGE
        };
        let lettered = |mark, at: (i32, i32)| {
            let mut stroke = line(mark, at, (at.0 + 4, at.1));
            stroke.form = Form::Glyph;
            stroke
        };
        let marks = [
            boxed(0),
            label(0),
            EDGE,
            boxed(1),
            label(1),
            EDGE,
            boxed(2),
            boxed(2),
            EDGE,
            label(2),
            EDGE,
        ];
        let strokes = vec![
            square(0, (0, 0), 20),
            lettered(1, (5, 8)),
            line(2, (21, 10), (59, 10)),
            square(3, (60, 0), 20),
            lettered(4, (65, 8)),
            line(5, (70, 21), (70, 59)),
            square(6, (60, 60), 20),
            square(7, (120, 60), 20),
            line(8, (81, 70), (119, 70)),
            lettered(9, (65, 68)),
            line(10, (300, 300), (320, 300)),
        ];
        let touching: Touching = [(0, 2), (2, 3), (3, 5), (5, 6), (6, 8), (7, 8)]
            .into_iter()
            .collect();
        let ordered = order(strokes, &marks, Point::new(0, 0), Some(&touching));

        assert_eq!(drawn(&ordered), [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(
            ordered.gaps,
            [
                Gap::Stage,
                Gap::Group,
                Gap::Cluster,
                Gap::Group,
                Gap::Group,
                Gap::Cluster,
                Gap::Group,
                Gap::Group,
                Gap::Group,
                Gap::Group,
                Gap::Cluster
            ]
        );
    }
}
