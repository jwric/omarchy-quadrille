//! A bar and a popup, drawn by quadrille, shown as layer surfaces.
mod ipc;
mod sys;

use iced_core::keyboard::{self, key::Named};
use iced_core::time::{Duration, Instant};
use iced_core::window;
use iced_core::{Alignment, Backend, Element, Event, Length};
use iced_futures::Subscription;
use iced_futures::backend::native::smol::time;
use iced_futures::event;
use iced_layer::{Anchor, Exclusive, KeyboardInteractivity, Layer, SurfaceSettings};
use iced_runtime::Task;
use iced_widget::{Widget as _, column, container, row, space};

use quadrille::{Theme, px, style, widget};

/// The height of the bar, in virtual pixels: 24 of content and a hairline.
const BAR_HEIGHT: u32 = 25;

/// The popup's size, in virtual pixels.
const POPUP_SIZE: (u32, u32) = (130, 105);

#[derive(Debug, Clone)]
enum Message {
    Tick(Instant),
    Workspace(u8),
    TogglePopup,
    ClosePopup,
    Escape,
    Unfocused(window::Id),
    Theme(usize),
    Note(String),
    Command(String),
}

struct Options {
    output: Option<String>,
    backend: Option<String>,
    tick_ms: u64,
    popup: bool,
    exclusive: bool,
    height: u32,
    exit_after: Option<Duration>,
}

struct Bar {
    options: Options,
    bar: window::Id,
    popup_id: window::Id,
    popup: bool,
    workspace: u8,
    theme: usize,
    note: String,
    time: (u8, u8, u8),
    cpu_times: sys::CpuTimes,
    cpu: u8,
    memory: u8,
    load: String,
    network: bool,
    battery: Option<u8>,
    started: Instant,
}

