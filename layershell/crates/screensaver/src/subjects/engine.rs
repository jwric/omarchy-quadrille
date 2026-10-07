//! A single-cylinder four-stroke engine in section: slider-crank, valves on
//! their timing, and the piston's travel charted against the crank angle.
use std::f32::consts::{PI, TAU};

use crate::draft::Placement::Auto;
use crate::draft::{Draft, Extent, Line, Tone, Turn, V2, arc_points, number, polar, v};

use super::{Card, Detail, Domain, Part, Reading, Revision, Subject, Unit};

const BORE: f32 = 40.0;
const CRANK: f32 = 20.0;
const ROD: f32 = 70.0;
/// From the gudgeon pin's centre up to the crown.
const COMPRESSION_HEIGHT: f32 = 19.0;
const SKIRT: f32 = 13.0;
/// The squish height that makes the compression ratio about 10:1.
const CLEARANCE: f32 = 4.4;
/// A slow crank, so the strokes can be followed.
const RPM: f32 = 20.0;

const DECK: f32 = CRANK + ROD + COMPRESSION_HEIGHT + CLEARANCE;
const HEAD: f32 = 18.0;
const WALL: f32 = 5.0;
const LINER_FOOT: f32 = ROD - CRANK - SKIRT + 4.0;

const VALVE_X: f32 = 9.0;
const VALVE_HEAD: f32 = 7.0;
const VALVE_LIFT: f32 = 4.0;

/// The chart of piston travel: its corner, and how much of the model a
/// degree and a millimetre take.
const CHART: V2 = v(52.0, 40.0);
const CHART_WIDTH: f32 = 72.0;
const CHART_HEIGHT: f32 = 40.0;

/// Valve events in crank degrees of the 720° cycle from the TDC that begins
/// the intake stroke: (opens, closes).
const INTAKE: (f32, f32) = (-10.0, 220.0);
const EXHAUST: (f32, f32) = (500.0, 730.0);

const STROKES: [&str; 4] = ["INTAKE", "COMPRESSION", "POWER", "EXHAUST"];
/// The strokes as the chart has room to letter them.
const SHORT: [&str; 4] = ["IN", "COMP", "POW", "EXH"];

pub struct Engine {
    card: Card,
}

impl Engine {
    pub fn new() -> Self {
        let swept = PI / 4.0 * BORE * BORE * 2.0 * CRANK / 1000.0;
        let ratio = (swept * 1000.0 + Self::clearance_volume()) / Self::clearance_volume();

        let card = Card {
            title: "FOUR-STROKE SINGLE".into(),
            number: "QD-M-0254".into(),
            domain: Domain::Mechanical,
            unit: Unit::Millimetre,
            scaled: true,
            view: "SECTION THROUGH THE CYLINDER AXIS".into(),
            notes: vec![
                format!(
                    "BORE {} × STROKE {}, {} cm³",
                    number(BORE),
                    number(2.0 * CRANK),
                    number(swept.round())
                ),
                format!("COMPRESSION RATIO {:.1}:1", ratio),
                format!("ROD/CRANK {:.2}", ROD / CRANK),
                "VALVES: IO 10° BTDC, IC 40° ABDC, EO 40° BBDC, EC 10° ATDC".into(),
            ],
            revisions: vec![Revision::first()],
            parts: vec![
                Part::new("PISTON", 1, "AL ALLOY")
                    .spec("DIAMETER", format!("Ø{}", number(BORE - 0.1)))
                    .spec("RINGS", "2 COMPRESSION, 1 OIL")
                    .spec("PIN", "Ø10 FULL FLOATING")
                    .detail(v(-BORE / 2.0 + 2.0, DECK - CLEARANCE - 12.0), 6.0),
                Part::new("CON ROD", 1, "STEEL")
                    .spec("CENTRES", number(ROD))
                    .spec("BIG END", "Ø14 NEEDLE ROLLER")
                    .spec("SMALL END", "Ø10 BUSHED")
                    .detail(v(0.0, ROD + CRANK * 0.2), 10.0),
                Part::new("CRANKSHAFT", 1, "STEEL")
                    .spec("THROW", number(CRANK))
                    .spec("MAIN JOURNAL", "Ø18")
                    .spec("CRANKPIN", "Ø14")
                    .detail(V2::ZERO, 16.0),
                Part::new("CYLINDER", 1, "CAST IRON")
                    .spec("BORE", format!("Ø{} H7", number(BORE)))
                    .spec("WALL", number(WALL))
                    .spec("DECK", number(DECK))
                    .detail(v(-BORE / 2.0 - 2.0, DECK - 4.0), 7.0),
                Part::new("VALVE", 2, "STEEL")
                    .spec("HEAD", format!("Ø{}", number(2.0 * VALVE_HEAD)))
                    .spec("LIFT", number(VALVE_LIFT))
                    .spec("SEAT", "45°")
                    .detail(v(VALVE_X, DECK + 2.0), 6.0),
            ],
        };

        Self { card }
    }

