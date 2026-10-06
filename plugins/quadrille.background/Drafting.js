.pragma library

// Integer virtual-pixel strokes. The Canvas and the headless checks consume the
// same plan; the font contributes its native bitmap runs, never scaled glyphs.
function mark(mm, pixels, pxPerMm, phys) {
  return Math.round((pixels / 2 + mm * pxPerMm) / phys)
}

function layout(monitors) {
  if (!monitors.length) return []
  var left = monitors.slice()
  left.sort(function(a, b) {
    return Math.hypot(a.input.x || 0, a.input.y || 0) - Math.hypot(b.input.x || 0, b.input.y || 0)
  })
  var first = left.shift()
  var placed = [{ monitor: first, x: 0, y: 0 }]
  function gap(a, b, axis) {
    var pos = axis === "x" ? "x" : "y"
    var size = axis === "x" ? "widthPx" : "heightPx"
    var ap = a.input[pos] || 0, bp = b.input[pos] || 0
    var aw = a.physical[size] / (a.input.scale || 1)
    var bw = b.physical[size] / (b.input.scale || 1)
    return Math.max(0, bp - ap - aw, ap - bp - bw)
  }
  function coordinate(a, b, origin, axis) {
    var size = axis === "x" ? "widthPx" : "heightPx"
    var mm = axis === "x" ? "widthMm" : "heightMm"
    var ratio = axis === "x" ? "mmPerLogicalPixelX" : "mmPerLogicalPixelY"
    var ap = a.input[axis] || 0, bp = b.input[axis] || 0
    var aw = a.physical[size] / (a.input.scale || 1)
    var bw = b.physical[size] / (b.input.scale || 1)
    if (bp >= ap + aw - 0.01) return origin + a.physical[mm] + (bp - ap - aw) * b.physical[ratio]
    if (bp + bw <= ap + 0.01) return origin - b.physical[mm] - (ap - bp - bw) * b.physical[ratio]
    return origin + (bp - ap) * b.physical[ratio]
  }
  while (left.length) {
    var best = null
    for (var i = 0; i < left.length; i++) {
      for (var j = 0; j < placed.length; j++) {
        var a = placed[j].monitor, b = left[i]
        var distance = Math.hypot(gap(a, b, "x"), gap(a, b, "y"))
        if (!best || distance < best.distance) best = { i: i, j: j, distance: distance }
      }
    }
    var near = placed[best.j], next = left.splice(best.i, 1)[0]
    placed.push({ monitor: next,
      x: coordinate(near.monitor, next, near.x, "x"),
      y: coordinate(near.monitor, next, near.y, "y") })
  }
  return placed
}

