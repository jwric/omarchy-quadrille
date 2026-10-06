#!/usr/bin/env python3
"""The large face: Departure Mono's hero characters drawn at their own size.

The popups show a few readouts big: the temperature, the battery percentage, the
date. Drawing the 6 x 12 face with every pixel repeated n x n makes a mixel (a
pixel of another size beside everything else) and smoothing it (Scale2x) rounds
every corner. So the face is redrawn, at 2x (a 12 x 24 cell, caps 16 high, stems 2)
and 3x (18 x 36, caps 24, stems 3), by a few rules that keep Departure Mono's
shapes and its squareness and use the extra pixels for detail:

  * a stem is n pixels thick, as the small face's one pixel is n;
  * the small face turns a corner with a diagonal step; here the corner is square
    and only its outermost pixel is left out (`NOTCH`): a bowl is a box with a
    nibble taken out of each corner, not an octagon, not a curve;
  * a diagonal stroke (the slash of the 0, the 4, the 7, the %, v y x z k A M N) is a
    true 45 degree line, one pixel to a step, as thick as a stem is
    (n * 1.41 pixels along a row), not a staircase of n x n blocks;
  * everything else (bars, stems, dots) is the small glyph's own run, made n times
    bigger in both directions.

    plugins/tools/hero_face.py --bake        write Q/GlyphsBig.js
    plugins/tools/hero_preview.py OUT.png --mode native    see it

Hand-drawn exceptions are in OVERRIDES: rows of text, one per pixel row.
"""
import argparse
import math
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import hero_preview as hp

NOTCH = 1          # pixels cut from the outer corner of a bowl, at every size
SIZES = (2, 3)
# Characters the popups' heroes can show: digits, the signs, the units, the month names.
HERO = "0123456789 :.,-+%°CF—" + "abcdeghilmnoprstuvy" + "ADFJMNOS"


class Glyph:
    """A glyph in cells of the small face's grid (columns 0-6, rows 0-11, caps on rows 2-9),
    drawn at n pixels a cell: filled cells, nibbles taken out of outer corners, true diagonals."""

    def __init__(self):
        self.cells = set()
        self.nibs = []
        self.diags = []
        self.cuts = []

    def h(self, r, c0, c1):
        for c in range(c0, c1 + 1):
            self.cells.add((c, r))

    def v(self, c, r0, r1):
        for r in range(r0, r1 + 1):
            self.cells.add((c, r))

    def cell(self, c, r):
        self.cells.add((c, r))

    def box(self, c0, r0, c1, r1, nibs="tl tr bl br"):
        self.h(r0, c0, c1); self.h(r1, c0, c1); self.v(c0, r0, r1); self.v(c1, r0, r1)
        corners = {"tl": (c0, r0), "tr": (c1, r0), "bl": (c0, r1), "br": (c1, r1)}
        for k in nibs.split():
            self.nibs.append(corners[k] + (k,))

    def nib(self, c, r, corner):
        self.nibs.append((c, r, corner))

    def cut(self, c, r, side):
        """take the outer edge column ('l' or 'r') of a whole cell away: a waist."""
        self.cuts.append((c, r, side))

    def diag(self, c0, r0, c1, r1, thin=False):
        """a 45 degree stroke through the centres of two cells; `thin`: one pixel to a row
        per cell pixel (the small face's ratio) instead of as thick as a stem."""
        self.diags.append((c0, r0, c1, r1, thin))



def g_0(g):
    g.box(1, 2, 5, 9); g.diag(4, 4, 2, 6)
def g_1(g):
    g.v(3, 2, 9); g.h(3, 1, 3); g.h(9, 1, 5)
def g_2(g):
    g.h(2, 1, 5); g.v(1, 2, 3); g.v(5, 2, 4); g.diag(5, 4, 1, 8); g.h(9, 1, 5)
    g.nib(1, 2, "tl"); g.nib(5, 2, "tr")
