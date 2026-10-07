//! A Joukowski aerofoil in potential flow: the flow round a circle with the
//! circulation the Kutta condition asks for, mapped onto the aerofoil.
//! Particles released together trace timelines; the ones over the top
//! arrive first.
use std::f64::consts::{PI, TAU};

use num_complex::Complex64 as C;

use crate::draft::Placement::Auto;
use crate::draft::{Draft, Extent, Fill, Line, Tone, V2, number, v};

use super::{Card, Domain, Part, Reading, Revision, Subject, Unit};

/// The circle's centre off the origin: left for thickness, up for camber.
const CENTRE: C = C::new(-0.09, 0.07);
const ATTACK: f64 = 6.0;
/// Millimetres of model to one unit of the flow's plane: a 112 mm chord.
const SIZE: f32 = 28.0;
/// Where the streamlines start, across the flow and upstream.
const SEEDS: usize = 15;
const UPSTREAM: f64 = -3.4;
const DOWNSTREAM: f64 = 4.0;
/// Seconds between pulses of particles, and how fast the flow is drawn.
const PULSE: f32 = 0.45;
const PACE: f32 = 0.9;

/// The flow: everything about it that does not change.
struct Flow {
    radius: f64,
    circulation: f64,
    /// Each streamline as (time along it, point), in the aerofoil's plane.
    streamlines: Vec<Vec<(f32, V2)>>,
}

impl Flow {
    fn new() -> Self {
        let radius = (C::new(1.0, 0.0) - CENTRE).norm();
        let alpha = ATTACK.to_radians();
        // Kutta: the velocity vanishes at the trailing edge, ζ = 1.
        let circulation = 4.0 * PI * radius * (alpha - (C::new(1.0, 0.0) - CENTRE).arg()).sin();
        let mut flow = Self {
            radius,
            circulation,
            streamlines: Vec::new(),
        };

        flow.streamlines = (0..SEEDS)
            .map(|k| {
                let height = -1.5 + 3.0 * k as f64 / (SEEDS - 1) as f64;
                flow.streamline(C::new(UPSTREAM, height + 0.02))
            })
            .collect();

        flow
    }

    /// dw/dζ: the flow round the circle, free stream at the angle of attack.
    fn potential_slope(&self, zeta: C) -> C {
        let alpha = ATTACK.to_radians();
        let r = zeta - CENTRE;

        C::from_polar(1.0, -alpha) - C::from_polar(self.radius * self.radius, alpha) / (r * r)
            + C::i() * self.circulation / (TAU * r)
    }

    /// The velocity in the aerofoil's plane at the point that `zeta` maps to.
    fn velocity(&self, zeta: C) -> C {
        let stretch = C::new(1.0, 0.0) - 1.0 / (zeta * zeta);

        (self.potential_slope(zeta) / stretch).conj()
    }

    /// The point of the circle's plane outside the circle that maps to `z`.
    fn unmap(&self, z: C) -> C {
        let root = (z * z - 4.0).sqrt();
        let (a, b) = ((z + root) / 2.0, (z - root) / 2.0);

        if (a - CENTRE).norm() >= (b - CENTRE).norm() {
            a
        } else {
            b
        }
    }

