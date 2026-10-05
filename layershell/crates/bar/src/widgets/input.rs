//! A field to type in, one line, in the toolkit's own style.
use iced_widget::core::widget::Id;

use quadrille::{Element, widget};

/// A text field. `secure` hides what is typed.
pub fn field<'a, Message>(
    id: &'static str,
    placeholder: &'a str,
    value: &'a str,
    secure: bool,
    on_input: impl Fn(String) -> Message + 'a,
    on_submit: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    iced_widget::Widget::boxed(
        widget::text_input(placeholder, value)
            .id(Id::new(id))
            .secure(secure)
            .on_input(on_input)
            .on_submit(on_submit),
    )
}
