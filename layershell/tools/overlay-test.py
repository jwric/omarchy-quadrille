#!/usr/bin/env python3
"""One nested-only overlay check: timing, CAD geometry, input, hotplug and cost.

    python3 layershell/tools/overlay-test.py OUT

Live and calm each take the lock for at most 55 seconds; all input targets nested.sh.
Native ARGB dumps are enabled only during geometry checks. No live input or host.
"""
import argparse
import json
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time
from PIL import Image, ImageChops, ImageDraw

ROOT=Path(__file__).resolve().parents[1]
N=ROOT/'tools/nested.sh'; BIN=ROOT/'target/release/quadrille-bar'
children=[]
holder=None


def run(*args):
    return subprocess.check_output(list(map(str,args)),text=True).strip()


def nc(*args): return run(N,'ctl',*args)
def ctl(*args):
    return run(N,'bar-ctl',*args)

def launch(args,log,env=None):
    p=subprocess.Popen(list(map(str,args)),stdout=log.open('w'),stderr=subprocess.STDOUT,env=env,start_new_session=True)
    children.append(p); return p


def wait(predicate,timeout=2):
    end=time.monotonic()+timeout
    while time.monotonic()<end:
        try:
            value=predicate()
            if value: return value
        except (OSError,ValueError,subprocess.CalledProcessError): pass
        time.sleep(.015)
    raise AssertionError('condition timed out')


def frame(out):
    meta,pixels=(out/'reticle.frame').read_bytes().split(b'\n',1)
    return json.loads(meta),pixels


def current(out,name,resting):
    info,_=frame(out)
    return info if info['output']==name and info['resting']==resting else None


