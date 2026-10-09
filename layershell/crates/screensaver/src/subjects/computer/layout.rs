//! The topology sheet's block diagram, laid out from the inventory.
//!
//! The machine is drawn as a tree from the processor out. What sits on the
//! root bus hangs off a spine beside the CPU package; what sits behind a
//! bridge hangs off the bridge, drawn on the wire; a graphics card's
//! displays hang off its connectors, a USB host's devices off it and their
//! hubs. [`Diagram::of`] lays the tree out left to right, a column a level:
//! each block's first child level with it and the rest under it on a trunk,
//! so every wire runs across or down the grid and none crosses another.
//!
//! The units are the laptop's virtual pixels at full size. Lettering is a
//! whole number of pixels at any scale, so a block is made wide enough for
//! its lettering at the laptop's scale, the smallest the sheet draws the
//! diagram at, and [`Diagram::new`] folds a large tree until it fits the
//! laptop's view at that scale or larger (and [`Diagram::within`] until it
//! fits a smaller view, as far as it can): open connectors left out, alike
//! devices drawn as one (a server's eight NVMe drives, each on a root port
//! of its own, as one port and one block), then the last devices on a busy
//! hub, controller or bridge counted rather than drawn, displays last, and
//! at last the spine's last attachments.
//!
//! The processor is a die for each package, its cores drawn one by one up
//! to four rows, as clusters past that, and lettered by kind only on a
//! hybrid processor, where a kind means something beside the other.
use std::cmp::Reverse;

use quadrille::draw::{Anchor, Horizontal, Vertical};

use crate::draft::raster::LETTERING;
use crate::draft::{Extent, V2, v};
use crate::machine::{
    ConnectorKind, CoreKind, Cpu, Drive, DriveKind, Interface, Link, Machine, PciAddress,
    PciDevice, PciKind, UsbDevice,
};

use super::{binary, bits, cleaned, counted, decimal, fit, lettered};

/// The laptop's main view, in its virtual pixels: a diagram no larger is
/// drawn at the laptop's scale or larger, its lettering inside its blocks.
pub const BUDGET: V2 = v(581.0, 445.0);

/// Room round the drawing for the balloons to line up in.
const MARGIN: f32 = 22.0;
/// A line of lettering.
pub const LINE: f32 = 12.0;
/// Between a block's sides and its lettering.
const PAD: f32 = 6.0;
/// From a block's top to where its wires meet it: the middle of its first
/// line.
const PORT: f32 = 10.0;
/// A display's stand, or the base of the machine's own.
const STAND: f32 = 6.0;
/// Between blocks one under another on a trunk...
const SIBLINGS: f32 = 6.0;
/// ...and between what hangs off the root bus.
const ATTACHMENTS: f32 = 10.0;
/// The least room between two columns...
const SPAN: f32 = 28.0;
/// ...and where in it the trunk runs, from the column on its left.
const TRUNK: f32 = 10.0;
/// From the CPU package to the spine: room for the bus's name on the wire.
const SPINE: f32 = 32.0;
/// The lane between the spine and the first column, where bridges are
/// drawn with their addresses over them; narrower when there are none.
const LANE: f32 = 66.0;
const LANE_BARE: f32 = 20.0;
/// A bridge's symbol, and where it stands in the lane.
const BRIDGE: f32 = 10.0;
const BRIDGE_AT: f32 = 29.0;
/// The room a balloon takes beside lettering.
const BALLOON: f32 = 22.0;
/// The memory bus, from the memory to the package.
const BUS: f32 = 18.0;
/// The columns of blocks right of the spine: what is on the root bus, what
/// is on that, and one more.
const COLUMNS: usize = 3;
/// The squares a die has of each kind of core, at most, and to a row.
const MOST_SQUARES: u32 = 32;
const ACROSS: u32 = 8;
/// The last cache's band across a die: a line of lettering with the
/// ground it clears and room over and under that, so it never breaks the
/// band's edges.
const BAND: f32 = 14.0;
/// The memory modules drawn, at most.
const MOST_MODULES: usize = 16;
/// In a detail, lettering is at least twice its size in the view: a line
/// every half line, half a character's advance for every character.
pub const DETAIL_LINE: f32 = LINE / 2.0;
/// What a detail draws in a block, at the least width it takes: a graphics
/// card's outputs, a drive's partitions, a network adapter's jack or
/// antenna beside its lettering, a USB host's ports.
pub const PORTS: f32 = 72.0;
pub const PARTITIONS: f32 = 72.0;
pub const LINK: f32 = letters(DETAIL_ROOM) / 2.0 + 18.0;
pub const PLUGS: f32 = 84.0;
/// What a network adapter whose link is down is lettered.
const NO_LINK: &str = "NO LINK";
/// The characters of a detail's line a block is made wide enough for, at
/// the least: a line is cut to what the detail has room for, fewer than a
/// name may have (the specification has the whole of it).
const DETAIL_ROOM: usize = 22;

/// A run of `n` characters' width at the laptop's scale.
const fn letters(n: usize) -> f32 {
    n as f32 * LETTERING.advance() as f32
}

/// The widest of `lines`, in characters.
fn widest(lines: &[String]) -> usize {
    lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

/// What a block is, and the part of the sheet it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    Graphics,
    Display,
    Drive,
    Network,
    Usb,
    Bridge,
}

/// How a block is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// A box with its lettering in it.
    Block,
    /// A display: a screen on its stand or, the machine's own, its base.
    Screen { internal: bool },
    /// An output with nothing on it: an open terminal and its name.
    Terminal,
    /// A bridge: the converter symbol on the wire, its address over it.
    Bridge,
}

/// What a block is in the inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Pci(PciAddress),
    Drive(usize),
    Connector(usize),
    Usb(usize),
    /// Devices counted rather than drawn.
    Summary,
}

/// What measures the traffic on the wire to a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gauge {
    Drive(usize),
    Interface(usize),
}

/// A block of the tree, and what hangs off it.
#[derive(Debug, Clone)]
pub struct Node {
    pub group: Group,
    pub form: Form,
    pub source: Source,
    /// What else it stands for: devices alike merged into it.
    pub merged: Vec<Source>,
    /// Lettered in it in the view...
    pub lines: Vec<String>,
    /// ...and in a detail, where there is room for more, over what the
    /// detail draws of it...
    pub more: Vec<String>,
    /// ...which takes this much of its width at the least.
    pub drawing: f32,
    /// The connector the wire to it is, lettered on the wire.
    pub via: Option<String>,
    /// What measures the traffic to it: more than one for devices merged.
    pub gauges: Vec<Gauge>,
    /// How many devices it stands for.
    pub count: usize,
    /// Whether it carries its part's balloon: the first block of a part.
    pub balloon: bool,
    pub children: Vec<Node>,
}

impl Node {
    fn new(group: Group, form: Form, source: Source, lines: Vec<String>) -> Self {
        Self {
            group,
            form,
            source,
            merged: Vec::new(),
            lines,
            more: Vec::new(),
            drawing: 0.0,
            via: None,
            gauges: Vec::new(),
            count: 1,
            balloon: false,
            children: Vec::new(),
        }
    }

    fn more(mut self, more: Vec<String>) -> Self {
        self.more = more.into_iter().filter(|line| !line.is_empty()).collect();
        self
    }

    fn drawing(mut self, width: f32) -> Self {
        self.drawing = width;
        self
    }

    fn height(&self) -> f32 {
        match self.form {
            Form::Screen { .. } => 8.0 + 2.0 * LINE + STAND,
            Form::Block | Form::Bridge => 8.0 + LINE * self.lines.len().max(1) as f32,
            Form::Terminal => LINE,
        }
    }

    /// From its top to where its wires meet it.
    fn port(&self) -> f32 {
        match self.form {
            Form::Terminal => LINE / 2.0,
            _ => PORT,
        }
    }

    fn width(&self) -> f32 {
        // A balloon in a box stands right of its lettering.
        let balloon = if self.balloon && self.form == Form::Block {
            BALLOON
        } else {
            0.0
        };

        match self.form {
            Form::Terminal => 9.0 + letters(widest(&self.lines)) + 2.0,
            _ => {
                (letters(widest(&self.lines)) + balloon)
                    .max(letters(widest(&self.more).min(DETAIL_ROOM)) / 2.0 + 2.0)
                    .max(self.drawing)
                    .max(48.0)
                    + 2.0 * PAD
            }
        }
    }

    /// The devices in it and under it: a bridge is the way to them, not one
    /// of them.
    fn devices(&self) -> usize {
        let own = if self.form == Form::Bridge {
            0
        } else {
            self.count
        };

        own + self.children.iter().map(Node::devices).sum::<usize>()
    }

    /// What it is on the sheet: a bridge is the first thing behind it.
    fn kind(&self) -> Group {
        match (self.form, self.children.first()) {
            (Form::Bridge, Some(first)) => first.kind(),
            _ => self.group,
        }
    }
}

/// The order the sheet documents what a block is, and lays out what hangs
/// off the root bus.
fn rank(group: Group) -> u8 {
    group as u8
}

