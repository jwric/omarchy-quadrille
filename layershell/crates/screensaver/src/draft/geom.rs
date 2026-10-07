//! Points of a model: the units a subject is designed in, `y` up.
//!
//! A point is a [`glam::Vec2`]; this adds what drawings need of it.
use std::f32::consts::TAU;

/// A point or a direction in a model.
pub type V2 = glam::Vec2;

/// The point `(x, y)`.
pub const fn v(x: f32, y: f32) -> V2 {
    V2::new(x, y)
}

/// The point `radius` from the origin at `angle` radians counter-clockwise
/// from the `x` axis.
pub fn polar(radius: f32, angle: f32) -> V2 {
    V2::from_angle(angle) * radius
}

/// Turning about the origin, by an angle rather than a unit vector.
pub trait Turn {
    /// Turned `angle` radians counter-clockwise about the origin.
    fn turned(self, angle: f32) -> Self;
}

impl Turn for V2 {
    fn turned(self, angle: f32) -> Self {
        V2::from_angle(angle).rotate(self)
    }
}

/// An axis-aligned region of a model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extent {
    pub min: V2,
    pub max: V2,
}

impl Extent {
    pub const fn new(min: V2, max: V2) -> Self {
        Self { min, max }
    }

    /// The square of `radius` around `centre`.
    pub fn around(centre: V2, radius: f32) -> Self {
        Self::new(centre - V2::splat(radius), centre + V2::splat(radius))
    }

    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn centre(self) -> V2 {
        self.min.lerp(self.max, 0.5)
    }
}

/// `angle` wrapped into `[0, 2π)`.
pub fn wrap(angle: f32) -> f32 {
    angle.rem_euclid(TAU)
}

/// Points along the arc of `radius` around `centre` from `start`, sweeping
/// `sweep` radians (counter-clockwise when positive), `segments` chords.
pub fn arc_points(centre: V2, radius: f32, start: f32, sweep: f32, segments: usize) -> Vec<V2> {
    let segments = segments.max(1);

    (0..=segments)
        .map(|i| centre + polar(radius, start + sweep * i as f32 / segments as f32))
        .collect()
}

/// The length of the polyline through `points`.
pub fn length(points: &[V2]) -> f32 {
    points
        .windows(2)
        .map(|pair| pair[0].distance(pair[1]))
        .sum()
}

/// The point `distance` along the polyline through `points`, clamped to
/// its ends.
pub fn along(points: &[V2], distance: f32) -> V2 {
    let mut left = distance.max(0.0);

    for pair in points.windows(2) {
        let segment = pair[0].distance(pair[1]);

        if left <= segment && segment > 0.0 {
            return pair[0].lerp(pair[1], left / segment);
        }

        left -= segment;
    }

    points.last().copied().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_turn_is_counter_clockwise() {
        let turned = v(1.0, 0.0).turned(std::f32::consts::FRAC_PI_2);

        assert!(turned.abs_diff_eq(v(0.0, 1.0), 1e-6));
        assert_eq!(v(1.0, 0.0).perp(), v(0.0, 1.0));
    }

    #[test]
    fn a_point_along_a_polyline_turns_its_corners() {
        let path = [v(0.0, 0.0), v(10.0, 0.0), v(10.0, 5.0)];

        assert_eq!(length(&path), 15.0);
        assert_eq!(along(&path, 12.0), v(10.0, 2.0));
        assert_eq!(along(&path, 99.0), v(10.0, 5.0));
    }

    #[test]
    fn an_arc_ends_where_it_says() {
        let points = arc_points(V2::ZERO, 2.0, 0.0, std::f32::consts::PI, 8);

        assert_eq!(points.len(), 9);
        assert!((points[8].x + 2.0).abs() < 1e-5);
    }
}
