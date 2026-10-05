import QtQuick
import Quickshell
import Quickshell.Wayland
import "Q"

// Summon:  omarchy-shell shell summon quadrille.lab '{"screen":"HDMI-A-1"}'
// Hide:    omarchy-shell shell hide quadrille.lab
Item {
  id: root

  property bool opened: false
  property string screenName: ""
  readonly property var screenObj: {
    var all = Quickshell.screens
    for (var i = 0; i < all.length; i++) if (all[i].name === screenName) return all[i]
    return all.length > 0 ? all[0] : null
  }

  function open(payload) {
    try { screenName = String(JSON.parse(payload || "{}").screen || "") } catch (e) { screenName = "" }
    opened = true
  }
  function close() { opened = false }

  PanelWindow {
    id: win
    readonly property var g: Px.forWindow(win)
    visible: root.opened
    screen: root.screenObj
    anchors { top: true; left: true }
    margins { top: Math.round(g.px(24)); left: Math.round(g.px(24)) }
    implicitWidth: Math.ceil(g.px(260))
    implicitHeight: Math.ceil(g.px(200))
    color: Role.void_
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "quadrille-lab"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    mask: Region {}

    // metrics: ruler lines every vpx row at the left, text row
    Item {
      anchors.fill: parent

      // cap guides
      Rectangle { x: win.g.px(2); y: win.g.px(4); width: win.g.px(30); height: win.g.hair; color: Role.faint }
      Rectangle { x: win.g.px(2); y: win.g.px(4) + win.g.capTop; width: win.g.px(30); height: win.g.hair; color: Role.edge }
      Rectangle { x: win.g.px(2); y: win.g.px(4) + win.g.baseline; width: win.g.px(30); height: win.g.hair; color: Role.edge }
      Rectangle { x: win.g.px(2); y: win.g.px(4) + win.g.line; width: win.g.px(30); height: win.g.hair; color: Role.faint }
      PixelText { x: win.g.px(3); y: win.g.px(4); text: "HEgxy MENU 0123" }

      Row {
        x: win.g.px(2); y: win.g.px(20); spacing: win.g.px(2)
        Repeater {
          model: ["wifi", "ethernet", "offline", "volume", "muted", "microphone", "bluetooth", "battery", "plug", "cpu", "clock", "menu", "brightness", "bell", "warning"]
          Sprite { required property string modelData; rows: Sprites[modelData]; level: 2 }
        }
      }

      Row {
        x: win.g.px(2); y: win.g.px(32); spacing: win.g.px(4)
        Lamp { on: false }
        Lamp { on: true; tone: Role.accent }
        Lamp { on: true; tone: Role.live }
        Lamp { on: true; tone: Role.caution }
        Lamp { on: true; tone: Role.alarm }
        BarGauge { value: 0.6; fill: Role.live }
        BarGauge { value: 1; redline: 0.8; cells: 8 }
        Inverse { text: "ENGAGED" }
      }
      Row {
        x: win.g.px(2); y: win.g.px(44); spacing: win.g.px(2)
        Tab { text: "1"; active: true }
        Tab { text: "2"; occupied: true }
        Tab { text: "3" }
        Tab { text: "4" }
      }
      Group { x: win.g.px(2); y: win.g.px(62); width: win.g.px(120); name: "SAFETY"; Item { width: 10; height: 40 } }
      Group { x: win.g.px(130); y: win.g.px(62); width: win.g.px(40); name: "LONGNAME" }
      Brackets { x: win.g.px(180); y: win.g.px(62); width: win.g.px(30); height: win.g.px(20); anchors.fill: undefined }

      Grid {
        x: win.g.px(2); y: win.g.px(100); columns: 15; spacing: win.g.px(1)
        Repeater {
          model: ["void_", "ground", "raised", "hover", "edge", "ink", "muted", "faint", "line", "accent", "onAccent", "highlight", "live", "caution", "alarm"]
          Rectangle { required property string modelData; width: win.g.px(8); height: win.g.px(8); color: Role[modelData] }
        }
      }
    }
  }
}
