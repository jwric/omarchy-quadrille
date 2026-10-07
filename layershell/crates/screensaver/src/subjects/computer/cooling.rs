//! The machine's cooling as a schematic plan: the heat sources with their
//! temperatures, the heat pipe from the processor (and the graphics) to the
//! fins in the fans' outlets, the fans as rotors turning at the speeds their
//! monitors measure, and the air drawn in at the base, past what it cools
//! and out through the vents.
//!
//! The arrangement is a diagram, not the machine's: the inventory says what
//! is measured, not where it is. It is laid out like a laptop's base seen
//! from above with its vents at the back, the top of the sheet: the fans in
//! the corners with the processor between them under the heat pipe, and in
//! rows under it what the air cools, each giving its heat to the air drawn
//! up the side to the nearest fan. A desktop's fans are axial, a laptop's
//! blowers.
//!
//! A fan the monitor lists but that has not been seen turning is drawn in
//! phantom: a controller can list more fans than the machine has, and a fan
//! that has stopped reads 0 like one that is not there. With no fan
//! measured at all, the paths are drawn and nothing moves on them.
//!
//! The units are the laptop's virtual pixels at full size, as the topology
//! sheet's are, so lettering fits its blocks at the laptop's scale or
//! larger.
use std::cell::RefCell;
use std::f32::consts::{PI, TAU};

use quadrille::draw::Anchor;

use crate::draft::Placement::Auto;
use crate::draft::raster::LETTERING;
use crate::draft::{Draft, Extent, Fill, Line, Tone, V2, arc_points, polar, v};
use crate::machine::{
    ChargeState, ChargerKind, ChassisKind, Link, Machine, PciAddress, PciKind, Sensor, SensorKind,
    Site, Snapshot,
};

use super::super::{Card, Domain, Part, Reading, Revision, Subject, Unit};
use super::layout::{BUDGET, named, short};
use super::{SPEC_ROWS, binary, counted, decimal, fit, flow, lettered, rows};

/// A heat source's block: wide enough for its name and a temperature...
const BLOCK: f32 = 108.0;
/// ...lettering this far in from its sides...
const PAD: f32 = 6.0;
/// ...from its top to the middle of its title, and to its first gauge...
const TITLE: f32 = 10.0;
const GAUGES: f32 = 22.0;
/// ...between gauges, and under the last.
const GAUGE: f32 = 7.0;
const FOOT: f32 = 8.0;

/// A thermometer's bulb, from the block's side, and its radius; its tube's
/// length and half its bore, and the scale along it in °C.
const BULB: f32 = 9.0;
const BULB_RADIUS: f32 = 3.0;
const TUBE: f32 = 60.0;
const GLASS: f32 = 2.0;
const COLDEST: f32 = 20.0;
const HOTTEST: f32 = 100.0;
/// A temperature read in caution from here.
const HOT: f32 = 85.0;
/// The thermometers a block has at most: the rest of its sensors are in its
/// specification.
const MOST_GAUGES: usize = 4;
/// The processor's other sensors (its cores) as squares tinted by their
/// temperatures, this many to a row, and how many at most.
const CORE: f32 = 5.0;
const CORE_PITCH: f32 = 7.0;
const CORES_ACROSS: usize = 12;
const MOST_CORES: usize = 48;
/// A battery's gauge: its body's length.
const CELL: f32 = 44.0;
/// In a detail, a block's name over its first gauge, and the room after
/// a tube for its temperature.
const DETAIL_TITLE: f32 = 7.0;
const DETAIL_VALUE: f32 = 20.0;
/// Between a detail's circle and the bulbs and name it encloses.
const CLEAR: f32 = 5.0;

/// Between blocks across and down.
const ACROSS: f32 = 14.0;
const DOWN: f32 = 14.0;
/// Between the blocks and the fans, room for the heat's arrows.
const SIDE: f32 = 30.0;
/// Between the case and the fans, and between fans side by side.
const INSET: f32 = 8.0;
const BETWEEN: f32 = 10.0;
/// The case's corners, cut off; the room under the fans for the air drawn
/// up to them; the intakes' half width.
const CHAMFER: f32 = 6.0;
const AIRWAY: f32 = 40.0;
const INTAKE: f32 = 12.0;
/// From the top of the case: the exhaust's arrows, and the fans' speeds
/// over them under their names, a line of lettering (at the laptop's
/// scale) apart.
const EXHAUST: f32 = 14.0;
const NAMES: f32 = 24.0;
const LINE: f32 = 12.0;

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
/// in a second; the dots' spacing.
const AIR: f64 = 12.0;
const HEAT: f64 = 18.0;
const SPACING: f64 = 9.0;
/// The temperatures over which a source's heat goes from a few dots to all
/// of them.
const WARM: f32 = 30.0;
const SCALDING: f32 = 90.0;