    /// The volume above the piston at TDC, in mm³.
    fn clearance_volume() -> f32 {
        PI / 4.0 * BORE * BORE * CLEARANCE
    }

    /// The crank angle at `t`: degrees into the 720° cycle, from TDC at the
    /// start of intake. The crank turns clockwise as drawn.
    fn crank(t: f32) -> f32 {
        (RPM / 60.0 * 360.0 * t).rem_euclid(720.0)
    }

    /// The gudgeon pin's height over the crank centre at crank angle `angle`.
    fn pin_height(angle: f32) -> f32 {
        let theta = angle.to_radians();
        let reach = (ROD * ROD - (CRANK * theta.sin()).powi(2)).sqrt();

        CRANK * theta.cos() + reach
    }

    /// How far the piston is below TDC.
    fn travel(angle: f32) -> f32 {
        CRANK + ROD - Self::pin_height(angle)
    }

    /// A valve's lift at crank angle `angle`, open between `events`.
    fn lift(angle: f32, (opens, closes): (f32, f32)) -> f32 {
        let into = (angle - opens).rem_euclid(720.0);
        let duration = closes - opens;

        if into < duration {
            VALVE_LIFT * (PI * into / duration).sin().powi(2)
        } else {
            0.0
        }
    }

    fn chart_point(angle: f32, travel: f32) -> V2 {
        CHART
            + v(
                angle / 720.0 * CHART_WIDTH,
                CHART_HEIGHT - travel / (2.0 * CRANK) * CHART_HEIGHT,
            )
    }
}

/// `points` turned `angle` radians about the origin and moved to `at`.
fn placed(points: &[V2], angle: f32, at: V2) -> Vec<V2> {
    points
        .iter()
        .map(|point| at + point.turned(angle))
        .collect()
}

/// Sizes of the moving parts, from the bore and the stroke.
const PIN: f32 = 5.0;
const SMALL_END: f32 = 7.5;
const CRANKPIN: f32 = 7.0;
const BIG_END: f32 = 12.0;
const JOURNAL: f32 = 9.0;
const COUNTERWEIGHT: f32 = 25.0;
const STEM: f32 = 1.5;

