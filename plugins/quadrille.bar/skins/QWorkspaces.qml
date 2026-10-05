import QtQuick
import Quickshell
import Quickshell.Hyprland
import qs.Commons
import qs.Ui
import "../Q"

// Replaces omarchy.workspaces: numbered tabs. The workspace in use on this
// screen is lit (accent behind on_accent); a workspace with windows reads in
// ink, an empty one muted. Left focuses a workspace. Unlike the stock widget
// each bar lights its own screen's workspace, so two monitors show two lit tabs.
BarWidget {
  id: root
  moduleName: "omarchy.workspaces"

  readonly property var g: Px.of(root)

  // The bar offers each widget the room it has left; this one does not give way.
  property real room: 1e9
  readonly property bool elastic: false

  readonly property var screen: QsWindow.window ? QsWindow.window.screen : null
  readonly property var monitor: screen ? Hyprland.monitorFor(screen) : null
  readonly property int activeId: monitor && monitor.activeWorkspace ? monitor.activeWorkspace.id : -1

  function workspaceById(id) {
    var values = Hyprland.workspaces.values
    for (var i = 0; i < values.length; i++) if (values[i].id === id) return values[i]
    return null
  }

  function workspaceIds() {
    var ids = [1, 2, 3, 4, 5]
    var values = Hyprland.workspaces.values
    for (var i = 0; i < values.length; i++) {
      var id = values[i].id
      if (id > 0 && id <= 10 && ids.indexOf(id) === -1) ids.push(id)
    }
    ids.sort(function(a, b) { return a - b })
    return ids
  }

  function focusWorkspace(id) {
    if (!root.bar) return
    root.bar.run("hyprctl dispatch " + Util.shellQuote("hl.dsp.focus({ workspace = \"" + id + "\" })"))
  }

  implicitWidth: row.width + g.px(2)
  implicitHeight: barSize

  Row {
    id: row
    x: g.px(1)
    y: g.centre(root.height, height)
    spacing: g.px(2)

    Repeater {
      model: root.workspaceIds()

      BarButton {
        id: tabButton
        required property int modelData
        readonly property var workspace: root.workspaceById(modelData)
        readonly property bool occupied: workspace !== null && workspace.toplevels.values.length > 0

        bar: root.bar
        width: tab.width
        height: tab.height
        tooltipText: "Workspace " + modelData
        onPressed: function() { root.focusWorkspace(modelData) }

        Tab {
          id: tab
          text: modelData === 10 ? "0" : String(modelData)
          active: modelData === root.activeId
          occupied: tabButton.occupied
          hot: tabButton.hovered
          down: tabButton.down
        }
      }
    }
  }
}
