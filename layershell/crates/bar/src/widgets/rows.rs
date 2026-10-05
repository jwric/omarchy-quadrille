//! A row to choose from: an icon, a label, and whatever goes at its end.
//!
//! The row that is chosen (the default device, the network in use) is an
//! inverse block, the accent behind its text; the others have no face until
//! the pointer is over them. A label that does not fit is cut with an
//! ellipsis, and one with no room even for that is left out, never cut to a
//! letter: that is [`Face::fit`], used by [`fit`].
use iced_widget::core::{Alignment, Length};
use iced_widget::{Widget as _, row, space};

use super::Add as _;
use quadrille::draw::Sprite;
use quadrille::{Element, Face, px, style, widget};

/// The padding of a row, left and right.
pub const PADDING: u16 = 4;

/// `text` fitted to `width` pixels.
pub fn fit(text: &str, width: u16) -> String {
    Face::BODY.fit(text, f32::from(width)).into_owned()
}

/// How many pixels a row has for its label, in a panel `inner` pixels wide,
/// after its icon, its padding, the gaps and `trailing` pixels at its end.
pub fn label_width(inner: u16, icon: bool, trailing: u16) -> u16 {
    let icon = if icon { 7 + px::GAP as u16 } else { 0 };
    let trailing = if trailing > 0 {
        trailing + px::GAP as u16
    } else {
        0
    };

    inner.saturating_sub(2 * PADDING + icon + trailing)
}

/// A row that is pressed to choose it.
pub fn choice<'a, Message>(
    chosen: bool,
    sprite: Option<Sprite>,
    label: impl Into<String>,
    trailing: Option<Element<'a, Message>>,
    on_press: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let mut content = row![].spacing(px::GAP).align_y(Alignment::Center);

    if let Some(sprite) = sprite {
        content = content.add(widget::icon::<quadrille::Theme>(sprite));
    }

    content = content.add(widget::label(label.into()));
    content = content.add(space::horizontal());

    if let Some(trailing) = trailing {
        content = content.add(trailing);
    }

    let button = iced_widget::button(content)
        .padding(Face::BODY.padding(PADDING, 2, 2))
        .width(Length::Fill)
        .on_press(on_press);

    if chosen {
        button.style(style::button::engaged).boxed()
    } else {
        button.style(style::button::ghost).boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_is_dropped_before_it_is_cut_to_a_letter() {
        // Six pixels a cell.
        assert_eq!(fit("LIVING ROOM", 66), "LIVING ROOM");
        assert_eq!(
            fit("LIVING ROOM", 60),
            "LIVING ROO…".chars().take(9).collect::<String>() + "…"
        );
        assert_eq!(
            fit("LIVING ROOM", 11),
            "",
            "no room for a letter and an ellipsis"
        );
        assert_eq!(fit("LIVING ROOM", 12), "L…");
    }

    #[test]
    fn a_row_gives_its_label_what_the_rest_leaves() {
        // 144 pixels: padding 8, an icon and a gap 11, a 24-pixel reading and a gap 28.
        assert_eq!(label_width(144, true, 24), 144 - 8 - 11 - 28);
        assert_eq!(label_width(144, false, 0), 136);
        assert_eq!(label_width(10, true, 24), 0);
    }
}
