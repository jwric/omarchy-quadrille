// The body face at 2x and 3x its size, for the hero readouts.
//
// For now: the small face's own bitmap with every pixel repeated n x n, square
// pixels, hard corners, no interpolation of any kind. (The Scale2x / Scale3x
// expansion this file used before rounded every corner and diagonal, and was taken
// out on purpose: nothing here may smooth.) That is a mixel, a pixel of another
// size beside the 1x ones: it stands in until the native large face drawn by hand
// (plugins/tools/hero_face.py) replaces it. Do not bring a smoothing scale back.
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
  var big = repeat(src, n)
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
