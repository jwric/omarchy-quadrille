//! The machine as a block diagram: the CPU package with its cores, the
//! memory, what is on the PCIe buses (bridges, graphics, drives, network
//! adapters), the USB tree and the displays on the graphics card's
//! connectors. Traffic runs along the wires as dots, at the rates the
//! machine measures its drives and network adapters at.
use quadrille::draw::Anchor;

use crate::draft::Placement::Auto;
use crate::draft::geom::{along, length};
use crate::draft::{Draft, Extent, Fill, Line, Tone, V2, v};
use crate::machine::{
    CoreKind, Interface, Link, Machine, PciDevice, PciKind, SensorKind, Site, UsbDevice,
};

use super::super::schematic::Schematic;
use super::super::{Card, Domain, Part, Reading, Revision, Subject, Unit};
use super::layout::{Diagram, Form, Gauge, Group, Placed, Source, named, short, version};
use super::{binary, bits, counted, decimal, fit, lettered, rate};

/// A dot of traffic every so many units along a wire, running so fast.
const SPACING: f64 = 8.0;
const SPEED: f64 = 40.0;
/// The rates the dots run at, from a few to all of them: a kilobyte a
/// second to a hundred megabytes, by their logarithm.
const QUIETEST: f32 = 1e3;
const BUSIEST: f32 = 1e8;
/// The specification's rows, at most, and the characters a row holds on
/// the laptop's column: name and value with room between them.
const SPEC_ROWS: usize = 6;
const SPEC_ROOM: usize = 33;

/// What an item of the parts list is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Processor,
    Memory,
    Blocks(Group),
}

pub struct Topology {
    card: Card,
    machine: Machine,
    diagram: Diagram,
    /// The parts list's items, in its order.
    items: Vec<Item>,
    /// The sensor the processor's temperature is read from.
    temperature: Option<usize>,
}

impl Topology {
    /// The topology of `machine`, if it has a processor and something on
    /// its buses to draw.
    pub fn new(machine: &Machine) -> Option<Self> {
        let diagram = Diagram::new(machine)?;
        let mut items = vec![Item::Processor];

        items.extend(machine.memory.as_ref().map(|_| Item::Memory));

        for group in [
            Group::Graphics,
            Group::Display,
            Group::Drive,
            Group::Network,
            Group::Usb,
            Group::Bridge,
        ] {
            if diagram.blocks.iter().any(|block| block.group == group) {
                items.push(Item::Blocks(group));
            }
        }

        let parts = items
            .iter()
            .map(|item| part(machine, &diagram, *item))
            .collect();
        let card = Card {
            title: format!("{} TOPOLOGY", machine.chassis.kind.label()),
            number: "QD-C-0001".into(),
            domain: Domain::Computing,
            unit: Unit::Millimetre,
            scaled: false,
            view: "BLOCK DIAGRAM".into(),
            notes: vec![
                "READ FROM /sys AND /proc".into(),
                "DOTS: I/O MEASURED EACH SECOND".into(),
                "READ, RECEIVED: TOWARD THE CPU".into(),
                format!(
                    "{} OF {} PCI FUNCTIONS SHOWN",
                    diagram.shown,
                    machine.pci.len()
                ),
            ],
            revisions: vec![Revision::first()],
            parts,
        };
        let temperature = machine
            .sensors_on(Site::Processor)
            .filter(|(_, sensor)| sensor.kind == SensorKind::Temperature)
            .min_by_key(|(_, sensor)| {
                !sensor
                    .label
                    .as_deref()
                    .is_some_and(|label| label.to_lowercase().contains("package"))
            })
            .map(|(index, _)| index);

        Some(Self {
            card,
            machine: machine.clone(),
            diagram,
            items,
            temperature,
        })
    }

    /// The parts list's index of `item`.
    fn index(&self, item: Item) -> usize {
        self.items
            .iter()
            .position(|other| *other == item)
            .expect("Every item drawn is listed")
    }

