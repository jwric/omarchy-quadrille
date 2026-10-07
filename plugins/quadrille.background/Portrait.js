.pragma library
.import "Pen.js" as Pen

// The two small drawings that identify a display on the Aperture sheet, one
// pixel line each, much quieter than the star:
//
//   LOCATOR    a miniature of every output in the compositor's layout, this
//              one highlighted: where the display sits in the desktop.
//   ELEVATION  a dimensioned front elevation of the active area at its true
//              aspect ratio, drawn to a true, stated scale on the screen
//              itself: what the display physically is.
//
// Both are measure/draw pairs (measureLocator/drawLocator, measureElevation/
// drawElevation); measure/draw at the bottom dispatch on ctx.element for the
// element tools. All coordinates are whole virtual pixels; a drawing keeps
// inside the box measure() returns, registers itself as one pen target, and
// never lets a label be dropped (a drawing that cannot place its labels is not
// drawn at all: measure returns null).

// ---------------------------------------------------------------- locator

// Every output's logical rectangle in the compositor's layout, and which one
// this is. Pixel size over scale, swapped for the odd transforms.
function outputs(ctx) {
  var list = ctx.monitors && ctx.monitors.length ? ctx.monitors : [{input: ctx.input}]
  var current = ctx.monitors && ctx.monitors.length ? ctx.current : 0
  var rects = []
  for (var i = 0; i < list.length; i++) {
    var m = list[i].input
    var scale = typeof m.scale === "number" && isFinite(m.scale) && m.scale > 0 ? m.scale : 1
    var w = Math.max(1, Number(m.width) || 1) / scale, h = Math.max(1, Number(m.height) || 1) / scale
    if ((m.transform || 0) % 2 !== 0) { var t = w; w = h; h = t }
    rects.push({name: m.name || "", index: i, current: i === current, x: Number(m.x) || 0, y: Number(m.y) || 0, w: w, h: h})
  }
  return rects
}

// Geometry of the locator in its own box: one factor k for the whole drawing,
// every edge rounded to a whole vpx, every outline drawn one vpx inside its
// rect (so touching outputs stay two vpx apart), at least 3 x 3.
function layoutLocator(ctx, pen, maxW, maxH) {
  var rects = outputs(ctx)
  var ux = Infinity, uy = Infinity, ex = -Infinity, ey = -Infinity
  for (var i = 0; i < rects.length; i++) {
    ux = Math.min(ux, rects[i].x); uy = Math.min(uy, rects[i].y)
    ex = Math.max(ex, rects[i].x + rects[i].w); ey = Math.max(ey, rects[i].y + rects[i].h)
  }
  var UW = ex - ux, UH = ey - uy
  if (!(maxW >= 3 && maxH >= 3 && UW > 0 && UH > 0)) return null
  var k = Math.min(maxW / UW, maxH / UH)
  var W = Math.round(UW * k), H = Math.round(UH * k)
  if (W < 3 || H < 3) return null
  var boxes = []
  for (var i = 0; i < rects.length; i++) {
    var r = rects[i]
    var X0 = Math.round((r.x - ux) * k), X1 = Math.round((r.x + r.w - ux) * k)
    var Y0 = Math.round((r.y - uy) * k), Y1 = Math.round((r.y + r.h - uy) * k)
    var ow = Math.max(3, X1 - X0 - 2), oh = Math.max(3, Y1 - Y0 - 2)
    var ox = Math.min(X0 + 1, W - ow), oy = Math.min(Y0 + 1, H - oh)
    boxes.push({name: r.name, index: r.index, current: r.current, x: ox, y: oy, w: ow, h: oh})
  }
  var caption = ctx.caption ? "DESKTOP LAYOUT" : null
  var width = caption ? Math.max(W, pen.span(caption)) : W
  var height = caption ? H + 4 + 12 : H
  return {k: k, UW: UW, UH: UH, W: W, H: H, boxes: boxes, caption: caption, w: width, h: height}
}

// A number is drawn only when its outline's inside leaves 2 vpx of air round
// the label box; its box is then centred in the outline.
function numberBox(pen, b) {
  var text = String(b.index + 1), lw = pen.span(text)
  var iw = b.w - 2, ih = b.h - 2
  if (iw < lw + 4 || ih < 12 + 4) return null
  return {text: text, x: b.x + 1 + Math.floor((iw - lw) / 2), y: b.y + 1 + Math.floor((ih - 12) / 2), w: lw, h: 12}
}

function measureLocator(ctx, maxW, maxH) {
  // measure needs only span(), which is the glyph table's: any pen will do, so
  // a stand-in with the same rule is used when none is at hand.
  var l = layoutLocator(ctx, {span: function(v) { return String(v).length * 6 + 1 }}, maxW, maxH)
  return l ? {w: l.w, h: l.h, k: l.k} : null
}

