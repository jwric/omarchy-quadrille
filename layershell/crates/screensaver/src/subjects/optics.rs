//! A Cooke triplet with real rays: each traced through the six spherical
//! surfaces by Snell's law while the field angle sweeps.
use quadrille::draw::Anchor;

use crate::draft::{Draft, Extent, Line, Tone, V2, number, v};

use super::{Card, Detail, Domain, Part, Reading, Subject, Unit};

/// One refracting surface: its radius of curvature and the thickness and
/// refractive index (d line) of what follows it.
struct Surface {
    radius: f64,
    thickness: f64,
    index: f64,
}

const fn surface(radius: f64, thickness: f64, index: f64) -> Surface {
    Surface {
        radius,
        thickness,
        index,
    }
}

const SK16: f64 = 1.62041;
const F2: f64 = 1.62004;

/// The classic f/5, 50 mm triplet.
const PRESCRIPTION: [Surface; 6] = [
    surface(22.01359, 3.25896, SK16),
    surface(-435.7604, 6.00755, 1.0),
    surface(-22.21328, 0.99997, F2),
    surface(20.29192, 4.75041, 1.0),
    surface(79.6836, 2.95208, SK16),
    surface(-18.39533, 42.20778, 1.0),
];

/// Each element's clear radius, by its first surface.
const APERTURES: [(usize, f64); 3] = [(0, 7.5), (2, 5.0), (4, 7.0)];
/// The aperture stop sits at the fourth surface.
const STOP: usize = 3;
const ENTRANCE_PUPIL: f64 = 10.0;
const FIELD: f32 = 20.0;
/// Seconds for the field to sweep out and back.
const SWEEP: f32 = 16.0;
const RAYS: usize = 7;
/// Where the rays are drawn from, before the first surface.
const START: f64 = -12.0;

pub struct Optics {
    card: Card,
}

/// The vertex of each surface along the axis, and the image plane's.
fn vertices() -> ([f64; 6], f64) {
    let mut x = [0.0; 6];
    let mut at = 0.0;

    for (i, surface) in PRESCRIPTION.iter().enumerate() {
        x[i] = at;
        at += surface.thickness;
    }

    (x, at)
}

/// A ray: where it is and which way it goes, in the lens's millimetres.
#[derive(Debug, Clone, Copy)]
struct Ray {
    at: glam::DVec2,
    to: glam::DVec2,
}

/// Where `ray`, in glass of `index`, meets surface `i`, and the ray that
/// leaves it; `None` if it misses the sphere or is totally reflected.
fn refract(ray: Ray, i: usize, index: f64) -> Option<(glam::DVec2, Ray)> {
    let surface = &PRESCRIPTION[i];
    let centre = glam::DVec2::new(vertices().0[i] + surface.radius, 0.0);
    let offset = ray.at - centre;
    let b = offset.dot(ray.to);
    let c = offset.length_squared() - surface.radius * surface.radius;
    let discriminant = b * b - c;

    if discriminant < 0.0 {
        return None;
    }

    // The cap round the vertex: the near side of a sphere whose centre lies
    // beyond it, the far side of one whose centre lies before.
    let hit = ray.at + ray.to * (-b - surface.radius.signum() * discriminant.sqrt());

    // Snell's law, the normal turned against the ray.
    let mut normal = (hit - centre) / surface.radius;
    if normal.dot(ray.to) > 0.0 {
        normal = -normal;
    }
    let ratio = index / surface.index;
    let cos_in = -normal.dot(ray.to);
    let sin_out = ratio * ratio * (1.0 - cos_in * cos_in);

    (sin_out <= 1.0).then(|| {
        let to = ray.to * ratio + normal * (ratio * cos_in - (1.0 - sin_out).sqrt());

        (hit, Ray { at: hit, to })
    })
}

/// A surface's clear radius: its element's, or the stop's.
fn clear(i: usize) -> f64 {
    let element = APERTURES
        .iter()
        .find(|(first, _)| *first == i || *first + 1 == i)
        .map_or(f64::MAX, |(_, radius)| *radius);

    if i == STOP {
        element.min(stop_radius())
    } else {
        element
    }
}

