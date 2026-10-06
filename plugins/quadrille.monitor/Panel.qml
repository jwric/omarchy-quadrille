import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs.Ui
import qs.Commons
import "Model.js" as Model
import "Q"

Panel {
  id: root
  moduleName: "omarchy.monitor"
  ipcTarget: "omarchy.monitor"
  manageIpc: false

  // manageIpc: false so this panel can own the single IpcHandler the target
  // permits — needed for the brightness + state methods below.
  property int brightnessPercent: 0
  property int pendingBrightnessPercent: 0
  property bool brightnessSetQueued: false
  property bool brightnessAvailable: false
  property string internalMonitor: ""
  property string externalMonitor: ""
  property string focusedMonitor: ""
  property bool internalEnabled: false
  property bool mirrorEnabled: false
  property string monitorScale: ""
  property var displays: []
  property int enabledDisplayCount: 0

  // Carry sub-notch touchpad deltas between wheel events.
  property real wheelAccumulator: 0

  // Cursor model shared by keyboard and mouse. Sections:
  //   "brightness" - single slider row, selectedIndex = -1 sentinel
  //                  (mirrors Audio's slider rows). Only present if a
  //                  controllable backlight was detected.
  //   "scale"      - 6 Button scale presets; treated as a single
  //                  horizontal row from j/k's perspective. h/l moves
  //                  between presets, identical to bluetooth's header.
  //   "monitors"   - vertical display row list for enabling/disabling displays;
  //                  j/k walks each row.
  // Mouse hover on a target updates root state via the components' `hovered`
  // signal so keyboard cursor and pointer share one highlight.
  readonly property var scalePresets: ["1", "1.25", "1.6", "2", "3", "4"]
  readonly property var scaleValues: {
    for (var i = 0; i < displays.length; i++) {
      var display = displays[i]
      if (display && display.focused)
        return Model.availableScales(scalePresets, display.width, display.height)
    }
    return scalePresets
  }
  property string focusSection: "scale"
  property int selectedIndex: 0
  property bool cursorActive: false

  // Text size slider — curated macOS-style notches (px). The panel snaps to
  // these stops; the CLI (omarchy-display-text-size) accepts any integer in range.
  readonly property var textSizeStops: [9, 10, 11, 12, 14, 16, 20]
  // While a change is in flight, the chosen stop index overrides the live
  // base-size so the knob doesn't snap back during the file round-trip. -1 =
  // no pending change; follow Style.font.baseSize.
  property int textSizePreviewIndex: -1

  // A text-size change reflows the whole panel (both font and spacing scale),
  // which slides rows under a stationary pointer and fires synthetic hover.
  // While true, hover is not allowed to hijack the keyboard focus section —
  // otherwise h/l on the text-size slider can jump focus to another row.
  property bool reflowingText: false
  function markReflowing() {
    root.reflowingText = true
    reflowSettle.restart()
  }

  readonly property var visibleSections: {
    var list = []
    if (brightnessAvailable) list.push("brightness")
    list.push("textsize")
    list.push("scale")
    if (displays.length > 1) list.push("monitors")
    return list
  }

  function sectionCount(section) {
    if (section === "brightness") return 0  // only the slider sentinel at -1
    if (section === "textsize") return 0    // slider sentinel at -1, like brightness
    if (section === "scale") return scaleValues.length
    if (section === "monitors") return displays.length
    return 0
  }

  function sectionIsSingleRow(section) {
    // brightness and text size are lone sliders; scale presets sit horizontally.
    return section === "brightness" || section === "textsize" || section === "scale"
  }

  function sectionFirstIndex(section) {
    if (section === "brightness" || section === "textsize") return -1
    return 0
  }

  function moveCursor(delta) {
    var sections = visibleSections
    if (!sections || sections.length === 0) return
    var sIdx = sections.indexOf(focusSection)
    if (sIdx < 0) {
      focusSection = sections[0]
      selectedIndex = sectionFirstIndex(focusSection)
      return
    }
    var inSingleRow = sectionIsSingleRow(focusSection)
    var max = inSingleRow ? 0 : sectionCount(focusSection) - 1

    if (delta > 0) {
      if (!inSingleRow && selectedIndex < max) { selectedIndex = selectedIndex + 1; return }
      if (sIdx < sections.length - 1) {
        focusSection = sections[sIdx + 1]
        selectedIndex = sectionFirstIndex(focusSection)
      }
    } else {
      if (!inSingleRow && selectedIndex > 0) { selectedIndex = selectedIndex - 1; return }
      if (sIdx > 0) {
        var prev = sections[sIdx - 1]
        focusSection = prev
        // Coming up from below — land on the last navigable row of the prev
        // section, or its sentinel for single-row sections.
        selectedIndex = sectionIsSingleRow(prev) ? sectionFirstIndex(prev) : sectionCount(prev) - 1
      }
    }
  }

  // h/l: in scale section, walks the preset row; everywhere else, no-op
  // because adjustBrightness handles horizontal motion on the brightness
  // slider.
  function moveCursorH(delta) {
    if (focusSection !== "scale") return
    var next = selectedIndex + delta
    if (next < 0) next = 0
    if (next > scaleValues.length - 1) next = scaleValues.length - 1
    selectedIndex = next
  }

  function adjustBrightness(delta) {
    if (focusSection !== "brightness") return
    if (!brightnessAvailable) return
    setBrightness(root.brightnessPercent + delta)
  }

  function activateCursor() {
    if (focusSection === "scale" && selectedIndex >= 0 && selectedIndex < scaleValues.length) {
      setScale(scaleValues[selectedIndex])
      return
    }
    if (focusSection === "monitors" && selectedIndex >= 0 && selectedIndex < displays.length) {
      var d = displays[selectedIndex]
      if (d) toggleDisplay(d.name, d.enabled)
    }
    // brightness: no separate action; the slider value is the action.
  }

  function clampCursor() {
    var sections = visibleSections
    if (!sections || !sections.length) return
    if (sections.indexOf(focusSection) < 0) {
      focusSection = sections[0]
      selectedIndex = sectionFirstIndex(focusSection)
      return
    }
    var count = sectionCount(focusSection)
    if (sectionIsSingleRow(focusSection)) {
      // brightness/text size use the -1 sentinel; scale clamps into the presets.
      if (focusSection === "brightness" || focusSection === "textsize") selectedIndex = -1
      else if (selectedIndex < 0 || selectedIndex >= count) selectedIndex = 0
      return
    }
    if (count === 0) {
      var sIdx = sections.indexOf(focusSection)
      focusSection = sIdx > 0 ? sections[sIdx - 1] : sections[0]
      selectedIndex = sectionFirstIndex(focusSection)
      return
    }
    if (selectedIndex > count - 1) selectedIndex = count - 1
    if (selectedIndex < 0) selectedIndex = 0
  }

  // Keep the keyboard-focused row inside the viewport when the panel grows
  // taller than its allotted height (lots of displays). Mirrors audio's
  // ensureCursorVisible helper; the viewport is the kit's whole-pixel QScroll.
  function ensureCursorVisible(item) {
    if (!item || !scroll) return
    scroll.ensureItemVisible(item)
  }

  function brightnessIpc(percent) {
    var value = Number(percent)
    root.setBrightness(value)
    return "got " + root.pendingBrightnessPercent
  }

  function stateIpc() {
    return JSON.stringify({
      brightness: root.brightnessPercent,
      brightnessAvailable: root.brightnessAvailable,
      focusedMonitor: root.focusedMonitor,
      scale: root.monitorScale,
      displays: root.displays
    })
  }

  IpcHandler {
    target: "omarchy.monitor"

    function brightness(percent: string): string { return root.brightnessIpc(percent) }
    function state(): string { return root.stateIpc() }
    function open() { root.open() }
    function close() { root.close() }
    function toggle() { root.toggle() }
    function show() { root.open() }
    function hide() { root.close() }
  }

  function refresh() {
    if (!stateProc.running) stateProc.running = true
  }

  function setBrightness(value) {
    var percent = Model.clampBrightness(value)
    root.brightnessPercent = percent
    root.pendingBrightnessPercent = percent

    if (setBrightnessProc.running) {
      root.brightnessSetQueued = true
      return
    }

    root.brightnessSetQueued = false
    setBrightnessProc.command = ["omarchy-brightness-display", "--no-osd", "--monitor", root.focusedMonitor, percent + "%"]
    setBrightnessProc.running = true
  }

  function previewBrightness(value) {
    root.brightnessPercent = Model.clampBrightness(value)
    brightnessDebounce.restart()
  }

  function showBrightnessOsd(percent) {
    if (!bar || !bar.shell) return
    bar.shell.summon("omarchy.osd", JSON.stringify({
      icon: "brightness",
      value: percent
    }))
  }

  function normalizeScale(scale) {
    return Model.normalizeScale(scale)
  }

  function activeScaleIndex() {
    for (var i = 0; i < displays.length; i++) {
      var display = displays[i]
      if (display && display.focused)
        return Model.matchingScaleIndex(scaleValues, monitorScale, display.width, display.height)
    }
    return -1
  }

  function effectiveScale(scale) {
    for (var i = 0; i < displays.length; i++) {
      var display = displays[i]
      if (display && display.focused)
        return Model.cleanScale(scale, display.width, display.height)
    }
    return normalizeScale(scale)
  }

  // Playful mood-name for a given brightness percent. Bands intentionally
  // span ~10–20 points so casual tweaks change the label, while small
  // nudges within one band don't.
  function brightnessName(percent) {
    return Model.brightnessName(percent)
  }

  function updateDisplays(displaysJson) {
    var parsed = Model.parseDisplays(displaysJson)
    root.displays = parsed.displays
    root.enabledDisplayCount = parsed.enabledDisplayCount
  }

  function toggleDisplay(name, enabled) {
    if (!name) return
    if (enabled && root.enabledDisplayCount <= 1) return

    actionProc.command = ["hyprctl", "keyword", "monitor", name + (enabled ? ",disable" : ",preferred,auto,auto")]
    if (!actionProc.running) actionProc.running = true
  }

  function setScale(scale) {
    actionProc.command = ["bash", "-c", "omarchy-hyprland-monitor-scaling " + scale]
    if (!actionProc.running) actionProc.running = true
  }

  // ---- Text size (shell base font + GTK text-scaling, via one CLI) ----
  function nearestTextStop(px) {
    var best = 0
    var bestDist = 1e9
    for (var i = 0; i < textSizeStops.length; i++) {
      var d = Math.abs(textSizeStops[i] - px)
      if (d < bestDist) { bestDist = d; best = i }
    }
    return best
  }

  // Effective stop index: the pending choice while a change is in flight,
  // otherwise whatever Style's live base-size rounds to.
  function currentTextIndex() {
    return textSizePreviewIndex >= 0 ? textSizePreviewIndex : nearestTextStop(Style.font.baseSize)
  }

  // px shown in the header: the pending stop if any, else the true base-size
  // (which may be an off-notch value set from the CLI).
  function displayedTextPx() {
    return textSizePreviewIndex >= 0 ? textSizeStops[textSizePreviewIndex] : Style.font.baseSize
  }

  function setTextSize(px) {
    textScaleProc.command = ["omarchy-display-text-size", String(px)]
    if (!textScaleProc.running) textScaleProc.running = true
  }

  function adjustTextSize(deltaSteps) {
    var idx = currentTextIndex() + deltaSteps
    if (idx < 0) idx = 0
    if (idx > textSizeStops.length - 1) idx = textSizeStops.length - 1
    markReflowing()
    textSizePreviewIndex = idx
    setTextSize(textSizeStops[idx])
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: refresh()

  // KeyboardPanel primes focus at open-time, so SUPER-bound IPC summons land
  // with j/k ready to navigate. Keep a default landing point, but don't paint
  // the cursor until hover or the first navigation key.
  onOpenedChanged: {
    if (opened) {
      refresh()
      if (brightnessAvailable) {
        focusSection = "brightness"
        selectedIndex = -1
      } else {
        focusSection = "scale"
        selectedIndex = 0
      }
      cursorActive = false
    }
  }

  onBrightnessAvailableChanged: clampCursor()
  onDisplaysChanged: clampCursor()
  onScaleValuesChanged: clampCursor()
  onVisibleSectionsChanged: clampCursor()

  // Only poll while the panel is open; the bar glyph tracks monitor count via
  // Quickshell.screens, and open-time refresh + Component.onCompleted cover the
  // rest. External brightness changes are reflected whenever the panel is open.
  Timer {
    interval: 5000
    running: root.opened
    repeat: true
    onTriggered: root.refresh()
  }

  Process {
    id: stateProc
    command: ["omarchy-monitor-state"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        var lines = String(text || "").split("\n")
        var brightness = String(lines[0] || "").trim()
        root.brightnessAvailable = brightness !== "unavailable" && brightness !== ""
        root.brightnessPercent = root.brightnessAvailable ? Math.max(0, Math.min(100, parseInt(brightness, 10))) : 0
        root.internalMonitor = String(lines[1] || "").trim()
        root.externalMonitor = String(lines[2] || "").trim()
        root.internalEnabled = String(lines[3] || "").trim() !== ""
        root.mirrorEnabled = String(lines[4] || "").trim() === root.externalMonitor && root.externalMonitor !== ""
        root.focusedMonitor = String(lines[5] || "").trim()
        root.monitorScale = root.normalizeScale(String(lines[6] || "").trim())
        root.updateDisplays(String(lines[7] || "[]").trim())
      }
    }
  }

  Timer {
    id: brightnessDebounce
    interval: 180
    repeat: false
    onTriggered: root.setBrightness(root.brightnessPercent)
  }

  Process {
    id: setBrightnessProc
    stdout: StdioCollector { waitForEnd: true }
    // Do NOT call refresh() after a brightness set completes. The local
    // brightnessPercent we just wrote is authoritative; re-reading via
    // `omarchy-brightness-display` races the hardware/driver and can
    // return an empty string, which the parser then coerces to 0 —
    // visible as a "bounce to zero" after h/l keypresses. External
    // brightness changes are still picked up by the 5s periodic refresh,
    // the open-time refresh, and Component.onCompleted.
    onRunningChanged: {
      if (running) return
      if (root.brightnessSetQueued) {
        root.setBrightness(root.pendingBrightnessPercent)
      }
    }
  }

  Process {
    id: actionProc
    stdout: StdioCollector { waitForEnd: true }
    onRunningChanged: if (!running) root.refresh()
  }

  // Applies text size via the CLI, which rewrites the shell override file;
  // Style picks the new base-size up through its own file watch, so there's
  // nothing to refresh here.
  Process {
    id: textScaleProc
    stdout: StdioCollector { waitForEnd: true }
  }

  // Clears the hover-suppression flag once the reflow triggered by a text-size
  // change has settled.
  Timer {
    id: reflowSettle
    interval: 300
    repeat: false
    onTriggered: root.reflowingText = false
  }

  // Once Style's base-size catches up to the pending choice, drop the preview
  // so the slider tracks the live value again. The change itself reflows the
  // panel, so suppress hover for a beat while it lands.
  Connections {
    target: Style
    function onFontBaseSizeChanged() {
      root.markReflowing()
      if (root.textSizePreviewIndex >= 0
          && root.nearestTextStop(Style.font.baseSize) === root.textSizePreviewIndex)
        root.textSizePreviewIndex = -1
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: Quickshell.screens.length > 1 ? "󰍺" : "󰍹"
    onPressed: function(b) { root.toggle() }
    onWheelMoved: function(delta) {
      if (!root.brightnessAvailable) return
      var wheel = Util.wheelSteps(root.wheelAccumulator, delta)
      root.wheelAccumulator = wheel.remainder
      if (wheel.steps === 0) return
      root.setBrightness(root.brightnessPercent + wheel.steps * 5)
      root.showBrightnessOsd(root.brightnessPercent)
    }
  }

  // ---- Sprites for what the stock panel drew in Nerd Font glyphs ----
  Icons { id: icons }

  // A text-size change is in flight: the CLI is running, or Style has not yet
  // caught up with the chosen stop. The picker's lit cells go muted until it lands.
  readonly property bool textSizeBusy: textSizePreviewIndex >= 0 || textScaleProc.running

  // A display plugged in or out changes the list, and with it the card's
  // height: read the state then (read-only), so the popup opens at its final
  // size instead of growing when the open-time refresh lands.
  Connections {
    target: Quickshell
    function onScreensChanged() { root.refresh() }
  }

  QPopup {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(panel.cardWidth(44))
    contentHeight: panel.fittedContentHeight(panelColumn.implicitHeight, panel.g.px(320))

    // Both sliders the same width, one their cells fill exactly (brightness:
    // cells of 3 and a gap of 1; text size: one cell a stop), so their ends line
    // up; the reading takes the 4 cells at the right.
    readonly property int sliderVpx: {
      var avail = panel.g.vpx(panel.innerWidth) - (4 + 3 + 24 + 4)
      var stops = Math.max(2, root.textSizeStops.length)
      var a = 4, b = stops, lcm = a * b
      for (var k = Math.max(a, b); k <= a * b; k++) if (k % a === 0 && k % b === 0) { lcm = k; break }
      var w = Math.floor((avail + 1) / lcm) * lcm - 1
      return w > 0 ? w : avail
    }

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) {
        if (!root.cursorActive) { root.cursorActive = true; return }
        if (dy !== 0) root.moveCursor(dy)
        else if (dx !== 0) {
          if (root.focusSection === "brightness") root.adjustBrightness(dx * 5)
          else if (root.focusSection === "textsize") root.adjustTextSize(dx)
          else if (root.focusSection === "scale") root.moveCursorH(dx)
        }
      }
      onActivateRequested: if (root.cursorActive) root.activateCursor()
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }

      // The scroll's gutter hangs into the card's right padding, so the content
      // keeps the same air on both sides and the scroll mark (there only with
      // more displays than fit) sits just inside the hairline.
      QScroll {
        id: scroll
        width: parent.width + scroll.gutter
        height: parent.height
        contentHeight: panelColumn.implicitHeight

        Column {
          id: panelColumn
          width: parent.width
          spacing: panel.g.px(6)

          // ---------- Hero: display(s) · title / the brightness's mood ----------
          // The icon is a native 15 x 15 sprite (outline in ink, the glass its
          // second tone in muted), not a 7 x 7 one doubled.
          QHero {
            width: parent.width
            icon: root.displays.length > 1 ? icons.heroDisplays : icons.heroDisplay
            iconLevel: 1
            iconDim: Role.muted
            title: "Display"
            subtitle: root.brightnessAvailable
              ? root.brightnessName(brightnessSlider.dragging ? brightnessSlider.liveValue : root.brightnessPercent)
              : "Fixed brightness"
          }

          // ---------- Brightness ----------
          // The stock panel hides this section where the focused output has no
          // control; here it stays, so the card is the same height on every
          // output (it would otherwise jump when the state arrives after the
          // popup opens on another output), and says so in a muted line. It is
          // out of the keyboard's path then, as in the stock (visibleSections).
          Group {
            name: "BRIGHTNESS"
            width: parent.width
            height: implicitHeight

            Item {
              id: brightnessRow
              width: parent.width
              height: panel.g.px(13)
              readonly property bool hasCursor: root.brightnessAvailable && root.cursorActive && root.focusSection === "brightness" && root.selectedIndex === -1
              onHasCursorChanged: if (hasCursor) root.ensureCursorVisible(brightnessRow)

              QEmpty {
                visible: !root.brightnessAvailable
                width: parent.width
                icon: Sprites.brightness
                readonly property string full: "Not adjustable on " + root.focusedMonitor
                // Drop the output's name, not cut it, when it does not fit.
                text: root.focusedMonitor !== "" && full.length <= panel.g.columns(width - panel.g.px(13))
                  ? full : "Not adjustable"
              }
              QSlider {
                id: brightnessSlider
                visible: root.brightnessAvailable
                enabled: root.brightnessAvailable
                x: panel.g.px(4)
                y: panel.g.centre(parent.height, height)
                width: panel.g.px(panel.sliderVpx)
                minimum: 1
                maximum: 100
                step: 1
                integer: true
                value: root.brightnessPercent
                onMoved: function(v) { root.previewBrightness(v) }
                onReleased: function(v) {
                  brightnessDebounce.stop()
                  root.setBrightness(v)
                }
              }
              PixelText {
                visible: root.brightnessAvailable
                x: parent.width - panel.g.px(4) - width
                y: panel.g.centre(parent.height, height)
                text: Math.round(brightnessSlider.dragging ? brightnessSlider.liveValue : root.brightnessPercent) + "%"
                ink: Role.muted
              }
              Loader { anchors.fill: parent; active: brightnessRow.hasCursor; sourceComponent: Brackets { arm: 3 } }
              HoverHandler {
                enabled: root.brightnessAvailable
                onHoveredChanged: if (hovered && !root.reflowingText) {
                  root.cursorActive = true
                  root.focusSection = "brightness"
                  root.selectedIndex = -1
                }
              }
            }
          }

          // ---------- Text size: a picker of the curated stops ----------
          Group {
            name: "TEXT SIZE"
            width: parent.width
            height: implicitHeight

            Item {
              id: textSizeRow
              width: parent.width
              height: panel.g.px(13)
              readonly property bool hasCursor: root.cursorActive && root.focusSection === "textsize" && root.selectedIndex === -1
              onHasCursorChanged: if (hasCursor) root.ensureCursorVisible(textSizeRow)

              QSlider {
                id: textSizeSlider
                x: panel.g.px(4)
                y: panel.g.centre(parent.height, height)
                width: panel.g.px(panel.sliderVpx)
                discrete: true
                minimum: 0
                maximum: root.textSizeStops.length - 1
                step: 1
                integer: true
                value: root.currentTextIndex()
                fill: root.textSizeBusy ? Role.muted : Role.ink
                onReleased: function(v) { root.setTextSize(root.textSizeStops[Math.round(v)]) }
              }
              PixelText {
                x: parent.width - panel.g.px(4) - width
                y: panel.g.centre(parent.height, height)
                text: (textSizeSlider.dragging
                       ? root.textSizeStops[Math.round(textSizeSlider.liveValue)]
                       : root.displayedTextPx()) + "px"
                ink: Role.muted
              }
              Loader { anchors.fill: parent; active: textSizeRow.hasCursor; sourceComponent: Brackets { arm: 3 } }
              HoverHandler {
                onHoveredChanged: if (hovered && !root.reflowingText) {
                  root.cursorActive = true
                  root.focusSection = "textsize"
                  root.selectedIndex = -1
                }
              }
            }
          }

          // ---------- Scale: a button a preset, the one in use lit ----------
          // It applies to the focused output, which is named when there is more
          // than one display on.
          Group {
            name: root.focusedMonitor !== "" && root.enabledDisplayCount > 1
              ? "SCALE · " + root.focusedMonitor : "SCALE"
            width: parent.width
            height: implicitHeight

            Item {
              width: parent.width
              height: panel.g.line

              Row {
                id: scaleRow
                readonly property int count: root.scaleValues.length
                readonly property real cellWidth: count > 0
                  ? panel.g.floor((parent.width - spacing * (count - 1)) / count)
                  : 0
                x: panel.g.centre(parent.width, count * cellWidth + Math.max(0, count - 1) * spacing)
                spacing: panel.g.px(3)

                Repeater {
                  model: root.scaleValues

                  ScalePill {
                    required property string modelData
                    required property int index

                    scaleValue: modelData
                    scaleIndex: index
                    fixedWidth: scaleRow.cellWidth
                  }
                }
              }
            }
          }

          // ---------- Displays ----------
          Group {
            name: "DISPLAYS"
            visible: root.displays.length > 1
            width: parent.width
            height: visible ? implicitHeight : 0

            Column {
              width: parent.width
              spacing: panel.g.px(2)

              Repeater {
                model: root.displays

                MonitorRow {
                  required property var modelData
                  required property int index

                  width: parent.width
                  display: modelData
                  rowIndex: index
                }
              }
            }
          }
        }
      }
    }
  }

  component ScalePill: QButton {
    id: pill
    required property string scaleValue
    required property int scaleIndex

    text: root.effectiveScale(scaleValue) + "x"

    active: root.activeScaleIndex() === scaleIndex
    hasCursor: root.cursorActive && root.focusSection === "scale" && root.selectedIndex === scaleIndex

    onClicked: root.setScale(scaleValue)
    onHovered: function(isHovered) {
      if (!isHovered || root.reflowingText) return
      root.cursorActive = true
      root.focusSection = "scale"
      root.selectedIndex = pill.scaleIndex
    }
  }

  // A display: the focused one is the inverse block, one that is on carries a
  // tick at the right, and the last one on (which cannot be turned off) is faint
  // unless it is the focused one, where the block wins (the stock dims the row
  // to 45 % instead; the kit has no faint inverse block).
  component MonitorRow: QRow {
    id: monitorRow
    required property var display
    required property int rowIndex

    readonly property bool isFocused: display && display.focused
    readonly property bool canToggle: display && (!display.enabled || root.enabledDisplayCount > 1)

    icon: icons.display
    text: monitorRow.display ? String(monitorRow.display.name) : ""
    // Every row keeps the reading's room (blank when not focused), so a long
    // name wraps the same whichever display has the focus: no height change
    // when the focus moves.
    detail: isFocused ? "FOCUSED" : "       "
    current: isFocused
    dimmed: !canToggle
    hasCursor: root.cursorActive && root.focusSection === "monitors" && root.selectedIndex === rowIndex
    onHasCursorChanged: if (hasCursor) root.ensureCursorVisible(monitorRow)

    onHovered: if (!root.reflowingText) {
      root.cursorActive = true
      root.focusSection = "monitors"
      root.selectedIndex = monitorRow.rowIndex
    }
    onClicked: function(b) {
      if (b === Qt.LeftButton && monitorRow.canToggle)
        root.toggleDisplay(monitorRow.display.name, monitorRow.display.enabled)
    }

    // The tick's slot is always there, so a name does not reflow when its
    // display is turned on or off; it sits on the first line's capitals, half
    // a cell clear of the reading before it.
    Item {
      width: monitorRow.g.px(3 + 7)
      height: monitorRow.height
      Sprite {
        visible: !!monitorRow.display && monitorRow.display.enabled === true
        x: monitorRow.g.px(3)
        y: monitorRow.g.hair + monitorRow.g.onCaps(7)
        rows: icons.tick
        color: monitorRow.ink
      }
    }
  }
}
