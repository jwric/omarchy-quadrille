//! The machine as a block diagram: the CPU package with its cores, the
//! memory, what is on the PCIe buses (bridges, graphics, drives, network
//! adapters), the USB tree and the displays on the graphics card's
//! connectors. Traffic runs along the wires as dots, at the rates the
//! machine measures its drives and network adapters at.
use std::cell::RefCell;
use std::f32::consts::PI;
use std::rc::Rc;

use quadrille::draw::Anchor;

use crate::draft::Placement::Auto;
use crate::draft::{Draft, Extent, Fill, Line, Tone, V2, v};
use crate::machine::{
    Connector, ConnectorKind, CoreKind, Drive, DriveKind, Interface, Link, Machine, Panel,
    Partition, PciAddress, PciDevice, SensorKind, Site, UsbDevice,
};

use super::super::schematic::Schematic;
use super::super::{Card, Domain, Part, Reading, Revision, Room, Subject, Unit};
use super::layout::{
    BUDGET, Bank, DETAIL_LINE, Diagram, Die, Form, Gauge, Group, LINE, Package, Placed, Route,
    Source, bridged, named, short, version,
};
use super::{
    SPEC_ROWS, alike, binary, bits, counted, decimal, fit, flow_by, lettered, rate, rows, set,
};

/// A dot of traffic every so many units along a wire, running so fast.
const SPACING: f64 = 8.0;
const SPEED: f64 = 40.0;
/// The rates the dots run at, from a few to all of them: a kilobyte a
/// second to a hundred megabytes, by their logarithm.
const QUIETEST: f32 = 1e3;
const BUSIEST: f32 = 1e8;

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
    /// The topology folded for each view smaller than the laptop's that a
    /// sheet has asked for, if it reads the same there.
    fitted: RefCell<Vec<(V2, Option<Rc<Topology>>)>>,
}

impl Topology {
    /// The topology of `machine`, if it has a processor and something on
    /// its buses to draw.
    pub fn new(machine: &Machine) -> Option<Self> {
        Some(Self::of(machine, Diagram::new(machine)?))
    }

    /// The topology of `machine` drawn as `diagram`.
    fn of(machine: &Machine, diagram: Diagram) -> Self {
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
            .sensors_on(Site::Processor(0))
            .filter(|(_, sensor)| sensor.kind == SensorKind::Temperature)
            .min_by_key(|(_, sensor)| {
                !sensor
                    .label
                    .as_deref()
                    .is_some_and(|label| label.to_lowercase().contains("package"))
            })
            .map(|(index, _)| index);

        Self {
            card,
            machine: machine.clone(),
            diagram,
            items,
            temperature,
            fitted: RefCell::default(),
        }
    }

    /// The parts list's index of `item`.
    fn index(&self, item: Item) -> usize {
        self.items
            .iter()
            .position(|other| *other == item)
            .expect("Every item drawn is listed")
    }

    /// The processor: a die for each package, with its cores and last
    /// cache. It is set on the grid by its top right corner, where its wire
    /// leaves for the spine; each die by its top left corner inside it, and
    /// its cores a row at a time (see [`in_rows`]).
    fn package(&self, d: &mut Draft) {
        let package = &self.diagram.package;
        let cpu = self.machine.cpu.as_ref().expect("A diagram has a CPU");
        let frame = package.frame;
        let title = frame.max.y - 10.0;

        d.snapped(frame.max, |d| {
            d.rect(frame.min, frame.max, Line::Outline);
            d.label(v(frame.min.x + 6.0, title), "CPU")
                .anchor(Anchor::LEFT)
                .tone(Tone::Ink);
            d.label(
                v(frame.max.x - 6.0, title),
                counted(cpu.cores as usize, "CORE", "CORES"),
            )
            .anchor(Anchor::RIGHT);

            for (n, die) in package.dies.iter().enumerate() {
                d.snapped(top_left(die.frame), |d| self.die(d, die, n == 0));
            }
        });
    }

    /// A die of the processor, the first of them lettered for its detail.
    fn die(&self, d: &mut Draft, die: &Die, first: bool) {
        let squares: Vec<Extent> = die.cores.iter().map(|core| core.square).collect();

        d.rect(die.frame.min, die.frame.max, Line::Thin);

        centred_rows(d, &squares, &mut |d, k| {
            let core = &die.cores[k];

            d.rect(core.square.min, core.square.max, Line::Outline);

            // The efficient cores tinted on the view; a detail letters each
            // core's kind, and a tint would show only round the letter.
            if core.kind == Some(CoreKind::Efficient) {
                d.in_main(|d| {
                    d.area(&corners(core.square), Fill::Tint(4));
                });
            }
        });

        if let Some((band, text)) = &die.cache {
            d.rect(band.min, band.max, Line::Thin);
            d.label(band.centre(), text.as_str());
        }

        // What the detail of the first die has room to say: the kind of
        // every core where there are two kinds, or how many cores a square
        // stands for, and the cache. The laptop's window holds the cores
        // and cache alone, so the processor in full is the specification's.
        if !first {
            return;
        }

        d.in_own_detail(|d| {
            centred_rows(d, &squares, &mut |d, k| {
                let core = &die.cores[k];
                let letter = match (core.kind, core.cores) {
                    (_, cores) if cores > 1 => cores.to_string(),
                    (Some(CoreKind::Performance), _) => "P".into(),
                    (Some(CoreKind::Efficient), _) => "E".into(),
                    (None, _) => return,
                };

                d.label(core.square.centre(), letter).tone(Tone::Faint);
            });

            if let Some((band, text)) = &die.cache {
                d.label(band.centre(), format!("{text} CACHE"));
            }
        });
    }

    /// The memory: its modules as sticks, and its bus to the package. The
    /// bank is set on the grid by its bottom left corner, where the bus
    /// leaves it, and its sticks a row at a time.
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

        d.snapped(frame.min, |d| {
            d.rect(frame.min, frame.max, Line::Outline);
            d.label(v(frame.min.x + 6.0, title), "MEMORY")
                .anchor(Anchor::LEFT)
                .tone(Tone::Ink);
            d.label(v(frame.max.x - 6.0, title), size.as_str())
                .anchor(Anchor::RIGHT);

            // Each stick, and the memory chips on it in a detail, as many
            // as it has room for.
            centred_rows(d, &bank.sticks, &mut |d, k| {
                let stick = bank.sticks[k];

                d.rect(stick.min, stick.max, Line::Outline);
                d.in_detail(|d| {
                    let chips = ((stick.width() - 2.0) / 9.0).floor();
                    let left = stick.min.x + (stick.width() - (chips * 9.0 - 2.0)) / 2.0;

                    for chip in 0..chips as usize {
                        let x = left + chip as f32 * 9.0;

                        d.rect(
                            v(x, stick.min.y + 2.0),
                            v(x + 7.0, stick.max.y - 2.0),
                            Line::Thin,
                        );
                    }
                });
            });

            // In its own detail, the bank in full: over its sticks, or in
            // its middle where the kernel shows no module.
            d.in_own_detail(|d| match &bank.generation {
                _ if bank.sticks.is_empty() => {
                    d.label(frame.centre(), format!("MEMORY {size}, MODULES NOT SHOWN"))
                        .tone(Tone::Ink);
                }
                generation => {
                    let what: Vec<&str> = ["MEMORY", &size]
                        .into_iter()
                        .chain(generation.as_deref())
                        .collect();

                    d.label(v(frame.min.x + 3.0, title), what.join(" "))
                        .anchor(Anchor::LEFT)
                        .tone(Tone::Ink);
                }
            });
        });

