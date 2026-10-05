#!/usr/bin/env python3
"""Cut a popup out of a screenshot by what differs from the wallpaper alone.

    popup-crop.py BASE.png SHOT.png OUT.png [BAR_PX=34] [MARGIN=10] [SCALE=1]

The rows and columns under BAR_PX (the bar, whose button changes while a popup
is open) are ignored, and so are a few stray pixels (a moved pointer); what is
left is the card. OUT keeps the bar strip above it and MARGIN px around the
card. Prints the card's box in SHOT's pixels. Pair it with popup-shots.sh and
layershell/tools/crisp.py: the box's corner is a multiple of the pixel scale,
so it can be handed to crisp.py as it is.
"""
import sys
from PIL import Image, ImageChops
base = Image.open(sys.argv[1]).convert("RGB"); shot = Image.open(sys.argv[2]).convert("RGB")
bar = int(sys.argv[4]) if len(sys.argv) > 4 else 34
margin = int(sys.argv[5]) if len(sys.argv) > 5 else 12
scale = float(sys.argv[6]) if len(sys.argv) > 6 else 1
diff = ImageChops.difference(base, shot).convert("L").point(lambda v: 255 if v > 0 else 0)
diff.paste(0, (0, 0, diff.width, bar))
w, h = diff.size
px = diff.load()
# rows and columns with a real amount of difference (a moved pointer is a few pixels)
rows = [y for y in range(h) if sum(1 for x in range(0, w, 1) if px[x, y]) > 40]
cols = [x for x in range(w) if sum(1 for y in range(0, h, 1) if px[x, y]) > 40]
if not rows or not cols:
    print("no popup found"); sys.exit(1)
x0, x1, y0, y1 = min(cols), max(cols), min(rows), max(rows)
box = (max(0, x0 - margin), 0, min(w, x1 + 1 + margin), min(h, y1 + 1 + margin))
out = shot.crop(box)
if scale != 1: out = out.resize((int(out.width * scale), int(out.height * scale)), Image.NEAREST)
out.save(sys.argv[3]); print((x0, y0, x1, y1), out.size)
