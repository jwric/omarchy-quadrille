.pragma library
.import "quadrille.bar/Q/Glyphs.js" as Glyphs

// The lit pixels of a line of text, in virtual pixels, as rectangles with the runs that repeat down consecutive rows
// merged (the same pixels as Glyphs.runs, in fewer rectangles), and as one SVG path. A glyph is worked out once and
// kept, so a line is a concatenation, not a pass over bits.
var glyphs = {}

function glyph(code) {
  var hit = glyphs[code]
  if (hit) return hit
  var hex = Glyphs.glyphs[String(code)]
  if (hex === undefined) hex = Glyphs.missing
  var out = [], open = {}, next = {}
  for (var y = 0; y <= Glyphs.rows; y++) {
    open = next; next = {}
    if (y === Glyphs.rows) break
    var bits = parseInt(hex.substr(y * 2, 2), 16)
    var x = 0
    while (x < Glyphs.cols) {
      if (!((bits >> (Glyphs.cols - 1 - x)) & 1)) { x++; continue }
      var start = x
      while (x < Glyphs.cols && ((bits >> (Glyphs.cols - 1 - x)) & 1)) x++
      var key = start + ":" + (x - start)
      var up = open[key]
      if (up) { up.h += 1; next[key] = up }
      else { var rc = { x: start, y: y, w: x - start, h: 1 }; out.push(rc); next[key] = rc }
    }
  }
  // a relative path: the first rectangle's corner, then each next corner from the one before it
  var d = "", px = 0, py = 0
  for (var i = 0; i < out.length; i++) {
    var r = out[i]
    d += (i === 0 ? "M" + r.x + " " + r.y : "m" + (r.x - px) + " " + (r.y - py)) + "h" + r.w + "v" + r.h + "h" + (-r.w) + "z"
    px = r.x; py = r.y
  }
  hit = { rects: out, path: d, first: out.length ? out[0] : null, last: out.length ? out[out.length - 1] : null }
  glyphs[code] = hit
  return hit
}

function rects(text) {
  var chars = Array.from(String(text)), out = []
  for (var i = 0; i < chars.length; i++) {
    var g = glyph(chars[i].codePointAt(0)).rects, off = i * Glyphs.cell
    for (var j = 0; j < g.length; j++) out.push({ x: g[j].x + off, y: g[j].y, w: g[j].w, h: g[j].h })
  }
  return out
}

// One path for the line: each glyph's own relative path, joined by a move from the last corner of the one before to
// the first corner of the next (relative moves, and "z" leaves the point at the corner it started from).
function svgPath(text) {
  var chars = Array.from(String(text)), s = "", lx = 0, ly = 0, started = false
  for (var i = 0; i < chars.length; i++) {
    var g = glyph(chars[i].codePointAt(0))
    if (!g.first) continue
    var fx = g.first.x + i * Glyphs.cell, fy = g.first.y
    s += started ? "m" + (fx - lx) + " " + (fy - ly) : "M" + fx + " " + fy
    s += g.path.substring(g.path.indexOf("h"))
    lx = g.last.x + i * Glyphs.cell; ly = g.last.y
    started = true
  }
  return s
}
