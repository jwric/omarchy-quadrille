//! A 555 timer as an astable flasher: the schematic, its current flowing
//! as the capacitor charges and discharges, and a scope sweeping both.
use quadrille::draw::Anchor;

use crate::draft::geom::{along, length};
use crate::draft::{Draft, Extent, Line, Tone, V2, number, v};

use super::schematic::Schematic;
use super::{Card, Domain, Part, Reading, Subject, Unit};

const SUPPLY: f32 = 5.0;
const R1: f32 = 1.0e3;
const R2: f32 = 10.0e3;
const C1: f32 = 47.0e-6;
const R3: f32 = 330.0;
const LED_DROP: f32 = 2.0;

/// The grid: the supply rails, and the column the timing parts stand in.
const RAIL: f32 = 64.0;
const GROUND: f32 = -16.0;
const TIMING: f32 = -22.0;
const OUT: f32 = 50.0;

/// The IC's body and its pins: (number, name, where the pin's lead ends).
const BODY: (V2, V2) = (v(0.0, 0.0), v(32.0, 44.0));
const PINS: [(u8, &str, V2, V2); 8] = [
    (7, "DIS", v(0.0, 36.0), v(-6.0, 36.0)),
    (6, "THR", v(0.0, 22.0), v(-6.0, 22.0)),
    (2, "TRIG", v(0.0, 8.0), v(-6.0, 8.0)),
    (8, "VCC", v(8.0, 44.0), v(8.0, 50.0)),
    (4, "RST", v(24.0, 44.0), v(24.0, 50.0)),
    (1, "GND", v(8.0, 0.0), v(8.0, -6.0)),
    (5, "CV", v(24.0, 0.0), v(24.0, -6.0)),
    (3, "OUT", v(32.0, 22.0), v(38.0, 22.0)),
];

/// The scope's screen: its corner, size, divisions and seconds a division.
const SCREEN: V2 = v(66.0, -6.0);
const SCREEN_SIZE: V2 = v(72.0, 54.0);
const DIVISIONS: (u32, u32) = (8, 6);
const PER_DIVISION: f32 = 0.2;

pub struct Timer {
    card: Card,
}

/// Seconds the output is high, and low.
fn high() -> f32 {
    std::f32::consts::LN_2 * (R1 + R2) * C1
}

fn low() -> f32 {
    std::f32::consts::LN_2 * R2 * C1
}

/// The capacitor's voltage and whether the output is high, at `t`, once
/// the circuit is running steadily: it charges through R1 and R2 from a
/// third of the supply to two thirds, then discharges through R2 alone.
fn state(t: f32) -> (f32, bool) {
    let into = t.rem_euclid(high() + low());
    let (third, two_thirds) = (SUPPLY / 3.0, 2.0 * SUPPLY / 3.0);

    if into < high() {
        let decay = (-into / ((R1 + R2) * C1)).exp();
        (SUPPLY - (SUPPLY - third) * decay, true)
    } else {
        let decay = (-(into - high()) / (R2 * C1)).exp();
        (two_thirds * decay, false)
    }
}

