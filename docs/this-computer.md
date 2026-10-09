# "This computer": sheets of the machine the screensaver runs on

Besides its seven designed subjects, the screensaver draws the machine it runs
on, so everyone's sheets are their own hardware: its topology, its displays and
its cooling. This is their design: what is read and what never is, how the
sheets are built, and where they fall short. The screensaver as a whole is
described in [`screensaver.md`](screensaver.md).

## Where it lives

- `layershell/crates/screensaver/src/machine/`: the inventory (what the machine
  is) and its sensors (what it is doing now). No drawing here.
- `layershell/crates/screensaver/src/subjects/computer/`: the sheets, each a
  [`Subject`] like the others, built from an inventory.
- `subjects::all()` takes the inventory to draw from. `quadrille-screensaver`
  gets a global `--machine <live|fixture>` option, `live` by default: `run`,
  `render` and `bench` draw this machine; tests and every image that is
  committed draw the fixture. The other kinds of machine the sheets must read
  well on are fixtures too: `--machine fixture-desktop`, `fixture-server` and
  `fixture-vm` (`machine::Fixture`), and the tests draw every sheet of each.
  `render --output laptop|ultrawide` draws for the desk's two outputs and
  `render --size WIDTHxHEIGHT[@SCALE][:MM]` for any other (`1366x768`,
  `2880x1800@2:302`, its panel's width in millimetres guessed when left
  out), so a sheet can be seen as other people's displays show it.

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
- `/sys/class/hwmon/*` (live, see below): temperatures and fan speeds, by chip
  name; inputs that read like nothing is connected (0 °C or below, 115 °C or
  above) are left out.

### Privacy, without exception

- Never read or keep serial numbers, MAC addresses, UUIDs, hostnames, user
  names, Wi-Fi network names, mount point paths under `/home`, or anything
  from `/sys/class/dmi/id/*serial*`, `product_uuid`, or `address` files. The
  inventory types have no field that could hold them, and a test checks a
  fake tree's serials and MACs never reach the drawn marks.
- Nothing drawn from the live machine is committed: README screenshots, test
  snapshots and anything else in the repo come from the fixture.
- The fixtures are synthetic: a plausible laptop, desktop tower, server and
  virtual machine whose parts are generic ("NVME SSD 1 TB", "16 CORES"), not
  this one's.

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
   the CPU with a die for each package and its cores, memory, the PCIe tree
   (bridges, the GPU, the drives, the network controllers), the USB tree,
   the displays on the GPU's connectors. What moves: traffic, dots along the
   buses at rates from the measured I/O. Each detail draws what its block
   has more to it than lettering: a drive's partitions, a graphics card's
   sockets, a display's pixels dimensioned, a USB host's ports, a network
   adapter's jack or antenna and its rates. Folded for the laptop's view,
   and further for a smaller one (`Subject::fitted`).
2. **Displays** (to scale): every display, the machine's own and those
   connected, drawn at their true physical sizes (from the EDID, as the
   scale bar already trusts), side by side or in rows, whichever the view
   draws larger; dimensioned in millimetres with the diagonal in inches and
   the pixel pitch, each named under its width by its connector and
   resolution; the detail magnifies a corner to show the pixel grid. Laid
   out for the view it is drawn in (`Subject::fitted`), its room in that
   view's pixels.
3. **Cooling** (a diagram, or to scale where sizes are known): the fans as
   rotors turning at their measured rpm (or, with no fan sensor, the cooling
   path without motion), the heat sources (CPU, GPU, drives, memory) with their
   temperatures, the battery's charge; readings are the live values. A
   laptop's is drawn with blowers and a heat pipe, a desktop's or server's as
   a tower with case fans and heatsinks; under a wide plan, on a wide display,
   a chart of the last two minutes. Its sources are laid out in the columns
   that fit its view best, the laptop's or a smaller one.

Later, if there is room: a storage sheet (each drive's partitions as a
proportional bar, filesystems by type).

## Limits

- Checked on a laptop and on the made-up desktop, server and virtual machine,
  on the author's two displays and on common sizes from 1366 × 768 to
  2880 × 1800; other machines may lay out in ways none of those does.
- On a view smaller than about 1366 × 768 at scale 1 (or 1920 × 1200 at 1.25),
  the topology is drawn at about 0.7 to 0.8 of its size, as only a hub's devices
  fold to save width; the side column's detail window is then only a few dozen
  pixels tall, as it is for every subject.
- A fan that reads 0 rpm cannot be told from an empty header, so a fan not seen
  turning is drawn in phantom; the sheets are laid out when the screensaver
  starts, so a fan that only starts turning later is drawn as it was then.
- Drive temperatures and I/O rates appear a second or so after the screensaver
  starts, as they are read in the background; a one-off `render` shows them as
  `--`.
- Filesystems on a whole disk without a partition table, and ZFS datasets, are
  not found.
- A balloon's leader can cross the wire its block's devices hang from.
