#!/usr/bin/env python3
"""Render the actual QML sheet, assert layout geometry, and check every PNG.

wallpaper.py OUT [--explore]   no live session; every matrix case (26: the layouts at
scales 1 and 1.666667, plus hd and qhd) in 5 themes (terminal, paper, phosphor, amber,
lcd) = 130 renders, each checked for size, ruler ticks, crispness and, from the PNG
alone, the star's wedges, rings, outline, bursts, slanted edge, DETAIL A and swatches.
--explore renders the star's tone options (edge, faint, muted) instead: themes terminal
and paper, the real layout only, both outputs at their real scales = 12 renders.
--nested checks the service clone on QA/QB under the live lock (55 s maximum).
The generic offscreen driver is reused; ordinary renders need no compositor.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from fractions import Fraction
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile
import signal
import shutil
import sys
import time
import tomllib
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
THEMES = ['terminal', 'paper', 'phosphor', 'amber', 'lcd']
STAR_TONES = ['edge', 'faint', 'muted']


def run(args, log, env=None):
    with log.open('w') as f:
        subprocess.run(list(map(str, args)), cwd=ROOT, env=env, stdout=f, stderr=f, check=True)


def luminance(rgb):
    """WCAG relative luminance of an sRGB colour."""
    def lin(v):
        v /= 255
        return v/12.92 if v <= 0.03928 else ((v+0.055)/1.055)**2.4
    r, g, b = map(lin, rgb)
    return 0.2126*r + 0.7152*g + 0.0722*b


def contrast_ratio(a, b):
    hi, lo = sorted((luminance(a), luminance(b)), reverse=True)
    return (hi+0.05)/(lo+0.05)


def leader_pixels(detail):
    """Every virtual pixel of DETAIL A's leader, rebuilt from its end points."""
    pixels = set()
    leader = detail and detail.get('leader')
    if not leader:
        return pixels
    for i in range(leader['x1']-leader['x0']+1):
        pixels.add((leader['x0']+i, leader['y0']-i))
    for x in range(leader['x1'], leader.get('x2', leader['x1']-1)+1):
        pixels.add((x, leader['y1']))
    return pixels


