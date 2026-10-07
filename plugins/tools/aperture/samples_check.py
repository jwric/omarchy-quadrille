#!/usr/bin/env python3
"""Check the Samples element (PATTERN -> RESULT) from a rendered PNG alone.

check(pixel, ps, colors, info) -> dict verifies, sampling every virtual pixel at its
top-left physical pixel, and against an independent Python rebuild (exact fractions)
  * every enlarged column of each PATTERN view is the grating at X/6 (stripe widths 6, 4, 3);
  * the sample ticks: 2 vpx of `line` under the view at every cell's left edge, and nothing
    else around the view;
  * each RESULT view equals an independent point-sampling of the grating at integer vpx,
    enlarged 6 times (n=2 alternates, 4/3 has period 4 in bands of 2, n=1 is flat);
  * the 1:1 patch, when present, is the same cells at their real size;
  * the arrow is a 10 wide `line` shaft and a solid 3 deep head, centred, and 4+ clear of both
    views, and the bracket spans one ink and one void stripe of the first pattern;
  * every label's box holds only void and its own role's colour; nothing else in the element's
    box is anything but void, and no pixel is anything but a theme role colour.
Run as a script it renders laptop and dell in all five themes and checks each (plus crisp.py)."""
import json, subprocess, sys, tempfile
from fractions import Fraction
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
ROOT = HERE.parents[2]
OUT = Path('/tmp/quadrille-aperture/Samples')
CASES = {'2': (Fraction(2), 0), '4/3': (Fraction(4, 3), 3), '1': (Fraction(1), 0)}
E = 6