function plan(input, physical, monitors, glyphs, topInset) {
  var phys = physical.pixelsPerVpx
  var w = Math.ceil(physical.widthPx / phys), h = Math.ceil(physical.heightPx / phys)
  var strokes = [], labels = [], plates = []
  topInset = Math.max(0, Math.round(topInset || 0))
  function rect(x, y, width, height, role) {
    x = Math.round(x); y = Math.round(y); width = Math.round(width); height = Math.round(height)
    if (width > 0 && height > 0) strokes.push({ x: x, y: y, w: width, h: height, role: role })
  }
  function outline(x, y, width, height, role) {
    rect(x, y, width, 1, role); rect(x, y + height - 1, width, 1, role)
    rect(x, y, 1, height, role); rect(x + width - 1, y, 1, height, role)
  }
  function text(value, x, y, room, role, backed) {
    value = String(value)
    if (glyphs.length(value) * 6 + 1 > room) return false
    x = Math.round(x); y = Math.round(y)
    if (x < 0 || y < 0 || x + glyphs.length(value) * 6 + 1 > w || y + 12 > h) return false
    labels.push({ text: value, x: x, y: y, room: room })
    if (backed) rect(x - 1, y + 1, glyphs.length(value) * 6 + 3, 10, "void")
    var runs = glyphs.runs(value)
    for (var i = 0; i < runs.length; i++) rect(x + runs[i].x, y + runs[i].y, runs[i].w, 1, role || "ink")
    return true
  }
  var estimated = physical.estimated ? "~" : ""
  var axisX = [], axisY = []
  function ruler(axis, size, pixels, density, marks) {
    var last = -1000
    for (var mm = -Math.floor(size / 2); mm <= Math.floor(size / 2); mm++) {
      var at = mark(mm, pixels, density, phys)
      if (at < 0 || at >= (axis === "x" ? w : h)) continue
      var length = mm % 10 === 0 ? 8 : mm % 5 === 0 ? 4 : 2
      var role = mm % 10 === 0 ? "edge" : mm % 5 === 0 ? "line" : "faint"
      if (axis === "x") { rect(at, topInset, 1, length, role); rect(at, h - length, 1, length, role) }
      else { rect(0, at, length, 1, role); rect(w - length, at, length, 1, role) }
      if (mm % 10 !== 0) continue
      marks.push({ mm: mm, vpx: at })
      var mainRole = mm % 50 === 0 ? "line" : "edge"
      if (axis === "x") rect(at, 9, 1, h - 18, mainRole)
      else rect(9, at, w - 18, 1, mainRole)
      var label = estimated + (mm > 0 ? "+" : "") + (mm / 10)
      var width = glyphs.length(label) * 6 + 1
      if (axis === "x") {
        var x = at - Math.floor(width / 2)
        if (x < 48 || x + width > w - 48 || x <= last + 4) continue
        text(label, x, topInset + 10, width, "ink", true); text(label, x, h - 23, width, "ink", true)
        last = x + width
      } else {
        var y = at - 6
        if (y < topInset + 28 || y + 12 > h - 28 || y <= last + 4) continue
        text(label, 10, y, width, "ink", true); text(label, w - width - 10, y, width, "ink", true)
        last = y + 12
      }
    }
  }
  ruler("x", physical.widthMm, physical.widthPx, physical.pxPerMmX, axisX)
  ruler("y", physical.heightMm, physical.heightPx, physical.pxPerMmY, axisY)
  var cx = mark(0, physical.widthPx, physical.pxPerMmX, phys)
  var cy = mark(0, physical.heightPx, physical.pxPerMmY, phys)
  rect(cx - 13, cy, 10, 1, "accent"); rect(cx + 4, cy, 10, 1, "accent")
  rect(cx, cy - 13, 1, 10, "accent"); rect(cx, cy + 4, 1, 10, "accent")
  text("cm", 10, topInset + 10, 18, "ink")

  function plate(x, y, width, height, title) {
    rect(x, y, width, height, "void")
    outline(x, y, width, height, "edge")
    rect(x + 1, y + 17, width - 2, 1, "edge")
    text(title, x + 5, y + 3, width - 10, "ink")
    plates.push({ x: x, y: y, w: width, h: height })
  }
  var titleWidth = Math.min(250, w >= 740 ? Math.floor(w * 0.4) : w - 48)
  var makeModel = (String(input.make || "") + " " + String(input.model || "")).trim()
  var modelLines = glyphs.wrap(makeModel, Math.max(1, Math.floor((titleWidth - 10) / 6)), 100)
  var titleLines = modelLines.concat([
    estimated + physical.widthMm.toFixed(1) + " x " + estimated + physical.heightMm.toFixed(1) + " mm",
    "diagonal " + estimated + (physical.diagonalMm / 10).toFixed(1) + " cm",
    physical.widthPx + " x " + physical.heightPx + " / " + Number(input.refreshRate || 0).toFixed(1) + " Hz",
    estimated + physical.pxPerMmX.toFixed(1) + " px/mm",
    "1 vpx = " + estimated + physical.mmPerVpxX.toFixed(2) + " mm",
    "scale " + Number(input.scale || 1).toFixed(6)])
  if (physical.estimated) titleLines.push("est. / assumed density")
  var titleHeight = 27 + titleLines.length * 14
  var tx = w - titleWidth - 24, ty = h - titleHeight - 28
  if (titleWidth >= 166 && ty > 40) {
    plate(tx, ty, titleWidth, titleHeight, "DISPLAY / " + (input.name || "OUTPUT"))
    for (var row = 0; row < titleLines.length; row++) text(titleLines[row].trim(), tx + 5, ty + 22 + row * 14, titleWidth - 10)
  }

  var placed = layout(monitors)
  if (placed.length && w >= 300 && h >= 220) {
    var minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity, smallest = Infinity
    for (var k = 0; k < placed.length; k++) {
      var p = placed[k], m = p.monitor.physical
      minX = Math.min(minX, p.x); minY = Math.min(minY, p.y)
      maxX = Math.max(maxX, p.x + m.widthMm); maxY = Math.max(maxY, p.y + m.heightMm)
      smallest = Math.min(smallest, m.widthPx / m.pixelsPerVpx)
    }
    var drawingWidth = Math.max(64, Math.floor(Math.min(w * 0.25, smallest * 0.25)))
    var mmPerVpx = Math.max(1, Math.ceil((maxX - minX) / drawingWidth), Math.ceil((maxY - minY) / Math.max(60, h * 0.24)))
    var dw = Math.ceil((maxX - minX) / mmPerVpx), dh = Math.ceil((maxY - minY) / mmPerVpx)
    var pw = Math.min(w - 48, Math.max(dw + 84, 262)), ph = dh + 122, lx = 24, ly = h - ph - 28
    if (lx + pw + 8 >= tx) ly = ty - ph - 8
    if (ly > topInset + 30) {
      plate(lx, ly, pw, ph, "OUTPUT LAYOUT / " + mmPerVpx + " mm/vpx")
      var ox = lx + 14, oy = ly + 50
      var dimensions = []
      for (var n = 0; n < placed.length; n++) {
        var entry = placed[n], mp = entry.monitor.physical
        var x = ox + Math.round((entry.x - minX) / mmPerVpx), y = oy + Math.round((entry.y - minY) / mmPerVpx)
        var rw = Math.max(1, Math.round(mp.widthMm / mmPerVpx)), rh = Math.max(1, Math.round(mp.heightMm / mmPerVpx))
        var here = entry.monitor.input.name === input.name
        if (here) rect(x, y, rw, rh, "line")
        outline(x, y, rw, rh, here ? "accent" : "edge")
        text(here ? "HERE" : entry.monitor.input.name, x + 4, y + 4, rw - 8, here ? "ink" : "ink")
        dimensions.push({ physical: mp, x: x, y: y, width: rw, height: rh })
      }
      // Bodies are drawn before dimensions so a solid HERE rectangle cannot
      // erase another output's lettering when their edges meet.
      for (var dim = 0; dim < dimensions.length; dim++) {
        var dimension = dimensions[dim], mp = dimension.physical
        var x = dimension.x, y = dimension.y, rw = dimension.width, rh = dimension.height
        var prefix = mp.estimated ? "~" : ""
        var wy = y - 12
        rect(x, wy, rw, 1, "edge"); rect(x, wy - 2, 1, 5, "edge"); rect(x + rw - 1, wy - 2, 1, 5, "edge")
        for (var arrow = 0; arrow < 4; arrow++) {
          rect(x + arrow, wy - arrow, 1, 2 * arrow + 1, "edge")
          rect(x + rw - 1 - arrow, wy - arrow, 1, 2 * arrow + 1, "edge")
        }
        var widthLabel = prefix + mp.widthMm.toFixed(1) + " mm"
        var tw = glyphs.length(widthLabel) * 6 + 1
        text(widthLabel, x + Math.floor((rw - tw) / 2), wy - 14, rw, "ink", true)
        var hx = x + rw + 10
        rect(hx, y, 1, rh, "edge"); rect(hx - 2, y, 5, 1, "edge"); rect(hx - 2, y + rh - 1, 5, 1, "edge")
        for (var arow = 0; arow < 4; arow++) {
          rect(hx - arow, y + arow, 2 * arow + 1, 1, "edge")
          rect(hx - arow, y + rh - 1 - arow, 2 * arow + 1, 1, "edge")
        }
        var heightLabel = prefix + mp.heightMm.toFixed(1) + " mm"
        var heightRoom = pw - (hx - lx) - 9
        if (!text(heightLabel, hx + 4, y + Math.floor((rh - 12) / 2), heightRoom))
          text(prefix + mp.heightMm.toFixed(1), hx + 4, y + Math.floor((rh - 12) / 2), heightRoom)
      }
      text("cursor layout / mm", lx + 5, ly + ph - 58, pw - 10)
      text("offsets follow compositor", lx + 5, ly + ph - 44, pw - 10)
      text("not desk placement", lx + 5, ly + ph - 30, pw - 10)
      text("edge rulers / cm", lx + 5, ly + ph - 16, pw - 10)
    }
  }
  return { width: w, height: h, strokes: strokes, labels: labels, plates: plates, xMarks: axisX, yMarks: axisY }
}

