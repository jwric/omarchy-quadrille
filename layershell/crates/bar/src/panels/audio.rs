//! Audio: where the sound goes and where it comes from, how loud, and how
//! loud each application is. Read and set with `pactl` (PipeWire answers it,
//! and it speaks JSON, so there is nothing to scrape).
//!
//! A device row chooses the default; a volume row is a stepped slider (twenty
//! steps of 5 %) with a mute button; an application is a volume row of its
//! own. The keyboard walks the rows (arrows, Tab), chooses with Enter, moves a
//! slider with Left and Right, mutes with M.
use std::time::{Duration, Instant};

use iced_core::Length;
use iced_runtime::Task;
use iced_widget::core::Alignment;
use iced_widget::{Widget as _, column, row, space};

use quadrille::{Element, px, style, widget};

use crate::commands::{self, Call, Shared, TIMEOUT};
use crate::panels::{Key, moved, step};
use crate::widgets::rows::{self, label_width};
use crate::widgets::slider::{percent_of, step_of};
use crate::widgets::{self, Add as _, Slider, icons};

/// The width of the panel, in virtual pixels.
pub const WIDTH: u16 = 170;

const INNER: u16 = WIDTH - 16;

/// The steps of a volume slider: 5 % each.
pub const STEPS: u16 = 20;

/// How long after the user moves something the machine's word on it is not
/// taken: it is still catching up.
const SETTLE: Duration = Duration::from_millis(1200);

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Target {
    Sink(String),
    Source(String),
    Stream(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Speaker,
    Headphones,
    Bluetooth,
    Mic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub name: String,
    pub label: String,
    pub volume: u8,
    pub muted: bool,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    pub index: u32,
    pub app: String,
    pub volume: u8,
    pub muted: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub sinks: Vec<Device>,
    pub sources: Vec<Device>,
    pub streams: Vec<Stream>,
    pub default_sink: Option<String>,
    pub default_source: Option<String>,
}

/// What the user asked for.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Default(Target),
    Volume(Target, u8),
    Mute(Target),
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Result<Snapshot, String>),
    Chose(Target),
    Volume(Target, u8),
    Mute(Target),
    Written(Result<(), String>),
}

/// A row of the keyboard's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Sink(usize),
    SinkVolume,
    Source(usize),
    SourceVolume,
    Stream(usize),
}

#[derive(Debug, Default)]
pub struct State {
    snapshot: Snapshot,
    loaded: bool,
    error: Option<String>,
    focus: usize,
    reading: bool,
    writing: bool,
    queue: Vec<Action>,
    touched: Option<Instant>,
}

impl State {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Reads the machine, unless a reading is on its way.
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
                        let settling = self.writing
                            || !self.queue.is_empty()
                            || self.touched.is_some_and(|at| at.elapsed() < SETTLE);

                        // While the user's changes are catching up, the
                        // machine's volumes are the old ones.
                        if self.loaded && settling {
                            self.merge_structure(snapshot);
                        } else {
                            self.snapshot = snapshot;
                        }

