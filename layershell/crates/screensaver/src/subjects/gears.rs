//! A pair of involute spur gears in mesh, keyed to their shafts.
use std::f32::consts::{PI, TAU};

use crate::draft::Placement::Auto;
use crate::draft::{
    Characteristic, Draft, Extent, Fill, Line, Tone, V2, arc_points, number, polar, v,
};

use super::{Card, Detail, Domain, Part, Place, Reading, Revision, Subject, Unit, View};

const MODULE: f32 = 2.0;
const PRESSURE_ANGLE: f32 = 20.0 * PI / 180.0;
const PINION_TEETH: u32 = 18;
const GEAR_TEETH: u32 = 30;
/// The pinion's speed: a slow turn, readable on a screensaver.
const PINION_RPM: f32 = 4.0;

const PINION: V2 = v(-30.0, 0.0);
const GEAR: V2 = v(18.0, 0.0);

/// A shaft and its key: bore radius, key width, depth into the shaft and
/// into the hub (ISO 6885 sizes for the bore).
struct Keyed {
    radius: f32,
    key: f32,
    shaft_depth: f32,
    hub_depth: f32,
}

const PINION_SHAFT: Keyed = Keyed {
    radius: 4.0,
    key: 2.0,
    shaft_depth: 1.2,
    hub_depth: 1.0,
};

const GEAR_SHAFT: Keyed = Keyed {
    radius: 6.0,
    key: 4.0,
    shaft_depth: 2.5,
    hub_depth: 1.8,
};

/// The axial dimensions, which the section shows: each wheel's face, the
/// gear's web and hub, and how far the shafts are drawn past their hubs.
const PINION_FACE: f32 = 22.0;
const GEAR_FACE: f32 = 20.0;
const WEB: f32 = 8.0;
const HUB_LENGTH: f32 = 26.0;
const HUB_RADIUS: f32 = 10.0;
const RIM_RADIUS: f32 = 24.0;
const SHAFT_OVERHANG: f32 = 8.0;

/// Lightening holes in the gear's web.
const HOLES: u32 = 6;
const HOLE_RADIUS: f32 = 4.0;
const HOLE_CIRCLE: f32 = 20.0;

pub struct Gears {
    card: Card,
}

/// The circles of a spur gear.
struct Wheel {
    teeth: u32,
    pitch: f32,
    base: f32,
    tip: f32,
    root: f32,
}

impl Wheel {
    fn new(teeth: u32) -> Self {
        let pitch = MODULE * teeth as f32 / 2.0;

        Self {
            teeth,
            pitch,
            base: pitch * PRESSURE_ANGLE.cos(),
            tip: pitch + MODULE,
            root: pitch - 1.25 * MODULE,
        }
    }

    /// The closed outline of every tooth, the first centred on `rotation`.
    fn outline(&self, centre: V2, rotation: f32) -> Vec<V2> {
        let involute = |angle: f32| angle.tan() - angle;
        let pressure_at = |radius: f32| (self.base / radius).clamp(-1.0, 1.0).acos();

        // Half a tooth's angular thickness at the pitch circle, and where its
        // flank leaves the base circle.
        let half = PI / (2.0 * self.teeth as f32);
        let foot = half + involute(PRESSURE_ANGLE);
        let roll_tip = ((self.tip / self.base).powi(2) - 1.0).sqrt();
        let step = TAU / self.teeth as f32;

        // One flank, from the base circle (or the root, below it) to the tip,
        // as (radius, angle from the tooth's centre line).
        let mut flank = Vec::new();

        if self.root < self.base {
            flank.push((self.root, foot));
        }

        const STEPS: usize = 16;

        for i in 0..=STEPS {
            let roll = roll_tip * i as f32 / STEPS as f32;
            let radius = self.base * (1.0 + roll * roll).sqrt();

            flank.push((radius, foot - involute(pressure_at(radius))));
        }

        let tip_half = flank.last().expect("A flank").1;
        let mut points = Vec::new();

        for tooth in 0..self.teeth {
            let middle = rotation + tooth as f32 * step;
            let at = |radius: f32, angle: f32| centre + polar(radius, middle + angle);

            // Up the trailing flank, across the tip, down the leading one.
            points.extend(flank.iter().map(|&(radius, angle)| at(radius, -angle)));
            points.extend(
                arc_points(centre, self.tip, middle - tip_half, 2.0 * tip_half, 3)[1..3].iter(),
            );
            points.extend(flank.iter().rev().map(|&(radius, angle)| at(radius, angle)));

            // Along the root to the next tooth.
            let gap_from = middle + foot;
            let gap = step - 2.0 * foot;
            points.extend(arc_points(centre, self.root, gap_from, gap, 4)[1..4].iter());
        }

        points
    }
}

