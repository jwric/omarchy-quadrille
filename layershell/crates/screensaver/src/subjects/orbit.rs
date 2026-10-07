//! A Hohmann transfer from low Earth orbit to geostationary orbit, seen
//! from over the North Pole: positions by Kepler's equation, burns as
//! changes of velocity, and the Earth turning under the satellite.
use std::f64::consts::{PI, TAU};

use crate::draft::scale::grouped;
use crate::draft::{Draft, Extent, Fill, Line, Tone, V2, polar, v};

use super::{Card, Detail, Domain, Part, Reading, Subject, Unit};

/// km³/s².
const MU: f64 = 398_600.441_8;
const EARTH: f64 = 6378.137;
const LEO: f64 = EARTH + 300.0;
const GEO: f64 = 42_164.0;
/// A sidereal day.
const DAY: f64 = 86_164.1;

/// The mission as shown, in seconds of the sheet: a turn of the parking
/// orbit, the transfer, then a stretch of the geostationary orbit.
const PARKING: f32 = 5.0;
const TRANSFER: f32 = 10.0;
const ARRIVED: f32 = 11.0;
/// The share of a geostationary turn the last phase shows.
const GEO_SHARE: f64 = 0.25;

/// The transfer ellipse's semi-major axis and eccentricity.
fn ellipse() -> (f64, f64) {
    ((LEO + GEO) / 2.0, (GEO - LEO) / (GEO + LEO))
}

fn speed(radius: f64, semi_major: f64) -> f64 {
    (MU * (2.0 / radius - 1.0 / semi_major)).sqrt()
}

/// The two burns: onto the transfer at perigee, onto GEO at apogee.
fn burns() -> (f64, f64) {
    let (a, _) = ellipse();

    (
        speed(LEO, a) - (MU / LEO).sqrt(),
        (MU / GEO).sqrt() - speed(GEO, a),
    )
}

fn period(semi_major: f64) -> f64 {
    TAU * (semi_major.powi(3) / MU).sqrt()
}

/// The eccentric anomaly for mean anomaly `mean`: Kepler's equation
/// `M = E − e sin E`, by Newton's method.
fn eccentric(mean: f64, e: f64) -> f64 {
    let mut anomaly = if e > 0.8 { PI } else { mean };

    for _ in 0..12 {
        anomaly -= (anomaly - e * anomaly.sin() - mean) / (1.0 - e * anomaly.cos());
    }

    anomaly
}

/// Where the mission is.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Parking,
    Transfer,
    Geostationary,
}

/// The state of the satellite at sheet time `t`.
#[derive(Debug, Clone, Copy)]
struct Craft {
    phase: Phase,
    /// km, from the Earth's centre.
    at: glam::DVec2,
    /// km/s.
    velocity: glam::DVec2,
    /// Seconds since the mission began.
    elapsed: f64,
}

fn craft(t: f32) -> Craft {
    let cycle = PARKING + TRANSFER + ARRIVED;
    let into = t.rem_euclid(cycle);
    let circular = |radius: f64, angle: f64| {
        let along = glam::DVec2::from_angle(angle);
        Craft {
            phase: Phase::Parking,
            at: along * radius,
            velocity: along.perp() * (MU / radius).sqrt(),
            elapsed: 0.0,
        }
    };
    let (a, e) = ellipse();

    if into < PARKING {
        let share = f64::from(into / PARKING);

        Craft {
            elapsed: share * period(LEO),
            ..circular(LEO, TAU * share)
        }
    } else if into < PARKING + TRANSFER {
        let share = f64::from((into - PARKING) / TRANSFER);
        let anomaly = eccentric(PI * share, e);
        let b = a * (1.0 - e * e).sqrt();
        let at = glam::DVec2::new(a * (anomaly.cos() - e), b * anomaly.sin());
        let towards = glam::DVec2::new(-a * anomaly.sin(), b * anomaly.cos()).normalize();

        Craft {
            phase: Phase::Transfer,
            at,
            velocity: towards * speed(at.length(), a),
            elapsed: period(LEO) + share * period(a) / 2.0,
        }
    } else {
        let share = f64::from((into - PARKING - TRANSFER) / ARRIVED) * GEO_SHARE;

        Craft {
            phase: Phase::Geostationary,
            elapsed: period(LEO) + period(a) / 2.0 + share * period(GEO),
            ..circular(GEO, PI + TAU * share)
        }
    }
}

/// The Earth's turn at mission time `elapsed`: a meridian it carries meets
/// the satellite when it arrives at geostationary orbit, and keeps under it.
fn earth_turn(elapsed: f64) -> f64 {
    let arrival = period(LEO) + period(ellipse().0) / 2.0;

    PI + (elapsed - arrival) * TAU / DAY
}

