//! How the pen plots a sheet: the order it draws in, how it moves and how
//! long it takes.
//!
//! A [`PlotStyle`] is a set of knobs, and each preset one setting of them,
//! so a mix of two presets is one struct literal: `PlotStyle { order:
//! Order::Pens, ..PlotStyle::DRAFTING }`. Lengths and speeds are for the
//! laptop's sheet, 533 virtual pixels high; a taller sheet scales them by
//! its height, so a plot takes as long on every output.
use crate::draft::Tone;

use super::super::timeline;

/// How the pen plots a sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlotStyle {
    /// What it is called on the command line.
    pub name: &'static str,
    /// Seconds the plot takes, from the pen's start to the subject's run.
    pub length: f32,
    /// How the pen moves; [`Motion::Reveal`] is the plot as it was, and
    /// takes none of the other knobs.
    pub motion: Motion,
    /// What it draws first.
    pub order: Order,
    /// Whether, within what the order keeps together, the pen goes to the
    /// nearest stroke next (turning a line round or starting a loop where
    /// it is nearest), rather than taking them as they were drawn.
    pub nearest: bool,
    /// How section lining is drawn.
    pub lining: Lining,
    /// How a letter is set: whole at a touch, or traced a stroke at a time.
    pub glyphs: Glyphs,
    /// Where a circle is started and which way round it goes.
    pub circles: Circles,
    /// Whether the nearest-first order is then improved where moving a few
    /// strokes elsewhere, or turning a run of them round, saves the pen a
    /// journey: no darting back across the sheet for a stroke left behind.
    pub polish: bool,
    /// Whether what moves along the drawing's traces (bus traffic, flow,
    /// a signal) waits for the subject's run to appear, the machine
    /// switching on, rather than being plotted where it stands.
    pub traces_wait: bool,
    /// Touches a second before the plot is fitted to its length: dots, and
    /// letters set whole at a touch of the pen.
    pub lettering: f32,
    /// How long the pen takes to settle after a long move, if at all.
    pub drop: Option<Drop>,
    /// The pauses between what the order keeps apart.
    pub beats: Beats,
    /// A pen for each tone, changed at the carousel, if the plotter has one.
    pub carousel: Option<Carousel>,
    /// Whether the pen starts at home, off the drawing, rather than at its
    /// first stroke.
    pub from_home: bool,
    /// Whether the pen goes home at the end, rather than lifting where it
    /// stops.
    pub park: bool,
    /// Whether the pen's head is shown, and the carriage with it.
    pub head: bool,
    /// Whether the carriage is shown riding its rails: a tick in the
    /// sheet's zone bands above and below the head while it plots the
    /// sheet, and at the ends of a gantry's arm as it wipes the sheet too.
    pub ticks: bool,
    /// Whether the carriage's arm is shown across the sheet, under the ink,
    /// at the head: a gantry plotter's.
    pub gantry: bool,
    /// Whether a part's detail is plotted by the pen, its circle on the view
    /// and then its view, rather than revealed as it was.
    pub details: bool,
    /// How the sheet is wiped.
    pub wipe: Wipe,
    /// Whether a diagram, a sheet not drawn to scale, grows from its first
    /// part along what joins it to the others, each part drawn whole with
    /// its lettering as the pen reaches it, rather than part by part.
    pub grow: bool,
    /// Seconds the ink stays wet behind the pen, shown in the accent until
    /// it dries to its tone; none if nought.
    pub wet: f32,
    /// Whether a compass is seen drawing a circle or an arc: an arm from
    /// its point at the centre out to the pen.
    pub compass: bool,
    /// Whether the title block, the notes and the parts list fill in as the
    /// drawing proceeds, as a draughtsman fills in a printed form, rather
    /// than typing themselves in first.
    pub fills: bool,
}

/// How the pen moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Motion {
    /// The plot as it was: every piece of the drawing in the order of its
    /// passes, revealed at one rate of pen travel, the pen leaping between
    /// them.
    Reveal,
    /// As a plotter's carriage moves: at constant accelerations, slowing
    /// for corners.
    Physical(Physics),
    /// Each stroke eased in and out along its length as a whole, with no
    /// corners.
    Eased(Easing),
}

