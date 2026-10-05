import QtQuick
import qs.Commons
import "../Q"

// The bar's click contract, drawn by quadrille. A drop-in for what the bar's
// slot expects of a button (see Ui/WidgetButton.qml): it registers itself as a
// click target, tells the bar about hover for the tooltip, and raises
// `pressed(button)` / `wheelMoved(delta)`. It paints nothing: put a visual
// inside, and read `hovered` and `down` to step its face.
Item {
  id: root

  property var bar: null
  property string tooltipText: ""
  property bool interactive: true
  property bool pressable: true
  property bool concealed: false
  property var registeredBar: null
  readonly property bool hovered: area.containsMouse
  readonly property bool down: area.pressed
  readonly property bool tooltipHovered: visible && interactive && !concealed && area.containsMouse
  readonly property bool vertical: bar ? bar.vertical : false
  readonly property int barSize: bar ? bar.barSize : Style.bar.sizeHorizontal
  default property alias content: holder.data

  signal pressed(int button)
  signal wheelMoved(int delta)

  function triggerPress(button) {
    if (root.bar) root.bar.hideTooltip(root)
    root.pressed(button)
  }

  function syncClickRegistration() {
    if (registeredBar && registeredBar.unregisterClickTarget) registeredBar.unregisterClickTarget(root)
    registeredBar = root.bar
    if (registeredBar && registeredBar.registerClickTarget) registeredBar.registerClickTarget(root)
  }
  onBarChanged: syncClickRegistration()
  onVisibleChanged: if (!visible && root.bar) root.bar.hideTooltip(root)
  Component.onCompleted: syncClickRegistration()
  Component.onDestruction: if (registeredBar && registeredBar.unregisterClickTarget) registeredBar.unregisterClickTarget(root)

  Item { id: holder; anchors.fill: parent }

  MouseArea {
    id: area
    anchors.fill: parent
    acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
    enabled: root.interactive
    hoverEnabled: true
    cursorShape: root.pressable ? Qt.PointingHandCursor : Qt.ArrowCursor
    onEntered: if (root.bar) root.bar.showTooltip(root, root.tooltipText)
    onExited: if (root.bar) root.bar.hideTooltip(root)
    onClicked: function(mouse) { if (root.pressable) root.triggerPress(mouse.button) }
    onWheel: function(wheel) { root.wheelMoved(wheel.angleDelta.y) }
  }
}
