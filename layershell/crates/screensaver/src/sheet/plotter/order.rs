//! The order the pen draws its strokes in.
//!
//! Strokes go in groups, the strokes of one mark, which keep their order:
//! a dimension's extension lines, then its line, its arrowheads and its
//! value. The style's [`Order`] gives each group a key, and the groups are
//! drawn in the order of their keys. Groups with the same key are drawn
//! nearest first, if the style says so, from wherever the pen is: a line
//! is turned round, an area's lining run backwards or a loop started where
//! that is nearer. The rest of the key keeps clusters apart for the
//! motion's beats: a stage from the next, a part or a view from another.
use iced_core::Point;

use crate::draft::{Line, Pass, Tone};

use super::strokes::{Form, Stroke};
use super::style::{Order, PlotStyle};

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
    pub groups: usize,
}

/// A group's place in the order: its stage, the cluster within the stage,
/// and what orders the cluster within.
type Key = (u8, u32, u32, u32);

/// The strokes of one mark (of one pen, in a pen-sorted plot), in order.
struct Group {
    strokes: Vec<Stroke>,
    key: Key,
    /// Whether it may be drawn backwards whole: an area's lining or rows.
    backwards: bool,
}

/// How far apart, along a loop, the pen may start it.
const SEAMS: usize = 16;

/// `strokes`, in the order made, put in `style`'s order for a pen
/// starting at `from`; `marks` says what their marks are.
pub fn order(style: &PlotStyle, strokes: Vec<Stroke>, marks: &[Meta], from: Point<i32>) -> Ordered {
    let mut groups = group(style, strokes, marks);

    // A stable sort: groups with the same key stay in the order made.
    groups.sort_by_key(|group| group.key);

    let count = groups.len();
    let mut ordered = Ordered {
        strokes: Vec::new(),
        gaps: Vec::new(),
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

        if style.nearest {
            cluster = nearest(cluster, &mut pen);
        }

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
                ordered.strokes.push(stroke);
            }
        }

        last = Some(key);
    }

    ordered
}

/// The strokes in groups, each with its key.
fn group(style: &PlotStyle, strokes: Vec<Stroke>, marks: &[Meta]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();

    for stroke in strokes {
        let meta = &marks[stroke.mark];
        let split = style.order == Order::Pens;
        let found = groups
            .iter_mut()
            .rev()
            .take_while(|group| group.strokes[0].mark == stroke.mark)
            .find(|group| !split || group.strokes[0].tone == stroke.tone);

        match found {
            Some(group) => {
                group.backwards &= group.strokes[0].piece == stroke.piece;
                group.strokes.push(stroke);
            }
            None => groups.push(Group {
                key: key(style.order, meta, stroke.tone),
                backwards: meta.pass == Pass::Areas,
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

/// Where a mark of `meta`, in `tone`, goes in `order`.
fn key(order: Order, meta: &Meta, tone: Tone) -> Key {
    // Centre and phantom lines, and construction belonging to no part: the
    // skeleton every view is built on.
    let skeleton =
        meta.pass == Pass::Construction && !(meta.line == Some(Line::Thin) && meta.part.is_some());

    match order {
        Order::Passes => (meta.pass as u8, 0, 0, 0),
        Order::Pens => (pen(tone), 0, 0, 0),
        Order::Stages => match meta.pass {
            _ if skeleton => (0, 0, 0, 0),
            Pass::Construction | Pass::Edges | Pass::Hidden => {
                let sub = match meta.pass {
                    Pass::Hidden => 3,
                    _ if meta.curved => 1,
                    _ => 2,
                };

                (1, meta.pane as u32, rank(meta.part) + 1, sub)
            }
            Pass::Areas => (2, rank(meta.part), 0, 0),
            Pass::Annotation => match meta.cuts {
                // A cutting plane before the section it cuts.
                Some(section) => (1, section as u32, 0, 0),
                None => (3, meta.pane as u32, 0, 0),
            },
            Pass::Traces => (4, 0, 0, 0),
            Pass::Balloons => (5, meta.item.unwrap_or(0) as u32, 0, 0),
        },
        Order::Parts => {
            let part = match meta.pass {
                Pass::Balloons => meta.item.map(|item| item.saturating_sub(1)),
                _ => meta.part,
            };
            let sub = match meta.pass {
                Pass::Construction | Pass::Edges => 0,
                Pass::Hidden => 1,
                Pass::Areas => 2,
                Pass::Traces | Pass::Annotation => 3,
                Pass::Balloons => 4,
            };

            match part {
                _ if skeleton => (0, 0, 0, 0),
                Some(part) => (1, rank(Some(part)), 0, sub),
                // What belongs to no part: its line work before the parts,
                // what is said about it after them.
                None if sub >= 3 => (2, meta.pane as u32, 0, sub),
                None => (1, 0, 0, sub),
            }
        }
    }
}

/// A pen's place on a carousel: the lightest first, which here is also the
/// construction first.
pub fn pen(tone: Tone) -> u8 {
    match tone {
        Tone::Faint => 0,
        Tone::Line => 1,
        Tone::Muted => 2,
        Tone::Ink => 3,
        Tone::Live => 4,
        Tone::Caution => 5,
        Tone::Accent => 6,
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

#[cfg(test)]
mod tests {
    use super::*;

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
        let style = PlotStyle {
            order: Order::Passes,
            nearest: true,
            ..PlotStyle::DRAFTING
        };
        let ordered = order(&style, strokes, &[EDGE; 3], Point::new(0, 0));
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
        let ordered = order(&PlotStyle::DRAFTING, strokes, &marks, Point::new(0, 0));
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
}
