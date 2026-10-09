//! Marks on the pixel grid.
//!
//! [`rasterize`] turns a [`Mark`] into [`Piece`]s: paths whose pixels are in
//! the order a pen would draw them, rows of an area, lines of text. The order
//! is what lets a sheet plot a drawing a pixel at a time, and lets a line type
//! be a pattern counted along the path, so a dashed circle is dashed the same
//! way round as a dashed line is along.
use std::collections::HashMap;

use iced_core::{Color, Point, Rectangle};
use quadrille::draw::{Anchor, Horizontal, Pen, Polygon, Vertical, shape};
use quadrille::{Face, Palette};

use super::{
    Axis, Characteristic, Extent, Fill, Ink, Line, Mark, Measure, Placement, Shape, Tone, V2,
    polar, v,
};

/// Where a model lands on the sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    /// The pixel the model's origin falls on.
    pub origin: (f32, f32),
    /// Pixels per model unit.
    pub scale: f32,
    /// A point of the model and the pixel it is set on, for a figure set on
    /// the grid as one (see [`Draft::snapped`](super::Draft::snapped)):
    /// every point lands that pixel plus its offset from the point, rounded.
    pub snap: Option<(V2, Point<i32>)>,
}

impl Projection {
    pub const fn new(origin: (f32, f32), scale: f32) -> Self {
        Self {
            origin,
            scale,
            snap: None,
        }
    }

    /// The projection of a figure set on the grid by `at`, inside the
    /// figure this one sets, if any.
    pub fn snapped(&self, at: V2) -> Self {
        Self {
            snap: Some((at, self.px(at))),
            ..*self
        }
    }

    /// `extent` centred in `area` at `scale` pixels per unit. The origin is
    /// put on a whole pixel, so a model point a whole number of pixels from
    /// it lands exactly.
    pub fn centred(extent: Extent, area: Rectangle<i32>, scale: f32) -> Self {
        let centre = extent.centre();
        let x = area.x as f32 + area.width as f32 / 2.0;
        let y = area.y as f32 + area.height as f32 / 2.0;

        Self::new(
            (
                (x - centre.x * scale).round(),
                (y + centre.y * scale).round(),
            ),
            scale,
        )
    }

    pub fn px(&self, point: V2) -> Point<i32> {
        match self.snap {
            Some((at, pixel)) => {
                let offset = steady(point - at) * self.scale;

                Point::new(
                    pixel.x + offset.x.round() as i32,
                    pixel.y - offset.y.round() as i32,
                )
            }
            None => Point::new(
                (self.origin.0 + point.x * self.scale).round() as i32,
                (self.origin.1 - point.y * self.scale).round() as i32,
            ),
        }
    }

    /// The same point, not rounded: for directions and placements.
    fn exact(&self, point: V2) -> V2 {
        match self.snap {
            Some((at, pixel)) => {
                let offset = steady(point - at) * self.scale;

                v(pixel.x as f32 + offset.x, pixel.y as f32 - offset.y)
            }
            None => v(
                self.origin.0 + point.x * self.scale,
                self.origin.1 - point.y * self.scale,
            ),
        }
    }

    pub fn length(&self, length: f32) -> i32 {
        (length * self.scale).round() as i32
    }
}

/// An offset in the model to a 64th of a unit, so the rounding error of
/// working it out from two points far from the origin cannot round the
/// same offset either way at different places: an offset and its opposite
/// are rounded alike.
fn steady(offset: V2) -> V2 {
    (offset * 64.0).round() / 64.0
}

/// Which pixels of a path are inked, counted along it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stipple {
    Solid,
    Dash {
        on: u8,
        off: u8,
    },
    /// A long dash, then `shorts` short dashes, each with a gap either side.
    Chain {
        long: u8,
        gap: u8,
        short: u8,
        shorts: u8,
    },
}

impl Stipple {
    pub fn of(line: Line) -> Self {
        match line {
            Line::Outline | Line::Thin | Line::Trace => Self::Solid,
            Line::Hidden => Self::Dash { on: 4, off: 3 },
            Line::Centre => Self::Chain {
                long: 11,
                gap: 2,
                short: 1,
                shorts: 1,
            },
            Line::Phantom => Self::Chain {
                long: 11,
                gap: 2,
                short: 1,
                shorts: 2,
            },
            Line::Path => Self::Dash { on: 1, off: 2 },
        }
    }

    pub fn lights(self, index: usize) -> bool {
        match self {
            Self::Solid => true,
            Self::Dash { on, off } => index % usize::from(on + off).max(1) < usize::from(on),
            Self::Chain {
                long,
                gap,
                short,
                shorts,
            } => {
                let (long, gap, short) = (usize::from(long), usize::from(gap), usize::from(short));
                let period = long + usize::from(shorts) * (gap + short) + gap;
                let at = index % period.max(1);

                at < long || {
                    let rest = at - long;
                    rest % (gap + short) >= gap && rest < usize::from(shorts) * (gap + short)
                }
            }
        }
    }
}

/// An area's texture, counted from an origin that moves with what it fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Texture {
    Solid,
    /// Lines rising to the right every `n` pixels.
    Rising(u8),
    /// Lines falling to the right every `n` pixels.
    Falling(u8),
    Bayer(u8),
}

impl Texture {
    fn of(fill: Fill) -> Self {
        match fill {
            Fill::Hatch => Self::Rising(4),
            Fill::CrossHatch => Self::Falling(4),
            Fill::Solid => Self::Solid,
            Fill::Tint(level) => Self::Bayer(level),
        }
    }

    pub fn lights(self, x: i32, y: i32) -> bool {
        const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

        match self {
            Self::Solid => true,
            Self::Rising(n) => (x + y).rem_euclid(i32::from(n.max(1))) == 0,
            Self::Falling(n) => (x - y).rem_euclid(i32::from(n.max(1))) == 0,
            Self::Bayer(level) => BAYER[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize] < level,
        }
    }
}

/// A mark on the pixel grid.
#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    /// Pixels in the order a pen draws them; the stipple decides which are
    /// inked.
    Path {
        pixels: Vec<Point<i32>>,
        stipple: Stipple,
        /// How far along its stroke the first pixel is, for the pattern: a
        /// stroke cut in pieces keeps its dashes where they were.
        phase: usize,
    },
    /// The rows of an area as `(y, first x, last x)`.
    Rows {
        rows: Vec<(i32, i32, i32)>,
        texture: Texture,
        origin: Point<i32>,
    },
    /// One line of text, its line box's top-left corner at `at`.
    Text { at: Point<i32>, text: String },
    /// A solid rectangle.
    Block(Rectangle<i32>),
    /// The sheet's ground behind text, so it reads over line work.
    Knockout(Rectangle<i32>),
}

/// A piece and the role it is inked in.
#[derive(Debug, Clone, PartialEq)]
pub struct Inked {
    pub piece: Piece,
    pub tone: Tone,
}

/// What a pen-plotter animation needs to know of a piece drawn in part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Head(pub Point<i32>);

const TEXT_COST: usize = 6;
const ROW_COST: usize = 2;

impl Piece {
    /// A whole stroke's pixels.
    pub fn path(pixels: Vec<Point<i32>>, stipple: Stipple) -> Self {
        Self::Path {
            pixels,
            stipple,
            phase: 0,
        }
    }

    /// The piece in consecutive pieces of at most `cost` each, which plot
    /// and draw as it does: a stroke in stretches, an area in bands of rows.
    pub fn split(self, cost: usize) -> Vec<Self> {
        match self {
            Self::Path {
                pixels,
                stipple,
                phase,
            } if pixels.len() > cost => pixels
                .chunks(cost.max(1))
                .enumerate()
                .map(|(k, chunk)| Self::Path {
                    pixels: chunk.to_vec(),
                    stipple,
                    phase: phase + k * cost.max(1),
                })
                .collect(),
            Self::Rows {
                rows,
                texture,
                origin,
            } if rows.len() * ROW_COST > cost => rows
                .chunks((cost / ROW_COST).max(1))
                .map(|band| Self::Rows {
                    rows: band.to_vec(),
                    texture,
                    origin,
                })
                .collect(),
            other => vec![other],
        }
    }

