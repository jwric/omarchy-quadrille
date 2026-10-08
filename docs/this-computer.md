# "This computer": sheets of the machine the screensaver runs on

The user's idea (2026-10-07): besides the seven designed subjects, the
screensaver draws the machine it runs on, so every user's sheets are their own
hardware. This file is the design, the rules and the running status for the
work; whoever picks it up (an agent of the build workflow, or a later session)
reads it first and appends to **Status** below when they finish a step.

## Where it lives

- `layershell/crates/screensaver/src/machine/`: the inventory (what the machine
  is) and its sensors (what it is doing now). No drawing here.
- `layershell/crates/screensaver/src/subjects/computer/`: the sheets, each a
  [`Subject`] like the others, built from an inventory.
- `subjects::all()` takes the inventory to draw from. `quadrille-screensaver`
  gets a global `--machine <live|fixture>` option, `live` by default: `run`,
  `render` and `bench` draw this machine; tests and every image that is
  committed draw the fixture.

## The inventory (`machine/`)

Read once at start, read-only, no root, no subprocesses: `/sys` and `/proc`
files only, through a root path so tests can point it at a fake tree in a
temporary directory. Every source is optional: a field the machine does not
expose is absent, and a sheet with too little to show is left out of the
schedule rather than drawn empty.

Sources (what the development machine, a laptop, has is noted; other
machines differ):

- `/sys/class/dmi/id/`: `sys_vendor`, `product_name`, `chassis_type` (desktop,
  laptop, ...), `board_vendor`, `board_name`, `bios_version`.
- `/proc/cpuinfo` and `/sys/devices/system/cpu/`: model name, packages, cores
  and threads, core types where the kernel says (`/sys/devices/cpu_core`,
  `cpu_atom` on hybrid Intel), cache sizes from `cpu0/cache/index*/`, base and
  maximum frequency from `cpufreq`.
- `/proc/meminfo` for the total; the DIMMs themselves only as far as the
  kernel shows them without root (here `spd5118` hwmon devices on i2c, one per
  module, with a temperature); never `dmidecode`.
- `/sys/block/*`: drives (model, size, rotational, the `nvme` transport),
  their partitions and sizes; filesystems by type only, from `/proc/mounts`.
- `/sys/bus/pci/devices/*`: class, vendor and device ids, the bridge each sits
  behind (the topology), names from `pci.ids` when it is installed
  (`/usr/share/hwdata/pci.ids`), else the class name.
- `/sys/bus/usb/devices/*`: the tree of hubs and devices, speeds, product
  strings.
- `/sys/class/drm/card*-*`: connectors, which are connected, their EDID; the
  physical size of each display comes from `quadrille_desktop::physical`
  (`PhysicalSize::resolve`), the same as the sheets' scales.
- `/sys/class/power_supply/*`: battery design and full capacity, cycle count,
  chemistry; the charger.
- `/sys/class/net/*`: interfaces by kind (ethernet, wireless), link speed.
- `/sys/class/hwmon/*` (live, see below): temperatures, fan speeds, voltages
  and power, by chip name.

### Privacy, without exception

- Never read or keep serial numbers, MAC addresses, UUIDs, hostnames, user
  names, Wi-Fi network names, mount point paths under `/home`, or anything
  from `/sys/class/dmi/id/*serial*`, `product_uuid`, or `address` files. The
  inventory types have no field that could hold them, and a test checks a
  fake tree's serials and MACs never reach the drawn marks.
- Nothing drawn from the live machine is committed: README screenshots, test
  snapshots and anything else in the repo come from the fixture.
- The fixture is synthetic: a plausible laptop whose parts are generic
  ("NVME SSD 1 TB", "16 CORES"), not this one's.

### Sensors

Live values (temperatures, fan rpm, battery charge, I/O rates from
`/proc/diskstats` and `/sys/class/net/*/statistics`) are sampled at most once a
second off the drawing path (a background thread or a sampler that only
re-reads when a second has passed) and read by the sheets from a shared
snapshot, so a frame never waits on sysfs. In fixture mode the values are a
pure function of time, so renders are reproducible.

## The sheets (`subjects/computer/`)

Each is a `Subject` like the designed ones and follows the same conventions:
whole-pixel lettering in the theme's roles, parts lit in turn with a detail
view, balloons placed automatically (`Placement::Auto`), no overlaps (the
existing placement and lettering tests apply), readings along the top. Part
names at most 14 characters and the parts list's value column at most 10 (a
subjects test enforces it): the full model goes in the part's specification,
which has room.

1. **Topology** (a diagram, `scaled: false`): the machine as a block diagram:
   the CPU package with its cores, memory, the PCIe tree (bridges, the GPU,
   the drives, the network controllers), the USB tree, the displays on the
   GPU's connectors. What moves: traffic, dots along the buses at rates from
   the measured I/O.
2. **Displays** (to scale): every display, the machine's own and those
   connected, drawn side by side at their true physical sizes (from the EDID,
   as the scale bar already trusts), dimensioned in millimetres with the
   diagonal in inches and the pixel pitch; the detail magnifies a corner to
   show the pixel grid.