fn to_model(point: glam::DVec2) -> V2 {
    v(point.x as f32, point.y as f32)
}

/// Kilometres as a drawing writes them.
fn km(value: f64) -> String {
    format!("{} km", grouped(value.round() as u64))
}

/// Hours and minutes.
fn clock(seconds: f64) -> String {
    let minutes = (seconds / 60.0).round() as u64;

    format!("{} h {:02} min", minutes / 60, minutes % 60)
}

pub struct Orbit {
    card: Card,
}

impl Orbit {
    pub fn new() -> Self {
        let (a, _) = ellipse();
        let (first, second) = burns();

        let card = Card {
            title: "HOHMANN TRANSFER".into(),
            number: "QD-S-0001".into(),
            domain: Domain::Astronautical,
            unit: Unit::Kilometre,
            scaled: true,
            view: "PLAN FROM OVER THE NORTH POLE".into(),
            notes: vec![
                format!(
                    "Δv {:.2} + {:.2} = {:.2} km/s",
                    first,
                    second,
                    first + second
                ),
                format!("TRANSFER TAKES {}", clock(period(a) / 2.0)),
                "POSITIONS BY KEPLER'S EQUATION".into(),
                "IN GEO THE SATELLITE KEEPS OVER ONE MERIDIAN".into(),
            ],
            parts: vec![
                Part::new("EARTH", 1, "—")
                    .spec("RADIUS", km(EARTH))
                    .spec("SIDEREAL DAY", clock(DAY))
                    .detail(V2::ZERO, EARTH as f32 * 1.4),
                Part::new("PARKING ORBIT", 1, "LEO")
                    .spec("ALTITUDE", km(LEO - EARTH))
                    .spec("SPEED", format!("{:.2} km/s", (MU / LEO).sqrt()))
                    .spec("PERIOD", clock(period(LEO)))
                    .detail(v(LEO as f32, 0.0), 2500.0),
                Part::new("TRANSFER", 1, "ELLIPSE")
                    .spec("PERIGEE", km(LEO - EARTH))
                    .spec("APOGEE", km(GEO - EARTH))
                    .spec("ECCENTRICITY", format!("{:.3}", ellipse().1))
                    .spec("FIRST BURN", format!("{first:.2} km/s")),
                Part::new("GEOSTATIONARY", 1, "GEO")
                    .spec("RADIUS", km(GEO))
                    .spec("SPEED", format!("{:.2} km/s", (MU / GEO).sqrt()))
                    .spec("SECOND BURN", format!("{second:.2} km/s"))
                    .detail(v(-GEO as f32, 0.0), 5000.0),
                Part::new("SATELLITE", 1, "—").spec("Δv", "THE SAME FOR ANY MASS"),
            ],
        };

        Self { card }
    }
}

impl Subject for Orbit {
    fn name(&self) -> &'static str {
        "orbit"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        let r = GEO as f32 * 1.03;

        Extent::new(v(-r, -r), v(r, r))
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let (a, e) = ellipse();
        let now = craft(t);
        let (leo, geo) = (LEO as f32, GEO as f32);

        d.centre_mark(V2::ZERO, geo, 1500.0);

        d.part(0, |d| {
            let earth =
                crate::draft::arc_points(V2::ZERO, EARTH as f32, 0.0, std::f32::consts::TAU, 48);
            d.area(&earth, Fill::Tint(2)).tone(Tone::Faint);
            d.circle(V2::ZERO, EARTH as f32, Line::Outline);
            d.label(V2::ZERO, "N").tone(Tone::Muted);
        });
        d.part(1, |d| {
            d.circle(V2::ZERO, leo, Line::Phantom);
        });
        d.part(3, |d| {
            d.circle(V2::ZERO, geo, Line::Phantom);
        });

        // The transfer: the half flown solid, the other half hidden.
        d.part(2, |d| {
            let b = a * (1.0 - e * e).sqrt();
            let point = |anomaly: f64| {
                to_model(glam::DVec2::new(a * (anomaly.cos() - e), b * anomaly.sin()))
            };
            let flown: Vec<V2> = (0..=90).map(|i| point(PI * f64::from(i) / 90.0)).collect();
            let unflown: Vec<V2> = (0..=90)
                .map(|i| point(PI + PI * f64::from(i) / 90.0))
                .collect();

            d.polyline(&flown, Line::Outline);
            d.polyline(&unflown, Line::Hidden);
        });

        d.dim_h(v(leo, 0.0), v(-geo, 0.0), -geo * 0.94)
            .text(format!("2a {}", km(2.0 * a)));
        d.dim_radius(V2::ZERO, geo, 2.1, 22);
        d.note(v(0.0, -leo), (16, 30), format!("LEO {}", km(LEO - EARTH)));
        d.note(polar(geo, -0.9), (14, 16), "GEO");