function drawLocator(pen, x, y, ctx, maxW, maxH) {
  var l = layoutLocator(ctx, pen, maxW, maxH)
  if (!l) return null
  var current = null, others = []
  for (var i = 0; i < l.boxes.length; i++) (l.boxes[i].current ? (current = l.boxes[i]) : others.push(l.boxes[i]))
  // The current output is drawn last, on top where outputs overlap (mirrored).
  var mine = current ? numberBox(pen, current) : null
  if (mine) { mine.x += x; mine.y += y }
  for (var i = 0; i < others.length; i++) pen.outline(x + others[i].x, y + others[i].y, others[i].w, others[i].h, "faint")
  var numbers = []
  for (var i = 0; i < others.length; i++) {
    var n = numberBox(pen, others[i])
    if (!n) continue
    n.x += x; n.y += y
    // A faint number under the highlighted output is simply left out.
    if (current && (Pen.overlaps(n, {x: x + current.x, y: y + current.y, w: current.w, h: current.h}, 1) ||
        (mine && Pen.overlaps(n, mine, 3)))) continue
    if (pen.text(n.text, n.x, n.y, "faint")) numbers.push({index: others[i].index, value: others[i].index + 1, x: n.x, y: n.y})
  }
  if (current) {
    pen.outline(x + current.x, y + current.y, current.w, current.h, "accent")
    if (mine && pen.text(mine.text, mine.x, mine.y, "accent")) numbers.push({index: current.index, value: current.index + 1, x: mine.x, y: mine.y})
  }
  pen.target({x: x, y: y, w: l.W, h: l.H, kind: "locator"})
  var caption = null
  if (l.caption && pen.text(l.caption, x, y + l.H + 4, "muted")) caption = {text: l.caption, x: x, y: y + l.H + 4, w: pen.span(l.caption), h: 12}
  var rects = l.boxes.map(function(b) {
    return {name: b.name, index: b.index, current: b.current, x: x + b.x, y: y + b.y, w: b.w, h: b.h}
  })
  return {k: l.k, union: {w: l.W, h: l.H}, box: {x: x, y: y, w: l.w, h: l.h}, rects: rects, numbers: numbers, caption: caption}
}

// -------------------------------------------------------------- elevation

var scales = [2, 3, 4, 5, 8, 10, 15, 20, 25, 30, 40, 50]

function formatMm(physical, est, mm) {
  if (physical.source === "override") return mm.toFixed(1) + " mm"
  return est + Math.round(mm) + " mm"
}

// Everything at scale 1:N in the element's own box (origin top-left); the
// outline's top-left is (ox, oy). Rows from the top: title (12), 4 air, the
// outline, 8 to the dimension line, its value 4 under the line (5 from the
// row), then ESTIMATED when the size is inferred.
function layoutElevation(ctx, pen, N) {
  var p = ctx.physical
  var Wr = Math.round(p.widthMm / N / p.mmPerVpxX), Hr = Math.round(p.heightMm / N / p.mmPerVpxY)
  var est = ctx.est || (p.estimated ? "~" : "")
  var widthText = formatMm(p, est, p.widthMm), heightText = formatMm(p, est, p.heightMm)
  var estimated = p.source !== "override"
  var titleParts = [["ACTIVE AREA", "ink"], ["  1:" + N, "muted"]]
  var title = titleParts[0][0] + titleParts[1][0]
  var oy = 16
  var D = oy + Hr + 8, Dx = Wr + 8                        // dimension line row / column (ox = 0)
  var wTextW = pen.span(widthText), hTextW = pen.span(heightText)
  var wTextX = Math.floor((Wr + 1 - wTextW) / 2), wTextY = D + 5
  var hTextX = Dx + 7, hTextY = oy + Math.floor((Hr + 1 - 12) / 2)
  var estY = wTextY + 12 + 4
  var left = Math.min(0, wTextX)
  var right = Math.max(Wr + 1, pen.span(title), wTextX + wTextW, Dx + 4, hTextX + hTextW, estimated ? pen.span("ESTIMATED") : 0)
  var bottom = Math.max(D + 4, wTextY + 12, hTextY + 12, estimated ? estY + 12 : 0)
  var shift = -left
  return {N: N, Wr: Wr, Hr: Hr, oy: oy, D: D, Dx: Dx, shift: shift, w: right - left, h: bottom,
    widthText: widthText, heightText: heightText, estimated: estimated, titleParts: titleParts,
    wTextX: wTextX, wTextY: wTextY, hTextX: hTextX, hTextY: hTextY, estY: estY}
}

