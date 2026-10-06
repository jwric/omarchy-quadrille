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
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

const SIZE: (u32, u32) = (25, 25);
const REST: Duration = Duration::from_millis(300);
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
    pub workspace: i64,
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
    pub resting: bool,
    pub windows: Vec<Window>,
}

impl Snapshot {
    /// A failed query invalidates the previous location. Keeping it could
    /// leave a mapped reticle on an output the pointer has already left.
    fn set_cursor(&mut self, cursor: Option<(f64, f64)>) -> bool {
        let changed = self.cursor != cursor;
        self.cursor = cursor;
        if changed {
            self.resting = false;
        }
        changed
    }

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
            self.snapshot
                .monitor()
                .zip(self.snapshot.cursor)
                .map(|(monitor, cursor)| {
                    render(
                        monitor,
                        cursor,
                        theme,
                        self.snapshot.resting,
                        &self.snapshot.windows,
                    )
                })
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
}

fn placement(monitor: &Monitor, cursor: (f64, f64)) -> Placement {
    let ps = monitor.size.pixels_per_vpx;
    let x = containing_pixel(cursor.0 - monitor.x, monitor.scale, ps);
    let y = containing_pixel(cursor.1 - monitor.y, monitor.scale, ps);
    let centre = (12, 12);
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
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    at: (f64, f64),
    size: (f64, f64),
}

impl Window {
    fn contains(&self, cursor: (f64, f64)) -> bool {
        cursor.0 >= self.at.0
            && cursor.1 >= self.at.1
            && cursor.0 < self.at.0 + self.size.0
            && cursor.1 < self.at.1 + self.size.1
    }
}

#[derive(Debug, Clone)]
struct Dimension {
    axis: &'static str,
    kind: &'static str,
    start: (i32, i32),
    end: (i32, i32),
    track: i32,
    plate: (i32, i32, i32, i32),
    text: String,
    mm: i32,
}

