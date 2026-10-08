//! The displays at their true sizes: each one's active area as its EDID
//! measures it, dimensioned in millimetres with its diagonal in inches,
//! and named under its width by its connector and resolution. A detail
//! magnifies its top left corner, where pixel (0, 0) is, far enough to show
//! the pixel grid at the display's pitch.
//!
//! The displays stand side by side on one baseline, or in rows where rows
//! let the view draw them larger: a laptop's own panel over a large monitor
//! is drawn twice the size it would be beside it. Their lettering is a
//! whole number of pixels at any scale, so they are laid out for the view
//! they are drawn in (see [`Subject::fitted`]), the room round them in its
//! pixels at the scale it draws them at; until a sheet says which view, for
//! the laptop's.
//!
//! What moves is each display's scan line, running down it at its refresh
//! rate slowed down enough to follow, so a faster display sweeps more often.
use std::cell::RefCell;
use std::iter::successors;
use std::rc::Rc;

use crate::draft::Placement::{self, Auto};
use crate::draft::raster::{BALLOON, LETTERING};
use crate::draft::scale::Ratio;
use crate::draft::{Draft, Extent, Fill, Line, Tone, V2, number, v};
use crate::machine::{Connector, ConnectorKind, Machine, Panel, SensorKind, Site};

use super::super::{Card, Domain, Part, Reading, Revision, Room, Subject, Unit};
use super::layout::{BUDGET, LINE, short};
use super::{SPEC_ROOM, chain, counted, fit, lettered};

/// The room the displays are laid out for until a sheet says which: the
/// laptop's view, [`BUDGET`] virtual pixels of 0.404 mm on its glass.
const LAPTOP: Room = Room {
    view: BUDGET,
    mm_per_vpx: 0.404,
};

/// The room round the displays, in pixels of the view: between lettering
/// and the lines it keeps clear of...
const CLEAR: f32 = 3.0;
/// ...how far a centre line runs past its display...
const OVERRUN: f32 = 4.0;
/// ...a width's dimension line's distance under its display, its value
/// lettered across the line clear of the centre line's end...
const UNDER: f32 = 13.0;
/// ...the room over a row, for its balloons...
const OVER: f32 = 32.0;
/// ...and how far a balloon beside a display is from its edge.
const ASIDE: f32 = 18.0;

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
    /// A pixel of the view they are laid out for at the scale it draws
    /// them at, in millimetres: the unit of the room round them.
    pixel: f32,
    /// The sensor the graphics' temperature is read from.
    temperature: Option<usize>,
    /// The displays laid out for each room a sheet has asked for.
    fitted: RefCell<Vec<(Room, Rc<Displays>)>>,
}

/// A display as the sheet draws it.
struct Screen {
    connector: Connector,
    panel: Panel,
    /// Its active area in millimetres, standing on its row's baseline.
    area: Extent,
    /// What is lettered under its width: its connector and resolution, on
    /// one line, or on two if one is wider than the display.
    caption: Vec<String>,
    /// How far left of it its height is dimensioned, in millimetres.
    height: f32,
    /// Whether its diagonal's value is lettered off it on a leader, as it
    /// would not clear its edges by a line inside it.
    leader: bool,
    /// What its balloon points at, and where the balloon goes.
    balloon: (V2, Placement),
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
}

/// The diagonal as a display is sold by: `14.0"`.
fn inches(panel: &Panel) -> String {
    format!("{:.1}\"", panel.inches())
}

/// What a display is lettered with, which the room round it is for.
struct Letters {
    /// Its active area's width and height, in millimetres.
    size: V2,
    /// Its height's value, its width's and its diagonal's...
    height: String,
    width: String,
    diagonal: String,
    /// ...and its connector and resolution.
    name: String,
    pixels: String,
}

impl Letters {
    /// What `panel`, on `connector`, is lettered with.
    fn of(connector: &Connector, panel: &Panel) -> Self {
        let size = v(panel.size.width_mm as f32, panel.size.height_mm as f32);

        Self {
            size,
            height: number(size.y),
            width: number(size.x),
            diagonal: inches(panel),
            name: connector.name.clone(),
            pixels: format!("{} × {}", panel.pixels.0, panel.pixels.1),
        }
    }
}

