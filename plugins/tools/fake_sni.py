#!/usr/bin/env python3
"""A few fake system tray items, to try the tray with.

    plugins/tools/fake_sni.py [seconds] [count]

Registers `count` StatusNotifierItems (default 4) on the session bus, each on a
connection of its own, with
themed icons, a drawn pixmap icon, a tooltip and a small dbusmenu (an entry, a
separator, a check item, a submenu, a disabled entry), keeps them for `seconds`
(default 20) and exits, taking them with it. Clicks and menu events print to
stdout. They appear in whatever is the tray on that bus: that is the point, and
also why it should be short.
"""
import struct
import sys
import time

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib

ITEM_XML = """
<node>
  <interface name="org.kde.StatusNotifierItem">
    <property name="Category" type="s" access="read"/>
    <property name="Id" type="s" access="read"/>
    <property name="Title" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <property name="WindowId" type="i" access="read"/>
    <property name="IconName" type="s" access="read"/>
    <property name="IconPixmap" type="a(iiay)" access="read"/>
    <property name="OverlayIconName" type="s" access="read"/>
    <property name="AttentionIconName" type="s" access="read"/>
    <property name="ToolTip" type="(sa(iiay)ss)" access="read"/>
    <property name="ItemIsMenu" type="b" access="read"/>
    <property name="Menu" type="o" access="read"/>
    <method name="Activate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
    <method name="SecondaryActivate"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
    <method name="ContextMenu"><arg type="i" direction="in"/><arg type="i" direction="in"/></method>
    <method name="Scroll"><arg type="i" direction="in"/><arg type="s" direction="in"/></method>
    <signal name="NewStatus"><arg type="s"/></signal>
  </interface>
</node>
"""

MENU_XML = """
<node>
  <interface name="com.canonical.dbusmenu">
    <property name="Version" type="u" access="read"/>
    <property name="TextDirection" type="s" access="read"/>
    <property name="Status" type="s" access="read"/>
    <method name="GetLayout">
      <arg type="i" direction="in"/><arg type="i" direction="in"/><arg type="as" direction="in"/>
      <arg type="u" direction="out"/><arg type="(ia{sv}av)" direction="out"/>
    </method>
    <method name="GetGroupProperties">
      <arg type="ai" direction="in"/><arg type="as" direction="in"/>
      <arg type="a(ia{sv})" direction="out"/>
    </method>
    <method name="GetProperty">
      <arg type="i" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/>
    </method>
    <method name="Event">
      <arg type="i" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="in"/><arg type="u" direction="in"/>
    </method>
    <method name="EventGroup">
      <arg type="a(isvu)" direction="in"/><arg type="ai" direction="out"/>
    </method>
    <method name="AboutToShow"><arg type="i" direction="in"/><arg type="b" direction="out"/></method>
    <method name="AboutToShowGroup"><arg type="ai" direction="in"/><arg type="ai" direction="out"/><arg type="ai" direction="out"/></method>
    <signal name="LayoutUpdated"><arg type="u"/><arg type="i"/></signal>
  </interface>
</node>
"""

V = GLib.Variant


def pixmap(seed):
    """A 22 x 22 ARGB icon drawn in code: a rounded tile with a diagonal stripe."""
    size = 22
    colours = [(0xE8, 0x55, 0x3E), (0x3E, 0x9A, 0xE8), (0x9F, 0xC7, 0x9A), (0xE4, 0xC6, 0x4C)]
    r, g, b = colours[seed % len(colours)]
    data = bytearray()
    for y in range(size):
        for x in range(size):
            inside = 1 <= x < size - 1 and 1 <= y < size - 1
            corner = (x in (1, size - 2)) and (y in (1, size - 2))
            if not inside or corner:
                data += bytes((0, 0, 0, 0))
            elif abs((x - y)) < 3:
                data += bytes((255, 255, 255, 255))
            else:
                data += bytes((255, r, g, b))
    return [(size, size, bytes(data))]


MENU = [
    (1, {"label": V("s", "Open window"), "enabled": V("b", True)}, []),
    (2, {"type": V("s", "separator")}, []),
    (3, {"label": V("s", "Mute notifications"), "toggle-type": V("s", "checkmark"), "toggle-state": V("i", 1)}, []),
    (4, {"label": V("s", "Recent"), "children-display": V("s", "submenu")}, [
        (41, {"label": V("s", "report.pdf")}, []),
        (42, {"label": V("s", "notes.txt")}, []),
        (43, {"label": V("s", "A file with a really rather long name.tar.gz")}, []),
    ]),
    (5, {"label": V("s", "Disabled entry"), "enabled": V("b", False)}, []),
    (6, {"label": V("s", "Quit"), "icon-name": V("s", "application-exit")}, []),
]