/// A PCI address as `lspci` writes it on a sheet: bus, device, function.
pub fn short(address: PciAddress) -> String {
    format!(
        "{:02x}:{:02x}.{:x}",
        address.bus, address.device, address.function
    )
}

/// The tree of `machine`: what hangs off its root bus, in order, and how
/// many of its PCI functions it shows.
pub fn tree(machine: &Machine) -> (Vec<Node>, usize) {
    // Each block on the PCI bus, by the device on the root bus it is
    // reached through.
    let mut roots: Vec<(PciAddress, Vec<(PciAddress, Node)>)> = Vec::new();

    for device in &machine.pci {
        let Some(node) = block(machine, device) else {
            continue;
        };
        let mut root = device.address;

        while let Some(parent) = machine.pci_device(root).and_then(|d| d.parent) {
            root = parent;
        }

        match roots.iter_mut().find(|(address, _)| *address == root) {
            Some((_, nodes)) => nodes.push((device.address, node)),
            None => roots.push((root, vec![(device.address, node)])),
        }
    }

    let mut shown = 0;
    let mut attachments: Vec<(u8, PciAddress, Node)> = roots
        .into_iter()
        .map(|(root, mut nodes)| {
            nodes.sort_by_key(|(address, node)| (rank(node.group), *address));
            shown += nodes.len();
            let first = rank(nodes[0].1.group);

            let node = match nodes.as_slice() {
                [(address, _)] if *address == root => nodes.remove(0).1,
                _ => {
                    shown += 1;
                    bridge(machine, root, nodes.into_iter().map(|(_, node)| node))
                }
            };

            (first, root, node)
        })
        .collect();

    attachments.sort_by_key(|(first, root, _)| (*first, *root));

    (
        attachments.into_iter().map(|(_, _, node)| node).collect(),
        shown,
    )
}

/// The block a PCI device is on the sheet, if it is one: a graphics card,
/// a drive's controller, a network adapter or a USB host with buses.
fn block(machine: &Machine, device: &PciDevice) -> Option<Node> {
    let drives: Vec<(usize, &Drive)> = machine
        .drives
        .iter()
        .enumerate()
        .filter(|(_, drive)| drive.pci == Some(device.address) && drive.kind != DriveKind::Usb)
        .collect();

    match device.kind() {
        PciKind::Display => Some(graphics(machine, device)),
        _ if !drives.is_empty() => Some(storage(device, &drives)),
        PciKind::Network => Some(network(machine, device)),
        PciKind::Usb => usb(machine, device),
        _ => None,
    }
}

/// What a PCI device says it is: its name, cleaned, or its class.
pub fn named(device: &PciDevice) -> String {
    match (&device.name, &device.class_name) {
        (Some(name), _) => cleaned(name),
        (None, Some(class)) => lettered(class),
        (None, None) => "PCI DEVICE".into(),
    }
}

/// What a PCI device is, for a detail's line: without the platform it is a
/// part of (`… SERIES PROCESSORS`) or the word `CONTROLLER`, which every
/// device is, and where it is longer than a line without the standard it
/// keeps in brackets, so its model is kept.
fn called(device: &PciDevice) -> String {
    let name = named(device);
    let mut words: Vec<&str> = name.split_whitespace().collect();

    if let Some(series) = words.iter().position(|word| *word == "SERIES") {
        let mut part = series + 1;

        while words
            .get(part)
            .is_some_and(|word| ["PROCESSOR", "PROCESSORS", "CHIPSET"].contains(word))
        {
            part += 1;
        }

        if part < words.len() {
            words.drain(..part);
        }
    }

    words.retain(|word| *word != "CONTROLLER");
    let mut name = match words.as_slice() {
        [] => name.clone(),
        words => words.join(" "),
    };

    while name.chars().count() > DETAIL_ROOM
        && let (Some(open), Some(close)) = (name.find('('), name.find(')'))
        && open < close
    {
        name.replace_range(open..=close, " ");
        name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    }

    name
}

/// What a bridge on the root bus is, by where it is rather than by the
/// platform's long name for it: a root port or a bridge; a device that is
/// not a bridge (a VMD controller, which drives sit behind) by its name.
pub fn bridged(device: &PciDevice) -> String {
    if device.kind() != PciKind::Bridge {
        return named(device);
    }

    let name = device.name.as_deref().unwrap_or_default().to_lowercase();
    let express = device.driver.as_deref() == Some("pcieport")
        || ["pcie", "pci express", "root port"]
            .iter()
            .any(|word| name.contains(word));

    match (express, device.parent) {
        (true, None) => "PCIe ROOT PORT".into(),
        (true, Some(_)) => "PCIe BRIDGE".into(),
        (false, _) => "PCI BRIDGE".into(),
    }
}

/// Its address and driver, for a detail.
fn bound(device: &PciDevice) -> String {
    match &device.driver {
        Some(driver) => format!("PCI {} {driver}", short(device.address)),
        None => format!("PCI {}", short(device.address)),
    }
}

fn graphics(machine: &Machine, device: &PciDevice) -> Node {
    let children: Vec<Node> = machine
        .connectors
        .iter()
        .enumerate()
        .filter(|(_, connector)| connector.gpu == Some(device.address))
        .map(|(index, connector)| match &connector.panel {
            Some(panel) => {
                let internal = connector.kind == ConnectorKind::Internal;
                let pixels = format!("{} × {}", panel.pixels.0, panel.pixels.1);
                // A size guessed from the pixels alone is not lettered as
                // if it were measured.
                let (inches, pitch) = if panel.size.estimated {
                    ("SIZE UNKNOWN".to_owned(), String::new())
                } else {
                    (
                        format!("{:.1}\"", panel.inches()),
                        format!("PITCH {:.3} mm", panel.pitch_mm()),
                    )
                };
                let name = match (&panel.name, internal) {
                    (_, true) => "BUILT-IN PANEL".to_owned(),
                    (Some(name), false) => lettered(name),
                    (None, false) => "MONITOR".to_owned(),
                };
                let refresh = panel
                    .refresh
                    .map(|hz| format!("{} Hz", hz.round()))
                    .unwrap_or_default();

                Node {
                    via: Some(connector.name.clone()),
                    ..Node::new(
                        Group::Display,
                        Form::Screen { internal },
                        Source::Connector(index),
                        vec![inches.clone(), pixels.clone()],
                    )
                    .more(vec![
                        name,
                        [inches, refresh]
                            .into_iter()
                            .filter(|what| !what.is_empty())
                            .collect::<Vec<_>>()
                            .join(", "),
                        pitch,
                    ])
                }
            }
            // An open output is the card's, not a display's.
            None => Node::new(
                Group::Graphics,
                Form::Terminal,
                Source::Connector(index),
                vec![connector.name.clone()],
            ),
        })
        .collect();
    let outputs = counted(children.len(), "OUTPUT", "OUTPUTS");

    Node {
        children,
        ..Node::new(
            Group::Graphics,
            Form::Block,
            Source::Pci(device.address),
            vec!["GRAPHICS".into(), outputs.clone()],
        )
        .more(vec![called(device), bound(device)])
        .drawing(PORTS)
    }
}

/// What a drive is, in a word or two.
fn drive_kind(drive: &Drive) -> &'static str {
    match drive.kind {
        DriveKind::Nvme => "NVMe SSD",
        DriveKind::Mmc => "MMC",
        // A virtual machine's disk says it spins whatever it is on.
        DriveKind::Virtual => "VIRTUAL DISK",
        _ if drive.rotational => "HARD DISK",
        DriveKind::Sata => "SSD",
        _ => "DRIVE",
    }
}

fn drive(index: usize, drive: &Drive) -> Node {
    let size = decimal(drive.bytes);

    Node {
        gauges: vec![Gauge::Drive(index)],
        ..Node::new(
            Group::Drive,
            Form::Block,
            Source::Drive(index),
            vec![drive_kind(drive).into(), size.clone()],
        )
        .more(vec![
            lettered(drive.model.as_deref().unwrap_or(drive_kind(drive))),
            format!(
                "{size}, {}",
                counted(drive.partitions.len(), "PARTITION", "PARTITIONS")
            ),
        ])
        .drawing(PARTITIONS)
    }
}

/// A drive's controller: an NVMe drive is its own, and one block with it,
/// its detail closing on the controller's address rather than the drive's
/// name.
fn storage(device: &PciDevice, drives: &[(usize, &Drive)]) -> Node {
    if let [(index, only)] = drives
        && only.kind == DriveKind::Nvme
    {
        return drive(*index, only);
    }

    let label = if (device.class >> 8) & 0xff == 0x06 {
        "SATA"
    } else {
        "STORAGE"
    };
    let count = counted(drives.len(), "DRIVE", "DRIVES");

    Node {
        children: drives.iter().map(|(index, d)| drive(*index, d)).collect(),
        ..Node::new(
            Group::Drive,
            Form::Block,
            Source::Pci(device.address),
            vec![label.into(), count.clone()],
        )
        .more(vec![
            format!("{label}, {count}"),
            called(device),
            bound(device),
        ])
    }
}