impl Timer {
    pub fn new() -> Self {
        let frequency = 1.0 / (high() + low());
        let ms = |seconds: f32| format!("{} ms", (seconds * 1000.0).round());

        let card = Card {
            title: "555 ASTABLE FLASHER".into(),
            number: "QD-E-0555".into(),
            domain: Domain::Electrical,
            unit: Unit::Millimetre,
            scaled: false,
            view: "SCHEMATIC".into(),
            notes: vec![
                format!("HIGH 0.693 (R1 + R2) C1 = {}", ms(high())),
                format!("LOW 0.693 R2 C1 = {}", ms(low())),
                format!("f = 1.44 / ((R1 + 2 R2) C1) = {:.2} Hz", frequency),
                format!("SUPPLY {} V; PIN 5 NOT CONNECTED", number(SUPPLY)),
            ],
            parts: vec![
                Part::new("U1 NE555", 1, "DIP-8")
                    .spec("SUPPLY", "4.5 TO 16 V")
                    .spec("THRESHOLDS", "1/3 AND 2/3 VCC")
                    .spec("OUTPUT", "200 mA SINK/SOURCE")
                    .detail(v(16.0, 22.0), 24.0),
                Part::new("R1", 1, "1 kΩ")
                    .spec("ROLE", "CHARGES C1 WITH R2")
                    .spec("POWER", "0.25 W, 5 %")
                    .detail(v(TIMING, 50.0), 9.0),
                Part::new("R2", 1, "10 kΩ")
                    .spec("ROLE", "CHARGES AND DISCHARGES C1")
                    .spec("POWER", "0.25 W, 5 %")
                    .detail(v(TIMING, 29.0), 9.0),
                Part::new("C1", 1, "47 µF")
                    .spec("TYPE", "ELECTROLYTIC, 16 V")
                    .spec(
                        "SWING",
                        format!("{:.2} TO {:.2} V", SUPPLY / 3.0, 2.0 * SUPPLY / 3.0),
                    )
                    .detail(v(TIMING, -4.0), 9.0),
                Part::new("R3", 1, "330 Ω")
                    .spec("ROLE", "LIMITS THE LED")
                    .spec(
                        "CURRENT",
                        format!("{} mA", number((SUPPLY - LED_DROP) / R3 * 1000.0)),
                    )
                    .detail(v(OUT, 13.0), 9.0),
                Part::new("D1 LED", 1, "RED 5 mm")
                    .spec("FORWARD", format!("{} V", number(LED_DROP)))
                    .detail(v(OUT, -3.0), 9.0),
            ],
        };

        Self { card }
    }

    /// The 555 itself: body, pins with numbers and names.
    fn chip(d: &mut Draft) {
        let (low, high) = BODY;

        d.rect(low, high, Line::Outline);
        d.label(v(16.0, 30.0), "U1").tone(Tone::Ink);
        d.label(v(16.0, 24.0), "NE555");

        for (number, name, at, end) in PINS {
            d.line(at, end, Line::Outline);

            let inward = (at - end).normalize_or_zero();
            let anchor = match (inward.x as i32, inward.y as i32) {
                (1, _) => Anchor::LEFT,
                (-1, _) => Anchor::RIGHT,
                (_, 1) => Anchor::new(
                    quadrille::draw::Horizontal::Centre,
                    quadrille::draw::Vertical::Top,
                ),
                _ => Anchor::new(
                    quadrille::draw::Horizontal::Centre,
                    quadrille::draw::Vertical::Bottom,
                ),
            };

            d.label(at + inward * 1.5, name)
                .anchor(anchor)
                .tone(Tone::Faint);
            d.label(end.lerp(at, 0.5) + inward.perp() * 2.5, number.to_string())
                .tone(Tone::Faint);
        }
    }

    /// What is inside the 555, legible only in a detail: the divider, the
    /// two comparators, the flip-flop, the discharge transistor and the
    /// output stage.
    fn inside(d: &mut Draft) {
        // The divider of three equal resistors from VCC to ground.
        for (top, bottom) in [(40.0, 33.0), (31.0, 18.0), (16.0, 4.0)] {
            d.rect(v(4.0, bottom), v(6.0, top), Line::Thin);
        }
        d.label(v(5.0, 25.0), "5k").nudge(-8, 0);

        // The comparators: threshold against 2/3, trigger against 1/3.
        for (y, name) in [(30.0, "+"), (12.0, "−")] {
            d.polygon(
                &[v(10.0, y - 4.0), v(10.0, y + 4.0), v(16.0, y)],
                Line::Thin,
            );
            d.label(v(13.0, y), name).tone(Tone::Faint);
            d.line(v(6.0, y + 2.0), v(10.0, y + 2.0), Line::Thin);
        }

        // The flip-flop, the discharge transistor and the output buffer.
        d.rect(v(19.0, 14.0), v(25.0, 30.0), Line::Thin);
        d.label(v(22.0, 26.0), "R");
        d.label(v(22.0, 18.0), "S");
        d.line(v(16.0, 30.0), v(19.0, 26.0), Line::Thin);
        d.line(v(16.0, 12.0), v(19.0, 18.0), Line::Thin);
        d.polygon(&[v(27.0, 18.0), v(27.0, 26.0), v(31.0, 22.0)], Line::Thin);
        d.line(v(25.0, 22.0), v(27.0, 22.0), Line::Thin);
        d.circle(v(22.0, 37.0), 3.0, Line::Thin);
        d.line(v(22.0, 30.0), v(22.0, 34.0), Line::Thin);
    }

