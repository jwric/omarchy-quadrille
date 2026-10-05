# quadrille on wlr-layer-shell: the spike and the panel host

Two stages. Stage 2, the panel host, is first; stage 1, the feasibility spike
that it grew from (the approaches compared, what the fork needs, fractional
scale), follows unchanged in substance.

## Stage 2: a panel host with a system monitor

`quadrille-bar` (`crates/bar`) is a bar on every output, and panels that are
summoned over a socket, on tiny-skia, drawn with quadrille's widgets. It runs
on `crates/iced_layer`, the layer-shell shell of stage 1, which gained a focus
grab, an output list and per-output surfaces for it.

### What it does

- **A bar per output**, each at that output's own scale: 25 virtual pixels
  tall, the compositor's exclusive zone, with a brand block, the clock and
  date, CPU/MEM/BAT gauges, a NET lamp and a SYS button. On the laptop
  (1.666667) a virtual pixel is 3 physical pixels and the bar is 1536 x 45
  logical (exact); on the ultrawide (1) it is 2 and the bar is 3440 x 50. Bars
  follow the compositor's output list: an output that goes takes its bar, one
  that comes gets one.
- **Panels**, one at a time, as overlay popups at the top right, below whatever
  has claimed the top of the output (exclusive zone 0, so the compositor places
  them under every bar, quadrille's and Omarchy's alike): `sysmon`, 160 x 305
  virtual pixels (a multiple of 5, so exact at 1.666667), and `demo`, which has
  a text field to test the keyboard with.
- **The control socket**, `$XDG_RUNTIME_DIR/quadrille-bar.sock` (or
  `$QUADRILLE_BAR_SOCKET`), for a Hyprland binding or an Omarchy menu action:

  ```sh
  quadrille-bar ctl toggle sysmon            # show it, or hide it if it is shown
  quadrille-bar ctl summon sysmon '{"output":"eDP-2"}'
  quadrille-bar ctl hide [sysmon|all]
  quadrille-bar ctl list                     # panels, outputs (with scale), theme
  quadrille-bar ctl reload-theme
  quadrille-bar ctl quit
  ```

  The answer comes back on the socket and is printed; an `error:` answer makes
  `ctl` exit with 1. Without an output in the JSON a panel opens on the output
  whose bar the pointer was last over, else the first. A panel whose output goes
  away moves to the first. The SYS button toggles `sysmon` on its own output.
- **Dismissal by a click outside**, through `hyprland_focus_grab_v1`; Escape
  and the X button close a panel as well.
- **The theme follows Omarchy**, live.
- **`sysmon`**: CPU per core as bar gauges (22 here, in three columns, red past
  90 %), RAM and swap, the network over the last minute as a strip chart
  (received in the live colour, sent in the accent, the scale round and printed),
  battery/power if there is one (percent, status, watts), and the four busiest
  processes (the biggest, when nothing is busy). All from `/proc` and `/sys`:
  `/proc/stat`, `/proc/meminfo`, `/proc/net/dev`, `/proc/PID/stat`,
  `/sys/class/power_supply`. It samples once a second **only while it is shown**:
  its subscription exists only then, so a hidden panel has no timer, no thread
  work and no state to keep current. It starts from a baseline taken when it is
  shown and reads again 250 ms later, so the first rates arrive at once.

### The focus grab