function paint(ctx, drawing, phys, colors, glow, dpr) {
  ctx.reset()
  ctx.scale(1 / dpr, 1 / dpr)
  ctx.clearRect(0, 0, drawing.width * phys, drawing.height * phys)
  if (glow) {
    ctx.fillStyle = colors.void
    ctx.fillRect(0, 0, drawing.width * phys, drawing.height * phys)
    ctx.fillStyle = colors.ground
    var bayer = [[0,8,2,10],[12,4,14,6],[3,11,1,9],[15,7,13,5]]
    var w = drawing.width, h = drawing.height, cx = Math.floor(w / 2), cy = Math.floor(h / 2)
    for (var y = 0; y < h; y++) {
      var ny = (y - cy) / (h * 0.62)
      for (var x = 0; x < w; x++) {
        var nx = (x - cx) / (w * 0.55)
        var level = Math.max(0, 1 - Math.sqrt(nx * nx + ny * ny))
        if (level * 16 > bayer[y % 4][x % 4] + 0.5) ctx.fillRect(x * phys, y * phys, phys, phys)
      }
    }
  }
  var last = ""
  for (var i = 0; i < drawing.strokes.length; i++) {
    var s = drawing.strokes[i]
    if (last !== s.role) { ctx.fillStyle = colors[s.role]; last = s.role }
    ctx.fillRect(s.x * phys, s.y * phys, s.w * phys, s.h * phys)
  }
}
