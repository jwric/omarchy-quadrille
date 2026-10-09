//! The draughtsman's hand: how fast the pen moves, along the drawing and
//! between its strokes, and where it pauses.
//!
//! The carriage moves at constant accelerations and slows for corners, a
//! short move stepped rather than ramped; it is carried up faster than it
//! draws, settles after a long move, and hovers a moment between the
//! stages of the drawing and between its parts. The figures keep the
//! ratios of real plotters and hands (see `docs/plotter.md`), and a plot
//! plays the moves [`PACE`] times as fast. Lengths and speeds are for the
//! laptop's sheet, 533 virtual pixels high; a taller sheet scales them by
//! its height, so a drawing takes as long on every output.
use crate::draft::Tone;

/// One frame of the screensaver's 30 a second: what the timings below are
/// chosen to read at.
pub const FRAME: f32 = 1.0 / 30.0;

/// How fast the pen draws in `tone`, by the weight of its pen: light
/// construction runs faster, the ink pen slowest.
pub fn speed(tone: Tone) -> f32 {
    match tone {
        Tone::Faint => 2600.0,
        Tone::Line | Tone::Muted => 2000.0,
        Tone::Ink | Tone::Accent | Tone::Live | Tone::Caution => 1600.0,
    }
}

/// The acceleration along a long line...
pub const ACCEL: f32 = 16_000.0;
/// ...and along one shorter than [`SHORT`]: a step rather than a ramp, so
/// lettering, hatching, ticks and short edges peck while long edges ease
/// (Calcomp's dual mode).
pub const STEP: f32 = 300_000.0;
pub const SHORT: f32 = 32.0;

/// How far a corner may be cut, which sets how fast it is turned (GRBL's
/// junction deviation): a right angle keeps a fifth of the speed, a
/// circle's chords all of it.
pub const DEVIATION: f32 = 3.0;

/// The pen carried up: its speed and acceleration.
pub const TRAVEL: f32 = 4000.0;
pub const TRAVEL_ACCEL: f32 = 40_000.0;

/// After a move longer than [`NEAR`] the pen takes a frame to settle onto
/// the paper; after a shorter one it is already down.
pub const NEAR: f32 = 48.0;
pub const SETTLING: f32 = FRAME;

/// Seconds the pen hovers between stages of the drawing, and between what
/// a stage keeps apart: a part from the next, a view from another.
pub const STAGE_BEAT: f32 = 0.15;
pub const CLUSTER_BEAT: f32 = 0.06;

/// Touches a second: a dot, or a stroke of a letter no longer than a pixel.
pub const TOUCHES: f32 = 45.0;

/// How many times as fast as the hand's the moves are played: a plot is a
/// time-lapse of a drafting office's work, its pauses kept as they are.
/// A sheet's plot takes as long as its work does at this pace, within the
/// plot's lengths (`timeline::PLOT`), so the pen moves as fast on a sparse
/// sheet as on a dense one.
pub const PACE: f64 = 2.25;

/// Seconds the ink stays wet behind the pen, shown in the accent until it
/// dries to its tone.
pub const WET: f32 = 0.05;