Hyprland implements `hyprland_focus_grab_v1` (it installs the generated header;
Quickshell's `HyprlandFocusGrab` is built on it). The XML is not installed, so
`crates/iced_layer/protocols/hyprland-focus-grab-v1.xml` is written out from
that header (order of requests and events, argument types: the wire format
depends on nothing else), with wayland-scanner generating the bindings; the
upstream XML is the reference. A surface declares a part in the grab
(`SurfaceSettings::grab`: `Popup`, `Member`, `None`) and the shell holds one
grab while a popup is up. What was found, against a real Hyprland:

- A click outside a grabbed surface sends `cleared`; the click itself goes to
  the popup (with coordinates outside it), not to what is under it. The shell
  gives the popups a `CloseRequested` event, and the host hides the panel.
- **The grab gives the keyboard to an arbitrary surface of the grab** if the
  popup does not already have it: with the bar in the grab as a member, the
  keyboard went to the bar. So only the popup is in the grab, and the keyboard
  then stays on it (typing into the demo panel's field works, Escape closes).
- The SYS button therefore toggles without being a member: clicking it while a
  panel is open is a click outside, which clears the grab and hides the panel;
  clicking it with none open opens one.
- A cleared grab is not made again while the same popup stays up, whatever the
  program does with `CloseRequested`.
- Where the protocol is absent (`Env::focus_grab` is false) a panel takes the
  keyboard exclusively instead (Escape or X to close; no outside click), as it
  does with `--keyboard-exclusive`. That branch is unit-tested; it was not run
  against a compositor that lacks the protocol, because none was at hand.

### The theme

`~/.local/state/omarchy/current/theme.name` names the theme and
`current/theme/colors.toml` has its colours. `omarchy theme set` stages a
directory, removes `theme`, moves the staged one in and writes the name, so the
host watches the `current` directory with inotify (one thread blocked in
`read`, nothing while nothing changes), waits for a burst of events to end
(120 ms), reads both again and redraws every surface. `ctl reload-theme` does
the same on demand.

- `quadrille-terminal`, `-paper`, `-phosphor`, `-amber`, `-lcd` are quadrille's
  own themes, used as they are.
- Any other theme gets a palette built from its `colors.toml` (the roles
  Omarchy themes all have; `tools/gen_themes.py` writes the same ones from
  quadrille's palettes, and the mapping is its reverse):

  | palette role | from `colors.toml` |
  |---|---|
  | `void` | `background` |
  | `ground` | `lighter_background` (5 % from `background` towards the ink if missing or the same) |
  | `raised`, `hover` | the ground, 8 % and 16 % of the way to the ink |
  | `edge` | `selection`, or 22 % to the ink if that is too close to the ground to see |
  | `ink` | `foreground` |
  | `muted`, `faint` | the ground, 70 % and 35 % of the way to the ink |
  | `line` | the ground, 45 % of the way to the ink |
  | `accent` | `accent` |
  | `on_accent` | `background` or `foreground`, whichever is further from the accent |
  | `highlight` | the ground, 35 % of the way to the accent |
  | `live`, `caution`, `alarm` | `green`, `yellow`, `red` |

  The steps are the proportions quadrille's own palettes use, so
  `quadrille-terminal`'s `colors.toml` run through the mapping comes back to
  `Palette::TERMINAL` exactly in the roles the file carries, and within a few
  levels in the rest (unit-tested). `muted` and `faint` are mixed rather than
  taken from the file because Omarchy's `muted` is often too dark to read on a
  panel (Tokyo Night's is 1.6:1 against its ground; quadrille-terminal's is 4.9:1).
- With a missing or unreadable file the host uses Terminal.

### How it was tested, and the rules it was tested under

Nothing in this work clicks or types on the real session. Input was tested in a
**nested Hyprland**, started by `tools/nested.sh`:

- Hyprland 0.56 has no headless-only mode (`CBackend::create()` fails without a
  DRM session), so a nested one is a *window of the real session* while it runs
  (about a minute for the whole test). Inside it, `hyprctl output create
  headless` makes outputs that are not windows: `QA` (2560 x 1600 at 1.666667,
  like the laptop) and `QB` (3440 x 1440 at 1, like the ultrawide), both placed
  by `hl.monitor` rules in a Lua config (the legacy `.conf` makes Hyprland draw a
  deprecation banner into every screenshot). `grim` and the virtual pointer and
  keyboard clients work on them.
- `tools/nested.sh run CMD` points a command at the nested compositor, and
  refuses if that is not up or is the real display. `vptr` (the virtual pointer
  tool) refuses to run unless `QUADRILLE_NESTED` names the display it was started
  on, which only `nested.sh run` sets. `nested.sh down` is a `kill`, never a
  `hyprctl` (with a lost signature `hyprctl` would talk to the real session).
- `tools/nested-test.sh` is the whole run: 52 checks, all passing on the final
  build, with screenshots and logs. It covers a bar per output with the right
  size and exclusive zone and crisp pixels on both scales (`tools/crisp.py`:
  few colours, every colour change on the pixel grid); summon and dismissal by a
  click outside, a click inside keeping the panel, SYS toggling; the control
  socket's commands and errors; the panel moving between outputs; the keyboard
  (typing into the demo field, Escape); live `omarchy theme set`-style changes
  (tokyo-night, quadrille-paper, quadrille-terminal, white: the bar's ground
  matches each theme's) through a copy of the state directory, so the real one
  is never touched; a live scale change on a running output (1, 2, 1.25 and
  back to 1.666667: the bar re-sizes to 50, 50, 60 and 45 logical, exact and
  crisp each time); an output removed (its panel moves) and created again.
- Not covered: touch, IME, clipboard (none in the shell); the exclusive-keyboard
  fallback against a compositor without the focus grab; more than one seat.
- Unit tests (`cargo test`, 31): the size rule, the theme mapping, the `/proc`
  parsers, the sampler, and the host's decisions (surfaces per output, panel
  placement, keyboard modes, commands and their errors).

### Measurements: hidden against shown

On the real session, rendering only (`--passive`: no keyboard, no grab, and no
input of mine), eDP-2 at 1.666667, release build, 25 s after a 7 s settle. The
bar's gauges are read every 2 s by default (`--bar-tick-ms`); the clock ticks
once a minute, on the minute.

| case | CPU | ctx switches | RSS (PSS) | threads |
|---|---|---|---|---|
| tiny-skia, sysmon hidden, gauges off | **0.000 %** | 0/s | 12.4 MB (9.2) | 5 |
| tiny-skia, sysmon hidden, gauges every 2 s | 0.060 % (0.6 ms/s) | 7/s | 13.5 MB | 5 |
| tiny-skia, sysmon shown, gauges off | 0.90 % (9.0 ms/s) | 16/s | 17.4 MB (12.1) | 5 |
| tiny-skia, sysmon shown, gauges every 2 s | 1.00 % (10 ms/s) | 19/s | 17.4 MB | 5 |
| wgpu, sysmon hidden | 0.010 % | 0.1/s | 120 MB (77) | 12 |
| wgpu, sysmon shown | 0.92 % | 15/s | 126 MB (83) | 13 |

Hidden, with nothing ticking, the host does not run: 0 CPU and 0 wakeups.
Shown, a sample and a redraw cost about 9 ms a second (0.9 % of a core), most of
it reading some 600 `/proc/PID/stat` files; the redraw itself is 0.7 ms (median
`prepare` 46 us, `draw` 20 us, `present` 0.9 ms at 480 x 915 physical pixels).
The processes could be read every other sample to halve that; it was left at a
fixed one-second cadence, as asked.

## Stage 1: the feasibility spike

### Verdict

**Feasible, and it already works.** A quadrille application runs as a
layer-shell bar plus a popup on Hyprland 0.56, crisp at the laptop's 1.6667
scale and at 1.0, with pointer, cursor shapes and keyboard input, summoned over
a socket. It needs **no change to the iced fork**.

What made it cheap: the fork's compositors (wgpu, and tiny-skia through
softbuffer) take raw Wayland handles, and the virtual-pixel viewport and the
nearest-neighbour upscale live in them. All that was missing was a windowing
shell in the place of `iced_winit`. `crates/iced_layer` is that shell: about
2.6k lines on smithay-client-toolkit (1.0k Wayland side, 1.2k iced side, the
rest keys, handles and a builder). Its core is `iced_winit::run_instance`
re-expressed for surfaces that the program declares.

| | idle, static UI | one redraw (wall, at 60/s) | RSS | threads |
|---|---|---|---|---|
| tiny-skia | 0.006 % CPU, 0 wakeups/s | 0.50 ms | 12.6 MB | 4 |
| wgpu (Intel iGPU) | 0.06 % CPU, 13 wakeups/s | 0.74 ms | 122 MB | 11 |
| Quickshell (the omarchy shell, for scale) | 0.03 % CPU | n/a | 345 MB | 35 |

The recommendation is tiny-skia for panels (see "Measurements").

### What I evaluated, and why I chose what I chose

#### 1. `iced_layershell` 0.19.1 (waycrate): rejected, by reading its source

- It builds on the **crates.io iced 0.14** crates (`iced_core = "0.14"`,
  `iced_runtime`, `iced_renderer`, `iced_graphics`, `iced_program`). The fork
  is iced master (0.15-dev), whose `Shell`, `Bus`, `core::Window`, clipboard
  requests and `UserInterface::update` all differ. A widget from the fork's
  `iced_widget` (which is what quadrille is made of) cannot be handed to
  0.14's `UserInterface`.
- It makes its viewport with `Viewport::with_physical_size(physical, factor)`
  (`multi_window/state.rs:242`): no virtual pixels and no pixel-scale mode, so
  the fork's reason to exist would have to be ported in.
- Porting means taking over `multi_window.rs` (1279 lines), its macros and
  `layershellev`, against the fork's API. That is the same work as writing the
  shell, on a base that moves with someone else's iced version.
- It does confirm the architecture: it also drives iced's own compositors from
  raw handles and uses `wp_viewporter` + `wp_fractional_scale`.

#### 2. `layershellev` 0.19.1 (its windowing core): viable fallback, not used

It is independent of iced (sctk 0.20, calloop 0.14, xkbcommon, fractional
scale, viewporter, cursor shape, popups; 3.2k lines) and exposes raw handles
and `set_destination`. I did not take it because the spike's main risks were
all in the sequence "scale known, size settled, buffer attached" and I wanted
to own that, and because its callback loop wants to own the event loop. If
maintaining sctk glue turns out to be a chore, this is what to try in place of
`wl.rs`.

#### 3. Own sctk shell on the fork's compositors: chosen

`wl.rs` is the Wayland side and knows nothing about iced; `shell.rs` is the
iced side. Both renderers work with the same code, so **GPU and CPU rendering
are one flag apart** (`--backend`).

#### 4. Bypass: tiny-skia into our own wl_shm buffers: not needed

`iced_tiny_skia`'s compositor already draws the virtual-pixel framebuffer on
the CPU and upscales it by the integer pixel scale into a softbuffer
(`wl_shm`) buffer, only the damaged region. Given a raw `wl_surface` it does
exactly what the bypass would. Writing it again would only add code.

### What the fork needs

Nothing. Things I ran into, none a blocker:

- `iced_widget` must be built with its `wgpu` feature when both renderers are
  on, or `iced_widget::Renderer` (what `quadrille::Element` defaults to) is
  tiny-skia alone and does not match `iced_renderer::Renderer`. This is why
  `Cargo.toml` here has `features = ["wgpu"]` on `iced_widget`.
- `Viewport::with_pixel_scale` rounds the target **up**, so at 1.6667 (three
  physical pixels to a virtual one) a 2560-px-wide output is 854 virtual
  pixels and the last one shows a single physical column. That is the fork's
  design and harmless for a bar whose background fills it; a floor variant
  with the remainder cleared would give whole pixels at both ends of an
  edge-to-edge layout. Optional.
- `core::window::Settings` cannot say anchor, layer, margin or keyboard mode.
  Rather than add a platform struct, `iced_layer` has the program **declare**
  its surfaces from its state (`fn surfaces(&State) -> Vec<(window::Id,
  SurfaceSettings)>`), and the shell opens, updates and closes them to match.
  A popup is `if state.popup { surfaces.push(..) }`: Elm-shaped, no window tasks.

### How it works (`crates/iced_layer`)

- `wl.rs`: sctk + calloop. Binds `zwlr_layer_shell_v1`, `wp_viewporter`,
  `wp_fractional_scale_manager_v1`, seats (keyboard with repeat; pointer with
  `wp_cursor_shape_v1` through sctk's `ThemedPointer`). Per surface it keeps
  the last configure, the scale and the geometry applied, and queues events.
- `shell.rs`: the loop. Dispatch, then: reconcile the declared surfaces; fold
  Wayland events into iced events; settle geometry; open a window (compositor
  surface + renderer) once a surface has a final size and a known scale; run
  runtime actions; update interfaces with input; run `update`; redraw what is
  dirty (feeding `RedrawRequested` to the interface as `iced_winit` does, so
  hover and focus states resolve), draw, present. It sleeps in `calloop` until
  a Wayland event, an action from the runtime or the next `RedrawRequest::At`,
  so a static UI costs nothing.
- `handle.rs`: `wl_surface` + `wl_display` pointers as `raw-window-handle`.
  `wayland-backend` is built with `client_system` so the pointers are
  libwayland's, which wgpu (Vulkan WSI) and softbuffer both need.
- `keys.rs`: xkb keysyms to iced keys. `app.rs`: an
  `application(boot, update, view, surfaces)` builder, where `surfaces` gets an
  `Env` (the outputs, and whether the focus grab exists) and says which
  surfaces there should be. `focus_grab.rs`: the generated bindings of
  `hyprland_focus_grab_v1` (stage 2).
- `crates/bar`: `quadrille-bar`, the host of stage 2 (in the spike, a bar and a
  popup with a socket to summon it). `crates/vptr`: a test tool that moves and
  clicks a `wlr-virtual-pointer`; it refuses to run except against a nested
  compositor (see stage 2).

### Fractional scale: what actually happens at 1.6667

Hyprland tells every surface `wp_fractional_scale_v1.preferred_scale(200)`
(200/120). The shell does what the protocol asks: buffer scale stays 1, the
buffer is `round(logical x scale)` physical pixels, and `wp_viewport`'s
destination is the logical size, so the compositor can show the buffer 1:1.
The scale is also guessed before the first commit from the output (mode width
over xdg-output logical width), so the surface asks for the right size at once
and is never shown at a wrong one.

With the fork's `PixelScaleMode::Auto(2)` that resolves to a **pixel scale of
3** (`round(2 x 1.6667)`): a virtual pixel is 3 physical pixels, which is 1.8
logical ones (2 at scale 1). The bar is 854 x 25 virtual pixels on the laptop
and 1720 x 25 on the ultrawide, and `quadrille::settings()` is used unchanged.

The thing to know: **the logical size must make `size x scale` a whole
number**, and logical sizes are integers. Measured with `grim` on the live
output (colour count and the position of every colour change along the bar's
rows and columns; 3 physical pixels to a virtual pixel):

| bar height asked | logical | physical it comes to | what Hyprland shows |
|---|---|---|---|
| 25 vpx | 45 | 75 exactly | **crisp**: 7 colours, every change on a multiple of 3, both axes |
| 26 vpx (first version of the size rule) | 47 | 78.33 | **filtered**: 175 colours, changes at residues 1 and 2 |
| 27 vpx (same) | 52 | 86.67 | crisp, by luck: the edges happened to round to the buffer's size |
| 30 vpx | 54 | 90 exactly | crisp |

A fractional size makes the compositor round the surface's two edges
separately and, depending on where the surface sits, sometimes show a 78-row
buffer over 79 rows, bilinearly. Position does not matter once `size x scale`
is integral, because both edges then round alike (the bar sat at 53.33 physical
pixels under Omarchy's own bar and was exact).

So `logical_for(virtual_px, pixel_scale, scale)` looks for the smallest logical
length that is exact and a whole number of virtual pixels, within two virtual
pixels of what was asked. `SurfaceSettings` lengths (size and margins) are in
virtual pixels. At 1.6667 and pixel scale 3 the exact sizes are the
**multiples of 5 virtual pixels** (5 vpx = 15 physical = 9 logical), which is a
good rule for a design that must be exact on the laptop; every size is exact
at scale 1 and 2. Other sizes come out at the next exact size up, which the
layout sees as the viewport size, so a `Length::Fill` bar is a little taller.
Unit tests cover the function; the popup (140 x 185 vpx = 252 x 333 logical =
420 x 555 physical) is crisp on both backends (9 colours, all changes aligned).

Not tested live, because it would mean changing the monitor config: a scale
change under a running surface. The code path is there (a new preferred scale
re-derives the geometry, asks for a new size, rebuilds the viewport and
`configure_surface`s) but it was not exercised.

### Input

- **Pointer**: enter/leave/motion/buttons/axis, converted from surface-local
  logical pixels to layout pixels with `x x scale / viewport.scale_factor()`.
  Hover, press and the SYS button work (driven with a virtual pointer); the
  cursor changes shape (the hand over buttons) through `wp_cursor_shape_v1`.
- **Keyboard**: sctk's xkb handling, repeat, modifiers. A `text_input` in the
  popup took typed text, showed its focus ring and caret, and Escape closed the
  popup.
- Missing in the shell (spike): clipboard (requests are dropped), IME
  (text-input-v3), touch, drag and drop, accessibility.

Keyboard focus on Hyprland is where behaviour is not what a bar author would
guess (found in the spike, on the real session):

- Hyprland gives a new `on_demand` layer surface keyboard focus when it maps,
  but with `follow_mouse` it **takes it away again as soon as the pointer is
  elsewhere**. A popup summoned over IPC while the mouse was over another
  window got its focus and lost it within a frame. So "close on unfocus", the
  obvious way to dismiss a popup, closes an IPC-summoned one at once; the host
  does not use it.
- Layer-shell has no "click outside": that is what the focus grab of stage 2
  is for. For a keyboard-driven panel `KeyboardInteractivity::Exclusive` is the
  other way.

### Measurements

Machine: Intel Core Ultra 9 185H (Arc iGPU) + RTX 4060; eDP-2 at 1.6667,
2560 x 1600; the bar is 1536 x 45 logical = 2560 x 75 physical.
`--no-exclusive` so as not to move the user's windows. CPU is the sum of
`/proc/PID/task/*/schedstat` over 25 s after a 7 s settle; RSS from
`/proc/PID/status`. Release build (`opt-level 3`, thin LTO). Redraw timings are
inside the shell (`ICED_LAYER_STATS=1`).

| case | CPU | ctx switches | RSS (PSS) | threads |
|---|---|---|---|---|
| tiny-skia, static (no ticks) | 0.006 % (0.06 ms/s) | 0/s | 12.6 MB (9.3) | 4 |
| tiny-skia, bar + popup open, static | 0.011 % | 0.3/s | 13.6 MB (10.0) | 4 |
| tiny-skia, 1 Hz clock | 0.094 % (0.94 ms/s) | 10/s | 13.4 MB | 4 |
| tiny-skia, 60 redraws/s | 2.98 % (29.8 ms/s = 0.50 ms per redraw) | 505/s | 13.2 MB | 4 |
| wgpu, static | 0.063 % (0.63 ms/s) | 13/s | 122 MB (84) | 11 |
| wgpu, bar + popup open, static | ~0 % | 0/s | 122 MB (83) | 11 |
| wgpu, 1 Hz clock | 0.149 % (1.49 ms/s) | 16/s | 123 MB | 11 |
| wgpu, 60 redraws/s | 4.42 % (44.2 ms/s = 0.74 ms per redraw) | 566/s | 122 MB | 11 |
| Quickshell, same machine, idle | 0.03 % (0.3 ms/s) | n/a | 345 MB (314) | 35 |

Per redraw, inside the shell, at 2560 x 75 (median, p95 in parentheses):

| | build + RedrawRequested | draw | present |
|---|---|---|---|
| tiny-skia | 41 us (88) | 3.6 us (7) | 134 us (461) |
| wgpu | 43 us (87) | 5.6 us (9) | 398 us (730) |

`present` on tiny-skia includes the integer upscale of the damaged region and
the shm copy. The wall cost per redraw above also covers the demo's `update`
(reading `/proc` for the gauges), a second interface build for input, and
Wayland traffic. First frame, from the shell's start: tiny-skia **32 ms**,
wgpu **150 to 260 ms** (instance, adapter, device, pipelines). At scale 1 on the
ultrawide (3440 x 50 physical) a redraw is as cheap: tiny-skia `present` median
0.7 ms.

Reading it:

- The UI is **event-driven**: the static runs drew one frame per surface and
  then slept (0 wakeups/s on tiny-skia).
- **tiny-skia is the better fit for a panel.** 10x less memory (12.6 vs
  122 MB; the GPU path maps Vulkan and Mesa's caches), a third of the threads,
  no idle wakeups, five times faster to the first frame, no GPU driver, and the
  output is identical (the same 7 and 9 colours on both). A pixel-art bar is
  about 190 thousand pixels; copying them is cheaper than asking a GPU.
- wgpu works (no `wp_viewport` clash with Mesa's WSI, nothing blocked) and
  picked the Intel iGPU by adapter order; set `power_preference = LowPower` so
  it can never pick the RTX. It makes sense only for a surface with real shader
  work (the fork's CRT effect, a large animated visualization).
- The Quickshell comparison is rough (different work; it was restarting during
  my tests) but the order of magnitude holds: an iced/quadrille panel is a few
  percent of its memory.

### Not done, and risks

Stage 1 listed a scale change under a live surface, output hotplug, a focus
grab and a surface per output as not done: stage 2 did them, and tested them in
a nested compositor. What is left:

- **wgpu on a hidden output**: Vulkan WSI on Wayland can block `present` until a
  frame callback that a switched-off output never sends, freezing the
  single-threaded loop. Not observed (default `AutoVsync`); `vsync: false` is the
  mitigation. Moot on tiny-skia, which the host uses by default.
- **Clipboard** (requests are dropped, so a text field cannot copy or paste),
  **IME**, touch, drag and drop, accessibility: not in the shell.
- **The real Omarchy bar**: while `omarchy-bar` holds the top 32 px quadrille's
  bar sits under it and exclusive zones add up. Replacing it means hiding that
  surface (the shell's own configuration), which this work does not touch.
- **A compositor that closes a surface** (an output that disappears) drops it
  and the host asks for it again half a second later if its output is still
  listed; with the output gone the host stops declaring it, which is what the
  hotplug test exercises.
- **No other compositor was tried.** The focus grab is Hyprland's; sway and the
  like would take the exclusive-keyboard fallback, which is unit-tested only.

### Recommended next step

Pick the next real panel from the Omarchy shell's list (the audio mixer, the
calendar, the notification centre) and port it the way `sysmon` was done:
a module with a sampler (or a subscription) that exists only while the panel is
shown, a view built from quadrille's widgets, and an entry in `PANELS`. Wire
`quadrille-bar ctl toggle <id>` to the existing menu actions. Before replacing
Omarchy's bar, decide how the stock one is turned off.

### Running it

```sh
cd layershell
cargo build --release
./target/release/quadrille-bar                        # a bar on every output
./target/release/quadrille-bar ctl toggle sysmon      # summon, hide, list, ...
tools/nested-test.sh                                   # the whole input test (about a minute)
cargo test
```

Options of `quadrille-bar`: `--output NAME` (repeatable; the outputs that get a
bar), `--backend tiny-skia|wgpu`, `--bar-tick-ms MS` (0: the bar's gauges are
never read), `--no-exclusive`, `--height VPX`, `--keyboard-exclusive`,
`--passive` (panels take no keyboard and no grab: for screenshots on a desktop
in use), `--theme-dir DIR`, `--open PANEL`, `--exit-after SECS`.
`RUST_LOG=iced_layer=debug` logs geometry, focus and grabs;
`ICED_LAYER_STATS=1` prints timings on exit.

`tools/crisp.py IMAGE X0,Y0,X1,Y1 PIXEL_SCALE` is the crispness check of the
tests: over a region of a `grim` screenshot, the number of distinct colours (7 to
10 for a bar or a panel) and whether every colour change falls on a multiple of
the pixel scale from the surface's own origin.
