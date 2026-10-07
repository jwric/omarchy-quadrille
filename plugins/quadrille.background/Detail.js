.pragma library
.import "Star.js" as Star

// DETAIL A: the native pixel detail, a circular view of the star's centre on the
// panel's own pixels. On the sheet's star the accent GRID LIMIT ring (radius
// 32/pi virtual pixels: inside it the drawing grid can no longer resolve the
// wedges) is the detail circle "A"; this view is exactly the disc inside that
// ring, magnified and drawn on native pixels, and the same accent ring is its
// rim. Inside it a `caution` ring marks where the panel's own pixels stop
// resolving the wedges (32/pi panel pixels): PANEL LIMIT.
//
//   measure(ctx)               -> {w, h, ...} or null when there is no star to draw
//   draw(pen, x, y, ctx)       -> info (geometry in sheet vpx, every printed number)
//   anchor(ctx, x, y)          -> {cx, cy, rim}: the rim, for the sheet's own leader
//
// cfg (fields of ctx): factor (default 6), panelLeader ("se" default, "sw", "ne",
// "nw"), gridLeader ("sw" default; never the panel leader's direction), title
// (default true), pitch ("below" default | "above": the pitch line between the title
// row and the disc, nothing under the disc).
//
// The geometry. A panel pixel is a block of b x b virtual pixels, b = max(1,
// round(factor / ps)), so the printed factor is F = b * ps. Panel pixel (0, 0),
// the one at the star's centre, is the block whose top-left virtual pixel is
// (x0, y0); the disc is centred on the middle of that block, which sits on a half
// pixel when b is even. Every distance is held doubled, (U, V) = 2 * (offset from
// that centre), so the rings are exact integer arithmetic and, through the
// symmetric test on (U, V), 8-fold symmetric about the centre for either parity.
//
// A ring of doubled radius p is the pixels whose D = U^2 + V^2 lies in
// (p^2 - 2p, p^2 + 2p]: pen.circle's rule (within half a pixel of the radius, flat
// sides, no nub at the compass points) written for a centre on a half pixel too.
//
// Inclusion rule: the disc is the rim's inside, so a panel pixel is drawn wherever
// any of its virtual pixels lies inside the rim, and the rim cuts the blocks it
// crosses (the pixels under or beyond it are not drawn). Blocks chosen by their
// centres leave gaps of 2 to 5 virtual pixels along the rim on b = 2 and 3; this
// rule leaves none (the interior is filled right up to the ring).

var leaderDirections = {se: [1, 1], sw: [-1, 1], ne: [1, -1], nw: [-1, -1]}

function high(p) { return p*p+2*p }
function low(p) { return p*p-2*p }

// The largest |U| of a pixel inside or on a ring of doubled radius p, for pixels
// whose U has the parity of b - 1.
function reach(p, parity) {
  var u = parity
  while ((u+2)*(u+2) <= high(p)) u += 2
  return u
}

