#!/usr/bin/env python3
"""Detail.js (DETAIL A) checked from a rendered PNG alone.

check(pixel, ps, colors, info) -> dict   verifies, from the pixels, every drawn fact: each
panel-pixel block uniform and in the role an independent float angle test of the 32-pair star
gives, both rings (pixels and radii), the leader's pixels, and the rim's measured diameter.

Run as a script it renders the element (element.py) for the laptop and the Dell in all five
themes under /tmp/quadrille-aperture/Detail/ and runs check() and crisp.py
on each."""
import json, math, subprocess, sys
from pathlib import Path
from PIL import Image

HERE = Path(__file__).resolve().parent; ROOT = HERE.parents[2]
OUT = Path('/tmp/quadrille-aperture/Detail')
THEMES = ['terminal', 'paper', 'phosphor', 'amber', 'lcd']


def check(pixel, ps, colors, info):
    b = info['block']; disc = info['disc']; cx = info['centre']['x']; cy = info['centre']['y']
    ox, oy = info['origin']['x'], info['origin']['y']
    rim2, pan2 = info['rim2'], info['pan2']; ring_apart = info['panelRing']
    high, low = colors[info['tones']['high']], colors[info['tones']['low']]
    def at(x, y): return pixel[x*ps, y*ps]
    def whole(x, y):
        c = at(x, y)
        return all(pixel[x*ps+u, y*ps+v] == c for u in range(ps) for v in range(ps))
    def d2(x, y): return (x-cx)**2+(y-cy)**2          # exact: half pixels
    def band(x, y, p):                                  # within half a pixel of radius p/2
        r = p/2; d = d2(x, y); return r*r-r < d <= r*r+r
    X0, Y0, X1, Y1 = disc['x'], disc['y'], disc['x']+disc['w'], disc['y']+disc['h']

    # The leaders' pixels: each a diagonal, then a shoulder.
    leader = set(); leaders = [info['rimLeader']] + ([info['leader']] if ring_apart else [])
    for L in leaders:
        dx = (L['x1'] > L['x0'])-(L['x1'] < L['x0']); dy = (L['y1'] > L['y0'])-(L['y1'] < L['y0'])
        n = abs(L['x1']-L['x0']); assert abs(L['y1']-L['y0']) == n, 'leader not 45 degrees'
        for i in range(n+1): leader.add((L['x0']+dx*i, L['y0']+dy*i))
        hx = (L['x2'] > L['x1'])-(L['x2'] < L['x1']); assert hx == dx, 'shoulder does not run on from the diagonal'
        for x in range(L['x1'], L['x2']+hx, hx): leader.add((x, L['y1']))

    # 1. The rim and the panel limit ring: exactly the pixels within half a pixel of the radius.
    rim = []; pan = []
    for y in range(Y0, Y1):
        for x in range(X0, X1):
            c = at(x, y)
            if band(x, y, rim2): assert c == colors['accent'], f'rim pixel {x},{y} is {c}'; rim.append((x, y))
            else: assert c != colors['accent'], f'stray accent pixel {x},{y}'
            if ring_apart and band(x, y, pan2): assert c == colors['caution'], f'panel ring pixel {x},{y} is {c}'; pan.append((x, y))
            else: assert c != colors['caution'], f'stray caution pixel {x},{y}'
    # Measured, in vpx: the rim's diameter along the middle rows/columns and its extreme pixels.
    xs = [p[0] for p in rim]; ys = [p[1] for p in rim]
    diameter = max(xs)-min(xs)+1
    assert diameter == max(ys)-min(ys)+1 == disc['w'] == disc['h'], f'rim extents {diameter} vs box {disc}'
    assert abs((max(xs)+min(xs))/2-cx) == 0 and abs((max(ys)+min(ys))/2-cy) == 0, 'rim not centred'
    assert abs(diameter/2-info['rimRadius']) <= 1.0, f'rim radius {diameter/2} vs {info["rimRadius"]}'
    pan_radius = None
    if ring_apart:
        pxs = [p[0] for p in pan]; pys = [p[1] for p in pan]
        pan_radius = (max(pxs)-min(pxs)+1)/2
        assert (max(pys)-min(pys)+1)/2 == pan_radius and (max(pxs)+min(pxs))/2 == cx, 'panel limit ring not round/centred'
        assert abs(pan_radius-info['panelRadius']) <= 1.0, f'panel ring radius {pan_radius} vs {info["panelRadius"]}'
    # 8-fold symmetry of both rings about the centre.
    for name, pts in (('rim', rim), ('panel ring', pan)):
        s = set(pts)
        for x, y in pts:
            u, v = x-cx, y-cy
            for a, c in ((u, -v), (-u, v), (-u, -v), (v, u), (v, -u), (-v, u), (-v, -u)):
                assert (cx+a, cy+c) in s, f'{name} not symmetric at {x},{y}'

    # 2. Every block: uniform, and the role of the independent angle test.
    pitchX, pitchY = info['pitchX'], info['pitchY']
    checked = ties = hidden = 0; blocks_seen = set()
    for y in range(Y0, Y1):
        for x in range(X0, X1):
            if d2(x, y) > (rim2/2)**2-rim2/2:             # the rim's own pixels and beyond
                assert at(x, y) == colors['void'] or band(x, y, rim2) or (x, y) in leader, f'pixel {x},{y} beyond the rim is not void'
                continue
            assert whole(x, y), f'vpx {x},{y} is not one colour over its {ps} x {ps} physical pixels'
            if (x, y) in leader or (ring_apart and band(x, y, pan2)): continue
            i = (x-ox)//b; j = (y-oy)//b; c = at(x, y)
            assert c in (high, low), f'pixel {x},{y} is neither star role: {c}'
            blocks_seen.add((i, j))
            if i == 0 and j == 0: continue
            angle = (math.atan2(j*pitchY, i*pitchX) % (2*math.pi))/(math.pi/32)
            if abs(angle-round(angle)) < .005: ties += 1; continue
            want = high if math.floor(angle) % 2 == 0 else low
            assert c == want, f'block {i},{j} at {x},{y}: {c} but the star wants {want} (angle {angle:.3f})'
            checked += 1
    # Uniform blocks: every pixel of one block that is wholly inside the ring is the same colour.
    for (i, j) in blocks_seen:
        cols = {at(ox+i*b+u, oy+j*b+v) for v in range(b) for u in range(b)
                if d2(ox+i*b+u, oy+j*b+v) <= (rim2/2)**2-rim2/2 and (ox+i*b+u, oy+j*b+v) not in leader
                and not (ring_apart and band(ox+i*b+u, oy+j*b+v, pan2))}
        assert len(cols) == 1, f'block {i},{j} is not uniform: {cols}'
    assert checked > .8*math.pi*(info['rimRadius']-1)**2, f'only {checked} pixels checked against the angle test'

    # 3. The leader, pixel by pixel: muted where it shows, nothing else muted around it.
    shown = 0
    for (x, y) in leader:
        covered = band(x, y, rim2) or (ring_apart and band(x, y, pan2))
        if not covered: assert at(x, y) == colors['muted'], f'leader pixel {x},{y} is {at(x, y)}'; shown += 1
    for L in leaders:
        xs_ = [L['x0'], L['x1'], L['x2']]; ys_ = [L['y0'], L['y1']]
        extra = [(x, y) for y in range(min(ys_), max(ys_)+1) for x in range(min(xs_), max(xs_)+1)
                 if at(x, y) == colors['muted'] and (x, y) not in leader]
        assert not extra, f'muted pixels beside a leader: {extra[:5]}'

    # 4. The text: only the role it should have in its box.
    for t in info['texts']:
        assert t['drawn'], f"{t['text']} not drawn"
        allowed = {colors['void'], colors[t['role']]}
        if t['text'].startswith('NATIVE'): allowed.add(colors['muted'])
        lit = 0
        for y in range(t['y'], t['y']+t['h']):
            for x in range(t['x'], t['x']+t['w']):
                c = at(x, y); assert c in allowed, f"{t['text']}: pixel {x},{y} is {c}"; lit += c != colors['void']
        assert lit > 0, f"{t['text']} left no pixels"
    return {'diameter_vpx': diameter, 'rim_radius_vpx': diameter/2, 'panel_ring_radius_vpx': pan_radius,
            'rim_pixels': len(rim), 'panel_ring_pixels': len(pan), 'blocks': len(blocks_seen),
            'pixels_checked_by_angle': checked, 'ties_skipped': ties, 'leader_pixels_shown': shown}


