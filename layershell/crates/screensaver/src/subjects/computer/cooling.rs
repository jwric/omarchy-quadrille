//! The machine's cooling as a schematic plan: the heat sources with their
//! temperatures, the fans as rotors turning at the speeds their monitors
//! measure, the air drawn in at the base, past what it cools and out
//! through the vents, and the way each source's heat gets to it.
//!
//! The arrangement is a diagram, not the machine's: the inventory says what
//! is measured, not where it is. It is laid out with the vents at the top of
//! the sheet. A laptop (a tablet, a mini PC) cools its processor and
//! graphics through a heat pipe to fins in its blowers' outlets: the blowers
//! in the top corners, the processor between them under the pipe. A desktop
//! or a server gives them heatsinks of their own in the air its case fans
//! move: axial fans in the corners, exhausts at the top and intakes at the
//! base. Under them in rows is what the air cools, each giving its heat to
//! the air drawn up the side to the nearest fan that turns.
//!
//! A fan a controller lists but that has not been seen turning is drawn in
//! phantom: an embedded controller can list more fans than the machine has,
//! and a fan that has stopped reads 0 like one that is not there. A board's
//! monitor lists a header for every fan the board could take, so its
//! headers reading 0 when the machine is read are left out and counted. A
//! fan on a device, a graphics card's, is the device's, not the case's: its
//! speed is lettered in the device's block. With no fan measured at all,
//! the paths are drawn and nothing moves on them.
//!
//! The units are the laptop's virtual pixels at full size, as the topology
//! sheet's are, so lettering fits its blocks at the laptop's scale or
//! larger. Each block, thermometer and fan is set on the pixel grid as one
//! figure ([`Draft::snapped`]): drawn the same wherever it falls, symmetric
//! where it is symmetric, its lettering and gauges as far from its outline
//! on every side, and the heat pipe meeting what it runs into.
use std::cell::RefCell;
use std::f32::consts::{PI, TAU};
use std::rc::Rc;

use quadrille::draw::{Anchor, Horizontal, Vertical};

use crate::draft::Placement::Auto;
use crate::draft::raster::LETTERING;
use crate::draft::{Draft, Extent, Fill, Line, Shape, Tone, V2, arc_points, polar, v};
use crate::machine::{
    ChargeState, ChargerKind, ChassisKind, Drive, DriveKind, HISTORY, Link, Machine, PciAddress,
    PciKind, Sensor, SensorKind, Site, Snapshot,
};

use super::super::{Card, Domain, Part, Place, Reading, Revision, Room, Subject, Unit, View};
use super::layout::{BUDGET, named};
use super::{SPEC_ROWS, alike, binary, chain, counted, decimal, fit, flow, lettered, rows, set};

/// A heat source's block: wide enough for its name and a temperature...
const BLOCK: f32 = 108.0;
/// ...everything in it as far from each of its sides...
const PAD: f32 = 6.0;
/// ...under a line of lettering, its name and hottest temperature, and
/// another for the speed of a fan of its own...
const LINE: f32 = 12.0;
/// ...and from its top to the middle of its name's capitals, where the
/// arrow of its heat leaves it.
const TITLE: f32 = PAD + 4.0;

/// A thermometer: its bulb's radius, its tube's length past the bulb's
/// centre and half its bore, and the scale along it in °C.
const BULB_RADIUS: f32 = 3.0;
const TUBE: f32 = 60.0;
const GLASS: f32 = 2.0;
const COLDEST: f32 = 20.0;
const HOTTEST: f32 = 100.0;
/// The tick over a tube at the limit its chip gives.
const LIMIT: f32 = 2.0;
/// Between thermometers one under another, bulb to bulb: a bulb and room
/// for a pixel or more between bulbs at any scale the view is drawn at.
const GAUGE: f32 = 9.0;
/// A temperature read in caution from here.
const HOT: f32 = 85.0;
/// The thermometers a block has at most: the rest of its sensors are in its
/// specification.
const MOST_GAUGES: usize = 4;
/// The processor's other sensors (its cores) as squares tinted by their
/// temperatures under its thermometer: how far under, their size and
/// pitch, how many to a row and how many at most.
const CORES_UNDER: f32 = 4.0;
const CORE: f32 = 5.0;
const CORE_PITCH: f32 = 7.5;
const CORES_ACROSS: usize = 12;
const MOST_CORES: usize = 48;
/// A battery's gauge: its body's length and half its height, and its
/// terminal's length and half its height.
const CELL: f32 = 44.0;
const CELL_HALF: f32 = 3.5;
const TERMINAL: f32 = 2.5;
const TERMINAL_HALF: f32 = 1.5;
/// In a block's detail: the room over its first bulb for its name and
/// left of its bulbs; after a tube, the gap to its temperature and the
/// room for it, `100 °C` at twice the laptop's view.
const DETAIL_NAME: f32 = 6.0;
const DETAIL_LEFT: f32 = 2.0;
const DETAIL_GAP: f32 = 4.0;
const DETAIL_VALUE: f32 = DETAIL_GAP + 36.0 / 2.36;

/// Between blocks across and down.
const ACROSS: f32 = 14.0;
const DOWN: f32 = 16.0;
/// Between the blocks and the fans, room for the heat's arrows.
const SIDE: f32 = 30.0;
/// Between the case and the fans, and between fans side by side.
const INSET: f32 = 8.0;
const BETWEEN: f32 = 10.0;
/// Between a tower's fans one over another on a side: room to see the air
/// go up from one to the other.
const STACKED: f32 = 30.0;
/// The case's corners, cut off; the room under the fans for the air drawn
/// up to them; the intakes' half width.
const CHAMFER: f32 = 6.0;
const AIRWAY: f32 = 40.0;
const INTAKE: f32 = 12.0;
/// From the top of the case: the exhaust's arrows, and the fans' speeds
/// over them under their names, a line of lettering (at the laptop's
/// scale) apart. Under the base, a fan's speed over its name.
const EXHAUST: f32 = 14.0;
const NAMES: f32 = 24.0;

/// The fin stack under each vent: its depth and the pitch of its fins.
const FINS: f32 = 16.0;
const FIN: f32 = 3.0;
/// The heat pipe: its bore, its axis through the fins, and from the top of
/// the case to the blocks it drops to.
const BORE: f32 = 5.0;
const PIPE: f32 = -FINS / 2.0;
const COOLED: f32 = 26.0;
/// A fin stack with no fan: its width.
const SINK: f32 = 48.0;
/// A heatsink's fins over a desktop's processor or graphics.
const HEATSINK: f32 = 8.0;

/// A rotor: its radius, its hub's, and (a blower's) where its blades start.
const ROTOR: f32 = 24.0;
const HUB: f32 = 9.0;
const ROOT: f32 = 14.0;
/// A blower's blades, and how far each curves forward from root to tip.
const BLADES: usize = 19;
const CURVE: f32 = 0.45;
/// A blower's scroll, from its tongue (at an angle from the outlet's side)
/// round to the outlet, and its outlet's length past the tongue.
const TONGUE: f32 = 0.6 * PI;
const SCROLL_IN: f32 = ROTOR + 4.0;
const SCROLL_OUT: f32 = ROTOR + 16.0;
const OUTLET: f32 = 8.0;
/// An axial fan: its frame's half side, the vanes of its rotor.
const FRAME: f32 = 30.0;
const VANES: usize = 7;

/// How many times slower than they turn the rotors are drawn.
const SLOWED: f32 = 150.0;
/// The longest a fan's turning is carried from one frame to the next: past
/// it, the frame is not the next one.
const STEP: f64 = 0.5;
/// How far air runs for each radian its fan turns, and heat along the pipe
/// in a second; the dots' spacing and size.
const AIR: f64 = 12.0;
const HEAT: f64 = 18.0;
const SPACING: f64 = 9.0;
/// The temperatures over which a source's heat goes from a few dots to all
/// of them.
const WARM: f32 = 30.0;
const SCALDING: f32 = 90.0;

/// The fans drawn, at most, and their names, by the monitor's number for
/// them where those tell them apart.
const MOST_FANS: usize = 4;
const FAN_NAMES: [&str; 16] = [
    "FAN 1", "FAN 2", "FAN 3", "FAN 4", "FAN 5", "FAN 6", "FAN 7", "FAN 8", "FAN 9", "FAN 10",
    "FAN 11", "FAN 12", "FAN 13", "FAN 14", "FAN 15", "FAN 16",
];
/// The monitors on a board that list a header for every fan the board could
/// take, by the start of their drivers' names: the Super I/O chips, and the
/// boards' own embedded controllers.
const HEADER_CHIPS: [&str; 8] = [
    "nct", "it8", "f71", "f81", "w83", "sch5", "asusec", "gigabyte",
];

/// Room round the drawing for the balloons to line up in.
const MARGIN: f32 = 22.0;

/// The chart of the last two minutes under the plan: its height; the room
/// either side of its plot for its scales, over it for its key and under
/// it for its times.
const CHART: f32 = 80.0;
const CHART_SIDE: f32 = 34.0;
const CHART_KEY: f32 = 14.0;
const CHART_TIME: f32 = 12.0;
/// How much wider than tall a wide display's view is, about: a plan that
/// wide for its height and the chart's leaves room for the chart under it
/// there, which a sheet adds on a wide display only, at whatever scale
/// fits both.
const WIDE_VIEW: f32 = 1.3;
/// Its temperatures' grid, every 20 °C, and its fans' speed grid, in steps
/// of 1000 rpm.
const CHART_STEP: f32 = 20.0;
const RPM_STEP: f32 = 1000.0;

/// What a heat source is, in the order the parts list documents them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Processor,
    Graphics,
    Memory,
    Drive,
    Network,
    Device,
    Board,
    Battery,
}

impl Kind {
    /// The parts list's name for the sources of this kind.
    fn part(self) -> &'static str {
        match self {
            Self::Processor => "CPU",
            Self::Graphics => "GRAPHICS",
            Self::Memory => "MEMORY",
            Self::Drive => "DRIVE",
            Self::Network => "NETWORK",
            Self::Device => "DEVICE",
            Self::Board => "BOARD",
            Self::Battery => "BATTERY",
        }
    }

    /// Whether it is cooled through fins of its own: the heat pipe's on a
    /// laptop, a heatsink's on a desktop. The air cools the rest as it
    /// passes.
    fn finned(self) -> bool {
        matches!(self, Self::Processor | Self::Graphics)
    }
}

/// An item of the parts list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Sources(Kind),
    Fans,
}

/// A heat source: a block lettered with its name and its hottest
/// temperature, with a thermometer for each of its sensors.
#[derive(Debug, Clone)]
struct Source {
    kind: Kind,
    name: String,
    /// The PCI device it is, for a device's.
    device: Option<PciAddress>,
    /// Which of the machine's drives, processor packages or batteries it
    /// is, for one of those.
    drive: Option<usize>,
    package: Option<usize>,
    battery: Option<usize>,
    /// Every temperature sensor on it, by index in a snapshot...
    sensors: Vec<usize>,
    /// ...those it has a thermometer for...
    gauges: Vec<usize>,
    /// ...and the processor's others, its cores, drawn as squares.
    cores: Vec<usize>,
    /// Its own fans' speed sensors: a graphics card's.
    fans: Vec<usize>,
    /// Whether a detail magnifies it: the first of its kind, whose part's
    /// detail is centred on it.
    detailed: bool,
    frame: Extent,
    /// The points the figure its block is set inside is set by (see
    /// [`Draft::snapped`]): the heat pipe's, for a block under it, which
    /// meets its top.
    snaps: Vec<V2>,
}

impl Source {
    fn new(kind: Kind) -> Self {
        Self {
            kind,
            name: String::new(),
            device: None,
            drive: None,
            package: None,
            battery: None,
            sensors: Vec::new(),
            gauges: Vec::new(),
            cores: Vec::new(),
            fans: Vec::new(),
            detailed: false,
            frame: Extent::new(V2::ZERO, V2::ZERO),
            snaps: Vec::new(),
        }
    }

    /// Its block's height: its name's line, its fan's, its gauges and its
    /// cores' rows, with the same room over and under them.
    fn height(&self) -> f32 {
        let fan = if self.fans.is_empty() { 0.0 } else { LINE };
        let gauges = self.gauges.len().max(1) as f32;

        PAD + LINE + fan + 2.0 * BULB_RADIUS + (gauges - 1.0) * GAUGE + self.cores_height() + PAD
    }

    /// The room its cores take under its thermometer.
    fn cores_height(&self) -> f32 {
        match self.cores.len().div_ceil(CORES_ACROSS) {
            0 => 0.0,
            rows => CORES_UNDER + (rows - 1) as f32 * CORE_PITCH + CORE,
        }
    }

    /// Its block's top left corner, which it is set on the grid by.
    fn corner(&self) -> V2 {
        v(self.frame.min.x, self.frame.max.y)
    }

    /// The bottom left of its last thermometer's bulb, or of a battery's
    /// gauge: `PAD` in from its block's side and over what is under it.
    fn foot(&self) -> V2 {
        self.frame.min + v(PAD, PAD + self.cores_height())
    }

    /// The middle of the bulb of its thermometer `k`, or of a battery's
    /// gauge's left end.
    fn gauge(&self, k: usize) -> V2 {
        let last = self.gauges.len().max(1) - 1;

        self.foot()
            + v(
                BULB_RADIUS,
                BULB_RADIUS + (last - k.min(last)) as f32 * GAUGE,
            )
    }

    /// The square of its core `k`.
    fn core(&self, k: usize) -> Extent {
        let rows = self.cores.len().div_ceil(CORES_ACROSS);
        let (row, column) = (k / CORES_ACROSS, k % CORES_ACROSS);
        let corner = self.frame.min
            + v(
                PAD + column as f32 * CORE_PITCH,
                PAD + (rows - 1 - row) as f32 * CORE_PITCH,
            );

        Extent::new(corner, corner + V2::splat(CORE))
    }

    /// What a detail of it must hold in full: its name, its gauges and
    /// the values a detail letters after them, and its cores.
    fn held(&self) -> Extent {
        let first = self.gauge(0);
        let bottom = match self.cores.len() {
            0 => self.foot().y,
            n => self.core(n - 1).min.y,
        };

        Extent::new(
            v(first.x - BULB_RADIUS - DETAIL_LEFT, bottom),
            v(
                first.x + BULB_RADIUS + TUBE + DETAIL_VALUE,
                first.y + BULB_RADIUS + DETAIL_NAME,
            ),
        )
    }

    /// The circle a detail of it magnifies: round what the detail holds
    /// (see [`Source::held`]), as a detail at twice the view's scale or
    /// more has room for (on the laptop, a detail twice the view's scale
    /// holds a block's width a little short of its block's).
    fn ring(&self) -> (V2, f32) {
        let held = self.held();
        let centre = held.centre();

        (
            v(centre.x.round(), centre.y.round()),
            (v(held.width(), held.height()) / 2.0).length().ceil(),
        )
    }

