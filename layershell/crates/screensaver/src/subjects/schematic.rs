//! Schematic symbols, as verbs of a [`Draft`]: what a circuit diagram is
//! drawn with. Lengths are in the diagram's grid units; a symbol sits on the
//! straight lead between its two terminals.
use quadrille::draw::Anchor;

use crate::draft::{Draft, Fill, Line, Tone, V2, v};

/// The length of a two-terminal symbol's body along its leads.
const BODY: f32 = 8.0;

pub trait Schematic {
    /// A wire through `points`.
    fn wire(&mut self, points: &[V2]);

    /// The dot where three wires join.
    fn junction(&mut self, at: V2);

    /// A zigzag resistor between `a` and `b`, its designator and value beside.
    fn resistor(&mut self, a: V2, b: V2, name: &str, value: &str);

    /// A capacitor between `a` and `b`; `polarised` marks the plate at `a`
    /// positive.
    fn capacitor(&mut self, a: V2, b: V2, name: &str, value: &str, polarised: bool);

    /// A light-emitting diode from anode `a` to cathode `b`, filled when lit.
    fn led(&mut self, a: V2, b: V2, name: &str, lit: bool);

    /// A supply terminal: a small open circle and its voltage.
    fn terminal(&mut self, at: V2, name: &str);
}

/// The unit vector from `a` to `b`, and the one across it.
fn axes(a: V2, b: V2) -> (V2, V2) {
    let along = (b - a).normalize_or_zero();

    (along, along.perp())
}

/// Where a symbol's designator and value go: beside its body, on the side
/// away from the drawing's left.
fn letter(d: &mut Draft, middle: V2, across: V2, name: &str, value: &str) {
    let side = if across.x.abs() > 0.5 {
        across * across.x.signum()
    } else {
        across * -across.y.signum()
    };
    let at = middle + side * 6.0;
    let anchor = if side.x > 0.5 {
        Anchor::LEFT
    } else {
        Anchor::CENTRE
    };

    d.label(at, name)
        .anchor(anchor)
        .nudge(0, -6)
        .tone(Tone::Ink);
    d.label(at, value).anchor(anchor).nudge(0, 6);
}

impl Schematic for Draft {
    fn wire(&mut self, points: &[V2]) {
        self.polyline(points, Line::Outline);
    }

    fn junction(&mut self, at: V2) {
        self.dot(at, 3).tone(Tone::Ink);
    }

    fn resistor(&mut self, a: V2, b: V2, name: &str, value: &str) {
        let (along, across) = axes(a, b);
        let middle = a.lerp(b, 0.5);
        let start = middle - along * (BODY / 2.0);
        let mut points = vec![a, start];

        for k in 0..6 {
            let side = if k % 2 == 0 { 1.0 } else { -1.0 };
            points.push(start + along * (BODY * (k as f32 + 0.5) / 6.0) + across * (1.6 * side));
        }

        points.extend([middle + along * (BODY / 2.0), b]);
        self.polyline(&points, Line::Outline);
        letter(self, middle, across, name, value);
    }

    fn capacitor(&mut self, a: V2, b: V2, name: &str, value: &str, polarised: bool) {
        let (along, across) = axes(a, b);
        let middle = a.lerp(b, 0.5);
        let (near, far) = (middle - along * 1.0, middle + along * 1.0);

        self.line(a, near, Line::Outline);
        self.line(far, b, Line::Outline);
        self.line(near - across * 4.0, near + across * 4.0, Line::Outline);
        self.line(far - across * 4.0, far + across * 4.0, Line::Outline);

        if polarised {
            self.label(near - along * 3.0 - across * 4.0, "+")
                .tone(Tone::Muted);
        }

        letter(self, middle, across, name, value);
    }

    fn led(&mut self, a: V2, b: V2, name: &str, lit: bool) {
        let (along, across) = axes(a, b);
        let middle = a.lerp(b, 0.5);
        let (base, tip) = (middle - along * 3.0, middle + along * 3.0);
        let triangle = [base - across * 3.5, base + across * 3.5, tip];

        self.line(a, base, Line::Outline);
        self.line(tip, b, Line::Outline);
        self.moving(|d| {
            if lit {
                d.area(&triangle, Fill::Solid).tone(Tone::Accent);
            }
            d.polygon(&triangle, Line::Outline);
        });
        self.line(tip - across * 3.5, tip + across * 3.5, Line::Outline);

        // The light going out, drawn only while it does.
        self.moving(|d| {
            for offset in [0.0, 3.0] {
                let from = middle + across * 5.0 + along * (offset - 2.0);
                let to = from + (across * 1.0 + along * 0.7).normalize() * 4.5;

                d.arrow(from, to, Line::Thin)
                    .tone(if lit { Tone::Accent } else { Tone::Faint });
            }
        });

        letter(self, middle, -across, name, "");
    }

    fn terminal(&mut self, at: V2, name: &str) {
        self.circle(at, 1.2, Line::Outline);
        self.label(at - v(2.5, 0.0), name)
            .anchor(Anchor::RIGHT)
            .tone(Tone::Ink);
    }
}