def grating(n, phase, t):
    """The role of the grating of period n (vpx), shifted by phase, at t (a Fraction)."""
    return 'ink' if int((2 * (t + phase) / n) // 1) % 2 == 0 else 'void'

def check(pixel, ps, colors, info):
    def at(x, y): return pixel[int(x * ps), int(y * ps)]
    def name(c):
        n = [k for k, v in colors.items() if tuple(v) == tuple(c)]
        return '/'.join(n) or str(tuple(c))
    def expect(x, y, role, what):
        got = at(x, y)
        assert tuple(got) == tuple(colors[role]), f'{what} at ({x},{y}): want {role} {colors[role]}, got {name(got)} {got}'
    role_colors = {tuple(v) for v in colors.values()}; void = tuple(colors['void'])
    claimed = {}
    def claim(x, y, what):
        assert (x, y) not in claimed, f'{what} at ({x},{y}) overlaps {claimed[(x, y)]}'
        claimed[(x, y)] = what
    cells = info['cells']; assert info['factor'] == E
    out = {'cases': {}}
    for c in info['cases']:
        n, phase = CASES[c['n']]; tag = f"n={c['n']}"
        p, r = c['pattern'], c['result']
        assert p['w'] == cells * E and p['h'] == 18 and r['w'] == cells * E and r['h'] == 18, f'{tag}: view sizes'
        # Frames: one vpx of `edge` outline, one vpx outside each view.
        def frame_check(v, what):
            f = v['frame']; assert f == {'x': v['x'] - 1, 'y': v['y'] - 1, 'w': v['w'] + 2, 'h': v['h'] + 2}, f'{tag}: {what} frame box {f}'
            for i in range(f['w']):
                for y in (f['y'], f['y'] + f['h'] - 1): expect(f['x'] + i, y, 'edge', f'{tag} {what} frame'); claim(f['x'] + i, y, f'{tag} {what} frame')
            for j in range(1, f['h'] - 1):
                for x in (f['x'], f['x'] + f['w'] - 1): expect(x, f['y'] + j, 'edge', f'{tag} {what} frame'); claim(x, f['y'] + j, f'{tag} {what} frame')
        frame_check(p, 'pattern'); frame_check(r, 'result')
        # PATTERN: the grating sampled E times finer than the grid, every enlarged column.
        want_cols = [grating(n, phase, Fraction(X, E)) for X in range(p['w'])]
        for X in range(p['w']):
            for dy in range(18):
                expect(p['x'] + X, p['y'] + dy, want_cols[X], f'{tag} pattern column {X} row {dy}'); claim(p['x'] + X, p['y'] + dy, tag + ' pattern')
        runs, s = [], 0
        for X in range(1, p['w'] + 1):
            if X == p['w'] or want_cols[X] != want_cols[s]: runs.append(X - s); s = X
        width = {'2': 6, '4/3': 4, '1': 3}[c['n']]
        assert all(w == width for w in runs[1:-1]) and len(runs) >= 4, f'{tag}: pattern stripe widths {runs}'
        # Ticks: 2 vpx of `line` under every cell's left edge, a vpx clear of the view.
        assert c['ticks'] == [p['x'] + k * E for k in range(cells)], f'{tag}: tick positions'
        # (the ticks start a vpx below the frame)
        for tx in range(p['x'], p['x'] + p['w']):
            on = (tx - p['x']) % E == 0
            expect(tx, p['y'] + 19, 'void', f'{tag} gap under the frame'); claim(tx, p['y'] + 19, tag + ' gap')
            for dy in (20, 21):
                expect(tx, p['y'] + dy, 'line' if on else 'void', f'{tag} tick row {dy}'); claim(tx, p['y'] + dy, tag + ' tick')
        # RESULT: independent point-sampling at each integer vpx, enlarged E times.
        want = [grating(n, phase, Fraction(k)) for k in range(cells)]
        assert c['result']['cells'] == want, f'{tag}: info result cells {c["result"]["cells"]} != {want}'
        for k in range(cells):
            for dx in range(E):
                for dy in range(18):
                    expect(r['x'] + k * E + dx, r['y'] + dy, want[k], f'{tag} result cell {k}'); claim(r['x'] + k * E + dx, r['y'] + dy, tag + ' result')
        if c['n'] == '2': assert all(want[k] != want[k - 1] for k in range(1, cells)), 'n=2 must alternate'
        if c['n'] == '4/3': assert all(want[k] == want[k % 4] for k in range(cells)) and want[0] == want[1] != want[2] == want[3], 'n=4/3: period 4, bands of 2'
        if c['n'] == '1': assert len(set(want)) == 1, 'n=1 must be flat'
        # 1:1 patch.
        if c['actual']:
            a = c['actual']; assert (a['w'], a['h']) == (cells, 18) and a['x'] - (r['x'] + r['w']) == 7, f'{tag}: 1:1 box'
            frame_check(a, '1:1')
            for k in range(cells):
                for dy in range(18):
                    expect(a['x'] + k, a['y'] + dy, want[k], f'{tag} 1:1 cell {k}'); claim(a['x'] + k, a['y'] + dy, tag + ' 1:1')
        # Arrow: 10 shaft, 3 deep solid head, centred, 4+ clear of both views.
        ar = c['arrow']; assert ar['y'] == p['y'] + 8 and ar['x1'] == ar['x0'] + 12
        assert ar['x0'] - (p['frame']['x'] + p['frame']['w']) >= 4 and r['frame']['x'] - (ar['x1'] + 1) >= 4, f'{tag}: arrow air from the frames'
        arrow = {(ar['x0'] + i, ar['y']) for i in range(10)}
        arrow |= {(ar['x1'] - k, ar['y'] + d) for k in range(3) for d in range(-k, k + 1)}
        for (ax, ay) in arrow: expect(ax, ay, 'line', f'{tag} arrow'); claim(ax, ay, tag + ' arrow')
        out['cases'][c['n']] = {'text': c['text'], 'runs': runs[:4], 'result': ''.join('#' if v == 'ink' else '.' for v in want)}
    # Bracket over the first pair of the first pattern view.
    b = info['bracket']; p0 = info['cases'][0]['pattern']
    assert b['x1'] - b['x0'] + 1 == 12 and b['x0'] == p0['x'] and b['y'] == p0['y'] - 5, f'bracket box {b}'
    for x in range(b['x0'], b['x1'] + 1): expect(x, b['y'], 'line', 'bracket bar'); claim(x, b['y'], 'bracket')
    for x in (b['x0'], b['x1']): expect(x, b['y'] + 1, 'line', 'bracket tick'); claim(x, b['y'] + 1, 'bracket')
    first = [at(b['x0'] + i, p0['y']) for i in range(12)]
    assert all(tuple(v) == tuple(colors['ink']) for v in first[:6]) and all(tuple(v) == tuple(colors['void']) for v in first[6:]), 'bracket must span one ink and one void stripe'
    # Labels.
    for l in info['labels']:
        ink = 0
        for y in range(l['y'], l['y'] + l['h']):
            for x in range(l['x'], l['x'] + l['w']):
                col = tuple(at(x, y))
                assert col in (void, tuple(colors[l['role']])), f'label "{l["text"]}" at ({x},{y}): {name(col)} is neither void nor {l["role"]}'
                if col != void:
                    ink += 1; assert (x, y) not in claimed, f'label "{l["text"]}" touches {claimed[(x, y)]} at ({x},{y})'
        assert ink >= len(l['text']), f'label "{l["text"]}" is not drawn'
        for y in range(l['y'], l['y'] + l['h']):
            for x in range(l['x'], l['x'] + l['w']): claimed.setdefault((x, y), 'label ' + l['text'])
    texts = [l['text'] for l in info['labels']]
    for t in ('PAIRS', 'PER mm', 'PATTERN', 'RESULT 6:1', '1 PAIR', 'RESOLVED', 'FALSE DETAIL', 'ALIASED', 'DETAIL LOST'): assert t in texts, f'label {t} missing'
    # The element's box: only role colours, and void outside everything claimed.
    stray = 0
    for y in range(info['y'], info['y'] + info['h']):
        for x in range(info['x'], info['x'] + info['w']):
            col = tuple(at(x, y)); assert col in role_colors, f'({x},{y}) is {col}: not a role colour'
            if (x, y) not in claimed: assert col == void, f'({x},{y}) outside every drawing is {name(col)}, not void'; stray += 1
    out.update({'numbers': info['numbers'], 'box': [info['w'], info['h']], 'void': stray, 'actual': info['cases'][0]['actual'] is not None})
    return out

def render(monitor, theme, cfg):
    import element
    prefix = str(OUT / f'{monitor}-{"actual" if cfg.get("actual") else "compact" if cfg.get("compact") else "plain"}')
    r = subprocess.run([sys.executable, str(HERE / 'element.py'), 'Samples', prefix, '--monitor', monitor, '--layout', 'real',
                        '--cfg', json.dumps(cfg), '--themes', theme, '--zoom', '2'], capture_output=True, text=True)
    assert r.returncode == 0, f'element.py failed: {r.stdout}{r.stderr}'
    d = json.loads(r.stdout.strip().splitlines()[0])
    assert not d['dropped'] and d['outside'] == 0
    return d, Path(f'{prefix}-{theme}.png'), element.roles(theme)

if __name__ == '__main__':
    bad = 0
    for monitor in ('laptop', 'dell'):
        for cfg in ({}, {'actual': True}, {'compact': True}):
            for theme in ['terminal', 'paper', 'phosphor', 'amber', 'lcd']:
                try:
                    d, png, colors = render(monitor, theme, cfg)
                    im = Image.open(png).convert('RGB'); ps = d['ps']
                    res = check(im.load(), ps, colors, d['result'])
                except AssertionError as e:
                    print(f'FAIL {monitor}/{cfg}/{theme}: {e}'); bad += 1; continue
                c = subprocess.run([sys.executable, str(ROOT / 'layershell/tools/crisp.py'), str(png), f'0,0,{im.width},{im.height}', str(ps), '--check'], capture_output=True, text=True)
                if c.returncode: print(f'FAIL crisp {monitor}/{cfg}/{theme}: {c.stdout}{c.stderr}'); bad += 1; continue
                print(f'ok   {monitor:6} {"actual" if "actual" in cfg else "compact" if cfg else "plain":7} {theme:8} ps={ps} box={res["box"]} numbers={res["numbers"]} results={[v["result"] for v in res["cases"].values()]}')
    print('all checks passed' if not bad else f'{bad} FAILED')
    sys.exit(1 if bad else 0)