/// A plotter's carriage, in virtual pixels and seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Physics {
    /// Drawing speed, by the pen's weight: light construction runs faster.
    pub speeds: Speeds,
    /// The acceleration along a long line...
    pub accel: f32,
    /// ...and along one shorter than `short`: a step rather than a ramp,
    /// so lettering and hatching peck while long edges ease (Calcomp's dual
    /// mode).
    pub step: f32,
    pub short: f32,
    /// How far a corner may be cut, which sets how fast it is turned (GRBL's
    /// junction deviation): a right angle keeps a fifth of the speed, a
    /// circle's chords all of it.
    pub deviation: f32,
    /// The pen up: its speed and acceleration.
    pub travel: f32,
    pub travel_accel: f32,
}

/// Drawing speeds by the weight of the pen's tone.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Speeds {
    pub faint: f32,
    pub line: f32,
    pub ink: f32,
}

impl Speeds {
    pub fn of(self, tone: Tone) -> f32 {
        match tone {
            Tone::Faint => self.faint,
            Tone::Line | Tone::Muted => self.line,
            Tone::Ink | Tone::Accent | Tone::Live | Tone::Caution => self.ink,
        }
    }
}

/// Strokes eased along their length, in virtual pixels and seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Easing {
    pub speed: f32,
    /// A stroke longer than `long` takes at least `least`, so it is seen to
    /// be drawn.
    pub long: f32,
    pub least: f32,
    /// The pen up: its speed, and the least a move longer than `long`
    /// takes, so it is seen to go.
    pub travel: f32,
    pub least_travel: f32,
}

/// The pen settling onto the paper after a move longer than `near`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drop {
    pub near: f32,
    pub far: f32,
}

/// Seconds the pen hovers between stages of the order, and between what a
/// stage keeps apart (a part from the next, a pane from another).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beats {
    pub stage: f32,
    pub cluster: f32,
}

/// A pen carousel at home: seconds to change pens, and for each position
/// the carousel turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Carousel {
    pub change: f32,
    pub click: f32,
}

/// How a letter is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyphs {
    /// Whole, at a touch of the pen.
    Touched,
    /// Along the strokes of its glyph, each in turn, as it is lettered.
    Traced,
}

/// Where a circle is started and which way round it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Circles {
    /// Wherever is nearest the pen.
    Seamed,
    /// As a plotter's circle instruction draws it: the pen comes to the
    /// centre, goes out to the right, round anticlockwise, and back to the
    /// centre.
    Centred,
    /// Wherever is nearest the pen, and round whichever way carries on the
    /// way it was going, as a hand does.
    Onward,
}

/// How the sheet is wiped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wipe {
    /// A line crossing it at one rate.
    Line,
    /// The pen's carriage sweeping it from where it is parked, easing in
    /// and out.
    Sweep,
}

/// What the pen draws first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// Pass by pass (construction, edges, hidden lines, areas, traces,
    /// annotation, balloons), over every view, as the marks were made.
    Passes,
    /// Pen by pen, lightest first, over the whole sheet: a pen-sorted plot.
    Pens,
    /// A drafting office's order: the skeleton of every view, then the
    /// bodies view by view and part by part, the lining, the annotation,
    /// the traces, the balloons.
    Stages,
    /// The skeleton, then each part whole, then what belongs to none.
    Parts,
}

/// How section lining is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lining {
    /// Row by row, back and forth.
    Rows,
    /// Along its own diagonals, back and forth across the area.
    Diagonals,
}

/// One frame of the screensaver's 30 a second: what the timings below are
/// chosen to read at.
const FRAME: f32 = 1.0 / 30.0;

impl PlotStyle {
    /// The plot as it was before the pen was planned.
    pub const TODAY: Self = Self {
        name: "today",
        length: timeline::PLOT,
        motion: Motion::Reveal,
        order: Order::Passes,
        nearest: false,
        lining: Lining::Rows,
        glyphs: Glyphs::Touched,
        circles: Circles::Seamed,
        polish: false,
        traces_wait: false,
        lettering: 45.0,
        drop: None,
        beats: Beats {
            stage: 0.0,
            cluster: 0.0,
        },
        carousel: None,
        from_home: false,
        park: false,
        head: true,
        ticks: false,
        gantry: false,
        details: false,
        wipe: Wipe::Line,
        grow: false,
        wet: 0.0,
        compass: false,
        fills: false,
    };