fn network(machine: &Machine, device: &PciDevice) -> Node {
    let interfaces: Vec<(usize, &Interface)> = machine
        .interfaces
        .iter()
        .enumerate()
        .filter(|(_, interface)| interface.pci == Some(device.address) && !interface.usb)
        .collect();
    let wireless = (device.class >> 16) as u8 == 0x0d || (device.class >> 8) & 0xff == 0x80;
    let link = interfaces.first().map_or(
        if wireless {
            Link::Wireless
        } else {
            Link::Ethernet
        },
        |(_, i)| i.link,
    );
    let name = match link {
        Link::Ethernet => "ETHERNET",
        Link::Wireless => "WI-FI",
        Link::Other => "NETWORK",
    };
    let state = interfaces.first().map(|(_, interface)| match interface {
        Interface {
            up: true,
            speed: Some(speed),
            ..
        } => bits(*speed as f32),
        Interface { up: true, .. } => "LINK UP".into(),
        Interface { up: false, .. } => NO_LINK.into(),
    });
    let mut lines = vec![name.to_owned()];
    lines.extend(state.clone());

    Node {
        gauges: interfaces
            .first()
            .map(|(index, _)| Gauge::Interface(*index))
            .into_iter()
            .collect(),
        ..Node::new(
            Group::Network,
            Form::Block,
            Source::Pci(device.address),
            lines,
        )
        .more(vec![
            called(device),
            bound(device),
            state.unwrap_or_default(),
        ])
        .drawing(LINK)
    }
}

/// A USB version as it is marketed: `2.00` is USB 2.0, `3.20` USB 3.2.
pub fn version(device: &UsbDevice) -> Option<(u8, u8)> {
    let (major, minor) = device.version.as_deref()?.trim().split_once('.')?;

    Some((major.parse().ok()?, minor.get(..1)?.parse().ok()?))
}

/// A USB host, with its root hubs' devices: the hubs are the host's own.
fn usb(machine: &Machine, device: &PciDevice) -> Option<Node> {
    let hubs: Vec<&UsbDevice> = machine
        .usb_on(None)
        .filter(|hub| hub.controller == Some(device.address))
        .collect();

    if hubs.is_empty() {
        return None;
    }

    let fastest = hubs.iter().filter_map(|hub| version(hub)).max();
    let label = fastest.map_or("USB".into(), |(major, minor)| {
        format!("USB {major}.{minor}")
    });
    let mut children: Vec<Node> = hubs
        .iter()
        .flat_map(|hub| machine.usb_on(Some(&hub.port)))
        .map(|device| usb_device(machine, device, 1))
        .collect();

    merge(&mut children);

    Some(Node {
        children,
        ..Node::new(
            Group::Usb,
            Form::Block,
            Source::Pci(device.address),
            vec!["USB HOST".into(), label.clone()],
        )
        .more(vec![called(device), bound(device)])
        .drawing(PLUGS)
    })
}

/// What a USB device is, by its class.
fn usb_class(class: u8) -> &'static str {
    match class {
        0x01 => "AUDIO",
        0x02 | 0x0a => "MODEM",
        0x03 => "INPUT",
        0x06 => "IMAGING",
        0x07 => "PRINTER",
        0x08 => "STORAGE",
        0x09 => "HUB",
        0x0b => "SMART CARD",
        0x0e => "CAMERA",
        0x10 => "AUDIO/VIDEO",
        0x11 => "BILLBOARD",
        0xe0 => "WIRELESS",
        _ => "DEVICE",
    }
}

/// A USB device in column `column` (the USB host's is the first), with
/// what is on it if it is a hub and there is a column for that.
fn usb_device(machine: &Machine, device: &UsbDevice, column: usize) -> Node {
    let index = machine
        .usb
        .iter()
        .position(|other| other.port == device.port)
        .unwrap_or(0);
    let class = usb_class(device.class);
    let mut children: Vec<Node> = if device.hub() {
        machine
            .usb_on(Some(&device.port))
            .map(|on| usb_device(machine, on, column + 1))
            .collect()
    } else {
        Vec::new()
    };

    merge(&mut children);

    let speed = device.speed.map(bits).unwrap_or_default();
    let mut node = Node {
        children,
        ..Node::new(
            Group::Usb,
            Form::Block,
            Source::Usb(index),
            vec![class.into()],
        )
        .more(vec![
            lettered(device.product.as_deref().unwrap_or(class)),
            format!("PORT {} {speed}", device.port),
        ])
    };

    // On the last column, what is on a hub is counted in it.
    if column + 1 >= COLUMNS {
        hub_summary(&mut node);
    }

    node
}

/// A hub with no column for what is on it: the devices counted in it.
fn hub_summary(node: &mut Node) {
    if node.children.is_empty() {
        return;
    }

    let devices = node.devices() - node.count;

    node.children.clear();
    node.lines = vec!["HUB".into(), counted(devices, "DEVICE", "DEVICES")];
}

/// Merges devices alike that have nothing on them into one block for all
/// of them: two keyboards are `INPUT ×2`.
fn merge(nodes: &mut Vec<Node>) {
    let mut merged: Vec<Node> = Vec::new();

    for node in nodes.drain(..) {
        let alike = merged.iter_mut().find(|other| {
            other.children.is_empty()
                && node.children.is_empty()
                && other.form == node.form
                && other.lines == node.lines
                && other.gauges.is_empty()
                && node.gauges.is_empty()
        });

        match alike {
            Some(other) => other.count += node.count,
            None => merged.push(node),
        }
    }

    for node in &mut merged {
        if node.count > 1 {
            node.lines[0] = format!("{} ×{}", node.lines[0], node.count);
        }
    }

    *nodes = merged;
}

/// The device on the root bus that `nodes` are reached through, drawn on
/// the wire to them.
fn bridge(machine: &Machine, root: PciAddress, nodes: impl Iterator<Item = Node>) -> Node {
    let what = machine
        .pci_device(root)
        .map_or("PCI BRIDGE".into(), |device| fit(&bridged(device), 16));

    Node {
        children: nodes.collect(),
        ..Node::new(
            Group::Bridge,
            Form::Bridge,
            Source::Pci(root),
            vec![short(root)],
        )
        .more(vec![what, short(root)])
    }
}

/// Marks the first block of each part, in the order they are drawn, as
/// the one its balloon points at and its detail is of: an open terminal
/// never, and of the drives a drive before its controller, so the detail
/// shows a drive's partitions.
fn mark_balloons(tree: &mut [Node]) {
    let preferred = |node: &Node| match node.group {
        Group::Drive => matches!(node.source, Source::Drive(_)),
        _ => true,
    };
    let mut first: Vec<Group> = Vec::new();

    walk(tree, 0, &mut |node, _| {
        if node.form != Form::Terminal && preferred(node) && !first.contains(&node.group) {
            first.push(node.group);
        }
    });

    let mut seen = Vec::new();

    walk(tree, 0, &mut |node, _| {
        node.balloon = node.form != Form::Terminal
            && !seen.contains(&node.group)
            && (preferred(node) || !first.contains(&node.group));

        if node.balloon {
            seen.push(node.group);
        }
    });
}

/// Visits every node of `nodes` with the column it is drawn in: a bridge is
/// on the wire, and what is behind it in the same column as it.
fn walk(nodes: &mut [Node], column: usize, visit: &mut impl FnMut(&mut Node, usize)) {
    for node in nodes {
        visit(node, column);

        let next = if node.form == Form::Bridge {
            column
        } else {
            column + 1
        };

        walk(&mut node.children, next, visit);
    }
}

/// The order in which busy blocks have what hangs off them counted when a
/// diagram is too tall: a USB host's or hub's devices first, then a
/// controller's drives, network adapters, what is behind a bridge, and a
/// graphics card's displays last, as a sheet that leaves a display out
/// says less about the machine than one that counts a keyboard.
fn fold_order(group: Group) -> u8 {
    match group {
        Group::Usb => 0,
        Group::Drive => 1,
        Group::Network => 2,
        Group::Bridge => 3,
        Group::Graphics | Group::Display => 4,
    }
}

/// How a busy block ranks for folding: by [`fold_order`], then the
/// busiest, then the deepest.
type Busy = (u8, Reverse<usize>, Reverse<usize>);

/// The way down `nodes`, `depth` from the spine, to the block whose last
/// children are counted first: of those with three or more blocks hanging
/// off them, the first by [`Busy`].
fn busiest(
    nodes: &[Node],
    depth: usize,
    path: &mut Vec<usize>,
    best: &mut Option<(Busy, Vec<usize>)>,
) {
    for (index, node) in nodes.iter().enumerate() {
        path.push(index);

        let many = node.children.len();
        let rank = (fold_order(node.group), Reverse(many), Reverse(depth));

        if matches!(node.form, Form::Block | Form::Bridge)
            && many >= 3
            && best.as_ref().is_none_or(|(other, _)| rank < *other)
        {
            *best = Some((rank, path.clone()));
        }

        busiest(&node.children, depth + 1, path, best);
        path.pop();
    }
}