                        self.loaded = true;
                        self.error = None;
                        self.clamp();
                    }
                    Err(error) => self.error = Some(error),
                }

                Task::none()
            }
            Message::Chose(target) => {
                self.focus_on(&target, true);
                self.choose(&target);
                self.write(Action::Default(target), runner)
            }
            Message::Volume(target, percent) => {
                self.focus_on(&target, false);
                self.set_volume(&target, percent);
                self.write(Action::Volume(target, percent), runner)
            }
            Message::Mute(target) => {
                self.focus_on(&target, false);
                self.toggle_mute(&target);
                self.write(Action::Mute(target), runner)
            }
            Message::Written(result) => {
                self.writing = false;

                if let Err(error) = result {
                    self.error = Some(error);
                }

                if self.queue.is_empty() {
                    self.refresh(runner)
                } else {
                    self.flush(runner)
                }
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
            Key::Enter | Key::Space => {
                return match item {
                    Item::Sink(i) => self.update(
                        Message::Chose(Target::Sink(self.snapshot.sinks[i].name.clone())),
                        runner,
                    ),
                    Item::Source(i) => self.update(
                        Message::Chose(Target::Source(self.snapshot.sources[i].name.clone())),
                        runner,
                    ),
                    _ => self.mute_item(item, runner),
                };
            }
            Key::Char('m') => return self.mute_item(item, runner),
            Key::Left | Key::Right | Key::PageUp | Key::PageDown | Key::Home | Key::End => {
                if let Some((target, percent)) = self.volume_of(item) {
                    let to = match key {
                        Key::Left => moved(percent, -1, STEPS),
                        Key::Right => moved(percent, 1, STEPS),
                        Key::PageDown => moved(percent, -4, STEPS),
                        Key::PageUp => moved(percent, 4, STEPS),
                        Key::Home => 0,
                        _ => 100,
                    };

                    if to != percent {
                        return self.update(Message::Volume(target, to), runner);
                    }
                }
            }
            Key::Char(_) => {}
        }

        Task::none()
    }

    fn mute_item(&mut self, item: Item, runner: &Shared) -> Task<Message> {
        match self.volume_of(item) {
            Some((target, _)) => self.update(Message::Mute(target), runner),
            None => Task::none(),
        }
    }

    /// The rows the keyboard goes through, in the order they are drawn.
    fn items(&self) -> Vec<Item> {
        let mut items: Vec<Item> = (0..self.snapshot.sinks.len()).map(Item::Sink).collect();

        if self.default_sink().is_some() {
            items.push(Item::SinkVolume);
        }

        items.extend((0..self.snapshot.sources.len()).map(Item::Source));

        if self.default_source().is_some() {
            items.push(Item::SourceVolume);
        }

        items.extend((0..self.snapshot.streams.len()).map(Item::Stream));
        items
    }

    fn default_sink(&self) -> Option<&Device> {
        let name = self.snapshot.default_sink.as_deref()?;

        self.snapshot
            .sinks
            .iter()
            .find(|device| device.name == name)
    }

    fn default_source(&self) -> Option<&Device> {
        let name = self.snapshot.default_source.as_deref()?;

        self.snapshot
            .sources
            .iter()
            .find(|device| device.name == name)
    }

    /// The target and the volume of a row that is a volume.
    fn volume_of(&self, item: Item) -> Option<(Target, u8)> {
        match item {
            Item::SinkVolume => {
                let device = self.default_sink()?;

                Some((Target::Sink(device.name.clone()), device.volume))
            }
            Item::SourceVolume => {
                let device = self.default_source()?;

                Some((Target::Source(device.name.clone()), device.volume))
            }
            Item::Stream(i) => {
                let stream = self.snapshot.streams.get(i)?;

                Some((Target::Stream(stream.index), stream.volume))
            }
            _ => None,
        }
    }

    /// Puts the keyboard where the pointer has just acted: on the device row
    /// when `device`, on the volume row otherwise.
    fn focus_on(&mut self, target: &Target, device: bool) {
        let item = match target {
            Target::Sink(name) if device => self
                .snapshot
                .sinks
                .iter()
                .position(|sink| &sink.name == name)
                .map(Item::Sink),
            Target::Source(name) if device => self
                .snapshot
                .sources
                .iter()
                .position(|source| &source.name == name)
                .map(Item::Source),
            Target::Sink(_) => Some(Item::SinkVolume),
            Target::Source(_) => Some(Item::SourceVolume),
            Target::Stream(index) => self
                .snapshot
                .streams
                .iter()
                .position(|stream| stream.index == *index)
                .map(Item::Stream),
        };

        if let Some(position) = item.and_then(|item| self.items().iter().position(|i| *i == item)) {
            self.focus = position;
        }
    }

    fn clamp(&mut self) {
        let count = self.items().len();

        self.focus = self.focus.min(count.saturating_sub(1));
    }

    /// Takes the devices and streams of a reading, and keeps the volumes the
    /// user has just set.
    fn merge_structure(&mut self, mut new: Snapshot) {
        let keep = |old: &[Device], new: &mut [Device]| {
            for device in new {
                if let Some(old) = old.iter().find(|old| old.name == device.name) {
                    device.volume = old.volume;
                    device.muted = old.muted;
                }
            }
        };

        keep(&self.snapshot.sinks, &mut new.sinks);
        keep(&self.snapshot.sources, &mut new.sources);

        for stream in &mut new.streams {
            if let Some(old) = self
                .snapshot
                .streams
                .iter()
                .find(|old| old.index == stream.index)
            {
                stream.volume = old.volume;
                stream.muted = old.muted;
            }
        }

        new.default_sink = self.snapshot.default_sink.clone().or(new.default_sink);
        new.default_source = self.snapshot.default_source.clone().or(new.default_source);

        self.snapshot = new;
    }

    fn choose(&mut self, target: &Target) {
        match target {
            Target::Sink(name) => self.snapshot.default_sink = Some(name.clone()),
            Target::Source(name) => self.snapshot.default_source = Some(name.clone()),
            Target::Stream(_) => {}
        }
    }

    fn set_volume(&mut self, target: &Target, percent: u8) {
        if let Some((volume, _)) = self.slot(target) {
            *volume = percent;
        }
    }

    fn toggle_mute(&mut self, target: &Target) {
        if let Some((_, muted)) = self.slot(target) {
            *muted = !*muted;
        }
    }

    fn slot(&mut self, target: &Target) -> Option<(&mut u8, &mut bool)> {
        match target {
            Target::Sink(name) => self
                .snapshot
                .sinks
                .iter_mut()
                .find(|device| &device.name == name)
                .map(|device| (&mut device.volume, &mut device.muted)),
            Target::Source(name) => self
                .snapshot
                .sources
                .iter_mut()
                .find(|device| &device.name == name)
                .map(|device| (&mut device.volume, &mut device.muted)),
            Target::Stream(index) => self
                .snapshot
                .streams
                .iter_mut()
                .find(|stream| stream.index == *index)
                .map(|stream| (&mut stream.volume, &mut stream.muted)),
        }
    }

    /// Queues an action; a volume replaces the volume of the same thing that
    /// is waiting, so dragging a slider sends the last place it stopped at
    /// and not every place it passed.
    fn write(&mut self, action: Action, runner: &Shared) -> Task<Message> {
        self.touched = Some(Instant::now());

        if let Action::Volume(target, _) = &action {
            self.queue
                .retain(|queued| !matches!(queued, Action::Volume(other, _) if other == target));
        }

        self.queue.push(action);

        if self.writing {
            Task::none()
        } else {
            self.flush(runner)
        }
    }

    fn flush(&mut self, runner: &Shared) -> Task<Message> {
        let actions = std::mem::take(&mut self.queue);

        if actions.is_empty() {
            return Task::none();
        }

        self.writing = true;

        let calls: Vec<Call> = actions
            .iter()
            .flat_map(|action| calls(action, &self.snapshot))
            .collect();

        let runner = runner.clone();

        Task::perform(
            commands::blocking(move || {
                let mut result = Ok(());

                for call in calls {
                    let args: Vec<&str> = call[1..].iter().map(String::as_str).collect();

                    if let Err(error) = runner.run(&call[0], &args, TIMEOUT) {
                        result = Err(error);
                    }
                }

                result
            }),
            Message::Written,
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

        let named = label_width(INNER, true, 0);

        let mut output = column![].spacing(0);

        for (i, device) in self.snapshot.sinks.iter().enumerate() {
            output = output.add(widgets::marks(
                rows::choice(
                    self.snapshot.default_sink.as_deref() == Some(device.name.as_str()),
                    Some(icon_of(device.kind)),
                    rows::fit(&device.label, named),
                    None,
                    Message::Chose(Target::Sink(device.name.clone())),
                ),
                focused(Item::Sink(i)),
            ));
        }

        if let Some(device) = self.default_sink() {
            output = output.add(widgets::marks(
                volume_row(
                    Target::Sink(device.name.clone()),
                    device.volume,
                    device.muted,
                    icons::SPEAKER,
                    icons::SPEAKER_MUTED,
                ),
                focused(Item::SinkVolume),
            ));
        }

        let mut input = column![].spacing(0);

        for (i, device) in self.snapshot.sources.iter().enumerate() {
            input = input.add(widgets::marks(
                rows::choice(
                    self.snapshot.default_source.as_deref() == Some(device.name.as_str()),
                    Some(icon_of(device.kind)),
                    rows::fit(&device.label, named),
                    None,
                    Message::Chose(Target::Source(device.name.clone())),
                ),
                focused(Item::Source(i)),
            ));
        }

        if let Some(device) = self.default_source() {
            input = input.add(widgets::marks(
                volume_row(
                    Target::Source(device.name.clone()),
                    device.volume,
                    device.muted,
                    icons::MIC,
                    icons::MIC_MUTED,
                ),
                focused(Item::SourceVolume),
            ));
        }

        let mut streams = column![].spacing(0);

        if self.snapshot.streams.is_empty() {
            streams = streams.add(widget::label("NOTHING PLAYING").style(style::text::faint));
        }

        for (i, stream) in self.snapshot.streams.iter().enumerate() {
            streams = streams.add(widgets::marks(stream_row(stream), focused(Item::Stream(i))));
        }

        let mut body = column![
            widget::group("OUTPUT", output).width(Length::Fill),
            widget::group("INPUT", input).width(Length::Fill),
            widget::group("APPLICATIONS", streams).width(Length::Fill),
        ]
        .spacing(px::GAP);

        if let Some(error) = &self.error {
            body = body.add(widget::label(rows::fit(error, INNER)).style(style::text::alarm));
        }

        body.boxed()
    }
}

