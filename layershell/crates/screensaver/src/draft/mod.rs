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

/// What a geometric tolerance controls (ISO 1101).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Characteristic {
    Position,
    Perpendicularity,
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
    /// A surface texture symbol standing on `at`, for a surface machined
    /// to `text`: `Ra 0.8`.
    Finish { at: V2, text: String },
    /// A datum feature: a filled triangle on `at`, its leader running
    /// `toward` a frame with the datum's letter.
    Datum { at: V2, toward: V2, letter: char },
    /// A feature control frame `offset` pixels from `at`, its leader
    /// arrowed onto `at`: what it controls, the tolerance zone and the
    /// datums it is measured from.
    Control {
        at: V2,
        offset: (i32, i32),
        characteristic: Characteristic,
        tolerance: String,
        datums: String,
    },
    /// Where the section that is view `view` is cut: a chain line from
    /// `from` to `to`, thick at its ends, where arrows point the way it is
    /// seen, `toward`, each by the section's letter. Drawn only when the
    /// view is.
    Section {
        view: usize,
        from: V2,
        to: V2,
        toward: V2,
        letter: char,
    },
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
    /// The subject's view it is drawn in, by its index among the other
    /// views; `None` for the front view.
    pub view: Option<usize>,
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
    /// Whether the mark is drawn in `view` (`None`: the front view).
    pub fn shown_in(&self, view: Option<usize>) -> bool {
        self.view == view && self.scope != Scope::Detail
    }

    /// Whether the mark is drawn in a detail, which magnifies the front
    /// view.
    pub fn magnified(&self) -> bool {
        self.view.is_none() && self.scope != Scope::Main
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
            Ink::Label { .. }
            | Ink::Dimension { .. }
            | Ink::Note { .. }
            | Ink::Section { .. }
            | Ink::Finish { .. }
            | Ink::Datum { .. }
            | Ink::Control { .. } => Pass::Annotation,
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
    view: Option<usize>,
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

    /// Gives a dimension its limits: `upper` and `lower` deviations from
    /// what it measures, written `±` when they are the same either way.
    pub fn tolerance(self, upper: f32, lower: f32) -> Self {
        if let Ink::Dimension { text, .. } = &mut self.0.ink {
            if (upper + lower).abs() < 1e-6 {
                text.push_str(&format!(" ±{}", deviation(upper)));
            } else {
                text.push_str(&format!(
                    " {}{} {}{}",
                    sign(upper),
                    deviation(upper.abs()),
                    sign(lower),
                    deviation(lower.abs())
                ));
            }
        }
        self
    }

    /// Gives a dimension a fit: an ISO 286 tolerance class, or a hole's and
    /// a shaft's for two parts that fit together, `H7/k6`.
    pub fn fit(self, classes: &str) -> Self {
        if let Ink::Dimension { text, .. } = &mut self.0.ink {
            text.push(' ');
            text.push_str(classes);
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

    /// Records what `draw` makes as the drawing of the subject's other view
    /// `index` (see [`Subject::views`](crate::subjects::Subject::views)).
    pub fn in_view(&mut self, index: usize, draw: impl FnOnce(&mut Self)) {
        let outer = self.view.replace(index);
        draw(self);
        self.view = outer;
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
            Ink::Label { .. }
                | Ink::Dimension { .. }
                | Ink::Note { .. }
                | Ink::Balloon { .. }
                | Ink::Section { .. }
                | Ink::Finish { .. }
                | Ink::Datum { .. }
                | Ink::Control { .. }
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
            view: self.view,
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

    /// Where section `letter`, the subject's view `view`, is cut: from
    /// `from` to `to`, seen looking `toward`.
    pub fn cutting_plane(
        &mut self,
        view: usize,
        from: V2,
        to: V2,
        toward: V2,
        letter: char,
    ) -> Made<'_> {
        self.push(
            Ink::Section {
                view,
                from,
                to,
                toward,
                letter,
            },
            Tone::Ink,
        )
    }

    /// The surface at `at` machined to `requirement`: `Ra 0.8`.
    pub fn finish(&mut self, at: V2, requirement: impl Into<String>) -> Made<'_> {
        self.push(
            Ink::Finish {
                at,
                text: requirement.into(),
            },
            Tone::Ink,
        )
    }

    /// The feature at `at` is datum `letter`, its frame `toward` it.
    pub fn datum(&mut self, at: V2, toward: V2, letter: char) -> Made<'_> {
        self.push(Ink::Datum { at, toward, letter }, Tone::Ink)
    }

    /// The feature at `at` held to `tolerance` in `characteristic`, from
    /// `datums`, its frame `offset` pixels away.
    pub fn control(
        &mut self,
        at: V2,
        offset: (i32, i32),
        characteristic: Characteristic,
        tolerance: impl Into<String>,
        datums: impl Into<String>,
    ) -> Made<'_> {
        self.push(
            Ink::Control {
                at,
                offset,
                characteristic,
                tolerance: tolerance.into(),
                datums: datums.into(),
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

/// The sign a deviation is written with: a minus, not a hyphen.
fn sign(deviation: f32) -> char {
    if deviation < 0.0 { '−' } else { '+' }
}

/// A deviation as limits are written: to the thousandth it is given to,
/// and no further.
fn deviation(value: f32) -> String {
    let written = format!("{value:.3}");

    written
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
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
    fn limits_and_fits_follow_the_value() {
        let mut draft = Draft::new();

        draft
            .dim_h(V2::ZERO, v(48.0, 0.0), 5.0)
            .tolerance(0.02, -0.02);
        draft
            .dim_h(V2::ZERO, v(20.0, 0.0), 5.0)
            .tolerance(0.0, -0.1);
        draft.dim_diameter(V2::ZERO, 4.0, 0.0, 10).fit("H7/k6");

        let texts: Vec<&str> = draft
            .marks()
            .iter()
            .filter_map(|mark| match &mark.ink {
                Ink::Dimension { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        assert_eq!(texts, ["48 ±0.02", "20 +0 −0.1", "Ø8 H7/k6"]);
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
