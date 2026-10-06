#!/usr/bin/env python3
"""Check the painted 10 mm marks against the shared physical model's plan."""
import json
from pathlib import Path
import sys
import tomllib

from PIL import Image

out = Path(sys.argv[1])
metrics = json.loads((out / "ruler-metrics.json").read_text())
repo = Path(__file__).resolve().parents[2]
roles = tomllib.loads((repo / "themes/quadrille-terminal/shell.toml").read_text())["quadrille"]
line_colors = {tuple(bytes.fromhex(roles[name].lstrip("#"))) for name in ("line", "edge")}
for name, data in metrics.items():
    image = Image.open(out / f"{name}.png").convert("RGB")
    phys = data["pixelsPerVpx"]
    y = next(y for y in range(150, 250) if all(not m["vpx"] * phys <= y < (m["vpx"] + 1) * phys for m in data["yMarks"]))
    marks = [m["vpx"] * phys for m in data["xMarks"] if 150 < m["vpx"] * phys < image.width - 150]
    for x in marks:
        assert all(image.getpixel((at, y)) in line_colors for at in range(x, x + phys)), (name, "missing", x, y)
        assert image.getpixel((x - 1, y)) not in line_colors, (name, "wide", x, y)
        assert image.getpixel((x + phys, y)) not in line_colors, (name, "wide", x, y)
    intervals = sorted(set(b - a for a, b in zip(marks, marks[1:])))
    assert intervals == data["intervalsPx"], (name, intervals)
    print(f"{name}: {len(marks)} painted marks, {intervals} physical px per 10 mm; absolute error <= {phys / 2:g} px")