    /// How long the piece takes to plot, in pixels of pen travel.
    pub fn cost(&self) -> usize {
        match self {
            Self::Path { pixels, .. } => pixels.len(),
            Self::Rows { rows, .. } => rows.len() * ROW_COST,
            Self::Text { text, .. } => text.chars().count() * TEXT_COST,
            Self::Block(_) | Self::Knockout(_) => 1,
        }
    }

    /// Draws the first `budget` of the piece's cost, inside `clip`; returns
    /// where the pen is if it stopped short of the end within the clip.
    pub fn draw<Renderer>(
        &self,
        pen: &mut Pen<'_, Renderer>,
        color: Color,
        ground: Color,
        budget: usize,
        clip: Rectangle<i32>,
    ) -> Option<Head>
    where
        Renderer: iced_widget::graphics::geometry::Renderer,
    {
        let partial = budget < self.cost();
        let inside = |pixel: &Point<i32>| contains(clip, *pixel);

        match self {
            Self::Path {
                pixels,
                stipple,
                phase,
            } => {
                let drawn = &pixels[..budget.min(pixels.len())];
                let lit: Vec<_> = drawn
                    .iter()
                    .enumerate()
                    .filter(|(i, pixel)| stipple.lights(phase + i) && inside(pixel))
                    .map(|(_, pixel)| *pixel)
                    .collect();

                fill_pixels(pen, &lit, color);
            }
            Self::Rows {
                rows,
                texture,
                origin,
            } => {
                let count = if partial {
                    budget / ROW_COST
                } else {
                    rows.len()
                };
                let (left, right) = (clip.x, clip.x + clip.width - 1);

                for &(y, from, to) in &rows[..count] {
                    if y < clip.y || y >= clip.y + clip.height {
                        continue;
                    }

                    let (from, to) = (from.max(left), to.min(right));
                    let mut start = None;

                    for x in from..=to + 1 {
                        let lit = x <= to && texture.lights(x - origin.x, y - origin.y);

                        match (lit, start) {
                            (true, None) => start = Some(x),
                            (false, Some(first)) => {
                                pen.fill(rect(first, y, x - first, 1), color);
                                start = None;
                            }
                            _ => {}
                        }
                    }
                }
            }
            Self::Text { at, text } => {
                let typed = if partial {
                    budget / TEXT_COST
                } else {
                    text.chars().count()
                };

                // All of it, or none if it is not all inside: a value cut
                // short reads as a different value.
                if shows(*at, text, clip) {
                    let shown: String = text.chars().take(typed).collect();

                    super::letters::write(pen, &shown, *at, color);
                }
            }
            Self::Block(bounds) | Self::Knockout(bounds) => {
                if let Some(bounds) = intersection(*bounds, clip).filter(|_| budget > 0) {
                    let fill = if matches!(self, Self::Knockout(_)) {
                        ground
                    } else {
                        color
                    };

                    pen.fill(bounds, fill);
                }
            }
        }

        self.head(budget, clip)
    }

    /// Where the pen is with the first `budget` of the piece's cost drawn,
    /// if it stopped short of the end inside `clip`.
    pub fn head(&self, budget: usize, clip: Rectangle<i32>) -> Option<Head> {
        if budget >= self.cost() {
            return None;
        }

        let head = match self {
            Self::Path { pixels, .. } => pixels[..budget.min(pixels.len())]
                .last()
                .map(|pixel| Head(*pixel)),
            Self::Rows { rows, .. } => rows
                .get(budget / ROW_COST)
                .map(|&(y, from, _)| Head(Point::new(from, y))),
            Self::Text { at, .. } => {
                let face = &LETTERING;
                let typed = (budget / TEXT_COST) as i32;
                let baseline = at.y + i32::from(face.baseline());

                Some(Head(Point::new(
                    at.x + i32::from(face.advance()) * typed,
                    baseline - 1,
                )))
            }
            Self::Block(_) | Self::Knockout(_) => None,
        };

        head.filter(|Head(at)| contains(clip, *at))
    }
}

/// Whether a line of `text` with its line box's top-left corner at `at` is
/// drawn inside `clip`: all of it, or none if it is not all inside, as a
/// value cut short reads as a different value.
pub fn shows(at: Point<i32>, text: &str, clip: Rectangle<i32>) -> bool {
    let face = &LETTERING;
    let cap_top = at.y + i32::from(face.cap_top());
    let baseline = at.y + i32::from(face.baseline());
    let right = at.x + text.chars().count() as i32 * i32::from(face.advance());

    cap_top >= clip.y
        && baseline <= clip.y + clip.height
        && at.x >= clip.x
        && right <= clip.x + clip.width
}

pub fn contains(clip: Rectangle<i32>, pixel: Point<i32>) -> bool {
    pixel.x >= clip.x
        && pixel.y >= clip.y
        && pixel.x < clip.x + clip.width
        && pixel.y < clip.y + clip.height
}

pub fn intersection(a: Rectangle<i32>, b: Rectangle<i32>) -> Option<Rectangle<i32>> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = (a.x + a.width).min(b.x + b.width);
    let bottom = (a.y + a.height).min(b.y + b.height);

    (right > x && bottom > y).then(|| rect(x, y, right - x, bottom - y))
}