        d.moving(|d| {
            // The Earth turning: meridians, one of them marked.
            d.part(0, |d| {
                let turn = earth_turn(now.elapsed) as f32;

                for k in 0..6 {
                    let angle = turn + k as f32 * std::f32::consts::PI / 3.0;
                    let tone = if k == 0 { Tone::Accent } else { Tone::Faint };
                    d.line(
                        polar(EARTH as f32 * 0.15, angle),
                        polar(EARTH as f32, angle),
                        Line::Thin,
                    )
                    .tone(tone);
                }
            });

            // The burns, lit as they happen.
            let burning = |at: f32| (t.rem_euclid(PARKING + TRANSFER + ARRIVED) - at).abs() < 0.8;
            let (first, second) = burns();
            for (at, point, along, size) in [
                (PARKING, v(leo, 0.0), v(0.0, 1.0), first),
                (PARKING + TRANSFER, v(-geo, 0.0), v(0.0, -1.0), second),
            ] {
                let tone = if burning(at) {
                    Tone::Caution
                } else {
                    Tone::Faint
                };
                // Velocities to scale: 2000 km of drawing per km/s.
                d.arrow(point, point + along * (size as f32 * 2000.0), Line::Trace)
                    .tone(tone);
            }

            // The satellite and its velocity, to scale: 2000 km per km/s.
            d.part(4, |d| {
                let at = to_model(now.at);
                d.dot(at, 5).tone(Tone::Accent);
                d.arrow(at, at + to_model(now.velocity) * 2000.0, Line::Trace);
                d.balloon(4, at, (22, -22));
            });
        });

        d.note(v(leo, 0.0), (26, 22), "PERIGEE BURN");
        d.note(v(-geo, 0.0), (24, 34), "APOGEE BURN");

        d.part(0, |d| {
            d.balloon(0, polar(EARTH as f32 * 0.7, 2.4), (-26, -40));
        });
        d.part(1, |d| {
            d.balloon(1, polar(leo, 2.0), (-40, -20));
        });
        d.part(2, |d| {
            d.balloon(
                2,
                to_model(glam::DVec2::new(-a * e, a * (1.0 - e * e).sqrt())),
                (20, -26),
            );
        });
        d.part(3, |d| {
            d.balloon(3, polar(geo, 2.6), (-24, -26));
        });
    }

    fn detail(&self, index: usize, t: f32) -> Option<Detail> {
        match index {
            // The satellite, wherever it is.
            4 => Some(Detail::following(to_model(craft(t).at), 2500.0)),
            // The transfer's fastest stretch, just past perigee.
            2 => Some(Detail::fixed(polar(LEO as f32 * 1.3, 0.7), 4000.0)),
            _ => self.card.parts.get(index)?.detail,
        }
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let now = craft(t);
        let phase = match now.phase {
            Phase::Parking => "PARKING",
            Phase::Transfer => "TRANSFER",
            Phase::Geostationary => "GEO",
        };

        vec![
            Reading::new("PHASE", phase),
            Reading::new("T+", clock(now.elapsed)),
            Reading::new("ALT", km(now.at.length() - EARTH)),
            Reading::new("V", format!("{:.2} km/s", now.velocity.length())),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_transfer_takes_five_and_a_quarter_hours_and_costs_three_point_nine_km_s() {
        let (a, _) = ellipse();
        let (first, second) = burns();

        assert!((period(a) / 2.0 / 3600.0 - 5.27).abs() < 0.05);
        assert!((first + second - 3.89).abs() < 0.02, "{}", first + second);
    }

    #[test]
    fn keplers_equation_is_solved() {
        for e in [0.0, 0.3, 0.73, 0.95] {
            for i in 0..50 {
                let mean = PI * f64::from(i) / 49.0;
                let anomaly = eccentric(mean, e);

                assert!((anomaly - e * anomaly.sin() - mean).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn the_satellite_slows_as_it_climbs_and_arrives_on_geo() {
        let perigee = craft(PARKING + 0.01);
        let apogee = craft(PARKING + TRANSFER - 0.01);

        assert!(perigee.velocity.length() > 10.0 && apogee.velocity.length() < 1.7);
        assert!((apogee.at.length() - GEO).abs() < 50.0);
    }

    #[test]
    fn in_geo_the_satellite_keeps_over_the_marked_meridian() {
        for k in 0..10 {
            let now = craft(PARKING + TRANSFER + ARRIVED * k as f32 / 10.0);
            let off = (now.at.to_angle() - earth_turn(now.elapsed)).rem_euclid(TAU);

            assert!(off.min(TAU - off) < 1e-3, "{off}");
        }
    }
}
