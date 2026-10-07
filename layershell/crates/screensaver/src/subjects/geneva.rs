//! A Geneva drive: a turning pin indexes a six-slot wheel a sixth of a turn,
//! and a locking disc holds it still between.
use std::f32::consts::{PI, TAU};

use crate::draft::{Draft, Extent, Line, Tone, V2, arc_points, geom::wrap, number, polar, v};

use super::{Card, Detail, Domain, Part, Reading, Subject, Unit};

const SLOTS: u32 = 6;
/// Between the driver's and the wheel's centres.
const CENTRES: f32 = 50.0;
const PIN: f32 = 3.0;
const LOCK: f32 = 18.0;
const SHAFT: f32 = 5.0;
/// The slot's bottom, from the wheel's centre.
const SLOT_FOOT: f32 = 21.0;
const DRIVER_RPM: f32 = 10.0;

const WHEEL: V2 = v(CENTRES, 0.0);

/// The driver's crank radius: the pin enters a slot square to it.
fn crank() -> f32 {
    CENTRES * (PI / SLOTS as f32).sin()
}

/// Half the angle the driver turns while the pin is in a slot.
fn engagement() -> f32 {
    PI / 2.0 - PI / SLOTS as f32
}

pub struct Geneva {
    card: Card,
}

impl Geneva {
    pub fn new() -> Self {
        let index = 360.0 / SLOTS as f32;
        let moving = 2.0 * engagement().to_degrees();

        let card = Card {
            title: "GENEVA DRIVE".into(),
            number: "QD-M-0306".into(),
            domain: Domain::Mechanical,
            unit: Unit::Millimetre,
            scaled: true,
            view: "FRONT VIEW".into(),
            notes: vec![
                format!("{SLOTS} SLOTS: THE WHEEL INDEXES {}° A TURN", number(index)),
                format!(
                    "INDEXING {}°, LOCKED {}° OF THE DRIVER'S TURN",
                    number(moving),
                    number(360.0 - moving)
                ),
                "THE PIN ENTERS AND LEAVES EACH SLOT SQUARE TO IT: NO SHOCK".into(),
            ],
            parts: vec![
                Part::new("DRIVER", 1, "STEEL")
                    .spec("CRANK", number(crank()))
                    .spec("LOCKING DISC", format!("Ø{}", number(2.0 * LOCK)))
                    .detail(v(LOCK - 2.0, 0.0), 7.0),
                Part::new("DRIVE PIN", 1, "STEEL")
                    .spec("DIAMETER", format!("Ø{}", number(2.0 * PIN)))
                    .spec("FIT", "ROLLER ON NEEDLES")
                    .detail(v(CENTRES - crank() - 6.0, 0.0), 9.0),
                Part::new("WHEEL", 1, "STEEL")
                    .spec("SLOTS", SLOTS.to_string())
                    .spec("RADIUS", number(Self::rim()))
                    .spec("SLOT", format!("{} WIDE", number(2.0 * PIN + 0.6)))
                    .detail(v(CENTRES - SLOT_FOOT - 2.0, 0.0), 8.0),
                Part::new("SHAFT", 2, "STEEL")
                    .spec("DIAMETER", format!("Ø{}", number(2.0 * SHAFT)))
                    .detail(WHEEL, 8.0),
            ],
        };

        Self { card }
    }

    /// The wheel's radius to the tips between slots.
    fn rim() -> f32 {
        (CENTRES * CENTRES - crank() * crank()).sqrt()
    }

    /// The driver's angle at `t`, turning counter-clockwise from pointing at
    /// the wheel.
    fn driver(t: f32) -> f32 {
        TAU * DRIVER_RPM / 60.0 * t - engagement() - 0.4
    }

