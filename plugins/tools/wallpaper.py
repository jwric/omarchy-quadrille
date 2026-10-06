#!/usr/bin/env python3
"""Render the actual QML sheet, assert layout geometry, and check every PNG.

wallpaper.py OUT [--explore]   no live session; 3 themes, both scales, case matrix
Rejected compositions are also renderable with --explore (real layout only).
--nested checks the service clone on QA/QB under the live lock (55 s maximum).
The generic offscreen driver is reused; ordinary renders need no compositor.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
import json
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


def run(args, log, env=None):
    with log.open('w') as f:
        subprocess.run(list(map(str, args)), cwd=ROOT, env=env, stdout=f, stderr=f, check=True)


def render(out, matrix, theme, scale, explore):
    roles = tomllib.loads((ROOT / f'themes/quadrille-{theme}/shell.toml').read_text())['quadrille']
    roles = {f'quadrille.{key}': value for key, value in roles.items() if isinstance(value, str)}
    selected = [case for case in matrix if case['scale'] == scale and (not explore or case['name'] == 'real')]
    compositions = ['atlas', 'comparator', 'section'] if explore else ['atlas']
    for case in selected:
        steps = [{'eval': 'Color.shellValues = ' + json.dumps(roles)}, {'wait': 300}]
        for composition in compositions:
            steps += [{'set': {'composition': composition}}, {'wait': 150},
                      {'grab': f'{composition}-{theme}-{case["id"]}'}]
        env = dict(os.environ, H_EXTRA=str(ROOT / 'plugins/tools/offscreen/Wallpaper.qml'), H_TIMEOUT='15',
                   H_OUTPUTS=json.dumps(case['outputs']), H_CURRENT=str(case['current']),
                   QT_NO_XDG_DESKTOP_PORTAL='1', QT_ACCESSIBILITY='0', NO_AT_BRIDGE='1')
        run([ROOT / 'plugins/tools/offscreen/run.sh', ROOT / 'plugins/quadrille.background', 'Wallpaper.qml',
             scale, json.dumps(steps), out], out / f'{theme}-{case["id"]}.log', env)
        for composition in compositions:
            p = out / f'{composition}-{theme}-{case["id"]}.png'
            with Image.open(p) as im:
                w, h = im.size
            m = case['outputs'][case['current']]
            expected = (m['height'],m['width']) if m['transform'] % 2 else (m['width'],m['height'])
            assert (w, h) == expected, (p, (w, h), expected)
            ps = max(1, round(2 * scale))
            if composition == 'atlas':
                image=Image.open(p).convert('RGB'); ruler=case['ruler']; y=(ruler['y']+8)*ps
                ink=tuple(bytes.fromhex(roles['quadrille.ink'].lstrip('#')))
                for mark in case['marks']:
                    x=(ruler['x']+mark['vpx'])*ps
                    assert all(image.getpixel((x+i,y)) == ink for i in range(ps)), (p,'missing 10 mm tick',x)
                    assert image.getpixel((x-1,y)) != ink and image.getpixel((x+ps,y)) != ink, (p,'wide tick',x)
            run(['python3', ROOT / 'layershell/tools/crisp.py', p, f'0,0,{w},{h}', ps, '--check'], p.with_suffix('.crisp.log'))


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
        p=background/'Graticule.qml';text=p.read_text().replace('onPaint: Drafting.paint(', 'onPaint: {console.log("WALLPAPER_FRAME",root.monitor.name,root.visible,Math.round(width*root.dpr),Math.round(height*root.dpr),root.physical.widthPx,root.physical.heightPx); Drafting.paint(')
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
            sys.exit(subprocess.call(['flock','-o','-w','900','/tmp/quadrille-live.lock','timeout','-k','2','55',sys.executable,__file__,str(out),'--nested','--locked']))
        nested(out); return
    run(['node', ROOT / 'plugins/tools/wallpaper-test.js', '--json', out / 'matrix.json'], out / 'geometry.log')
    matrix = json.loads((out / 'matrix.json').read_text())
    jobs = [('terminal', s) for s in [1, 1.666667]] if args.explore else [(t, s) for t in ['terminal', 'paper', 'phosphor'] for s in [1, 1.666667]]
    with ThreadPoolExecutor(max_workers=3) as pool:
        futures = [pool.submit(render, out, matrix, theme, scale, args.explore) for theme, scale in jobs]
        for future in futures:
            future.result()
    # A contact sheet is only an index; exact-size PNGs remain authoritative.
    paths = sorted(p for p in out.glob('*.png') if p.name.startswith(('atlas-', 'comparator-', 'section-')))
    thumbnails = []
    for p in paths:
        im = Image.open(p).convert('RGB'); im.thumbnail((480, 300), Image.Resampling.NEAREST)
        thumbnails.append(im)
    sheet = Image.new('RGB', (1440, ((len(paths) + 2) // 3) * 300))
    for i, im in enumerate(thumbnails):
        sheet.paste(im, ((i % 3) * 480, (i // 3) * 300))
    sheet.save(out / 'contact.png')
    print(f'{len(paths)} QML renders; geometry and crispness pass: {out}')


if __name__ == '__main__':
    main()
