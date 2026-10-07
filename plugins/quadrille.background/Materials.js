.pragma library
.import "Pen.js" as Pen

// The material specimen: the theme's roles shown the way they are used, in three
// plainly named groups.
//
//   SURFACES  void ground raised hover   the grounds things sit on
//   INKS      edge line faint muted ink  what marks are made with
//   SIGNALS   accent live caution alarm  small chips, so bright colours are deliberate
//
// A surface is a flat fill over its Bayer treatment (a stepped ramp toward the next
// surface), framed by one pixel of `edge` outside it. An ink is a flat fill over the
// ink used as a one-bit texture on `void`: grid, hatch, dots, stepped shading. A
// signal is a flat chip. Tones come only from role choice and ordered dither; every
// pixel is exactly one role.
//
//   measure(ctx) -> {w, h} | null
//   draw(pen, x, y, ctx) -> info
//
// cfg (fields of ctx): compact (false), layout "row" (default) | "stack",
// groups (default all three, in order; names are case-insensitive), base (true:
// in a row the signal chips stand on the specimens' baseline, so every role name
// sits on one line; false hangs them from the group name instead).

var GAP = 8          // between specimens (beyond the wider of specimen and its name)
var GROUP_GAP = 20   // between groups
var NAME_GAP = 4     // group name to the specimens' frame, and role name to it
var TITLE_ROLE = "ink"
var LABEL_ROLE = "muted"
var FRAME_ROLE = "edge"
var HALF = 12        // flat half, texture half

var GROUPS = {
  surfaces: {title: "SURFACES", roles: ["void", "ground", "raised", "hover"],
    next: {void: "ground", ground: "raised", raised: "hover", hover: "edge"},
    w: 36, wc: 28, h: 24, hc: 24, frame: true},
  inks: {title: "INKS", roles: ["edge", "line", "faint", "muted", "ink"],
    kinds: {edge: "grid", line: "hatch", faint: "dots", muted: "bayer", ink: "bayer"},
    levels: {muted: 4, ink: 8},
    w: 24, wc: 18, h: 24, hc: 24, frame: false},
  signals: {title: "SIGNALS", roles: ["accent", "live", "caution", "alarm"],
    w: 10, wc: 8, h: 10, hc: 8, frame: false}
}
var ORDER = ["surfaces", "inks", "signals"]
var LEVELS = [4, 8, 12]   // the ramp's three bands, of 16

function span(pen, text) { return pen ? pen.span(text) : text.length*6+1 }

// The pixel of an ink's texture, i and j counted from the specimen's top-left.
function inkPattern(kind, i, j, level) {
  if (kind === "grid") return i%4 === 0 || j%4 === 0
  if (kind === "hatch") return (i+j)%4 === 0
  if (kind === "dots") return i%3 === 0 && j%3 === 0
  return Pen.dither(i, j, level)
}

// The ramp band (0, 1, 2) of column i of a specimen w wide: three equal bands.
function band(i, w) { return Math.min(2, Math.floor(3*i/w)) }

// Everything's place, relative to the strip's top-left. A pen is only needed for
// the width of text (a glyph cell is six wide and one spare column).
function plan(ctx, pen) {
  var compact = !!ctx.compact, stack = ctx.layout === "stack"
  var keys = (ctx.groups || ORDER).map(function(g) { return String(g).toLowerCase() })
    .filter(function(g) { return GROUPS[g] })
  if (!keys.length) return null
  var groups = [], cursor = 0, widest = 0
  var tallest = 0
  keys.forEach(function(key) { tallest = Math.max(tallest, compact ? GROUPS[key].hc : GROUPS[key].h) })
  keys.forEach(function(key) {
    var d = GROUPS[key], w = compact ? d.wc : d.w, h = compact ? d.hc : d.h
    var outer = w+(d.frame ? 2 : 0), cells = [], at = 0
    d.roles.forEach(function(role, n) {
      var cell = Math.max(outer, span(pen, role))
      cells.push({role: role, x: at, w: cell, bx: at+(d.frame ? 1 : 0), sw: w, sh: h})
      at += cell+GAP
    })
    var gw = at-GAP
    // body top: title row (12), a gap, one pixel for the frame
    var top = 12+NAME_GAP+1
    if (key === "signals" && ctx.base !== false && !stack) top += tallest-h
    var g = {key: key, title: d.title, def: d, cells: cells, w: gw, h: h, top: top,
      height: top+h+1+NAME_GAP+12}
    groups.push(g)
    widest = Math.max(widest, gw)
  })
  var height = 0
  groups.forEach(function(g, n) {
    if (stack) { g.x = 0; g.y = height; height += g.height+(n < groups.length-1 ? GROUP_GAP : 0) }
    else { g.x = cursor; g.y = 0; cursor += g.w+GROUP_GAP; height = Math.max(height, g.height) }
  })
  return {groups: groups, w: stack ? widest : cursor-GROUP_GAP, h: height}
}