/// Where a display goes, and how it is lettered there.
struct Spot {
    area: Extent,
    caption: Vec<String>,
    height: f32,
    leader: bool,
    balloon: (V2, Placement),
}

/// The displays laid out for a view at one scale.
struct Laid {
    ratio: f64,
    /// The view, in its pixels, and its pixel at that scale in
    /// millimetres.
    view: V2,
    pixel: f32,
    rows: usize,
    spots: Vec<Spot>,
    extent: Extent,
}

impl Laid {
    /// How much of the view it takes, across or down, whichever is more:
    /// no more than all of it if it fits.
    fn fill(&self) -> f32 {
        let size = v(self.extent.width(), self.extent.height()) / self.pixel;

        (size.x / self.view.x).max(size.y / self.view.y)
    }
}

/// `displays` laid out in the rows that let a view of `room` draw them at
/// the largest preferred scale, and in as few as do: side by side, unless
/// rows draw them larger.
fn arranged(displays: &[Letters], room: Room) -> Laid {
    let ratios: Vec<f64> = successors(Ratio::preferred_at_most(10.0), |ratio| {
        Ratio::preferred_at_most(ratio.0 * (1.0 - 1e-6))
    })
    .map(|ratio| ratio.0)
    .take_while(|&ratio| ratio >= 1e-4)
    .collect();
    let rowings = rowings(displays.len());

    rowings
        .iter()
        .filter_map(|rows| {
            ratios
                .iter()
                .map(|&ratio| laid_out(displays, rows, ratio, room))
                .find(|laid| laid.fill() <= 1.0)
        })
        .max_by(|a, b| {
            a.ratio
                .total_cmp(&b.ratio)
                .then(b.rows.cmp(&a.rows))
                .then(b.fill().total_cmp(&a.fill()))
        })
        .unwrap_or_else(|| {
            // None fits: as small as the sheet draws them, the rows that
            // overflow the view least.
            let smallest = ratios[ratios.len() - 1];

            rowings
                .iter()
                .map(|rows| laid_out(displays, rows, smallest, room))
                .min_by(|a, b| a.fill().total_cmp(&b.fill()))
                .expect("A display")
        })
}

/// Every way of putting `n` displays in rows in their order, as how many
/// each row holds; past a dozen displays, rows that hold alike only.
fn rowings(n: usize) -> Vec<Vec<usize>> {
    if n > 12 {
        return (1..=n)
            .map(|rows| {
                (0..rows)
                    .map(|row| n * (row + 1) / rows - n * row / rows)
                    .collect()
            })
            .collect();
    }

    (0..1_u32 << (n - 1))
        .map(|breaks| {
            let mut rows = vec![1];

            for after in 0..n - 1 {
                if breaks >> after & 1 == 1 {
                    rows.push(1);
                } else {
                    *rows.last_mut().expect("A row") += 1;
                }
            }

            rows
        })
        .collect()
}

