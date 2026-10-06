#!/usr/bin/env python3
"""timeline.py TAG CASE [FROM_MS TO_MS] : the STALL and SURF lines of the scratch shell's log (flick-TAG.log) around the
open command of CASE (the MARK line of flick-A.out / flick-B.out), in milliseconds after it, with the frame in which the
card first shows (frames.py's diff) at the end. `gap=` lines are stretches in which the GUI thread did not run."""
import glob, os, re, sys
S = os.environ.get("SURF_WORK") or os.path.join(os.environ.get("TMPDIR", "/tmp"), "quadrille-surfaces")
tag, case = sys.argv[1], sys.argv[2]
lo, hi = (int(sys.argv[3]), int(sys.argv[4])) if len(sys.argv) > 4 else (-100, 3000)
t0 = None; cur = None
for f in glob.glob(S + "/flick-[AB].out"):
    for l in open(f, errors="replace"):
        m = re.match(r"=== (\w+) legacy=(\d)", l)
        if m: cur = m.group(1)
        m = re.match(r"MARK (\d+) open " + re.escape(case) + r"\s*$", l.strip())
        if m and cur == tag: t0 = int(m.group(1))
if t0 is None: sys.exit("no open mark for " + case)
for l in open(f"{S}/flick-{tag}.log", errors="replace"):
    l = re.sub(r"\x1b\[[0-9;]*m", "", l).strip()
    m = re.search(r"(STALL|SURF) (\d+) (.*)$", l)
    if not m: continue
    rel = int(m.group(2)) - t0
    if lo <= rel <= hi:
        body = m.group(3)
        if m.group(1) == "SURF":
            body = re.sub(r"\| size=.*?(card=)", r"\1", body)[:140]
        print(f"{rel:+6d}ms {m.group(1):5s} {body}")