fn icon_of(kind: Kind) -> quadrille::draw::Sprite {
    match kind {
        Kind::Speaker => icons::SPEAKER,
        Kind::Headphones => icons::HEADPHONES,
        Kind::Bluetooth => icons::BLUETOOTH,
        Kind::Mic => icons::MIC,
    }
}

/// The master volume of a device: a button that mutes, a slider, a reading.
fn volume_row<'a>(
    target: Target,
    percent: u8,
    muted: bool,
    sound: quadrille::draw::Sprite,
    silence: quadrille::draw::Sprite,
) -> Element<'a, Message> {
    let mute = iced_widget::button(widget::icon::<quadrille::Theme>(if muted {
        silence
    } else {
        sound
    }))
    .padding(2)
    .style(if muted {
        style::button::default
    } else {
        style::button::ghost
    })
    .on_press(Message::Mute(target.clone()));

    let slider_target = target;

    row![
        mute,
        Slider::new(STEPS, step_of(percent, STEPS), move |step| {
            Message::Volume(slider_target.clone(), percent_of(step, STEPS))
        })
        .segment(3)
        .muted(muted),
        widget::label(format!("{percent:>3}%")).style(if muted {
            style::text::faint
        } else {
            style::text::default
        }),
    ]
    .spacing(px::GAP)
    .padding([0.0, f32::from(rows::PADDING)])
    .align_y(Alignment::Center)
    .boxed()
}