    /// The CPU package: its die, cores and last cache.
    fn package(&self, d: &mut Draft) {
        let package = &self.diagram.package;
        let cpu = self.machine.cpu.as_ref().expect("A diagram has a CPU");
        let frame = package.frame;
        let title = frame.max.y - 10.0;

        d.rect(frame.min, frame.max, Line::Outline);
        d.rect(package.die.min, package.die.max, Line::Thin);
        d.label(v(frame.min.x + 6.0, title), "CPU")
            .anchor(Anchor::LEFT)
            .tone(Tone::Ink);
        d.label(
            v(frame.max.x - 6.0, title),
            counted(cpu.cores as usize, "CORE", "CORES"),
        )
        .anchor(Anchor::RIGHT);

        for (core, kind) in &package.cores {
            d.rect(core.min, core.max, Line::Outline);

            if *kind == CoreKind::Efficient {
                d.area(&corners(*core), Fill::Tint(4));
            }
        }

        if let Some((band, text)) = &package.cache {
            d.rect(band.min, band.max, Line::Thin);
            d.label(band.centre(), text.as_str());
        }

        // What a detail has room to say: the kind of every core, and the
        // processor in full.
        d.in_detail(|d| {
            for (core, kind) in &package.cores {
                let letter = match kind {
                    CoreKind::Performance => "P",
                    CoreKind::Efficient => "E",
                };

                d.label(core.centre(), letter).tone(Tone::Faint);
            }

            d.label(
                v(frame.min.x + 3.0, title),
                format!(
                    "CPU, {}, {} THREADS",
                    counted(cpu.cores as usize, "CORE", "CORES"),
                    cpu.threads
                ),
            )
            .anchor(Anchor::LEFT)
            .tone(Tone::Ink);

            if let Some((band, text)) = &package.cache {
                d.label(band.centre(), format!("{text} CACHE"));
            }
        });
    }

    /// The memory: its modules as sticks, and its bus to the package.
    fn memory(&self, d: &mut Draft) {
        let Some(bank) = &self.diagram.bank else {
            return;
        };
        let memory = self.machine.memory.as_ref();
        let size = memory
            .and_then(|memory| memory.installed())
            .map(binary)
            .unwrap_or_default();
        let frame = bank.frame;
        let title = frame.max.y - 10.0;

        d.rect(frame.min, frame.max, Line::Outline);
        d.label(v(frame.min.x + 6.0, title), "MEMORY")
            .anchor(Anchor::LEFT)
            .tone(Tone::Ink);
        d.label(v(frame.max.x - 6.0, title), size.as_str())
            .anchor(Anchor::RIGHT);

        for stick in &bank.sticks {
            d.rect(stick.min, stick.max, Line::Outline);
        }

        let (from, to) = bank.bus;

        for side in [-2.0, 2.0] {
            d.line(from + v(side, 0.0), to + v(side, 0.0), Line::Outline);
        }

        if let Some(generation) = &bank.generation {
            d.label(from.lerp(to, 0.5) + v(6.0, 0.0), generation.as_str())
                .anchor(Anchor::LEFT);
        }

        // The memory chips on the sticks, and the bank in full.
        d.in_detail(|d| {
            for stick in &bank.sticks {
                for chip in 0..4 {
                    let x = stick.min.x + 3.0 + chip as f32 * 9.0;
                    d.rect(
                        v(x, stick.min.y + 2.0),
                        v(x + 7.0, stick.max.y - 2.0),
                        Line::Thin,
                    );
                }
            }

            let generation = bank.generation.as_deref().unwrap_or_default();
            d.label(
                v(frame.min.x + 3.0, title),
                format!("MEMORY {size} {generation}"),
            )
            .anchor(Anchor::LEFT)
            .tone(Tone::Ink);
        });
    }

