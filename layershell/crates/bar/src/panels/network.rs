//! Network: the connection and its signal, the Wi-Fi networks in range, and
//! connecting to them; ethernet and VPN status. Everything goes through
//! `nmcli` in its terse mode, which is meant to be parsed.
//!
//! A network row chooses: Enter on the one in use disconnects it, on a saved
//! or an open one connects, and on a secured one nobody has joined asks for the
//! password in a field below the list. A name that does not fit is cut with an
//! ellipsis, or left out when there is no room even for that.
use std::collections::HashSet;

use iced_core::Length;
use iced_runtime::Task;
use iced_runtime::widget::operation;
use iced_widget::core::Alignment;
use iced_widget::core::widget::Id;
use iced_widget::{Widget as _, column, row, space};

use quadrille::{Element, px, style, widget};

use crate::commands::{self, LONG, Shared, TIMEOUT};
use crate::panels::{Key, step};
use crate::widgets::rows::{self, label_width};
use crate::widgets::{self, Add as _, icons};

pub const WIDTH: u16 = 170;

const INNER: u16 = WIDTH - 16;

/// How many networks the list shows.
pub const LISTED: usize = 7;

const PASSWORD: &str = "network-password";

#[derive(Debug, Clone, PartialEq)]
pub struct Wifi {
    pub ssid: String,
    /// 0 to 100.
    pub signal: u8,
    pub secured: bool,
    pub active: bool,
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wired {
    pub device: String,
    pub connected: bool,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Vpn {
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub wifi_device: Option<String>,
    pub wifi_enabled: bool,
    pub networks: Vec<Wifi>,
    pub wired: Vec<Wired>,
    pub vpns: Vec<Vpn>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Result<Snapshot, String>),
    /// Choose the network at this place in the list.
    Choose(usize),
    ToggleWifi,
    ToggleVpn(usize),
    Rescan,
    Typed(String),
    Submit,
    Cancel,
    Done(Result<String, String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Radio,
    Network(usize),
    Rescan,
    Vpn(usize),
}

/// What is asked for, as the calls it makes.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Connect {
        ssid: String,
        known: bool,
        password: Option<String>,
    },
    Disconnect {
        device: String,
    },
    Radio(bool),
    Vpn {
        name: String,
        up: bool,
    },
    Rescan,
}

