#!/usr/bin/env python3
"""overlays-report.py SCREEN: for each opened overlay, the SURF lines of its window and its card in the 600 ms
after the open mark (ms relative to the mark), and a verdict: how many distinct card sizes were ever visible."""
import os, re, sys
S = os.environ.get("SURF_WORK") or os.path.join(os.environ.get("TMPDIR", "/tmp"), "quadrille-surfaces")
screen = sys.argv[1]
marks = []
for l in open(f"{S}/overlays-{screen}.out", errors="replace"):
    m = re.match(r"MARK (\d+) (.*)", l.strip())
    if m: marks.append((int(m.group(1)), m.group(2)))
rows = []
for l in open(f"{S}/overlays-{screen}.log", errors="replace"):
    l = re.sub(r"\x1b\[[0-9;]*m", "", l).strip()
    m = re.search(r"SURF (\d+) (\S+) (.*)$", l)
    if m: rows.append((int(m.group(1)), m.group(2), m.group(3)))
for i, (t0, name) in enumerate(marks):
    end = marks[i + 1][0] if i + 1 < len(marks) else t0 + 3000
    end = min(end, t0 + 700)
    print(f"-- {name}")
    sizes = []
    for ts, tag, what in rows:
        if t0 - 2 <= ts < end and ".card" in tag or (t0 - 2 <= ts < end and ".column" in tag):
            vis = re.search(r"vis=(\w+)", what)
            sz = re.search(r"size=(\S+)", what)
            print(f"  +{ts - t0:4d}ms {tag:18s} {what[:130]}")
            if vis and vis.group(1) == "true" and sz and sz.group(1) not in ("0x0",): sizes.append(sz.group(1))
        elif t0 - 2 <= ts < end and ".window" in tag and what.split(" |")[0] in ("visible", "dpr", "screen"):
            print(f"  +{ts - t0:4d}ms {tag:18s} {what[:130]}")
    print(f"   distinct visible card sizes: {sorted(set(sizes))}")
