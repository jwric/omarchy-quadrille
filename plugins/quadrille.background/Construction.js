.pragma library
.import "Star.js" as Star

// B, the exploded pixel construction: one small drawing that says what a
// virtual pixel is on this output. The same piece of the sheet's star, a window
// of whole virtual pixels across an edge, is shown twice, on the drawing grid
// (every virtual pixel one block) and on the panel's own pixels (every panel
// pixel one block), and one drawing-grid pixel is lifted off and exploded into
// the panel pixels it stands for. It depicts the renderer's grid and nothing
// about the panel itself.
//
//   windowOf(ctx)          the window on the star (sheet vpx), or null
//   markWindow(pen, ctx)   its marker on the star, to call after the star is drawn
//   measure(ctx)           {w, h, ...} or null when it cannot be drawn
//   draw(pen, x, y, ctx)   the figure, in [x, x+w) x [y, y+h); returns its info
//
// ctx: the element context (see the common brief) plus
//   cells     the window's side in virtual pixels (default 8, 6 when compact)
//   boundary  which wedge boundary the window sits on: its angle is
//             boundary * pi / 32 (screen angle, y down); default 36, up-left
//   at        the window centre's distance from the star's centre as a fraction
//             of the star's radius (default 0.8)
//   mirror    the layout mirrored left to right (views right, labels left); the
//             drawings themselves are not flipped. info.viewAnchor is the point 2 vpx
//             outside the coarse view's frame, facing away from the labels, at its middle row.
//   compact   a smaller drawing: 6 cells, a tile of side ps * round(9 / ps)

// The two side faces of the lifted tile, by role. The top face is the star's
// own lit tone. muted and edge sit on either side of faint in every theme and
// stay apart from the void and from each other (measured in the five themes).
var LEFT_FACE = "muted"
var RIGHT_FACE = "edge"
var SLAB = 3            // thickness of the lifted tile, in vpx
var TITLE = 24          // the title row: the large letter's box
var VIEW_TOP = 6        // gap under the title row to the first view
var VIEW_GAP = 12       // between the two views
var ISO_GAP = 18        // from the views' frames to the exploded pixel
var LABEL_GAP = 10      // from the exploded pixel to its labels
var FACTOR_GAP = 8      // from the letter to the factor beside it

function chars(text) { return Array.from(String(text)).length }
function span(text, n) { return chars(text) * 6 * (n || 1) + 1 }

// ---- the window on the star ------------------------------------------------

// The boundaries (k pi / 32, k = 0..63, rays from the star's centre) that cross
// the rectangle [x0, x1] x [y0, y1], offsets from the star's centre in vpx.
function crossings(ctx, x0, y0, x1, y1) {
  var mx = ctx.physical.mmPerVpxX, my = ctx.physical.mmPerVpxY
  var xs = [x0 * mx, x1 * mx], ys = [y0 * my, y1 * my]
  var mid = {x: (xs[0] + xs[1]) / 2, y: (ys[0] + ys[1]) / 2}, out = []
  for (var k = 0; k < 64; k++) {
    var dx = Math.cos(k * Math.PI / 32), dy = Math.sin(k * Math.PI / 32)
    var lo = Infinity, hi = -Infinity
    for (var i = 0; i < 2; i++) for (var j = 0; j < 2; j++) {
      var side = xs[i] * dy - ys[j] * dx
      lo = Math.min(lo, side); hi = Math.max(hi, side)
    }
    if (lo < -1e-9 && hi > 1e-9 && mid.x * dx + mid.y * dy > 0) out.push(k)
  }
  return out
}

// The mm distance from the origin of the nearest and the farthest point of a rectangle.
function reach(ctx, x0, y0, x1, y1) {
  var mx = ctx.physical.mmPerVpxX, my = ctx.physical.mmPerVpxY
  var nx = x0 > 0 ? x0 : (x1 < 0 ? x1 : 0), ny = y0 > 0 ? y0 : (y1 < 0 ? y1 : 0)
  var fx = Math.max(Math.abs(x0), Math.abs(x1)), fy = Math.max(Math.abs(y0), Math.abs(y1))
  return {near: Math.sqrt(nx * mx * nx * mx + ny * my * ny * my), far: Math.sqrt(fx * mx * fx * mx + fy * my * fy * my)}
}