function layout(ctx) {
  if (!ctx || !ctx.star || !ctx.physical) return null
  var ps = ctx.ps > 0 ? ctx.ps : 1
  var factor = Number(ctx.factor) > 0 ? Number(ctx.factor) : 6
  var b = Math.max(1, Math.round(factor/ps))
  var limit = Star.limitRadius()
  var R = limit*ps                       // the grid limit in panel pixels
  var rim2 = Math.round(2*limit*ps*b)    // the grid limit ring, doubled radius, vpx
  var pan2 = Math.round(2*limit*b)       // the panel limit ring, doubled radius, vpx
  var parity = (b-1)%2
  var umax = reach(rim2, parity), S = umax+1
  var dirKey = leaderDirections[ctx.panelLeader] ? ctx.panelLeader : "se"
  // The rim's own leader: never the panel leader's direction.
  var gridKey = leaderDirections[ctx.gridLeader] ? ctx.gridLeader : "sw"
  if (gridKey === dirKey) gridKey = ["sw", "nw", "ne", "se"].filter(function(k) { return k !== dirKey })[0]
  var withTitle = ctx.title !== false
  var est = ctx.est || ""
  var phys = ctx.physical
  var F = b*ps

  // Everything below is in a working frame; shifted to the element's own at the end.
  // pitch "above": the PIXEL PITCH line is its own row between the title row and
  // the disc (4 clear of each), so that nothing is under the disc.
  var above = ctx.pitch === "above"
  var pitchY0 = withTitle ? 28 : 0
  var discX = 0, discY = (withTitle ? 28 : 0)+(above ? 16 : 0)
  var x0 = discX+(umax-b+1)/2, y0 = discY+(umax-b+1)/2    // top-left of block (0, 0)
  var cx = discX+umax/2, cy = discY+umax/2

  // A leader: a pixel diagonal along the 45 degree wedge boundary through the
  // top-left corner of block (0, 0) (the star's centre in the sampler's terms).
  function diagonal(key, t) {
    var sx = leaderDirections[key][0], sy = leaderDirections[key][1]
    if (sx > 0 && sy > 0) return [x0+t, y0+t]
    if (sx < 0 && sy < 0) return [x0-1-t, y0-1-t]
    if (sx > 0) return [x0+t, y0-1-t]
    return [x0-1-t, y0+t]
  }
  function dist2(p) {
    var u = 2*(p[0]-x0)-(b-1), v = 2*(p[1]-y0)-(b-1)
    return u*u+v*v
  }
  // From the ring's own pixel at that bearing out to 4 vpx outside the rim.
  function leaderFor(key, ringLow, text, shoulder) {
    var t0 = 0, t1
    while (dist2(diagonal(key, t0)) <= ringLow) t0++
    t1 = t0
    while (dist2(diagonal(key, t1)) < (rim2+9)*(rim2+9)) t1++
    return {key: key, sx: leaderDirections[key][0], start: diagonal(key, t0), elbow: diagonal(key, t1),
      text: text, shoulder: shoulder}
  }
  // On a panel whose pixels are the drawing grid's (ps 1) the panel's limit is the
  // grid's: the two rings are one, so the caution ring, its leader and its label
  // give way to the rim (the rings are drawn only when 3 vpx apart or more).
  var apart = rim2-pan2 >= 6
  var leaders = [leaderFor(gridKey, low(rim2), "GRID LIMIT", 12)]
  if (apart) leaders.push(leaderFor(dirKey, low(pan2), "PANEL LIMIT", 10))

  var texts = [], items = []
  function box(x, y, w, h) { items.push({x: x, y: y, w: w, h: h}) }
  box(discX, discY, S, S)

  // Title row: the letter A in the native 2x face, then the title; the small
  // face's capitals are bottom-aligned with the A's cap height.
  var title = null
  var titleLabel = "NATIVE PIXEL DETAIL"
  if (withTitle) {
    texts.push({text: "A", x: discX, y: 0, role: "accent", size: 2})
    texts.push({text: titleLabel+"  "+F+":1", x: discX+17, y: 10, role: "ink", tail: "  "+F+":1"})
    title = {x: discX, y: 0, w: 0, h: 24}    // width fixed by the pen's span, below
  }
  // Under the disc.
  var pitchX = Number(phys.mmPerPixelX), pitchY = Number(phys.mmPerPixelY)
  var pitch = "PIXEL PITCH "+est+pitchX.toFixed(3)
  if (Math.abs(pitchX-pitchY) > 0.0005) pitch += " × "+est+pitchY.toFixed(3)
  pitch += " mm"
  texts.push({text: pitch, x: discX, y: above ? pitchY0 : discY+S+4, role: "muted"})

  return {b: b, F: F, R: R, rim2: rim2, pan2: pan2, parity: parity, umax: umax, S: S,
    discX: discX, discY: discY, x0: x0, y0: y0, cx: cx, cy: cy,
    apart: apart, dir: dirKey, gridDir: gridKey, leaders: leaders,
    title: title, titleLabel: titleLabel, texts: texts, items: items, withTitle: withTitle,
    pitch: pitch, pitchAbove: above, ps: ps}
}

// Spans need the pen (the glyph table), so the final frame is fixed at draw and
// measure time through a pen-like span function.
function place(ctx, span) {
  var m = layout(ctx)
  if (!m) return null
  var items = m.items.slice()
  var S = m.S, discLeft = m.discX, discRight = m.discX+S
  var texts = []
  for (var i = 0; i < m.texts.length; i++) {
    var t = m.texts[i], w = span(t.text, t.size)
    texts.push({text: t.text, x: t.x, y: t.y, w: w, h: 12*(t.size || 1), role: t.role, size: t.size || 1, tail: t.tail})
    items.push({x: t.x, y: t.y, w: w, h: 12*(t.size || 1)})
  }
  if (m.title) m.title.w = 17+span(m.titleLabel+texts[1].tail)
  // The shoulders and the limits' labels. A label clears the disc's box by 4 and
  // its shoulder's end by 4; a shoulder is at least 10 (panel) or 12 (grid) long.
  for (var n = 0; n < m.leaders.length; n++) {
    var ld = m.leaders[n], lw = span(ld.text)
    var ex = ld.elbow[0], ey = ld.elbow[1], lx, x2
    if (ld.sx > 0) { lx = Math.max(ex+ld.shoulder+5, discRight+4); x2 = lx-5 }
    else { var right = Math.min(ex-ld.shoulder-4, discLeft-4); lx = right-lw; x2 = right+4 }
    ld.x2 = x2
    texts.push({text: ld.text, x: lx, y: ey-6, w: lw, h: 12, role: "ink", size: 1})
    items.push({x: lx, y: ey-6, w: lw, h: 12})
    items.push({x: Math.min(ex, x2), y: ey, w: Math.abs(x2-ex)+1, h: 1})
    items.push({x: Math.min(ld.start[0], ex), y: Math.min(ld.start[1], ey), w: Math.abs(ex-ld.start[0])+1, h: Math.abs(ey-ld.start[1])+1})
  }
  if (m.title) items.push({x: m.title.x, y: m.title.y, w: m.title.w, h: m.title.h})
  var minX = 1e9, minY = 1e9, maxX = -1e9, maxY = -1e9
  for (var k = 0; k < items.length; k++) {
    minX = Math.min(minX, items[k].x); minY = Math.min(minY, items[k].y)
    maxX = Math.max(maxX, items[k].x+items[k].w); maxY = Math.max(maxY, items[k].y+items[k].h)
  }
  m.shift = {x: -minX, y: -minY}
  m.w = maxX-minX; m.h = maxY-minY
  m.texts = texts
  return m
}

