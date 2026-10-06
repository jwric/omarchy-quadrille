// The body face at 2x and 3x its size, redrawn at the surface's own pixel.
//
// A pixel font enlarged by repeating every pixel is a mixel: its stems are 2 or 3
// of the surface's pixels thick, beside text and icons of 1. So a larger size is
// drawn, not scaled: the glyph's bitmap (Glyphs.js) is expanded with Scale2x /
// Scale3x (EPX and AdvMAME3x), which keep the stems and smooth every diagonal and
// corner by the pixels the extra size allows, and what comes out is drawn one
// pixel to one pixel of the surface: a 12 x 24 or 18 x 36 cell whose diagonals
// are real diagonals.
.pragma library
.import "Glyphs.js" as Glyphs

var cache = {}

function bitmap(code) {
  var hex = Glyphs.glyphs[String(code)]
  if (hex === undefined) hex = Glyphs.missing
  var out = []
  for (var y = 0; y < Glyphs.rows; y++) {
    var bits = parseInt(hex.substr(y * 2, 2), 16)
    var row = []
    for (var x = 0; x < Glyphs.cols; x++) row.push((bits >> (Glyphs.cols - 1 - x)) & 1)
    out.push(row)
  }
  return out
}

function px(src, x, y) {
  if (y < 0 || y >= src.length || x < 0 || x >= src[0].length) return 0
  return src[y][x]
}

// Scale2x: each pixel becomes four, a corner taking its neighbour's value where
// two neighbours agree and the others do not.
function scale2(src) {
  var h = src.length, w = src[0].length, out = []
  for (var y = 0; y < h * 2; y++) out.push(new Array(w * 2))
  for (y = 0; y < h; y++) for (var x = 0; x < w; x++) {
    var P = px(src, x, y), A = px(src, x, y - 1), B = px(src, x + 1, y), C = px(src, x - 1, y), D = px(src, x, y + 1)
    out[2 * y][2 * x]         = (C === A && C !== D && A !== B) ? A : P
    out[2 * y][2 * x + 1]     = (A === B && A !== C && B !== D) ? B : P
    out[2 * y + 1][2 * x]     = (D === C && D !== B && C !== A) ? C : P
    out[2 * y + 1][2 * x + 1] = (B === D && B !== A && D !== C) ? D : P
  }
  return out
}

// Scale3x (AdvMAME3x).
function scale3(src) {
  var h = src.length, w = src[0].length, out = []
  for (var y = 0; y < h * 3; y++) out.push(new Array(w * 3))
  for (y = 0; y < h; y++) for (var x = 0; x < w; x++) {
    var A = px(src, x - 1, y - 1), B = px(src, x, y - 1), C = px(src, x + 1, y - 1)
    var D = px(src, x - 1, y),     E = px(src, x, y),     F = px(src, x + 1, y)
    var G = px(src, x - 1, y + 1), H = px(src, x, y + 1), I = px(src, x + 1, y + 1)
    var e = [E, E, E, E, E, E, E, E, E]
    if (B !== H && D !== F) {
      e[0] = (D === B) ? D : E
      e[1] = ((D === B && E !== C) || (B === F && E !== A)) ? B : E
      e[2] = (B === F) ? F : E
      e[3] = ((D === B && E !== G) || (D === H && E !== A)) ? D : E
      e[5] = ((B === F && E !== I) || (H === F && E !== C)) ? F : E
      e[6] = (D === H) ? D : E
      e[7] = ((D === H && E !== I) || (H === F && E !== G)) ? H : E
      e[8] = (H === F) ? F : E
    }
    for (var j = 0; j < 3; j++) for (var i = 0; i < 3; i++) out[3 * y + j][3 * x + i] = e[3 * j + i]
  }
  return out
}

function repeat(src, n) {
  var out = []
  for (var y = 0; y < src.length; y++) {
    var row = []
    for (var x = 0; x < src[y].length; x++) for (var i = 0; i < n; i++) row.push(src[y][x])
    for (var j = 0; j < n; j++) out.push(row)
  }
  return out
}

// The lit runs of one enlarged glyph as rectangles: [{x, y, w, h}], runs that
// repeat down consecutive rows merged into one.
function glyphRects(code, n) {
  var key = n + ":" + code
  if (cache[key]) return cache[key]
  var src = bitmap(code)
  var big = n === 2 ? scale2(src) : (n === 3 ? scale3(src) : repeat(src, n))
  var rects = [], open = {}
  for (var y = 0; y <= big.length; y++) {
    var row = y < big.length ? big[y] : []
    var runs = []
    var start = -1
    for (var x = 0; x <= row.length; x++) {
      var on = x < row.length && row[x] === 1
      if (on && start < 0) start = x
      else if (!on && start >= 0) { runs.push([start, x - start]); start = -1 }
    }
    var next = {}
    for (var r = 0; r < runs.length; r++) {
      var k = runs[r][0] + ":" + runs[r][1]
      if (open[k]) { open[k].h += 1; next[k] = open[k] }
      else { var rc = { x: runs[r][0], y: y, w: runs[r][1], h: 1 }; rects.push(rc); next[k] = rc }
    }
    open = next
  }
  cache[key] = rects
  return rects
}

// All of `text` at size n (2 or 3): rectangles in surface pixels, cells 6 * n wide.
function rects(text, n) {
  var chars = Array.from(String(text))
  var out = []
  for (var i = 0; i < chars.length; i++) {
    var g = glyphRects(chars[i].codePointAt(0), n)
    for (var j = 0; j < g.length; j++) out.push({ x: g[j].x + i * Glyphs.cell * n, y: g[j].y, w: g[j].w, h: g[j].h })
  }
  return out
}
