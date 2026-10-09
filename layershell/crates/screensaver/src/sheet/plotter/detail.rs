//! A part's detail, plotted by the pen as the sheet was.
//!
//! When a part is picked out, the pen comes from home and draws the circle
//! round it on the view, goes to the circle's letter and letters it, then
//! goes to the detail's window and plots the view there: its boundary
//! circle, then the magnified marks in the style's order, nearest first;
//! and goes home. A pen that does not start from home starts on the
//! circle, where it is nearest the letter, and one that does not go home
//! lifts off the view's last mark. Its work is planned once, from the
//! drawing as it stands when the part is picked out, and fitted to the
//! time the sheet gives the circle and the view.
//!
//! The part may move while it is plotted, and the detail with it, so the
//! plan keeps each stroke by what it draws, not by its pixels: a piece of a
//! mark, the mark known by its kind and which of the marks of that kind it
//! is, which stays the same while the marks move though how many there are
//! of another kind may not (teeth coming into mesh behind others). Each
//! frame draws the marks as they are then, each as far along as the plan
//! has drawn its stroke, in the order they are painted.
use std::collections::BTreeMap;
use std::ops::Range;

use iced_core::{Point, Rectangle};

use crate::draft::Pass;
use crate::draft::raster::{self, Inked, LETTERING, Piece, ROW_COST, Stipple};

use super::motion::{HOLD, Motions, Place, Planner, REST};
use super::pen::{Head, Pose};
use super::strokes::{self, Form, Stroke};
use super::style::{Circles, Order, PlotStyle};
use crate::sheet::timeline::{DETAIL_PLOT, MARK};

/// What a detail is plotted from, as it stands when its part is picked out.
pub struct Sketch {
    /// Where the pen comes from and goes back to.
    pub home: Point<i32>,
    /// The circle round the part on the view, and where its letter is.
    pub ring: Stroke,
    pub letter: Point<i32>,
    /// The view's boundary circle, and its pieces in the order they are
    /// painted, inside `clip`.
    pub window: Stroke,
    pub pieces: Vec<Sketched>,
    pub clip: Rectangle<i32>,
}

/// Which piece of a detail's view a stroke draws: the kind of its mark,
/// which of the marks of that kind it is, and which piece of the mark.
pub type Id = (Kind, usize, usize);

/// A kind of mark: its pass, its part, its pen and what it is.
pub type Kind = (Pass, Option<usize>, u8, u8);

/// A piece of a detail's view.
pub struct Sketched {
    pub id: Id,
    pub inked: Inked,
}

/// A detail's plot, worked out once.
pub struct Detail {
    /// The pen's work drawing the circle on the view, from home, and
    /// going to its letter...
    ring: Course,
    /// ...then lettering it and plotting the view: the boundary circle
    /// first, then its pieces; and going home.
    view: Course,
    /// The view's strokes by the pieces they draw, and the pieces by
    /// their strokes.
    places: BTreeMap<Id, usize>,
    ids: Vec<Option<Id>>,
}

/// Strokes and the pen's work drawing them.
struct Course {
    strokes: Vec<Stroke>,
    motions: Motions,
}

/// What the pen is drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drawing {
    /// The circle on the view.
    Ring,
    /// The view's boundary circle.
    Window,
    /// A piece of the view.
    Piece(Id),
}

/// Where a detail's plot is at a moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Now {
    /// Where the pen is, as planned: [`Drawing`] says where it is on the
    /// drawing as it is now.
    pub head: Option<Head>,
    /// How much of the circle on the view, and of the view's boundary
    /// circle, is drawn.
    pub ring: f64,
    pub window: f64,
    /// What the pen is drawing.
    pub drawing: Option<Drawing>,
    /// Where the view's plot is, once it is begun.
    view: Option<Place>,
}

/// How long the pen takes to letter the circle's letter.
const LETTERING_TIME: f64 = 1.0 / 30.0;

/// A stroke of the view, and the piece it draws.
struct Planned {
    stroke: Stroke,
    id: Option<Id>,
}