function measure(ctx) {
  var m = place(ctx, function(text, n) { return measureSpan(text, n) })
  return m ? {w: m.w, h: m.h, disc: m.S, block: m.b, factor: m.F} : null
}

// The glyph table is not importable here (the pen owns it), so measure spans the
// way the pen does: one 6 x 12 cell per character, 6 n a cell, plus 1.
function measureSpan(text, n) { return Array.from(String(text)).length*6*(n || 1)+1 }

function anchor(ctx, x, y) {
  var m = place(ctx, measureSpan)
  if (!m) return null
  return {cx: x+m.shift.x+m.cx, cy: y+m.shift.y+m.cy, rim: m.rim2/2}
}

function draw(pen, x, y, ctx) {
  var m = place(ctx, pen.span)
  if (!m) return null
  var ox = x+m.shift.x, oy = y+m.shift.y
  var b = m.b, bx = ox+m.x0, by = oy+m.y0
  var box = {x: ox+m.discX, y: oy+m.discY, w: m.S, h: m.S}
  var tone = Star.panelSampler(ctx.star, ctx.physical)
  var rimHigh = high(m.rim2), rimLow = low(m.rim2), panHigh = high(m.pan2), panLow = low(m.pan2)
  function ring(lo, hi, role) {
    pen.raster(box, function(px, py) {
      var u = 2*(px-bx)-(b-1), v = 2*(py-by)-(b-1), d = u*u+v*v
      return d > lo && d <= hi ? role : null
    })
  }

  pen.target({x: box.x, y: box.y, w: box.w, h: box.h, kind: "detail"})
  // The panel's own pixels, every one a block of b x b, inside the rim.
  var blocks = 0
  pen.raster(box, function(px, py) {
    var u = 2*(px-bx)-(b-1), v = 2*(py-by)-(b-1)
    if (u*u+v*v > rimLow) return null
    return tone(Math.floor((px-bx)/b), Math.floor((py-by)/b))
  })
  var N = Math.ceil(m.rim2/(2*b))+2
  for (var j = -N; j <= N; j++) for (var i = -N; i <= N; i++) {
    var seen = false
    for (var q = 0; q < b && !seen; q++) for (var p = 0; p < b && !seen; p++) {
      var u = 2*(i*b+p)-(b-1), v = 2*(j*b+q)-(b-1)
      if (u*u+v*v <= rimLow) seen = true
    }
    if (seen) blocks++
  }
  // The leaders go under the rings, which stay whole.
  var info_leaders = {}
  for (var n = 0; n < m.leaders.length; n++) {
    var ld = m.leaders[n], st = ld.start, el = ld.elbow
    pen.leader(ox+st[0], oy+st[1], ox+el[0], oy+el[1], "muted")
    pen.leader(ox+el[0], oy+el[1], ox+ld.x2, oy+el[1], "muted")
    info_leaders[ld.text] = {x0: ox+st[0], y0: oy+st[1], x1: ox+el[0], y1: oy+el[1], x2: ox+ld.x2}
  }
  ring(rimLow, rimHigh, "accent")
  if (m.apart) ring(panLow, panHigh, "caution")

  var texts = []
  for (var k = 0; k < m.texts.length; k++) {
    var t = m.texts[k], drawn
    if (t.tail) drawn = pen.keyed([[m.titleLabel, "ink"], [t.tail, "muted"]], ox+t.x, oy+t.y)
    else drawn = pen.text(t.text, ox+t.x, oy+t.y, t.role, t.size)
    texts.push({text: t.text, x: ox+t.x, y: oy+t.y, w: t.w, h: t.h, role: t.role, drawn: drawn})
  }
  return {
    factor: m.F, block: b, R: m.R,
    centre: {x: ox+m.cx, y: oy+m.cy}, origin: {x: bx, y: by},
    rimRadius: m.rim2/2, panelRadius: m.pan2/2, rim2: m.rim2, pan2: m.pan2,
    disc: box, blocks: blocks, panelLeader: m.dir, pitchAbove: m.pitchAbove, tones: {high: ctx.star.high, low: ctx.star.low},
    inclusion: "a panel pixel is drawn wherever any of its virtual pixels is inside the rim; the rim cuts the blocks it crosses",
    panelRing: m.apart,
    gridLeader: m.gridDir, leader: info_leaders["PANEL LIMIT"] || null, rimLeader: info_leaders["GRID LIMIT"],
    title: m.title ? {x: ox+m.title.x, y: oy+m.title.y, w: m.title.w, h: m.title.h} : null,
    texts: texts, pitch: m.pitch, pitchX: ctx.physical.mmPerPixelX, pitchY: ctx.physical.mmPerPixelY,
    box: {x: x, y: y, w: m.w, h: m.h}
  }
}