/// `displays` in `rows` (how many each holds, in order), left-aligned, the
/// first row at the top, each on its baseline, with the room their
/// lettering takes in a view of `room` drawing them at `ratio`.
fn laid_out(displays: &[Letters], rows: &[usize], ratio: f64, room: Room) -> Laid {
    let pixel = (room.mm_per_vpx / ratio) as f32;
    let wide = |text: &str| f32::from(LETTERING.width(text));
    let cap = f32::from(LETTERING.cap());
    // Of several rows, the last display of each has its balloon beside its
    // right edge, where the paper is open by every row; any other over its
    // top edge, in the room over its row.
    let beside = rows.len() > 1;
    let aside = if beside {
        ASIDE + BALLOON as f32 + 1.0 + CLEAR
    } else {
        0.0
    };

    // What each display takes round it, in pixels: its height's value
    // lettered across its dimension line, the line clear of the centre
    // line's end; its caption, and how far that or its width's value, past
    // the end of its dimension if the display is too narrow to hold it,
    // spills past its sides.
    struct Needs {
        caption: Vec<String>,
        offset: f32,
        left: f32,
        spill: f32,
        past: f32,
        leader: bool,
    }

    let needs: Vec<Needs> = displays
        .iter()
        .map(|display| {
            let size = display.size / pixel;
            let half = wide(&display.height) / 2.0;
            let offset = half + 1.0 + OVERRUN + CLEAR;
            let one = format!("{}  {}", display.name, display.pixels);
            let caption = if wide(&one) <= size.x {
                vec![one]
            } else {
                vec![display.name.clone(), display.pixels.clone()]
            };
            let widest = caption.iter().map(|line| wide(line)).fold(0.0, f32::max);
            let width = wide(&display.width);

            Needs {
                offset,
                left: offset + half + 2.0,
                spill: ((widest + 3.0 - size.x) / 2.0).max(0.0),
                past: if size.x >= width + 12.0 {
                    0.0
                } else {
                    width + 6.0
                },
                leader: size.x < wide(&display.diagonal) + 3.0 + 2.0 * LINE
                    || size.y < cap + 4.0 + 2.0 * LINE,
                caption,
            }
        })
        .collect();
    let gap = |a: &Needs, b: &Needs| {
        (OVERRUN + CLEAR + b.left)
            .max(a.spill + b.spill + CLEAR)
            .max(a.past + CLEAR)
    };

    let mut spots = Vec::with_capacity(displays.len());
    let (mut left, mut right) = (0.0_f32, 0.0_f32);
    // The top of the next row's room, from the top of the drawing down.
    let mut top = 0.0;
    let mut start = 0;

    for &count in rows {
        let row = start..start + count;
        let tallest = row.clone().map(|i| displays[i].size.y).fold(0.0, f32::max);
        let over = if beside && count == 1 {
            LINE / 2.0
        } else {
            OVER
        };
        let baseline = top - over * pixel - tallest;
        let mut x = 0.0;
        let mut lines = 1;

        for i in row.clone() {
            if i > start {
                x += gap(&needs[i - 1], &needs[i]) * pixel;
            }

            let size = displays[i].size;
            // Over its top edge on the right, unless its diagonal's value
            // goes there on a leader.
            let along = if needs[i].leader { 0.2 } else { 0.8 };
            let balloon = if beside && i == row.end - 1 {
                (
                    v(x + size.x, baseline + size.y * 0.75),
                    Placement::Offset(ASIDE as i32, 0),
                )
            } else {
                (v(x + size.x * along, baseline + size.y), Auto)
            };

            spots.push(Spot {
                area: Extent::new(v(x, baseline), v(x + size.x, baseline + size.y)),
                caption: needs[i].caption.clone(),
                height: needs[i].offset * pixel,
                leader: needs[i].leader,
                balloon,
            });
            x += size.x;
            lines = lines.max(needs[i].caption.len());
        }

        let (first, last) = (&needs[start], &needs[row.end - 1]);

        left = left.max(first.left.max(first.spill) + CLEAR);
        right = right.max(x + (OVERRUN.max(last.spill).max(last.past) + CLEAR).max(aside) * pixel);
        top = baseline - (UNDER + LINE * lines as f32 + LINE / 2.0) * pixel;
        start = row.end;
    }

    Laid {
        ratio,
        view: room.view,
        pixel,
        rows: rows.len(),
        spots,
        extent: Extent::new(v(-left * pixel, top), v(right, 0.0)),
    }
}

impl Displays {
    /// The displays of `machine` whose sizes are known, if any are, laid
    /// out for the laptop's view.
    pub fn new(machine: &Machine) -> Option<Self> {
        Self::for_room(machine, LAPTOP)
    }