#[derive(Debug, Default)]
struct Prompt {
    ssid: String,
    password: String,
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
    prompt: Option<Prompt>,
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
            Message::Choose(index) => self.choose(index, runner),
            Message::ToggleWifi => {
                self.focus_on(Item::Radio);

                if self.busy {
                    return Task::none();
                }

                let on = !self.snapshot.wifi_enabled;

                self.snapshot.wifi_enabled = on;

                self.act(Action::Radio(on), runner)
            }
            Message::ToggleVpn(index) => {
                self.focus_on(Item::Vpn(index));

                match self.snapshot.vpns.get(index) {
                    Some(vpn) => {
                        let action = Action::Vpn {
                            name: vpn.name.clone(),
                            up: !vpn.active,
                        };

                        self.act(action, runner)
                    }
                    None => Task::none(),
                }
            }
            Message::Rescan => {
                self.focus_on(Item::Rescan);

                if self.busy {
                    return Task::none();
                }

                self.notice = Some("SCANNING".into());

                self.act(Action::Rescan, runner)
            }
            Message::Typed(password) => {
                if let Some(prompt) = &mut self.prompt {
                    prompt.password = password;
                }

                Task::none()
            }
            Message::Submit => match self.prompt.take() {
                Some(prompt) if !prompt.password.is_empty() => self.act(
                    Action::Connect {
                        ssid: prompt.ssid,
                        known: false,
                        password: Some(prompt.password),
                    },
                    runner,
                ),
                other => {
                    self.prompt = other;

                    Task::none()
                }
            },
            Message::Cancel => {
                self.prompt = None;

                Task::none()
            }
            Message::Done(result) => {
                self.busy = false;

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

    /// Backs out of the password field; false if there was none to back out of.
    pub fn escape(&mut self) -> bool {
        self.prompt.take().is_some()
    }

    pub fn key(&mut self, key: Key, runner: &Shared) -> Task<Message> {
        let items = self.items();

        if items.is_empty() || self.prompt.is_some() {
            return Task::none();
        }

        let item = items[self.focus.min(items.len() - 1)];

        match key {
            Key::Up | Key::BackTab => self.focus = step(self.focus, items.len(), false),
            Key::Down | Key::Tab => self.focus = step(self.focus, items.len(), true),
            Key::Enter | Key::Space => {
                return match item {
                    Item::Radio => self.update(Message::ToggleWifi, runner),
                    Item::Network(i) => self.update(Message::Choose(i), runner),
                    Item::Rescan => self.update(Message::Rescan, runner),
                    Item::Vpn(i) => self.update(Message::ToggleVpn(i), runner),
                };
            }
            Key::Char('r') => return self.update(Message::Rescan, runner),
            Key::Home => self.focus = 0,
            Key::End => self.focus = items.len() - 1,
            _ => {}
        }

        Task::none()
    }

    fn items(&self) -> Vec<Item> {
        let mut items = Vec::new();

        if self.snapshot.wifi_device.is_some() {
            items.push(Item::Radio);

            if self.snapshot.wifi_enabled {
                items.extend((0..self.snapshot.networks.len().min(LISTED)).map(Item::Network));
                items.push(Item::Rescan);
            }
        }

        items.extend((0..self.snapshot.vpns.len()).map(Item::Vpn));
        items
    }

    fn focus_on(&mut self, item: Item) {
        if let Some(position) = self.items().iter().position(|i| *i == item) {
            self.focus = position;
        }
    }

    fn choose(&mut self, index: usize, runner: &Shared) -> Task<Message> {
        self.focus_on(Item::Network(index));

        let Some(network) = self.snapshot.networks.get(index).cloned() else {
            return Task::none();
        };

        if network.active {
            return match self.snapshot.wifi_device.clone() {
                Some(device) => self.act(Action::Disconnect { device }, runner),
                None => Task::none(),
            };
        }

        if network.secured && !network.known {
            self.prompt = Some(Prompt {
                ssid: network.ssid,
                password: String::new(),
            });

            return operation::focus(Id::new(PASSWORD));
        }

        self.act(
            Action::Connect {
                ssid: network.ssid,
                known: network.known,
                password: None,
            },
            runner,
        )
    }

    fn act(&mut self, action: Action, runner: &Shared) -> Task<Message> {
        if self.busy {
            return Task::none();
        }

        self.busy = true;

        if let Action::Connect { ssid, .. } = &action {
            self.notice = Some(format!("CONNECTING TO {ssid}"));
        }

        let calls = calls(&action);
        let runner = runner.clone();
        let wait = if matches!(action, Action::Connect { .. }) {
            LONG
        } else {
            TIMEOUT
        };

        Task::perform(
            commands::blocking(move || {
                for call in calls {
                    let args: Vec<&str> = call[1..].iter().map(String::as_str).collect();

                    runner.run(&call[0], &args, wait)?;
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

        // What is connected.
        let mut connection = column![].spacing(0);

        let active = self.snapshot.networks.iter().find(|network| network.active);

        if self.snapshot.wifi_device.is_some() {
            connection = connection.add(status_row(
                if self.snapshot.wifi_enabled {
                    icons::WIFI
                } else {
                    icons::WIFI_OFF
                },
                match (self.snapshot.wifi_enabled, active) {
                    (false, _) => "WI-FI OFF".to_owned(),
                    (true, Some(network)) => network.ssid.clone(),
                    (true, None) => "NOT CONNECTED".to_owned(),
                },
                active.map(|network| network.signal),
            ));
        }

        for wired in &self.snapshot.wired {
            connection = connection.add(status_row(
                icons::ETHERNET,
                format!(
                    "{} {}",
                    wired.device,
                    if wired.connected {
                        "CONNECTED"
                    } else {
                        "UNPLUGGED"
                    }
                ),
                None,
            ));
        }

        body = body.add(widget::group("CONNECTION", connection).width(Length::Fill));

        if self.snapshot.wifi_device.is_some() {
            let mut list = column![].spacing(0);

            list = list.add(widgets::marks(
                iced_widget::container(
                    widget::toggler("WI-FI", self.snapshot.wifi_enabled)
                        .on_toggle(|_| Message::ToggleWifi),
                )
                .width(Length::Fill)
                .padding([2.0, 0.0])
                .boxed(),
                focused(Item::Radio),
            ));

            if self.snapshot.wifi_enabled {
                let signal = 11 + 7 + px::GAP as u16;
                let named = label_width(INNER, true, signal);

                for (i, network) in self.snapshot.networks.iter().take(LISTED).enumerate() {
                    list = list.add(widgets::marks(
                        rows::choice(
                            network.active,
                            Some(if network.secured {
                                icons::LOCK
                            } else {
                                icons::WIFI
                            }),
                            rows::fit(&network.ssid, named),
                            Some(signal_steps(network.signal, network.active)),
                            Message::Choose(i),
                        ),
                        focused(Item::Network(i)),
                    ));
                }

                let more = self.snapshot.networks.len().saturating_sub(LISTED);

                list = list.add(widgets::marks(
                    row![
                        widget::button(if self.notice.is_some() {
                            "SCANNING"
                        } else {
                            "RESCAN"
                        })
                        .on_press(Message::Rescan),
                        space::horizontal(),
                        widget::label(if more > 0 {
                            format!("{more} MORE")
                        } else {
                            String::new()
                        })
                        .style(style::text::faint),
                    ]
                    .padding([2.0, 0.0])
                    .align_y(Alignment::Center)
                    .boxed(),
                    focused(Item::Rescan),
                ));

                if let Some(prompt) = &self.prompt {
                    list = list.add(
                        column![
                            widget::label(rows::fit(
                                &format!("PASSWORD FOR {}", prompt.ssid),
                                INNER
                            ))
                            .style(style::text::muted),
                            widgets::field(
                                PASSWORD,
                                "PASSWORD",
                                &prompt.password,
                                true,
                                Message::Typed,
                                Message::Submit,
                            ),
                            row![
                                widget::button("CONNECT").on_press(Message::Submit),
                                widget::button("CANCEL").on_press(Message::Cancel),
                            ]
                            .spacing(px::GAP),
                        ]
                        .spacing(px::TIGHT)
                        .padding([4.0, 0.0]),
                    );
                }
            }

            body = body.add(widget::group("WI-FI", list).width(Length::Fill));
        }

        if !self.snapshot.vpns.is_empty() {
            let mut vpns = column![].spacing(0);

            for (i, vpn) in self.snapshot.vpns.iter().enumerate() {
                vpns = vpns.add(widgets::marks(
                    rows::choice(
                        vpn.active,
                        Some(icons::SHIELD),
                        rows::fit(&vpn.name, label_width(INNER, true, 0)),
                        None,
                        Message::ToggleVpn(i),
                    ),
                    focused(Item::Vpn(i)),
                ));
            }

            body = body.add(widget::group("VPN", vpns).width(Length::Fill));
        }

        if let Some(error) = &self.error {
            body = body.add(widget::label(rows::fit(error, INNER)).style(style::text::alarm));
        } else if let Some(notice) = &self.notice {
            body = body.add(widget::label(rows::fit(notice, INNER)).style(style::text::muted));
        }

        body.boxed()
    }
}

/// How many of four steps a signal strength lights.
pub fn signal_level(signal: u8) -> u8 {
    match signal {
        75..=100 => 4,
        50..=74 => 3,
        25..=49 => 2,
        1..=24 => 1,
        0 => 0,
        _ => 4,
    }
}

/// The signal as four steps; knocked out of the row when it is the one in use.
fn signal_steps<'a>(signal: u8, inverse: bool) -> Element<'a, Message> {
    widgets::Steps::new(u16::from(signal_level(signal)), 4)
        .inverse(inverse)
        .boxed()
}

/// A line of what is connected: an icon, a name, and the signal if it has one.
fn status_row<'a>(
    sprite: quadrille::draw::Sprite,
    text: String,
    signal: Option<u8>,
) -> Element<'a, Message> {
    let named = label_width(INNER, true, if signal.is_some() { 11 } else { 0 });

    let mut line = row![
        widget::icon::<quadrille::Theme>(sprite),
        widget::label(rows::fit(&text, named)),
        space::horizontal(),
    ]
    .spacing(px::GAP)
    .padding([2.0, f32::from(rows::PADDING)])
    .align_y(Alignment::Center);

    if let Some(signal) = signal {
        line = line.add(signal_steps(signal, false));
    }

    line.boxed()
}

/// The calls an action makes.
pub fn calls(action: &Action) -> Vec<Vec<String>> {
    let call =
        |words: &[&str]| -> Vec<String> { words.iter().map(|word| (*word).to_owned()).collect() };

    match action {
        Action::Connect {
            ssid, known: true, ..
        } => {
            vec![call(&["nmcli", "connection", "up", "id", ssid])]
        }
        Action::Connect {
            ssid,
            password: Some(password),
            ..
        } => vec![call(&[
            "nmcli", "device", "wifi", "connect", ssid, "password", password,
        ])],
        Action::Connect { ssid, .. } => vec![call(&["nmcli", "device", "wifi", "connect", ssid])],
        Action::Disconnect { device } => vec![call(&["nmcli", "device", "disconnect", device])],
        Action::Radio(on) => vec![call(&[
            "nmcli",
            "radio",
            "wifi",
            if *on { "on" } else { "off" },
        ])],
        Action::Vpn { name, up } => vec![call(&[
            "nmcli",
            "connection",
            if *up { "up" } else { "down" },
            "id",
            name,
        ])],
        Action::Rescan => vec![call(&["nmcli", "device", "wifi", "rescan"])],
    }
}

/// Reads the devices, the radio, the networks in range and the connections.
pub fn read(runner: &dyn commands::Runner) -> Result<Snapshot, String> {
    let devices = runner.run(
        "nmcli",
        &[
            "-t",
            "-f",
            "DEVICE,TYPE,STATE,CONNECTION",
            "device",
            "status",
        ],
        TIMEOUT,
    )?;

    let (wifi_device, wired) = parse_devices(&devices);

    let wifi_enabled = wifi_device.is_some()
        && runner
            .run("nmcli", &["radio", "wifi"], TIMEOUT)
            .is_ok_and(|radio| radio.trim() == "enabled");

    let connections = runner
        .run(
            "nmcli",
            &["-t", "-f", "NAME,TYPE", "connection", "show"],
            TIMEOUT,
        )
        .unwrap_or_default();

    let active = runner
        .run(
            "nmcli",
            &["-t", "-f", "NAME,TYPE", "connection", "show", "--active"],
            TIMEOUT,
        )
        .unwrap_or_default();

    let known: HashSet<String> = fields(&connections)
        .filter(|fields| fields.get(1).is_some_and(|kind| kind == "802-11-wireless"))
        .map(|fields| fields[0].clone())
        .collect();

    let networks = if wifi_enabled {
        let list = runner
            .run(
                "nmcli",
                &[
                    "-t",
                    "-f",
                    "IN-USE,SSID,SIGNAL,SECURITY",
                    "device",
                    "wifi",
                    "list",
                    "--rescan",
                    "no",
                ],
                TIMEOUT,
            )
            .unwrap_or_default();

        parse_networks(&list, &known)
    } else {
        Vec::new()
    };

    Ok(Snapshot {
        wifi_device,
        wifi_enabled,
        networks,
        wired,
        vpns: parse_vpns(&connections, &active),
    })
}

/// The lines of terse `nmcli` output as their fields.
fn fields(output: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    output
        .lines()
        .filter(|line| !line.is_empty())
        .map(split_terse)
}

/// Splits a line of `nmcli -t` on its colons: a colon in a value is written
/// `\:` and a backslash `\\`.
pub fn split_terse(line: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut chars = line.chars();

    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    fields.last_mut().expect("A field").push(escaped);
                }
            }
            ':' => fields.push(String::new()),
            other => fields.last_mut().expect("A field").push(other),
        }
    }

    fields
}