    /// Its hottest temperature now.
    fn hottest(&self, snapshot: &Snapshot) -> Option<f32> {
        self.sensors
            .iter()
            .filter_map(|&index| celsius(snapshot, index))
            .reduce(f32::max)
    }

    /// Draws `draw` set on the grid as its block is, by its top left corner.
    fn set(&self, d: &mut Draft, draw: impl FnOnce(&mut Draft)) {
        set(d, &self.snaps, |d| d.snapped(self.corner(), draw));
    }

    /// ...by its top right corner, from which its value is set in.
    fn set_right(&self, d: &mut Draft, draw: impl FnOnce(&mut Draft)) {
        self.set(d, |d| d.snapped(self.frame.max, draw));
    }

    /// ...by its bottom left corner, from which its gauges are set up.
    fn set_foot(&self, d: &mut Draft, draw: impl FnOnce(&mut Draft)) {
        self.set(d, |d| d.snapped(self.frame.min, draw));
    }

    /// ...by the middle of thermometer `k`'s bulb, set a whole number of
    /// gauges' pitches over the last one, which is set by the bottom left
    /// of its bulb: so every bulb is as far from the next, and the last as
    /// far from the block's bottom as its name is from the top.
    fn set_gauge(&self, d: &mut Draft, k: usize, draw: impl FnOnce(&mut Draft, V2)) {
        let last = self.gauges.len().max(1) - 1;
        let centre = self.gauge(k);

        self.set_foot(d, |d| {
            chain(d, self.foot(), v(0.0, GAUGE), last - k.min(last), |d| {
                d.snapped(centre, |d| draw(d, centre));
            });
        });
    }

    /// ...by core `k`'s square's bottom left corner, set a whole number of
    /// pitches from the first of the bottom row: every square the same and
    /// as far from the next.
    fn set_core(&self, d: &mut Draft, k: usize, draw: impl FnOnce(&mut Draft, Extent)) {
        let rows = self.cores.len().div_ceil(CORES_ACROSS);
        let (row, column) = (k / CORES_ACROSS, k % CORES_ACROSS);
        let origin = self.frame.min + V2::splat(PAD);
        let up = origin + v(0.0, (rows - 1 - row) as f32 * CORE_PITCH);
        let square = self.core(k);

        self.set_foot(d, |d| {
            chain(d, origin, v(0.0, CORE_PITCH), rows - 1 - row, |d| {
                chain(d, up, v(CORE_PITCH, 0.0), column, |d| draw(d, square));
            });
        });
    }
}

/// Records what `draw` makes for the detail of `source`'s part, if that
/// detail is of `source`: the first block of a kind, which it is centred
/// on.
fn in_its_detail(d: &mut Draft, source: &Source, draw: impl FnOnce(&mut Draft)) {
    if source.detailed {
        d.in_own_detail(draw);
    }
}

/// A fan, as the sheet draws it.
#[derive(Debug, Clone)]
struct Fan {
    /// Its speed's sensor, by index in a snapshot.
    sensor: usize,
    /// Its name on the sheet.
    name: &'static str,
    /// What its monitor calls it, and the monitor.
    label: Option<String>,
    chip: String,
    /// Whether it was turning when the machine was first read.
    turning: bool,
    /// Its axis, whether it is drawn mirrored (its outlet on its left), and
    /// whether it is a tower's intake at the base rather than an exhaust at
    /// the top.
    centre: V2,
    mirrored: bool,
    low: bool,
}

impl Fan {
    /// A point of its drawing, given round its axis as it is drawn
    /// unmirrored.
    fn at(&self, point: V2) -> V2 {
        self.centre
            + if self.mirrored {
                v(-point.x, point.y)
            } else {
                point
            }
    }

    /// Its outlet's span across: where its fins are, under its vent.
    fn vent(&self, rotor: Rotor) -> (f32, f32) {
        let (a, b) = rotor.outlet();
        let (a, b) = (self.at(v(a, 0.0)).x, self.at(v(b, 0.0)).x);

        (a.min(b), a.max(b))
    }

    /// What it takes up across.
    fn span(&self, rotor: Rotor) -> (f32, f32) {
        let reach = rotor.reach();
        let (a, b) = (self.at(reach.min).x, self.at(reach.max).x);

        (a.min(b), a.max(b))
    }

    /// What it takes up down, from its base to its top.
    fn height(&self, rotor: Rotor) -> (f32, f32) {
        let reach = rotor.reach();

        (self.centre.y + reach.min.y, self.centre.y + reach.max.y)
    }
}

/// What kind of fans a machine has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rotor {
    /// A centrifugal blower in its scroll, as a laptop has, blowing through
    /// the fins at the end of the heat pipe.
    Blower,
    /// An axial fan in a square frame, as a desktop's or a server's case
    /// has.
    Axial,
}

impl Rotor {
    /// How far its axis is under the top of the case: under its fins and,
    /// a blower's, its outlet.
    fn depth(self) -> f32 {
        match self {
            Self::Blower => (FINS + OUTLET + SCROLL_IN * TONGUE.sin()).round(),
            Self::Axial => INSET + FRAME,
        }
    }

    /// Its housing round its axis, its outlet up and right of its axis: a
    /// blower's scroll and the walls of its outlet up to its fins, or an
    /// axial fan's frame.
    fn housing(self) -> Vec<V2> {
        let top = self.depth() - FINS;

        match self {
            Self::Blower => {
                let tongue = polar(SCROLL_IN, TONGUE);
                let mut points = vec![v(tongue.x, top)];

                points.extend((0..=48).map(|k| {
                    let angle = TONGUE + (TAU - TONGUE) * k as f32 / 48.0;
                    polar(scroll(angle), angle)
                }));
                points.push(v(SCROLL_OUT, top));
                points
            }
            Self::Axial => vec![
                v(-FRAME, -FRAME),
                v(FRAME, -FRAME),
                v(FRAME, FRAME),
                v(-FRAME, FRAME),
                v(-FRAME, -FRAME),
            ],
        }
    }

    /// Its outlet's span across, from its axis.
    fn outlet(self) -> (f32, f32) {
        match self {
            Self::Blower => (SCROLL_IN * TONGUE.cos(), SCROLL_OUT),
            Self::Axial => (-FRAME, FRAME),
        }
    }

    /// What its housing takes up round its axis.
    fn reach(self) -> Extent {
        let points = self.housing();
        let min = points.iter().copied().reduce(V2::min).unwrap_or_default();
        let max = points.iter().copied().reduce(V2::max).unwrap_or_default();

        Extent::new(min.floor(), max.ceil())
    }

    /// Where its balloon points: its housing's outer side, away from its
    /// outlet and the fins.
    fn tip(self) -> V2 {
        match self {
            Self::Blower => polar(scroll(0.8 * PI), 0.8 * PI),
            Self::Axial => v(-FRAME, 0.0),
        }
    }

    /// The circle a detail of it magnifies.
    fn ring(self) -> f32 {
        match self {
            Self::Blower => SCROLL_OUT + 2.0,
            Self::Axial => FRAME + 4.0,
        }
    }
}

/// A blower's scroll: its radius at `angle`, from the tongue's round to
/// the outlet's.
fn scroll(angle: f32) -> f32 {
    SCROLL_IN + (SCROLL_OUT - SCROLL_IN) * (angle - TONGUE) / (TAU - TONGUE)
}

/// The points the drawing round the heat pipe is set on the grid by,
/// outermost first: the top of the case, the pipe's axis under it, which
/// the fins are split round and the drops hang from, and the bottom of the
/// fins, which the blowers' outlets meet. Each is set from the one before,
/// so what is drawn from one meets what is drawn from another.
const TOP: V2 = v(0.0, 0.0);
const AXIS: V2 = v(0.0, PIPE);
const UNDER_FINS: V2 = v(0.0, -FINS);

/// Where the case and the paths are, in the laptop's virtual pixels, `y`
/// up and the top of the case at 0.
#[derive(Debug, Clone)]
struct Plan {
    case: Extent,
    /// The gaps in the case's top over the fans' outlets or the fins, and
    /// the columns of air drawn up from the base.
    vents: Vec<(f32, f32)>,
    intakes: Vec<f32>,
    /// The heat pipe's run under the vents, end to end, and where it drops
    /// to each block it cools.
    pipe: Option<(f32, f32)>,
    drops: Vec<f32>,
    /// The fins the pipe ends in when no fan is measured.
    sink: Option<(f32, f32)>,
    /// The heat the air takes from the sources.
    arrows: Vec<Arrow>,
    /// Where the pipe is named, if it has room.
    legend: Option<V2>,
    extent: Extent,
}

/// A trace of the chart: what it follows, its value now and the height of
/// that, its tone, and its height at each sample, if there was one.
struct Trace {
    name: String,
    value: String,
    height: Option<f32>,
    tone: Tone,
    points: Vec<Option<f32>>,
}

/// The heat a source gives the air: an arrow from its block into the air
/// drawn up to a fan, or rising with none.
#[derive(Debug, Clone)]
struct Arrow {
    source: usize,
    from: V2,
    to: V2,
    fan: Option<usize>,
}

/// Lays `sources` and `fans` out, the sources the air cools in rows of
/// `columns`.
///
/// The fans are dealt to the sides in turn, the first (those turning) to
/// the outer places: a blower's beside the one before it at the top, an
/// axial fan's at the base under it.
fn plan(sources: &mut [Source], fans: &mut [Fan], rotor: Rotor, columns: usize) -> Plan {
    let reach = rotor.reach();
    let depth = rotor.depth();
    let sides: [Vec<usize>; 2] = [
        (0..fans.len()).step_by(2).collect(),
        (1..fans.len()).step_by(2).collect(),
    ];
    let piped = rotor == Rotor::Blower;

    // The fans on the left, their outlets on their right, toward the
    // middle.
    let mut x = INSET;
    let mut edge = INSET;

    for (place, &k) in sides[0].iter().enumerate() {
        let fan = &mut fans[k];

        fan.mirrored = false;
        fan.low = rotor == Rotor::Axial && place > 0;
        fan.centre = v(x - reach.min.x, -depth);
        edge = fan.centre.x + reach.max.x;

        if rotor == Rotor::Blower {
            x = edge + BETWEEN;
        }
    }

    let middle = if sides[0].is_empty() {
        INSET + SIDE / 2.0
    } else {
        edge + SIDE
    };
    let (first, rest): (Vec<usize>, Vec<usize>) =
        (0..sources.len()).partition(|&index| sources[index].kind.finned());
    let width = |n: usize| n as f32 * BLOCK + n.saturating_sub(1) as f32 * ACROSS;
    let columns = columns.min(rest.len()).max(1);
    let span = width(first.len()).max(width(columns));
    let centre = middle + span / 2.0;
    // What a desktop's heatsinks take over their blocks.
    let heatsinks = if piped { 0.0 } else { HEATSINK };

    // What has fins of its own, side by side under the top...
    let mut x = middle + (span - width(first.len())) / 2.0;
    let mut bottom = -COOLED;

    for &index in &first {
        let source = &mut sources[index];
        let height = source.height();

        source.frame = Extent::new(v(x, -COOLED - height), v(x + BLOCK, -COOLED));
        source.snaps = if piped {
            vec![TOP, AXIS, v(0.0, -COOLED)]
        } else {
            Vec::new()
        };
        bottom = bottom.min(-COOLED - height);
        x += BLOCK + ACROSS;
    }

    // ...and what the air cools, in rows under that.
    let mut top = if first.is_empty() {
        -COOLED
    } else {
        bottom - DOWN
    };
    let left = middle + (span - width(columns)) / 2.0;

    for row in rest.chunks(columns) {
        let mut lowest = top;

        for (column, &index) in row.iter().enumerate() {
            let source = &mut sources[index];
            let height = source.height();
            let x = left + column as f32 * (BLOCK + ACROSS);

            source.frame = Extent::new(v(x, top - height), v(x + BLOCK, top));
            source.snaps = Vec::new();
            lowest = lowest.min(top - height);
        }

        bottom = bottom.min(lowest);
        top = lowest - DOWN;
    }

    // The fans on the right, mirrored, from the middle out.
    let mut x = middle + span + SIDE;
    let mut edge = x;

    for (place, &k) in sides[1].iter().enumerate().rev() {
        let fan = &mut fans[k];

        fan.mirrored = true;
        fan.low = rotor == Rotor::Axial && place > 0;
        fan.centre = v(x + reach.max.x, -depth);
        edge = fan.centre.x - reach.min.x;

        if rotor == Rotor::Blower {
            x = edge + BETWEEN;
        }
    }

    // The case's floor: under the blocks, and far enough under the fans at
    // the top for the air drawn up to them, or for a tower's intakes under
    // them.
    let mut floor = bottom - DOWN;

    if !fans.is_empty() {
        floor = floor.min(-depth + reach.min.y - AIRWAY);
    }

    if fans.iter().any(|fan| fan.low) {
        floor = floor.min(-depth + reach.min.y - STACKED - reach.height() - INSET);

        for fan in fans.iter_mut().filter(|fan| fan.low) {
            fan.centre.y = floor + INSET - reach.min.y;
        }
    }

    let sink = (fans.is_empty() && piped && !first.is_empty()).then_some((x, x + SINK));
    let right = match sink {
        Some((_, end)) => end + INSET,
        None if !sides[1].is_empty() => edge + INSET,
        None => middle + span + SIDE / 2.0,
    };
    let mut vents: Vec<(f32, f32)> = fans
        .iter()
        .filter(|fan| !fan.low)
        .map(|fan| fan.vent(rotor))
        .collect();

    vents.extend(sink);

    if vents.is_empty() && piped {
        vents.push((centre - SINK / 2.0, centre + SINK / 2.0));
    }

    let case = Extent::new(v(0.0, floor), v(right, 0.0));
    let mut intakes: Vec<f32> = fans.iter().map(|fan| fan.centre.x).collect();

    intakes.sort_by(f32::total_cmp);
    intakes.dedup_by(|a, b| (*a - *b).abs() < 0.5);

    if intakes.is_empty() {
        intakes.push(centre);
    }

    // The pipe from the fins at one end to those at the other, over every
    // block it drops to.
    let drops: Vec<f32> = if piped {
        first
            .iter()
            .map(|&index| sources[index].frame.centre().x)
            .collect()
    } else {
        Vec::new()
    };
    let pipe = (!drops.is_empty() && !vents.is_empty()).then(|| {
        let ends = vents
            .iter()
            .map(|&(a, b)| (a + 2.0, b - 2.0))
            .chain(drops.iter().map(|&x| (x - BORE, x + BORE)));
        let from = ends.clone().map(|(a, _)| a).fold(f32::INFINITY, f32::min);
        let to = ends.map(|(_, b)| b).fold(f32::NEG_INFINITY, f32::max);

        (from, to)
    });

    // The pipe's name, in the longest stretch of it clear of the drops and
    // the fans.
    let legend = pipe.and_then(|(from, to)| {
        let mut clear: Vec<(f32, f32)> = drops
            .iter()
            .map(|&x| (x - BORE / 2.0 - 3.0, x + BORE / 2.0 + 3.0))
            .collect();

        clear.extend(fans.iter().map(|fan| fan.span(rotor)));
        clear.extend(sink);

        let (a, b) = gaps(from, to, &clear)
            .into_iter()
            .max_by(|a, b| (a.1 - a.0).total_cmp(&(b.1 - b.0)))?;
        let room = letters("HEAT PIPE".len()) + 8.0;

        (b - a >= room).then(|| v(((a + b) / 2.0).round(), PIPE - BORE / 2.0 - 7.0))
    });

    // What the air cools gives its heat to the air drawn up the side
    // nearer it (in one column between fans on both sides, to each side in
    // turn), to the fan nearest it there that turns; or with none, to the
    // air rising.
    let aired: Vec<usize> = if piped {
        rest.clone()
    } else {
        first.iter().chain(&rest).copied().collect()
    };
    let mut alternate = false;
    let arrows = aired
        .iter()
        .map(|&source| {
            let frame = sources[source].frame;
            let y = frame.max.y - TITLE;

            if fans.is_empty() {
                let x = frame.centre().x;
                let room = if sources[source].kind.finned() {
                    heatsinks
                } else {
                    0.0
                };

                return Arrow {
                    source,
                    from: v(x, frame.max.y + room + 2.0),
                    to: v(x, frame.max.y + room + DOWN - 4.0),
                    fan: None,
                };
            }

            let middle = frame.centre().x;
            let leftward = match (sides[0].is_empty(), sides[1].is_empty()) {
                (false, true) => true,
                (true, false) => false,
                _ if (middle - centre).abs() < 1.0 => {
                    alternate = !alternate;
                    alternate
                }
                _ => middle < centre,
            };
            let side = &sides[if leftward { 0 } else { 1 }];
            // The fan that takes the heat: a blower's nearest the blocks
            // that turns, an axial one's at the top if it turns.
            let order: Vec<usize> = match rotor {
                Rotor::Blower => side.iter().rev().copied().collect(),
                Rotor::Axial => side.clone(),
            };
            let fan = order
                .iter()
                .copied()
                .find(|&k| fans[k].turning)
                .unwrap_or(order[0]);
            let column = fans[fan].centre.x;
            // Short of any housing on the way at its height.
            let mut end = if leftward { column + 3.0 } else { column - 3.0 };

            for &k in side {
                let (low, high) = fans[k].height(rotor);
                let (a, b) = fans[k].span(rotor);

                if (low - 3.0..=high + 3.0).contains(&y) {
                    end = if leftward {
                        end.max(b + 3.0)
                    } else {
                        end.min(a - 3.0)
                    };
                }
            }

            let from = if leftward {
                frame.min.x - 3.0
            } else {
                frame.max.x + 3.0
            };

            Arrow {
                source,
                from: v(from, y),
                to: v(end, y),
                fan: Some(fan),
            }
        })
        .collect();

    let under = if fans.iter().any(|fan| fan.low) {
        24.0 + LINE + 6.0
    } else {
        30.0
    };
    let over = if vents.is_empty() {
        EXHAUST + 6.0
    } else {
        NAMES + LINE + 6.0
    };
    let extent = Extent::new(
        v(-MARGIN, floor - under - MARGIN),
        v(right + MARGIN, over + MARGIN),
    );

    Plan {
        case,
        vents,
        intakes,
        pipe,
        drops,
        sink,
        arrows,
        legend,
        extent,
    }
}

