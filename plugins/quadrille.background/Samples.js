.pragma library
.import "Pen.js" as Pen

// SAMPLES: PATTERN -> RESULT. What the drawing grid does to fine stripes.
//
// Three gratings are shown twice, enlarged 6:1: as intended (PATTERN, the
// stripes sampled six times finer than the grid) and as this output's grid
// draws them (RESULT). The result is not drawn to suggest anything: it is made
// by the sheet's own path, a scratch pen's raster point-sampling the grating at
// each virtual pixel's integer coordinate, and every stroke of that scratch pen
// is then drawn enlarged. Optionally the same raster is drawn again at 1:1.
//
//   RESOLVED      period 2 vpx: the finest the grid draws
//   FALSE DETAIL  period 4/3 vpx: comes out as a 4 vpx period (ALIASED)
//   DETAIL LOST   period 1 vpx: comes out flat
//
// measure(ctx) -> {w, h} or null; draw(pen, x, y, ctx) -> info.
// Element config: cells (8; 6 when compact), compact (false), actual (false).

var E = 6                    // enlargement: one virtual pixel is E wide
var VIEW = 18                // height of every view
var PITCH = 34               // from one row's view to the next
var GAP = 25                 // between the PATTERN and RESULT views: frame, 5 air, a 13 wide arrow, 5 air, frame
var NAMES = 8                // from the last view to the case names
var NUMBERS = 8              // from the number column to the pattern
var ROWS = [
  {n: "2",   num: 2, den: 1, phase: 0, label: "RESOLVED",     tech: null},
  {n: "4/3", num: 4, den: 3, phase: 3, label: "FALSE DETAIL", tech: "ALIASED"},
  {n: "1",   num: 1, den: 1, phase: 0, label: "DETAIL LOST",  tech: null}
]

// Is virtual pixel t (an integer, or an enlarged pixel over E) of the grating
// of period num/den, shifted by phase, in a light stripe? Exact: with t = X/E,
// floor(2 (t + phase) / n) = floor(2 den (X + E phase) / (E num)).
function lit(row, X, scale) {
  var s = scale || 1
  var k = Math.floor((2 * row.den * (X + s * row.phase)) / (s * row.num))
  return k % 2 === 0
}
function role(row, X, scale) { return lit(row, X, scale) ? "ink" : "void" }

function config(ctx) {
  var compact = !!ctx.compact
  var cells = Math.round(Number(ctx.cells) || (compact ? 6 : 8))
  return {compact: compact, cells: Math.max(2, cells), actual: !!ctx.actual}
}

// A glyph cell is six wide and the box seven: the width pen.span would give.
function span(text) { return Array.from(String(text)).length * 6 + 1 }

function numbers(ctx) {
  var mm = ctx.physical && ctx.physical.mmPerVpxX
  if (!(mm > 0) || !isFinite(mm)) return null
  var est = ctx.est || ""
  return ROWS.map(function(r) {
    var f = r.den / (r.num * mm)
    return {frequency: f, text: est + f.toFixed(2)}
  })
}

function layout(ctx) {
  var nums = numbers(ctx)
  if (!nums) return null
  var c = config(ctx), vw = c.cells * E
  var column = Math.max(span("PAIRS"), span("PER mm"))
  nums.forEach(function(n) { column = Math.max(column, span(n.text)) })
  var px = column + NUMBERS, rx = px + vw + GAP
  var ax = rx + vw + 7           // the 1:1 patch: its frame 5 clear of the result's
  var edge = c.actual ? ax + c.cells : rx + vw
  var names = Math.max(span("RESOLVED"), span("FALSE DETAIL"), span("DETAIL LOST"))
  var right = Math.max(edge + NAMES + names, rx + span("RESULT 6:1"), px + 18 + span("1 PAIR"))
  // Top: headings (two lines, 15 apart: labels keep 3 clear), the bracket's label, the bracket, then the first view.
  var top0 = 48
  var h = top0 + 2 * PITCH + VIEW + 4
  return {c: c, nums: nums, vw: vw, column: column, px: px, rx: rx, ax: ax, edge: edge, top0: top0,
    w: right, h: h}
}

function measure(ctx) {
  var l = layout(ctx)
  return l ? {w: l.w, h: l.h} : null
}

// A scratch pen of the grid's own cells, rastered by the sheet's raster path.
function sample(pen, row, cells) {
  var scratch = Pen.make(cells, VIEW, pen.glyphs, pen.big, {L: 0, T: 0, R: cells - 1, B: VIEW})
  scratch.raster({x: 0, y: 0, w: cells, h: VIEW}, function(x, y) { return role(row, x) })
  return scratch.strokes
}