/// The fans drawn, at most, and their names in the readings.
const MOST_FANS: usize = 4;
const FAN_NAMES: [&str; MOST_FANS] = ["FAN 1", "FAN 2", "FAN 3", "FAN 4"];

/// Room round the drawing for the balloons to line up in.
const MARGIN: f32 = 22.0;

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

    /// Whether the heat pipe cools it; the air cools the rest.
    fn piped(self) -> bool {
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
    /// Which of the machine's batteries it is, for a battery.
    battery: Option<usize>,
    /// Every temperature sensor on it, by index in a snapshot...
    sensors: Vec<usize>,
    /// ...those it has a thermometer for...
    gauges: Vec<usize>,
    /// ...and the processor's others, its cores, drawn as squares.
    cores: Vec<usize>,
    frame: Extent,
}

impl Source {
    /// Its block's height: its title, its gauges and its cores' rows.
    fn height(&self) -> f32 {
        let gauges = self.gauges.len().max(1);
        let below = match self.cores.len().div_ceil(CORES_ACROSS) {
            0 => FOOT,
            rows => 6.0 + (rows - 1) as f32 * CORE_PITCH + CORE + 5.0,
        };

        GAUGES + (gauges - 1) as f32 * GAUGE + below
    }

    /// The middle of the bulb of its thermometer `k`, or of a battery's
    /// gauge.
    fn gauge(&self, k: usize) -> V2 {
        v(
            self.frame.min.x + BULB,
            self.frame.max.y - GAUGES - k as f32 * GAUGE,
        )
    }

    /// The circle a detail of it magnifies: its gauges and the lettering a
    /// detail adds round them, as a detail at twice the view's scale or
    /// more has room for, with room to spare left of its bulbs, where its
    /// name starts.
    fn ring(&self) -> (V2, f32) {
        let first = self.gauge(0);
        let last = match self.cores.len() {
            0 => self.gauge(self.gauges.len().max(1) - 1).y - BULB_RADIUS,
            n => self.core(n - 1).min.y,
        };
        let top = first.y + DETAIL_TITLE + 3.0;
        let (left, right) = (
            first.x - BULB_RADIUS - CLEAR,
            first.x + BULB_RADIUS + TUBE + DETAIL_VALUE,
        );

        (
            v(((left + right) / 2.0).round(), ((top + last) / 2.0).round()),
            ((right - left) / 2.0).round(),
        )
    }

    /// The square of its core `k`.
    fn core(&self, k: usize) -> Extent {
        let last = self.gauge(self.gauges.len().max(1) - 1);
        let (row, column) = (k / CORES_ACROSS, k % CORES_ACROSS);
        let left = self.frame.min.x + PAD + column as f32 * CORE_PITCH;
        let top = last.y - 6.0 - row as f32 * CORE_PITCH;

        Extent::new(v(left, top - CORE), v(left + CORE, top))
    }

    /// Its hottest temperature now.
    fn hottest(&self, snapshot: &Snapshot) -> Option<f32> {
        self.sensors
            .iter()
            .filter_map(|&index| celsius(snapshot, index))
            .reduce(f32::max)
    }
}

/// A fan, as the sheet draws it.
#[derive(Debug, Clone)]
struct Fan {
    /// Its speed's sensor, by index in a snapshot.
    sensor: usize,
    /// What its monitor calls it, and the monitor.
    label: Option<String>,
    chip: String,
    /// Its axis, and whether it is drawn mirrored: its outlet on its left.
    centre: V2,
    mirrored: bool,
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
}

/// What kind of fans a machine has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rotor {
    /// A centrifugal blower in its scroll, as a laptop has.
    Blower,
    /// An axial fan in a square frame, as a desktop has.
    Axial,
}

