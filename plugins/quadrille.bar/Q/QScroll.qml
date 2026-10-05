import QtQuick
import "."

// A viewport on content taller than it: the content moves in whole virtual
// pixels (a wheel notch is `wheelStep` of them, a row or so), and a one-pixel
// scroll mark on the right edge says where in it you are: a faint rail and a
// thumb in muted, there only when there is more to see. The content's width is
// the viewport's less the gutter, always, so what is wrapped does not reflow
// when the mark appears.
//
//     QScroll { anchors.fill: parent; Column { width: parent.width ... } }
Item {
  id: root

  readonly property var g: Px.of(root)

  default property alias content: holder.data
  // Logical px of whole content. By default what the holder's children cover.
  property real contentHeight: holder.childrenRect.height
  // Logical px scrolled, a whole number of vpx.
  property real offset: 0
  // Vpx per wheel notch.
  property int wheelStep: 14
  readonly property real gutter: g.px(4)
  readonly property real maxOffset: Math.max(0, g.ceil(contentHeight) - height)
  readonly property bool scrollable: maxOffset > 0

  clip: true

  onMaxOffsetChanged: if (offset > maxOffset) offset = maxOffset

  function scrollTo(y) { offset = g.snap(Math.max(0, Math.min(maxOffset, y))) }
  function scrollBy(dy) { scrollTo(offset + dy) }
  function reset() { offset = 0 }

  // Bring the part of the content from `y` for `h` logical px into view, with a
  // little air; `item` and its position in the content are given instead when a
  // child asks.
  function ensureVisible(y, h) {
    var margin = g.px(2)
    if (y < offset + margin) scrollTo(y - margin)
    else if (y + h > offset + height - margin) scrollTo(y + h + margin - height)
  }
  function ensureItemVisible(item) {
    if (!item) return
    var p = item.mapToItem(holder, 0, 0)
    ensureVisible(p.y, item.height)
  }

  Item {
    id: holder
    y: -root.offset
    width: root.width - root.gutter
    height: childrenRect.height
  }

  // The scroll mark.
  Rectangle {
    visible: root.scrollable
    x: root.width - root.g.hair
    width: root.g.hair
    height: root.height
    antialiasing: false
    color: Role.edge
  }
  Rectangle {
    visible: root.scrollable
    readonly property real total: Math.max(1, root.g.ceil(root.contentHeight))
    x: root.width - root.g.hair
    width: root.g.hair
    height: Math.max(root.g.px(6), root.g.snap(root.height * root.height / total))
    y: root.g.snap((root.height - height) * (root.maxOffset > 0 ? root.offset / root.maxOffset : 0))
    antialiasing: false
    color: Role.muted
  }

  WheelHandler {
    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
    enabled: root.scrollable
    onWheel: function(event) {
      var notches = event.angleDelta.y / 120
      if (notches === 0) return
      root.scrollBy(-Math.sign(notches) * Math.max(1, Math.round(Math.abs(notches))) * root.wheelStep * root.g.unit)
    }
  }
}