/// An application: its name, a slider, a reading.
fn stream_row<'a>(stream: &Stream) -> Element<'a, Message> {
    let target = Target::Stream(stream.index);
    let slider_target = target.clone();

    // What is left of the row after the slider and the reading.
    let name = INNER - 2 * rows::PADDING - 59 - 24 - 2 * px::GAP as u16;

    let label = widget::label(rows::fit(&stream.app, name)).style(if stream.muted {
        style::text::faint
    } else {
        style::text::default
    });

    row![
        iced_widget::button(label)
            .padding(0)
            .width(Length::Fixed(f32::from(name)))
            .style(style::button::ghost)
            .on_press(Message::Mute(target)),
        space::horizontal(),
        Slider::new(STEPS, step_of(stream.volume, STEPS), move |step| {
            Message::Volume(slider_target.clone(), percent_of(step, STEPS))
        })
        .segment(2)
        .muted(stream.muted),
        widget::label(format!("{:>3}%", stream.volume)),
    ]
    .spacing(px::GAP)
    .padding([2.0, f32::from(rows::PADDING)])
    .align_y(Alignment::Center)
    .boxed()
}

/// The calls an action makes.
pub fn calls(action: &Action, snapshot: &Snapshot) -> Vec<Call> {
    let call = |words: &[&str]| -> Call { words.iter().map(|word| (*word).to_owned()).collect() };

    match action {
        Action::Default(Target::Sink(name)) => {
            let mut calls = vec![call(&["pactl", "set-default-sink", name])];

            // Streams that follow the default follow it; the ones that were
            // moved by hand are moved with it, as Omarchy's own switch does.
            for stream in &snapshot.streams {
                calls.push(call(&[
                    "pactl",
                    "move-sink-input",
                    &stream.index.to_string(),
                    name,
                ]));
            }

            calls
        }
        Action::Default(Target::Source(name)) => vec![call(&["pactl", "set-default-source", name])],
        Action::Default(Target::Stream(_)) => Vec::new(),
        Action::Volume(target, percent) => {
            let volume = format!("{}%", (*percent).min(100));

            vec![match target {
                Target::Sink(name) => call(&["pactl", "set-sink-volume", name, &volume]),
                Target::Source(name) => call(&["pactl", "set-source-volume", name, &volume]),
                Target::Stream(index) => call(&[
                    "pactl",
                    "set-sink-input-volume",
                    &index.to_string(),
                    &volume,
                ]),
            }]
        }
        Action::Mute(target) => vec![match target {
            Target::Sink(name) => call(&["pactl", "set-sink-mute", name, "toggle"]),
            Target::Source(name) => call(&["pactl", "set-source-mute", name, "toggle"]),
            Target::Stream(index) => {
                call(&["pactl", "set-sink-input-mute", &index.to_string(), "toggle"])
            }
        }],
    }
}