def g_3(g):
    g.h(2, 1, 5); g.v(5, 2, 9); g.h(9, 1, 5); g.v(1, 2, 3); g.v(1, 8, 9); g.h(5, 3, 5)
    for c, r, k in ((1, 2, "tl"), (5, 2, "tr"), (1, 9, "bl"), (5, 9, "br")): g.nib(c, r, k)
def g_4(g):
    g.v(5, 2, 9); g.diag(4, 2, 1, 5); g.v(1, 5, 7); g.h(7, 1, 5)
def g_5(g):
    g.h(2, 1, 5); g.v(1, 2, 5); g.h(5, 1, 5); g.v(5, 5, 8); g.h(9, 1, 5); g.v(1, 8, 9)
    for c, r, k in ((5, 5, "tr"), (5, 9, "br"), (1, 9, "bl")): g.nib(c, r, k)
def g_6(g):
    g.h(2, 1, 5); g.v(1, 2, 9); g.h(9, 1, 5); g.h(5, 1, 5); g.v(5, 5, 9); g.v(5, 2, 3)
    for c, r, k in ((1, 2, "tl"), (5, 2, "tr"), (1, 9, "bl"), (5, 9, "br"), (5, 5, "tr")): g.nib(c, r, k)
def g_7(g):
    g.h(2, 1, 5); g.v(5, 2, 4); g.diag(5, 4, 2, 7); g.v(2, 7, 9)
def g_8(g):
    g.box(1, 2, 5, 5, "tl tr"); g.box(1, 5, 5, 9, "bl br")
    g.cut(1, 5, "l"); g.cut(5, 5, "r")
def g_9(g):
    g.box(1, 2, 5, 6, "tl tr bl"); g.v(5, 2, 9); g.h(9, 1, 5); g.v(1, 8, 9)
    g.nib(5, 9, "br"); g.nib(1, 9, "bl")
def g_colon(g):
    g.v(3, 4, 5); g.v(3, 8, 9)
def g_dot(g):
    g.v(3, 8, 9)
def g_comma(g):
    g.v(3, 8, 9); g.cell(2, 10)
def g_minus(g):
    g.h(6, 2, 4)
def g_plus(g):
    g.v(3, 4, 8); g.h(6, 1, 5)
def g_emdash(g):
    g.h(6, 0, 6)
def g_degree(g):
    g.box(2, 2, 4, 4)
def g_percent(g):
    g.box(1, 2, 3, 4); g.box(4, 7, 6, 9); g.diag(6, 2, 1, 7, True)
def g_C(g):
    g.h(2, 1, 5); g.v(1, 2, 9); g.h(9, 1, 5); g.v(5, 2, 3); g.v(5, 8, 9)
    for c, r, k in ((1, 2, "tl"), (5, 2, "tr"), (1, 9, "bl"), (5, 9, "br")): g.nib(c, r, k)
def g_F(g):
    g.h(2, 1, 5); g.v(1, 2, 9); g.h(5, 1, 4)

# ---- letters: the month names of the clock's date
def nibs(g, *spec):
    for c, r, k in spec: g.nib(c, r, k)
def l_a(g):
    g.h(4, 1, 5); g.v(5, 4, 9); g.h(6, 1, 5); g.v(1, 6, 9); g.h(9, 1, 5)
    nibs(g, (1, 4, "tl"), (5, 4, "tr"), (1, 9, "bl"), (5, 9, "br"))
def l_b(g):
    g.v(1, 2, 9); g.h(4, 1, 5); g.h(9, 1, 5); g.v(5, 4, 9); nibs(g, (5, 4, "tr"), (5, 9, "br"))
def l_c(g):
    g.h(4, 1, 5); g.v(1, 4, 9); g.h(9, 1, 5); g.v(5, 4, 5); g.v(5, 8, 9)
    nibs(g, (1, 4, "tl"), (5, 4, "tr"), (1, 9, "bl"), (5, 9, "br"))
