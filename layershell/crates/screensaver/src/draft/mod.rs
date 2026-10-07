//! Technical drawing in a model's own units.
//!
//! A subject draws on a [`Draft`] with the verbs of a drafting office (lines
//! of each type, hatching, dimensions, notes, balloons), in millimetres or
//! whatever its unit is, `y` up. The draft only records [`Mark`]s. The sheet
//! decides where they land: [`raster`] puts them on the pixel grid through a
//! [`Projection`](raster::Projection), once for the view and again, larger,
//! for a detail, and plots them stroke by stroke.
pub mod geom;
pub mod letters;
pub mod place;
pub mod raster;
pub mod scale;

pub use geom::{Extent, Turn, V2, arc_points, polar, v};

use quadrille::draw::Anchor;

/// What a stroke stands for, which decides its pattern and colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// A visible edge.
    Outline,
    /// Construction: extension lines, leaders, a reference.
    Thin,
    /// An edge behind something: dashed.
    Hidden,
    /// An axis, a pitch circle, a plane of symmetry: long dash and dot.
    Centre,
    /// Where something else is or will be: long dash and two dots.
    Phantom,
    /// Something live: a signal, a ray, a body in flight.
    Trace,
    /// The way a point travels: dotted.
    Path,
}

/// The palette role a mark is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Ink,
    Line,
    Muted,
    Faint,
    Accent,
    Live,
    Caution,
}

impl Line {
    pub fn tone(self) -> Tone {
        match self {
            Self::Outline => Tone::Ink,
            Self::Thin | Self::Hidden => Tone::Line,
            Self::Centre | Self::Phantom => Tone::Faint,
            Self::Trace | Self::Path => Tone::Live,
        }
    }
}

/// How an area is filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    /// Section lines: the cut face of a solid.
    Hatch,
    /// Section lines the other way, for the second of two parts that touch.
    CrossHatch,
    /// Every pixel.
    Solid,
    /// A 4 × 4 ordered dither with `n` of 16 pixels lit.
    Tint(u8),
}

/// Which way a dimension measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Across, along the sheet's rows.
    Horizontal,
    /// Up, along the sheet's columns.
    Vertical,
}

/// What a dimension measures.
#[derive(Debug, Clone, PartialEq)]
pub enum Measure {
    /// The distance between `a` and `b` along `axis`. The dimension line is
    /// `offset` model units clear of the points: above or right when
    /// positive, below or left when negative.
    Linear {
        a: V2,
        b: V2,
        axis: Axis,
        offset: f32,
    },
    /// A diameter or radius: an arrow onto the circle at `angle`, a leader
    /// `reach` pixels out and a shelf with the value.
    Radial {
        centre: V2,
        radius: f32,
        angle: f32,
        reach: i32,
    },
    /// The angle at `vertex` from `from` to `to` (counter-clockwise), as an
    /// arc `radius` pixels out.
    Angle {
        vertex: V2,
        from: f32,
        to: f32,
        radius: i32,
    },
}

/// Where an annotation goes, from what it points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// Wherever the sheet finds room for it in the main view, clear of the
    /// drawing and of the other annotations; left out if there is none.
    Auto,
    /// This many pixels across and down from its target.
    Offset(i32, i32),
}

impl From<(i32, i32)> for Placement {
    fn from((x, y): (i32, i32)) -> Self {
        Self::Offset(x, y)
    }
}

/// A mark's content.
#[derive(Debug, Clone, PartialEq)]
pub enum Ink {
    /// A line of some type.
    Stroke { shape: Shape, line: Line },
    /// A straight line of some type ending in a solid arrowhead at `to`.
    Arrow { from: V2, to: V2, line: Line },
    /// An area, filled.
    Area { contour: Vec<V2>, fill: Fill },
    /// Text at a point of the model, nudged by whole pixels.
    Label {
        at: V2,
        nudge: (i32, i32),
        text: String,
        anchor: Anchor,
    },
    /// A dimension and its value.
    Dimension { measure: Measure, text: String },
    /// A leader from `target` to an elbow, then a shelf carrying `text`.
    Note {
        target: V2,
        elbow: Placement,
        text: String,
    },
    /// A part's item number in a circle, with a leader to the part.
    Balloon {
        item: usize,
        target: V2,
        offset: Placement,
    },
    /// A square dot on the pixel grid, `size` pixels across.
    Dot { at: V2, size: i32 },
}

