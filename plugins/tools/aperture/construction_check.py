#!/usr/bin/env python3
"""Checks Construction.js (B, the exploded pixel construction) from a rendered PNG alone.

    check(pixel, ps, colors, info, star_pixel=None) -> dict of measured numbers

pixel: PIL pixel access of the image; each virtual pixel is sampled at its top-left physical
pixel. colors: {role: (r, g, b)}. info: draw()'s return value (sheet vpx). star_pixel(x, y):
the colour of the sheet's own pixel at sheet vpx (x, y), to check that the coarse view IS
the star's piece and that the window's marker is drawn (used when a sheet PNG is checked).

    construction_check.py     renders the figure for the laptop and the Dell in all five
                              themes with element.py and checks each, plus crispness.
"""
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(HERE))

SLAB = 3


def expected_iso(info):
    """{(x, y): role} of the exploded pixel, rebuilt from the spec (not from the JS):
    a 2:1 isometric rhombus of side s (4 s wide, 2 s tall; rows widen two pixels each side
    from a four pixel top to the two widest rows), a slab of 3 under the tile, the panel
    pixels on the floor as ps x ps cells."""
    s, ps = info['tile']['s'], info['panel']['n']
    out = {}

    def rhombus_mask(ox, oy):
        rows = {}
        for r in range(2 * s):
            half = 2 * (r if r < s else 2 * s - 1 - r) + 2
            rows[r] = (ox - half, ox + half)           # [lo, hi)
        return rows

    # ---- the tile (row 1)
    ox, oy = info['tile']['x'], info['tile']['y']
    for r, (lo, hi) in rhombus_mask(ox, oy).items():
        for x in range(lo, hi):
            out[(x, oy + r)] = info['tile']['top']
    for x in range(ox - 2 * s, ox):                     # left face
        k = (x - (ox - 2 * s)) // 2
        for y in range(oy + s + k + 1, oy + s + k + 1 + SLAB):
            out[(x, y)] = info['tile']['left']
    for x in range(ox, ox + 2 * s):                     # right face
        k = (ox + 2 * s - 1 - x) // 2
        for y in range(oy + s + k + 1, oy + s + k + 1 + SLAB):
            out[(x, y)] = info['tile']['right']

    # ---- the projection lines: 1 vpx, dashed [1, 1], the outer columns of the corners
    pj = info['projection']
    for col in (pj['left'], pj['right']):
        for y in range(pj['y0'], pj['y1'] + 1):
            if (y - pj['y0']) % 2 == 0:
                out[(col, y)] = 'line'

    # ---- the block (row 2): cells, void gaps along both iso axes, outline in line
    bx, by = info['panel']['x'], info['panel']['y']
    mask = rhombus_mask(bx, by)
    inside = lambda x, y: (y - by) in mask and mask[y - by][0] <= x < mask[y - by][1]
    t = s // ps
    for r, (lo, hi) in mask.items():
        for x in range(lo, hi):
            a, b = x - bx, r                             # offsets from the top vertex
            role = info['panel']['fill']
            # gap lines along the u axis (a - 2 b = -4 i t + {0, 1}) and the v axis (a + 2 b = 4 i t - {1, 2})
            for i in range(1, ps):
                k = b - i * t
                if 0 <= k < s and (a - 2 * b + 4 * i * t) in (0, 1):
                    role = info['panel']['gap']
                k = b - i * t
                if 0 <= k < s and (a + 2 * b - 4 * i * t) in (-1, -2):
                    role = info['panel']['gap']
            # the outline: the rhombus's own pixels that touch the outside
            if not all(inside(x + dx, r + dy + by) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                role = info['panel']['edge']
            out[(x, by + r)] = role
    return out


def sample(pixel, ps, x, y):
    return tuple(pixel[x * ps, y * ps][:3])


def check(pixel, ps, colors, info, star_pixel=None):
    got = {}
    col = lambda role: tuple(colors[role])
    n, F, b = info['cells'], info['factor'], info['block']
    assert F == b * ps, f'factor {F} != block {b} x ps {ps}'

    def at(x, y):
        return sample(pixel, ps, x, y)

    # ---- the two views: every block uniform, in the right role
    def view(name, v, unit, count):
        assert v['size'] == count * unit, f'{name}: size {v["size"]} != {count} x {unit}'
        used = {}
        for j in range(count):
            for i in range(count):
                want = col(v['roles'][j][i])
                for dy in range(unit):
                    for dx in range(unit):
                        x, y = v['x'] + i * unit + dx, v['y'] + j * unit + dy
                        assert at(x, y) == want, (f'{name}: block ({i},{j}) pixel ({x},{y}) is {at(x, y)}, '
                                                  f'expected {v["roles"][j][i]} {want}')
                used[v['roles'][j][i]] = used.get(v['roles'][j][i], 0) + 1
        # the frame, one vpx outside the view
        x0, y0, s = v['x'] - 1, v['y'] - 1, v['size'] + 2
        for k in range(s):
            for (x, y) in ((x0 + k, y0), (x0 + k, y0 + s - 1), (x0, y0 + k), (x0 + s - 1, y0 + k)):
                assert at(x, y) == col('edge'), f'{name}: frame pixel ({x},{y}) is {at(x, y)}'
        return used

    got['coarse_roles'] = view('coarse view', info['coarse'], F, n)
    got['fine_roles'] = view('fine view', info['fine'], b, n * ps)
    assert info['fine']['x'] == info['coarse']['x'] and info['fine']['y'] - info['coarse']['y'] == info['coarse']['size'] + 12
    assert len(info['coarse']['roles']) == n and len(info['fine']['roles']) == n * ps
    assert len(set(got['coarse_roles'])) == 2, f'the window must show both tones: {got["coarse_roles"]}'

    # ---- the exploded pixel: every pixel of its box, tile + projection + block, nothing stray
    want = expected_iso(info)
    s = info['tile']['s']
    x0, x1 = info['tile']['x'] - 2 * s, info['tile']['x'] + 2 * s
    y0, y1 = info['tile']['y'], info['panel']['y'] + 2 * s
    lit = 0
    for y in range(y0, y1):
        for x in range(x0, x1):
            role = want.get((x, y), 'void')
            assert at(x, y) == col(role), f'exploded pixel ({x},{y}) is {at(x, y)}, expected {role}'
            lit += role != 'void'
    got['iso_pixels'] = lit
    # the side faces differ from void, the top and each other
    sides = [info['tile']['left'], info['tile']['right'], info['tile']['top'], 'void']
    assert len({col(r) for r in sides}) == 4, f'tile faces not distinct: {sides}'
    # the block's centre: ps x ps cells, lit in the star's tone
    cells = info['panel']['fill']
    got['tile_top'] = info['tile']['top']
    assert info['tile']['top'] == info['panel']['fill']

    # ---- labels: their pixels in their role, nothing else inside their boxes
    for l in info['labels']:
        assert l['drawn'], f'label {l["text"]} not drawn'
        mine = other = 0
        for y in range(l['y'], l['y'] + l['h']):
            for x in range(l['x'], l['x'] + l['w']):
                c = at(x, y)
                if c == col(l['role']):
                    mine += 1
                else:
                    assert c == col('void'), f'label {l["text"]}: stray colour {c} at ({x},{y})'
        assert mine, f'label {l["text"]} has no pixels'
        got['label ' + l['text']] = mine
    texts = [l['text'] for l in info['labels']]
    assert 'DRAWING GRID ×' + str(ps) in texts and 'PANEL PIXELS 1:1' in texts and f'{F}:1' in texts and 'B' in texts
    # B and the factor on one baseline: the large letter's capitals end 20 rows into its box,
    # the small face's 10 into its own
    B = next(l for l in info['labels'] if l['text'] == 'B')
    f = next(l for l in info['labels'] if l['text'] == f'{F}:1')
    assert B['y'] + 20 == f['y'] + 10, 'B and the factor are not on one baseline'
    assert f['x'] - (B['x'] + B['w']) == 8

    # ---- the anchor: 2 vpx outside the coarse frame, away from the labels, on the middle row
    a = info['viewAnchor']; c = info['coarse']
    frame_l, frame_r = c['x'] - 1, c['x'] + c['size']
    assert a['x'] == (frame_r + 2 if info['mirror'] else frame_l - 2) and a['y'] == c['y'] + c['size'] // 2, f'anchor {a}'
    assert at(a['x'], a['y']) == col('void') and at(a['x'] + (1 if not info['mirror'] else -1), a['y']) == col('void'), 'anchor not clear'
    labels_x = [l['x'] for l in info['labels'] if l['text'].startswith(('DRAWING', 'PANEL'))]
    assert (min(labels_x) < c['x']) == info['mirror'], 'labels on the wrong side'

    # ---- the star: the coarse view is the star's own piece, and the window is marked
    if star_pixel is not None:
        w = info['window']
        for j in range(n):
            for i in range(n):
                assert star_pixel(w['x'] + i, w['y'] + j) == col(info['coarse']['roles'][j][i]), \
                    f'the star at ({w["x"] + i},{w["y"] + j}) differs from the coarse view'
        for k in range(n + 2):
            for (x, y) in ((w['x'] - 1 + k, w['y'] - 1), (w['x'] - 1 + k, w['y'] + n), (w['x'] - 1, w['y'] - 1 + k), (w['x'] + n, w['y'] - 1 + k)):
                assert star_pixel(x, y) == col('ink'), f'window marker pixel ({x},{y}) is not ink'
        got['star_matches'] = n * n
    return got


if __name__ == '__main__':
    from PIL import Image
    import importlib.util
    spec = importlib.util.spec_from_file_location('crisp', ROOT / 'layershell/tools/crisp.py')
    crisp = importlib.util.module_from_spec(spec); spec.loader.exec_module(crisp)
    from element import roles
    out = Path('/tmp/quadrille-aperture/Construction')
    themes = ['terminal', 'paper', 'phosphor', 'amber', 'lcd']
    failures = 0
    for monitor in ('laptop', 'dell'):
        for cfg in ({}, {'compact': True}, {'mirror': True}, {'mirror': True, 'compact': True}):
            tag = monitor + ''.join('-' + k for k in cfg)
            r = subprocess.run([sys.executable, str(HERE / 'element.py'), 'Construction', str(out / tag), '--monitor', monitor,
                                '--layout', 'real', '--cfg', json.dumps(cfg), '--themes', ','.join(themes), '--zoom', '3'],
                               capture_output=True, text=True)
            assert r.returncode == 0, f'element.py failed for {tag}: {r.stdout}{r.stderr}'
            res = json.loads(r.stdout.strip().splitlines()[0])
            info, ps = res['result'], res['ps']
            for theme in themes:
                path = out / f'{tag}-{theme}.png'
                im = Image.open(path).convert('RGB')
                try:
                    got = check(im.load(), ps, roles(theme), info)
                    colors, hx, hy = crisp.analyse(str(path), (0, 0, im.width, im.height), ps)
                    off = sum(v for k, v in hx.items() if k) + sum(v for k, v in hy.items() if k)
                    assert off == 0 and len(colors) <= 40, f'not crisp: {off} changes off the grid, {len(colors)} colours'
                    print(f'ok   {tag:16s} {theme:9s} ps {ps} F {info["factor"]}:1 cells {info["cells"]} '
                          f'iso lit {got["iso_pixels"]} colours {len(colors)}')
                except AssertionError as e:
                    failures += 1
                    print(f'FAIL {tag:16s} {theme:9s} {e}')
    sys.exit(1 if failures else 0)
