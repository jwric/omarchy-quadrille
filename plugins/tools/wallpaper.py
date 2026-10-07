#!/usr/bin/env python3
"""Render the actual QML sheet (Aperture, a technical portrait of each output), assert the
layout geometry, and check every PNG.

wallpaper.py OUT [--explore]   no live session; every matrix case of wallpaper-test.js (the
layouts real, vertical, side, three, portrait, single, estimated and rotated at scales 1 and
1.666667, plus hd, qhd and portrait1080) in 5 themes (terminal, paper, phosphor, amber, lcd),
each render checked for size, crispness and, from the PNG alone: the star's wedges against an
independent double-precision angle test, its muted rim and accent grid-limit ring, the
millimetre reference's ticks, and every module (DETAIL A, B, samples, materials, elevation,
locator) through its own plugins/tools/aperture/*_check.py. <png>.numeric.json holds what was
measured. --explore renders the star's tone options (edge, faint, muted) instead: themes
terminal and paper, the real layout only, both outputs at their real scales.
--nested checks the service clone on QA/QB under the live lock (55 s maximum).
The generic offscreen driver is reused; ordinary renders need no compositor.
"""
import argparse
from concurrent.futures import ProcessPoolExecutor
import importlib.util
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
APERTURE = ROOT / 'plugins/tools/aperture'
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