/// The geometry of a stroke.
#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Polyline {
        points: Vec<V2>,
        closed: bool,
    },
    Circle {
        centre: V2,
        radius: f32,
    },
    /// From `start`, sweeping `sweep` radians, counter-clockwise when positive.
    Arc {
        centre: V2,
        radius: f32,
        start: f32,
        sweep: f32,
    },
}

/// Which views a mark appears in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Geometry: the main view and any detail of it.
    Everywhere,
    /// What the main view says: its dimensions, notes and balloons.
    Main,
    /// What only a magnified detail can say legibly.
    Detail,
}

/// One recorded mark.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub ink: Ink,
    pub tone: Tone,
    /// The part it documents, by its index in the card's parts list.
    pub part: Option<usize>,
    /// Whether it moves while the subject runs, and so is drawn every frame.
    pub moving: bool,
    pub scope: Scope,
}

/// The order a plotter draws in: the construction first, then the edges,
/// then what the drawing says about them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Pass {
    Construction,
    Edges,
    Hidden,
    Areas,
    Traces,
    Annotation,
    Balloons,
}

impl Mark {
    /// Whether the mark belongs in a main view (`false`: in a detail).
    pub fn shown_in(&self, main: bool) -> bool {
        match self.scope {
            Scope::Everywhere => true,
            Scope::Main => main,
            Scope::Detail => !main,
        }
    }

    pub fn pass(&self) -> Pass {
        match &self.ink {
            Ink::Stroke { line, .. } | Ink::Arrow { line, .. } => match line {
                Line::Centre | Line::Phantom | Line::Thin => Pass::Construction,
                Line::Outline => Pass::Edges,
                Line::Hidden => Pass::Hidden,
                Line::Trace | Line::Path => Pass::Traces,
            },
            Ink::Area { .. } => Pass::Areas,
            Ink::Dot { .. } => Pass::Traces,
            Ink::Label { .. } | Ink::Dimension { .. } | Ink::Note { .. } => Pass::Annotation,
            Ink::Balloon { .. } => Pass::Balloons,
        }
    }
}

/// The marks of one drawing, recorded in the order they were made.
#[derive(Debug, Default)]
pub struct Draft {
    marks: Vec<Mark>,
    part: Option<usize>,
    moving: bool,
    detail: bool,
}

/// The mark just made, to say more about it.
pub struct Made<'a>(&'a mut Mark);

impl Made<'_> {
    /// Draws it in `tone` instead of its line's.
    pub fn tone(self, tone: Tone) -> Self {
        self.0.tone = tone;
        self
    }

    /// Sets what a dimension, label or note says instead of what it measures.
    pub fn text(self, text: impl Into<String>) -> Self {
        match &mut self.0.ink {
            Ink::Dimension { text: old, .. }
            | Ink::Label { text: old, .. }
            | Ink::Note { text: old, .. } => *old = text.into(),
            _ => {}
        }
        self
    }

    /// Places a label by `anchor` instead of its centre.
    pub fn anchor(self, anchor: Anchor) -> Self {
        if let Ink::Label { anchor: old, .. } = &mut self.0.ink {
            *old = anchor;
        }
        self
    }

    /// Moves a label by whole pixels.
    pub fn nudge(self, dx: i32, dy: i32) -> Self {
        if let Ink::Label { nudge, .. } = &mut self.0.ink {
            *nudge = (dx, dy);
        }
        self
    }
}

impl Draft {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn marks(&self) -> &[Mark] {
        &self.marks
    }

    pub fn marks_mut(&mut self) -> &mut [Mark] {
        &mut self.marks
    }

    /// Records what `draw` makes as the drawing of part `index`.
    pub fn part(&mut self, index: usize, draw: impl FnOnce(&mut Self)) {
        let outer = self.part.replace(index);
        draw(self);
        self.part = outer;
    }

