//! The compositor's pointer, measured on each output's own physical grid.
//! Native small buffers keep protocol margins separate from physical pixels.
use crate::physical::{DisplayInput, Overrides, PhysicalSize};
use iced_core::Color;
use iced_futures::{
    futures::{SinkExt, StreamExt},
    stream,
};
use iced_layer::{Anchor, Exclusive, Layer, RasterBuffer, SurfaceSettings};
use quadrille::Theme;
use smol::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use std::{
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

const SIZE: (u32, u32) = (165, 50);
const MOVING: Duration = Duration::from_millis(400);
const FAST: Duration = Duration::from_micros(16_667);
const STILL: Duration = Duration::from_millis(200);
static OUTPUT_REVISION: AtomicU64 = AtomicU64::new(0);

pub fn outputs_changed() {
    OUTPUT_REVISION.fetch_add(1, Ordering::Relaxed);
}

#[derive(Debug, Clone)]
pub struct Monitor {
    pub name: String,
    pub x: f64,
    pub y: f64,
    pub scale: f64,
    pub size: PhysicalSize,
}

impl Monitor {
    fn contains(&self, (x, y): (f64, f64)) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x + self.size.width_px as f64 / self.scale
            && y < self.y + self.size.height_px as f64 / self.scale
    }
}

#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub monitors: Vec<Monitor>,
    pub cursor: Option<(f64, f64)>,
    pub fullscreen: bool,
    pub locked: bool,
}

impl Snapshot {
    fn monitor(&self) -> Option<&Monitor> {
        self.cursor.and_then(|cursor| {
            self.monitors
                .iter()
                .find(|monitor| monitor.contains(cursor))
        })
    }

    fn visible(&self) -> bool {
        !self.fullscreen && !self.locked && self.monitor().is_some()
    }
}

pub struct State {
    pub enabled: bool,
    snapshot: Snapshot,
    surface: Option<SurfaceSettings>,
}

impl State {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            snapshot: Snapshot::default(),
            surface: None,
        }
    }

    pub fn clear(&mut self) {
        self.surface = None;
    }

    pub fn update(&mut self, snapshot: Snapshot, theme: &Theme) {
        self.snapshot = snapshot;
        self.restyle(theme);
    }

    pub fn restyle(&mut self, theme: &Theme) {
        self.surface = if self.enabled && self.snapshot.visible() {
            let monitor = self.snapshot.monitor().unwrap();
            let cursor = self.snapshot.cursor.unwrap();
            Some(render(monitor, cursor, theme))
        } else {
            None
        };
    }

    pub fn surface(&self) -> Option<SurfaceSettings> {
        self.surface.clone()
    }

    pub fn status(&self) -> String {
        let state = if !self.enabled {
            "off"
        } else if self.snapshot.locked {
            "on (locked)"
        } else if self.snapshot.fullscreen {
            "on (fullscreen)"
        } else if self.surface.is_some() {
            "on"
        } else {
            "on (no pointer output)"
        };
        let output = self
            .surface
            .as_ref()
            .and_then(|surface| surface.output.as_deref())
            .unwrap_or("-");
        format!("overlay {state} output {output}\n")
    }
}

/// The containing virtual pixel starts here. The real pointer remains in the
/// transparent two-pixel gap; rounding a logical margin never moves the grid.
fn containing_pixel(logical: f64, scale: f64, vpx: u32) -> i32 {
    ((logical * scale).floor() as i32).div_euclid(vpx as i32) * vpx as i32
}

#[derive(Debug, Clone, Copy)]
struct Placement {
    margin: (i32, i32),
    offset: (i32, i32),
    centre: (i32, i32),
    plate: (i32, i32),
}