def render(monitor, theme, prefix, cfg='{}'):
    r = subprocess.run([sys.executable, str(HERE/'element.py'), 'Detail', str(prefix), '--monitor', monitor, '--layout', 'real',
                        '--themes', theme, '--zoom', '2', '--cfg', cfg], capture_output=True, text=True, cwd=ROOT)
    assert r.returncode == 0, f'element.py failed for {monitor} {theme}: {r.stdout}{r.stderr}'
    return json.loads(r.stdout.strip().splitlines()[0])


def main():
    sys.path.insert(0, str(HERE)); import element
    OUT.mkdir(parents=True, exist_ok=True)
    failures = 0
    variants = [('', '{}')] + [('-above', json.dumps({'pitch': 'above', 'gridLeader': 'nw', 'panelLeader': 'sw'}))]
    for monitor, (suffix, cfg), theme in [(m, v, t) for v in variants for m in ('laptop', 'dell') for t in THEMES]:
        if True:
            prefix = OUT/(monitor+suffix)
            data = render(monitor, theme, prefix, cfg)
            ps = data['ps']; png = OUT/f'{monitor}{suffix}-{theme}.png'
            im = Image.open(png).convert('RGB'); colors = element.roles(theme)
            try:
                got = check(im.load(), ps, colors, data['result'])
                crisp = subprocess.run([sys.executable, str(ROOT/'layershell/tools/crisp.py'), str(png), f'0,0,{im.width},{im.height}', str(ps), f'{monitor}{suffix}-{theme}', '--check'],
                                       capture_output=True, text=True)
                assert crisp.returncode == 0, 'crisp.py: ' + crisp.stdout.strip().replace('\n', ' ')
                print(f'ok   {monitor+suffix:12} {theme:9} ps{ps} diameter {got["diameter_vpx"]} vpx, rim r={got["rim_radius_vpx"]}, panel ring r={got["panel_ring_radius_vpx"]}, '
                      f'{got["blocks"]} blocks, {got["pixels_checked_by_angle"]} pixels vs angle test ({got["ties_skipped"]} ties skipped), leader {got["leader_pixels_shown"]} px, crisp')
            except AssertionError as e:
                failures += 1; print(f'FAIL {monitor}{suffix} {theme}: {e}')
    sys.exit(1 if failures else 0)


if __name__ == '__main__':
    main()