def move(name,x,y,moving=False):
    global holder
    if holder and holder.poll() is None:
        os.killpg(holder.pid,signal.SIGTERM);holder.wait(timeout=2)
    args=[N,'run','env',f'VPTR_EXTENT={"1536x960" if name=="QA" else "3440x1440"}',ROOT/'target/release/vptr',name]
    for i in range(500):
        dx=i%100 if moving else 0
        args+=['move',str(x+dx),str(y+dx//2),'sleep','16']
    holder=launch(args,Path(os.environ['QUADRILLE_NESTED_DIR']).parent/'pointer.log')
    time.sleep(.045)
    if not moving:
        m=next(m for m in json.loads(nc('monitors','-j')) if m['name']==name)
        ew,eh=(1536,960) if name=='QA' else (3440,1440)
        expected=(m['x']+x*round(m['width']/m['scale'])/ew,m['y']+y*round(m['height']/m['scale'])/eh)
        def matches(cursor):return all(abs(a-b)<=1 for a,b in zip(cursor,expected))
        def placed():
            cursor=json.loads(nc('cursorpos','-j'))
            return matches((cursor['x'],cursor['y']))
        wait(placed)
        out=Path(os.environ['QUADRILLE_NESTED_DIR']).parent
        if (out/'reticle.capture').exists() and (out/'reticle.frame').exists():
            wait(lambda:matches(frame(out)[0]['cursor']))


def layers(): return nc('layers')
def hidden(): return not re.search(r'namespace: quadrille-(reticle|dimensions)',layers())


def raster_check(out,name,label,resting=True):
    info=wait(lambda: current(out,name,resting))
    time.sleep(.08)
    info,pixels=frame(out); w,h=info['size']; ps=info['vpx']
    raw=Image.frombytes('RGBA',(w,h),pixels,'raw','BGRA')
    left,top=[math.floor(v*info['scale']+.5) for v in info['margin']]
    centre=[info['centre'][i]*ps+info['offset'][i]+[left,top][i] for i in [0,1]]
    assert centre==info['snapped'],(centre,info)
    mask=raw.getchannel('A'); draw=ImageDraw.Draw(mask)
    gap=16 if resting else 4
    draw.rectangle((centre[0]-left-gap,centre[1]-top-gap,centre[0]-left+100,centre[1]-top+100),fill=0)
    assert mask.getbbox(), 'cursor exclusion must leave measurable marks'
    # Inspect output-aligned native cells, even when logical margins have a residual.
    aligned=Image.new('RGBA',((w+ps*2)//ps*ps,(h+ps*2)//ps*ps))
    aligned.paste(raw,(left%ps,top%ps))
    buffer=out/f'{label}-buffer.png';aligned.save(buffer)
    run('python3',ROOT/'tools/crisp.py',buffer,f'0,0,{aligned.width},{aligned.height}',ps,'--check')
    shot=out/f'{label}-screen.png';nc('dismissnotify');run(N,'run','grim','-o',name,shot)
    screen=Image.open(shot).convert('RGB')
    presented=screen.crop((left,top,left+w,top+h));shot.unlink()
    edge=ImageDraw.Draw(mask)
    if screen.width-left<w:edge.rectangle((screen.width-left,0,w,h),fill=0)
    if screen.height-top<h:edge.rectangle((0,screen.height-top,w,h),fill=0)
    diff=ImageChops.difference(raw.convert('RGB'),presented)
    r,g,b=diff.split();diff=ImageChops.multiply(ImageChops.lighter(ImageChops.lighter(r,g),b),mask)
    bad=sum(diff.histogram()[1:]);maximum=max(i for i,n in enumerate(diff.histogram()) if n)
    if bad:diff.save(out/f'{label}-difference.png')
    raw.save(out/f'{label}.png')
    (out/f'{label}.json').write_text(json.dumps(info,indent=2))
    print(f'{label}: {bad} differing opaque pixels, maximum channel delta {maximum}',flush=True)
    return {'label':label,'bad':bad,'max_delta':maximum},info


def cpu(pid): return sum(int(p.read_text().split()[0]) for p in Path(f'/proc/{pid}/task').glob('*/schedstat'))
def rss(pid): return int(next(l for l in Path(f'/proc/{pid}/status').read_text().splitlines() if l.startswith('VmRSS:')).split()[1])
def cost(pid,label):
    before=cpu(pid);started=time.monotonic();time.sleep(2)
    dt=time.monotonic()-started
    (Path(os.environ['QUADRILLE_NESTED_DIR']).parent/f'{label}.smaps').write_text(Path(f'/proc/{pid}/smaps').read_text())
    return {'state':label,'cpu_percent':(cpu(pid)-before)/1e9/dt*100,'rss_mib':rss(pid)/1024,'seconds':dt}


def motion_check(out,name,rest_ms):
    polls=out/'reticle.polls';before=polls.read_text().splitlines() if polls.exists() else []
    move(name,400,300,moving=True)
    expected=rest_ms==0
    wait(lambda:current(out,name,expected))
    seen=[];started=time.monotonic()
    while time.monotonic()-started<.55:
        info,_=frame(out)
        assert info['output']==name and info['resting']==expected,info
        assert bool(info['dimensions'])==expected,info
        seen.append((info['snapped'],info['dimensions']))
        time.sleep(.035)
    assert len({tuple(p) for p,_ in seen})>5,'reticle did not follow motion'
    if expected:
        assert len({json.dumps(d) for _,d in seen})>5,'dimensions did not follow motion'
    after=polls.read_text().splitlines()[len(before):]
    assert after.count('j/cursorpos')>10,after
    assert 1<=after.count('j/clients')<=4,after
    actual=re.findall(r'namespace: quadrille-(?:dimensions|reticle)',layers())
    assert len(actual)==1,actual
    print(f'{name}: continuous motion, dimensions={expected}, clients={after.count("j/clients")} in 0.55 s',flush=True)


def main(out,rest_ms):
    out.mkdir(parents=True,exist_ok=True)
    for p in out.glob('reticle.*'):p.unlink()
    os.environ.update(QUADRILLE_NESTED_DIR=str(out/'nested'),QUADRILLE_BAR_SOCKET=f'quadrille-overlay-test-{os.getpid()}.sock')
    home=out/'home';(home/'.config/quadrille').mkdir(parents=True,exist_ok=True)
    (home/'.config/quadrille/displays.toml').write_text('[QA]\nwidth_mm=344.6265107477\nheight_mm=215.3915692173\n[QB]\nwidth_mm=796.620037497\nheight_mm=333.468852906\n')
    theme=out/'current';theme.mkdir(exist_ok=True);(theme/'theme.name').write_text('quadrille-terminal')
    capture=out/'reticle.capture';capture.touch()
    run(N,'up');nc('eval','hl.config({animations={enabled=false}})');move('QB',420,300)
    env=dict(os.environ,HOME=str(home),XDG_CONFIG_HOME=str(home/'.config'),QUADRILLE_RETICLE_DUMP=str(out/'reticle'),QUADRILLE_COMMANDS=str(ROOT/'tools/stubs'),
             QUADRILLE_STUB_STATE=str(out/'stubs'),RUST_LOG='quadrille_bar=debug' if rest_ms else 'warn')
    host=launch([N,'run',BIN,'--no-bar','--overlay-rest-ms',rest_ms,'--theme-dir',theme],out/'host.log',env)
    wait(lambda:'on' in ctl('overlay','status'));move('QB',420,300)
    wait(lambda:current(out,'QB',True))
    pid=int(re.search(r'namespace: quadrille-dimensions, pid: (\d+)',layers())[1])
    evidence=[];timings=[]
    for name in ['QA','QB']:
        motion_check(out,name,rest_ms)
        for x,y in [(103,207),(700,500)]:
            started=time.monotonic();move(name,x,y)
            if rest_ms:
                wait(lambda:current(out,name,False));wait(lambda:'quadrille-dimensions' not in layers())
                e,_=raster_check(out,name,f'{name}-calm-reticle',False);evidence.append(e)
            wait(lambda:current(out,name,True));elapsed=time.monotonic()-started
            assert elapsed<(rest_ms/1000+.5),elapsed
            if rest_ms:assert elapsed>=rest_ms/1000,elapsed
            timings.append(elapsed)
            e,_=raster_check(out,name,f'{name}-{x}-{y}');evidence.append(e)
    if rest_ms:
        decisions=[int(n) for n in re.findall(r'cursor rested (\d+) ms',(out/'host.log').read_text())]
        assert decisions and all(rest_ms<=n<rest_ms+35 for n in decisions),decisions
        (out/'evidence.json').write_text(json.dumps({'timings':timings,'comparisons':evidence,'rest_ms':decisions},indent=2))
        assert all(e.get('bad',0)==0 for e in evidence),evidence
        ctl('quit');return
    # Known floating rectangle, independently commanded then checked via j/clients.
    move('QB',450,300)
    fixture=launch([N,'run','foot','--app-id=quadrille-dimension-fixture','-e','sleep','50'],out/'fixture.log')
    active=wait(lambda:json.loads(nc('activewindow','-j')).get('pid'))
    for expr in ['hl.dsp.window.float({action="set"})','hl.dsp.window.resize({x=600,y=400,relative=false})',
                 'hl.dsp.window.move({x=30100,y=100,relative=false})']:
        nc('dispatch',expr)
    time.sleep(.25)
    c=next(c for c in json.loads(nc('clients','-j')) if c['pid']==active)
    assert c['at']==[30100,100] and c['size']==[600,400],(c['at'],c['size'])
    move('QB',450,300);wait(lambda:current(out,'QB',True));time.sleep(.1)
    e,info=raster_check(out,'QB','window-600x400');evidence.append(e)
    measured={(d['kind'],d['axis']):d['mm'] for d in info['dimensions']}
    assert measured[('W','x')]==round(250*796.620037497/3440),measured
    assert measured[('W','y')]==round(200*333.468852906/1440),measured
    # Motion over the window updates both known distances in whole millimetres.
    move('QB',460,310);time.sleep(.08);info,_=frame(out)
    updated={(d['kind'],d['axis']):d['mm'] for d in info['dimensions']}
    assert updated[('W','x')]==round(240*796.620037497/3440),updated
    assert updated[('W','y')]==round(190*333.468852906/1440),updated
    assert all('.' not in d['text'] for d in info['dimensions'])
    print('known window distances:',measured,'then',updated,flush=True)
    # Empty input region: a nested click passes through and focuses the fixture.
    move('QB',450,300);run(N,'run','env','VPTR_EXTENT=3440x1440',ROOT/'target/release/vptr','QB','click','sleep','50')
    assert json.loads(nc('activewindow','-j'))['pid']==active
    nc('dispatch','hl.dsp.window.fullscreen({mode="fullscreen",action="set"})')
    wait(lambda:'fullscreen' in ctl('overlay','status'));wait(hidden)
    capture.unlink();rows=[cost(pid,'QB_suppressed')]
    os.killpg(fixture.pid,signal.SIGTERM);fixture.wait(timeout=2)
    capture.touch();move('QB',420,300);wait(lambda:current(out,'QB',True))
    # Session lock belongs exclusively to the nested compositor; no PAM.
    lock=out/'lock.qml';lock.write_text('import Quickshell\nimport Quickshell.Wayland\nimport Quickshell.Io\nShellRoot { WlSessionLock { id: l; locked: false; WlSessionLockSurface {color: "black"} } IpcHandler { target: "overlaytest"; function lock(): void { l.locked=true } function unlock(): void { l.locked=false } } }')
    locker=launch([N,'run','quickshell','-p',lock],out/'lock.log');time.sleep(.4)
    run(N,'run','quickshell','ipc','-p',lock,'call','overlaytest','lock');wait(lambda:'locked' in ctl('overlay','status'));wait(hidden)
    run(N,'run','quickshell','ipc','-p',lock,'call','overlaytest','unlock');time.sleep(.25)
    move('QB',420,300);wait(lambda:current(out,'QB',True))
    for label in ['before-hotplug','after-hotplug']:
        if label.startswith('after'):
            nc('output','remove','QB');time.sleep(.12);assert 'output QB' not in ctl('overlay','status')
            nc('output','create','headless','QB');time.sleep(.4);move('QB',420,300)
        e,_=raster_check(out,'QB',label);evidence.append(e)
    # Scale changes realign the output grid; theme changes restyle stationary marks.
    nc('eval','hl.monitor({output="QA",mode="2560x1600@60",position="20000x0",scale=1})')
    move('QA',420,300);wait(lambda:frame(out)[0]['scale']==1)
    e,_=raster_check(out,'QA','QA-scale-1');evidence.append(e)
    nc('eval','hl.monitor({output="QA",mode="2560x1600@60",position="20000x0",scale=1.666667})')
    move('QA',420,300);wait(lambda:abs(frame(out)[0]['scale']-5/3)<.001)
    e,_=raster_check(out,'QA','QA-scale-restored');evidence.append(e)
    before=frame(out)[1];(theme/'theme.name').write_text('quadrille-paper')
    wait(lambda:frame(out)[1]!=before)
    e,_=raster_check(out,'QA','QA-paper');evidence.append(e)
    (theme/'theme.name').write_text('quadrille-terminal');wait(lambda:frame(out)[1]==before)
    # Use a fresh host without dump allocations or tracing for the cost samples.
    capture.unlink()
    ctl('quit');host.wait(timeout=2)
    env.pop('QUADRILLE_RETICLE_DUMP')
    host=launch([N,'run',BIN,'--no-bar','--theme-dir',theme],out/'cost-host.log',env)
    wait(lambda:'on' in ctl('overlay','status'));move('QA',420,300)
    wait(lambda:'quadrille-dimensions' in layers())
    pid=int(re.search(r'namespace: quadrille-dimensions, pid: (\d+)',layers())[1])
    # Continuous motion outlasts each sample, with dimensioning still enabled.
    for name in ['QA','QB']:
        move(name,400,300,moving=True);time.sleep(.25)
        assert f'output {name}' in ctl('overlay','status') and 'quadrille-dimensions' in layers()
        rows.append(cost(pid,name+'_moving'))
        assert holder.poll() is None and 'quadrille-dimensions' in layers()
        move(name,420,300);time.sleep(.6)
        rows.append(cost(pid,name+'_still'))
        ctl('overlay','off');time.sleep(.2);wait(hidden);rows.append(cost(pid,name+'_off'))
        ctl('overlay','on');time.sleep(.15)
    (out/'cost.json').write_text(json.dumps(rows,indent=2))
    (out/'evidence.json').write_text(json.dumps({'timings':timings,'comparisons':evidence,'window':str(measured)},indent=2))
    assert all(e.get('bad',0)==0 for e in evidence),evidence
    for row in rows:print(f"{row['state']}: {row['cpu_percent']:.3f}% core, {row['rss_mib']:.2f} MiB RSS",flush=True)
    for name in ['QA','QB']:
        states={r['state'].split('_',1)[1]:r for r in rows if r['state'].startswith(name+'_')}
        assert states['moving']['cpu_percent']<8,states
        assert states['still']['cpu_percent']<.5 and states['off']['cpu_percent']<.1,states
        assert states['still']['rss_mib']-states['off']['rss_mib']>10,states
    ctl('quit')


if __name__=='__main__':
    ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('out',type=Path);ap.add_argument('--locked',action='store_true')
    ap.add_argument('--rest-ms',type=int,default=0);args=ap.parse_args();out=args.out.resolve()
    if not args.locked:
        status=0
        for mode,rest in [('live',0),('calm',300)]:
            status|=subprocess.call(['flock','-o','-w','900','/tmp/quadrille-live.lock','timeout','-k','2','55',sys.executable,__file__,str(out/mode),'--locked','--rest-ms',str(rest)])
        sys.exit(status)
    # timeout must run cleanup too, and every child starts without the lock fd.
    for sig in [signal.SIGTERM,signal.SIGINT]:signal.signal(sig,lambda *_:sys.exit(130))
    try:main(out,args.rest_ms)
    finally:
        for child in reversed(children):
            if child.poll() is None:
                try:os.killpg(child.pid,signal.SIGTERM);child.wait(timeout=2)
                except (ProcessLookupError,subprocess.TimeoutExpired):pass
        subprocess.run([str(N),'down'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
