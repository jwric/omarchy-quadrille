//! The panel host: a bar on every output, and panels that are summoned.
use crate::commands::{self, Shared};
use crate::ipc::{self, Request};
use crate::panels::{Key, audio, bluetooth, network, power};
use crate::sys::{self, LocalTime};
use crate::sysmon;
use crate::theme;

use iced_core::keyboard::{self, key::Named};
use iced_core::time::Duration;
use iced_core::window;
use iced_core::{Alignment, Element, Event, Length};
use iced_futures::Subscription;
use iced_futures::backend::native::smol::time;
use iced_futures::event;
use iced_futures::futures::StreamExt;
use iced_layer::{Anchor, Env, Exclusive, Grab, KeyboardInteractivity, Layer, SurfaceSettings};
use iced_runtime::Task;
use iced_widget::{Widget as _, column, container, mouse_area, row, space};

use quadrille::{Theme, px, style, widget};

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

/// The height of the bar, in virtual pixels: 24 of content and a hairline.
pub const BAR_HEIGHT: u32 = 25;

/// A panel that can be summoned.
pub struct PanelDef {
    pub id: &'static str,
    pub title: &'static str,
    /// In virtual pixels. At 1.6667 a size is exact when it is a multiple of 5.
    pub size: (u32, u32),
    /// How often the panel reads the machine while it is shown, in
    /// milliseconds. Hidden, it never does.
    pub refresh_ms: u64,
}

pub const PANELS: &[PanelDef] = &[
    PanelDef {
        id: "sysmon",
        title: "SYSMON",
        size: (160, 305),
        refresh_ms: 1000,
    },
    PanelDef {
        id: "audio",
        title: "AUDIO",
        size: (170, 250),
        refresh_ms: 1000,
    },
    PanelDef {
        id: "network",
        title: "NETWORK",
        size: (170, 300),
        refresh_ms: 4000,
    },
    PanelDef {
        id: "bluetooth",
        title: "BLUETOOTH",
        size: (170, 200),
        refresh_ms: 3000,
    },
    PanelDef {
        id: "power",
        title: "POWER",
        size: (170, 245),
        refresh_ms: 2000,
    },
    PanelDef {
        id: "demo",
        title: "DEMO",
        size: (140, 90),
        refresh_ms: 0,
    },
];

fn panel(id: &str) -> Option<&'static PanelDef> {
    PANELS.iter().find(|panel| panel.id == id)
}

#[derive(Debug, Clone)]
pub enum Message {
    /// The bars read the machine.
    Tick,
    /// The minute changed.
    Minute,
    /// The system monitor reads the machine, while it is shown.
    Sample,
    /// A panel reads the machine: it is shown, and this is its beat.
    Refresh(&'static str),
    Audio(audio::Message),
    Network(network::Message),
    Bluetooth(bluetooth::Message),
    Power(power::Message),
    /// A key the keyboard-driven panels answer to.
    Key(Key),
    /// Where the text a `ctl find` looked for is, in the panel's own pixels.
    Found(Request, Option<[f32; 4]>),
    /// The pointer is over the bar of this output.
    Hover(String),
    /// A panel's button on the bar of this output.
    Toggle(&'static str, String),
    Hide,
    Escape,
    /// The compositor asked a window to close: a click outside its grab.
    CloseRequested(window::Id),
    Note(String),
    ThemeChanged,
    Request(Request),
    Quit,
}

#[derive(Debug, Clone)]
pub struct Options {
    /// The outputs that get a bar (and panels); every one does when empty.
    pub outputs: Vec<String>,
    /// No bar at all: no surface, no exclusive zone, no timer, until a panel is
    /// summoned. The host is a service for the panels, beside another bar.
    pub no_bar: bool,
    pub backend: Option<String>,
    /// How often the bar's gauges are read, in milliseconds; 0 never.
    pub bar_tick_ms: u64,
    pub exclusive: bool,
    pub height: u32,
    /// Take the keyboard exclusively for a panel, grab or no grab.
    pub exclusive_keyboard: bool,
    /// No keyboard and no focus grab for panels: they only show. For
    /// screenshots, on a desktop that is in use.
    pub passive: bool,
    pub theme_dir: PathBuf,
    /// A panel to show at the start.
    pub open: Option<String>,
    pub exit_after: Option<Duration>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            outputs: Vec::new(),
            no_bar: false,
            backend: None,
            bar_tick_ms: 2000,
            exclusive: true,
            height: BAR_HEIGHT,
            exclusive_keyboard: false,
            passive: false,
            theme_dir: theme::default_dir(),
            open: None,
            exit_after: None,
        }
    }
}

/// What a window is for.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Role {
    Bar(String),
    Panel(&'static str),
}

