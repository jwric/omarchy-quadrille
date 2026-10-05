import QtQuick
import qs.Ui
import "../Q"

// Replaces omarchy.menu: the launcher as a 7 x 7 pixel icon. Left opens the
// Omarchy menu, right a terminal, exactly as the stock button does.
BarWidget {
  id: root
  moduleName: "omarchy.menu"

  implicitWidth: (7 + 2 * 4) * Px.unit
  implicitHeight: barSize

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
      x: 0; y: Px.px(2)
      width: parent.width; height: Px.px(12)
      antialiasing: false
      color: button.down ? Role.hover : (button.hovered ? Role.raised : "transparent")
    }
    Sprite {
      x: Px.centre(parent.width, width)
      y: Px.centre(parent.height, height)
      rows: Sprites.menu
      color: button.hovered || button.down ? Role.ink : Role.muted
    }
  }
}
