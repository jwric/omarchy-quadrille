import QtQuick
import qs.Ui
import "../Q"

// Replaces omarchy.menu: the launcher as a 7 x 7 pixel icon. Left opens the
// Omarchy menu, right a terminal, exactly as the stock button does.
BarWidget {
  id: root
  moduleName: "omarchy.menu"

  readonly property var g: Px.of(root)

  implicitWidth: vertical ? barSize : (7 + 2 * 4) * g.unit
  implicitHeight: vertical ? (7 + 2 * 4) * g.unit : barSize

  BarButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    tooltipText: "Menu"
    onPressed: function(b) {
      if (!root.bar) return
      if (b === Qt.RightButton) root.bar.run("xdg-terminal-exec")
      else root.bar.run("omarchy-shell shell toggle omarchy.menu '{\"menu\":\"root\"}'")
    }

    Rectangle {
      x: root.vertical ? g.centre(parent.width, width) : 0
      y: root.vertical ? 0 : g.px(2)
      width: root.vertical ? g.px(12) : parent.width
      height: root.vertical ? parent.height : g.px(12)
      antialiasing: false
      color: button.down ? Role.hover : (button.hovered ? Role.raised : "transparent")
    }
    Sprite {
      x: g.centre(parent.width, width)
      y: g.centre(parent.height, height)
      rows: Sprites.menu
      color: button.hovered || button.down ? Role.ink : Role.muted
    }
  }
}
