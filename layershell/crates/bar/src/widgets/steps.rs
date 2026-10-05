//! A read-only stepped gauge for a level of a few steps: the signal of a
//! network, in four. On an inverse row (the accent behind it) the lit steps
//! are knocked out of it, in the colour of the text there, instead of
//! disappearing into it.
use iced_widget::core::layout::{self, Layout};
use iced_widget::core::renderer;
use iced_widget::core::widget::{Meta, Tree};
use iced_widget::core::{Length, Point, Rectangle, Size, Widget, mouse};

use quadrille::theme::mix;
use quadrille::{Theme, px};

const HEIGHT: i32 = 5;
const GAP: i32 = 1;

/// `level` of `of` steps lit.
pub struct Steps {
    level: u16,
    of: u16,
    segment: u16,
    inverse: bool,
}

impl Steps {
    pub fn new(level: u16, of: u16) -> Self {
        Self {
            level: level.min(of),
            of: of.max(1),
            segment: 2,
            inverse: false,
        }
    }

    pub fn inverse(mut self, inverse: bool) -> Self {
        self.inverse = inverse;
        self
    }

    /// The width of a gauge of `of` steps of `segment` pixels.
    pub fn width_of(of: u16, segment: u16) -> u16 {
        of * segment + of.saturating_sub(1) * GAP as u16
    }

    fn width(&self) -> f32 {
        f32::from(Self::width_of(self.of, self.segment))
    }
}

impl Meta for Steps {}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Steps
where
    Renderer: iced_widget::core::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.width()), Length::Fixed(HEIGHT as f32))
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, self.width(), HEIGHT as f32);
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = px::interior(layout.bounds());
        let palette = theme.palette();

        let (lit, unlit) = if self.inverse {
            (
                palette.on_accent,
                mix(palette.accent, palette.on_accent, 0.35),
            )
        } else {
            (palette.accent, palette.raised)
        };

        let pitch = i32::from(self.segment) + GAP;

        for step in 0..i32::from(self.of) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        Point::new((bounds.x + step * pitch) as f32, bounds.y as f32),
                        Size::new(f32::from(self.segment), HEIGHT as f32),
                    ),
                    snap: true,
                    ..renderer::Quad::default()
                },
                if step < i32::from(self.level) {
                    lit
                } else {
                    unlit
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_steps_of_two_are_eleven_wide() {
        assert_eq!(Steps::width_of(4, 2), 11);
    }
}
