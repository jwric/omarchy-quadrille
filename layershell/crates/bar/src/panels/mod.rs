//! The panels that read and change the machine: audio, network, Bluetooth and
//! power. Each is a module with the same shape: a `State`, a `Message`, a
//! `refresh` that reads, an `update` that acts, a `key` for the keyboard, an
//! `escape` for backing out of what a panel is in the middle of, and a `view`
//! built from the shared widgets.
//!
//! None of them runs anything while it is hidden. The host asks for a
//! `refresh` when a panel is shown and on a fixed beat while it stays shown.
pub mod audio;
pub mod bluetooth;
pub mod network;
pub mod power;

use iced_core::keyboard::{self, Modifiers, key::Named};

/// The keys a panel answers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Space,
    Tab,
    BackTab,
    Home,
    End,
    PageUp,
    PageDown,
    Char(char),
}

impl Key {
    /// The key a press means to a panel, if it means one.
    pub fn of(key: &keyboard::Key, modifiers: Modifiers) -> Option<Self> {
        if modifiers.control() || modifiers.alt() || modifiers.logo() {
            return None;
        }

        Some(match key {
            keyboard::Key::Named(Named::ArrowUp) => Self::Up,
            keyboard::Key::Named(Named::ArrowDown) => Self::Down,
            keyboard::Key::Named(Named::ArrowLeft) => Self::Left,
            keyboard::Key::Named(Named::ArrowRight) => Self::Right,
            keyboard::Key::Named(Named::Enter) => Self::Enter,
            keyboard::Key::Named(Named::Space) => Self::Space,
            keyboard::Key::Named(Named::Tab) if modifiers.shift() => Self::BackTab,
            keyboard::Key::Named(Named::Tab) => Self::Tab,
            keyboard::Key::Named(Named::Home) => Self::Home,
            keyboard::Key::Named(Named::End) => Self::End,
            keyboard::Key::Named(Named::PageUp) => Self::PageUp,
            keyboard::Key::Named(Named::PageDown) => Self::PageDown,
            keyboard::Key::Character(text) => {
                let mut chars = text.chars();

                match (chars.next(), chars.next()) {
                    (Some(c), None) => Self::Char(c.to_ascii_lowercase()),
                    _ => return None,
                }
            }
            _ => return None,
        })
    }
}

/// Where the keyboard goes next among `count` items, wrapping at the ends.
pub fn step(focus: usize, count: usize, forward: bool) -> usize {
    if count == 0 {
        0
    } else if forward {
        (focus + 1) % count
    } else {
        (focus + count - 1) % count
    }
}

/// A volume moved by `steps` of `of` (in a slider of `of` steps), as a
/// percentage, never outside 0 to 100.
pub fn moved(percent: u8, steps: i32, of: u16) -> u8 {
    use crate::widgets::slider::{percent_of, step_of};

    let step = i32::from(step_of(percent, of)) + steps;

    percent_of(step.clamp(0, i32::from(of)) as u16, of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_core::SmolStr;

    #[test]
    fn the_keyboard_wraps_round_a_panel() {
        assert_eq!(step(0, 3, true), 1);
        assert_eq!(step(2, 3, true), 0);
        assert_eq!(step(0, 3, false), 2);
        assert_eq!(step(0, 0, true), 0);
    }

    #[test]
    fn keys_mean_what_a_panel_expects() {
        let none = Modifiers::empty();
        let named = |named| keyboard::Key::Named(named);

        assert_eq!(Key::of(&named(Named::ArrowUp), none), Some(Key::Up));
        assert_eq!(Key::of(&named(Named::Enter), none), Some(Key::Enter));
        assert_eq!(Key::of(&named(Named::Tab), none), Some(Key::Tab));
        assert_eq!(
            Key::of(&named(Named::Tab), Modifiers::SHIFT),
            Some(Key::BackTab)
        );
        assert_eq!(
            Key::of(
                &keyboard::Key::Character(SmolStr::new("M")),
                Modifiers::SHIFT
            ),
            Some(Key::Char('m'))
        );
        assert_eq!(
            Key::of(
                &keyboard::Key::Character(SmolStr::new("c")),
                Modifiers::CTRL
            ),
            None,
            "a shortcut of the desktop is not for a panel"
        );
        assert_eq!(Key::of(&named(Named::F5), none), None);
    }

    #[test]
    fn a_volume_moves_by_steps_and_stays_in_range() {
        assert_eq!(moved(50, 1, 20), 55);
        assert_eq!(moved(50, -2, 20), 40);
        assert_eq!(moved(98, 3, 20), 100);
        assert_eq!(moved(2, -5, 20), 0);
    }
}