/// Reads sinks, sources, streams and the defaults.
pub fn read(runner: &dyn commands::Runner) -> Result<Snapshot, String> {
    let sinks = runner.run("pactl", &["-f", "json", "list", "sinks"], TIMEOUT)?;
    let sources = runner.run("pactl", &["-f", "json", "list", "sources"], TIMEOUT)?;
    let streams = runner
        .run("pactl", &["-f", "json", "list", "sink-inputs"], TIMEOUT)
        .unwrap_or_default();

    let default = |what: &str| {
        runner
            .run("pactl", &[what], TIMEOUT)
            .ok()
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
    };

    Ok(Snapshot {
        sinks: parse_devices(&sinks, false)?,
        sources: parse_devices(&sources, true)?,
        streams: if streams.trim().is_empty() {
            Vec::new()
        } else {
            parse_streams(&streams)?
        },
        default_sink: default("get-default-sink"),
        default_source: default("get-default-source"),
    })
}

/// The average of the channels' volumes, in percent.
fn volume_of(value: &serde_json::Value) -> u8 {
    let Some(channels) = value.as_object() else {
        return 0;
    };

    let percents: Vec<u32> = channels
        .values()
        .filter_map(|channel| {
            channel
                .get("value_percent")?
                .as_str()?
                .trim_end_matches('%')
                .parse()
                .ok()
        })
        .collect();

    if percents.is_empty() {
        0
    } else {
        (percents.iter().sum::<u32>() / percents.len() as u32).min(100) as u8
    }
}