impl Bar {
    fn new(options: Options) -> (Self, Task<Message>) {
        let popup = options.popup;

        (
            Self {
                options,
                bar: window::Id::unique(),
                popup_id: window::Id::unique(),
                popup,
                workspace: 1,
                theme: 0,
                note: String::new(),
                time: sys::local_time(),
                cpu_times: sys::cpu_times().unwrap_or_default(),
                cpu: 0,
                memory: sys::memory_percent(),
                load: sys::load_average(),
                network: sys::network_up(),
                battery: sys::battery_percent(),
                started: Instant::now(),
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick(_) => {
                self.time = sys::local_time();

                if let Some(now) = sys::cpu_times() {
                    self.cpu = sys::cpu_percent(self.cpu_times, now);
                    self.cpu_times = now;
                }

                self.memory = sys::memory_percent();
                self.network = sys::network_up();
                self.battery = sys::battery_percent();

                if self.popup {
                    self.load = sys::load_average();
                }

                if self
                    .options
                    .exit_after
                    .is_some_and(|after| self.started.elapsed() >= after)
                {
                    return iced_runtime::exit();
                }
            }
            Message::Workspace(workspace) => self.workspace = workspace,
            Message::TogglePopup => self.popup = !self.popup,
            Message::ClosePopup | Message::Escape => self.popup = false,
            Message::Unfocused(window) => {
                if window == self.popup_id {
                    self.popup = false;
                }
            }
            Message::Theme(theme) => self.theme = theme,
            Message::Note(note) => self.note = note,
            Message::Command(command) => {
                log::info!("command: {command}");

                match command.split_whitespace().next() {
                    Some("toggle-popup") => self.popup = !self.popup,
                    Some("open-popup") => self.popup = true,
                    Some("close-popup") => self.popup = false,
                    Some("quit") => return iced_runtime::exit(),
                    _ => {}
                }
            }
        }

        Task::none()
    }

    fn theme(&self) -> Theme {
        Theme::ALL[self.theme].clone()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            event::listen_with(|event, _status, window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(Named::Escape),
                    ..
                }) => Some(Message::Escape),
                Event::Window(window::Event::Unfocused) => Some(Message::Unfocused(window)),
                _ => None,
            }),
            Subscription::run(ipc::commands).map(Message::Command),
        ];

        if self.options.tick_ms > 0 {
            subscriptions.push(
                time::every(Duration::from_millis(self.options.tick_ms)).map(Message::Tick),
            );
        }

        Subscription::batch(subscriptions)
    }

    fn surfaces(&self) -> Vec<(window::Id, SurfaceSettings)> {
        let mut surfaces = vec![(
            self.bar,
            SurfaceSettings {
                namespace: "quadrille-bar".into(),
                layer: Layer::Top,
                anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
                size: (0, self.options.height),
                exclusive: if self.options.exclusive {
                    Exclusive::Own
                } else {
                    Exclusive::None
                },
                keyboard: KeyboardInteractivity::None,
                output: self.options.output.clone(),
                ..SurfaceSettings::default()
            },
        )];

        if self.popup {
            surfaces.push((
                self.popup_id,
                SurfaceSettings {
                    namespace: "quadrille-popup".into(),
                    layer: Layer::Overlay,
                    anchor: Anchor::TOP | Anchor::RIGHT,
                    size: POPUP_SIZE,
                    exclusive: Exclusive::Ignore,
                    // Under the bar, a little in from the edge.
                    margin: [self.options.height as i32, 4, 0, 0],
                    keyboard: KeyboardInteractivity::OnDemand,
                    output: self.options.output.clone(),
                },
            ));
        }

        surfaces
    }

    fn view(&self, window: window::Id) -> Element<'_, Message, Theme, iced_renderer::Renderer> {
        if window == self.popup_id {
            self.popup_view()
        } else {
            self.bar_view()
        }
    }

    fn bar_view(&self) -> quadrille::Element<'_, Message> {
        let brand = widget::inverse(widget::label(" QUADRILLE "));

        let workspaces = row((1..=5u8).map(|workspace| {
            widget::tab(workspace.to_string(), self.workspace == workspace)
                .on_press(Message::Workspace(workspace))
                .boxed()
        }))
        .spacing(px::HAIR);

        let (hours, minutes, seconds) = self.time;

        let clock = widget::label(format!("{hours:02}:{minutes:02}:{seconds:02}"));

        let gauge = |name: &'static str, value: u8| -> quadrille::Element<'_, Message> {
            row![
                widget::label(name).style(style::text::muted),
                widget::bar(0.0..=100.0, f32::from(value))
                    .width(Length::Fixed(24.0))
                    .height(5),
                widget::label(format!("{value:>3}")),
            ]
            .spacing(px::GAP)
            .align_y(Alignment::Center)
            .boxed()
        };

        let mut readings = row![gauge("CPU", self.cpu), gauge("MEM", self.memory)]
            .spacing(px::WIDE)
            .align_y(Alignment::Center);

        if let Some(battery) = self.battery {
            readings = readings.push(gauge("BAT", battery));
        }

        let lamps = row![
            widget::indicator("NET", self.network),
            widget::indicator("MIC", false),
        ]
        .spacing(px::WIDE)
        .align_y(Alignment::Center);

        let sys = widget::tab("SYS", self.popup).on_press(Message::TogglePopup);

        let content = row![
            brand,
            workspaces,
            space::horizontal(),
            clock,
            space::horizontal(),
            readings,
            lamps,
            sys,
        ]
        .spacing(px::WIDE)
        .padding([0.0, px::GAP])
        .align_y(Alignment::Center)
        .height(Length::Fill);

        column![widget::panel(content), widget::divider()].boxed()
    }

    fn popup_view(&self) -> quadrille::Element<'_, Message> {
        let system = widget::group(
            "SYSTEM",
            column![
                widget::reading("CPU", format!("{}%", self.cpu)),
                widget::bar(0.0..=100.0, f32::from(self.cpu)).height(5),
                widget::reading("MEM", format!("{}%", self.memory)),
                widget::bar(0.0..=100.0, f32::from(self.memory)).height(5),
                widget::reading("LOAD", self.load.clone()),
            ]
            .spacing(px::GAP),
        );

        let themes = widget::group(
            "THEME",
            widget::segmented(
                Theme::ALL
                    .iter()
                    .enumerate()
                    .map(|(index, theme)| (index, &theme.name()[..1])),
                Some(self.theme),
                Message::Theme,
            ),
        );

        let note = widget::group(
            "NOTE",
            widget::text_input("TYPE HERE", &self.note).on_input(Message::Note),
        );

        let footer = row![
            widget::label("ESC CLOSES").style(style::text::faint),
            space::horizontal(),
            widget::button("CLOSE").on_press(Message::ClosePopup),
        ]
        .align_y(Alignment::Center);

        container(
            column![system, themes, note, footer]
                .spacing(px::WIDE)
                .padding(px::WIDE),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme: &Theme| {
            iced_widget::container::Style::default()
                .background(theme.palette().ground)
                .border(style::hairline(theme.palette().edge))
        })
        .boxed()
    }
}

