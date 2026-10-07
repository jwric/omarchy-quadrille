//! The displays, side by side at their true sizes: each one's active area
//! as its EDID measures it, dimensioned in millimetres with its diagonal in
//! inches. A detail magnifies its top left corner, where pixel (0, 0) is,
//! far enough to show the pixel grid at the display's pitch.
//!
//! What moves is each display's scan line, running down it at its refresh
//! rate slowed down enough to follow, so a faster display sweeps more often.
use crate::draft::Placement::Auto;
use crate::draft::{Draft, Extent, Fill, Line, Tone, V2, number, v};
use crate::machine::{Connector, ConnectorKind, Machine, Panel, SensorKind, Site};

use super::super::{Card, Domain, Part, Reading, Revision, Subject, Unit};
use super::layout::short;
use super::{SPEC_ROOM, counted, fit, lettered};

/// The room the drawing leaves round and between the displays, in units of
/// the row's length over `UNIT`: room for the dimensions and their values.
const UNIT: f32 = 36.0;
/// A width's dimension line's distance from what it measures, and a
/// height's: its value is lettered across its line, and clears the edge it
/// measures at the scales the sheet draws the row at...
const OFFSET: f32 = 1.0;
const HEIGHT: f32 = 2.5;
/// ...the gap between two displays, which holds the height of the second
/// clear of the first...
const GAP: f32 = 5.0;
/// ...and the margins round the row.
const LEFT: f32 = 4.5;
const RIGHT: f32 = 1.0;
const BELOW: f32 = 2.0;
const ABOVE: f32 = 1.0;
/// How far a centre line runs past its display.
const OVERRUN: f32 = 0.4;

/// The radius of the circle a detail magnifies, in pixels of its display,
/// and how far in from the corner its centre is, in radii: the corner and a
/// few pixels each way.
const DETAIL: f32 = 3.5;
const INSET: f32 = 0.55;
/// How far the detail's pixel grid runs from the corner, in radii: past the
/// edge of the widest detail window.
const GRID: f32 = 4.0;

/// How many times slower than the display refreshes its scan line runs.
const SLOWED: f32 = 240.0;

pub struct Displays {
    card: Card,
    machine: Machine,
    screens: Vec<Screen>,
    extent: Extent,
    /// The unit of the room round the drawing, in millimetres.
    room: f32,
    /// The sensor the graphics' temperature is read from.
    temperature: Option<usize>,
}

/// A display as the sheet draws it.
struct Screen {
    connector: Connector,
    panel: Panel,
    /// Its active area in millimetres, on the baseline.
    area: Extent,
}

impl Screen {
    /// The distance between pixels across and down, in millimetres.
    fn pitch(&self) -> V2 {
        v(
            self.panel.size.mm_per_pixel_x as f32,
            self.panel.size.mm_per_pixel_y as f32,
        )
    }

    /// The top left corner of its active area, where pixel (0, 0) is.
    fn origin(&self) -> V2 {
        v(self.area.min.x, self.area.max.y)
    }

    /// The circle its detail magnifies: the corner, a few pixels each way.
    fn detail(&self) -> (V2, f32) {
        let radius = DETAIL * self.pitch().max_element();

        (self.origin() + v(INSET, -INSET) * radius, radius)
    }

    /// The diagonal as a display is sold by: `14.0"`.
    fn inches(&self) -> String {
        format!("{:.1}\"", self.panel.inches())
    }
}