/// Whether `a` and `b` can be drawn as one block standing for both: blocks
/// with nothing hanging off them, lettered alike (two disks of one model
/// and size) or functions of one network adapter.
fn alike(a: &Node, b: &Node) -> bool {
    let function = |node: &Node| match node.source {
        Source::Pci(address) => Some((address.domain, address.bus, address.device)),
        _ => None,
    };

    a.children.is_empty()
        && b.children.is_empty()
        && a.form == Form::Block
        && b.form == Form::Block
        && a.group == b.group
        && a.source != Source::Summary
        && b.source != Source::Summary
        && (a.lines == b.lines
            || (a.group == Group::Network && function(a).is_some() && function(a) == function(b)))
}

/// The one block that `alike` blocks are drawn as: the first, standing for
/// them all, with their traffic together.
fn combined(mut alike: Vec<Node>) -> Node {
    let rest = alike.split_off(1);
    let mut node = alike.pop().expect("One at least");

    if rest.is_empty() {
        return node;
    }

    let states: Vec<Option<String>> = std::iter::once(&node)
        .chain(&rest)
        .map(|node| node.lines.get(1).cloned())
        .collect();

    for other in rest {
        node.count += other.count;
        node.gauges.extend(other.gauges);
        node.merged.push(other.source);
        node.merged.extend(other.merged);
    }

    let count = node.count;
    node.lines[0] = format!("{} ×{count}", node.lines[0]);

    match node.group {
        // A network adapter's ports may each be up or down.
        Group::Network if states.iter().any(|state| *state != states[0]) => {
            let up = states
                .iter()
                .filter(|state| state.as_deref().is_some_and(|state| state != NO_LINK))
                .count();

            node.lines.truncate(1);
            node.lines.push(format!("{up} OF {count} UP"));

            if let Some(state) = node.more.get_mut(2) {
                *state = node.lines[1].clone();
            }
        }
        Group::Drive => {
            if let Some(size) = node.lines.get_mut(1) {
                size.push_str(" EACH");

                // Its detail draws the first one's partitions.
                if let Some(drives) = node.more.get_mut(1) {
                    *drives = format!("{count} DRIVES, {size}");
                }
            }
        }
        _ => {}
    }

    node
}

/// Merges the alike blocks of `nodes` (see [`alike`]); whether any were.
fn merge_alike(nodes: &mut Vec<Node>) -> bool {
    let mut sets: Vec<Vec<Node>> = Vec::new();

    for node in nodes.drain(..) {
        match sets.iter_mut().find(|set| alike(&set[0], &node)) {
            Some(set) => set.push(node),
            None => sets.push(vec![node]),
        }
    }

    let merged = sets.iter().any(|set| set.len() > 1);

    *nodes = sets.into_iter().map(combined).collect();
    merged
}

/// Merges the bridges on the spine that each have one block behind them,
/// alike (an NVMe drive on each of a server's root ports), into the first:
/// one bridge standing for them all, with the blocks merged behind it.
/// Whether any were.
fn merge_ports(tree: &mut Vec<Node>) -> bool {
    let lone = |node: &Node| match (node.form, node.children.as_slice()) {
        (Form::Bridge, [only]) => Some(only.clone()),
        _ => None,
    };
    let mut sets: Vec<Vec<Node>> = Vec::new();

    for node in tree.drain(..) {
        let set = sets.iter_mut().find(|set| {
            lone(&set[0])
                .zip(lone(&node))
                .is_some_and(|(a, b)| alike(&a, &b))
        });

        match set {
            Some(set) => set.push(node),
            None => sets.push(vec![node]),
        }
    }

    let merged = sets.iter().any(|set| set.len() > 1);

    *tree = sets
        .into_iter()
        .map(|mut set| {
            if set.len() == 1 {
                return set.pop().expect("One");
            }

            let behind: Vec<Node> = set
                .iter_mut()
                .map(|port| port.children.pop().expect("One behind each"))
                .collect();
            let ports = set.len();
            let mut first = set.remove(0);

            first.merged.extend(set.into_iter().map(|port| port.source));
            first.count = ports;
            first.lines = vec![counted(ports, "PORT", "PORTS")];

            if let Some(what) = first.more.first_mut() {
                *what = format!("{what} ×{ports}");
            }

            first.children = vec![combined(behind)];
            first
        })
        .collect();

    merged
}

/// A block counting `count` devices of `group` not drawn, which carries no
/// traffic of its own.
fn summary(group: Group, count: usize) -> Node {
    let (one, many) = match group {
        Group::Drive => ("MORE DRIVE", "MORE DRIVES"),
        Group::Display => ("MORE DISPLAY", "MORE DISPLAYS"),
        _ => ("MORE DEVICE", "MORE DEVICES"),
    };

    Node {
        count,
        ..Node::new(
            group,
            Form::Block,
            Source::Summary,
            vec![format!("+{count} MORE")],
        )
        .more(vec![counted(count, one, many)])
    }
}

/// How many devices `node` stands for, counted in a summary.
fn counted_in(node: &Node) -> usize {
    match node.source {
        Source::Summary => node.count,
        _ => node.devices(),
    }
}

/// Folds the tree a step smaller: `wide` when it is too wide, else too
/// tall. Whether there was anything left to fold.
fn fold(tree: &mut Vec<Node>, wide: bool) -> bool {
    let mut folded = false;

    // Too wide: no last column, what would be on it counted in its hubs.
    if wide {
        walk(tree, 0, &mut |node, column| {
            if column + 2 == COLUMNS && node.group == Group::Usb && !node.children.is_empty() {
                hub_summary(node);
                folded = true;
            }
        });

        return folded;
    }

    // Too tall: the open connectors left out first...
    walk(tree, 0, &mut |node, _| {
        let before = node.children.len();

        node.children.retain(|child| child.form != Form::Terminal);
        folded |= node.children.len() != before;
    });

    if folded {
        return true;
    }

    // ...then blocks alike drawn as one, which loses nothing but their
    // traffic apart...
    folded = merge_ports(tree) | merge_alike(tree);
    walk(tree, 0, &mut |node, _| {
        folded |= merge_alike(&mut node.children)
    });

    if folded {
        return true;
    }

    // ...then the last two blocks off the busiest counted in one...
    let mut best = None;
    busiest(tree, 0, &mut Vec::new(), &mut best);

    if let Some((_, path)) = best {
        let mut node = &mut tree[path[0]];

        for &index in &path[1..] {
            node = &mut node.children[index];
        }

        let last = node.children.pop().expect("Three or more");
        let before = node.children.pop().expect("Three or more");

        node.children.push(summary(
            before.kind(),
            counted_in(&last) + counted_in(&before),
        ));

        return true;
    }

    // ...and at last the spine's last two attachments.
    if tree.len() < 2 {
        return false;
    }

    let last = tree.pop().expect("Two or more");
    let before = tree.pop().expect("Two or more");

    tree.push(summary(
        before.kind(),
        counted_in(&last) + counted_in(&before),
    ));

    true
}

/// A block placed on the sheet.
#[derive(Debug, Clone)]
pub struct Placed {
    pub group: Group,
    pub form: Form,
    pub source: Source,
    /// What else it stands for: devices alike merged into it.
    pub merged: Vec<Source>,
    /// Its outline: a screen's without its stand, a bridge's symbol.
    pub frame: Extent,
    /// Where its wire meets it.
    pub port: V2,
    pub lines: Vec<String>,
    pub more: Vec<String>,
    /// Whether it carries its part's balloon.
    pub balloon: bool,
    /// The points it is set on the pixel grid by, outermost first (see
    /// [`Draft::snapped`](crate::draft::Draft::snapped)): the block it
    /// hangs off's and its own top left corner. So every block is as many
    /// pixels across as another as many units across, and the wires drawn
    /// from a block (set as it is) meet what hangs off it exactly.
    pub snaps: Vec<V2>,
}

/// A wire, set on the grid as the block it is drawn from is (see
/// [`Placed::snaps`]): none for the spine's.
#[derive(Debug, Clone)]
pub struct Wire {
    pub snaps: Vec<V2>,
    pub points: Vec<V2>,
}

/// Where three wires join, set on the grid as they are.
#[derive(Debug, Clone)]
pub struct Junction {
    pub snaps: Vec<V2>,
    pub at: V2,
}

/// Lettering along the wires: a connector's name, a bridge's address; set
/// on the grid as the wire it names is.
#[derive(Debug, Clone)]
pub struct Legend {
    pub snaps: Vec<V2>,
    pub at: V2,
    pub text: String,
    pub anchor: Anchor,
}

/// The way traffic takes from the processor to a block, as the wires it
/// runs along: broken where it passes through a block on the way.
#[derive(Debug, Clone)]
pub struct Route {
    /// What measures it: the traffic is theirs together.
    pub gauges: Vec<Gauge>,
    pub pieces: Vec<Vec<V2>>,
    /// What each piece is set on the grid by, as its wire is.
    pub snaps: Vec<Vec<V2>>,
}

/// A square on a die: a core, or where a package has more cores of a kind
/// than a die has room for, a cluster of them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Core {
    pub square: Extent,
    /// Its kind on a hybrid processor; none where the cores are all alike.
    pub kind: Option<CoreKind>,
    /// The cores it stands for.
    pub cores: u32,
}

/// A package's die: its cores and its share of the last cache.
#[derive(Debug, Clone)]
pub struct Die {
    pub frame: Extent,
    pub cores: Vec<Core>,
    /// The last cache's band across the die, and its level and size.
    pub cache: Option<(Extent, String)>,
}

