#!/usr/bin/env python3
"""Is a region of a screenshot pixel-crisp?

    crisp.py IMAGE X0,Y0,X1,Y1 PIXEL_SCALE [LABEL] [--check]

A surface drawn in virtual pixels and upscaled nearest-neighbour has few
colours (no blending), and every change of colour along a row or a column
falls on a multiple of the pixel scale from the region's origin. The origin
must be the surface's own: its physical position.

With --check the exit status is 0 when the region has at most 40 colours and
not one change off the grid, and 1 otherwise.
"""
import sys
from collections import Counter

from PIL import Image


def analyse(path, box, ps):
    im = Image.open(path).convert("RGB")
    x0, y0, x1, y1 = box
    px = im.load()
    colors = Counter()
    for y in range(y0, y1):
        for x in range(x0, x1):
            colors[px[x, y]] += 1
    hx = Counter()
    for y in range(y0, y1):
        for x in range(x0 + 1, x1):
            if px[x, y] != px[x - 1, y]:
                hx[(x - x0) % ps] += 1
    hy = Counter()
    for x in range(x0, x1):
        for y in range(y0 + 1, y1):
            if px[x, y] != px[x, y - 1]:
                hy[(y - y0) % ps] += 1
    return colors, hx, hy


def main():
    args = [a for a in sys.argv[1:] if a != "--check"]
    check = "--check" in sys.argv
    path = args[0]
    box = tuple(int(v) for v in args[1].split(","))
    ps = int(args[2])
    label = args[3] if len(args) > 3 else ""
    colors, hx, hy = analyse(path, box, ps)
    print(f"{label} box={box} unique colours={len(colors)} top5={colors.most_common(5)}")
    print(f"  x-transition residues mod {ps}: {dict(sorted(hx.items()))}")
    print(f"  y-transition residues mod {ps}: {dict(sorted(hy.items()))}")
    off = sum(n for r, n in hx.items() if r) + sum(n for r, n in hy.items() if r)
    if check:
        sys.exit(0 if len(colors) <= 40 and off == 0 else 1)


if __name__ == "__main__":
    main()