def l_d(g):
    g.v(5, 2, 9); g.h(4, 1, 5); g.h(9, 1, 5); g.v(1, 4, 9); nibs(g, (1, 4, "tl"), (1, 9, "bl"))
def l_e(g):
    g.h(4, 1, 5); g.v(5, 4, 6); g.h(6, 1, 5); g.v(1, 4, 9); g.h(9, 1, 5); g.v(5, 8, 9)
    nibs(g, (1, 4, "tl"), (5, 4, "tr"), (1, 9, "bl"), (5, 9, "br"))
def l_g(g):
    g.h(4, 1, 5); g.v(1, 4, 9); g.h(9, 1, 5); g.v(5, 4, 11); g.h(11, 1, 5)
    nibs(g, (1, 4, "tl"), (1, 9, "bl"), (5, 11, "br"), (1, 11, "bl"))
def l_h(g):
    g.v(1, 2, 9); g.h(4, 1, 5); g.v(5, 4, 9); nibs(g, (5, 4, "tr"))
def l_i(g):
    g.cell(3, 2); g.h(4, 1, 3); g.v(3, 4, 9); g.h(9, 1, 5)
def l_l(g):
    g.h(2, 1, 3); g.v(3, 2, 9); g.h(9, 1, 5)
def l_m(g):
    g.v(1, 4, 9); g.v(3, 4, 9); g.v(5, 4, 9); g.h(4, 1, 5); nibs(g, (5, 4, "tr"))
def l_n(g):
    g.v(1, 4, 9); g.h(4, 1, 5); g.v(5, 4, 9); nibs(g, (5, 4, "tr"))
def l_o(g):
    g.box(1, 4, 5, 9)
def l_p(g):
    g.v(1, 4, 11); g.h(4, 1, 5); g.h(9, 1, 5); g.v(5, 4, 9); nibs(g, (5, 4, "tr"), (5, 9, "br"))
def l_r(g):
    g.v(2, 4, 9); g.h(4, 1, 2); g.h(5, 2, 3); g.h(4, 4, 5); g.diag(3, 5, 4, 4, True); g.h(9, 1, 4)
def l_s(g):
    g.h(4, 1, 5); g.v(1, 4, 6); g.h(6, 1, 5); g.v(5, 6, 9); g.h(9, 1, 5); g.v(5, 4, 5); g.v(1, 8, 9)
    nibs(g, (1, 4, "tl"), (5, 4, "tr"), (1, 6, "bl"), (5, 6, "tr"), (1, 9, "bl"), (5, 9, "br"))
def l_t(g):
    g.v(2, 2, 9); g.h(4, 1, 5); g.h(9, 2, 5); g.v(5, 8, 9); nibs(g, (5, 9, "br"))
def l_u(g):
    g.v(1, 4, 9); g.v(5, 4, 9); g.h(9, 1, 5); nibs(g, (1, 9, "bl"))
def l_v(g):
    g.v(1, 4, 6); g.v(5, 4, 6); g.diag(1, 7, 3, 9); g.diag(5, 7, 3, 9)
def l_y(g):
    g.v(1, 4, 6); g.v(5, 4, 6); g.diag(1, 7, 3, 9); g.diag(5, 7, 3, 9); g.v(3, 9, 10); g.h(11, 1, 3)
def l_A(g):
    g.diag(3, 2, 1, 4); g.diag(3, 2, 5, 4); g.v(1, 4, 9); g.v(5, 4, 9); g.h(6, 1, 5)
def l_D(g):
    g.box(1, 2, 5, 9, "tr br")
def l_F(g):
    g.h(2, 1, 5); g.v(1, 2, 9); g.h(5, 1, 4)
def l_J(g):
    g.h(2, 3, 5); g.v(5, 2, 9); g.h(9, 1, 5); g.v(1, 8, 9); nibs(g, (1, 9, "bl"), (5, 9, "br"))
