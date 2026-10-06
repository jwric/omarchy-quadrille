import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import QtQuick
import QtQuick.Effects
import QtQuick.Shapes
import qs.Commons
import qs.Ui
import "Q"
import "Physical.js" as Physical

// quadrille.background: omarchy.background (cloned; MIT), with the graticule
// wallpaper drawn instead of scaled.
//
// The host contract is the stock plugin's, unchanged: the `background` IPC target
// (refresh / set / setInstant / transition / themeTransition), the
// ~/.local/state/omarchy/current/background link, the reveal between pictures,
// the double clicks. Two things differ:
//
//   * A quadrille graticule is drawn per output, physically calibrated, with
//     a true-size ruler and an active-area elevation. Dither, hairlines and native
//     bitmap lettering share that output's whole-device-pixel grid.
//   * That picture does not reveal: it is there at once, and the theme with it.
//     (A photograph, or a wallpaper that is not a quadrille graticule, goes through
//     the stock path whole: the same reveal, the same smooth scaling.)
//
// If the shader cannot be built the PNG is shown, as the stock background does.
Item {
  id: root

  readonly property string home: Quickshell.env("HOME")
  readonly property string stateHome: home + "/.local/state"
  readonly property string currentBackgroundLink: stateHome + "/omarchy/current/background"
  readonly property bool hyprlandSession: Quickshell.env("HYPRLAND_INSTANCE_SIGNATURE") !== ""
  property var displayOverrides: ({})
  property bool monitorRefreshQueued: false
  readonly property var monitorData: {
    var values = Hyprland.monitors.values
    var out = []
    for (var i = 0; i < values.length; i++) {
      var object = values[i].lastIpcObject
      if (object && object.width > 0 && object.height > 0 && object.scale > 0) out.push(object)
    }
    return out
  }

  function monitorFor(screen) {
    for (var i = 0; i < monitorData.length; i++) if (monitorData[i].name === screen.name) return monitorData[i]
    // Non-Hyprland fallback, or a compositor without monitor metadata.
    if (hyprlandSession) return null
    var scale = screen.devicePixelRatio > 0 ? screen.devicePixelRatio : 1
    return { name: screen.name, width: Math.round(screen.width * scale), height: Math.round(screen.height * scale), scale: scale }
  }

  function refreshDisplays() {
    if (!hyprlandSession || monitorRefreshQueued) return
    monitorRefreshQueued = true
    Qt.callLater(function() { root.monitorRefreshQueued = false; Hyprland.refreshMonitors() })
  }

  FileView {
    id: displayFile
    path: root.home + "/.config/quadrille/displays.toml"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      try { root.displayOverrides = Physical.parseOverrides(text()) }
      catch (error) { console.warn("quadrille.background: displays.toml:", error); root.displayOverrides = ({}) }
    }
    onLoadFailed: root.displayOverrides = ({})
  }

  Connections {
    target: Hyprland
    function onRawEvent(event) {
      if (event && (String(event.name).indexOf("monitor") === 0 || event.name === "configreloaded")) root.refreshDisplays()
    }
  }
  Connections {
    target: Quickshell
    function onScreensChanged() { root.refreshDisplays() }
  }
  // The IPC singleton connects after component creation. This is a startup
  // retry, not a poll; events alone refresh it for the lifetime of the shell.
  Timer { interval: 300; running: true; onTriggered: root.refreshDisplays() }

  property string currentBackground: ""
  // Whether currentBackground is a quadrille graticule (isPixelWallpaper).
  property bool pixelMode: false
  property string displayedBackground: ""
  property string incomingBackground: ""
  property string oldBackground: ""
  property bool finishingTransition: false
  property int backgroundVersion: 0
  property int revealStartedVersion: -1
  property int pendingThemeVersion: -1
  property string pendingColorsRaw: ""
  property string pendingShellRaw: ""
  property real revealProgress: 1

  // A quadrille theme's wallpaper: backgrounds/*graticule*.png. (The shell stages a
  // theme under current/theme, so the path no longer says whose it is; the name
  // does, and Role takes whatever theme is live.)
  function isPixelWallpaper(path) {
    return /\/backgrounds\/[^\/]*graticule[^\/]*\.png$/i.test(String(path || ""))
  }

  function imageUrl(path) {
    return Util.fileUrl(path)
  }

  function refreshBackground() {
    displayFile.reload()
    refreshDisplays()
    if (!readlinkProc.running) readlinkProc.running = true
  }

  function setBackground(path, instant) {
    transitionBackground("", path, path, instant, false)
  }

  function transitionBackground(fromPath, path, finalPath, instant, force) {
    path = String(path || "").trim()
    finalPath = String(finalPath || path).trim()
    fromPath = String(fromPath || "").trim()
    if (!path || (!force && finalPath === currentBackground)) return
    currentBackground = finalPath
    pixelMode = isPixelWallpaper(finalPath)
    // A drawn picture has nothing to reveal through.
    if (pixelMode) instant = true
    backgroundVersion += 1
    revealStartedVersion = -1

    revealAnimation.stop()
    finishingTransition = false

    if (instant || !displayedBackground) {
      oldBackground = ""
      incomingBackground = ""
      displayedBackground = path
      revealProgress = 1
      return
    }

    oldBackground = fromPath || displayedBackground
    incomingBackground = path
    revealProgress = 0
  }

  function setPendingTheme(colorsB64, shellB64) {
    pendingColorsRaw = Util.decodeBase64(colorsB64)
    pendingShellRaw = Util.decodeBase64(shellB64)
    pendingThemeVersion = backgroundVersion
    pendingThemeFallbackTimer.restart()
  }

  function applyPendingTheme() {
    // Background polling can advance backgroundVersion while a theme switch is
    // pending; the latest theme payload should still apply.
    if (pendingThemeVersion < 0) return
    pendingThemeFallbackTimer.stop()
    Color.loadColors(pendingColorsRaw)
    // Color.loadShell also refreshes Style so the type scale flips with the
    // background reveal instead of waiting for a separate reload path.
    Color.loadShell(pendingShellRaw)
    Style.scheduleRefresh()
    pendingThemeVersion = -1
    pendingColorsRaw = ""
    pendingShellRaw = ""
  }

  function transitionBackgroundWithTheme(fromPath, path, finalPath, colorsB64, shellB64) {
    transitionBackground(fromPath, path, finalPath, false, true)
    setPendingTheme(colorsB64, shellB64)
    if (!incomingBackground || revealProgress >= 1) applyPendingTheme()
  }

  function startReveal(panel) {
    if (!incomingBackground) return
    panel.maskReady = true
    if (revealStartedVersion === backgroundVersion) return
    revealStartedVersion = backgroundVersion
    applyPendingTheme()
    revealAnimation.restart()
  }

  function openSelector() {
    if (!bgSwitchProc.running) bgSwitchProc.running = true
  }

  function openThemeSwitcher() {
    if (!themeSwitchProc.running) themeSwitchProc.running = true
  }

  Process {
    id: bgSwitchProc
    command: ["bash", "-c", "background=$(omarchy-theme-bg-switcher); [[ -n $background ]] && omarchy-theme-bg-set \"$background\""]
    onExited: root.refreshBackground()
  }

  Process {
    id: themeSwitchProc
    command: ["bash", "-c", "theme=$(omarchy-theme-switcher); [[ -n $theme ]] && omarchy-theme-set \"$theme\" >/dev/null 2>&1 &"]
    onExited: root.refreshBackground()
  }

  Process {
    id: readlinkProc
    command: ["readlink", "-f", root.currentBackgroundLink]
    stdout: StdioCollector {
      onStreamFinished: root.setBackground(String(text || "").trim(), false)
    }
  }

  IpcHandler {
    target: "background"

    function refresh(): void {
      root.refreshBackground()
    }

    function set(path: string): void {
      root.setBackground(path, false)
    }

    function setInstant(path: string): void {
      root.setBackground(path, true)
    }

    function transition(fromPath: string, path: string): void {
      root.transitionBackground(fromPath, path, path, false, false)
    }

    function themeTransition(fromPath: string, path: string, finalPath: string, colorsB64: string, shellB64: string): void {
      root.transitionBackgroundWithTheme(fromPath, path, finalPath, colorsB64, shellB64)
    }
  }

  Timer {
    id: pendingThemeFallbackTimer
    interval: 300
    repeat: false
    onTriggered: root.applyPendingTheme()
  }

  NumberAnimation {
    id: revealAnimation
    target: root
    property: "revealProgress"
    from: 0
    to: 1
    duration: 420
    easing.type: Easing.InOutCubic
    onFinished: {
      if (root.incomingBackground) {
        root.displayedBackground = root.currentBackground || root.incomingBackground
        root.finishingTransition = true
      }
      root.revealProgress = 1
    }
  }

  Component.onCompleted: refreshBackground()

  Variants {
    model: Quickshell.screens

    PanelWindow {
      id: panel
      required property var modelData

      screen: modelData
      visible: !remapGuard.remapping
      anchors { top: true; bottom: true; left: true; right: true }

      ScreenMoveRemap {
        id: remapGuard
        window: panel
      }
      color: "transparent"
      // Keep render updates enabled. The background layer has been observed to
      // lose its committed buffer while parked with updatesEnabled=false,
      // leaving a black desktop until omarchy-shell is restarted. The wallpaper
      // itself is static, so this favors correctness over a small render-loop
      // optimization.
      updatesEnabled: true

      property bool maskReady: false

      // The grid of this output: a virtual pixel is `g.phys` device pixels.
      readonly property var g: Px.forWindow(panel)
      readonly property var monitor: root.monitorFor(modelData)

      Connections {
        target: panel.modelData
        ignoreUnknownSignals: true
        function onWidthChanged() { root.refreshDisplays() }
        function onHeightChanged() { root.refreshDisplays() }
        function onXChanged() { root.refreshDisplays() }
        function onYChanged() { root.refreshDisplays() }
        function onDevicePixelRatioChanged() { root.refreshDisplays() }
      }
      Connections {
        target: panel
        function onDevicePixelRatioChanged() { root.refreshDisplays() }
      }
      // The graticule is drawn here unless it could not be built.
      readonly property bool drawGraticule: root.pixelMode && !graticule.failed

      function maybeStartReveal() {
        if (!root.incomingBackground || root.revealProgress !== 0 || maskReady) return
        if (incomingFrame.status !== Image.Ready) return
        Qt.callLater(function() {
          if (!root.incomingBackground || root.revealProgress !== 0 || maskReady) return
          if (incomingFrame.status !== Image.Ready) return
          root.startReveal(panel)
        })
      }

      WlrLayershell.namespace: "omarchy-background"
      WlrLayershell.layer: WlrLayer.Background
      WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
      exclusionMode: ExclusionMode.Ignore

      Graticule {
        id: graticule
        anchors.fill: parent
        visible: root.pixelMode && panel.monitor !== null && settle.ready
        dpr: panel.g.dpr
        monitor: panel.monitor || { width: 1, height: 1 }
        monitors: root.monitorData.length ? root.monitorData : (panel.monitor ? [panel.monitor] : [])
        overrides: root.displayOverrides
      }

      Settle { id: settle; window: panel }

      Image {
        id: base
        anchors.fill: parent
        // The graticule's PNG is not decoded while the shader draws it.
        source: panel.drawGraticule ? "" : root.imageUrl(root.displayedBackground)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: true
        onStatusChanged: {
          if (status === Image.Ready && root.finishingTransition) {
            root.incomingBackground = ""
            root.oldBackground = ""
            root.finishingTransition = false
          }
        }
      }

      Image {
        id: oldFrame
        anchors.fill: parent
        source: root.imageUrl(root.oldBackground)
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: false
        smooth: true
        mipmap: true
        visible: root.oldBackground !== "" && root.revealProgress < 1
        onStatusChanged: panel.maybeStartReveal()
      }

      Item {
        id: incomingLayer
        anchors.fill: parent
        visible: root.incomingBackground !== "" && incomingFrame.status === Image.Ready && (root.revealProgress >= 1 || panel.maskReady)
        layer.enabled: root.incomingBackground !== "" && root.revealProgress < 1
        layer.smooth: true
        layer.effect: MultiEffect {
          maskEnabled: true
          maskSource: revealMask
          maskThresholdMin: 0.5
          maskSpreadAtMin: 0.02
        }

        Image {
          id: incomingFrame
          anchors.fill: parent
          source: root.imageUrl(root.incomingBackground)
          fillMode: Image.PreserveAspectCrop
          asynchronous: true
          cache: false
          smooth: true
          mipmap: true
          onStatusChanged: panel.maybeStartReveal()
        }
      }

      Item {
        id: revealMask
        anchors.fill: parent
        visible: false
        layer.enabled: true

        readonly property real slant: -0.18
        readonly property real centerTop: width / 2 - slant * height / 2
        readonly property real centerBottom: width / 2 + slant * height / 2
        readonly property real reach: width / 2 + Math.abs(slant) * height / 2 + 4
        readonly property real spread: reach * root.revealProgress

        Shape {
          anchors.fill: parent
          antialiasing: true
          preferredRendererType: Shape.CurveRenderer
          ShapePath {
            fillColor: "white"
            strokeColor: "transparent"
            startX: revealMask.centerTop - revealMask.spread; startY: 0
            PathLine { x: revealMask.centerTop + revealMask.spread; y: 0 }
            PathLine { x: revealMask.centerBottom + revealMask.spread; y: revealMask.height }
            PathLine { x: revealMask.centerBottom - revealMask.spread; y: revealMask.height }
            PathLine { x: revealMask.centerTop - revealMask.spread; y: 0 }
          }
        }
      }

      Connections {
        target: root
        function onIncomingBackgroundChanged() {
          panel.maskReady = false
          panel.maybeStartReveal()
        }
      }

      MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onDoubleClicked: function(mouse) {
          if (mouse.button === Qt.RightButton) root.openThemeSwitcher()
          else root.openSelector()
          mouse.accepted = true
        }
      }
    }
  }
}
