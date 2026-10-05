import QtQuick
import Quickshell
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

  function iconSource(icon) {
    var value = String(icon || "")
    if (value.length === 0) return Quickshell.iconPath("application-x-executable", true)
    if (value.indexOf("file://") === 0 || value.indexOf("image://") === 0) return value
    if (value.charAt(0) === "/") return "file://" + value
    var themed = Quickshell.iconPath(value, true)
    return themed.length > 0 ? themed : Quickshell.iconPath("application-x-executable", true)
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