/// The stretches of `from` to `to` that `holes` leave.
fn gaps(from: f32, to: f32, holes: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut holes = holes.to_vec();
    let mut stretches = Vec::new();
    let mut at = from;

    holes.sort_by(|a, b| a.0.total_cmp(&b.0));

    for (a, b) in holes {
        if a > at {
            stretches.push((at, a.min(to)));
        }
        at = at.max(b);
    }

    if at < to {
        stretches.push((at, to));
    }

    stretches.retain(|(a, b)| b > a);
    stretches
}

/// A run of `n` characters' width at the laptop's scale.
fn letters(n: usize) -> f32 {
    n as f32 * f32::from(LETTERING.advance())
}

/// Sensor `index`'s temperature in `snapshot`, if it reads one.
fn celsius(snapshot: &Snapshot, index: usize) -> Option<f32> {
    snapshot.sensor(index).filter(|celsius| celsius.is_finite())
}

/// A fan's speed in `snapshot`: 0 when it reads nothing.
fn rpm(snapshot: &Snapshot, index: usize) -> f32 {
    snapshot
        .sensor(index)
        .filter(|rpm| rpm.is_finite() && *rpm > 0.0)
        .unwrap_or(0.0)
}

/// Whether a temperature is on the thermometers' scale, short of its ends.
fn on_scale(celsius: f32) -> bool {
    celsius > COLDEST && celsius < HOTTEST
}

/// A temperature as a sheet letters it.
fn degrees(celsius: Option<f32>) -> String {
    match celsius {
        Some(celsius) => format!("{celsius:.0} °C"),
        None => "-- °C".into(),
    }
}

/// A monitor's chip as its driver names it, without the bus or instance a
/// driver adds to tell its devices apart: `r8169_0_600:00` and
/// `mt7921_phy0` are `R8169` and `MT7921`.
fn chip(name: &str) -> String {
    let mut parts = name.split('_');
    let first = parts.next().unwrap_or_default();
    let named: Vec<&str> = std::iter::once(first)
        .chain(parts.take_while(|part| !part.chars().any(|c| c.is_ascii_digit())))
        .collect();

    lettered(&named.join("_"))
}

/// What a monitor calls a sensor, and the monitor: `PACKAGE ID 0, CORETEMP`.
fn called(sensor: &Sensor) -> String {
    match &sensor.label {
        Some(label) => format!("{}, {}", lettered(label), chip(&sensor.chip)),
        None => chip(&sensor.chip),
    }
}

/// What a drive is, in a word, as its block is named.
fn drive_name(drive: &Drive) -> &'static str {
    match drive.kind {
        DriveKind::Nvme => "NVMe",
        DriveKind::Mmc => "eMMC",
        DriveKind::Virtual => "DISK",
        DriveKind::Usb => "USB DRIVE",
        _ if drive.rotational => "HDD",
        DriveKind::Sata => "SSD",
        DriveKind::Other => "DRIVE",
    }
}

/// The heat sources of `machine`: what its temperature sensors are on (a
/// processor package, the memory, a drive, a device, the board), then its
/// batteries.
fn sources(machine: &Machine) -> Vec<Source> {
    let mut sources: Vec<Source> = Vec::new();
    let mut modules: Vec<(usize, usize)> = Vec::new();

    for (index, sensor) in machine.sensors.iter().enumerate() {
        if sensor.kind != SensorKind::Temperature {
            continue;
        }

        let mut source = match sensor.site {
            Site::Processor(package) => Source {
                package: Some(package),
                ..Source::new(Kind::Processor)
            },
            Site::Module(module) => {
                modules.push((module, index));
                Source::new(Kind::Memory)
            }
            Site::Drive(drive) => Source {
                drive: Some(drive),
                ..Source::new(Kind::Drive)
            },
            Site::Board => Source::new(Kind::Board),
            Site::Device(address) => Source {
                device: Some(address),
                ..Source::new(device(machine, address))
            },
        };

        match sources.iter_mut().find(|other| {
            (other.kind, other.device, other.drive, other.package)
                == (source.kind, source.device, source.drive, source.package)
        }) {
            Some(other) => other.sensors.push(index),
            None => {
                source.sensors.push(index);
                sources.push(source);
            }
        }
    }

    // A device's own fans are its, not the case's.
    for (index, sensor) in machine.fans() {
        if let Site::Device(address) = sensor.site
            && let Some(source) = sources
                .iter_mut()
                .find(|source| source.device == Some(address) && source.drive.is_none())
        {
            source.fans.push(index);
        }
    }

    sources.extend((0..machine.batteries.len()).map(|battery| Source {
        battery: Some(battery),
        ..Source::new(Kind::Battery)
    }));
    sources.sort_by_key(|source| (source.kind, source.package, source.drive));

    let label = |index: usize| machine.sensors[index].label.as_deref();

    for source in &mut sources {
        match source.kind {
            // The package's (or the die's) thermometer, and the cores'
            // squares.
            Kind::Processor => {
                let package = source
                    .sensors
                    .iter()
                    .position(|&index| label(index).is_some_and(package))
                    .unwrap_or(0);
                let mut others = source.sensors.clone();

                source.gauges = vec![others.remove(package)];
                source.cores = others.into_iter().take(MOST_CORES).collect();
            }
            // A thermometer a module, in the modules' order.
            Kind::Memory => {
                modules.sort();
                modules.dedup_by_key(|(module, _)| *module);
                source.gauges = modules
                    .iter()
                    .map(|&(_, index)| index)
                    .take(MOST_GAUGES)
                    .collect();
            }
            // A board's monitors with what they measure named first: its
            // own inputs (SYSTIN, CPUTIN) before a thermal zone's.
            Kind::Board => {
                let mut sensors = source.sensors.clone();

                sensors.sort_by_key(|&index| label(index).is_none());
                source.gauges = sensors.into_iter().take(MOST_GAUGES).collect();
            }
            _ => source.gauges = source.sensors.iter().copied().take(MOST_GAUGES).collect(),
        }

        let chip = source
            .sensors
            .first()
            .map(|&index| machine.sensors[index].chip.as_str())
            .unwrap_or_default();

        source.name = match source.kind {
            Kind::Processor => "CPU",
            Kind::Graphics => "GPU",
            Kind::Memory => "MEMORY",
            Kind::Drive => source
                .drive
                .and_then(|drive| machine.drives.get(drive))
                .map_or("DRIVE", drive_name),
            Kind::Network => match link(machine, source.device) {
                Some(Link::Wireless) => "WI-FI",
                Some(Link::Ethernet) => "ETHERNET",
                _ => "NETWORK",
            },
            Kind::Device => match source
                .device
                .and_then(|address| machine.pci_device(address))
                .map(|device| device.kind())
            {
                Some(PciKind::Usb) => "USB",
                Some(PciKind::Audio) => "AUDIO",
                Some(PciKind::Chipset | PciKind::Bridge) => "CHIPSET",
                _ if chip.starts_with("pch") => "CHIPSET",
                _ => "DEVICE",
            },
            Kind::Board => "BOARD",
            Kind::Battery => "BATTERY",
        }
        .into();
    }

    // Sources of the same name numbered apart, and the first of each kind
    // in its part's detail.
    let names: Vec<String> = sources.iter().map(|source| source.name.clone()).collect();
    let kinds: Vec<Kind> = sources.iter().map(|source| source.kind).collect();

    for (index, source) in sources.iter_mut().enumerate() {
        if names.iter().filter(|name| **name == source.name).count() > 1 {
            let nth = names[..index]
                .iter()
                .filter(|name| **name == source.name)
                .count();
            source.name = fit(&format!("{} {}", source.name, nth + 1), 10);
        }

        source.detailed = !kinds[..index].contains(&source.kind);
    }

    sources
}

/// Whether a processor's sensor `label` is its package's or its die's, not
/// a core's.
fn package(label: &str) -> bool {
    let label = label.to_lowercase();

    ["package", "tctl", "tdie"]
        .iter()
        .any(|word| label.contains(word))
}

/// What the PCI device at `address` is, as a source of heat.
fn device(machine: &Machine, address: PciAddress) -> Kind {
    if machine
        .drives
        .iter()
        .any(|drive| drive.pci == Some(address))
    {
        return Kind::Drive;
    }

    match machine.pci_device(address).map(|device| device.kind()) {
        Some(PciKind::Display) => Kind::Graphics,
        Some(PciKind::Storage) => Kind::Drive,
        Some(PciKind::Network) => Kind::Network,
        _ => Kind::Device,
    }
}

/// The link of the network adapter at `address`.
fn link(machine: &Machine, address: Option<PciAddress>) -> Option<Link> {
    machine
        .interfaces
        .iter()
        .find(|interface| interface.pci == address && !interface.usb)
        .map(|interface| interface.link)
}

/// The anchor lettering in a block's corners is set by: its capitals' top.
const CAPS_LEFT: Anchor = Anchor::new(Horizontal::Left, Vertical::CapTop);
const CAPS_RIGHT: Anchor = Anchor::new(Horizontal::Right, Vertical::CapTop);

pub struct Cooling {
    card: Card,
    machine: Machine,
    rotor: Rotor,
    sources: Vec<Source>,
    fans: Vec<Fan>,
    plan: Plan,
    /// The parts list's items, in its order.
    items: Vec<Item>,
    /// How far each fan has turned, in radians, and the moment it was
    /// drawn at: its speed changes as the machine is sampled, so its
    /// turning is carried from frame to frame rather than worked out from
    /// the moment alone.
    turned: RefCell<Vec<Option<(f64, f64)>>>,
    /// Whether each fan has been seen turning.
    seen: RefCell<Vec<bool>>,
    /// The samples the chart was last drawn from, and the second they end
    /// at: drawn again a second later.
    history: RefCell<Option<(i64, Vec<std::sync::Arc<Snapshot>>)>>,
    /// The plan laid out for each view smaller than the laptop's that a
    /// sheet has asked for, if it reads the same there.
    fitted: RefCell<Vec<(V2, Option<Rc<Cooling>>)>>,
}

impl Cooling {
    /// The cooling of `machine`, if it measures a temperature.
    pub fn new(machine: &Machine) -> Option<Self> {
        Self::within(machine, BUDGET)
    }

