#!/usr/bin/env python3
"""Check native buffers against the nested output, then measure host threads."""
import json
import atexit
import math
import os
import re
import signal
from pathlib import Path
import subprocess
import sys
import time

from PIL import Image, ImageChops, ImageDraw

HERE, OUT, PID = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3])
N = str(HERE / "tools/nested.sh")
BIN = str(HERE / "target/release/quadrille-bar")
CHILDREN = []
MOVERS = []


def stop_child(child):
    def descendants(pid):
        try:
            children = [int(p) for p in Path(f"/proc/{pid}/task/{pid}/children").read_text().split()]
        except OSError:
            return []
        return [p for kid in children for p in descendants(kid)] + children
    for pid in descendants(child.pid) + [child.pid]:
        try:
            os.kill(pid,signal.SIGTERM)
        except ProcessLookupError:
            pass
    try:
        child.wait(timeout=2)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait()


def cleanup_children():
    for child in CHILDREN:
        if child.poll() is None:
            stop_child(child)
            try:
                child.wait(timeout=1)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait()


atexit.register(cleanup_children)


def run(*args, **kwargs):
    return subprocess.check_output([str(a) for a in args], text=True, **kwargs).strip()


def ctl(*args):
    return run(BIN, "ctl", *args)


def cursor_queries():
    return (OUT / "reticle.polls").read_text().splitlines().count("j/cursorpos")


def frame():
    metadata, pixels = (OUT / "reticle.frame").read_bytes().split(b"\n", 1)
    return json.loads(metadata), pixels


def wait_output(name, ps=None, scale=None, cursor=None):
    for _ in range(40):
        try:
            value, _ = frame()
            if value["output"] == name and (ps is None or value["vpx"] == ps) and (scale is None or abs(value["scale"] - scale) < 0.00001) and (cursor is None or value["cursor"] == cursor):
                return value
        except (OSError, ValueError):
            pass
        time.sleep(0.05)
    raise AssertionError(f"no reticle on {name} at pixel scale {ps}: {ctl('overlay', 'status')}")


def move(output, x, y, extent):
    for old in MOVERS:
        if old.poll() is None:
            stop_child(old)
    MOVERS.clear()
    arguments=[N,"run","env",f"VPTR_EXTENT={extent}",str(HERE/"target/release/vptr"),output]
    for _ in range(150):
        arguments += ["move",str(x),str(y),"sleep","16"]
    mover = subprocess.Popen(arguments, stdout=subprocess.DEVNULL)
    CHILDREN.append(mover)
    MOVERS.append(mover)
    time.sleep(0.3)
    return mover