def l_M(g):
    g.v(1, 2, 9); g.v(5, 2, 9); g.diag(1, 3, 3, 5); g.diag(5, 3, 3, 5); g.v(3, 5, 6)
def l_N(g):
    g.v(1, 2, 9); g.v(5, 2, 9); g.diag(1, 3, 5, 7)
def l_O(g):
    g.box(1, 2, 5, 9)
def l_S(g):
    g.h(2, 1, 5); g.v(1, 2, 5); g.h(5, 1, 5); g.v(5, 5, 9); g.h(9, 1, 5); g.v(5, 2, 3); g.v(1, 8, 9)
    nibs(g, (1, 2, "tl"), (5, 2, "tr"), (1, 5, "bl"), (5, 5, "tr"), (1, 9, "bl"), (5, 9, "br"))


DEFS = {"0": g_0, "1": g_1, "2": g_2, "3": g_3, "4": g_4, "5": g_5, "6": g_6, "7": g_7, "8": g_8, "9": g_9,
        ":": g_colon, ".": g_dot, ",": g_comma, "-": g_minus, "+": g_plus, "—": g_emdash, "°": g_degree,
        "%": g_percent, "C": g_C, "F": g_F,
        "a": l_a, "b": l_b, "c": l_c, "d": l_d, "e": l_e, "g": l_g, "h": l_h, "i": l_i, "l": l_l, "m": l_m,
        "n": l_n, "o": l_o, "p": l_p, "r": l_r, "s": l_s, "t": l_t, "u": l_u, "v": l_v, "y": l_y,
        "A": l_A, "D": l_D, "J": l_J, "M": l_M, "N": l_N, "O": l_O, "S": l_S}
DEFS["F"] = l_F


def rasterise(g, n, notch=NOTCH):
    """-> (12 n) rows of (7 n) 0/1."""
    W, H = 7 * n, 12 * n
    out = [[0] * W for _ in range(H)]
    for (c, r) in g.cells:
        for dy in range(n):
            for dx in range(n):
                out[r * n + dy][c * n + dx] = 1
    for (c, r, k) in g.nibs:
        for a in range(notch):
            for b in range(notch - a):
                x = c * n + (b if k[1] == "l" else n - 1 - b)
                y = r * n + (a if k[0] == "t" else n - 1 - a)
                out[y][x] = 0
    for (c, r, side) in g.cuts:
        x = c * n if side == "l" else c * n + n - 1
        for dy in range(n):
            out[r * n + dy][x] = 0
    base = [row[:] for row in out]
    for (c0, r0, c1, r1, thin) in g.diags:
        run = n if thin else round(n * 1.4142)
        s = 1 if c1 > c0 else -1
        lo, hi = min(c0, c1) * n, (max(c0, c1) + 1) * n
        cx0, cy0 = c0 * n + n / 2.0, r0 * n + n / 2.0
        for y in range(r0 * n, (r1 + 1) * n):
            xc = cx0 + s * (y + 0.5 - cy0)
            xa = int(math.floor(xc - run / 2.0 + 0.5))
            for x in range(max(lo, xa), min(hi, xa + run)):
                if thin and any(0 <= y + dy < H and 0 <= x + dx < W and base[y + dy][x + dx]
                                for dy in (-1, 0, 1) for dx in (-1, 0, 1)):
                    continue          # a thin stroke keeps a pixel of air round the shapes it passes
                out[y][x] = 1
    return out


def small_bitmap(glyphs, ch):
    g = glyphs.get(ord(ch))
    return [row[:] for row in g] if g else [[0] * 7 for _ in range(12)]


_cache = {}


def glyph(ch, n):
    key = (ch, n)
    if key not in _cache:
        if ch in DEFS:
            g = Glyph(); DEFS[ch](g)
            _cache[key] = rasterise(g, n)
        else:
            # not designed yet: the small glyph, repeated
            b = small_bitmap(hp.small_glyphs(), ch)
            _cache[key] = [[v for v in row for _ in range(n)] for row in b for _ in range(n)]
    return _cache[key]


