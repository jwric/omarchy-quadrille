import re,sys
import os
S=os.environ.get("SURF_WORK") or os.path.join(os.environ.get("TMPDIR","/tmp"),"quadrille-surfaces")
secs={}; cur=None
import glob
for l in [x for f in glob.glob(S+"/flick-[AB].out") for x in open(f, errors="replace")]:
    m=re.match(r"=== (\w+) legacy=(\d)", l)
    if m: cur=m.group(1)+("-legacy" if m.group(2)=="1" else ""); secs[cur]=[]; continue
    m=re.match(r"MARK (\d+) (.*)", l.strip())
    if m and cur: secs[cur].append((int(m.group(1)), m.group(2)))
def rows(tag):
    out=[]
    for l in open(f"{S}/flick-{tag}.log", errors="replace"):
        l=re.sub(r'\x1b\[[0-9;]*m','',l).strip()
        m=re.search(r'SURF (\d+) (.*)$', l)
        if m: out.append((int(m.group(1)), m.group(2)))
    return out
want=sys.argv[1:]
for tag in want:
    r=rows(tag); marks=secs.get(tag,[])
    print("=====",tag,len(r),"rows",len(marks),"marks")
    for i,(mt,name) in enumerate(marks):
        if not name.startswith("open") and name!="tooltip": continue
        if len(sys.argv)>2 and sys.argv[2] not in name: pass
        print(f"-- {name}")
        end=mt+600
        for ts,x in r:
            if mt-2<=ts<end and "created" not in x and not ("vis=false" in x and " visible " not in x[:40]):
                print(f"  +{ts-mt:4d}ms {x[:215]}")
