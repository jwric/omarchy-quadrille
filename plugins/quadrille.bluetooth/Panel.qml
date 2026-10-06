import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import Quickshell.Bluetooth
import Quickshell.Services.Pipewire
import qs.Ui
import qs.Commons
import "Model.js" as Model
import "Q"
import "Q/Glyphs.js" as Glyphs

Panel {
  id: root
  moduleName: "omarchy.bluetooth"
  ipcTarget: "omarchy.bluetooth"
  // manageIpc: false so this panel can own the single IpcHandler the target
  // permits — needed for the toggleBluetooth method below.
  manageIpc: false

  // Address -> "connecting" | "disconnecting" | "forgetting".
  // The actual Bluetooth sequencing lives in bin/omarchy-bluetooth-device;
  // this map only keeps the panel responsive while BlueZ catches up.
  property var pendingActions: ({})

  readonly property var adapter: Bluetooth.defaultAdapter

  // True while this instance owes BlueZ a StopDiscovery: set when it starts
  // discovery (or opens onto a session already running) and cleared once
  // discovery is confirmed down after close. Ownership, not state — BlueZ's
  // Discovering property also reflects sessions other clients hold, which are
  // never this panel's to stop.
  property bool owesDiscoveryStop: false
  readonly property var devices: Bluetooth.devices ? Bluetooth.devices.values : []
  readonly property var pipewireNodes: Pipewire.nodes ? Pipewire.nodes.values : []
  property var pendingAudioOutputDevice: null
  property int pendingAudioOutputAttempts: 0

  function deviceLabel(device) {
    return Model.deviceLabel(device)
  }

  function isUuidLike(value) {
    return Model.isUuidLike(value)
  }

  function isAddressLike(value) {
    return Model.isAddressLike(value)
  }

  function hasHumanName(device) {
    return Model.hasHumanName(device)
  }

  readonly property var deviceGroups: Model.deviceLists(devices)
  readonly property var connectedDevices: deviceGroups.connected || []
  readonly property var knownDevices: deviceGroups.known || []
  readonly property var discoveredDevices: deviceGroups.discovered || []

  readonly property string icon: {
    if (!adapter) return ""
    if (!adapter.enabled) return "󰂲"
    if (connectedDevices.length > 0) return "󰂱"
    return "󰂯"
  }

  property int phraseIndex: 0
  readonly property var activePhrases: [
    "Untangling wires",
    "Streaming vikings",
    "Pairing mysteries",
    "Herding headsets",
    "Taming radios",
    "Summoning speakers",
    "Wrangling codecs",
    "Polishing packets"
  ]
  readonly property bool rotatingPhrases: adapter && adapter.enabled
  readonly property string heroStatusText: {
    if (!adapter) return "No adapter"
    if (!adapter.enabled) return "Turned Off"
    return activePhrases[phraseIndex % activePhrases.length]
  }

  // Single cursor model shared by keyboard and mouse. Sections:
  //   "connected"  — currently connected devices; Enter disconnects.
  //   "known"      — remembered devices; Enter connects.
  //   "discovered" — unremembered devices visible while scanning; Enter connects.
  // Visuals always come from CursorSurface (hasCursor / current),
  // never from containsMouse. Mouse hover updates root cursor state too,
  // guaranteeing one highlight on screen.
  property string focusSection: "connected"
  property int selectedIndex: 0
  property bool actionFocused: false
  property bool cursorActive: false

  // Stable identity for the focused device. Devices move between sections as
  // they connect, disconnect, pair, or get forgotten, so follow the BlueZ
  // address across section changes instead of preserving a stale row index.
  property string focusedDeviceAddress: ""

  // "header" is a virtual section for the hero Bluetooth on/off toggle; it
  // sits above the device sections so the adapter can be toggled by keyboard
  // even when it is off and no device rows exist.
  readonly property bool headerHasCursor: cursorActive && focusSection === "header"
  readonly property string toggleHint: root.adapter && root.adapter.enabled ? "Turn Bluetooth off" : "Turn Bluetooth on"

  readonly property color hoverFill: bar
    ? Style.hoverFillFor(bar.foreground, Color.accent)
    : "transparent"
  readonly property color selectedFill: bar
    ? Style.selectedFillFor(bar.foreground, Color.accent)
    : "transparent"

  function sectionCount(section) {
    if (section === "connected") return connectedDevices.length
    if (section === "known") return knownDevices.length
    if (section === "discovered") return discoveredDevices.length
    return 0
  }

  function sectionVisible(section) {
    if (section === "connected") return connectedDevices.length > 0
    if (section === "known") return knownDevices.length > 0
    if (section === "discovered") return adapter && adapter.discovering && discoveredDevices.length > 0
    return false
  }

  readonly property var visibleSections: {
    return Model.visibleSections(deviceGroups, adapter && adapter.discovering)
  }

  function devicesForSection(section) {
    return Model.sectionDevices(deviceGroups, section)
  }

  // The scrollable half of the panel — remembered devices, then whatever the
  // scan turned up — flattened into one model so a ListView can own the
  // viewport. Each entry carries the section it came from, which is what lets
  // the delegate and the cursor keep working in section-relative terms.
  readonly property var scrollRows: {
    var rows = []
    for (var k = 0; k < knownDevices.length; k++)
      rows.push({ dev: Model.deviceRow(knownDevices[k]), section: "known", indexInSection: k })
    if (sectionVisible("discovered"))
      for (var d = 0; d < discoveredDevices.length; d++)
        rows.push({ dev: Model.deviceRow(discoveredDevices[d]), section: "discovered", indexInSection: d })
    return rows
  }

  // Connected devices render above the scroll area; same primitives-only
  // projection so those delegates never hold Device QObject wrappers either.
  readonly property var connectedRows: {
    var rows = []
    for (var i = 0; i < connectedDevices.length; i++)
      rows.push(Model.deviceRow(connectedDevices[i]))
    return rows
  }

  // Live BlueZ device behind a row. Rows carry primitives only, so actions
  // resolve the backend object here rather than holding a wrapper that can
  // dangle mid-incubation. `devices` is already the raw device array (see the
  // property declaration), so it is iterated directly.
  function deviceFor(row) {
    if (!row || !row.dev) return null
    var addr = row.dev.address || ""
    var devs = devices || []
    for (var i = 0; i < devs.length; i++) {
      if ((devs[i].address || "") === addr) return devs[i]
    }
    return null
  }

  // Flat position of the keyboard cursor, or -1 while it sits on the hero or
  // in the connected list (both of which live outside the scroll area).
  readonly property int scrollRowIndex: {
    if (focusSection !== "known" && focusSection !== "discovered") return -1
    for (var i = 0; i < scrollRows.length; i++)
      if (scrollRows[i].section === focusSection && scrollRows[i].indexInSection === selectedIndex) return i
    return -1
  }

  // A row opens a section when it is the first of its kind in the flat list.
  function scrollSectionTitle(index) {
    var rows = scrollRows
    if (index < 0 || index >= rows.length) return ""
    if (index > 0 && rows[index - 1].section === rows[index].section) return ""
    return rows[index].section === "known" ? "PAIRED" : "AVAILABLE"
  }

  function audioSinks() {
    var sinks = []
    for (var i = 0; i < pipewireNodes.length; i++) {
      var node = pipewireNodes[i]
      if (node && node.isSink && !node.isStream) sinks.push(node)
    }
    return sinks
  }

  function bluetoothAudioSink(device) {
    var sinks = audioSinks()
    for (var i = 0; i < sinks.length; i++) {
      if (Model.bluetoothSinkMatchesDevice(sinks[i], device)) return sinks[i]
    }
    return null
  }

  function setDefaultAudioSink(sink) {
    if (!sink) return
    Pipewire.preferredDefaultAudioSink = sink
    if (sink.id !== undefined && sink.name) {
      Quickshell.execDetached([
        "omarchy-audio-output-set-default",
        String(sink.id),
        String(sink.name)
      ])
    }
  }

  function scheduleAudioOutputSwitch(device) {
    pendingAudioOutputDevice = {
      address: device && device.address ? device.address : "",
      name: device && device.name ? device.name : "",
      deviceName: device && device.deviceName ? device.deviceName : ""
    }
    pendingAudioOutputAttempts = 0
    audioSwitchTimer.restart()
  }

  function switchPendingAudioOutput() {
    if (!pendingAudioOutputDevice) return

    var sink = bluetoothAudioSink(pendingAudioOutputDevice)
    if (sink) {
      setDefaultAudioSink(sink)
      pendingAudioOutputDevice = null
      audioSwitchTimer.stop()
      return
    }

    pendingAudioOutputAttempts += 1
    if (pendingAudioOutputAttempts >= 8) {
      pendingAudioOutputDevice = null
      return
    }
    audioSwitchTimer.restart()
  }

  function deviceAt(section, index) {
    var list = devicesForSection(section)
    return index >= 0 && index < list.length ? list[index] : null
  }

  function cloneMap(map) {
    return Model.cloneMap(map)
  }

  function pendingAction(address) {
    return Model.pendingAction(pendingActions, address)
  }

  function setPendingAction(address, action) {
    if (!address) return
    pendingActions = Model.withPendingAction(pendingActions, address, action)
    if (action) pendingTimeout.restart()
  }

  function deviceCommand(action, address) {
    return ["omarchy-bluetooth-device", action, address]
  }

  function runDeviceAction(device, action, pending) {
    if (!device || !device.address) return
    setPendingAction(device.address, pending)
    Quickshell.execDetached(deviceCommand(action, device.address))
  }

  function connectDevice(device) {
    if (!device || device.connected) return
    if (device.paired || device.bonded || device.trusted) runDeviceAction(device, "connect", "connecting")
    else runDeviceAction(device, "pair", "connecting")
  }

  function disconnectDevice(device) {
    if (!device || !device.address) return
    if (!device.connected) return
    setPendingAction(device.address, "disconnecting")
    if (device.disconnect) device.disconnect()
    Quickshell.execDetached(deviceCommand("disconnect", device.address))
  }

  function forgetDevice(device) {
    if (!device || !device.address) return
    runDeviceAction(device, "forget", "forgetting")
  }

  function syncPendingActions() {
    var next = cloneMap(pendingActions)
    var changed = false

    for (var address in next) {
      var action = next[address]
      var found = null

      for (var i = 0; i < devices.length; i++) {
        var d = devices[i]
        if (d && d.address === address) {
          found = d
          break
        }
      }

      var finishedConnecting = action === "connecting" && found && found.connected
      if (finishedConnecting
          || (action === "disconnecting" && found && !found.connected)
          || (action === "forgetting" && (!found || (!found.paired && !found.bonded && !found.trusted)))) {
        if (finishedConnecting) scheduleAudioOutputSwitch(found)
        delete next[address]
        changed = true
      }
    }

    if (changed) pendingActions = next
  }

  // j/k navigates the hero toggle ("header") and the device sections
  // row-by-row.
  function moveCursor(delta) {
    var sections = visibleSections
    if (focusSection === "header") {
      if (delta > 0 && sections && sections.length > 0) {
        focusSection = sections[0]; selectedIndex = 0; actionFocused = false
      }
      return
    }
    if (!sections || sections.length === 0) { focusSection = "header"; actionFocused = false; return }
    var sIdx = sections.indexOf(focusSection)
    if (sIdx < 0) { focusSection = sections[0]; selectedIndex = 0; actionFocused = false; return }

    var idx = selectedIndex
    var max = sectionCount(focusSection) - 1

    if (delta > 0) {
      if (idx < max) { selectedIndex = idx + 1; actionFocused = false; return }
      if (sIdx < sections.length - 1) {
        focusSection = sections[sIdx + 1]
        selectedIndex = 0
        actionFocused = false
      }
    } else {
      if (idx > 0) { selectedIndex = idx - 1; actionFocused = false; return }
      if (sIdx > 0) {
        focusSection = sections[sIdx - 1]
        selectedIndex = sectionCount(focusSection) - 1
        actionFocused = false
      } else {
        focusSection = "header"; actionFocused = false
      }
    }
  }

  function setHeaderCursor() {
    cursorActive = true
    focusSection = "header"
    actionFocused = false
  }

  function moveCursorH(delta) {
    if (!cursorActive) { cursorActive = true; return }
    if (focusSection !== "known" && focusSection !== "connected") return
    var dev = deviceAt(focusSection, selectedIndex)
    if (!dev || !dev.address) return
    if (delta > 0) actionFocused = true
    else if (delta < 0) actionFocused = false
  }

  function activateCursor() {
    if (focusSection === "header") {
      toggleBluetooth()
      return
    }
    if (actionFocused) {
      deleteSelected()
      return
    }

    if (focusSection === "connected" || focusSection === "known") {
      var dev = deviceAt(focusSection, selectedIndex)
      if (!dev) return
      if (dev.connected) disconnectDevice(dev)
      else connectDevice(dev)
      return
    }
    if (focusSection === "discovered") {
      var d = discoveredDevices[selectedIndex]
      if (!d) return
      connectDevice(d)
    }
  }

  // 'x' forgets remembered devices. For connected devices this first
  // disconnects, then removes the BlueZ pairing record via omarchy-bluetooth-device.
  function deleteSelected() {
    if (focusSection !== "known" && focusSection !== "connected") return
    var dev = deviceAt(focusSection, selectedIndex)
    if (!dev) return
    forgetDevice(dev)
  }

  onOpenedChanged: {
    if (opened) {
      // Adopt a discovery session that is already running — a popout handoff
      // from another monitor, or one leaked by an instance that could not
      // finish its own stop — so this close settles it either way.
      if (adapter !== null && adapter.discovering) owesDiscoveryStop = true
      if (connectedDevices.length > 0) { focusSection = "connected"; selectedIndex = 0 }
      else if (knownDevices.length > 0) { focusSection = "known"; selectedIndex = 0 }
      else if (discoveredDevices.length > 0) { focusSection = "discovered"; selectedIndex = 0 }
      else { focusSection = "header" }
      actionFocused = false
      cursorActive = false
    }
  }

  // Another per-monitor instance of this widget whose panel is open, if any.
  // All instances share the default adapter, and switching the popout to a
  // different monitor closes one instance as it opens the next, so the
  // closing side has to leave the scan alone for the side still on screen.
  function openSibling() {
    if (!bar || typeof bar.moduleWidgets !== "function") return null
    var items = bar.moduleWidgets(moduleName)
    for (var i = 0; i < items.length; i++) {
      if (items[i] && items[i] !== root && items[i].opened === true) return items[i]
    }
    return null
  }

  function updateFocusedAddress() {
    var d = deviceAt(focusSection, selectedIndex)
    focusedDeviceAddress = d ? (d.address || "") : ""
  }

  function reselectFocusedDevice() {
    if (focusedDeviceAddress === "") {
      clampCursor()
      return
    }

    var sections = ["connected", "known", "discovered"]
    for (var s = 0; s < sections.length; s++) {
      var section = sections[s]
      if (!sectionVisible(section)) continue
      var list = devicesForSection(section)
      for (var i = 0; i < list.length; i++) {
        if (list[i] && list[i].address === focusedDeviceAddress) {
          focusSection = section
          selectedIndex = i
          clampCursor()
          return
        }
      }
    }

    clampCursor()
  }

  onSelectedIndexChanged: updateFocusedAddress()
  onFocusSectionChanged: updateFocusedAddress()
  onConnectedDevicesChanged: { reselectFocusedDevice(); syncPendingActions() }
  onKnownDevicesChanged: { reselectFocusedDevice(); syncPendingActions() }
  onDiscoveredDevicesChanged: { reselectFocusedDevice(); syncPendingActions() }
  onVisibleSectionsChanged: clampCursor()

  function clampCursor() {
    var sections = visibleSections
    // "header" is virtual and never appears in visibleSections, so it has to
    // be let through: toggling the adapter empties and refills the device
    // lists, and clamping would knock the cursor off the hero switch every
    // time it is used.
    if (focusSection === "header") return
    if (!sections || !sections.length) {
      selectedIndex = 0
      return
    }
    if (sections.indexOf(focusSection) < 0) {
      focusSection = sections[0]
      selectedIndex = 0
      return
    }
    var count = sectionCount(focusSection)
    if (count === 0) {
      // Section emptied out — bounce to the previous visible one.
      var sIdx = sections.indexOf(focusSection)
      focusSection = sIdx > 0 ? sections[sIdx - 1] : sections[0]
      selectedIndex = Math.max(0, sectionCount(focusSection) - 1)
      return
    }
    if (selectedIndex > count - 1) selectedIndex = count - 1
    if (selectedIndex < 0) selectedIndex = 0
  }

  visible: adapter !== null
  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  // BlueZ rejects StartDiscovery while the adapter is still powering up, and
  // discovery can also time out on its own. While the panel is open, keep
  // nudging it back on so an enabled adapter is always scanning.
  Timer {
    id: discoveryRetry
    interval: 1000
    repeat: true
    triggeredOnStart: true
    running: root.opened && root.adapter !== null && root.adapter.enabled && !root.adapter.discovering
    onTriggered: {
      root.owesDiscoveryStop = true
      root.adapter.discovering = true
    }
  }

  // The way back down. The BlueZ discovery session behind adapter.discovering
  // is held by quickshell's D-Bus connection, so nothing ends it at close:
  // without this timer, one visit to the panel left the radio in inquiry
  // until the next shell restart, starving A2DP audio on the same controller
  // into stutters.
  //
  // A timer bound to the confirmed state rather than a write at close time:
  // quickshell only forwards a discovering write that differs from the last
  // state BlueZ reported, so a stop issued while a just-fired StartDiscovery
  // is still awaiting confirmation would be swallowed and leak the session.
  // Binding to adapter.discovering means a confirmation landing at any point
  // after close re-arms the stop, and a reopen inside the first interval
  // keeps the scan running uninterrupted. Attempts are bounded so a session
  // some other BlueZ client keeps up cannot draw StopDiscovery fire forever.
  Timer {
    id: discoveryStop
    interval: 1000
    repeat: true
    property int attempts: 0
    running: !root.opened && root.owesDiscoveryStop && root.adapter !== null && root.adapter.discovering === true
    onRunningChanged: if (running) attempts = 0
    onTriggered: {
      // The scan now serves the open panel, so the debt moves with it — B may
      // have opened before BlueZ confirmed A's start, in which case B's own
      // open-time adoption saw nothing to adopt.
      var sibling = root.openSibling()
      if (sibling) {
        sibling.owesDiscoveryStop = true
        root.owesDiscoveryStop = false
        return
      }
      attempts += 1
      if (attempts > 3) { root.owesDiscoveryStop = false; return }
      root.adapter.discovering = false
    }
  }

  // The debt is settled the moment BlueZ reports discovery down — whether
  // because the stop above landed or the session ended some other way — so a
  // stale claim never touches a scan another client starts later. While the
  // panel is open, discoveryRetry re-incurs it as it restarts the scan.
  Connections {
    target: root.adapter
    function onDiscoveringChanged() {
      if (!root.adapter.discovering) root.owesDiscoveryStop = false
    }
  }

  // A destroyed instance cannot wait for BlueZ confirmations, so it hands any
  // debt to a surviving sibling — whose declarative stop catches even a start
  // confirmed after this object is gone — and only writes the stop directly
  // when it is the last one standing.
  Component.onDestruction: {
    if (!owesDiscoveryStop) return
    var items = bar && typeof bar.moduleWidgets === "function" ? bar.moduleWidgets(moduleName) : []
    for (var i = 0; i < items.length; i++) {
      if (items[i] && items[i] !== root) { items[i].owesDiscoveryStop = true; return }
    }
    if (adapter !== null && adapter.discovering) adapter.discovering = false
  }

  Timer {
    id: pendingTimeout
    interval: 20000
    repeat: false
    onTriggered: root.pendingActions = ({})
  }

  Timer {
    id: audioSwitchTimer
    interval: 500
    repeat: false
    onTriggered: root.switchPendingAudioOutput()
  }

  // The phrase changes where it is: nothing fades. (Stock wrapped the swap in a
  // fade, and stopped it when the phrases stopped rotating.)
  Timer {
    id: phraseTimer
    interval: 2800
    running: root.opened && root.rotatingPhrases
    repeat: true
    onTriggered: root.phraseIndex = (root.phraseIndex + 1) % root.activePhrases.length
  }

  // Not adapter.enabled: that writes BlueZ's Powered, which nothing persists, so
  // the adapter came back on at the next boot. omarchy-bluetooth-power moves the
  // rfkill soft block instead, which systemd-rfkill restores across reboots.
  // Powered still follows the block, so the switch and icon read it as before.
  //
  // Asking for a direction rather than a toggle: the helper runs detached and the
  // switch only moves once BlueZ catches up, so a second click inside that window
  // would re-read the old state and undo the first.
  function toggleBluetooth() {
    if (!adapter) return
    Quickshell.execDetached(["omarchy-bluetooth-power", adapter.enabled ? "off" : "on"])
  }

  IpcHandler {
    target: "omarchy.bluetooth"

    function open() { root.open() }
    function close() { root.close() }
    function show() { root.open() }
    function hide() { root.close() }
    function toggle() { root.toggle() }
    function toggleBluetooth() { root.toggleBluetooth() }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.icon
    onPressed: function(b) {
      if (b === Qt.RightButton) root.toggleBluetooth()
      else root.toggle()
    }
  }


  // ---------------------------------------------------------------------------
  // The view, on the pixel grid. Everything above is the stock panel's logic.

  readonly property QtObject icons: Icons {}

  // BlueZ's icon name for each device (a freedesktop name: audio-headset,
  // input-mouse, phone...), by address. Strings only, like the rows (see
  // Model.deviceRow), so no delegate holds a Device object.
  readonly property var deviceIconNames: {
    var names = ({})
    var devs = devices || []
    for (var i = 0; i < devs.length; i++) {
      var d = devs[i]
      if (d && d.address) names[d.address] = String(d.icon || "")
    }
    return names
  }

  // The sprite for a device's kind; the rune when BlueZ does not say.
  function deviceSprite(address) {
    var name = String(deviceIconNames[address] || "")
    if (name.indexOf("audio-headset") === 0) return icons.headset
    if (name.indexOf("audio-headphones") === 0) return icons.headphones
    if (name.indexOf("audio-") === 0 || name.indexOf("multimedia-player") === 0) return icons.speaker
    if (name.indexOf("input-mouse") === 0) return icons.mouse
    if (name.indexOf("input-keyboard") === 0) return icons.keyboard
    if (name.indexOf("input-gaming") === 0) return icons.gamepad
    if (name.indexOf("phone") === 0) return icons.phone
    if (name.indexOf("watch") !== -1) return icons.watch
    if (name.indexOf("computer") === 0) return icons.computer
    if (name.indexOf("video-display") === 0) return icons.display
    if (name.indexOf("camera") === 0) return icons.camera
    return icons.bluetooth
  }

  // The scrolled list split back into its two sections, each entry as stock
  // made it: { dev, section, indexInSection }.
  readonly property var pairedRows: scrollRows.filter(function(r) { return r.section === "known" })
  readonly property var availableRows: scrollRows.filter(function(r) { return r.section === "discovered" })

  readonly property bool radioOn: !!adapter && adapter.enabled
  readonly property bool scanning: radioOn && adapter.discovering === true

  // While the panel is open the radio is kept scanning (discoveryRetry above):
  // a lamp steps on and off beside the word. Nothing tweens.
  property bool scanLamp: false
  Timer {
    interval: 500
    repeat: true
    running: root.opened && root.scanning
    onRunningChanged: root.scanLamp = running
    onTriggered: root.scanLamp = !root.scanLamp
  }

  // A cursor moved by j/k scrolls its row into view; one moved by the pointer
  // does not, so the list never moves under a resting hand.
  property bool cursorFromKeys: false
  function ensureRowVisible(item) {
    if (item && scroll) scroll.ensureItemVisible(item)
  }

  // What a device row says. The status is stock's (its DeviceRow.statusText):
  // the battery goes to the reading at the right, the rest under the name.
  function rowStatus(dev, sectionName, isDiscovered) {
    if (!dev) return ""
    var action = pendingAction(dev.address || "")
    var devState = dev.state !== undefined ? dev.state : -1
    if (action === "forgetting") return "Forgetting…"
    if (action === "disconnecting" || devState === 2) return "Disconnecting…"
    if (dev.connected) {
      if (dev.batteryAvailable) return Math.round(dev.battery * 100) + "%"
      return sectionName === "connected" ? "" : "Connected"
    }
    if (action === "connecting" || devState === 3 || dev.pairing === true) return "Connecting…"
    if (isDiscovered) return ""
    return ""
  }
  function rowBattery(dev) {
    return dev && dev.connected && dev.batteryAvailable ? Math.round(dev.battery * 100) + "%" : ""
  }
  function rowSub(dev, sectionName, isDiscovered) {
    var status = rowStatus(dev, sectionName, isDiscovered)
    return status !== rowBattery(dev) ? status : ""
  }

  // A row's height in vpx, worked out the way QRow lays it out (a line per line
  // of the name, wrapped to two, and one for the status; a vpx of air above and
  // below), from the data alone.
  readonly property int rowActionsVpx: 2 * 17 + 2
  function rowVpx(dev, sectionName, isDiscovered) {
    var rowWidth = Math.floor(panel.innerWidth / panel.g.unit + 1e-6) - 4   // less the scroll gutter
    var battery = rowBattery(dev)
    var detail = battery.length > 0 ? (Glyphs.length(battery) + 1) * 6 : 0
    var text = Math.max(0, rowWidth - 2 * 3 - (7 + 3) - detail - (rowActionsVpx + 2))
    var lines = Glyphs.wrap(deviceLabel(dev) || "Device", Math.max(1, Math.floor(text / 6)), 2).length
    var sub = rowSub(dev, sectionName, isDiscovered).length > 0 ? 1 : 0
    return (lines + sub) * 12 + 2
  }
  function rowsVpx(rows, sectionName, isDiscovered) {
    var h = 0
    for (var i = 0; i < rows.length; i++) {
      var dev = rows[i] && rows[i].dev !== undefined ? rows[i].dev : rows[i]
      h += rowVpx(dev, sectionName, isDiscovered) + (i > 0 ? 2 : 0)
    }
    return h
  }

  // How tall the list is. While the radio is on, a scan's finds arrive after the
  // first frame, so the AVAILABLE group has room for four rows from the start and
  // what is more scrolls. It is worked out from the data (a Column gives its
  // height only once its window is on screen) and held while the panel is open,
  // so the card does not change size under the eye; switching the radio settles
  // it again. In vpx, so it holds whatever the grid.
  readonly property int liveListVpx: {
    var header = 12 + 4          // a Group's name line and its gap
    var empty = 14               // a QEmpty row
    var parts = []
    if (!adapter) parts.push(empty)
    if (connectedRows.length > 0) parts.push(header + rowsVpx(connectedRows, "connected", false))
    if (adapter && (pairedRows.length > 0 || connectedRows.length === 0))
      parts.push(header + (pairedRows.length > 0 ? rowsVpx(pairedRows, "known", false) : empty))
    if (adapter) parts.push(header + (radioOn ? 4 * 14 + 3 * 2 : empty))
    var h = 0
    for (var p = 0; p < parts.length; p++) h += parts[p]
    return Math.max(14, h + 6 * Math.max(0, parts.length - 1))
  }
  property int heldListVpx: -1
  readonly property int listVpx: heldListVpx >= 0 ? heldListVpx : liveListVpx
  Connections {
    target: root
    function onOpenedChanged() { root.heldListVpx = root.opened ? root.liveListVpx : -1 }
    function onRadioOnChanged() { if (root.opened) root.heldListVpx = root.liveListVpx }
    function onAdapterChanged() { if (root.opened) root.heldListVpx = root.liveListVpx }
  }

  QPopup {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(panel.cardWidth(44))
    contentHeight: panel.fittedContentHeight(hero.height + column.spacing + scroll.height)

    onOpenChanged: if (open) scroll.reset()

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      onMoveRequested: function(dx, dy) {
        root.cursorFromKeys = true
        if (!root.cursorActive) { root.cursorActive = true; return }
        if (dy !== 0) root.moveCursor(dy)
        else if (dx !== 0) root.moveCursorH(dx)
      }
      onActivateRequested: if (root.cursorActive) root.activateCursor()
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onDeleteRequested: if (root.cursorActive) root.deleteSelected()
      onTextKey: function(t) {
        if (t === "b" || t === "B") root.toggleBluetooth()
      }

      // Hero over list, placed by hand rather than by a Column: a positioner gives
      // nothing to children it cannot see, and nothing is seen while the window is
      // hidden, so its height would be decided only once the popup is on screen.
      Item {
        id: column
        width: parent.width
        readonly property real spacing: panel.g.px(6)
        height: hero.height + spacing + scroll.height

        // ---------- Hero: rune · title/status · scanning · switch ----------
        // Two lines tall. Status only; the switch owns the radio, mouse and
        // keyboard alike. The rune is a 15 x 15 drawing at the surface's own
        // pixel, beside the two lines (QHero would double a 7 x 7 one).
        Item {
          id: hero
          z: 2   // the switch's tip hangs over the list
          width: parent.width - scroll.gutter
          height: 2 * panel.g.line

          readonly property real textX: panel.g.px(15 + 4)
          readonly property real trailSpace: heroTrail.width > 0 ? heroTrail.width + panel.g.px(3) : 0
          readonly property int columns: panel.g.columns(width - textX - trailSpace)

          Sprite {
            y: panel.g.centre(hero.height, height)
            rows: root.connectedDevices.length > 0 && root.radioOn ? root.icons.bluetoothLinked15 : root.icons.bluetooth15
            color: root.radioOn ? Role.ink : Role.muted
            mid: root.radioOn ? Role.muted : Role.faint
          }

          Column {
            x: hero.textX
            PixelText { text: "Bluetooth"; ink: Role.ink; columns: hero.columns }
            PixelText { text: root.heroStatusText.toUpperCase(); ink: Role.muted; columns: hero.columns }
          }

          Row {
            id: heroTrail
            anchors.right: parent.right
            height: hero.height
            spacing: panel.g.px(4)

            Item {
              visible: root.radioOn
              width: panel.g.px(6 + 3) + scanWord.width
              height: parent.height
              Lamp {
                visible: root.scanning
                y: panel.g.centre(hero.height, panel.g.line) + panel.g.onCaps(6)
                on: root.scanLamp
                tone: Role.live
              }
              PixelText {
                id: scanWord
                visible: root.scanning
                x: panel.g.px(6 + 3)
                y: panel.g.centre(hero.height, height)
                text: "SCANNING"
                ink: Role.muted
              }
            }

            QSwitch {
              id: powerSwitch
              visible: !!root.adapter
              y: panel.g.centre(hero.height, height)
              checked: root.radioOn
              hasCursor: root.headerHasCursor
              onHovered: function(on) {
                if (!on) return
                root.cursorFromKeys = false
                root.setHeaderCursor()
              }
              onToggled: root.toggleBluetooth()

              QTip {
                text: root.toggleHint
                shown: powerSwitch.containsMouse
                x: powerSwitch.width - width
              }
            }
          }
        }

        // ---------- Devices: connected, paired, available ----------
        QScroll {
          id: scroll
          y: hero.height + column.spacing
          width: parent.width
          readonly property real cap: panel.g.floor(Math.min(panel.g.px(280),
            panel.availableCardHeight - 2 * panel.inset - hero.height - column.spacing))
          height: Math.min(panel.g.px(root.listVpx), Math.max(panel.g.px(14), cap))
          contentHeight: listColumn.implicitHeight

          Column {
            id: listColumn
            width: parent.width
            spacing: panel.g.px(6)

            QEmpty {
              visible: !root.adapter
              width: parent.width
              icon: root.icons.bluetooth
              text: "No Bluetooth adapter"
            }

            Group {
              name: "CONNECTED"
              visible: root.connectedRows.length > 0
              width: parent.width
              height: visible ? implicitHeight : 0

              Column {
                width: parent.width
                spacing: panel.g.px(2)
                Repeater {
                  model: root.connectedRows
                  DeviceRow {
                    required property var modelData
                    required property int index
                    width: parent.width
                    dev: modelData
                    rowIndex: index
                    sectionName: "connected"
                    isDiscovered: false
                  }
                }
              }
            }

            Group {
              name: "PAIRED"
              visible: !!root.adapter && (root.pairedRows.length > 0 || root.connectedRows.length === 0)
              width: parent.width
              height: visible ? implicitHeight : 0

              Column {
                width: parent.width
                spacing: panel.g.px(2)
                Repeater {
                  model: root.pairedRows
                  DeviceRow {
                    required property var modelData
                    width: parent.width
                    dev: modelData.dev
                    rowIndex: modelData.indexInSection
                    sectionName: modelData.section
                    isDiscovered: false
                  }
                }
                QEmpty {
                  visible: root.pairedRows.length === 0
                  width: parent.width
                  icon: root.icons.bluetooth
                  text: "No paired devices"
                }
              }
            }

            Group {
              name: "AVAILABLE"
              visible: !!root.adapter
              width: parent.width
              height: visible ? implicitHeight : 0

              Column {
                width: parent.width
                spacing: panel.g.px(2)
                Repeater {
                  model: root.availableRows
                  DeviceRow {
                    required property var modelData
                    width: parent.width
                    dev: modelData.dev
                    rowIndex: modelData.indexInSection
                    sectionName: modelData.section
                    isDiscovered: true
                  }
                }
                QEmpty {
                  visible: root.availableRows.length === 0
                  width: parent.width
                  icon: root.radioOn ? Pictograms.search : root.icons.bluetooth
                  text: root.radioOn ? "No devices found" : "Turn Bluetooth on to scan"
                }
              }
            }
          }
        }
      }
    }
  }

  // A device: its kind, its name (two lines, then an ellipsis), the battery as a
  // reading, and what is under way on a second line. The connected one is the
  // inverse block. Pending state is owned by the panel so it survives rows
  // moving between sections.
  //
  // The row under the cursor shows its actions at the right: connect or
  // disconnect (or pair), and forget. Their room is kept on every row, so a
  // name does not re-wrap when they appear. j/k move between rows, l/h onto and
  // off forget, as stock.
  component DeviceRow: QRow {
    id: row
    required property var dev
    required property int rowIndex
    required property string sectionName
    required property bool isDiscovered

    readonly property bool isConnected: dev && dev.connected
    readonly property string actionTooltip: {
      if (!dev) return ""
      if (isConnected) return "Disconnect"
      if (isDiscovered) return "Pair"
      return "Connect"
    }

    readonly property bool rowSelected: root.cursorActive && root.focusSection === sectionName && root.selectedIndex === rowIndex
    readonly property bool forgetAvailable: (sectionName === "known" || sectionName === "connected") && !isDiscovered
    readonly property bool showActions: rowSelected || hot

    readonly property string batteryText: root.rowBattery(dev)

    // Which action the pointer is on: "" | "primary" | "forget".
    property string pointedAction: ""

    icon: root.deviceSprite(dev ? dev.address : "")
    text: root.deviceLabel(dev) || "Device"
    sub: root.rowSub(dev, sectionName, isDiscovered)
    detail: batteryText
    current: isConnected
    dimmed: !root.radioOn
    hasCursor: rowSelected && !root.actionFocused

    onRowSelectedChanged: if (rowSelected && root.cursorFromKeys) root.ensureRowVisible(row)

    onHovered: {
      root.cursorFromKeys = false
      root.cursorActive = true
      root.focusSection = row.sectionName
      root.selectedIndex = row.rowIndex
      root.actionFocused = false
    }

    onClicked: function(button) {
      var dev = root.deviceFor(row)
      if (!dev) return
      if (button === Qt.RightButton) {
        if (row.isConnected) root.disconnectDevice(dev)
        else if (!row.isDiscovered) root.forgetDevice(dev)
        return
      }
      if (row.isConnected) root.disconnectDevice(dev)
      else root.connectDevice(dev)
    }

    // The actions: two slots, kept whether shown or not, and two vpx of air
    // at the right so the row's own brackets do not touch the last one. On the
    // first line, beside the reading. As tall as the row, so that QRow, which
    // centres what it is given, has nothing to centre: an anchor rounds a centre
    // to a whole logical pixel, which is off the grid at 1.8 a vpx.
    Item {
      id: actions
      width: row.g.px(root.rowActionsVpx)
      height: row.height

      RowAction {
        visible: row.showActions
        icon: row.isConnected ? root.icons.unlink : (row.isDiscovered ? root.icons.plus : root.icons.link)
        onInverse: row.current
        onHovered: function(isHovered) {
          if (isHovered) {
            row.pointedAction = "primary"
            root.cursorFromKeys = false
            root.cursorActive = true
            root.focusSection = row.sectionName
            root.selectedIndex = row.rowIndex
            root.actionFocused = false
          } else if (row.pointedAction === "primary") {
            row.pointedAction = ""
          }
        }
        onClicked: {
          var dev = root.deviceFor(row)
          if (!dev) return
          if (row.isConnected) root.disconnectDevice(dev)
          else root.connectDevice(dev)
        }
      }

      RowAction {
        x: row.g.px(17)
        visible: row.showActions && row.forgetAvailable
        icon: root.icons.trash
        danger: true
        onInverse: row.current
        hasCursor: row.rowSelected && root.actionFocused
        onHovered: function(isHovered) {
          if (!isHovered) {
            if (row.pointedAction === "forget") row.pointedAction = ""
            if (row.hot) root.actionFocused = false
            return
          }
          row.pointedAction = "forget"
          root.cursorFromKeys = false
          root.cursorActive = true
          root.focusSection = row.sectionName
          root.selectedIndex = row.rowIndex
          root.actionFocused = true
        }
        onClicked: {
          var dev = root.deviceFor(row)
          if (!dev) return
          root.forgetDevice(dev)
        }
      }

      // What the pointed action does, to the left of the actions, inside the row.
      QTip {
        text: row.pointedAction === "forget" ? "Forget" : (row.pointedAction === "primary" ? row.actionTooltip : "")
        shown: row.showActions && row.pointedAction !== ""
        x: -width - row.g.px(2)
        y: row.g.centre(row.g.px(14), height)
      }
    }
  }

  // A small icon button in a row, with its keyboard brackets a virtual pixel
  // outside its box (on the inverse block they are ink, as QRow's are).
  component RowAction: Item {
    id: act
    readonly property var g: Px.of(act)
    property var icon: null
    property bool danger: false
    property bool onInverse: false
    property bool hasCursor: false
    signal clicked()
    signal hovered(bool isHovered)

    width: g.px(17)
    height: g.px(14)

    QButton {
      x: act.g.px(1)
      y: act.g.px(1)
      icon: act.icon
      danger: act.danger
      onClicked: act.clicked()
      onHovered: function(isHovered) { act.hovered(isHovered) }
    }

    Loader {
      anchors.fill: parent
      active: act.hasCursor
      sourceComponent: Brackets { arm: 3; color: act.onInverse ? Role.ink : Role.accent }
    }
  }
}