impl Detail {
    /// The plot of `sketch` in `style`, its lengths and speeds scaled by
    /// `scale`.
    pub fn new(style: &PlotStyle, sketch: Sketch, scale: f64) -> Self {
        let mut ring = sketch.ring;

        if !matches!(ring.form, Form::Circle(_)) {
            // Started where it is nearest the pen coming from home, or
            // else where the pen goes on to its letter soonest.
            let near = if style.from_home {
                sketch.home
            } else {
                sketch.letter
            };
            let seam = (0..ring.pixels.len())
                .min_by_key(|&index| {
                    let pixel = ring.pixels[index];

                    ((pixel.x - near.x).pow(2) + (pixel.y - near.y).pow(2), index)
                })
                .unwrap_or(0);

            ring.rotate(seam);
        }

        let mut plan = Planner::new(
            style,
            scale,
            if style.from_home {
                sketch.home
            } else {
                ring.start()
            },
        );

        plan.stroke(0, &ring);
        plan.go(sketch.letter);

        let ring = Course {
            motions: plan.fit(f64::from(MARK)),
            strokes: vec![ring],
        };

        // The view's pieces in the style's order: pen by pen, or pass by
        // pass; nearest first within.
        let key = |id: &Id| match style.order {
            Order::Pens => (id.0).2,
            _ => (id.0).0 as u8,
        };
        let mut pieces: Vec<(u8, Planned)> = sketch
            .pieces
            .iter()
            .filter_map(|sketched| {
                let stroke = stroke(&sketched.inked, sketch.clip)?;

                Some((
                    key(&sketched.id),
                    Planned {
                        stroke,
                        id: Some(sketched.id),
                    },
                ))
            })
            .collect();

        pieces.sort_by_key(|(key, _)| *key);

        let mut planned = vec![Planned {
            stroke: sketch.window,
            id: None,
        }];
        let mut pen = planned[0].stroke.end();
        let mut rest = pieces.into_iter().peekable();

        while let Some((key, first)) = rest.next() {
            let mut cluster = vec![first];

            while let Some((_, next)) = rest.next_if(|(next, _)| *next == key) {
                cluster.push(next);
            }

            while !cluster.is_empty() {
                let next = (0..cluster.len())
                    .min_by_key(|&index| {
                        let at = cluster[index].stroke.start();

                        ((at.x - pen.x).pow(2) + (at.y - pen.y).pow(2), index)
                    })
                    .unwrap_or(0);
                let next = cluster.remove(next);

                pen = next.stroke.end();
                planned.push(next);
            }
        }

        let places = planned
            .iter()
            .enumerate()
            .filter_map(|(index, planned)| Some((planned.id?, index)))
            .collect();
        let ids: Vec<Option<Id>> = planned.iter().map(|planned| planned.id).collect();
        let strokes: Vec<Stroke> = planned.into_iter().map(|planned| planned.stroke).collect();
        let mut plan = Planner::new(style, scale, sketch.letter);

        plan.still(Pose::Down, LETTERING_TIME);

        for (index, stroke) in strokes.iter().enumerate() {
            plan.stroke(index, stroke);
        }

        if style.park {
            plan.go(sketch.home);
            plan.still(Pose::Up, HOLD);
        }

        Self {
            ring,
            view: Course {
                motions: plan.fit(f64::from(DETAIL_PLOT - REST)),
                strokes,
            },
            places,
            ids,
        }
    }

    /// Where the plot is `time` seconds after the part is picked out.
    pub fn at(&self, time: f32) -> Now {
        let course =
            |course: &Course, time: f32| course.motions.at(f64::from(time), &course.strokes);

        if time < MARK {
            let place = course(&self.ring, time);

            Now {
                head: place.head,
                ring: drawn(&self.ring.strokes, &place, 0),
                window: 0.0,
                drawing: place.drawing.map(|_| Drawing::Ring),
                view: None,
            }
        } else {
            let place = course(&self.view, time - MARK);

            Now {
                head: place.head,
                ring: 1.0,
                window: drawn(&self.view.strokes, &place, 0),
                drawing: place
                    .drawing
                    .map(|(index, _)| self.ids[index].map_or(Drawing::Window, Drawing::Piece)),
                view: Some(place),
            }
        }
    }

