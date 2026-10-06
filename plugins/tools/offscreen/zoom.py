#!/usr/bin/env python3
"""zoom.py IN.png OUT.png X Y W H FACTOR: crop (relative to what differs from the corner) and enlarge, nearest."""
import sys
from PIL import Image, ImageChops
src, out, x, y, w, h, f = sys.argv[1:8]; x, y, w, h, f = map(int, (x, y, w, h, f))
im = Image.open(src).convert("RGBA")
bg = Image.new("RGBA", im.size, im.getpixel((0, 0)))
bb = ImageChops.difference(im, bg).convert("L").point(lambda v: 255 if v > 0 else 0).getbbox()
c = im.crop((bb[0] + x, bb[1] + y, bb[0] + x + w, bb[1] + y + h))
flat = Image.new("RGBA", c.size, (40, 40, 70, 255)); flat.alpha_composite(c)
flat.resize((c.width * f, c.height * f), Image.NEAREST).convert("RGB").save(out); print(bb)