impl Displays {
    /// The displays of `machine` whose sizes are known, if any are.
    pub fn new(machine: &Machine) -> Option<Self> {
        let measured: Vec<(&Connector, &Panel)> = machine
            .displays()
            .filter(|(_, panel)| !panel.size.estimated)
            .collect();

        if measured.is_empty() {
            return None;
        }

        let row: f32 = measured
            .iter()
            .map(|(_, panel)| panel.size.width_mm as f32)
            .sum();
        let room = row / UNIT;
        let mut x = 0.0;
        let screens: Vec<Screen> = measured
            .iter()
            .map(|&(connector, panel)| {
                let size = v(panel.size.width_mm as f32, panel.size.height_mm as f32);
                let screen = Screen {
                    connector: connector.clone(),
                    panel: panel.clone(),
                    area: Extent::new(v(x, 0.0), v(x + size.x, size.y)),
                };

                x += size.x + GAP * room;
                screen
            })
            .collect();

        let right = screens.last().map_or(0.0, |screen| screen.area.max.x);
        let tallest = screens
            .iter()
            .map(|screen| screen.area.max.y)
            .fold(0.0, f32::max);
        let extent = Extent::new(
            v(-LEFT * room, -BELOW * room),
            v(right + RIGHT * room, tallest + ABOVE * room),
        );

        let sources: Vec<&str> = screens
            .iter()
            .map(|screen| screen.panel.size.source)
            .collect();
        let measured_by = if sources.iter().all(|source| *source == "edid") {
            "EDID"
        } else if sources.iter().all(|source| *source == "override") {
            "displays.toml"
        } else {
            "EDID AND displays.toml"
        };
        let unknown = machine.displays().count() - screens.len();
        let mut notes = vec![
            format!("ACTIVE AREAS FROM {measured_by}"),
            "DIAGONALS IN INCHES".into(),
            "PITCH: PIXEL CENTRE TO CENTRE".into(),
            format!("SCAN LINES SLOWED {SLOWED} TIMES"),
        ];

        if unknown > 0 {
            notes.push(format!(
                "{} OF UNKNOWN SIZE NOT SHOWN",
                counted(unknown, "DISPLAY", "DISPLAYS")
            ));
        }

        let card = Card {
            title: format!("{} DISPLAYS", machine.chassis.kind.label()),
            number: "QD-C-0002".into(),
            domain: Domain::Computing,
            unit: Unit::Millimetre,
            scaled: true,
            view: "FRONT VIEW, ACTIVE AREAS".into(),
            notes,
            revisions: vec![Revision::first()],
            parts: screens.iter().map(part).collect(),
        };

        // The graphics driving the displays, if it measures its temperature.
        let temperature = screens
            .iter()
            .filter_map(|screen| screen.connector.gpu)
            .flat_map(|gpu| machine.sensors_on(Site::Device(gpu)))
            .find(|(_, sensor)| sensor.kind == SensorKind::Temperature)
            .map(|(index, _)| index);

        Some(Self {
            card,
            machine: machine.clone(),
            screens,
            extent,
            room,
            temperature,
        })
    }

    /// A display's active area, its dimensions and what is lettered on it.
    fn screen(&self, d: &mut Draft, screen: &Screen) {
        let area = screen.area;
        let (low, high) = (area.min, area.max);
        let middle = area.centre();
        let overrun = OVERRUN * self.room;

        // Its edges, centre lines and diagonal are hundreds of times longer
        // than its detail is wide: the detail draws its corner itself.
        d.in_main(|d| {
            d.rect(low, high, Line::Outline);
            d.line(
                v(middle.x, low.y - overrun),
                v(middle.x, high.y + overrun),
                Line::Centre,
            );
            d.line(
                v(low.x - overrun, middle.y),
                v(high.x + overrun, middle.y),
                Line::Centre,
            );
            d.arrow(middle, high, Line::Thin);
            d.arrow(middle, low, Line::Thin);
        });

        d.dim_h(low, v(high.x, low.y), -OFFSET * self.room);
        d.dim_v(low, v(low.x, high.y), -HEIGHT * self.room);
    }

    /// What the detail of `screen` shows: its corner, the frame beyond it,
    /// the pixels from (0, 0) with their centres, and their pitch.
    fn corner(&self, d: &mut Draft, index: usize, screen: &Screen) {
        let origin = screen.origin();
        let pitch = screen.pitch();
        let (_, radius) = screen.detail();
        let reach = GRID * radius;
        let columns = (reach / pitch.x).ceil() as usize;
        let rows = (reach / pitch.y).ceil() as usize;
        let (right, bottom) = (origin.x + reach, origin.y - reach);
        let first = origin + v(pitch.x, -pitch.y) / 2.0;

        d.in_detail(|d| {
            // Outside the active area, the frame round it.
            d.area(
                &[
                    origin + v(-reach, reach),
                    v(right, origin.y + reach),
                    v(right, origin.y),
                    origin,
                    v(origin.x, bottom),
                    v(origin.x - reach, bottom),
                ],
                Fill::Tint(2),
            );

            for column in 1..=columns {
                let x = origin.x + column as f32 * pitch.x;
                d.line(v(x, origin.y), v(x, bottom), Line::Thin);
            }

            for row in 1..=rows {
                let y = origin.y - row as f32 * pitch.y;
                d.line(v(origin.x, y), v(right, y), Line::Thin);
            }

            for column in 0..columns {
                for row in 0..rows {
                    let at = first + v(column as f32 * pitch.x, -(row as f32) * pitch.y);
                    d.dot(at, 2).tone(Tone::Muted);
                }
            }

            d.part(index, |d| {
                d.line(origin, v(right, origin.y), Line::Outline);
                d.line(origin, v(origin.x, bottom), Line::Outline);

                // From the centre of pixel (0, 0) to the next one's, over
                // the edge.
                d.dim_h(first, first + v(pitch.x, 0.0), 1.1 * pitch.y)
                    .text(format!("{:.3}", pitch.x));

                // Most pixels are square; a pitch down that differs from
                // the pitch across is dimensioned too.
                if (pitch.x - pitch.y).abs() > pitch.x * 0.005 {
                    d.dim_v(first - v(0.0, pitch.y), first, -1.1 * pitch.x)
                        .text(format!("{:.3}", pitch.y));
                }
            });
        });
    }