    /// The displays of `machine` whose sizes are known laid out for a view
    /// of `room`.
    fn for_room(machine: &Machine, room: Room) -> Option<Self> {
        let measured: Vec<(&Connector, &Panel)> = machine
            .displays()
            .filter(|(_, panel)| !panel.size.estimated)
            .collect();

        if measured.is_empty() {
            return None;
        }

        let letters: Vec<Letters> = measured
            .iter()
            .map(|&(connector, panel)| Letters::of(connector, panel))
            .collect();
        let laid = arranged(&letters, room);
        let screens: Vec<Screen> = measured
            .iter()
            .zip(laid.spots)
            .map(|(&(connector, panel), spot)| Screen {
                connector: connector.clone(),
                panel: panel.clone(),
                area: spot.area,
                caption: spot.caption,
                height: spot.height,
                leader: spot.leader,
                balloon: spot.balloon,
            })
            .collect();

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
            extent: laid.extent,
            pixel: laid.pixel,
            temperature,
            fitted: RefCell::default(),
        })
    }

    /// A display's active area, its dimensions and its name.
    fn screen(&self, d: &mut Draft, screen: &Screen) {
        let area = screen.area;
        let (low, high) = (area.min, area.max);
        let middle = area.centre();
        let overrun = OVERRUN * self.pixel;
        let under = UNDER * self.pixel;

        // Its edges and centre lines are hundreds of times longer than its
        // detail is wide: the detail draws its corner itself.
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
        });

        d.dim_h(low, v(high.x, low.y), -under);
        d.dim_v(low, v(low.x, high.y), -screen.height);

        // Under its width's value, as a view's name is under the view.
        for (line, text) in screen.caption.iter().enumerate() {
            d.label(v(middle.x, low.y - under), text.as_str())
                .nudge(0, LINE as i32 * (line as i32 + 1));
        }

        // From the diagonal toward its top corner, where it is clear of
        // the dimensions and the balloon keeps to the other side.
        if screen.leader {
            d.note(low.lerp(high, 0.7), Auto, inches(&screen.panel))
                .tone(Tone::Ink);
        }
    }

    /// The diagonal, dimensioned corner to corner with its value in its
    /// middle, or there without it if that is lettered on a leader. Drawn
    /// with what moves, after the scan line, so the line passes behind it.
    fn diagonal(&self, d: &mut Draft, screen: &Screen) {
        let text = if screen.leader {
            String::new()
        } else {
            inches(&screen.panel)
        };

        d.dim_diagonal(screen.area.min, screen.area.max).text(text);
    }

    /// What the detail of `screen` shows: its corner, the frame beyond it,
    /// the pixels from (0, 0) with their centres, and their pitch.
    ///
    /// The grid is set on the sheet's pixels a pixel of the display at a
    /// time from the corner, so each is as many of the sheet's across and
    /// down as the next, its centre in the same place in it, and the pitch
    /// is dimensioned from one centre to the next as they are set.
    fn corner(&self, d: &mut Draft, index: usize, screen: &Screen) {
        let origin = screen.origin();
        let pitch = screen.pitch();
        let (across, down) = (v(pitch.x, 0.0), v(0.0, -pitch.y));
        let (_, radius) = screen.detail();
        let reach = GRID * radius;
        let columns = (reach / pitch.x).ceil() as usize;
        let rows = (reach / pitch.y).ceil() as usize;
        let (right, bottom) = (origin.x + reach, origin.y - reach);
        let first = origin + (across + down) / 2.0;

        d.in_detail(|d| {
            d.snapped(origin, |d| {
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

                d.part(index, |d| {
                    d.line(origin, v(right, origin.y), Line::Outline);
                    d.line(origin, v(origin.x, bottom), Line::Outline);
                });
            });

            for column in 1..=columns {
                chain(d, origin, across, column, |d| {
                    let x = origin.x + column as f32 * pitch.x;
                    d.line(v(x, origin.y), v(x, bottom), Line::Thin);
                });
            }

            for row in 1..=rows {
                chain(d, origin, down, row, |d| {
                    let y = origin.y - row as f32 * pitch.y;
                    d.line(v(origin.x, y), v(right, y), Line::Thin);
                });
            }

            for column in 0..columns {
                let top = origin + across * column as f32;

                chain(d, origin, across, column, |d| {
                    for row in 0..rows {
                        let corner = top + down * row as f32;

                        chain(d, top, down, row, |d| {
                            d.dot(corner + (across + down) / 2.0, 3).tone(Tone::Muted);
                        });
                    }
                });
            }

            d.part(index, |d| {
                d.snapped(origin, |d| {
                    d.snapped(first, |d| {
                        // From the centre of pixel (0, 0) to the next
                        // one's, over the edge.
                        d.dim_h(first, first + across, 1.1 * pitch.y)
                            .text(format!("{:.3}", pitch.x));

                        // Most pixels are square; a pitch down that differs
                        // from the pitch across is dimensioned too.
                        if (pitch.x - pitch.y).abs() > pitch.x * 0.005 {
                            d.dim_v(first + down, first, -1.1 * pitch.x)
                                .text(format!("{:.3}", pitch.y));
                        }
                    });
                });
            });
        });
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

    fn fitted(&self, room: Room) -> Option<Rc<dyn Subject>> {
        let mut fitted = self.fitted.borrow_mut();

        if let Some((_, displays)) = fitted.iter().find(|(made, _)| *made == room) {
            return Some(displays.clone());
        }

        let displays = Rc::new(Self::for_room(&self.machine, room)?);

        // A room for each output the sheet is drawn on, and a few more for
        // one that changes size.
        if fitted.len() >= 8 {
            fitted.remove(0);
        }

        fitted.push((room, displays.clone()));
        Some(displays)
    }

    fn draw(&self, d: &mut Draft, t: f32) {
        for (index, screen) in self.screens.iter().enumerate() {
            d.part(index, |d| self.screen(d, screen));
            self.corner(d, index, screen);
            d.moving(|d| {
                d.part(index, |d| {
                    self.scan(d, screen, t);
                    self.diagonal(d, screen);
                });
            });
        }

        for (index, screen) in self.screens.iter().enumerate() {
            d.part(index, |d| {
                d.balloon(index, screen.balloon.0, screen.balloon.1);
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
        assert_eq!(own.area.min.y, monitor.area.min.y);
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

    /// The displays are drawn as large as the view they are laid out for
    /// lets them be: side by side where rows would not draw them larger, in
    /// rows where they would, and never smaller for want of room for their
    /// lettering, which takes the view's pixels, not a share of the row.
    #[test]
    fn displays_are_put_in_rows_where_rows_draw_them_larger() {
        use crate::headless::Output;
        use crate::machine::Fixture;
        use crate::machine::tests::plugged;
        use crate::sheet::layout::Layout;

        let laid = |machine: &Machine, output: Output| {
            let (width, height) = output.virtual_size();
            let displays = Displays::new(machine).expect("Displays");
            let view = Layout::new(width as i32, height as i32, displays.card()).view;
            let room = Room {
                view: v(view.width as f32, view.height as f32),
                mm_per_vpx: output.display.mm_per_vpx,
            };
            let letters: Vec<Letters> = machine
                .displays()
                .map(|(connector, panel)| Letters::of(connector, panel))
                .collect();
            let laid = arranged(&letters, room);

            (laid.rows, Ratio(laid.ratio).label())
        };
        let mut beside = Fixture::Laptop.machine();

        for connector in &mut beside.connectors {
            if connector.kind != ConnectorKind::Internal {
                connector.panel = None;
            }
        }

        let beside = plugged(beside, "HDMI-A-1", (797.2, 333.7), (3440, 1440));
        let three = plugged(
            Fixture::Desktop.machine(),
            "DP-2",
            (597.7, 336.2),
            (3840, 2160),
        );
        let (laptop, ultrawide) = (Output::LAPTOP, Output::ULTRAWIDE);

        // A laptop beside a monitor twice its width: side by side on the
        // laptop, over it in two rows twice as large on the ultrawide.
        assert_eq!(laid(&Machine::fixture(), laptop), (1, "1:5".into()));
        assert_eq!(laid(&Machine::fixture(), ultrawide), (2, "1:2.5".into()));
        // Beside one wider still, or two monitors: rows draw them twice as
        // large as a row would on the laptop.
        assert_eq!(laid(&beside, laptop), (2, "1:5".into()));
        assert_eq!(laid(&Fixture::Desktop.machine(), laptop), (2, "1:5".into()));
        assert_eq!(
            laid(&Fixture::Desktop.machine(), ultrawide),
            (2, "1:2.5".into())
        );
        // Three monitors side by side, as large as rows would draw them;
        // room for their lettering as a share of the row had made it 1:20.
        assert_eq!(laid(&three, laptop), (1, "1:10".into()));
    }

    /// Displays can be put in rows every way that keeps them in order, and
    /// past a dozen in rows that hold alike.
    #[test]
    fn displays_are_put_in_rows_every_way() {
        let mut ways = rowings(3);
        ways.sort();

        assert_eq!(ways, [vec![1, 1, 1], vec![1, 2], vec![2, 1], vec![3]]);
        assert_eq!(rowings(1), [vec![1]]);
        assert_eq!(rowings(14).len(), 14);
        assert!(
            rowings(14)
                .iter()
                .all(|rows| rows.iter().sum::<usize>() == 14)
        );
    }

    /// The detail's pixel grid is set a pixel of the display at a time: at
    /// any magnification, wherever it falls on the sheet's grid, its pixels
    /// are as many of the sheet's as each other, each centre's dot is in
    /// the same place in its pixel, and the pitch is dimensioned from the
    /// middle of one dot to the middle of the next.
    #[test]
    fn the_detail_grid_is_even_at_any_magnification() {
        use std::collections::HashSet;

        use crate::draft::Ink;
        use crate::draft::raster::{Piece, Projection, rasterize};

        let displays = Displays::new(&Machine::fixture()).unwrap();

        for (index, screen) in displays.screens.iter().enumerate() {
            let mut draft = Draft::new();
            displays.corner(&mut draft, index, screen);

            let (origin, pitch) = (screen.origin(), screen.pitch().x);

            for (across, shift) in [
                (9.3, 0.3),
                (14.6, 0.5),
                (15.5, 0.8),
                (19.3, 0.1),
                (63.7, 0.6),
            ] {
                let scale = across / pitch;
                let projection = Projection::new(
                    (
                        100.0 + shift - origin.x * scale,
                        50.0 + shift + origin.y * scale,
                    ),
                    scale,
                );
                let (mut lines, mut dots, mut extensions) = (Vec::new(), Vec::new(), Vec::new());

                for mark in draft.marks() {
                    let mut pieces = Vec::new();
                    rasterize(mark, &projection, &mut pieces);

                    for inked in pieces {
                        let upright = |pixels: &[iced_core::Point<i32>]| {
                            pixels.len() > 1 && pixels.iter().all(|pixel| pixel.x == pixels[0].x)
                        };

                        match (&mark.ink, inked.piece) {
                            (Ink::Stroke { .. }, Piece::Path { pixels, .. })
                                if upright(&pixels) =>
                            {
                                lines.push(pixels[0].x);
                            }
                            (Ink::Dot { .. }, Piece::Block(block)) => dots.push(block),
                            (Ink::Dimension { .. }, Piece::Path { pixels, .. })
                                if upright(&pixels) =>
                            {
                                extensions.push(pixels[0].x);
                            }
                            _ => {}
                        }
                    }
                }

                lines.sort_unstable();
                lines.dedup();
                extensions.sort_unstable();

                let at = format!("{} at {across} pixels a pixel", screen.connector.name);
                let steps: HashSet<i32> = lines.windows(2).map(|pair| pair[1] - pair[0]).collect();
                let top = dots.iter().map(|dot| dot.y).min().expect("Dots");
                let mut first: Vec<_> = dots.iter().filter(|dot| dot.y == top).collect();
                first.sort_by_key(|dot| dot.x);
                let places: HashSet<i32> = first
                    .iter()
                    .map(|dot| {
                        let before = lines.iter().filter(|&&x| x < dot.x).max().expect("A line");
                        dot.x - before
                    })
                    .collect();

                assert_eq!(steps.len(), 1, "{at}: {lines:?}");
                assert_eq!(places.len(), 1, "{at}: {first:?}");
                assert_eq!(
                    extensions,
                    [first[0].x + 1, first[1].x + 1],
                    "{at}: {first:?}"
                );
            }
        }
    }
}