/// The points a ray passes through from its start to the image plane, and
/// whether it got there: an aperture or a missed surface stops it.
fn trace(mut ray: Ray) -> (Vec<glam::DVec2>, bool) {
    let mut points = vec![ray.at];
    let mut index = 1.0;

    for (i, surface) in PRESCRIPTION.iter().enumerate() {
        let Some((hit, out)) = refract(ray, i, index) else {
            return (points, false);
        };

        points.push(hit);

        if hit.y.abs() > clear(i) {
            return (points, false);
        }

        ray = out;
        index = surface.index;
    }

    let s = (vertices().1 - ray.at.x) / ray.to.x;
    points.push(ray.at + ray.to * s);

    (points, true)
}

/// The height at which `ray` meets surface `last`, no aperture in its way.
fn height_at(mut ray: Ray, last: usize) -> Option<f64> {
    let mut index = 1.0;

    for (i, surface) in PRESCRIPTION.iter().enumerate().take(last + 1) {
        let (hit, out) = refract(ray, i, index)?;

        if i == last {
            return Some(hit.y);
        }

        ray = out;
        index = surface.index;
    }

    None
}

/// The stop's radius: where the edge of the axial entrance pupil meets it.
fn stop_radius() -> f64 {
    height_at(ray(0.0, ENTRANCE_PUPIL / 2.0), STOP).map_or(ENTRANCE_PUPIL / 2.0, f64::abs)
}

/// A ray at `field` degrees whose height where it starts is `height`.
fn ray(field: f32, height: f64) -> Ray {
    let angle = -f64::from(field).to_radians();

    Ray {
        at: glam::DVec2::new(START, height),
        to: glam::DVec2::new(angle.cos(), angle.sin()),
    }
}

/// The starting height of the chief ray at `field`: the one through the
/// stop's centre, found by the secant method.
fn chief(field: f32) -> f64 {
    let at_stop = |height: f64| height_at(ray(field, height), STOP).unwrap_or(height);
    let (mut a, mut b) = (0.0, 1.0);
    let (mut fa, mut fb) = (at_stop(a), at_stop(b));

    for _ in 0..12 {
        if (fb - fa).abs() < 1e-12 {
            break;
        }
        let c = b - fb * (b - a) / (fb - fa);
        let fc = at_stop(c);
        (a, fa, b, fb) = (b, fb, c, fc);
    }

    b
}

/// The field angle at `t`: out to the edge of the field and back.
fn field(t: f32) -> f32 {
    FIELD * (1.0 - (std::f32::consts::TAU * t / SWEEP).cos()) / 2.0
}

/// The fan of rays at `field`: the chief ray, and rays spread across the
/// pupil above and below it.
fn fan(field: f32) -> Vec<(Vec<glam::DVec2>, bool)> {
    let centre = chief(field);
    let spread = ENTRANCE_PUPIL / 2.0 / f64::from(field.to_radians()).cos();

    (0..RAYS)
        .map(|k| {
            let share = k as f64 / (RAYS - 1) as f64 * 2.0 - 1.0;
            trace(ray(field, centre + share * spread * 1.15))
        })
        .collect()
}

/// The paraxial focal length and back focal distance.
fn paraxial() -> (f64, f64) {
    let (mut height, mut slope, mut index) = (1.0, 0.0, 1.0);

    for (i, surface) in PRESCRIPTION.iter().enumerate() {
        slope = (index * slope - height * (surface.index - index) / surface.radius) / surface.index;
        index = surface.index;

        if i + 1 < PRESCRIPTION.len() {
            height += slope * surface.thickness;
        }
    }

    (-1.0 / slope, -height / slope)
}

fn to_model(point: glam::DVec2) -> V2 {
    v(point.x as f32, point.y as f32)
}