/// The processor: a die for each package, side by side or one over the
/// other, in one frame.
#[derive(Debug, Clone)]
pub struct Package {
    pub frame: Extent,
    pub dies: Vec<Die>,
    /// Where the spine's wire leaves it.
    pub port: V2,
}

/// The memory, its modules drawn as sticks.
#[derive(Debug, Clone)]
pub struct Bank {
    pub frame: Extent,
    pub sticks: Vec<Extent>,
    /// The bus to the package, and what it is.
    pub bus: (V2, V2),
    pub generation: Option<String>,
}

/// The diagram, laid out.
#[derive(Debug, Clone)]
pub struct Diagram {
    pub extent: Extent,
    pub package: Package,
    pub bank: Option<Bank>,
    pub blocks: Vec<Placed>,
    pub wires: Vec<Wire>,
    pub junctions: Vec<Junction>,
    pub legends: Vec<Legend>,
    pub routes: Vec<Route>,
    /// How many of the machine's PCI functions it shows.
    pub shown: usize,
}

impl Diagram {
    /// The diagram of `machine`, if it has a processor and something on
    /// its root bus to show; folded until it fits [`BUDGET`], if it can be.
    pub fn new(machine: &Machine) -> Option<Self> {
        Self::within(machine, BUDGET)
    }

    /// The diagram of `machine` folded until it fits `budget` instead: a
    /// view smaller than the laptop's, where a diagram any larger would be
    /// drawn smaller than its lettering.
    pub fn within(machine: &Machine, budget: V2) -> Option<Self> {
        let cpu = machine.cpu.as_ref()?;
        let (mut tree, shown) = tree(machine);

        if tree.is_empty() {
            return None;
        }

        loop {
            mark_balloons(&mut tree);

            let mut diagram = Self::of(machine, cpu, &tree, shown);
            let size = v(diagram.extent.width(), diagram.extent.height());
            let folded = (size.x > budget.x && fold(&mut tree, true))
                || (size.y > budget.y && fold(&mut tree, false));

            if !folded {
                // A diagram smaller than the budget is given the room of
                // one as large, so the laptop draws it at a pixel to a unit
                // as its lettering is sized for, and what its details show
                // is the same however much is on its buses.
                let room = (budget - size).max(V2::ZERO) / 2.0;

                diagram.extent.min -= room.floor();
                diagram.extent.max += room.ceil();
                return Some(diagram);
            }
        }
    }

    /// `tree` laid out beside `cpu`'s package.
    fn of(machine: &Machine, cpu: &Cpu, tree: &[Node], shown: usize) -> Self {
        let (package_size, package_layout) = package(cpu);
        let bank_size = machine
            .memory
            .as_ref()
            .map(|memory| bank(memory.modules.len()));
        let width = package_size.x.max(bank_size.map_or(0.0, |size| size.x));
        let spine = width + SPINE;
        let bridged = tree.iter().any(|node| node.form == Form::Bridge);
        let first = spine + if bridged { LANE } else { LANE_BARE };

        let mut grid = Grid {
            columns: columns(tree, first),
            spine,
            ..Grid::default()
        };

        // What hangs off the spine, one under another.
        let mut cursor = 0.0;
        let mut ports = Vec::new();

        for (index, node) in tree.iter().enumerate() {
            let top = if index == 0 {
                0.0
            } else {
                cursor - ATTACHMENTS
            };
            let port = top - attachment_port(node);

            cursor = grid.attach(node, port);
            ports.push(port);
        }

        // The package level with the middle of the spine, the memory over
        // it.
        let middle = ((ports[0] + ports[ports.len() - 1]) / 2.0).round();
        let top = (middle + package_size.y / 2.0).round();
        let left = ((width - package_size.x) / 2.0).round();
        let package = package_layout(v(left, top), middle);
        let bank = bank_size.map(|size| {
            let bus_top = package.frame.max.y + BUS;
            let left = ((width - size.x) / 2.0).round();
            let frame = Extent::new(v(left, bus_top), v(left + size.x, bus_top + size.y));
            let modules = machine.memory.as_ref().map_or(0, |m| m.modules.len());
            let middle = frame.centre().x.round();

            Bank {
                sticks: sticks(frame, modules),
                bus: (v(middle, bus_top), v(middle, package.frame.max.y)),
                generation: machine
                    .memory
                    .as_ref()
                    .and_then(|m| m.modules.first()?.generation.clone()),
                frame,
            }
        });

        // The spine, the package's wire to it, and the processor's end of
        // every route.
        let package_port = package.port;
        grid.trunk(spine, &[middle], &ports, &[]);
        grid.wires.push(Wire {
            snaps: Vec::new(),
            points: vec![package_port, v(spine, middle)],
        });

        for route in &mut grid.routes {
            route.pieces[0].splice(0..0, [package_port, v(spine, middle)]);
        }

        grid.legends.push(Legend {
            snaps: Vec::new(),
            at: v((package_port.x + spine) / 2.0, middle + 3.0),
            text: "PCIe".into(),
            anchor: Anchor::new(Horizontal::Centre, Vertical::Baseline),
        });

        let mut bounds = vec![package.frame];

        bounds.extend(bank.as_ref().map(|bank| bank.frame));
        bounds.extend(grid.blocks.iter().map(|block| match block.form {
            Form::Screen { internal: true } => Extent::new(
                block.frame.min - v(3.0, STAND),
                block.frame.max + v(3.0, 0.0),
            ),
            Form::Screen { internal: false } => {
                Extent::new(block.frame.min - v(0.0, STAND), block.frame.max)
            }
            Form::Bridge => Extent::new(block.frame.min, block.frame.max + v(0.0, LINE * 1.5)),
            _ => block.frame,
        }));

        let mut extent = bounds[0];

        for bound in &bounds[1..] {
            extent.min = extent.min.min(bound.min);
            extent.max = extent.max.max(bound.max);
        }

        extent.min -= V2::splat(MARGIN);
        extent.max += V2::splat(MARGIN);

        Self {
            extent,
            package,
            bank,
            blocks: grid.blocks,
            wires: grid.wires,
            junctions: grid.junctions,
            legends: grid.legends,
            routes: grid.routes,
            shown,
        }
    }
}

/// Where an attachment's wire leaves the spine, from its top.
fn attachment_port(node: &Node) -> f32 {
    match node.form {
        Form::Bridge => node.children.first().map_or(PORT, Node::port),
        _ => node.port(),
    }
}

/// The columns right of the spine, from `first`: each as wide as its
/// widest block, and far enough from the next for the connectors' names
/// on the wires between them.
fn columns(tree: &[Node], first: f32) -> Vec<(f32, f32)> {
    let mut widths = vec![0.0_f32; COLUMNS];
    let mut vias = vec![0_usize; COLUMNS];

    fn visit(node: &Node, column: usize, widths: &mut [f32], vias: &mut [usize]) {
        if node.form == Form::Bridge {
            for child in &node.children {
                visit(child, column, widths, vias);
            }
            return;
        }

        if let Some(width) = widths.get_mut(column) {
            *width = width.max(node.width());
        }

        if let (Some(via), Some(room)) = (&node.via, column.checked_sub(1)) {
            vias[room] = vias[room].max(via.chars().count());
        }

        for child in &node.children {
            visit(child, column + 1, widths, vias);
        }
    }

    for node in tree {
        visit(node, 0, &mut widths, &mut vias);
    }

    let mut x = first;

    widths
        .iter()
        .zip(&vias)
        .map(|(&width, &via)| {
            let width = width.ceil();
            let column = (x, width);
            let span = if via > 0 {
                TRUNK + 4.0 + letters(via) + 6.0
            } else {
                SPAN
            };

            x += width + span.max(SPAN).ceil();
            column
        })
        .collect()
}

/// The diagram as it is placed: the blocks, the wires and what is lettered
/// on them.
#[derive(Debug, Default)]
struct Grid {
    columns: Vec<(f32, f32)>,
    spine: f32,
    blocks: Vec<Placed>,
    wires: Vec<Wire>,
    junctions: Vec<Junction>,
    legends: Vec<Legend>,
    routes: Vec<Route>,
}

/// The route to a block: its pieces, each with what it is set on the grid
/// by.
type Incoming = Vec<(Vec<V2>, Vec<V2>)>;

impl Grid {
    /// A wire through `points`, set on the grid by `snaps`.
    fn wire(&mut self, snaps: &[V2], points: Vec<V2>) {
        self.wires.push(Wire {
            snaps: snaps.to_vec(),
            points,
        });
    }