    /// The cooling of `machine` laid out to fit `budget` best (see
    /// [`Cooling::fold`]).
    fn within(machine: &Machine, budget: V2) -> Option<Self> {
        if !machine
            .sensors
            .iter()
            .any(|sensor| sensor.kind == SensorKind::Temperature)
        {
            return None;
        }

        let rotor = match machine.chassis.kind {
            ChassisKind::Desktop | ChassisKind::Server => Rotor::Axial,
            _ => Rotor::Blower,
        };
        // The case's fans: not a device's, and not a board's headers that
        // read 0 when the machine was first read, which are empty as often
        // as not. Those turning are drawn before those that were not.
        let first = machine.sample(0.0);
        let mut headers = 0;
        let mut listed: Vec<(usize, &Sensor)> = Vec::new();

        for (index, sensor) in machine.fans() {
            if matches!(sensor.site, Site::Device(_)) {
                continue;
            }

            if rpm(&first, index) <= 0.0
                && HEADER_CHIPS
                    .iter()
                    .any(|chip| sensor.chip.starts_with(chip))
            {
                headers += 1;
                continue;
            }

            listed.push((index, sensor));
        }

        listed.sort_by_key(|&(sensor, _)| rpm(&first, sensor) <= 0.0);

        let drawn = &listed[..listed.len().min(MOST_FANS)];
        // Named by their monitors' numbers for them where those tell them
        // apart, as `sensors` and a board's headers name them.
        let mut channels: Vec<u32> = drawn.iter().map(|(_, sensor)| sensor.channel).collect();

        channels.sort();
        channels.dedup();

        let numbered = channels.len() == drawn.len()
            && channels
                .iter()
                .all(|&channel| (1..=FAN_NAMES.len() as u32).contains(&channel));
        let fans: Vec<Fan> = drawn
            .iter()
            .enumerate()
            .map(|(k, &(sensor, about))| Fan {
                sensor,
                name: FAN_NAMES[if numbered {
                    about.channel as usize - 1
                } else {
                    k
                }],
                label: about.label.clone(),
                chip: about.chip.clone(),
                turning: rpm(&first, sensor) > 0.0,
                centre: V2::ZERO,
                mirrored: false,
                low: false,
            })
            .collect();
        let (sources, fans, plan) = Self::fold(sources(machine), fans, rotor, budget);

        let mut items: Vec<Item> = Vec::new();

        for source in &sources {
            if !items.contains(&Item::Sources(source.kind)) {
                items.push(Item::Sources(source.kind));
            }
        }

        if !fans.is_empty() {
            items.push(Item::Fans);
        }

        let mut notes = vec!["READ FROM /sys/class/hwmon".to_owned()];

        if fans.is_empty() {
            notes.push("NO FAN MEASURED: NONE TURNS".into());
        } else {
            notes.push(format!("ROTORS SLOWED {SLOWED} TIMES"));
        }

        if fans.iter().any(|fan| !fan.turning) {
            notes.push("PHANTOM FANS: NOT SEEN TURNING".into());
        }

        notes.push(format!("THERMOMETERS {COLDEST} TO {HOTTEST} °C"));

        if sources
            .iter()
            .flat_map(|source| &source.gauges)
            .any(|&gauge| machine.sensors[gauge].limit.is_some_and(on_scale))
        {
            notes.push("TICK OVER A TUBE: ITS HIGH LIMIT".into());
        }

        if listed.len() > fans.len() {
            notes.push(format!(
                "{} MORE NOT SHOWN",
                counted(listed.len() - fans.len(), "FAN", "FANS")
            ));
        }

        if headers > 0 {
            notes.push(format!(
                "{} AT 0 rpm OMITTED",
                counted(headers, "FAN HEADER", "FAN HEADERS")
            ));
        }

        let mut cooling = Self {
            card: Card {
                title: format!("{} COOLING", machine.chassis.kind.label()),
                number: "QD-C-0003".into(),
                domain: Domain::Computing,
                unit: Unit::Millimetre,
                scaled: false,
                view: "SCHEMATIC PLAN".into(),
                notes,
                revisions: vec![Revision::first()],
                parts: Vec::new(),
            },
            machine: machine.clone(),
            rotor,
            turned: RefCell::new(vec![None; fans.len()]),
            seen: RefCell::new(vec![false; fans.len()]),
            history: RefCell::new(None),
            fitted: RefCell::default(),
            sources,
            fans,
            plan,
            items,
        };

        cooling.card.parts = cooling
            .items
            .iter()
            .map(|&item| cooling.part(item))
            .collect();

        Some(cooling)
    }

    /// `sources` and `fans` laid out in as many columns as fit `budget`
    /// (the laptop's view, or a smaller one) best: two, a side each, when
    /// there are fans on both sides; one by the fans on one side; up to
    /// three, the air rising, with none.
    fn fold(
        sources: Vec<Source>,
        fans: Vec<Fan>,
        rotor: Rotor,
        budget: V2,
    ) -> (Vec<Source>, Vec<Fan>, Plan) {
        let columns: &[usize] = match fans.len() {
            0 => &[3, 2, 1],
            1 => &[1],
            _ => &[2, 1],
        };
        let overflow =
            |plan: &Plan| (plan.extent.width() / budget.x).max(plan.extent.height() / budget.y);

        columns
            .iter()
            .map(|&columns| {
                let (mut sources, mut fans) = (sources.clone(), fans.clone());
                let plan = plan(&mut sources, &mut fans, rotor, columns);

                (sources, fans, plan)
            })
            .reduce(|best, next| {
                // The first that fits, or else the one that overflows least.
                if overflow(&best.2) <= 1.0 || overflow(&best.2) <= overflow(&next.2) {
                    best
                } else {
                    next
                }
            })
            .expect("At least one way to lay it out")
    }

    /// The parts list's index of `item`.
    fn index(&self, item: Item) -> usize {
        self.items
            .iter()
            .position(|other| *other == item)
            .expect("Every item drawn is listed")
    }

    /// The points fan `fan`'s drawing is set on the grid by: a blower's
    /// from the bottom of the fins its outlet meets.
    fn fan_snaps(&self, fan: &Fan) -> Vec<V2> {
        match self.rotor {
            Rotor::Blower => vec![TOP, AXIS, UNDER_FINS, fan.centre],
            Rotor::Axial => vec![TOP, fan.centre],
        }
    }

    /// The case in phantom, broken at its vents and intakes, and, with no
    /// fan to draw it, the air going in at its base and out of its vents.
    fn case(&self, d: &mut Draft) {
        let plan = &self.plan;
        let Extent { min, max } = plan.case;

        for (x, inward) in [(min.x, CHAMFER), (max.x, -CHAMFER)] {
            d.polyline(
                &[
                    v(x + inward, max.y),
                    v(x, max.y - CHAMFER),
                    v(x, min.y + CHAMFER),
                    v(x + inward, min.y),
                ],
                Line::Phantom,
            );
        }

        let intakes: Vec<(f32, f32)> = plan
            .intakes
            .iter()
            .map(|&x| (x - INTAKE, x + INTAKE))
            .collect();

        for (a, b) in gaps(min.x + CHAMFER, max.x - CHAMFER, &plan.vents) {
            d.line(v(a, max.y), v(b, max.y), Line::Phantom);
        }

        for (a, b) in gaps(min.x + CHAMFER, max.x - CHAMFER, &intakes) {
            d.line(v(a, min.y), v(b, min.y), Line::Phantom);
        }

        for &x in &plan.intakes {
            if !self.fans.iter().any(|fan| fan.low && fan.centre.x == x) {
                d.label(v(x, min.y - 24.0), "AIR IN");
            }
        }

        if self.fans.is_empty() {
            for &(a, b) in &plan.vents {
                for x in exhausts(a, b) {
                    d.arrow(v(x, 2.0), v(x, EXHAUST), Line::Thin);
                }
            }

            for &x in &plan.intakes {
                d.arrow(v(x, min.y - 16.0), v(x, min.y + DOWN), Line::Thin);
            }
        }

        if let Some((a, b)) = plan.sink {
            d.label(v((a + b) / 2.0, NAMES), "FINS").tone(Tone::Muted);
        }
    }

    /// Where the air drawn up to `fan` meets it: under its housing.
    fn inlet(&self, fan: &Fan) -> f32 {
        fan.height(self.rotor).0 - 3.0
    }

    /// The air into `fan`, a path from under the case or, a tower's fan at
    /// the top, from the one at the base under it.
    fn intake(&self, fan: &Fan) -> [V2; 2] {
        let x = fan.centre.x;
        let from = match self
            .fans
            .iter()
            .find(|low| low.low && !fan.low && low.centre.x == x)
        {
            Some(low) => low.height(self.rotor).1 + 3.0,
            None => self.plan.case.min.y - 16.0,
        };

        [v(x, from), v(x, self.inlet(fan))]
    }

    /// The heat pipe and its drops to the blocks it cools, and the fins it
    /// ends in with no fan.
    fn pipe(&self, d: &mut Draft) {
        let plan = &self.plan;
        let Some((from, to)) = plan.pipe else {
            return;
        };
        let half = BORE / 2.0;
        let (upper, lower) = (PIPE + half, PIPE - half);

        set(d, &[TOP, AXIS], |d| {
            let drops: Vec<(f32, f32)> = plan.drops.iter().map(|&x| (x - half, x + half)).collect();

            d.line(v(from, upper), v(to, upper), Line::Outline);

            for (a, b) in gaps(from, to, &drops) {
                d.line(v(a, lower), v(b, lower), Line::Outline);
            }

            for x in [from, to] {
                d.line(v(x, upper), v(x, lower), Line::Outline);
            }

            // Each drop symmetric about its axis, from the pipe down to the
            // top of its block.
            for &x in &plan.drops {
                d.snapped(v(x, PIPE), |d| {
                    for side in [-half, half] {
                        d.line(v(x + side, lower), v(x + side, -COOLED), Line::Outline);
                    }
                });
            }
        });

        if let Some(at) = plan.legend {
            d.label(at, "HEAT PIPE");
        }

        if let Some(sink) = plan.sink {
            self.fins(d, sink, Line::Outline, Line::Thin);
        }
    }

    /// The fins across a vent from `a` to `b`, split round the pipe that
    /// runs through them.
    fn fins(&self, d: &mut Draft, (a, b): (f32, f32), edge: Line, fin: Line) {
        let half = BORE / 2.0;
        let piped = self.plan.pipe.is_some();
        let count = ((b - a) / FIN).floor().max(1.0) as usize;
        let first = a + (b - a - (count - 1) as f32 * FIN) / 2.0;

        set(d, &[TOP, AXIS], |d| {
            for x in [a, b] {
                d.line(v(x, -FINS), v(x, 0.0), edge);
            }

            for k in 0..count {
                let x = first + k as f32 * FIN;

                if piped {
                    d.line(v(x, -FINS), v(x, PIPE - half), fin);
                    d.line(v(x, PIPE + half), v(x, 0.0), fin);
                } else {
                    d.line(v(x, -FINS), v(x, 0.0), fin);
                }
            }
        });
    }

    /// A source's block: its outline and name, its gauges' glass, its
    /// cores' squares, a desktop's heatsink.
    fn block(&self, d: &mut Draft, source: &Source) {
        let frame = source.frame;

        source.set(d, |d| {
            d.rect(frame.min, frame.max, Line::Outline);
            d.label(
                v(frame.min.x + PAD, frame.max.y - PAD),
                source.name.as_str(),
            )
            .anchor(CAPS_LEFT)
            .nudge(-1, 0)
            .tone(Tone::Ink);

            if !source.fans.is_empty() {
                d.label(v(frame.min.x + PAD, frame.max.y - PAD - LINE), "FAN")
                    .anchor(CAPS_LEFT)
                    .nudge(-1, 0)
                    .tone(Tone::Muted);
            }

            // A heatsink's fins standing on it.
            if self.rotor == Rotor::Axial && source.kind.finned() {
                let count = ((BLOCK - 2.0 * PAD) / FIN).floor() as usize;

                for k in 0..=count {
                    let x = frame.min.x + PAD + k as f32 * FIN;
                    let line = if k == 0 || k == count {
                        Line::Outline
                    } else {
                        Line::Thin
                    };

                    d.line(v(x, frame.max.y), v(x, frame.max.y + HEATSINK), line);
                }
            }
        });

        if source.kind == Kind::Battery {
            let foot = source.foot();
            let middle = foot + v(0.0, CELL_HALF);

            source.set_foot(d, |d| {
                d.snapped(foot, |d| {
                    d.snapped(middle, |d| {
                        d.rect(
                            middle - v(0.0, CELL_HALF),
                            middle + v(CELL, CELL_HALF),
                            Line::Outline,
                        );
                        d.rect(
                            middle + v(CELL, -TERMINAL_HALF),
                            middle + v(CELL + TERMINAL, TERMINAL_HALF),
                            Line::Outline,
                        );
                    });
                });
            });
        } else {
            for (k, &sensor) in source.gauges.iter().enumerate() {
                let limit = self.limit(sensor);

                source.set_gauge(d, k, |d, at| {
                    d.keyhole(at, BULB_RADIUS, GLASS, BULB_RADIUS + TUBE, Line::Outline);

                    // Where its chip says it should stay under, ticked over
                    // the tube.
                    if let Some(limit) = limit {
                        let x = at.x + BULB_RADIUS + TUBE * (limit - COLDEST) / (HOTTEST - COLDEST);

                        d.line(v(x, at.y + GLASS), v(x, at.y + GLASS + LIMIT), Line::Thin)
                            .tone(Tone::Caution);
                    }

                    // The scale, every 20 °C, where a detail has room for it.
                    d.in_detail(|d| {
                        for step in 0..=4 {
                            let x = at.x + BULB_RADIUS + TUBE * step as f32 / 4.0;
                            d.line(v(x, at.y - GLASS), v(x, at.y - GLASS - 1.5), Line::Thin);
                        }
                    });
                });
            }

            for k in 0..source.cores.len() {
                source.set_core(d, k, |d, square| {
                    d.rect(square.min, square.max, Line::Thin);
                });
            }
        }

        // Its name in its own detail, over its first gauge, where the
        // detail has it in view.
        source.set_gauge(d, 0, |d, at| {
            let top = if source.kind == Kind::Battery {
                at.y + CELL_HALF - BULB_RADIUS
            } else {
                at.y
            };

            in_its_detail(d, source, |d| {
                d.label(
                    v(at.x - BULB_RADIUS, top + BULB_RADIUS + 2.0),
                    source.name.as_str(),
                )
                .anchor(Anchor::BASELINE_LEFT)
                .nudge(-1, 0)
                .tone(Tone::Ink);
            });
        });
    }