    /// What is lettered at a display's centre: its connector, diagonal and
    /// resolution. Drawn with what moves, after the scan line, so the line
    /// passes behind it.
    fn lettering(&self, d: &mut Draft, screen: &Screen) {
        let middle = screen.area.centre();
        let pixels = screen.panel.pixels;

        d.label(middle, screen.inches()).tone(Tone::Ink);
        d.label(middle, format!("{} × {}", pixels.0, pixels.1))
            .nudge(0, 12);
        d.label(middle, screen.connector.name.as_str())
            .nudge(0, -12);
    }

    /// The scan line of `screen` at `t`: down the display at its refresh
    /// rate, slowed.
    fn scan(&self, d: &mut Draft, screen: &Screen, t: f32) {
        let hertz = screen
            .panel
            .refresh
            .filter(|hertz| hertz.is_finite() && *hertz > 0.0)
            .unwrap_or(60.0);
        let down = (t * hertz / SLOWED).fract();
        let area = screen.area;
        let y = area.max.y - down * area.height();

        d.in_main(|d| {
            d.line(v(area.min.x, y), v(area.max.x, y), Line::Trace);
        });
    }
}

/// The parts list's item for `screen`.
fn part(screen: &Screen) -> Part {
    let panel = &screen.panel;
    let connector = &screen.connector;
    let place = match connector.kind {
        ConnectorKind::Internal => "BUILT-IN",
        _ => "EXTERNAL",
    };
    let output = match connector.gpu {
        Some(gpu) => format!("{place}, PCI {}", short(gpu)),
        None => place.into(),
    };
    // The maker's PNP ID, unless the name already starts with it.
    let name = panel.name.as_deref().map(lettered);
    let maker = panel.maker.as_deref().map(lettered).filter(|maker| {
        !name
            .as_deref()
            .is_some_and(|name| name.starts_with(maker.as_str()))
    });
    let model: Vec<String> = [maker, name].into_iter().flatten().collect();
    let model = match (model.is_empty(), panel.year) {
        (true, Some(year)) => format!("MADE {year}"),
        (false, Some(year)) => format!("{} ({year})", model.join(" ")),
        (false, None) => model.join(" "),
        (true, None) => String::new(),
    };
    let refresh = panel
        .refresh
        .map(|hertz| format!(" AT {} Hz", number(hertz.round())))
        .unwrap_or_default();
    let pitch = panel.pitch_mm();

    let mut spec = vec![("OUTPUT".to_owned(), output)];

    if !model.is_empty() {
        spec.push(("MODEL".into(), model));
    }

    spec.extend([
        (
            "PIXELS".into(),
            format!("{} × {}{refresh}", panel.pixels.0, panel.pixels.1),
        ),
        (
            "DIAGONAL".into(),
            format!(
                "{:.1} IN, {} mm",
                panel.inches(),
                number(panel.size.diagonal_mm as f32)
            ),
        ),
        (
            "PITCH".into(),
            format!("{pitch:.3} mm, {:.0} PPI", 25.4 / pitch),
        ),
        ("ASPECT".into(), aspect(panel.pixels)),
    ]);

    // A name or model longer than the column is cut, not wrapped: the
    // column has a row for each.
    let spec = spec
        .into_iter()
        .map(|(name, value)| {
            let room = SPEC_ROOM - name.chars().count() - 2;
            let value = fit(&value, room);
            (name, value)
        })
        .collect();
    let (centre, radius) = screen.detail();
    let mut part = Part::new(
        &fit(&connector.name, 14),
        1,
        &format!("{:.1} IN", panel.inches()),
    )
    .detail(centre, radius);

    part.spec = spec;
    part
}