/// A bore or shaft outline with its keyway: a circle of `radius` broken by a
/// notch `width` wide reaching `depth` outward (a hub) or inward (a shaft,
/// negative depth), centred on `angle`.
fn keyed_circle(centre: V2, radius: f32, width: f32, depth: f32, angle: f32) -> Vec<V2> {
    let half = (width / 2.0 / radius).asin();
    let mut points = arc_points(centre, radius, angle + half, TAU - 2.0 * half, 40);
    let edge = |along: f32, side: f32| centre + polar(along, angle) + polar(side, angle + PI / 2.0);
    let inner = (radius * radius - width * width / 4.0).sqrt();

    points.push(edge(inner + depth, -width / 2.0));
    points.push(edge(inner + depth, width / 2.0));

    points
}

/// Section A–A, through both shafts: across it the front view's `x`, up it
/// the depth along the shafts.
///
/// As a drawing sections an assembly: the wheels' bodies are lined, each
/// its own way; their teeth, the shafts and the keys are not; where the
/// teeth mesh the pinion's are seen and the gear's are hidden behind them.
fn section(d: &mut Draft) {
    let (pinion, gear) = (Wheel::new(PINION_TEETH), Wheel::new(GEAR_TEETH));
    // A half-profile, as (radius, depth) points, on one side of an axis.
    let side = |centre: f32, sign: f32, profile: &[(f32, f32)]| -> Vec<V2> {
        profile
            .iter()
            .map(|&(radius, depth)| v(centre + sign * radius, depth))
            .collect()
    };
    let rectangle =
        |x0: f32, x1: f32, z0: f32, z1: f32| [v(x0, z0), v(x1, z0), v(x1, z1), v(x0, z1)];

    // The pinion: solid to its root, its bore round the shaft.
    let (bore, face) = (PINION_SHAFT.radius, PINION_FACE / 2.0);

    d.part(0, |d| {
        for sign in [-1.0, 1.0] {
            let body = side(
                PINION.x,
                sign,
                &[
                    (bore, -face),
                    (pinion.root, -face),
                    (pinion.root, face),
                    (bore, face),
                ],
            );

            d.area(&body, Fill::Hatch);
            d.polygon(&body, Line::Outline);

            let teeth = rectangle(
                PINION.x + sign * pinion.root,
                PINION.x + sign * pinion.tip,
                -face,
                face,
            );
            d.polygon(&teeth, Line::Outline);

            let pitch = PINION.x + sign * pinion.pitch;
            d.line(v(pitch, -face - 2.0), v(pitch, face + 2.0), Line::Centre);
        }
    });

    // The gear: a hub on its shaft, a web, and a rim under its teeth.
    let (bore, face, web, hub) = (
        GEAR_SHAFT.radius,
        GEAR_FACE / 2.0,
        WEB / 2.0,
        HUB_LENGTH / 2.0,
    );
    let key_length = HUB_LENGTH - 4.0;

    d.part(1, |d| {
        for sign in [-1.0, 1.0] {
            let mut profile = vec![
                (bore, -hub),
                (HUB_RADIUS, -hub),
                (HUB_RADIUS, -web),
                (RIM_RADIUS, -web),
                (RIM_RADIUS, -face),
                (gear.root, -face),
                (gear.root, face),
                (RIM_RADIUS, face),
                (RIM_RADIUS, web),
                (HUB_RADIUS, web),
                (HUB_RADIUS, hub),
                (bore, hub),
            ];

            // The keyway, on the side the key is drawn.
            if sign > 0.0 {
                let depth = bore + GEAR_SHAFT.hub_depth;
                profile.extend([
                    (bore, key_length / 2.0),
                    (depth, key_length / 2.0),
                    (depth, -key_length / 2.0),
                    (bore, -key_length / 2.0),
                ]);
            }

            let body = side(GEAR.x, sign, &profile);

            d.area(&body, Fill::CrossHatch);
            d.polygon(&body, Line::Outline);

            let pitch = GEAR.x + sign * gear.pitch;
            d.line(v(pitch, -face - 2.0), v(pitch, face + 2.0), Line::Centre);
        }

        // The teeth away from the mesh, and those in it, behind the
        // pinion's.
        d.polygon(
            &rectangle(GEAR.x + gear.root, GEAR.x + gear.tip, -face, face),
            Line::Outline,
        );

        let (tip, root) = (GEAR.x - gear.tip, GEAR.x - gear.root);
        let pinion_tip = PINION.x + pinion.tip;

        d.line(v(tip, -face), v(tip, face), Line::Hidden);
        for depth in [-face, face] {
            d.line(v(tip, depth), v(pinion_tip, depth), Line::Hidden);
            d.line(v(pinion_tip, depth), v(root, depth), Line::Outline);
        }
    });

    // The shafts and keys, which a section does not cut.
    for (centre, keyed, length) in [
        (PINION.x, &PINION_SHAFT, PINION_FACE / 2.0 + SHAFT_OVERHANG),
        (GEAR.x, &GEAR_SHAFT, HUB_LENGTH / 2.0 + SHAFT_OVERHANG),
    ] {
        d.line(
            v(centre, -length - 3.0),
            v(centre, length + 3.0),
            Line::Centre,
        );
        d.part(2, |d| {
            d.polygon(
                &rectangle(
                    centre - keyed.radius,
                    centre + keyed.radius,
                    -length,
                    length,
                ),
                Line::Outline,
            );
        });
    }

    d.part(3, |d| {
        let inner = GEAR_SHAFT.radius - GEAR_SHAFT.shaft_depth;
        let outer = GEAR_SHAFT.radius + GEAR_SHAFT.hub_depth;

        d.polygon(
            &rectangle(
                GEAR.x + inner,
                GEAR.x + outer,
                -key_length / 2.0,
                key_length / 2.0,
            ),
            Line::Outline,
        );
    });

    d.dim_v(
        v(GEAR.x + gear.tip, -face),
        v(GEAR.x + gear.tip, face),
        14.0,
    );
    d.dim_v(
        v(PINION.x - pinion.tip, -PINION_FACE / 2.0),
        v(PINION.x - pinion.tip, PINION_FACE / 2.0),
        -5.0,
    );

    // The gear's shaft is the datum its faces run true to, and its teeth
    // are ground.
    let shaft_end = HUB_LENGTH / 2.0 + SHAFT_OVERHANG;

    d.datum(
        v(GEAR.x - GEAR_SHAFT.radius, -(hub + shaft_end) / 2.0),
        v(-1.0, 0.0),
        'A',
    );
    d.control(
        v(GEAR.x + (RIM_RADIUS + gear.root) / 2.0, -face),
        (14, 10),
        Characteristic::Perpendicularity,
        "0.02",
        "A",
    );
    d.finish(v(GEAR.x + (gear.root + gear.tip) / 2.0, face), "Ra 0.8");
}

