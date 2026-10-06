//! Run an iced program on wlr-layer-shell surfaces: bars, panels, popups.
//!
//! This is a windowing shell for the pixel-scale iced fork, in the place of
//! `iced_winit`. It speaks Wayland through smithay-client-toolkit, and gives
//! iced's own compositors (wgpu and tiny-skia) the raw handles of the
//! surfaces, so the fork's virtual pixels and nearest-neighbour upscale work
//! as they do in a window.
mod app;
mod focus_grab;
mod handle;
mod keys;
mod shell;
mod wl;

pub use app::{Application, application};
pub use shell::{Error, Layered, run};
pub use wl::{
    Anchor, Env, Exclusive, Geometry, Grab, KeyboardInteractivity, Layer, OutputInfo, RasterBuffer,
    SurfaceSettings, geometry, is_exact, logical_for, logical_for_native,
};
