//! The Wayland side of the shell: sctk state, protocol handlers and the
//! surfaces they manage. It knows nothing about iced; the shell reads the
//! events it queues and the geometry it settles.
use iced_core::window;

use smithay_client_toolkit::compositor::{CompositorHandler, CompositorState};
use smithay_client_toolkit::globals::GlobalData;
use smithay_client_toolkit::output::{OutputHandler, OutputState};
use smithay_client_toolkit::reexports::calloop::LoopHandle;
use smithay_client_toolkit::reexports::client::globals::GlobalList;
use smithay_client_toolkit::reexports::client::protocol::{
    wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface,
};
use smithay_client_toolkit::reexports::client::{
    Connection, Dispatch, Proxy, QueueHandle, backend::ObjectId,
};
use smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use smithay_client_toolkit::reexports::protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::{
    self, WpFractionalScaleV1,
};
use smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewport::WpViewport;
use smithay_client_toolkit::reexports::protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use smithay_client_toolkit::registry::{ProvidesRegistryState, RegistryState};
use smithay_client_toolkit::seat::keyboard::{
    KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers,
};
use smithay_client_toolkit::seat::pointer::{
    AxisScroll, CursorIcon, PointerEvent, PointerEventKind, PointerHandler, ThemeSpec,
    ThemedPointer,
};
use smithay_client_toolkit::seat::{Capability, SeatHandler, SeatState};
use smithay_client_toolkit::shell::WaylandSurface;
use smithay_client_toolkit::shell::wlr_layer::{
    LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure,
};
use smithay_client_toolkit::shm::{Shm, ShmHandler};
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::{
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm, registry_handlers,
};

use crate::focus_grab::protocol::hyprland_focus_grab_manager_v1::HyprlandFocusGrabManagerV1;
use crate::focus_grab::protocol::hyprland_focus_grab_v1::{self, HyprlandFocusGrabV1};

pub use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How long a surface waits for the compositor to tell it its scale before it
/// goes with what it can guess.
pub const SCALE_GRACE: Duration = Duration::from_millis(120);

/// How much of the screen edge a surface claims for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exclusive {
    /// Nothing: other surfaces may be placed under it, or move out of its way
    /// (`exclusive_zone` 0).
    None,
    /// Its own thickness along the edge it is anchored to, like a bar.
    Own,
    /// Neither claims space nor is moved by anyone else's (`exclusive_zone`
    /// -1), like a popup.
    Ignore,
}

/// A surface's part in a focus grab.
///
/// While a [`Popup`](Grab::Popup) is shown, the compositor limits input to the
/// grab's surfaces (the popups and the [`Member`](Grab::Member)s, a bar whose
/// button opens the popup, say) and a click anywhere else clears the grab: the
/// popups then get a `CloseRequested` event. Where the compositor has no focus
/// grab protocol nothing happens and the program should ask for an exclusive
/// keyboard instead, which [`Env::focus_grab`] tells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Grab {
    #[default]
    None,
    /// Stays usable while a popup holds the grab.
    Member,
    /// Held by the grab, and closed when it is cleared.
    Popup,
}

/// An output, as far as a program needs to know it to put surfaces on it.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputInfo {
    pub name: String,
    /// The scale it is shown at, which the compositor may refine per surface.
    pub scale: f64,
    /// Its size in logical pixels.
    pub logical_size: (i32, i32),
}

/// What the shell can tell a program about the compositor it runs on.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Env {
    /// The outputs there are, by name.
    pub outputs: Vec<OutputInfo>,
    /// Whether a click outside a popup can be heard (`hyprland_focus_grab_v1`).
    pub focus_grab: bool,
    /// The output each surface is on, by name, once the compositor has put it
    /// somewhere: which is how a program that left the choice to the
    /// compositor (an output of `None`) finds out where it ended up.
    pub placements: HashMap<window::Id, String>,
}

/// What a layer surface looks like to the compositor. Lengths are in virtual
/// pixels: the shell turns them into the logical pixels the protocol speaks,
/// at the scale of the output the surface is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceSettings {
    pub namespace: String,
    pub layer: Layer,
    pub anchor: Anchor,
    /// Width and height. Zero stretches the axis, which needs both of its
    /// edges anchored.
    pub size: (u32, u32),
    pub exclusive: Exclusive,
    /// Top, right, bottom, left.
    pub margin: [i32; 4],
    pub keyboard: KeyboardInteractivity,
    /// The output to appear on, by name (`eDP-2`); the compositor's choice
    /// when `None`.
    pub output: Option<String>,
    pub grab: Grab,
    /// Logical protocol margins, when positioning a physical-pixel raster.
    pub logical_margin: Option<[i32; 4]>,
    /// An empty Wayland input region: neither clicks nor pointer focus.
    pub input_passthrough: bool,
    /// Native premultiplied ARGB8888 pixels, with no renderer or filtering.
    pub raster: Option<RasterBuffer>,
    /// Programs that track content changes can spare unrelated windows a
    /// redraw when a small independent surface changes.
    pub repaint_revision: Option<u64>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct RasterBuffer {
    pub size: (u32, u32),
    pub pixels: Arc<[u8]>,
}