def numeric(p, case, roles, composition='aperture', tone=None, explore=False):
    """Measure the rendered PNG, and only the PNG, independently of the JS predicates.

    Every virtual pixel is sampled at its physical top-left pixel (x*ps, y*ps).
    tone names the star's high role when it is not the one the matrix was planned with.
    """
    ps = case['physical']['pixelsPerVpx']
    mx, my = (case['physical'][k] for k in ['mmPerVpxX', 'mmPerVpxY'])
    data = case['drawings'][composition]
    star = data['star']; image = Image.open(p).convert('RGB'); pixel = image.load()
    colors = {k.removeprefix('quadrille.'): tuple(bytes.fromhex(v.lstrip('#'))) for k, v in roles.items()}
    tones = dict(case['tones'])
    if tone:
        tones['high'] = tone
    high, low = colors[tones['high']], colors[tones['low']]
    assert sum((a-b)**2 for a,b in zip(colors['ink'], colors['void'])) > 10000, (p, 'pattern contrast')
    ratio = contrast_ratio(high, low)
    if tones['high'] == 'faint' and not explore:
        assert ratio >= 1.8, (p, 'star contrast', tones, round(ratio, 3))
    detail = data.get('detail')
    leader = leader_pixels(detail)
    step = 2*max(mx, my)
    # 1. Star wedges. Independent double-precision angles check the fixed-point JS
    # boundaries. Exclude ray ties, the gridRadius disc, the annotation rings and
    # the leader; inspect every other cell.
    for y in range(star['y'],star['y']+star['h']):
        for x in range(star['x'],star['x']+star['w']):
            dx,dy = (x-star['cx'])*mx, (y-star['cy'])*my
            radius = math.hypot(dx,dy)
            if not case['gridRadius']+step < radius < star['diameter']/2-step: continue
            if any(abs(radius-ring['mm']) <= step for ring in data['rings']): continue
            if (x,y) in leader: continue
            angle = (math.atan2(dy,dx)%(2*math.pi))/(math.pi/32)
            if abs(angle-round(angle)) < .005: continue
            expected = high if math.floor(angle)%2==0 else low
            assert pixel[x*ps,y*ps] == expected, (p,'binary wedge mismatch',x,y)
    # 3. Annulus count and frequency.
    checks = []
    for ring in data['rings']:
        # Trace a clean annulus outside the printed hairline. Its actual pairs
        # and mean sample radius measure the tangential pitch independently.
        # A sample on the leader carries the previous value.
        r = ring['mm']+2.5*max(mx,my)
        binary = []; measured_r = 0
        for i in range(8192):
            theta = i*2*math.pi/8192
            x = round(star['cx']+r*math.cos(theta)/mx)
            y = round(star['cy']+r*math.sin(theta)/my)
            measured_r += math.hypot((x-star['cx'])*mx,(y-star['cy'])*my)/8192
            if (x,y) in leader:
                binary.append(None); continue
            sample = pixel[x*ps,y*ps]
            assert sample in [high,low], (p,'annulus obstructed',x,y)
            binary.append(sample == high)
        known = [i for i,v in enumerate(binary) if v is not None]
        for i in range(len(binary)):
            if binary[i] is None:
                binary[i] = binary[max([k for k in known if k < i], default=known[-1])]
        count = sum(v != binary[i-1] for i,v in enumerate(binary))/2
        assert count == 32, (p,'measured wedge count',count)
        pitch = 2*math.pi*measured_r/count
        f = (1/pitch)*(r/ring['mm'])
        assert abs(f-ring['frequency']) < .001, (p,'frequency',f,ring['frequency'])
        checks.append({'radius_mm':ring['mm'], 'printed_lp_mm':round(ring['frequency'],2), 'measured_lp_mm':round(f,5), 'pairs':count})
    # 4. The star's outline: the accent pixels of the rim, whole and symmetric.
    rx, ry = star['rx'], star['ry']; m = min(rx, ry)
    rim = set()
    for y in range(star['y'],star['y']+star['h']):
        for x in range(star['x'],star['x']+star['w']):
            dx, dy = x-star['cx'], y-star['cy']
            d = math.hypot(dx,dy) if rx == ry else math.hypot(dx/rx,dy/ry)*m
            if m-3 <= d <= m+1 and pixel[x*ps,y*ps] == colors['accent']:
                rim.add((dx,dy))
    extents = {'right':max(dx for dx,dy in rim), 'left':max(-dx for dx,dy in rim),
               'down':max(dy for dx,dy in rim), 'up':max(-dy for dx,dy in rim)}
    assert extents['right'] == extents['left'] == rx and extents['down'] == extents['up'] == ry, (p,'outline extents',extents,rx,ry)
    symmetry = {'x': {(-dx,dy) for dx,dy in rim} == rim, 'y': {(dx,-dy) for dx,dy in rim} == rim,
                'diagonal': {(dy,dx) for dx,dy in rim} == rim if rx == ry else None}
    assert symmetry['x'] and symmetry['y'] and symmetry['diagonal'] in (True, None), (p,'outline symmetry',symmetry)
    # 5. Bursts and the slanted edge.
    for burst in data['bursts']:
        n, d = Fraction(burst['period']['n']).limit_denominator(1000), burst['period']['d']
        for y in range(burst['h']):
            for x in range(burst['w']):
                at = y if burst['vertical'] else x
                expected = colors['ink'] if math.floor(2*at*d/n)%2==0 else colors['void']
                assert pixel[(burst['x']+x)*ps,(burst['y']+y)*ps] == expected, (p,'burst mismatch',burst['period'])
        density = my if burst['vertical'] else mx
        assert abs(burst['frequency']-burst['period']['d']/burst['period']['n']/density)<1e-9
    for edge in data.get('edges', []):
        for y in range(edge['h']):
            for x in range(edge['w']):
                expected = colors['ink'] if 12*(x-24)>=y-16 else colors['void']
                assert pixel[(edge['x']+x)*ps,(edge['y']+y)*ps] == expected, (p,'slanted stair step')
    # 6. DETAIL A: every panel pixel, as a block of virtual pixels, in its wedge's tone.
    detail_view = None
    if detail:
        block, count = detail['block'], detail['count']; half = (count-1)//2
        ccx = detail['x']+half*block+(block-1)/2; ccy = detail['y']+half*block+(block-1)/2
        rings = [r for r in (detail['panelRing'], detail['gridRing']) if r is not None]
        checked = eligible = 0
        for j in range(count):
            for i in range(count):
                dx, dy = (i-half)*detail['pitchX'], (j-half)*detail['pitchY']
                if dx == 0 and dy == 0: continue
                angle = (math.atan2(dy,dx)%(2*math.pi))/(math.pi/32)
                if abs(angle-round(angle)) < .005: continue
                expected = high if math.floor(angle)%2==0 else low
                eligible += 1; seen = False
                for v in range(block):
                    for u in range(block):
                        x, y = detail['x']+i*block+u, detail['y']+j*block+v
                        d = math.hypot(x-ccx, y-ccy)
                        if any(abs(d-r) <= 1.0 for r in rings): continue
                        assert pixel[x*ps,y*ps] == expected, (p,'detail pixel mismatch',i,j,x,y)
                        seen = True
                checked += seen
        # The centre and the exact ray ties (the axes and the diagonals, 4(count-1)+... pixels,
        # about 9% of a square panel) are never decidable, so 90% is taken of the rest.
        assert checked >= .9*eligible, (p,'detail pixels checked',checked,eligible,count*count)
        frame = [(x,detail['y']-1) for x in range(detail['x']-1,detail['x']+detail['w']+1)] + \
                [(x,detail['y']+detail['h']) for x in range(detail['x']-1,detail['x']+detail['w']+1)] + \
                [(detail['x']-1,y) for y in range(detail['y']-1,detail['y']+detail['h']+1)] + \
                [(detail['x']+detail['w'],y) for y in range(detail['y']-1,detail['y']+detail['h']+1)]
        assert all(pixel[x*ps,y*ps] == colors['edge'] for x,y in frame), (p,'detail frame')
        detail_view = {'factor':detail['factor'], 'block':block, 'count':count, 'pitch_mm':detail['pitchX'],
                       'px_per_mm':1/detail['pitchX'], 'across_mm':count*detail['pitchX'],
                       'panel_ring_vpx':detail['panelRing'], 'grid_ring_vpx':detail['gridRing'],
                       'panel_pixels_checked':checked, 'panel_pixels_eligible':eligible, 'panel_pixels':count*count}
    # 7. Swatches: an edge frame round the role's own colour.
    for sw in data['swatches']:
        for y in range(sw['h']):
            for x in range(sw['w']):
                border = x in (0, sw['w']-1) or y in (0, sw['h']-1)
                expected = colors['edge'] if border else colors[sw['role']]
                assert pixel[(sw['x']+x)*ps,(sw['y']+y)*ps] == expected, (p,'swatch',sw['role'],x,y)
    result = {'star_diameter_mm':star['diameter'], 'diameter_physical_px':[2*star['rx']*ps,2*star['ry']*ps],
              'diameter_error_mm':[2*star['rx']*mx-star['diameter'],2*star['ry']*my-star['diameter']],
              'frequency_rings':checks,'panel_alias_radius_mm':case['panelRadius'],
              'grid_alias_radius_mm':case['gridRadius'],'panel_alias_radius_px':case['panelRadius']/mx*ps,
              'grid_alias_radius_px':case['gridRadius']/mx*ps,
              'tones':tones, 'contrast_ratio':round(ratio, 3),
              'outline':{'extents':extents, 'symmetry':symmetry}, 'detail_view':detail_view}
    p.with_suffix('.numeric.json').write_text(json.dumps(result,indent=2)+'\n')