struct Open {
    panel: &'static str,
    /// Where it was asked to appear; the output of the last bar the pointer
    /// was over, or the first, when `None`.
    output: Option<String>,
}

#[derive(Default)]
struct BarData {
    time: LocalTime,
    cpu_times: sys::CpuTimes,
    cpu: u8,
    memory: u8,
    network: bool,
    battery: Option<u8>,
}

pub struct Host {
    options: Options,
    ids: RefCell<HashMap<String, window::Id>>,
    roles: RefCell<HashMap<window::Id, Role>>,
    env: RefCell<Env>,
    open: Option<Open>,
    pointer_output: Option<String>,
    theme: Theme,
    bar: BarData,
    sysmon: sysmon::State,
    audio: audio::State,
    network: network::State,
    bluetooth: bluetooth::State,
    power: power::State,
    note: String,
    runner: Shared,
    /// The panel that was just shown and has to read the machine.
    kick: Option<&'static str>,
}

impl Host {
    pub fn new(options: Options) -> (Self, Task<Message>) {
        Self::with_runner(options, commands::from_env())
    }

    /// A host that reads and changes the machine through `runner`.
    pub fn with_runner(options: Options, runner: Shared) -> (Self, Task<Message>) {
        let theme = theme::current(&options.theme_dir);
        let open = options.open.as_deref().and_then(panel).map(|def| Open {
            panel: def.id,
            output: options.outputs.first().cloned(),
        });

        let mut host = Self {
            ids: RefCell::default(),
            roles: RefCell::default(),
            env: RefCell::default(),
            open: None,
            pointer_output: None,
            theme,
            bar: BarData {
                time: LocalTime::now(),
                cpu_times: sys::cpu_times().unwrap_or_default(),
                memory: sys::memory_percent(),
                network: sys::network_up(),
                battery: sys::battery_percent(),
                ..BarData::default()
            },
            sysmon: sysmon::State::default(),
            audio: audio::State::default(),
            network: network::State::default(),
            bluetooth: bluetooth::State::default(),
            power: power::State::default(),
            note: String::new(),
            runner,
            kick: None,
            options,
        };

        if let Some(open) = open {
            host.show(open.panel, open.output);
        }

        let task = host.kicked();

        (host, task)
    }

    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    /// Shows a panel on an output, taking the place of the one that is up.
    fn show(&mut self, id: &'static str, output: Option<String>) {
        if self.open.as_ref().map(|open| open.panel) != Some(id) {
            self.forget();

            match id {
                "sysmon" => {
                    // A baseline to take the first rates from.
                    self.sysmon.reset();
                    self.sysmon.sample();
                }
                _ => self.kick = Some(id),
            }
        }

        self.open = Some(Open { panel: id, output });
    }

    fn hide(&mut self) -> bool {
        self.forget();

        self.open.take().is_some()
    }

    /// Lets go of what the panel that was up had read: nothing of a hidden
    /// panel is kept, so a shown one starts from what the machine says now.
    fn forget(&mut self) {
        self.audio.reset();
        self.network.reset();
        self.bluetooth.reset();
        self.power.reset();
        self.kick = None;
    }

    /// The task that makes a panel that has just been shown read the machine.
    fn kicked(&mut self) -> Task<Message> {
        match self.kick.take() {
            Some(id) => self.refresh(id),
            None => Task::none(),
        }
    }

    fn refresh(&mut self, id: &str) -> Task<Message> {
        match id {
            "audio" => self.audio.refresh(&self.runner).map(Message::Audio),
            "network" => self.network.refresh(&self.runner).map(Message::Network),
            "bluetooth" => self.bluetooth.refresh(&self.runner).map(Message::Bluetooth),
            "power" => self.power.refresh(&self.runner).map(Message::Power),
            _ => Task::none(),
        }
    }