    /// What a source's gauges read now: its hottest temperature, each
    /// thermometer's column and its value (in its detail), its cores'
    /// tints, its fan's speed; a battery's charge and its power.
    fn readings_of(&self, d: &mut Draft, source: &Source, snapshot: &Snapshot) {
        let frame = source.frame;
        let value = v(frame.max.x - PAD, frame.max.y - PAD);
        let valued = |d: &mut Draft, text: String| {
            source.set_right(d, |d| {
                d.label(value, text)
                    .anchor(CAPS_RIGHT)
                    .nudge(1, 0)
                    .tone(Tone::Muted);
            });
        };

        if let Some(battery) = source.battery {
            let charge = snapshot.batteries.get(battery).cloned().unwrap_or_default();
            let foot = source.foot();
            let middle = foot + v(0.0, CELL_HALF);
            let fraction = charge
                .fraction
                .filter(|f| f.is_finite())
                .unwrap_or(0.0)
                .clamp(0.0, 1.0);
            let tone = if fraction < 0.15 {
                Tone::Caution
            } else {
                Tone::Live
            };
            let watts = charge.watts.filter(|w| w.is_finite());
            let power = match (charge.state, watts) {
                (Some(ChargeState::Full), _) => "FULL".into(),
                (Some(ChargeState::Idle), _) => "IDLE".into(),
                (Some(ChargeState::Charging), Some(watts)) => format!("+{watts:.1} W"),
                (_, Some(watts)) => format!("−{watts:.1} W"),
                _ => String::new(),
            };
            let state = match charge.state {
                Some(ChargeState::Charging) => "CHARGING",
                Some(ChargeState::Discharging) => "DISCHARGING",
                Some(ChargeState::Full) => "FULL",
                Some(ChargeState::Idle) => "IDLE",
                None => "",
            };
            let percent = charge
                .fraction
                .map_or("-- %".into(), |f| format!("{:.0} %", f * 100.0));

            valued(d, percent.clone());

            source.set_foot(d, |d| {
                d.snapped(foot, |d| {
                    d.snapped(middle, |d| {
                        // The charge inside the body, up to its share of it.
                        if fraction > 0.0 {
                            d.fill_inside(
                                rectangle(Extent::new(
                                    middle - v(0.0, CELL_HALF),
                                    middle + v(CELL * fraction, CELL_HALF),
                                )),
                                Fill::Solid,
                            )
                            .tone(tone);
                        }

                        d.label(middle + v(CELL + TERMINAL + 6.0, 0.0), power)
                            .anchor(Anchor::LEFT)
                            .tone(Tone::Muted);

                        // What it is doing, in words, in its own detail.
                        in_its_detail(d, source, |d| {
                            let right = middle.x + CELL + TERMINAL + 4.0;
                            let mut how = percent;

                            if let Some(watts) = watts {
                                how += &format!(", {watts:.1} W");
                            }

                            d.label(v(right, middle.y + 3.0), state)
                                .anchor(Anchor::LEFT)
                                .tone(Tone::Ink);
                            d.label(v(right, middle.y - 3.0), how)
                                .anchor(Anchor::LEFT)
                                .tone(Tone::Muted);
                        });
                    });
                });
            });
            return;
        }

        valued(d, degrees(source.hottest(snapshot)));

        if let Some(&fan) = source.fans.first() {
            source.set_right(d, |d| {
                d.label(
                    value - v(0.0, LINE),
                    format!("{:.0} rpm", rpm(snapshot, fan)),
                )
                .anchor(CAPS_RIGHT)
                .nudge(1, 0)
                .tone(Tone::Ink);
            });
        }

        for (k, &sensor) in source.gauges.iter().enumerate() {
            let reading = celsius(snapshot, sensor);

            source.set_gauge(d, k, |d, at| {
                // An unread thermometer is empty glass.
                if let Some(celsius) = reading {
                    let share = ((celsius - COLDEST) / (HOTTEST - COLDEST)).clamp(0.0, 1.0);

                    d.fill_inside(
                        Shape::Keyhole {
                            centre: at,
                            radius: BULB_RADIUS,
                            half: GLASS,
                            length: BULB_RADIUS + TUBE * share,
                        },
                        Fill::Solid,
                    )
                    .tone(warmth(celsius));
                }

                in_its_detail(d, source, |d| {
                    d.label(
                        v(at.x + BULB_RADIUS + TUBE + DETAIL_GAP, at.y),
                        degrees(reading),
                    )
                    .anchor(Anchor::LEFT)
                    .tone(Tone::Ink);
                });
            });
        }

        for (k, &sensor) in source.cores.iter().enumerate() {
            let Some(c) = celsius(snapshot, sensor) else {
                continue;
            };
            let level = (1.0 + 15.0 * (c - WARM) / (HOTTEST - WARM)).round() as u8;

            source.set_core(d, k, |d, square| {
                d.fill_inside(rectangle(square), Fill::Tint(level.clamp(1, 16)))
                    .tone(warmth(c));
            });
        }
    }

    /// Fan `index` turned to `angle`: its housing, its fins and its rotor,
    /// in phantom until it has been seen turning, the air through it, and
    /// its name and speed over its vent or under its intake.
    fn fan(&self, d: &mut Draft, index: usize, angle: f32, seen: bool, speed: f32) {
        let fan = &self.fans[index];
        let (edge, thin) = if seen {
            (Line::Outline, Line::Thin)
        } else {
            (Line::Phantom, Line::Phantom)
        };
        let drawn = |points: &[V2]| points.iter().map(|&p| fan.at(p)).collect::<Vec<_>>();

        set(d, &self.fan_snaps(fan), |d| {
            d.polyline(&drawn(&self.rotor.housing()), edge);

            match self.rotor {
                Rotor::Blower => {
                    d.circle(fan.centre, ROOT, thin);

                    for k in 0..BLADES {
                        let base = angle + k as f32 * TAU / BLADES as f32;
                        let blade: Vec<V2> = (0..=4)
                            .map(|s| {
                                let s = s as f32 / 4.0;
                                polar(ROOT + (ROTOR - ROOT) * s, base + CURVE * s)
                            })
                            .collect();

                        d.polyline(&drawn(&blade), edge);
                    }
                }
                Rotor::Axial => {
                    for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                        d.circle(fan.centre + v(x, y) * (FRAME - 5.0), 2.0, edge);
                    }

                    d.circle(fan.centre, ROTOR + 4.0, edge);

                    for k in 0..VANES {
                        let base = angle + k as f32 * TAU / VANES as f32;
                        let mut vane: Vec<V2> = (0..=4)
                            .map(|s| {
                                let s = s as f32 / 4.0;
                                polar(HUB + (ROTOR + 1.0 - HUB) * s, base + 0.3 + 0.35 * s)
                            })
                            .collect();

                        vane.extend(arc_points(V2::ZERO, ROTOR + 1.0, base + 0.65, -0.75, 3));
                        vane.extend((0..=4).rev().map(|s| {
                            let s = s as f32 / 4.0;
                            polar(HUB + (ROTOR + 1.0 - HUB) * s, base - 0.25 + 0.15 * s)
                        }));

                        d.polygon(&drawn(&vane), edge);
                    }
                }
            }

            d.circle(fan.centre, HUB, edge);

            // Which way it turns, in its own detail.
            d.in_own_detail(|d| {
                let arc = drawn(&arc_points(V2::ZERO, HUB - 3.0, 0.3 * PI, 1.2 * PI, 16));
                let (tail, head) = (arc[arc.len() - 2], arc[arc.len() - 1]);

                d.polyline(&arc[..arc.len() - 1], Line::Thin)
                    .tone(Tone::Live);
                d.arrow(tail, head, Line::Thin).tone(Tone::Live);
            });
        });

        if self.rotor == Rotor::Blower {
            self.fins(d, fan.vent(self.rotor), edge, thin);
        }

        // The air through it: in from under it, and out of its vent over
        // it, or into the fan over it.
        let path = if seen { Line::Thin } else { Line::Phantom };
        let [from, to] = self.intake(fan);

        d.arrow(from, to, path);

        let (name, value) = if seen {
            (Tone::Muted, Tone::Ink)
        } else {
            (Tone::Faint, Tone::Faint)
        };

