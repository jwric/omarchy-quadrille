#!/usr/bin/env python3
"""Render the hero strings in the small face and the large one(s) on one sheet, one
font pixel to a block of PNG pixels, to judge corners and family resemblance.

    plugins/tools/hero_preview.py OUT.png [--mode replicate|native] [--zoom 4] [STRING ...]

`replicate` draws the small face with every pixel repeated n x n (the stand-in);
`native` draws the large face of tools/hero_face.py at its own size. Both are drawn
one surface pixel to one block of `zoom` pixels. Strings default to the ones the
popups show: the temperature, the battery percentage, the clock's date and a time.
"""
import argparse
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw

HERE = Path(__file__).resolve().parent
GLYPHS = HERE.parent / "quadrille.bar/Q/Glyphs.js"
INK, MUTED, GROUND, EDGE = (200, 204, 208), (120, 124, 128), (24, 26, 28), (60, 64, 68)


def small_glyphs():
    text = GLYPHS.read_text()
    out = {}
    for code, hexrow in re.findall(r'"(\d+)":"([0-9a-f]{24})"', text):
        rows = []
        for y in range(12):
            bits = int(hexrow[2 * y:2 * y + 2], 16)
            rows.append([(bits >> (6 - x)) & 1 for x in range(7)])
        out[int(code)] = rows
    return out


def draw_small(glyphs, text):
    """-> list of rows (lists of 0/1) for `text` at 1x: 6 columns a cell, 12 rows."""
    w = 6 * len(text) + 1
    grid = [[0] * w for _ in range(12)]
    for i, ch in enumerate(text):
        g = glyphs.get(ord(ch))
        if not g:
            continue
        for y in range(12):
            for x in range(7):
                if g[y][x]:
                    grid[y][6 * i + x] = 1
    return grid


def replicate(grid, n):
    out = []
    for row in grid:
        big = [v for v in row for _ in range(n)]
        out.extend([big] * n)
    return out


def native(text, n):
    sys.path.insert(0, str(HERE))
    import hero_face
    return hero_face.render(text, n)


def blit(img, grid, ox, oy, zoom, ink=INK):
    d = ImageDraw.Draw(img)
    for y, row in enumerate(grid):
        for x, v in enumerate(row):
            if v:
                d.rectangle([ox + x * zoom, oy + y * zoom, ox + (x + 1) * zoom - 1, oy + (y + 1) * zoom - 1], fill=ink)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--mode", default="replicate", choices=["replicate", "native"])
    ap.add_argument("--zoom", type=int, default=4)
    ap.add_argument("strings", nargs="*")
    a = ap.parse_args()
    strings = a.strings or ["6°C", "-12°C", "95%", "62%", "23:10", "October 6", "100%"]
    glyphs = small_glyphs()
    z = a.zoom
    rows = []
    for s in strings:
        small = draw_small(glyphs, s)
        for n in (2, 3):
            big = replicate(small, n) if a.mode == "replicate" else native(s, n)
            rows.append((s, n, small, big))
    pad = 10
    width = max((len(sm[0]) + 3) * z + len(g[0]) * z for _, n, sm, g in rows) + 110 + 2 * pad
    heights = [len(g) * z + 2 * pad for _, n, sm, g in rows]
    img = Image.new("RGB", (width, sum(heights) + pad), GROUND)
    d = ImageDraw.Draw(img)
    y = pad
    for (s, n, small, big), h in zip(rows, heights):
        d.text((6, y), f"{s}  {n}x", fill=MUTED)
        # the small face first, at its own size, beside the large one
        blit(img, small, 110, y + 6, z, MUTED)
        blit(img, big, 110 + (len(small[0]) + 3) * z, y, z)
        y += h
    img.save(a.out)
    print(a.out, img.size)


if __name__ == "__main__":
    main()
