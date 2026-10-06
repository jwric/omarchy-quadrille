#!/usr/bin/env python3
"""Plan empty workspaces and guard every live wallpaper capture."""
import json
from pathlib import Path
import re
import subprocess
import sys

from PIL import Image


def workspace_plan(monitors, workspaces):
    if not monitors:
        raise ValueError("no outputs")
    by_id = {w["id"]: w for w in workspaces}
    used = set(by_id) | {m["activeWorkspace"]["id"] for m in monitors}
    planned = []
    for monitor in monitors:
        name = monitor["name"]
        if not re.fullmatch(r"[A-Za-z0-9_-]+", name):
            raise ValueError("unsupported output name")
        current = monitor["activeWorkspace"]["id"]
        if current <= 0 or monitor["specialWorkspace"]["id"] != 0:
            raise ValueError(f"{name}: a special workspace is visible")
        workspace = by_id.get(current)
        if workspace and workspace.get("windows") == 0 and workspace.get("monitor") == name:
            target = current
        else:
            target = next(i for i in range(8, 100000) if i not in used)
            used.add(target)
        planned.append({"name": name, "original": current, "target": target})
    return planned


def require_empty(monitors, workspaces, expected_names):
    if {m["name"] for m in monitors} != set(expected_names):
        raise ValueError("the output set changed during the preview")
    by_id = {w["id"]: w for w in workspaces}
    for monitor in monitors:
        workspace = by_id.get(monitor["activeWorkspace"]["id"])
        if workspace is None or workspace.get("windows") != 0 or monitor["specialWorkspace"]["id"] != 0:
            raise ValueError(f'{monitor["name"]}: the actual displayed workspace is not empty')


def live_state():
    monitors = json.loads(subprocess.check_output(["hyprctl", "monitors", "-j"]))
    workspaces = json.loads(subprocess.check_output(["hyprctl", "workspaces", "-j"]))
    return monitors, workspaces


def geometry(monitor):
    return tuple(monitor.get(k) for k in ("width", "height", "scale", "transform"))


def capture(planned, work, out):
    names = [m["name"] for m in planned]
    written = []
    try:
        for item in planned:
            before, workspaces = live_state()
            require_empty(before, workspaces, names)
            before_monitor = next(m for m in before if m["name"] == item["name"])
            raw = work / f'{item["name"]}.png'
            subprocess.run(["grim", "-o", item["name"], str(raw)], check=True)
            after, workspaces = live_state()
            require_empty(after, workspaces, names)
            after_monitor = next(m for m in after if m["name"] == item["name"])
            if geometry(before_monitor) != geometry(after_monitor):
                raise ValueError("output geometry changed during a grab")
            image = Image.open(raw)
            phys = max(1, round(2 * after_monitor["scale"]))
            # Discard the whole live bar, including workspace/app/status text.
            result = out / f'live-{item["name"]}.png'
            image.crop((0, 20 * phys, image.width, image.height)).save(result)
            written.append(result)
        monitors, workspaces = live_state()
        require_empty(monitors, workspaces, names)
    except Exception:
        for result in written:
            result.unlink(missing_ok=True)
        raise


def command(field, value):
    return "hyprctl dispatch 'hl.dsp.focus({ " + field + " = " + json.dumps(str(value)) + " })' >/dev/null"


def main():
    mode, directory = sys.argv[1:3]
    work = Path(directory)
    if mode == "plan":
        monitors = json.loads((work / "monitors.json").read_text())
        workspaces = json.loads((work / "workspaces.json").read_text())
        planned = workspace_plan(monitors, workspaces)
        focus = next((m["name"] for m in monitors if m.get("focused")), monitors[0]["name"])
        show, restore = [], []
        for item in planned:
            show += [command("monitor", item["name"]), command("workspace", item["target"])]
            restore += [command("monitor", item["name"]), command("workspace", item["original"])]
        restore.append(command("monitor", focus))
        (work / "show.sh").write_text("\n".join(show) + "\n")
        (work / "restore.sh").write_text("\n".join(restore) + "\n")
        (work / "captures.json").write_text(json.dumps(planned))
    elif mode == "guard":
        planned = json.loads((work / "captures.json").read_text())
        monitors, workspaces = live_state()
        require_empty(monitors, workspaces, [m["name"] for m in planned])
    elif mode == "capture":
        capture(json.loads((work / "captures.json").read_text()), work, Path(sys.argv[3]))
    else:
        raise ValueError("expected plan, guard or capture")


if __name__ == "__main__":
    main()