impl Engine {
    /// The cylinder in section: two hatched walls, and the head with its
    /// ports, valve guides and spark plug.
    fn cylinder(d: &mut Draft) {
        let half = BORE / 2.0;
        let outer = half + WALL;
        let edge = outer + 3.0;

        for side in [-1.0, 1.0] {
            let wall = [
                v(side * half, LINER_FOOT),
                v(side * half, DECK),
                v(side * outer, DECK),
                v(side * outer, LINER_FOOT),
            ];
            d.hatch(&wall);
            d.polygon(&wall, Line::Outline);
        }

        d.rect(v(-edge, DECK), v(edge, DECK + HEAD), Line::Outline);

        // Each port runs from its seat out through the side of the head.
        for side in [-1.0, 1.0] {
            let x = side * VALVE_X;
            let (low, high) = (DECK + HEAD * 0.3, DECK + HEAD * 0.65);

            d.polyline(
                &[
                    v(x - side * VALVE_HEAD, DECK),
                    v(x - side * 2.0, high),
                    v(side * edge, high),
                ],
                Line::Outline,
            );
            d.polyline(
                &[
                    v(x + side * VALVE_HEAD, DECK),
                    v(x + side * 4.0, low),
                    v(side * edge, low),
                ],
                Line::Outline,
            );
            d.line(
                v(x - STEM - 1.0, high),
                v(x - STEM - 1.0, DECK + HEAD),
                Line::Hidden,
            );
            d.line(
                v(x + STEM + 1.0, high),
                v(x + STEM + 1.0, DECK + HEAD),
                Line::Hidden,
            );
        }

        d.rect(v(-2.0, DECK), v(2.0, DECK + HEAD), Line::Hidden);
        d.rect(
            v(-4.0, DECK + HEAD),
            v(4.0, DECK + HEAD + 6.0),
            Line::Outline,
        );
        d.line(
            v(0.0, DECK + HEAD + 6.0),
            v(0.0, DECK + HEAD + 10.0),
            Line::Outline,
        );

        // The crankcase, behind, open where the rod swings.
        d.arc(
            V2::ZERO,
            COUNTERWEIGHT + 5.0,
            -0.3 * PI,
            -0.4 * PI,
            Line::Hidden,
        );
        d.arc(
            V2::ZERO,
            COUNTERWEIGHT + 5.0,
            -1.3 * PI,
            0.6 * PI,
            Line::Hidden,
        );
    }

    /// The piston in section at pin height `pin`: crown, ring grooves, skirt
    /// and the gudgeon pin.
    fn piston(d: &mut Draft, pin: V2) {
        let r = BORE / 2.0 - 0.3;
        let (top, foot) = (pin.y + COMPRESSION_HEIGHT, pin.y - SKIRT);
        let (wall, crown) = (2.5, 5.0);
        let body = [
            v(-r, foot),
            v(-r, top),
            v(r, top),
            v(r, foot),
            v(r - wall, foot),
            v(r - wall, top - crown),
            v(-r + wall, top - crown),
            v(-r + wall, foot),
        ];

        d.hatch(&body);
        d.polygon(&body, Line::Outline);

        for k in 0..3 {
            let y = top - 2.5 - k as f32 * 2.8;

            for side in [-1.0, 1.0] {
                d.rect(v(side * r, y), v(side * (r - 1.8), y - 1.2), Line::Outline);
            }
        }

        d.circle(pin, PIN, Line::Outline);
        d.hatch(&arc_points(pin, PIN, 0.0, TAU, 24));
    }

    /// The connecting rod between the crankpin and the gudgeon pin.
    fn rod(d: &mut Draft, crankpin: V2, pin: V2) {
        let along = (pin - crankpin).normalize_or_zero();
        let across = along.perp();
        let shank = [
            crankpin + along * (BIG_END - 1.0) + across * 5.0,
            pin - along * (SMALL_END - 1.0) + across * 3.0,
            pin - along * (SMALL_END - 1.0) - across * 3.0,
            crankpin + along * (BIG_END - 1.0) - across * 5.0,
        ];

        d.polygon(&shank, Line::Outline);
        d.circle(crankpin, BIG_END, Line::Outline);
        d.circle(pin, SMALL_END, Line::Outline);
        d.line(crankpin, pin, Line::Centre);
    }

    /// The crankshaft turned `theta` clockwise: web and counterweight, main
    /// journal and crankpin.
    fn crankshaft(d: &mut Draft, theta: f32, crankpin: V2) {
        // At TDC: the counterweight's arc below, clockwise from lower right
        // to lower left, then over the crankpin's boss.
        let mut web = arc_points(V2::ZERO, COUNTERWEIGHT, -PI / 2.0 + 1.05, -2.1, 20);
        web.extend(arc_points(
            v(0.0, CRANK),
            CRANKPIN + 3.0,
            PI + 0.35,
            -(PI + 0.7),
            12,
        ));

        d.polygon(&placed(&web, -theta, V2::ZERO), Line::Outline);
        d.circle(V2::ZERO, JOURNAL, Line::Outline);
        d.circle(crankpin, CRANKPIN, Line::Outline);
        d.line(V2::ZERO, crankpin, Line::Thin);
    }

