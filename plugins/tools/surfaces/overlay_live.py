#!/usr/bin/env python3
"""An owned host, empty outputs, and a tightly cropped live reticle preview."""
import json
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import threading
import time

from PIL import Image

from wallpaper_live import geometry, live_state, require_empty, workspace_plan


class ProtocolEvidence:
    """Keep only numeric region/layer facts; never retain the raw debug stream."""
    def __init__(self):
        self.regions = {}
        self.inputs = {}
        self.layers = {}
        self.lock = threading.Lock()

    def feed(self, line):
        match = re.search(r"-> (\w+)[@#](\d+)\.(\w+)\((.*)\)\s*$", line)
        if not match:
            return
        interface, ident, method, args = match.groups()
        ident = int(ident)
        refs = {kind: int(value) for kind, value in re.findall(r"(\w+)[@#](\d+)", args)}
        with self.lock:
            if interface == "wl_compositor" and method == "create_region" and "wl_region" in refs:
                self.regions[refs["wl_region"]] = False
            elif interface == "wl_region" and method == "add":
                self.regions[ident] = True
            elif interface == "wl_surface" and method == "set_input_region":
                region = refs.get("wl_region")
                self.inputs.setdefault(ident, []).append(region in self.regions and not self.regions[region])
            elif interface == "zwlr_layer_shell_v1" and method == "get_layer_surface" and '"quadrille-reticle"' in args:
                if "zwlr_layer_surface_v1" not in refs or "wl_surface" not in refs:
                    return
                parts = args.split(",")
                self.layers[refs["zwlr_layer_surface_v1"]] = {
                    "surface": refs["wl_surface"], "overlay_layer": parts[-2].strip() == "3",
                    "keyboard": None, "exclusive_zone": None,
                }
            elif interface == "zwlr_layer_surface_v1" and ident in self.layers:
                if method == "set_keyboard_interactivity" and re.fullmatch(r"\d+", args.strip()):
                    self.layers[ident]["keyboard"] = int(args)
                elif method == "set_exclusive_zone" and re.fullmatch(r"-?\d+", args.strip()):
                    self.layers[ident]["exclusive_zone"] = int(args)

    def snapshot(self):
        with self.lock:
            layers = []
            for value in self.layers.values():
                inputs = self.inputs.get(value["surface"], [])
                layers.append({**value, "namespace": "quadrille-reticle", "input_region_updates": len(inputs),
                    "empty_input_region": bool(inputs) and all(inputs)})
            return {"layers": layers, "raw_protocol_retained": False}

    def consume(self, stream):
        for line in stream:
            self.feed(line)


def layer_rectangles(value, pid):
    found = []
    for name, output in value.items():
        for level, layers in output.get("levels", {}).items():
            for layer in layers:
                if layer.get("namespace") != "quadrille-reticle" or layer.get("pid") != pid:
                    continue
                if level != "3" or float(layer.get("alpha", 0)) <= 0:
                    raise ValueError("the owned reticle is not a visible overlay layer")
                rect = tuple(float(layer[k]) for k in ("x", "y", "w", "h"))
                if not all(math.isfinite(v) for v in rect) or rect[2] <= 0 or rect[3] <= 0:
                    raise ValueError("invalid reticle layer geometry")
                found.append({"output": name, "rect": rect, "namespace": "quadrille-reticle", "pid": pid})
    return found


def layers(pid):
    return layer_rectangles(json.loads(subprocess.check_output(["hyprctl", "layers", "-j"], timeout=2)), pid)


def crop_box(rect, monitor, size):
    if monitor.get("transform", 0) != 0:
        raise ValueError("live preview crop supports unrotated outputs only")
    scale = monitor["scale"]
    x, y, w, h = rect
    left = math.floor((x - monitor["x"]) * scale)
    top = math.floor((y - monitor["y"]) * scale)
    right = math.ceil((x + w - monitor["x"]) * scale)
    bottom = math.ceil((y + h - monitor["y"]) * scale)
    # Drop the complete live bar even if the actual pointer is beside it.
    bar = 20 * max(1, math.floor(2 * scale + 0.5))
    box = (max(0, left), max(bar, top), min(size[0], right), min(size[1], bottom))
    if box[2] <= box[0] or box[3] <= box[1]:
        raise ValueError("the reticle is entirely under the excluded live bar")
    return box