function draw(pen, x, y, ctx) {
  var l = layout(ctx)
  if (!l) return null
  var c = l.c, E_ = E, vw = l.vw
  var px = x + l.px, rx = x + l.rx, ax = x + l.ax
  var info = {factor: E_, cells: c.cells, compact: c.compact, x: x, y: y, w: l.w, h: l.h,
    cases: [], bracket: null, numbers: [], headings: {}, labels: []}
  // Every label of the element, with its box, so a render can be checked against it.
  function say(text, tx, ty, kind, right) {
    var bx = right ? tx - span(text) + 1 : tx
    if (right) pen.textRight(text, tx, ty, kind); else pen.text(text, tx, ty, kind)
    info.labels.push({text: text, x: bx, y: ty, w: span(text), h: 12, role: kind})
  }

  // Headings: the frequency column's two lines, PATTERN and RESULT 6:1 on the lower one.
  var hy = y + 15
  say("PAIRS", x + l.column - 1, y, "muted", true)
  say("PER mm", x + l.column - 1, hy, "muted", true)
  say("PATTERN", px, hy, "muted")
  say("RESULT 6:1", rx, hy, "muted")
  info.headings = {pairs: {x: x, y: y}, pattern: {x: px, y: hy}, result: {x: rx, y: hy}}

  for (var i = 0; i < ROWS.length; i++) {
    var row = ROWS[i], vy = y + l.top0 + i * PITCH
    var kase = {n: row.n, phase: row.phase, frequency: l.nums[i].frequency, text: l.nums[i].text,
      label: row.label, technical: row.tech}

    // The number, right-aligned in its column, centred on the view.
    say(l.nums[i].text, x + l.column - 1, vy + 3, "muted", true)
    info.numbers.push(l.nums[i].text)

    // Every view has a one pixel `edge` frame one pixel outside it, so a window that
    // begins or ends on a void stripe still shows its extent.
    pen.outline(px - 1, vy - 1, vw + 2, VIEW + 2, "edge")
    pen.outline(rx - 1, vy - 1, vw + 2, VIEW + 2, "edge")
    if (c.actual) pen.outline(ax - 1, vy - 1, c.cells + 2, VIEW + 2, "edge")

    // PATTERN: the grating sampled E times finer than the grid.
    var roles = []
    for (var X = 0; X < vw; X++) roles.push(role(row, X, E_))
    var start = 0
    for (var X = 1; X <= vw; X++) {
      if (X === vw || roles[X] !== roles[start]) { pen.rect(px + start, vy, X - start, VIEW, roles[start]); start = X }
    }
    pen.target({x: px - 1, y: vy - 1, w: vw + 2, h: VIEW + 5, kind: "samples-pattern"})
    // Where the drawing grid samples it: one tick under every cell's left edge, a pixel below the frame.
    var ticks = []
    for (var k = 0; k < c.cells; k++) { pen.rect(px + k * E_, vy + VIEW + 2, 1, 2, "line"); ticks.push(px + k * E_) }
    kase.pattern = {x: px, y: vy, w: vw, h: VIEW, roles: roles, frame: {x: px - 1, y: vy - 1, w: vw + 2, h: VIEW + 2}}
    kase.ticks = ticks

    // The arrow: a 10 wide shaft, a solid head, 5 clear of both frames.
    var ay = vy + 8, shaft = px + vw + 6
    pen.leader(shaft, ay, shaft + 9, ay, "line")
    pen.arrow(shaft + 12, ay, "r", "line")
    kase.arrow = {x0: shaft, x1: shaft + 12, y: ay}

    // RESULT: the sheet's raster, every stroke of it enlarged E times.
    var strokes = sample(pen, row, c.cells)
    for (var s = 0; s < strokes.length; s++)
      pen.rect(rx + strokes[s].x * E_, vy + strokes[s].y, strokes[s].w * E_, strokes[s].h, strokes[s].role)
    pen.target({x: rx - 1, y: vy - 1, w: vw + 2, h: VIEW + 2, kind: "samples-result"})
    var cells = []
    for (var k = 0; k < c.cells; k++) cells.push(null)
    for (var s = 0; s < strokes.length; s++) {
      if (strokes[s].y !== 0) continue
      for (var k = strokes[s].x; k < strokes[s].x + strokes[s].w; k++) cells[k] = strokes[s].role
    }
    kase.result = {x: rx, y: vy, w: vw, h: VIEW, cells: cells, frame: {x: rx - 1, y: vy - 1, w: vw + 2, h: VIEW + 2}}

    // The same raster at its real size.
    kase.actual = null
    if (c.actual) {
      for (var s = 0; s < strokes.length; s++)
        pen.rect(ax + strokes[s].x, vy + strokes[s].y, strokes[s].w, strokes[s].h, strokes[s].role)
      pen.target({x: ax - 1, y: vy - 1, w: c.cells + 2, h: VIEW + 2, kind: "samples-actual"})
      kase.actual = {x: ax, y: vy, w: c.cells, h: VIEW, frame: {x: ax - 1, y: vy - 1, w: c.cells + 2, h: VIEW + 2}}
      if (i === 0) say("1:1", ax, vy - 17, "muted")
    }

    // The case, right of the last view, centred on the row; the technical word under it.
    var nx = x + l.edge + NAMES
    say(row.label, nx, vy + 3, "ink")
    if (row.tech) say(row.tech, nx, vy + 18, "muted")
    kase.name = {x: nx, y: vy + 3}
    info.cases.push(kase)

    // The bracket over the first view's first light and dark stripe: one pair.
    if (i === 0) {
      var by = vy - 5, x0 = px, x1 = px + row.num * E_ - 1
      pen.leader(x0, by, x1, by, "line")
      pen.leader(x0, by, x0, by + 1, "line")
      pen.leader(x1, by, x1, by + 1, "line")
      say("1 PAIR", x1 + 7, by - 12, "muted")
      info.bracket = {x0: x0, x1: x1, y: by, label: {x: x1 + 7, y: by - 12}}
    }
  }
  return info
}

// Names the sheet's composer is likely to ask for.
function measureSamples(ctx) { return measure(ctx) }
function drawSamples(pen, x, y, ctx) { return draw(pen, x, y, ctx) }