/// The key's section: a rectangle across the joint of shaft and hub.
fn key(centre: V2, keyed: &Keyed, angle: f32) -> Vec<V2> {
    let inner = (keyed.radius * keyed.radius - keyed.key * keyed.key / 4.0).sqrt();
    let corner =
        |along: f32, side: f32| centre + polar(along, angle) + polar(side, angle + PI / 2.0);
    let (low, high) = (inner - keyed.shaft_depth, inner + keyed.hub_depth);
    let half = keyed.key / 2.0;

    vec![
        corner(low, -half),
        corner(high, -half),
        corner(high, half),
        corner(low, half),
    ]
}

impl Gears {
    pub fn new() -> Self {
        let (pinion, gear) = (Wheel::new(PINION_TEETH), Wheel::new(GEAR_TEETH));
        let ratio = GEAR_TEETH as f32 / PINION_TEETH as f32;
        let contact = Self::path_of_contact();
        let base_pitch = PI * MODULE * PRESSURE_ANGLE.cos();
        let diameter = |radius: f32| format!("Ø{}", number(2.0 * radius));

        let card = Card {
            title: "SPUR GEAR PAIR".into(),
            number: "QD-M-0118".into(),
            domain: Domain::Mechanical,
            unit: Unit::Millimetre,
            scaled: true,
            view: "FRONT VIEW, SHAFTS IN SECTION".into(),
            notes: vec![
                format!(
                    "INVOLUTE TEETH, MODULE {}, PRESSURE ANGLE 20°",
                    number(MODULE)
                ),
                format!("RATIO {GEAR_TEETH}:{PINION_TEETH} = {:.3}", ratio),
                format!(
                    "CONTACT RATIO {:.2}: TWO PAIRS IN MESH PART OF THE TIME",
                    (contact.1 - contact.0) / base_pitch
                ),
                "KEYS TO ISO 6885".into(),
            ],
            revisions: vec![
                Revision::first(),
                Revision::new('B', "SECTION A–A ADDED", "2026-10-07"),
            ],
            parts: vec![
                Part::new("PINION", 1, "STEEL")
                    .spec("TEETH", PINION_TEETH.to_string())
                    .spec("PITCH", diameter(pinion.pitch))
                    .spec("TIP", diameter(pinion.tip))
                    .spec("ROOT", diameter(pinion.root))
                    .spec("BASE", diameter(pinion.base))
                    .detail(PINION + v(pinion.pitch, 0.0), 4.0),
                Part::new("GEAR", 1, "STEEL")
                    .spec("TEETH", GEAR_TEETH.to_string())
                    .spec("PITCH", diameter(gear.pitch))
                    .spec("TIP", diameter(gear.tip))
                    .spec("ROOT", diameter(gear.root))
                    .spec("HOLES", format!("{HOLES} × Ø{}", number(2.0 * HOLE_RADIUS)))
                    .detail(GEAR + v(0.0, gear.pitch), 4.5),
                Part::new("SHAFT", 2, "STEEL")
                    .spec("PINION", diameter(PINION_SHAFT.radius))
                    .spec("GEAR", diameter(GEAR_SHAFT.radius))
                    .spec("FIT", "H7/k6")
                    .detail(PINION, 5.5),
                Part::new("KEY", 2, "STEEL")
                    .spec("PINION", format!("{0} × {0}", number(PINION_SHAFT.key)))
                    .spec("GEAR", format!("{0} × {0}", number(GEAR_SHAFT.key)))
                    .spec("FORM", "A, ROUND ENDS")
                    .detail(GEAR, 8.0),
            ],
        };

        Self { card }
    }

