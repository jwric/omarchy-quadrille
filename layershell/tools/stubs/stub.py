#!/usr/bin/env python3
"""Stands in for the programs the panels run, so that they can be tested with
real clicks and keys without changing the machine.

Each program of the panels (pactl, nmcli, bluetoothctl, powerprofilesctl, and
the Omarchy commands they call) is a link to this file, run through the
scripts beside it. A call is written to `calls.log` in the state directory
(`QUADRILLE_STUB_STATE`), one line, the program and its words; what is read
comes from `state.json` there; and what is changed (a default sink, a volume, a
network joined, a device connected) changes `state.json`, so that the panel
sees it on its next reading, the way it would see the real thing. Nothing here
runs anything else: the power actions are only written down.

The state starts as the fixtures in crates/bar/fixtures and the data below.
"""
import fcntl
import json
import os
import sys
import time

STATE = os.environ.get("QUADRILLE_STUB_STATE")
HERE = os.path.dirname(os.path.realpath(__file__))
FIXTURES = os.path.join(HERE, "..", "..", "crates", "bar", "fixtures")

# The password of the network that asks for one.
PASSWORD = "hunter2"


def initial():
    def fixture(name):
        with open(os.path.join(FIXTURES, name)) as file:
            return json.load(file)

    return {
        "default_sink": "sink.speakers",
        "default_source": "source.mic",
        "sinks": fixture("pactl-sinks.json"),
        "sources": fixture("pactl-sources.json"),
        "streams": fixture("pactl-sink-inputs.json"),
        "wifi_on": True,
        "devices": [
            ["wlp1s0", "wifi", "connected", "Home"],
            ["br-0123", "bridge", "connected (externally)", "br-0123"],
            ["lo", "loopback", "connected (externally)", "lo"],
            ["p2p-dev-wlp1s0", "wifi-p2p", "disconnected", ""],
            ["enp2s0", "ethernet", "unavailable", ""],
            ["veth1234", "ethernet", "unmanaged", ""],
        ],
        "networks": [
            {"ssid": "Home", "signal": 81, "security": "WPA2", "known": True},
            {"ssid": "Lobby", "signal": 70, "security": "--", "known": False},
            {"ssid": "Cafe:Free", "signal": 62, "security": "--", "known": False},
            {"ssid": "Neighbour", "signal": 35, "security": "WPA2 WPA3", "known": False},
            {"ssid": "Known Away", "signal": 20, "security": "WPA2", "known": True},
        ],
        "in_use": "Home",
        "vpns": [
            {"name": "Work VPN", "type": "vpn", "active": False},
            {"name": "wg0", "type": "wireguard", "active": False},
        ],
        "bt_powered": True,
        "bt_devices": [
            {"mac": "22:22:22:22:22:22", "name": "Headphones", "paired": True, "connected": True},
            {"mac": "44:44:44:44:44:44", "name": "Keyboard K1", "paired": True, "connected": False},
            {"mac": "11:11:11:11:11:11", "name": "Speaker One", "paired": True, "connected": False},
            {"mac": "33:33:33:33:33:33", "name": "Mystery Phone", "paired": False, "connected": False},
        ],
        "profiles": ["performance", "balanced", "power-saver"],
        "profile": "balanced",
    }


def esc(text):
    return text.replace("\\", "\\\\").replace(":", "\\:")


class Store:
    def __enter__(self):
        os.makedirs(STATE, exist_ok=True)
        self.lock = open(os.path.join(STATE, "lock"), "w")
        fcntl.flock(self.lock, fcntl.LOCK_EX)
        path = os.path.join(STATE, "state.json")
        if os.path.exists(path):
            with open(path) as file:
                self.state = json.load(file)
        else:
            self.state = initial()
        return self.state

    def __exit__(self, *_):
        with open(os.path.join(STATE, "state.json"), "w") as file:
            json.dump(self.state, file)
        fcntl.flock(self.lock, fcntl.LOCK_UN)


def fail(message, code=1):
    sys.stderr.write(message + "\n")
    sys.exit(code)


def percent(volume):
    return {
        name: {"value": int(value * 65536 / 100), "value_percent": f"{value}%", "db": "0.00 dB"}
        for name, value in volume
    }


def set_volume(item, value):
    channels = list(item["volume"].keys())
    item["volume"] = percent([(name, value) for name in channels])


def find(items, key, value):
    for item in items:
        if str(item.get(key)) == str(value):
            return item
    fail(f"no such thing: {value}")


def pactl(state, args):
    if args[:3] == ["-f", "json", "list"]:
        kind = {"sinks": "sinks", "sources": "sources", "sink-inputs": "streams"}[args[3]]
        print(json.dumps(state[kind]))
    elif args == ["get-default-sink"]:
        print(state["default_sink"])
    elif args == ["get-default-source"]:
        print(state["default_source"])
    elif args[0] == "set-default-sink":
        state["default_sink"] = args[1]
    elif args[0] == "set-default-source":
        state["default_source"] = args[1]
    elif args[0] in ("set-sink-volume", "set-source-volume"):
        items = state["sinks" if "sink" in args[0] else "sources"]
        set_volume(find(items, "name", args[1]), int(args[2].rstrip("%")))
    elif args[0] == "set-sink-input-volume":
        set_volume(find(state["streams"], "index", args[1]), int(args[2].rstrip("%")))
    elif args[0] in ("set-sink-mute", "set-source-mute"):
        item = find(state["sinks" if "sink" in args[0] else "sources"], "name", args[1])
        item["mute"] = not item["mute"]
    elif args[0] == "set-sink-input-mute":
        item = find(state["streams"], "index", args[1])
        item["mute"] = not item["mute"]
    elif args[0] == "move-sink-input":
        pass
    else:
        fail(f"pactl: unknown command {args}")


