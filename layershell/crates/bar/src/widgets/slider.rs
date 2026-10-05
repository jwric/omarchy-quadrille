//! A stepped slider: a row of segments, the first `value` of them lit.
//!
//! Click or drag along it to set the value, scroll to move it a step, or
//! (with the keyboard in the panel) step it with the arrow keys; the panel
//! does that, since a panel has one place for the keyboard, shown by
//! [`focus`](super::focus). Nothing is in between the steps: a segment is lit
//! or it is not.
use iced_widget::core::layout::{self, Layout};
use iced_widget::core::renderer;
use iced_widget::core::widget::{Meta, Tree, tree};
use iced_widget::core::{Color, Event, Length, Point, Rectangle, Size};
use iced_widget::core::{Shell, Widget, mouse, window};

use quadrille::{Theme, px};

/// The height of the slider's reach, in virtual pixels; its segments are
/// [`SEGMENT_HEIGHT`] high in the middle of it, so that the target is not as
/// thin as the mark.
pub const HEIGHT: u16 = 9;

pub const SEGMENT_HEIGHT: i32 = 5;

/// The gap between two segments.
pub const GAP: i32 = 1;

/// A stepped slider of `steps` segments.
pub struct Slider<'a, Message> {
    steps: u16,
    value: u16,
    segment: u16,
    muted: bool,
    on_change: Box<dyn Fn(u16) -> Message + 'a>,
}

impl<'a, Message> Slider<'a, Message> {
    /// A slider of `steps` segments with the first `value` lit.
    pub fn new(steps: u16, value: u16, on_change: impl Fn(u16) -> Message + 'a) -> Self {
        let steps = steps.max(1);

        Self {
            steps,
            value: value.min(steps),
            segment: 3,
            muted: false,
            on_change: Box::new(on_change),
        }
    }

    /// Sets how wide a segment is, in virtual pixels.
    pub fn segment(mut self, segment: u16) -> Self {
        self.segment = segment.max(1);
        self
    }

    /// Draws it dimmed: what is set, but not heard.
    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }

    /// The width of a slider of `steps` segments of `segment` pixels.
    pub fn width_of(steps: u16, segment: u16) -> u16 {
        steps * segment + steps.saturating_sub(1) * GAP as u16
    }

    fn width(&self) -> f32 {
        f32::from(Self::width_of(self.steps, self.segment))
    }

    fn set(&mut self, value: u16, shell: &mut Shell<'_, Message>) {
        let value = value.min(self.steps);

        // Events can arrive in a batch before the view is built again, so the
        // slider keeps what it published: the next event goes on from there.
        if value != self.value {
            self.value = value;
            shell.publish((self.on_change)(value));
        }
    }
}

/// Which step a pointer `x` pixels from the left edge of a slider means: none
/// to the left of it (zero), the segment it is over or has just passed, and
/// all of them to the right.
pub fn step_at(x: f32, steps: u16, segment: u16) -> u16 {
    if x < 0.0 {
        return 0;
    }

    let pitch = i32::from(segment) + GAP;

    ((x.floor() as i32 / pitch) + 1).clamp(0, i32::from(steps)) as u16
}

#[derive(Debug, Default)]
struct State {
    dragging: bool,
    wheel: f32,
}

impl<Message> Meta for Slider<'_, Message> {}

impl<Message, Renderer> Widget<Message, Theme, Renderer> for Slider<'_, Message>
where
    Renderer: iced_widget::core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(
            Length::Fixed(self.width()),
            Length::Fixed(f32::from(HEIGHT)),
        )
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = layout::atomic(limits, self.width(), f32::from(HEIGHT));
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(position) = cursor.position_over(bounds) {
                    state.dragging = true;
                    self.set(
                        step_at(position.x - bounds.x, self.steps, self.segment),
                        shell,
                    );
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if state.dragging
                    && let Some(position) = cursor.land().position()
                {
                    self.set(
                        step_at(position.x - bounds.x, self.steps, self.segment),
                        shell,
                    );
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.dragging = false;
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                // One notch is one step, whatever the platform calls a notch:
                // small deltas (a touchpad's) add up until they are one.
                let notches = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 100.0,
                };

                state.wheel += notches.clamp(-1.0, 1.0);

                let whole = state.wheel.trunc();

                state.wheel -= whole;

                if whole != 0.0 {
                    let value = i32::from(self.value) + whole as i32;

                    self.set(value.clamp(0, i32::from(self.steps)) as u16, shell);
                }

                shell.capture_event();
            }
            Event::Window(window::Event::Unfocused) => state.dragging = false,
            _ => {}
        }
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

        let lit = if self.muted {
            palette.faint
        } else {
            palette.accent
        };
        let unlit = palette.raised;

        let top = bounds.y + (bounds.height - SEGMENT_HEIGHT) / 2;
        let pitch = i32::from(self.segment) + GAP;

        for step in 0..i32::from(self.steps) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        Point::new((bounds.x + step * pitch) as f32, top as f32),
                        Size::new(f32::from(self.segment), SEGMENT_HEIGHT as f32),
                    ),
                    snap: true,
                    ..renderer::Quad::default()
                },
                if step < i32::from(self.value) {
                    lit
                } else {
                    unlit
                },
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();

        if state.dragging || cursor.is_over(layout.bounds()) {
            mouse::Interaction::ResizingHorizontally
        } else {
            mouse::Interaction::None
        }
    }
}

/// A slider as a value of the number of steps of a percentage: 5 % each when
/// there are twenty.
pub fn percent_of(step: u16, steps: u16) -> u8 {
    (u32::from(step) * 100 / u32::from(steps.max(1))).min(100) as u8
}

/// The nearest step of `steps` to a percentage.
pub fn step_of(percent: u8, steps: u16) -> u16 {
    ((u32::from(percent.min(100)) * u32::from(steps) + 50) / 100) as u16
}

#[allow(dead_code)]
const fn _colour() -> Color {
    Color::TRANSPARENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slider_is_as_wide_as_its_segments_and_gaps() {
        assert_eq!(Slider::<()>::width_of(20, 3), 79);
        assert_eq!(Slider::<()>::width_of(20, 2), 59);
        assert_eq!(Slider::<()>::width_of(1, 5), 5);
    }

    #[test]
    fn a_pointer_means_the_segment_it_is_over_or_past() {
        // Segments of 3 with a gap of 1: they start at 0, 4, 8, ...
        assert_eq!(step_at(-0.1, 20, 3), 0, "left of the first is zero");
        assert_eq!(step_at(0.0, 20, 3), 1);
        assert_eq!(step_at(2.9, 20, 3), 1);
        assert_eq!(
            step_at(3.5, 20, 3),
            1,
            "the gap belongs to the segment before it"
        );
        assert_eq!(step_at(4.0, 20, 3), 2);
        assert_eq!(step_at(78.9, 20, 3), 20);
        assert_eq!(
            step_at(500.0, 20, 3),
            20,
            "right of the last is all of them"
        );
    }

    #[test]
    fn percentages_and_steps_round_trip() {
        for step in 0..=20 {
            assert_eq!(step_of(percent_of(step, 20), 20), step);
        }

        assert_eq!(percent_of(10, 20), 50);
        assert_eq!(step_of(63, 20), 13);
        assert_eq!(step_of(200, 20), 20);
    }
}