/// Fills pixels as runs along whichever axis makes fewer of them.
pub fn fill_pixels<Renderer>(pen: &mut Pen<'_, Renderer>, pixels: &[Point<i32>], color: Color)
where
    Renderer: iced_widget::graphics::geometry::Renderer,
{
    let across = shape::runs(pixels, false);
    let down = shape::runs(pixels, true);

    for run in if down.len() < across.len() {
        down
    } else {
        across
    } {
        pen.fill(run, color);
    }
}

pub const fn rect(x: i32, y: i32, width: i32, height: i32) -> Rectangle<i32> {
    Rectangle {
        x,
        y,
        width,
        height,
    }
}

/// The colour of a role.
pub fn colour(palette: &Palette, tone: Tone) -> Color {
    match tone {
        Tone::Ink => palette.ink,
        Tone::Line => palette.line,
        Tone::Muted => palette.muted,
        Tone::Faint => palette.faint,
        Tone::Accent => palette.accent,
        Tone::Live => palette.live,
        Tone::Caution => palette.caution,
    }
}

/// The face every annotation is lettered in.
pub const LETTERING: Face = Face::BODY;

/// The top-left corner of `text`'s line box when placed by `anchor` at `at`,
/// the way [`Pen::text`] places it.
pub fn place(face: Face, text: &str, at: Point<i32>, anchor: Anchor) -> Point<i32> {
    let width = i32::from(face.width(text));

    let x = match anchor.x {
        Horizontal::Left => at.x,
        Horizontal::Centre => at.x - width.div_euclid(2),
        Horizontal::Right => at.x - width,
    };

    let y = match anchor.y {
        Vertical::Top => at.y,
        Vertical::CapTop => at.y - i32::from(face.cap_top()),
        Vertical::Middle => at.y - i32::from(face.cap_top()) - i32::from(face.cap()).div_euclid(2),
        Vertical::Baseline => at.y - i32::from(face.baseline()),
        Vertical::Bottom => at.y - i32::from(face.line()),
    };

    Point::new(x, y)
}

/// Pieces for `text` with a knockout behind its capitals.
fn lettering(text: &str, top_left: Point<i32>, tone: Tone, out: &mut Vec<Inked>) {
    let face = LETTERING;
    let cap_top = top_left.y + i32::from(face.cap_top());

    out.push(Inked {
        piece: Piece::Knockout(rect(
            top_left.x - 2,
            cap_top - 2,
            i32::from(face.width(text)) + 3,
            i32::from(face.cap()) + 4,
        )),
        tone,
    });
    out.push(Inked {
        piece: Piece::Text {
            at: top_left,
            text: text.to_owned(),
        },
        tone,
    });
}

/// `projection` as `mark` is drawn through it: set on the grid by the
/// points its figure is set by (see [`Draft::snapped`](super::Draft::snapped)).
pub fn through(mark: &Mark, projection: &Projection) -> Projection {
    mark.snaps
        .iter()
        .fold(*projection, |projection, &at| projection.snapped(at))
}

/// The pieces of `mark` under `projection`.
pub fn rasterize(mark: &Mark, projection: &Projection, out: &mut Vec<Inked>) {
    let tone = mark.tone;
    let snapped = through(mark, projection);
    let projection = &snapped;

    match &mark.ink {
        Ink::Stroke { shape, line } => out.push(Inked {
            piece: Piece::path(stroke(shape, projection), Stipple::of(*line)),
            tone,
        }),
        Ink::Arrow { from, to, line } => {
            let tip = projection.px(*to);

            out.push(Inked {
                piece: Piece::path(shape::line(projection.px(*from), tip), Stipple::of(*line)),
                tone,
            });
            arrowhead(
                tip,
                projection.exact(*to) - projection.exact(*from),
                tone,
                out,
            );
        }
        Ink::Area { contour, fill } => {
            let points: Vec<_> = contour.iter().map(|point| projection.px(*point)).collect();
            let origin = points.first().copied().unwrap_or(Point::new(0, 0));

            if points.len() >= 3 {
                let mut rows = Polygon::new(points).rows();

                // Lining stops a pixel short of the area's edge, which is
                // drawn over it otherwise and broken where it lands.
                if *fill != Fill::Solid {
                    rows = inset(&rows);
                }

                out.push(Inked {
                    piece: Piece::Rows {
                        rows,
                        texture: Texture::of(*fill),
                        origin,
                    },
                    tone,
                });
            }
        }
        Ink::Inside { shape, fill } => {
            let rows = inside(&stroke(shape, projection));

            if !rows.is_empty() {
                out.push(Inked {
                    piece: Piece::Rows {
                        origin: Point::new(rows[0].1, rows[0].0),
                        rows,
                        texture: Texture::of(*fill),
                    },
                    tone,
                });
            }
        }
        Ink::Label {
            at,
            nudge,
            text,
            anchor,
        } => {
            let at = projection.px(*at);
            let at = Point::new(at.x + nudge.0, at.y + nudge.1);

            lettering(text, place(LETTERING, text, at, *anchor), tone, out);
        }
        Ink::Dimension { measure, text } => dimension(measure, text, tone, projection, out),
        // An annotation placed automatically is drawn once it is placed.
        Ink::Note {
            target,
            elbow: Placement::Offset(x, y),
            text,
        } => {
            let target = projection.px(*target);

            out.push(dot(v(target.x as f32, target.y as f32), 3, Tone::Line));
            callout(target, (*x, *y), text, tone, out);
        }
        Ink::Balloon {
            item,
            target,
            offset: Placement::Offset(x, y),
        } => balloon(*item, projection.px(*target), (*x, *y), tone, out),
        Ink::Note { .. } | Ink::Balloon { .. } => {}
        Ink::Dot { at, size } => out.push(dot(projection.exact(*at), *size, tone)),
        Ink::Finish { at, text } => finish(projection.px(*at), text, tone, out),
        Ink::Datum { at, toward, letter } => {
            datum(
                projection.px(*at),
                v(toward.x, -toward.y),
                *letter,
                tone,
                out,
            );
        }
        Ink::Control {
            at,
            offset,
            characteristic,
            tolerance,
            datums,
        } => control(
            projection.px(*at),
            *offset,
            *characteristic,
            tolerance,
            datums,
            tone,
            out,
        ),
        Ink::Section {
            from,
            to,
            toward,
            letter,
            ..
        } => cutting_plane(
            projection.exact(*from),
            projection.exact(*to),
            v(toward.x, -toward.y),
            *letter,
            tone,
            out,
        ),
    }
}

/// The rows of an area a pixel in from its edge: each row short of its
/// ends and of the rows above and below it.
fn inset(rows: &[(i32, i32, i32)]) -> Vec<(i32, i32, i32)> {
    let mut spans: HashMap<i32, Vec<(i32, i32)>> = HashMap::new();

    for &(y, from, to) in rows {
        spans.entry(y).or_default().push((from, to));
    }

    rows.iter()
        .flat_map(|&(y, from, to)| {
            // Within a pixel of its own row's ends, and of the rows either
            // side's: any pixel of the edge's.
            [y - 1, y + 1]
                .iter()
                .fold(vec![(from + 1, to - 1)], |pieces, other| {
                    let neighbours = spans.get(other).map_or(&[][..], Vec::as_slice);

                    pieces
                        .into_iter()
                        .flat_map(|(from, to)| clip_to(from, to, neighbours))
                        .collect()
                })
                .into_iter()
                .map(move |(from, to)| (y, from, to))
        })
        .collect()
}

/// The parts of `from..=to` within a pixel of the inside of `spans`.
fn clip_to(from: i32, to: i32, spans: &[(i32, i32)]) -> Vec<(i32, i32)> {
    spans
        .iter()
        .filter_map(|&(left, right)| {
            let (a, b) = (from.max(left + 1), to.min(right - 1));
            (a <= b).then_some((a, b))
        })
        .collect()
}

/// A square dot `size` pixels across centred on `at` as nearly as the grid
/// allows: an odd size on the pixel `at` is in, an even one between the two
/// pixels nearest it.
fn dot(at: V2, size: i32, tone: Tone) -> Inked {
    let size = size.max(1);
    let half = (size - 1) as f32 / 2.0;
    let corner = |at: f32| (at - half).round() as i32;

    Inked {
        piece: Piece::Block(rect(corner(at.x), corner(at.y), size, size)),
        tone,
    }
}

/// The pixels of a stroke, in drawing order.
fn stroke(shape: &Shape, projection: &Projection) -> Vec<Point<i32>> {
    match shape {
        Shape::Polyline { points, closed } => {
            let mut pixels: Vec<Point<i32>> = Vec::with_capacity(points.len() + 1);

            for point in points.iter().chain(closed.then(|| &points[0])) {
                let pixel = projection.px(*point);

                if pixels.last() != Some(&pixel) {
                    pixels.push(pixel);
                }
            }

            shape::polyline(&pixels)
        }
        Shape::Circle { centre, radius } => {
            let centre = projection.px(*centre);
            let radius = projection.length(*radius);

            ordered_circle(centre, radius)
        }
        Shape::Arc {
            centre,
            radius,
            start,
            sweep,
        } => {
            let centre = projection.px(*centre);
            let radius = projection.length(*radius).max(0);
            let from = bearing(*start);
            let to = bearing(start + sweep);

            if *sweep >= 0.0 {
                // Counter-clockwise on the model is anticlockwise on the
                // sheet too: the dial's arc from `to` round to `from`,
                // walked backwards.
                let mut pixels = clockwise_arc(centre, radius, to, from);
                pixels.reverse();
                pixels
            } else {
                clockwise_arc(centre, radius, from, to)
            }
        }
        Shape::Keyhole {
            centre,
            radius,
            half,
            length,
        } => keyhole(
            projection.px(*centre),
            projection.length(*radius),
            projection.length(*half),
            projection.length(*length),
        ),
    }
}

/// The pixels of a keyhole's outline (see [`Shape::Keyhole`]): the 1 px
/// circle of `radius` round `centre` with its pixels between the slot's
/// sides left out, from where the slot's lower side leaves it round by the
/// bottom, the left and the top to where its upper side does, then along
/// that side, down its closed end `length` from `centre` and back.
///
/// Each side starts on the pixel after the circle's last on its row, so the
/// outline is closed and the same either side of the centre line; a slot
/// as wide as the hole is a pixel narrower than it, and one that ends
/// inside the circle leaves the circle whole.
fn keyhole(centre: Point<i32>, radius: i32, half: i32, length: i32) -> Vec<Point<i32>> {
    let circle = ordered_circle(centre, radius);
    let half = half.clamp(1, (radius - 1).max(1));
    // The circle's last pixel on each of the slot's sides.
    let neck = circle
        .iter()
        .filter(|pixel| pixel.y == centre.y - half && pixel.x > centre.x)
        .map(|pixel| pixel.x)
        .max();
    let end = centre.x + length;

    let Some(neck) = neck.filter(|&neck| radius > 1 && end > neck) else {
        return circle;
    };

    let mouth = |pixel: &Point<i32>| pixel.x > centre.x && (pixel.y - centre.y).abs() < half;
    // The walk goes clockwise from the top, so it passes the mouth on the
    // right: start after it.
    let after = circle.iter().rposition(mouth).map_or(0, |last| last + 1);
    let mut pixels: Vec<Point<i32>> = circle[after..]
        .iter()
        .chain(&circle[..after])
        .filter(|pixel| !mouth(pixel))
        .copied()
        .collect();
    let (upper, lower) = (centre.y - half, centre.y + half);

    pixels.extend((neck + 1..=end).map(|x| Point::new(x, upper)));
    pixels.extend((upper + 1..=lower).map(|y| Point::new(end, y)));
    pixels.extend((neck + 1..end).rev().map(|x| Point::new(x, lower)));
    pixels
}

/// The rows strictly inside an outline each row crosses once in and once
/// out: on each row, the pixels after its first run of the outline's and
/// before its last.
fn inside(outline: &[Point<i32>]) -> Vec<(i32, i32, i32)> {
    let mut rows: Vec<(i32, Vec<i32>)> = Vec::new();

    for pixel in outline {
        match rows.iter_mut().find(|(y, _)| *y == pixel.y) {
            Some((_, xs)) => xs.push(pixel.x),
            None => rows.push((pixel.y, vec![pixel.x])),
        }
    }

    rows.sort_by_key(|(y, _)| *y);
    rows.into_iter()
        .filter_map(|(y, mut xs)| {
            xs.sort_unstable();
            xs.dedup();

            // The first run's end and the last run's start.
            let first = xs.windows(2).position(|pair| pair[1] > pair[0] + 1)?;
            let last = xs.windows(2).rposition(|pair| pair[1] > pair[0] + 1)? + 1;

            Some((y, xs[first] + 1, xs[last] - 1))
        })
        .collect()
}

/// The pixels of a 1 px circle of `radius` round `centre`, each once, in
/// order clockwise from the top.
///
/// They are the toolkit's circle (`shape::circle`): its first octant walked
/// the same way, and its eight images laid end to end round the dial, so
/// the order comes from the walk instead of a sort.
pub fn ordered_circle(centre: Point<i32>, radius: i32) -> Vec<Point<i32>> {
    if radius <= 0 {
        return vec![centre];
    }

    // The first octant, from (r, 0) down to the diagonal, as `shape` walks it.
    let mut octant = Vec::new();
    let (mut x, mut y, mut error) = (radius, 0, 1 - radius);

    while x >= y {
        octant.push((x, y));
        y += 1;

        if error < 0 {
            error += 2 * y + 1;
        } else {
            x -= 1;
            error += 2 * (y - x) + 1;
        }
    }

    // Each octant of the dial from 12 o'clock, its image of the walk and
    // whether the walk runs forwards in it (sheet y grows downwards).
    type Image = fn((i32, i32)) -> (i32, i32);
    let images: [(Image, bool); 8] = [
        (|(x, y)| (y, -x), true),
        (|(x, y)| (x, -y), false),
        (|(x, y)| (x, y), true),
        (|(x, y)| (y, x), false),
        (|(x, y)| (-y, x), true),
        (|(x, y)| (-x, y), false),
        (|(x, y)| (-x, -y), true),
        (|(x, y)| (-y, -x), false),
    ];
    let steps = octant.len();
    let mut pixels: Vec<Point<i32>> = Vec::with_capacity(steps * 8);

    for (image, forwards) in images {
        for k in 0..steps {
            let (dx, dy) = image(octant[if forwards { k } else { steps - 1 - k }]);
            let pixel = Point::new(centre.x + dx, centre.y + dy);

            // Octants share their ends.
            if pixels.last() != Some(&pixel) {
                pixels.push(pixel);
            }
        }
    }

    if pixels.len() > 1 && pixels.last() == pixels.first() {
        pixels.pop();
    }

    pixels
}

/// The pixels of the arc of a 1 px circle clockwise from bearing `from` to
/// bearing `to`, in that order: the toolkit's arc (`shape::arc`), cut from
/// the walked circle.
fn clockwise_arc(centre: Point<i32>, radius: i32, from: f32, to: f32) -> Vec<Point<i32>> {
    let circle = ordered_circle(centre, radius);
    let bearing = |pixel: &Point<i32>| shape::bearing(pixel.x - centre.x, pixel.y - centre.y);
    let sweep = match (to - from).rem_euclid(360.0) {
        0.0 if to != from => 360.0,
        sweep => sweep,
    };

    // The walk's bearings rise from 0 round to 360, so the arc is one run of
    // it, starting where the bearing first reaches `from`.
    let start = circle.partition_point(|pixel| bearing(pixel) < from);

    circle[start..]
        .iter()
        .chain(&circle[..start])
        .copied()
        .take_while(|pixel| (bearing(pixel) - from).rem_euclid(360.0) <= sweep)
        .collect()
}

/// The bearing (degrees clockwise from up, on the sheet) of a model angle.
fn bearing(angle: f32) -> f32 {
    (90.0 - angle.to_degrees()).rem_euclid(360.0)
}

/// A solid arrowhead with its tip at `tip`, pointing along `towards`.
///
/// Its sides are at 45° to the shaft, three pixels deep: the toolkit's
/// arrowhead at any angle.
fn arrowhead(tip: Point<i32>, towards: V2, tone: Tone, out: &mut Vec<Inked>) {
    arrowhead_within(tip, towards, None, tone, out);
}

/// [`arrowhead`], its corners kept inside `bounds` (the top left and
/// bottom right pixels) if given: one pointing into a corner of a
/// rectangle along a line shallower or steeper than 45° would stand a
/// pixel out past the edge the line meets at less than 45°.
fn arrowhead_within(
    tip: Point<i32>,
    towards: V2,
    bounds: Option<(Point<i32>, Point<i32>)>,
    tone: Tone,
    out: &mut Vec<Inked>,
) {
    const DEPTH: f32 = 3.0;

    let along = towards.normalize_or_zero();

    if along == V2::ZERO {
        return;
    }

    let across = along.perp();
    let tip_f = v(tip.x as f32, tip.y as f32);
    let base = tip_f - along * DEPTH;
    let corner = |point: V2| {
        let pixel = Point::new(point.x.round() as i32, point.y.round() as i32);

        match bounds {
            Some((low, high)) => {
                Point::new(pixel.x.clamp(low.x, high.x), pixel.y.clamp(low.y, high.y))
            }
            None => pixel,
        }
    };

    out.push(Inked {
        piece: Piece::Rows {
            rows: Polygon::new([
                tip,
                corner(base + across * DEPTH),
                corner(base - across * DEPTH),
            ])
            .rows(),
            texture: Texture::Solid,
            origin: tip,
        },
        tone,
    });
}

/// A leader from `target` to an elbow, then a shelf the text sits on,
/// running away from the target.
fn callout(target: Point<i32>, elbow: (i32, i32), text: &str, tone: Tone, out: &mut Vec<Inked>) {
    let face = LETTERING;
    let corner = Point::new(target.x + elbow.0, target.y + elbow.1);
    let width = i32::from(face.width(text));
    let rightwards = elbow.0 >= 0;
    let end = if rightwards {
        corner.x + width + 3
    } else {
        corner.x - width - 3
    };

    let mut pixels = shape::line(target, corner);
    pixels.extend(
        shape::line(corner, Point::new(end, corner.y))
            .into_iter()
            .skip(1),
    );

    out.push(Inked {
        piece: Piece::path(pixels, Stipple::Solid),
        tone: Tone::Line,
    });

    let left = if rightwards { corner.x + 2 } else { end + 2 };

    let at = place(
        face,
        text,
        Point::new(left, corner.y - 1),
        Anchor::BASELINE_LEFT,
    );
    let cap_top = at.y + i32::from(face.cap_top());

    // The ground behind the lettering, down to the shelf.
    out.push(Inked {
        piece: Piece::Knockout(rect(
            at.x - 1,
            cap_top - 1,
            width + 2,
            corner.y - cap_top + 1,
        )),
        tone,
    });
    out.push(Inked {
        piece: Piece::Text {
            at,
            text: text.to_owned(),
        },
        tone,
    });
}

/// A cutting plane on the sheet from `from` to `to`: a chain line, thick
/// for a stretch at each end, where an arrow on the viewer's side points the
/// way the section is seen, `toward`, its letter at its tail.
fn cutting_plane(from: V2, to: V2, toward: V2, letter: char, tone: Tone, out: &mut Vec<Inked>) {
    /// The thick stretch at each end...
    const END: f32 = 8.0;
    /// ...and the arrows' length.
    const ARROW: f32 = 10.0;

    let along = (to - from).normalize_or_zero();
    let toward = toward.normalize_or_zero();

    if along == V2::ZERO || toward == V2::ZERO {
        return;
    }

    let snap = |point: V2| Point::new(point.x.round() as i32, point.y.round() as i32);

    out.push(Inked {
        piece: Piece::path(shape::line(snap(from), snap(to)), Stipple::of(Line::Centre)),
        tone: Tone::Line,
    });

    for (end, inward) in [(from, along), (to, -along)] {
        // Two pixels thick, the second on the viewer's side.
        for side in [0.0, 1.0] {
            let back = toward * -side;

            out.push(path(
                shape::line(snap(end + back), snap(end + back + inward * END)),
                tone,
            ));
        }

        let tip = end - toward * 2.0;
        let tail = tip - toward * ARROW;

        out.push(path(shape::line(snap(tail), snap(tip)), tone));
        arrowhead(snap(tip), toward, tone, out);

        let text = letter.to_string();
        let at = snap(tail - toward * 7.0);

        lettering(
            &text,
            place(LETTERING, &text, at, Anchor::CENTRE),
            tone,
            out,
        );
    }
}

/// The outline of `area`, a pixel wide.
fn outline(area: Rectangle<i32>, tone: Tone) -> Inked {
    let (left, top) = (area.x, area.y);
    let (right, bottom) = (area.x + area.width - 1, area.y + area.height - 1);

    path(
        shape::polyline(&[
            Point::new(left, top),
            Point::new(right, top),
            Point::new(right, bottom),
            Point::new(left, bottom),
            Point::new(left, top),
        ]),
        tone,
    )
}

/// The surface texture symbol of a machined surface (ISO 1302) standing on
/// `tip`: a V of unequal legs, a bar along from the longer, and the
/// requirement under the bar.
fn finish(tip: Point<i32>, text: &str, tone: Tone, out: &mut Vec<Inked>) {
    let short = Point::new(tip.x - 4, tip.y - 7);
    let long = Point::new(tip.x + 8, tip.y - 14);
    let width = i32::from(LETTERING.width(text));

    out.push(path(shape::polyline(&[short, tip, long]), tone));
    out.push(path(
        shape::line(long, Point::new(long.x + width + 4, long.y)),
        tone,
    ));
    lettering(
        text,
        place(
            LETTERING,
            text,
            Point::new(long.x + 3, long.y + 3),
            Anchor::new(Horizontal::Left, Vertical::CapTop),
        ),
        tone,
        out,
    );
}

/// A datum feature symbol (ISO 5459) on `at`: a filled triangle on the
/// feature, a leader running `toward` its frame, and the letter in it.
fn datum(at: Point<i32>, toward: V2, letter: char, tone: Tone, out: &mut Vec<Inked>) {
    const FRAME: i32 = 13;

    let toward = toward.normalize_or_zero();

    if toward == V2::ZERO {
        return;
    }

    let across = toward.perp();
    let snap = |point: V2| Point::new(point.x.round() as i32, point.y.round() as i32);
    let base = v(at.x as f32, at.y as f32);
    let apex = base + toward * 5.0;

    out.push(Inked {
        piece: Piece::Rows {
            rows: Polygon::new([
                snap(base + across * 3.0),
                snap(base - across * 3.0),
                snap(apex),
            ])
            .rows(),
            texture: Texture::Solid,
            origin: at,
        },
        tone,
    });

    let end = apex + toward * 6.0;
    out.push(path(shape::line(snap(apex), snap(end)), tone));

    // The frame, its near side's middle on the leader's end.
    let middle = end + toward * (FRAME as f32 / 2.0);
    let corner = snap(middle - v(FRAME as f32 / 2.0, FRAME as f32 / 2.0));
    let frame = rect(corner.x, corner.y, FRAME, FRAME);

    out.push(Inked {
        piece: Piece::Knockout(frame),
        tone,
    });
    out.push(outline(frame, tone));

    let text = letter.to_string();
    out.push(Inked {
        piece: Piece::Text {
            at: place(LETTERING, &text, snap(middle), Anchor::CENTRE),
            text,
        },
        tone,
    });
}

/// A feature control frame (ISO 1101) `offset` from `at`: cells for what it
/// controls, the tolerance and the datums, and a leader arrowed onto `at`
/// from the frame's nearer end.
fn control(
    at: Point<i32>,
    offset: (i32, i32),
    characteristic: Characteristic,
    tolerance: &str,
    datums: &str,
    tone: Tone,
    out: &mut Vec<Inked>,
) {
    const HEIGHT: i32 = 13;

    let corner = Point::new(at.x + offset.0, at.y + offset.1);
    let cell = |text: &str| i32::from(LETTERING.width(text)) + 6;
    let widths: Vec<i32> = [HEIGHT, cell(tolerance)]
        .into_iter()
        .chain((!datums.is_empty()).then(|| cell(datums)))
        .collect();
    let width = widths.iter().sum::<i32>() - (widths.len() as i32 - 1);
    let frame = rect(corner.x, corner.y, width, HEIGHT);

    // The leader, from whichever end of the frame faces the feature.
    let middle = corner.y + HEIGHT / 2;
    let start = if at.x < corner.x {
        Point::new(corner.x - 1, middle)
    } else {
        Point::new(corner.x + width, middle)
    };

    out.push(path(shape::line(start, at), Tone::Line));
    arrowhead(
        at,
        v((at.x - start.x) as f32, (at.y - start.y) as f32),
        Tone::Line,
        out,
    );

    out.push(Inked {
        piece: Piece::Knockout(frame),
        tone,
    });
    out.push(outline(frame, tone));

    let mut left = corner.x;

    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            out.push(path(
                shape::line(
                    Point::new(left, corner.y),
                    Point::new(left, corner.y + HEIGHT - 1),
                ),
                tone,
            ));
        }

        let centre = Point::new(left + width / 2, middle);
        let text = match index {
            1 => tolerance,
            2 => datums,
            _ => "",
        };

        if index == 0 {
            symbol(characteristic, centre, tone, out);
        } else {
            out.push(Inked {
                piece: Piece::Text {
                    at: place(LETTERING, text, centre, Anchor::CENTRE),
                    text: text.to_owned(),
                },
                tone,
            });
        }

        left += width - 1;
    }
}