function tryWindow(ctx, cells, boundary, at) {
  var star = ctx.star, mx = ctx.physical.mmPerVpxX, my = ctx.physical.mmPerVpxY
  var radius = star.diameter / 2, t = boundary * Math.PI / 32
  // the point on the boundary at that distance, in the star's own (mm) geometry
  var px = at * radius * Math.cos(t) / mx, py = at * radius * Math.sin(t) / my
  var half = Math.floor(cells / 2)
  var wx = Math.round(star.cx + px) - half, wy = Math.round(star.cy + py) - half
  var ox = wx - star.cx, oy = wy - star.cy        // offset of the first sample
  // the window's pixels (the fine view's last panel pixel ends just short of ox + cells)
  var found = crossings(ctx, ox - 0.5, oy - 0.5, ox + cells, oy + cells)
  if (found.length !== 1 || found[0] !== ((boundary % 64) + 64) % 64) return null
  // wholly inside the disc and outside the grid-limit ring, marker included
  var pitch = Math.max(mx, my)
  var r = reach(ctx, ox - 1.5, oy - 1.5, ox + cells + 0.5, oy + cells + 0.5)
  if (r.far > radius - 2 * pitch) return null
  if (r.near < Star.limitRadius() * mx + 2 * pitch) return null
  return {x: wx, y: wy, w: cells, h: cells, cells: cells, boundary: boundary, at: at}
}

// The window: cells x cells virtual pixels centred on the boundary at `at`
// of the star's radius, holding exactly that one wedge boundary. When the star
// is too small (or its pixels too big) for the asked cells, the largest even
// window down to 4 that does is used; null when none does.
function windowOf(ctx) {
  if (!ctx || !ctx.star || !ctx.physical) return null
  var boundary = ctx.boundary === undefined ? 36 : ctx.boundary
  var at = ctx.at === undefined ? 0.8 : ctx.at
  var asked = ctx.cells !== undefined ? ctx.cells : (ctx.compact ? 6 : 8)
  for (var cells = asked; cells >= 4; cells -= 2) {
    var w = tryWindow(ctx, cells, boundary, at)
    if (w) { w.asked = asked; return w }
  }
  return null
}

// The window's marker on the star: a one-pixel ink outline one pixel outside it.
function markWindow(pen, ctx) {
  var w = windowOf(ctx)
  if (!w) return null
  var m = {x: w.x - 1, y: w.y - 1, w: w.w + 2, h: w.h + 2}
  pen.outline(m.x, m.y, m.w, m.h, "ink")
  return m
}

// ---- the figure ------------------------------------------------------------

// Everything's place, relative to the figure's top-left corner.
function geometry(ctx) {
  var win = windowOf(ctx)
  if (!win) return null
  var ps = ctx.ps, cells = win.cells
  var block = Math.max(1, Math.round(6 / ps)), factor = block * ps, size = cells * factor
  var s = ps * Math.max(1, Math.round((ctx.compact ? 9 : 12) / ps))
  var g = {win: win, ps: ps, cells: cells, block: block, factor: factor, size: size, s: s}
  g.factorLabel = factor + ":1"
  g.letterW = span("B", 2)
  g.mirror = !!ctx.mirror
  g.label1 = {text: "DRAWING GRID \u00d7" + ps}
  g.label2 = {text: "PANEL PIXELS 1:1"}
  var labelW = Math.max(span(g.label1.text), span(g.label2.text))
  var frameLeft, isoLeft
  if (g.mirror) {
    // the labels on the left, right-aligned against the exploded pixel; the views on the right
    isoLeft = labelW + LABEL_GAP
    frameLeft = isoLeft + 4 * s + ISO_GAP
    g.label1.x = labelW - span(g.label1.text); g.label2.x = labelW - span(g.label2.text)
  } else {
    frameLeft = 0
    isoLeft = size + 2 + ISO_GAP
    g.label1.x = g.label2.x = isoLeft + 4 * s + LABEL_GAP
  }
  g.view1 = {x: frameLeft + 1, y: TITLE + VIEW_TOP}
  g.view2 = {x: frameLeft + 1, y: g.view1.y + size + VIEW_GAP}
  g.ox = isoLeft + 2 * s                       // the tile's and the block's centre column
  g.tileTop = g.view1.y + Math.floor((size - (2 * s + SLAB)) / 2)
  g.blockTop = g.view2.y + Math.floor((size - 2 * s) / 2)
  g.label1.y = g.view1.y + size / 2 - 6
  g.label2.y = g.view2.y + size / 2 - 6
  var titleW = g.letterW + FACTOR_GAP + span(g.factorLabel)
  g.w = Math.max(frameLeft + size + 2, g.mirror ? 0 : g.label1.x + labelW, titleW)
  // "B  F:1" reads left to right either way; mirrored it ends on the views' right frame
  g.title = {x: g.mirror ? g.w - titleW : 0, y: 0}
  // 2 vpx outside the coarse view's frame, on the side facing away from the labels, at its middle row
  g.anchor = {x: g.mirror ? frameLeft + size + 1 + 2 : frameLeft - 2, y: g.view1.y + size / 2}
  g.h = g.view2.y + size + 1
  return g
}

