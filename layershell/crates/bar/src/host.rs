//! The panel host: a bar on every output, and panels that are summoned.
use crate::ipc::{self, Request};
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
}

pub const PANELS: &[PanelDef] = &[
    PanelDef {
        id: "sysmon",
        title: "SYSMON",
        size: (160, 305),
    },
    PanelDef {
        id: "demo",
        title: "DEMO",
        size: (140, 90),
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
    note: String,
}

impl Host {
    pub fn new(options: Options) -> (Self, Task<Message>) {
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
            note: String::new(),
            options,
        };

        if let Some(open) = open {
            host.show(open.panel, open.output);
        }

        (host, Task::none())
    }

    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }

    /// Shows a panel on an output, taking the place of the one that is up.
    fn show(&mut self, id: &'static str, output: Option<String>) {
        if self.open.as_ref().map(|open| open.panel) != Some(id) && id == "sysmon" {
            // A baseline to take the first rates from.
            self.sysmon.reset();
            self.sysmon.sample();
        }

        self.open = Some(Open { panel: id, output });
    }

    fn hide(&mut self) -> bool {
        self.open.take().is_some()
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
            Message::Hover(output) => self.pointer_output = Some(output),
            Message::Toggle(id, output) => {
                if self.open.as_ref().map(|open| open.panel) == Some(id) {
                    let _ = self.hide();
                } else if let Some(def) = panel(id) {
                    self.show(def.id, Some(output));
                }
            }
            Message::Hide | Message::Escape => {
                let _ = self.hide();
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
                let (answer, quit) = self.command(&request.line);

                request.respond(answer);

                if quit {
                    return iced_runtime::exit();
                }
            }
            Message::Quit => return iced_runtime::exit(),
        }

        Task::none()
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
                    let output = self.target(open).unwrap_or_else(|| "?".to_owned());

                    format!("visible on {output}")
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

    /// The output an open panel is on: where it was asked for if there is a
    /// bar there, else the first output that has one.
    fn target(&self, open: &Open) -> Option<String> {
        let env = self.env.borrow();

        let outputs: Vec<_> = env
            .outputs
            .iter()
            .filter(|output| self.wants(&output.name))
            .collect();

        open.output
            .as_ref()
            .filter(|name| outputs.iter().any(|output| &output.name == *name))
            .cloned()
            .or_else(|| outputs.first().map(|output| output.name.clone()))
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
            Subscription::run(minutes),
        ];

        let theme_dir = self.options.theme_dir.clone();

        subscriptions.push(Subscription::run_with(theme_dir, |dir| {
            theme::changes(dir.clone()).map(|()| Message::ThemeChanged)
        }));

        if self.options.bar_tick_ms > 0 {
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

        let mut surfaces: Vec<_> = outputs
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

            if let Some(output) = self.target(open) {
                surfaces.push((
                    self.id(format!("panel:{}:{output}", def.id), Role::Panel(def.id)),
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
                        output: Some(output),
                        grab: if grab { Grab::Popup } else { Grab::None },
                    },
                ));
            }
        }

        surfaces
    }

    pub fn view(&self, window: window::Id) -> Element<'_, Message, Theme, iced_renderer::Renderer> {
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
}