    /// The scope: graticule, the two thresholds, and the traces swept up to
    /// `t`, the beam a dot at the sweep's edge.
    fn scope(d: &mut Draft, t: f32) {
        let (across, down) = DIVISIONS;
        let corner = SCREEN + SCREEN_SIZE;
        let window = PER_DIVISION * across as f32;

        d.rect(SCREEN - v(2.0, 2.0), corner + v(2.0, 2.0), Line::Outline);

        for i in 1..across {
            let x = SCREEN.x + SCREEN_SIZE.x * i as f32 / across as f32;
            d.line(v(x, SCREEN.y), v(x, corner.y), Line::Path)
                .tone(Tone::Faint);
        }
        for j in 1..down {
            let y = SCREEN.y + SCREEN_SIZE.y * j as f32 / down as f32;
            d.line(v(SCREEN.x, y), v(corner.x, y), Line::Path)
                .tone(Tone::Faint);
        }

        // Channel 1 (the capacitor) in the lower half, 1 V a division from
        // 1 V; channel 2 (the output) in the upper, 2.5 V a division.
        let division = SCREEN_SIZE.y / down as f32;
        let ch1 = |volts: f32| SCREEN.y + (volts - 1.0) * division;
        let ch2 = |volts: f32| SCREEN.y + 3.5 * division + volts / 2.5 * division;

        for threshold in [SUPPLY / 3.0, 2.0 * SUPPLY / 3.0] {
            d.line(
                v(SCREEN.x, ch1(threshold)),
                v(corner.x, ch1(threshold)),
                Line::Hidden,
            )
            .tone(Tone::Faint);
        }

        d.label(
            v(SCREEN.x, corner.y + 4.0),
            "CH1 C1 1 V/DIV  CH2 OUT 2.5 V/DIV",
        )
        .anchor(Anchor::LEFT)
        .tone(Tone::Faint);
        d.label(
            v(corner.x, SCREEN.y - 6.0),
            format!("{} ms/DIV", number(PER_DIVISION * 1000.0)),
        )
        .anchor(Anchor::RIGHT)
        .tone(Tone::Faint);

        d.moving(|d| {
            // Sweep mode: the beam writes left to right over the last sweep,
            // with a gap ahead of it.
            let beam = t.rem_euclid(window);
            let gap = window * 0.06;
            let x = |seconds: f32| SCREEN.x + seconds / window * SCREEN_SIZE.x;
            let samples = 160;

            for (channel, tone) in [(0, Tone::Live), (1, Tone::Accent)] {
                let mut trace: Vec<V2> = Vec::new();

                for i in 0..=samples {
                    let at = i as f32 / samples as f32 * window;
                    // The sample is from this sweep left of the beam, from
                    // the last one right of the gap.
                    let when = if at <= beam {
                        t - (beam - at)
                    } else {
                        t - (beam - at) - window
                    };

                    if at > beam && at < beam + gap {
                        if trace.len() > 1 {
                            d.polyline(&trace, Line::Trace).tone(tone);
                        }
                        trace.clear();
                        continue;
                    }

                    let (volts, output) = state(when);
                    let y = if channel == 0 {
                        ch1(volts)
                    } else {
                        ch2(if output { SUPPLY } else { 0.0 })
                    };

                    trace.push(v(x(at), y));
                }

                if trace.len() > 1 {
                    d.polyline(&trace, Line::Trace).tone(tone);
                }
            }

            let (volts, _) = state(t);
            d.dot(v(x(beam), ch1(volts)), 3).tone(Tone::Ink);
        });
    }
}

impl Subject for Timer {
    fn name(&self) -> &'static str {
        "timer"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        Extent::new(v(-46.0, -24.0), v(142.0, 72.0))
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let (_, output) = state(t);

        // The rails and the supply.
        d.terminal(v(-36.0, RAIL), "+5 V");
        d.terminal(v(-36.0, GROUND), "0 V");
        d.wire(&[v(-34.8, RAIL), v(24.0, RAIL), v(24.0, 50.0)]);
        d.wire(&[v(8.0, RAIL), v(8.0, 50.0)]);
        d.wire(&[v(-34.8, GROUND), v(OUT, GROUND), v(OUT, -10.0)]);
        d.wire(&[v(8.0, -6.0), v(8.0, GROUND)]);