/// The Wi-Fi device (the one that is not peer to peer) and the ethernet ones
/// that are physical: bridges, veths and the rest are not shown.
pub fn parse_devices(status: &str) -> (Option<String>, Vec<Wired>) {
    let mut wifi = None;
    let mut wired = Vec::new();

    for fields in fields(status) {
        let [device, kind, state, ..] = fields.as_slice() else {
            continue;
        };

        match kind.as_str() {
            "wifi" if wifi.is_none() => wifi = Some(device.clone()),
            "ethernet" if state != "unmanaged" => wired.push(Wired {
                device: device.clone(),
                connected: state.starts_with("connected"),
                state: state.clone(),
            }),
            _ => {}
        }
    }

    (wifi, wired)
}

/// The networks in range, the strongest of each name, the one in use first,
/// the others by signal; hidden networks (with no name) are left out.
pub fn parse_networks(list: &str, known: &HashSet<String>) -> Vec<Wifi> {
    let mut networks: Vec<Wifi> = Vec::new();

    for fields in fields(list) {
        let [in_use, ssid, signal, security, ..] = fields.as_slice() else {
            continue;
        };

        if ssid.is_empty() {
            continue;
        }

        let network = Wifi {
            ssid: ssid.clone(),
            signal: signal.parse().unwrap_or(0),
            secured: !security.is_empty() && security != "--",
            active: in_use == "*",
            known: known.contains(ssid),
        };

        match networks.iter_mut().find(|other| other.ssid == network.ssid) {
            Some(other) => {
                if network.active || (!other.active && network.signal > other.signal) {
                    *other = network;
                }
            }
            None => networks.push(network),
        }
    }

    networks.sort_by(|a, b| b.active.cmp(&a.active).then(b.signal.cmp(&a.signal)));
    networks
}

