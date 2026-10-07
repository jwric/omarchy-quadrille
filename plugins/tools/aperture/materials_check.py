#!/usr/bin/env python3
"""Check the Materials strip from a rendered PNG alone.

check(pixel, ps, colors, info) -> dict verifies, pixel by pixel and against an
independent rebuild (its own Bayer matrix, bands, hatch, grid and dots), that
  * every flat half is uniform in its role, every chip is flat;
  * every surface texture is the ramp from its role toward the next surface in the
    three bands at Bayer levels 4, 8, 12, anchored at the specimen's top-left;
  * every ink texture is its one-bit pattern on void, anchored the same way;
  * every surface has its one-pixel edge frame, and nothing else but void lies
    outside the specimens, frames and labels (so nothing is stray);
  * role names are muted, group names ink, and each label box holds only void and
    its role's colour;
  * no pixel of the strip is anything but one of the theme's role colours.
Run as a script it renders laptop and dell, row and stack, in all five themes, and
checks each (plus crisp.py)."""
import json, subprocess, sys, tempfile
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
ROOT = HERE.parents[2]
BAYER = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
NEXT = {'void': 'ground', 'ground': 'raised', 'raised': 'hover', 'hover': 'edge'}
INK_ROLES = ['edge', 'line', 'faint', 'muted', 'ink']
OUT = Path('/tmp/quadrille-aperture/Materials')

def lit(i, j, level): return BAYER[j % 4][i % 4] < level
def ink_pattern(role, i, j):
    return {'edge': lambda: i % 4 == 0 or j % 4 == 0, 'line': lambda: (i + j) % 4 == 0,
            'faint': lambda: i % 3 == 0 and j % 3 == 0, 'muted': lambda: lit(i, j, 4),
            'ink': lambda: lit(i, j, 8)}[role]()