fn parse() -> Result<Options, String> {
    let mut options = Options {
        output: None,
        backend: None,
        tick_ms: 1000,
        popup: false,
        exclusive: true,
        height: BAR_HEIGHT,
        exit_after: None,
    };

    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        let mut value = |name: &str| args.next().ok_or(format!("{name} needs a value"));

        match arg.as_str() {
            "--output" => options.output = Some(value("--output")?),
            "--backend" => options.backend = Some(value("--backend")?),
            "--tick-ms" => {
                options.tick_ms = value("--tick-ms")?
                    .parse()
                    .map_err(|_| "--tick-ms needs a number")?
            }
            "--exit-after" => {
                options.exit_after = Some(Duration::from_secs_f64(
                    value("--exit-after")?
                        .parse()
                        .map_err(|_| "--exit-after needs seconds")?,
                ))
            }
            "--popup" => options.popup = true,
            "--no-exclusive" => options.exclusive = false,
            "--height" => {
                options.height = value("--height")?
                    .parse()
                    .map_err(|_| "--height needs a number")?
            }
            other => return Err(format!("unknown option {other}")),
        }
    }

    Ok(options)
}

fn main() {
    env_logger::init();

    let mut args = std::env::args().skip(1);

    if args.next().as_deref() == Some("ctl") {
        let command: Vec<String> = args.collect();

        if let Err(error) = ipc::send(&command.join(" ")) {
            eprintln!("quadrille-bar is not listening: {error}");
            std::process::exit(1);
        }

        return;
    }

    let options = match parse() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("quadrille-bar: {error}");
            eprintln!(
                "usage: quadrille-bar [--output NAME] [--backend wgpu|tiny-skia] \
                 [--tick-ms MS] [--popup] [--no-exclusive] [--height VPX] [--exit-after SECS] | ctl COMMAND"
            );
            std::process::exit(2);
        }
    };

    if ipc::is_running() {
        eprintln!("quadrille-bar is already running");
        std::process::exit(1);
    }

    let mut settings = quadrille::settings();

    if let Some(backend) = &options.backend {
        settings.backend = Backend::Custom(backend.clone());
    }

    let options = std::cell::RefCell::new(Some(options));

    let result = iced_layer::application(
        move || Bar::new(options.borrow_mut().take().expect("Boot once")),
        Bar::update,
        |bar: &Bar, window| bar.view(window),
        Bar::surfaces,
    )
    .settings(settings)
    .subscription(Bar::subscription)
    .theme(|bar: &Bar, _| bar.theme())
    .run();

    let _ = std::fs::remove_file(ipc::socket_path());

    if let Err(error) = result {
        eprintln!("quadrille-bar: {error}");
        std::process::exit(1);
    }
}
