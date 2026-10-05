//! The system monitor: CPU per core, memory, network, power and the busiest
//! processes, from `/proc` and `/sys` alone.
//!
//! [`Sampler`] reads the counters and keeps the last ones to take rates from;
//! it does nothing between two calls, and the panel only calls it while it is
//! shown. The drawing is quadrille's: bar gauges, groups, readings, and a
//! strip chart in a canvas.
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::time::Instant;

use iced_widget::canvas::{self, Frame, Geometry};
use iced_widget::core::{Length, Point, Rectangle, mouse};
use iced_widget::{Widget as _, column, row, space};

use quadrille::draw::{Dash, Pen, rectangle};
use quadrille::{Theme, px, style, widget};

/// How many readings the network chart keeps: a minute at the panel's pace.
pub const HISTORY: usize = 60;

/// How many processes are listed.
pub const TOP: usize = 4;

#[derive(Debug, Clone, Default)]
pub struct Memory {
    pub total_kb: u64,
    pub used_kb: u64,
    pub swap_total_kb: u64,
    pub swap_used_kb: u64,
}

#[derive(Debug, Clone)]
pub struct Power {
    pub percent: u8,
    pub status: String,
    pub watts: Option<f32>,
    pub on_ac: bool,
}

#[derive(Debug, Clone)]
pub struct Process {
    pub name: String,
    /// Of one core, the way `top` has it.
    pub cpu: f32,
    pub rss_kb: u64,
}

/// What one sample found.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub total: u8,
    pub cores: Vec<u8>,
    pub memory: Memory,
    /// Bytes per second, received and sent, over every interface but `lo`.
    pub rx: f32,
    pub tx: f32,
    pub power: Option<Power>,
    pub top: Vec<Process>,
}

/// Reads the machine and remembers what it needs for the next reading.
pub struct Sampler {
    clock_ticks: f64,
    page_kb: u64,
    last: Option<Instant>,
    cpu: Vec<(u64, u64)>,
    net: Option<(u64, u64)>,
    processes: HashMap<u32, u64>,
}

impl Default for Sampler {
    fn default() -> Self {
        // SAFETY: `sysconf` takes a plain constant.
        let (clock_ticks, page) = unsafe {
            (
                libc::sysconf(libc::_SC_CLK_TCK),
                libc::sysconf(libc::_SC_PAGESIZE),
            )
        };

        Self {
            clock_ticks: if clock_ticks > 0 {
                clock_ticks as f64
            } else {
                100.0
            },
            page_kb: if page > 0 { page as u64 / 1024 } else { 4 },
            last: None,
            cpu: Vec::new(),
            net: None,
            processes: HashMap::new(),
        }
    }
}

impl Sampler {
    /// Forgets the counters, so that the next sample is a baseline.
    pub fn reset(&mut self) {
        self.last = None;
        self.cpu.clear();
        self.net = None;
        self.processes.clear();
    }

    /// Reads everything. The rates are over the time since the last call, and
    /// zero on the first, which only takes a baseline.
    pub fn sample(&mut self) -> Snapshot {
        let now = Instant::now();
        let elapsed = self
            .last
            .map(|last| now.duration_since(last).as_secs_f64())
            .filter(|elapsed| *elapsed > 0.0);

        self.last = Some(now);

        let (total, cores) = self.cpu_usage();
        let (rx, tx) = self.network(elapsed);
        let top = self.processes(elapsed);

        Snapshot {
            total,
            cores,
            memory: memory(),
            rx,
            tx,
            power: power(),
            top,
        }
    }

    fn cpu_usage(&mut self) -> (u8, Vec<u8>) {
        let Ok(stat) = fs::read_to_string("/proc/stat") else {
            return (0, Vec::new());
        };

        let now: Vec<(u64, u64)> = stat
            .lines()
            .take_while(|line| line.starts_with("cpu"))
            .map(|line| {
                let values: Vec<u64> = line
                    .split_whitespace()
                    .skip(1)
                    .filter_map(|value| value.parse().ok())
                    .collect();

                // user nice system idle iowait irq softirq steal
                let total: u64 = values.iter().take(8).sum();
                let idle =
                    values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);

                (total.saturating_sub(idle), total)
            })
            .collect();