function measure(ctx) {
  var p = plan(ctx, null)
  return p ? {w: p.w, h: p.h, layout: ctx.layout === "stack" ? "stack" : "row", compact: !!ctx.compact} : null
}

function draw(pen, x, y, ctx) {
  var p = plan(ctx, pen)
  var info = {groups: [], specimens: []}
  if (!p) return info
  x = Math.round(x); y = Math.round(y)
  info.w = p.w; info.h = p.h
  info.layout = ctx.layout === "stack" ? "stack" : "row"; info.compact = !!ctx.compact
  var specimens = []
  p.groups.forEach(function(g) {
    var d = g.def, gx = x+g.x, gy = y+g.y
    g.cells.forEach(function(c) {
      var bx = gx+c.bx, by = gy+g.top, w = c.sw, h = c.sh, role = c.role
      var s = {group: g.key, role: role, x: bx, y: by, w: w, h: h,
        flat: {x: bx, y: by, w: w, h: HALF}, texture: null,
        label: {x: gx+c.x, y: by+h+1+NAME_GAP}, cellX: gx+c.x, cellW: c.w}
      if (g.key === "signals") {
        pen.rect(bx, by, w, h, role)
        s.flat = {x: bx, y: by, w: w, h: h}
        pen.target({x: bx, y: by, w: w, h: h, kind: "material"})
      } else {
        var th = h-HALF, ty = by+HALF
        pen.rect(bx, by, w, HALF, role)
        if (d.frame) {
          pen.outline(bx-1, by-1, w+2, h+2, FRAME_ROLE)
          s.frame = {x: bx-1, y: by-1, w: w+2, h: h+2}
          var to = d.next[role]
          s.texture = {x: bx, y: ty, w: w, h: th, kind: "ramp", from: role, to: to, levels: LEVELS.slice()}
          pen.raster(s.texture, function(px, py) {
            return Pen.dither(px-bx, py-ty, LEVELS[band(px-bx, w)]) ? to : role
          })
        } else {
          var kind = d.kinds[role], level = d.levels[role]
          s.texture = {x: bx, y: ty, w: w, h: th, kind: kind, from: "void", to: role}
          if (level !== undefined) s.texture.level = level
          pen.raster(s.texture, function(px, py) {
            return inkPattern(kind, px-bx, py-ty, level) ? role : "void"
          })
        }
        var f = s.frame || {x: bx, y: by, w: w, h: h}
        pen.target({x: f.x, y: f.y, w: f.w, h: f.h, kind: "material"})
      }
      specimens.push(s)
    })
  })
  // Names last, so every target is on record before a label asks for room.
  p.groups.forEach(function(g) {
    var gx = x+g.x, gy = y+g.y
    var ok = pen.text(g.title, gx, gy, TITLE_ROLE)
    info.groups.push({name: g.title, x: gx, y: gy, w: pen.span(g.title), h: 12, drawn: ok})
  })
  specimens.forEach(function(s) {
    s.label.text = s.role
    s.label.w = pen.span(s.role); s.label.h = 12
    s.label.drawn = pen.text(s.role, s.label.x, s.label.y, LABEL_ROLE)
    info.specimens.push(s)
  })
  return info
}