    /// Records what `draw` makes as moving: drawn afresh on every frame.
    pub fn moving(&mut self, draw: impl FnOnce(&mut Self)) {
        let outer = std::mem::replace(&mut self.moving, true);
        draw(self);
        self.moving = outer;
    }

    /// Records what `draw` makes for detail views only: the dimensions and
    /// notes of features too small to letter in the main view.
    pub fn in_detail(&mut self, draw: impl FnOnce(&mut Self)) {
        let outer = std::mem::replace(&mut self.detail, true);
        draw(self);
        self.detail = outer;
    }

    fn push(&mut self, ink: Ink, tone: Tone) -> Made<'_> {
        let annotation = matches!(
            ink,
            Ink::Label { .. } | Ink::Dimension { .. } | Ink::Note { .. } | Ink::Balloon { .. }
        );
        let scope = if self.detail {
            Scope::Detail
        } else if annotation {
            Scope::Main
        } else {
            Scope::Everywhere
        };

        self.marks.push(Mark {
            ink,
            tone,
            part: self.part,
            moving: self.moving,
            scope,
        });

        Made(self.marks.last_mut().expect("Just pushed"))
    }

    pub fn stroke(&mut self, shape: Shape, line: Line) -> Made<'_> {
        self.push(Ink::Stroke { shape, line }, line.tone())
    }

    pub fn line(&mut self, a: V2, b: V2, line: Line) -> Made<'_> {
        self.polyline(&[a, b], line)
    }

    pub fn polyline(&mut self, points: &[V2], line: Line) -> Made<'_> {
        self.stroke(
            Shape::Polyline {
                points: points.to_vec(),
                closed: false,
            },
            line,
        )
    }

    pub fn polygon(&mut self, points: &[V2], line: Line) -> Made<'_> {
        self.stroke(
            Shape::Polyline {
                points: points.to_vec(),
                closed: true,
            },
            line,
        )
    }

    /// The rectangle between two corners.
    pub fn rect(&mut self, a: V2, b: V2, line: Line) -> Made<'_> {
        self.polygon(&[a, v(b.x, a.y), b, v(a.x, b.y)], line)
    }

    pub fn circle(&mut self, centre: V2, radius: f32, line: Line) -> Made<'_> {
        self.stroke(Shape::Circle { centre, radius }, line)
    }

    pub fn arc(&mut self, centre: V2, radius: f32, start: f32, sweep: f32, line: Line) -> Made<'_> {
        self.stroke(
            Shape::Arc {
                centre,
                radius,
                start,
                sweep,
            },
            line,
        )
    }

    pub fn arrow(&mut self, from: V2, to: V2, line: Line) -> Made<'_> {
        self.push(Ink::Arrow { from, to, line }, line.tone())
    }

    /// Section lining over the cut face `contour`, its outline left to the
    /// caller.
    pub fn hatch(&mut self, contour: &[V2]) -> Made<'_> {
        self.area(contour, Fill::Hatch)
    }

    pub fn area(&mut self, contour: &[V2], fill: Fill) -> Made<'_> {
        self.push(
            Ink::Area {
                contour: contour.to_vec(),
                fill,
            },
            Tone::Line,
        )
    }

    /// The centre lines of a circle: a cross reaching `reach` past it.
    pub fn centre_mark(&mut self, centre: V2, radius: f32, reach: f32) {
        let arm = radius + reach;

        self.line(centre - v(arm, 0.0), centre + v(arm, 0.0), Line::Centre);
        self.line(centre - v(0.0, arm), centre + v(0.0, arm), Line::Centre);
    }

    pub fn label(&mut self, at: V2, text: impl Into<String>) -> Made<'_> {
        self.push(
            Ink::Label {
                at,
                nudge: (0, 0),
                text: text.into(),
                anchor: Anchor::CENTRE,
            },
            Tone::Muted,
        )
    }

    pub fn note(
        &mut self,
        target: V2,
        elbow: impl Into<Placement>,
        text: impl Into<String>,
    ) -> Made<'_> {
        self.push(
            Ink::Note {
                target,
                elbow: elbow.into(),
                text: text.into(),
            },
            Tone::Muted,
        )
    }

    /// A balloon numbering part `index` (shown one-based).
    pub fn balloon(&mut self, index: usize, target: V2, offset: impl Into<Placement>) -> Made<'_> {
        self.push(
            Ink::Balloon {
                item: index + 1,
                target,
                offset: offset.into(),
            },
            Tone::Ink,
        )
    }

    pub fn dot(&mut self, at: V2, size: i32) -> Made<'_> {
        self.push(Ink::Dot { at, size }, Tone::Live)
    }

    fn dimension(&mut self, measure: Measure, text: String) -> Made<'_> {
        self.push(Ink::Dimension { measure, text }, Tone::Ink)
    }

    /// The horizontal distance between `a` and `b`.
    pub fn dim_h(&mut self, a: V2, b: V2, offset: f32) -> Made<'_> {
        let text = number((b.x - a.x).abs());
        let measure = Measure::Linear {
            a,
            b,
            axis: Axis::Horizontal,
            offset,
        };

        self.dimension(measure, text)
    }

    /// The vertical distance between `a` and `b`.
    pub fn dim_v(&mut self, a: V2, b: V2, offset: f32) -> Made<'_> {
        let text = number((b.y - a.y).abs());
        let measure = Measure::Linear {
            a,
            b,
            axis: Axis::Vertical,
            offset,
        };

        self.dimension(measure, text)
    }

    pub fn dim_diameter(&mut self, centre: V2, radius: f32, angle: f32, reach: i32) -> Made<'_> {
        let text = format!("Ø{}", number(2.0 * radius));
        let measure = Measure::Radial {
            centre,
            radius,
            angle,
            reach,
        };

        self.dimension(measure, text)
    }

    pub fn dim_radius(&mut self, centre: V2, radius: f32, angle: f32, reach: i32) -> Made<'_> {
        let text = format!("R{}", number(radius));
        let measure = Measure::Radial {
            centre,
            radius,
            angle,
            reach,
        };

        self.dimension(measure, text)
    }

    /// The angle at `vertex` from `from` counter-clockwise to `to`.
    pub fn dim_angle(&mut self, vertex: V2, from: f32, to: f32, radius: i32) -> Made<'_> {
        let degrees = geom::wrap(to - from).to_degrees();
        let text = format!("{}°", number(degrees));
        let measure = Measure::Angle {
            vertex,
            from,
            to,
            radius,
        };

        self.dimension(measure, text)
    }
}

