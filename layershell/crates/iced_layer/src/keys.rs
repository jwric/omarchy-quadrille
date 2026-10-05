//! xkbcommon keysyms to iced's keyboard types.
use iced_core::SmolStr;
use iced_core::keyboard::key::{Key, Named, NativeCode, Physical};
use iced_core::keyboard::{Location, Modifiers};

use smithay_client_toolkit::seat::keyboard::{self, Keysym};

pub fn modifiers(modifiers: keyboard::Modifiers) -> Modifiers {
    let mut result = Modifiers::empty();

    result.set(Modifiers::SHIFT, modifiers.shift);
    result.set(Modifiers::CTRL, modifiers.ctrl);
    result.set(Modifiers::ALT, modifiers.alt);
    result.set(Modifiers::LOGO, modifiers.logo);

    result
}

/// The logical key of a keysym, with the text it produced as a fallback.
pub fn key(keysym: Keysym, text: Option<&str>) -> Key {
    use Named::*;

    let named = match keysym {
        Keysym::Return | Keysym::KP_Enter => Enter,
        Keysym::Escape => Escape,
        Keysym::Tab | Keysym::ISO_Left_Tab => Tab,
        Keysym::BackSpace => Backspace,
        Keysym::Delete | Keysym::KP_Delete => Delete,
        Keysym::Insert => Insert,
        Keysym::space => Space,
        Keysym::Left | Keysym::KP_Left => ArrowLeft,
        Keysym::Right | Keysym::KP_Right => ArrowRight,
        Keysym::Up | Keysym::KP_Up => ArrowUp,
        Keysym::Down | Keysym::KP_Down => ArrowDown,
        Keysym::Home | Keysym::KP_Home => Home,
        Keysym::End | Keysym::KP_End => End,
        Keysym::Page_Up | Keysym::KP_Page_Up => PageUp,
        Keysym::Page_Down | Keysym::KP_Page_Down => PageDown,
        Keysym::Shift_L | Keysym::Shift_R => Shift,
        Keysym::Control_L | Keysym::Control_R => Control,
        Keysym::Alt_L | Keysym::Alt_R => Alt,
        Keysym::Super_L | Keysym::Super_R => Super,
        Keysym::Meta_L | Keysym::Meta_R => Meta,
        Keysym::Caps_Lock => CapsLock,
        Keysym::Num_Lock => NumLock,
        Keysym::F1 => F1,
        Keysym::F2 => F2,
        Keysym::F3 => F3,
        Keysym::F4 => F4,
        Keysym::F5 => F5,
        Keysym::F6 => F6,
        Keysym::F7 => F7,
        Keysym::F8 => F8,
        Keysym::F9 => F9,
        Keysym::F10 => F10,
        Keysym::F11 => F11,
        Keysym::F12 => F12,
        _ => {
            return match text.filter(|text| text.chars().all(|c| !c.is_control())) {
                Some(text) if !text.is_empty() => Key::Character(SmolStr::new(text)),
                _ => match keysym.key_char() {
                    Some(c) if !c.is_control() => Key::Character(SmolStr::new(c.to_string())),
                    _ => Key::Unidentified,
                },
            };
        }
    };

    Key::Named(named)
}

/// The text a key press produced, if it is text and not a control.
pub fn text(text: Option<&str>) -> Option<SmolStr> {
    text.filter(|text| !text.is_empty() && text.chars().all(|c| !c.is_control()))
        .map(SmolStr::new)
}

/// The physical key: the evdev code the compositor sent, offset by 8 the way
/// xkbcommon numbers them.
pub fn physical(raw_code: u32) -> Physical {
    Physical::Unidentified(NativeCode::Xkb(raw_code + 8))
}

pub fn location(keysym: Keysym) -> Location {
    match keysym {
        Keysym::Shift_L | Keysym::Control_L | Keysym::Alt_L | Keysym::Super_L | Keysym::Meta_L => {
            Location::Left
        }
        Keysym::Shift_R | Keysym::Control_R | Keysym::Alt_R | Keysym::Super_R | Keysym::Meta_R => {
            Location::Right
        }
        k if (Keysym::KP_Space.raw()..=Keysym::KP_9.raw()).contains(&k.raw()) => Location::Numpad,
        _ => Location::Standard,
    }
}