def checker(name):
    """One of plugins/tools/aperture/*_check.py, imported by path."""
    sys.path.insert(0, str(APERTURE))
    spec = importlib.util.spec_from_file_location(name, APERTURE / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def js_round(v):
    return math.floor(v + .5)


def sign(v):
    return (v > 0) - (v < 0)


def leader_pixels(points):
    """Every virtual pixel of a leader given as a point list, drawn the way Pen.leader draws
    each segment: n = the longer side, one pixel a step, every coordinate rounded."""
    pixels = set()
    for (x0, y0), (x1, y1) in zip(points or [], (points or [])[1:]):
        if (x0, y0) == (x1, y1):
            continue
        n = max(abs(x1-x0), abs(y1-y0)); dx, dy = sign(x1-x0), sign(y1-y0); i = 0
        while i <= n:
            pixels.add((js_round(x0+dx*i), js_round(y0+dy*i))); i += 1
    return pixels


def circle_pixels(cx, cy, rx, ry):
    """Pen.circle's pixels: within half a pixel of the radius, flat sides (integer arithmetic
    when round, the elliptical error scaled by the smaller radius otherwise)."""
    out = set()
    for y in range(cy-ry-1, cy+ry+2):
        for x in range(cx-rx-1, cx+rx+2):
            dx, dy = x-cx, y-cy
            if rx == ry:
                d = dx*dx+dy*dy
                if rx*rx-rx < d <= rx*rx+rx:
                    out.add((x, y))
            else:
                e = math.sqrt(dx*dx/(rx*rx)+dy*dy/(ry*ry))-1; m = min(rx, ry)
                if -0.5 < e*m <= 0.5:
                    out.add((x, y))
    return out


def retone(value, old, new):
    """A copy of an info object with the plan's star tone swapped for the one rendered."""
    if isinstance(value, dict):
        return {k: retone(v, old, new) for k, v in value.items()}
    if isinstance(value, list):
        return [retone(v, old, new) for v in value]
    return new if value == old else value


def numeric(p, case, roles, tone=None, explore=False):
    """Measure the rendered PNG, and only the PNG, independently of the JS predicates.

    Every virtual pixel is sampled at its physical top-left pixel (x*ps, y*ps).
    tone names the star's high role when it is not the one the matrix was planned with.
    """
    ps = case['physical']['pixelsPerVpx']
    mx, my = (case['physical'][k] for k in ['mmPerVpxX', 'mmPerVpxY'])
    plan = case['plan']; parts = plan['parts']; star = plan['star']
    image = Image.open(p).convert('RGB'); pixel = image.load()
    colors = {k.removeprefix('quadrille.'): tuple(bytes.fromhex(v.lstrip('#'))) for k, v in roles.items()}
    planned = dict(case['tones'])
    tones = dict(planned)
    if tone:
        tones['high'] = tone
    high, low = colors[tones['high']], colors[tones['low']]
    assert sum((a-b)**2 for a, b in zip(colors['ink'], colors['void'])) > 10000, (p, 'pattern contrast')
    ratio = contrast_ratio(high, low)
    if tones['high'] == 'faint' and not explore:
        assert ratio >= 1.8, (p, 'star contrast', tones, round(ratio, 3))
    D = star['diameter']; cx, cy, rx, ry = star['cx'], star['cy'], star['rx'], star['ry']
    at = lambda x, y: pixel[x*ps, y*ps]
    step = 2*max(mx, my)

    # The sheet's own leaders and the star's window marker, rebuilt from the plan.
    leaders = leader_pixels(parts.get('detailLeader')) | leader_pixels(parts.get('windowLeader'))
    marker = set()
    if parts.get('window'):
        w = parts['window']
        for k in range(w['w']):
            marker |= {(w['x']+k, w['y']), (w['x']+k, w['y']+w['h']-1)}
        for k in range(w['h']):
            marker |= {(w['x'], w['y']+k), (w['x']+w['w']-1, w['y']+k)}

    # 1. Star wedges: every star pixel against an independent double-precision angle test,
    # except ray ties, the grid-limit disc, the rim, the window marker and the leaders.
    checked = 0
    for y in range(star['y'], star['y']+star['h']):
        for x in range(star['x'], star['x']+star['w']):
            dx, dy = (x-cx)*mx, (y-cy)*my
            radius = math.hypot(dx, dy)
            if not plan['gridRadius']+step < radius < D/2-step:
                continue
            if (x, y) in leaders or (x, y) in marker:
                continue
            angle = (math.atan2(dy, dx) % (2*math.pi))/(math.pi/32)
            if abs(angle-round(angle)) < .005:
                continue
            expected = high if math.floor(angle) % 2 == 0 else low
            assert at(x, y) == expected, (p, 'binary wedge mismatch', x, y)
            checked += 1
    assert checked > .8*math.pi*rx*ry*(1-2*step/D)**2*.9, (p, 'wedge pixels checked', checked)

    # 2. The rim: `muted` pixels, whole, extents rx/ry, 4-fold (8-fold when round) symmetric;
    # the grid-limit ring: `accent`. The reference's extension lines are drawn over the rim's
    # lowest flat sides, and are the only pixels allowed to differ.
    x0, x1 = plan['ruler']['x'], plan['ruler']['x']+js_round(D/mx)
    covered = {(x, y) for x in (x0, x1) for y in range(cy+4, plan['ruler']['y'])}
    rim_rule = circle_pixels(cx, cy, rx, ry)
    for (x, y) in rim_rule - covered - leaders:
        assert at(x, y) == colors['muted'], (p, 'rim pixel is not muted', x, y)
    extents = symmetry = None
    if tones['high'] != 'muted':
        rim = set()
        for y in range(star['y']-1, star['y']+star['h']+1):
            for x in range(star['x']-1, star['x']+star['w']+1):
                if at(x, y) == colors['muted'] and (x, y) not in covered:
                    rim.add((x-cx, y-cy))
        assert rim == {(x-cx, y-cy) for (x, y) in rim_rule - covered}, (p, 'muted pixels in the star are not the rim', sorted(rim ^ {(x-cx, y-cy) for (x, y) in rim_rule - covered})[:6])
        extents = {'right': max(dx for dx, dy in rim), 'left': max(-dx for dx, dy in rim),
                   'down': max(dy for dx, dy in rim), 'up': max(-dy for dx, dy in rim)}
        assert extents['right'] == extents['left'] == rx and extents['down'] == extents['up'] == ry, (p, 'rim extents', extents, rx, ry)
        mask = {(x-cx, y-cy) for (x, y) in covered}
        def mirrored(points, f):
            return all(f(q) in points or f(q) in mask or q in mask for q in points)
        symmetry = {'x': mirrored(rim, lambda q: (-q[0], q[1])), 'y': mirrored(rim, lambda q: (q[0], -q[1])),
                    'diagonal': mirrored(rim, lambda q: (q[1], q[0])) if rx == ry else None}
        assert symmetry['x'] and symmetry['y'] and symmetry['diagonal'] in (True, None), (p, 'rim symmetry', symmetry)
    g = round(32/math.pi)
    ring = circle_pixels(cx, cy, g, g)
    for (x, y) in ring - leaders:
        assert at(x, y) == colors['accent'], (p, 'grid-limit ring pixel is not accent', x, y)
    stray = [(x, y) for y in range(star['y'], star['y']+star['h']) for x in range(star['x'], star['x']+star['w'])
             if at(x, y) == colors['accent'] and (x, y) not in ring and (x, y) not in marker]  # (in lcd accent == ink)
    assert not stray, (p, 'stray accent pixels in the star', stray[:5])

    # 3. The reference: every 10 mm tick `ink` at its x for the tick's rows, one vpx wide;
    # the line under them whole.
    ruler = plan['ruler']; yd = ruler['y']
    for x in range(x0, x1+1):
        assert at(x, yd) == colors['ink'], (p, 'reference line broken', x)
    marks = []
    for mark in plan['xMarks']:
        x = ruler['x']+mark['vpx']
        for y in range(yd-7, yd):
            assert at(x, y) == colors['ink'], (p, 'missing 10 mm tick', mark['mm'], x, y)
            assert at(x-1, y) != colors['ink'] and at(x+1, y) != colors['ink'], (p, 'wide tick', mark['mm'], x, y)
        assert all(pixel[x*ps+i, (yd-4)*ps+j] == colors['ink'] for i in range(ps) for j in range(ps)), (p, 'tick is not whole physical pixels', x)
        marks.append({'mm': mark['mm'], 'x_vpx': x, 'x_physical': x*ps})

    # 4. Every module, from the PNG alone, with the sheet's own leaders taken out of the
    # copy the module checks read (a leader ends beside a module and may cross its box).
    masked = image.copy(); void = colors['void']
    for (x, y) in leaders:
        for j in range(ps):
            for i in range(ps):
                masked.putpixel((x*ps+i, y*ps+j), void)
    mp = masked.load()
    results = {}
    def star_pixel(x, y):
        return pixel[x*ps, y*ps]
    def fix(info):
        return retone(info, planned['high'], tones['high']) if tone and tone != planned['high'] else info
    detail = checker('detail_check'); construction = checker('construction_check')
    samples = checker('samples_check'); materials = checker('materials_check'); portrait = checker('portrait_check')
    # DETAIL A's leaders are muted, so a muted star cannot be told from its leaders (the check
    # looks for stray muted pixels); the lifted tile's side faces are muted and edge, so B's own
    # check (four distinct faces) holds for the sheet's tone only. Both are checked where they can be.
    if tones['high'] != 'muted' and parts.get('detail'):
        results['detail'] = detail.check(mp, ps, colors, fix(parts['detail']))
    if tones['high'] == 'faint' and parts.get('construction'):
        results['construction'] = construction.check(mp, ps, colors, parts['construction'], star_pixel)
    if parts.get('samples'):
        results['samples'] = samples.check(mp, ps, colors, parts['samples'])
    if parts.get('materials'):
        results['materials'] = materials.check(mp, ps, colors, parts['materials'])
    if parts.get('elevation'):
        results['elevation'] = portrait.check_elevation(mp, ps, colors, parts['elevation'])
    if parts.get('locator'):
        results['locator'] = portrait.check_locator(mp, ps, colors, parts['locator'])

    ev = parts.get('elevation'); b = parts.get('construction'); a = parts.get('detail'); sm = parts.get('samples')
    result = {'star_diameter_mm': D, 'diameter_physical_px': [2*rx*ps, 2*ry*ps],
              'diameter_error_mm': [2*rx*mx-D, 2*ry*my-D], 'star_pixels_checked_by_angle': checked,
              'panel_alias_radius_mm': plan['panelRadius'], 'grid_alias_radius_mm': plan['gridRadius'],
              'panel_alias_radius_px': plan['panelRadius']/case['physical']['mmPerPixelX'],
              'grid_alias_radius_px': plan['gridRadius']/mx*ps,
              'tones': tones, 'contrast_ratio': round(ratio, 3), 'omitted': plan['omitted'],
              'rim': {'extents': extents, 'symmetry': symmetry},
              'ruler': {'marks': marks, 'labelStep': ruler['labelStep'], 'length_mm': ruler['length']},
              'detail_A': a and {'factor': a['factor'], 'block': a['block'], 'pitch_mm': [a['pitchX'], a['pitchY']],
                           'rim_radius_vpx': a['rimRadius'], 'panel_ring_radius_vpx': a['panelRadius'] if a['panelRing'] else None},
              'construction_B': b and {'factor': b['factor'], 'cells': b['cells'], 'window': b['window']},
              'samples': sm and {'frequencies': [c['text'] for c in sm['cases']],
                          'results': [''.join('#' if v == 'ink' else '.' for v in c['result']['cells']) for c in sm['cases']]},
              'elevation': ev and {'scale': ev['scale'], 'N': ev['N'], 'on_screen_width_mm': ev['onScreenWidthMm'], 'on_screen_width_px': ev['rect']['w']*ps},
              'locator': [{k: r[k] for k in ('index', 'name', 'current', 'x', 'y', 'w', 'h')} for r in parts['locator']['rects']] if parts.get('locator') else None,
              'checks': results}
    p.with_suffix('.numeric.json').write_text(json.dumps(result, indent=2)+'\n')


def render(out, theme, case, explore):
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
        assert tuple(expected) == (case['plan']['pixelWidth'], case['plan']['pixelHeight']), (p, expected, case['plan']['pixelWidth'])
        w, h = expected
        run(['python3', ROOT / 'layershell/tools/crisp.py', p, f'0,0,{w},{h}', ps, '--check'], p.with_suffix('.crisp.log'))
        try:
            numeric(p, case, roles, tone, explore)
        except AssertionError as e:
            raise AssertionError(f'{p.name}: {e}') from None


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
                numeric(p,case,{f'quadrille.{k}':v for k,v in roles.items() if isinstance(v,str)})
            import re
            visible=re.findall(r'WALLPAPER_FRAME (QA|QB) true (\d+) (\d+) (\d+) (\d+)',(out/'gpu.log').read_text())
            assert {v[0] for v in visible}=={'QA','QB'}
            for name,w,h,pw,ph in visible: assert (w,h)==(pw,ph), (name,w,h,pw,ph)
            print('nested wallpaper: final buffer sizes from first paint; QA/QB crisp')
        finally:
            if pointer and pointer.poll() is None:os.killpg(pointer.pid,signal.SIGTERM);pointer.wait(timeout=2)
            if child and child.poll() is None:os.killpg(child.pid,signal.SIGTERM);child.wait(timeout=2)
            subprocess.run([str(n),'down'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)


def geometry(out):
    """wallpaper-test.js writes the matrix whether or not its assertions hold; a failure is
    reported here (and in the exit status), and the renders still run."""
    r = subprocess.run(['node', ROOT / 'plugins/tools/wallpaper-test.js', '--json', out / 'matrix.json'],
                       cwd=ROOT, capture_output=True, text=True)
    (out / 'geometry.log').write_text(r.stdout + r.stderr)
    print((r.stdout + r.stderr).strip())
    return r.returncode == 0


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
            geometry(out)
            sys.exit(subprocess.call(['flock','-o','-w','900','/tmp/quadrille-live.lock','timeout','-k','2','55',sys.executable,__file__,str(out),'--nested','--locked']))
        nested(out); return
    geometry_ok = geometry(out)
    matrix = json.loads((out / 'matrix.json').read_text())
    if args.explore:
        # The real layout only: the laptop at 1.666667 and the Dell at 1.
        cases = [c for c in matrix if c['name'] == 'real' and c['scale'] == (1.666667 if c['current'] == 0 else 1)]
        themes = ['terminal', 'paper']
    else:
        cases, themes = matrix, THEMES
    jobs = [(theme, case) for theme in themes for case in cases]
    # Processes, not threads: each render is a quickshell subprocess, but the PNG checks are
    # pure Python and would share one interpreter lock. Every failure is collected.
    failures = []
    with ProcessPoolExecutor(max_workers=4) as pool:
        futures = [(theme, case['id'], pool.submit(render, out, theme, case, args.explore)) for theme, case in jobs]
        for theme, case_id, future in futures:
            try:
                future.result()
            except Exception as e:
                failures.append(f'{theme} {case_id}: {type(e).__name__}: {e}')
    (out / 'failures.txt').write_text('\n'.join(failures)+('\n' if failures else ''))
    # A contact sheet is only an index; exact-size PNGs remain authoritative.
    if args.explore:
        names = [f'star-{tone}-{theme}-{case["id"]}.png' for theme in themes for case in cases for tone in STAR_TONES]
        columns, sheet_name = len(STAR_TONES), 'explore.png'
    else:
        names = [f'aperture-{theme}-{case["id"]}.png' for theme in themes for case in cases]
        columns, sheet_name = 3, 'contact.png'
    paths = [out / n for n in names if (out / n).exists()]
    thumbnails = []
    for p in paths:
        im = Image.open(p).convert('RGB'); im.thumbnail((480, 300), Image.Resampling.NEAREST)
        thumbnails.append(im)
    sheet = Image.new('RGB', (columns * 480, ((len(paths) + columns - 1) // columns) * 300))
    for i, im in enumerate(thumbnails):
        sheet.paste(im, ((i % columns) * 480, (i // columns) * 300))
    sheet.save(out / sheet_name)
    what = (f'{len(paths)} QML renders ({len(cases)} cases x {len(themes)} themes'
            f'{" x " + str(len(STAR_TONES)) + " star tones" if args.explore else ""})')
    if failures or not geometry_ok:
        for f in failures:
            print('FAIL', f)
        print(f'{what}: {len(failures)} render checks FAILED'
              f'{"" if geometry_ok else "; the geometry test failed (see geometry.log)"}: {out}')
        sys.exit(1)
    print(f'{what}; geometry, crispness and numeric checks pass: {out}')


if __name__ == '__main__':
    main()