impl Optics {
    pub fn new() -> Self {
        let (focal, back) = paraxial();
        let element = |name: &str, glass: &str, first: usize| {
            let (a, b) = (&PRESCRIPTION[first], &PRESCRIPTION[first + 1]);
            let (vertex, _) = vertices();

            Part::new(name, 1, glass)
                .spec("RADII", format!("{:.2} / {:.2}", a.radius, b.radius))
                .spec("CENTRE", format!("{:.2} THICK", a.thickness))
                .spec("INDEX", format!("{:.4} (d)", a.index))
                .detail(v(vertex[first] as f32 + a.thickness as f32 / 2.0, 4.0), 4.5)
        };

        let card = Card {
            title: "COOKE TRIPLET".into(),
            number: "QD-O-0050".into(),
            domain: Domain::Optical,
            unit: Unit::Millimetre,
            scaled: true,
            view: "MERIDIONAL SECTION".into(),
            notes: vec![
                format!(
                    "f' {} mm, f/5, FIELD ±{}°",
                    number(focal as f32),
                    number(FIELD)
                ),
                "REAL RAYS: SNELL'S LAW AT EVERY SURFACE".into(),
                format!("BACK FOCUS {} mm (PARAXIAL)", number(back as f32)),
                "GLASS SK16 AND F2, d LINE".into(),
            ],
            parts: vec![
                element("L1", "SK16", 0),
                element("L2", "F2", 2),
                element("L3", "SK16", 4),
                Part::new("STOP", 1, "f/5")
                    .spec("DIAMETER", format!("Ø{:.2}", 2.0 * stop_radius()))
                    .spec("PUPIL", format!("Ø{}", number(ENTRANCE_PUPIL as f32))),
                Part::new("IMAGE PLANE", 1, "SENSOR")
                    .spec(
                        "HEIGHT",
                        format!(
                            "±{} mm",
                            number((focal * f64::from(FIELD).to_radians().tan()) as f32)
                        ),
                    )
                    .spec("SPOT", "SEE DETAIL"),
            ],
        };

        Self { card }
    }

    /// An element's outline: its two surfaces' arcs out to its clear
    /// radius, joined by flat edges.
    fn element(first: usize, clear: f64) -> Vec<V2> {
        let (vertex, _) = vertices();
        let sag = |surface: &Surface, height: f64| {
            surface.radius
                - surface.radius.signum() * (surface.radius.powi(2) - height * height).sqrt()
        };
        let steps = 16;
        let (front, back) = (&PRESCRIPTION[first], &PRESCRIPTION[first + 1]);
        let mut points = Vec::new();

        for k in 0..=steps {
            let h = clear * (k as f64 / steps as f64 * 2.0 - 1.0);
            points.push(v((vertex[first] + sag(front, h)) as f32, h as f32));
        }
        for k in (0..=steps).rev() {
            let h = clear * (k as f64 / steps as f64 * 2.0 - 1.0);
            points.push(v((vertex[first + 1] + sag(back, h)) as f32, h as f32));
        }

        points
    }

    /// Where the chief ray meets the image plane at `t`.
    fn image_point(t: f32) -> V2 {
        let (points, _) = trace(ray(field(t), chief(field(t))));

        points.last().copied().map(to_model).unwrap_or_default()
    }
}

impl Subject for Optics {
    fn name(&self) -> &'static str {
        "optics"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        Extent::new(v(START as f32 - 2.0, -24.0), v(66.0, 13.0))
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let (vertex, image) = vertices();
        let (focal, back) = paraxial();
        let image = image as f32;
        let focus = (vertex[5] + back) as f32;
        let principal = focus - focal as f32;

        d.line(v(START as f32, 0.0), v(image + 4.0, 0.0), Line::Centre);

        for (index, (first, clear)) in APERTURES.iter().enumerate() {
            d.part(index, |d| {
                d.polygon(&Self::element(*first, *clear), Line::Outline);
            });
        }

        // The stop: two plates leaving its aperture open.
        d.part(3, |d| {
            let x = vertex[STOP] as f32 + 1.2;
            let open = stop_radius() as f32;

            for side in [-1.0, 1.0] {
                d.rect(
                    v(x - 0.4, side * open),
                    v(x + 0.4, side * (open + 4.0)),
                    Line::Outline,
                );
            }
        });

        d.part(4, |d| {
            d.line(v(image, -22.0), v(image, 9.0), Line::Outline);
            d.line(v(image + 0.6, -22.0), v(image + 0.6, 9.0), Line::Outline);
        });