        if fan.low {
            let base = self.plan.case.min.y;

            d.label(v(fan.centre.x, base - 24.0), format!("{speed:.0} rpm"))
                .tone(value);
            d.label(v(fan.centre.x, base - 24.0 - LINE), fan.name)
                .tone(name);
        } else {
            let (a, b) = fan.vent(self.rotor);
            let middle = ((a + b) / 2.0).round();

            for x in exhausts(a, b) {
                d.arrow(v(x, 2.0), v(x, EXHAUST), path);
            }

            d.label(v(middle, NAMES), fan.name)
                .nudge(0, -(LINE as i32))
                .tone(name);
            d.label(v(middle, NAMES), format!("{speed:.0} rpm"))
                .tone(value);
        }
    }

    /// What runs along the paths: heat up the pipe from what it cools, more
    /// of it the hotter that is, to the fins of each fan seen turning; the
    /// air each fan draws in and blows out, as far as the fan has turned;
    /// the heat that air takes on its way.
    fn flows(&self, d: &mut Draft, snapshot: &Snapshot, turns: &[f64], seen: &[bool], t: f32) {
        let heat = |source: &Source| {
            source
                .hottest(snapshot)
                .map_or(0.0, |c| ((c - WARM) / (SCALDING - WARM)).clamp(0.05, 1.0))
        };
        let piped = self.sources.iter().filter(|source| source.kind.finned());

        if self.plan.pipe.is_some() {
            let vents: Vec<(f32, f32)> = self
                .fans
                .iter()
                .zip(seen)
                .filter(|&(_, &seen)| seen)
                .map(|(fan, _)| fan.vent(self.rotor))
                .chain(self.plan.sink)
                .collect();

            for (source, &x) in piped.zip(&self.plan.drops) {
                for &(a, b) in &vents {
                    let route = [vec![v(x, -COOLED), v(x, PIPE), v((a + b) / 2.0, PIPE)]];

                    flow(
                        d,
                        &route,
                        heat(source),
                        SPACING,
                        f64::from(t) * HEAT,
                        false,
                        Tone::Accent,
                    );
                }
            }
        }

        for (k, fan) in self.fans.iter().enumerate() {
            if !seen[k] {
                continue;
            }

            let travelled = turns[k] * AIR;
            let mut routes = vec![self.intake(fan).to_vec()];

            if !fan.low {
                let (a, b) = fan.vent(self.rotor);
                routes.extend(exhausts(a, b).map(|x| vec![v(x, 2.0), v(x, EXHAUST)]));
            }

            for route in routes {
                flow(d, &[route], 1.0, SPACING, travelled, false, Tone::Live);
            }

            for arrow in self.plan.arrows.iter().filter(|a| a.fan == Some(k)) {
                let route = [vec![arrow.from, arrow.to]];

                flow(
                    d,
                    &route,
                    heat(&self.sources[arrow.source]),
                    SPACING,
                    travelled,
                    false,
                    Tone::Accent,
                );
            }
        }
    }

    /// Whether each fan has been seen turning, as of `snapshot`.
    fn seen(&self, snapshot: &Snapshot) -> Vec<bool> {
        let mut seen = self.seen.borrow_mut();

        for (fan, seen) in self.fans.iter().zip(seen.iter_mut()) {
            *seen |= rpm(snapshot, fan.sensor) > 0.0;
        }

        seen.clone()
    }

    /// How far each fan has turned at `t`, in radians: as far as it had
    /// when last drawn a moment before, and on at its speed since; or,
    /// when it was not, as if it had always turned at its speed now.
    fn turns(&self, snapshot: &Snapshot, t: f32) -> Vec<f64> {
        let mut turned = self.turned.borrow_mut();
        let t = f64::from(t);

        self.fans
            .iter()
            .zip(turned.iter_mut())
            .map(|(fan, last)| {
                let speed =
                    f64::from(rpm(snapshot, fan.sensor) / SLOWED) / 60.0 * std::f64::consts::TAU;
                let angle = match *last {
                    Some((then, angle)) if (0.0..=STEP).contains(&(t - then)) => {
                        angle + speed * (t - then)
                    }
                    _ => speed * t,
                };

                *last = Some((t, angle));
                angle
            })
            .collect()
    }

    /// Whether the plan is wide enough for the chart under it.
    fn charted(&self) -> bool {
        let extent = self.plan.extent;

        (extent.height() + CHART) * WIDE_VIEW <= extent.width()
    }

    /// The chart's plot, under the plan and as wide as its case.
    fn plot(&self) -> Extent {
        let case = self.plan.case;

        Extent::new(
            v(case.min.x + CHART_SIDE, CHART_TIME),
            v(case.max.x - CHART_SIDE, CHART - CHART_KEY),
        )
    }

    /// The chart's frame, grid and fixed lettering: the temperatures up its
    /// left, the minutes along its foot.
    fn chart(&self, d: &mut Draft) {
        let plot = self.plot();
        let height =
            |celsius: f32| plot.min.y + (celsius - COLDEST) / (HOTTEST - COLDEST) * plot.height();

        d.rect(plot.min, plot.max, Line::Thin);

        let mut celsius = COLDEST;

        while celsius <= HOTTEST {
            let y = height(celsius);

            if celsius > COLDEST && celsius < HOTTEST {
                d.line(v(plot.min.x, y), v(plot.max.x, y), Line::Path)
                    .tone(Tone::Faint);
            }

            // Numbered every other line, as the rpm are, which a chart as
            // short as the laptop's has room for.
            if (celsius - COLDEST) % (2.0 * CHART_STEP) == 0.0 {
                d.label(v(plot.min.x - 4.0, y), format!("{celsius:.0}"))
                    .anchor(Anchor::RIGHT)
                    .tone(Tone::Muted);
            }

            celsius += CHART_STEP;
        }

        let minute = plot.max.x - plot.width() * 60.0 / (HISTORY - 1) as f32;

        d.line(v(minute, plot.min.y), v(minute, plot.max.y), Line::Path)
            .tone(Tone::Faint);

        for (x, text, anchor) in [
            (plot.min.x, "2 MIN AGO", Anchor::LEFT),
            (minute, "1 MIN", Anchor::CENTRE),
            (plot.max.x, "NOW", Anchor::RIGHT),
        ] {
            d.label(v(x, plot.min.y - 7.0), text)
                .anchor(anchor)
                .tone(Tone::Muted);
        }
    }

    /// The chart's traces from the samples up to `t`: the processor's
    /// temperature, the graphics' or else the hottest of the rest, and the
    /// first turning fan's speed on a scale of its own, each keyed over the
    /// plot in its tone with its value now.
    fn traces(&self, d: &mut Draft, now: &Snapshot, t: f32, seen: &[bool]) {
        let second = t.floor() as i64;
        let mut cached = self.history.borrow_mut();

        if cached.as_ref().is_none_or(|(at, _)| *at != second) {
            *cached = Some((second, self.machine.history(t)));
        }

        let Some((_, history)) = cached.as_ref().filter(|(_, history)| !history.is_empty()) else {
            return;
        };
        let plot = self.plot();
        let x = |k: usize| {
            plot.max.x - (history.len() - 1 - k) as f32 * plot.width() / (HISTORY - 1) as f32
        };
        let hottest = |kind: Kind, snapshot: &Snapshot| {
            self.sources
                .iter()
                .filter(|source| source.kind == kind)
                .filter_map(|source| source.hottest(snapshot))
                .reduce(f32::max)
        };
        // The second temperature: the graphics', or that of whatever else
        // has run hottest over the chart.
        let other = if self.sources.iter().any(|s| s.kind == Kind::Graphics) {
            Some(Kind::Graphics)
        } else {
            self.sources
                .iter()
                .filter(|source| !matches!(source.kind, Kind::Processor | Kind::Battery))
                .filter_map(|source| {
                    let hottest = history
                        .iter()
                        .filter_map(|snapshot| source.hottest(snapshot))
                        .reduce(f32::max)?;

                    Some((source.kind, hottest))
                })
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(kind, _)| kind)
        };
        let fan = (0..self.fans.len()).find(|&k| seen[k]);
        let fastest = fan.map_or(0.0, |k| {
            history
                .iter()
                .map(|snapshot| rpm(snapshot, self.fans[k].sensor))
                .fold(0.0, f32::max)
        });
        let top = (fastest / RPM_STEP).ceil().max(1.0) * RPM_STEP;
        let warmth = |celsius: f32| {
            plot.min.y + ((celsius - COLDEST) / (HOTTEST - COLDEST)).clamp(0.0, 1.0) * plot.height()
        };
        let speed = |rpm: f32| plot.min.y + (rpm / top).clamp(0.0, 1.0) * plot.height();
        let mut traces: Vec<Trace> = Vec::new();

        for (kind, tone) in [(Some(Kind::Processor), Tone::Accent), (other, Tone::Ink)] {
            let Some(kind) = kind else {
                continue;
            };
            let name = match kind {
                Kind::Processor => "CPU",
                Kind::Graphics => "GPU",
                kind => kind.part(),
            };
            let values: Vec<Option<f32>> = history
                .iter()
                .map(|snapshot| hottest(kind, snapshot).map(warmth))
                .collect();

            let celsius = hottest(kind, now);

            traces.push(Trace {
                name: name.into(),
                value: degrees(celsius),
                height: celsius.map(warmth),
                tone,
                points: values,
            });
        }

        if let Some(k) = fan {
            let values = history
                .iter()
                .map(|snapshot| Some(speed(rpm(snapshot, self.fans[k].sensor))))
                .collect();

            let turning = rpm(now, self.fans[k].sensor);

            traces.push(Trace {
                name: self.fans[k].name.into(),
                value: format!("{turning:.0} rpm"),
                height: Some(speed(turning)),
                tone: Tone::Live,
                points: values,
            });

            // Its scale up the right.
            for step in [0.0, 0.5, 1.0] {
                d.label(
                    v(plot.max.x + 4.0, plot.min.y + step * plot.height()),
                    format!("{:.0}", top * step),
                )
                .anchor(Anchor::LEFT)
                .tone(Tone::Live);
            }
        }

        // The key along the top, a whole number of pixels apart whatever
        // the scale.
        let mut key = 0;

        for Trace {
            name,
            value,
            height,
            tone,
            points,
        } in traces
        {
            // A trace broken where its source read nothing.
            for run in points
                .iter()
                .enumerate()
                .collect::<Vec<_>>()
                .split(|(_, value)| value.is_none())
            {
                let points: Vec<V2> = run
                    .iter()
                    .filter_map(|&(k, value)| Some(v(x(k), (*value)?)))
                    .collect();

                if points.len() > 1 {
                    d.polyline(&points, Line::Trace).tone(tone);
                }
            }

            // Where it is now, even before there is a trace to it.
            if let Some(y) = height {
                d.dot(v(plot.max.x, y), 3).tone(tone);
            }

            let text = format!("{name} {value}");

            d.label(v(plot.min.x, plot.max.y + 7.0), text.as_str())
                .anchor(Anchor::LEFT)
                .nudge(key, 0)
                .tone(tone);
            key += letters(text.chars().count() + 3) as i32;
        }
    }

    /// Sensor `index`'s limit, if its chip gives one on the thermometers'
    /// scale.
    fn limit(&self, index: usize) -> Option<f32> {
        self.machine.sensors[index]
            .limit
            .filter(|&limit| on_scale(limit))
    }

    /// Where fan `fan` is in the case, in words: `LEFT`, `TOP RIGHT`.
    fn position(&self, fan: &Fan) -> String {
        let side = if fan.mirrored { "RIGHT" } else { "LEFT" };

        match self.rotor {
            Rotor::Axial if fan.low => format!("BASE {side}"),
            Rotor::Axial => format!("TOP {side}"),
            Rotor::Blower => {
                // The one nearer the middle of two on a side is inner.
                let inner = self.fans.iter().any(|other| {
                    other.mirrored == fan.mirrored
                        && (other.centre.x - fan.centre.x) * if fan.mirrored { 1.0 } else { -1.0 }
                            > 0.0
                });

                if inner {
                    format!("{side}, INNER")
                } else {
                    side.into()
                }
            }
        }
    }

    /// The parts list's item `item`.
    fn part(&self, item: Item) -> Part {
        let machine = &self.machine;
        let sources: Vec<&Source> = self
            .sources
            .iter()
            .filter(|source| Item::Sources(source.kind) == item)
            .collect();
        let mut spec: Vec<(String, String)> = Vec::new();
        // The way the air takes a source's heat.
        let aired = |source: &Source| match self
            .plan
            .arrows
            .iter()
            .find(|arrow| std::ptr::eq(&self.sources[arrow.source], source))
            .and_then(|arrow| arrow.fan)
        {
            Some(fan) => format!("AIR TO {}", self.fans[fan].name),
            None => "AIR RISING".into(),
        };
        // ...and its fins'.
        let finned = |source: &Source| match self.rotor {
            Rotor::Blower => match self.fans.len() {
                0 => "BY HEAT PIPE TO FINS".to_owned(),
                n => format!("BY HEAT PIPE TO {}", counted(n, "FAN", "FANS")),
            },
            Rotor::Axial => format!("BY HEATSINK, {}", aired(source)),
        };

        let (name, quantity, value) = match item {
            Item::Sources(Kind::Processor) => {
                let cpu = machine.cpu.as_ref();

                if let Some(model) = cpu.and_then(|cpu| cpu.model.as_deref()) {
                    spec.extend(rows("MODEL", &lettered(model)));
                }

                if let [source] = sources.as_slice()
                    && let Some(&gauge) = source.gauges.first()
                {
                    spec.extend(rows("SENSOR", &called(&machine.sensors[gauge])));
                } else if let Some(&gauge) = sources.first().and_then(|s| s.gauges.first()) {
                    spec.push((
                        "SENSORS".into(),
                        format!(
                            "{} PACKAGES, {}",
                            sources.len(),
                            chip(&machine.sensors[gauge].chip)
                        ),
                    ));
                }

                // Every sensor but a package's is a core's or a die's, drawn
                // or not, as the monitor names them.
                let others: Vec<String> = sources
                    .iter()
                    .flat_map(|source| &source.sensors)
                    .map(|&index| {
                        machine.sensors[index]
                            .label
                            .as_deref()
                            .unwrap_or_default()
                            .to_lowercase()
                    })
                    .filter(|label| !package(label))
                    .collect();
                let what = if others.iter().all(|label| label.starts_with("core")) {
                    "CORES"
                } else if others.iter().all(|label| label.starts_with("tccd")) {
                    "DIES"
                } else {
                    "SENSORS"
                };

                if !others.is_empty() {
                    spec.push((what.into(), format!("{} MEASURED", others.len())));
                }

                if let Some(source) = sources.first() {
                    spec.extend(rows("COOLED", &finned(source)));
                }

                if let Some(cpu) = cpu {
                    let ghz = |mhz: u32| format!("{:.1}", mhz as f32 / 1000.0);

                    match (cpu.base_mhz, cpu.max_mhz) {
                        (Some(base), Some(max)) if base < max => spec
                            .push(("CLOCK".into(), format!("{} TO {} GHz", ghz(base), ghz(max)))),
                        (_, Some(max)) => spec.push(("CLOCK".into(), format!("{} GHz", ghz(max)))),
                        _ => {}
                    }
                }

                let value = cpu.map_or("PACKAGE".into(), |cpu| {
                    counted(cpu.cores as usize, "CORE", "CORES")
                });

                (
                    "CPU",
                    cpu.map_or(sources.len() as u32, |cpu| cpu.packages.max(1)),
                    value,
                )
            }
            Item::Sources(Kind::Battery) => {
                let batteries: Vec<_> = sources
                    .iter()
                    .filter_map(|source| machine.batteries.get(source.battery?))
                    .collect();

                for battery in &batteries {
                    if let Some(chemistry) = &battery.chemistry {
                        spec.push(("CHEMISTRY".into(), lettered(chemistry)));
                    }

                    if let Some(design) = battery.design_wh {
                        spec.push(("DESIGN".into(), format!("{design:.1} Wh")));
                    }

                    if let (Some(full), Some(health)) = (battery.full_wh, battery.health()) {
                        spec.push((
                            "FULL".into(),
                            format!("{full:.1} Wh, {:.0} %", health * 100.0),
                        ));
                    }

                    if let Some(cycles) = battery.cycles.filter(|&cycles| cycles > 0) {
                        spec.push(("CYCLES".into(), cycles.to_string()));
                    }
                }

                for charger in &machine.chargers {
                    let kind = match charger.kind {
                        ChargerKind::Mains => "MAINS",
                        ChargerKind::Usb => "USB",
                    };

                    spec.push((
                        "CHARGER".into(),
                        match charger.watts {
                            Some(watts) => format!("{kind}, {watts:.0} W"),
                            None => kind.into(),
                        },
                    ));
                }

                let value = match batteries.as_slice() {
                    [battery] => battery
                        .design_wh
                        .map_or("BATTERY".into(), |wh| format!("{wh:.0} Wh")),
                    _ => counted(batteries.len(), "CELL", "CELLS"),
                };

                ("BATTERY", batteries.len() as u32, value)
            }
            Item::Sources(Kind::Memory) => {
                let memory = machine.memory.as_ref();
                // The modules there are, not the thermometers drawn.
                let modules = memory.map_or(0, |memory| memory.modules.len());
                let installed = memory.and_then(|memory| memory.installed()).map(binary);
                let generation = memory
                    .and_then(|memory| memory.modules.first()?.generation.clone())
                    .unwrap_or("DIMM".into());

                spec.extend(installed.clone().map(|size| ("INSTALLED".into(), size)));
                spec.push(("MODULES".into(), format!("{modules} × {generation}")));

                if let Some(&gauge) = sources.first().and_then(|s| s.gauges.first()) {
                    spec.push((
                        "SENSORS".into(),
                        format!("{} ON EACH", chip(&machine.sensors[gauge].chip)),
                    ));
                }

                if let Some(source) = sources.first() {
                    spec.push(("COOLED".into(), format!("BY {}", aired(source))));
                }

                (
                    "MEMORY",
                    modules.max(1) as u32,
                    installed.unwrap_or(generation),
                )
            }
            Item::Sources(kind) => {
                let mut values = Vec::new();

                for source in &sources {
                    let device = source
                        .device
                        .and_then(|address| machine.pci_device(address));
                    let drive = source.drive.and_then(|drive| machine.drives.get(drive));

                    match (drive, device) {
                        (Some(drive), _) => {
                            let model = drive.model.as_deref().map(lettered).unwrap_or_default();

                            spec.extend(rows(&drive.name, &model));
                            values.push(decimal(drive.bytes));
                        }
                        // A device by its kind on a network adapter, else
                        // by what measures it, as the board is: its bus
                        // address tells a reader nothing here.
                        (None, Some(device)) => {
                            spec.extend(rows(&source.name, &named(device)));
                            values.push(match kind {
                                Kind::Network => source.name.clone(),
                                _ => counted(source.sensors.len(), "SENSOR", "SENSORS"),
                            });
                        }
                        // The board's monitors: an ACPI thermal zone, or
                        // its own chips.
                        (None, None) => values.push(
                            if source
                                .sensors
                                .iter()
                                .all(|&i| machine.sensors[i].chip == "acpitz")
                            {
                                "ACPI ZONE".into()
                            } else {
                                counted(source.sensors.len(), "SENSOR", "SENSORS")
                            },
                        ),
                    }

                    let sensors: Vec<String> = source
                        .sensors
                        .iter()
                        .map(|&index| {
                            let sensor = &machine.sensors[index];
                            match (&sensor.label, sensor.chip.as_str()) {
                                (Some(label), _) => lettered(label),
                                (None, "acpitz") => "ACPI THERMAL ZONE".into(),
                                (None, name) => chip(name),
                            }
                        })
                        .collect();

                    spec.extend(rows("SENSORS", &sensors.join(", ")));

                    if let Some(&fan) = source.fans.first() {
                        spec.push((
                            "FAN".into(),
                            format!("ITS OWN, {}", chip(&machine.sensors[fan].chip)),
                        ));
                    }

                    if !kind.finned() {
                        spec.push(("COOLED".into(), format!("BY {}", aired(source))));
                    }
                }

                if let Some(source) = sources.first().filter(|_| kind.finned()) {
                    spec.extend(rows("COOLED", &finned(source)));
                }

                let value = match values.as_slice() {
                    [one] => one.clone(),
                    _ if kind == Kind::Network => values.join(", ").replace("ETHERNET", "ETH"),
                    _ if kind == Kind::Drive => counted(values.len(), "DRIVE", "DRIVES"),
                    _ => counted(values.len(), "SOURCE", "SOURCES"),
                };

                (kind.part(), sources.len() as u32, value)
            }
            Item::Fans => {
                let mut chips: Vec<String> = self.fans.iter().map(|fan| chip(&fan.chip)).collect();

                chips.dedup();
                spec.extend(rows("MONITOR", &chips.join(", ")));

                for fan in &self.fans {
                    let mut about = Vec::new();

                    about.extend(fan.label.as_deref().map(lettered));
                    about.push(self.position(fan));

                    if !fan.turning {
                        about.push("PHANTOM".into());
                    }

                    spec.push((fan.name.into(), about.join(", ")));
                }

                spec.push(("DRAWN".into(), format!("SLOWED {SLOWED} TIMES")));

                let value = match self.rotor {
                    Rotor::Blower => "BLOWER",
                    Rotor::Axial => "AXIAL",
                };

                ("FAN", self.fans.len() as u32, value.into())
            }
        };

        // Its detail: the first of its blocks, or the first fan; no more
        // magnified than lets the window hold its gauges and values, or the
        // fan's rotor, on a display that draws the view large.
        let (centre, radius, holds) = match item {
            Item::Fans => (
                self.fans[0].centre,
                self.rotor.ring(),
                Extent::around(self.fans[0].centre, ROTOR),
            ),
            Item::Sources(_) => {
                let (centre, radius) = sources[0].ring();

                (centre, radius, sources[0].held())
            }
        };
        let mut part = Part::new(name, quantity, &fit(&value, 10))
            .detail(centre, radius)
            .holding(holds);

        part.spec = spec.into_iter().take(SPEC_ROWS).collect();
        part
    }
}

/// The three arrows' places across a vent from `a` to `b`.
fn exhausts(a: f32, b: f32) -> impl Iterator<Item = f32> {
    (1..=3).map(move |k| (a + (b - a) * k as f32 / 4.0).round())
}

/// The tone a temperature is filled in: live, or in caution when hot.
fn warmth(celsius: f32) -> Tone {
    if celsius >= HOT {
        Tone::Caution
    } else {
        Tone::Live
    }
}

/// `extent`'s outline, as a shape to fill inside.
fn rectangle(extent: Extent) -> Shape {
    Shape::Polyline {
        points: vec![
            extent.min,
            v(extent.max.x, extent.min.y),
            extent.max,
            v(extent.min.x, extent.max.y),
        ],
        closed: true,
    }
}