    /// The wheel's turn at driver angle `phi`, and whether it is indexing.
    ///
    /// Within each driver turn the pin is in a slot while it is within the
    /// engagement angle of the line of centres; the slot then points from the
    /// wheel's centre at the pin. Otherwise the wheel is locked where the
    /// last index left it.
    fn wheel(phi: f32) -> (f32, bool) {
        let turns = (phi / TAU).round();
        let within = phi - turns * TAU;
        let index = TAU / SLOTS as f32;
        let alpha = engagement();
        // The slot's direction, from where it starts to where the pin is.
        let slot = |u: f32| {
            let pin = polar(crank(), u);
            wrap((pin - WHEEL).to_angle())
        };

        let (offset, indexing) = if within < -alpha {
            (0.0, false)
        } else if within > alpha {
            (-index, false)
        } else {
            (slot(within) - slot(-alpha), true)
        };

        (-turns * index + offset, indexing)
    }

    /// The wheel's outline turned `turn` from rest: slots with round feet,
    /// and between them the concave arcs the locking disc runs in.
    fn wheel_outline(turn: f32) -> Vec<V2> {
        let index = TAU / SLOTS as f32;
        let half = PIN + 0.3;
        let arc = LOCK + 0.5;
        let mut points = Vec::new();

        for k in 0..SLOTS {
            let slot = turn + index / 2.0 + k as f32 * index;
            let along = V2::from_angle(slot);
            let side = along.perp();

            // Where an edge of the slot meets the locking arc beyond it.
            let meets = |sign: f32, arc_angle: f32| {
                let centre = WHEEL + polar(CENTRES, arc_angle);
                let start = WHEEL + side * (sign * half);
                let offset = start - centre;
                let b = offset.dot(along);
                let c = offset.length_squared() - arc * arc;

                start + along * (-b - (b * b - c).max(0.0).sqrt())
            };

            let before = meets(-1.0, slot - index / 2.0);
            let after = meets(1.0, slot + index / 2.0);

            // In along the slot's leading edge, round its foot, out again.
            points.push(before);
            points.extend(arc_points(
                WHEEL + along * SLOT_FOOT,
                half,
                slot - PI / 2.0,
                -PI,
                8,
            ));
            points.push(after);

            // The concave arc to the next slot.
            let centre = WHEEL + polar(CENTRES, slot + index / 2.0);
            let next = turn + index / 2.0 + (k + 1) as f32 * index;
            let next_before = {
                let along = V2::from_angle(next);
                let start = WHEEL - along.perp() * half;
                let offset = start - centre;
                let b = offset.dot(along);
                let c = offset.length_squared() - arc * arc;

                start + along * (-b - (b * b - c).max(0.0).sqrt())
            };
            let from = (after - centre).to_angle();
            let to = (next_before - centre).to_angle();
            let sweep = (to - from + PI).rem_euclid(TAU) - PI;

            points.extend(arc_points(centre, arc, from, sweep, 10)[1..10].iter());
        }

        points
    }

    /// The driver's locking disc at angle `phi`, cut away where the wheel's
    /// tips pass while the pin drives it.
    fn locking_disc(phi: f32) -> Vec<V2> {
        let cut = polar(CENTRES, phi);
        let clearance = Self::rim() + 1.0;

        arc_points(V2::ZERO, LOCK, phi, TAU, 96)
            .into_iter()
            .map(|point| {
                let away = point - cut;

                if away.length() < clearance {
                    cut + away.normalize_or_zero() * clearance
                } else {
                    point
                }
            })
            .collect()
    }
}

impl Subject for Geneva {
    fn name(&self) -> &'static str {
        "geneva"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        Extent::new(v(-34.0, -50.0), v(CENTRES + Self::rim() + 6.0, 50.0))
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let phi = Self::driver(t);
        let (turn, _) = Self::wheel(phi);
        let pin = polar(crank(), phi);
        let alpha = engagement();

        d.centre_mark(V2::ZERO, LOCK, 4.0);
        d.centre_mark(WHEEL, Self::rim(), 4.0);
        d.circle(V2::ZERO, crank(), Line::Phantom);

        // Where the pin is in a slot: the two radii that bound it.
        for edge in [-alpha, alpha] {
            d.line(V2::ZERO, polar(crank() + 6.0, edge), Line::Thin);
        }

