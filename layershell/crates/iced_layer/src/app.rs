//! A builder for programs that live on layer surfaces.
use crate::shell::{self, Error, Layered};
use crate::wl::SurfaceSettings;

use iced_core::theme;
use iced_core::window;
use iced_core::{Element, Settings, Widget};
use iced_futures::Subscription;
use iced_program::Program;
use iced_renderer::Renderer;
use iced_runtime::Task;

type Boot<State, Message> = Box<dyn Fn() -> (State, Task<Message>)>;
type Update<State, Message> = Box<dyn Fn(&mut State, Message) -> Task<Message>>;
type View<State, Message, Theme> =
    Box<dyn for<'a> Fn(&'a State, window::Id) -> Element<'a, Message, Theme, Renderer>>;
type Surfaces<State> = Box<dyn Fn(&State) -> Vec<(window::Id, SurfaceSettings)>>;

/// The shape of an application whose windows are layer surfaces: what a
/// `iced::daemon` is, with the windows declared by the state instead of
/// opened by tasks.
pub struct Application<State, Message, Theme> {
    boot: Boot<State, Message>,
    update: Update<State, Message>,
    view: View<State, Message, Theme>,
    surfaces: Surfaces<State>,
    subscription: Option<Box<dyn Fn(&State) -> Subscription<Message>>>,
    theme: Option<Box<dyn Fn(&State, window::Id) -> Theme>>,
    settings: Settings,
}

/// Starts an [`Application`].
///
/// `surfaces` is what makes it a layer-shell program: it says which surfaces
/// exist for the current state, and the shell opens and closes them to match.
pub fn application<State, Message, Theme>(
    boot: impl Fn() -> (State, Task<Message>) + 'static,
    update: impl Fn(&mut State, Message) -> Task<Message> + 'static,
    view: impl for<'a> Fn(&'a State, window::Id) -> Element<'a, Message, Theme, Renderer> + 'static,
    surfaces: impl Fn(&State) -> Vec<(window::Id, SurfaceSettings)> + 'static,
) -> Application<State, Message, Theme>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base + 'static,
{
    Application {
        boot: Box::new(boot),
        update: Box::new(update),
        view: Box::new(view),
        surfaces: Box::new(surfaces),
        subscription: None,
        theme: None,
        settings: Settings::default(),
    }
}

impl<State, Message, Theme> Application<State, Message, Theme>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base + 'static,
{
    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }

    pub fn subscription(mut self, f: impl Fn(&State) -> Subscription<Message> + 'static) -> Self {
        self.subscription = Some(Box::new(f));
        self
    }

    pub fn theme(mut self, f: impl Fn(&State, window::Id) -> Theme + 'static) -> Self {
        self.theme = Some(Box::new(f));
        self
    }

    pub fn run(self) -> Result<(), Error> {
        shell::run(self)
    }
}

impl<State, Message, Theme> Program for Application<State, Message, Theme>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base + 'static,
{
    type State = State;
    type Message = Message;
    type Theme = Theme;
    type Renderer = Renderer;
    type Executor = iced_futures::backend::native::smol::Executor;

    fn name() -> &'static str {
        "iced_layer"
    }

    fn settings(&self) -> Settings {
        self.settings.clone()
    }

    fn window(&self) -> Option<window::Settings> {
        None
    }

    fn boot(&self) -> (Self::State, Task<Self::Message>) {
        (self.boot)()
    }

    fn update(&self, state: &mut Self::State, message: Self::Message) -> Task<Self::Message> {
        (self.update)(state, message)
    }

    fn view<'a>(
        &self,
        state: &'a Self::State,
        window: window::Id,
    ) -> impl Widget<Self::Message, Self::Theme, Self::Renderer> + 'a {
        (self.view)(state, window)
    }

    fn subscription(&self, state: &Self::State) -> Subscription<Self::Message> {
        self.subscription
            .as_ref()
            .map(|subscription| subscription(state))
            .unwrap_or_else(Subscription::none)
    }

    fn theme(&self, state: &Self::State, window: window::Id) -> Option<Self::Theme> {
        self.theme.as_ref().map(|theme| theme(state, window))
    }
}

impl<State, Message, Theme> Layered for Application<State, Message, Theme>
where
    State: 'static,
    Message: Send + 'static,
    Theme: theme::Base + 'static,
{
    fn surfaces(&self, state: &Self::State) -> Vec<(window::Id, SurfaceSettings)> {
        (self.surfaces)(state)
    }
}
