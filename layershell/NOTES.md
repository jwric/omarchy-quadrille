# quadrille on wlr-layer-shell: feasibility spike

## Verdict

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

## What I evaluated, and why I chose what I chose

### 1. `iced_layershell` 0.19.1 (waycrate): rejected, by reading its source

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

### 2. `layershellev` 0.19.1 (its windowing core): viable fallback, not used

It is independent of iced (sctk 0.20, calloop 0.14, xkbcommon, fractional
scale, viewporter, cursor shape, popups; 3.2k lines) and exposes raw handles
and `set_destination`. I did not take it because the spike's main risks were
all in the sequence "scale known, size settled, buffer attached" and I wanted
to own that, and because its callback loop wants to own the event loop. If
maintaining sctk glue turns out to be a chore, this is what to try in place of
`wl.rs`.

### 3. Own sctk shell on the fork's compositors: chosen

`wl.rs` is the Wayland side and knows nothing about iced; `shell.rs` is the
iced side. Both renderers work with the same code, so **GPU and CPU rendering
are one flag apart** (`--backend`).

### 4. Bypass: tiny-skia into our own wl_shm buffers: not needed

`iced_tiny_skia`'s compositor already draws the virtual-pixel framebuffer on
the CPU and upscales it by the integer pixel scale into a softbuffer
(`wl_shm`) buffer, only the damaged region. Given a raw `wl_surface` it does
exactly what the bypass would. Writing it again would only add code.

## What the fork needs

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

## How it works (`crates/iced_layer`)

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
  `application(boot, update, view, surfaces)` builder.
- `crates/bar`: the demo (`quadrille-bar`): a bar with brand block, workspace
  tabs, clock, CPU/MEM/BAT gauges, NET/MIC lamps and a SYS button that opens
  the popup (gauges, a theme selector, a text field), plus a unix socket
  (`ctl`) to summon it. `crates/vptr`: a test tool that moves and clicks a
  `wlr-virtual-pointer`, since nothing else here could drive a pointer on a
  live compositor.

## Fractional scale: what actually happens at 1.6667

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

## Input

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
guess:

- Hyprland gives a new `on_demand` layer surface keyboard focus when it maps,
  but with `follow_mouse` it **takes it away again as soon as the pointer is
  elsewhere**. A popup summoned over IPC while the mouse was over another
  window got its focus and lost it within a frame. So "close on unfocus", the
  obvious way to dismiss a popup, closes an IPC-summoned one at once. It is
  `--close-on-unfocus` here, off by default.
- Layer-shell has no "click outside". For a keyboard-driven panel use
  `KeyboardInteractivity::Exclusive` (`--keyboard exclusive`, which I used to
  type into the popup safely). For a mouse-driven popup that should go away when
  clicked off, Hyprland implements `hyprland_focus_grab_v1` (it ships the
  generated header; Quickshell's `HyprlandFocusGrab` is built on it). That is
  the right thing to add next, and a small protocol.

## Measurements

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

## Not done, and risks

- **Scale change under a live surface**: coded, not exercised.
- **Output hotplug / `closed`**: the shell drops a surface the compositor
  closes and recreates it half a second later while the program still declares
  it; not exercised. A real bar wants one surface per output, declared from the
  output list (`Wl::output_names` is there for that).
- **wgpu on a hidden output**: Vulkan WSI on Wayland can block `present` until a
  frame callback that a switched-off output never sends, freezing the
  single-threaded loop. Not observed (default `AutoVsync`); `vsync: false` is the
  mitigation. Moot on tiny-skia.
- **Focus grab**, **clipboard**, **IME**, as above.
- Layer ordering is the compositor's: while Omarchy's own `omarchy-bar` holds
  the top 32 px this bar sits under it and exclusive zones add up (checked: the
  reserved area grew by the bar's 50 px on the ultrawide and went back on exit).
  A real replacement would hide the stock bar's surface first.

## Recommended next step

Turn this into a panel host: keep `iced_layer` and tiny-skia; add
`hyprland_focus_grab_v1` (outside-click dismissal, and a solid IPC-summoned
flow), one surface per output, and a `summon <id> <json>` command on the
existing socket; then put one real panel (the clock/calendar or the audio
mixer) behind `omarchy-shell shell summon`. Keep wgpu as a flag.

## Running it

```sh
cd layershell
cargo build --release
./target/release/quadrille-bar --output eDP-2 --backend tiny-skia --no-exclusive
./target/release/quadrille-bar ctl toggle-popup   # also open-popup, close-popup, quit
```

Options: `--output NAME`, `--backend wgpu|tiny-skia`, `--tick-ms MS` (0 =
static), `--popup`, `--no-exclusive`, `--height VPX`, `--keyboard
exclusive|on-demand`, `--close-on-unfocus`, `--exit-after SECS` (checked on
ticks). `RUST_LOG=iced_layer=debug` logs geometry and focus;
`ICED_LAYER_STATS=1` prints timings on exit. Escape closes the popup.
`crates/vptr` (`vptr eDP-2 move X Y click`) drives a virtual pointer for
testing; it moves the real cursor, so mind what is under it.

Reproducing the crispness check: `grim -o eDP-2 out.png`, then over the bar's
rows count the distinct colours (7 for a crisp bar) and check that every colour
change falls on a multiple of the pixel scale from the surface's origin.