    /// The pinion's and the gear's rotation at `t`: the gear turns the other
    /// way, slower by the ratio, and starts half a pitch round so that a gap
    /// faces the pinion's tooth.
    fn angles(t: f32) -> (f32, f32) {
        let pinion = TAU * PINION_RPM / 60.0 * t;
        let gear = PI + PI / GEAR_TEETH as f32 - pinion * PINION_TEETH as f32 / GEAR_TEETH as f32;

        (pinion, gear)
    }

    /// The line of action: tangent to both base circles through the pitch
    /// point, at the pressure angle to the common tangent. Its direction from
    /// the pinion's tangent point towards the gear's, and the two points.
    fn line_of_action() -> (V2, V2, V2) {
        let normal = v(PRESSURE_ANGLE.cos(), PRESSURE_ANGLE.sin());
        let (pinion, gear) = (Wheel::new(PINION_TEETH), Wheel::new(GEAR_TEETH));
        let (from, to) = (PINION + normal * pinion.base, GEAR - normal * gear.base);

        ((to - from).normalize_or_zero(), from, to)
    }

    /// Where along the line of action (from the pinion's tangent point) the
    /// teeth touch: from where it enters the gear's tip circle to where it
    /// leaves the pinion's.
    fn path_of_contact() -> (f32, f32) {
        let (direction, from, _) = Self::line_of_action();
        let (pinion, gear) = (Wheel::new(PINION_TEETH), Wheel::new(GEAR_TEETH));

        // The two parameters where the line crosses a circle.
        let crossing = |centre: V2, radius: f32, sign: f32| {
            let offset = from - centre;
            let b = offset.dot(direction);
            let c = offset.dot(offset) - radius * radius;

            -b + sign * (b * b - c).max(0.0).sqrt()
        };

        (
            crossing(GEAR, gear.tip, -1.0),
            crossing(PINION, pinion.tip, 1.0),
        )
    }