function measure(ctx) {
  var g = geometry(ctx)
  return g ? {w: g.w, h: g.h, cells: g.cells, factor: g.factor, s: g.s} : null
}

// ---- pixel-art isometric drawing (2:1: two pixels across to one down) ----

// A line of length L from the grid point (u, v) of an iso lattice whose origin is
// (ox, oy): U runs down-right, V down-left, each step two pixels wide.
function isoU(pen, ox, oy, u, v, L, role) {
  var x = ox + 2 * (u - v), y = oy + u + v
  for (var k = 0; k < L; k++) pen.rect(x + 2 * k, y + k, 2, 1, role)
}
function isoV(pen, ox, oy, u, v, L, role) {
  var x = ox + 2 * (u - v), y = oy + u + v
  for (var k = 0; k < L; k++) pen.rect(x - 2 - 2 * k, y + k, 2, 1, role)
}

// The rhombus of side s whose top vertex is (ox, oy): 4 s wide, 2 s tall, the
// first row four pixels, every row two more each side until the widest two rows.
function rhombus(pen, ox, oy, s, role) {
  for (var r = 0; r < 2 * s; r++) {
    var k = r < s ? r : 2 * s - 1 - r
    pen.rect(ox - 2 * k - 2, oy + r, 4 * k + 4, 1, role)
  }
}

function rhombusEdge(pen, ox, oy, s, role) {
  isoU(pen, ox, oy, 0, 0, s, role); isoV(pen, ox, oy, 0, 0, s, role)
  isoV(pen, ox, oy, s, 0, s, role); isoU(pen, ox, oy, 0, s, s, role)
}

// A lifted tile: its top face and the slab under it (two side faces, the
// bottom edge stepping the way the top's lower edges do).
function tile(pen, ox, oy, s, top, left, right) {
  rhombus(pen, ox, oy, s, top)
  for (var i = 0; i < 2 * s; i++) {
    var k = Math.floor(i / 2)
    pen.rect(ox - 2 * s + i, oy + s + k + 1, 1, SLAB, left)
    pen.rect(ox + 2 * s - 1 - i, oy + s + k + 1, 1, SLAB, right)
  }
}

// The same rhombus on the floor, in n x n cells: the cells lit, a void gap
// between them along both axes, the outline in `line`.
function cellBlock(pen, ox, oy, s, n, fill, gap, edge) {
  rhombus(pen, ox, oy, s, fill)
  var t = s / n
  for (var i = 1; i < n; i++) { isoU(pen, ox, oy, 0, i * t, s, gap); isoV(pen, ox, oy, i * t, 0, s, gap) }
  rhombusEdge(pen, ox, oy, s, edge)
}

// A view: every sample of a square window as a block of `unit` vpx, runs of one
// role merged into one rectangle a row of blocks tall.
function blocks(pen, x, y, n, unit, sample) {
  var roles = []
  for (var j = 0; j < n; j++) {
    var row = []
    for (var i = 0; i < n; i++) row.push(sample(i, j))
    roles.push(row)
    var start = 0
    for (var i = 1; i <= n; i++) {
      if (i === n || row[i] !== row[start]) {
        pen.rect(x + start * unit, y + j * unit, (i - start) * unit, unit, row[start])
        start = i
      }
    }
  }
  return roles
}