fn placement(monitor: &Monitor, cursor: (f64, f64)) -> Placement {
    let ps = monitor.size.pixels_per_vpx;
    let x = containing_pixel(cursor.0 - monitor.x, monitor.scale, ps);
    let y = containing_pixel(cursor.1 - monitor.y, monitor.scale, ps);
    let right = x + SIZE.0 as i32 * ps as i32 > monitor.size.width_px as i32;
    let bottom = y + SIZE.1 as i32 * ps as i32 > monitor.size.height_px as i32;
    let centre = (if right { 151 } else { 8 }, if bottom { 41 } else { 8 });
    let left = ((x - centre.0 * ps as i32) as f64 / monitor.scale)
        .floor()
        .max(0.0) as i32;
    let top = ((y - centre.1 * ps as i32) as f64 / monitor.scale)
        .floor()
        .max(0.0) as i32;
    let offset = (
        x - (left as f64 * monitor.scale).round() as i32 - centre.0 * ps as i32,
        y - (top as f64 * monitor.scale).round() as i32 - centre.1 * ps as i32,
    );
    Placement {
        margin: (left, top),
        offset,
        centre,
        plate: (if right { 0 } else { 18 }, if bottom { 1 } else { 18 }),
    }
}

fn render(monitor: &Monitor, cursor: (f64, f64), theme: &Theme) -> SurfaceSettings {
    let p = placement(monitor, cursor);
    let ps = monitor.size.pixels_per_vpx;
    let logical = (
        iced_layer::logical_for_native(SIZE.0, ps, monitor.scale),
        iced_layer::logical_for_native(SIZE.1, ps, monitor.scale),
    );
    let physical = (
        (logical.0 as f64 * monitor.scale).round() as u32,
        (logical.1 as f64 * monitor.scale).round() as u32,
    );
    let mut raster = Raster::new(physical.0, physical.1, ps, p.offset);
    let roles = theme.palette();
    let (cx, cy) = p.centre;
    for (x, y, w, h) in [
        (cx - 7, cy, 5, 1),
        (cx + 3, cy, 5, 1),
        (cx, cy - 7, 1, 5),
        (cx, cy + 3, 1, 5),
    ] {
        raster.rect(x - 1, y - 1, w + 2, h + 2, roles.void);
        raster.rect(x, y, w, h, roles.accent);
    }
    let (x, y) = p.plate;
    raster.rect(x, y, 142, 30, roles.void);
    raster.outline(x, y, 142, 30, roles.edge);
    let mmx = (cursor.0 - monitor.x) * monitor.scale * monitor.size.mm_per_pixel_x;
    let mmy = (cursor.1 - monitor.y) * monitor.scale * monitor.size.mm_per_pixel_y;
    let est = if monitor.size.estimated { "~" } else { "" };
    raster.text(
        x + 3,
        y + 2,
        &format!("x{est}{mmx:.1} y{est}{mmy:.1} mm"),
        roles.ink,
    );
    raster.text(
        x + 3,
        y + 15,
        &format!(
            "C x{est}{:.1} y{est}{:.1}",
            mmx - monitor.size.width_mm / 2.0,
            mmy - monitor.size.height_mm / 2.0
        ),
        roles.ink,
    );
    // Explicit nested-only debugging: no desktop capture or window content.
    if std::env::var_os("QUADRILLE_NESTED").is_some() {
        if let Some(path) = std::env::var_os("QUADRILLE_RETICLE_DUMP") {
            let path = PathBuf::from(path);
            if path.with_extension("capture").exists() {
                let metadata = serde_json::json!({
                "output":monitor.name,"size":physical,"scale":monitor.scale,"vpx":ps,
                "margin":p.margin,"offset":p.offset,"centre":p.centre,"cursor":cursor,
                "snapped":[containing_pixel(cursor.0-monitor.x,monitor.scale,ps),containing_pixel(cursor.1-monitor.y,monitor.scale,ps)]
                }).to_string();
                let mut frame = metadata.into_bytes();
                frame.push(b'\n');
                frame.extend_from_slice(&raster.pixels);
                if std::fs::write(path.with_extension("tmp"), frame).is_ok() {
                    let _ =
                        std::fs::rename(path.with_extension("tmp"), path.with_extension("frame"));
                }
            }
        }
    }
    let buffer = RasterBuffer {
        size: physical,
        pixels: raster.pixels.into(),
    };
    SurfaceSettings {
        namespace: "quadrille-reticle".into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP | Anchor::LEFT,
        size: SIZE,
        exclusive: Exclusive::Ignore,
        logical_margin: Some([p.margin.1, 0, 0, p.margin.0]),
        input_passthrough: true,
        output: Some(monitor.name.clone()),
        raster: Some(buffer),
        ..SurfaceSettings::default()
    }
}