    /// A streamline from `start`, integrated in the circle's plane where
    /// the flow has no corner.
    fn streamline(&self, start: C) -> Vec<(f32, V2)> {
        let map = |zeta: C| zeta + 1.0 / zeta;
        // dζ/dt from the velocity in the aerofoil's plane.
        let rate = |zeta: C| {
            let stretch = C::new(1.0, 0.0) - 1.0 / (zeta * zeta);
            self.velocity(zeta) / stretch
        };
        let step = 0.01;
        let mut zeta = self.unmap(start);
        let mut points = vec![(0.0, to_model(map(zeta)))];

        for i in 1..3000 {
            let k1 = rate(zeta);
            let k2 = rate(zeta + k1 * (step / 2.0));
            let k3 = rate(zeta + k2 * (step / 2.0));
            let k4 = rate(zeta + k3 * step);
            zeta += (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (step / 6.0);

            if !(zeta.re.is_finite() && zeta.im.is_finite()) {
                break;
            }

            let z = map(zeta);
            points.push(((i as f64 * step) as f32, to_model(z)));

            if z.re > DOWNSTREAM {
                break;
            }
        }

        points
    }

    /// The aerofoil: the circle mapped, from the trailing edge round the top.
    fn outline(&self) -> Vec<V2> {
        let start = (C::new(1.0, 0.0) - CENTRE).arg();

        (0..=180)
            .map(|i| CENTRE + C::from_polar(self.radius, start + TAU * i as f64 / 180.0))
            .map(|zeta| to_model(zeta + 1.0 / zeta))
            .collect()
    }

    /// The pressure coefficient round the surface, at the outline's points.
    fn pressure(&self) -> Vec<(V2, f64)> {
        let start = (C::new(1.0, 0.0) - CENTRE).arg();

        (1..180)
            .map(|i| CENTRE + C::from_polar(self.radius * 1.0005, start + TAU * i as f64 / 180.0))
            .map(|zeta| {
                (
                    to_model(zeta + 1.0 / zeta),
                    1.0 - self.velocity(zeta).norm_sqr(),
                )
            })
            .collect()
    }

    /// The lift coefficient on the chord: 2Γ / (U c).
    fn lift(&self) -> f64 {
        2.0 * self.circulation / f64::from(chord() / SIZE)
    }
}

fn to_model(z: C) -> V2 {
    v(z.re as f32 * SIZE, z.im as f32 * SIZE)
}

/// The chord, leading to trailing edge, in millimetres.
fn chord() -> f32 {
    let flow_lead = CENTRE - C::new((C::new(1.0, 0.0) - CENTRE).norm(), 0.0);
    let lead = flow_lead + 1.0 / flow_lead;

    (2.0 - lead.re) as f32 * SIZE
}

pub struct Aerofoil {
    card: Card,
    flow: Flow,
}

impl Aerofoil {
    pub fn new() -> Self {
        let flow = Flow::new();
        let peak = flow
            .pressure()
            .iter()
            .map(|(_, cp)| *cp)
            .fold(f64::MAX, f64::min);
        let lead = to_model({
            let zeta = CENTRE - C::new(flow.radius, 0.0);
            zeta + 1.0 / zeta
        });
        let (upper, _) = Self::suction_peak(&flow);

        let card = Card {
            title: "JOUKOWSKI AEROFOIL".into(),
            number: "QD-A-0006".into(),
            domain: Domain::Aeronautical,
            unit: Unit::Millimetre,
            scaled: true,
            view: "SECTION IN POTENTIAL FLOW".into(),
            notes: vec![
                format!(
                    "ANGLE OF ATTACK {}°, CL {:.2}",
                    number(ATTACK as f32),
                    flow.lift()
                ),
                "CIRCULATION BY THE KUTTA CONDITION".into(),
                "DOTS RELEASED TOGETHER: THE UPPER ONES ARRIVE FIRST".into(),
                "ENVELOPE: PRESSURE COEFFICIENT, SUCTION OUTWARD".into(),
            ],
            revisions: vec![Revision::first()],
            parts: vec![
                Part::new("SECTION", 1, "AL 2024")
                    .spec("CHORD", format!("{} mm", number(chord())))
                    .spec("CIRCLE", format!("{:.2} {:+.2}i", CENTRE.re, CENTRE.im))
                    .detail(v(0.0, 0.0), 16.0),
                Part::new("LEADING EDGE", 1, "—")
                    .spec("STAGNATION", "JUST BELOW THE NOSE")
                    .detail(lead, 7.0),
                Part::new("UPPER SKIN", 1, "—")
                    .spec("SUCTION PEAK", format!("Cp {:.2}", peak))
                    .detail(upper, 7.0),
                Part::new("TRAILING EDGE", 1, "—")
                    .spec("FORM", "CUSP")
                    .spec("KUTTA", "FLOW LEAVES SMOOTHLY")
                    .detail(v(2.0 * SIZE, 0.0), 7.0),
            ],
        };

        Self { card, flow }
    }

    /// Where the suction is strongest, and its pressure coefficient.
    fn suction_peak(flow: &Flow) -> (V2, f64) {
        flow.pressure()
            .into_iter()
            .fold((V2::ZERO, f64::MAX), |best, (at, cp)| {
                if cp < best.1 { (at, cp) } else { best }
            })
    }

    /// Where each pulse of particles is at `t` along a streamline.
    fn particles(line: &[(f32, V2)], t: f32) -> impl Iterator<Item = V2> + '_ {
        let end = line.last().map_or(0.0, |(time, _)| *time);
        let travelled = (t * PACE).rem_euclid(PULSE);

        (0..)
            .map(move |k| travelled + k as f32 * PULSE)
            .take_while(move |time| *time <= end)
            .map(move |time| {
                let at = line
                    .partition_point(|(when, _)| *when < time)
                    .min(line.len() - 1);
                line[at].1
            })
    }
}

