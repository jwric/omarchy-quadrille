.pragma library

// The integer raster every part of the Aperture sheet draws with: rectangles
// of whole virtual pixels in theme roles, nothing blended. A pen also keeps the
// sheet's collision record: labels, targets (drawings) and leaders, so a label
// is placed only where it is wholly inside the content box and clear of the
// rest ("drop, don't cut").
//
//   var pen = Pen.make(width, height, glyphs, big, {L, T, R, B})
//
// width/height: the sheet in virtual pixels; glyphs: Q/Glyphs.js; big:
// Q/GlyphsBig.js (2x and 3x drawn faces); the content box is inclusive of L
// and T, and a label may end one pixel past R (a glyph's spare column).

function overlaps(a, b, gap) {
  gap = gap || 0
  return a.x < b.x+b.w+gap && b.x < a.x+a.w+gap &&
    a.y < b.y+b.h+gap && b.y < a.y+a.h+gap
}

function make(w, h, glyphs, big, box) {
  var pen = {w: w, h: h, glyphs: glyphs, big: big, box: box,
    strokes: [], labels: [], targets: [], leaders: [], dropped: []}

  pen.rect = function(x, y, width, height, role) {
    x = Math.round(x); y = Math.round(y); width = Math.round(width); height = Math.round(height)
    var right = Math.min(w, x+width), bottom = Math.min(h, y+height)
    x = Math.max(0, x); y = Math.max(0, y)
    if (right > x && bottom > y) pen.strokes.push({x: x, y: y, w: right-x, h: bottom-y, role: role})
  }
  pen.px = function(x, y, role) { pen.rect(x, y, 1, 1, role) }
  pen.outline = function(x, y, width, height, role) {
    pen.rect(x, y, width, 1, role); pen.rect(x, y+height-1, width, 1, role)
    pen.rect(x, y, 1, height, role); pen.rect(x+width-1, y, 1, height, role)
  }
  // A dashed or solid line one pixel thick; dash = [on, off], counted from the
  // first end given (the end that must look right).
  pen.hline = function(x0, x1, y, role, dash) {
    var step = x1 >= x0 ? 1 : -1, n = Math.abs(x1-x0)
    for (var i = 0; i <= n; i++) if (!dash || i % (dash[0]+dash[1]) < dash[0]) pen.px(x0+step*i, y, role)
  }
  pen.vline = function(x, y0, y1, role, dash) {
    var step = y1 >= y0 ? 1 : -1, n = Math.abs(y1-y0)
    for (var i = 0; i <= n; i++) if (!dash || i % (dash[0]+dash[1]) < dash[0]) pen.px(x, y0+step*i, role)
  }
  // A solid 45 degree arrowhead three pixels deep, its tip at (x, y), pointing
  // "l", "r", "u" or "d".
  pen.arrow = function(x, y, dir, role) {
    for (var k = 0; k < 3; k++) {
      if (dir === "l") pen.rect(x+k, y-k, 1, 2*k+1, role)
      else if (dir === "r") pen.rect(x-k, y-k, 1, 2*k+1, role)
      else if (dir === "u") pen.rect(x-k, y+k, 2*k+1, 1, role)
      else pen.rect(x-k, y-k, 2*k+1, 1, role)
    }
  }

  pen.span = function(value, n) { return glyphs.length(String(value))*6*(n||1)+1 }
  // Is a box clear of every label, target and leader (by gap) and inside the content box?
  pen.free = function(b, gap) {
    gap = gap === undefined ? 3 : gap
    if (b.x < box.L || b.y < box.T || b.x+b.w > box.R+1 || b.y+b.h > box.B) return false
    for (var i = 0; i < pen.labels.length; i++) if (overlaps(b, pen.labels[i], gap)) return false
    for (var i = 0; i < pen.targets.length; i++) if (overlaps(b, pen.targets[i], gap)) return false
    for (var i = 0; i < pen.leaders.length; i++) if (overlaps(b, pen.leaders[i], gap)) return false
    return true
  }
  pen.target = function(t) { pen.targets.push(t); return t }
  // Text is a list of [words, role] parts on one line.
  pen.keyed = function(parts, x, y, n) {
    n = n || 1
    var value = parts.map(function(p) { return p[0] }).join("")
    var b = {text: value, x: Math.round(x), y: Math.round(y), w: pen.span(value, n), h: 12*n}
    var free = b.x >= box.L && b.y >= box.T && b.x+b.w <= box.R+1 && b.y+b.h <= box.B
    for (var i = 0; free && i < pen.labels.length; i++) if (overlaps(b, pen.labels[i], 3)) free = false
    for (var i = 0; free && i < pen.targets.length; i++) if (overlaps(b, pen.targets[i], 3)) free = false
    for (var i = 0; free && i < pen.leaders.length; i++) if (overlaps(b, pen.leaders[i], 3)) free = false
    if (!free) { pen.dropped.push(value); return false }
    pen.labels.push(b)
    var at = 0
    for (var p = 0; p < parts.length; p++) {
      var runs = n === 1 ? glyphs.runs(parts[p][0]) : big.rects(parts[p][0], n)
      for (var i = 0; i < runs.length; i++)
        pen.rect(b.x+at*6*n+runs[i].x, b.y+runs[i].y, runs[i].w, runs[i].h || 1, parts[p][1])
      at += glyphs.length(parts[p][0])
    }
    return true
  }
  pen.text = function(value, x, y, role, n) { return pen.keyed([[String(value), role || "ink"]], x, y, n) }
  pen.textRight = function(value, x, y, role, n) { return pen.text(value, x-pen.span(value, n)+1, y, role, n) }
  pen.keyedRight = function(parts, x, y) {
    return pen.keyed(parts, x-pen.span(parts.map(function(p) { return p[0] }).join(""))+1, y)
  }
  pen.centred = function(parts, cx, y) {
    return pen.keyed(parts, cx-Math.floor(pen.span(parts.map(function(p) { return p[0] }).join(""))/2), y)
  }
  // Emphasis: text knocked out of an accent block, one pixel of block above the
  // capitals and two to each side. The box returned is the block's.
  pen.inverse = function(value, x, y) {
    var b = {text: value, x: Math.round(x), y: Math.round(y), w: pen.span(value)+4, h: 12}
    if (!pen.free(b)) { pen.dropped.push(value); return null }
    pen.labels.push(b)
    pen.rect(b.x, b.y, b.w, b.h, "accent")
    var runs = glyphs.runs(value)
    for (var i = 0; i < runs.length; i++) pen.rect(b.x+2+runs[i].x, b.y+runs[i].y, runs[i].w, 1, "on_accent")
    return b
  }
  pen.cross = function(x, y, size, role) { pen.rect(x-size, y, 2*size+1, 1, role); pen.rect(x, y-size, 1, 2*size+1, role) }
  // Every pixel of a box from fn(x, y) -> role or null, as horizontal runs.
  pen.raster = function(b, pixel) {
    for (var y = b.y; y < b.y+b.h; y++) {
      var start = b.x, last = pixel(start, y)
      for (var x = start+1; x <= b.x+b.w; x++) {
        var role = x < b.x+b.w ? pixel(x, y) : null
        if (role !== last) { if (last) pen.rect(start, y, x-start, 1, last); start = x; last = role }
      }
    }
  }
  // A one-pixel ring of radius r round a centre that may sit on a half pixel.
  pen.ring = function(x, y, r, role, clip) {
    var b = clip || {x: Math.floor(x-r)-1, y: Math.floor(y-r)-1, w: Math.ceil(2*r)+4, h: Math.ceil(2*r)+4}
    pen.raster(b, function(px, py) {
      var d = Math.sqrt((px-x)*(px-x)+(py-y)*(py-y))
      return d <= r+0.5 && d > r-0.5 ? role : null
    })
  }
  // A one-pixel ring the way a pixel artist draws it: the pixels within half a
  // pixel of the radius, so the sides run flat and no lone pixel juts out at
  // the four compass points. Integer arithmetic when the pixels are square.
  pen.circle = function(x, y, rx, ry, role) {
    rx = Math.max(1, Math.round(rx)); ry = Math.max(1, Math.round(ry))
    pen.raster({x: x-rx-1, y: y-ry-1, w: 2*rx+3, h: 2*ry+3}, function(px, py) {
      var dx = px-x, dy = py-y
      if (rx === ry) { var d = dx*dx+dy*dy; return d > rx*rx-rx && d <= rx*rx+rx ? role : null }
      var e = Math.sqrt(dx*dx/(rx*rx)+dy*dy/(ry*ry))-1, m = Math.min(rx, ry)
      return e*m > -0.5 && e*m <= 0.5 ? role : null
    })
  }
  // Leaders run only horizontally, vertically or at 45 degrees, one pixel a step.
  pen.leader = function(x0, y0, x1, y1, role) {
    var n = Math.max(Math.abs(x1-x0), Math.abs(y1-y0)), dx = Math.sign(x1-x0), dy = Math.sign(y1-y0)
    for (var i = 0; i <= n; i++) pen.rect(x0+dx*i, y0+dy*i, 1, 1, role)
    // Collision boxes cover exactly the leader's own pixels, four steps a box.
    for (var i = 0; i <= n; i += 4) {
      var k = Math.min(4, n-i), xa = x0+dx*i, ya = y0+dy*i, xb = xa+dx*k, yb = ya+dy*k
      pen.leaders.push({x: Math.min(xa, xb), y: Math.min(ya, yb), w: Math.abs(xb-xa)+1, h: Math.abs(yb-ya)+1})
    }
  }
  return pen
}

// The 4 x 4 Bayer matrix: a level k of 16 lights the cells whose entry is < k.
var bayer = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
function dither(x, y, level) { return bayer[((y%4)+4)%4][((x%4)+4)%4] < level }