struct Raster {
    width: u32,
    height: u32,
    ps: i32,
    offset: (i32, i32),
    pixels: Vec<u8>,
}

impl Raster {
    fn new(width: u32, height: u32, ps: u32, offset: (i32, i32)) -> Self {
        Self {
            width,
            height,
            ps: ps as i32,
            offset,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        let [r, g, b, a] = color.into_rgba8();
        let pixel = [b, g, r, a];
        let (x, y) = (x * self.ps + self.offset.0, y * self.ps + self.offset.1);
        for yy in y.max(0)..(y + h * self.ps).min(self.height as i32) {
            for xx in x.max(0)..(x + w * self.ps).min(self.width as i32) {
                let index = ((yy * self.width as i32 + xx) * 4) as usize;
                self.pixels[index..index + 4].copy_from_slice(&pixel);
            }
        }
    }
    fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        self.rect(x, y, w, 1, color);
        self.rect(x, y + h - 1, w, 1, color);
        self.rect(x, y, 1, h, color);
        self.rect(x + w - 1, y, 1, h, color);
    }
    fn text(&mut self, x: i32, y: i32, text: &str, color: Color) {
        if text.chars().count() * 6 > 136 {
            return;
        }
        for (index, ch) in text.chars().enumerate() {
            let bits = glyphs()
                .get(&(ch as u32))
                .unwrap_or(&[62, 34, 34, 34, 34, 34, 34, 34, 34, 62, 0, 0]);
            for (row, bits) in bits.iter().enumerate() {
                for col in 0..7 {
                    if bits & (1 << (6 - col)) != 0 {
                        self.rect(x + index as i32 * 6 + col, y + row as i32, 1, 1, color);
                    }
                }
            }
        }
    }
}

fn glyphs() -> &'static std::collections::HashMap<u32, [u8; 12]> {
    static GLYPHS: OnceLock<std::collections::HashMap<u32, [u8; 12]>> = OnceLock::new();
    GLYPHS.get_or_init(|| {
        let source = include_str!("../../../../plugins/quadrille.bar/Q/Glyphs.js");
        let (_, source) = source.split_once("var glyphs = {").unwrap();
        let (source, _) = source.split_once('}').unwrap();
        serde_json::from_str::<std::collections::HashMap<String, String>>(&format!("{{{source}}}"))
            .unwrap()
            .into_iter()
            .map(|(key, value)| {
                (
                    key.parse().unwrap(),
                    std::array::from_fn(|row| {
                        u8::from_str_radix(&value[row * 2..row * 2 + 2], 16).unwrap()
                    }),
                )
            })
            .collect()
    })
}

fn socket(name: &str) -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?)
            .join("hypr")
            .join(std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?)
            .join(name),
    )
}

async fn query(command: &str) -> std::io::Result<serde_json::Value> {
    serde_json::from_str(&request(command).await?).map_err(std::io::Error::other)
}

