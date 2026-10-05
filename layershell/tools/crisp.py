import sys
from PIL import Image
from collections import Counter

def analyse(path, box, ps, label=""):
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
    print(f"{label} box={box} unique colours={len(colors)} top5={colors.most_common(5)}")
    print(f"  x-transition residues mod {ps}: {dict(sorted(hx.items()))}")
    print(f"  y-transition residues mod {ps}: {dict(sorted(hy.items()))}")

if __name__ == "__main__":
    path = sys.argv[1]
    box = tuple(int(v) for v in sys.argv[2].split(","))
    ps = int(sys.argv[3])
    analyse(path, box, ps, sys.argv[4] if len(sys.argv) > 4 else "")