def capture(planned, work, out, pid):
    names = [m["name"] for m in planned]
    written = []
    try:
        rectangles = layers(pid)
        if not rectangles:
            raise ValueError("no owned reticle is mapped; leave the pointer on an output before the preview")
        for item in rectangles:
            before, workspaces = live_state()
            require_empty(before, workspaces, names)
            monitor = next(m for m in before if m["name"] == item["output"])
            raw = work / "reticle-output.png"
            subprocess.run(["grim", "-o", item["output"], str(raw)], check=True, timeout=2)
            after, workspaces = live_state()
            require_empty(after, workspaces, names)
            after_monitor = next(m for m in after if m["name"] == item["output"])
            if geometry(monitor) != geometry(after_monitor) or (monitor["x"], monitor["y"]) != (after_monitor["x"], after_monitor["y"]):
                raise ValueError("output geometry changed during a grab")
            if layers(pid) != rectangles:
                raise ValueError("the actual pointer moved during the grab; retry while it is still")
            with Image.open(raw) as image:
                box = crop_box(item["rect"], after_monitor, image.size)
                result = out / f'live-overlay-{item["output"]}.png'
                image.crop(box).save(result)
            raw.unlink()
            written.append(result)
        monitors, workspaces = live_state()
        require_empty(monitors, workspaces, names)
        return rectangles
    except BaseException:
        for result in written:
            result.unlink(missing_ok=True)
        (work / "reticle-output.png").unlink(missing_ok=True)
        raise


def focus(field, value):
    argument = "hl.dsp.focus({ " + field + " = " + json.dumps(str(value)) + " })"
    subprocess.run(["hyprctl", "dispatch", argument], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=2)


def ctl(binary, env, *args):
    result = subprocess.check_output([str(binary), "ctl", *args], env=env, text=True,
        stderr=subprocess.DEVNULL, timeout=1)
    if not re.fullmatch(r"overlay (?:on(?: \((?:locked|fullscreen|no pointer output)\))?|off) output [A-Za-z0-9_-]+\n", result):
        if args == ("quit",):
            return "quit"
        raise ValueError("unexpected overlay control response")
    return result.strip()


def stop_signal(signum, frame):
    raise InterruptedError("preview interrupted")


def main():
    repo, out = Path(sys.argv[1]), Path(sys.argv[2])
    binary = repo / "layershell/target/release/quadrille-bar"
    if not binary.is_file():
        raise ValueError("build the release host before the preview")
    out.mkdir(parents=True, exist_ok=True)
    for signum in (signal.SIGINT, signal.SIGTERM):
        signal.signal(signum, stop_signal)
    monitors, workspaces = live_state()
    planned = workspace_plan(monitors, workspaces)
    original_focus = next((m["name"] for m in monitors if m.get("focused")), monitors[0]["name"])
    names = [m["name"] for m in planned]
    env = dict(os.environ, QUADRILLE_BAR_SOCKET=f"quadrille-overlay-preview-{os.getpid()}.sock",
        WAYLAND_DEBUG="client", RUST_LOG="warn")
    socket = Path(env.get("XDG_RUNTIME_DIR", tempfile.gettempdir())) / env["QUADRILLE_BAR_SOCKET"]
    evidence = ProtocolEvidence()
    host = None
    reader = None
    try:
        for item in planned:
            focus("monitor", item["name"])
            focus("workspace", item["target"])
        actual, current = live_state()
        require_empty(actual, current, names)
        host = subprocess.Popen([str(binary), "--no-bar"], env=env, stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE, text=True, close_fds=True)
        reader = threading.Thread(target=evidence.consume, args=(host.stderr,), daemon=True)
        reader.start()
        status = None
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if host.poll() is not None:
                raise ValueError("the disposable overlay host exited")
            try:
                status = ctl(binary, env, "overlay", "status")
                if status.startswith("overlay on output ") and layers(host.pid):
                    break
            except (OSError, ValueError, subprocess.SubprocessError):
                pass
            time.sleep(0.05)
        else:
            raise ValueError("no mapped reticle from the disposable overlay host")
        time.sleep(0.15)
        snapshot = evidence.snapshot()
        if not snapshot["layers"] or not all(v["empty_input_region"] and v["overlay_layer"] and v["keyboard"] == 0 and v["exclusive_zone"] == -1 for v in snapshot["layers"]):
            raise ValueError("empty input region and passive overlay protocol were not observed")
        with tempfile.TemporaryDirectory() as directory:
            rectangles = capture(planned, Path(directory), out, host.pid)
        (out / "overlay-status.txt").write_text(status + "\n")
        (out / "overlay-protocol.json").write_text(json.dumps({**snapshot, "mapped_layers": rectangles}, indent=2) + "\n")
        print("owned overlay captured; empty input region, keyboard none and overlay layer observed")
    except BaseException:
        for path in out.glob("live-overlay-*.png"):
            path.unlink(missing_ok=True)
        raise
    finally:
        if host is not None:
            for command in [("overlay", "off"), ("quit",)]:
                try:
                    ctl(binary, env, *command)
                except (OSError, ValueError, subprocess.SubprocessError):
                    pass
            try:
                host.wait(timeout=1)
            except subprocess.TimeoutExpired:
                host.terminate()
                try:
                    host.wait(timeout=1)
                except subprocess.TimeoutExpired:
                    host.kill()
                    host.wait()
            if reader:
                reader.join(timeout=1)
            socket.unlink(missing_ok=True)
        for item in planned:
            try:
                focus("monitor", item["name"])
                focus("workspace", item["original"])
            except subprocess.SubprocessError:
                pass
        try:
            focus("monitor", original_focus)
        except subprocess.SubprocessError:
            pass


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError, InterruptedError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
