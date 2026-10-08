//! The machine as a block diagram: the CPU package with its cores, the
//! memory, what is on the PCIe buses (bridges, graphics, drives, network
//! adapters), the USB tree and the displays on the graphics card's
//! connectors. Traffic runs along the wires as dots, at the rates the
//! machine measures its drives and network adapters at.
use quadrille::draw::Anchor;

use crate::draft::Placement::Auto;
use crate::draft::{Draft, Extent, Fill, Line, Tone, V2, v};
use crate::machine::{
    CoreKind, Drive, DriveKind, Interface, Link, Machine, Panel, PciAddress, PciDevice, PciKind,
    SensorKind, Site, UsbDevice,
};

use super::super::schematic::Schematic;
use super::super::{Card, Domain, Part, Reading, Revision, Subject, Unit};
use super::layout::{
    Bank, Diagram, Form, Gauge, Group, Package, Placed, Source, named, short, version,
};
use super::{SPEC_ROWS, binary, bits, counted, decimal, fit, flow, lettered, rate, rows};

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
            .sensors_on(Site::Processor(0))
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

    /// The processor: a die for each package, with its cores and last
    /// cache.
    fn package(&self, d: &mut Draft) {
        let package = &self.diagram.package;
        let cpu = self.machine.cpu.as_ref().expect("A diagram has a CPU");
        let frame = package.frame;
        let title = frame.max.y - 10.0;

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
            d.rect(die.frame.min, die.frame.max, Line::Thin);

            for core in &die.cores {
                d.rect(core.square.min, core.square.max, Line::Outline);

                // The efficient cores tinted on the view; a detail letters
                // each core's kind, and a tint would show only round the
                // letter.
                if core.kind == Some(CoreKind::Efficient) {
                    d.in_main(|d| {
                        d.area(&corners(core.square), Fill::Tint(4));
                    });
                }
            }

            if let Some((band, text)) = &die.cache {
                d.rect(band.min, band.max, Line::Thin);
                d.label(band.centre(), text.as_str());
            }

            // What the detail of the first die has room to say: the kind of
            // every core where there are two kinds, or how many cores a
            // square stands for, the processor over it (where a window is
            // tall enough: the laptop's holds the die alone) and the cache.
            if n > 0 {
                continue;
            }

            d.in_own_detail(|d| {
                for core in &die.cores {
                    let letter = match (core.kind, core.cores) {
                        (_, cores) if cores > 1 => cores.to_string(),
                        (Some(CoreKind::Performance), _) => "P".into(),
                        (Some(CoreKind::Efficient), _) => "E".into(),
                        (None, _) => continue,
                    };

                    d.label(core.square.centre(), letter).tone(Tone::Faint);
                }

                let what = match package.dies.len() {
                    1 => format!(
                        "CPU, {}, {} THREADS",
                        counted(cpu.cores as usize, "CORE", "CORES"),
                        cpu.threads
                    ),
                    dies => format!(
                        "CPU 1 OF {dies}, {}",
                        counted((cpu.cores as usize).div_ceil(dies), "CORE", "CORES")
                    ),
                };

                d.label(v(die.frame.centre().x, title), what)
                    .tone(Tone::Ink);

                if let Some((band, text)) = &die.cache {
                    d.label(band.centre(), format!("{text} CACHE"));
                }
            });
        }
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

        // The memory chips on the sticks, as many as a stick has room for;
        // and in its own detail the bank in full, over its sticks or in its
        // middle where the kernel shows no module.
        d.in_detail(|d| {
            for stick in &bank.sticks {
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
            }
        });

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

        // A screen's lettering clears its glass.
        let first = match block.form {
            Form::Screen { .. } => frame.max.y - 7.0,
            _ => frame.max.y - 5.0,
        };

        self.in_its_detail(d, block, |d| {
            for (k, line) in block.more.iter().enumerate() {
                d.label(v(frame.min.x + 4.0, first - 6.0 * k as f32), line.as_str())
                    .anchor(Anchor::LEFT)
                    .tone(if k == 0 { Tone::Ink } else { Tone::Muted });
            }
        });
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
/// if `inward`: dots a set distance apart running at a set speed, more of
/// them on a busier link.
fn dots(d: &mut Draft, pieces: &[Vec<V2>], rate: f32, inward: bool, t: f32, tone: Tone) {
    let share =
        ((rate.max(1.0).log10() - QUIETEST.log10()) / (BUSIEST / QUIETEST).log10()).clamp(0.0, 1.0);

    flow(
        d,
        pieces,
        share,
        SPACING,
        f64::from(t) * SPEED,
        inward,
        tone,
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
                        let name = match device.kind() {
                            PciKind::Bridge => {
                                device.name.as_deref().map_or("PCI BRIDGE".into(), lettered)
                            }
                            _ => named(device),
                        };
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
        let extent = topology.extent();
        let main = (layout.view.width as f32 / extent.width())
            .min(layout.view.height as f32 / extent.height());
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
            // A unit to spare for the outlines' pixels.
            for corner in [extent.min, extent.max] {
                let off = (corner - centre).abs() + 1.0;
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

    #[test]
    fn readings_are_the_live_values() {
        let topology = Topology::new(&Machine::fixture()).unwrap();
        let names: Vec<&str> = topology.readings(3.0).iter().map(|r| r.name).collect();

        assert_eq!(names, ["CPU", "DISK", "NET", "BATTERY"]);
    }
}