/// The symbol of a geometric characteristic, drawn round `centre`: the
/// lettering face has no glyphs for them.
fn symbol(characteristic: Characteristic, centre: Point<i32>, tone: Tone, out: &mut Vec<Inked>) {
    let at = |x: i32, y: i32| Point::new(centre.x + x, centre.y + y);
    let lines: Vec<Vec<Point<i32>>> = match characteristic {
        // A circle and a cross through it.
        Characteristic::Position => vec![
            ordered_circle(centre, 3),
            shape::line(at(-4, 0), at(4, 0)),
            shape::line(at(0, -4), at(0, 4)),
        ],
        Characteristic::Perpendicularity => vec![
            shape::line(at(0, -4), at(0, 3)),
            shape::line(at(-4, 3), at(4, 3)),
        ],
    };

    for pixels in lines {
        out.push(path(pixels, tone));
    }
}

/// The radius of a balloon's circle.
pub const BALLOON: i32 = 7;

/// A part's number in a circle, its leader ending in a dot on the part.
fn balloon(item: usize, target: Point<i32>, offset: (i32, i32), tone: Tone, out: &mut Vec<Inked>) {
    const RADIUS: i32 = BALLOON;

    let centre = Point::new(target.x + offset.0, target.y + offset.1);
    let towards = v((target.x - centre.x) as f32, (target.y - centre.y) as f32);
    let start = towards.normalize_or_zero() * (RADIUS as f32 + 1.0);
    let start = Point::new(
        centre.x + start.x.round() as i32,
        centre.y + start.y.round() as i32,
    );

    out.push(Inked {
        piece: Piece::path(shape::line(start, target), Stipple::Solid),
        tone: Tone::Line,
    });
    out.push(dot(v(target.x as f32, target.y as f32), 3, Tone::Line));
    out.push(Inked {
        piece: Piece::Knockout(rect(
            centre.x - RADIUS,
            centre.y - RADIUS,
            2 * RADIUS + 1,
            2 * RADIUS + 1,
        )),
        tone,
    });
    out.push(Inked {
        piece: Piece::path(ordered_circle(centre, RADIUS), Stipple::Solid),
        tone,
    });

    let text = item.to_string();

    out.push(Inked {
        piece: Piece::Text {
            at: place(LETTERING, &text, centre, Anchor::CENTRE),
            text,
        },
        tone,
    });
}