function chooseElevation(ctx, pen, maxW, maxH) {
  if (!ctx.physical || !(ctx.physical.mmPerVpxX > 0) || !(ctx.physical.mmPerVpxY > 0)) return null
  for (var i = 0; i < scales.length; i++) {
    var l = layoutElevation(ctx, pen, scales[i])
    if (l.w <= maxW && l.h <= maxH) return l.Wr >= 24 && l.Hr >= 12 ? l : null
  }
  return null
}

function measureElevation(ctx, maxW, maxH) {
  var l = chooseElevation(ctx, {span: function(v) { return String(v).length * 6 + 1 }}, maxW, maxH)
  return l ? {w: l.w, h: l.h, N: l.N} : null
}

function drawElevation(pen, x, y, ctx, maxW, maxH) {
  var l = chooseElevation(ctx, pen, maxW, maxH)
  if (!l) return null
  var p = ctx.physical
  var x0 = x + l.shift, y0 = y + l.oy, Wr = l.Wr, Hr = l.Hr
  var D = y + l.D, Dx = x0 + l.Dx
  // The active area.
  pen.outline(x0, y0, Wr + 1, Hr + 1, "muted")
  // The sheet's main drawing at the same scale, where the screen centres it.
  var star = null
  var d = ctx.star && ctx.star.diameter
  if (d > 0) {
    var rx = Math.round(d / l.N / 2 / p.mmPerVpxX), ry = Math.round(d / l.N / 2 / p.mmPerVpxY)
    var cx = x0 + Math.round(Wr / 2), cy = y0 + Math.round(Hr / 2)
    if (rx >= 3 && ry >= 1 && cx - rx >= x0 + 2 && cx + rx <= x0 + Wr - 2 && cy - ry >= y0 + 2 && cy + ry <= y0 + Hr - 2) {
      pen.circle(cx, cy, rx, ry, "line")
      star = {cx: cx, cy: cy, rx: rx, ry: ry}
    }
  }
  // Width: extension lines from 2 below the corners to 3 past the dimension
  // line, the line between solid arrowheads whose tips sit on the pixel just
  // inside each extension line (so a head reads as 1, 3, 5 pixels beside it).
  for (var e = 0; e < 2; e++) pen.vline(x0 + e * Wr, y0 + Hr + 2, D + 3, "line")
  pen.arrow(x0 + 1, D, "l", "line"); pen.arrow(x0 + Wr - 1, D, "r", "line")
  pen.hline(x0 + 4, x0 + Wr - 4, D, "line")
  // Height, the same turned.
  for (var e = 0; e < 2; e++) pen.hline(x0 + Wr + 2, Dx + 3, y0 + e * Hr, "line")
  pen.arrow(Dx, y0 + 1, "u", "line"); pen.arrow(Dx, y0 + Hr - 1, "d", "line")
  pen.vline(Dx, y0 + 4, y0 + Hr - 4, "line")
  // Words.
  var labels = []
  function say(parts, tx, ty) {
    var text = parts.map(function(q) { return q[0] }).join("")
    if (pen.keyed(parts, tx, ty)) labels.push({text: text, x: tx, y: ty, w: pen.span(text), h: 12})
  }
  say(l.titleParts, x0, y)
  say([[l.widthText, "muted"]], x + l.shift + l.wTextX, y + l.wTextY)
  say([[l.heightText, "muted"]], x0 + l.hTextX, y + l.hTextY)
  if (l.estimated) say([["ESTIMATED", "muted"]], x0, y + l.estY)
  // The drawing is one target: the outline and its dimension lines.
  var target = {x: x0, y: y0 - 0, w: l.Dx + 4, h: l.D + 4 - l.oy, kind: "elevation"}
  pen.target(target)
  return {N: l.N, scale: "1:" + l.N, rect: {x: x0, y: y0, w: Wr, h: Hr},
    widthText: l.widthText, heightText: l.heightText, estimated: l.estimated,
    widthMm: p.widthMm, heightMm: p.heightMm,
    onScreenWidthMm: Wr * p.mmPerVpxX, onScreenHeightMm: Hr * p.mmPerVpxY,
    star: star, box: {x: x, y: y, w: l.w, h: l.h}, target: target,
    dimension: {widthRow: D, heightColumn: Dx, extensionWidth: [x0, x0 + Wr], extensionHeight: [y0, y0 + Hr]},
    labels: labels}
}

// ------------------------------------------------- dispatch for the tools

function measure(ctx) {
  return ctx.element === "locator" ? measureLocator(ctx, ctx.maxW || 72, ctx.maxH || 40)
    : measureElevation(ctx, ctx.maxW || 160, ctx.maxH || 110)
}

function draw(pen, x, y, ctx) {
  return ctx.element === "locator" ? drawLocator(pen, x, y, ctx, ctx.maxW || 72, ctx.maxH || 40)
    : drawElevation(pen, x, y, ctx, ctx.maxW || 160, ctx.maxH || 110)
}