    /// A block of the diagram, by its form.
    fn block(&self, d: &mut Draft, block: &Placed) {
        let frame = block.frame;

        match block.form {
            Form::Block => d.rect(frame.min, frame.max, Line::Outline),
            Form::Screen { internal } => {
                d.rect(frame.min, frame.max, Line::Outline);
                d.rect(frame.min + v(2.0, 2.0), frame.max - v(2.0, 2.0), Line::Thin);

                let (left, right, foot) = (frame.min.x, frame.max.x, frame.min.y);

                if internal {
                    // The machine's own panel stands on its base.
                    d.polygon(
                        &[
                            v(left, foot - 1.0),
                            v(right, foot - 1.0),
                            v(right + 3.0, foot - 5.0),
                            v(left - 3.0, foot - 5.0),
                        ],
                        Line::Outline,
                    )
                } else {
                    let middle = frame.centre().x.round();

                    d.line(v(middle, foot), v(middle, foot - 4.0), Line::Outline);
                    d.line(
                        v(middle - 12.0, foot - 5.0),
                        v(middle + 12.0, foot - 5.0),
                        Line::Outline,
                    )
                }
            }
            Form::Terminal => {
                d.circle(v(frame.min.x + 3.0, block.port.y), 2.5, Line::Outline);
                d.label(v(frame.min.x + 9.0, block.port.y), block.lines[0].as_str())
                    .anchor(Anchor::LEFT);
                return;
            }
            Form::Bridge => {
                // The converter: a square crossed by its diagonal.
                d.rect(frame.min, frame.max, Line::Outline);
                d.line(frame.min, frame.max, Line::Outline);
                d.in_detail(|d| {
                    for (k, line) in block.more.iter().rev().enumerate() {
                        d.label(
                            v(frame.centre().x, frame.max.y + 5.0 + 6.0 * k as f32),
                            line.as_str(),
                        )
                        .tone(if k + 1 == block.more.len() {
                            Tone::Ink
                        } else {
                            Tone::Muted
                        });
                    }
                });
                return;
            }
        };

        for (k, line) in block.lines.iter().enumerate() {
            d.label(
                v(frame.min.x + 6.0, block.port.y - 12.0 * k as f32),
                line.as_str(),
            )
            .anchor(Anchor::LEFT)
            .tone(if k == 0 { Tone::Ink } else { Tone::Muted });
        }

        // A screen's lettering clears its glass.
        let first = match block.form {
            Form::Screen { .. } => frame.max.y - 7.0,
            _ => frame.max.y - 5.0,
        };

        d.in_detail(|d| {
            for (k, line) in block.more.iter().enumerate() {
                d.label(v(frame.min.x + 4.0, first - 6.0 * k as f32), line.as_str())
                    .anchor(Anchor::LEFT)
                    .tone(if k == 0 { Tone::Ink } else { Tone::Muted });
            }
        });
    }

