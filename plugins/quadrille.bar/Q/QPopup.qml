import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.Commons
import "."

// The popup of a bar widget, on the pixel grid: a flat `ground` card in a
// hairline, a whole number of virtual pixels in every dimension, set where the
// bar's frame ends. Nothing fades.
//
// This is qs.Ui's KeyboardPanel (MIT, cloned) with its contract kept whole -
// the properties and functions the panels call (`anchorItem`, `owner`, `bar`,
// `open`, `centerOnBar`, `focusTarget`, `contentWidth`, `contentHeight`,
// `fittedContentWidth`, `fittedContentHeight`), the layer-shell scaffolding
// (an overlay surface the size of the screen, keyboard focus primed Exclusive
// then OnDemand, an outside click that dismisses and a click on the bar that is
// forwarded to its button, a transparent twin on every other output) - and
// only the card redrawn:
//
//   * the card is `ground` in a 1-vpx `edge` hairline, square, snapped to this
//     screen's grid; the stock card is rounded, 2 logical px thick and fades
//     over 140 ms;
//   * `padding`, `margin` and `gap` are counts of virtual pixels;
//   * `fittedContentWidth(w)` takes the width the panel wants in logical pixels
//     (use `cardWidth(cells)`), `fittedContentHeight(h)` takes the height of the
//     content and returns the card's, both whole virtual pixels.
//
// The grid is the surface's own (`Px.forWindow`): the unit is a whole number of
// this output's device pixels, 2 at scale 1 and 1.8 logical px (3 device) on a
// 1.666667 output.
PanelWindow {
  id: root

  readonly property var g: Px.forWindow(root)

  required property Item anchorItem
  required property QtObject bar
  property var owner: null
  // Air between the card and the edge of the screen, and between it and the
  // bar, in logical px (whole vpx).
  property real margin: g.px(4)
  property real gap: g.px(4)
  // Air inside the hairline.
  property real padding: g.px(4)
  property real contentWidth: g.px(240)
  property real contentHeight: g.px(120)
  property bool centerOnBar: false
  property bool open: false
  property bool popoutSwitching: false
  property bool popoutSwitchClosing: false
  property bool focusPrimed: false

  // Item that should take keyboard focus once the panel maps (see KeyboardPanel).
  property Item focusTarget: null

  default property alias contentItem: contentHolder.children

  readonly property var coordinatorKey: owner || root
  readonly property var anchorWindow: anchorItem ? anchorItem.QsWindow.window : null
  readonly property string barPos: bar ? bar.position : "top"

  // Where the content starts inside the card, and how wide it is.
  readonly property real inset: g.hair + padding
  readonly property real innerWidth: Math.max(0, contentWidth - 2 * inset)
  readonly property int innerColumns: g.columns(innerWidth)

  function close() {
    if (owner && "close" in owner) owner.close()
    else root.open = false
  }

  function beginFocusPrime() {
    if (open && backingWindowVisible) focusPrimeTimer.restart()
  }

  // --- screen + lifetime ---------------------------------------------------

  screen: anchorWindow ? anchorWindow.screen : null
  visible: open || popoutSwitching
  color: "transparent"
  exclusionMode: ExclusionMode.Ignore

  WlrLayershell.namespace: "omarchy-keyboard-panel"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: open
    ? (focusPrimed ? WlrKeyboardFocus.OnDemand : WlrKeyboardFocus.Exclusive)
    : WlrKeyboardFocus.None

  onBackingWindowVisibleChanged: beginFocusPrime()

  anchors {
    top: true
    bottom: true
    left: true
    right: true
  }

  readonly property real _barStripSize: {
    if (!bar) return 0
    var actual = (root.barPos === "top" || root.barPos === "bottom") ? root.barH : root.barW
    return Math.max(bar.barSize, actual) + root.gap
  }
  mask: Region {
    width: root.screenW
    height: root.screenH
  }

  TransformWatcher {
    id: anchorWatcher
    a: anchorWindow ? anchorWindow.contentItem : null
    b: anchorItem
  }

  readonly property point anchorScreenPos: {
    anchorWatcher.transform  // reactive dependency
    if (!anchorItem || !anchorWindow) return Qt.point(0, 0)
    return anchorItem.mapToItem(anchorWindow.contentItem, 0, 0)
  }
  readonly property real anchorW: anchorItem ? anchorItem.width : 0
  readonly property real anchorH: anchorItem ? anchorItem.height : 0
  readonly property real screenW: screen ? screen.width : 0
  readonly property real screenH: screen ? screen.height : 0
  readonly property real barW: anchorWindow ? anchorWindow.width : screenW
  readonly property real barH: anchorWindow ? anchorWindow.height : 0
  // The bar's frame, in the direction away from its edge: 16 vpx, which is
  // less than its window when the window is rounded up to a logical pixel.
  readonly property real barFrame: g.bar

  readonly property real availableCardWidth: screenW > 0
    ? Math.max(g.px(40), screenW - ((barPos === "left" || barPos === "right") ? barFrame + gap + margin : margin * 2))
    : 0
  readonly property real availableCardHeight: screenH > 0
    ? Math.max(g.px(40), screenH - ((barPos === "top" || barPos === "bottom") ? barFrame + gap + margin : margin * 2))
    : 0
  readonly property real verticalContentInset: 2 * inset

  // The card's width for `n` cells of content (logical px, whole vpx).
  function cardWidth(n) { return n * g.cellW + 2 * inset }

  function fittedContentWidth(width, cap) {
    var desired = Math.max(g.px(8), Number(width) || 1)
    var maxWidth = root.availableCardWidth > 0 ? root.availableCardWidth : desired
    if (cap !== undefined && Number(cap) > 0) maxWidth = Math.min(maxWidth, Number(cap))
    return g.floor(Math.min(desired, maxWidth))
  }

  // The card's height for content `implicitHeight` tall, up to what fits.
  function fittedContentHeight(implicitHeight, cap) {
    var desired = Math.max(root.verticalContentInset, (Number(implicitHeight) || 0) + root.verticalContentInset)
    var maxHeight = root.availableCardHeight > 0 ? root.availableCardHeight : desired
    if (cap !== undefined && Number(cap) > 0) maxHeight = Math.min(maxHeight, Number(cap))
    return g.floor(Math.min(g.ceil(desired), maxHeight))
  }

  function cappedContentHeight(height) {
    var desired = Math.max(root.padding * 2, Number(height) || root.padding * 2)
    var maxHeight = root.availableCardHeight > 0 ? root.availableCardHeight : desired
    return g.floor(Math.min(g.ceil(desired), maxHeight))
  }

  // Desired top-left of the card in screen coordinates, on this screen's grid.
  readonly property point cardOrigin: {
    if (!anchorItem || !bar) return Qt.point(margin, margin)
    var x = 0, y = 0
    var below = barFrame + gap
    if (centerOnBar && (barPos === "top" || barPos === "bottom")) {
      x = screenW / 2 - contentWidth / 2
      y = barPos === "bottom" ? screenH - below - contentHeight : below
    } else if (centerOnBar) {
      x = barPos === "left" ? below : screenW - below - contentWidth
      y = screenH / 2 - contentHeight / 2
    } else if (barPos === "bottom") {
      x = anchorScreenPos.x + anchorW / 2 - contentWidth / 2
      y = screenH - below - contentHeight
    } else if (barPos === "left") {
      x = below
      y = anchorScreenPos.y + anchorH / 2 - contentHeight / 2
    } else if (barPos === "right") {
      x = screenW - below - contentWidth
      y = anchorScreenPos.y + anchorH / 2 - contentHeight / 2
    } else { // "top" (default)
      x = anchorScreenPos.x + anchorW / 2 - contentWidth / 2
      y = below
    }
    x = Math.max(margin, Math.min(x, screenW - contentWidth - margin))
    y = Math.max(margin, Math.min(y, screenH - contentHeight - margin))
    return Qt.point(g.snap(x), g.snap(y))
  }

  // --- popout coordination (same-bar single-popout model) -----------------

  onOpenChanged: {
    if (open) {
      focusPrimed = false
      beginFocusPrime()
      if (focusTarget) Qt.callLater(function() {
        if (root.open && root.focusTarget) root.focusTarget.forceActiveFocus()
      })
    } else {
      focusPrimeTimer.stop()
      focusPrimed = false
    }
    if (!bar) return
    if (open) {
      popoutSwitchClosing = false
      popoutSwitching = bar.activePopout && bar.activePopout !== coordinatorKey
      bar.requestPopout(coordinatorKey)
      if (popoutSwitching) popoutSwitchTimer.restart()
    } else {
      popoutSwitchClosing = !!(owner && owner.popoutSwitchClosing)
      popoutSwitching = false
      if (bar.activePopout === coordinatorKey) bar.releasePopout(coordinatorKey)
      if (popoutSwitchClosing) closeSwitchTimer.restart()
    }
  }

  Timer {
    id: focusPrimeTimer
    interval: 75
    onTriggered: if (root.open) root.focusPrimed = true
  }

  Timer {
    id: popoutSwitchTimer
    interval: 150
    onTriggered: root.popoutSwitching = false
  }

  Timer {
    id: closeSwitchTimer
    interval: 1
    onTriggered: root.popoutSwitchClosing = false
  }

  // --- outside-click dismissal --------------------------------------------

  MouseArea {
    id: dismissArea
    anchors.fill: parent
    enabled: root.open
    acceptedButtons: Qt.AllButtons
    hoverEnabled: true
    property bool hoveringBar: false
    cursorShape: hoveringBar ? Qt.PointingHandCursor : Qt.ArrowCursor

    function inBarRegion(px, py) {
      if (root.barPos === "bottom") return py >= root.screenH - root._barStripSize
      if (root.barPos === "left") return px <= root._barStripSize
      if (root.barPos === "right") return px >= root.screenW - root._barStripSize
      return py <= root._barStripSize
    }

    function barPoint(px, py) {
      if (root.barPos === "bottom") return Qt.point(px, py - (root.screenH - root.barH))
      if (root.barPos === "right") return Qt.point(px - (root.screenW - root.barW), py)
      return Qt.point(px, py)
    }

    function pressTargetAt(px, py) {
      if (!root.anchorWindow || !root.anchorWindow.contentItem || !root.bar || !root.bar.clickTargets) return null
      var p = barPoint(px, py)
      var targets = root.bar.clickTargets
      for (var i = targets.length - 1; i >= 0; i--) {
        var target = targets[i]
        if (!target || !target.triggerPress || target.visible === false || target.opacity === 0 || !target.mapToItem) continue
        if (root.bar.targetBelongsToWindow && !root.bar.targetBelongsToWindow(target, root.anchorWindow)) continue
        var pos = root.anchorWindow.itemPosition(target)
        if (p.x >= pos.x && p.x <= pos.x + target.width && p.y >= pos.y && p.y <= pos.y + target.height) return target
      }
      return null
    }

    function forwardBarClick(px, py, button) {
      if (button !== Qt.LeftButton && button !== Qt.RightButton && button !== Qt.MiddleButton) return false
      var target = pressTargetAt(px, py)
      if (!target) return false
      target.triggerPress(button)
      return true
    }

    onPositionChanged: function(mouse) { hoveringBar = inBarRegion(mouse.x, mouse.y) }
    onExited: hoveringBar = false
    onClicked: function(mouse) {
      if (root.focusPrimed && inBarRegion(mouse.x, mouse.y) && forwardBarClick(mouse.x, mouse.y, mouse.button)) return
      root.close()
    }
  }

  // A transparent twin on every other output, to catch a click there.
  Variants {
    model: root.open ? Quickshell.screens : []

    delegate: Component {
      PanelWindow {
        required property var modelData

        screen: modelData
        visible: root.open && !!root.screen && modelData.name !== root.screen.name
        color: "transparent"
        exclusionMode: ExclusionMode.Ignore

        WlrLayershell.namespace: "omarchy-keyboard-panel-dismiss"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

        anchors {
          top: true
          bottom: true
          left: true
          right: true
        }

        MouseArea {
          anchors.fill: parent
          acceptedButtons: Qt.AllButtons
          onPressed: root.close()
        }
      }
    }
  }

  // --- card ----------------------------------------------------------------

  Rectangle {
    id: card
    x: root.cardOrigin.x
    y: root.cardOrigin.y
    width: root.contentWidth
    height: root.contentHeight
    antialiasing: false
    color: Role.edge
    visible: root.open || root.popoutSwitching

    // The face, inside the hairline.
    Rectangle {
      anchors.fill: parent
      anchors.margins: root.g.hair
      antialiasing: false
      color: Role.ground
    }

    // Swallow clicks on the card so they do not reach the dismissal area.
    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.AllButtons
    }

    Item {
      id: contentHolder
      anchors.fill: parent
      anchors.margins: root.inset
    }
  }
}
