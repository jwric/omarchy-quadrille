#!/usr/bin/env python3
"""frames.py TAG CASE...: for each captured frame of a case, the bounding box and the number of pixels that differ from
the frame taken before the open (frame 1), with the time of the frame relative to the open mark. A card that jumps
size shows as a changing box; a gap between two popups as a count that falls to the bar's own."""
import re, sys
from PIL import Image, ImageChops
import os
S=os.environ.get("SURF_WORK") or os.path.join(os.environ.get("TMPDIR","/tmp"),"quadrille-surfaces")
tag=sys.argv[1]
ts={}
for l in open(f"{S}/frames/{tag}/ts"):
    a,b=l.split(); ts[a]=int(b)
marks=[]
for f in (S+"/flick-A.out", S+"/flick-B.out"):
    try: lines=open(f, errors="replace").read().split("\n")
    except OSError: continue
    cur=None
    for l in lines:
        m=re.match(r"=== (\w+) legacy=(\d)", l)
        if m: cur=m.group(1)+("-legacy" if m.group(2)=="1" else ""); continue
        m=re.match(r"MARK (\d+) (.*)", l.strip())
        if m and cur==tag: marks.append((int(m.group(1)), m.group(2)))
for case in sys.argv[2:]:
    n=sorted(int(k.split("-")[1]) for k in ts if k.startswith(case+"-"))
    ref=Image.open(f"{S}/frames/{tag}/{case}-1.ppm").convert("RGB")
    opens=[t for t,name in marks if name.startswith("open") and (case in name or case=="switch" or (case=="tray" and False))]
    print(f"== {tag} {case}")
    t0=None
    for t,name in marks:
        if ts.get(f"{case}-1") and abs(t-ts[f"{case}-1"])<600 and name.startswith("open"): t0=t; break
    for i in n:
        im=Image.open(f"{S}/frames/{tag}/{case}-{i}.ppm").convert("RGB")
        d=ImageChops.difference(im,ref)
        box=d.getbbox()
        cnt=sum(1 for p in d.convert("L").getdata() if p>24) if box else 0
        rel=(ts[f"{case}-{i}"]-t0) if t0 else 0
        print(f"  frame {i} {rel:+5d}ms  diff pixels {cnt:7d}  bbox {box}  size {None if not box else (box[2]-box[0], box[3]-box[1])}")