    /// How much of the view's piece `id` is drawn at `now`, from none to
    /// all of it. A piece the plan does not know, of a mark there was not
    /// when it was made, is drawn as far as the last of its kind it knows.
    pub fn drawn(&self, now: &Now, id: Id) -> f64 {
        let Some(place) = now.view else {
            return 0.0;
        };
        let known = self
            .places
            .range((id.0, 0, 0)..=id)
            .next_back()
            .map(|(_, index)| *index);

        match known {
            Some(index) => drawn(&self.view.strokes, &place, index),
            None if place.done >= self.view.strokes.len() => 1.0,
            None => 0.0,
        }
    }
}

/// The share of `piece` the pen draws inside `clip`, in its cost: from
/// where it comes in to where it goes out; none if it stays out.
fn span(piece: &Piece, clip: Rectangle<i32>) -> Option<Range<usize>> {
    match piece {
        Piece::Path { pixels, .. } => {
            let inside = |pixel: &Point<i32>| raster::contains(clip, *pixel);

            Some(pixels.iter().position(inside)?..pixels.iter().rposition(inside)? + 1)
        }
        Piece::Rows { rows, .. } => {
            let inside = |&(y, from, to): &(i32, i32, i32)| {
                y >= clip.y
                    && y < clip.y + clip.height
                    && to >= clip.x
                    && from < clip.x + clip.width
            };

            Some(
                rows.iter().position(inside)? * ROW_COST
                    ..(rows.iter().rposition(inside)? + 1) * ROW_COST,
            )
        }
        Piece::Text { at, text } => raster::shows(*at, text, clip).then(|| 0..piece.cost()),
        Piece::Block(bounds) => raster::intersection(*bounds, clip).map(|_| 0..1),
        Piece::Knockout(_) => None,
    }
}

/// How much of `piece`'s cost is drawn with `drawn` of it plotted inside
/// `clip`: all of it once it is all plotted, or the ground a knockout
/// clears, from the start, a gap the pen leaves for lettering.
pub fn budget(piece: &Piece, clip: Rectangle<i32>, drawn: f64) -> usize {
    match span(piece, clip) {
        _ if drawn >= 1.0 || matches!(piece, Piece::Knockout(_)) => usize::MAX,
        Some(span) => span.start + (drawn * span.len() as f64) as usize,
        None => 0,
    }
}

/// How much of stroke `index` of `strokes` is drawn at `place`.
fn drawn(strokes: &[Stroke], place: &Place, index: usize) -> f64 {
    match place.drawing {
        _ if index < place.done => 1.0,
        Some((drawing, passed)) if drawing == index => {
            passed as f64 / strokes[index].pixels.len() as f64
        }
        _ => 0.0,
    }
}

/// A circle round `centre` of `radius`, lined as `stipple` says, in
/// `tone`, as the pen draws it in `style`: from its centre, if the style
/// draws circles so, or else from the top.
pub fn circle(
    centre: Point<i32>,
    radius: i32,
    stipple: Stipple,
    tone: crate::draft::Tone,
    style: &PlotStyle,
) -> Stroke {
    let pixels = raster::ordered_circle(centre, radius);
    let lit = (0..pixels.len())
        .map(|index| stipple.lights(index))
        .collect();
    let circle = Stroke {
        pixels,
        lit,
        tone,
        form: Form::Loop,
        mark: 0,
        piece: 0,
    };

    if style.circles == Circles::Centred && circle.pixels.len() >= 4 {
        strokes::centred(circle, centre)
    } else {
        circle
    }
}