/// Extension lines start this far from the feature...
const GAP: i32 = 2;
/// ...and reach this far past the dimension line.
const OVERSHOOT: i32 = 3;

fn path(pixels: Vec<Point<i32>>, tone: Tone) -> Inked {
    Inked {
        piece: Piece::path(pixels, Stipple::Solid),
        tone,
    }
}

fn dimension(measure: &Measure, text: &str, tone: Tone, p: &Projection, out: &mut Vec<Inked>) {
    match *measure {
        Measure::Linear { a, b, axis, offset } => {
            let (pa, pb) = (p.exact(a), p.exact(b));

            // The dimension line's two ends, and the direction from the
            // features out to it.
            let (start, end, out_dir) = match axis {
                Axis::Horizontal => {
                    let offset = offset * p.scale;
                    let y = if offset >= 0.0 {
                        pa.y.min(pb.y) - offset
                    } else {
                        pa.y.max(pb.y) - offset
                    };
                    let (left, right) = if pa.x <= pb.x { (pa, pb) } else { (pb, pa) };

                    (
                        v(left.x, y),
                        v(right.x, y),
                        v(0.0, if offset >= 0.0 { -1.0 } else { 1.0 }),
                    )
                }
                Axis::Vertical => {
                    let offset = offset * p.scale;
                    let x = if offset >= 0.0 {
                        pa.x.max(pb.x) + offset
                    } else {
                        pa.x.min(pb.x) + offset
                    };
                    let (top, bottom) = if pa.y <= pb.y { (pa, pb) } else { (pb, pa) };

                    (
                        v(x, top.y),
                        v(x, bottom.y),
                        v(if offset >= 0.0 { 1.0 } else { -1.0 }, 0.0),
                    )
                }
            };

            let snap = |point: V2| Point::new(point.x.round() as i32, point.y.round() as i32);
            let feature_of = |end: V2, feature: V2| {
                // The feature point, moved along the dimension line's normal
                // onto the extension line.
                end - out_dir * (end - feature).dot(out_dir)
            };

            for (end, feature) in [(start, pa), (end, pb)] {
                let foot = feature_of(end, feature);
                let distance = (end - foot).dot(out_dir);

                if distance.abs() > GAP as f32 {
                    let from = foot + out_dir * GAP as f32 * distance.signum();
                    let to = end + out_dir * OVERSHOOT as f32 * distance.signum();

                    out.push(path(shape::line(snap(from), snap(to)), Tone::Line));
                }
            }

            let (s, e) = (snap(start), snap(end));
            let span = start.distance(end);

            out.push(path(shape::line(s, e), Tone::Line));

            if span >= 12.0 {
                arrowhead(s, start - end, Tone::Line, out);
                arrowhead(e, end - start, Tone::Line, out);
            }

            let middle = snap(start.lerp(end, 0.5));
            let width = i32::from(LETTERING.width(text)) as f32;
            let fits = match axis {
                Axis::Vertical => span >= f32::from(LETTERING.cap()) + 10.0,
                _ => span >= width + 12.0,
            };

            let at = if fits {
                place(LETTERING, text, middle, Anchor::CENTRE)
            } else {
                // Too short to hold its value: the value goes past the end.
                let beyond = snap(end + (end - start).normalize_or_zero() * 5.0);

                place(LETTERING, text, beyond, Anchor::LEFT)
            };

            lettering(text, at, tone, out);
        }
        Measure::Diagonal { a, b } => {
            let snap = |point: V2| Point::new(point.x.round() as i32, point.y.round() as i32);
            let (pa, pb) = (p.exact(a), p.exact(b));
            // From the pixel inside each corner, so the rectangle's edges,
            // which what it measures draws, are left whole.
            let inside = |from: Point<i32>, to: Point<i32>| {
                Point::new(
                    from.x + (to.x - from.x).signum(),
                    from.y + (to.y - from.y).signum(),
                )
            };
            let (sa, sb) = (snap(pa), snap(pb));
            let (sa, sb) = (inside(sa, sb), inside(sb, sa));
            let low = Point::new(sa.x.min(sb.x), sa.y.min(sb.y));
            let high = Point::new(sa.x.max(sb.x), sa.y.max(sb.y));

            out.push(path(shape::line(sa, sb), Tone::Line));

            if pa.distance(pb) >= 12.0 {
                arrowhead_within(sa, pa - pb, Some((low, high)), Tone::Line, out);
                arrowhead_within(sb, pb - pa, Some((low, high)), Tone::Line, out);
            }

            if text.is_empty() {
                return;
            }

            // In the break at the middle if its ground is inside the
            // rectangle, clear of its edges; past the end otherwise.
            let width = i32::from(LETTERING.width(text));
            let ground = (width + 3, i32::from(LETTERING.cap()) + 4);
            let at = if high.x - low.x > ground.0 + 2 && high.y - low.y > ground.1 + 2 {
                place(LETTERING, text, snap(pa.lerp(pb, 0.5)), Anchor::CENTRE)
            } else {
                let beyond = snap(pb + (pb - pa).normalize_or_zero() * 5.0);

                place(LETTERING, text, beyond, Anchor::LEFT)
            };

            lettering(text, at, tone, out);
        }
        Measure::Radial {
            centre,
            radius,
            angle,
            reach,
        } => {
            let tip = p.exact(centre + polar(radius, angle));
            let outward = v(angle.cos(), -angle.sin());
            let corner = tip + outward * reach as f32;
            let snap = |point: V2| Point::new(point.x.round() as i32, point.y.round() as i32);
            let tip_px = snap(tip);

            arrowhead(tip_px, -outward, Tone::Line, out);

            let elbow = snap(corner);

            callout(
                tip_px,
                (
                    elbow.x - tip_px.x + if outward.x >= 0.0 { 0 } else { -1 },
                    elbow.y - tip_px.y,
                ),
                text,
                tone,
                out,
            );
        }
        Measure::Angle {
            vertex,
            from,
            to,
            radius,
        } => {
            let centre = p.px(vertex);
            let mut pixels = clockwise_arc(centre, radius, bearing(to), bearing(from));
            pixels.reverse();

            out.push(path(pixels, Tone::Line));

            let middle = from + super::geom::wrap(to - from) / 2.0;
            let at = p.exact(vertex) + v(middle.cos(), -middle.sin()) * (radius as f32 + 8.0);

            lettering(
                text,
                place(
                    LETTERING,
                    text,
                    Point::new(at.x.round() as i32, at.y.round() as i32),
                    Anchor::CENTRE,
                ),
                tone,
                out,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draft::Draft;

    fn pixels(mark: &Mark, projection: &Projection) -> Vec<Point<i32>> {
        let mut out = Vec::new();
        rasterize(mark, projection, &mut out);

        match &out[0].piece {
            Piece::Path { pixels, .. } => pixels.clone(),
            other => panic!("not a path: {other:?}"),
        }
    }

    const UNIT: Projection = Projection::new((50.0, 50.0), 1.0);

    #[test]
    fn the_model_is_drawn_y_up() {
        assert_eq!(UNIT.px(v(10.0, 10.0)), Point::new(60, 40));
    }

    #[test]
    fn a_circle_is_walked_round_from_the_top() {
        let mut draft = Draft::new();
        draft.circle(V2::ZERO, 10.0, Line::Outline);
        let path = pixels(&draft.marks()[0], &UNIT);

        assert_eq!(path[0], Point::new(50, 40));
        // Neighbours along the path touch, all the way round.
        for pair in path.windows(2) {
            assert!((pair[0].x - pair[1].x).abs() <= 1 && (pair[0].y - pair[1].y).abs() <= 1);
        }
    }

    #[test]
    fn an_arc_starts_at_its_start_and_turns_its_way() {
        let mut draft = Draft::new();
        // A quarter turn counter-clockwise from east: east to north.
        draft.arc(
            V2::ZERO,
            10.0,
            0.0,
            std::f32::consts::FRAC_PI_2,
            Line::Outline,
        );
        draft.arc(
            V2::ZERO,
            10.0,
            0.0,
            -std::f32::consts::FRAC_PI_2,
            Line::Outline,
        );
        let up = pixels(&draft.marks()[0], &UNIT);
        let down = pixels(&draft.marks()[1], &UNIT);

        assert_eq!(up[0], Point::new(60, 50));
        assert_eq!(*up.last().unwrap(), Point::new(50, 40));
        assert_eq!(down[0], Point::new(60, 50));
        assert_eq!(*down.last().unwrap(), Point::new(50, 60));
    }

    /// The walked circle and arc are the toolkit's, pixel for pixel, in
    /// order round the dial.
    #[test]
    fn walked_circles_and_arcs_are_the_toolkits() {
        let sorted = |mut pixels: Vec<Point<i32>>| {
            pixels.sort_by_key(|p| (p.x, p.y));
            pixels
        };
        let centre = Point::new(7, -3);

        for radius in (0..400).chain([997, 1620, 5000, 20_604 / 5]) {
            let walked = ordered_circle(centre, radius);

            assert_eq!(
                sorted(walked.clone()),
                sorted(shape::circle(centre, radius)),
                "radius {radius}"
            );

            let bearings: Vec<f32> = walked
                .iter()
                .map(|p| shape::bearing(p.x - centre.x, p.y - centre.y))
                .collect();
            assert!(
                bearings.windows(2).all(|pair| pair[0] <= pair[1]),
                "radius {radius} out of order"
            );

            for (from, to) in [
                (0.0, 90.0),
                (300.0, 20.0),
                (45.0, 45.0),
                (123.4, 321.0),
                (10.0, 370.0),
            ] {
                assert_eq!(
                    sorted(clockwise_arc(centre, radius, from, to)),
                    sorted(shape::arc(centre, radius, from, to)),
                    "radius {radius}, {from} to {to}"
                );
            }
        }
    }

    #[test]
    fn line_types_are_their_patterns() {
        let pattern = |line| {
            (0..18)
                .map(|i| {
                    if Stipple::of(line).lights(i) {
                        '#'
                    } else {
                        '.'
                    }
                })
                .collect::<String>()
        };

        assert_eq!(pattern(Line::Outline), "##################");
        assert_eq!(pattern(Line::Hidden), "####...####...####");
        assert_eq!(pattern(Line::Centre), "###########..#..##");
        assert_eq!(pattern(Line::Phantom), "###########..#..#.");
    }

    #[test]
    fn a_dimension_measures_between_its_extension_lines() {
        let mut draft = Draft::new();
        draft.dim_h(v(0.0, 0.0), v(40.0, 0.0), 10.0);
        let mut out = Vec::new();
        rasterize(&draft.marks()[0], &UNIT, &mut out);

        let line = out
            .iter()
            .find_map(|inked| match &inked.piece {
                Piece::Path { pixels, .. } if pixels.len() == 41 => Some(pixels.clone()),
                _ => None,
            })
            .expect("a dimension line 41 pixels long");

        assert!(line.iter().all(|pixel| pixel.y == 40));
        assert!(
            out.iter()
                .any(|inked| matches!(&inked.piece, Piece::Text { text, .. } if text == "40"))
        );
    }

    /// A diagonal's arrowheads have their tips in its rectangle's corners
    /// and stay inside its edges, whatever its shape and wherever it falls
    /// on the grid; its value goes in the break at its middle.
    #[test]
    fn a_diagonals_arrowheads_stay_inside_its_rectangle() {
        for (width, height) in [
            (160.0, 90.0),
            (160.0, 100.0),
            (210.0, 90.0),
            (90.0, 160.0),
            (100.0, 100.0),
        ] {
            for shift in [0.0, 0.3, 0.5, 0.7] {
                let a = v(shift, shift * 0.7);
                let b = a + v(width, height);
                let mut draft = Draft::new();
                draft.dim_diagonal(a, b).text("27.0\"");
                let mut out = Vec::new();
                rasterize(&draft.marks()[0], &UNIT, &mut out);

                // The rectangle's corners, and the pixels inside them.
                let (low, high) = (UNIT.px(v(a.x, b.y)), UNIT.px(v(b.x, a.y)));
                let (low, high) = (
                    Point::new(low.x + 1, low.y + 1),
                    Point::new(high.x - 1, high.y - 1),
                );
                let heads: Vec<&[(i32, i32, i32)]> = out
                    .iter()
                    .filter_map(|inked| match &inked.piece {
                        Piece::Rows { rows, .. } => Some(rows.as_slice()),
                        _ => None,
                    })
                    .collect();
                let on = |corner: Point<i32>, head: &[(i32, i32, i32)]| {
                    head.iter()
                        .any(|&(y, from, to)| y == corner.y && from <= corner.x && corner.x <= to)
                };
                let at = format!("{width} × {height} at {shift}");

                assert_eq!(heads.len(), 2, "{at}");
                assert!(
                    on(Point::new(low.x, high.y), heads[0])
                        && on(Point::new(high.x, low.y), heads[1]),
                    "{at}"
                );

                for &(y, from, to) in heads.iter().copied().flatten() {
                    assert!(
                        (low.y..=high.y).contains(&y) && low.x <= from && to <= high.x,
                        "{at}: row {y} from {from} to {to} leaves {low:?} to {high:?}"
                    );
                }

                let middle = UNIT.px(a.lerp(b, 0.5));
                assert!(out.iter().any(|inked| matches!(
                    inked.piece,
                    Piece::Knockout(area) if contains(area, middle)
                )));
            }
        }
    }

    #[test]
    fn a_stroke_cut_in_pieces_keeps_its_pattern() {
        let pixels = shape::line(Point::new(0, 0), Point::new(99, 0));
        let lit = |pieces: &[Piece]| -> Vec<Point<i32>> {
            pieces
                .iter()
                .flat_map(|piece| match piece {
                    Piece::Path {
                        pixels,
                        stipple,
                        phase,
                    } => pixels
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| stipple.lights(phase + i))
                        .map(|(_, pixel)| *pixel)
                        .collect::<Vec<_>>(),
                    _ => Vec::new(),
                })
                .collect()
        };
        let whole = Piece::path(pixels.clone(), Stipple::of(Line::Centre));
        let cut = whole.clone().split(7);

        assert_eq!(cut.len(), 15);
        assert_eq!(lit(&cut), lit(&[whole]));
        assert_eq!(cut.iter().map(Piece::cost).sum::<usize>(), 100);
    }

    #[test]
    fn lining_keeps_off_its_areas_edge() {
        let mut draft = Draft::new();
        let contour = [
            v(-20.0, -10.0),
            v(15.0, -14.0),
            v(22.0, 12.0),
            v(-18.0, 16.0),
        ];
        draft.hatch(&contour);
        draft.polygon(&contour, Line::Outline);

        let mut out = Vec::new();
        for mark in draft.marks() {
            rasterize(mark, &UNIT, &mut out);
        }

        let edge: Vec<Point<i32>> = match &out[1].piece {
            Piece::Path { pixels, .. } => pixels.clone(),
            other => panic!("not the edge: {other:?}"),
        };
        let Piece::Rows { rows, .. } = &out[0].piece else {
            panic!("not the lining");
        };

        assert!(!rows.is_empty());
        for &(y, from, to) in rows {
            assert!(
                !edge
                    .iter()
                    .any(|pixel| pixel.y == y && (from..=to).contains(&pixel.x)),
                "row {y} from {from} to {to} touches the edge"
            );
        }
    }

    /// The scales the sheets draw at, in the view and magnified, and some
    /// that round a half either way.
    const SCALES: [f32; 8] = [0.92, 1.0, 1.18, 1.25, 1.5, 1.64, 2.36, 3.28];

    /// A figure set on the grid as one is drawn the same wherever it falls:
    /// lines either side of its point the same distance from its pixel, and
    /// a figure set inside it from a point and back on its own pixel.
    #[test]
    fn a_snapped_figure_is_drawn_alike_wherever_it_falls() {
        for scale in SCALES {
            let projection = Projection::new((50.0, 50.0), scale);
            let mut shapes = Vec::new();

            for k in 0..16 {
                let at = v(100.0 + k as f32 * 0.37, -80.0 - k as f32 * 0.29);
                let mut draft = Draft::new();

                draft.snapped(at, |d| {
                    d.line(at + v(-3.0, 2.0), at + v(30.0, 2.0), Line::Outline);
                    d.line(at + v(-3.0, -2.0), at + v(30.0, -2.0), Line::Outline);
                    d.snapped(at + v(4.5, -7.5), |d| {
                        d.line(at, at + v(1.0, 0.0), Line::Outline);
                    });
                });

                let centre = projection.px(at);
                let relative: Vec<Vec<(i32, i32)>> = draft
                    .marks()
                    .iter()
                    .map(|mark| {
                        pixels(mark, &projection)
                            .iter()
                            .map(|p| (p.x - centre.x, p.y - centre.y))
                            .collect()
                    })
                    .collect();

                assert_eq!(relative[0][0].1, -relative[1][0].1, "at {scale}");
                assert_eq!(relative[2][0], (0, 0), "at {scale}");
                shapes.push(relative);
            }

            assert!(
                shapes.windows(2).all(|pair| pair[0] == pair[1]),
                "at {scale}"
            );
        }
    }

    /// A keyhole's outline is one closed stroke, the same either side of
    /// its centre line, and what fills it stays inside it, filling every
    /// row it crosses.
    #[test]
    fn a_keyhole_is_closed_symmetric_and_filled_inside() {
        for scale in SCALES {
            for (radius, half) in [(3.0, 2.0), (3.5, 1.5), (5.0, 2.0)] {
                let projection = Projection::new((50.0, 50.0), scale);
                let shape = Shape::Keyhole {
                    centre: V2::ZERO,
                    radius,
                    half,
                    length: 40.0,
                };
                let outline = stroke(&shape, &projection);
                let at = format!("{radius}, {half} at {scale}");

                for pair in outline
                    .windows(2)
                    .chain([[outline[outline.len() - 1], outline[0]].as_slice()])
                {
                    assert!(
                        (pair[0].x - pair[1].x).abs() <= 1 && (pair[0].y - pair[1].y).abs() <= 1,
                        "{at}: {:?} to {:?}",
                        pair[0],
                        pair[1]
                    );
                }

                let mut sorted = outline.clone();
                let mut mirrored: Vec<Point<i32>> =
                    outline.iter().map(|p| Point::new(p.x, 100 - p.y)).collect();
                sorted.sort_by_key(|p| (p.x, p.y));
                mirrored.sort_by_key(|p| (p.x, p.y));
                assert_eq!(sorted, mirrored, "{at}");

                let rows = inside(&outline);
                for &(y, from, to) in &rows {
                    assert!(
                        !outline
                            .iter()
                            .any(|p| p.y == y && (from..=to).contains(&p.x)),
                        "{at}: row {y} touches the glass"
                    );
                    assert!(outline.iter().any(|p| p.y == y && p.x == from - 1), "{at}");
                    assert!(outline.iter().any(|p| p.y == y && p.x == to + 1), "{at}");
                }

                let top = outline.iter().map(|p| p.y).min().unwrap();
                assert_eq!(rows.len() as i32, 2 * (50 - top) - 1, "{at}");
            }
        }
    }

    /// A dot of an odd size is centred on its pixel; an even one is as
    /// near its point as the grid allows.
    #[test]
    fn dots_are_centred_on_their_points() {
        let block = |at: V2, size: i32| match dot(at, size, Tone::Live).piece {
            Piece::Block(block) => block,
            other => panic!("not a dot: {other:?}"),
        };

        assert_eq!(block(v(10.0, 20.0), 3), rect(9, 19, 3, 3));
        assert_eq!(block(v(10.4, 19.6), 3), rect(9, 19, 3, 3));
        assert_eq!(block(v(10.3, 20.7), 2), rect(10, 20, 2, 2));
    }

    #[test]
    fn plotting_part_of_a_path_stops_the_pen_inside_it() {
        let piece = Piece::path(
            shape::line(Point::new(0, 0), Point::new(9, 0)),
            Stipple::Solid,
        );

        assert_eq!(piece.cost(), 10);
    }
}