        let percent = |before: (u64, u64), after: (u64, u64)| -> u8 {
            let total = after.1.saturating_sub(before.1);

            if total == 0 {
                0
            } else {
                (after.0.saturating_sub(before.0) * 100 / total).min(100) as u8
            }
        };

        let usage = if self.cpu.len() == now.len() {
            self.cpu
                .iter()
                .zip(&now)
                .map(|(before, after)| percent(*before, *after))
                .collect()
        } else {
            vec![0; now.len()]
        };

        self.cpu = now;

        match usage.split_first() {
            Some((total, cores)) => (*total, cores.to_vec()),
            None => (0, Vec::new()),
        }
    }

    fn network(&mut self, elapsed: Option<f64>) -> (f32, f32) {
        let Ok(dev) = fs::read_to_string("/proc/net/dev") else {
            return (0.0, 0.0);
        };

        let (mut rx, mut tx) = (0u64, 0u64);

        for line in dev.lines().skip(2) {
            let Some((name, counters)) = line.split_once(':') else {
                continue;
            };

            if name.trim() == "lo" {
                continue;
            }

            let counters: Vec<u64> = counters
                .split_whitespace()
                .filter_map(|value| value.parse().ok())
                .collect();

            rx += counters.first().copied().unwrap_or(0);
            tx += counters.get(8).copied().unwrap_or(0);
        }

        let rates = match (self.net, elapsed) {
            (Some((last_rx, last_tx)), Some(elapsed)) => (
                (rx.saturating_sub(last_rx) as f64 / elapsed) as f32,
                (tx.saturating_sub(last_tx) as f64 / elapsed) as f32,
            ),
            _ => (0.0, 0.0),
        };

        self.net = Some((rx, tx));

        rates
    }

    fn processes(&mut self, elapsed: Option<f64>) -> Vec<Process> {
        let Ok(entries) = fs::read_dir("/proc") else {
            return Vec::new();
        };

        let mut seen = HashMap::with_capacity(self.processes.len());
        let mut found = Vec::new();

        for entry in entries.flatten() {
            let Some(pid) = entry
                .file_name()
                .to_str()
                .and_then(|name| name.parse::<u32>().ok())
            else {
                continue;
            };

            let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
                continue;
            };

            let Some((name, ticks, rss_pages)) = parse_stat(&stat) else {
                continue;
            };

            let cpu = match (self.processes.get(&pid), elapsed) {
                (Some(before), Some(elapsed)) => {
                    (ticks.saturating_sub(*before) as f64 / (self.clock_ticks * elapsed) * 100.0)
                        as f32
                }
                _ => 0.0,
            };

            let _ = seen.insert(pid, ticks);

            found.push(Process {
                name,
                cpu,
                rss_kb: rss_pages * self.page_kb,
            });
        }

        self.processes = seen;

        // The busiest first; with nothing busy, the biggest.
        found.sort_by(|a, b| {
            b.cpu
                .partial_cmp(&a.cpu)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.rss_kb.cmp(&a.rss_kb))
        });
        found.truncate(TOP);
        found
    }
}

/// The name, the user and system time in ticks, and the resident pages of a
/// `/proc/PID/stat`. The name is in parentheses and may hold anything, spaces
/// and parentheses included, so the fields are counted from the last one.
pub fn parse_stat(stat: &str) -> Option<(String, u64, u64)> {
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;

    let name = stat.get(open + 1..close)?.to_owned();
    let rest: Vec<&str> = stat.get(close + 1..)?.split_whitespace().collect();

    // After the name: state is field 3, utime 14, stime 15, rss 24.
    let utime: u64 = rest.get(11)?.parse().ok()?;
    let stime: u64 = rest.get(12)?.parse().ok()?;
    let rss: i64 = rest.get(21)?.parse().ok()?;

    Some((name, utime + stime, rss.max(0) as u64))
}

fn memory() -> Memory {
    let Ok(meminfo) = fs::read_to_string("/proc/meminfo") else {
        return Memory::default();
    };

    let field = |name: &str| -> u64 {
        meminfo
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .and_then(|rest| rest.trim_start_matches(':').split_whitespace().next())
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    };

    let total_kb = field("MemTotal");
    let swap_total_kb = field("SwapTotal");

    Memory {
        total_kb,
        used_kb: total_kb.saturating_sub(field("MemAvailable")),
        swap_total_kb,
        swap_used_kb: swap_total_kb.saturating_sub(field("SwapFree")),
    }
}