def layout_variant(entry):
    ident, props, children = entry
    return V("(ia{sv}av)", (ident, props, [V("(ia{sv}av)", layout_tuple(c)) for c in children]))


def layout_tuple(entry):
    ident, props, children = entry
    return (ident, props, [V("(ia{sv}av)", layout_tuple(c)) for c in children])


class Item:
    def __init__(self, n, name, icon, use_pixmap):
        # One bus connection each, as separate applications have: the watcher keys
        # an item by its bus name, so items sharing a connection are dropped one
        # at a time when it goes away.
        address = Gio.dbus_address_get_for_bus_sync(Gio.BusType.SESSION, None)
        conn = Gio.DBusConnection.new_for_address_sync(
            address,
            Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION,
            None, None)
        self.conn, self.n, self.name, self.icon, self.use_pixmap = conn, n, name, icon, use_pixmap
        self.path = "/StatusNotifierItem"
        self.menu_path = "/StatusNotifierItem/menu"
        node = Gio.DBusNodeInfo.new_for_xml(ITEM_XML)
        conn.register_object(self.path, node.interfaces[0], self.on_call, self.on_get, None)
        menu = Gio.DBusNodeInfo.new_for_xml(MENU_XML)
        conn.register_object(self.menu_path, menu.interfaces[0], self.on_menu_call, self.on_menu_get, None)

    def on_get(self, conn, sender, path, iface, prop):
        pix = pixmap(self.n) if self.use_pixmap else []
        values = {
            "Category": V("s", "ApplicationStatus"), "Id": V("s", self.name), "Title": V("s", self.name),
            "Status": V("s", "NeedsAttention" if self.n == 3 else "Active"), "WindowId": V("i", 0),
            "IconName": V("s", "" if self.use_pixmap else self.icon),
            "IconPixmap": V("a(iiay)", [(w, h, list(d)) for w, h, d in pix]),
            "OverlayIconName": V("s", ""), "AttentionIconName": V("s", ""),
            "ToolTip": V("(sa(iiay)ss)", ("", [], self.name, "a fake tray item")),
            "ItemIsMenu": V("b", False), "Menu": V("o", self.menu_path),
        }
        return values.get(prop)

    def on_call(self, conn, sender, path, iface, method, params, invocation):
        print("item", self.n, method, params.unpack(), flush=True)
        invocation.return_value(None)

    def on_menu_get(self, conn, sender, path, iface, prop):
        return {"Version": V("u", 3), "TextDirection": V("s", "ltr"), "Status": V("s", "normal")}.get(prop)

    def on_menu_call(self, conn, sender, path, iface, method, params, invocation):
        if method == "GetLayout":
            root = (0, {"children-display": V("s", "submenu")}, MENU)
            invocation.return_value(V("(u(ia{sv}av))", (1, layout_tuple(root))))
        elif method == "GetGroupProperties":
            invocation.return_value(V("(a(ia{sv}))", ([],)))
        elif method == "AboutToShow":
            invocation.return_value(V("(b)", (False,)))
        elif method == "AboutToShowGroup":
            invocation.return_value(V("(aiai)", ([], [])))
        elif method == "Event":
            print("menu", self.n, params.unpack()[:2], flush=True)
            invocation.return_value(None)
        elif method == "EventGroup":
            invocation.return_value(V("(ai)", ([],)))
        elif method == "GetProperty":
            invocation.return_value(V("(v)", (V("s", ""),)))
        else:
            invocation.return_value(None)


def main():
    seconds = float(sys.argv[1]) if len(sys.argv) > 1 else 20
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 4
    specs = [("quadrille-a", "firefox", False), ("quadrille-b", "steam", False), ("quadrille-c", "", True),
             ("quadrille-d", "folder", False), ("quadrille-e", "signal-desktop", False), ("quadrille-f", "spotify", False),
             ("quadrille-g", "chromium", False), ("quadrille-h", "obsidian", False)]
    items = [Item(i + 1, *specs[i % len(specs)]) for i in range(count)]
    for item in items:
        item.conn.call_sync("org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher",
                            "RegisterStatusNotifierItem", V("(s)", (item.path,)), None, Gio.DBusCallFlags.NONE, 2000, None)
    print("registered", count, flush=True)
    loop = GLib.MainLoop()
    GLib.timeout_add(int(seconds * 1000), loop.quit)
    loop.run()


if __name__ == "__main__":
    main()
