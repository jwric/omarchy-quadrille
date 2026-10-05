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
    visible: root.opened
    screen: root.screenObj
    anchors { top: true; left: true }
    margins { top: Px.px(24); left: Px.px(24) }
    implicitWidth: Px.px(260)
    implicitHeight: Px.px(200)
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
      Rectangle { x: Px.px(2); y: Px.px(4); width: Px.px(30); height: Px.hair; color: Role.faint }
      Rectangle { x: Px.px(2); y: Px.px(4) + Px.capTop; width: Px.px(30); height: Px.hair; color: Role.edge }
      Rectangle { x: Px.px(2); y: Px.px(4) + Px.baseline; width: Px.px(30); height: Px.hair; color: Role.edge }
      Rectangle { x: Px.px(2); y: Px.px(4) + Px.line; width: Px.px(30); height: Px.hair; color: Role.faint }
      PixelText { x: Px.px(3); y: Px.px(4); text: "HEgxy MENU 0123" }

      Row {
        x: Px.px(2); y: Px.px(20); spacing: Px.px(2)
        Repeater {
          model: ["wifi", "ethernet", "offline", "volume", "muted", "microphone", "bluetooth", "battery", "plug", "cpu", "clock", "menu", "brightness", "bell", "warning"]
          Sprite { required property string modelData; rows: Sprites[modelData]; level: 2 }
        }
      }

      Row {
        x: Px.px(2); y: Px.px(32); spacing: Px.px(4)
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
        x: Px.px(2); y: Px.px(44); spacing: Px.px(2)
        Tab { text: "1"; active: true }
        Tab { text: "2"; occupied: true }
        Tab { text: "3" }
        Tab { text: "4" }
      }
      Group { x: Px.px(2); y: Px.px(62); width: Px.px(120); name: "SAFETY"; Item { width: 10; height: 40 } }
      Group { x: Px.px(130); y: Px.px(62); width: Px.px(40); name: "LONGNAME" }
      Brackets { x: Px.px(180); y: Px.px(62); width: Px.px(30); height: Px.px(20); anchors.fill: undefined }

      Grid {
        x: Px.px(2); y: Px.px(100); columns: 15; spacing: Px.px(1)
        Repeater {
          model: ["void_", "ground", "raised", "hover", "edge", "ink", "muted", "faint", "line", "accent", "onAccent", "highlight", "live", "caution", "alarm"]
          Rectangle { required property string modelData; width: Px.px(8); height: Px.px(8); color: Role[modelData] }
        }
      }
    }
  }
}
