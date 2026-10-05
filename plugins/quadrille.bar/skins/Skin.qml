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
  property rect mark: Qt.rect(0, Px.px(2), width, Px.px(12))

  function prop(name, fallback) {
    return host && host[name] !== undefined && host[name] !== null ? host[name] : fallback
  }

  implicitHeight: bar ? bar.barSize : Style.bar.sizeHorizontal
  height: implicitHeight
}