    /// The two valves at crank angle `angle`, open by their cams.
    fn valves(d: &mut Draft, angle: f32) {
        for (side, events) in [(-1.0, INTAKE), (1.0, EXHAUST)] {
            let x = side * VALVE_X;
            let lift = Self::lift(angle, events);
            let seat = DECK - lift;
            let tip = DECK + HEAD + 8.0 - lift;
            let valve = [
                v(x - VALVE_HEAD, seat),
                v(x + VALVE_HEAD, seat),
                v(x + VALVE_HEAD - 2.0, seat + 2.0),
                v(x + STEM, seat + 5.0),
                v(x + STEM, tip),
                v(x - STEM, tip),
                v(x - STEM, seat + 5.0),
                v(x - VALVE_HEAD + 2.0, seat + 2.0),
            ];

            d.polygon(&valve, Line::Outline);
        }
    }

    /// The chart of piston travel over the cycle, without its cursor.
    fn chart(d: &mut Draft) {
        let end = CHART + v(CHART_WIDTH, 0.0);

        d.line(CHART, end, Line::Thin);
        d.line(CHART, CHART + v(0.0, CHART_HEIGHT), Line::Thin);

        for stroke in 0..=4 {
            let x = CHART.x + CHART_WIDTH * stroke as f32 / 4.0;

            d.line(
                v(x, CHART.y - 1.5),
                v(x, CHART.y + CHART_HEIGHT),
                Line::Centre,
            );

            if let Some(name) = SHORT.get(stroke) {
                d.label(v(x + CHART_WIDTH / 8.0, CHART.y - 5.0), *name)
                    .tone(Tone::Faint);
            }
        }

        let curve: Vec<V2> = (0..=144)
            .map(|i| i as f32 * 5.0)
            .map(|angle| Self::chart_point(angle, Self::travel(angle)))
            .collect();

        d.polyline(&curve, Line::Thin).tone(Tone::Muted);
        d.note(CHART + v(0.0, CHART_HEIGHT), (10, -10), "TRAVEL FROM TDC");
    }
}

impl Subject for Engine {
    fn name(&self) -> &'static str {
        "engine"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        Extent::new(
            v(-46.0, -COUNTERWEIGHT - 3.0),
            v(CHART.x + CHART_WIDTH + 2.0, DECK + HEAD + 17.0),
        )
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let angle = Self::crank(t);
        let theta = angle.to_radians();
        let crankpin = polar(CRANK, PI / 2.0 - theta);
        let pin = v(0.0, Self::pin_height(angle));
        let crown = pin.y + COMPRESSION_HEIGHT;
        let half = BORE / 2.0;
        let (tdc, bdc) = (
            CRANK + ROD + COMPRESSION_HEIGHT,
            ROD - CRANK + COMPRESSION_HEIGHT,
        );

        // The cylinder's axis, the crank's centre lines and the crankpin's
        // circle; top and bottom dead centre, and the stroke between them.
        d.line(
            v(0.0, -COUNTERWEIGHT - 6.0),
            v(0.0, DECK + HEAD + 12.0),
            Line::Centre,
        );
        d.line(
            v(-COUNTERWEIGHT - 6.0, 0.0),
            v(COUNTERWEIGHT + 6.0, 0.0),
            Line::Centre,
        );
        d.circle(V2::ZERO, CRANK, Line::Phantom);

        for (y, name) in [(tdc, "TDC"), (bdc, "BDC")] {
            d.line(v(-half - WALL - 8.0, y), v(-half + 4.0, y), Line::Phantom);
            d.label(v(-half - WALL - 9.0, y), name)
                .anchor(quadrille::draw::Anchor::RIGHT)
                .tone(Tone::Faint);
        }
        d.dim_v(v(-half, tdc), v(-half, bdc), -(WALL + 4.0));
        d.dim_h(v(-half, DECK + HEAD), v(half, DECK + HEAD), 14.0)
            .text(format!("Ø{}", number(BORE)))
            .fit("H7");

