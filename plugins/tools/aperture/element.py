#!/usr/bin/env python3
"""element.py MODULE OUT_PREFIX [--monitor laptop] [--layout real] [--cfg JSON] [--themes terminal,paper] [--zoom 2]

Draws one Aperture element module alone (element.js), paints it exactly as the sheet's
Canvas does (each stroke a rectangle of whole physical pixels in its theme role), and
writes OUT_PREFIX-<theme>.png (and -zoom.png enlarged nearest-neighbour when --zoom > 1).
Exit status 1 when the element leaves its measured box or drops a label."""
import argparse, json, subprocess, sys, tomllib, tempfile
from pathlib import Path
from PIL import Image, ImageDraw
HERE = Path(__file__).resolve().parent; ROOT = HERE.parents[2]
def roles(theme):
    r = tomllib.loads((ROOT / f'themes/quadrille-{theme}/shell.toml').read_text())['quadrille']
    return {k: tuple(bytes.fromhex(v.lstrip('#'))) for k, v in r.items() if isinstance(v, str)}
def paint(plan, col, outline=False):
    ps = plan['ps']; im = Image.new('RGB', (plan['pixelWidth'], plan['pixelHeight']), col['void']); dr = ImageDraw.Draw(im)
    for x, y, w, h, r in plan['strokes']:
        dr.rectangle([x*ps, y*ps, (x+w)*ps-1, (y+h)*ps-1], fill=col[r])
    return im
def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('module'); ap.add_argument('out'); ap.add_argument('--monitor', default='laptop')
    ap.add_argument('--layout', default='single'); ap.add_argument('--cfg', default='{}'); ap.add_argument('--star', default='{}')
    ap.add_argument('--themes', default='terminal'); ap.add_argument('--zoom', type=int, default=1); ap.add_argument('--margin', type=int, default=16)
    a = ap.parse_args()
    with tempfile.TemporaryDirectory() as t:
        spec = Path(t) / 'spec.json'; spec.write_text(json.dumps({'monitor': a.monitor, 'layout': a.layout, 'cfg': json.loads(a.cfg), 'star': json.loads(a.star), 'margin': a.margin}))
        out = Path(t) / 'plan.json'
        r = subprocess.run(['node', str(HERE / 'element.js'), a.module, str(out), str(spec)], capture_output=True, text=True)
        print(r.stdout.strip()); print(r.stderr.strip(), file=sys.stderr)
        plan = json.loads(out.read_text())[0]
    for theme in a.themes.split(','):
        im = paint(plan, roles(theme)); p = Path(f'{a.out}-{theme}.png'); p.parent.mkdir(parents=True, exist_ok=True); im.save(p)
        if a.zoom > 1: im.resize((im.width*a.zoom, im.height*a.zoom), Image.NEAREST).save(f'{a.out}-{theme}-zoom.png')
    sys.exit(r.returncode)
if __name__ == '__main__':
    main()
