//! The pieces the panels share: icons, a stepped slider, rows to choose from,
//! the corner brackets of focus, and a field to type in. Built from the
//! toolkit's parts and styled by its theme, so a panel is a composition of
//! these and the toolkit's own widgets, and nothing in it is drawn twice.
pub mod focus;
pub mod icons;
pub mod input;
pub mod rows;
pub mod slider;
pub mod steps;

pub use focus::marks;
pub use input::field;
pub use slider::Slider;
pub use steps::Steps;

use iced_widget::{Column, Row, Widget};

use quadrille::{Element, Theme};

/// Adds any widget to a column or a row of elements, boxing it: the toolkit's
/// widgets are widgets, not yet elements, and `push` wants elements.
pub trait Add<'a, Message> {
    fn add(self, child: impl Widget<Message, Theme, iced_widget::Renderer> + 'a) -> Self;
}

impl<'a, Message: 'a> Add<'a, Message> for Column<Element<'a, Message>> {
    fn add(self, child: impl Widget<Message, Theme, iced_widget::Renderer> + 'a) -> Self {
        self.push(child.boxed())
    }
}

impl<'a, Message: 'a> Add<'a, Message> for Row<Element<'a, Message>> {
    fn add(self, child: impl Widget<Message, Theme, iced_widget::Renderer> + 'a) -> Self {
        self.push(child.boxed())
    }
}