impl std::fmt::Debug for RasterBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RasterBuffer")
            .field("size", &self.size)
            .field("bytes", &self.pixels.len())
            .finish()
    }
}

impl Default for SurfaceSettings {
    fn default() -> Self {
        Self {
            namespace: String::from("iced"),
            layer: Layer::Top,
            anchor: Anchor::empty(),
            size: (0, 0),
            exclusive: Exclusive::None,
            margin: [0; 4],
            keyboard: KeyboardInteractivity::None,
            output: None,
            grab: Grab::None,
            logical_margin: None,
            input_passthrough: false,
            raster: None,
            repaint_revision: None,
        }
    }
}

/// What the protocol is told, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geometry {
    pub size: (u32, u32),
    pub margin: [i32; 4],
    pub exclusive: i32,
}

/// A logical length for a surface that should show `virtual_px` virtual pixels
/// of `pixel_scale` physical pixels each, at `scale`.
///
/// A logical length is an integer, and at a fractional scale most of them are
/// not a whole number of physical pixels: 47 is 78.33 at 1.6667. The
/// compositor then rounds the surface's edges and, depending on where it ends
/// up, shows a buffer of 78 rows over 79, and filters it. So this looks, from
/// the size asked for upwards, for a length that is exactly a whole number of
/// physical pixels, preferably a whole number of virtual pixels as well. At
/// 1.6667 with three physical pixels to a virtual one those are the multiples
/// of 9 logical pixels, which is to say of 5 virtual pixels.
///
/// It never goes further than two virtual pixels past what was asked for; past
/// that it takes what comes closest (a length that is whole in physical
/// pixels, or just the next integer).
pub fn logical_for(virtual_px: u32, pixel_scale: u32, scale: f64) -> u32 {
    if virtual_px == 0 {
        return 0;
    }

    let pixel_scale = pixel_scale.max(1);
    let wanted = f64::from(virtual_px * pixel_scale);
    let first = (wanted / scale).ceil() as u32;
    let reach = (2.0 * f64::from(pixel_scale) / scale).ceil() as u32 + 1;

    let physical = |logical: u32| (f64::from(logical) * scale).round() as u32;
    let candidates = || first..first + reach;

    candidates()
        .find(|logical| is_exact(*logical, scale) && physical(*logical) % pixel_scale == 0)
        .or_else(|| candidates().find(|logical| is_exact(*logical, scale)))
        .or_else(|| candidates().find(|logical| physical(*logical) % pixel_scale == 0))
        .unwrap_or(first)
}

/// Native buffers need an integral physical extent even at uncommon scales.
/// Transparent padding is cheaper than filtering a measuring instrument.
pub fn logical_for_native(virtual_px: u32, pixel_scale: u32, scale: f64) -> u32 {
    let first = logical_for(virtual_px, pixel_scale, scale);
    (first..=first + 120)
        .find(|logical| is_exact(*logical, scale))
        .unwrap_or(first)
}

/// Whether a logical length maps to a whole number of physical pixels at
/// `scale`, which is when the compositor can show the buffer unfiltered.
pub fn is_exact(logical: u32, scale: f64) -> bool {
    // Scales are conveyed in 120ths: compare in them.
    let physical = f64::from(logical) * (scale * 120.0).round() / 120.0;

    (physical - physical.round()).abs() < 1e-6
}

pub fn geometry(settings: &SurfaceSettings, pixel_scale: u32, scale: f64) -> Geometry {
    let to_logical = |virtual_px: u32| logical_for(virtual_px, pixel_scale, scale);

    let size = if settings.raster.is_some() {
        (
            logical_for_native(settings.size.0, pixel_scale, scale),
            logical_for_native(settings.size.1, pixel_scale, scale),
        )
    } else {
        (to_logical(settings.size.0), to_logical(settings.size.1))
    };

    let margin = settings.logical_margin.unwrap_or_else(|| {
        settings.margin.map(|margin| {
            let logical = to_logical(margin.unsigned_abs()) as i32;

            if margin < 0 { -logical } else { logical }
        })
    });

    let exclusive = match settings.exclusive {
        Exclusive::None => 0,
        Exclusive::Ignore => -1,
        Exclusive::Own => {
            let anchor = settings.anchor;

            if anchor.contains(Anchor::TOP) != anchor.contains(Anchor::BOTTOM) {
                size.1 as i32
                    + if anchor.contains(Anchor::TOP) {
                        margin[0]
                    } else {
                        margin[2]
                    }
            } else if anchor.contains(Anchor::LEFT) != anchor.contains(Anchor::RIGHT) {
                size.0 as i32
                    + if anchor.contains(Anchor::LEFT) {
                        margin[3]
                    } else {
                        margin[1]
                    }
            } else {
                0
            }
        }
    };

    Geometry {
        size,
        margin,
        exclusive,
    }
}

