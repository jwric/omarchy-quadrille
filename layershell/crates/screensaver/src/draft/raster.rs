//! Marks on the pixel grid.
//!
//! [`rasterize`] turns a [`Mark`] into [`Piece`]s: paths whose pixels are in
//! the order a pen would draw them, rows of an area, lines of text. The order
//! is what lets a sheet plot a drawing a pixel at a time, and lets a line type
//! be a pattern counted along the path, so a dashed circle is dashed the same
//! way round as a dashed line is along.
use iced_core::{Color, Point, Rectangle};
use quadrille::draw::{Anchor, Horizontal, Pen, Polygon, Vertical, shape};
use quadrille::{Face, Palette};

use super::{Axis, Extent, Fill, Ink, Line, Mark, Measure, Placement, Shape, Tone, V2, polar, v};

/// Where a model lands on the sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    /// The pixel the model's origin falls on.
    pub origin: (f32, f32),
    /// Pixels per model unit.
    pub scale: f32,
}

impl Projection {
    /// `extent` centred in `area` at `scale` pixels per unit. The origin is
    /// put on a whole pixel, so a model point a whole number of pixels from
    /// it lands exactly.
    pub fn centred(extent: Extent, area: Rectangle<i32>, scale: f32) -> Self {
        let centre = extent.centre();
        let x = area.x as f32 + area.width as f32 / 2.0;
        let y = area.y as f32 + area.height as f32 / 2.0;

        Self {
            origin: (
                (x - centre.x * scale).round(),
                (y + centre.y * scale).round(),
            ),
            scale,
        }
    }

    pub fn px(&self, point: V2) -> Point<i32> {
        Point::new(
            (self.origin.0 + point.x * self.scale).round() as i32,
            (self.origin.1 - point.y * self.scale).round() as i32,
        )
    }

    /// The same point, not rounded: for directions and placements.
    fn exact(&self, point: V2) -> V2 {
        v(
            self.origin.0 + point.x * self.scale,
            self.origin.1 - point.y * self.scale,
        )
    }

    pub fn length(&self, length: f32) -> i32 {
        (length * self.scale).round() as i32
    }
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

    fn lights(self, x: i32, y: i32) -> bool {
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

        let head = match self {
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

                partial
                    .then(|| drawn.last().map(|pixel| Head(*pixel)))
                    .flatten()
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

                partial
                    .then(|| {
                        rows.get(count)
                            .map(|&(y, from, _)| Head(Point::new(from, y)))
                    })
                    .flatten()
            }
            Self::Text { at, text } => {
                let face = &LETTERING;
                let typed = if partial {
                    budget / TEXT_COST
                } else {
                    text.chars().count()
                };
                let advance = i32::from(face.advance());
                let cap_top = at.y + i32::from(face.cap_top());
                let baseline = at.y + i32::from(face.baseline());

                // Whole characters, only where their capitals are inside.
                if cap_top >= clip.y && baseline <= clip.y + clip.height {
                    let visible: Vec<usize> = (0..typed)
                        .filter(|i| {
                            let x = at.x + *i as i32 * advance;
                            x >= clip.x && x + advance <= clip.x + clip.width
                        })
                        .collect();

                    if let (Some(&first), Some(&last)) = (visible.first(), visible.last()) {
                        let shown: String =
                            text.chars().skip(first).take(last - first + 1).collect();

                        super::letters::write(
                            pen,
                            &shown,
                            Point::new(at.x + first as i32 * advance, at.y),
                            color,
                        );
                    }
                }

                partial.then(|| Head(Point::new(at.x + advance * typed as i32, baseline - 1)))
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
                None
            }
        };

        head.filter(|Head(at)| inside(at))
    }
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
fn fill_pixels<Renderer>(pen: &mut Pen<'_, Renderer>, pixels: &[Point<i32>], color: Color)
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

/// The pieces of `mark` under `projection`.
pub fn rasterize(mark: &Mark, projection: &Projection, out: &mut Vec<Inked>) {
    let tone = mark.tone;

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
                out.push(Inked {
                    piece: Piece::Rows {
                        rows: Polygon::new(points).rows(),
                        texture: Texture::of(*fill),
                        origin,
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

            out.push(dot(target, 3, Tone::Line));
            callout(target, (*x, *y), text, tone, out);
        }
        Ink::Balloon {
            item,
            target,
            offset: Placement::Offset(x, y),
        } => balloon(*item, projection.px(*target), (*x, *y), tone, out),
        Ink::Note { .. } | Ink::Balloon { .. } => {}
        Ink::Dot { at, size } => out.push(dot(projection.px(*at), *size, tone)),
    }
}

fn dot(at: Point<i32>, size: i32, tone: Tone) -> Inked {
    let size = size.max(1);

    Inked {
        piece: Piece::Block(rect(at.x - size / 2, at.y - size / 2, size, size)),
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
    }
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
    const DEPTH: f32 = 3.0;

    let along = towards.normalize_or_zero();

    if along == V2::ZERO {
        return;
    }

    let across = along.perp();
    let tip_f = v(tip.x as f32, tip.y as f32);
    let base = tip_f - along * DEPTH;
    let corner = |point: V2| Point::new(point.x.round() as i32, point.y.round() as i32);

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
    out.push(dot(target, 3, Tone::Line));
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

    const UNIT: Projection = Projection {
        origin: (50.0, 50.0),
        scale: 1.0,
    };

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
    fn plotting_part_of_a_path_stops_the_pen_inside_it() {
        let piece = Piece::path(
            shape::line(Point::new(0, 0), Point::new(9, 0)),
            Stipple::Solid,
        );

        assert_eq!(piece.cost(), 10);
    }
}