    /// The traffic on every route, at the rates measured at `t`.
    fn traffic(&self, d: &mut Draft, t: f32) {
        let snapshot = self.machine.sample(t);

        for route in &self.diagram.routes {
            let (inward, outward) = match route.gauge {
                Gauge::Drive(index) => snapshot
                    .drives
                    .get(index)
                    .copied()
                    .flatten()
                    .map(|transfer| (transfer.read, transfer.written)),
                Gauge::Interface(index) => snapshot
                    .interfaces
                    .get(index)
                    .copied()
                    .flatten()
                    .map(|traffic| (traffic.received, traffic.sent)),
            }
            .unwrap_or_default();

            dots(d, &route.pieces, inward, true, t, Tone::Live);
            dots(d, &route.pieces, outward, false, t, Tone::Accent);
        }
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

/// Traffic along `pieces` at `rate` bytes a second, toward the processor
/// if `inward`: dots a set distance apart running at a set speed, each
/// there or not by its own number and the rate, so a busier link carries
/// more of them and none blinks as it runs.
fn dots(d: &mut Draft, pieces: &[Vec<V2>], rate: f32, inward: bool, t: f32, tone: Tone) {
    let share =
        ((rate.max(1.0).log10() - QUIETEST.log10()) / (BUSIEST / QUIETEST).log10()).clamp(0.0, 1.0);

    if !share.is_finite() || share <= 0.0 {
        return;
    }

    let lengths: Vec<f32> = pieces.iter().map(|piece| length(piece)).collect();
    let total: f32 = lengths.iter().sum();
    let travelled = f64::from(t) * SPEED;
    let first = ((travelled - f64::from(total)) / SPACING).ceil() as i64;
    let last = (travelled / SPACING).floor() as i64;
    // The two ways' dots are numbered apart, so they are not all there or
    // not together.
    let offset = if inward { 0.5 } else { 0.0 };

    for number in first..=last {
        let lot = (number as f64 * 0.618_034 + offset).rem_euclid(1.0);

        if lot >= f64::from(share) {
            continue;
        }

        let run = (travelled - number as f64 * SPACING) as f32;
        let mut left = if inward { total - run } else { run };

        for (piece, &length) in pieces.iter().zip(&lengths) {
            if left <= length {
                d.dot(along(piece, left), 2).tone(tone);
                break;
            }
            left -= length;
        }
    }
}

/// Where the balloon for `block` points: its top right corner, which
/// leaves it the room right of a block's lettering.
fn tip(block: &Placed) -> V2 {
    match block.form {
        Form::Bridge => v(block.frame.centre().x, block.frame.min.y),
        Form::Terminal => v(block.frame.min.x + 3.0, block.port.y - 2.5),
        _ => block.frame.max,
    }
}

/// The circle a detail of `block` magnifies.
fn ring(block: &Placed) -> (V2, f32) {
    let frame = block.frame;

    match block.form {
        // The symbol and its address over it.
        Form::Bridge => (frame.centre() + v(0.0, 6.0), 22.0),
        _ => (
            frame.centre(),
            (frame.width().max(frame.height()) / 2.0 + 4.0).round(),
        ),
    }
}

/// Specification rows of `name` and `value`, the value carried onto a
/// second row if it is too long for one.
fn rows(name: &str, value: &str) -> Vec<(String, String)> {
    let room = SPEC_ROOM - name.chars().count() - 2;

    if value.chars().count() <= room {
        return vec![(name.into(), value.into())];
    }

    textwrap::wrap(value, room)
        .into_iter()
        .take(2)
        .enumerate()
        .map(|(k, line)| {
            let name = if k == 0 { name } else { "" };
            (name.to_owned(), fit(&line, room))
        })
        .collect()
}

/// The parts list's item `item` of `machine`, drawn as `diagram`.
fn part(machine: &Machine, diagram: &Diagram, item: Item) -> Part {
    let mut spec: Vec<(String, String)> = Vec::new();
    let blocks: Vec<&Placed> = diagram
        .blocks
        .iter()
        .filter(|block| Item::Blocks(block.group) == item)
        .collect();
    let pci = |block: &Placed| match block.source {
        Source::Pci(address) => machine.pci_device(address),
        _ => None,
    };

    let (name, quantity, value, (centre, radius)) = match item {
        Item::Processor => {
            let cpu = machine.cpu.as_ref().expect("A diagram has a CPU");
            let model = cpu.model.as_deref().map_or("PROCESSOR".into(), lettered);
            let cores = match cpu.kinds.as_slice() {
                [] => cpu.cores.to_string(),
                kinds => kinds
                    .iter()
                    .map(|cores| {
                        let kind = match cores.kind {
                            CoreKind::Performance => "P",
                            CoreKind::Efficient => "E",
                        };
                        format!("{} {kind}", cores.cores)
                    })
                    .collect::<Vec<_>>()
                    .join(" + "),
            };
            let ghz = |mhz: u32| format!("{:.1}", mhz as f32 / 1000.0);
            let mut levels: Vec<u8> = cpu.caches.iter().map(|cache| cache.level).collect();
            levels.sort_unstable();
            levels.dedup();
            let caches: Vec<String> = levels
                .iter()
                .filter(|&&level| level >= 2)
                .map(|&level| {
                    let bytes: u64 = cpu
                        .caches
                        .iter()
                        .filter(|cache| cache.level == level)
                        .map(|cache| cache.bytes * u64::from(cache.instances))
                        .sum();
                    format!("L{level} {}", binary(bytes))
                })
                .collect();

            spec.extend(rows("MODEL", &model));
            spec.push(("CORES".into(), cores));
            spec.push(("THREADS".into(), cpu.threads.to_string()));

            match (cpu.base_mhz, cpu.max_mhz) {
                (Some(base), Some(max)) if base < max => {
                    spec.push(("CLOCK".into(), format!("{} TO {} GHz", ghz(base), ghz(max))));
                }
                (_, Some(max)) => spec.push(("CLOCK".into(), format!("{} GHz", ghz(max)))),
                _ => {}
            }

            if !caches.is_empty() {
                spec.push(("CACHE".into(), caches.join(", ")));
            }

            let die = diagram.package.die;

            (
                "CPU",
                cpu.packages.max(1),
                counted(cpu.cores as usize, "CORE", "CORES"),
                (
                    die.centre(),
                    (diagram.package.frame.width() / 2.0 + 2.0).round(),
                ),
            )
        }
        Item::Memory => {
            let memory = machine.memory.as_ref().expect("Listed with memory");
            let installed = memory.installed().map(binary).unwrap_or("?".into());
            let modules = memory.modules.len();
            let bank = diagram.bank.as_ref().expect("Drawn with memory");

            spec.push(("INSTALLED".into(), installed.clone()));
            spec.extend(
                memory
                    .bytes
                    .map(|bytes| ("AVAILABLE".into(), binary(bytes))),
            );
            spec.push((
                "MODULES".into(),
                match memory.modules.first() {
                    Some(module) => format!(
                        "{modules} × {}",
                        module.generation.as_deref().unwrap_or("DIMM")
                    ),
                    None => "NOT SHOWN".into(),
                },
            ));

            (
                "MEMORY",
                modules.max(1) as u32,
                installed,
                (
                    bank.frame.centre(),
                    (bank.frame.width() / 2.0 + 2.0).round(),
                ),
            )
        }
        Item::Blocks(group) => {
            let first = blocks
                .iter()
                .find(|block| block.balloon)
                .unwrap_or(&blocks[0]);
            let ring = ring(first);
            let devices: Vec<&PciDevice> = blocks.iter().filter_map(|block| pci(block)).collect();

            match group {
                Group::Graphics => {
                    let mut outputs = 0;

                    for device in &devices {
                        let names: Vec<&str> = machine
                            .connectors
                            .iter()
                            .filter(|c| c.gpu == Some(device.address))
                            .map(|c| c.name.as_str())
                            .collect();

                        outputs += names.len();
                        spec.extend(rows(&short(device.address), &named(device)));
                        spec.extend(rows("OUTPUTS", &names.join(", ")));
                    }

                    (
                        "GRAPHICS",
                        devices.len() as u32,
                        counted(outputs, "OUTPUT", "OUTPUTS"),
                        ring,
                    )
                }
                Group::Display => {
                    let mut inches = Vec::new();

                    for block in &blocks {
                        if let Source::Connector(index) = block.source
                            && let Some(connector) = machine.connectors.get(index)
                            && let Some(panel) = &connector.panel
                        {
                            let refresh = panel
                                .refresh
                                .map(|hz| format!("{} Hz, ", hz.round()))
                                .unwrap_or_default();

                            inches.push(panel.inches());
                            spec.push((
                                connector.name.clone(),
                                format!(
                                    "{:.1}\" {} × {}",
                                    panel.inches(),
                                    panel.pixels.0,
                                    panel.pixels.1
                                ),
                            ));
                            spec.push((
                                String::new(),
                                format!("{refresh}PITCH {:.3} mm", panel.pitch_mm()),
                            ));
                        }
                    }

                    let sizes: Vec<String> = inches
                        .iter()
                        .map(|inches| inches.round().to_string())
                        .collect();
                    let value = match sizes.as_slice() {
                        [_] => format!("{:.1} IN", inches[0]),
                        _ if sizes.join("+").len() + 3 <= 10 => format!("{} IN", sizes.join("+")),
                        _ => format!(
                            "≤ {} IN",
                            inches.iter().copied().fold(0.0, f64::max).round()
                        ),
                    };

                    ("DISPLAY", inches.len() as u32, value, ring)
                }
                Group::Drive => {
                    let mut bytes = 0;
                    let mut drives = 0;

                    for block in &blocks {
                        if let Source::Drive(index) = block.source
                            && let Some(drive) = machine.drives.get(index)
                        {
                            let model = drive.model.as_deref().map(lettered);
                            let filesystems: Vec<String> = drive
                                .partitions
                                .iter()
                                .filter_map(|p| p.filesystem.as_deref())
                                .map(str::to_uppercase)
                                .collect();

                            bytes += drive.bytes;
                            drives += 1;
                            spec.extend(rows(&drive.name, model.as_deref().unwrap_or("")));
                            spec.push((
                                String::new(),
                                format!(
                                    "{}, {}",
                                    decimal(drive.bytes),
                                    counted(drive.partitions.len(), "PARTITION", "PARTITIONS")
                                ),
                            ));

                            if !filesystems.is_empty() {
                                spec.extend(rows("", &filesystems.join(", ")));
                            }
                        }
                    }

                    ("DRIVE", drives, decimal(bytes), ring)
                }
                Group::Network => {
                    let mut links = Vec::new();

                    for device in &devices {
                        let interface = machine
                            .interfaces
                            .iter()
                            .find(|i| i.pci == Some(device.address) && !i.usb);
                        let state = match interface {
                            Some(Interface {
                                up: true,
                                speed: Some(speed),
                                ..
                            }) => format!("{}, ", bits(*speed as f32)),
                            Some(Interface { up: true, .. }) => "UP, ".into(),
                            Some(Interface { up: false, .. }) => "DOWN, ".into(),
                            None => String::new(),
                        };
                        let link = interface.map(|i| i.link);
                        let kind = match link {
                            Some(Link::Wireless) => "WI-FI",
                            Some(Link::Ethernet) => "ETHERNET",
                            _ => "NETWORK",
                        };

                        links.push(link);
                        spec.extend(rows(kind, &named(device)));
                        spec.push((
                            String::new(),
                            format!("{state}PCI {}", short(device.address)),
                        ));
                    }

                    let value = match links.as_slice() {
                        [Some(Link::Ethernet)] => "ETHERNET".into(),
                        [Some(Link::Wireless)] => "WI-FI".into(),
                        [Some(Link::Ethernet), Some(Link::Wireless)]
                        | [Some(Link::Wireless), Some(Link::Ethernet)] => "ETH, WI-FI".into(),
                        _ => counted(links.len(), "LINK", "LINKS"),
                    };

                    ("NETWORK", links.len() as u32, value, ring)
                }
                Group::Usb => {
                    let mut fastest = None;

                    for device in &devices {
                        let buses: Vec<&UsbDevice> = machine
                            .usb_on(None)
                            .filter(|hub| hub.controller == Some(device.address))
                            .collect();
                        let on: Vec<&UsbDevice> = machine
                            .usb
                            .iter()
                            .filter(|d| d.controller == Some(device.address) && d.parent.is_some())
                            .collect();
                        let ports: u32 = buses.iter().map(|hub| hub.ports).sum();

                        fastest = fastest.max(buses.iter().filter_map(|hub| version(hub)).max());
                        spec.push((
                            short(device.address),
                            format!(
                                "{}, {}",
                                counted(buses.len(), "BUS", "BUSES"),
                                counted(ports as usize, "PORT", "PORTS")
                            ),
                        ));
                        spec.push((
                            "DEVICES".into(),
                            format!(
                                "{}, {}",
                                on.len(),
                                counted(on.iter().filter(|d| d.hub()).count(), "HUB", "HUBS")
                            ),
                        ));
                    }

                    let value = fastest.map_or("USB".into(), |(major, minor)| {
                        format!("USB {major}.{minor}")
                    });

                    ("USB", devices.len() as u32, value, ring)
                }
                Group::Bridge => {
                    for device in &devices {
                        let name = match device.kind() {
                            PciKind::Bridge => {
                                device.name.as_deref().map_or("PCI BRIDGE".into(), lettered)
                            }
                            _ => named(device),
                        };

                        spec.extend(rows(&short(device.address), &name));
                    }

                    ("PCIe BRIDGE", blocks.len() as u32, "PCIe".into(), ring)
                }
            }
        }
    };

    let mut part = Part::new(name, quantity, &fit(&value, 10)).detail(centre, radius);

    part.spec = spec.into_iter().take(SPEC_ROWS).collect();
    part
}

impl Subject for Topology {
    fn name(&self) -> &'static str {
        "topology"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        self.diagram.extent
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let diagram = &self.diagram;

        for wire in &diagram.wires {
            d.wire(wire);
        }

        for at in &diagram.junctions {
            d.junction(*at);
        }

        for legend in &diagram.legends {
            d.label(legend.at, legend.text.as_str())
                .anchor(legend.anchor)
                .tone(Tone::Muted);
        }

        d.part(self.index(Item::Processor), |d| self.package(d));

        if diagram.bank.is_some() {
            d.part(self.index(Item::Memory), |d| self.memory(d));
        }

        for block in &diagram.blocks {
            d.part(self.index(Item::Blocks(block.group)), |d| {
                self.block(d, block)
            });
        }

        d.moving(|d| self.traffic(d, t));

        // A balloon for each item, on the first of its blocks.
        let package = diagram.package.frame;

        d.part(self.index(Item::Processor), |d| {
            d.balloon(
                self.index(Item::Processor),
                v(package.min.x + 6.0, package.max.y),
                Auto,
            );
        });

        if let Some(bank) = &diagram.bank {
            d.part(self.index(Item::Memory), |d| {
                d.balloon(
                    self.index(Item::Memory),
                    v(bank.frame.min.x + 6.0, bank.frame.max.y),
                    Auto,
                );
            });
        }

        for block in diagram.blocks.iter().filter(|block| block.balloon) {
            let index = self.index(Item::Blocks(block.group));

            d.part(index, |d| {
                d.balloon(index, tip(block), Auto);
            });
        }
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let snapshot = self.machine.sample(t);
        let mut readings = Vec::new();

        if let Some(celsius) = self
            .temperature
            .and_then(|index| snapshot.sensor(index))
            .filter(|celsius| celsius.is_finite())
        {
            readings.push(Reading::new("CPU", format!("{celsius:.0} °C")));
        }

        let drives: Vec<_> = snapshot.drives.iter().flatten().collect();

        if !drives.is_empty() {
            let total = drives.iter().map(|d| d.read + d.written).sum();
            readings.push(Reading::new("DISK", rate(total)));
        }

        let interfaces: Vec<_> = snapshot.interfaces.iter().flatten().collect();

        if !interfaces.is_empty() {
            let total = interfaces.iter().map(|i| i.received + i.sent).sum();
            readings.push(Reading::new("NET", rate(total)));
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

    #[test]
    fn the_fixture_is_documented_part_by_part() {
        let topology = Topology::new(&Machine::fixture()).unwrap();
        let card = topology.card();
        let names: Vec<&str> = card.parts.iter().map(|p| p.name.as_str()).collect();

        assert_eq!(card.title, "LAPTOP TOPOLOGY");
        assert_eq!(
            names,
            [
                "CPU",
                "MEMORY",
                "GRAPHICS",
                "DISPLAY",
                "DRIVE",
                "NETWORK",
                "USB",
                "PCIe BRIDGE"
            ]
        );

        let values: Vec<&str> = card.parts.iter().map(|p| p.material.as_str()).collect();
        assert_eq!(
            values,
            [
                "16 CORES",
                "32 GiB",
                "3 OUTPUTS",
                "14+27 IN",
                "1 TB",
                "ETH, WI-FI",
                "USB 3.2",
                "PCIe"
            ]
        );

        let quantities: Vec<u32> = card.parts.iter().map(|p| p.quantity).collect();
        assert_eq!(quantities, [1, 2, 1, 2, 1, 2, 1, 2]);
    }

    /// The specification's rows fit the laptop's column, name and value
    /// apart, and there are no more of them than the column has room for.
    #[test]
    fn the_specifications_fit_their_column() {
        let topology = Topology::new(&Machine::fixture()).unwrap();

        for part in &topology.card().parts {
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

    /// Dots run toward the processor for what is read and away from it for
    /// what is written, more of them the busier the link, none when idle.
    #[test]
    fn traffic_runs_at_the_rate_measured() {
        let pieces = vec![vec![v(0.0, 0.0), v(200.0, 0.0)]];
        let count = |rate: f32| {
            let mut draft = Draft::new();
            dots(&mut draft, &pieces, rate, false, 10.0, Tone::Live);
            draft.marks().len()
        };

        assert_eq!(count(0.0), 0);
        assert!(count(1e5) > 0);
        assert!(count(1e7) > count(1e5));

        // On a wire shorter than the spacing, one dot: a moment later it
        // is further along its way, out or in.
        let short = vec![vec![v(0.0, 0.0), v(6.0, 0.0)]];
        let at = |inward: bool, t: f32| {
            let mut draft = Draft::new();
            dots(&mut draft, &short, BUSIEST, inward, t, Tone::Live);
            match &draft.marks()[0].ink {
                crate::draft::Ink::Dot { at, .. } => at.x,
                _ => unreachable!(),
            }
        };

        assert!(at(false, 10.075) > at(false, 10.025));
        assert!(at(true, 10.075) < at(true, 10.025));
    }

    /// The budget the diagram is folded to is the laptop's main view, so
    /// the laptop draws it at a pixel to a unit or more.
    #[test]
    fn the_budget_is_the_laptops_view() {
        use super::super::layout::BUDGET;
        use crate::headless::Output;
        use crate::sheet::layout::Layout;

        let topology = Topology::new(&Machine::fixture()).unwrap();
        let (width, height) = Output::LAPTOP.virtual_size();
        let view = Layout::new(width as i32, height as i32, topology.card()).view;

        assert_eq!(
            (view.width, view.height),
            (BUDGET.x as i32, BUDGET.y as i32)
        );
    }

    #[test]
    fn readings_are_the_live_values() {
        let topology = Topology::new(&Machine::fixture()).unwrap();
        let names: Vec<&str> = topology.readings(3.0).iter().map(|r| r.name).collect();

        assert_eq!(names, ["CPU", "DISK", "NET", "BATTERY"]);
    }
}