fn power() -> Option<Power> {
    let supplies = fs::read_dir("/sys/class/power_supply").ok()?;
    let mut battery = None;
    let mut on_ac = false;

    for entry in supplies.flatten() {
        let path = entry.path();
        let read = |name: &str| fs::read_to_string(path.join(name)).ok();
        let kind = read("type").unwrap_or_default();

        match kind.trim() {
            "Mains" => on_ac |= read("online").is_some_and(|online| online.trim() == "1"),
            "Battery" if battery.is_none() => {
                let percent = read("capacity")?.trim().parse().ok()?;
                let status = read("status").map(|status| status.trim().to_owned());

                // Either the power is reported, or it is the current at the voltage.
                let micro =
                    |name: &str| read(name).and_then(|value| value.trim().parse::<f64>().ok());

                let watts = micro("power_now")
                    .or_else(|| Some(micro("current_now")? * micro("voltage_now")? / 1e6))
                    .map(|micro| (micro / 1e6) as f32)
                    .filter(|watts| *watts > 0.0);

                battery = Some((percent, status.unwrap_or_default(), watts));
            }
            _ => {}
        }
    }

    battery.map(|(percent, status, watts)| Power {
        percent,
        status,
        watts,
        on_ac,
    })
}

/// What the panel shows: the last sample, and the network over the last
/// minute.
#[derive(Debug, Default)]
pub struct State {
    pub sampler: Sampler,
    pub snapshot: Option<Snapshot>,
    pub rx: VecDeque<f32>,
    pub tx: VecDeque<f32>,
}

impl State {
    /// Takes a sample, and keeps the network rates for the chart.
    pub fn sample(&mut self) {
        let snapshot = self.sampler.sample();

        for (history, value) in [(&mut self.rx, snapshot.rx), (&mut self.tx, snapshot.tx)] {
            if history.len() == HISTORY {
                let _ = history.pop_front();
            }

            history.push_back(value);
        }

        self.snapshot = Some(snapshot);
    }

    /// Starts again: what was kept is from before the panel was last hidden.
    pub fn reset(&mut self) {
        self.sampler.reset();
        self.snapshot = None;
        self.rx.clear();
        self.tx.clear();
    }
}

impl std::fmt::Debug for Sampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sampler").finish_non_exhaustive()
    }
}

/// `1.2G`, `340M`, `12K`: kilobytes in the shortest readable form.
pub fn size(kb: u64) -> String {
    let kb = kb as f64;

    if kb >= 1024.0 * 1024.0 * 10.0 {
        format!("{:.0}G", kb / (1024.0 * 1024.0))
    } else if kb >= 1024.0 * 1024.0 {
        format!("{:.1}G", kb / (1024.0 * 1024.0))
    } else if kb >= 1024.0 {
        format!("{:.0}M", kb / 1024.0)
    } else {
        format!("{kb:.0}K")
    }
}

/// A rate in bytes per second, as `1.2M`, `340K`, `12B`.
pub fn rate(bytes: f32) -> String {
    if bytes >= 10.0 * 1024.0 * 1024.0 {
        format!("{:.0}M", bytes / (1024.0 * 1024.0))
    } else if bytes >= 1024.0 * 1024.0 {
        format!("{:.1}M", bytes / (1024.0 * 1024.0))
    } else if bytes >= 1024.0 {
        format!("{:.0}K", bytes / 1024.0)
    } else {
        format!("{bytes:.0}B")
    }
}

/// The chart's top: the next round size above `peak` (1, 2 or 5 of a power of
/// ten, in the units that `rate` prints: K, M, G), at least 20 KB/s so that a
/// quiet line is not stretched over the screen.
pub fn ceiling(peak: f32) -> f32 {
    let peak = peak.max(20.0 * 1024.0);

    (0..4)
        .flat_map(|exponent| {
            [1.0f32, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0]
                .map(|step| step * 1024.0 * 1024f32.powi(exponent))
        })
        .find(|ceiling| *ceiling >= peak)
        .unwrap_or(peak)
}

