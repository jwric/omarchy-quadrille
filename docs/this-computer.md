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