fn dimensions(monitor: &Monitor, cursor: (f64, f64), windows: &[Window]) -> Vec<Dimension> {
    let ps = monitor.size.pixels_per_vpx;
    let w = monitor.size.width_px as i32 / ps as i32;
    let h = monitor.size.height_px as i32 / ps as i32;
    let cx = containing_pixel(cursor.0 - monitor.x, monitor.scale, ps) / ps as i32;
    let cy = containing_pixel(cursor.1 - monitor.y, monitor.scale, ps) / ps as i32;
    let local = (cursor.0 - monitor.x, cursor.1 - monitor.y);
    let mut requests = Vec::new();
    let right = local.0 > monitor.size.width_px as f64 / monitor.scale / 2.0;
    let bottom = local.1 > monitor.size.height_px as f64 / monitor.scale / 2.0;
    let sx = if right { w - 1 } else { 0 };
    let sy = if bottom { h - 1 } else { 0 };
    let screen_mm_x = if right {
        monitor.size.width_mm - local.0 * monitor.size.mm_per_logical_pixel_x
    } else {
        local.0 * monitor.size.mm_per_logical_pixel_x
    };
    let screen_mm_y = if bottom {
        monitor.size.height_mm - local.1 * monitor.size.mm_per_logical_pixel_y
    } else {
        local.1 * monitor.size.mm_per_logical_pixel_y
    };
    requests.push(("x", "S", sx, screen_mm_x, 28));
    requests.push(("y", "S", sy, screen_mm_y, 36));
    if let Some(window) = windows.iter().find(|window| window.contains(cursor)) {
        for (axis, pos, size, origin, value, ratio) in [
            (
                "x",
                window.at.0,
                window.size.0,
                monitor.x,
                cursor.0,
                monitor.size.mm_per_logical_pixel_x,
            ),
            (
                "y",
                window.at.1,
                window.size.1,
                monitor.y,
                cursor.1,
                monitor.size.mm_per_logical_pixel_y,
            ),
        ] {
            let nearest = if value - pos <= pos + size - value {
                pos
            } else {
                pos + size
            };
            // A spanning window's nearest edge may be on a different output.
            // Its length is not measurable with THIS output's physical ruler.
            let extent = if axis == "x" { w } else { h };
            let edge = containing_pixel(nearest - origin, monitor.scale, ps) / ps as i32;
            if (0..extent).contains(&edge) {
                requests.push((axis, "W", edge, (value - nearest).abs() * ratio, -20));
            }
        }
    }
    let mut result: Vec<Dimension> = Vec::new();
    for (axis, kind, edge, mm, offset) in requests {
        let start = (cx, cy);
        let end = if axis == "x" { (edge, cy) } else { (cx, edge) };
        let text = format!(
            "{kind} {}{:.0} mm",
            if monitor.size.estimated { "~" } else { "" },
            mm
        );
        let tw = text.chars().count() as i32 * 6 + 7;
        if tw >= w || h < 30 {
            continue;
        }
        let coord = if axis == "x" { cy } else { cx };
        let limit = if axis == "x" { h } else { w };
        let mut chosen = None;
        for displacement in [offset, -offset, offset * 2, -offset * 2] {
            let track = coord + displacement;
            if track < 6 || track >= limit - 6 {
                continue;
            }
            let x = if axis == "x" {
                ((cx + edge - tw) / 2).clamp(4, w - tw - 4)
            } else {
                (track + 6).clamp(4, w - tw - 4)
            };
            let y = if axis == "x" {
                (track - 8).clamp(4, h - 20)
            } else {
                ((cy + edge - 16) / 2).clamp(4, h - 20)
            };
            let plate = (x, y, tw, 16);
            let collision = |a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)| {
                a.0 < b.0 + b.2 + 4
                    && b.0 < a.0 + a.2 + 4
                    && a.1 < b.1 + b.3 + 4
                    && b.1 < a.1 + a.3 + 4
            };
            if collision(plate, (cx - 9, cy - 9, 19, 19))
                || result.iter().any(|d| collision(plate, d.plate))
            {
                continue;
            }
            chosen = Some((track, plate));
            break;
        }
        if let Some((track, plate)) = chosen {
            result.push(Dimension {
                axis,
                kind,
                start,
                end,
                track,
                plate,
                text,
                mm: mm.round() as i32,
            });
        }
    }
    result
}