        let (from, to) = bank.bus;

        for side in [-2.0, 2.0] {
            d.line(from + v(side, 0.0), to + v(side, 0.0), Line::Outline);
        }

        if let Some(generation) = &bank.generation {
            d.label(from.lerp(to, 0.5) + v(6.0, 0.0), generation.as_str())
                .anchor(Anchor::LEFT);
        }
    }

    /// A block of the diagram, by its form.
    fn block(&self, d: &mut Draft, block: &Placed) {
        let frame = block.frame;

        match block.form {
            Form::Block => {
                d.rect(frame.min, frame.max, Line::Outline);
            }
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
                    );
                } else {
                    // The stand set on the grid by the top of its neck, so
                    // its foot is as long either side of it.
                    let neck = v(frame.centre().x.round(), foot);

                    d.snapped(neck, |d| {
                        d.line(neck, neck - v(0.0, 4.0), Line::Outline);
                        d.line(neck - v(12.0, 5.0), neck + v(12.0, -5.0), Line::Outline);
                    });
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
                self.in_its_detail(d, block, |d| {
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

        self.in_its_detail(d, block, |d| self.detail(d, block));
    }

    /// What the detail of `block` shows that the view has no room for: its
    /// lettering in full and, for what has more to it, a drawing of that
    /// under it.
    fn detail(&self, d: &mut Draft, block: &Placed) {
        let frame = block.frame;
        let plate = plate(frame);
        // A screen's lettering clears its glass.
        let first = match block.form {
            Form::Screen { .. } => frame.max.y - 7.0,
            _ => frame.max.y - 5.0,
        };

        let outputs: Vec<&Connector> = match (block.group, block.source) {
            (Group::Graphics, Source::Pci(address)) => self
                .machine
                .connectors
                .iter()
                .filter(|c| c.gpu == Some(address))
                .collect(),
            _ => Vec::new(),
        };
        let symbol = block.group == Group::Network && frame.height() >= 2.0 * LINE + 8.0;
        // As many characters as the plate has room for at twice the view's
        // scale, three units each: a screen's short of the dimension down
        // its glass, an adapter's of its jack or antenna.
        let right = match block.form {
            Form::Screen { .. } => across(frame) - 6.0,
            _ if symbol => plate.max.x - 14.0,
            _ => plate.max.x,
        };
        let room = ((right - plate.min.x) / 3.0).floor() as usize;
        // A graphics card whose outputs' names take two rows under them
        // has its name alone over them.
        let lines = if ports_in_one_row(plate, &outputs) {
            block.more.len()
        } else {
            1
        };

        for (k, line) in block.more.iter().take(lines).enumerate() {
            d.label(
                v(plate.min.x, first - DETAIL_LINE * k as f32),
                fit(line, room),
            )
            .anchor(Anchor::LEFT)
            .tone(if k == 0 { Tone::Ink } else { Tone::Muted });
        }

        // Under its lines of lettering.
        let under = first - DETAIL_LINE * (lines as f32 - 0.5) - 1.0;

        match (block.form, block.source) {
            (Form::Screen { .. }, Source::Connector(index)) => {
                if let Some(panel) = self
                    .machine
                    .connectors
                    .get(index)
                    .and_then(|c| c.panel.as_ref())
                {
                    screen(d, frame, panel);
                }
            }
            (Form::Block, Source::Drive(index)) => {
                if let Some(drive) = self.machine.drives.get(index) {
                    partitions(
                        d,
                        Extent::new(
                            v(plate.min.x, frame.min.y + 4.0),
                            v(plate.max.x, under.min(frame.min.y + 13.0)),
                        ),
                        drive,
                    );
                }
            }
            (Form::Block, Source::Pci(address)) => match block.group {
                Group::Graphics => {
                    ports(
                        d,
                        Extent::new(v(plate.min.x, frame.min.y + 2.0), v(plate.max.x, under)),
                        &outputs,
                    );
                }
                Group::Usb => {
                    let hubs: Vec<&UsbDevice> = self
                        .machine
                        .usb_on(None)
                        .filter(|hub| hub.controller == Some(address))
                        .collect();

                    plugs(
                        d,
                        &self.machine,
                        Extent::new(v(plate.min.x, frame.min.y + 2.0), v(plate.max.x, under)),
                        &hubs,
                    );
                }
                Group::Network if symbol => {
                    let wireless = self
                        .machine
                        .interfaces
                        .iter()
                        .find(|i| i.pci == Some(address) && !i.usb)
                        .is_some_and(|i| i.link == Link::Wireless);
                    let side = v(plate.max.x - 7.0, frame.centre().y);

                    if wireless {
                        antenna(d, side);
                    } else {
                        jack(d, side);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// Records what `draw` makes for the detail of `block`'s part, if that
    /// detail is of `block`: the first block of a part, which carries its
    /// balloon and which its detail is centred on. Another block in the
    /// detail's window is shown without, so nothing is lettered across the
    /// window's edge.
    fn in_its_detail(&self, d: &mut Draft, block: &Placed, draw: impl FnOnce(&mut Draft)) {
        if block.balloon {
            d.in_own_detail(draw);
        }
    }

    /// In the network adapter's detail, under its lettering, what it
    /// receives and sends at `t`.
    fn rates(&self, d: &mut Draft, t: f32) {
        let Some(block) = self
            .diagram
            .blocks
            .iter()
            .find(|block| block.group == Group::Network && block.balloon)
            .filter(|block| block.frame.height() >= 2.0 * LINE + 8.0)
        else {
            return;
        };
        let snapshot = self.machine.sample(t);
        let (received, sent) = std::iter::once(&block.source)
            .chain(&block.merged)
            .filter_map(|source| match source {
                Source::Pci(address) => Some(*address),
                _ => None,
            })
            .flat_map(|address| {
                self.machine
                    .interfaces
                    .iter()
                    .enumerate()
                    .filter(move |(_, i)| i.pci == Some(address) && !i.usb)
                    .map(|(index, _)| index)
            })
            .filter_map(|index| snapshot.interfaces.get(index).copied().flatten())
            .fold((0.0, 0.0), |(received, sent), traffic| {
                (received + traffic.received, sent + traffic.sent)
            });
        let line = block.frame.max.y - 5.0 - DETAIL_LINE * block.more.len() as f32;

        d.part(self.index(Item::Blocks(Group::Network)), |d| {
            set(d, &block.snaps, |d| {
                d.in_own_detail(|d| {
                    d.label(
                        v(plate(block.frame).min.x, line),
                        format!("RX {:.1} TX {}", received.max(0.0) / 1e6, megabytes(sent)),
                    )
                    .anchor(Anchor::LEFT)
                    .tone(Tone::Live);
                });
            });
        });
    }

    /// The traffic on every route, at the rates measured at `t`.
    fn traffic(&self, d: &mut Draft, t: f32) {
        let snapshot = self.machine.sample(t);

        for route in &self.diagram.routes {
            let (inward, outward) = route
                .gauges
                .iter()
                .filter_map(|gauge| match *gauge {
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
                })
                .fold((0.0, 0.0), |(inward, outward), (into, out)| {
                    (inward + into, outward + out)
                });

            dots(d, route, inward, true, t, Tone::Live);
            dots(d, route, outward, false, t, Tone::Accent);
        }
    }
}

/// The widest a detail's drawing and lettering are: what the laptop's
/// detail window shows of a block at twice the view's scale, the least a
/// detail magnifies, with a little to spare.
const PLATE: f32 = 100.0;

/// Where a detail of a block framed by `frame` draws: inside it, across its
/// middle, no wider than [`PLATE`].
fn plate(frame: Extent) -> Extent {
    let half = ((frame.width() - 8.0) / 2.0).min(PLATE / 2.0);
    let middle = frame.centre().x;

    Extent::new(
        v((middle - half).round(), frame.min.y + 2.0),
        v((middle + half).round(), frame.max.y - 2.0),
    )
}

/// Where the dimension down a display's glass runs, in the detail of a
/// screen framed by `frame`: clear of the glass's edge by its value.
fn across(frame: Extent) -> f32 {
    plate(frame).max.x - 8.0
}

/// A display's glass in its detail, framed by `frame`: its pixels
/// dimensioned across it and down it.
fn screen(d: &mut Draft, frame: Extent, panel: &Panel) {
    let glass = Extent::new(frame.min + v(2.0, 2.0), frame.max - v(2.0, 2.0));
    let low = glass.min.y + 4.0;
    let x = across(frame);

    d.dim_h(v(glass.min.x, low), v(glass.max.x, low), 0.0)
        .text(format!("{} PX", panel.pixels.0));
    d.dim_v(v(x, glass.min.y), v(x, glass.max.y), 0.0)
        .text(panel.pixels.1.to_string());
}

/// The least a partition is drawn across, however small: room for its
/// number and the ground it clears at twice the view's scale.
const SLIVER: f32 = 7.0;

/// A drive's partitions as a bar across `bar`, in their order on it: each
/// as long as its share of the drive, a sliver at the least, lettered with
/// its number and filesystem as far as there is room; the space no
/// partition takes hatched.
fn partitions(d: &mut Draft, bar: Extent, drive: &Drive) {
    let total = drive.bytes.max(1);
    let mut parts: Vec<&Partition> = drive.partitions.iter().collect();
    let mut pieces: Vec<(Option<&Partition>, u64)> = Vec::new();
    let mut end = 0;

    parts.sort_by_key(|part| part.start);

    // Space between partitions, past the alignment they leave.
    for part in parts {
        if part.start > end + total / 100 {
            pieces.push((None, part.start - end));
        }

        pieces.push((Some(part), part.bytes));
        end = end.max(part.start + part.bytes);
    }

    if total > end + total / 100 {
        pieces.push((None, total - end));
    }

    let width = bar.width();
    let mut lengths: Vec<f32> = pieces
        .iter()
        .map(|&(part, bytes)| {
            let share = (bytes as f64 / total as f64) as f32 * width;

            match part {
                Some(_) => share.max(SLIVER),
                None => share,
            }
        })
        .collect();

    // What the slivers take, from the longest.
    let over: f32 = lengths.iter().sum::<f32>() - width;

    if let Some(longest) = lengths.iter_mut().max_by(|a, b| a.total_cmp(b)) {
        *longest -= over;
    }

    d.rect(bar.min, bar.max, Line::Outline);

    let mut left = bar.min.x;

    for (k, ((part, _), length)) in pieces.iter().zip(&lengths).enumerate() {
        let right = if k + 1 == pieces.len() {
            bar.max.x
        } else {
            (left + length).round()
        };

        if k > 0 {
            d.line(v(left, bar.min.y), v(left, bar.max.y), Line::Outline);
        }

        match part {
            None => {
                d.hatch(&corners(Extent::new(
                    v(left, bar.min.y),
                    v(right, bar.max.y),
                )));
            }
            Some(part) => {
                let number = part.number.to_string();
                let filesystem = part.filesystem.as_deref().map(str::to_uppercase);
                let mut names = Vec::new();

                if let Some(filesystem) = &filesystem {
                    if part.mapped {
                        names.push(format!("{number} {filesystem} ON DM"));
                    }
                    names.push(format!("{number} {filesystem}"));
                }
                names.push(number);

                // A character is three units across at twice the view's
                // scale, and its ground a little more.
                if let Some(name) = names
                    .into_iter()
                    .find(|name| name.chars().count() as f32 * 3.0 + 4.0 <= right - left)
                {
                    d.label(v((left + right) / 2.0, bar.centre().y), name);
                }
            }
        }

        left = right;
    }

    if pieces.is_empty() {
        d.label(bar.centre(), "NO PARTITIONS");
    }
}

/// The outline of a graphics card's socket of `kind`, ten units across
/// and six high round `at`: DisplayPort's with a corner cut, HDMI's
/// narrowing at the foot, DVI's and VGA's tapered, the machine's own
/// panel's flat ribbon.
fn socket(kind: ConnectorKind, at: V2, scale: f32) -> Vec<V2> {
    let points: &[(f32, f32)] = match kind {
        ConnectorKind::DisplayPort => &[
            (-5.0, 3.0),
            (5.0, 3.0),
            (5.0, -3.0),
            (-3.0, -3.0),
            (-5.0, -1.0),
        ],
        ConnectorKind::Hdmi => &[
            (-5.0, 3.0),
            (5.0, 3.0),
            (5.0, 0.0),
            (3.5, -3.0),
            (-3.5, -3.0),
            (-5.0, 0.0),
        ],
        ConnectorKind::Dvi => &[
            (-5.0, 3.0),
            (5.0, 3.0),
            (5.0, -1.0),
            (4.0, -3.0),
            (-4.0, -3.0),
            (-5.0, -1.0),
        ],
        ConnectorKind::Vga => &[(-5.0, 3.0), (5.0, 3.0), (4.0, -3.0), (-4.0, -3.0)],
        ConnectorKind::Internal => &[(-5.0, 1.5), (5.0, 1.5), (5.0, -1.5), (-5.0, -1.5)],
        ConnectorKind::Other => &[(-5.0, 3.0), (5.0, 3.0), (5.0, -3.0), (-5.0, -3.0)],
    };

    points.iter().map(|&(x, y)| at + v(x, y) * scale).collect()
}

/// How far apart a graphics card's outputs are across `room`.
fn port_pitch(room: Extent, outputs: &[&Connector]) -> f32 {
    (room.width() / outputs.len().max(1) as f32)
        .min(40.0)
        .floor()
}

/// Whether the names of a graphics card's outputs each fit under its
/// socket, across the plate of its detail, at twice the view's scale: three
/// units a character, and the ground each clears.
fn ports_in_one_row(plate: Extent, outputs: &[&Connector]) -> bool {
    let pitch = port_pitch(plate, outputs);

    outputs
        .iter()
        .all(|connector| connector.name.chars().count() as f32 * 3.0 + 4.0 <= pitch)
}

/// A graphics card's outputs across `room`, as the sockets on its bracket:
/// each by its kind, filled where a display is on it, its name under it,
/// the names in two rows where one has no room for them.
fn ports(d: &mut Draft, room: Extent, outputs: &[&Connector]) {
    if outputs.is_empty() {
        return;
    }

    let n = outputs.len() as f32;
    let pitch = port_pitch(room, outputs);
    let left = (room.centre().x - pitch * n / 2.0).round();
    // A name's ground reaches three units over and under it at twice the
    // view's scale, so a socket stands that far and half a unit over it.
    let (sockets, names) = if ports_in_one_row(room, outputs) {
        (room.min.y + 9.5, [room.min.y + 3.0; 2])
    } else {
        (room.min.y + 15.5, [room.min.y + 9.0, room.min.y + 3.0])
    };
    let scale = ((pitch - 2.0) / 10.0).min(1.0);

    let middles: Vec<V2> = (0..outputs.len())
        .map(|k| v(left + pitch * (k as f32 + 0.5), sockets))
        .collect();

    // Each socket set on the grid by its middle, so it is symmetric, and
    // each a pitch on from the one before.
    in_rows(d, &middles, &mut |d, k| {
        let connector = outputs[k];
        let middle = middles[k];
        let outline = socket(connector.kind, middle, scale);

        if connector.panel.is_some() {
            d.area(&outline, Fill::Tint(8));
        }

        d.polygon(&outline, Line::Outline);
        d.label(v(middle.x, names[k % 2]), connector.name.as_str());
    });
}

/// A USB device's port on the hub it is on: `1-4` is the fourth.
fn root_port(device: &UsbDevice) -> Option<u32> {
    device
        .port
        .rsplit('-')
        .next()?
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// A USB host's root hubs across `room`, a row each: the bus's version and
/// a socket for each of its ports, filled where a device is on it.
fn plugs(d: &mut Draft, machine: &Machine, room: Extent, hubs: &[&UsbDevice]) {
    let rows: Vec<&&UsbDevice> = hubs.iter().take(2).collect();
    let label = 3.0 * "USB 0.0".len() as f32 + 3.0;
    let most = rows.iter().map(|hub| hub.ports).max().unwrap_or(0).max(1) as f32;
    let pitch = ((room.width() - label) / most).min(6.0);
    let middle = room.centre().y.round();

    for (k, hub) in rows.iter().enumerate() {
        let y = middle + (rows.len() as f32 - 1.0) * 3.5 - 7.0 * k as f32;
        let used: Vec<u32> = machine
            .usb_on(Some(&hub.port))
            .filter_map(root_port)
            .collect();
        let name = version(hub).map_or("USB".into(), |(major, minor)| {
            format!("USB {major}.{minor}")
        });

        d.label(v(room.min.x, y), name).anchor(Anchor::LEFT);

        let sockets: Vec<Extent> = (0..hub.ports)
            .map(|port| {
                let x = room.min.x + label + pitch * port as f32;

                Extent::new(v(x, y - 1.5), v(x + pitch - 1.5, y + 1.5))
            })
            .collect();
        let corners_of: Vec<V2> = sockets.iter().copied().map(top_left).collect();

        in_rows(d, &corners_of, &mut |d, port| {
            let socket = sockets[port];

            if used.contains(&(port as u32 + 1)) {
                d.area(&corners(socket), Fill::Tint(8));
            }

            d.rect(socket.min, socket.max, Line::Outline);
        });
    }
}

/// A wireless adapter's antenna standing on `at`: a mast on its foot, the
/// waves either side of its tip.
fn antenna(d: &mut Draft, at: V2) {
    let tip = at + v(0.0, 2.0);

    d.line(at - v(0.0, 4.0), tip, Line::Outline);
    d.polygon(
        &[at - v(2.5, 5.0), at + v(2.5, -5.0), at - v(0.0, 2.5)],
        Line::Outline,
    );

    for radius in [2.5, 4.5] {
        for start in [-0.7, PI - 0.7] {
            d.arc(tip, radius, start, 1.4, Line::Outline);
        }
    }
}

/// An Ethernet adapter's jack, seen from the front, round `at`: the socket
/// with the notch its plug's latch goes in, and its contacts.
fn jack(d: &mut Draft, at: V2) {
    let outline: Vec<V2> = [
        (-5.0, 4.0),
        (5.0, 4.0),
        (5.0, -2.0),
        (2.0, -2.0),
        (2.0, -4.0),
        (-2.0, -4.0),
        (-2.0, -2.0),
        (-5.0, -2.0),
    ]
    .iter()
    .map(|&(x, y)| at + v(x, y))
    .collect();

    d.polygon(&outline, Line::Outline);

    for x in [-3.0, -1.0, 1.0, 3.0] {
        d.line(at + v(x, 4.0), at + v(x, 2.0), Line::Thin);
    }
}

/// Bytes a second in megabytes, to a tenth.
fn megabytes(bytes: f32) -> String {
    let bytes = if bytes.is_finite() {
        bytes.max(0.0)
    } else {
        0.0
    };

    format!("{:.1} MB/s", bytes / 1e6)
}

/// The top left corner of `extent`, which what it frames is set on the grid
/// by.
fn top_left(extent: Extent) -> V2 {
    v(extent.min.x, extent.max.y)
}

/// Draws `draw` for each of `points`, set on the grid by it, a row at a
/// time (points level with each other): each by the one before it in its
/// row, and the first of a row by the first of the row over it. So points
/// as far apart in units as others are as far apart in pixels wherever
/// they fall, and rows of them line up.
fn in_rows(d: &mut Draft, points: &[V2], draw: &mut impl FnMut(&mut Draft, usize)) {
    fn down<F: FnMut(&mut Draft, usize)>(d: &mut Draft, rows: &[Row], draw: &mut F) {
        if let Some((row, rest)) = rows.split_first() {
            d.snapped(row[0].1, |d| {
                along(d, row, draw);
                down(d, rest, draw);
            });
        }
    }

    down(d, &rows_of(points.iter().copied()), draw);
}

/// Draws `draw` for each of `squares`, rows of them centred on a middle,
/// set on the grid a row at a time from the row's middle out: those right
/// of it each by its top left corner from the one before, those left of it
/// each by its top right corner from the one after. So the squares of a
/// row are as far apart as each other in pixels, and as far in from either
/// end of what they are centred in.
fn centred_rows(d: &mut Draft, squares: &[Extent], draw: &mut impl FnMut(&mut Draft, usize)) {
    fn down<F: FnMut(&mut Draft, usize)>(
        d: &mut Draft,
        squares: &[Extent],
        rows: &[Row],
        draw: &mut F,
    ) {
        let Some((row, rest)) = rows.split_first() else {
            return;
        };
        let first = squares[row[0].0];
        let last = squares[row[row.len() - 1].0];
        let middle = v((first.min.x + last.max.x) / 2.0, first.max.y);
        let (left, right): (Row, Row) = row
            .iter()
            .map(|&(k, _)| (k, squares[k]))
            .map(|(k, square)| {
                if square.centre().x < middle.x {
                    (k, v(square.max.x, square.max.y))
                } else {
                    (k, v(square.min.x, square.max.y))
                }
            })
            .partition(|&(k, _)| squares[k].centre().x < middle.x);

        d.snapped(middle, |d| {
            let left: Row = left.into_iter().rev().collect();

            for side in [&left, &right] {
                if let Some(&(_, first)) = side.first() {
                    d.snapped(first, |d| along(d, side, draw));
                }
            }

            down(d, squares, rest, draw);
        });
    }

    let rows = rows_of(squares.iter().map(|square| v(square.min.x, square.max.y)));

    down(d, squares, &rows, draw);
}

/// A row of points, each with its index.
type Row = Vec<(usize, V2)>;

/// `points` in rows of those level with each other, each with its index.
fn rows_of(points: impl Iterator<Item = V2>) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();

    for (k, point) in points.enumerate() {
        match rows.last_mut() {
            Some(row) if row[0].1.y == point.y => row.push((k, point)),
            _ => rows.push(vec![(k, point)]),
        }
    }

    rows
}

/// Draws `draw` for each of `row`, the first in the figure set by its
/// point already and each after in one set by its point inside the one
/// before.
fn along<F: FnMut(&mut Draft, usize)>(d: &mut Draft, row: &[(usize, V2)], draw: &mut F) {
    if let Some((&(k, _), rest)) = row.split_first() {
        draw(d, k);

        if let Some(&(_, next)) = rest.first() {
            d.snapped(next, |d| along(d, rest, draw));
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

/// Traffic along `route` at `rate` bytes a second, toward the processor
/// if `inward`: dots a set distance apart running at a set speed, more of
/// them on a busier link, each set on the grid as the wire it is on.
fn dots(d: &mut Draft, route: &Route, rate: f32, inward: bool, t: f32, tone: Tone) {
    let share =
        ((rate.max(1.0).log10() - QUIETEST.log10()) / (BUSIEST / QUIETEST).log10()).clamp(0.0, 1.0);

    flow_by(
        d,
        &route.pieces,
        share,
        SPACING,
        f64::from(t) * SPEED,
        inward,
        |d, piece, at| {
            set(d, &route.snaps[piece], |d| {
                d.dot(at, 3).tone(tone);
            });
        },
    );
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

/// The circle a detail of the processor magnifies: the cores and the cache
/// of its first die, which is all the laptop's short detail window has
/// room for at twice the view's scale.
fn processor_ring(package: &Package) -> (V2, f32) {
    let die = &package.dies[0];
    let mut top = die.frame.min.y;
    let mut bottom = die.frame.max.y;

    for extent in die
        .cores
        .iter()
        .map(|core| &core.square)
        .chain(die.cache.as_ref().map(|(band, _)| band))
    {
        top = top.max(extent.max.y);
        bottom = bottom.min(extent.min.y);
    }

    (
        v(die.frame.centre().x, ((top + bottom) / 2.0).round()),
        (die.frame.width() / 2.0 + 2.0).round(),
    )
}

/// The circle a detail of the memory magnifies: the bank, or where it is
/// taller than the laptop's short detail window holds at twice the view's
/// scale, its lettering and its first rows of sticks.
fn memory_ring(bank: &Bank) -> (V2, f32) {
    let frame = bank.frame;
    let middle = frame.centre().y.max(frame.max.y - 22.0);

    (
        v(frame.centre().x, middle.round()),
        (frame.width() / 2.0 + 2.0).round(),
    )
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
                [] if cpu.packages > 1 => format!(
                    "{} ({} × {})",
                    cpu.cores,
                    cpu.packages,
                    cpu.cores / cpu.packages
                ),
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

            (
                "CPU",
                cpu.packages.max(1),
                counted(cpu.cores as usize, "CORE", "CORES"),
                processor_ring(&diagram.package),
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
                memory_ring(bank),
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
                    // Every display on the graphics drawn, whether its block
                    // is drawn or counted.
                    let gpus: Vec<PciAddress> = diagram
                        .blocks
                        .iter()
                        .filter(|block| block.group == Group::Graphics)
                        .filter_map(|block| match block.source {
                            Source::Pci(address) => Some(address),
                            _ => None,
                        })
                        .collect();
                    let panels: Vec<(&str, &Panel)> = machine
                        .connectors
                        .iter()
                        .filter(|c| c.gpu.is_some_and(|gpu| gpus.contains(&gpu)))
                        .filter_map(|c| Some((c.name.as_str(), c.panel.as_ref()?)))
                        .collect();
                    // The diagonals measured: a size guessed from the pixels
                    // alone is not given.
                    let inches: Vec<f64> = panels
                        .iter()
                        .filter(|(_, panel)| !panel.size.estimated)
                        .map(|(_, panel)| panel.inches())
                        .collect();

                    for (name, panel) in &panels {
                        let refresh = panel.refresh.map(|hz| format!("{} Hz", hz.round()));
                        let pixels = format!("{} × {}", panel.pixels.0, panel.pixels.1);
                        let (size, pitch) = if panel.size.estimated {
                            (pixels, "SIZE UNKNOWN".to_owned())
                        } else {
                            (
                                format!("{:.1}\" {pixels}", panel.inches()),
                                format!("PITCH {:.3} mm", panel.pitch_mm()),
                            )
                        };
                        let second: Vec<String> = refresh.into_iter().chain([pitch]).collect();

                        spec.push(((*name).to_owned(), size));
                        spec.push((String::new(), second.join(", ")));
                    }

                    let displays = panels.len();
                    let sizes: Vec<String> = inches
                        .iter()
                        .map(|inches| inches.round().to_string())
                        .collect();
                    let value = match sizes.as_slice() {
                        _ if inches.len() < displays => counted(displays, "DISPLAY", "DISPLAYS"),
                        [_] => format!("{:.1} IN", inches[0]),
                        _ if sizes.join("+").len() + 3 <= 10 => format!("{} IN", sizes.join("+")),
                        _ => format!(
                            "≤ {} IN",
                            inches.iter().copied().fold(0.0, f64::max).round()
                        ),
                    };

                    ("DISPLAY", displays as u32, value, ring)
                }
                Group::Drive => {
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

                            // Drives alike drawn as one are one entry.
                            let (name, each) = match block.merged.len() {
                                0 => (drive.name.clone(), ""),
                                more => (format!("{} +{more}", drive.name), " EACH"),
                            };

                            spec.extend(rows(&name, model.as_deref().unwrap_or("")));
                            spec.push((
                                String::new(),
                                format!(
                                    "{}{each}, {}",
                                    decimal(drive.bytes),
                                    counted(drive.partitions.len(), "PARTITION", "PARTITIONS")
                                ),
                            ));

                            if !filesystems.is_empty() {
                                spec.extend(rows("", &filesystems.join(", ")));
                            }
                        }
                    }

                    // Every drive on the machine's buses: those counted in
                    // a block as well as those drawn.
                    let drives: Vec<&Drive> = machine
                        .drives
                        .iter()
                        .filter(|drive| {
                            drive.kind != DriveKind::Usb
                                && drive
                                    .pci
                                    .is_some_and(|address| machine.pci_device(address).is_some())
                        })
                        .collect();

                    (
                        "DRIVE",
                        drives.len() as u32,
                        decimal(drives.iter().map(|drive| drive.bytes).sum()),
                        ring,
                    )
                }
                Group::Network => {
                    let mut links = Vec::new();

                    for block in &blocks {
                        let functions: Vec<&PciDevice> = std::iter::once(&block.source)
                            .chain(&block.merged)
                            .filter_map(|source| match source {
                                Source::Pci(address) => machine.pci_device(*address),
                                _ => None,
                            })
                            .collect();
                        let Some(device) = functions.first() else {
                            continue;
                        };
                        let interfaces: Vec<Option<&Interface>> = functions
                            .iter()
                            .map(|function| {
                                machine
                                    .interfaces
                                    .iter()
                                    .find(|i| i.pci == Some(function.address) && !i.usb)
                            })
                            .collect();
                        let up = interfaces.iter().flatten().filter(|i| i.up).count();
                        let state = match interfaces.as_slice() {
                            [
                                Some(Interface {
                                    up: true,
                                    speed: Some(speed),
                                    ..
                                }),
                            ] => format!("{}, ", bits(*speed as f32)),
                            [Some(Interface { up: true, .. })] => "UP, ".into(),
                            [Some(Interface { up: false, .. })] => "DOWN, ".into(),
                            [None] => String::new(),
                            all => format!("{up} OF {} UP, ", all.len()),
                        };
                        let link = interfaces[0].map(|i| i.link);
                        let kind = match link {
                            Some(Link::Wireless) => "WI-FI",
                            Some(Link::Ethernet) => "ETHERNET",
                            _ => "NETWORK",
                        };
                        let (kind, address) = match functions.len() {
                            1 => (kind.to_owned(), short(device.address)),
                            n => (
                                format!("{kind} ×{n}"),
                                format!("{} +{}", short(device.address), n - 1),
                            ),
                        };

                        links.extend(interfaces.iter().map(|i| i.map(|i| i.link)));
                        spec.extend(rows(&kind, &named(device)));
                        spec.push((String::new(), format!("{state}PCI {address}")));
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
                    for block in &blocks {
                        let Some(device) = pci(block) else {
                            continue;
                        };
                        let name = bridged(device);
                        let address = match block.merged.len() {
                            0 => short(device.address),
                            more => format!("{} +{more}", short(device.address)),
                        };

                        spec.extend(rows(&address, &name));
                    }

                    let ports = blocks
                        .iter()
                        .map(|block| 1 + block.merged.len())
                        .sum::<usize>();

                    ("PCIe BRIDGE", ports as u32, "PCIe".into(), ring)
                }
            }
        }
    };

    // What its detail must hold in full: the cores and cache of the first
    // die, the bank's lettering and first rows, a block's plate, a bridge
    // and its lettering over it.
    let holds = match item {
        Item::Processor => {
            let die = &diagram.package.dies[0];
            let (middle, _) = processor_ring(&diagram.package);
            let half = (middle.y
                - die
                    .cores
                    .iter()
                    .map(|core| core.square.min.y)
                    .chain(die.cache.as_ref().map(|(band, _)| band.min.y))
                    .fold(f32::MAX, f32::min))
            .abs();

            Extent::new(
                v(die.frame.min.x, middle.y - half),
                v(die.frame.max.x, middle.y + half),
            )
        }
        Item::Memory => {
            let frame = diagram.bank.as_ref().expect("Drawn with memory").frame;

            Extent::new(
                v(frame.min.x, frame.min.y.max(frame.max.y - 44.0)),
                frame.max,
            )
        }
        Item::Blocks(_) => {
            let first = blocks
                .iter()
                .find(|block| block.balloon)
                .unwrap_or(&blocks[0]);
            let frame = first.frame;

            match first.form {
                Form::Bridge => Extent::new(
                    v(frame.centre().x - 24.0, frame.min.y),
                    v(
                        frame.centre().x + 24.0,
                        frame.max.y + 8.0 + DETAIL_LINE * first.more.len() as f32,
                    ),
                ),
                _ => {
                    let plate = plate(frame);

                    Extent::new(v(plate.min.x, frame.min.y), v(plate.max.x, frame.max.y))
                }
            }
        }
    };
    let mut part = Part::new(name, quantity, &fit(&value, 10))
        .detail(centre, radius)
        .holding(holds);

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

    /// In a view smaller than the laptop's, the topology folded to fit it,
    /// so its lettering stays inside its blocks: where that keeps the same
    /// parts, notes and room for the specification.
    fn fitted(&self, room: Room) -> Option<Rc<dyn Subject>> {
        let budget = room.view.min(BUDGET);

        if budget == BUDGET {
            return None;
        }

        let mut fitted = self.fitted.borrow_mut();

        if let Some((_, topology)) = fitted.iter().find(|(made, _)| *made == budget) {
            return topology.clone().map(|topology| topology as Rc<dyn Subject>);
        }

        let topology = Diagram::within(&self.machine, budget)
            .map(|diagram| Self::of(&self.machine, diagram))
            .filter(|topology| alike(&self.card, &topology.card))
            .map(Rc::new);

        // A view for each output the sheet is drawn on, and a few more for
        // one that changes size.
        if fitted.len() >= 8 {
            fitted.remove(0);
        }

        fitted.push((budget, topology.clone()));
        topology.map(|topology| topology as Rc<dyn Subject>)
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        let diagram = &self.diagram;

        for wire in &diagram.wires {
            set(d, &wire.snaps, |d| d.wire(&wire.points));
        }

        for junction in &diagram.junctions {
            set(d, &junction.snaps, |d| d.junction(junction.at));
        }

        for legend in &diagram.legends {
            set(d, &legend.snaps, |d| {
                d.label(legend.at, legend.text.as_str())
                    .anchor(legend.anchor)
                    .tone(Tone::Muted);
            });
        }

        d.part(self.index(Item::Processor), |d| self.package(d));

        if diagram.bank.is_some() {
            d.part(self.index(Item::Memory), |d| self.memory(d));
        }

        for block in &diagram.blocks {
            d.part(self.index(Item::Blocks(block.group)), |d| {
                set(d, &block.snaps, |d| self.block(d, block));
            });
        }

        d.moving(|d| {
            self.traffic(d, t);
            self.rates(d, t);
        });

        // A balloon for each item, on the first of its blocks.
        let package = diagram.package.frame;

        d.part(self.index(Item::Processor), |d| {
            d.snapped(package.max, |d| {
                d.balloon(
                    self.index(Item::Processor),
                    v(package.min.x + 6.0, package.max.y),
                    Auto,
                );
            });
        });

        if let Some(bank) = &diagram.bank {
            d.part(self.index(Item::Memory), |d| {
                d.snapped(bank.frame.min, |d| {
                    d.balloon(
                        self.index(Item::Memory),
                        v(bank.frame.min.x + 6.0, bank.frame.max.y),
                        Auto,
                    );
                });
            });
        }

        for block in diagram.blocks.iter().filter(|block| block.balloon) {
            let index = self.index(Item::Blocks(block.group));

            d.part(index, |d| {
                set(d, &block.snaps, |d| {
                    d.balloon(index, tip(block), Auto);
                });
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
        use super::super::SPEC_ROOM;

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
        let route = |end: f32| Route {
            gauges: Vec::new(),
            pieces: vec![vec![v(0.0, 0.0), v(end, 0.0)]],
            snaps: vec![Vec::new()],
        };
        let long = route(200.0);
        let count = |rate: f32| {
            let mut draft = Draft::new();
            dots(&mut draft, &long, rate, false, 10.0, Tone::Live);
            draft.marks().len()
        };

        assert_eq!(count(0.0), 0);
        assert!(count(1e5) > 0);
        assert!(count(1e7) > count(1e5));

        // On a wire shorter than the spacing, one dot: a moment later it
        // is further along its way, out or in.
        let short = route(6.0);
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

    /// The processor's detail on the laptop, at twice the view's scale in
    /// its short window, shows all its cores and its cache, two rows of
    /// efficient cores among them.
    #[test]
    fn the_processors_detail_fits_the_laptops_window() {
        use crate::headless::Output;
        use crate::machine::Cores;
        use crate::sheet::layout::{Layout, inset};

        let mut machine = Machine::fixture();
        let cpu = machine.cpu.as_mut().unwrap();

        cpu.kinds = vec![
            Cores {
                kind: CoreKind::Performance,
                cores: 6,
                ..cpu.kinds[0].clone()
            },
            Cores {
                kind: CoreKind::Efficient,
                cores: 10,
                ..cpu.kinds[1].clone()
            },
        ];

        let topology = Topology::new(&machine).unwrap();
        let package = &topology.diagram.package;
        let (centre, _) = processor_ring(package);
        let (width, height) = Output::LAPTOP.virtual_size();
        let layout = Layout::new(width as i32, height as i32, topology.card());
        let (main, _) = crate::sheet::fit(
            topology.card(),
            topology.extent(),
            layout.view,
            Output::LAPTOP.display,
        );
        let window = inset(layout.detail_window(), 2);
        // What the window shows round the circle's centre at twice the
        // view's scale, the least a diagram's detail magnifies.
        let half = v(window.width as f32, window.height as f32) / (4.0 * main);

        let die = &package.dies[0];

        for extent in die
            .cores
            .iter()
            .map(|core| &core.square)
            .chain(die.cache.as_ref().map(|(band, _)| band))
        {
            // Half a unit to spare, a pixel, for the outlines.
            for corner in [extent.min, extent.max] {
                let off = (corner - centre).abs() + 0.5;
                assert!(off.x <= half.x && off.y <= half.y, "{corner} of {half}");
            }
        }
    }

    /// A display whose size is guessed from its pixels (a virtual machine's,
    /// a projector's) is lettered with its resolution, not a diagonal.
    #[test]
    fn a_display_of_unknown_size_is_not_given_one() {
        let mut machine = Machine::fixture();

        machine.connectors[1]
            .panel
            .as_mut()
            .expect("The monitor")
            .size
            .estimated = true;

        let topology = Topology::new(&machine).unwrap();
        let display = topology
            .card()
            .parts
            .iter()
            .find(|part| part.name == "DISPLAY")
            .unwrap();
        let monitor = topology
            .diagram
            .blocks
            .iter()
            .find(|block| block.source == Source::Connector(1))
            .unwrap();

        assert_eq!(display.material, "2 DISPLAYS");
        assert_eq!(display.spec[2], ("DP-1".into(), "3840 × 2160".into()));
        assert_eq!(display.spec[3].1, "60 Hz, SIZE UNKNOWN");
        assert_eq!(monitor.lines[0], "SIZE UNKNOWN");
        assert!(!monitor.more.iter().any(|line| line.contains("PITCH")));
    }

    /// A graphics card with more displays than the view holds: those not
    /// drawn are counted in a block, and the parts list still has them all.
    #[test]
    fn every_display_is_listed_when_some_are_counted() {
        use crate::machine::Fixture;

        let mut machine = Fixture::Desktop.machine();
        let monitor = machine.connectors[0].clone();

        for n in 2..=7 {
            machine.connectors.push(crate::machine::Connector {
                name: format!("DP-{n}"),
                ..monitor.clone()
            });
        }

        let topology = Topology::new(&machine).unwrap();
        let display = topology
            .card()
            .parts
            .iter()
            .find(|part| part.name == "DISPLAY")
            .unwrap();
        let drawn = topology
            .diagram
            .blocks
            .iter()
            .filter(|block| matches!(block.form, Form::Screen { .. }))
            .count();

        assert!(drawn < 8, "{drawn} drawn");
        assert_eq!(display.quantity, 8);
        assert_eq!(display.spec[0].0, "DP-1");
    }

    /// Cores all alike are not lettered with a kind, which means something
    /// only beside another; a hybrid processor's are.
    #[test]
    fn only_a_hybrid_processors_cores_are_lettered_by_kind() {
        use crate::draft::{Ink, Scope};
        use crate::machine::Fixture;

        for (fixture, lettered) in [(Fixture::Laptop, true), (Fixture::Desktop, false)] {
            let topology = Topology::new(&fixture.machine()).unwrap();
            let mut draft = Draft::new();

            topology.draw(&mut draft, 20.0);

            let kinds = draft
                .marks()
                .iter()
                .filter(|mark| mark.scope == Scope::Own)
                .filter(|mark| matches!(&mark.ink, Ink::Label { text, .. } if text == "P" || text == "E"))
                .count();

            assert_eq!(kinds > 0, lettered, "{fixture:?}");
        }
    }

    /// A processor of several packages has a die for each, its cores and
    /// its share of the last cache on it: a server's two of 32 cores, a
    /// virtual machine's four of one.
    #[test]
    fn each_package_has_a_die() {
        use crate::machine::Fixture;

        for (fixture, dies, cores, cache) in [
            (Fixture::Server, 2, 32, "L3 48 MiB"),
            (Fixture::Vm, 4, 1, "L3 16 MiB"),
            (Fixture::Desktop, 1, 8, "L3 32 MiB"),
        ] {
            let topology = Topology::new(&fixture.machine()).unwrap();
            let package = &topology.diagram.package;

            assert_eq!(package.dies.len(), dies, "{fixture:?}");

            for die in &package.dies {
                let drawn: u32 = die.cores.iter().map(|core| core.cores).sum();

                assert_eq!(drawn, cores, "{fixture:?}");
                assert_eq!(die.cache.as_ref().unwrap().1, cache, "{fixture:?}");
                let (outer, inner) = (package.frame, die.frame);

                assert!(outer.min.cmple(inner.min).all() && inner.max.cmple(outer.max).all());
            }
        }
    }

    /// A package with more cores than a die has room for draws a square
    /// for each cluster sharing a cache, lettered with how many it stands
    /// for.
    #[test]
    fn a_processor_with_many_cores_draws_their_clusters() {
        use crate::machine::{Cache, CacheKind};

        let mut machine = crate::machine::Fixture::Desktop.machine();
        let cpu = machine.cpu.as_mut().unwrap();

        cpu.cores = 128;
        cpu.threads = 256;
        cpu.caches = vec![Cache {
            level: 3,
            kind: CacheKind::Unified,
            bytes: 32 << 20,
            instances: 16,
        }];

        let topology = Topology::new(&machine).unwrap();
        let die = &topology.diagram.package.dies[0];

        assert_eq!(die.cores.len(), 16);
        assert!(die.cores.iter().all(|core| core.cores == 8));
        assert_eq!(die.cache.as_ref().unwrap().1, "L3 512 MiB");
    }

    /// At any scale, whole or not, the diagram is set on the pixel grid as
    /// one: blocks as large as each other in units are as large in pixels,
    /// a die's cores as far apart as each other, every wire meets the
    /// block it ends at exactly, and every dot of traffic is centred on a
    /// wire.
    #[test]
    fn the_diagram_is_set_on_the_pixel_grid() {
        use std::collections::{BTreeMap, BTreeSet};

        use crate::draft::raster::{self, Piece, Projection};
        use crate::draft::{Ink, Shape};
        use crate::machine::Fixture;
        use iced_core::Point;

        let key = |size: V2| {
            (
                (size.x * 64.0).round() as i64,
                (size.y * 64.0).round() as i64,
            )
        };

        for fixture in Fixture::ALL {
            let topology = Topology::new(&fixture.machine()).unwrap();
            let diagram = &topology.diagram;
            let mut draft = Draft::new();

            topology.draw(&mut draft, 20.0);

            for scale in [1.0, 1.25, 1.396, 1.43, 1.5, 1.77, 2.3] {
                let projection = Projection::new((17.0, 900.0), scale);
                let at = format!("{fixture:?} at {scale}");
                let mut sizes: BTreeMap<(i64, i64), BTreeSet<(i32, i32)>> = BTreeMap::new();
                // Each block's outline, by its frame, as drawn.
                let mut outlines: Vec<(Extent, (i32, i32, i32, i32))> = Vec::new();
                let mut wires: Vec<(Vec<V2>, Vec<Point<i32>>)> = Vec::new();
                let mut dots = Vec::new();
                // Each row of cores: their left and right edges, in units
                // and as drawn.
                let mut rows: BTreeMap<i64, Vec<(f32, f32, i32, i32)>> = BTreeMap::new();
                let squares: Vec<Extent> = diagram
                    .package
                    .dies
                    .iter()
                    .flat_map(|die| die.cores.iter().map(|core| core.square))
                    .collect();

                for mark in draft.marks().iter().filter(|mark| mark.shown_in(None)) {
                    let mut pieces = Vec::new();
                    raster::rasterize(mark, &projection, &mut pieces);

                    match (&mark.ink, pieces.first().map(|inked| &inked.piece)) {
                        (
                            Ink::Stroke {
                                shape: Shape::Polyline { points, closed },
                                ..
                            },
                            Some(Piece::Path { pixels, .. }),
                        ) => {
                            let bounds = (
                                pixels.iter().map(|p| p.x).min().unwrap(),
                                pixels.iter().map(|p| p.x).max().unwrap(),
                                pixels.iter().map(|p| p.y).min().unwrap(),
                                pixels.iter().map(|p| p.y).max().unwrap(),
                            );

                            if *closed && points.len() == 4 {
                                let frame = Extent::new(points[0], points[2]);

                                sizes
                                    .entry(key(v(frame.width(), frame.height())))
                                    .or_default()
                                    .insert((bounds.1 - bounds.0, bounds.3 - bounds.2));

                                if squares.contains(&frame) {
                                    rows.entry((frame.max.y * 64.0).round() as i64)
                                        .or_default()
                                        .push((frame.min.x, frame.max.x, bounds.0, bounds.1));
                                }

                                outlines.push((frame, bounds));
                            } else if !*closed {
                                wires.push((points.clone(), pixels.clone()));
                            }
                        }
                        (Ink::Dot { .. }, Some(Piece::Block(area))) => {
                            dots.push(Point::new(
                                area.x + area.width / 2,
                                area.y + area.height / 2,
                            ));
                        }
                        _ => {}
                    }
                }

                for (size, drawn) in &sizes {
                    assert_eq!(drawn.len(), 1, "{at}: {size:?} drawn as {drawn:?}");
                }

                // Cores are set from the middle of their row out, so they
                // are as far apart as each other either side of it, and as
                // far in from either side of their die, a pixel apart at
                // most.
                let dies: Vec<(i32, i32)> = diagram
                    .package
                    .dies
                    .iter()
                    .filter_map(|die| outlines.iter().find(|(frame, _)| *frame == die.frame))
                    .map(|(_, bounds)| (bounds.0, bounds.1))
                    .collect();

                for row in rows.values_mut() {
                    row.sort_by(|a, b| a.0.total_cmp(&b.0));

                    let middle = (row[0].0 + row[row.len() - 1].1) / 2.0;
                    let mut steps: BTreeMap<i64, BTreeSet<i32>> = BTreeMap::new();

                    for pair in row
                        .windows(2)
                        .filter(|pair| pair[1].0 <= middle || pair[0].1 >= middle)
                    {
                        steps
                            .entry(((pair[1].0 - pair[0].0) * 64.0).round() as i64)
                            .or_default()
                            .insert(pair[1].2 - pair[0].2);
                    }

                    for (step, drawn) in steps {
                        assert_eq!(drawn.len(), 1, "{at}: cores {step} apart drawn {drawn:?}");
                    }

                    let (left, right) = (row[0].2, row[row.len() - 1].3);

                    if let Some((outer_left, outer_right)) =
                        dies.iter().find(|(l, r)| *l < left && right < *r)
                    {
                        let margins = (left - outer_left, outer_right - right);

                        assert!((margins.0 - margins.1).abs() <= 1, "{at}: {margins:?}");
                    }
                }

                // A wire's end on a block's left or right edge lands on its
                // outline's column of pixels.
                for (points, pixels) in &wires {
                    for (end, pixel) in [
                        (points[0], pixels[0]),
                        (*points.last().unwrap(), *pixels.last().unwrap()),
                    ] {
                        for (frame, bounds) in &outlines {
                            if !(frame.min.y < end.y && end.y < frame.max.y) {
                                continue;
                            }

                            if end.x == frame.min.x {
                                assert_eq!(pixel.x, bounds.0, "{at}: {points:?} at {frame:?}");
                            }
                            if end.x == frame.max.x {
                                assert_eq!(pixel.x, bounds.1, "{at}: {points:?} at {frame:?}");
                            }
                        }
                    }
                }

                let inked: BTreeSet<(i32, i32)> = wires
                    .iter()
                    .flat_map(|(_, pixels)| pixels.iter().map(|p| (p.x, p.y)))
                    .collect();

                assert!(!dots.is_empty(), "{at}: no traffic");

                for dot in dots {
                    assert!(
                        inked.contains(&(dot.x, dot.y)),
                        "{at}: a dot off its wire at {dot:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn readings_are_the_live_values() {
        let topology = Topology::new(&Machine::fixture()).unwrap();
        let names: Vec<&str> = topology.readings(3.0).iter().map(|r| r.name).collect();

        assert_eq!(names, ["CPU", "DISK", "NET", "BATTERY"]);
    }
}