/// The devices of `pactl -f json list sinks` or `... sources`: for sources
/// the monitors of the sinks are left out, and for sinks the ones whose
/// active port says there is nothing plugged into it (a monitor's HDMI port),
/// as Omarchy's own list leaves them out.
pub fn parse_devices(json: &str, sources: bool) -> Result<Vec<Device>, String> {
    let list: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|error| format!("pactl: {error}"))?;

    Ok(list
        .iter()
        .filter_map(|device| {
            let name = device.get("name")?.as_str()?.to_owned();
            let properties = device.get("properties");

            let property = |key: &str| {
                properties
                    .and_then(|properties| properties.get(key))
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_lowercase()
            };

            if sources && (name.ends_with(".monitor") || property("device.class") == "monitor") {
                return None;
            }

            if !sources {
                let active = device.get("active_port").and_then(|port| port.as_str());
                let unplugged = device
                    .get("ports")
                    .and_then(|ports| ports.as_array())
                    .into_iter()
                    .flatten()
                    .any(|port| {
                        port.get("name").and_then(|port| port.as_str()) == active
                            && port.get("availability").and_then(|a| a.as_str())
                                == Some("not available")
                    });

                if unplugged {
                    return None;
                }
            }

            let label = device
                .get("description")
                .and_then(|description| description.as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| name.clone());

            let blob = format!(
                "{} {} {} {}",
                name.to_lowercase(),
                label.to_lowercase(),
                property("device.form_factor"),
                property("device.icon_name"),
            );

            let kind = if blob.contains("headset")
                || blob.contains("headphone")
                || blob.contains("earbud")
                || blob.contains("airpod")
            {
                Kind::Headphones
            } else if property("device.bus") == "bluetooth" || blob.contains("bluez") {
                Kind::Bluetooth
            } else if sources {
                Kind::Mic
            } else {
                Kind::Speaker
            };

            Some(Device {
                name,
                label: friendly(&label),
                volume: volume_of(device.get("volume")?),
                muted: device.get("mute")?.as_bool()?,
                kind,
            })
        })
        .collect())
}

/// A device's name as the shell shows it: without the controller it is on.
fn friendly(label: &str) -> String {
    let label = label.trim();

    for prefix in ["Meteor Lake-P HD Audio Controller ", "Built-in Audio "] {
        if let Some(rest) = label.strip_prefix(prefix) {
            return rest.trim().to_owned();
        }
    }

    label.to_owned()
}

