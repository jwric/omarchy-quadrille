#!/usr/bin/env python3
"""portrait_check.py [OUT_DIR]

check(pixel, ps, colors, info) verifies, from a rendered PNG alone, every drawn fact of
the two Portrait drawings (the elevation, whose info has `rect`/`scale`, and the locator,
whose info has `rects`): sample each virtual pixel at its top-left physical pixel.
Raises AssertionError with a clear message; returns the measured numbers.

Run as a script it renders both drawings for the laptop and the Dell in all five themes
through element.py, runs check() and crisp.py on every image, and prints a summary."""
import json, re, subprocess, sys
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))

_glyphs = {int(k): v for k, v in re.findall(r'"(\d+)":"([0-9a-f]{24})"', (ROOT / 'plugins/quadrille.bar/Q/Glyphs.js').read_text())}

def text_pixels(text, x, y):
    """The vpx a label of the small face lights, as {(x, y)} (cells 6 wide, 7 bits a row)."""
    out = {}
    for i, ch in enumerate(text):
        rows = _glyphs.get(ord(ch))
        if rows is None:
            raise AssertionError(f'no glyph for {ch!r}')
        for r in range(12):
            bits = int(rows[2 * r:2 * r + 2], 16)
            for c in range(7):
                if bits >> (6 - c) & 1:
                    out[(x + 6 * i + c, y + r)] = i
    return out

def _role_at(pixel, ps, colors, x, y):
    c = pixel[x * ps, y * ps]
    c = tuple(c[:3])
    return [r for r, v in colors.items() if tuple(v) == c]

def _is(pixel, ps, colors, x, y, role):
    return tuple(pixel[x * ps, y * ps][:3]) == tuple(colors[role])

def _lit(pixel, ps, colors, x, y):
    return tuple(pixel[x * ps, y * ps][:3]) != tuple(colors['void'])

def _label_parts(text, role_parts):
    """Per-character role of a label."""
    roles = []
    for part, role in role_parts:
        roles += [role] * len(part)
    assert len(roles) == len(text)
    return roles

def _check_labels(pixel, ps, colors, labels, parts_of, what):
    """Every label's glyphs are in the right colours, nothing else is lit inside its box,
    and the lit pixels of the label are exactly the face's."""
    ink = {}
    for l in labels:
        roles = _label_parts(l['text'], parts_of(l['text']))
        want = text_pixels(l['text'], l['x'], l['y'])
        for (x, y), i in want.items():
            assert _is(pixel, ps, colors, x, y, roles[i]), f'{what}: label {l["text"]!r} pixel ({x},{y}) is not {roles[i]}'
        for y in range(l['y'], l['y'] + l['h']):
            for x in range(l['x'], l['x'] + l['w']):
                if (x, y) not in want:
                    assert not _lit(pixel, ps, colors, x, y), f'{what}: stray pixel ({x},{y}) inside label {l["text"]!r}'
        ink[l['text']] = set(want)
    return ink

def _air(pixel, ps, colors, box, ink_sets, need, what):
    """Smallest Chebyshev distance between any label ink and any other lit pixel in box."""
    labelled = set().union(*ink_sets.values()) if ink_sets else set()
    geometry = [(x, y) for y in range(box['y'] - 6, box['y'] + box['h'] + 6) for x in range(box['x'] - 6, box['x'] + box['w'] + 6)
                if (x, y) not in labelled and _lit(pixel, ps, colors, x, y)]
    best = 99
    for name, pts in ink_sets.items():
        for (x, y) in pts:
            for (gx, gy) in geometry:
                d = max(abs(gx - x), abs(gy - y))
                if d < best:
                    best = d
                assert d >= need, f'{what}: label {name!r} ink at ({x},{y}) is only {d - 1} vpx from a line at ({gx},{gy})'
    return best - 1 if best != 99 else None

