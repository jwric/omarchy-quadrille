//! Bluetooth: whether the adapter is on, the devices paired and the ones in
//! range, and connecting to them. Read with `bluetoothctl`, changed with the
//! commands Omarchy itself uses (`omarchy-bluetooth-power`, which keeps the
//! state across a reboot, and `omarchy-bluetooth-device`, which powers the
//! adapter up, pairs, trusts and connects the way a keyboard needs).
//!
//! A connected device is an inverse row. Enter connects or disconnects a paired
//! device and pairs one that is only nearby; S looks for devices for a few
//! seconds.
use iced_core::Length;
use iced_runtime::Task;
use iced_widget::core::Alignment;
use iced_widget::{Widget as _, column, row, space};

use quadrille::{Element, px, style, widget};

use crate::commands::{self, LONG, Shared, TIMEOUT};
use crate::panels::{Key, step};
use crate::widgets::rows::{self, label_width};
use crate::widgets::{self, Add as _, icons};

pub const WIDTH: u16 = 170;

const INNER: u16 = WIDTH - 16;

/// How long a search for devices runs, in seconds.
pub const SCAN_SECONDS: &str = "8";

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub mac: String,
    pub name: String,
    pub paired: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub powered: bool,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Result<Snapshot, String>),
    TogglePower,
    /// Connect, disconnect or pair the device at this place in the list.
    Choose(usize),
    Scan,
    Done(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Power,
    Scan,
    Device(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Power(bool),
    Connect(String),
    Disconnect(String),
    Pair(String),
    Scan,
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
    scanning: bool,
}

impl State {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn refresh(&mut self, runner: &Shared) -> Task<Message> {
        if self.reading {
            return Task::none();
        }

        self.reading = true;

        let runner = runner.clone();

        Task::perform(commands::blocking(move || read(&*runner)), Message::Loaded)
    }

    pub fn update(&mut self, message: Message, runner: &Shared) -> Task<Message> {
        match message {
            Message::Loaded(result) => {
                self.reading = false;

                match result {
                    Ok(snapshot) => {
                        self.snapshot = snapshot;
                        self.loaded = true;
                        self.error = None;
                        self.focus = self.focus.min(self.items().len().saturating_sub(1));
                    }
                    Err(error) => self.error = Some(error),
                }

                Task::none()
            }
            Message::TogglePower => {
                self.focus_on(Item::Power);

                let on = !self.snapshot.powered;

                self.snapshot.powered = on;

                self.act(Action::Power(on), runner)
            }
            Message::Choose(index) => {
                self.focus_on(Item::Device(index));

                if self.busy {
                    return Task::none();
                }

                let Some(device) = self.snapshot.devices.get(index) else {
                    return Task::none();
                };

                let action = match (device.paired, device.connected) {
                    (_, true) => Action::Disconnect(device.mac.clone()),
                    (true, false) => Action::Connect(device.mac.clone()),
                    (false, false) => Action::Pair(device.mac.clone()),
                };

                self.notice = Some(
                    match &action {
                        Action::Disconnect(_) => format!("DISCONNECTING {}", device.name),
                        Action::Connect(_) => format!("CONNECTING TO {}", device.name),
                        _ => format!("PAIRING WITH {}", device.name),
                    }
                    .to_uppercase(),
                );

                self.act(action, runner)
            }
            Message::Scan => {
                self.focus_on(Item::Scan);

                if self.scanning || !self.snapshot.powered {
                    return Task::none();
                }

                self.scanning = true;

                let runner = runner.clone();
                let calls = calls(&Action::Scan);

                Task::perform(
                    commands::blocking(move || run_calls(&*runner, &calls, LONG)),
                    |result| Message::Done(result.map(|()| "SCAN DONE".to_owned())),
                )
            }
            Message::Done(result) => {
                self.busy = false;
                self.scanning = false;

                match result {
                    Ok(notice) => {
                        self.error = None;
                        self.notice = (!notice.is_empty()).then_some(notice);
                    }
                    Err(error) => {
                        self.notice = None;
                        self.error = Some(error);
                    }
                }

                self.reading = false;
                self.refresh(runner)
            }
        }
    }

    pub fn escape(&mut self) -> bool {
        false
    }

    pub fn key(&mut self, key: Key, runner: &Shared) -> Task<Message> {
        let items = self.items();

        if items.is_empty() {
            return Task::none();
        }

        let item = items[self.focus.min(items.len() - 1)];

        match key {
            Key::Up | Key::BackTab => self.focus = step(self.focus, items.len(), false),
            Key::Down | Key::Tab => self.focus = step(self.focus, items.len(), true),
            Key::Home => self.focus = 0,
            Key::End => self.focus = items.len() - 1,
            Key::Char('s') => return self.update(Message::Scan, runner),
            Key::Enter | Key::Space => {
                return match item {
                    Item::Power => self.update(Message::TogglePower, runner),
                    Item::Scan => self.update(Message::Scan, runner),
                    Item::Device(i) => self.update(Message::Choose(i), runner),
                };
            }
            _ => {}
        }

        Task::none()
    }

    fn focus_on(&mut self, item: Item) {
        if let Some(position) = self.items().iter().position(|i| *i == item) {
            self.focus = position;
        }
    }

    fn items(&self) -> Vec<Item> {
        let mut items = vec![Item::Power];

        if self.snapshot.powered {
            items.push(Item::Scan);
            items.extend((0..self.snapshot.devices.len()).map(Item::Device));
        }

        items
    }

    fn act(&mut self, action: Action, runner: &Shared) -> Task<Message> {
        if self.busy {
            return Task::none();
        }

        self.busy = true;

        let runner = runner.clone();
        let calls = calls(&action);

        Task::perform(
            commands::blocking(move || run_calls(&*runner, &calls, LONG).map(|()| String::new())),
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

        let mut adapter = column![widgets::marks(
            iced_widget::container(
                widget::toggler("POWERED", self.snapshot.powered)
                    .on_toggle(|_| Message::TogglePower),
            )
            .width(Length::Fill)
            .padding([2.0, 0.0])
            .boxed(),
            focused(Item::Power),
        )]
        .spacing(0);

        if self.snapshot.powered {
            adapter = adapter.add(widgets::marks(
                row![
                    widget::button(if self.scanning { "SCANNING" } else { "SCAN" })
                        .on_press(Message::Scan),
                    space::horizontal(),
                ]
                .padding([2.0, 0.0])
                .align_y(Alignment::Center)
                .boxed(),
                focused(Item::Scan),
            ));
        }

        let mut body =
            column![widget::group("ADAPTER", adapter).width(Length::Fill)].spacing(px::GAP);

        if self.snapshot.powered {
            let list = |paired: bool| {
                let mut list = column![].spacing(0);
                let mut any = false;

                for (i, device) in self
                    .snapshot
                    .devices
                    .iter()
                    .enumerate()
                    .filter(|(_, device)| device.paired == paired)
                {
                    any = true;

                    let status = if device.connected {
                        "CONNECTED"
                    } else if paired {
                        ""
                    } else {
                        "PAIR"
                    };

                    // What is left for the name after the status, if there is one.
                    let width = quadrille::Face::BODY.width(status);
                    let named = label_width(INNER, true, width);

                    list = list.add(widgets::marks(
                        rows::choice(
                            device.connected,
                            Some(icon_of(&device.name)),
                            rows::fit(&device.name, named),
                            (!status.is_empty()).then(|| widget::label(status).boxed()),
                            Message::Choose(i),
                        ),
                        focused(Item::Device(i)),
                    ));
                }

                if !any {
                    list = list.add(
                        widget::label(if paired {
                            "NONE PAIRED"
                        } else {
                            "NOTHING NEARBY"
                        })
                        .style(style::text::faint),
                    );
                }

                list
            };

            body = body
                .add(widget::group("PAIRED", list(true)).width(Length::Fill))
                .add(widget::group("NEARBY", list(false)).width(Length::Fill));
        } else {
            body = body.add(widget::label("BLUETOOTH IS OFF").style(style::text::muted));
        }

        if let Some(error) = &self.error {
            body = body.add(widget::label(rows::fit(error, INNER)).style(style::text::alarm));
        } else if let Some(notice) = &self.notice {
            body = body.add(widget::label(rows::fit(notice, INNER)).style(style::text::muted));
        }

        body.boxed()
    }
}

/// An icon for a device, from what its name suggests.
pub fn icon_of(name: &str) -> quadrille::draw::Sprite {
    let name = name.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|word| name.contains(word));

    if has(&[
        "buds",
        "pods",
        "headphone",
        "headset",
        "momentum",
        "wh-1",
        "wf-",
    ]) {
        icons::HEADPHONES
    } else if has(&["keyboard", "k250", "k380", "mx keys"]) {
        icons::KEYBOARD
    } else if has(&["mouse", "m196", "mx master", "trackpad"]) {
        icons::MOUSE
    } else if has(&["phone", "pixel", "galaxy"]) {
        icons::PHONE
    } else if has(&["speaker", "boom", "jbl", "soundbar", "echo"]) {
        icons::SPEAKER
    } else {
        icons::BLUETOOTH
    }
}

fn run_calls(
    runner: &dyn commands::Runner,
    calls: &[Vec<String>],
    wait: std::time::Duration,
) -> Result<(), String> {
    for call in calls {
        let args: Vec<&str> = call[1..].iter().map(String::as_str).collect();

        runner.run(&call[0], &args, wait)?;
    }

    Ok(())
}

/// The calls an action makes.
pub fn calls(action: &Action) -> Vec<Vec<String>> {
    let call =
        |words: &[&str]| -> Vec<String> { words.iter().map(|word| (*word).to_owned()).collect() };

    match action {
        Action::Power(on) => vec![call(&[
            "omarchy-bluetooth-power",
            if *on { "on" } else { "off" },
        ])],
        Action::Connect(mac) => vec![call(&["omarchy-bluetooth-device", "connect", mac])],
        Action::Disconnect(mac) => vec![call(&["omarchy-bluetooth-device", "disconnect", mac])],
        Action::Pair(mac) => vec![call(&["omarchy-bluetooth-device", "pair", mac])],
        Action::Scan => vec![call(&[
            "bluetoothctl",
            "--timeout",
            SCAN_SECONDS,
            "scan",
            "on",
        ])],
    }
}

/// Reads the adapter and the devices.
pub fn read(runner: &dyn commands::Runner) -> Result<Snapshot, String> {
    let show = runner.run("bluetoothctl", &["show"], TIMEOUT)?;

    if !show.contains("Controller") {
        return Err("NO BLUETOOTH ADAPTER".to_owned());
    }

    let powered = show.contains("Powered: yes");

    if !powered {
        return Ok(Snapshot {
            powered,
            devices: Vec::new(),
        });
    }

    let all = runner.run("bluetoothctl", &["devices"], TIMEOUT)?;
    let paired = runner
        .run("bluetoothctl", &["devices", "Paired"], TIMEOUT)
        .unwrap_or_default();
    let connected = runner
        .run("bluetoothctl", &["devices", "Connected"], TIMEOUT)
        .unwrap_or_default();

    Ok(Snapshot {
        powered,
        devices: merge(&all, &paired, &connected),
    })
}

/// `Device AA:BB:CC:DD:EE:FF Name` lines as (address, name).
pub fn parse_devices(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("Device ")?;
            let (mac, name) = rest.split_once(' ')?;

            Some((mac.to_owned(), name.trim().to_owned()))
        })
        .collect()
}