impl Subject for Cooling {
    fn name(&self) -> &'static str {
        "cooling"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        self.plan.extent
    }

    /// In a view smaller than the laptop's, the plan laid out in the
    /// columns that fit it best, so its lettering stays inside its blocks:
    /// where that keeps the same parts, notes and room for the
    /// specification.
    fn fitted(&self, room: Room) -> Option<Rc<dyn Subject>> {
        let budget = room.view.min(BUDGET);

        if budget == BUDGET {
            return None;
        }

        let mut fitted = self.fitted.borrow_mut();

        if let Some((_, cooling)) = fitted.iter().find(|(made, _)| *made == budget) {
            return cooling.clone().map(|cooling| cooling as Rc<dyn Subject>);
        }

        let cooling = Self::within(&self.machine, budget)
            .filter(|cooling| cooling.plan.extent != self.plan.extent)
            .filter(|cooling| alike(&self.card, &cooling.card))
            .map(Rc::new);

        // A view for each output the sheet is drawn on, and a few more for
        // one that changes size.
        if fitted.len() >= 8 {
            fitted.remove(0);
        }

        fitted.push((budget, cooling.clone()));
        cooling.map(|cooling| cooling as Rc<dyn Subject>)
    }

    /// Under a plan wide enough for it, the temperatures and a fan's speed
    /// over the last two minutes: where a wide display has room for it
    /// without drawing the plan much smaller. Under a tall plan it would.
    fn views(&self) -> Vec<View> {
        let case = self.plan.case;

        if !self.charted() {
            return Vec::new();
        }

        vec![View {
            name: "LAST 2 MINUTES".into(),
            place: Place::Under,
            extent: Extent::new(v(case.min.x, 0.0), v(case.max.x, CHART)),
        }]
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let snapshot = self.machine.sample(t);
        let seen = self.seen(&snapshot);
        let turns = self.turns(&snapshot, t);

        self.case(d);
        self.pipe(d);

        for arrow in &self.plan.arrows {
            d.arrow(arrow.from, arrow.to, Line::Thin);
        }

        for source in &self.sources {
            d.part(self.index(Item::Sources(source.kind)), |d| {
                self.block(d, source)
            });
        }

        for fan in &self.fans {
            set(d, &self.fan_snaps(fan), |d| {
                d.centre_mark(fan.centre, HUB, 3.0);
            });
        }

        d.moving(|d| {
            for source in &self.sources {
                d.part(self.index(Item::Sources(source.kind)), |d| {
                    self.readings_of(d, source, &snapshot)
                });
            }

            for (k, fan) in self.fans.iter().enumerate() {
                d.part(self.index(Item::Fans), |d| {
                    self.fan(d, k, turns[k] as f32, seen[k], rpm(&snapshot, fan.sensor));
                });
            }

            if !self.fans.is_empty() {
                self.flows(d, &snapshot, &turns, &seen, t);
            }

            if self.charted() {
                d.in_view(0, |d| self.traces(d, &snapshot, t, &seen));
            }
        });

        if self.charted() {
            d.in_view(0, |d| self.chart(d));
        }

        // A balloon for each item, on the first of its blocks or fans: a
        // block under the pipe at its top corner, by the room beside the
        // pipe; one the air cools at its foot on the side its heat goes,
        // by the room under its arrow.
        for &item in &self.items {
            let index = self.index(item);
            let target = match item {
                Item::Fans => self.fans[0].at(self.rotor.tip()),
                Item::Sources(kind) => {
                    let source = self
                        .sources
                        .iter()
                        .position(|source| source.kind == kind)
                        .expect("Every item has a source");
                    let frame = self.sources[source].frame;
                    let arrow = self.plan.arrows.iter().find(|a| a.source == source);

                    match arrow {
                        Some(arrow) if arrow.to.x < frame.min.x => frame.min,
                        Some(_) => v(frame.max.x, frame.min.y),
                        None => frame.max,
                    }
                }
            };

            d.part(index, |d| {
                d.balloon(index, target, Auto);
            });
        }
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let snapshot = self.machine.sample(t);
        let seen = self.seen(&snapshot);
        let mut readings = Vec::new();

        for (kind, name) in [(Kind::Processor, "CPU"), (Kind::Graphics, "GPU")] {
            if let Some(celsius) = self
                .sources
                .iter()
                .filter(|source| source.kind == kind)
                .filter_map(|source| source.hottest(&snapshot))
                .reduce(f32::max)
            {
                readings.push(Reading::new(name, format!("{celsius:.0} °C")));
            }
        }

        let turning: Vec<usize> = (0..self.fans.len()).filter(|&k| seen[k]).collect();

        if !self.fans.is_empty() && turning.is_empty() {
            readings.push(Reading::new("FANS", "STOPPED"));
        }

        for k in turning.into_iter().take(2) {
            readings.push(Reading::new(
                self.fans[k].name,
                format!("{:4.0} rpm", rpm(&snapshot, self.fans[k].sensor)),
            ));
        }

        if let Some(fraction) = snapshot
            .batteries
            .first()
            .and_then(|charge| charge.fraction)
            .filter(|fraction| fraction.is_finite())
        {
            readings.push(Reading::new(
                "BATTERY",
                format!("{:.0} %", fraction * 100.0),
            ));
        }

        readings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::Ink;
    use crate::draft::raster::{Piece, Projection, rasterize};
    use crate::machine::Fixture;

    #[test]
    fn the_fixture_is_documented_part_by_part() {
        let cooling = Cooling::new(&Machine::fixture()).unwrap();
        let card = cooling.card();
        let names: Vec<&str> = card.parts.iter().map(|p| p.name.as_str()).collect();
        let values: Vec<&str> = card.parts.iter().map(|p| p.material.as_str()).collect();
        let quantities: Vec<u32> = card.parts.iter().map(|p| p.quantity).collect();

        assert_eq!(card.title, "LAPTOP COOLING");
        assert_eq!(names, ["CPU", "MEMORY", "DRIVE", "BOARD", "BATTERY", "FAN"]);
        assert_eq!(
            values,
            ["16 CORES", "32 GiB", "1 TB", "ACPI ZONE", "60 Wh", "BLOWER"]
        );
        assert_eq!(quantities, [1, 2, 1, 1, 1, 2]);
    }

    /// The specification's rows fit the laptop's column, name and value
    /// apart, and there are no more of them than the column has room for.
    #[test]
    fn the_specifications_fit_their_column() {
        use super::super::SPEC_ROOM;

        let cooling = Cooling::new(&Machine::fixture()).unwrap();

        for part in &cooling.card().parts {
            assert!(part.spec.len() <= SPEC_ROWS, "{}", part.name);

            for (name, value) in &part.spec {
                assert!(
                    name.chars().count() + value.chars().count() + 2 <= SPEC_ROOM,
                    "{}: {name} {value}",
                    part.name
                );
            }
        }
    }

    /// The plan fits the laptop's view, so the laptop letters it at its
    /// own size or larger; the fans are in the corners, their outlets in,
    /// and the heat pipe runs from fins to fins over the processor.
    #[test]
    fn the_fixture_fits_the_laptop_with_a_fan_in_each_corner() {
        let cooling = Cooling::new(&Machine::fixture()).unwrap();
        let extent = cooling.extent();

        assert!(extent.width() <= BUDGET.x && extent.height() <= BUDGET.y);

        let [left, right] = &cooling.fans[..] else {
            panic!("two fans");
        };
        let cpu = &cooling.sources[0];

        assert!(!left.mirrored && right.mirrored);
        assert!(left.centre.x < cpu.frame.min.x && cpu.frame.max.x < right.centre.x);

        let (from, to) = cooling.plan.pipe.unwrap();
        assert!(from < left.vent(cooling.rotor).1 && to > right.vent(cooling.rotor).0);
        assert_eq!(cooling.plan.drops, [cpu.frame.centre().x]);

        // Every block apart, inside the case.
        for (i, a) in cooling.sources.iter().enumerate() {
            let case = cooling.plan.case;

            assert!(a.frame.min.x > case.min.x && a.frame.max.x < case.max.x);
            assert!(a.frame.min.y > case.min.y && a.frame.max.y < case.max.y);

            for b in &cooling.sources[i + 1..] {
                let apart = a.frame.max.x < b.frame.min.x
                    || b.frame.max.x < a.frame.min.x
                    || a.frame.max.y < b.frame.min.y
                    || b.frame.max.y < a.frame.min.y;
                assert!(apart, "{} and {} overlap", a.name, b.name);
            }
        }
    }

    /// Each rotor turns at its fan's speed, slowed: carried on from the
    /// frame before, or from nothing when there was none just before.
    #[test]
    fn rotors_turn_at_their_speed_slowed() {
        let machine = Machine::fixture();
        let cooling = Cooling::new(&machine).unwrap();
        let speed = |t: f32| {
            let rpm = rpm(&machine.sample(t), cooling.fans[0].sensor);
            f64::from(rpm / SLOWED) / 60.0 * std::f64::consts::TAU
        };

        let first = cooling.turns(&machine.sample(20.0), 20.0)[0];
        assert!((first - speed(20.0) * 20.0).abs() < 1e-6);

        let next = cooling.turns(&machine.sample(20.1), 20.1)[0];
        assert!((next - first - speed(20.1) * 0.1).abs() < 1e-4);

        // A moment far from the last is drawn afresh.
        let later = cooling.turns(&machine.sample(40.0), 40.0)[0];
        assert!((later - speed(40.0) * 40.0).abs() < 1e-6);
    }

    /// A fan the monitor lists that has never read a speed is drawn in
    /// phantom, and drawn in outline once it has.
    #[test]
    fn a_fan_not_seen_turning_is_a_phantom() {
        let mut machine = Machine::fixture();
        let mut ghost = machine.fans().last().unwrap().1.clone();

        ghost.label = None;
        machine.sensors.push(ghost);

        let cooling = Cooling::new(&machine).unwrap();
        assert_eq!(cooling.fans.len(), 3);

        let mut draft = Draft::new();
        cooling.draw(&mut draft, 3.0);

        let fans = cooling.index(Item::Fans);
        let housings: Vec<Line> = draft
            .marks()
            .iter()
            .filter(|mark| mark.part == Some(fans))
            .filter_map(|mark| match &mark.ink {
                Ink::Stroke {
                    shape: Shape::Polyline { points, .. },
                    line,
                } if points.len() > 40 => Some(*line),
                _ => None,
            })
            .collect();

        assert_eq!(housings, [Line::Outline, Line::Outline, Line::Phantom]);
        assert!(!cooling.readings(3.0).iter().any(|r| r.name == "FAN 3"));
    }

    /// A monitor that lists its empty headers before the fans on it: the
    /// fans turning are drawn, and the empty headers are the ones left out.
    #[test]
    fn the_fans_turning_are_drawn_before_empty_headers() {
        let fake = crate::machine::tests::laptop();
        let ec = "sys/class/hwmon/hwmon5";

        for (fan, rpm) in [(1, 0), (2, 0), (3, 0), (4, 0), (5, 1200), (6, 900)] {
            fake.file(&format!("{ec}/fan{fan}_input"), format!("{rpm}\n"));
        }

        let machine = fake.read();
        let cooling = Cooling::new(&machine).unwrap();
        let labels: Vec<Option<&str>> = cooling
            .fans
            .iter()
            .map(|fan| machine.sensors[fan.sensor].label.as_deref())
            .collect();

        assert_eq!(labels, [None, None, Some("CPU Fan"), Some("System Fan")]);
        assert_eq!(rpm(&machine.sample(0.0), cooling.fans[0].sensor), 1200.0);
        assert!(
            cooling
                .card()
                .notes
                .contains(&"2 FANS MORE NOT SHOWN".into())
        );

        let readings: Vec<String> = cooling
            .readings(1.0)
            .iter()
            .map(|reading| format!("{} {}", reading.name, reading.value))
            .collect();

        // Named by the monitor's numbers for them, as `sensors` names them.
        assert!(readings.contains(&"FAN 5 1200 rpm".into()), "{readings:?}");
    }

    /// Counts in the parts list are of what the machine has, not of what
    /// the sheet has room to draw.
    #[test]
    fn counts_are_of_what_is_measured() {
        let mut machine = Machine::fixture();
        let package = machine.sensors[1].clone();
        let module = machine.sensors[3].clone();

        for core in 0..60 {
            machine.sensors.push(Sensor {
                label: Some(format!("Core {core}")),
                ..package.clone()
            });
        }

        for n in 2..8 {
            machine.sensors.push(Sensor {
                site: Site::Module(n),
                ..module.clone()
            });
        }

        let memory = machine.memory.as_mut().unwrap();
        let first = memory.modules[0].clone();
        memory.modules.resize(8, first);

        let cooling = Cooling::new(&machine).unwrap();
        let card = cooling.card();
        let spec = |part: &str, row: &str| {
            card.parts
                .iter()
                .find(|p| p.name == part)
                .and_then(|p| p.spec.iter().find(|(name, _)| name == row))
                .map(|(_, value)| value.clone())
        };

        assert_eq!(cooling.sources[0].cores.len(), MOST_CORES);
        assert_eq!(spec("CPU", "CORES").as_deref(), Some("60 MEASURED"));
        assert_eq!(spec("MEMORY", "MODULES").as_deref(), Some("8 × DDR5"));
        assert_eq!(
            card.parts
                .iter()
                .find(|p| p.name == "MEMORY")
                .unwrap()
                .quantity,
            8
        );
    }

    /// Four fans, two a side, leave room for one column between them: what
    /// the air cools gives its heat to each side in turn, to the fan there
    /// that turns, and it all fits the laptop's view. The fans are dealt to
    /// the sides in turn, so the two turning take a corner each.
    #[test]
    fn four_fans_take_the_heat_from_one_column_each_side_in_turn() {
        let mut machine = Machine::fixture();
        let fan = machine.fans().last().unwrap().1.clone();

        machine.sensors.extend([fan.clone(), fan]);

        let cooling = Cooling::new(&machine).unwrap();
        let extent = cooling.extent();
        let sides: Vec<bool> = cooling.fans.iter().map(|fan| fan.mirrored).collect();

        assert_eq!(sides, [false, true, false, true]);
        assert!(extent.width() <= BUDGET.x && extent.height() <= BUDGET.y);

        let leftward: Vec<bool> = cooling
            .plan
            .arrows
            .iter()
            .map(|arrow| arrow.to.x < arrow.from.x)
            .collect();

        assert_eq!(leftward, [true, false, true, false]);
        assert_eq!(
            cooling
                .plan
                .arrows
                .iter()
                .map(|a| a.fan)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(0), Some(1)]
        );
    }

    /// A desktop's fans are axial, in square frames under their fins.
    #[test]
    fn a_desktops_fans_are_axial() {
        let mut machine = Machine::fixture();

        machine.chassis.kind = ChassisKind::Desktop;

        let cooling = Cooling::new(&machine).unwrap();
        let fan = &cooling.fans[0];
        let (a, b) = fan.vent(cooling.rotor);
        let extent = cooling.extent();

        assert_eq!(cooling.rotor, Rotor::Axial);
        assert_eq!(cooling.card().title, "DESKTOP COOLING");
        assert_eq!((b - a, fan.centre.y), (2.0 * FRAME, -(INSET + FRAME)));
        assert!(extent.width() <= BUDGET.x && extent.height() <= BUDGET.y);
    }

    /// With no fan measured the paths are drawn and nothing runs along
    /// them; with no temperature measured there is no sheet.
    #[test]
    fn without_fans_nothing_moves_and_without_temperatures_there_is_no_sheet() {
        let mut machine = Machine::fixture();

        machine
            .sensors
            .retain(|sensor| sensor.kind != SensorKind::Fan);

        let cooling = Cooling::new(&machine).unwrap();
        let mut draft = Draft::new();
        cooling.draw(&mut draft, 12.0);

        assert!(cooling.plan.sink.is_some() && cooling.plan.pipe.is_some());
        assert!(
            !draft
                .marks()
                .iter()
                .any(|mark| mark.view.is_none() && matches!(mark.ink, Ink::Dot { .. }))
        );
        assert!(!cooling.card().parts.iter().any(|part| part.name == "FAN"));

        machine
            .sensors
            .retain(|sensor| sensor.kind != SensorKind::Temperature);
        assert!(Cooling::new(&machine).is_none());
    }

    /// A processor's cores are drawn as squares beside its package's
    /// thermometer, and the modules each have one.
    #[test]
    fn the_package_has_a_thermometer_and_the_cores_squares() {
        let mut machine = Machine::fixture();
        let package = machine.sensors[1].clone();

        for core in 0..8 {
            machine.sensors.push(Sensor {
                label: Some(format!("Core {core}")),
                ..package.clone()
            });
        }

        let sources = sources(&machine);
        let cpu = &sources[0];

        assert_eq!(cpu.kind, Kind::Processor);
        assert_eq!(cpu.gauges, [1]);
        assert_eq!(cpu.cores.len(), 8);
        assert_eq!(sources[1].gauges, [3, 4]);

        let cooling = Cooling::new(&machine).unwrap();
        let mut draft = Draft::new();
        cooling.draw(&mut draft, 3.0);
    }

    /// The pixels of what `mark` draws under `projection`: its strokes'
    /// and areas', and the box of its lettering's capitals.
    fn inked(mark: &crate::draft::Mark, projection: &Projection) -> Vec<(i32, i32)> {
        let mut pieces = Vec::new();
        let mut pixels = Vec::new();

        rasterize(mark, projection, &mut pieces);

        for inked in pieces {
            match inked.piece {
                Piece::Path { pixels: path, .. } => {
                    pixels.extend(path.iter().map(|p| (p.x, p.y)));
                }
                Piece::Rows { rows, .. } => {
                    for (y, from, to) in rows {
                        pixels.extend((from..=to).map(|x| (x, y)));
                    }
                }
                Piece::Block(block) => {
                    for y in block.y..block.y + block.height {
                        pixels.extend((block.x..block.x + block.width).map(|x| (x, y)));
                    }
                }
                // A glyph's ink starts a column into its cell and fills the
                // rest of it, from the capitals' top to the baseline.
                Piece::Text { at, text } => {
                    let width = i32::from(LETTERING.width(&text));
                    let top = at.y + i32::from(LETTERING.cap_top());
                    let bottom = at.y + i32::from(LETTERING.baseline()) - 1;

                    pixels.extend([(at.x + 1, top), (at.x + width - 1, bottom)]);
                }
                Piece::Knockout(_) => {}
            }
        }

        pixels
    }

    const SCALES: [f32; 8] = [0.92, 1.0, 1.07, 1.18, 1.25, 1.37, 1.64, 2.36];

    /// Every block of every machine is padded alike on its four sides (its
    /// name and value as far from its top and sides as its last gauge is
    /// from its bottom), blocks as tall in units are as tall in pixels, and
    /// each thermometer is one closed outline the same either side of its
    /// centre line, its column inside it, a pixel or more clear of the
    /// next: at every scale the sheet is drawn at, wherever the block falls
    /// on the grid.
    #[test]
    fn every_block_is_padded_alike_and_its_thermometers_are_whole() {
        for fixture in Fixture::ALL {
            let machine = fixture.machine();
            let Some(cooling) = Cooling::new(&machine) else {
                continue;
            };
            let snapshot = machine.sample(12.0);

            for scale in SCALES {
                for origin in [(13.0, 517.0), (240.0, 61.0)] {
                    let projection = Projection::new(origin, scale);
                    let mut heights: Vec<(f32, i32)> = Vec::new();

                    for source in &cooling.sources {
                        let at = format!("{fixture:?} {} at {scale}", source.name);
                        let mut d = Draft::new();

                        cooling.block(&mut d, source);
                        cooling.readings_of(&mut d, source, &snapshot);

                        let marks: Vec<&crate::draft::Mark> =
                            d.marks().iter().filter(|m| m.shown_in(None)).collect();
                        let outline = inked(marks[0], &projection);
                        let (left, right) = (
                            outline.iter().map(|p| p.0).min().unwrap(),
                            outline.iter().map(|p| p.0).max().unwrap(),
                        );
                        let (top, bottom) = (
                            outline.iter().map(|p| p.1).min().unwrap(),
                            outline.iter().map(|p| p.1).max().unwrap(),
                        );
                        let inside: Vec<(i32, i32)> = marks[1..]
                            .iter()
                            .flat_map(|mark| inked(mark, &projection))
                            .filter(|&(x, y)| x > left && x < right && y > top && y < bottom)
                            .collect();
                        let pads = [
                            inside.iter().map(|p| p.0).min().unwrap() - left - 1,
                            right - inside.iter().map(|p| p.0).max().unwrap() - 1,
                            inside.iter().map(|p| p.1).min().unwrap() - top - 1,
                            bottom - inside.iter().map(|p| p.1).max().unwrap() - 1,
                        ];

                        assert!(pads.iter().all(|&pad| pad == pads[0]), "{at}: {pads:?}");
                        heights.push((source.frame.height(), bottom - top));

                        // Its thermometers.
                        let glass: Vec<Vec<(i32, i32)>> = marks
                            .iter()
                            .filter(|mark| {
                                matches!(
                                    mark.ink,
                                    Ink::Stroke {
                                        shape: Shape::Keyhole { .. },
                                        ..
                                    }
                                )
                            })
                            .map(|mark| inked(mark, &projection))
                            .collect();
                        let columns: Vec<(i32, i32)> = marks
                            .iter()
                            .filter(|mark| {
                                matches!(
                                    mark.ink,
                                    Ink::Inside {
                                        shape: Shape::Keyhole { .. },
                                        ..
                                    }
                                )
                            })
                            .flat_map(|mark| inked(mark, &projection))
                            .collect();

                        for outline in &glass {
                            let closed = outline
                                .iter()
                                .zip(outline.iter().cycle().skip(1))
                                .all(|(a, b)| (a.0 - b.0).abs() <= 1 && (a.1 - b.1).abs() <= 1);
                            let (high, low) = (
                                outline.iter().map(|p| p.1).min().unwrap(),
                                outline.iter().map(|p| p.1).max().unwrap(),
                            );
                            let mut mirrored: Vec<(i32, i32)> =
                                outline.iter().map(|&(x, y)| (x, high + low - y)).collect();
                            let mut sorted = outline.clone();

                            mirrored.sort();
                            sorted.sort();
                            assert!(closed, "{at}: a thermometer is open");
                            assert_eq!(sorted, mirrored, "{at}: a thermometer is crooked");
                        }

                        for pair in glass.windows(2) {
                            let above = pair[0].iter().map(|p| p.1).max().unwrap();
                            let below = pair[1].iter().map(|p| p.1).min().unwrap();

                            assert!(below - above >= 2, "{at}: bulbs touch");
                        }

                        let glass: Vec<&(i32, i32)> = glass.iter().flatten().collect();
                        assert!(
                            !columns.iter().any(|pixel| glass.contains(&pixel)),
                            "{at}: a column is on its glass"
                        );
                    }

                    for a in &heights {
                        for b in heights.iter().filter(|b| b.0 == a.0) {
                            assert_eq!(a.1, b.1, "{fixture:?} at {scale}: blocks differ");
                        }
                    }
                }
            }
        }
    }

    /// The heat pipe's drops meet it and the block each drops to, and the
    /// fins it runs through meet it, whatever the scale.
    #[test]
    fn the_heat_pipe_meets_what_it_runs_into() {
        let cooling = Cooling::new(&Machine::fixture()).unwrap();
        let cpu = &cooling.sources[0];

        for scale in SCALES {
            let projection = Projection::new((31.0, 402.0), scale);
            let mut pipe = Draft::new();
            let mut block = Draft::new();

            cooling.pipe(&mut pipe);
            cooling.block(&mut block, cpu);

            let lines: Vec<Vec<(i32, i32)>> = pipe
                .marks()
                .iter()
                .filter(|mark| mark.pass() == crate::draft::Pass::Edges)
                .map(|mark| inked(mark, &projection))
                .collect();
            let top = inked(&block.marks()[0], &projection)
                .iter()
                .map(|p| p.1)
                .min()
                .unwrap();
            // The upper line, then the lower's stretches, the ends, and the
            // drop's sides.
            let upper = lines[0][0].1;
            let lower = lines[1][0].1;
            let (axis_up, axis_down) = (upper, lower);

            for side in &lines[lines.len() - 2..] {
                let rows: Vec<i32> = side.iter().map(|p| p.1).collect();

                assert_eq!(rows.iter().min(), Some(&lower), "at {scale}");
                assert_eq!(rows.iter().max(), Some(&top), "at {scale}");
            }

            // The fins either side of the pipe stop on its lines.
            let mut fins = Draft::new();
            let vent = cooling.fans[0].vent(cooling.rotor);

            cooling.fins(&mut fins, vent, Line::Outline, Line::Thin);

            for mark in fins
                .marks()
                .iter()
                .filter(|m| m.pass() < crate::draft::Pass::Edges)
            {
                let rows: Vec<i32> = inked(mark, &projection).iter().map(|p| p.1).collect();
                let (min, max) = (*rows.iter().min().unwrap(), *rows.iter().max().unwrap());

                assert!(
                    max == axis_up || min == axis_down,
                    "at {scale}: {min}..{max}"
                );
            }
        }
    }

    /// A graphics card's own fan is lettered in its block, not drawn as one
    /// of the case's; a board's headers reading 0 are left out and counted;
    /// each processor package and each drive is a block of its own; and a
    /// tower's four fans and seven blocks fit the laptop's view.
    #[test]
    fn a_desktop_is_drawn_as_a_tower() {
        let machine = Fixture::Desktop.machine();
        let cooling = Cooling::new(&machine).unwrap();
        let gpu = cooling
            .sources
            .iter()
            .find(|source| source.kind == Kind::Graphics)
            .unwrap();
        let names: Vec<&str> = cooling.fans.iter().map(|fan| fan.name).collect();
        let blocks: Vec<&str> = cooling.sources.iter().map(|s| s.name.as_str()).collect();

        assert_eq!(gpu.fans.len(), 1);
        assert!(
            !cooling
                .fans
                .iter()
                .any(|fan| gpu.fans.contains(&fan.sensor))
        );
        assert_eq!(names, ["FAN 2", "FAN 4", "FAN 5", "FAN 7"]);
        assert!(cooling.fans.iter().all(|fan| fan.turning));
        assert!(
            cooling
                .card()
                .notes
                .contains(&"3 FAN HEADERS AT 0 rpm OMITTED".into())
        );
        assert!(
            !cooling
                .card()
                .notes
                .iter()
                .any(|n| n.starts_with("PHANTOM"))
        );
        assert_eq!(
            blocks,
            ["CPU", "GPU", "MEMORY", "NVMe", "SSD", "HDD", "BOARD"]
        );
        assert!(cooling.plan.pipe.is_none());
        // Its processor's other sensor is a die's, not a core's.
        assert!(
            cooling.card().parts[0]
                .spec
                .contains(&("DIES".into(), "1 MEASURED".into()))
        );

        let extent = cooling.extent();
        assert!(extent.width() <= BUDGET.x && extent.height() <= BUDGET.y);

        // Two fans at the top exhausting, two at the base taking air in.
        let low: Vec<bool> = cooling.fans.iter().map(|fan| fan.low).collect();
        assert_eq!(low, [false, false, true, true]);

        let server = Cooling::new(&Fixture::Server.machine()).unwrap();
        let cpus: Vec<&str> = server
            .sources
            .iter()
            .filter(|s| s.kind == Kind::Processor)
            .map(|s| s.name.as_str())
            .collect();
        let spec = &server.card().parts[0].spec;

        assert_eq!(cpus, ["CPU 1", "CPU 2"]);
        assert!(
            spec.contains(&("CORES".into(), "64 MEASURED".into())),
            "{spec:?}"
        );
    }

    /// No heat arrow runs through a fan's housing: one at the height of a
    /// housing stops at its side.
    #[test]
    fn heat_arrows_stop_short_of_the_housings() {
        for fixture in Fixture::ALL {
            let Some(cooling) = Cooling::new(&fixture.machine()) else {
                continue;
            };

            for arrow in &cooling.plan.arrows {
                for fan in &cooling.fans {
                    let (a, b) = fan.span(cooling.rotor);
                    let (low, high) = fan.height(cooling.rotor);
                    let (from, to) = (arrow.from.x.min(arrow.to.x), arrow.from.x.max(arrow.to.x));

                    assert!(
                        !((low..=high).contains(&arrow.from.y) && from < b && to > a),
                        "{fixture:?}: {arrow:?} crosses {}",
                        fan.name
                    );
                }
            }
        }
    }

    /// Under the plan, the chart runs over the last two minutes: the
    /// processor's temperature, the next source's and a fan's speed, from
    /// the left of its plot to now at its right.
    #[test]
    fn the_chart_runs_over_the_last_two_minutes() {
        let machine = Machine::fixture();
        let cooling = Cooling::new(&machine).unwrap();
        let history = machine.history(40.5);
        let mut draft = Draft::new();

        assert_eq!(history.len(), HISTORY);
        assert_eq!(*history[HISTORY - 1], *machine.sample(40.0));
        assert_eq!(*history[0], *machine.sample(40.0 - (HISTORY - 1) as f32));

        cooling.draw(&mut draft, 40.5);

        let plot = cooling.plot();
        let traces: Vec<(f32, f32, Tone)> = draft
            .marks()
            .iter()
            .filter(|mark| mark.view == Some(0))
            .filter_map(|mark| match &mark.ink {
                Ink::Stroke {
                    shape: Shape::Polyline { points, .. },
                    line: Line::Trace,
                } => Some((points[0].x, points[points.len() - 1].x, mark.tone)),
                _ => None,
            })
            .collect();
        let tones: Vec<Tone> = traces.iter().map(|trace| trace.2).collect();

        assert_eq!(tones, [Tone::Accent, Tone::Ink, Tone::Live]);
        assert!(
            traces
                .iter()
                .all(|&(from, to, _)| from == plot.min.x && to == plot.max.x)
        );
        assert_eq!(cooling.views()[0].extent.width(), cooling.plan.case.width());
    }

    #[test]
    fn readings_are_the_live_values() {
        let cooling = Cooling::new(&Machine::fixture()).unwrap();
        let readings = cooling.readings(3.0);
        let names: Vec<&str> = readings.iter().map(|r| r.name).collect();

        assert_eq!(names, ["CPU", "FAN 1", "FAN 2", "BATTERY"]);
        assert!(readings[1].value.ends_with(" rpm"));
    }
}