        d.dim_h(V2::ZERO, WHEEL, -(Self::rim() + 2.0));
        d.dim_angle(V2::ZERO, -alpha, alpha, 46).tone(Tone::Muted);

        d.moving(|d| {
            d.part(0, |d| {
                d.polygon(&Self::locking_disc(phi), Line::Outline);

                // The arm from the hub out to the pin.
                let along = V2::from_angle(phi);
                let side = along.perp() * 4.0;
                d.polygon(
                    &[
                        side,
                        pin - along * PIN + side,
                        pin - along * PIN - side,
                        -side,
                    ],
                    Line::Hidden,
                );
            });
            d.part(1, |d| {
                d.circle(pin, PIN, Line::Outline);
                d.circle(pin, PIN - 1.2, Line::Thin);
            });
            d.part(2, |d| {
                d.polygon(&Self::wheel_outline(turn), Line::Outline);
            });
            d.part(3, |d| {
                for centre in [V2::ZERO, WHEEL] {
                    let section = arc_points(centre, SHAFT, 0.0, TAU, 24);
                    d.hatch(&section);
                    d.polygon(&section, Line::Outline);
                }
            });

            d.part(1, |d| {
                d.balloon(1, pin, (-34, -30));
            });
        });

        d.part(0, |d| {
            d.balloon(0, polar(LOCK - 3.0, 3.6), (-30, 30));
        });
        d.part(2, |d| {
            d.balloon(2, WHEEL + v(14.0, -26.0), (36, 26));
        });
        d.part(3, |d| {
            d.balloon(3, WHEEL + v(2.0, -2.0), (40, -56));
            d.dim_diameter(WHEEL, SHAFT, 2.4, 30);
        });
        d.part(2, |d| {
            d.dim_radius(WHEEL, Self::rim(), 0.62, 20);
        });

        d.in_detail(|d| {
            d.part(1, |d| {
                d.dim_diameter(pin, PIN, 1.2, 16);
            });
        });
    }

    fn detail(&self, index: usize, t: f32) -> Option<Detail> {
        match index {
            // The pin, wherever it is.
            1 => Some(Detail::following(polar(crank(), Self::driver(t)), 6.0)),
            _ => self.card.parts.get(index)?.detail,
        }
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let phi = Self::driver(t);
        let (turn, indexing) = Self::wheel(phi);
        let degrees = |angle: f32| format!("{:5.1}°", wrap(angle).to_degrees());

        vec![
            Reading::new("DRIVER", degrees(phi)),
            Reading::new("WHEEL", degrees(turn)),
            Reading::new("STATE", if indexing { "INDEXING" } else { "LOCKED" }),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_driver_turn_indexes_the_wheel_one_slot_without_a_jump() {
        let step = 0.002;
        let mut last = Geneva::wheel(0.0).0;

        for i in 1..=(TAU / step) as usize * 2 {
            let (turn, _) = Geneva::wheel(i as f32 * step);

            assert!(
                (turn - last).abs() < 0.01,
                "the wheel jumps at {}",
                i as f32 * step
            );
            assert!(turn <= last + 1e-6, "the wheel turns back");
            last = turn;
        }

        let one = Geneva::wheel(TAU).0 - Geneva::wheel(0.0).0;
        assert!((one + TAU / SLOTS as f32).abs() < 1e-4, "{one}");
    }

    #[test]
    fn the_pin_stays_in_its_slot_while_it_drives() {
        let alpha = engagement();

        for i in -50..=50 {
            let phi = alpha * i as f32 / 50.0;
            let (turn, indexing) = Geneva::wheel(phi);
            let pin = polar(crank(), phi);
            let index = TAU / SLOTS as f32;

            assert!(indexing);

            // Some slot points straight at the pin.
            let towards = (pin - WHEEL).to_angle();
            let off = (0..SLOTS)
                .map(|k| {
                    (wrap(turn + index / 2.0 + k as f32 * index - towards) + PI).rem_euclid(TAU)
                        - PI
                })
                .map(f32::abs)
                .fold(f32::MAX, f32::min);

            assert!(off < 1e-3, "{off} at {phi}");
        }
    }
}