async fn request(command: &str) -> std::io::Result<String> {
    if matches!(command, "j/cursorpos" | "j/locked")
        && std::env::var_os("QUADRILLE_NESTED").is_some()
    {
        if let Some(path) = std::env::var_os("QUADRILLE_RETICLE_DUMP") {
            let path = PathBuf::from(path);
            if path.with_extension("capture").exists() {
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path.with_extension("polls"))
                {
                    let _ = writeln!(file, "{command}");
                }
            }
        }
    }
    smol::future::race(
        async {
            let path = socket(".socket.sock").ok_or(std::io::ErrorKind::NotFound)?;
            let mut stream = smol::net::unix::UnixStream::connect(path).await?;
            stream.write_all(command.as_bytes()).await?;
            stream.shutdown(std::net::Shutdown::Write)?;
            let mut reply = String::new();
            stream.read_to_string(&mut reply).await?;
            Ok(reply)
        },
        async {
            smol::Timer::after(Duration::from_secs(1)).await;
            Err(std::io::ErrorKind::TimedOut.into())
        },
    )
    .await
}

const RULE_ON: &str = "eval if quadrille_reticle_rule and quadrille_reticle_rule:is_enabled() ~= nil then quadrille_reticle_rule:set_enabled(true) else quadrille_reticle_rule = hl.layer_rule({name='quadrille-reticle',match={namespace='^quadrille-reticle$'},no_anim=true,animation='none'}) end";
const RULE_OFF: &str =
    "eval if quadrille_reticle_rule then quadrille_reticle_rule:set_enabled(false) end";

async fn stationary_rule() -> bool {
    match request(RULE_ON).await {
        Ok(reply) if reply.trim() == "ok" => true,
        result => {
            log::warn!("reticle needs its namespace no-animation rule: {result:?}");
            false
        }
    }
}

/// The runtime rule belongs only to our namespace. Disable the retained Lua
/// handle when the subscription ends; reusing it prevents rule accumulation.
struct RuleGuard;
impl Drop for RuleGuard {
    fn drop(&mut self) {
        use std::io::{Read, Write};
        if let Some(path) = socket(".socket.sock") {
            if let Ok(mut stream) = std::os::unix::net::UnixStream::connect(path) {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
                let _ = stream.set_write_timeout(Some(Duration::from_millis(200)));
                let _ = stream.write_all(RULE_OFF.as_bytes());
                let _ = stream.shutdown(std::net::Shutdown::Write);
                let mut reply = String::new();
                let _ = stream.read_to_string(&mut reply);
            }
        }
    }
}

fn monitors(value: &serde_json::Value) -> Vec<Monitor> {
    let overrides = Overrides::load_default();
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            if m["disabled"].as_bool().unwrap_or(false)
                || m["mirrorOf"]
                    .as_str()
                    .is_some_and(|name| !name.is_empty() && name != "none")
            {
                return None;
            }
            let mut input = DisplayInput::from_monitor(m)?;
            // Match Wayland's fractional-scale protocol unit, one 120th.
            input.scale = (input.scale * 120.0).round().max(1.0) / 120.0;
            Some(Monitor {
                name: input.name.clone(),
                x: m["x"].as_f64()?,
                y: m["y"].as_f64()?,
                scale: input.scale,
                size: PhysicalSize::resolve(&input, &overrides),
            })
        })
        .collect()
}

fn interval(last_moved: Instant, now: Instant, allowed: bool) -> Option<Duration> {
    allowed.then_some(if now.duration_since(last_moved) < MOVING {
        FAST
    } else {
        STILL
    })
}