def check(pixel, ps, colors, info):
    def at(x, y): return pixel[int(x * ps), int(y * ps)]
    def name(c):
        n = [k for k, v in colors.items() if tuple(v) == tuple(c) and k != 'highlight']
        return '/'.join(n) or str(tuple(c))
    def expect(x, y, role, what):
        got = at(x, y)
        assert tuple(got) == tuple(colors[role]), f'{what} at ({x},{y}): want {role} {colors[role]}, got {name(got)} {got}'
    role_colors = {tuple(v) for v in colors.values()}
    void = tuple(colors['void'])
    claimed = {}   # (x, y) -> what claims it
    def claim(x, y, what):
        assert (x, y) not in claimed, f'{what} at ({x},{y}) overlaps {claimed[(x, y)]}'
        claimed[(x, y)] = what
    n_pix = {'flat': 0, 'texture': 0, 'frame': 0}
    bands_seen = set()
    for s in info['specimens']:
        w, h, x0, y0, role, grp = s['w'], s['h'], s['x'], s['y'], s['role'], s['group']
        tag = f"{grp}/{role}"
        f = s['flat']
        assert (f['x'], f['y']) == (x0, y0) and f['w'] == w, f'{tag}: flat box'
        for y in range(f['y'], f['y'] + f['h']):
            for x in range(f['x'], f['x'] + f['w']):
                expect(x, y, role, f'{tag} flat'); claim(x, y, tag); n_pix['flat'] += 1
        t = s['texture']
        if grp == 'signals':
            assert t is None and f['h'] == h, f'{tag}: chip is flat all through'
            continue
        assert t['x'] == x0 and t['w'] == w and t['y'] == y0 + 12 and t['h'] == h - 12 == 12, f'{tag}: texture box'
        for j in range(t['h']):
            for i in range(w):
                if grp == 'surfaces':
                    band = min(2, (3 * i) // w); bands_seen.add(band)
                    want = NEXT[role] if lit(i, j, (4, 8, 12)[band]) else role
                else:
                    want = role if ink_pattern(role, i, j) else 'void'
                expect(x0 + i, t['y'] + j, want, f'{tag} texture'); claim(x0 + i, t['y'] + j, tag); n_pix['texture'] += 1
        if grp == 'surfaces':
            fr = s['frame']
            assert fr == {'x': x0 - 1, 'y': y0 - 1, 'w': w + 2, 'h': h + 2}, f'{tag}: frame box {fr}'
            for i in range(fr['w']):
                for y in (fr['y'], fr['y'] + fr['h'] - 1):
                    expect(fr['x'] + i, y, 'edge', f'{tag} frame'); claim(fr['x'] + i, y, tag + ' frame'); n_pix['frame'] += 1
            for j in range(1, fr['h'] - 1):
                for x in (fr['x'], fr['x'] + fr['w'] - 1):
                    expect(x, fr['y'] + j, 'edge', f'{tag} frame'); claim(x, fr['y'] + j, tag + ' frame'); n_pix['frame'] += 1
    if any(s['group'] == 'surfaces' for s in info['specimens']):
        assert bands_seen == {0, 1, 2}, f'ramp bands seen {bands_seen}'
    # labels: only void and their own role's colour inside the box, and some of it
    def label(box, text, role):
        ink_px = 0
        for y in range(box['y'], box['y'] + box['h']):
            for x in range(box['x'], box['x'] + box['w']):
                c = tuple(at(x, y))
                assert c in (void, tuple(colors[role])), f'label "{text}" at ({x},{y}): {name(c)} is neither void nor {role}'
                if c == tuple(colors[role]) and c != void: ink_px += 1
                if (x, y) in claimed and c != void: raise AssertionError(f'label "{text}" touches {claimed[(x, y)]} at ({x},{y})')
        assert ink_px >= len(text), f'label "{text}" is not drawn ({ink_px} px)'
        for y in range(box['y'], box['y'] + box['h']):
            for x in range(box['x'], box['x'] + box['w']): claimed.setdefault((x, y), 'label ' + text)
    for g in info['groups']: label({'x': g['x'], 'y': g['y'], 'w': g['w'], 'h': g['h']}, g['name'], 'ink')
    for s in info['specimens']:
        l = s['label']; label(l, s['role'], 'muted')
    # the strip: every pixel a theme role colour, and outside the claimed pixels void
    ox = min(g['x'] for g in info['groups']); oy = min(g['y'] for g in info['groups'])
    stray = 0
    for y in range(oy, oy + info['h']):
        for x in range(ox, ox + info['w']):
            c = tuple(at(x, y))
            assert c in role_colors, f'({x},{y}) is {c}: not a role colour (blended?)'
            if (x, y) not in claimed:
                assert c == void, f'({x},{y}) outside every specimen/label is {name(c)}, not void'; stray += 1
    return {'specimens': len(info['specimens']), **n_pix, 'void_between': stray, 'strip': [info['w'], info['h']]}

def render(monitor, layout, theme, extra=None):
    import element
    cfg = {'layout': layout, **(extra or {})}
    with tempfile.TemporaryDirectory() as t:
        prefix = str(OUT / f'{monitor}-{layout}')
        r = subprocess.run([sys.executable, str(HERE / 'element.py'), 'Materials', prefix, '--monitor', monitor, '--layout', 'real',
                            '--cfg', json.dumps(cfg), '--themes', theme, '--zoom', '2'], capture_output=True, text=True)
    assert r.returncode == 0, f'element.py failed: {r.stdout}{r.stderr}'
    d = json.loads(r.stdout.strip().splitlines()[0])
    assert not d['dropped'] and d['outside'] == 0
    return d, Path(f'{prefix}-{theme}.png'), element.roles(theme)

if __name__ == '__main__':
    themes = ['terminal', 'paper', 'phosphor', 'amber', 'lcd']
    bad = 0
    for monitor in ('laptop', 'dell'):
        for layout in ('row', 'stack'):
            for theme in themes:
                d, png, colors = render(monitor, layout, theme)
                im = Image.open(png).convert('RGB'); ps = d['ps']
                try:
                    res = check(im.load(), ps, colors, d['result'])
                except AssertionError as e:
                    print(f'FAIL {monitor}/{layout}/{theme}: {e}'); bad += 1; continue
                c = subprocess.run([sys.executable, str(ROOT / 'layershell/tools/crisp.py'), str(png), f'0,0,{im.width},{im.height}', str(ps), '--check'],
                                   capture_output=True, text=True)
                if c.returncode: print(f'FAIL crisp {monitor}/{layout}/{theme}: {c.stdout}{c.stderr}'); bad += 1; continue
                print(f'ok   {monitor:6} {layout:5} {theme:8} ps={ps} strip={res["strip"]} flat={res["flat"]} texture={res["texture"]} frame={res["frame"]} void={res["void_between"]}')
    print('all checks passed' if not bad else f'{bad} FAILED')
    sys.exit(1 if bad else 0)
