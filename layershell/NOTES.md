# quadrille on wlr-layer-shell: feasibility spike

Status: in progress. This file starts as the plan and ends as the verdict.

## Question

Can an iced application built on quadrille (whole-pixel layout, pixel fonts,
nearest-neighbour upscale, the `0.15-pixel-scale` iced fork) run as a layer-shell
surface (bar and popup panel) on Hyprland 0.56 at a fractional scale of 1.6667,
and stay crisp, cheap and interactive?

## Plan

1. Evaluate the options by reading their source, not guessing:
   - `iced_layershell` 0.19.1 (waycrate), and its windowing core `layershellev`;
   - own shell: smithay-client-toolkit (sctk) driving `iced_runtime` /
     `iced_graphics` compositors directly;
   - bypass iced's compositors and blit tiny-skia output into wl_shm ourselves.
2. Smallest thing that could work: sctk + the fork's own `Compositor`
   (softbuffer/tiny-skia and wgpu both take raw wayland handles) showing a
   quadrille bar.
3. Then widen: popup surface, keyboard, pointer, cursor shapes, fractional
   scale, damage/redraw-on-events-only, measurements.
