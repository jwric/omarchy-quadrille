#!/usr/bin/env python3
"""Draw every sprite of Q/Sprites.qml on a sheet, so a new one can be judged
without a shell: plugins/tools/preview_sprites.py [out.png] [name ...]

Lit pixels are ink, `1`-`9` digits show at full level (lit) on the left of each
pair and at level 0 (dim) on the right, `!` is the alarm colour.
"""
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw

SRC = Path(__file__).resolve().parent.parent / "quadrille.bar/Q/Sprites.qml"
INK, DIM, ALARM, GROUND, EDGE = (192, 192, 192), (90, 90, 90), (232, 85, 62), (34, 34, 34), (72, 72, 68)


def parse():
    text = SRC.read_text()
    out = {}
    for name, body in re.findall(r"readonly property var (\w+): \[(.*?)\]", text, re.S):
        rows = re.findall(r'"([^"]*)"', body)
        if rows:
            out[name] = rows
    return out


def draw(rows, level, scale, tone):
    w, h = len(rows[0]), len(rows)
    img = Image.new("RGB", (w * scale, h * scale), GROUND)
    d = ImageDraw.Draw(img)
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            colour = None
            if ch == "#":
                colour = INK
            elif ch == "!":
                colour = ALARM
            elif ch.isdigit() and ch != "0":
                colour = INK if int(ch) <= level else DIM
            if colour:
                d.rectangle([x * scale, y * scale, (x + 1) * scale - 1, (y + 1) * scale - 1], fill=colour)
    return img


def main():
    args = [a for a in sys.argv[1:]]
    out = args[0] if args and args[0].endswith(".png") else "sprites.png"
    names = [a for a in args if not a.endswith(".png")]
    sprites = parse()
    if names:
        sprites = {k: v for k, v in sprites.items() if k in names}
    scale, pad, cols = 8, 10, 8
    cell_w = 2 * 11 * scale + 4 * pad + 24
    cell_h = 11 * scale + 2 * pad + 12
    rows = (len(sprites) + cols - 1) // cols
    sheet = Image.new("RGB", (cols * cell_w, rows * cell_h), (20, 20, 20))
    d = ImageDraw.Draw(sheet)
    for i, (name, bitmap) in enumerate(sprites.items()):
        cx, cy = (i % cols) * cell_w, (i // cols) * cell_h
        a = draw(bitmap, 9, scale, INK)
        b = draw(bitmap, 0, scale, DIM)
        sheet.paste(a, (cx + pad, cy + pad + 12))
        sheet.paste(b, (cx + 2 * pad + a.width, cy + pad + 12))
        real = draw(bitmap, 9, 2, INK)
        sheet.paste(real, (cx + 3 * pad + 2 * a.width, cy + pad + 12))
        d.text((cx + pad, cy + 2), name, fill=(200, 200, 120))
    sheet.save(out)
    print(len(sprites), "sprites ->", out)


if __name__ == "__main__":
    main()