def check_elevation(pixel, ps, colors, info):
    R = info['rect']; x0, y0, Wr, Hr = R['x'], R['y'], R['w'], R['h']
    N = info['N']
    P = lambda x, y, role: _is(pixel, ps, colors, x, y, role)
    what = f'elevation 1:{N}'
    # True scale: the outline's length in vpx is the physical size over N, to half a vpx.
    mmx = info['onScreenWidthMm'] / Wr; mmy = info['onScreenHeightMm'] / Hr
    assert abs(Wr * mmx * N - info['widthMm']) <= N * mmx / 2 + 1e-9, f'{what}: width not to scale'
    assert abs(Hr * mmy * N - info['heightMm']) <= N * mmy / 2 + 1e-9, f'{what}: height not to scale'
    assert info['scale'] == f'1:{N}'
    # The outline: muted columns exactly Wr apart on a row clear of the circle, rows Hr apart on a column clear of it.
    row = [x for x in range(x0 - 2, x0 + Wr + 3) if P(x, y0 + 1, 'muted')]
    assert row == [x0, x0 + Wr], f'{what}: outline columns on row y0+1 are {row}, want {[x0, x0 + Wr]}'
    col = [y for y in range(y0 - 2, y0 + Hr + 3) if P(x0 + 1, y, 'muted')]
    assert col == [y0, y0 + Hr], f'{what}: outline rows on column x0+1 are {col}, want {[y0, y0 + Hr]}'
    for x in range(x0, x0 + Wr + 1):
        assert P(x, y0, 'muted') and P(x, y0 + Hr, 'muted'), f'{what}: top/bottom line broken at x={x}'
    for y in range(y0, y0 + Hr + 1):
        assert P(x0, y, 'muted') and P(x0 + Wr, y, 'muted'), f'{what}: side line broken at y={y}'
    # Width dimension: extension lines 2 below the corners to 3 past the line, arrowheads, the line.
    D = info['dimension']['widthRow']; Dx = info['dimension']['heightColumn']
    assert D == y0 + Hr + 8 and Dx == x0 + Wr + 8
    for ex in (x0, x0 + Wr):
        assert not P(ex, y0 + Hr + 1, 'line'), f'{what}: extension line starts too soon at x={ex}'
        for y in range(y0 + Hr + 2, D + 4):
            assert P(ex, y, 'line'), f'{what}: extension line gap at ({ex},{y})'  # unbroken through the dimension row
        assert not P(ex, D + 4, 'line'), f'{what}: extension line runs past 3 vpx'
    def head(tx, ty, dx, dy):
        """The solid 45 degree head 3 deep with its tip at (tx, ty), pointing (dx, dy), and
        nothing else round it (the extension line crosses at the tip, the dimension line
        leaves from the middle of the base)."""
        for k in range(-1, 4):
            for j in range(-3, 4):
                x, y = (tx - dx * k, ty + j) if dx else (tx + j, ty - dy * k)
                if k == -1:
                    want = None                          # the extension line, checked on its own
                elif k == 0:
                    want = j == 0
                elif k == 3:
                    want = j == 0
                else:
                    want = abs(j) <= k
                if want is None:
                    continue
                assert P(x, y, 'line') == want, f'{what}: arrowhead at ({tx},{ty}) wrong at ({x},{y}): {"missing" if want else "stray"}'
    head(x0 + 1, D, -1, 0); head(x0 + Wr - 1, D, 1, 0)
    for x in range(x0 + 4, x0 + Wr - 3):
        assert P(x, D, 'line'), f'{what}: dimension line gap at x={x}'
    # Height dimension.
    for ey in (y0, y0 + Hr):
        assert not P(x0 + Wr + 1, ey, 'line'), f'{what}: height extension line starts too soon'
        for x in range(x0 + Wr + 2, Dx + 4):
            assert P(x, ey, 'line'), f'{what}: height extension line gap at ({x},{ey})'
        assert not P(Dx + 4, ey, 'line'), f'{what}: height extension line runs past 3 vpx'
    head(Dx, y0 + 1, 0, -1); head(Dx, y0 + Hr - 1, 0, 1)
    for y in range(y0 + 4, y0 + Hr - 3):
        assert P(Dx, y, 'line'), f'{what}: height dimension line gap at y={y}'
    # The sheet's star at this scale, where the screen centres it.
    star = info['star']
    if star:
        cx, cy, rx, ry = star['cx'], star['cy'], star['rx'], star['ry']
        assert (cx, cy) == (x0 + int(Wr / 2 + 0.5), y0 + int(Hr / 2 + 0.5)), f'{what}: star not at the screen centre'
        for (x, y) in ((cx - rx, cy), (cx + rx, cy), (cx, cy - ry), (cx, cy + ry)):
            assert P(x, y, 'line'), f'{what}: circle missing at its compass point ({x},{y})'
        assert not P(cx, cy, 'line') and not P(cx - rx - 1, cy, 'line') and not P(cx + rx + 1, cy, 'line'), f'{what}: circle not one pixel thick'
    # The words: the right glyphs in the right colours, nothing else in their boxes.
    labels = info['labels']
    want_text = ['ACTIVE AREA  1:%d' % N, info['widthText'], info['heightText']] + (['ESTIMATED'] if info['estimated'] else [])
    assert [l['text'] for l in labels] == want_text, f'{what}: labels {[l["text"] for l in labels]} != {want_text}'
    est = info['estimated']
    if est:
        assert '.' not in info['widthText'] and '.' not in info['heightText'], f'{what}: inferred size printed with decimals'
    else:
        assert re.fullmatch(r'\d+\.\d mm', info['widthText']) and re.fullmatch(r'\d+\.\d mm', info['heightText']), f'{what}: measured size is not one decimal'
        assert '~' not in info['widthText']
    for t, mm in ((info['widthText'], info['widthMm']), (info['heightText'], info['heightMm'])):
        v = float(t.replace('~', '').replace(' mm', ''))
        assert abs(v - mm) <= (0.05 if not est else 0.5) + 1e-9, f'{what}: printed {t} but the size is {mm}'
    parts_of = lambda t: [(t[:11], 'ink'), (t[11:], 'muted')] if t.startswith('ACTIVE AREA') else [(t, 'muted')]
    ink = _check_labels(pixel, ps, colors, labels, parts_of, what)
    # ESTIMATED exactly when required, in muted pixels, left-aligned with the outline; absent otherwise.
    wl = next(l for l in labels if l['text'] == info['widthText'])
    row_box = {'x': x0, 'y': wl['y'] + 16, 'w': 55, 'h': 12}
    lit = [(x, y) for y in range(row_box['y'], row_box['y'] + 12) for x in range(row_box['x'], row_box['x'] + 55) if _lit(pixel, ps, colors, x, y)]
    if est:
        es = next(l for l in labels if l['text'] == 'ESTIMATED')
        assert es['x'] == x0 and es['y'] == row_box['y'], f'{what}: ESTIMATED not left-aligned under the width value'
        assert lit and all(P(x, y, 'muted') for x, y in lit), f'{what}: ESTIMATED row is not muted pixels'
    else:
        assert not lit, f'{what}: something is drawn where ESTIMATED would go on a measured size'
    air = _air(pixel, ps, colors, info['box'], ink, 3, what)
    # The height value keeps >= 4 vpx of air to its extension lines and dimension line.
    _air(pixel, ps, colors, info['box'], {info['heightText']: ink[info['heightText']]}, 5, what + ' height value')
    return {'scale': info['scale'], 'outline_w_vpx': Wr, 'outline_h_vpx': Hr, 'widthText': info['widthText'], 'heightText': info['heightText'],
            'estimated': est, 'min_air_vpx': air}

