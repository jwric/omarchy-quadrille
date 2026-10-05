import QtQuick
import "."

// A picture of an application on the pixel grid: `cells` x `cells` virtual
// pixels, however large the file is. PixelIcon does the work (the picture is
// box-filtered to `cells` pixels and its colours cut to a few); here it is
// given what it lacks:
//
//   * no picture (no icon named, a file that is not there, a format Qt cannot
//     read): the application's initial in a hairline box, in the muted role,
//     the same 11 x 11 in size;
//   * the pixel pass failing (nothing painted a moment after the picture
//     loaded): the picture itself, drawn nearest-neighbour at the cell size.
//
//     AppIcon { source: row.iconUrl; label: row.name; cells: 11 }
Item {
  id: root

  readonly property var g: Px.of(root)

  property string source: ""
  // The name to take the placeholder's letter from.
  property string label: ""
  property int cells: 11
  // The colour of the placeholder: muted on a plain row, on_accent on an
  // inverse one.
  property color ink: Role.muted
  property bool dimmed: false

  readonly property bool failed: source === "" || probe.status === Image.Error
  readonly property bool picture: pixel.ready
  readonly property bool fallback: !failed && !pixel.ready && gaveUp
  property bool gaveUp: false
  readonly property string initial: {
    var m = String(label).match(/[A-Za-z0-9]/)
    return m ? m[0].toUpperCase() : "?"
  }

  implicitWidth: cells * g.unit
  implicitHeight: cells * g.unit
  width: implicitWidth
  height: implicitHeight

  onSourceChanged: gaveUp = false
  Timer {
    interval: 1500
    running: !root.failed && !pixel.ready && !root.gaveUp
    onTriggered: root.gaveUp = true
  }

  PixelIcon {
    id: pixel
    source: root.source
    cells: root.cells
    dimmed: root.dimmed
  }

  // Only to learn whether the file can be read (the pixel pass does not say):
  // a small synchronous decode, as the shell's own icons are (an asynchronous
  // read of an image:// icon aborts the process), and an Error here shows the
  // placeholder at once instead of an empty slot.
  Image {
    id: probe
    visible: false
    source: root.source
    sourceSize.width: 8
    sourceSize.height: 8
    asynchronous: false
    cache: true
  }

  // the plain picture, when the pixel pass did not deliver
  Image {
    anchors.fill: parent
    visible: root.fallback && status === Image.Ready
    source: root.fallback ? root.source : ""
    sourceSize.width: Math.round(root.width * root.g.dpr)
    sourceSize.height: Math.round(root.height * root.g.dpr)
    fillMode: Image.PreserveAspectFit
    smooth: false
    asynchronous: true
  }

  // the placeholder: a hairline box and the initial, centred on the capitals
  Item {
    anchors.fill: parent
    visible: root.failed

    Rectangle {
      anchors.fill: parent
      color: "transparent"
      border.color: root.ink
      border.width: root.g.unit
      antialiasing: false
    }
    PixelText {
      // the glyph is 5 columns in a 6-column cell, capitals on rows 2-9
      x: root.g.unit * Math.floor((root.cells - 6) / 2)
      y: root.g.unit * (Math.floor((root.cells - 8) / 2) - 2)
      text: root.initial
      ink: root.ink
    }
  }
}