        // The rear principal plane and the focal point, and the focal
        // length between them.
        d.line(v(principal, -10.0), v(principal, 11.0), Line::Phantom);
        d.label(v(principal, -12.0), "H'").tone(Tone::Muted);
        d.label(v(focus, 0.0), "F'")
            .anchor(Anchor::RIGHT)
            .nudge(-4, -7)
            .tone(Tone::Muted);
        d.dim_h(v(principal, 9.0), v(focus, 0.0), 2.0)
            .text(format!("f' {}", number(focal as f32)));
        d.dim_h(v(vertex[5] as f32, 0.0), v(image, 0.0), -26.0)
            .text(format!("{:.2}", image - vertex[5] as f32));
        d.dim_h(v(0.0, 0.0), v(vertex[5] as f32, 0.0), -20.0);

        d.moving(|d| {
            let field = field(t);
            let rays = fan(field);

            for (k, (points, reached)) in rays.iter().enumerate() {
                let points: Vec<V2> = points.iter().copied().map(to_model).collect();
                let tone = if k == RAYS / 2 {
                    Tone::Accent
                } else if *reached {
                    Tone::Live
                } else {
                    Tone::Faint
                };

                d.polyline(&points, Line::Trace).tone(tone);
            }

            d.dot(Self::image_point(t), 3).tone(Tone::Accent);
            d.dim_angle(v(START as f32 + 4.0, 0.0), -field.to_radians(), 0.0, 40)
                .text(format!("{field:.1}°"));
        });

        d.part(0, |d| {
            d.balloon(0, v(1.5, 6.0), (-30, -30));
        });
        d.part(1, |d| {
            d.balloon(1, v(vertex[2] as f32 + 0.4, -4.0), (-18, 52));
        });
        d.part(2, |d| {
            d.balloon(2, v(vertex[4] as f32 + 1.8, 5.5), (14, -42));
        });
        d.part(3, |d| {
            d.balloon(
                3,
                v(vertex[STOP] as f32 + 1.2, -(stop_radius() as f32 + 3.0)),
                (26, 40),
            );
        });
        d.part(4, |d| {
            d.balloon(4, v(image, 7.0), (20, -24));
        });

        d.in_detail(|d| {
            d.part(4, |d| {
                d.label(Self::image_point(t), "SPOT")
                    .anchor(Anchor::LEFT)
                    .nudge(10, -12);
            });
        });
    }

    fn detail(&self, index: usize, t: f32) -> Option<Detail> {
        match index {
            // The spot where the rays meet the image plane, wherever the
            // field puts it.
            4 => Some(Detail::following(Self::image_point(t), 0.6)),
            // The stop and the rays it lets through.
            3 => Some(Detail::fixed(v(vertices().0[STOP] as f32 + 1.2, 0.0), 6.0)),
            _ => self.card.parts.get(index)?.detail,
        }
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let field = field(t);
        let rays = fan(field);
        let through = rays.iter().filter(|(_, reached)| *reached).count();

        vec![
            Reading::new("FIELD", format!("{field:4.1}°")),
            Reading::new("IMAGE", format!("{:6.2} mm", Self::image_point(t).y)),
            Reading::new("RAYS", format!("{through} OF {RAYS} THROUGH")),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_prescription_is_a_fifty_millimetre_lens() {
        let (focal, back) = paraxial();

        assert!((focal - 50.0).abs() < 0.1, "{focal}");
        assert!((back - 42.4).abs() < 0.1, "{back}");
    }

    #[test]
    fn an_axial_bundle_focuses_on_the_image_plane() {
        let landed: Vec<f64> = fan(0.0)
            .into_iter()
            .filter(|(_, reached)| *reached)
            .map(|(points, _)| points.last().unwrap().y)
            .collect();

        assert!(landed.len() >= 5);
        assert!(landed.iter().all(|y| y.abs() < 0.05), "{landed:?}");
    }

    #[test]
    fn the_chief_ray_lands_near_the_ideal_image_height() {
        let (focal, _) = paraxial();

        for field in [5.0f32, 10.0, 15.0, 20.0] {
            let landed = Optics::image_point(
                SWEEP * 0.0 + {
                    // The time at which the sweep reaches `field`.
                    let share = 1.0 - 2.0 * field / FIELD;
                    share.clamp(-1.0, 1.0).acos() / std::f32::consts::TAU * SWEEP
                },
            );
            let ideal = -focal * f64::from(field).to_radians().tan();

            assert!(
                (f64::from(landed.y) - ideal).abs() < 0.5,
                "{field}°: {} against {ideal}",
                landed.y
            );
        }
    }
}