/// The playback streams of `pactl -f json list sink-inputs`, the ones of
/// applications (with a name) only.
pub fn parse_streams(json: &str) -> Result<Vec<Stream>, String> {
    let list: Vec<serde_json::Value> =
        serde_json::from_str(json).map_err(|error| format!("pactl: {error}"))?;

    Ok(list
        .iter()
        .filter_map(|stream| {
            let properties = stream.get("properties")?;
            let app = properties
                .get("application.name")
                .or_else(|| properties.get("media.name"))?
                .as_str()?
                .to_owned();

            Some(Stream {
                index: stream.get("index")?.as_u64()? as u32,
                app,
                volume: volume_of(stream.get("volume")?),
                muted: stream.get("mute")?.as_bool()?,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::recorder::Recorder;

    const SINKS: &str = include_str!("../../fixtures/pactl-sinks.json");
    const SOURCES: &str = include_str!("../../fixtures/pactl-sources.json");
    const STREAMS: &str = include_str!("../../fixtures/pactl-sink-inputs.json");

    fn recorder() -> (std::sync::Arc<Recorder>, Shared) {
        let (recorder, shared) = Recorder::shared();

        recorder.answer("pactl -f json list sinks", Ok(SINKS));
        recorder.answer("pactl -f json list sources", Ok(SOURCES));
        recorder.answer("pactl -f json list sink-inputs", Ok(STREAMS));
        recorder.answer("pactl get-default-sink", Ok("sink.speakers\n"));
        recorder.answer("pactl get-default-source", Ok("source.mic\n"));

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
    fn sinks_are_read_with_their_volume_and_kind() {
        let snapshot = read(&*recorder().1).unwrap();
        let names: Vec<_> = snapshot.sinks.iter().map(|d| d.name.as_str()).collect();

        // The HDMI sink with nothing plugged in is not offered.
        assert_eq!(names, ["sink.speakers", "sink.headset", "sink.hdmi"]);

        let speakers = &snapshot.sinks[0];

        assert_eq!(speakers.label, "Speaker");
        assert_eq!(speakers.volume, 50);
        assert!(!speakers.muted);
        assert_eq!(speakers.kind, Kind::Speaker);

        assert_eq!(snapshot.sinks[1].kind, Kind::Headphones);
        assert!(snapshot.sinks[1].muted);
        assert_eq!(snapshot.sinks[1].volume, 30, "the average of the channels");
        assert_eq!(snapshot.default_sink.as_deref(), Some("sink.speakers"));
    }

    #[test]
    fn sources_leave_out_the_monitors() {
        let snapshot = read(&*recorder().1).unwrap();
        let names: Vec<_> = snapshot.sources.iter().map(|d| d.name.as_str()).collect();

        assert_eq!(names, ["source.mic", "source.headset"]);
        assert_eq!(snapshot.sources[0].kind, Kind::Mic);
        assert_eq!(snapshot.sources[1].kind, Kind::Headphones);
    }

    #[test]
    fn streams_are_the_applications_with_a_name() {
        let snapshot = read(&*recorder().1).unwrap();

        assert_eq!(
            snapshot.streams.len(),
            2,
            "the unnamed filter stream is not an application"
        );
        assert_eq!(snapshot.streams[0].app, "Firefox");
        assert_eq!(snapshot.streams[0].index, 41);
        assert_eq!(snapshot.streams[0].volume, 80);
        assert!(snapshot.streams[1].muted);
    }

    #[test]
    fn a_broken_reading_is_an_error_not_a_blank_panel() {
        let (recorder, shared) = Recorder::shared();
        recorder.answer("pactl -f json list sinks", Ok("not json"));

        assert!(read(&*shared).unwrap_err().starts_with("pactl:"));

        let (recorder, shared) = Recorder::shared();
        recorder.answer("pactl -f json list sinks", Err("Connection refused"));

        assert_eq!(read(&*shared).unwrap_err(), "Connection refused");
    }

    #[test]
    fn choosing_a_sink_sets_it_and_moves_the_streams() {
        let snapshot = read(&*recorder().1).unwrap();
        let calls: Vec<String> = calls(
            &Action::Default(Target::Sink("sink.headset".into())),
            &snapshot,
        )
        .iter()
        .map(|call| call.join(" "))
        .collect();

        assert_eq!(
            calls,
            [
                "pactl set-default-sink sink.headset",
                "pactl move-sink-input 41 sink.headset",
                "pactl move-sink-input 42 sink.headset",
            ]
        );
    }

    #[test]
    fn volumes_and_mutes_name_what_they_set() {
        let snapshot = Snapshot::default();
        let one = |action| {
            calls(&action, &snapshot)
                .iter()
                .map(|call| call.join(" "))
                .collect::<Vec<_>>()
        };

        assert_eq!(
            one(Action::Volume(Target::Sink("s".into()), 65)),
            ["pactl set-sink-volume s 65%"]
        );
        assert_eq!(
            one(Action::Volume(Target::Source("m".into()), 0)),
            ["pactl set-source-volume m 0%"]
        );
        assert_eq!(
            one(Action::Volume(Target::Stream(7), 100)),
            ["pactl set-sink-input-volume 7 100%"]
        );
        assert_eq!(
            one(Action::Mute(Target::Sink("s".into()))),
            ["pactl set-sink-mute s toggle"]
        );
        assert_eq!(
            one(Action::Mute(Target::Stream(7))),
            ["pactl set-sink-input-mute 7 toggle"]
        );
        assert_eq!(
            one(Action::Default(Target::Source("m".into()))),
            ["pactl set-default-source m"]
        );
    }

    #[test]
    fn the_keyboard_walks_the_rows_and_chooses() {
        let (_, shared, mut state) = loaded();

        // Rows: three sinks, the master volume, two sources, the mic volume, two streams.
        assert_eq!(state.items().len(), 3 + 1 + 2 + 1 + 2);

        let _ = state.key(Key::Down, &shared);
        assert_eq!(state.focus, 1);

        let _ = state.key(Key::Up, &shared);
        let _ = state.key(Key::Up, &shared);
        assert_eq!(state.focus, state.items().len() - 1, "it wraps");

        state.focus = 1;
        let _ = state.key(Key::Enter, &shared);

        assert_eq!(state.snapshot.default_sink.as_deref(), Some("sink.headset"));
    }

    #[test]
    fn the_arrows_move_a_slider_a_step_and_m_mutes() {
        let (_, shared, mut state) = loaded();

        state.focus = 3; // the master volume of the default sink, at 50 %
        let _ = state.key(Key::Right, &shared);
        assert_eq!(state.default_sink().unwrap().volume, 55);

        let _ = state.key(Key::Left, &shared);
        let _ = state.key(Key::Left, &shared);
        assert_eq!(state.default_sink().unwrap().volume, 45);

        let _ = state.key(Key::End, &shared);
        assert_eq!(state.default_sink().unwrap().volume, 100);

        let _ = state.key(Key::Home, &shared);
        assert_eq!(state.default_sink().unwrap().volume, 0);

        assert!(!state.default_sink().unwrap().muted);
        let _ = state.key(Key::Char('m'), &shared);
        assert!(state.default_sink().unwrap().muted);
    }

    #[test]
    fn a_change_is_sent_and_a_drag_sends_only_where_it_stopped() {
        let (recorder, shared, mut state) = loaded();
        let sink = Target::Sink("sink.speakers".into());

        // The first change goes out at once; while it is running the rest wait,
        // and of the ones for the same slider only the last is kept.
        let _ = state.update(Message::Volume(sink.clone(), 55), &shared);
        assert!(state.writing);

        let _ = state.update(Message::Volume(sink.clone(), 60), &shared);
        let _ = state.update(Message::Volume(sink.clone(), 70), &shared);
        let _ = state.update(Message::Mute(sink.clone()), &shared);

        assert_eq!(
            state.queue,
            [Action::Volume(sink.clone(), 70), Action::Mute(sink.clone())]
        );
        assert_eq!(
            state.default_sink().unwrap().volume,
            70,
            "the slider moves at once"
        );
        assert!(
            recorder.calls().is_empty(),
            "nothing runs on the interface's own thread"
        );
    }

    #[test]
    fn a_reading_does_not_undo_what_the_user_has_just_done() {
        let (_, shared, mut state) = loaded();
        let sink = Target::Sink("sink.speakers".into());

        let _ = state.update(Message::Volume(sink, 85), &shared);
        let _ = state.update(Message::Written(Ok(())), &shared);

        // The machine still says 50 %: it has not caught up.
        let _ = state.update(Message::Loaded(read(&*recorder().1)), &shared);

        assert_eq!(state.default_sink().unwrap().volume, 85);
    }

    #[test]
    fn the_panel_is_not_read_twice_at_once() {
        let (_, shared) = recorder();
        let mut state = State::default();

        let _ = state.refresh(&shared);
        assert!(state.reading);

        // A second request while one is out does nothing (it is a no-op task).
        let _ = state.refresh(&shared);
        assert!(state.reading);
    }
}
