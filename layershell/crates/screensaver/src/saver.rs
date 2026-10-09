//! The screensaver on the desktop: a sheet over every output, until the
//! first key, click or movement of the pointer.
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use iced_core::keyboard;
use iced_core::window;
use iced_core::{Element, Event, Length, Point, mouse};
use iced_futures::Subscription;
use iced_futures::backend::native::smol::time;
use iced_futures::event;
use iced_layer::{Anchor, Env, Exclusive, KeyboardInteractivity, Layer, SurfaceSettings};
use iced_runtime::Task;
use iced_widget::{Widget as _, canvas};
use quadrille::Theme;

use crate::display;
use crate::machine::Machine;
use crate::sheet::{Display, Sheet, schedule::Schedule, timeline};
use crate::subjects::{self, Subject};

#[derive(Debug, Clone)]
pub struct Options {
    /// The layer surfaces' namespace, which also names the process to the
    /// desktop's own tools.
    pub namespace: String,
    /// The subject the first output starts with.
    pub first: Option<usize>,
    /// What the subjects' order is drawn from.
    pub seed: u64,
    pub fps: u32,
    /// Input this soon after starting is not taken as the user's.
    pub grace: Duration,
    pub theme: Theme,
    pub date: String,
    /// The computer the sheets of this computer draw.
    pub machine: Machine,
}

pub struct Saver {
    options: Options,
    subjects: Vec<Box<dyn Subject>>,
    displays: HashMap<String, Display>,
    started: Instant,
    now: Instant,
    /// Each output's surface, and the order outputs were first seen in.
    surfaces: RefCell<Vec<(String, window::Id)>>,
    /// Where the pointer was first seen on each surface.
    pointer: HashMap<window::Id, Point>,
    /// Which subject each output shows when.
    schedule: RefCell<Schedule>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick(Instant),
    /// A key or a button: the user is back.
    Input,
    Moved(window::Id, Point),
}

/// How far, in virtual pixels, the pointer may drift before it counts.
const STILL: f32 = 3.0;

impl Saver {
    pub fn new(options: Options) -> (Self, Task<Message>) {
        let now = Instant::now();
        let subjects = subjects::all(&options.machine);
        let schedule = Schedule::new(
            subjects.iter().map(|s| s.card().parts.len()).collect(),
            options.seed,
            options.first,
            timeline::PLOT,
        );
        let saver = Self {
            options,
            subjects,
            schedule: RefCell::new(schedule),
            displays: display::hyprland(),
            started: now,
            now,
            surfaces: RefCell::new(Vec::new()),
            pointer: HashMap::new(),
        };

        (saver, Task::none())
    }

    fn armed(&self) -> bool {
        self.now.duration_since(self.started) >= self.options.grace
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick(now) => {
                self.now = now;
                Task::none()
            }
            Message::Input if self.armed() => iced_runtime::exit(),
            Message::Input => Task::none(),
            Message::Moved(window, at) => {
                let first = *self.pointer.entry(window).or_insert(at);

                if self.armed() && first.distance(at) > STILL {
                    return iced_runtime::exit();
                }

                Task::none()
            }
        }
    }

    pub fn surfaces(&self, env: &Env) -> Vec<(window::Id, SurfaceSettings)> {
        let mut surfaces = self.surfaces.borrow_mut();

        env.outputs
            .iter()
            .map(|output| {
                let id = match surfaces.iter().find(|(name, _)| *name == output.name) {
                    Some((_, id)) => *id,
                    None => {
                        let id = window::Id::unique();
                        surfaces.push((output.name.clone(), id));
                        id
                    }
                };

                (
                    id,
                    SurfaceSettings {
                        namespace: self.options.namespace.clone(),
                        layer: Layer::Overlay,
                        anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                        size: (0, 0),
                        exclusive: Exclusive::Ignore,
                        keyboard: KeyboardInteractivity::Exclusive,
                        output: Some(output.name.clone()),
                        ..SurfaceSettings::default()
                    },
                )
            })
            .collect()
    }

    pub fn view(&self, window: window::Id) -> Element<'_, Message, Theme, iced_renderer::Renderer> {
        let surfaces = self.surfaces.borrow();
        let (index, name) = surfaces
            .iter()
            .enumerate()
            .find(|(_, (_, id))| *id == window)
            .map(|(index, (name, _))| (index, name.clone()))
            .unwrap_or_default();

        let count = self.subjects.len();
        let elapsed = self.now.duration_since(self.started).as_secs_f32();
        let showing = self.schedule.borrow_mut().at(index, elapsed);

        let sheet = Sheet {
            subject: self.subjects[showing.subject].as_ref(),
            number: showing.subject + 1,
            of: count,
            showing,
            display: self.displays.get(&name).copied().unwrap_or(display::GUESS),
            date: &self.options.date,
        };

        canvas(sheet)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let frame = Duration::from_secs_f64(1.0 / f64::from(self.options.fps.max(1)));

        Subscription::batch([
            time::every(frame).map(Message::Tick),
            event::listen_with(|event, _status, window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed { .. })
                | Event::Mouse(mouse::Event::ButtonPressed(_))
                | Event::Mouse(mouse::Event::WheelScrolled { .. }) => Some(Message::Input),
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    Some(Message::Moved(window, position))
                }
                _ => None,
            }),
        ])
    }

    pub fn theme(&self) -> Theme {
        self.options.theme.clone()
    }
}