/// A layer surface and what the compositor has told us about it.
pub struct Surface {
    pub layer: LayerSurface,
    viewport: Option<WpViewport>,
    fractional: Option<WpFractionalScaleV1>,
    pub settings: SurfaceSettings,
    /// The scale the surface is shown at: the compositor's preference when it
    /// has told us, an output's otherwise.
    pub scale: f64,
    pub scale_known: bool,
    pub created: Instant,
    /// The last size the compositor configured, in logical pixels.
    pub configured: Option<(u32, u32)>,
    /// A new size was asked for and not configured yet.
    pub awaiting: Option<Instant>,
    pub applied: Option<Geometry>,
    destination: Option<(u32, u32)>,
    buffer_scale: i32,
    /// The output it is on: the one asked for, then the one the compositor
    /// says it entered.
    pub output_name: Option<String>,
    /// Bumped whenever the shell has to look at the surface again.
    pub version: u64,
    raster_pool: Option<SlotPool>,
    raster_presented: Option<RasterBuffer>,
    raster_geometry_dirty: bool,
}

impl Surface {
    pub fn wl_surface(&self) -> &wl_surface::WlSurface {
        self.layer.wl_surface()
    }

    /// Whether the surface can be drawn at a size that is final.
    pub fn is_ready(&self) -> bool {
        self.configured.is_some()
            && self.awaiting.is_none()
            && (self.scale_known || self.created.elapsed() >= SCALE_GRACE)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        if let Some(fractional) = self.fractional.take() {
            fractional.destroy();
        }

        if let Some(viewport) = self.viewport.take() {
            viewport.destroy();
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    /// The size or the scale of the surface settled on something new.
    Changed,
    Closed,
    PointerEntered,
    PointerLeft,
    /// In logical pixels.
    PointerMoved((f64, f64)),
    PointerButton {
        button: u32,
        pressed: bool,
    },
    PointerScroll {
        x: AxisScroll,
        y: AxisScroll,
    },
    KeyPressed {
        event: KeyEvent,
        repeat: bool,
    },
    KeyReleased(KeyEvent),
    Modifiers(Modifiers),
    Focused(bool),
}

pub struct FractionalData(window::Id);

struct ActiveGrab {
    grab: HyprlandFocusGrabV1,
    members: Vec<window::Id>,
}

impl Drop for ActiveGrab {
    fn drop(&mut self) {
        self.grab.destroy();
    }
}

pub struct Wl {
    pub registry_state: RegistryState,
    pub seat_state: SeatState,
    pub output_state: OutputState,
    pub compositor_state: CompositorState,
    pub layer_shell: LayerShell,
    pub shm: Shm,
    viewporter: Option<WpViewporter>,
    fractional_manager: Option<WpFractionalScaleManagerV1>,
    grab_manager: Option<HyprlandFocusGrabManagerV1>,
    grab: Option<ActiveGrab>,
    /// The members of the grab that was cleared last, so that the same popup
    /// is not grabbed again until the program has had its say.
    grab_cleared_for: Option<Vec<window::Id>>,
    grab_cleared: bool,
    loop_handle: LoopHandle<'static, Wl>,

    pub surfaces: HashMap<window::Id, Surface>,
    by_wl: HashMap<ObjectId, window::Id>,
    pub events: Vec<(window::Id, Event)>,

    pointer: Option<ThemedPointer>,
    pointer_focus: Option<window::Id>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_focus: Option<window::Id>,
    cursor: Option<CursorIcon>,
    pub conn: Connection,
}

impl Wl {
    pub fn new(
        conn: &Connection,
        globals: &GlobalList,
        qh: &QueueHandle<Self>,
        loop_handle: LoopHandle<'static, Wl>,
    ) -> Result<Self, String> {
        let compositor_state = CompositorState::bind(globals, qh)
            .map_err(|error| format!("wl_compositor is not available: {error}"))?;

        let layer_shell = LayerShell::bind(globals, qh)
            .map_err(|error| format!("zwlr_layer_shell_v1 is not available: {error}"))?;

        let shm =
            Shm::bind(globals, qh).map_err(|error| format!("wl_shm is not available: {error}"))?;

        let viewporter = globals.bind(qh, 1..=1, GlobalData).ok();
        let fractional_manager = globals.bind(qh, 1..=1, GlobalData).ok();
        let grab_manager = globals.bind(qh, 1..=1, GlobalData).ok();

        log::info!(
            "wp_viewporter: {}, wp_fractional_scale_manager_v1: {}, hyprland_focus_grab_manager_v1: {}",
            viewporter.is_some(),
            fractional_manager.is_some(),
            grab_manager.is_some()
        );

        Ok(Self {
            registry_state: RegistryState::new(globals),
            seat_state: SeatState::new(globals, qh),
            output_state: OutputState::new(globals, qh),
            compositor_state,
            layer_shell,
            shm,
            viewporter,
            fractional_manager,
            grab_manager,
            grab: None,
            grab_cleared_for: None,
            grab_cleared: false,
            loop_handle,
            surfaces: HashMap::new(),
            by_wl: HashMap::new(),
            events: Vec::new(),
            pointer: None,
            pointer_focus: None,
            keyboard: None,
            keyboard_focus: None,
            cursor: None,
            conn: conn.clone(),
        })
    }

    pub fn display_ptr(&self) -> *mut std::ffi::c_void {
        self.conn.backend().display_ptr().cast()
    }

    /// The scale an output is shown at, worked out from its logical size and
    /// its current mode (`wl_output` itself only knows integers).
    fn output_scale(&self, output: &wl_output::WlOutput) -> Option<f64> {
        let info = self.output_state.info(output)?;
        let mode = info.modes.iter().find(|mode| mode.current)?;
        let (logical_width, logical_height) = info.logical_size?;

        let rotated = matches!(
            info.transform,
            wl_output::Transform::_90
                | wl_output::Transform::_270
                | wl_output::Transform::Flipped90
                | wl_output::Transform::Flipped270
        );

        let physical_width = if rotated {
            mode.dimensions.1
        } else {
            mode.dimensions.0
        };
        let _ = logical_height;

        (logical_width > 0).then(|| f64::from(physical_width) / f64::from(logical_width))
    }

    fn find_output(&self, name: &str) -> Option<wl_output::WlOutput> {
        self.output_state.outputs().find(|output| {
            self.output_state
                .info(output)
                .and_then(|info| info.name)
                .is_some_and(|candidate| candidate == name)
        })
    }

    pub fn output_names(&self) -> Vec<String> {
        self.outputs()
            .into_iter()
            .map(|output| output.name)
            .collect()
    }

    /// The outputs there are, by name.
    pub fn outputs(&self) -> Vec<OutputInfo> {
        let mut outputs: Vec<_> = self
            .output_state
            .outputs()
            .filter_map(|output| {
                let info = self.output_state.info(&output)?;

                Some(OutputInfo {
                    name: info.name?,
                    scale: self.output_scale(&output).unwrap_or(1.0),
                    logical_size: info.logical_size.unwrap_or((0, 0)),
                })
            })
            .collect();

        outputs.sort_by(|a, b| a.name.cmp(&b.name));
        outputs
    }

    /// The output each surface is on, where that is known.
    pub fn placements(&self) -> HashMap<window::Id, String> {
        self.surfaces
            .iter()
            .filter_map(|(id, surface)| Some((*id, surface.output_name.clone()?)))
            .collect()
    }

    pub fn has_focus_grab(&self) -> bool {
        self.grab_manager.is_some()
    }

    /// Makes the focus grab the one the program asks for: one while a popup is
    /// up, holding `members`, and none otherwise. A grab is made again when its
    /// surfaces change, and a cleared one is gone until the program shows a
    /// popup again.
    pub fn sync_grab(&mut self, qh: &QueueHandle<Self>, members: &[window::Id], popup: bool) {
        let Some(manager) = &self.grab_manager else {
            return;
        };

        let wanted = popup && !members.is_empty();

        if !wanted || self.grab_cleared_for.as_deref() != Some(members) {
            self.grab_cleared_for = None;
        }

        // A grab that was cleared stays cleared while the same surfaces are up.
        if wanted && self.grab.is_none() && self.grab_cleared_for.is_some() {
            return;
        }

        match &self.grab {
            Some(active) if wanted && active.members == members => return,
            None if !wanted => return,
            _ => {}
        }

        // Dropping the old grab destroys it.
        self.grab = None;

        if !wanted {
            return;
        }

        let grab = manager.create_grab(qh, GlobalData);

        for id in members {
            if let Some(surface) = self.surfaces.get(id) {
                grab.add_surface(surface.wl_surface());
            }
        }

        grab.commit();

        log::debug!("focus grab on {members:?}");

        self.grab = Some(ActiveGrab {
            grab,
            members: members.to_vec(),
        });
    }

    /// Whether the compositor cleared the focus grab since the last call.
    pub fn take_grab_cleared(&mut self) -> bool {
        std::mem::take(&mut self.grab_cleared)
    }

    pub fn create_surface(
        &mut self,
        qh: &QueueHandle<Self>,
        id: window::Id,
        settings: SurfaceSettings,
        pixel_mode: iced_core::PixelScaleMode,
    ) {
        let wl_surface = self.compositor_state.create_surface(qh);

        let output = settings
            .output
            .as_deref()
            .and_then(|name| self.find_output(name));

        if settings.output.is_some() && output.is_none() {
            log::warn!(
                "Output {:?} not found among {:?}; letting the compositor choose",
                settings.output,
                self.output_names()
            );
        }

        let output_name = output
            .as_ref()
            .and_then(|output| self.output_state.info(output)?.name);

        let hint = output.as_ref().and_then(|output| self.output_scale(output));
        let scale = hint.unwrap_or(1.0);

        let layer = self.layer_shell.create_layer_surface(
            qh,
            wl_surface,
            settings.layer,
            Some(settings.namespace.clone()),
            output.as_ref(),
        );

        layer.set_anchor(settings.anchor);
        layer.set_keyboard_interactivity(settings.keyboard);
        if settings.input_passthrough {
            let region = smithay_client_toolkit::compositor::Region::new(&self.compositor_state)
                .expect("An empty input region");
            layer
                .wl_surface()
                .set_input_region(Some(region.wl_region()));
        }

        // The first commit has no buffer, so the surface is not mapped and
        // whatever size it asks for here is corrected before anyone sees it.
        let geometry = geometry(&settings, pixel_mode.resolve(scale as f32), scale);

        layer.set_size(geometry.size.0, geometry.size.1);
        layer.set_margin(
            geometry.margin[0],
            geometry.margin[1],
            geometry.margin[2],
            geometry.margin[3],
        );
        layer.set_exclusive_zone(geometry.exclusive);

        let fractional = self.fractional_manager.as_ref().map(|manager| {
            manager.get_fractional_scale(layer.wl_surface(), qh, FractionalData(id))
        });

        let viewport = self
            .viewporter
            .as_ref()
            .map(|viewporter| viewporter.get_viewport(layer.wl_surface(), qh, GlobalData));

        layer.commit();

        let _ = self.by_wl.insert(layer.wl_surface().id(), id);

        let _ = self.surfaces.insert(
            id,
            Surface {
                layer,
                viewport,
                fractional,
                settings,
                scale,
                scale_known: false,
                created: Instant::now(),
                configured: None,
                awaiting: Some(Instant::now()),
                applied: Some(geometry),
                destination: None,
                buffer_scale: 1,
                output_name: output_name.clone(),
                version: 0,
                raster_pool: None,
                raster_presented: None,
                raster_geometry_dirty: true,
            },
        );
    }

    pub fn destroy_surface(&mut self, id: window::Id) {
        if let Some(surface) = self.surfaces.remove(&id) {
            let _ = self.by_wl.remove(&surface.wl_surface().id());
        }

        if self.pointer_focus == Some(id) {
            self.pointer_focus = None;
        }

        if self.keyboard_focus == Some(id) {
            self.keyboard_focus = None;
        }
    }

    /// Applies new settings to a live surface.
    pub fn update_surface(&mut self, id: window::Id, settings: &SurfaceSettings) {
        let Some(surface) = self.surfaces.get_mut(&id) else {
            return;
        };

        if surface.settings.anchor != settings.anchor {
            surface.layer.set_anchor(settings.anchor);
        }

        if surface.settings.keyboard != settings.keyboard {
            surface.layer.set_keyboard_interactivity(settings.keyboard);
        }

        if surface.settings.layer != settings.layer {
            surface.layer.set_layer(settings.layer);
        }

        if surface.settings.input_passthrough != settings.input_passthrough {
            if settings.input_passthrough {
                let region =
                    smithay_client_toolkit::compositor::Region::new(&self.compositor_state)
                        .expect("An empty input region");
                surface
                    .wl_surface()
                    .set_input_region(Some(region.wl_region()));
            } else {
                surface.wl_surface().set_input_region(None);
            }
        }

        if surface.settings != *settings {
            surface.settings = settings.clone();
            // Cursor motion changes only the margin and pixels. Keep the old
            // size so a move does not wait for a redundant size configure.
            if surface.settings.raster.is_none() {
                surface.layer.commit();
            }
        }
    }

    /// Presents only a small native buffer. Buffers are released by the
    /// compositor and the slot pool reuses their memory on subsequent moves.
    pub fn present_raster(&mut self, id: window::Id) -> Result<(), String> {
        let Some(surface) = self.surfaces.get(&id) else {
            return Ok(());
        };
        let Some(raster) = surface.settings.raster.clone() else {
            return Ok(());
        };
        if !surface.is_ready() {
            return Ok(());
        }
        if surface.raster_presented.as_ref() == Some(&raster) {
            if surface.raster_geometry_dirty {
                let surface = self.surfaces.get_mut(&id).expect("The surface exists");
                surface.layer.commit();
                surface.raster_geometry_dirty = false;
            }
            return Ok(());
        }
        let logical = surface.configured.expect("A ready surface has its size");
        log::debug!(
            "native raster {id:?}: buffer {:?}, destination {logical:?}, scale {}",
            raster.size,
            surface.scale
        );
        self.set_destination(id, logical);
        let surface = self.surfaces.get_mut(&id).expect("The surface exists");
        if surface.raster_pool.is_none() {
            surface.raster_pool = Some(
                SlotPool::new(raster.pixels.len() * 3, &self.shm)
                    .map_err(|error| error.to_string())?,
            );
        }
        let (buffer, canvas) = surface
            .raster_pool
            .as_mut()
            .unwrap()
            .create_buffer(
                raster.size.0 as i32,
                raster.size.1 as i32,
                raster.size.0 as i32 * 4,
                smithay_client_toolkit::reexports::client::protocol::wl_shm::Format::Argb8888,
            )
            .map_err(|error| error.to_string())?;
        canvas[..raster.pixels.len()].copy_from_slice(&raster.pixels);
        buffer
            .attach_to(surface.wl_surface())
            .map_err(|error| error.to_string())?;
        surface
            .wl_surface()
            .damage_buffer(0, 0, raster.size.0 as i32, raster.size.1 as i32);
        surface.layer.commit();
        surface.raster_presented = Some(raster);
        surface.raster_geometry_dirty = false;
        Ok(())
    }

    /// Asks the compositor for the geometry the surface's settings come to at
    /// its current scale, if it is not the one it has.
    pub fn sync_geometry(&mut self, id: window::Id, pixel_mode: iced_core::PixelScaleMode) {
        let Some(surface) = self.surfaces.get_mut(&id) else {
            return;
        };

        let pixel_scale = pixel_mode.resolve(surface.scale as f32);
        let wanted = geometry(&surface.settings, pixel_scale, surface.scale);

        if surface.applied == Some(wanted) {
            return;
        }

        let size_changed = surface.applied.map(|applied| applied.size) != Some(wanted.size);

        log::debug!(
            "surface {id:?}: asking for {wanted:?} (scale {:.4}, pixel scale {pixel_scale})",
            surface.scale
        );

        surface.layer.set_size(wanted.size.0, wanted.size.1);
        surface.layer.set_margin(
            wanted.margin[0],
            wanted.margin[1],
            wanted.margin[2],
            wanted.margin[3],
        );
        surface.layer.set_exclusive_zone(wanted.exclusive);

        if size_changed {
            surface.awaiting = Some(Instant::now());
        }

        surface.applied = Some(wanted);
        surface.raster_geometry_dirty = true;
        if surface.settings.raster.is_none() || size_changed {
            surface.layer.commit();
        }
    }

    /// Tells the compositor how big the buffer about to be attached is meant
    /// to appear. A buffer of `physical` pixels is shown over `logical` ones;
    /// without a viewporter the buffer's own scale carries the integer part.
    pub fn set_destination(&mut self, id: window::Id, logical: (u32, u32)) {
        let Some(surface) = self.surfaces.get_mut(&id) else {
            return;
        };

        if let Some(viewport) = &surface.viewport {
            if let Some(raster) = &surface.settings.raster {
                viewport.set_source(0.0, 0.0, raster.size.0 as f64, raster.size.1 as f64);
                viewport.set_destination(logical.0 as i32, logical.1 as i32);
                if surface.buffer_scale != 1 {
                    surface.wl_surface().set_buffer_scale(1);
                    surface.buffer_scale = 1;
                }
                surface.destination = Some(logical);
                return;
            }
            if surface.destination != Some(logical) || surface.buffer_scale != 1 {
                viewport.set_destination(logical.0 as i32, logical.1 as i32);
                surface.wl_surface().set_buffer_scale(1);
                surface.buffer_scale = 1;
                surface.destination = Some(logical);
            }
        } else {
            let scale = surface.scale.round().max(1.0) as i32;

            if surface.buffer_scale != scale {
                surface.wl_surface().set_buffer_scale(scale);
                surface.buffer_scale = scale;
            }
        }
    }

    pub fn set_cursor(&mut self, id: window::Id, icon: Option<CursorIcon>) {
        if self.pointer_focus != Some(id) {
            return;
        }

        if self.cursor == icon {
            return;
        }

        self.cursor = icon;

        let Some(pointer) = &self.pointer else {
            return;
        };

        let _ = match icon {
            Some(icon) => pointer.set_cursor(&self.conn, icon),
            None => pointer.hide_cursor(),
        };
    }

    fn id_of(&self, surface: &wl_surface::WlSurface) -> Option<window::Id> {
        self.by_wl.get(&surface.id()).copied()
    }

    fn push(&mut self, id: window::Id, event: Event) {
        self.events.push((id, event));
    }

    fn set_scale(&mut self, id: window::Id, scale: f64) {
        let Some(surface) = self.surfaces.get_mut(&id) else {
            return;
        };

        if surface.scale_known && (surface.scale - scale).abs() < 1e-6 {
            return;
        }

        log::info!("surface {id:?}: preferred scale {scale:.4}");

        surface.scale = scale;
        surface.scale_known = true;
        surface.version += 1;

        self.push(id, Event::Changed);
    }

    fn key(&mut self, event: KeyEvent, repeat: bool) {
        if let Some(id) = self.keyboard_focus {
            self.push(id, Event::KeyPressed { event, repeat });
        }
    }

    pub fn repeat_callback() -> Box<dyn FnMut(&mut Wl, &wl_keyboard::WlKeyboard, KeyEvent)> {
        Box::new(|wl, _keyboard, event| wl.key(event, true))
    }
}

impl CompositorHandler for Wl {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        // Only the integer fallback uses this: a compositor with fractional
        // scale tells each surface its own.
        if self.fractional_manager.is_some() {
            return;
        }

        if let Some(id) = self.id_of(surface) {
            self.set_scale(id, f64::from(new_factor));
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        output: &wl_output::WlOutput,
    ) {
        // Before the compositor has a preference, an output is a good guess.
        let Some(id) = self.id_of(surface) else {
            return;
        };

        let guess = self.output_scale(output);
        let name = self.output_state.info(output).and_then(|info| info.name);

        if let (Some(name), Some(surface)) = (name, self.surfaces.get_mut(&id)) {
            surface.output_name = Some(name);
        }

        if let (Some(guess), Some(surface)) = (guess, self.surfaces.get_mut(&id)) {
            if !surface.scale_known && (surface.scale - guess).abs() > 1e-6 {
                log::info!("surface {id:?}: entered an output at {guess:.4}");
                surface.scale = guess;
                surface.version += 1;
                self.push(id, Event::Changed);
            }
        }
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Wl {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for Wl {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(id) = self.id_of(layer.wl_surface()) {
            self.push(id, Event::Closed);
        }
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(id) = self.id_of(layer.wl_surface()) else {
            return;
        };

        if let Some(surface) = self.surfaces.get_mut(&id) {
            log::debug!("surface {id:?}: configured {:?}", configure.new_size);

            surface.configured = Some(configure.new_size);
            surface.awaiting = None;
            surface.version += 1;
        }

        self.push(id, Event::Changed);
    }
}

impl SeatHandler for Wl {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard if self.keyboard.is_none() => {
                match self.seat_state.get_keyboard_with_repeat(
                    qh,
                    &seat,
                    None,
                    self.loop_handle.clone(),
                    Wl::repeat_callback(),
                ) {
                    Ok(keyboard) => self.keyboard = Some(keyboard),
                    Err(error) => log::warn!("No keyboard: {error}"),
                }
            }
            Capability::Pointer if self.pointer.is_none() => {
                let cursor_surface = self.compositor_state.create_surface(qh);

                match self.seat_state.get_pointer_with_theme(
                    qh,
                    &seat,
                    self.shm.wl_shm(),
                    cursor_surface,
                    ThemeSpec::System,
                ) {
                    Ok(pointer) => self.pointer = Some(pointer),
                    Err(error) => log::warn!("No pointer: {error}"),
                }
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Keyboard => {
                if let Some(keyboard) = self.keyboard.take() {
                    keyboard.release();
                }
            }
            Capability::Pointer => {
                if let Some(pointer) = self.pointer.take() {
                    pointer.pointer().release();
                }
            }
            _ => {}
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for Wl {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        if let Some(id) = self.id_of(surface) {
            log::debug!("keyboard focus: {id:?}");

            self.keyboard_focus = Some(id);
            self.push(id, Event::Focused(true));
        }
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        if let Some(id) = self.id_of(surface) {
            log::debug!("keyboard focus lost: {id:?}");

            if self.keyboard_focus == Some(id) {
                self.keyboard_focus = None;
            }

            self.push(id, Event::Focused(false));
        }
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.key(event, false);
    }

    fn repeat_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        self.key(event, true);
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if let Some(id) = self.keyboard_focus {
            self.push(id, Event::KeyReleased(event));
        }
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _raw: RawModifiers,
        _layout: u32,
    ) {
        if let Some(id) = self.keyboard_focus {
            self.push(id, Event::Modifiers(modifiers));
        }
    }
}

impl PointerHandler for Wl {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let Some(id) = self.id_of(&event.surface) else {
                continue;
            };

            match event.kind {
                PointerEventKind::Enter { .. } => {
                    log::debug!("pointer entered {id:?} at {:?}", event.position);

                    self.pointer_focus = Some(id);
                    self.cursor = None;
                    self.push(id, Event::PointerEntered);
                    self.push(id, Event::PointerMoved(event.position));
                }
                PointerEventKind::Leave { .. } => {
                    if self.pointer_focus == Some(id) {
                        self.pointer_focus = None;
                    }

                    self.push(id, Event::PointerLeft);
                }
                PointerEventKind::Motion { .. } => {
                    self.push(id, Event::PointerMoved(event.position));
                }
                PointerEventKind::Press { button, .. } => {
                    log::debug!(
                        "pointer button {button:#x} on {id:?} at {:?}",
                        event.position
                    );

                    self.push(
                        id,
                        Event::PointerButton {
                            button,
                            pressed: true,
                        },
                    );
                }
                PointerEventKind::Release { button, .. } => {
                    self.push(
                        id,
                        Event::PointerButton {
                            button,
                            pressed: false,
                        },
                    );
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    self.push(
                        id,
                        Event::PointerScroll {
                            x: horizontal,
                            y: vertical,
                        },
                    );
                }
            }
        }
    }
}

impl ShmHandler for Wl {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Wl {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}

impl Dispatch<WpViewporter, GlobalData> for Wl {
    fn event(
        _: &mut Self,
        _: &WpViewporter,
        _: <WpViewporter as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpViewport, GlobalData> for Wl {
    fn event(
        _: &mut Self,
        _: &WpViewport,
        _: <WpViewport as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpFractionalScaleManagerV1, GlobalData> for Wl {
    fn event(
        _: &mut Self,
        _: &WpFractionalScaleManagerV1,
        _: <WpFractionalScaleManagerV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpFractionalScaleV1, FractionalData> for Wl {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        data: &FractionalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            log::debug!("fractional scale {:?}: {scale}/120", data.0);
            state.set_scale(data.0, f64::from(scale) / 120.0);
        }
    }
}

impl Dispatch<HyprlandFocusGrabManagerV1, GlobalData> for Wl {
    fn event(
        _: &mut Self,
        _: &HyprlandFocusGrabManagerV1,
        _: <HyprlandFocusGrabManagerV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<HyprlandFocusGrabV1, GlobalData> for Wl {
    fn event(
        state: &mut Self,
        grab: &HyprlandFocusGrabV1,
        event: hyprland_focus_grab_v1::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let hyprland_focus_grab_v1::Event::Cleared = event;

        {
            log::debug!("focus grab cleared");

            // Only the grab we hold counts; one we already replaced is noise.
            if state
                .grab
                .as_ref()
                .is_some_and(|active| &active.grab == grab)
            {
                state.grab = None;
                state.grab_cleared = true;
            }
        }
    }
}

delegate_compositor!(Wl);
delegate_output!(Wl);
delegate_shm!(Wl);
delegate_seat!(Wl);
delegate_keyboard!(Wl);
delegate_pointer!(Wl);
delegate_layer!(Wl);
delegate_registry!(Wl);

#[cfg(test)]
mod tests {
    use super::*;

    const FIVE_THIRDS: f64 = 200.0 / 120.0;

    #[test]
    fn multiples_of_five_virtual_pixels_are_exact_at_five_thirds() {
        for virtual_px in (5..=200).step_by(5) {
            let logical = logical_for(virtual_px, 3, FIVE_THIRDS);

            assert!(is_exact(logical, FIVE_THIRDS), "{virtual_px} -> {logical}");
            assert_eq!(
                (f64::from(logical) * FIVE_THIRDS).round() as u32,
                virtual_px * 3
            );
        }
    }

    #[test]
    fn the_bar_height_of_the_spike() {
        assert_eq!(logical_for(25, 3, FIVE_THIRDS), 45);
        assert_eq!(logical_for(25, 2, 1.0), 50);
        assert_eq!(logical_for(25, 4, 2.0), 50);
    }

    #[test]
    fn an_integer_scale_is_always_exact() {
        for virtual_px in 1..100 {
            assert_eq!(logical_for(virtual_px, 2, 1.0), virtual_px * 2);
            assert_eq!(logical_for(virtual_px, 4, 2.0), virtual_px * 2);
        }
    }

    #[test]
    fn a_size_that_cannot_be_exact_is_still_covered() {
        for virtual_px in 1..200 {
            let logical = logical_for(virtual_px, 3, FIVE_THIRDS);

            assert!(f64::from(logical) * FIVE_THIRDS + 0.5 >= f64::from(virtual_px * 3));
            // ...and by no more than three virtual pixels.
            assert!(f64::from(logical) * FIVE_THIRDS <= f64::from((virtual_px + 3) * 3) + 0.5);
        }
    }
}