function draw(pen, x, y, ctx) {
  var g = geometry(ctx)
  if (!g) return null
  var star = ctx.star, ps = g.ps, win = g.win, size = g.size, cells = g.cells
  var high = star.high
  var info = {window: {x: win.x, y: win.y, w: win.w, h: win.h}, cells: cells, factor: g.factor, block: g.block,
    boundary: win.boundary, at: win.at, mirror: g.mirror,
    viewAnchor: {x: x + g.anchor.x, y: y + g.anchor.y}, sides: {left: LEFT_FACE, right: RIGHT_FACE}, labels: [], box: {x: x, y: y, w: g.w, h: g.h}}

  // the three drawings, registered first so every label is checked against them
  var v1 = {x: x + g.view1.x, y: y + g.view1.y}, v2 = {x: x + g.view2.x, y: y + g.view2.y}
  var ox = x + g.ox, tileY = y + g.tileTop, blockY = y + g.blockTop, s = g.s
  pen.target({x: v1.x - 1, y: v1.y - 1, w: size + 2, h: size + 2, kind: "construction.coarse"})
  pen.target({x: v2.x - 1, y: v2.y - 1, w: size + 2, h: size + 2, kind: "construction.fine"})
  pen.target({x: ox - 2 * s, y: tileY, w: 4 * s, h: blockY + 2 * s - tileY, kind: "construction.exploded"})

  // row 1, the drawing grid: the sheet's own pixels, each one a block of F x F
  var grid = Star.gridSampler(star, ctx.physical)
  var coarse = blocks(pen, v1.x, v1.y, cells, g.factor, function(i, j) {
    return grid(win.x + i, win.y + j) || star.low
  })
  pen.outline(v1.x - 1, v1.y - 1, size + 2, size + 2, "edge")
  // row 2, the panel's pixels: the same window, each panel pixel a b x b block,
  // measured from the panel pixel at the star's centre
  var panel = Star.panelSampler(star, ctx.physical)
  var fine = blocks(pen, v2.x, v2.y, cells * ps, g.block, function(a, b) {
    return panel((win.x - star.cx) * ps + a, (win.y - star.cy) * ps + b)
  })
  pen.outline(v2.x - 1, v2.y - 1, size + 2, size + 2, "edge")
  info.coarse = {x: v1.x, y: v1.y, size: size, roles: coarse}
  info.fine = {x: v2.x, y: v2.y, size: size, roles: fine}

  // the exploded pixel: the lifted tile, the panel pixels it stands for, and
  // the two projection lines joining their left and right corners
  var lineTop = tileY + s + SLAB + 2, lineBottom = blockY + s - 3
  if ((lineBottom - lineTop) % 2) lineTop++
  pen.vline(ox - 2 * s, lineTop, lineBottom, "line", [1, 1])
  pen.vline(ox + 2 * s - 1, lineTop, lineBottom, "line", [1, 1])
  cellBlock(pen, ox, blockY, s, ps, high, "void", "line")
  tile(pen, ox, tileY, s, high, LEFT_FACE, RIGHT_FACE)
  info.tile = {x: ox, y: tileY, s: s, slab: SLAB, top: high, left: LEFT_FACE, right: RIGHT_FACE}
  info.panel = {x: ox, y: blockY, s: s, n: ps, fill: high, gap: "void", edge: "line"}
  info.projection = {left: ox - 2 * s, right: ox + 2 * s - 1, y0: lineTop, y1: lineBottom, dash: [1, 1]}

  // the title row: the large B, then the factor, baselines level
  var put = function(text, tx, ty, role, n) {
    var ok = pen.text(text, tx, ty, role, n)
    info.labels.push({text: text, x: tx, y: ty, w: span(text, n), h: 12 * (n || 1), role: role, n: n || 1, drawn: ok})
  }
  put("B", x + g.title.x, y, "accent", 2)
  // the capitals of the large face end on row 20 of its 24, of the small face on row 10 of 12
  put(g.factorLabel, x + g.title.x + g.letterW + FACTOR_GAP, y + 10, "muted", 1)
  put(g.label1.text, x + g.label1.x, y + g.label1.y, "ink", 1)
  put(g.label2.text, x + g.label2.x, y + g.label2.y, "ink", 1)
  return info
}