fn render(
    monitor: &Monitor,
    cursor: (f64, f64),
    theme: &Theme,
    resting: bool,
    windows: &[Window],
) -> SurfaceSettings {
    let ps = monitor.size.pixels_per_vpx;
    let mut p = placement(monitor, cursor);
    let size = if resting {
        p.margin = (0, 0);
        p.offset = (0, 0);
        p.centre = (
            containing_pixel(cursor.0 - monitor.x, monitor.scale, ps) / ps as i32,
            containing_pixel(cursor.1 - monitor.y, monitor.scale, ps) / ps as i32,
        );
        (monitor.size.width_px / ps, monitor.size.height_px / ps)
    } else {
        SIZE
    };
    let logical = (
        iced_layer::logical_for_native(size.0, ps, monitor.scale),
        iced_layer::logical_for_native(size.1, ps, monitor.scale),
    );
    let physical = (
        (logical.0 as f64 * monitor.scale).round() as u32,
        (logical.1 as f64 * monitor.scale).round() as u32,
    );
    let mut raster = Raster::new(physical.0, physical.1, ps, p.offset);
    let roles = theme.palette();
    let dims = if resting {
        dimensions(monitor, cursor, windows)
    } else {
        Vec::new()
    };
    for d in &dims {
        let (x, y) = d.start;
        let (ex, ey) = d.end;
        let mut line = |x, y, w, h| {
            raster.rect(x - 1, y - 1, w + 2, h + 2, roles.void);
            raster.rect(x, y, w, h, roles.line);
        };
        if d.axis == "x" {
            line(x.min(ex), d.track, (x - ex).abs() + 1, 1);
            for xx in [x, ex] {
                line(xx, y.min(d.track) - 3, 1, (y - d.track).abs() + 7);
            }
            for xx in [x, ex] {
                for k in -3..=3 {
                    raster.rect(xx + k, d.track + k, 1, 1, roles.accent);
                }
            }
        } else {
            line(d.track, y.min(ey), 1, (y - ey).abs() + 1);
            for yy in [y, ey] {
                line(x.min(d.track) - 3, yy, (x - d.track).abs() + 7, 1);
            }
            for yy in [y, ey] {
                for k in -3..=3 {
                    raster.rect(d.track + k, yy + k, 1, 1, roles.accent);
                }
            }
        }
    }
    // Plates are painted last, so every dimension keeps its own legible label.
    for d in &dims {
        let (x, y, w, h) = d.plate;
        raster.rect(x, y, w, h, roles.void);
        raster.outline(x, y, w, h, roles.edge);
        raster.text(x + 3, y + 2, &d.text, roles.ink);
    }
    let (cx, cy) = p.centre;
    // Four small drafting brackets surround a clear cursor cell.
    for dx in [-1, 1] {
        for dy in [-1, 1] {
            let x = cx + dx * 5;
            let y = cy + dy * 5;
            let hx = if dx < 0 { x } else { x - 2 };
            let vy = if dy < 0 { y } else { y - 2 };
            raster.rect(hx - 1, y - 1, 5, 3, roles.void);
            raster.rect(x - 1, vy - 1, 3, 5, roles.void);
            raster.rect(hx, y, 3, 1, roles.accent);
            raster.rect(x, vy, 1, 3, roles.accent);
        }
    }
    if std::env::var_os("QUADRILLE_NESTED").is_some() {
        if let Some(path) = std::env::var_os("QUADRILLE_RETICLE_DUMP") {
            let path = PathBuf::from(path);
            if path.with_extension("capture").exists() {
                let metadata=serde_json::json!({"output":monitor.name,"size":physical,"scale":monitor.scale,"vpx":ps,
                    "margin":p.margin,"offset":p.offset,"centre":p.centre,"cursor":cursor,"resting":resting,"dimensions":dims.iter().map(|d| serde_json::json!({"axis":d.axis,"kind":d.kind,"start":d.start,"end":d.end,"track":d.track,"plate":d.plate,"text":d.text,"mm":d.mm})).collect::<Vec<_>>(),
                    "snapped":[containing_pixel(cursor.0-monitor.x,monitor.scale,ps),containing_pixel(cursor.1-monitor.y,monitor.scale,ps)]}).to_string();
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
    SurfaceSettings {
        namespace: if resting {
            "quadrille-dimensions"
        } else {
            "quadrille-reticle"
        }
        .into(),
        layer: Layer::Overlay,
        anchor: Anchor::TOP | Anchor::LEFT,
        size,
        exclusive: Exclusive::Ignore,
        logical_margin: Some([p.margin.1, 0, 0, p.margin.0]),
        input_passthrough: true,
        output: Some(monitor.name.clone()),
        raster: Some(RasterBuffer {
            size: physical,
            pixels: raster.pixels.into(),
        }),
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
            let bits = crate::glyphs::glyph(ch as u32);
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

fn socket(name: &str) -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("XDG_RUNTIME_DIR")?)
            .join("hypr")
            .join(std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?)
            .join(name),
    )
}

async fn query(command: &str) -> std::io::Result<serde_json::Value> {
    let result = request(command)
        .await
        .and_then(|reply| serde_json::from_str(&reply).map_err(std::io::Error::other));
    if let Err(error) = &result {
        log::debug!("overlay {command}: {error}");
    }
    result
}

fn cursor_position(value: &serde_json::Value) -> Option<(f64, f64)> {
    Some((value["x"].as_f64()?, value["y"].as_f64()?))
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
                workspace: m["activeWorkspace"]["id"].as_i64().unwrap_or(-1),
                size: PhysicalSize::resolve(&input, &overrides),
            })
        })
        .collect()
}