impl Rotor {
    /// How far its axis is under the top of the case: under its fins and,
    /// a blower's, its outlet.
    fn depth(self) -> f32 {
        match self {
            Self::Blower => (FINS + OUTLET + SCROLL_IN * TONGUE.sin()).round(),
            Self::Axial => FINS + 2.0 + FRAME,
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

    /// Where its balloon points: its housing's outer side, high, toward
    /// the corner of the sheet.
    fn tip(self) -> V2 {
        match self {
            Self::Blower => polar(scroll(0.8 * PI), 0.8 * PI),
            Self::Axial => v(-FRAME, FRAME),
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

/// Where the case and the paths are, in the laptop's virtual pixels, `y`
/// up and the top of the case at 0.
#[derive(Debug, Clone)]
struct Plan {
    case: Extent,
    /// The gaps in the case's top over the fins, and the middles of those
    /// in its base where the air comes in.
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
fn plan(sources: &mut [Source], fans: &mut [Fan], rotor: Rotor, columns: usize) -> Plan {
    let reach = rotor.reach();
    let depth = rotor.depth();
    let left = fans.len() / 2;

    // The fans on the left, their outlets on their right, toward the
    // middle; those on the right mirrored.
    let mut x = INSET;

    for fan in &mut fans[..left] {
        fan.mirrored = false;
        fan.centre = v(x - reach.min.x, -depth);
        x = fan.centre.x + reach.max.x + BETWEEN;
    }

    let middle = if left > 0 {
        x - BETWEEN + SIDE
    } else {
        INSET + SIDE / 2.0
    };
    let (piped, aired): (Vec<usize>, Vec<usize>) =
        (0..sources.len()).partition(|&index| sources[index].kind.piped());
    let width = |n: usize| n as f32 * BLOCK + n.saturating_sub(1) as f32 * ACROSS;
    let columns = columns.min(aired.len()).max(1);
    let span = width(piped.len()).max(width(columns));

    // What the heat pipe cools, side by side under it...
    let mut x = middle + (span - width(piped.len())) / 2.0;
    let mut bottom = -COOLED;

    for &index in &piped {
        let height = sources[index].height();

        sources[index].frame = Extent::new(v(x, -COOLED - height), v(x + BLOCK, -COOLED));
        bottom = bottom.min(-COOLED - height);
        x += BLOCK + ACROSS;
    }

    // ...and what the air cools, in rows under that.
    let mut top = if piped.is_empty() {
        -COOLED
    } else {
        bottom - DOWN
    };
    let first = middle + (span - width(columns)) / 2.0;

    for row in aired.chunks(columns) {
        let mut lowest = top;

        for (column, &index) in row.iter().enumerate() {
            let height = sources[index].height();
            let x = first + column as f32 * (BLOCK + ACROSS);

            sources[index].frame = Extent::new(v(x, top - height), v(x + BLOCK, top));
            lowest = lowest.min(top - height);
        }

        bottom = bottom.min(lowest);
        top = lowest - DOWN;
    }

    let mut x = middle + span + SIDE;

    for fan in &mut fans[left..] {
        fan.mirrored = true;
        fan.centre = v(x + reach.max.x, -depth);
        x = fan.centre.x - reach.min.x + BETWEEN;
    }

    let mut vents: Vec<(f32, f32)> = fans.iter().map(|fan| fan.vent(rotor)).collect();
    let sink = (fans.is_empty() && !piped.is_empty()).then_some((x, x + SINK));
    let right = match sink {
        Some((_, end)) => end + INSET,
        None if fans.len() > left => x - BETWEEN + INSET,
        None => middle + span + SIDE / 2.0,
    };

    vents.extend(sink);

    let centre = middle + span / 2.0;

    if vents.is_empty() {
        vents.push((centre - SINK / 2.0, centre + SINK / 2.0));
    }

    let mut floor = bottom - DOWN;

    if !fans.is_empty() {
        floor = floor.min(-depth + reach.min.y - AIRWAY);
    }

    let case = Extent::new(v(0.0, floor), v(right, 0.0));
    let intakes = if fans.is_empty() {
        vec![centre]
    } else {
        fans.iter().map(|fan| fan.centre.x).collect()
    };

    // The pipe from the fins at one end to those at the other, over every
    // block it drops to.
    let drops: Vec<f32> = piped
        .iter()
        .map(|&index| sources[index].frame.centre().x)
        .collect();
    let pipe = (!drops.is_empty() && (sink.is_some() || !fans.is_empty())).then(|| {
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

    // What the air cools gives its heat to the air drawn up the side to
    // the nearest fan (in one column between fans on both sides, to each
    // side in turn), or with none to the air rising.
    let has_right = fans.len() > left;
    let arrows = aired
        .iter()
        .enumerate()
        .map(|(k, &source)| {
            let frame = sources[source].frame;
            let y = frame.max.y - TITLE;
            let leftward = match (left > 0, has_right) {
                (true, true) if columns == 1 => k % 2 == 0,
                (true, true) => k % columns == 0,
                (inward, _) => inward,
            };

            if fans.is_empty() {
                let x = frame.centre().x;

                Arrow {
                    source,
                    from: v(x, frame.max.y + 2.0),
                    to: v(x, frame.max.y + DOWN - 4.0),
                    fan: None,
                }
            } else if leftward {
                Arrow {
                    source,
                    from: v(frame.min.x - 3.0, y),
                    to: v(fans[left - 1].centre.x + 3.0, y),
                    fan: Some(left - 1),
                }
            } else {
                Arrow {
                    source,
                    from: v(frame.max.x + 3.0, y),
                    to: v(fans[left].centre.x - 3.0, y),
                    fan: Some(left),
                }
            }
        })
        .collect();

    let extent = Extent::new(
        v(-MARGIN, floor - 30.0 - MARGIN),
        v(right + MARGIN, NAMES + LINE + 6.0 + MARGIN),
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

/// A temperature as a sheet letters it.
fn degrees(celsius: Option<f32>) -> String {
    match celsius {
        Some(celsius) => format!("{celsius:.0} °C"),
        None => "-- °C".into(),
    }
}

/// What a monitor calls a sensor, and the monitor: `PACKAGE, CORETEMP`.
fn called(sensor: &Sensor) -> String {
    match &sensor.label {
        Some(label) => format!("{}, {}", lettered(label), lettered(&sensor.chip)),
        None => lettered(&sensor.chip),
    }
}

/// The heat sources of `machine`: what its temperature sensors are on,
/// then its batteries.
fn sources(machine: &Machine) -> Vec<Source> {
    let mut sources: Vec<Source> = Vec::new();
    let mut modules: Vec<(usize, usize)> = Vec::new();

    for (index, sensor) in machine.sensors.iter().enumerate() {
        if sensor.kind != SensorKind::Temperature {
            continue;
        }

        let (kind, device) = match sensor.site {
            Site::Processor => (Kind::Processor, None),
            Site::Module(module) => {
                modules.push((module, index));
                (Kind::Memory, None)
            }
            Site::Board => (Kind::Board, None),
            Site::Device(address) => (device(machine, address), Some(address)),
        };

        match sources
            .iter_mut()
            .find(|source| source.kind == kind && source.device == device)
        {
            Some(source) => source.sensors.push(index),
            None => sources.push(Source {
                kind,
                name: String::new(),
                device,
                battery: None,
                sensors: vec![index],
                gauges: Vec::new(),
                cores: Vec::new(),
                frame: Extent::new(V2::ZERO, V2::ZERO),
            }),
        }
    }

    sources.extend((0..machine.batteries.len()).map(|battery| Source {
        kind: Kind::Battery,
        name: "BATTERY".into(),
        device: None,
        battery: Some(battery),
        sensors: Vec::new(),
        gauges: Vec::new(),
        cores: Vec::new(),
        frame: Extent::new(V2::ZERO, V2::ZERO),
    }));
    sources.sort_by_key(|source| source.kind);

    for source in &mut sources {
        match source.kind {
            // The package's (or the die's) thermometer, and the cores'
            // squares.
            Kind::Processor => {
                let package = source
                    .sensors
                    .iter()
                    .position(|&index| {
                        machine.sensors[index]
                            .label
                            .as_deref()
                            .is_some_and(|label| {
                                let label = label.to_lowercase();
                                ["package", "tctl", "tdie"]
                                    .iter()
                                    .any(|word| label.contains(word))
                            })
                    })
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
            Kind::Drive => "DRIVE",
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

    // Sources of the same name numbered apart.
    let names: Vec<String> = sources.iter().map(|source| source.name.clone()).collect();

    for (index, source) in sources.iter_mut().enumerate() {
        if names.iter().filter(|name| **name == source.name).count() > 1 {
            let nth = names[..index]
                .iter()
                .filter(|name| **name == source.name)
                .count();
            source.name = fit(&format!("{} {}", source.name, nth + 1), 10);
        }
    }

    sources
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
}

impl Cooling {
    /// The cooling of `machine`, if it measures a temperature.
    pub fn new(machine: &Machine) -> Option<Self> {
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
        // The fans turning when the machine was first read are drawn before
        // those that were not: a desktop's monitor lists every header on the
        // board, the empty ones first as often as not.
        let first = machine.sample(0.0);
        let mut listed: Vec<(usize, &Sensor)> = machine.fans().collect();

        listed.sort_by_key(|&(sensor, _)| rpm(&first, sensor) <= 0.0);

        let fans: Vec<Fan> = listed
            .iter()
            .take(MOST_FANS)
            .map(|&(sensor, about)| Fan {
                sensor,
                label: about.label.clone(),
                chip: about.chip.clone(),
                centre: V2::ZERO,
                mirrored: false,
            })
            .collect();
        let sources = sources(machine);
        let (sources, fans, plan) = Self::fold(sources, fans, rotor);

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
            notes.push("PHANTOM FANS: NOT SEEN TURNING".into());
        }

        notes.push(format!("THERMOMETERS {COLDEST} TO {HOTTEST} °C"));

        if listed.len() > fans.len() {
            notes.push(format!(
                "{} MORE NOT SHOWN",
                counted(listed.len() - fans.len(), "FAN", "FANS")
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

    /// `sources` and `fans` laid out in as many columns as fit the
    /// laptop's view best: two, a side each, when there are fans on both
    /// sides; one by the fans on one side; up to three, the air rising,
    /// with none.
    fn fold(sources: Vec<Source>, fans: Vec<Fan>, rotor: Rotor) -> (Vec<Source>, Vec<Fan>, Plan) {
        let columns: &[usize] = match fans.len() {
            0 => &[3, 2, 1],
            1 => &[1],
            _ => &[2, 1],
        };
        let overflow =
            |plan: &Plan| (plan.extent.width() / BUDGET.x).max(plan.extent.height() / BUDGET.y);

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

    /// The case in phantom, broken at its vents and intakes, and the air
    /// going in at its base and out of its vents.
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

        for &(a, b) in &plan.vents {
            for k in 1..=3 {
                let x = (a + (b - a) * k as f32 / 4.0).round();
                d.arrow(v(x, 2.0), v(x, EXHAUST), Line::Path);
            }
        }

        // The air in: up to each fan, or into the case with none.
        for (k, &x) in plan.intakes.iter().enumerate() {
            let to = match self.fans.get(k) {
                Some(fan) => v(x, self.inlet(fan)),
                None => v(x, min.y + DOWN),
            };

            d.arrow(v(x, min.y - 16.0), to, Line::Path);
            d.label(v(x, min.y - 24.0), "AIR IN");
        }

        if let Some((a, b)) = plan.sink {
            d.label(v((a + b) / 2.0, NAMES), "FINS").tone(Tone::Muted);
        }
    }

    /// Where the air drawn up to `fan` meets it: under its housing.
    fn inlet(&self, fan: &Fan) -> f32 {
        fan.centre.y + self.rotor.reach().min.y - 3.0
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
        let drops: Vec<(f32, f32)> = plan.drops.iter().map(|&x| (x - half, x + half)).collect();

        d.line(v(from, upper), v(to, upper), Line::Outline);

        for (a, b) in gaps(from, to, &drops) {
            d.line(v(a, lower), v(b, lower), Line::Outline);
        }

        for x in [from, to] {
            d.line(v(x, upper), v(x, lower), Line::Outline);
        }

        for &x in &plan.drops {
            for side in [-half, half] {
                d.line(v(x + side, lower), v(x + side, -COOLED), Line::Outline);
            }
        }

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
    }

    /// A source's block: its outline and name, and its gauges' glass.
    fn block(&self, d: &mut Draft, source: &Source) {
        let frame = source.frame;
        let title = v(frame.min.x + PAD, frame.max.y - TITLE);

        d.rect(frame.min, frame.max, Line::Outline);
        d.label(title, source.name.as_str())
            .anchor(Anchor::LEFT)
            .tone(Tone::Muted);

        if source.kind == Kind::Battery {
            let at = source.gauge(0);
            let left = frame.min.x + PAD;

            d.rect(
                v(left, at.y - 3.5),
                v(left + CELL, at.y + 3.5),
                Line::Outline,
            );
            d.rect(
                v(left + CELL, at.y - 1.5),
                v(left + CELL + 2.5, at.y + 1.5),
                Line::Outline,
            );
        } else {
            for k in 0..source.gauges.len() {
                let at = source.gauge(k);
                // Where the tube leaves the bulb.
                let neck = (BULB_RADIUS * BULB_RADIUS - GLASS * GLASS).sqrt();
                let end = at.x + BULB_RADIUS + TUBE;

                d.circle(at, BULB_RADIUS, Line::Outline);
                d.polyline(
                    &[
                        v(at.x + neck, at.y + GLASS),
                        v(end, at.y + GLASS),
                        v(end, at.y - GLASS),
                        v(at.x + neck, at.y - GLASS),
                    ],
                    Line::Outline,
                );

                // The scale, every 20 °C, where a detail has room for it.
                d.in_detail(|d| {
                    for step in 0..=4 {
                        let x = at.x + BULB_RADIUS + TUBE * step as f32 / 4.0;
                        d.line(v(x, at.y - GLASS), v(x, at.y - GLASS - 1.5), Line::Thin);
                    }
                });
            }

            for k in 0..source.cores.len() {
                let square = source.core(k);
                d.rect(square.min, square.max, Line::Thin);
            }
        }

        d.in_detail(|d| {
            d.label(
                v(frame.min.x + PAD, source.gauge(0).y + DETAIL_TITLE),
                source.name.as_str(),
            )
            .anchor(Anchor::LEFT)
            .tone(Tone::Ink);
        });
    }

    /// What a source's gauges read now: its hottest temperature, each
    /// thermometer's column and its value (in a detail), its cores' tints;
    /// a battery's charge and its power.
    fn readings_of(&self, d: &mut Draft, source: &Source, snapshot: &Snapshot) {
        let frame = source.frame;
        let value = v(frame.max.x - PAD, frame.max.y - TITLE);

        if let Some(battery) = source.battery {
            let charge = snapshot.batteries.get(battery).cloned().unwrap_or_default();
            let at = source.gauge(0);
            let left = frame.min.x + PAD;
            let fraction = charge.fraction.filter(|f| f.is_finite()).unwrap_or(0.0);
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

            if fraction > 0.0 {
                d.area(
                    &corners(Extent::new(
                        v(left + 1.0, at.y - 2.5),
                        v(left + 1.0 + (CELL - 2.0) * fraction, at.y + 2.5),
                    )),
                    Fill::Solid,
                )
                .tone(tone);
            }

            d.label(
                value,
                charge
                    .fraction
                    .map_or("-- %".into(), |f| format!("{:.0} %", f * 100.0)),
            )
            .anchor(Anchor::RIGHT)
            .tone(Tone::Ink);
            d.label(v(left + CELL + 8.0, at.y), power)
                .anchor(Anchor::LEFT)
                .tone(Tone::Muted);

            // What it is doing, in words, where a detail has room.
            d.in_detail(|d| {
                let right = left + CELL + 6.0;
                let mut how = charge
                    .fraction
                    .map_or("-- %".into(), |f| format!("{:.0} %", f * 100.0));

                if let Some(watts) = watts {
                    how += &format!(", {watts:.1} W");
                }

                d.label(v(right, at.y + 3.0), state)
                    .anchor(Anchor::LEFT)
                    .tone(Tone::Ink);
                d.label(v(right, at.y - 3.0), how)
                    .anchor(Anchor::LEFT)
                    .tone(Tone::Muted);
            });
            return;
        }

        d.label(value, degrees(source.hottest(snapshot)))
            .anchor(Anchor::RIGHT)
            .tone(Tone::Ink);

        for (k, &sensor) in source.gauges.iter().enumerate() {
            let at = source.gauge(k);
            let reading = celsius(snapshot, sensor);
            let tone = if reading.is_some_and(|c| c >= HOT) {
                Tone::Caution
            } else {
                Tone::Accent
            };
            let share = reading
                .map(|c| ((c - COLDEST) / (HOTTEST - COLDEST)).clamp(0.0, 1.0))
                .unwrap_or(0.0);

            d.area(
                &arc_points(at, BULB_RADIUS - 1.0, 0.0, TAU, 12),
                Fill::Solid,
            )
            .tone(tone);

            if share > 0.0 {
                d.area(
                    &corners(Extent::new(
                        v(at.x + BULB_RADIUS - 1.0, at.y - GLASS + 1.0),
                        v(at.x + BULB_RADIUS + TUBE * share, at.y + GLASS - 1.0),
                    )),
                    Fill::Solid,
                )
                .tone(tone);
            }

            d.in_detail(|d| {
                d.label(v(at.x + BULB_RADIUS + TUBE + 4.0, at.y), degrees(reading))
                    .anchor(Anchor::LEFT)
                    .tone(Tone::Ink);
            });
        }

        for (k, &sensor) in source.cores.iter().enumerate() {
            let Some(c) = celsius(snapshot, sensor) else {
                continue;
            };
            let level = (1.0 + 15.0 * (c - WARM) / (HOTTEST - WARM)).round() as u8;

            d.area(&corners(source.core(k)), Fill::Tint(level.clamp(1, 16)))
                .tone(if c >= HOT {
                    Tone::Caution
                } else {
                    Tone::Accent
                });
        }
    }

    /// Fan `index` turned to `angle`: its housing, its fins and its rotor,
    /// in phantom until it has been seen turning, and its name and speed
    /// over its vent.
    fn fan(&self, d: &mut Draft, index: usize, angle: f32, seen: bool, speed: f32) {
        let fan = &self.fans[index];
        let (edge, thin) = if seen {
            (Line::Outline, Line::Thin)
        } else {
            (Line::Phantom, Line::Phantom)
        };
        let drawn = |points: &[V2]| points.iter().map(|&p| fan.at(p)).collect::<Vec<_>>();

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
        self.fins(d, fan.vent(self.rotor), edge, thin);

        // Which way it turns, where a detail has room to show it.
        d.in_detail(|d| {
            let arc = drawn(&arc_points(V2::ZERO, HUB - 3.0, 0.3 * PI, 1.2 * PI, 16));
            let (tail, head) = (arc[arc.len() - 2], arc[arc.len() - 1]);

            d.polyline(&arc[..arc.len() - 1], Line::Thin)
                .tone(Tone::Live);
            d.arrow(tail, head, Line::Thin).tone(Tone::Live);
        });

        let (a, b) = fan.vent(self.rotor);
        let middle = ((a + b) / 2.0).round();
        let (name, value) = if seen {
            (Tone::Muted, Tone::Ink)
        } else {
            (Tone::Faint, Tone::Faint)
        };

        d.label(v(middle, NAMES), FAN_NAMES[index])
            .nudge(0, -(LINE as i32))
            .tone(name);
        d.label(v(middle, NAMES), format!("{speed:.0} rpm"))
            .tone(value);
    }

    /// What runs along the paths: heat up the pipe from what it cools, more
    /// of it the hotter that is; the air each fan draws in and blows out,
    /// as far as the fan has turned; the heat that air takes on its way.
    fn flows(&self, d: &mut Draft, snapshot: &Snapshot, turns: &[f64], seen: &[bool], t: f32) {
        let heat = |source: &Source| {
            source
                .hottest(snapshot)
                .map_or(0.0, |c| ((c - WARM) / (SCALDING - WARM)).clamp(0.05, 1.0))
        };
        let piped = self.sources.iter().filter(|source| source.kind.piped());

        if self.plan.pipe.is_some() {
            for (source, &x) in piped.zip(&self.plan.drops) {
                for &(a, b) in &self.plan.vents {
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
            let x = fan.centre.x;
            let intake = [vec![
                v(x, self.plan.case.min.y - 16.0),
                v(x, self.inlet(fan)),
            ]];

            flow(d, &intake, 1.0, SPACING, travelled, false, Tone::Live);

            let (a, b) = fan.vent(self.rotor);

            for step in 1..=3 {
                let x = (a + (b - a) * step as f32 / 4.0).round();
                let out = [vec![v(x, 2.0), v(x, EXHAUST)]];

                flow(d, &out, 1.0, SPACING, travelled, false, Tone::Live);
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
        let cooled = |source: &Source| match self
            .plan
            .arrows
            .iter()
            .find(|arrow| std::ptr::eq(&self.sources[arrow.source], source))
            .map(|arrow| arrow.fan)
        {
            Some(Some(fan)) => format!("BY AIR TO {}", FAN_NAMES[fan]),
            _ => "BY AIR RISING".into(),
        };
        let fans = match self.fans.len() {
            0 => "FINS".into(),
            n => counted(n, "FAN", "FANS"),
        };

        let (name, quantity, value) = match item {
            Item::Sources(Kind::Processor) => {
                let cpu = machine.cpu.as_ref();

                if let Some(model) = cpu.and_then(|cpu| cpu.model.as_deref()) {
                    spec.extend(rows("MODEL", &lettered(model)));
                }

                for source in &sources {
                    if let Some(&gauge) = source.gauges.first() {
                        spec.extend(rows("SENSOR", &called(&machine.sensors[gauge])));
                    }

                    // Every sensor but the package's is a core's, drawn
                    // or not.
                    if !source.cores.is_empty() {
                        spec.push((
                            "CORES".into(),
                            format!("{} MEASURED", source.sensors.len() - 1),
                        ));
                    }
                }

                spec.push(("COOLED".into(), format!("BY HEAT PIPE TO {fans}")));

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

                ("CPU", cpu.map_or(1, |cpu| cpu.packages.max(1)), value)
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
                        format!("{} ON EACH", lettered(&machine.sensors[gauge].chip)),
                    ));
                }

                if let Some(source) = sources.first() {
                    spec.push(("COOLED".into(), cooled(source)));
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
                    let drive = machine
                        .drives
                        .iter()
                        .find(|drive| drive.pci.is_some() && drive.pci == source.device);

                    match (drive, device) {
                        (Some(drive), _) => {
                            let model = drive.model.as_deref().map(lettered).unwrap_or_default();

                            spec.extend(rows(&drive.name, &model));
                            values.push(decimal(drive.bytes));
                        }
                        (None, Some(device)) => {
                            spec.extend(rows(&source.name, &named(device)));
                            values.push(match kind {
                                Kind::Network => source.name.clone(),
                                _ => short(device.address),
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
                                (None, chip) => lettered(chip),
                            }
                        })
                        .collect();

                    spec.extend(rows("SENSORS", &sensors.join(", ")));

                    if !kind.piped() {
                        spec.push(("COOLED".into(), cooled(source)));
                    }
                }

                if kind.piped() {
                    spec.push(("COOLED".into(), format!("BY HEAT PIPE TO {fans}")));
                }

                let value = match values.as_slice() {
                    [one] => one.clone(),
                    _ if kind == Kind::Network => values.join(", ").replace("ETHERNET", "ETH"),
                    _ => counted(values.len(), "SOURCE", "SOURCES"),
                };

                (kind.part(), sources.len() as u32, value)
            }
            Item::Fans => {
                for (k, fan) in self.fans.iter().enumerate() {
                    let about = match &fan.label {
                        Some(label) => format!("{}, {}", lettered(label), lettered(&fan.chip)),
                        None => lettered(&fan.chip),
                    };

                    spec.extend(rows(FAN_NAMES[k], &about));
                }

                spec.push(("DRAWN".into(), format!("SLOWED {SLOWED} TIMES")));

                let value = match self.rotor {
                    Rotor::Blower => "BLOWER",
                    Rotor::Axial => "AXIAL",
                };

                ("FAN", self.fans.len() as u32, value.into())
            }
        };

        // Its detail: the first of its blocks, or the first fan.
        let (centre, radius) = match item {
            Item::Fans => (self.fans[0].centre, self.rotor.ring()),
            Item::Sources(_) => sources[0].ring(),
        };
        let mut part = Part::new(name, quantity, &fit(&value, 10)).detail(centre, radius);

        part.spec = spec.into_iter().take(SPEC_ROWS).collect();
        part
    }
}

/// The corners of `extent`, round it.
fn corners(extent: Extent) -> [V2; 4] {
    [
        extent.min,
        v(extent.max.x, extent.min.y),
        extent.max,
        v(extent.min.x, extent.max.y),
    ]
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

    fn draw(&self, d: &mut Draft, t: f32) {
        let snapshot = self.machine.sample(t);
        let seen = self.seen(&snapshot);
        let turns = self.turns(&snapshot, t);

        self.case(d);
        self.pipe(d);

        for arrow in &self.plan.arrows {
            d.arrow(arrow.from, arrow.to, Line::Path);
        }

        for source in &self.sources {
            d.part(self.index(Item::Sources(source.kind)), |d| {
                self.block(d, source)
            });
        }

        for fan in &self.fans {
            d.centre_mark(fan.centre, HUB, 3.0);
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
        });

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
                FAN_NAMES[k],
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
    use crate::draft::{Ink, Shape};

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

        assert!(readings.contains(&"FAN 1 1200 rpm".into()), "{readings:?}");
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
    /// the air cools gives its heat to each side in turn, and it all fits
    /// the laptop's view.
    #[test]
    fn four_fans_take_the_heat_from_one_column_each_side_in_turn() {
        let mut machine = Machine::fixture();
        let fan = machine.fans().last().unwrap().1.clone();

        machine.sensors.extend([fan.clone(), fan]);

        let cooling = Cooling::new(&machine).unwrap();
        let extent = cooling.extent();
        let sides: Vec<bool> = cooling.fans.iter().map(|fan| fan.mirrored).collect();

        assert_eq!(sides, [false, false, true, true]);
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
            [Some(1), Some(2), Some(1), Some(2)]
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
        assert_eq!((b - a, fan.centre.y), (2.0 * FRAME, -(FINS + 2.0 + FRAME)));
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
                .any(|mark| matches!(mark.ink, Ink::Dot { .. }))
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

    #[test]
    fn readings_are_the_live_values() {
        let cooling = Cooling::new(&Machine::fixture()).unwrap();
        let readings = cooling.readings(3.0);
        let names: Vec<&str> = readings.iter().map(|r| r.name).collect();

        assert_eq!(names, ["CPU", "FAN 1", "FAN 2", "BATTERY"]);
        assert!(readings[1].value.ends_with(" rpm"));
    }
}