def render(out, matrix, theme, case, explore):
    roles = tomllib.loads((ROOT / f'themes/quadrille-{theme}/shell.toml').read_text())['quadrille']
    roles = {f'quadrille.{key}': value for key, value in roles.items() if isinstance(value, str)}
    steps = [{'eval': 'Color.shellValues = ' + json.dumps(roles)}, {'wait': 300}]
    # The normal run grabs the sheet as it is; --explore grabs each star tone in turn.
    grabs = [(tone, f'star-{tone}-{theme}-{case["id"]}') for tone in STAR_TONES] if explore else [(None, f'aperture-{theme}-{case["id"]}')]
    for tone, name in grabs:
        if tone:
            steps.append({'set': {'starTone': tone}})
        steps += [{'wait': 250}, {'grab': name}]
    env = dict(os.environ, H_EXTRA=str(ROOT / 'plugins/tools/offscreen/Wallpaper.qml'), H_TIMEOUT='15',
               H_OUTPUTS=json.dumps(case['outputs']), H_CURRENT=str(case['current']),
               QT_NO_XDG_DESKTOP_PORTAL='1', QT_ACCESSIBILITY='0', NO_AT_BRIDGE='1')
    run([ROOT / 'plugins/tools/offscreen/run.sh', ROOT / 'plugins/quadrille.background', 'Wallpaper.qml',
         case['scale'], json.dumps(steps), out], out / f'{theme}-{case["id"]}.log', env)
    ps = case['physical']['pixelsPerVpx']
    for tone, name in grabs:
        p = out / f'{name}.png'
        with Image.open(p) as im:
            w, h = im.size
        m = case['outputs'][case['current']]
        expected = (m['height'],m['width']) if m['transform'] % 2 else (m['width'],m['height'])
        # Wallpaper.qml's window is whole logical pixels whose device size is whole too: where the
        # output is not (2560 / 1.5) it is a little larger, and only the output's pixels are checked.
        window = []
        for v in expected:
            n = math.floor(v/case['scale']+.5)
            for _ in range(8):
                if abs(n*case['scale']-round(n*case['scale'])) <= .001: break
                n += 1
            window.append(round(n*case['scale']))
        assert (w, h) == tuple(window), (p, (w, h), window, expected)
        assert all(0 <= a-b <= 2 for a, b in zip((w, h), expected)), (p, (w, h), expected)
        w, h = expected
        image = Image.open(p).convert('RGB'); ruler = case['ruler']; y = (ruler['y']+8)*ps
        ink = tuple(bytes.fromhex(roles['quadrille.ink'].lstrip('#')))
        for mark in case['marks']:
            x = (ruler['x']+mark['vpx'])*ps
            assert all(image.getpixel((x+i,y)) == ink for i in range(ps)), (p,'missing 10 mm tick',x)
            assert image.getpixel((x-1,y)) != ink and image.getpixel((x+ps,y)) != ink, (p,'wide tick',x)
        run(['python3', ROOT / 'layershell/tools/crisp.py', p, f'0,0,{w},{h}', ps, '--check'], p.with_suffix('.crisp.log'))
        numeric(p, case, roles, 'aperture', tone, explore)