def render(text, n):
    cols = 6 * n * len(text) + n
    grid = [[0] * cols for _ in range(12 * n)]
    for i, ch in enumerate(text):
        g = glyph(ch, n)
        for y in range(12 * n):
            for x in range(7 * n):
                if g[y][x]:
                    grid[y][6 * n * i + x] = 1
    return grid


JS_HEAD = """// Generated by plugins/tools/hero_face.py: do not edit (edit the glyph table there).
//
// The large face for the hero readouts (the temperature, the battery percentage, the
// clock's date), drawn at its own size, 2x and 3x of the 6 x 12 cell: a 12 x 24 and an
// 18 x 36 cell, stems 2 and 3 pixels, caps 16 and 24 high. Not the small face made
// bigger: its corners are square with one pixel nibbled off (the small face's
// diagonal step), its diagonals are true 45 degree lines one pixel to a step, and
// nothing in it is smoothed or interpolated. Drawn one pixel to one pixel of the surface.
//
// `rows` is 12 n rows of 7 n bits (the seventh column of a cell hangs into the next one,
// as the small face's does), hex, the leftmost pixel the highest bit. A character that
// has no large drawing is the small face's bitmap with every pixel repeated n x n:
// square, hard, never smoothed, and a mixel; the characters below are the ones the
// popups' heroes use.
.pragma library
.import "Glyphs.js" as Glyphs
"""

JS_TAIL = """
var cache = {}

function smallBitmap(code) {
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

function repeat(src, n) {
  var out = []
  for (var y = 0; y < src.length; y++) {
    var row = []
    for (var x = 0; x < src[y].length; x++) for (var i = 0; i < n; i++) row.push(src[y][x])
    for (var j = 0; j < n; j++) out.push(row)
  }
  return out
}

// The drawn bitmap of one character at size n, or null.
function drawn(code, n) {
  var table = n === 2 ? two : (n === 3 ? three : null)
  var rows = table ? table[String(code)] : undefined
  if (rows === undefined) return null
  var digits = Math.ceil(7 * n / 4), out = []
  for (var y = 0; y < 12 * n; y++) {
    var bits = parseInt(rows.substr(y * digits, digits), 16)
    var row = []
    for (var x = 0; x < 7 * n; x++) row.push((bits >> (7 * n - 1 - x)) & 1)
    out.push(row)
  }
  return out
}

// The lit runs of one glyph as rectangles: [{x, y, w, h}], runs that repeat down
// consecutive rows merged into one.
function glyphRects(code, n) {
  var key = n + ":" + code
  if (cache[key]) return cache[key]
  var big = drawn(code, n) || repeat(smallBitmap(code), n)
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
"""


def pack(grid, n):
    digits = (7 * n + 3) // 4
    out = []
    for row in grid:
        v = 0
        for x in range(7 * n):
            v = (v << 1) | row[x]
        out.append(format(v, "0%dx" % digits))
    return "".join(out)


def bake(path):
    chars = [c for c in HERO if c in DEFS]
    parts = [JS_HEAD]
    for n, name in ((2, "two"), (3, "three")):
        parts.append("var %s = {\n" % name)
        parts.append(",\n".join('  "%d":"%s"' % (ord(c), pack(glyph(c, n), n)) for c in chars))
        parts.append("\n}\n\n")
    parts.append(JS_TAIL)
    Path(path).write_text("".join(parts))
    print("baked %d characters at 2x and 3x -> %s" % (len(chars), path))


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bake", action="store_true", help="write quadrille.bar/Q/GlyphsBig.js")
    ap.add_argument("--out", default=str(HERE.parent / "quadrille.bar/Q/GlyphsBig.js"))
    a = ap.parse_args()
    if a.bake:
        bake(a.out)
    else:
        ap.print_help()