    /// Places what hangs off the spine with its wire leaving at `port`;
    /// the bottom of all it takes. The spine and its wires are set on the
    /// grid as the sheet's origin is.
    fn attach(&mut self, node: &Node, port: f32) -> f32 {
        let spine = self.spine;
        let column = self.columns[0].0;

        if node.form != Form::Bridge {
            let piece = vec![v(spine, port), v(column, port)];

            self.wire(&[], piece.clone());
            return self.place(node, 0, port, vec![(Vec::new(), piece)], &[]);
        }

        // A bridge on the wire, its address over it, and a trunk to what is
        // behind it if there is more than one.
        let x = spine + BRIDGE_AT;
        let half = BRIDGE / 2.0;
        let frame = Extent::new(v(x - half, port - half), v(x + half, port + half));
        let snaps = vec![v(frame.min.x, frame.max.y)];
        let piece = vec![v(spine, port), v(x - half, port)];

        self.blocks.push(Placed {
            group: node.group,
            form: node.form,
            source: node.source,
            merged: node.merged.clone(),
            frame,
            port: v(frame.min.x, port),
            lines: node.lines.clone(),
            more: node.more.clone(),
            balloon: node.balloon,
            snaps: snaps.clone(),
        });
        self.legends.push(Legend {
            snaps: snaps.clone(),
            at: v(x, port + half + 1.0),
            text: node.lines[0].clone(),
            anchor: Anchor::new(Horizontal::Centre, Vertical::Bottom),
        });
        self.wire(&[], piece.clone());

        self.children(
            node,
            -1,
            v(x + half, port),
            column - 8.0,
            vec![(Vec::new(), piece)],
            &snaps,
        )
    }

    /// Places `node` in column `column` with its port at `port`, reached by
    /// `incoming`, set on the grid inside what `within` sets; the bottom of
    /// it and all that hangs off it.
    fn place(
        &mut self,
        node: &Node,
        column: usize,
        port: f32,
        incoming: Incoming,
        within: &[V2],
    ) -> f32 {
        let (x, width) = self.columns[column];
        let top = port + node.port();
        let width = match node.form {
            Form::Terminal => node.width(),
            _ => width,
        };
        let frame = Extent::new(
            v(
                x,
                top - node.height()
                    + if matches!(node.form, Form::Screen { .. }) {
                        STAND
                    } else {
                        0.0
                    },
            ),
            v(x + width, top),
        );
        let mut snaps = within.to_vec();

        snaps.push(v(x, top));

        // A detail letters a line every half line, inside the frame.
        let room = ((frame.height() - 4.0) / DETAIL_LINE).floor() as usize;

        self.blocks.push(Placed {
            group: node.group,
            form: node.form,
            source: node.source,
            merged: node.merged.clone(),
            frame,
            port: v(x, port),
            lines: node.lines.clone(),
            more: node.more.iter().take(room).cloned().collect(),
            balloon: node.balloon,
            snaps: snaps.clone(),
        });

        if !node.gauges.is_empty() {
            let (snaps, pieces) = incoming.iter().cloned().unzip();

            self.routes.push(Route {
                gauges: node.gauges.clone(),
                pieces,
                snaps,
            });
        }

        let bottom = top - node.height();

        if node.children.is_empty() {
            return bottom;
        }

        let right = x + self.columns[column].1;
        let trunk = right + TRUNK;

        self.children(
            node,
            column as isize,
            v(right, port),
            trunk,
            incoming,
            &snaps,
        )
        .min(bottom)
    }

    /// Places `node`'s children in the column after `column`, the first
    /// level with `out`, the rest under it on a trunk at `trunk`, the wires
    /// to them set on the grid by `snaps`, as `node` is, and they inside
    /// it; the bottom of them all.
    fn children(
        &mut self,
        node: &Node,
        column: isize,
        out: V2,
        trunk: f32,
        incoming: Incoming,
        snaps: &[V2],
    ) -> f32 {
        let next = (column + 1) as usize;
        let x = self.columns[next].0;
        let mut cursor = out.y;
        let mut ports = Vec::new();

        for (index, child) in node.children.iter().enumerate() {
            let port = if index == 0 {
                out.y
            } else {
                cursor - SIBLINGS - child.port()
            };
            // An open terminal's wire ends at its circle.
            let end = if child.form == Form::Terminal {
                x + 0.5
            } else {
                x
            };
            let piece = if index == 0 {
                vec![out, v(end, port)]
            } else {
                vec![out, v(trunk, out.y), v(trunk, port), v(end, port)]
            };

            if index == 0 {
                self.wire(snaps, piece.clone());
            } else {
                self.wire(snaps, vec![v(trunk, port), v(end, port)]);
            }

            if let Some(via) = &child.via {
                self.legends.push(Legend {
                    snaps: snaps.to_vec(),
                    at: v(trunk + 4.0, port + 3.0),
                    text: via.clone(),
                    anchor: Anchor::new(Horizontal::Left, Vertical::Baseline),
                });
            }

            let mut route = incoming.clone();
            route.push((snaps.to_vec(), piece));

            cursor = self.place(child, next, port, route, snaps);
            ports.push(port);
        }

        // The first child's wire runs straight through the trunk's top.
        if ports.len() > 1 {
            self.trunk(trunk, &[out.y], &ports, snaps);
        }

        cursor
    }

    /// A trunk at `x` joining wires from the left at `left` and to the right
    /// at `right`, with a junction wherever three or more meet, set on the
    /// grid by `snaps`.
    fn trunk(&mut self, x: f32, left: &[f32], right: &[f32], snaps: &[V2]) {
        let ys: Vec<f32> = left.iter().chain(right).copied().collect();
        let (high, low) = ys.iter().fold((f32::MIN, f32::MAX), |(high, low), &y| {
            (high.max(y), low.min(y))
        });

        if high <= low {
            return;
        }

        self.wire(snaps, vec![v(x, high), v(x, low)]);

        let mut joins: Vec<f32> = ys.clone();
        joins.sort_by(|a, b| b.total_cmp(a));
        joins.dedup();

        for y in joins {
            let along = if y == high || y == low { 1 } else { 2 };
            let meeting = along + ys.iter().filter(|&&other| other == y).count();

            if meeting >= 3 {
                self.junctions.push(Junction {
                    snaps: snaps.to_vec(),
                    at: v(x, y),
                });
            }
        }
    }
}

/// The cores of one kind on a die, as they are drawn: how many, the
/// squares for them and the cores each stands for, and their side.
struct Squares {
    kind: Option<CoreKind>,
    cores: u32,
    squares: u32,
    each: u32,
    side: f32,
}

/// A core's side on a die: an efficient core's smaller, and every core's
/// on a die with many.
fn side(kind: Option<CoreKind>, squares: u32) -> f32 {
    if kind == Some(CoreKind::Efficient) || squares > 2 * ACROSS {
        7.0
    } else {
        10.0
    }
}

/// The width of a row of `n` squares of `kind`: efficient cores come in
/// clusters of four, a gap between.
fn row_width(kind: Option<CoreKind>, side: f32, n: u32) -> f32 {
    let n = n.min(ACROSS) as f32;
    let clusters = if kind == Some(CoreKind::Efficient) {
        ((n - 1.0) / 4.0).floor()
    } else {
        0.0
    };

    n * (side + 2.0) - 2.0 + clusters * 4.0
}

/// How many cores a square stands for, of `n` of a kind on a die whose
/// last cache is shared by `sharing` cores: one, unless there are more
/// than [`MOST_SQUARES`]; then the cores sharing a cache, if they are few
/// enough, else as many as it takes.
fn cluster(n: u32, sharing: u32) -> u32 {
    if n <= MOST_SQUARES {
        1
    } else if sharing > 1 && n.is_multiple_of(sharing) && n / sharing <= MOST_SQUARES {
        sharing
    } else {
        n.div_ceil(MOST_SQUARES)
    }
}