def verify(name, x, y, extent, label):
    mover = move(name, x, y, extent)
    monitor = next(m for m in json.loads(run(N,"ctl","monitors","-j")) if m["name"] == name)
    ew, eh = map(int, extent.split("x"))
    scale = monitor["scale"]
    expected = [monitor["x"] + math.floor(x * monitor["width"] / scale / ew), monitor["y"] + math.floor(y * monitor["height"] / scale / eh)]
    info = wait_output(name, cursor=expected)
    time.sleep(0.08)
    info, pixels = frame()
    assert info["output"] == name and info["cursor"] == expected, (label,info,expected)
    (OUT / f"{label}.argb").write_bytes(pixels)
    (OUT / f"{label}.json").write_text(json.dumps(info, indent=2))
    width, height = info["size"]
    raw = Image.frombytes("RGBA", (width, height), (OUT / f"{label}.argb").read_bytes(), "raw", "BGRA")
    raw.save(OUT / f"{label}-buffer.png")
    run(N, "ctl", "dismissnotify")
    shot = OUT / f"{label}-output.png"
    run(N, "run", "grim", "-o", name, shot)
    image = Image.open(shot).convert("RGB")
    (OUT / f"{label}-layers.txt").write_text(run(N, "ctl", "layers"))
    ps, scale = info["vpx"], info["scale"]
    left, top = [math.floor(value * scale + 0.5) for value in info["margin"]]
    centre = [info["centre"][i] * ps + info["offset"][i] + [left, top][i] for i in (0, 1)]
    assert centre == info["snapped"], (centre, info)
    mask = raw.getchannel("A")
    draw = ImageDraw.Draw(mask)
    draw.rectangle((centre[0]-left-3, centre[1]-top-3, centre[0]-left+47, centre[1]-top+47), fill=0)
    # Mask the cropped physical edges of outputs.
    visible = Image.new("L", (width, height))
    ImageDraw.Draw(visible).rectangle((max(0,-left),max(0,-top),min(width-1,image.width-left-1),min(height-1,image.height-top-1)),fill=255)
    mask = ImageChops.multiply(mask, visible)
    for attempt in range(10):
        difference = ImageChops.difference(raw.convert("RGB"), image.crop((left,top,left+width,top+height)))
        r, g, b = difference.split()
        difference = ImageChops.multiply(ImageChops.lighter(ImageChops.lighter(r,g),b),mask)
        bad = sum(difference.histogram()[1:])
        if not bad:
            break
        time.sleep(0.12)
        run(N, "run", "grim", "-o", name, shot)
        image = Image.open(shot).convert("RGB")
    if bad:
        image.crop((left,top,left+width,top+height)).save(OUT / f"{label}-failure.png")
        shot.unlink()
        raise AssertionError(f"{label}: {bad} opaque pixels differ from the native buffer")
    x0, y0 = max(0, centre[0] - 165 * ps), max(0, centre[1] - 55 * ps)
    x0, y0 = x0 // ps * ps, y0 // ps * ps
    x1, y1 = min(image.width // ps * ps, centre[0] + 165 * ps), min(image.height // ps * ps, centre[1] + 55 * ps)
    crop = image.crop((x0, y0, x1, y1))
    crop.save(OUT / f"{label}.png")
    # The readout plate is free of the compositor's own cursor sprite.
    plate_x = 0 if info["centre"][0] == 151 else 18
    plate_y = 1 if info["centre"][1] == 41 else 18
    px0, py0 = left + info["offset"][0] + plate_x * ps, top + info["offset"][1] + plate_y * ps
    plate = image.crop((px0, py0, px0 + 142 * ps, py0 + 30 * ps))
    plate.save(OUT / f"{label}-plate.png")
    run("python3", HERE / "tools/crisp.py", OUT / f"{label}-plate.png", f"0,0,{plate.width},{plate.height}", ps, label, "--check")
    shot.unlink()
    stop_child(mover)
    print(f"ok {label}: containing virtual pixel {centre}, exact residual {info['offset']}, native/screenshot pixels equal")


def cpu_ns():
    return sum(int(p.read_text().split()[0]) for p in Path(f"/proc/{PID}/task").glob("*/schedstat"))


def rss_kb():
    for line in Path(f"/proc/{PID}/status").read_text().splitlines():
        if line.startswith("VmRSS:"):
            return int(line.split()[1])


def cost(label, mover=None):
    before, started = cpu_ns(), time.monotonic()
    process = subprocess.Popen(mover, stdout=subprocess.DEVNULL) if mover else None
    time.sleep(6)
    if process:
        process.wait(timeout=1)
    seconds = time.monotonic() - started
    ns = cpu_ns() - before
    return {"state": label, "seconds": seconds, "cpu_ms": ns / 1e6,
            "cpu_percent": ns / 1e9 / seconds * 100, "rss_kb": rss_kb()}


for _ in range(50):
    try:
        ctl("overlay", "status")
        break
    except subprocess.CalledProcessError:
        time.sleep(0.1)

if not os.environ.get("QUADRILLE_OVERLAY_CONTROLS_ONLY"):
    verify("QA", 103, 207, "1536x960", "QA-residual")
    PID = int(re.search(r"namespace: quadrille-reticle, pid: (\d+)", run(N, "ctl", "layers")).group(1))
    verify("QA", 39, 99, "15360x9600", "QA-fractional")
    fractional = json.loads((OUT / "QA-fractional.json").read_text())
    assert fractional["cursor"] == [20003.0, 9.0], fractional
    assert fractional["snapped"] == [3, 15], fractional
    assert [math.floor(value * 5 / 3 / 3) * 3 for value in (3.9, 9.9)] == [6, 15]
    print("ok precision limit proven: actual fractional pointer vpx [6,15], IPC-reported vpx [3,15]")
    verify("QA", 1500, 935, "1536x960", "QA-flip")
    verify("QB", 113, 209, "3440x1440", "QB-residual")
    verify("QB", 3380, 1370, "3440x1440", "QB-flip")
    assert "off" in ctl("overlay", "off")
    time.sleep(0.15)
    assert "quadrille-reticle" not in run(N, "ctl", "layers")
    assert "on" in ctl("overlay", "on")
    scale_mover = move("QA", 400, 250, "1536x960")
    for scale in [1, 2, 1.25, 1.666667]:
        run(N, "ctl", "eval", f'hl.monitor({{output="QA",mode="2560x1600@60",position="20000x0",scale={scale}}})')
        wait_output("QA", math.floor(2 * scale + 0.5), scale)
        print(f"ok stationary pointer follows QA scale {scale}")
    stop_child(scale_mover)
    run(N, "ctl", "output", "remove", "QB")
    time.sleep(0.15)
    run(N, "ctl", "output", "create", "headless", "QB")
    time.sleep(0.65)
    verify("QB", 420, 300, "3440x1440", "QB-hotplug")

else:
    mover = move("QB",420,300,"3440x1440")
    ctl("overlay","off")
    ctl("overlay","on")
    wait_output("QB")
    PID = int(re.search(r"namespace: quadrille-reticle, pid: (\d+)", run(N,"ctl","layers")).group(1))
    stop_child(mover)

# The reticle never claims pointer or keyboard input from the panel beneath it.
ctl("summon", "audio", '{"output":"QB"}')
time.sleep(0.45)
run(N, "run", "wtype", "-k", "Home")
time.sleep(0.1)
assert "key Home" in (OUT / "host.log").read_text()
fx, fy, fw, fh = map(float, ctl("find", "mpv").split())
layers = run(N, "ctl", "layers")
box = re.search(r"xywh: (-?\d+) (-?\d+) (\d+) (\d+), a: \d+, namespace: quadrille-audio", layers)
assert box, layers
mx, my = map(int, box.groups()[:2])
output = next(m for m in json.loads(run(N, "ctl", "monitors", "-j")) if m["name"] == "QB")
qx, qy = float(output["x"]), float(output["y"])
cx, cy = round(mx - qx + (fx + fw / 2) * 2), round(my - qy + (fy + fh / 2) * 2)
run(N, "run", "env", "VPTR_EXTENT=3440x1440", HERE / "target/release/vptr", "QB", "move", cx, cy, "sleep", 250, "click", "sleep", 250)
assert "pactl set-sink-input-mute 42 toggle" in (OUT / "stubs/calls.log").read_text()
ctl("hide")
time.sleep(0.2)
print("ok reticle passes pointer click and keyboard to the underlying panel")

# A blank nested window proves compositor fullscreen inhibition.
foot_log = open(OUT / "fullscreen.log", "w")
foot = subprocess.Popen([N, "run", "foot", "--app-id=quadrille-overlay-fixture", "-e", "sleep", "15"], stdout=foot_log, stderr=foot_log)
CHILDREN.append(foot)
time.sleep(0.4)
active = json.loads(run(N,"ctl","activewindow","-j"))
assert active["class"] == "quadrille-overlay-fixture", active.get("class")
run(N, "ctl", "dispatch", 'hl.dsp.window.fullscreen({mode="fullscreen"})')
time.sleep(0.25)
assert "fullscreen" in ctl("overlay", "status")
assert "quadrille-reticle" not in run(N, "ctl", "layers")
before = cursor_queries()
time.sleep(0.25)
assert cursor_queries() == before
os.kill(active["pid"],signal.SIGTERM)
foot.wait(timeout=2)
time.sleep(0.25)
print("ok fullscreen unmaps the reticle and leaving it resumes")

# This is only the nested compositor's lock, with no PAM or real-session action.
lock_qml = OUT / "lock.qml"
lock_qml.write_text('''import Quickshell
import Quickshell.Wayland
import Quickshell.Io
ShellRoot {
  WlSessionLock { id: sessionLock; locked: false
    WlSessionLockSurface { color: "black" }
  }
  IpcHandler { target: "reticletest"
    function lock(): void { sessionLock.locked = true }
    function unlock(): void { sessionLock.locked = false }
  }
}
''')
lock_log = open(OUT / "lock.log", "w")
locker = subprocess.Popen([N, "run", "quickshell", "-p", str(lock_qml)], stdout=lock_log, stderr=lock_log)
CHILDREN.append(locker)
time.sleep(0.5)
run(N, "run", "quickshell", "ipc", "-p", lock_qml, "call", "reticletest", "lock")
time.sleep(0.4)
assert json.loads(run(N, "ctl", "locked", "-j"))["locked"]
assert "locked" in ctl("overlay", "status")
assert "quadrille-reticle" not in run(N, "ctl", "layers")
before = cursor_queries()
time.sleep(0.25)
assert cursor_queries() == before
run(N, "run", "quickshell", "ipc", "-p", lock_qml, "call", "reticletest", "unlock")
time.sleep(0.4)
assert not json.loads(run(N, "ctl", "locked", "-j"))["locked"]
stop_child(locker)
locker.wait(timeout=2)
mover = move("QB", 420, 300, "3440x1440")
wait_output("QB",cursor=[qx+420,qy+300])
stop_child(mover)
assert "output QB" in ctl("overlay", "status")
print("ok nested session lock unmaps the reticle and unlocking resumes")
(OUT / "reticle.capture").unlink()
time.sleep(0.5)
numbers = [cost("idle")]
args = [N, "run", "env", "VPTR_EXTENT=3440x1440", str(HERE / "target/release/vptr"), "QB"]
for index in range(350):
    args += ["move", str(400 + index % 200), "300", "sleep", "16"]
numbers += [cost("moving", args)]
ctl("overlay", "off")
time.sleep(0.3)
numbers += [cost("off")]
(OUT / "cost.json").write_text(json.dumps(numbers, indent=2))
for row in numbers:
    print(f"{row['state']}: {row['cpu_ms']:.2f} ms / {row['seconds']:.2f} s = {row['cpu_percent']:.3f}% core; RSS {row['rss_kb']/1024:.2f} MiB")
assert numbers[0]["cpu_percent"] < 0.5
assert numbers[1]["cpu_percent"] < 5
assert numbers[2]["cpu_ms"] < 5
ctl("quit")
