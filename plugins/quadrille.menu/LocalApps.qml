import QtQuick
import Quickshell
import Quickshell.Io
import "AppSearch.js" as AppSearch

// The desktop-application list, read straight from Quickshell's DesktopEntries.
//
// A third-party menu is meant to be handed `shell.appLibrary`, a facade over the
// shell's own AppLibrary. On Omarchy 4.0.4 the facade this clone receives has no
// application library in it, and the shell revokes the whole `shell` object a
// moment later, so the Apps menu opened empty ("Nothing here yet"). This object
// answers the same calls from DesktopEntries so the menu does not depend on it;
// Menu.qml prefers the shell's library whenever the host provides one.
//
// What it does not do: the shell's hidden-entries filter, launch feedback, or
// the icon-file fallback index.
QtObject {
  id: root

  property string omarchyPath: Quickshell.env("OMARCHY_PATH")

  signal appsChanged()

  property Connections watcher: Connections {
    target: DesktopEntries.applications
    function onValuesChanged() { root.appsChanged() }
  }

  function entryName(entry) { return AppSearch.entryName(entry) }
  function entrySubtext(entry) { return AppSearch.entrySubtext(entry) }

  function sortedEntries(query) {
    var values = DesktopEntries.applications.values || []
    return AppSearch.sortedEntries(values, query, null)
  }

  // Icon files named by absolute path, found to exist (see checkIcons). A path
  // is not offered to an Image until then: an Image that cannot open its file
  // writes a warning to the log, and a few entries name files that are gone.
  property var existing: ({})

  function checkIcons(paths) {
    var want = []
    for (var i = 0; i < paths.length; i++) {
      var p = String(paths[i] || "")
      if (p.charAt(0) === "/" && !existing[p] && want.indexOf(p) < 0) want.push(p)
    }
    if (want.length === 0 || iconCheck.running) return
    iconCheck.found = ({})
    iconCheck.command = ["bash", "-c", "for p in \"$@\"; do [ -f \"$p\" ] && printf '%s\\n' \"$p\"; done", "check"].concat(want)
    iconCheck.running = true
  }

  property Process iconCheck: Process {
    property var found: ({})
    stdout: SplitParser { onRead: function(line) { iconCheck.found[line] = true } }
    onExited: {
      var next = ({})
      for (var k in root.existing) next[k] = true
      for (var f in iconCheck.found) next[f] = true
      root.existing = next
    }
  }

  function iconSource(icon) {
    var value = String(icon || "")
    // An entry with no picture of its own is drawn by the menu (see AppIcon),
    // not given the theme's generic one.
    if (value.length === 0) return ""
    if (value.indexOf("file://") === 0 || value.indexOf("image://") === 0) return value
    if (value.charAt(0) === "/") return existing[value] ? "file://" + value : ""
    var themed = Quickshell.iconPath(value, true)
    return themed
  }

  function refreshIcons() { }

  function launch(desktopId, name) {
    var id = String(desktopId || "")
    if (!id) return
    // As the shell does: inside an app scope, with gtk-launch resolving the id.
    Quickshell.execDetached(["uwsm-app", "--", "gtk-launch", id + ".desktop"])
  }

  function remove(desktopId, name) {
    var id = String(desktopId || "")
    if (!id) return
    Quickshell.execDetached([root.omarchyPath + "/bin/omarchy-remove-launcher-entry", id, String(name || id)])
  }
}