        d.part(0, |d| {
            Self::chip(d);
            d.in_detail(Self::inside);
        });

        // Pin 5 is left open.
        d.line(v(22.5, -7.5), v(25.5, -4.5), Line::Outline);
        d.line(v(22.5, -4.5), v(25.5, -7.5), Line::Outline);

        // The timing column: R1, R2 and C1, tied to DIS, THR and TRIG.
        d.wire(&[v(-6.0, 36.0), v(TIMING, 36.0)]);
        d.wire(&[v(-6.0, 22.0), v(TIMING, 22.0), v(TIMING, 8.0)]);
        d.wire(&[v(-6.0, 8.0), v(TIMING, 8.0)]);
        d.part(1, |d| {
            d.resistor(v(TIMING, RAIL), v(TIMING, 36.0), "R1", "1k")
        });
        d.part(2, |d| {
            d.resistor(v(TIMING, 36.0), v(TIMING, 22.0), "R2", "10k")
        });
        d.part(3, |d| {
            d.capacitor(v(TIMING, 8.0), v(TIMING, GROUND), "C1", "47µ", true)
        });

        // The output through R3 and the LED to ground.
        d.wire(&[v(38.0, 22.0), v(OUT, 22.0)]);
        d.part(4, |d| d.resistor(v(OUT, 22.0), v(OUT, 4.0), "R3", "330"));
        d.part(5, |d| d.led(v(OUT, 4.0), v(OUT, -10.0), "D1", output));

        for at in [
            v(TIMING, RAIL),
            v(8.0, RAIL),
            v(TIMING, 36.0),
            v(TIMING, 22.0),
            v(TIMING, 8.0),
            v(TIMING, GROUND),
            v(8.0, GROUND),
        ] {
            d.junction(at);
        }

        // Current: down through R1 and R2 into C1 while it charges, back up
        // from C1 through R2 into DIS while it discharges.
        d.moving(|d| {
            let path: Vec<V2> = if output {
                vec![
                    v(TIMING, RAIL),
                    v(TIMING, 22.0),
                    v(TIMING, 8.0),
                    v(TIMING, 2.0),
                ]
            } else {
                vec![
                    v(TIMING, 2.0),
                    v(TIMING, 22.0),
                    v(TIMING, 36.0),
                    v(-6.0, 36.0),
                ]
            };
            let run = length(&path);
            let spacing = 6.0;
            let travelled = (t * 18.0).rem_euclid(spacing);

            for k in 0.. {
                let s = travelled + k as f32 * spacing;
                if s > run {
                    break;
                }
                d.dot(along(&path, s), 2).tone(Tone::Live);
            }

            if output {
                let path = [v(38.0, 22.0), v(OUT, 22.0), v(OUT, -10.0)];
                let travelled = (t * 18.0).rem_euclid(spacing);

                for k in 0.. {
                    let s = travelled + k as f32 * spacing;
                    if s > length(&path) {
                        break;
                    }
                    d.dot(along(&path, s), 2).tone(Tone::Accent);
                }
            }
        });

        Self::scope(d, t);

        d.part(0, |d| {
            d.balloon(0, v(30.0, 40.0), (26, -20));
        });
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let (volts, output) = state(t);
        let duty = high() / (high() + low()) * 100.0;

        vec![
            Reading::new("f", format!("{:.2} Hz", 1.0 / (high() + low()))),
            Reading::new("DUTY", format!("{} %", duty.round())),
            Reading::new("C1", format!("{volts:.2} V")),
            Reading::new("OUT", if output { "HIGH" } else { "LOW" }),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_capacitor_swings_between_a_third_and_two_thirds_of_the_supply() {
        for i in 0..2000 {
            let (volts, _) = state(i as f32 * 0.0013);

            assert!(
                (SUPPLY / 3.0 - 1e-3..=2.0 * SUPPLY / 3.0 + 1e-3).contains(&volts),
                "{volts}"
            );
        }
    }

    #[test]
    fn the_output_changes_where_the_capacitor_turns() {
        let (end_of_high, _) = state(high() - 1e-5);
        let (end_of_low, _) = state(high() + low() - 1e-5);

        assert!((end_of_high - 2.0 * SUPPLY / 3.0).abs() < 1e-3);
        assert!((end_of_low - SUPPLY / 3.0).abs() < 1e-3);
    }
}
