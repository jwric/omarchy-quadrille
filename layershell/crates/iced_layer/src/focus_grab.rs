//! Bindings for `hyprland_focus_grab_v1`, generated from the XML next to the
//! crate, which is how Hyprland lets a layer-surface popup be dismissed by a
//! click outside of it.
#![allow(clippy::all, missing_docs)]

pub mod protocol {
    use client as wayland_client;
    use client::protocol::*;
    use smithay_client_toolkit::reexports::client;

    pub mod __interfaces {
        use super::client::protocol::__interfaces::*;

        wayland_scanner::generate_interfaces!("protocols/hyprland-focus-grab-v1.xml");
    }

    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocols/hyprland-focus-grab-v1.xml");
}