impl Subject for Aerofoil {
    fn name(&self) -> &'static str {
        "aerofoil"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        Extent::new(
            v(UPSTREAM as f32 * SIZE, -1.5 * SIZE),
            v(DOWNSTREAM as f32 * SIZE, 1.5 * SIZE),
        )
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let outline = self.flow.outline();
        let lead = outline[outline.len() / 2];
        let trail = v(2.0 * SIZE, 0.0);

        for line in &self.flow.streamlines {
            let points: Vec<V2> = line.iter().step_by(4).map(|(_, at)| *at).collect();
            d.polyline(&points, Line::Thin).tone(Tone::Faint);
        }

        // The pressure envelope: each surface point pushed out along its
        // normal by the suction there, in by the pressure.
        let pressure = self.flow.pressure();
        let envelope: Vec<V2> = pressure
            .iter()
            .enumerate()
            .map(|(i, (at, cp))| {
                let (before, after) = (
                    pressure[i.saturating_sub(1)].0,
                    pressure[(i + 1).min(pressure.len() - 1)].0,
                );
                let outward = (after - before).perp().normalize_or_zero() * -1.0;

                *at + outward * (-*cp as f32 * 6.0)
            })
            .collect();
        d.polyline(&envelope, Line::Phantom).tone(Tone::Caution);

        d.part(0, |d| {
            d.area(&outline, Fill::Hatch);
            d.polygon(&outline, Line::Outline);
            d.line(lead, trail, Line::Centre);
        });

        // The free stream, at the angle of attack.
        let alpha = (ATTACK as f32).to_radians();
        let stream = V2::from_angle(alpha);
        for k in [-1.0, 0.0, 1.0] {
            let from = v(UPSTREAM as f32 * SIZE + 4.0, k * 12.0 - 26.0);
            d.arrow(from, from + stream * 18.0, Line::Thin)
                .tone(Tone::Muted);
        }
        let origin = v(UPSTREAM as f32 * SIZE + 4.0, -26.0);
        d.dim_angle(origin, 0.0, alpha, 64)
            .text(format!("α {}°", number(ATTACK as f32)));
        d.dim_h(lead, trail, -1.3 * SIZE);

        d.moving(|d| {
            for line in &self.flow.streamlines {
                for at in Self::particles(line, t) {
                    d.dot(at, 2).tone(Tone::Live);
                }
            }
        });

        d.part(0, |d| {
            d.balloon(0, v(20.0, 1.0), Auto);
        });
        d.part(1, |d| {
            d.balloon(1, lead, Auto);
        });
        d.part(2, |d| {
            // Mid-chord on the upper skin: the outline runs from the
            // trailing edge round the top.
            d.balloon(2, outline[outline.len() / 4], Auto);
        });
        d.part(3, |d| {
            d.balloon(3, trail, Auto);
        });
    }

    fn readings(&self, _t: f32) -> Vec<Reading> {
        let (_, peak) = Self::suction_peak(&self.flow);

        vec![
            Reading::new("α", format!("{}°", number(ATTACK as f32))),
            Reading::new("CL", format!("{:.2}", self.flow.lift())),
            Reading::new("Cp MIN", format!("{peak:.2}")),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flow_leaves_the_trailing_edge_smoothly() {
        let flow = Flow::new();

        assert!(flow.potential_slope(C::new(1.0, 0.0)).norm() < 1e-9);
    }

    #[test]
    fn the_lift_is_what_thin_aerofoil_theory_expects() {
        // About 2π per radian, a little more for thickness and camber.
        let lift = Flow::new().lift();
        let thin = 2.0 * PI * ATTACK.to_radians();

        assert!(lift > thin && lift < 2.5 * thin + 0.5, "{lift}");
    }

    #[test]
    fn particles_over_the_top_arrive_before_those_underneath() {
        let flow = Flow::new();
        let time_past_trailing_edge = |line: &Vec<(f32, V2)>| {
            line.iter()
                .find(|(_, at)| at.x > 2.5 * SIZE)
                .map(|(time, _)| *time)
        };
        // The streamlines just above and just below the aerofoil.
        let (above, below) = (
            &flow.streamlines[SEEDS / 2 + 1],
            &flow.streamlines[SEEDS / 2 - 1],
        );

        let (Some(a), Some(b)) = (
            time_past_trailing_edge(above),
            time_past_trailing_edge(below),
        ) else {
            panic!("a streamline did not pass the trailing edge");
        };
        assert!(a < b, "{a} against {b}");
    }
}