/// The panel's content.
pub fn view<'a, Message: 'a>(state: &'a State) -> quadrille::Element<'a, Message> {
    let Some(snapshot) = &state.snapshot else {
        return widget::label("READING...")
            .style(style::text::muted)
            .boxed();
    };

    let cores = {
        let column_of = |range: std::ops::Range<usize>| {
            column(
                range
                    .filter_map(|core| snapshot.cores.get(core))
                    .map(|usage| {
                        widget::bar(0.0..=100.0, f32::from(*usage))
                            .height(4)
                            .redline(90.0)
                            .boxed()
                    }),
            )
            .spacing(2)
            .width(Length::Fill)
        };

        // Three columns of gauges.
        let count = snapshot.cores.len();
        let third = count.div_ceil(3);

        row![
            column_of(0..third),
            column_of(third..(2 * third).min(count)),
            column_of((2 * third).min(count)..count)
        ]
        .spacing(px::GAP)
    };

    let cpu = widget::group(
        format!("CPU {:>3}%", snapshot.total),
        column![cores].spacing(px::GAP),
    )
    .width(Length::Fill);

    let memory = &snapshot.memory;

    let swap = if memory.swap_total_kb > 0 {
        widget::reading("SWP", size(memory.swap_used_kb)).boxed()
    } else {
        space::horizontal().boxed()
    };

    let memory = widget::group(
        "MEMORY",
        column![
            row![
                widget::reading(
                    "RAM",
                    format!("{}/{}", size(memory.used_kb), size(memory.total_kb))
                ),
                space::horizontal(),
                swap,
            ],
            widget::bar(0.0..=memory.total_kb.max(1) as f32, memory.used_kb as f32)
                .height(5)
                .redline(memory.total_kb as f32 * 0.9),
        ]
        .spacing(px::TIGHT),
    )
    .width(Length::Fill);

    let peak = state
        .rx
        .iter()
        .chain(&state.tx)
        .copied()
        .fold(0.0f32, f32::max);

    let net = widget::group(
        format!("NET v{} ^{}", rate(snapshot.rx), rate(snapshot.tx)),
        column![
            NetChart {
                rx: &state.rx,
                tx: &state.tx,
                ceiling: ceiling(peak),
                width: Length::Fill,
                height: Length::Fixed(24.0),
            },
            row![
                widget::label(format!("{} MAX", rate(ceiling(peak)))).style(style::text::faint),
                space::horizontal(),
                widget::label("60S").style(style::text::faint),
            ]
        ]
        .spacing(px::TIGHT),
    )
    .width(Length::Fill);

    let mut panels = column![cpu, memory, net].spacing(px::GAP);

    if let Some(power) = &snapshot.power {
        let watts = power
            .watts
            .map(|watts| format!(" {watts:.1}W"))
            .unwrap_or_default();

        let status = if power.on_ac && power.status.eq_ignore_ascii_case("full") {
            "AC".to_owned()
        } else {
            power.status.to_uppercase()
        };

        panels = panels.push(
            widget::group(
                "POWER",
                column![
                    widget::reading("BAT", format!("{:>3}% {status}{watts}", power.percent)),
                    widget::bar(0.0..=100.0, f32::from(power.percent)).height(5),
                ]
                .spacing(px::TIGHT),
            )
            .width(Length::Fill)
            .boxed(),
        );
    }

    let top = widget::group(
        "TOP",
        column(snapshot.top.iter().map(|process| {
            widget::label(format!(
                "{:<10.10} {:>3.0}% {:>5}",
                process.name,
                process.cpu,
                size(process.rss_kb)
            ))
            .boxed()
        })),
    )
    .width(Length::Fill);

    panels.push(top.boxed()).boxed()
}

/// The network over the last minute: received in the live colour, sent in the
/// accent, the newest at the right edge.
pub struct NetChart<'a> {
    rx: &'a VecDeque<f32>,
    tx: &'a VecDeque<f32>,
    ceiling: f32,
    width: Length,
    height: Length,
}

