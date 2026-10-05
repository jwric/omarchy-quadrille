import QtQuick
import qs.Commons
import "../Q"

// A pixel-drawn face for a stock bar widget. The stock widget keeps running
// (it owns the popup, the keyboard handling, the services and the settings),
// hidden; the skin draws what it reports. Every property is read defensively,
// so a stock widget that changes underneath degrades to an empty icon, not an
// error.
Item {
  id: root

  readonly property var g: Px.of(root)

  // The stock widget this skin stands in for, and the bar's host object.
  property var host: null
  property var bar: null
  // The pointer is over it, and its popup is open.
  property bool hot: false
  property bool open: false
  // Air either side of the mark, in vpx.
  property int pad: 3
  // The bounds corner brackets are drawn around while the popup is open,
  // in the skin's own coordinates.
  property rect mark: Qt.rect(0, g.px(2), width, g.px(12))

  // The widest the skin may be, in logical px (the bar sets it, for the skins
  // that give way: a title, a now-playing line), and whether it is one of them.
  property real room: 1e9
  property bool elastic: false

  function prop(name, fallback) {
    return host && host[name] !== undefined && host[name] !== null ? host[name] : fallback
  }

  // The first object under the host that has a property `name`: for a value the
  // stock widget keeps inside a child (the weather's report lives in the panel
  // it loads beside its button).
  function deep(name, from, depth) {
    var obj = from === undefined ? host : from
    var d = depth === undefined ? 0 : depth
    if (!obj || d > 4) return null
    if (name in obj) return obj
    var kids = obj.children
    if (!kids) return null
    for (var i = 0; i < kids.length; i++) {
      var found = deep(name, kids[i], d + 1)
      if (found) return found
    }
    return null
  }

  implicitHeight: bar ? bar.barSize : Style.bar.sizeHorizontal
  height: implicitHeight
}