/// Cursor queries stop when inhibited. A separate cheap lock-state query is
/// needed because Hyprland exposes `j/locked` but no socket2 lock event.
pub fn watch() -> impl iced_futures::futures::Stream<Item = Snapshot> {
    stream::channel(2, async |mut output| {
        let Some(path) = socket(".socket2.sock") else {
            return std::future::pending::<()>().await;
        };
        loop {
            let Ok(events) = smol::net::unix::UnixStream::connect(&path).await else {
                smol::Timer::after(Duration::from_secs(2)).await;
                continue;
            };
            if !stationary_rule().await {
                smol::Timer::after(Duration::from_secs(2)).await;
                continue;
            }
            let _rule = RuleGuard;
            let mut lines = smol::io::BufReader::new(events).lines();
            let mut state = Snapshot::default();
            state.monitors = query("j/monitors")
                .await
                .map(|v| monitors(&v))
                .unwrap_or_default();
            let mut last_moved = Instant::now() - MOVING;
            let mut last_lock = Instant::now() - STILL;
            let mut next_cursor = Instant::now();
            let mut event_refresh = true;
            let mut output_revision = OUTPUT_REVISION.load(Ordering::Relaxed);
            loop {
                let now = Instant::now();
                let mut changed = false;
                let revision = OUTPUT_REVISION.load(Ordering::Relaxed);
                if revision != output_revision {
                    if !stationary_rule().await {
                        break;
                    }
                    state.monitors = query("j/monitors")
                        .await
                        .map(|v| monitors(&v))
                        .unwrap_or_default();
                    output_revision = revision;
                    event_refresh = true;
                }
                if event_refresh {
                    let fullscreen = query("j/activewindow")
                        .await
                        .ok()
                        .and_then(|v| v["fullscreen"].as_i64())
                        .unwrap_or(0)
                        != 0;
                    changed |= state.fullscreen != fullscreen;
                    state.fullscreen = fullscreen;
                }
                if now.duration_since(last_lock) >= STILL || event_refresh {
                    let locked = query("j/locked")
                        .await
                        .ok()
                        .and_then(|v| v["locked"].as_bool())
                        .unwrap_or(true);
                    changed |= state.locked != locked;
                    if state.locked && !locked {
                        event_refresh = true;
                    }
                    state.locked = locked;
                    last_lock = now;
                }
                let allowed = !state.fullscreen
                    && !state.locked
                    && (state.monitor().is_some() || event_refresh);
                if allowed && (now >= next_cursor || event_refresh) {
                    let cursor = query("j/cursorpos")
                        .await
                        .ok()
                        .and_then(|v| Some((v["x"].as_f64()?, v["y"].as_f64()?)));
                    if cursor.is_some() && state.cursor != cursor {
                        last_moved = now;
                        changed = true;
                        state.cursor = cursor;
                    }
                    next_cursor = now + interval(last_moved, now, true).unwrap();
                }
                if changed || event_refresh {
                    if std::env::var_os("QUADRILLE_NESTED").is_some() {
                        if let Some(path) = std::env::var_os("QUADRILLE_RETICLE_DUMP") {
                            let path = PathBuf::from(path);
                            if path.with_extension("capture").exists() {
                                let _ = std::fs::write(
                                    path.with_extension("state"),
                                    format!("{state:?}"),
                                );
                            }
                        }
                    }
                    if output.send(state.clone()).await.is_err() {
                        return;
                    }
                }
                event_refresh = false;
                let next = interval(last_moved, now, state.visible())
                    .map(|_| next_cursor)
                    .map_or(last_lock + STILL, |cursor| cursor.min(last_lock + STILL));
                let result = smol::future::race(async { Some(lines.next().await) }, async {
                    smol::Timer::at(next).await;
                    None
                })
                .await;
                if let Some(line) = result {
                    let Some(Ok(line)) = line else { break };
                    let event = line.split_once(">>").map(|(event, _)| event).unwrap_or("");
                    if matches!(
                        event,
                        "monitoradded"
                            | "monitoraddedv2"
                            | "monitorremoved"
                            | "configreloaded"
                            | "focusedmon"
                    ) {
                        if !stationary_rule().await {
                            break;
                        }
                        state.monitors = query("j/monitors")
                            .await
                            .map(|v| monitors(&v))
                            .unwrap_or_default();
                        event_refresh = true;
                    } else if matches!(
                        event,
                        "activewindow"
                            | "activewindowv2"
                            | "fullscreen"
                            | "workspace"
                            | "workspacev2"
                            | "closewindow"
                    ) {
                        event_refresh = true;
                    }
                }
            }
            state.locked = true;
            let _ = output.send(state).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(scale: f64) -> Monitor {
        let input = DisplayInput {
            name: "QA".into(),
            make: String::new(),
            model: String::new(),
            width: 2560,
            height: 1600,
            physical_width: 340.0,
            physical_height: 220.0,
            transform: 0,
            scale,
        };
        Monitor {
            name: input.name.clone(),
            x: 0.0,
            y: 0.0,
            scale,
            size: PhysicalSize::resolve(&input, &Overrides::default()),
        }
    }

    #[test]
    fn containing_pixel_and_residual_are_exact_at_both_scales() {
        for scale in [1.0, 1.666667] {
            let monitor = monitor(scale);
            let ps = monitor.size.pixels_per_vpx as i32;
            for (x, y) in [
                (0.0, 0.0),
                (103.0, 207.0),
                (400.25, 250.75),
                (1500.0, 950.0),
            ] {
                let p = placement(&monitor, (x, y));
                assert_eq!(
                    (p.margin.0 as f64 * scale).round() as i32 + p.offset.0 + p.centre.0 * ps,
                    containing_pixel(x, scale, ps as u32)
                );
                assert_eq!(
                    (p.margin.1 as f64 * scale).round() as i32 + p.offset.1 + p.centre.1 * ps,
                    containing_pixel(y, scale, ps as u32)
                );
            }
        }
    }

    #[test]
    fn poll_scheduler_moves_settles_and_stops() {
        let moved = Instant::now();
        assert_eq!(
            interval(moved, moved + Duration::from_millis(399), true),
            Some(FAST)
        );
        assert_eq!(interval(moved, moved + MOVING, true), Some(STILL));
        assert_eq!(interval(moved, moved, false), None);
    }

    #[test]
    fn native_buffers_match_integral_physical_extents_at_fractional_scales() {
        for scale in [1.0, 1.25, 4.0 / 3.0, 1.5, 5.0 / 3.0, 1.7, 1.75, 2.0] {
            let monitor = monitor(scale);
            let surface = render(&monitor, (300.0, 200.0), &Theme::default());
            let geometry = iced_layer::geometry(&surface, monitor.size.pixels_per_vpx, scale);
            assert!(iced_layer::is_exact(geometry.size.0, scale));
            assert!(iced_layer::is_exact(geometry.size.1, scale));
            assert_eq!(
                surface.raster.unwrap().size,
                (
                    (geometry.size.0 as f64 * scale).round() as u32,
                    (geometry.size.1 as f64 * scale).round() as u32
                )
            );
        }
    }

    #[test]
    fn inhibited_states_have_no_surface_and_edges_flip() {
        let monitor = monitor(1.666667);
        let p = placement(&monitor, (1500.0, 950.0));
        assert_eq!(p.plate, (0, 1));
        let mut state = State::new(true);
        let theme = Theme::default();
        state.update(
            Snapshot {
                monitors: vec![monitor],
                cursor: Some((300.0, 300.0)),
                ..Snapshot::default()
            },
            &theme,
        );
        assert!(state.surface().unwrap().input_passthrough);
        state.snapshot.locked = true;
        state.restyle(&theme);
        assert!(state.surface().is_none());
        state.snapshot.locked = false;
        state.snapshot.fullscreen = true;
        state.restyle(&theme);
        assert!(state.surface().is_none());
        state.snapshot.fullscreen = false;
        state.snapshot.cursor = Some((-1.0, -1.0));
        state.restyle(&theme);
        assert!(state.surface().is_none());
        state.snapshot.cursor = Some((300.0, 300.0));
        state.enabled = false;
        state.restyle(&theme);
        assert!(state.surface().is_none());
    }
}