    fn open_panel(&self) -> Option<&'static str> {
        self.open.as_ref().map(|open| open.panel)
    }

    /// Backs out of what the panel that is up is in the middle of, if it is in
    /// the middle of something: a password, a question.
    fn escape_panel(&mut self) -> bool {
        match self.open_panel() {
            Some("network") => self.network.escape(),
            Some("power") => self.power.escape(),
            Some("audio") => self.audio.escape(),
            Some("bluetooth") => self.bluetooth.escape(),
            _ => false,
        }
    }

    fn key(&mut self, key: Key) -> Task<Message> {
        let runner = self.runner.clone();

        match self.open_panel() {
            Some("audio") => self.audio.key(key, &runner).map(Message::Audio),
            Some("network") => self.network.key(key, &runner).map(Message::Network),
            Some("bluetooth") => self.bluetooth.key(key, &runner).map(Message::Bluetooth),
            Some("power") => self.power.key(key, &runner).map(Message::Power),
            _ => Task::none(),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.bar.time = LocalTime::now();

                if let Some(now) = sys::cpu_times() {
                    self.bar.cpu = sys::cpu_percent(self.bar.cpu_times, now);
                    self.bar.cpu_times = now;
                }

                self.bar.memory = sys::memory_percent();
                self.bar.network = sys::network_up();
                self.bar.battery = sys::battery_percent();
            }
            Message::Minute => self.bar.time = LocalTime::now(),
            Message::Sample => self.sysmon.sample(),
            Message::Refresh(id) => {
                // A beat that comes after the panel is gone is nothing.
                if self.open_panel() == Some(id) {
                    return self.refresh(id);
                }
            }
            Message::Audio(message) => {
                let runner = self.runner.clone();

                return self.audio.update(message, &runner).map(Message::Audio);
            }
            Message::Network(message) => {
                let runner = self.runner.clone();

                return self.network.update(message, &runner).map(Message::Network);
            }
            Message::Bluetooth(message) => {
                let runner = self.runner.clone();

                return self
                    .bluetooth
                    .update(message, &runner)
                    .map(Message::Bluetooth);
            }
            Message::Power(message) => {
                let runner = self.runner.clone();
                let task = self.power.update(message, &runner).map(Message::Power);

                // A session action has gone out: nothing more to show.
                if self.power.take_close() {
                    let _ = self.hide();
                }

                return task;
            }
            Message::Key(key) => return self.key(key),
            Message::Found(request, bounds) => {
                request.respond(match bounds {
                    Some([x, y, width, height]) => {
                        format!("{x:.0} {y:.0} {width:.0} {height:.0}\n")
                    }
                    None => "error: not on screen\n".to_owned(),
                });
            }
            Message::Hover(output) => self.pointer_output = Some(output),
            Message::Toggle(id, output) => {
                if self.open.as_ref().map(|open| open.panel) == Some(id) {
                    let _ = self.hide();
                } else if let Some(def) = panel(id) {
                    self.show(def.id, Some(output));
                }
            }
            Message::Hide => {
                let _ = self.hide();
            }
            Message::Escape => {
                if !self.escape_panel() {
                    let _ = self.hide();
                }
            }
            Message::CloseRequested(window) => {
                let is_panel = matches!(self.roles.borrow().get(&window), Some(Role::Panel(_)));

                if is_panel {
                    let _ = self.hide();
                }
            }
            Message::Note(note) => self.note = note,
            Message::ThemeChanged => {
                let theme = theme::current(&self.options.theme_dir);

                log::info!("theme: {}", theme.name());

                self.theme = theme;
            }
            Message::Request(request) => {
                // Where some text is on screen, for the tests and for anyone
                // who wants to point at it: `find TEXT` answers `x y w h` in the
                // virtual pixels of the window it is in.
                if let Some(text) = request.line.strip_prefix("find ") {
                    let text = text.trim().to_owned();

                    // Text is matched as it reads: a reading padded with spaces
                    // to a width is still its number.
                    let wanted = text.clone();

                    return iced_runtime::widget::selector::find(
                        move |candidate: iced_runtime::widget::selector::Candidate<'_>| {
                            match candidate {
                                iced_runtime::widget::selector::Candidate::Text {
                                    content,
                                    visible_bounds,
                                    ..
                                } if content.trim() == wanted => Some(visible_bounds),
                                _ => None,
                            }
                        },
                    )
                    .map(move |found| {
                        Message::Found(
                            request.clone(),
                            found
                                .flatten()
                                .map(|bounds| [bounds.x, bounds.y, bounds.width, bounds.height]),
                        )
                    });
                }

                let (answer, quit) = self.command(&request.line);

                request.respond(answer);

                if quit {
                    return iced_runtime::exit();
                }
            }
            Message::Quit => return iced_runtime::exit(),
        }

        self.kicked()
    }

    /// Runs a command of the control socket, and gives back the answer and
    /// whether to quit.
    fn command(&mut self, line: &str) -> (String, bool) {
        log::info!("command: {line}");

        let mut words = line.splitn(3, char::is_whitespace);
        let verb = words.next().unwrap_or_default();
        let id = words.next().unwrap_or_default().trim();
        let arguments = words.next().unwrap_or_default().trim();

        let known = || {
            PANELS
                .iter()
                .map(|panel| panel.id)
                .collect::<Vec<_>>()
                .join(", ")
        };

        let output = |arguments: &str| -> Result<Option<String>, String> {
            if arguments.is_empty() {
                return Ok(None);
            }

            let value: serde_json::Value = serde_json::from_str(arguments)
                .map_err(|error| format!("error: arguments are not JSON: {error}\n"))?;

            let output = value.get("output").and_then(|output| output.as_str());

            if let Some(output) = output {
                let outputs = self.env.borrow().outputs.clone();

                if !outputs.iter().any(|candidate| candidate.name == output) {
                    return Err(format!(
                        "error: no output {output} (have: {})\n",
                        outputs
                            .iter()
                            .map(|output| output.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
            }

            Ok(output.map(str::to_owned))
        };

        let answer = match verb {
            "summon" | "toggle" => match panel(id) {
                None => Err(format!("error: no panel {id:?} (have: {})\n", known())),
                Some(def) => output(arguments).map(|output| {
                    let visible = self.open.as_ref().map(|open| open.panel) == Some(def.id);

                    if verb == "toggle" && visible {
                        let _ = self.hide();

                        format!("{} hidden\n", def.id)
                    } else {
                        let output = output.or_else(|| self.pointer_output.clone());

                        self.show(def.id, output);

                        format!("{} shown\n", def.id)
                    }
                }),
            },
            "hide" => {
                if id.is_empty()
                    || id == "all"
                    || self.open.as_ref().map(|open| open.panel) == Some(id)
                {
                    let was = self.hide();

                    Ok(if was {
                        "hidden\n"
                    } else {
                        "nothing was shown\n"
                    }
                    .to_owned())
                } else if panel(id).is_some() {
                    Ok(format!("{id} was not shown\n"))
                } else {
                    Err(format!("error: no panel {id:?} (have: {})\n", known()))
                }
            }
            "list" => Ok(self.list()),
            "reload-theme" => {
                self.theme = theme::current(&self.options.theme_dir);

                Ok(format!("theme {}\n", self.theme.name()))
            }
            "quit" => return ("bye\n".to_owned(), true),
            "" => Err("error: empty command\n".to_owned()),
            other => Err(format!(
                "error: unknown command {other:?} \
                 (summon, toggle, hide, list, reload-theme, quit)\n"
            )),
        };

        (answer.unwrap_or_else(|error| error), false)
    }

    fn list(&self) -> String {
        let mut text = String::new();

        for def in PANELS {
            let state = match &self.open {
                Some(open) if open.panel == def.id => {
                    format!("visible on {}", self.placed(open))
                }
                _ => "hidden".to_owned(),
            };

            text.push_str(&format!(
                "panel  {:<8} {:<20} {}x{} vpx\n",
                def.id, state, def.size.0, def.size.1
            ));
        }

        for output in &self.env.borrow().outputs {
            text.push_str(&format!(
                "output {:<10} scale {:.4}  {}x{} logical\n",
                output.name, output.scale, output.logical_size.0, output.logical_size.1
            ));
        }

        text.push_str(&format!("theme  {}\n", self.theme.name()));
        text
    }

    /// The output an open panel is asked to be on: where it was asked for if
    /// that output is there, else the first output that has a bar. With no bar
    /// there is no first output to prefer, and the answer is `None`: the
    /// compositor puts the panel on the output that has the focus.
    fn target(&self, open: &Open) -> Option<String> {
        let env = self.env.borrow();

        let outputs: Vec<_> = env
            .outputs
            .iter()
            .filter(|output| self.wants(&output.name))
            .collect();

        let asked = open
            .output
            .as_ref()
            .filter(|name| outputs.iter().any(|output| &output.name == *name))
            .cloned();

        if self.options.no_bar {
            asked
        } else {
            asked.or_else(|| outputs.first().map(|output| output.name.clone()))
        }
    }

    /// Where an open panel is, for `list`: the output it was put on, or the
    /// one the compositor chose for it.
    fn placed(&self, open: &Open) -> String {
        if let Some(output) = self.target(open) {
            return output;
        }

        let id = self
            .ids
            .borrow()
            .get(&Self::panel_key(open.panel, None))
            .copied();

        id.and_then(|id| self.env.borrow().placements.get(&id).cloned())
            .unwrap_or_else(|| "the focused output".to_owned())
    }

    fn panel_key(panel: &str, output: Option<&str>) -> String {
        format!("panel:{panel}:{}", output.unwrap_or("*"))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            event::listen_with(|event, _status, window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(Named::Escape),
                    ..
                }) => Some(Message::Escape),
                Event::Window(window::Event::CloseRequested) => {
                    Some(Message::CloseRequested(window))
                }
                _ => None,
            }),
            Subscription::run(|| ipc::requests().map(Message::Request)),
        ];

        // The bar's clock and gauges, which a host with no bar does not have.
        if !self.options.no_bar {
            subscriptions.push(Subscription::run(minutes));
        }

        let theme_dir = self.options.theme_dir.clone();

        subscriptions.push(Subscription::run_with(theme_dir, |dir| {
            theme::changes(dir.clone()).map(|()| Message::ThemeChanged)
        }));

        if self.options.bar_tick_ms > 0 && !self.options.no_bar {
            subscriptions.push(
                time::every(Duration::from_millis(self.options.bar_tick_ms)).map(|_| Message::Tick),
            );
        }

        // The monitor is read only while it is shown, at a fixed pace.
        if self
            .open
            .as_ref()
            .is_some_and(|open| open.panel == "sysmon")
        {
            subscriptions.push(Subscription::run(sysmon_ticks).map(|()| Message::Sample));
        }

        // The panels that read the machine do it on a fixed beat while they
        // are shown, and answer the keyboard only while they are.
        if let Some(def) = self.open_panel().and_then(panel) {
            subscriptions.push(event::listen_with(|event, status, _window| match event {
                Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. })
                    if status == iced_core::event::Status::Ignored =>
                {
                    Key::of(&key, modifiers).map(Message::Key)
                }
                _ => None,
            }));

            if def.refresh_ms > 0 && def.id != "sysmon" {
                subscriptions.push(Subscription::run_with(
                    (def.id, def.refresh_ms),
                    |(id, ms)| beat(id, *ms),
                ));
            }
        }

        if let Some(after) = self.options.exit_after {
            subscriptions.push(time::every(after).map(|_| Message::Quit));
        }

        Subscription::batch(subscriptions)
    }

    /// Whether the host puts a bar on an output.
    fn wants(&self, name: &str) -> bool {
        self.options.outputs.is_empty() || self.options.outputs.iter().any(|only| only == name)
    }

    fn id(&self, key: String, role: Role) -> window::Id {
        let id = *self
            .ids
            .borrow_mut()
            .entry(key)
            .or_insert_with(window::Id::unique);

        let _ = self.roles.borrow_mut().insert(id, role);

        id
    }

    pub fn surfaces(&self, env: &Env) -> Vec<(window::Id, SurfaceSettings)> {
        *self.env.borrow_mut() = env.clone();

        let grab = env.focus_grab && !self.options.exclusive_keyboard && !self.options.passive;

        let outputs: Vec<_> = env
            .outputs
            .iter()
            .filter(|output| self.wants(&output.name))
            .collect();

        let bars: &[_] = if self.options.no_bar { &[] } else { &outputs };

        let mut surfaces: Vec<_> = bars
            .iter()
            .map(|output| {
                (
                    self.id(
                        format!("bar:{}", output.name),
                        Role::Bar(output.name.clone()),
                    ),
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
                        output: Some(output.name.clone()),
                        // Not a member of the grab: Hyprland hands the keyboard to
                        // an arbitrary member of one, and it should be the popup's.
                        // A click on the bar then clears the grab, which is what a
                        // button that toggles the popup needs.
                        grab: Grab::None,
                        ..SurfaceSettings::default()
                    },
                )
            })
            .collect();

        if let Some(open) = &self.open {
            let def = panel(open.panel).expect("An open panel is a known one");
            let output = self.target(open);

            // With a bar on every output there is somewhere to put it; with
            // none, the compositor decides (an output of `None`).
            if output.is_some() || self.options.no_bar {
                surfaces.push((
                    self.id(
                        Self::panel_key(def.id, output.as_deref()),
                        Role::Panel(def.id),
                    ),
                    SurfaceSettings {
                        namespace: format!("quadrille-{}", def.id),
                        layer: Layer::Overlay,
                        anchor: Anchor::TOP | Anchor::RIGHT,
                        size: def.size,
                        // Below whatever has claimed the top of the output.
                        exclusive: Exclusive::None,
                        margin: [px::GAP as i32, px::GAP as i32, 0, 0],
                        keyboard: if self.options.passive {
                            KeyboardInteractivity::None
                        } else if grab {
                            KeyboardInteractivity::OnDemand
                        } else {
                            KeyboardInteractivity::Exclusive
                        },
                        output,
                        grab: if grab { Grab::Popup } else { Grab::None },
                    },
                ));
            }
        }

        surfaces
    }

    pub fn view(&self, window: window::Id) -> Element<'_, Message, Theme, iced_renderer::Renderer> {
        // The first surface is the first time text is measured.
        crate::graphics::fall_back_to_departure();

        let role = self.roles.borrow().get(&window).cloned();

        match role {
            Some(Role::Bar(output)) => self.bar_view(output),
            Some(Role::Panel(id)) => self.panel_view(id),
            None => space::horizontal().boxed(),
        }
    }

    fn bar_view(&self, output: String) -> quadrille::Element<'_, Message> {
        let brand = widget::inverse(widget::label(" QUADRILLE "));

        let time = self.bar.time;

        let clock = row![
            widget::label(format!("{:02}:{:02}", time.hour, time.minute)),
            widget::label(time.date()).style(style::text::muted),
        ]
        .spacing(px::WIDE);

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

        let mut readings = row![gauge("CPU", self.bar.cpu), gauge("MEM", self.bar.memory)]
            .spacing(px::WIDE)
            .align_y(Alignment::Center);

        if let Some(battery) = self.bar.battery {
            readings = readings.push(gauge("BAT", battery));
        }

        let lamps = widget::indicator("NET", self.bar.network);

        let sysmon_open = self
            .open
            .as_ref()
            .is_some_and(|open| open.panel == "sysmon");

        let sys =
            widget::tab("SYS", sysmon_open).on_press(Message::Toggle("sysmon", output.clone()));

        let content = row![
            brand,
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

        mouse_area(column![widget::panel(content), widget::divider()])
            .on_enter(Message::Hover(output))
            .boxed()
    }

    fn panel_view(&self, id: &'static str) -> quadrille::Element<'_, Message> {
        let def = panel(id).expect("A panel with a window is a known one");

        let header = row![
            widget::inverse(widget::label(format!(" {} ", def.title))),
            space::horizontal(),
            widget::button("X").on_press(Message::Hide),
        ]
        .align_y(Alignment::Center);

        let body: quadrille::Element<'_, Message> = match id {
            "sysmon" => sysmon::view(&self.sysmon),
            "audio" => self.audio.view().map(Message::Audio).boxed(),
            "network" => self.network.view().map(Message::Network).boxed(),
            "bluetooth" => self.bluetooth.view().map(Message::Bluetooth).boxed(),
            "power" => self.power.view().map(Message::Power).boxed(),
            _ => column![
                widget::group(
                    "NOTE",
                    widget::text_input("TYPE HERE", &self.note).on_input(Message::Note),
                ),
                widget::label("ESC CLOSES").style(style::text::faint),
            ]
            .spacing(px::WIDE)
            .boxed(),
        };

        // The lists that can be longer than the panel scroll.
        let body = match id {
            "audio" | "network" | "bluetooth" => widget::scroll(body)
                .width(Length::Fill)
                .height(Length::Fill)
                .boxed(),
            _ => body,
        };

        container(column![header, body].spacing(px::WIDE).padding(px::WIDE))
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

/// Once a minute, on the minute.
fn minutes() -> impl iced_futures::futures::Stream<Item = Message> {
    iced_futures::futures::stream::unfold((), |()| async {
        let wait = LocalTime::now().until_next_minute() + Duration::from_millis(30);

        smol::Timer::after(wait).await;

        Some((Message::Minute, ()))
    })
}

/// The beat of a panel that reads the machine while it is shown: every `ms`,
/// the first one after one.
fn beat(id: &'static str, ms: u64) -> impl iced_futures::futures::Stream<Item = Message> {
    smol::Timer::interval(Duration::from_millis(ms)).map(move |_| Message::Refresh(id))
}

/// The pace of the system monitor: a first reading soon after it is shown, so
/// that it has rates to show, then one a second.
fn sysmon_ticks() -> impl iced_futures::futures::Stream<Item = ()> {
    let start = std::time::Instant::now() + Duration::from_millis(250);

    smol::Timer::interval_at(start, Duration::from_secs(1)).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_layer::OutputInfo;

    fn host(options: Options) -> Host {
        Host::new(Options {
            theme_dir: std::env::temp_dir().join("quadrille-bar-no-theme"),
            ..options
        })
        .0
    }

    fn env(focus_grab: bool) -> Env {
        let output = |name: &str, scale: f64| OutputInfo {
            name: name.to_owned(),
            scale,
            logical_size: (1536, 960),
        };

        Env {
            outputs: vec![output("A", 1.6667), output("B", 1.0)],
            focus_grab,
            placements: Default::default(),
        }
    }

    fn panels(host: &Host, env: &Env) -> Vec<SurfaceSettings> {
        host.surfaces(env)
            .into_iter()
            .map(|(_, settings)| settings)
            .filter(|settings| settings.namespace != "quadrille-bar")
            .collect()
    }

    #[test]
    fn there_is_a_bar_on_every_output_and_no_panel() {
        let host = host(Options::default());
        let surfaces = host.surfaces(&env(true));

        assert_eq!(surfaces.len(), 2);
        assert!(
            surfaces
                .iter()
                .all(|(_, settings)| settings.namespace == "quadrille-bar")
        );

        let outputs: Vec<_> = surfaces
            .iter()
            .map(|(_, settings)| settings.output.clone().unwrap())
            .collect();

        assert_eq!(outputs, ["A", "B"]);
        assert_eq!(
            surfaces
                .iter()
                .map(|(id, _)| id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            2,
            "every bar is a window of its own"
        );
    }

    #[test]
    fn the_same_output_keeps_the_same_window() {
        let host = host(Options::default());

        let first = host.surfaces(&env(true));
        let second = host.surfaces(&env(true));

        assert_eq!(first[0].0, second[0].0);
        assert_eq!(first[1].0, second[1].0);
    }

    #[test]
    fn outputs_can_be_limited() {
        let host = host(Options {
            outputs: vec!["B".into()],
            ..Options::default()
        });

        let surfaces = host.surfaces(&env(true));

        assert_eq!(surfaces.len(), 1);
        assert_eq!(surfaces[0].1.output.as_deref(), Some("B"));
    }

    #[test]
    fn a_summoned_panel_is_a_popup_under_a_focus_grab() {
        let mut host = host(Options::default());
        host.surfaces(&env(true));

        let (answer, quit) = host.command("summon sysmon {\"output\":\"B\"}");

        assert_eq!(answer, "sysmon shown\n");
        assert!(!quit);

        let panels = panels(&host, &env(true));

        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].namespace, "quadrille-sysmon");
        assert_eq!(panels[0].output.as_deref(), Some("B"));
        assert_eq!(panels[0].grab, Grab::Popup);
        assert_eq!(panels[0].keyboard, KeyboardInteractivity::OnDemand);
        assert_eq!(panels[0].exclusive, Exclusive::None);
    }

    #[test]
    fn without_a_focus_grab_a_panel_takes_the_keyboard() {
        let mut host = host(Options::default());
        host.surfaces(&env(false));
        let _ = host.command("summon sysmon");

        let panels = panels(&host, &env(false));

        assert_eq!(panels[0].grab, Grab::None);
        assert_eq!(panels[0].keyboard, KeyboardInteractivity::Exclusive);
    }

    #[test]
    fn a_passive_panel_takes_nothing() {
        let mut host = host(Options {
            passive: true,
            ..Options::default()
        });
        host.surfaces(&env(true));
        let _ = host.command("summon sysmon");

        let panels = panels(&host, &env(true));

        assert_eq!(panels[0].grab, Grab::None);
        assert_eq!(panels[0].keyboard, KeyboardInteractivity::None);
    }

    #[test]
    fn toggle_and_hide() {
        let mut host = host(Options::default());
        host.surfaces(&env(true));

        assert_eq!(host.command("toggle sysmon").0, "sysmon shown\n");
        assert_eq!(panels(&host, &env(true)).len(), 1);

        assert_eq!(host.command("toggle sysmon").0, "sysmon hidden\n");
        assert!(panels(&host, &env(true)).is_empty());

        let _ = host.command("summon demo");
        assert_eq!(host.command("hide sysmon").0, "sysmon was not shown\n");
        assert_eq!(host.command("hide").0, "hidden\n");
        assert_eq!(host.command("hide").0, "nothing was shown\n");
    }

    #[test]
    fn one_panel_at_a_time() {
        let mut host = host(Options::default());
        host.surfaces(&env(true));

        let _ = host.command("summon sysmon");
        let _ = host.command("summon demo");

        let panels = panels(&host, &env(true));

        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].namespace, "quadrille-demo");
    }

    #[test]
    fn a_panel_whose_output_is_gone_moves_to_the_first() {
        let mut host = host(Options::default());
        host.surfaces(&env(true));
        let _ = host.command("summon sysmon {\"output\":\"B\"}");

        let mut remaining = env(true);
        remaining.outputs.retain(|output| output.name != "B");

        let panels = panels(&host, &remaining);

        assert_eq!(panels[0].output.as_deref(), Some("A"));
    }

    #[test]
    fn a_panel_defaults_to_where_the_pointer_was() {
        let mut host = host(Options::default());
        host.surfaces(&env(true));
        let _ = host.update(Message::Hover("B".into()));
        let _ = host.command("summon sysmon");

        assert_eq!(panels(&host, &env(true))[0].output.as_deref(), Some("B"));
    }

    #[test]
    fn mistakes_are_errors() {
        let mut host = host(Options::default());
        host.surfaces(&env(true));

        for line in [
            "summon nope",
            "toggle",
            "summon sysmon {oops",
            "summon sysmon {\"output\":\"Z\"}",
            "frobnicate",
            "hide nope",
        ] {
            assert!(
                host.command(line).0.starts_with("error:"),
                "{line:?} should be an error"
            );
        }

        assert!(panels(&host, &env(true)).is_empty());
    }

    #[test]
    fn list_tells_the_panels_the_outputs_and_the_theme() {
        let mut host = host(Options::default());
        host.surfaces(&env(true));
        let _ = host.command("summon sysmon {\"output\":\"A\"}");

        let list = host.command("list").0;

        assert!(list.contains("sysmon"), "{list}");
        assert!(list.contains("visible on A"), "{list}");
        assert!(list.contains("demo") && list.contains("hidden"), "{list}");
        assert!(
            list.contains("output A") && list.contains("1.6667"),
            "{list}"
        );
        assert!(list.contains("theme  Terminal"), "{list}");
    }

    #[test]
    fn a_click_outside_a_grab_closes_the_panel() {
        let mut host = host(Options::default());
        host.surfaces(&env(true));
        let _ = host.command("summon sysmon");

        let (id, _) = host
            .surfaces(&env(true))
            .into_iter()
            .find(|(_, settings)| settings.namespace == "quadrille-sysmon")
            .expect("A panel");

        let _ = host.update(Message::CloseRequested(id));

        assert!(panels(&host, &env(true)).is_empty());
    }

    #[test]
    fn closing_a_bar_does_not_close_the_panel() {
        let mut host = host(Options::default());
        let surfaces = host.surfaces(&env(true));
        let _ = host.command("summon sysmon");
        let _ = host.surfaces(&env(true));

        let _ = host.update(Message::CloseRequested(surfaces[0].0));

        assert_eq!(panels(&host, &env(true)).len(), 1);
    }

    fn without_a_bar() -> Options {
        Options {
            no_bar: true,
            ..Options::default()
        }
    }

    #[test]
    fn without_a_bar_there_is_no_surface_until_a_panel_is_summoned() {
        let mut host = host(without_a_bar());

        assert!(host.surfaces(&env(true)).is_empty());

        let _ = host.command("summon sysmon");

        assert_eq!(host.surfaces(&env(true)).len(), 1);

        let _ = host.command("hide");

        assert!(host.surfaces(&env(true)).is_empty());
    }

    #[test]
    fn without_a_bar_a_panel_is_left_to_the_compositor() {
        let mut host = host(without_a_bar());
        host.surfaces(&env(true));
        let _ = host.command("summon sysmon");

        let panels = panels(&host, &env(true));

        assert_eq!(panels.len(), 1);
        // The compositor puts it on the output that has the focus.
        assert_eq!(panels[0].output, None);
        assert_eq!(panels[0].grab, Grab::Popup);
        assert_eq!(panels[0].exclusive, Exclusive::None);
    }

    #[test]
    fn without_a_bar_a_panel_goes_where_it_is_told() {
        let mut host = host(without_a_bar());
        host.surfaces(&env(true));
        let _ = host.command("summon sysmon {\"output\":\"B\"}");

        assert_eq!(panels(&host, &env(true))[0].output.as_deref(), Some("B"));

        // If that output goes, the compositor's choice stands in.
        let mut remaining = env(true);
        remaining.outputs.retain(|output| output.name != "B");

        assert_eq!(panels(&host, &remaining)[0].output, None);
    }

    #[test]
    fn a_panel_of_every_kind_is_one_window_per_place() {
        let mut host = host(without_a_bar());
        host.surfaces(&env(true));

        let _ = host.command("summon sysmon");
        let anywhere = host.surfaces(&env(true))[0].0;

        let _ = host.command("summon sysmon {\"output\":\"A\"}");
        let on_a = host.surfaces(&env(true))[0].0;

        let _ = host.command("summon sysmon");

        assert_ne!(
            anywhere, on_a,
            "a panel on another output is another window"
        );
        assert_eq!(host.surfaces(&env(true))[0].0, anywhere);
    }

    #[test]
    fn list_tells_where_the_compositor_put_a_panel() {
        let mut host = host(without_a_bar());
        host.surfaces(&env(true));
        let _ = host.command("summon sysmon");

        assert!(
            host.command("list")
                .0
                .contains("visible on the focused output")
        );

        let (id, _) = host.surfaces(&env(true)).remove(0);

        let mut placed = env(true);
        let _ = placed.placements.insert(id, "B".to_owned());

        host.surfaces(&placed);

        assert!(host.command("list").0.contains("visible on B"));
    }

    #[test]
    fn a_click_outside_closes_a_panel_with_no_bar() {
        let mut host = host(without_a_bar());
        host.surfaces(&env(true));
        let _ = host.command("summon demo");

        let (id, _) = host.surfaces(&env(true)).remove(0);
        let _ = host.update(Message::CloseRequested(id));

        assert!(host.surfaces(&env(true)).is_empty());
    }
}
