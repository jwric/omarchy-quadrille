#!/usr/bin/env python3
"""sheet.py OUT.png WIDTH names...: crop each grab to what differs from its corner colour
and tile them on one sheet with labels."""
import sys
from PIL import Image, ImageDraw, ImageChops
out = sys.argv[1]; maxw = int(sys.argv[2]); files = sys.argv[3:]
tiles = []
for f in files:
    im = Image.open(f).convert("RGBA")
    bg = Image.new("RGBA", im.size, im.getpixel((0, 0)))
    bb = ImageChops.difference(im, bg).convert("L").point(lambda v: 255 if v > 0 else 0).getbbox() or (0, 0, 10, 10)
    m = 8
    bb = (max(0, bb[0] - m), max(0, bb[1] - m), min(im.width, bb[2] + m), min(im.height, bb[3] + m))
    c = im.crop(bb)
    flat = Image.new("RGB", c.size, (60, 60, 90)); flat.paste(c, mask=c.split()[3])
    tiles.append((f.split("/")[-1][:-4], flat))
x = y = rowh = 0; pos = []
for n, t in tiles:
    if x + t.width > maxw and x > 0: x = 0; y += rowh + 14; rowh = 0
    pos.append((x, y + 12)); x += t.width + 8; rowh = max(rowh, t.height + 12)
sheet = Image.new("RGB", (maxw, y + rowh + 4), (30, 30, 50)); d = ImageDraw.Draw(sheet)
for (n, t), (px, py) in zip(tiles, pos):
    sheet.paste(t, (px, py)); d.text((px, py - 11), n, fill=(200, 200, 120))
sheet.save(out); print(out, sheet.size)