/// A measurement as a drawing writes it: whole when it is, to a tenth
/// otherwise, and to a hundredth below one.
pub fn number(value: f32) -> String {
    let rounded = (value * 100.0).round() / 100.0;

    if (rounded - rounded.round()).abs() < 0.005 {
        format!("{}", rounded.round() as i64)
    } else if rounded.abs() < 1.0 {
        format!("{rounded:.2}")
    } else {
        format!("{rounded:.1}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurements_are_written_plainly() {
        assert_eq!(number(48.0), "48");
        assert_eq!(number(47.5), "47.5");
        assert_eq!(number(0.25), "0.25");
        assert_eq!(number(119.999), "120");
    }

    #[test]
    fn marks_remember_their_part_and_motion() {
        let mut draft = Draft::new();

        draft.part(2, |draft| {
            draft.moving(|draft| {
                draft.circle(V2::ZERO, 4.0, Line::Outline);
            });
            draft.line(V2::ZERO, v(1.0, 0.0), Line::Centre);
        });
        draft.dim_h(V2::ZERO, v(12.0, 3.0), 10.0);
        draft.in_detail(|draft| {
            draft.label(V2::ZERO, "A");
        });

        let marks = draft.marks();

        assert_eq!((marks[0].part, marks[0].moving), (Some(2), true));
        assert_eq!((marks[1].part, marks[1].moving), (Some(2), false));
        assert_eq!((marks[2].part, marks[2].moving), (None, false));
        assert_eq!(marks[1].pass(), Pass::Construction);
        assert!(matches!(&marks[2].ink, Ink::Dimension { text, .. } if text == "12"));
        assert_eq!(marks[0].scope, Scope::Everywhere);
        assert_eq!(marks[2].scope, Scope::Main);
        assert_eq!(marks[3].scope, Scope::Detail);
    }
}