/// The stroke the pen plans a piece of the view as, if it draws any of it
/// inside `clip`: a line along its pixels, back and forth along an area's
/// rows (by their ends, each row as much of the stroke as another), along a
/// line of lettering as it is typed, a touch for a dot.
fn stroke(inked: &Inked, clip: Rectangle<i32>) -> Option<Stroke> {
    let span = span(&inked.piece, clip)?;
    let (form, pixels) = match &inked.piece {
        Piece::Path { pixels, .. } => {
            let pixels = pixels[span].to_vec();
            let (first, last) = (pixels[0], pixels[pixels.len() - 1]);
            let closed =
                pixels.len() >= 4 && (first.x - last.x).abs() <= 1 && (first.y - last.y).abs() <= 1;

            match pixels.len() {
                1 => (Form::Touch(first), pixels),
                _ if closed => (Form::Loop, pixels),
                _ => (Form::Line, pixels),
            }
        }
        Piece::Rows { rows, .. } => {
            let (left, right) = (clip.x, clip.x + clip.width - 1);
            let mut pixels = Vec::new();

            for (index, &(y, from, to)) in rows[span.start / ROW_COST..span.end / ROW_COST]
                .iter()
                .enumerate()
            {
                let ends = [Point::new(from.max(left), y), Point::new(to.min(right), y)];

                if index % 2 == 0 {
                    pixels.extend(ends);
                } else {
                    pixels.extend(ends.into_iter().rev());
                }
            }

            (Form::Line, pixels)
        }
        Piece::Text { at, text } => {
            let y = at.y + i32::from(LETTERING.baseline()) - 1;
            let width = text.chars().count() as i32 * i32::from(LETTERING.advance());

            (
                Form::Line,
                (at.x..at.x + width.max(1))
                    .map(|x| Point::new(x, y))
                    .collect(),
            )
        }
        Piece::Block(bounds) => {
            let centre = Point::new(
                bounds.x + (bounds.width - 1) / 2,
                bounds.y + (bounds.height - 1) / 2,
            );

            (Form::Touch(centre), vec![centre])
        }
        Piece::Knockout(_) => return None,
    };

    (!pixels.is_empty()).then(|| Stroke {
        lit: vec![true; pixels.len()],
        pixels,
        tone: inked.tone,
        form,
        mark: 0,
        piece: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::raster::rect;
    use crate::draft::{Line, Tone};

    const CLIP: Rectangle<i32> = rect(500, 50, 200, 200);

    /// The kind of a stroke of the part's outline.
    const OUTLINE: Kind = (Pass::Edges, Some(0), 3, 0);
    /// The kind of a label.
    const LABEL: Kind = (Pass::Annotation, None, 3, 4);

    fn sketched(id: Id, piece: Piece) -> Sketched {
        Sketched {
            id,
            inked: Inked {
                piece,
                tone: Tone::Ink,
            },
        }
    }

    /// A detail with a line crossing its window and running out of it, a
    /// label and the ground under it.
    fn sketch(style: &PlotStyle) -> Sketch {
        Sketch {
            home: Point::new(5, 500),
            ring: circle(
                Point::new(200, 200),
                20,
                Stipple::of(Line::Phantom),
                Tone::Accent,
                style,
            ),
            letter: Point::new(230, 170),
            window: circle(
                Point::new(600, 150),
                80,
                Stipple::Dash { on: 11, off: 4 },
                Tone::Faint,
                style,
            ),
            pieces: vec![
                sketched(
                    (OUTLINE, 0, 0),
                    Piece::path(
                        quadrille::draw::shape::line(Point::new(450, 100), Point::new(650, 120)),
                        Stipple::Solid,
                    ),
                ),
                sketched((LABEL, 0, 0), Piece::Knockout(rect(540, 180, 40, 12))),
                sketched(
                    (LABEL, 0, 1),
                    Piece::Text {
                        at: Point::new(542, 180),
                        text: "PITCH".into(),
                    },
                ),
            ],
            clip: CLIP,
        }
    }

    /// The pen comes from home, draws the circle on the view and goes to
    /// its letter by the time it is lettered, then plots the view, never
    /// taking back what it has drawn, and is home with all of it drawn
    /// before the detail settles.
    #[test]
    fn the_pen_draws_the_circle_then_the_view_and_goes_home() {
        let style = PlotStyle::CAROUSEL;
        let detail = Detail::new(&style, sketch(&style), 1.0);
        let ids = [(OUTLINE, 0, 0), (LABEL, 0, 0), (LABEL, 0, 1)];
        let start = detail.at(0.0);

        assert_eq!(start.head.map(|head| head.at), Some(Point::new(5, 500)));
        assert_eq!((start.ring, start.window), (0.0, 0.0));

        let lettered = detail.at(MARK);

        assert_eq!(lettered.ring, 1.0);
        assert_eq!(
            lettered.head,
            Some(Head {
                at: Point::new(230, 170),
                pose: Pose::Down
            })
        );

        let mut before = vec![0.0; ids.len()];

        for frame in 0..=60 {
            let now = detail.at(MARK + frame as f32 * DETAIL_PLOT / 60.0);
            let drawn: Vec<f64> = ids.iter().map(|id| detail.drawn(&now, *id)).collect();

            assert!(
                drawn.iter().zip(&before).all(|(now, then)| now >= then),
                "{frame}"
            );
            before = drawn;
        }

        let settled = detail.at(MARK + DETAIL_PLOT - REST / 2.0);

        assert_eq!(settled.head, None);
        assert_eq!(settled.window, 1.0);
        assert!(ids.iter().all(|id| detail.drawn(&settled, *id) == 1.0));
    }

    /// A pen that neither starts from home nor goes back to it starts on
    /// the circle, where it is nearest the letter, and lifts off the view
    /// with all of it drawn, never going home.
    #[test]
    fn a_quick_studys_pen_starts_on_the_circle_and_lifts_off_the_view() {
        let style = PlotStyle::QUICK;
        let sketch = sketch(&style);
        let (home, letter) = (sketch.home, sketch.letter);
        let nearest = sketch
            .ring
            .pixels
            .iter()
            .copied()
            .min_by_key(|pixel| (pixel.x - letter.x).pow(2) + (pixel.y - letter.y).pow(2))
            .expect("A circle");
        let detail = Detail::new(&style, sketch, 1.0);
        let ids = [(OUTLINE, 0, 0), (LABEL, 0, 0), (LABEL, 0, 1)];

        assert_eq!(
            detail.at(0.0).head,
            Some(Head {
                at: nearest,
                pose: Pose::Down
            })
        );

        for frame in 0..=63 {
            let head = detail.at(frame as f32 / 30.0).head;

            assert!(head.is_none_or(|head| head.at != home), "{frame}");
        }

        let settled = detail.at(MARK + DETAIL_PLOT - REST / 2.0);

        assert_eq!(settled.head, None);
        assert!(ids.iter().all(|id| detail.drawn(&settled, *id) == 1.0));
    }

    /// A line running out of the window is drawn from where it comes in,
    /// and the ground under lettering is cleared from the start.
    #[test]
    fn what_is_drawn_is_what_is_in_the_window() {
        let line = Piece::path(
            quadrille::draw::shape::line(Point::new(450, 100), Point::new(650, 120)),
            Stipple::Solid,
        );

        assert_eq!(span(&line, CLIP), Some(50..201));
        assert_eq!(budget(&line, CLIP, 0.5), 50 + 75);
        assert_eq!(budget(&line, CLIP, 1.0), usize::MAX);
        assert_eq!(
            budget(&Piece::Knockout(rect(540, 180, 40, 12)), CLIP, 0.0),
            usize::MAX
        );
        assert_eq!(budget(&Piece::Block(rect(5, 5, 3, 3)), CLIP, 0.7), 0);
    }

    /// A piece of a mark the plan did not know, one more of a kind than
    /// there were when it was made, is drawn as far as the last of its
    /// kind the plan knows.
    #[test]
    fn a_mark_it_did_not_know_is_drawn_as_far_as_its_kind() {
        let style = PlotStyle::CAROUSEL;
        let detail = Detail::new(&style, sketch(&style), 1.0);

        for frame in 0..=30 {
            let now = detail.at(MARK + frame as f32 * DETAIL_PLOT / 30.0);

            assert_eq!(
                detail.drawn(&now, (OUTLINE, 3, 0)),
                detail.drawn(&now, (OUTLINE, 0, 0))
            );
        }
    }
}