def nested(out):
    """The real service clone with synthetic EDID, on QA/QB, never live input."""
    n=ROOT/'layershell/tools/nested.sh'; os.environ['QUADRILLE_NESTED_DIR']=str(out/'nested')
    child=None;pointer=None
    with tempfile.TemporaryDirectory() as work:
        work=Path(work); conf=work/'root';conf.mkdir();home=work/'home'
        for name in ['Commons','Ui','services']:
            (conf/name).symlink_to(Path('/usr/share/omarchy/shell')/name)
        current=home/'.local/state/omarchy/current';current.mkdir(parents=True)
        (current/'theme').symlink_to(ROOT/'themes/quadrille-terminal')
        (current/'background').symlink_to(ROOT/'themes/quadrille-terminal/backgrounds/1-graticule.png')
        background=work/'background';shutil.copytree(ROOT/'plugins/quadrille.background',background,symlinks=False)
        p=background/'Background.qml';text=p.read_text()
        text=text.replace('out.push(object)', 'out.push(Object.assign({}, object, {physicalWidth:object.name==="QA"?340:800,physicalHeight:object.name==="QA"?220:330,x:object.name==="QA"?0:-952,y:object.name==="QA"?0:-1440}))')
        text=text.replace('object.scale > 0)', 'object.scale > 0 && (object.name==="QA" || object.name==="QB"))')
        text=text.replace('model: Quickshell.screens','model: Quickshell.screens.filter(function(s){return s.name==="QA" || s.name==="QB"})');p.write_text(text)
        p=background/'Graticule.qml';text=p.read_text().replace('onPaint: Drafting.paint(', 'onPaint: {console.log("WALLPAPER_FRAME",root.monitor.name,root.visible,Math.round(root.width*root.dpr),Math.round(root.height*root.dpr),root.physical.widthPx,root.physical.heightPx,Math.round(width*root.dpr),Math.round(height*root.dpr)); Drafting.paint(')
        text=text.replace('root.software, root.dpr)', 'root.software, root.dpr) }');p.write_text(text)
        (conf/'shell.qml').write_text('import Quickshell\nimport QtQuick\nShellRoot {Loader {source:Quickshell.env("H_BACKGROUND")}}')
        bus=work/'bus.conf';bus.write_text('<busconfig><type>session</type><listen>unix:tmpdir=/tmp</listen><auth>EXTERNAL</auth><policy context="default"><allow send_destination="*"/><allow receive_sender="*"/><allow own="*"/></policy></busconfig>')
        try:
            run([n,'up'],out/'nested.log')
            env=dict(os.environ,HOME=str(home),XDG_STATE_HOME=str(home/'.local/state'),XDG_CONFIG_HOME=str(home/'.config'),H_BACKGROUND='file:'+str(background/'Background.qml'),QT_QPA_PLATFORM='wayland',QT_NO_XDG_DESKTOP_PORTAL='1',QT_ACCESSIBILITY='0',NO_AT_BRIDGE='1',QT_QPA_PLATFORMTHEME='generic')
            child=subprocess.Popen([str(n),'run','dbus-run-session','--config-file',str(bus),'--','quickshell','-p',str(conf)],env=env,stdout=(out/'gpu.log').open('w'),stderr=subprocess.STDOUT,start_new_session=True)
            time.sleep(3)
            outputs=json.loads(subprocess.check_output([str(n),'ctl','monitors','-j']))
            other=next(m['name'] for m in outputs if m['name'] not in ['QA','QB'])
            pointer=subprocess.Popen([str(n),'run',str(ROOT/'layershell/target/release/vptr'),other,'move','10','10','sleep','8000'],start_new_session=True)
            time.sleep(.1)
            for name,ps,w,h in [('QA',3,2560,1600),('QB',2,3440,1440)]:
                p=out/f'gpu-{name}.png';subprocess.run([str(n),'run','grim','-o',name,str(p)],check=True)
                run(['python3',ROOT/'layershell/tools/crisp.py',p,f'0,0,{w},{h}',ps,'--check'],out/f'{name}.crisp.log')
                matrix=json.loads((out/'matrix.json').read_text())
                case=next(c for c in matrix if c['name']=='real' and c['current']==(0 if name=='QA' else 1)
                          and c['scale']==(1.666667 if name=='QA' else 1))
                roles=tomllib.loads((ROOT/'themes/quadrille-terminal/shell.toml').read_text())['quadrille']
                numeric(p,case,{f'quadrille.{k}':v for k,v in roles.items() if isinstance(v,str)},'aperture')
            import re
            visible=re.findall(r'WALLPAPER_FRAME (QA|QB) true (\d+) (\d+) (\d+) (\d+)',(out/'gpu.log').read_text())
            assert {v[0] for v in visible}=={'QA','QB'}
            for name,w,h,pw,ph in visible: assert (w,h)==(pw,ph), (name,w,h,pw,ph)
            print('nested wallpaper: final buffer sizes from first paint; QA/QB crisp')
        finally:
            if pointer and pointer.poll() is None:os.killpg(pointer.pid,signal.SIGTERM);pointer.wait(timeout=2)
            if child and child.poll() is None:os.killpg(child.pid,signal.SIGTERM);child.wait(timeout=2)
            subprocess.run([str(n),'down'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('out', type=Path)
    ap.add_argument('--explore', action='store_true')
    ap.add_argument('--nested', action='store_true')
    ap.add_argument('--locked', action='store_true', help=argparse.SUPPRESS)
    args = ap.parse_args()
    out = args.out.resolve(); out.mkdir(parents=True, exist_ok=True)
    if args.nested:
        if not args.locked:
            run(['node', ROOT / 'plugins/tools/wallpaper-test.js', '--json', out / 'matrix.json'], out / 'geometry.log')
            sys.exit(subprocess.call(['flock','-o','-w','900','/tmp/quadrille-live.lock','timeout','-k','2','55',sys.executable,__file__,str(out),'--nested','--locked']))
        nested(out); return
    run(['node', ROOT / 'plugins/tools/wallpaper-test.js', '--json', out / 'matrix.json'], out / 'geometry.log')
    matrix = json.loads((out / 'matrix.json').read_text())
    if args.explore:
        # The real layout only: the laptop at 1.666667 and the Dell at 1.
        cases = [c for c in matrix if c['name'] == 'real' and c['scale'] == (1.666667 if c['current'] == 0 else 1)]
        themes = ['terminal', 'paper']
    else:
        cases, themes = matrix, THEMES
    jobs = [(theme, case) for theme in themes for case in cases]
    with ThreadPoolExecutor(max_workers=3) as pool:
        futures = [pool.submit(render, out, matrix, theme, case, args.explore) for theme, case in jobs]
        for future in futures:
            future.result()
    # A contact sheet is only an index; exact-size PNGs remain authoritative.
    if args.explore:
        names = [f'star-{tone}-{theme}-{case["id"]}.png' for theme in themes for case in cases for tone in STAR_TONES]
        columns, sheet_name = len(STAR_TONES), 'explore.png'
    else:
        names = [f'aperture-{theme}-{case["id"]}.png' for theme in themes for case in cases]
        columns, sheet_name = 3, 'contact.png'
    paths = [out / n for n in names]
    thumbnails = []
    for p in paths:
        im = Image.open(p).convert('RGB'); im.thumbnail((480, 300), Image.Resampling.NEAREST)
        thumbnails.append(im)
    sheet = Image.new('RGB', (columns * 480, ((len(paths) + columns - 1) // columns) * 300))
    for i, im in enumerate(thumbnails):
        sheet.paste(im, ((i % columns) * 480, (i // columns) * 300))
    sheet.save(out / sheet_name)
    print(f'{len(paths)} QML renders ({len(cases)} cases x {len(themes)} themes'
          f'{" x " + str(len(STAR_TONES)) + " star tones" if args.explore else ""}); geometry, crispness and numeric checks pass: {out}')


if __name__ == '__main__':
    main()
