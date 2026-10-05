//! The corner brackets that say where the keyboard is.
//!
//! Selection and focus are brackets, not boxes: an arm of three pixels along
//! each side at each corner of what is focused, in the ink. [`marks`] puts
//! them over a row (they draw nothing when it is not focused, and take no
//! part in the mouse), so the row's own look is the same either way.
use iced_widget::canvas::{self, Frame, Geometry};
use iced_widget::core::{Length, Rectangle, mouse};
use iced_widget::stack;

use iced_widget::Widget as _;

use quadrille::draw::{Pen, rectangle};
use quadrille::{Element, Theme, px};

/// The length of an arm of a bracket.
const ARM: i32 = 3;

struct Brackets {
    focused: bool,
    width: Length,
    height: Length,
}

impl<Message> canvas::Program<Message, Theme> for Brackets {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced_widget::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        if !self.focused {
            return Vec::new();
        }

        let size = px::floor(bounds.size());
        let mut frame = Frame::new(renderer, bounds.size());

        Pen::new(&mut frame).brackets(
            rectangle(0, 0, size.width, size.height),
            ARM,
            1,
            theme.palette().ink,
        );

        vec![frame.into_geometry()]
    }
}

quadrille::canvas_widget!(Brackets);

/// `content`, with the brackets of focus over it when `focused`.
pub fn marks<'a, Message: 'a>(
    content: Element<'a, Message>,
    focused: bool,
) -> Element<'a, Message> {
    stack![
        content,
        Brackets {
            focused,
            width: Length::Fill,
            height: Length::Fill,
        }
    ]
    .boxed()
}
