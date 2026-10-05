#!/usr/bin/env python3
"""The graticule wallpaper at an output's own pixel grid.

tools/gen_themes.py draws one 3440x1440 picture at 2 physical pixels per
virtual pixel. On an output whose scale makes a virtual pixel 3 physical pixels
(2560x1600 at 1.666667) that picture is resampled by a non-integer factor and
the dither lands uneven. plugins/quadrille.background draws the picture per
output at the right block size instead (graticule.frag, the same picture as
gen_themes.graticule). This writes it as a PNG: for a single-output setup (set
it with omarchy-theme-bg-set), as the reference a grim capture of the live
background is compared with (--compare), and for a quick look.

    tools/gen_wallpapers.py --size 2560x1600 --block 3 --theme terminal out.png
    tools/gen_wallpapers.py --size 2560x1600 --block 3 --all outdir/
    tools/gen_wallpapers.py --size 2560x1600 --block 3 --compare capture.png --skip-top 48

--size is in physical pixels, --block is physical pixels per virtual pixel
(max(1, round(2 * scale)): 2 at scale 1, 3 at 1.666667, 4 at 2). The picture is
ceil(size / block) virtual pixels across, anchored at the top left, and the
screen's edge crops the last one when the size is not a multiple of the block.
At 3440x1440 and block 2 it is the PNG gen_themes.py ships, byte for byte.
"""
import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_themes as g  # noqa: E402


def wallpaper(roles, width, height, block):
    """RGB rows of a `width` x `height` output at `block` physical px per vpx."""
    w, h = -(-width // block), -(-height // block)
    out = []
    for row in g.graticule(roles, w, h):
        wide = [p for p in row for _ in range(block)][:width]
        out += [wide] * block
    return out[:height]


def compare(capture, rows, skip_top):
    """Print how many pixels of `capture` differ from `rows`, and where."""
    from PIL import Image  # only for this: the writer above needs nothing

    shot = Image.open(capture).convert("RGB")
    height, width = len(rows), len(rows[0])
    if shot.size != (width, height):
        print(f"size {shot.size} is not {width}x{height}")
        return 1
    px = shot.load()
    off = [(x, y) for y in range(skip_top, height) for x in range(width) if px[x, y] != tuple(rows[y][x])]
    box = (min(x for x, _ in off), min(y for _, y in off), max(x for x, _ in off), max(y for _, y in off)) if off else None
    print(f"{len(off)} of {width * (height - skip_top)} pixels differ" + (f", within {box}" if box else ""))
    return 1 if off else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--quadrille", type=Path, default=g.DEFAULT_THEME_RS)
    parser.add_argument("--size", default="3440x1440", help="output size in physical pixels")
    parser.add_argument("--block", type=int, default=2, help="physical pixels per virtual pixel")
    parser.add_argument("--theme", default="terminal", help="terminal, paper, phosphor, amber or lcd")
    parser.add_argument("--all", action="store_true", help="every theme; OUT is a directory")
    parser.add_argument("--compare", type=Path, metavar="CAPTURE",
                        help="instead of writing, count the pixels of a screenshot that differ from the picture")
    parser.add_argument("--skip-top", type=int, default=0, help="with --compare: ignore this many rows (the bar)")
    parser.add_argument("out", type=Path, nargs="?")
    args = parser.parse_args()

    width, height = (int(n) for n in args.size.split("x"))
    palettes = g.parse_palettes(args.quadrille)
    if args.compare:
        return compare(args.compare, wallpaper(palettes[args.theme], width, height, args.block), args.skip_top)
    if args.out is None:
        parser.error("OUT is required unless --compare")
    names = [n for n in palettes if n in g.SPECS] if args.all else [args.theme]
    if args.all:
        args.out.mkdir(parents=True, exist_ok=True)
    for name in names:
        target = args.out / f"quadrille-{name}-{width}x{height}.png" if args.all else args.out
        target.write_bytes(g.png(width, height, wallpaper(palettes[name], width, height, args.block)))
        print(target)


if __name__ == "__main__":
    sys.exit(main())