fn windows(value: &serde_json::Value, monitors: &[Monitor]) -> Vec<Window> {
    let mut clients: Vec<_> = value
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| {
            c["mapped"].as_bool().unwrap_or(true) && !c["hidden"].as_bool().unwrap_or(false)
        })
        .collect();
    clients.sort_by_key(|c| c["focusHistoryID"].as_u64().unwrap_or(u64::MAX));
    clients
        .into_iter()
        .filter_map(|c| {
            let at = (c["at"][0].as_f64()?, c["at"][1].as_f64()?);
            let size = (c["size"][0].as_f64()?, c["size"][1].as_f64()?);
            let visible = c["pinned"].as_bool().unwrap_or(false)
                || monitors
                    .iter()
                    .any(|m| m.workspace == c["workspace"]["id"].as_i64().unwrap_or(-1));
            (visible && size.0 > 0.0 && size.1 > 0.0).then_some(Window { at, size })
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
                    state.windows = query("j/clients")
                        .await
                        .ok()
                        .map(|v| windows(&v, &state.monitors))
                        .unwrap_or_default();
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
                    && (state.monitor().is_some() || state.cursor.is_none() || event_refresh);
                let mut sampled_cursor = false;
                if allowed && (now >= next_cursor || event_refresh) {
                    sampled_cursor = true;
                    let cursor = query("j/cursorpos")
                        .await
                        .ok()
                        .and_then(|v| cursor_position(&v));
                    if state.set_cursor(cursor) {
                        if cursor.is_some() {
                            last_moved = now;
                        }
                        changed = true;
                    }
                    next_cursor = now + interval(last_moved, now, true).unwrap_or(STILL);
                }
                let resting = state.visible() && now.duration_since(last_moved) >= REST;
                if resting && !state.resting {
                    log::debug!(
                        "cursor rested {} ms",
                        now.duration_since(last_moved).as_millis()
                    );
                }
                if resting
                    && (!state.resting
                        || (sampled_cursor && now.duration_since(last_moved) >= MOVING))
                {
                    let windows = query("j/clients")
                        .await
                        .ok()
                        .map(|v| windows(&v, &state.monitors))
                        .unwrap_or_default();
                    changed |= state.windows != windows;
                    state.windows = windows;
                }
                changed |= state.resting != resting;
                state.resting = resting;
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
                let mut next = interval(last_moved, now, state.visible())
                    .map(|_| next_cursor)
                    .map_or(last_lock + STILL, |cursor| cursor.min(last_lock + STILL));
                if state.visible() && !state.resting {
                    next = next.min(last_moved + REST);
                }
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
                            | "workspace"
                            | "workspacev2"
                            | "moveworkspace"
                            | "moveworkspacev2"
                    ) {
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
                            | "openwindow"
                            | "movewindow"
                            | "movewindowv2"
                            | "changefloatingmode"
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
            workspace: 1,
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
            let surface = render(&monitor, (300.0, 200.0), &Theme::default(), false, &[]);
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
        assert_eq!(p.centre, (12, 12));
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

    #[test]
    fn cursor_crossing_warp_and_topology_keep_one_complete_surface() {
        let a = monitor(5.0 / 3.0);
        let mut b = monitor(1.0);
        b.name = "QB".into();
        b.x = 2000.0;
        let theme = Theme::default();
        let mut snapshot = Snapshot {
            monitors: vec![a, b],
            ..Snapshot::default()
        };
        let mut state = State::new(true);
        for (cursor, output) in [
            (Some((300.0, 200.0)), Some("QA")),
            (Some((2300.0, 200.0)), Some("QB")),
            (Some((1.0, 1.0)), Some("QA")),
            (Some((2500.0, 1000.0)), Some("QB")),
            (Some((-1.0, -1.0)), None),
            (None, None),
        ] {
            snapshot.set_cursor(cursor);
            state.update(snapshot.clone(), &theme);
            assert_eq!(
                state.surface().and_then(|surface| surface.output),
                output.map(str::to_owned)
            );
            if let Some(cursor) = cursor.filter(|_| output.is_some()) {
                let monitor = snapshot.monitor().unwrap();
                assert!(monitor.contains(cursor));
                assert!(state.surface().unwrap().raster.is_some());
            }
        }
        snapshot.set_cursor(Some((2300.0, 200.0)));
        state.update(snapshot.clone(), &theme);
        assert_eq!(state.surface().unwrap().output.as_deref(), Some("QB"));
        // The pointer does not move while the containing output is removed.
        snapshot.monitors.retain(|monitor| monitor.name != "QB");
        state.update(snapshot.clone(), &theme);
        assert!(state.surface().is_none());
        // Repositioning QA under the stationary pointer must redraw there.
        snapshot.monitors[0].x = 2000.0;
        state.update(snapshot, &theme);
        assert_eq!(state.surface().unwrap().output.as_deref(), Some("QA"));
    }

    #[test]
    fn a_failed_or_incomplete_query_clears_the_previous_output() {
        let mut snapshot = Snapshot {
            monitors: vec![monitor(1.0)],
            cursor: Some((300.0, 200.0)),
            ..Snapshot::default()
        };
        let mut state = State::new(true);
        let theme = Theme::default();
        state.update(snapshot.clone(), &theme);
        assert!(state.surface().is_some());
        for reply in [serde_json::json!({}), serde_json::json!({"x": 2300})] {
            assert!(snapshot.set_cursor(cursor_position(&reply)));
            state.update(snapshot.clone(), &theme);
            assert!(state.surface().is_none());
            assert!(snapshot.set_cursor(Some((300.0, 200.0))));
        }
        assert!(snapshot.set_cursor(None));
        state.update(snapshot, &theme);
        assert!(state.surface().is_none());
    }

    #[test]
    fn dimension_geometry_precision_and_collision() {
        for scale in [1.0, 5.0 / 3.0] {
            let monitor = monitor(scale);
            let window = Window {
                at: (100.0, 100.0),
                size: (600.0, 400.0),
            };
            for cursor in [(200.0, 200.0), (0.0, 0.0), (1500.0, 950.0), (650.0, 450.0)] {
                let dims = dimensions(&monitor, cursor, &[window.clone()]);
                assert!(dims.len() <= 4);
                for (i, d) in dims.iter().enumerate() {
                    assert!(!d.text.contains('.'));
                    assert!(d.plate.0 >= 0 && d.plate.1 >= 0);
                    for other in &dims[..i] {
                        let (x, y, w, h) = d.plate;
                        let (ox, oy, ow, oh) = other.plate;
                        assert!(x >= ox + ow || ox >= x + w || y >= oy + oh || oy >= y + h);
                    }
                    if d.kind == "W" && cursor == (200.0, 200.0) {
                        let ratio = if d.axis == "x" {
                            monitor.size.mm_per_logical_pixel_x
                        } else {
                            monitor.size.mm_per_logical_pixel_y
                        };
                        assert_eq!(d.mm, (100.0 * ratio).round() as i32);
                    }
                }
                for resting in [false, true] {
                    let surface = render(
                        &monitor,
                        cursor,
                        &Theme::default(),
                        resting,
                        &[window.clone()],
                    );
                    assert!(surface.input_passthrough);
                    let geom = iced_layer::geometry(&surface, monitor.size.pixels_per_vpx, scale);
                    assert!(iced_layer::is_exact(geom.size.0, scale));
                    assert!(iced_layer::is_exact(geom.size.1, scale));
                }
            }
        }
    }
}