/// The devices known, the connected first, then the paired, then the ones
/// only nearby, each by name.
pub fn merge(all: &str, paired: &str, connected: &str) -> Vec<Device> {
    let paired: Vec<_> = parse_devices(paired)
        .into_iter()
        .map(|(mac, _)| mac)
        .collect();
    let connected: Vec<_> = parse_devices(connected)
        .into_iter()
        .map(|(mac, _)| mac)
        .collect();

    let mut devices: Vec<Device> = parse_devices(all)
        .into_iter()
        .map(|(mac, name)| Device {
            paired: paired.contains(&mac) || connected.contains(&mac),
            connected: connected.contains(&mac),
            mac,
            name,
        })
        .collect();

    // A device that BlueZ remembers only by its address, named after it, is
    // not a device anyone chose to see.
    devices.retain(|device| device.name.replace('-', ":") != device.mac);

    devices.sort_by(|a, b| {
        b.connected
            .cmp(&a.connected)
            .then(b.paired.cmp(&a.paired))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    devices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::recorder::Recorder;

    const SHOW: &str = "Controller AA:AA:AA:AA:AA:AA (public)\n\tName: laptop\n\tPowered: yes\n";

    const ALL: &str = "\
Device 11:11:11:11:11:11 Speaker One
Device 22:22:22:22:22:22 Headphones
Device 33:33:33:33:33:33 Mystery Phone
Device 44:44:44:44:44:44 Keyboard K1
Device 55:55:55:55:55:55 55-55-55-55-55-55
";

    const PAIRED: &str = "\
Device 11:11:11:11:11:11 Speaker One
Device 22:22:22:22:22:22 Headphones
Device 44:44:44:44:44:44 Keyboard K1
";

    const CONNECTED: &str = "Device 22:22:22:22:22:22 Headphones\n";

    fn recorder() -> (std::sync::Arc<Recorder>, Shared) {
        let (recorder, shared) = Recorder::shared();

        recorder.answer("bluetoothctl show", Ok(SHOW));
        recorder.answer("bluetoothctl devices", Ok(ALL));
        recorder.answer("bluetoothctl devices Paired", Ok(PAIRED));
        recorder.answer("bluetoothctl devices Connected", Ok(CONNECTED));

        (recorder, shared)
    }

    fn loaded() -> (std::sync::Arc<Recorder>, Shared, State) {
        let (recorder, shared) = recorder();
        let mut state = State::default();

        let _ = state.update(Message::Loaded(read(&*shared)), &shared);
        recorder.clear();

        (recorder, shared, state)
    }

    #[test]
    fn devices_are_connected_first_then_paired_then_nearby() {
        let devices = merge(ALL, PAIRED, CONNECTED);
        let names: Vec<_> = devices
            .iter()
            .map(|d| (d.name.as_str(), d.paired, d.connected))
            .collect();

        assert_eq!(
            names,
            [
                ("Headphones", true, true),
                ("Keyboard K1", true, false),
                ("Speaker One", true, false),
                ("Mystery Phone", false, false),
            ],
            "the one known only by its address is not listed"
        );
    }

    #[test]
    fn an_adapter_that_is_off_has_no_devices_to_show() {
        let (recorder, shared) = Recorder::shared();
        recorder.answer(
            "bluetoothctl show",
            Ok("Controller AA:AA (public)\n\tPowered: no\n"),
        );

        let snapshot = read(&*shared).unwrap();

        assert!(!snapshot.powered);
        assert!(snapshot.devices.is_empty());
        assert_eq!(
            recorder.calls(),
            ["bluetoothctl show"],
            "it does not ask for devices"
        );
    }

    #[test]
    fn no_adapter_is_an_error() {
        let (recorder, shared) = Recorder::shared();
        recorder.answer("bluetoothctl show", Ok("No default controller available\n"));

        assert_eq!(read(&*shared).unwrap_err(), "NO BLUETOOTH ADAPTER");
    }

    fn call_lines(action: Action) -> Vec<String> {
        calls(&action).iter().map(|call| call.join(" ")).collect()
    }

    #[test]
    fn what_is_sent_for_each_action_is_what_omarchy_sends() {
        let mac = "22:22:22:22:22:22";

        assert_eq!(
            call_lines(Action::Power(true)),
            ["omarchy-bluetooth-power on"]
        );
        assert_eq!(
            call_lines(Action::Power(false)),
            ["omarchy-bluetooth-power off"]
        );
        assert_eq!(
            call_lines(Action::Connect(mac.into())),
            ["omarchy-bluetooth-device connect 22:22:22:22:22:22"]
        );
        assert_eq!(
            call_lines(Action::Disconnect(mac.into())),
            ["omarchy-bluetooth-device disconnect 22:22:22:22:22:22"]
        );
        assert_eq!(
            call_lines(Action::Pair(mac.into())),
            ["omarchy-bluetooth-device pair 22:22:22:22:22:22"]
        );
        assert_eq!(
            call_lines(Action::Scan),
            ["bluetoothctl --timeout 8 scan on"]
        );
    }

    #[test]
    fn enter_connects_disconnects_or_pairs_by_what_the_device_is() {
        let (_, shared, mut state) = loaded();

        // Order: Headphones (connected), Keyboard K1, Speaker One, Mystery Phone.
        assert_eq!(state.items().len(), 2 + 4);

        let _ = state.update(Message::Choose(0), &shared);
        assert_eq!(state.notice.as_deref(), Some("DISCONNECTING HEADPHONES"));
        state.busy = false;

        let _ = state.update(Message::Choose(1), &shared);
        assert_eq!(state.notice.as_deref(), Some("CONNECTING TO KEYBOARD K1"));
        state.busy = false;

        let _ = state.update(Message::Choose(3), &shared);
        assert_eq!(state.notice.as_deref(), Some("PAIRING WITH MYSTERY PHONE"));
    }

    #[test]
    fn a_second_action_waits_for_the_first() {
        let (_, shared, mut state) = loaded();

        let _ = state.update(Message::Choose(1), &shared);
        assert!(state.busy);

        state.notice = None;
        let _ = state.update(Message::Choose(2), &shared);

        assert_eq!(state.notice, None, "ignored while one is running");
    }

    #[test]
    fn the_keyboard_walks_the_rows_and_toggles_power() {
        let (_, shared, mut state) = loaded();

        let _ = state.key(Key::Enter, &shared); // the first row is the power
        assert!(!state.snapshot.powered);
        assert_eq!(
            state.items(),
            [Item::Power],
            "off, there is only the switch"
        );
    }

    #[test]
    fn a_scan_needs_the_adapter_on_and_runs_once() {
        let (_, shared, mut state) = loaded();

        let _ = state.update(Message::Scan, &shared);
        assert!(state.scanning);

        let _ = state.update(Message::Scan, &shared);
        assert!(state.scanning);

        state.scanning = false;
        state.snapshot.powered = false;
        let _ = state.update(Message::Scan, &shared);
        assert!(!state.scanning, "not with the adapter off");
    }

    #[test]
    fn a_device_gets_the_icon_its_name_suggests() {
        assert_eq!(icon_of("MOMENTUM 4"), icons::HEADPHONES);
        assert_eq!(icon_of("Logi K250"), icons::KEYBOARD);
        assert_eq!(icon_of("BluetoothMouse3600"), icons::MOUSE);
        assert_eq!(icon_of("Alex's Buds3 Pro"), icons::HEADPHONES);
        assert_eq!(icon_of("MEGABOOM 3"), icons::SPEAKER);
        assert_eq!(icon_of("Mystery"), icons::BLUETOOTH);
    }
}