        d.part(3, Self::cylinder);
        Self::chart(d);

        d.moving(|d| {
            d.part(0, |d| Self::piston(d, pin));
            d.part(1, |d| Self::rod(d, crankpin, pin));
            d.part(2, |d| Self::crankshaft(d, theta, crankpin));
            d.part(4, |d| Self::valves(d, angle));

            // A spark at the top of compression.
            if (345.0..365.0).contains(&angle) {
                let gap = v(0.0, DECK - 1.0);

                for k in 0..6 {
                    let ray = polar(3.5, k as f32 * TAU / 6.0 + 0.3);
                    d.line(gap + ray * 0.4, gap + ray, Line::Trace)
                        .tone(Tone::Caution);
                }
            }

            // How far the piston is from TDC, measured live and charted.
            d.dim_v(v(half, tdc), v(half, crown), WALL + 5.0)
                .text(number((tdc - crown).max(0.0)));

            let at = Self::chart_point(angle, Self::travel(angle));
            d.line(
                v(at.x, CHART.y),
                v(at.x, CHART.y + CHART_HEIGHT),
                Line::Trace,
            )
            .tone(Tone::Faint);
            d.dot(at, 3).tone(Tone::Accent);

            // Balloons that follow their parts.
            d.part(0, |d| {
                d.balloon(0, v(-half + 6.0, crown - 2.0), Auto);
            });
            d.part(1, |d| {
                d.balloon(1, crankpin.lerp(pin, 0.45), Auto);
            });
        });

        d.part(2, |d| {
            d.balloon(2, v(-COUNTERWEIGHT * 0.7, -COUNTERWEIGHT * 0.5), Auto);
        });
        d.part(3, |d| {
            d.balloon(3, v(half + WALL - 1.0, LINER_FOOT + 6.0), Auto);
        });
        d.part(4, |d| {
            d.balloon(4, v(VALVE_X + STEM, DECK + HEAD + 4.0), Auto);
        });

        d.in_detail(|d| {
            d.part(4, |d| {
                d.dim_angle(v(VALVE_X - VALVE_HEAD, DECK), 0.0, PI / 4.0, 22)
                    .text("45°");
            });
            d.part(0, |d| {
                d.note(v(-half + 0.9, crown - 3.1), (20, -14), "TOP RING");
            });
        });
    }

    fn detail(&self, index: usize, t: f32) -> Option<Detail> {
        let pin = Self::pin_height(Self::crank(t));

        match index {
            // The rings, riding with the piston.
            0 => Some(Detail::following(
                v(-BORE / 2.0 + 2.0, pin + COMPRESSION_HEIGHT - 5.0),
                6.0,
            )),
            // The small end round the gudgeon pin.
            1 => Some(Detail::following(v(0.0, pin), 9.0)),
            _ => self.card.parts.get(index)?.detail,
        }
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let angle = Self::crank(t);

        vec![
            Reading::new("CRANK", format!("{:5.1}°", angle % 360.0)),
            Reading::new("STROKE", STROKES[(angle / 180.0) as usize % 4]),
            Reading::new("PISTON", format!("{:4.1} mm", Self::travel(angle))),
            Reading::new("SPEED", format!("{} rpm", number(RPM))),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_piston_travels_its_stroke_and_never_meets_a_valve() {
        let stroke = Engine::travel(180.0) - Engine::travel(0.0);

        assert!((stroke - 2.0 * CRANK).abs() < 1e-3);

        for step in 0..720 {
            let angle = step as f32;
            let crown = Engine::pin_height(angle) + COMPRESSION_HEIGHT;

            for events in [INTAKE, EXHAUST] {
                let valve = DECK - Engine::lift(angle, events);

                assert!(valve > crown + 0.5, "the piston meets a valve at {angle}°");
            }
        }
    }

    #[test]
    fn each_valve_opens_once_a_cycle() {
        for events in [INTAKE, EXHAUST] {
            let open = (0..720)
                .filter(|angle| Engine::lift(*angle as f32, events) > 0.0)
                .count();

            assert!((220..=240).contains(&open), "{open}");
        }
    }
}