/// The shape of a display of `pixels`, as displays are described: `16:9`,
/// `16:10` (rather than 8:5), or `1.78:1` when its whole ratio is unwieldy.
fn aspect((width, height): (u32, u32)) -> String {
    fn gcd(a: u32, b: u32) -> u32 {
        if b == 0 { a } else { gcd(b, a % b) }
    }

    let common = gcd(width, height).max(1);
    let (across, down) = (width / common, height / common);

    match (across, down) {
        (8, 5) => "16:10".into(),
        (across, down) if across <= 50 && down <= 50 => format!("{across}:{down}"),
        _ => format!("{:.2}:1", width as f32 / height.max(1) as f32),
    }
}

impl Subject for Displays {
    fn name(&self) -> &'static str {
        "displays"
    }

    fn card(&self) -> &Card {
        &self.card
    }

    fn extent(&self) -> Extent {
        self.extent
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        for (index, screen) in self.screens.iter().enumerate() {
            d.part(index, |d| self.screen(d, screen));
            self.corner(d, index, screen);
            d.moving(|d| {
                d.part(index, |d| {
                    self.scan(d, screen, t);
                    self.lettering(d, screen);
                });
            });
        }

        for (index, screen) in self.screens.iter().enumerate() {
            let area = screen.area;

            d.part(index, |d| {
                d.balloon(index, v(area.min.x + area.width() * 0.8, area.max.y), Auto);
            });
        }
    }

    fn readings(&self, t: f32) -> Vec<Reading> {
        let pixels: f64 = self
            .screens
            .iter()
            .map(|screen| f64::from(screen.panel.pixels.0) * f64::from(screen.panel.pixels.1))
            .sum();
        let area: f32 = self
            .screens
            .iter()
            .map(|screen| screen.area.width() * screen.area.height())
            .sum();
        let mut readings = vec![
            Reading::new("DISPLAYS", self.screens.len().to_string()),
            Reading::new("PIXELS", format!("{:.1} M", pixels / 1e6)),
            Reading::new("AREA", format!("{:.2} m²", area / 1e6)),
        ];

        if let Some(celsius) = self
            .temperature
            .and_then(|index| self.machine.sample(t).sensor(index))
            .filter(|celsius| celsius.is_finite())
        {
            readings.push(Reading::new("GPU", format!("{celsius:.0} °C")));
        }

        readings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_is_drawn_to_scale_side_by_side() {
        let displays = Displays::new(&Machine::fixture()).unwrap();
        let card = displays.card();
        let names: Vec<&str> = card.parts.iter().map(|p| p.name.as_str()).collect();
        let values: Vec<&str> = card.parts.iter().map(|p| p.material.as_str()).collect();

        assert_eq!(card.title, "LAPTOP DISPLAYS");
        assert_eq!(names, ["eDP-1", "DP-1"]);
        assert_eq!(values, ["14.0 IN", "27.0 IN"]);

        // Each the size its EDID says, standing on one line, apart.
        let [own, monitor] = &displays.screens[..] else {
            panic!("two displays");
        };

        assert!((own.area.width() - 301.6).abs() < 0.1, "{:?}", own.area);
        assert!((monitor.area.height() - 336.2).abs() < 0.1);
        assert_eq!((own.area.min.y, monitor.area.min.y), (0.0, 0.0));
        assert!(monitor.area.min.x > own.area.max.x);
    }

    /// The specification's rows fit the laptop's column, name and value
    /// apart, and say the diagonal and the pitch.
    #[test]
    fn the_specifications_fit_their_column() {
        let displays = Displays::new(&Machine::fixture()).unwrap();

        for part in &displays.card().parts {
            for (name, value) in &part.spec {
                assert!(
                    name.chars().count() + value.chars().count() + 2 <= SPEC_ROOM,
                    "{}: {name} {value}",
                    part.name
                );
            }
        }

        let spec = &displays.card().parts[0].spec;
        let row = |name: &str| {
            spec.iter()
                .find(|(n, _)| n == name)
                .map(|(_, value)| value.as_str())
        };

        assert_eq!(row("DIAGONAL"), Some("14.0 IN, 355.6 mm"));
        assert_eq!(row("PITCH"), Some("0.118 mm, 216 PPI"));
        assert_eq!(row("PIXELS"), Some("2560 × 1600 AT 120 Hz"));
        assert_eq!(row("ASPECT"), Some("16:10"));
        assert_eq!(row("MODEL"), Some("GEN (2025)"));
    }

    /// A display is named by its maker's PNP ID and the name it gives
    /// itself, the ID left out when the name starts with it.
    #[test]
    fn models_are_named_once() {
        let mut machine = Machine::fixture();
        let model = |machine: &Machine| {
            let displays = Displays::new(machine).unwrap();
            let spec = &displays.card().parts[1].spec;
            spec.iter()
                .find(|(name, _)| name == "MODEL")
                .map(|(_, value)| value.clone())
        };

        assert_eq!(model(&machine).as_deref(), Some("GEN 27 MONITOR (2024)"));

        let panel = machine.connectors[1].panel.as_mut().expect("The monitor");
        panel.maker = Some("ACM".into());
        panel.name = Some("Acme Q27".into());

        assert_eq!(model(&machine).as_deref(), Some("ACME Q27 (2024)"));
    }

    /// The detail's grid is the display's own: lines a pitch apart from the
    /// corner, drawn only in the detail, where the long edges are not.
    #[test]
    fn the_detail_shows_the_pixel_grid_at_its_pitch() {
        use crate::draft::{Ink, Scope, Shape};

        let displays = Displays::new(&Machine::fixture()).unwrap();
        let screen = &displays.screens[0];
        let mut draft = Draft::new();
        displays.draw(&mut draft, 3.0);

        let origin = screen.origin();
        let mut columns: Vec<f32> = draft
            .marks()
            .iter()
            .filter(|mark| mark.scope == Scope::Detail)
            .filter_map(|mark| match &mark.ink {
                Ink::Stroke {
                    shape: Shape::Polyline { points, .. },
                    line: Line::Thin,
                } if points[0].x == points[1].x && points[0].y == origin.y => {
                    Some(points[0].x - origin.x)
                }
                _ => None,
            })
            .collect();
        columns.sort_by(f32::total_cmp);

        let pitch = screen.pitch().x;
        assert!(columns.len() > 8);
        for (k, x) in columns.iter().take(8).enumerate() {
            assert!((x - (k + 1) as f32 * pitch).abs() < 1e-4, "{x}");
        }

        // Nothing longer than the detail is wide is magnified in it.
        let (_, radius) = screen.detail();
        for mark in draft.marks().iter().filter(|mark| mark.magnified()) {
            if let Ink::Stroke {
                shape: Shape::Polyline { points, .. },
                ..
            } = &mark.ink
            {
                let span = points[0].distance(points[points.len() - 1]);
                assert!(span <= 2.0 * GRID * radius, "{mark:?}");
            }
        }
    }

    #[test]
    fn displays_of_unknown_size_are_left_out() {
        let mut machine = Machine::fixture();

        for connector in &mut machine.connectors {
            if let Some(panel) = &mut connector.panel {
                panel.size.estimated = true;
            }
        }

        assert!(Displays::new(&machine).is_none());

        machine.connectors[1]
            .panel
            .as_mut()
            .expect("The monitor")
            .size
            .estimated = false;
        let displays = Displays::new(&machine).unwrap();

        assert_eq!(displays.screens.len(), 1);
        assert!(
            displays
                .card()
                .notes
                .iter()
                .any(|note| note == "1 DISPLAY OF UNKNOWN SIZE NOT SHOWN")
        );
    }

    #[test]
    fn aspects_are_written_as_displays_are_described() {
        assert_eq!(aspect((2560, 1600)), "16:10");
        assert_eq!(aspect((3840, 2160)), "16:9");
        assert_eq!(aspect((3440, 1440)), "43:18");
        assert_eq!(aspect((2256, 1504)), "3:2");
        assert_eq!(aspect((1366, 768)), "1.78:1");
    }

    /// A faster display's scan line sweeps more often: the line is where
    /// its refresh, slowed, has taken it.
    #[test]
    fn scan_lines_run_at_the_refresh_rate_slowed() {
        let displays = Displays::new(&Machine::fixture()).unwrap();
        let line = |index: usize, t: f32| {
            let mut draft = Draft::new();
            displays.scan(&mut draft, &displays.screens[index], t);
            match &draft.marks()[0].ink {
                crate::draft::Ink::Stroke {
                    shape: crate::draft::Shape::Polyline { points, .. },
                    ..
                } => points[0].y,
                _ => unreachable!(),
            }
        };

        // 120 Hz slowed 240 times: a sweep every 2 s, half of it in 1 s.
        let own = &displays.screens[0].area;
        assert!((line(0, 1.0) - own.centre().y).abs() < 1e-3);
        // 60 Hz: a sweep every 4 s.
        let monitor = &displays.screens[1].area;
        assert!((line(1, 2.0) - monitor.centre().y).abs() < 1e-3);
    }
}
