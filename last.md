A exists: centre-zero metric wallpaper, millimetre ticks, centimetre rulers,
spec plates and a physical-scale output drawing; event-driven redraws,
estimated-size labels and measured overrides.

B exists: default-on, click-through Rust reticle and millimetre readout,
small native buffers, theme following, adaptive polling and fullscreen/lock
suppression.

Final six-second measurements: idle **0.125%** of one core; moving **2.564%**;
both **14.17 MiB RSS**. Off: 0.002%, 14.04 MiB.

Per 10 mm, laptop ideal **74.283316 physical px**, rendered **72/75**;
Dell ideal **43.055809**, rendered **42/44**. Maximum absolute mark errors:
**1.466944 / 0.995437 px**, within half a vpx. Interval errors can reach one vpx.

Verified shared Rust/JS vectors, offscreen wallpaper cases, GPU grid/first-frame
checks, all existing nested panel sections, overlay placement/edge/scale/input
checks, fullscreen/lock suppression and zero inhibited cursor queries.
Workspace: **120 tests passed, one ignored**.

Strict overlay hotplug crispness still fails: 840 presented pixels differ by
one colour level despite exact native buffers, consistent with compositor
filtering. No live preview was performed because that verification gate failed.
Hyprland also floors fractional cursor IPC coordinates; actual pointer-pixel
precision cannot be guaranteed. Calculated laptop vpx is 0.404 mm; Dell snaps
to 34.1 by the requested nearest-size rule. A namespace-only runtime rule
prevents reticle animation. Nothing installed, restarted or pushed.

```sh
quadrille-bar ctl overlay on
quadrille-bar ctl overlay off
quadrille-bar ctl overlay status
mkdir -p ~/.config/quadrille
$EDITOR ~/.config/quadrille/displays.toml
```

Example override; then run overlay off/on to reload:

```toml
["HDMI-A-1"]
width_mm = 797.8
height_mm = 333.9
```