    /// A carousel plotter at work, after HP's: pen by pen, lightest first,
    /// a trip home to the carousel for every pen; a snappy carriage on a
    /// gantry, its circles drawn from their centres and its letters stroke
    /// by stroke; the details plotted by the same pen, and the sheet wiped
    /// by the same arm.
    pub const CAROUSEL: Self = Self {
        name: "carousel",
        length: 12.0,
        motion: Motion::Physical(Physics {
            speeds: Speeds {
                faint: 2000.0,
                line: 2000.0,
                ink: 2000.0,
            },
            accel: 30_000.0,
            // Hatching, ticks and short edges stepped, as well as letters.
            step: 200_000.0,
            short: 32.0,
            deviation: 2.0,
            // Pen up only half as fast again as down, as on HP's plotters.
            travel: 3000.0,
            travel_accel: 30_000.0,
        }),
        order: Order::Pens,
        nearest: true,
        lining: Lining::Diagonals,
        glyphs: Glyphs::Traced,
        circles: Circles::Centred,
        polish: true,
        traces_wait: true,
        lettering: 35.0,
        drop: Some(Drop {
            near: 25.0,
            far: FRAME,
        }),
        beats: Beats {
            stage: 0.0,
            cluster: 0.0,
        },
        carousel: Some(Carousel {
            change: 0.25,
            click: 0.06,
        }),
        from_home: true,
        park: true,
        head: true,
        ticks: true,
        gantry: true,
        details: true,
        wipe: Wipe::Sweep,
        grow: false,
        wet: 0.0,
        compass: false,
        fills: false,
    };

    /// A drafting office: the skeleton of every view, then each part's
    /// body in turn (a diagram grown from its first part along its wires),
    /// its lining, the annotation, the balloons each with its row of the
    /// parts list, and the sign-off, with a beat between; a hand-like
    /// carriage lettering a stroke at a time, its ink wet behind it and a
    /// compass drawing its circles; the form filling in as it goes, the
    /// details plotted by the same pen and the sheet wiped from where it
    /// parks.
    pub const DRAFTING: Self = Self {
        name: "drafting",
        length: 10.0,
        motion: Motion::Physical(Physics {
            speeds: Speeds {
                faint: 2600.0,
                line: 2000.0,
                ink: 1600.0,
            },
            accel: 16_000.0,
            // Hatching, ticks and short edges stepped, as well as letters.
            step: 300_000.0,
            short: 32.0,
            deviation: 3.0,
            travel: 4000.0,
            travel_accel: 40_000.0,
        }),
        order: Order::Stages,
        nearest: true,
        lining: Lining::Diagonals,
        glyphs: Glyphs::Traced,
        circles: Circles::Onward,
        polish: true,
        traces_wait: true,
        lettering: 45.0,
        drop: Some(Drop {
            near: 48.0,
            far: FRAME,
        }),
        beats: Beats {
            stage: 0.15,
            cluster: 0.06,
        },
        carousel: None,
        from_home: true,
        park: true,
        head: true,
        ticks: true,
        gantry: false,
        details: true,
        wipe: Wipe::Sweep,
        grow: true,
        wet: 0.05,
        compass: true,
        fills: true,
    };

    /// A quick study: the skeleton, then each part whole in eased strokes,
    /// its words typed in a burst.
    pub const QUICK: Self = Self {
        name: "quick",
        length: 6.0,
        motion: Motion::Eased(Easing {
            speed: 3200.0,
            long: 40.0,
            least: 2.0 * FRAME,
            travel: 6000.0,
            least_travel: FRAME,
        }),
        order: Order::Parts,
        nearest: true,
        lining: Lining::Diagonals,
        glyphs: Glyphs::Touched,
        circles: Circles::Seamed,
        polish: false,
        traces_wait: true,
        lettering: 150.0,
        drop: None,
        beats: Beats {
            stage: 0.0,
            cluster: 0.0,
        },
        carousel: None,
        from_home: false,
        park: false,
        head: true,
        ticks: false,
        gantry: false,
        details: false,
        wipe: Wipe::Line,
        grow: false,
        wet: 0.0,
        compass: false,
        fills: false,
    };

    /// Every preset, today's first.
    pub const ALL: [Self; 4] = [Self::TODAY, Self::CAROUSEL, Self::DRAFTING, Self::QUICK];
}

impl Default for PlotStyle {
    fn default() -> Self {
        Self::TODAY
    }
}