/// The size of `cpu`'s package, and how to lay it out with its top left
/// at a point and its wire to the spine at a height.
fn package(cpu: &Cpu) -> (V2, impl Fn(V2, f32) -> Package) {
    let packages = cpu.packages.max(1);
    // The last cache: each package's share of it, and how many cores share
    // one.
    let last = cpu
        .caches
        .iter()
        .max_by_key(|cache| (cache.level, cache.bytes));
    let instances = last.map_or(1, |cache| (cache.instances / packages).max(1));
    let cache = last.map(|cache| {
        format!(
            "L{} {}",
            cache.level,
            binary(cache.bytes * u64::from(instances))
        )
    });
    let kinds: Vec<(Option<CoreKind>, u32)> = if cpu.kinds.is_empty() {
        vec![(None, cpu.cores / packages)]
    } else {
        cpu.kinds
            .iter()
            .map(|cores| (Some(cores.kind), cores.cores / packages))
            .collect()
    };
    let squares: Vec<Squares> = kinds
        .into_iter()
        .map(|(kind, cores)| {
            let cores = cores.max(1);
            let each = cluster(cores, cpu.cores / packages / instances);
            let squares = cores.div_ceil(each);

            Squares {
                kind,
                cores,
                squares,
                each,
                side: side(kind, squares),
            }
        })
        .collect();
    let cores_width = squares
        .iter()
        .map(|kind| row_width(kind.kind, kind.side, kind.squares))
        .fold(0.0, f32::max);
    // Every row of cores as far from the next, of whichever kind: the die
    // kept short, for the laptop's detail window to hold its cores and
    // cache at twice the view's scale.
    let cores_height: f32 = squares
        .iter()
        .map(|kind| kind.squares.div_ceil(ACROSS) as f32 * (kind.side + 2.0) - 2.0)
        .sum::<f32>()
        + 2.0 * (squares.len() as f32 - 1.0);
    let band = if cache.is_some() { 2.0 + BAND } else { 0.0 };
    // Dies one over another, or two to a row when there are more than two,
    // as wide as their cores and cache need and filling the frame.
    let across = if packages > 2 { 2 } else { 1 };
    let rows = packages.div_ceil(across) as f32;
    let across = across as f32;
    let needed = (cores_width + 8.0).max(match (&cache, packages) {
        (_, 1) => 0.0,
        // The cache's lettering and the ground it clears inside its band.
        (Some(cache), _) => letters(cache.chars().count()) + 16.0,
        (None, _) => 0.0,
    });
    let die_width = needed
        .max((96.0 - 8.0 - (across - 1.0) * 4.0) / across)
        .ceil();
    let die_height = (4.0 + cores_height + band + 4.0).ceil();
    let width = across * die_width + (across - 1.0) * 4.0 + 8.0;
    let height = 18.0 + rows * die_height + (rows - 1.0) * 4.0 + 4.0;

    let layout = move |corner: V2, port: f32| {
        let frame = Extent::new(
            v(corner.x, corner.y - height),
            v(corner.x + width, corner.y),
        );
        let dies = (0..packages)
            .map(|n| {
                let (row, column) = ((n / across as u32) as f32, (n % across as u32) as f32);
                let left = frame.min.x + 4.0 + column * (die_width + 4.0);
                let top = frame.max.y - 18.0 - row * (die_height + 4.0);
                let die = Extent::new(v(left, top - die_height), v(left + die_width, top));
                let mut cores = Vec::new();
                let mut y = die.max.y - 4.0;

                for kind in &squares {
                    let side = kind.side;

                    for row in 0..kind.squares.div_ceil(ACROSS) {
                        let in_row = (kind.squares - row * ACROSS).min(ACROSS);
                        let mut x = (die.centre().x
                            - row_width(kind.kind, side, kind.squares) / 2.0)
                            .round();

                        for i in 0..in_row {
                            if kind.kind == Some(CoreKind::Efficient) && i > 0 && i % 4 == 0 {
                                x += 4.0;
                            }

                            // The last square stands for those left.
                            let before = (row * ACROSS + i) * kind.each;

                            cores.push(Core {
                                square: Extent::new(v(x, y - side), v(x + side, y)),
                                kind: kind.kind,
                                cores: kind.each.min(kind.cores - before),
                            });
                            x += side + 2.0;
                        }

                        y -= side + 2.0;
                    }
                }

                let cache = cache.clone().map(|text| {
                    let top = die.min.y + 4.0 + BAND;
                    (
                        Extent::new(v(die.min.x + 4.0, die.min.y + 4.0), v(die.max.x - 4.0, top)),
                        text,
                    )
                });

                Die {
                    frame: die,
                    cores,
                    cache,
                }
            })
            .collect();

        Package {
            frame,
            dies,
            port: v(frame.max.x, port),
        }
    };

    (v(width, height), layout)
}

/// The modules a row of a bank has, of `modules`: two, or four when there
/// are more than eight.
fn per_row(modules: usize) -> usize {
    if modules > 8 { 4 } else { 2 }
}

/// The memory's size with `modules` sticks in it.
fn bank(modules: usize) -> V2 {
    let modules = modules.min(MOST_MODULES);
    let rows = modules.div_ceil(per_row(modules)) as f32;

    v(96.0, 20.0 + rows * 12.0)
}