/// The VPN and WireGuard connections there are, and which are up.
pub fn parse_vpns(all: &str, active: &str) -> Vec<Vpn> {
    let vpn = |kind: &str| kind == "vpn" || kind == "wireguard";

    let up: HashSet<String> = fields(active)
        .filter(|fields| fields.get(1).is_some_and(|kind| vpn(kind)))
        .map(|fields| fields[0].clone())
        .collect();

    fields(all)
        .filter(|fields| fields.get(1).is_some_and(|kind| vpn(kind)))
        .map(|fields| Vpn {
            active: up.contains(&fields[0]),
            name: fields[0].clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::recorder::Recorder;

    const DEVICES: &str = "\
wlp1s0:wifi:connected:Home
br-0123:bridge:connected (externally):br-0123
docker0:bridge:connected (externally):docker0
lo:loopback:connected (externally):lo
p2p-dev-wlp1s0:wifi-p2p:disconnected:
enp2s0:ethernet:unavailable:
veth1234:ethernet:unmanaged:
";

    const WIFI: &str = "\
 :Cafe\\:Free:62:--
*:Home:81:WPA2
 :Home:44:WPA2
 :Neighbour:35:WPA2 WPA3
 ::90:WPA2
 :Known Away:20:WPA2
 :Lobby:70:--
";

    const CONNECTIONS: &str = "\
Home:802-11-wireless
Known Away:802-11-wireless
Work VPN:vpn
wg0:wireguard
docker0:bridge
";

    const ACTIVE: &str = "Home:802-11-wireless\nwg0:wireguard\n";

    fn known() -> HashSet<String> {
        ["Home".to_owned(), "Known Away".to_owned()].into()
    }

    fn recorder() -> (std::sync::Arc<Recorder>, Shared) {
        let (recorder, shared) = Recorder::shared();

        recorder.answer(
            "nmcli -t -f DEVICE,TYPE,STATE,CONNECTION device status",
            Ok(DEVICES),
        );
        recorder.answer("nmcli radio wifi", Ok("enabled\n"));
        // The more specific answer last: the last one that fits wins.
        recorder.answer("nmcli -t -f NAME,TYPE connection show", Ok(CONNECTIONS));
        recorder.answer("nmcli -t -f NAME,TYPE connection show --active", Ok(ACTIVE));
        recorder.answer("nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY", Ok(WIFI));

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
    fn colons_in_a_value_are_escaped() {
        assert_eq!(split_terse("a:b\\:c:d"), ["a", "b:c", "d"]);
        assert_eq!(split_terse("a\\\\b:"), ["a\\b", ""]);
        assert_eq!(
            split_terse("*:Cafe\\:Free:62:--"),
            ["*", "Cafe:Free", "62", "--"]
        );
    }

    #[test]
    fn the_wifi_device_and_the_physical_ethernet_are_found() {
        let (wifi, wired) = parse_devices(DEVICES);

        assert_eq!(wifi.as_deref(), Some("wlp1s0"));
        assert_eq!(
            wired,
            [Wired {
                device: "enp2s0".into(),
                connected: false,
                state: "unavailable".into()
            }]
        );
    }

    #[test]
    fn networks_are_ordered_by_use_then_signal_and_each_name_once() {
        let networks = parse_networks(WIFI, &known());
        let names: Vec<_> = networks.iter().map(|n| n.ssid.as_str()).collect();

        // The hidden one is left out, Home is once (the one in use), the
        // rest by signal.
        assert_eq!(
            names,
            ["Home", "Lobby", "Cafe:Free", "Neighbour", "Known Away"]
        );

        assert!(networks[0].active && networks[0].known && networks[0].secured);
        assert_eq!(
            networks[0].signal, 81,
            "the one in use, not the weaker copy"
        );
        assert!(!networks[1].secured, "-- is no security");
        assert!(networks[4].known && !networks[4].active);
    }

    #[test]
    fn signal_is_four_steps() {
        assert_eq!(signal_level(100), 4);
        assert_eq!(signal_level(75), 4);
        assert_eq!(signal_level(74), 3);
        assert_eq!(signal_level(50), 3);
        assert_eq!(signal_level(49), 2);
        assert_eq!(signal_level(25), 2);
        assert_eq!(signal_level(24), 1);
        assert_eq!(signal_level(1), 1);
        assert_eq!(signal_level(0), 0);
    }

    #[test]
    fn vpns_are_the_profiles_and_whether_they_are_up() {
        let vpns = parse_vpns(CONNECTIONS, ACTIVE);

        assert_eq!(
            vpns,
            [
                Vpn {
                    name: "Work VPN".into(),
                    active: false
                },
                Vpn {
                    name: "wg0".into(),
                    active: true
                },
            ]
        );
    }

    #[test]
    fn the_snapshot_has_it_all() {
        let snapshot = read(&*recorder().1).unwrap();

        assert!(snapshot.wifi_enabled);
        assert_eq!(snapshot.networks.len(), 5);
        assert_eq!(snapshot.wired.len(), 1);
        assert_eq!(snapshot.vpns.len(), 2);
    }

    #[test]
    fn no_network_manager_is_an_error() {
        let (recorder, shared) = Recorder::shared();
        recorder.answer("nmcli", Err("NetworkManager is not running."));

        assert_eq!(
            read(&*shared).unwrap_err(),
            "NetworkManager is not running."
        );
    }

    fn call_lines(action: Action) -> Vec<String> {
        calls(&action).iter().map(|call| call.join(" ")).collect()
    }

    #[test]
    fn what_is_sent_for_each_action() {
        assert_eq!(
            call_lines(Action::Connect {
                ssid: "Home".into(),
                known: true,
                password: None
            }),
            ["nmcli connection up id Home"]
        );
        assert_eq!(
            call_lines(Action::Connect {
                ssid: "Lobby".into(),
                known: false,
                password: None
            }),
            ["nmcli device wifi connect Lobby"]
        );
        assert_eq!(
            call_lines(Action::Connect {
                ssid: "Neighbour".into(),
                known: false,
                password: Some("hunter2".into())
            }),
            ["nmcli device wifi connect Neighbour password hunter2"]
        );
        assert_eq!(
            call_lines(Action::Disconnect {
                device: "wlp1s0".into()
            }),
            ["nmcli device disconnect wlp1s0"]
        );
        assert_eq!(call_lines(Action::Radio(false)), ["nmcli radio wifi off"]);
        assert_eq!(call_lines(Action::Radio(true)), ["nmcli radio wifi on"]);
        assert_eq!(
            call_lines(Action::Vpn {
                name: "Work VPN".into(),
                up: true
            }),
            ["nmcli connection up id Work VPN"]
        );
        assert_eq!(call_lines(Action::Rescan), ["nmcli device wifi rescan"]);
    }

    #[test]
    fn choosing_a_secured_new_network_asks_for_the_password() {
        let (_, shared, mut state) = loaded();

        // Order: Home (in use), Lobby, Cafe:Free, Neighbour, Known Away.
        let _ = state.update(Message::Choose(3), &shared);

        assert_eq!(
            state.prompt.as_ref().map(|p| p.ssid.as_str()),
            Some("Neighbour")
        );
        assert!(!state.busy, "nothing was sent: it waits for a password");

        // An empty password is not sent.
        let _ = state.update(Message::Submit, &shared);
        assert!(state.prompt.is_some() && !state.busy);

        let _ = state.update(Message::Typed("hunter2".into()), &shared);
        let _ = state.update(Message::Submit, &shared);

        assert!(state.prompt.is_none());
        assert!(state.busy);
    }

    #[test]
    fn escape_backs_out_of_the_password_before_it_closes_anything() {
        let (_, shared, mut state) = loaded();

        assert!(!state.escape(), "nothing to back out of");

        let _ = state.update(Message::Choose(3), &shared);

        assert!(state.escape());
        assert!(state.prompt.is_none());
        assert!(!state.escape());
    }

    #[test]
    fn the_network_in_use_is_disconnected_and_a_saved_one_is_joined() {
        let (_, shared, mut state) = loaded();

        let _ = state.update(Message::Choose(0), &shared);
        assert!(state.busy, "disconnecting");

        state.busy = false;
        let _ = state.update(Message::Choose(4), &shared);
        assert!(state.busy, "Known Away is saved: no password");
        assert!(state.prompt.is_none());
    }

    #[test]
    fn the_keyboard_walks_the_list_and_never_types_into_the_field() {
        let (_, shared, mut state) = loaded();

        // Rows: the radio, five networks (all shown), rescan, two VPNs.
        assert_eq!(state.items().len(), 1 + 5 + 1 + 2);

        let _ = state.key(Key::Down, &shared);
        assert_eq!(state.focus, 1);

        let _ = state.key(Key::End, &shared);
        assert_eq!(state.focus, state.items().len() - 1);

        // With the password field open, keys belong to the field.
        state.prompt = Some(Prompt::default());
        let before = state.focus;
        let _ = state.key(Key::Up, &shared);
        assert_eq!(state.focus, before);
    }

    #[test]
    fn with_wifi_off_only_the_radio_and_the_vpns_are_rows() {
        let (_, shared, mut state) = loaded();

        let _ = state.update(Message::ToggleWifi, &shared);

        assert!(!state.snapshot.wifi_enabled);
        assert_eq!(state.items(), [Item::Radio, Item::Vpn(0), Item::Vpn(1)]);
    }
}
