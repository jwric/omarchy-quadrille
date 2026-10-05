import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.Commons
import "../Q"

// A small popup that hangs off a bar item: a hairline card on the ground, sized
// in whole virtual pixels of the screen it opens on, closed by a click anywhere
// else (Hyprland's focus grab, as the stock cards use) and announced to the bar
// as its popout so only one is open at a time. Put content in it; give it
// `contentWidth` and `contentHeight` (logical px: use `g.px()` and `g.cellW`).
//
// Not the host's PopupCard, whose card fades in over 140 ms and draws its text
// through the distance-field renderer: this one appears whole and draws with the
// kit.
PopupWindow {
  id: root

  required property Item anchorItem
  required property var bar
  // The object the bar knows this popout by; the popup itself by default.
  property var owner: null
  property bool open: false
  // Content box, inside 1 vpx of hairline and `pad` vpx of air.
  property real contentWidth: g.px(40)
  property real contentHeight: g.px(20)
  property int pad: 3
  // Open above the anchor instead of below (a bar on the bottom edge).
  readonly property bool above: bar && bar.position === "bottom"
  default property alias content: holder.data

  readonly property var g: Px.forWindow(root)
  readonly property var coordinatorKey: owner || root
  readonly property var anchorWindow: anchorItem ? anchorItem.QsWindow.window : null
  readonly property real cardWidth: contentWidth + 2 * (g.hair + pad * g.unit)
  readonly property real cardHeight: contentHeight + 2 * (g.hair + pad * g.unit)

  signal dismissed()

  function close() {
    if (owner && "close" in owner) owner.close()
    else open = false
  }

  visible: open
  color: "transparent"
  // A surface is a whole number of logical pixels; the card inside it is whole
  // device pixels.
  implicitWidth: Math.ceil(cardWidth)
  implicitHeight: Math.ceil(cardHeight)

  onOpenChanged: {
    if (!bar) return
    if (open) bar.requestPopout(coordinatorKey)
    else {
      if (bar.activePopout === coordinatorKey) bar.releasePopout(coordinatorKey)
      dismissed()
    }
  }

  HyprlandFocusGrab {
    active: root.open
    windows: root.anchorWindow ? [root, root.anchorWindow] : [root]
    onCleared: root.close()
  }

  anchor {
    id: popupAnchor
    window: root.anchorWindow
    adjustment: PopupAdjustment.Slide
    edges: Edges.Top | Edges.Left
    gravity: Edges.Bottom | Edges.Right
    rect.width: 1
    rect.height: 1

    onAnchoring: {
      var target = root.anchorItem
      var window = root.anchorWindow
      if (!target || !window || !root.bar) return
      var gap = root.g.px(2)
      var x = target.width / 2 - root.implicitWidth / 2
      var y = target.height + gap
      if (root.bar.position === "bottom") y = -root.implicitHeight - gap
      else if (root.bar.position === "left") { x = target.width + gap; y = target.height / 2 - root.implicitHeight / 2 }
      else if (root.bar.position === "right") { x = -root.implicitWidth - gap; y = target.height / 2 - root.implicitHeight / 2 }
      var point = window.contentItem.mapFromItem(target, x, y)
      var m = root.g.px(2)
      point.x = Math.max(m, Math.min(point.x, window.width - root.implicitWidth - m))
      popupAnchor.rect.x = Math.round(point.x)
      popupAnchor.rect.y = Math.round(point.y)
    }
  }

  Rectangle {
    id: card
    x: 0; y: 0
    width: root.cardWidth
    height: root.cardHeight
    color: Role.edge
    antialiasing: false

    Rectangle {
      anchors.fill: parent
      anchors.margins: root.g.hair
      color: Role.ground
      antialiasing: false
    }
    Item {
      id: holder
      x: root.g.hair + root.pad * root.g.unit
      y: root.g.hair + root.pad * root.g.unit
      width: root.contentWidth
      height: root.contentHeight
      clip: true
    }
  }
}
