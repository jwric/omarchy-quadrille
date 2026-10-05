//! The raw Wayland handles of a layer surface, as iced's compositors want them.
use iced_core::window::raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle, WindowHandle,
};

use std::ffi::c_void;
use std::ptr::NonNull;

/// A `wl_surface` and the `wl_display` it lives on.
///
/// The pointers come from libwayland (`wayland-backend` is built with
/// `client_system`), which is what both wgpu (Vulkan WSI) and softbuffer need
/// to attach buffers to a surface that sctk created. A [`Handle`] must not
/// outlive the surface: the shell drops the renderer's surface first.
#[derive(Debug, Clone, Copy)]
pub struct Handle {
    display: NonNull<c_void>,
    surface: NonNull<c_void>,
}

// SAFETY: the pointers are only handed to the graphics libraries, which use
// them from the thread that owns the event loop (the only thread that draws).
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

impl Handle {
    pub(crate) fn new(display: NonNull<c_void>, surface: NonNull<c_void>) -> Self {
        Self { display, surface }
    }
}

impl HasWindowHandle for Handle {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: see the type's documentation.
        Ok(unsafe {
            WindowHandle::borrow_raw(RawWindowHandle::Wayland(WaylandWindowHandle::new(
                self.surface,
            )))
        })
    }
}

impl HasDisplayHandle for Handle {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        // SAFETY: see the type's documentation.
        Ok(unsafe {
            DisplayHandle::borrow_raw(RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                self.display,
            )))
        })
    }
}