3. **Cooling** (a diagram, or to scale where sizes are known): the fans as
   rotors turning at their measured rpm (or, with no fan sensor, the cooling
   path without motion), the heat sources (CPU, GPU, drives, memory) with their
   temperatures, the battery's charge; readings are the live values.

Later, if there is room: a storage sheet (each drive's partitions as a
proportional bar, filesystems by type).

## Rules for the work

- Headless only: `quadrille-screensaver render --machine fixture|live`,
  `bench`, tests. Never open a window or a nested compositor on the user's
  session; never edit `~/.config/hypr`, `/usr/share`, or anything outside the
  repo; never inject input.
- Test first where it can be: the inventory against fake sysfs trees, the
  sheets against the fixture.
- Match the code round it: the comment and doc style (plain prose, what and
  why), names, `glam`/`textwrap`/`jiff`-style crates over hand-rolled code.
- Commit each finished step, plain messages in the repo's style, with no
  Co-Authored-By or other trailer; never push.
- Keep `cargo test -p quadrille-screensaver` and
  `cargo clippy -p quadrille-screensaver --all-targets` clean (the clippy
  warnings in `desktop` and `iced_layer` predate this).

## Status

- 2026-10-07: design written; the build workflow starts with the inventory.
- 2026-10-07: the inventory and its plumbing, no sheet yet. `machine/` reads the chassis, the CPU (core kinds, caches, clocks), memory (the total; modules by their `spd5118`/`jc42` sensors), drives with partitions and filesystem types, the PCI tree named from `pci.ids`, the USB tree, the connectors with their displays measured through `PhysicalSize`, batteries and chargers, network interfaces by kind, and the hwmon sensors, each placed on its part (`Site`); `Tree` refuses every file of identifiers, and a test plants serials, MACs and UUIDs in a fake tree and none reaches the inventory or a sample. `Machine::sample(t)` is the latest of a background thread's once-a-second reads (the fixture's: a function of `t`); `--machine live|fixture` and `subjects::all(&Machine)` are in, tests on the fixture, which is also built as a sysfs tree and read back. Open: `machine` carries `#![allow(dead_code)]` until the sheets use it; a fan reading 0 cannot be told from an absent one (this laptop's controller lists four, two always 0), so the cooling sheet needs a rule; a cold read costs about 100 ms more for the ACPI battery, before the first frame; an NVMe temperature read each second may keep the drive out of its deepest sleep.
- 2026-10-07: the topology sheet, `subjects/computer/` (`topology`, QD-C-0001, in a new COMPUTING domain). The tree is laid out from the inventory left to right on the grid: the CPU package with its cores by kind and its last cache, the memory and its bus, the root bus as a spine with bridges on the wires by address (nested bridges and a VMD folded into the one on the root bus), graphics with displays on their connectors and open outputs as terminals, drives (an NVMe drive one block with its controller, a SATA controller with its drives behind it), network adapters, USB hosts with their hubs and devices. Traffic dots run at the measured drive and network rates, inward for reads and receives. Eight parts are lit in turn, each detail lettering what the view has no room for; the readings are the CPU temperature, disk and network rates and the battery. Units are the laptop's virtual pixels, so lettering fits its blocks at the laptop's scale or larger; a tree too large for the laptop's view (a test ties the budget to it) is folded: a hub's devices counted in it when too wide, open outputs dropped and then a busy hub's last devices counted when too tall. `machine` now allows dead code item by item, on what the displays and cooling sheets will read. Open: this laptop's tree is folded to fit (open outputs dropped, two USB devices counted); DISK and NET read only from the second sample, so a one-shot `render --machine live` shows neither; no README row or screenshot yet; on the ultrawide a block's detail magnifies it about 4.5×, its lettering widely spaced.
- 2026-10-07: the displays sheet, `subjects/computer/displays.rs` (`displays`, QD-C-0002, COMPUTING, to scale). Every connected display whose size is known (not `PhysicalSize::estimated`) is drawn at its true size, side by side on one baseline: its active area dimensioned in millimetres, its diagonal in inches across it, its connector and resolution lettered at its centre; the room round and between them is a share of the row's length. Each display is a part (its connector, its diagonal as the value; output, model with the year, pixels and refresh, diagonal, pitch with PPI, aspect in its specification), and its detail magnifies the top left corner, pixel (0, 0): the frame tinted, the pixel grid at the display's own pitch with every centre dotted, and the pitch dimensioned centre to centre. A scan line runs down each display at its refresh rate slowed 240 times. Readings: the displays, total pixels and area, and the graphics' temperature where a sensor is on it. The draft gains `in_main` for geometry the main view draws alone: a display's edges are hundreds of times longer than its detail is wide, so the detail draws its corner itself rather than magnifying them. The displays sheet's `allow(dead_code)` items in `machine` are gone. Open: the scale is a preferred one, so a row of displays wider than about a metre (a laptop beside a large monitor) is drawn at 1:10 on the laptop, and the ultrawide draws the fixture at 1:5 with room to spare; the detail's magnification is the sheet's, about 50:1 on the laptop and 100 to 250:1 on the ultrawide; displays are drawn as their EDID describes them, landscape, as the inventory knows nothing of rotation; no README row or screenshot yet.
- 2026-10-07: the cooling sheet, `subjects/computer/cooling.rs` (`cooling`, QD-C-0003, COMPUTING, a diagram: SCHEMATIC PLAN). Laid out like a laptop's base from above, vents at the top: up to four fans in the corners (half a side), outlets inward, a fin stack under each vent; a heat pipe through every fin stack drops to the CPU and any GPU; what the air cools (memory, drives, network adapters, other devices, the board, the battery) in rows between the fans, each giving its heat by an arrow to the air drawn up the side from the intakes in the base to the nearest fan. Each source is a block with its hottest temperature and a thermometer per sensor (up to four; the processor's other sensors, its cores, as squares tinted by temperature); the battery shows its charge and power. Fans are blowers (scroll, forward-curved impeller) on a laptop and axial in square frames on a desktop or server; rotors turn at their measured rpm slowed 150 times, the angle carried from frame to frame because the speed changes with each sample (from `t` alone when a draw is not the next frame); the rule for a fan reading 0 is that a fan not yet seen turning is drawn in phantom. Dots run along the intakes, exhausts and heat arrows as far as their fan has turned, and along the pipe more densely the hotter its source; with no fan measured nothing moves and the pipe ends in a fin stack. Parts by kind (the fixture: CPU, MEMORY, DRIVE, BOARD, BATTERY, FAN), lit in turn; a block's detail magnifies its gauges with each value, a fan's its rotor with its direction of turning; readings are the CPU (and GPU) temperature, the first two turning fans' rpm and the battery. The sources fold into two columns between fans on both sides or one, whichever fits the laptop's view or overflows it least. Shared with the topology sheet now in `computer/mod.rs`: `flow` (its traffic dots) and `rows` with the specification's limits. The title block's third row is rebalanced (UNIT 3, SHEET 5, DATE 6, DRAWN 6 of 20) so `10 OF 10` fits the laptop's column; `machine` allows dead code only on `pci_behind` now. Open: this machine's controller lists four fans, two turning under load and two never, drawn in phantom; with them and six air-cooled sources its plan is about a tenth taller than the laptop's view, so the laptop draws it at about 0.92; a headless render of one moment starts each rotor from `t`, as no frame came before; positions are schematic, as the inventory knows none; the inventory reads no battery temperature; no README row or screenshot yet (README.md and main.rs still carry someone else's uncommitted `record` command).
- 2026-10-07: review fixes. Robustness: a lone NVMe drive with nothing mounted no longer panics the topology (its detail's last line is found, not indexed), and a filesystem is found through stacked holders (LVM on LUKS); an eMMC chip's boot and RPMB areas are not drives; the cooling sheet draws the fans turning at the first sample before empty headers and counts the modules and cores measured, not those drawn; a too tall diagram folds any busy block (a controller's last drives counted as `+n MORE`, with no traffic) and DRIVE counts every drive on the buses; a display whose size is estimated is lettered SIZE UNKNOWN on the topology; `displays.toml` keys by make and model match as Hyprland gives them (the maker's name from `pnp.ids`, the product code `0x%04X` when the display gives no name); `drivetemp` and `nvme` temperatures are read once a minute on the sampling thread and never before the first frame. Privacy: the identifiers are planted in one place (`machine::tests::plant`), and a test draws every sheet from that tree and finds none in the marks, cards or readings. Drawing, for every sheet: a detail's circle goes under the view's marks and its boundary under the detail's, so lettering reads across both, and the circle's letter takes the clearest of its four corners, on a knockout (letters move on the gears, Geneva and timer sheets too, off line work). Displays: heights are dimensioned 2.5 rooms out (gap 5, left margin 4.5) so their values clear the edges at 1:5 on both outputs (a test checks it), and the lettering at a display's centre is drawn after its scan line, which passes behind it. Topology: the die is shorter (rows of cores evenly spaced, 2 above the cache) and the processor's detail is centred on its cores and cache, which the laptop's window now holds at twice the view's scale (6 P + 10 E included; a test checks it); E cores are tinted on the view only; PCIe and NVMe keep their case through `lettered`. Cooling: a source's detail circle clears its name and bulbs; note 3 reads PHANTOM FANS. Left: balloons 3 and 7 stay in the room their blocks keep for them, by the topology's design (outside, they need more room between columns); the ultrawide's diagram details stay 3 to 3.7 times the view (the designed timer's are 5.8), since a cap is a sheet-wide rule that would change the timer; a filesystem on a whole disk or a ZFS dataset is not found; a one-shot `render --machine live` shows a drive's temperature as `-- °C`, as it shows no I/O rates; a diagram drawn more than about 1.15 times the laptop's view (a machine with little on its buses) still crops the die in the laptop's detail, which the sheet magnifies at least twice the view; the balloon planner is sensitive to small shifts of the layout (9-unit P cores sent balloon 5 across the diagram, so they stay 10). Then the docs: the README's Screensaver section describes the machine's sheets, with the fixture's topology on the ultrawide (`plugins/screenshots/screensaver-computer-topology-paper-ultrawide.png`).