def nmcli(state, args):
    if args[:3] == ["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION"]:
        for device in state["devices"]:
            print(":".join(esc(field) for field in device))
    elif args == ["radio", "wifi"]:
        print("enabled" if state["wifi_on"] else "disabled")
    elif args[:2] == ["radio", "wifi"] and len(args) == 3:
        state["wifi_on"] = args[2] == "on"
    elif args[:2] == ["-t", "-f"] and args[2] == "NAME,TYPE":
        active = "--active" in args
        rows = []
        for network in state["networks"]:
            if network["known"]:
                if not active or state["in_use"] == network["ssid"]:
                    rows.append((network["ssid"], "802-11-wireless"))
        for vpn in state["vpns"]:
            if not active or vpn["active"]:
                rows.append((vpn["name"], vpn["type"]))
        for name, kind in rows:
            print(f"{esc(name)}:{kind}")
    elif args[:2] == ["-t", "-f"] and args[2] == "IN-USE,SSID,SIGNAL,SECURITY":
        for network in state["networks"]:
            use = "*" if state["in_use"] == network["ssid"] else " "
            print(f"{use}:{esc(network['ssid'])}:{network['signal']}:{esc(network['security'])}")
        # A hidden network, and a weaker copy of one that is listed.
        print(" ::90:WPA2")
        print(" :Home:44:WPA2")
    elif args[:3] == ["connection", "up", "id"] or args[:3] == ["connection", "down", "id"]:
        up = args[1] == "up"
        name = args[3]
        for vpn in state["vpns"]:
            if vpn["name"] == name:
                vpn["active"] = up
                return
        for network in state["networks"]:
            if network["ssid"] == name and network["known"]:
                state["in_use"] = name if up else (None if state["in_use"] == name else state["in_use"])
                return
        fail(f"Error: unknown connection '{name}'.", 10)
    elif args[:3] == ["device", "wifi", "connect"]:
        ssid = args[3]
        password = args[5] if len(args) > 5 and args[4] == "password" else None
        for network in state["networks"]:
            if network["ssid"] == ssid:
                if network["security"] != "--" and not network["known"] and password != PASSWORD:
                    fail("Error: Connection activation failed: (7) Secrets were required, but not provided.", 4)
                network["known"] = True
                state["in_use"] = ssid
                return
        fail(f"Error: No network with SSID '{ssid}' found.", 10)
    elif args[:2] == ["device", "disconnect"]:
        state["in_use"] = None
    elif args[:3] == ["device", "wifi", "rescan"]:
        time.sleep(0.2)
    else:
        fail(f"nmcli: unknown command {args}")


def bluetoothctl(state, args):
    if args == ["show"]:
        print("Controller AA:AA:AA:AA:AA:AA (public)\n\tName: stub\n\tPowered: " + ("yes" if state["bt_powered"] else "no"))
    elif args[:1] == ["devices"]:
        kind = args[1] if len(args) > 1 else None
        for device in state["bt_devices"]:
            if kind == "Paired" and not device["paired"]:
                continue
            if kind == "Connected" and not device["connected"]:
                continue
            print(f"Device {device['mac']} {device['name']}")
    elif args[:3] == ["--timeout", "8", "scan"]:
        time.sleep(1.0)
    else:
        fail(f"bluetoothctl: unknown command {args}")


def omarchy_bluetooth_power(state, args):
    state["bt_powered"] = args[0] == "on"


def omarchy_bluetooth_device(state, args):
    action, mac = args[0], args[1]
    device = find(state["bt_devices"], "mac", mac)
    if action == "pair":
        device["paired"] = True
        device["connected"] = True
    elif action == "connect":
        device["connected"] = True
    elif action == "disconnect":
        device["connected"] = False
    else:
        fail(f"unknown action {action}")


def powerprofilesctl(state, args):
    if args == ["get"]:
        print(state["profile"])
    elif args == ["list"]:
        for profile in state["profiles"]:
            print(("* " if profile == state["profile"] else "  ") + profile + ":")
            print("    CpuDriver:\tintel_pstate\n")
    else:
        fail(f"powerprofilesctl: unknown command {args}")


def main():
    if not STATE:
        fail("QUADRILLE_STUB_STATE is not set", 2)

    name = sys.argv[1]
    args = sys.argv[2:]

    with Store() as state:
        with open(os.path.join(STATE, "calls.log"), "a") as log:
            log.write(" ".join([name] + args) + "\n")

        if name == "pactl":
            pactl(state, args)
        elif name == "nmcli":
            nmcli(state, args)
        elif name == "bluetoothctl":
            bluetoothctl(state, args)
        elif name == "omarchy-bluetooth-power":
            omarchy_bluetooth_power(state, args)
        elif name == "omarchy-bluetooth-device":
            omarchy_bluetooth_device(state, args)
        elif name == "powerprofilesctl":
            powerprofilesctl(state, args)
        elif name == "omarchy-powerprofiles-set":
            state["profile"] = args[-1]
        elif name in ("omarchy-system-lock", "omarchy-system-logout", "omarchy-system-reboot", "omarchy-system-shutdown"):
            pass  # written down, never done
        elif name == "systemctl" and args == ["suspend"]:
            pass  # written down, never done
        else:
            fail(f"{name} {' '.join(args)}: no such stub", 127)


if __name__ == "__main__":
    main()