/// The sticks of `modules` modules in a bank framed by `frame`: rows of
/// them across its middle, the last row centred if it is short.
fn sticks(frame: Extent, modules: usize) -> Vec<Extent> {
    let modules = modules.min(MOST_MODULES);
    let across = per_row(modules);
    // Two sticks 40 wide 6 apart, or four 20 wide 2 apart: 86 either way.
    let (width, gap) = if across == 2 {
        (40.0, 6.0)
    } else {
        (20.0, 2.0)
    };

    (0..modules)
        .map(|i| {
            let (row, column) = (i / across, i % across);
            let in_row = (modules - row * across).min(across) as f32;
            let left = frame.centre().x - (in_row * width + (in_row - 1.0) * gap) / 2.0;
            let x = left + column as f32 * (width + gap);
            let top = frame.max.y - 20.0 - row as f32 * 12.0;

            Extent::new(v(x, top - 8.0), v(x + width, top))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_hangs_off_its_root_bus_in_order() {
        let machine = Machine::fixture();
        let (tree, shown) = tree(&machine);
        let groups: Vec<Group> = tree.iter().map(|node| node.group).collect();

        // The graphics, the drive and the Ethernet adapter behind their
        // root ports, the Wi-Fi, the USB host.
        assert_eq!(
            groups,
            [
                Group::Graphics,
                Group::Bridge,
                Group::Network,
                Group::Bridge,
                Group::Usb
            ]
        );
        assert_eq!(tree[1].children[0].group, Group::Drive);
        assert_eq!(tree[1].lines, ["00:06.0"]);
        assert_eq!(tree[3].children[0].group, Group::Network);
        // Five devices and the two root ports they are behind, of eleven.
        assert_eq!(shown, 7);

        let displays: Vec<_> = tree[0].children.iter().map(|c| c.form).collect();
        assert_eq!(
            displays,
            [
                Form::Screen { internal: true },
                Form::Screen { internal: false },
                Form::Terminal
            ]
        );

        // The receiver, the camera, the Bluetooth radio and the hub with
        // the card reader on it.
        let usb = &tree[4];
        let classes: Vec<_> = usb.children.iter().map(|c| c.lines[0].as_str()).collect();
        assert_eq!(classes, ["INPUT", "CAMERA", "WIRELESS", "HUB"]);
        assert_eq!(usb.children[3].children[0].lines, ["STORAGE"]);
    }

    /// No two blocks overlap, and every block's lettering fits inside it at
    /// the laptop's scale, and its lettering for a detail at twice that.
    #[test]
    fn blocks_are_apart_and_hold_their_lettering() {
        let machine = Machine::fixture();
        let diagram = Diagram::new(&machine).unwrap();
        let size = v(diagram.extent.width(), diagram.extent.height());

        assert!(size.x <= BUDGET.x && size.y <= BUDGET.y, "{size}");
        assert_apart(&diagram);

        for block in &diagram.blocks {
            if matches!(block.form, Form::Block | Form::Screen { .. }) {
                let frame = block.frame;
                assert!(
                    letters(widest(&block.lines)) + 2.0 * PAD <= frame.width() + 0.5,
                    "{:?}",
                    block.lines
                );
                assert!(
                    letters(widest(&block.more).min(DETAIL_ROOM)) / 2.0 + 2.0 <= frame.width(),
                    "{:?}",
                    block.more
                );
                assert!(
                    block.more.len() as f32 * DETAIL_LINE <= frame.height(),
                    "{:?}",
                    block.more
                );
            }
        }
    }

    fn assert_apart(diagram: &Diagram) {
        let mut frames: Vec<Extent> = diagram.blocks.iter().map(|b| b.frame).collect();
        frames.push(diagram.package.frame);
        frames.extend(diagram.bank.as_ref().map(|b| b.frame));

        for (i, a) in frames.iter().enumerate() {
            assert!(
                a.min.x >= diagram.extent.min.x && a.max.x <= diagram.extent.max.x,
                "{a:?}"
            );
            assert!(
                a.min.y >= diagram.extent.min.y && a.max.y <= diagram.extent.max.y,
                "{a:?}"
            );

            for b in &frames[i + 1..] {
                let apart = a.max.x <= b.min.x
                    || b.max.x <= a.min.x
                    || a.max.y <= b.min.y
                    || b.max.y <= a.min.y;
                assert!(apart, "{a:?} and {b:?} overlap");
            }
        }
    }

    /// A machine with more on its buses than a sheet holds is folded until
    /// it fits: its open connectors left out, its busiest hub counted.
    #[test]
    fn a_busy_machine_is_folded_to_fit() {
        let mut machine = Machine::fixture();
        let template = machine.usb[1].clone();

        // Fourteen hubs, each with a device on it: none alike.
        for port in 21..=34 {
            let hub = format!("1-{port}");

            machine.usb.push(UsbDevice {
                port: hub.clone(),
                class: 0x09,
                ..template.clone()
            });
            machine.usb.push(UsbDevice {
                port: format!("{hub}.1"),
                parent: Some(hub),
                ..template.clone()
            });
        }

        let diagram = Diagram::new(&machine).unwrap();
        let size = v(diagram.extent.width(), diagram.extent.height());

        assert!(size.x <= BUDGET.x && size.y <= BUDGET.y, "{size}");
        assert!(!diagram.blocks.iter().any(|b| b.form == Form::Terminal));
        assert!(diagram.blocks.iter().any(|b| b.source == Source::Summary));
        assert_apart(&diagram);
    }

    /// Every route starts at the package and ends at its block, through the
    /// wires drawn.
    #[test]
    fn traffic_runs_from_the_package_to_its_block() {
        let machine = Machine::fixture();
        let diagram = Diagram::new(&machine).unwrap();

        // The drive, and both network adapters.
        assert_eq!(diagram.routes.len(), 3);

        for route in &diagram.routes {
            let start = route.pieces[0][0];
            let end = *route.pieces.last().unwrap().last().unwrap();
            let block = diagram
                .blocks
                .iter()
                .find(|block| block.port == end)
                .expect("A block at the route's end");

            assert_eq!(start, diagram.package.port);
            assert!(block.form == Form::Block);
        }
    }

    /// A desktop: cores all alike, and a SATA controller with two drives
    /// on it, drawn as the controller with the drives behind it, each with
    /// its traffic.
    #[test]
    fn a_controller_with_drives_has_them_behind_it() {
        let mut machine = Machine::fixture();
        let sata = PciAddress::new(0, 0, 0x17, 0);
        let mut device = machine.pci[0].clone();

        device.address = sata;
        device.class = 0x010601;
        machine.pci.push(device);

        let cpu = machine.cpu.as_mut().unwrap();
        cpu.kinds.clear();
        cpu.cores = 12;

        for (name, rotational) in [("sda", false), ("sdb", true)] {
            let mut drive = machine.drives[0].clone();

            drive.name = name.into();
            drive.kind = DriveKind::Sata;
            drive.rotational = rotational;
            drive.pci = Some(sata);
            machine.drives.push(drive);
        }

        let (tree, _) = tree(&machine);
        let controller = tree
            .iter()
            .find(|node| node.source == Source::Pci(sata))
            .expect("The SATA controller");
        let drives: Vec<&str> = controller
            .children
            .iter()
            .map(|drive| drive.lines[0].as_str())
            .collect();

        assert_eq!(controller.lines, ["SATA", "2 DRIVES"]);
        assert_eq!(drives, ["SSD", "HARD DISK"]);

        let diagram = Diagram::new(&machine).unwrap();

        // Cores all alike are of no kind, one square each.
        let cores = &diagram.package.dies[0].cores;

        assert_eq!(cores.len(), 12);
        assert!(
            cores
                .iter()
                .all(|core| core.kind.is_none() && core.cores == 1)
        );
        assert_eq!(diagram.routes.len(), 5);
        assert_apart(&diagram);
    }

    /// A controller with more drives than the laptop's view holds, none
    /// alike: its last drives are counted in a block of their own, with no
    /// traffic.
    #[test]
    fn a_controller_with_many_drives_is_folded_to_fit() {
        let mut machine = Machine::fixture();
        let sata = PciAddress::new(0, 0, 0x17, 0);
        let mut device = machine.pci[0].clone();

        device.address = sata;
        device.class = 0x010601;
        machine.pci.push(device);

        for n in 0..16 {
            let mut drive = machine.drives[0].clone();

            drive.name = format!("sd{}", char::from(b'a' + n));
            drive.kind = DriveKind::Sata;
            drive.rotational = true;
            drive.pci = Some(sata);
            drive.bytes = (u64::from(n) + 1) * 1_000_000_000_000;
            machine.drives.push(drive);
        }

        let diagram = Diagram::new(&machine).unwrap();
        let size = v(diagram.extent.width(), diagram.extent.height());
        let summary = diagram
            .blocks
            .iter()
            .find(|block| block.source == Source::Summary)
            .expect("The drives counted");
        let drawn = diagram
            .blocks
            .iter()
            .filter(|block| matches!(block.source, Source::Drive(_)))
            .count();

        assert!(size.x <= BUDGET.x && size.y <= BUDGET.y, "{size}");
        assert_eq!(summary.group, Group::Drive);
        assert_eq!(
            summary.lines,
            [format!("+{} MORE", 17 - drawn)],
            "{drawn} drawn"
        );
        // The NVMe drive's route, and those of the SATA drives drawn.
        assert_eq!(diagram.routes.len(), drawn + 2);
        assert_apart(&diagram);
    }

    /// The two-socket server: its alike drives, its root ports with a
    /// drive each and its network adapters' ports drawn as one block each,
    /// standing for them all with their traffic together, until it fits.
    #[test]
    fn a_servers_alike_devices_are_drawn_as_one() {
        let machine = crate::machine::Fixture::Server.machine();
        let diagram = Diagram::new(&machine).unwrap();
        let size = v(diagram.extent.width(), diagram.extent.height());
        let lettered = |first: &str| {
            diagram
                .blocks
                .iter()
                .find(|block| block.lines[0] == first)
                .unwrap_or_else(|| panic!("{first}"))
        };

        assert!(size.x <= BUDGET.x && size.y <= BUDGET.y, "{size}");
        assert!(!diagram.blocks.iter().any(|b| b.source == Source::Summary));
        assert_apart(&diagram);

        let nvme = lettered("NVMe SSD ×8");
        let disks = lettered("HARD DISK ×8");
        let ports = lettered("8 PORTS");

        assert_eq!(nvme.lines[1], "3.8 TB EACH");
        assert_eq!((nvme.merged.len(), disks.merged.len()), (7, 7));
        assert_eq!((ports.form, ports.merged.len()), (Form::Bridge, 7));
        assert_eq!(lettered("ETHERNET ×4").lines[1], "1 OF 4 UP");
        assert_eq!(lettered("ETHERNET ×2").lines[1], "10 GBIT/S");

        // Every drive's traffic runs to the block it is drawn in.
        let gauged: usize = diagram.routes.iter().map(|route| route.gauges.len()).sum();
        assert_eq!(gauged, machine.drives.len() + 6);
    }

    /// A machine with a graphics card driving three displays and a USB
    /// host as busy: the USB host's devices are counted before a display
    /// is, so every display is drawn.
    #[test]
    fn displays_are_the_last_folded() {
        use crate::machine::Fixture;

        let mut machine = Fixture::Desktop.machine();
        let monitor = machine
            .connectors
            .iter()
            .find_map(|connector| connector.panel.clone())
            .unwrap();

        for connector in &mut machine.connectors {
            connector.panel.get_or_insert_with(|| monitor.clone());
        }

        // As many devices on the USB host's root hub as displays on the
        // graphics card, none alike.
        let template = machine
            .usb
            .iter()
            .find(|d| d.parent.is_some())
            .unwrap()
            .clone();

        for (n, class) in [0x01, 0x0e].into_iter().enumerate() {
            machine.usb.push(UsbDevice {
                port: format!("{}{n}", template.port),
                class,
                ..template.clone()
            });
        }

        let (tree, _) = tree(&machine);
        let screens = |diagram: &Diagram| {
            diagram
                .blocks
                .iter()
                .filter(|b| matches!(b.form, Form::Screen { .. }))
                .count()
        };
        let diagram = Diagram::new(&machine).unwrap();
        let size = v(diagram.extent.width(), diagram.extent.height());

        fn find(nodes: &[Node], group: Group) -> Option<&Node> {
            nodes.iter().find_map(|node| {
                (node.group == group)
                    .then_some(node)
                    .or_else(|| find(&node.children, group))
            })
        }

        assert_eq!(find(&tree, Group::Usb).unwrap().children.len(), 4);
        assert_eq!(find(&tree, Group::Graphics).unwrap().children.len(), 4);
        assert!(
            Diagram::of(&machine, machine.cpu.as_ref().unwrap(), &tree, 0)
                .extent
                .height()
                > BUDGET.y
        );
        assert!(size.x <= BUDGET.x && size.y <= BUDGET.y, "{size}");
        assert_eq!(screens(&diagram), machine.displays().count());
        assert!(
            diagram
                .blocks
                .iter()
                .any(|b| b.source == Source::Summary && b.group == Group::Usb)
        );
    }

    /// An NVMe drive with nothing on it mounted (a second system's, a
    /// spare) is drawn all the same.
    #[test]
    fn a_drive_with_nothing_mounted_is_drawn() {
        let mut machine = Machine::fixture();

        for partition in &mut machine.drives[0].partitions {
            partition.filesystem = None;
        }

        let diagram = Diagram::new(&machine).unwrap();
        let drive = diagram
            .blocks
            .iter()
            .find(|block| block.source == Source::Drive(0))
            .expect("The drive");

        assert_eq!(drive.more, ["NVMe SSD 1TB", "1 TB, 2 PARTITIONS"]);
    }

    /// A detail's line names a device by what tells it from another: not
    /// the platform it is part of, not the word every device has, not the
    /// standard where the model would not fit beside it.
    #[test]
    fn a_detail_names_a_device_by_its_model() {
        let mut device = Machine::fixture().pci[0].clone();
        let mut called_as = |name: &str| {
            device.name = Some(name.into());
            called(&device)
        };

        assert_eq!(
            called_as("Generic 9 Series Processors USB 3.2 xHCI Host Controller"),
            "USB 3.2 XHCI HOST"
        );
        assert_eq!(called_as("Ethernet Controller X100-V"), "ETHERNET X100-V");
        assert_eq!(
            called_as("Wi-Fi 7(802.11be) WX700*/WX710* 2x2"),
            "WI-FI 7 WX700 2X2"
        );
        assert_eq!(called_as("SATA Controller"), "SATA");
    }

    #[test]
    fn a_machine_with_nothing_on_its_buses_has_no_diagram() {
        let mut machine = Machine::fixture();
        machine.pci.clear();

        assert!(Diagram::new(&machine).is_none());
    }
}
