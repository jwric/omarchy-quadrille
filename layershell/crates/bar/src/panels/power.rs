//! Power: the battery, the power profile, and the session actions (lock,
//! suspend, log out, reboot, power off).
//!
//! The profile is read with `powerprofilesctl` and set the way Omarchy's menu
//! sets it; the actions run the commands Omarchy's own menu runs. An action
//! asks first: choosing it shows `REBOOT?` with YES and NO, the keyboard on NO,
//! and nothing is sent until YES is pressed. Escape backs out.
use iced_core::Length;
use iced_runtime::Task;
use iced_widget::core::Alignment;
use iced_widget::{Widget as _, column, row, space};

use quadrille::draw::Sprite;
use quadrille::{Element, px, style, widget};

use crate::commands::{self, LONG, Shared, TIMEOUT};
use crate::panels::{Key, step};
use crate::sysmon;
use crate::widgets::rows::{self, label_width};
use crate::widgets::{self, Add as _, icons};

pub const WIDTH: u16 = 170;

const INNER: u16 = WIDTH - 16;

/// What can be done to the session, in the order of Omarchy's menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    Lock,
    Suspend,
    Logout,
    Reboot,
    Shutdown,
}

impl Session {
    pub const ALL: [Session; 5] = [
        Session::Lock,
        Session::Suspend,
        Session::Logout,
        Session::Reboot,
        Session::Shutdown,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Session::Lock => "LOCK",
            Session::Suspend => "SUSPEND",
            Session::Logout => "LOG OUT",
            Session::Reboot => "REBOOT",
            Session::Shutdown => "POWER OFF",
        }
    }

    fn icon(self) -> Sprite {
        match self {
            Session::Lock => icons::LOCK,
            Session::Suspend => icons::MOON,
            Session::Logout => icons::LOGOUT,
            Session::Reboot => icons::REBOOT,
            Session::Shutdown => icons::POWER,
        }
    }

    /// The command Omarchy's menu (`omarchy-menu.jsonc`, `system.*`) runs.
    pub fn command(self) -> &'static [&'static str] {
        match self {
            Session::Lock => &["omarchy-system-lock"],
            Session::Suspend => &["systemctl", "suspend"],
            Session::Logout => &["omarchy-system-logout"],
            Session::Reboot => &["omarchy-system-reboot"],
            Session::Shutdown => &["omarchy-system-shutdown"],
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub battery: Option<Battery>,
    pub on_ac: bool,
    pub profiles: Vec<String>,
    pub active: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub percent: u8,
    pub status: String,
    pub watts: Option<f32>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Result<Snapshot, String>),
    Profile(usize),
    /// Choose a session action: asks first.
    Ask(Session),
    Confirm,
    Cancel,
    Done(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Profile(usize),
    Session(Session),
    Yes,
    No,
}

#[derive(Debug, Default)]
pub struct State {
    snapshot: Snapshot,
    loaded: bool,
    error: Option<String>,
    notice: Option<String>,
    focus: usize,
    reading: bool,
    busy: bool,
    asking: Option<Session>,
    /// A session action is on its way.
    closing: bool,
    /// A session action went through: the panel has nothing more to say.
    close: bool,
}

impl State {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Whether the panel should be closed now, once: an action has gone out.
    pub fn take_close(&mut self) -> bool {
        std::mem::take(&mut self.close)
    }

    pub fn refresh(&mut self, runner: &Shared) -> Task<Message> {
        if self.reading {
            return Task::none();
        }

        self.reading = true;

        let runner = runner.clone();

        Task::perform(
            commands::blocking(move || Ok(read(&*runner))),
            Message::Loaded,
        )
    }

    pub fn update(&mut self, message: Message, runner: &Shared) -> Task<Message> {
        match message {
            Message::Loaded(result) => {
                self.reading = false;

                match result {
                    Ok(snapshot) => {
                        self.snapshot = snapshot;
                        self.loaded = true;
                        self.focus = self.focus.min(self.items().len().saturating_sub(1));
                    }
                    Err(error) => self.error = Some(error),
                }

                Task::none()
            }
            Message::Profile(index) => {
                self.focus = index;

                let Some(profile) = self.snapshot.profiles.get(index).cloned() else {
                    return Task::none();
                };

                self.snapshot.active = Some(profile.clone());

                self.run(vec![profile_call(&profile)], false, runner)
            }
            Message::Ask(session) => {
                self.asking = Some(session);

                // The keyboard goes to NO: Enter by accident is safe.
                self.focus = self
                    .items()
                    .iter()
                    .position(|item| *item == Item::No)
                    .unwrap_or(0);

                Task::none()
            }
            Message::Confirm => match self.asking.take() {
                Some(session) => {
                    self.focus = 0;

                    self.run(vec![session_call(session)], true, runner)
                }
                None => Task::none(),
            },
            Message::Cancel => {
                self.cancel();

                Task::none()
            }
            Message::Done(result) => {
                self.busy = false;

                match result {
                    Ok(_) => {
                        self.error = None;
                        self.close = self.closing;
                    }
                    Err(error) => self.error = Some(error),
                }

                self.closing = false;

                self.reading = false;
                self.refresh(runner)
            }
        }
    }

    fn cancel(&mut self) {
        if self.asking.take().is_some() {
            self.focus = 0;
        }
    }

    /// Backs out of a question; false if there was none.
    pub fn escape(&mut self) -> bool {
        let asking = self.asking.is_some();

        self.cancel();

        asking
    }

    pub fn key(&mut self, key: Key, runner: &Shared) -> Task<Message> {
        let items = self.items();

        if items.is_empty() {
            return Task::none();
        }

        let item = items[self.focus.min(items.len() - 1)];

        match key {
            Key::Up | Key::Left | Key::BackTab => self.focus = step(self.focus, items.len(), false),
            Key::Down | Key::Right | Key::Tab => self.focus = step(self.focus, items.len(), true),
            Key::Home => self.focus = 0,
            Key::End => self.focus = items.len() - 1,
            Key::Enter | Key::Space => {
                return match item {
                    Item::Profile(i) => self.update(Message::Profile(i), runner),
                    Item::Session(session) => self.update(Message::Ask(session), runner),
                    Item::Yes => self.update(Message::Confirm, runner),
                    Item::No => self.update(Message::Cancel, runner),
                };
            }
            Key::Char('y') if self.asking.is_some() => {
                return self.update(Message::Confirm, runner);
            }
            Key::Char('n') if self.asking.is_some() => return self.update(Message::Cancel, runner),
            _ => {}
        }

        Task::none()
    }

    fn items(&self) -> Vec<Item> {
        if self.asking.is_some() {
            return vec![Item::Yes, Item::No];
        }

        let mut items: Vec<Item> = (0..self.snapshot.profiles.len())
            .map(Item::Profile)
            .collect();

        items.extend(Session::ALL.map(Item::Session));
        items
    }

    fn run(&mut self, calls: Vec<Vec<String>>, session: bool, runner: &Shared) -> Task<Message> {
        if self.busy {
            return Task::none();
        }

        self.busy = true;
        self.closing = session;

        let runner = runner.clone();

        Task::perform(
            commands::blocking(move || {
                for call in calls {
                    let args: Vec<&str> = call[1..].iter().map(String::as_str).collect();

                    runner.run(&call[0], &args, LONG)?;
                }

                Ok(String::new())
            }),
            Message::Done,
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        if !self.loaded {
            return match &self.error {
                Some(error) => widget::label(rows::fit(error, INNER))
                    .style(style::text::alarm)
                    .boxed(),
                None => widget::label("READING...")
                    .style(style::text::muted)
                    .boxed(),
            };
        }

        let items = self.items();
        let focused = |item: Item| items.get(self.focus) == Some(&item);

        let mut body = column![].spacing(px::GAP);

        // The battery.
        let battery = match &self.snapshot.battery {
            Some(battery) => {
                let watts = battery
                    .watts
                    .map(|watts| format!(" {watts:.1}W"))
                    .unwrap_or_default();

                column![
                    row![
                        widget::icon::<quadrille::Theme>(icons::BATTERY),
                        widget::label(rows::fit(
                            &format!(
                                "{:>3}% {}{watts}",
                                battery.percent,
                                battery.status.to_uppercase()
                            ),
                            label_width(INNER, true, 0),
                        )),
                    ]
                    .spacing(px::GAP)
                    .padding([2.0, f32::from(rows::PADDING)])
                    .align_y(Alignment::Center),
                    row![
                        widget::bar(0.0..=100.0, f32::from(battery.percent))
                            .segments(3)
                            .height(5)
                            .width(Length::Fill),
                    ]
                    .padding([0.0, f32::from(rows::PADDING)]),
                ]
                .spacing(px::TIGHT)
            }
            None => column![
                row![
                    widget::icon::<quadrille::Theme>(icons::PLUG),
                    widget::label(if self.snapshot.on_ac {
                        "NO BATTERY, ON MAINS"
                    } else {
                        "NO BATTERY"
                    }),
                ]
                .spacing(px::GAP)
                .padding([2.0, f32::from(rows::PADDING)])
                .align_y(Alignment::Center),
            ],
        };

        body = body.add(widget::group("BATTERY", battery).width(Length::Fill));

        if !self.snapshot.profiles.is_empty() {
            let mut profiles = column![].spacing(0);

            for (i, profile) in self.snapshot.profiles.iter().enumerate() {
                profiles = profiles.add(widgets::marks(
                    rows::choice(
                        self.snapshot.active.as_deref() == Some(profile.as_str()),
                        Some(profile_icon(profile)),
                        rows::fit(&profile.to_uppercase(), label_width(INNER, true, 0)),
                        None,
                        Message::Profile(i),
                    ),
                    focused(Item::Profile(i)),
                ));
            }

            body = body.add(widget::group("PROFILE", profiles).width(Length::Fill));
        }

        match self.asking {
            None => {
                let mut session = column![].spacing(0);

                for action in Session::ALL {
                    session = session.add(widgets::marks(
                        rows::choice(
                            false,
                            Some(action.icon()),
                            action.label(),
                            None,
                            Message::Ask(action),
                        ),
                        focused(Item::Session(action)),
                    ));
                }

                body = body.add(widget::group("SESSION", session).width(Length::Fill));
            }
            Some(action) => {
                body = body.add(
                    widget::group(
                        "CONFIRM",
                        column![
                            row![
                                widget::icon::<quadrille::Theme>(action.icon()),
                                widget::label(format!("{}?", action.label())),
                            ]
                            .spacing(px::GAP)
                            .padding([2.0, f32::from(rows::PADDING)])
                            .align_y(Alignment::Center),
                            row![
                                widgets::marks(
                                    widget::button("YES").on_press(Message::Confirm).boxed(),
                                    focused(Item::Yes),
                                ),
                                widgets::marks(
                                    widget::button("NO").on_press(Message::Cancel).boxed(),
                                    focused(Item::No),
                                ),
                                space::horizontal(),
                            ]
                            .spacing(px::WIDE)
                            .padding([2.0, 2.0]),
                        ]
                        .spacing(px::GAP),
                    )
                    .width(Length::Fill),
                );
            }
        }

        if let Some(error) = &self.error {
            body = body.add(widget::label(rows::fit(error, INNER)).style(style::text::alarm));
        } else if let Some(notice) = &self.notice {
            body = body.add(widget::label(rows::fit(notice, INNER)).style(style::text::muted));
        }

        body.boxed()
    }
}

fn profile_icon(profile: &str) -> Sprite {
    match profile {
        "power-saver" => icons::LEAF,
        "performance" => icons::BOLT,
        _ => icons::SCALES,
    }
}

/// The call that sets a profile: Omarchy's own, which remembers it for
/// whether the machine is on mains or on the battery.
pub fn profile_call(profile: &str) -> Vec<String> {
    ["omarchy-powerprofiles-set", "autodetect", profile]
        .iter()
        .map(|word| (*word).to_owned())
        .collect()
}

/// The call that does a session action.
pub fn session_call(session: Session) -> Vec<String> {
    session
        .command()
        .iter()
        .map(|word| (*word).to_owned())
        .collect()
}

/// Reads the battery from `/sys` and the profiles from `powerprofilesctl`.
pub fn read(runner: &dyn commands::Runner) -> Snapshot {
    let power = sysmon::power();

    let profiles = runner
        .run("powerprofilesctl", &["list"], TIMEOUT)
        .map(|list| parse_profiles(&list))
        .unwrap_or_default();

    let active = runner
        .run("powerprofilesctl", &["get"], TIMEOUT)
        .ok()
        .map(|active| active.trim().to_owned())
        .filter(|active| !active.is_empty());

    Snapshot {
        on_ac: power.as_ref().is_some_and(|power| power.on_ac),
        battery: power.map(|power| Battery {
            percent: power.percent,
            status: power.status,
            watts: power.watts,
        }),
        profiles,
        active,
    }
}

/// The profiles `powerprofilesctl list` names, in the order Omarchy's panel
/// has them: power saver, balanced, performance.
pub fn parse_profiles(list: &str) -> Vec<String> {
    let mut profiles: Vec<String> = list
        .lines()
        .filter_map(|line| {
            let line = line.trim_start_matches(['*', ' ', '\t']);

            line.strip_suffix(':')
                .filter(|name| {
                    !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '-')
                })
                .map(str::to_owned)
        })
        .collect();

    let rank = |name: &str| match name {
        "power-saver" => 0,
        "balanced" => 1,
        "performance" => 2,
        _ => 3,
    };

    profiles.sort_by_key(|name| rank(name));
    profiles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::recorder::Recorder;

    const LIST: &str = "\
* performance:
    CpuDriver:\tintel_pstate
    Degraded:   no

  balanced:
    CpuDriver:\tintel_pstate
    PlatformDriver:\tplaceholder

  power-saver:
    CpuDriver:\tintel_pstate
    PlatformDriver:\tplaceholder
";

    fn recorder() -> (std::sync::Arc<Recorder>, Shared) {
        let (recorder, shared) = Recorder::shared();

        recorder.answer("powerprofilesctl list", Ok(LIST));
        recorder.answer("powerprofilesctl get", Ok("balanced\n"));

        (recorder, shared)
    }

    fn loaded() -> (std::sync::Arc<Recorder>, Shared, State) {
        let (recorder, shared) = recorder();
        let mut state = State::default();

        let snapshot = read(&*shared);
        let _ = state.update(Message::Loaded(Ok(snapshot)), &shared);
        recorder.clear();

        (recorder, shared, state)
    }

    #[test]
    fn profiles_are_read_in_omarchys_order() {
        assert_eq!(
            parse_profiles(LIST),
            ["power-saver", "balanced", "performance"]
        );
    }

    #[test]
    fn a_machine_with_no_power_profiles_has_none_to_show() {
        let (recorder, shared) = Recorder::shared();
        recorder.answer("powerprofilesctl", Err("not found"));

        let snapshot = read(&*shared);

        assert!(snapshot.profiles.is_empty());
        assert_eq!(snapshot.active, None);
    }

    #[test]
    fn the_session_actions_are_the_commands_of_omarchys_menu() {
        let lines: Vec<String> = Session::ALL
            .iter()
            .map(|session| session_call(*session).join(" "))
            .collect();

        assert_eq!(
            lines,
            [
                "omarchy-system-lock",
                "systemctl suspend",
                "omarchy-system-logout",
                "omarchy-system-reboot",
                "omarchy-system-shutdown",
            ]
        );
    }

    #[test]
    fn a_profile_is_set_the_way_the_menu_sets_it() {
        assert_eq!(
            profile_call("power-saver").join(" "),
            "omarchy-powerprofiles-set autodetect power-saver"
        );
    }

    #[test]
    fn an_action_asks_first_and_sends_nothing_until_yes() {
        let (recorder, shared, mut state) = loaded();

        let _ = state.update(Message::Ask(Session::Shutdown), &shared);

        assert_eq!(state.asking, Some(Session::Shutdown));
        assert!(!state.busy, "nothing is running");
        assert!(recorder.calls().is_empty());

        // The keyboard is on NO, so Enter is safe.
        assert_eq!(state.items()[state.focus], Item::No);

        let _ = state.key(Key::Enter, &shared);

        assert_eq!(state.asking, None, "NO backs out");
        assert!(!state.busy);
    }

    #[test]
    fn yes_runs_the_action_once_and_asks_the_panel_to_close() {
        let (_, shared, mut state) = loaded();

        let _ = state.update(Message::Ask(Session::Reboot), &shared);
        let _ = state.key(Key::Left, &shared);

        assert_eq!(state.items()[state.focus], Item::Yes);

        let _ = state.key(Key::Enter, &shared);

        assert!(state.busy, "the command is on its way");
        assert!(state.closing);
        assert!(!state.take_close(), "not until it has gone through");
        assert_eq!(state.asking, None);

        // A second yes while it runs does nothing.
        let _ = state.update(Message::Confirm, &shared);

        let _ = state.update(Message::Done(Ok(String::new())), &shared);

        assert!(state.take_close());
        assert!(!state.take_close(), "once");
    }

    #[test]
    fn a_failed_action_keeps_the_panel_open_and_says_why() {
        let (_, shared, mut state) = loaded();

        let _ = state.update(Message::Ask(Session::Suspend), &shared);
        let _ = state.update(Message::Confirm, &shared);
        let _ = state.update(Message::Done(Err("Access denied".into())), &shared);

        assert!(!state.close && !state.closing);
        assert_eq!(state.error.as_deref(), Some("Access denied"));
    }

    #[test]
    fn escape_backs_out_of_the_question_before_it_closes_anything() {
        let (_, shared, mut state) = loaded();

        assert!(!state.escape());

        let _ = state.update(Message::Ask(Session::Logout), &shared);

        assert!(state.escape());
        assert!(!state.escape());
        assert_eq!(state.asking, None);
    }

    #[test]
    fn y_and_n_answer_the_question_only_while_it_is_asked() {
        let (_, shared, mut state) = loaded();

        let _ = state.key(Key::Char('y'), &shared);
        assert!(!state.busy, "y does nothing with no question");

        let _ = state.update(Message::Ask(Session::Lock), &shared);
        let _ = state.key(Key::Char('n'), &shared);
        assert_eq!(state.asking, None);
        assert!(!state.busy);

        let _ = state.update(Message::Ask(Session::Lock), &shared);
        let _ = state.key(Key::Char('y'), &shared);
        assert!(state.busy);
    }

    #[test]
    fn while_asked_the_keyboard_has_only_yes_and_no() {
        let (_, shared, mut state) = loaded();

        // Three profiles and five actions.
        assert_eq!(state.items().len(), 3 + 5);

        let _ = state.update(Message::Ask(Session::Lock), &shared);

        assert_eq!(state.items(), [Item::Yes, Item::No]);
    }

    #[test]
    fn choosing_a_profile_sets_it_at_once_on_screen() {
        let (_, shared, mut state) = loaded();

        let _ = state.update(Message::Profile(0), &shared);

        assert_eq!(state.snapshot.active.as_deref(), Some("power-saver"));
        assert!(state.busy);
        assert!(!state.closing, "a profile does not close the panel");
    }
}
