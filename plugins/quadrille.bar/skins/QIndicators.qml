import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui
import "../Q"

// Replaces omarchy.indicators: dictation, screen recording, reminders, night
// light, do-not-disturb and stay-awake as 7 x 7 pixel sprites, lit in a tone when
// on and faint when off. Like the stock widget, only the lit ones show until the
// pointer is over the centre of the bar, which reveals the rest.
//
// The sources are the stock indicators' own, run the same way: the notification,
// night-light and idle services through the bar's proxies, `omarchy-voxtype-status`,
// `omarchy-reminder show --json`, and `pgrep` for the screen recorder, refreshed
// by the same `omarchy.indicators` IPC `refresh`. The widget's `items` and
// `alwaysShow` settings are honoured (ids as in the stock list: Dictation,
// ScreenRecording, Reminder, NightLight, Dnd, StayAwake).
BarWidget {
  id: root
  moduleName: "omarchy.indicators"

  readonly property var g: Px.of(root)

  // The bar offers each widget the room it has left; this one does not give way.
  property real room: 1e9
  readonly property bool elastic: false

  readonly property var defaultItems: ["Dictation", "ScreenRecording", "Reminder", "NightLight", "Dnd", "StayAwake"]
  readonly property var items: {
    var source = defaultItems
    if (settings && settings.items && typeof settings.items.length === "number" && settings.items.length > 0) source = settings.items
    else if (settings && settings.indicators && typeof settings.indicators.length === "number" && settings.indicators.length > 0) source = settings.indicators
    var out = []
    for (var i = 0; i < source.length; i++) {
      var id = typeof source[i] === "string" ? source[i] : (source[i] && source[i].id ? String(source[i].id) : "")
      if (id !== "" && out.indexOf(id) === -1) out.push(id)
    }
    return out
  }

  readonly property bool alwaysShow: setting("alwaysShow", false) === true
  property bool areaHovered: false
  readonly property bool reveal: alwaysShow || areaHovered
    || (bar && bar.centerSectionRevealHeld === true && bar.centerHoverRevealSuppressed !== true)

  // ---- state
  readonly property var notifications: bar?.shell?.firstPartyServiceFor("omarchy.notifications")
  readonly property var nightlight: bar?.shell?.firstPartyServiceFor("omarchy.nightlight")
  readonly property var idle: bar?.shell?.firstPartyServiceFor("omarchy.idle")
  property bool recording: false
  property int reminders: 0
  property string reminderTip: ""
  property string dictation: "idle"

  readonly property var active: ({
    Dnd: notifications ? notifications.doNotDisturb === true : false,
    NightLight: nightlight ? nightlight.enabled === true : false,
    StayAwake: idle ? idle.stayAwake === true : false,
    ScreenRecording: recording,
    Reminder: reminders > 0,
    Dictation: dictation === "recording" || dictation === "transcribing"
  })

  // What each one is: its sprite, the tone it is lit in, its tooltip, its click.
  function spec(id) {
    var on = active[id] === true
    switch (id) {
      case "Dictation":
        return { rows: dictation === "transcribing" ? Sprites.hourglass : Sprites.microphone, tone: dictation === "transcribing" ? Role.caution : Role.alarm,
                 tip: on ? dictation : "Dictate", press: function() { bar.run("omarchy-voxtype-config") } }
      case "ScreenRecording":
        return { rows: Sprites.record, tone: Role.alarm, tip: on ? "Stop recording" : "Screen Recording",
                 press: function() { bar.run(recording ? "omarchy-capture-screenrecording --stop-recording" : "omarchy-menu toggle trigger.capture.screenrecord") } }
      case "Reminder":
        return { rows: Sprites.bell, tone: Role.caution, tip: reminderTip,
                 press: function() { Quickshell.execDetached(reminders > 0 ? ["omarchy-reminder", "show"] : ["omarchy-reminder", "-i"]) } }
      case "NightLight":
        return { rows: Sprites.moon, tone: Role.caution, tip: on ? "Day Light" : "Night Light",
                 press: function() { if (nightlight) nightlight.setNightlight(!on) } }
      case "Dnd":
        return { rows: Sprites.bellOff, tone: Role.accent, tip: on ? "Allow Notifications" : "Silence Notifications",
                 press: function() { if (notifications) notifications.setDoNotDisturb(!notifications.doNotDisturb) } }
      case "StayAwake":
        return { rows: Sprites.coffee, tone: Role.accent, tip: on ? "Allow Idle Lock & Screensaver" : "Stay Awake",
                 press: function() { if (idle) idle.setIdleEnabled(on) } }
    }
    return { rows: Sprites.warning, tone: Role.faint, tip: id, press: function() { } }
  }

  // the ones to draw: lit always, unlit only on reveal; the stock widget puts the
  // unlit block to the left of the lit one, so newcomers land by the clock
  readonly property var shown: {
    var lit = [], unlit = []
    for (var i = 0; i < items.length; i++) (active[items[i]] === true ? lit : unlit).push(items[i])
    return (reveal ? unlit : []).concat(lit)
  }

  function refresh() {
    if (!statusProc.running) { statusProc.command = ["pgrep", "--quiet", "-f", "^gpu-screen-recorder"]; statusProc.running = true }
    if (!reminderProc.running) reminderProc.running = true
  }
  Component.onCompleted: refresh()

  IpcHandler {
    target: "omarchy.indicators"
    function refresh(): void { root.broadcast("refresh") }
  }

  Process {
    id: statusProc
    onExited: function(code) { root.recording = code === 0 }
  }
  Process {
    id: reminderProc
    command: ["omarchy-reminder", "show", "--json"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: {
        try { var d = JSON.parse(text); root.reminders = Number(d.count || 0); root.reminderTip = String(d.tooltip || "") }
        catch (e) { root.reminders = 0; root.reminderTip = "" }
      }
    }
    onExited: function(code) { if (code !== 0) { root.reminders = 0; root.reminderTip = "" } }
  }
  Process {
    id: dictationProc
    command: ["bash", "-c", "omarchy-voxtype-status"]
    running: root.items.indexOf("Dictation") !== -1
    stdout: SplitParser {
      onRead: function(line) {
        try { var d = JSON.parse(line); root.dictation = String(d.alt || d["class"] || "idle") }
        catch (e) { root.dictation = "idle" }
      }
    }
  }

  // ---- layout: 11 vpx a sprite (7 and 2 of air a side)
  readonly property real cellW: g.px(11)
  implicitWidth: vertical ? barSize : shown.length * cellW
  implicitHeight: vertical ? shown.length * cellW : barSize

  HoverHandler { onHoveredChanged: root.areaHovered = hovered }

  Grid {
    columns: root.vertical ? 1 : 1000
    y: root.vertical ? 0 : root.g.px(2)
    x: root.vertical ? root.g.centre(root.width, root.cellW) : 0
    Repeater {
      model: root.shown
      delegate: BarButton {
        id: cell
        required property string modelData
        readonly property var info: root.spec(modelData)
        readonly property bool on: root.active[modelData] === true
        bar: root.bar
        width: root.cellW
        height: root.g.px(12)
        tooltipText: info.tip
        onPressed: function() { info.press() }

        Rectangle {
          anchors.fill: parent; antialiasing: false
          color: cell.down ? Role.hover : (cell.hovered ? Role.raised : "transparent")
        }
        Sprite {
          x: root.g.px(2); y: root.g.centre(parent.height, height)
          rows: cell.info.rows
          level: cell.on ? 9 : 0
          color: cell.on ? cell.info.tone : (cell.hovered ? Role.muted : Role.faint)
          dim: Role.edge
        }
      }
    }
  }
}