impl<Message> canvas::Program<Message, Theme> for NetChart<'_> {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &iced_widget::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = theme.palette();
        let size = px::floor(bounds.size());
        let mut frame = Frame::new(renderer, bounds.size());

        {
            let mut pen = Pen::new(&mut frame);

            let screen = rectangle(0, 0, size.width, size.height);

            pen.fill(screen, palette.void);
            pen.outline(screen, palette.edge);

            // Inside the border.
            let (left, right) = (1, size.width - 2);
            let (top, bottom) = (1, size.height - 2);

            if right > left && bottom > top {
                let middle = (top + bottom) / 2;

                pen.dashed(
                    Point::new(left, middle),
                    Point::new(right, middle),
                    Dash::SPARSE,
                    palette.faint,
                );

                let trace = |history: &VecDeque<f32>| -> Vec<Point<i32>> {
                    let columns = (right - left) as usize;

                    history
                        .iter()
                        .rev()
                        .take(columns + 1)
                        .enumerate()
                        .map(|(age, value)| {
                            let fraction = (value / self.ceiling).clamp(0.0, 1.0);
                            let row = bottom - (fraction * (bottom - top) as f32).round() as i32;

                            Point::new(right - age as i32, row)
                        })
                        .collect()
                };

                pen.polyline(&trace(self.tx), palette.accent);
                pen.polyline(&trace(self.rx), palette.live);
            }
        }

        vec![frame.into_geometry()]
    }
}

quadrille::canvas_widget!(NetChart<'a>);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stat_line_with_a_hard_name_is_read() {
        let stat = "4242 (my (odd) name) S 1 4242 4242 0 -1 4194304 100 0 0 0 \
                    120 30 0 0 20 0 1 0 5000 1000000 2500 18446744073709551615 0 0 0 0 0 0 0 0 0 0 0 0 17 3 0 0 0 0 0";

        let (name, ticks, rss) = parse_stat(stat).expect("A stat line");

        assert_eq!(name, "my (odd) name");
        assert_eq!(ticks, 150);
        assert_eq!(rss, 2500);
    }

    #[test]
    fn a_short_stat_line_is_not_one() {
        assert!(parse_stat("1 (init) S 0").is_none());
        assert!(parse_stat("garbage").is_none());
    }

    #[test]
    fn sizes_and_rates_are_short() {
        assert_eq!(size(512), "512K");
        assert_eq!(size(2048), "2M");
        assert_eq!(size(1536 * 1024), "1.5G");
        assert_eq!(size(32 * 1024 * 1024), "32G");
        assert_eq!(rate(12.0), "12B");
        assert_eq!(rate(340.0 * 1024.0), "340K");
        assert_eq!(rate(1.5 * 1024.0 * 1024.0), "1.5M");
    }

    #[test]
    fn the_chart_tops_out_at_a_round_number() {
        // A quiet line is not stretched over the screen.
        assert_eq!(ceiling(0.0), ceiling(1.0));
        assert!(ceiling(0.0) >= 20.0 * 1024.0);

        // What the label says is round, in the units it is printed in.
        for (peak, label) in [
            (0.0, "20K"),
            (30.0 * 1024.0, "50K"),
            (300.0 * 1024.0, "500K"),
            (600.0 * 1024.0, "1.0M"),
            (3.5 * 1024.0 * 1024.0, "5.0M"),
            (40.0 * 1024.0 * 1024.0, "50M"),
        ] {
            assert_eq!(rate(ceiling(peak)), label, "peak {peak}");
        }
    }

    #[test]
    fn the_first_sample_is_a_baseline_and_the_second_has_rates() {
        let mut state = State::default();

        state.sample();

        let first = state.snapshot.clone().expect("A snapshot");

        assert_eq!(first.rx, 0.0);
        assert!(!first.cores.is_empty());
        assert!(first.memory.total_kb > 0);

        std::thread::sleep(std::time::Duration::from_millis(60));
        state.sample();

        assert_eq!(state.rx.len(), 2);
        assert!(state.snapshot.expect("A snapshot").top.len() <= TOP);
    }

    #[test]
    fn the_history_is_a_window() {
        let mut state = State::default();

        for _ in 0..HISTORY + 5 {
            state.sample();
        }

        assert_eq!(state.rx.len(), HISTORY);
        assert_eq!(state.tx.len(), HISTORY);
    }
}