    /// Where the teeth touch with the pinion at `angle`, along the line of
    /// action.
    ///
    /// A leading flank of the pinion is an involute whose generating line
    /// touches the base circle at `angle + foot − roll`; it lies on the line
    /// of action when that is the pressure angle, at `roll × base radius`
    /// along it. The teeth repeat a base pitch apart.
    fn contacts(angle: f32) -> Vec<f32> {
        let pinion = Wheel::new(PINION_TEETH);
        let involute = |angle: f32| angle.tan() - angle;
        let foot = PI / (2.0 * PINION_TEETH as f32) + involute(PRESSURE_ANGLE);
        let base_pitch = PI * MODULE * PRESSURE_ANGLE.cos();
        let (start, end) = Self::path_of_contact();
        let first = (pinion.base * (angle + foot - PRESSURE_ANGLE)).rem_euclid(base_pitch);

        (0..)
            .map(|k| first + k as f32 * base_pitch)
            .take_while(|along| *along <= end)
            .filter(|along| *along >= start)
            .collect()
    }
}

impl Subject for Gears {
    fn name(&self) -> &'static str {
        "gears"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        Extent::new(v(-58.0, -38.0), v(58.0, 40.0))
    }

    fn views(&self) -> Vec<View> {
        let depth = HUB_LENGTH / 2.0 + SHAFT_OVERHANG + 4.0;

        vec![View {
            name: "SECTION A–A".into(),
            place: Place::Under,
            extent: Extent::new(v(-58.0, -depth), v(58.0, depth)),
        }]
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let (pinion, gear) = (Wheel::new(PINION_TEETH), Wheel::new(GEAR_TEETH));
        let (pinion_angle, gear_angle) = Self::angles(t);
        let pitch_point = PINION + v(pinion.pitch, 0.0);
        let (direction, from, to) = Self::line_of_action();
        let (start, end) = Self::path_of_contact();

        // Construction: centre lines, pitch circles, the line of action.
        d.centre_mark(PINION, pinion.tip, 4.0);
        d.centre_mark(GEAR, gear.tip, 4.0);
        d.line(
            PINION + v(pinion.tip + 4.0, 0.0),
            GEAR - v(gear.tip + 4.0, 0.0),
            Line::Centre,
        );
        d.part(0, |d| {
            d.circle(PINION, pinion.pitch, Line::Centre);
        });
        d.part(1, |d| {
            d.circle(GEAR, gear.pitch, Line::Centre);
            d.circle(GEAR, HOLE_CIRCLE, Line::Centre);
        });
        d.line(from - direction * 6.0, to + direction * 6.0, Line::Phantom);
        d.line(
            pitch_point - v(0.0, 14.0),
            pitch_point + v(0.0, 16.0),
            Line::Thin,
        );

        d.moving(|d| {
            d.part(0, |d| {
                d.polygon(&pinion.outline(PINION, pinion_angle), Line::Outline);
                d.polygon(
                    &keyed_circle(
                        PINION,
                        PINION_SHAFT.radius,
                        PINION_SHAFT.key,
                        PINION_SHAFT.hub_depth,
                        pinion_angle + PI / 2.0,
                    ),
                    Line::Outline,
                );
            });

            d.part(1, |d| {
                d.polygon(&gear.outline(GEAR, gear_angle), Line::Outline);
                d.polygon(
                    &keyed_circle(
                        GEAR,
                        GEAR_SHAFT.radius,
                        GEAR_SHAFT.key,
                        GEAR_SHAFT.hub_depth,
                        gear_angle + PI / 2.0,
                    ),
                    Line::Outline,
                );
                d.circle(GEAR, GEAR_SHAFT.radius + 4.0, Line::Hidden);

                for hole in 0..HOLES {
                    let at =
                        GEAR + polar(HOLE_CIRCLE, gear_angle + TAU * hole as f32 / HOLES as f32);

                    d.circle(at, HOLE_RADIUS, Line::Outline);
                }
            });

            // The shafts are cut: section lining, and the keys the other way.
            for (centre, keyed, angle) in [
                (PINION, &PINION_SHAFT, pinion_angle + PI / 2.0),
                (GEAR, &GEAR_SHAFT, gear_angle + PI / 2.0),
            ] {
                let shaft = keyed_circle(
                    centre,
                    keyed.radius - 0.3,
                    keyed.key,
                    -keyed.shaft_depth,
                    angle,
                );
                let section = key(centre, keyed, angle);

                d.part(2, |d| {
                    d.hatch(&shaft);
                    d.polygon(&shaft, Line::Outline);
                });
                d.part(3, |d| {
                    d.area(&section, Fill::CrossHatch);
                    d.polygon(&section, Line::Outline);
                });
            }

            // Where the teeth touch, sliding along the line of action.
            for along in Self::contacts(pinion_angle) {
                d.dot(from + direction * along, 3).tone(Tone::Accent);
            }
        });

        // The path of contact, and what the drawing says.
        d.line(
            from + direction * start,
            from + direction * end,
            Line::Trace,
        );

        d.dim_angle(pitch_point, PI / 2.0, PI / 2.0 + PRESSURE_ANGLE, 34)
            .text("20°");
        d.dim_h(PINION, GEAR, -36.0).tolerance(0.02, -0.02);

        // Where section A–A is cut, and what it shows.
        d.cutting_plane(
            0,
            v(PINION.x - pinion.tip - 6.0, 0.0),
            v(GEAR.x + gear.tip + 6.0, 0.0),
            v(0.0, -1.0),
            'A',
        );
        d.in_view(0, section);

        d.part(0, |d| {
            d.dim_diameter(PINION, pinion.tip, 2.2, 18);
            d.balloon(0, PINION + polar(pinion.tip - 1.0, 2.7), Auto);
        });
        d.part(1, |d| {
            d.dim_diameter(GEAR, gear.tip, 0.75, 16);
            d.balloon(1, GEAR + polar(13.5, -0.6), Auto);
        });
        d.part(2, |d| {
            d.balloon(2, PINION + v(-1.5, -2.0), Auto);
        });

        d.moving(|d| {
            d.part(3, |d| {
                let (centre, keyed, angle) = (GEAR, &GEAR_SHAFT, gear_angle + PI / 2.0);
                let inner = (keyed.radius * keyed.radius - keyed.key * keyed.key / 4.0).sqrt();
                let middle =
                    centre + polar(inner + (keyed.hub_depth - keyed.shaft_depth) / 2.0, angle);

                d.balloon(3, middle, Auto);
            });
        });

        // Small features, legible only in a detail view.
        d.in_detail(|d| {
            d.part(0, |d| {
                d.circle(PINION, pinion.base, Line::Phantom);
                d.circle(PINION, pinion.root, Line::Thin);
                d.note(
                    PINION + polar(pinion.pitch, 0.16),
                    (-30, -22),
                    "PITCH CIRCLE",
                );
            });
            d.part(2, |d| {
                d.dim_diameter(PINION, PINION_SHAFT.radius, -0.5, 20)
                    .fit("H7/k6");
            });
        });
    }

    fn detail(&self, index: usize, t: f32) -> Option<Detail> {
        match index {
            // The gear's key, round with its shaft.
            3 => {
                let (_, gear) = Self::angles(t);
                Some(Detail::following(
                    GEAR + polar(GEAR_SHAFT.radius, gear + PI / 2.0),
                    5.0,
                ))
            }
            _ => self.card.parts.get(index)?.detail,
        }
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let (pinion, gear) = Self::angles(t);
        let degrees = |angle: f32| format!("{:5.1}°", angle.to_degrees().rem_euclid(360.0));
        let gear_rpm = PINION_RPM * PINION_TEETH as f32 / GEAR_TEETH as f32;

        vec![
            Reading::new(
                "PINION",
                format!("{} {} rpm", degrees(pinion), number(PINION_RPM)),
            ),
            Reading::new(
                "GEAR",
                format!(
                    "{} {} rpm",
                    degrees(gear - PI - PI / GEAR_TEETH as f32),
                    number(gear_rpm)
                ),
            ),
            Reading::new(
                "IN MESH",
                match Self::contacts(pinion).len() {
                    1 => "1 PAIR".to_owned(),
                    pairs => format!("{pairs} PAIRS"),
                },
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `point` is inside the closed `polygon` (even-odd).
    fn inside(point: V2, polygon: &[V2]) -> bool {
        let mut crossed = false;

        for i in 0..polygon.len() {
            let (a, b) = (polygon[i], polygon[(i + 1) % polygon.len()]);

            if (a.y > point.y) != (b.y > point.y)
                && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
            {
                crossed = !crossed;
            }
        }

        crossed
    }

    #[test]
    fn the_teeth_mesh_without_cutting_into_each_other() {
        let (pinion, gear) = (Wheel::new(PINION_TEETH), Wheel::new(GEAR_TEETH));

        for step in 0..200 {
            let t = step as f32 * 0.37;
            let (pinion_angle, gear_angle) = Gears::angles(t);
            let gear_outline = gear.outline(GEAR, gear_angle);
            let cutting: Vec<_> = pinion
                .outline(PINION, pinion_angle)
                .into_iter()
                .filter(|point| point.distance(GEAR) < gear.tip && inside(*point, &gear_outline))
                // A point on a flank in contact may sit a hair inside.
                .filter(|point| {
                    gear_outline
                        .iter()
                        .all(|other| other.distance(*point) > 0.05)
                })
                .collect();

            assert!(
                cutting.is_empty(),
                "at {t} s the pinion cuts the gear at {cutting:?}"
            );
        }
    }

    #[test]
    fn contact_is_on_the_path_and_one_or_two_pairs_share_it() {
        let (start, end) = Gears::path_of_contact();

        assert!(start > 0.0 && end > start);

        for step in 0..100 {
            let contacts = Gears::contacts(step as f32 * 0.05);

            assert!((1..=2).contains(&contacts.len()), "{contacts:?}");
        }
    }
}