def check_locator(pixel, ps, colors, info):
    what = 'locator'
    P = lambda x, y, role: _is(pixel, ps, colors, x, y, role)
    assert colors['accent'] != colors['faint']
    for q in info['rects']:
        role = 'accent' if q['current'] else 'faint'
        x, y, w, h = q['x'], q['y'], q['w'], q['h']
        for i in range(w):
            assert P(x + i, y, role) and P(x + i, y + h - 1, role), f'{what}: outline {q["index"]} broken on a row at x={x + i}'
        for j in range(h):
            assert P(x, y + j, role) and P(x + w - 1, y + j, role), f'{what}: outline {q["index"]} broken on a column at y={y + j}'
        assert sum(1 for q2 in info['rects'] if q2['current']) == 1
    labels = []
    for n in info['numbers']:
        q = info['rects'][n['index']]
        labels.append({'text': str(n['value']), 'x': n['x'], 'y': n['y'], 'w': 6 * len(str(n['value'])) + 1, 'h': 12, 'role': 'accent' if q['current'] else 'faint'})
        assert n['value'] == n['index'] + 1
    for l in labels:
        _check_labels(pixel, ps, colors, [l], lambda t, r=l['role']: [(t, r)], what)
    if info['caption']:
        c = info['caption']; assert c['text'] == 'DESKTOP LAYOUT'
        _check_labels(pixel, ps, colors, [c], lambda t: [(t, 'muted')], what)
    return {'rects': len(info['rects']), 'numbers': len(info['numbers']), 'current': next(q['index'] for q in info['rects'] if q['current'])}

def check(pixel, ps, colors, info):
    return check_elevation(pixel, ps, colors, info) if 'scale' in info else check_locator(pixel, ps, colors, info)

THEMES = ['terminal', 'paper', 'phosphor', 'amber', 'lcd']

def main():
    from element import roles
    out = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/quadrille-aperture/Portrait')
    out.mkdir(parents=True, exist_ok=True)
    failures = 0
    for monitor in ('laptop', 'dell'):
        for element, cfg in (('elevation', {'element': 'elevation'}), ('locator', {'element': 'locator', 'maxW': 96, 'maxH': 56, 'caption': True})):
            prefix = out / f'{element}-{monitor}'
            r = subprocess.run([sys.executable, str(HERE / 'element.py'), 'Portrait', str(prefix), '--monitor', monitor, '--layout', 'real',
                                '--cfg', json.dumps(cfg), '--themes', ','.join(THEMES), '--zoom', '4'], capture_output=True, text=True)
            if r.returncode:
                failures += 1; print(f'FAIL {element} {monitor}: element.py exited {r.returncode}\n{r.stdout}\n{r.stderr}'); continue
            res = json.loads(r.stdout.strip().splitlines()[0]); info = res['result']; ps = res['ps']
            for theme in THEMES:
                path = f'{prefix}-{theme}.png'
                im = Image.open(path).convert('RGB')
                try:
                    got = check(im.load(), ps, roles(theme), info)
                    c = subprocess.run([sys.executable, str(ROOT / 'layershell/tools/crisp.py'), path, f'0,0,{im.width},{im.height}', str(ps), '--check'], capture_output=True, text=True)
                    assert c.returncode == 0, 'crisp.py: ' + (c.stdout + c.stderr).strip().splitlines()[-1]
                    print(f'ok   {element:9} {monitor:6} {theme:8} {json.dumps(got)}')
                except AssertionError as e:
                    failures += 1; print(f'FAIL {element:9} {monitor:6} {theme:8} {e}')
    print('portrait_check:', 'FAILED' if failures else 'all ok')
    sys.exit(1 if failures else 0)

if __name__ == '__main__':
    main()
