import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Networking
import qs.Ui
import qs.Commons
import "Model.js" as Model
import "Q"
import "Q/Glyphs.js" as Glyphs

Panel {
  id: root
  moduleName: "omarchy.network"
  ipcTarget: "omarchy.network"
  // manageIpc: false so this panel can own the single IpcHandler the target
  // permits — needed for the toggleNetwork method below.
  manageIpc: false

  // Centralized close so callers can't forget to drop the passphrase prompt.
  function close() {
    root.controller.hide()
    cancelPasswordPrompt()
  }

  function cancelPasswordPrompt() {
    passwordSsid = ""
    passwordText = ""
    identityText = ""
  }

  // Live connection details from `ip` / /sys / iw.
  property var info: ({})  // { iface, type, ip, prefix, gateway, speed, duplex, ssid, signal, freq, bitrate, rx_bytes, tx_bytes, router_ping_ms, internet_ping_ms }

  // Throughput tracking. Rates are computed as deltas between successive
  // `omarchy-network-status --verbose` samples (~1.5s apart via detailsPoll).
  // We hold "prev" alongside a timestamp so the first sample after open or
  // after an interface switch doesn't manufacture a spike.
  property real prevRxBytes: 0
  property real prevTxBytes: 0
  property real prevSampleTime: 0
  property string prevIface: ""
  property real downloadRate: 0  // bytes/sec
  property real uploadRate: 0    // bytes/sec
  property string pingIface: ""
  property var routerPingSamples: []
  property var internetPingSamples: []
  property real routerPingLatency: -1
  property real internetPingLatency: -1
  property int internetPingPacketLoss: 0
  readonly property int pingHistoryWindow: 24
  readonly property int pingAverageWindow: 5
  readonly property bool hasInternetPing: internetPingSamples.length > 0
  // Every stat row stays mounted whether or not there is data behind it, so a
  // sample arriving late never reflows the grid. This says whether the numbers
  // are real yet or the row should read "--".
  readonly property bool hasTransferStats: info.rx_bytes !== undefined
  property int connectionPhraseIndex: 0
  readonly property var connectionPhrases: [
    "Wiring bits",
    "Handling packets",
    "Sorting frames",
    "Hauling bytes",
    "Routing crumbs",
    "Counting collisions",
    "Bending light",
  ]
  readonly property string connectionPhrase: connectionPhrases[connectionPhraseIndex % connectionPhrases.length]
  readonly property bool networkManagerAvailable: Networking.backend === NetworkBackendType.NetworkManager
  readonly property var networkDevices: Networking.devices ? Networking.devices.values : []
  readonly property var wifiDevice: findDevice(DeviceType.Wifi)
  readonly property var wifiNetworkObjects: wifiDevice && wifiDevice.networks ? wifiDevice.networks.values : []
  readonly property var connectedWifiNetwork: findConnectedWifiNetwork()
  property var wifiNetworks: []
  property bool scanning: false
  property bool wifiStationAvailable: false
  property string dnsProvider: ""
  property string pendingDnsProvider: ""
  // Wi-Fi band state from `omarchy-network-band`. `bandCurrent` is the band
  // the radio is actually on; `bandSelected` is the pinned choice ("auto" when
  // nothing is pinned), and the two differ whenever Auto is in effect.
  property string bandCurrent: ""
  property string bandSelected: "auto"
  property var bandAvailable: []
  property string pendingBand: ""

  // Per-row in-flight state. `actionSsid` flips on for the row whose action
  // is currently running so it can render "Connecting…" / "Disconnecting…" /
  // "Forgetting…". `passwordSsid` is the row currently expanded into
  // password-entry mode; we keep it open across refresh cycles so a slow scan
  // doesn't collapse the input the user is typing into. Rows must gate
  // comparisons on the matching `*Kind`/`*Reason` being non-empty so a
  // hidden-SSID row (ssid == "") doesn't collide with the "" defaults.
  property string actionSsid: ""
  property string actionKind: ""  // "connect" | "disconnect" | "forget"
  property string failureSsid: ""
  property string failureReason: ""
  property string passwordSsid: ""
  property string passwordText: ""
  property string identityText: ""

  // ConnectionFailReason values as a plain object, so Model.js helpers stay
  // pure JS and Node-testable.
  readonly property var connectionFailReasons: ({
    NoSecrets: ConnectionFailReason.NoSecrets,
    WifiAuthTimeout: ConnectionFailReason.WifiAuthTimeout,
    WifiNetworkLost: ConnectionFailReason.WifiNetworkLost,
    WifiClientDisconnected: ConnectionFailReason.WifiClientDisconnected,
    WifiClientFailed: ConnectionFailReason.WifiClientFailed
  })

  // True while any wifi action is mid-flight. Rows
  // disable themselves on this so clicks on the other rows don't silently
  // no-op against runNetworkAction's serialized guard.
  readonly property bool busy: actionKind !== ""

  // Index into `wifiNetworks` for keyboard navigation. -1 = no selection.
  property int selectedIndex: -1
  property bool wifiActionFocused: false
  property bool cursorActive: false

  // Keyboard focus zone for the panel. j/k crosses row boundaries:
  // header actions ⇄ band ⇄ DNS row ⇄ Wi-Fi networks. h/l move
  // within header actions, band pills, or DNS providers.
  property string focusSection: "dns"  // "header" | "band" | "dns" | "wifi"
  property int headerIndex: 0
  readonly property bool canDisconnect: !!connectedWifiNetwork
  readonly property bool headerHasDisconnect: false
  readonly property bool canShareWifi: info.type === "wifi" && canShareNetwork(connectedWifiNetwork)
  // The hero switch is the Wi-Fi radio, so it only exists when there is a
  // radio to switch. On a wired box it would otherwise sit there reading
  // "off" beside a perfectly live Ethernet connection.
  readonly property bool canToggleWifi: networkManagerAvailable && wifiStationAvailable
  readonly property int qrHeaderIndex: canShareWifi ? 0 : -1
  readonly property int speedHeaderIndex: canRunSpeedTest ? (canShareWifi ? 1 : 0) : -1
  readonly property int toggleHeaderIndex: canToggleWifi ? (canShareWifi ? 1 : 0) + (canRunSpeedTest ? 1 : 0) : -1
  readonly property int headerActionCount: (canShareWifi ? 1 : 0) + (canRunSpeedTest ? 1 : 0) + (canToggleWifi ? 1 : 0)
  readonly property bool qrHeaderHasCursor: cursorActive && focusSection === "header" && headerIndex === qrHeaderIndex
  readonly property bool speedHeaderHasCursor: cursorActive && focusSection === "header" && headerIndex === speedHeaderIndex
  readonly property bool toggleHeaderHasCursor: cursorActive && focusSection === "header" && headerIndex === toggleHeaderIndex
  readonly property string toggleHint: Networking.wifiEnabled ? "Turn Wi-Fi off" : "Turn Wi-Fi on"
  readonly property var dnsProviders: ["DHCP", "Cloudflare", "Google", "Custom"]
  property int dnsIndex: 0
  // ["2.4", "5", ...], or empty when there is nothing to choose between.
  // Wi-Fi only: on Ethernet the band of a secondary radio is not what the
  // panel is describing.
  // `bandBusy` keeps the section mounted across the reconnect a band change
  // causes: `kind` stops being "wifi" for a second or two in the middle of it,
  // and without this the whole segment would vanish and rebuild itself.
  // Worth showing when there is a real choice, or when a pin is in force even
  // though only one band answers right now -- otherwise the Automatic switch
  // vanishes and the pin becomes unclearable from the panel.
  readonly property bool canSelectBand: (kind === "wifi" || bandBusy)
    && (bandAvailable.length > 1 || bandPinned)
  // While a change is in flight, show the state that was asked for rather than
  // the one still in force, so the row answers the click immediately instead of
  // after the reconnect. actionProc puts it back if the change failed.
  readonly property string bandEffective: pendingBand !== "" ? pendingBand : bandSelected
  readonly property bool bandPinned: bandEffective !== "auto"
  // Under Automatic there is nothing to pick, so the pills collapse away and
  // the header states the live band instead.
  readonly property bool bandPillsVisible: canSelectBand && bandPinned
  readonly property string bandSectionTitle: Model.bandSectionTitle(bandEffective, bandCurrent)
  readonly property bool bandBusy: pendingBand !== ""
  // The speed test needs an interface to test, so its hero action only
  // appears once there is one.
  readonly property bool canRunSpeedTest: !!info.iface
  property int bandIndex: 0
  // The band section has up to two cursor rows: the Automatic switch on the
  // header line, then the pills. Same shape as wifiActionFocused.
  property bool bandAutoFocused: true

  onHeaderActionCountChanged: clampHeaderIndex()

  // Availability shifts as scans land, so the option list can shrink out from
  // under the cursor. Clamp the index and evacuate the section before it
  // disappears, or the panel is left highlighting nothing.
  onBandAvailableChanged: {
    if (bandIndex > bandAvailable.length - 1) bandIndex = Math.max(0, bandAvailable.length - 1)
  }

  onCanSelectBandChanged: {
    if (!canSelectBand && focusSection === "band") {
      focusSection = "dns"
      bandAutoFocused = true
    }
  }

  // Collapsing the pills out from under the cursor would leave it pointing at
  // nothing, so send it up to the switch that is still on screen.
  onBandPillsVisibleChanged: {
    if (!bandPillsVisible) bandAutoFocused = true
  }

  function clampHeaderIndex() {
    var max = Math.max(0, headerActionCount - 1)
    if (headerIndex > max) headerIndex = max
    if (headerIndex < 0) headerIndex = 0
  }

  function selectHeaderByDelta(delta) {
    headerIndex = Math.max(0, Math.min(headerActionCount - 1, headerIndex + delta))
  }

  function toggleNetwork() {
    if (!networkManagerAvailable) return
    Networking.wifiEnabled = !Networking.wifiEnabled
    Qt.callLater(function() { root.refresh(true) })
  }

  IpcHandler {
    target: "omarchy.network"

    function open() { root.open() }
    function close() { root.close() }
    function show() { root.open() }
    function hide() { root.close() }
    function toggle() { root.toggle() }
    function toggleNetwork() { root.toggleNetwork() }
    // Compat routes for configs that summon the centered cards through the
    // network target; both cards are their own plugins now.
    function showQr() { root.summonWifiQr(true) }
    function speedTest() { root.summonSpeedTest() }
  }

  function activateHeader() {
    if (headerIndex === qrHeaderIndex) summonWifiQr()
    else if (headerIndex === speedHeaderIndex) summonSpeedTest()
    else if (headerIndex === toggleHeaderIndex) toggleNetwork()
  }

  function setHeaderCursor(index) {
    cursorActive = true
    focusSection = "header"
    headerIndex = index
  }

  function selectDnsByDelta(delta) {
    dnsIndex = Math.max(0, Math.min(dnsProviders.length - 1, dnsIndex + delta))
  }

  function activateDns() {
    if (dnsIndex < 0 || dnsIndex >= dnsProviders.length) return
    setDns(dnsProviders[dnsIndex])
  }

  function selectBandByDelta(delta) {
    bandIndex = Math.max(0, Math.min(bandAvailable.length - 1, bandIndex + delta))
  }

  function activateBand() {
    if (bandAutoFocused) {
      toggleBandAuto()
      return
    }
    if (bandIndex < 0 || bandIndex >= bandAvailable.length) return
    setBand(bandAvailable[bandIndex])
  }

  // Switching Automatic off has to commit to something, so it pins whatever
  // band the radio already landed on -- the reading the pills are showing.
  function toggleBandAuto() {
    if (bandSelected !== "auto") {
      setBand("auto")
      return
    }
    if (bandCurrent === "") return
    setBand(bandCurrent)
  }

  // Park the cursor on the pinned band, so opening the panel highlights the
  // pill the user would expect. Under Automatic there are no pills, so the
  // cursor belongs on the switch.
  function syncBandIndex() {
    var idx = bandAvailable.indexOf(bandSelected)
    bandIndex = idx >= 0 ? idx : 0
    bandAutoFocused = !bandPillsVisible
  }

  function bandLabel(band) {
    return Model.bandLabel(band)
  }

  function bandTooltip(band) {
    return Model.bandTooltip(band)
  }

  // Single cursor model: exactly one highlighted spot across the whole
  // panel, located via `focusSection` + (`headerIndex` | `dnsIndex` |
  // `selectedIndex`). Mouse hover and keyboard nav both mutate this state
  // at the root; items never read containsMouse for visuals. See
  // CursorSurface for the shared chrome shared by rows and pills.
  readonly property color hoverFill: bar ? Style.hoverFillFor(bar.foreground, Color.accent) : "transparent"
  readonly property color selectedFill: bar ? Style.selectedFillFor(bar.foreground, Color.accent) : "transparent"

  // scannerEnabled lives on the shared WifiDevice, which has no reference
  // counting, and a bar widget is instantiated once per monitor. Tracking the
  // device this instance turned scanning on for keeps the release correct when
  // the panel closes, the device is replaced, or the widget is destroyed —
  // without a closed instance ever claiming the scanner.
  property var scannerDevice: null

  function setScannerEnabled(enabled) {
    var nextDevice = opened ? wifiDevice : null

    if (scannerDevice && scannerDevice !== nextDevice)
      scannerDevice.scannerEnabled = false

    scannerDevice = nextDevice

    if (scannerDevice)
      scannerDevice.scannerEnabled = enabled
  }

  Component.onDestruction: {
    if (scannerDevice) scannerDevice.scannerEnabled = false
  }

  // KeyboardPanel primes layer-shell focus whenever the panel opens. That's
  // what makes the SUPER+CTRL+W keybind land here with navigation ready.
  onOpenedChanged: {
    if (opened) {
      refresh(true)
      selectedIndex = wifiNetworks.length > 0 ? 0 : -1
      wifiActionFocused = false
      focusSection = wifiNetworks.length > 0 ? "wifi" : "dns"
      var idx = dnsProviders.indexOf(dnsProvider)
      dnsIndex = idx >= 0 ? idx : 0
      syncBandIndex()
      cursorActive = false
    } else {
      // Drop a restart armed by this open: without it a close/reopen inside
      // the 100ms window reuses the running timer and re-enables the scanner
      // almost immediately, undoing the deferral #6605 restored.
      scanRestart.stop()
      scanFallback.stop()
      scanPending = false
      // Reset throughput tracking so the next open doesn't compute a fake
      // rate from a sample taken minutes ago.
      prevSampleTime = 0
      downloadRate = 0
      uploadRate = 0
      pingIface = ""
      routerPingSamples = []
      internetPingSamples = []
      routerPingLatency = -1
      internetPingLatency = -1
      internetPingPacketLoss = 0
      setScannerEnabled(false)
    }
  }

  // When the passphrase prompt closes (Esc / Cancel / success) restore
  // focus to the keyCatcher so j/k/Enter resume working without a click.
  // The KeyboardPanel's focusTarget covers initial popup-open; this handles
  // the inline-editor case where focus was handed off to a child.
  onPasswordSsidChanged: {
    if (passwordSsid === "" && opened) {
      passwordText = ""
      Qt.callLater(function() { if (keyCatcher) keyCatcher.forceActiveFocus() })
    }
  }

  // Keep selectedIndex valid as scans refresh the network list.
  // If the list empties (station gone, e.g. wifi off), bounce the cursor
  // back to the DNS row so the panel doesn't end up with no cursor at all.
  onWifiNetworksChanged: {
    if (wifiNetworks.length === 0) {
      selectedIndex = -1
      wifiActionFocused = false
      if (focusSection === "wifi") focusSection = "dns"
    } else if (passwordSsid !== "") {
      var passwordIndex = wifiIndexForSsid(passwordSsid)
      if (passwordIndex >= 0) {
        selectedIndex = passwordIndex
        focusSection = "wifi"
      }
    } else if (selectedIndex >= wifiNetworks.length) {
      selectedIndex = wifiNetworks.length - 1
    } else if (selectedIndex < 0 && opened) {
      selectedIndex = 0
    }

    if (selectedIndex < 0 || selectedIndex >= wifiNetworks.length || !canForgetNetwork(wifiNetworks[selectedIndex])) {
      wifiActionFocused = false
    }
  }

  onWifiDeviceChanged: {
    setScannerEnabled(true)
    syncWifiNetworks()
  }

  // Access points arrive one signal at a time, a dozen in a burst while scanning. A sync
  // for each was a pass over the whole list for each; one in a short while takes them all.
  Timer {
    id: syncSoon
    interval: 60
    repeat: false
    onTriggered: root.syncWifiNetworks()
  }
  onWifiNetworkObjectsChanged: if (!syncSoon.running) syncSoon.start()

  function selectByDelta(delta) {
    if (wifiNetworks.length === 0) { selectedIndex = -1; return }
    if (selectedIndex < 0) selectedIndex = delta > 0 ? 0 : wifiNetworks.length - 1
    else selectedIndex = Math.max(0, Math.min(wifiNetworks.length - 1, selectedIndex + delta))
    wifiActionFocused = false
  }

  function canForgetNetwork(net) {
    return Model.canForgetNetwork(net)
  }

  function canShareNetwork(net) {
    if (!net || !net.connected) return false
    return net.security !== WifiSecurityType.Wpa2Eap && net.security !== WifiSecurityType.WpaEap
  }

  function selectWifiActionByDelta(delta) {
    if (selectedIndex < 0 || selectedIndex >= wifiNetworks.length) return
    if (!canForgetNetwork(wifiNetworks[selectedIndex])) {
      wifiActionFocused = false
      return
    }
    if (delta > 0) wifiActionFocused = true
    else if (delta < 0) wifiActionFocused = false
  }

  // Enter/Space on the highlighted row. Mirrors row-click semantics:
  // connected → disconnect, credentials-required/unknown → prompt,
  // passwordless/known → connect.
  function activateSelected() {
    if (busy || selectedIndex < 0 || selectedIndex >= wifiNetworks.length) return
    var net = wifiNetworks[selectedIndex]
    if (!net) return
    if (wifiActionFocused && canForgetNetwork(net)) { forget(net); return }
    // Only act on a row that still resolves. disconnect() falls back to
    // connectedWifiNetwork when handed null, so a row left stale by scan churn
    // would otherwise tear down whatever is connected now instead.
    if (net.connected) { disconnectRow(net.ssid); return }
    if (requiresCredentials(net.security) && !net.known) { openPasswordPrompt(net.ssid); return }
    connectDirectly(net.ssid)
  }

  // Bar pill state, derived from the native NetworkManager service so the
  // icon reflects connection changes without polling. Wired is preferred
  // when both are up, matching the default-route device.
  readonly property var wiredDevice: findDevice(DeviceType.Wired)
  readonly property string kind: {
    if (wiredDevice && wiredDevice.connected) return "ethernet"
    if (connectedWifiNetwork) return "wifi"
    return "disconnected"
  }
  readonly property int signalStrength: connectedWifiNetwork
    ? Math.round((connectedWifiNetwork.signalStrength || 0) * 100)
    : -1

  function copyToClipboard(value) {
    if (!value || !root.bar) return
    Quickshell.execDetached(["bash", "-c", "printf %s " + Util.shellQuote(value) + " | wl-copy"])
  }

  readonly property string icon: Model.connectionIcon(kind, signalStrength)

  // The share card is its own panel plugin (omarchy.wifiqr) so a replacement
  // design can take it over; summon() routes to whichever implementation is
  // enabled. The panel's own button pins the interface it is showing. The
  // IPC route forces self-detection instead: details polling stops while the
  // panel is closed, so its cached interface can be stale.
  function summonWifiQr(forceDetect) {
    controller.hide()
    cancelPasswordPrompt()
    var payload = {}
    if (!forceDetect && info.type === "wifi" && info.iface) {
      payload.iface = info.iface
      if (info.ssid) payload.ssid = info.ssid
    }
    // A third-party clone can be handed a shell without summon (or none).
    if (bar && bar.shell && typeof bar.shell.summon === "function")
      bar.shell.summon("omarchy.wifiqr", JSON.stringify(payload))
  }

  function refresh(scanWifi) {
    if (scanWifi === undefined) scanWifi = false
    if (!detailsProc.running) detailsProc.running = true
    if (!dnsProc.running) {
      dnsProc.command = ["bash", "-c", root.dnsCommand("")]
      dnsProc.running = true
    }
    if (!bandProc.running) {
      bandProc.command = ["omarchy-network-band"]
      bandProc.running = true
    }
    // A closed panel has no nearby-network list to fill, and bare refresh()
    // reaches here from action completion, timeouts and construction.
    if (opened && wifiDevice) {
      if (scanWifi) {
        scanning = true
        setScannerEnabled(false)
        armScan()
      } else {
        setScannerEnabled(true)
      }
    }
    syncWifiNetworks()
  }

  function formatHeaderSpeed(mbps) {
    return Model.formatHeaderSpeed(mbps)
  }

  function formatHeaderFreq(mhz) {
    return Model.formatHeaderFreq(mhz)
  }

  function headerDetail() {
    return Model.headerDetail(info)
  }

  function updateDetails(raw) {
    var next = Model.parseKeyValue(raw)

    // A band change tears the link down and brings it back, and the status
    // command reports nothing at all while there is no route. Publishing that
    // would blank every stat and unmount the whole section mid-toggle, so the
    // last good sample stands until the reconnect settles. A real disconnect is
    // still reported, because nothing is in flight then.
    if (bandBusy && !next.iface) return

    info = next
    updateThroughput(next)
    updatePingLatency(next)
  }

  function updateThroughput(next) {
    var state = Model.throughputState({
      prevIface: prevIface,
      prevRxBytes: prevRxBytes,
      prevTxBytes: prevTxBytes,
      prevSampleTime: prevSampleTime,
      downloadRate: downloadRate,
      uploadRate: uploadRate
    }, next, Date.now() / 1000)

    prevIface = state.prevIface
    prevRxBytes = state.prevRxBytes
    prevTxBytes = state.prevTxBytes
    prevSampleTime = state.prevSampleTime
    downloadRate = state.downloadRate
    uploadRate = state.uploadRate
  }

  function updatePingLatency(next) {
    var state = Model.pingLatencyState({
      pingIface: pingIface,
      routerPingSamples: routerPingSamples,
      internetPingSamples: internetPingSamples
    }, next, pingHistoryWindow, pingAverageWindow)

    pingIface = state.pingIface
    routerPingSamples = state.routerPingSamples
    internetPingSamples = state.internetPingSamples
    routerPingLatency = state.routerPingLatency
    internetPingLatency = state.internetPingLatency
    internetPingPacketLoss = state.internetPingPacketLoss
  }

  function formatBytes(bytes) {
    return Model.formatBytes(bytes)
  }

  function formatRate(bytesPerSec) {
    return Model.formatRate(bytesPerSec)
  }

  function formatPingLatency(ms) {
    return Model.formatPingLatency(ms, hasInternetPing)
  }

  function formatPacketLoss(percent) {
    return Model.formatPacketLoss(percent, hasInternetPing)
  }

  // Prefer a connected device: a machine can expose several NICs of the
  // same type (e.g. an idle onboard port alongside the active adapter),
  // and the first-enumerated one may be carrierless.
  function findDevice(type) {
    var devices = networkDevices || []
    var fallback = null
    for (var i = 0; i < devices.length; i++) {
      var device = devices[i]
      if (!device || device.type !== type) continue
      if (device.connected) return device
      if (!fallback) fallback = device
    }
    return fallback
  }

  function findConnectedWifiNetwork() {
    var networks = wifiNetworkObjects || []
    for (var i = 0; i < networks.length; i++) {
      if (networks[i] && networks[i].connected) return networks[i]
    }
    return null
  }

  function syncWifiNetworks() {
    var nets = []
    var networks = wifiNetworkObjects || []

    for (var i = 0; i < networks.length; i++) {
      var network = networks[i]
      if (!network) continue
      checkActionCompletion(network)
      var row = Model.wifiRow(network)
      if (row) nets.push(row)
    }
    wifiNetworks = Model.sortWifiRows(nets)
    wifiStationAvailable = !!wifiDevice
    scanning = false
  }

  function wifiSectionTitle(index) {
    return Model.wifiSectionTitle(wifiNetworks, index)
  }

  function wifiIconFor(strength) {
    return Model.wifiIconFor(strength)
  }

  function updateDns(raw) {
    var value = String(raw || "").trim()
    dnsProvider = value || "DHCP"
  }

  function updateBand(raw) {
    var status = Model.parseBandStatus(raw)

    // Mid-reconnect there is no connected station, so the command reports
    // nothing. Publishing that would empty the option list and unmount the
    // section on every toggle -- same guard as updateDetails.
    if (bandBusy && status.available.length === 0) return

    bandCurrent = status.band
    bandSelected = status.selected
    bandAvailable = status.available
  }

  // Pinning a band reassociates, but the panel deliberately stays open: the
  // reconnect is the thing you want to watch, and the details rows above
  // report it as it happens.
  function setBand(band) {
    if (!band || actionProc.running) return

    root.pendingBand = band
    actionProc.command = ["omarchy-network-band", band]
    actionProc.running = true
  }

  // The speed test is its own panel plugin (omarchy.speedtest) so a
  // replacement design can take it over; summon() routes to whichever
  // implementation is enabled. The payload names the connection when this
  // panel knows it; the plugin looks it up itself otherwise.
  function summonSpeedTest() {
    controller.hide()
    cancelPasswordPrompt()
    var connection = ""
    if (info.type === "wifi") connection = info.ssid || "Wi-Fi"
    else if (info.type === "ethernet") connection = "Ethernet"
    if (bar && bar.shell && typeof bar.shell.summon === "function")
      bar.shell.summon("omarchy.speedtest", connection ? JSON.stringify({ connection: connection }) : "{}")
  }

  function dnsCommand(provider) {
    var command = "omarchy-dns"
    if (provider) command += " " + Util.shellQuote(provider)
    return command
  }

  function setDns(provider) {
    if (!root.bar || !provider || actionProc.running) return

    if (provider === "Custom") {
      var launcher = "omarchy-launch-floating-terminal-with-presentation"
      root.bar.run(launcher + " " + Util.shellQuote(root.dnsCommand(provider)))
      root.close()
      return
    }

    root.pendingDnsProvider = provider
    actionProc.command = ["bash", "-c", root.dnsCommand(provider)]
    actionProc.running = true
    root.close()
  }

  function requiresCredentials(security) {
    return Model.requiresCredentials(security, WifiSecurityType.Open, WifiSecurityType.Owe)
  }

  function openPasswordPrompt(ssid) {
    if (passwordSsid !== ssid) {
      passwordText = ""
      identityText = ""
    }
    passwordSsid = ssid
  }

  function networkForSsid(ssid) {
    var networks = wifiNetworkObjects || []
    for (var i = 0; i < networks.length; i++) {
      if (networks[i] && networks[i].name === ssid) return networks[i]
    }
    return null
  }

  function wifiIndexForSsid(ssid) {
    for (var i = 0; i < wifiNetworks.length; i++) {
      if (wifiNetworks[i] && wifiNetworks[i].ssid === ssid) return i
    }
    return -1
  }

  function runNetworkAction(kind, network, callback) {
    if (actionKind !== "" || !network) return
    var ssid = network.name || ""
    actionSsid = ssid
    actionKind = kind
    failureSsid = ""
    failureReason = ""
    callback(network)
    // Safety net: if onExited never fires (process death, signal handler
    // throws, etc.), clear the busy state so the row doesn't get stuck on
    // "Connecting…" / "Disconnecting…" forever.
    actionTimeout.restart()
  }

  function clearNetworkAction() {
    actionTimeout.stop()
    if (actionKind === "connect") passwordSsid = ""
    failureSsid = ""
    failureReason = ""
    actionSsid = ""
    actionKind = ""
    refresh()
  }

  function failNetworkAction(network, reason) {
    if (!network || actionKind === "" || actionSsid !== (network.name || "")) return
    actionTimeout.stop()
    failureSsid = actionSsid
    failureReason = networkFailureReason(reason, requiresCredentials(network.security))
    actionSsid = ""
    actionKind = ""
    refresh()
  }

  function networkFailureReason(reason, needsCredentials) {
    return Model.networkFailureReason(reason, needsCredentials, connectionFailReasons)
  }

  function shouldRepromptPassphrase(reason, needsCredentials) {
    return Model.shouldRepromptPassphrase(reason, needsCredentials, connectionFailReasons)
  }

  function checkActionCompletion(network) {
    if (!network || actionKind === "" || actionSsid !== (network.name || "")) return
    if (actionKind === "connect" && network.connected) clearNetworkAction()
    else if (actionKind === "disconnect" && !network.connected && !network.stateChanging) clearNetworkAction()
    else if (actionKind === "forget" && !network.known && !network.stateChanging) clearNetworkAction()
  }

  function connectDirectly(ssid) {
    runNetworkAction("connect", networkForSsid(ssid), function(network) { network.connect() })
  }

  function connectWithPassphrase(ssid, passphrase) {
    runNetworkAction("connect", networkForSsid(ssid), function(network) { network.connectWithPsk(passphrase) })
  }

  function connectEnterprise(ssid, identity, passphrase) {
    runNetworkAction("connect", networkForSsid(ssid), function(network) {
      enterpriseConnect.secret = passphrase
      enterpriseConnect.command = ["bash", "-c", Model.enterpriseConnectScript, "nmcli-eap", ssid, identity]
      enterpriseConnect.running = true
    })
  }

  // Creates and activates the 802.1X profile (see Model.enterpriseConnectScript).
  // The password goes over stdin, never argv.
  Process {
    id: enterpriseConnect
    property string secret: ""
    stdinEnabled: true
    onStarted: {
      write(secret + "\n")
      secret = ""
    }
  }

  function disconnect(network) {
    runNetworkAction("disconnect", network || connectedWifiNetwork, function(net) { net.disconnect() })
  }

  // Disconnect from a row's SSID. Rows are primitive snapshots that can outlive
  // their WifiNetwork, and disconnect()'s null fallback targets whatever is
  // connected now, so a stale row must do nothing rather than hit an unrelated
  // network. Callers that mean "drop the current connection" call disconnect().
  function disconnectRow(ssid) {
    var network = networkForSsid(ssid)
    if (network) disconnect(network)
  }

  function forget(net) {
    runNetworkAction("forget", net ? networkForSsid(net.ssid) : null, function(network) { network.forget() })
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Component.onCompleted: refresh()

  // Pulls everything we want about the active route's interface in one shot.
  Process {
    id: detailsProc
    command: ["omarchy-network-status", "--verbose"]
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.updateDetails(text)
    }
  }

  // The scan waits for the card to be drawn. Its answers (a dozen access points, each
  // a D-Bus signal handled on the GUI thread) used to land in front of the first frame
  // and hold it back for most of a second; now the card is up with its list empty and
  // the rows come in under it (the list has a fixed height, so nothing moves).
  property bool scanPending: false
  function armScan() {
    scanPending = true
    if (panel.shown) { scanFallback.stop(); scanRestart.restart() }
    else scanFallback.restart()
  }
  Connections {
    target: panel
    function onShownChanged() {
      if (panel.shown && root.scanPending && root.opened) { scanFallback.stop(); scanRestart.restart() }
    }
  }
  // The card is not drawn after all (a handover that never lands): scan anyway.
  Timer {
    id: scanFallback
    interval: 700
    repeat: false
    onTriggered: if (root.scanPending && root.opened) scanRestart.restart()
  }

  Timer {
    id: scanRestart
    interval: 100
    repeat: false
    onTriggered: {
      root.scanPending = false
      if (root.opened && root.wifiDevice) {
        root.setScannerEnabled(true)
        scanDone.start()
      }
    }
  }

  Timer {
    id: scanDone
    interval: 1500
    repeat: false
    onTriggered: root.syncWifiNetworks()
  }

  Process {
    id: dnsProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.updateDns(text)
    }
  }

  Process {
    id: bandProc
    stdout: StdioCollector {
      waitForEnd: true
      onStreamFinished: root.updateBand(text)
    }
  }

  // Slower than detailsPoll on purpose: this shells out to nmcli several times,
  // and band availability only moves when a scan turns up a new BSSID.
  Timer {
    id: bandPoll
    interval: 4000
    repeat: true
    running: root.opened
    onTriggered: {
      if (bandProc.running) return
      bandProc.command = ["omarchy-network-band"]
      bandProc.running = true
    }
  }

  // Action runner for DNS provider changes. Wi-Fi actions use the
  // Quickshell.Networking NetworkManager backend directly.
  Process {
    id: actionProc
    stdout: StdioCollector { id: actionStdout; waitForEnd: true }
    stderr: StdioCollector { id: actionStderr; waitForEnd: true }
    onExited: function(exitCode) {
      if (root.pendingDnsProvider !== "") {
        if (exitCode === 0) root.dnsProvider = root.pendingDnsProvider
        root.pendingDnsProvider = ""
      }
      if (root.pendingBand !== "") {
        // A refused or reverted pin leaves bandSelected alone, so the pills
        // keep showing what is actually in force rather than what was asked.
        if (exitCode === 0) root.bandSelected = root.pendingBand
        root.pendingBand = ""
        // The panel stayed open through the reconnect, so pull fresh state now
        // instead of leaving stale readings until the next poll tick.
        root.refresh()
      }
    }
  }

  // Poll details while the panel is open so the IP/route header catches up
  // as soon as NetworkManager finishes activating a connection.
  Timer {
    id: detailsPoll
    interval: 1500
    repeat: true
    running: root.opened
    onTriggered: if (!detailsProc.running) detailsProc.running = true
  }

  Timer {
    id: connectionPhraseTimer
    interval: 2800
    running: root.opened && (root.info.type === "ethernet" || (root.info.type === "wifi" && root.canDisconnect))
    repeat: true
    // The phrase changes where it is: nothing fades (the stock swap faded the
    // hero's status line out and back in around this same step).
    onTriggered: root.connectionPhraseIndex = (root.connectionPhraseIndex + 1) % root.connectionPhrases.length
  }

  Timer {
    id: actionTimeout
    // Must outlast NetworkManager's 25s supplicant timeout: a wrong saved
    // PSK fails with WifiAuthTimeout at ~25s, and that failure has to land
    // while the action is still tracked to show "Wrong password" and reopen
    // the passphrase prompt.
    interval: 30000
    repeat: false
    onTriggered: {
      if (!root.actionKind) return
      var reason
      if (root.actionKind === "connect") reason = "Timed out connecting"
      else if (root.actionKind === "disconnect") reason = "Timed out disconnecting"
      else reason = "Timed out forgetting"
      root.failureSsid = root.actionSsid
      root.failureReason = reason
      root.actionSsid = ""
      root.actionKind = ""
      root.refresh()
    }
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    text: root.icon

    onPressed: function(b) {
      if (root.opened) root.close()
      // open() is enough: onOpenedChanged runs refresh(true), which defers the
      // PHY scan past the first frame. The bare refresh() that used to follow
      // took the no-scan branch and set scannerEnabled synchronously, undoing
      // that deferral and stalling the open on NetworkManager's AP flood.
      else root.open()
    }
  }


  // ==========================================================================
  // The view, on the pixel grid (quadrille's popup kit in place of qs.Ui's
  // panel widgets). Everything above is the stock panel's; this reads that
  // state and calls the same functions the stock view called.
  // ==========================================================================

  Icons { id: icons }

  // The stock list opened at its top: so does this one.
  Connections {
    target: root
    function onOpenedChanged() { if (root.opened) scroll.reset() }
  }

  // Sprites for what the stock panel drew as Nerd Font glyphs
  // (Model.connectionIcon / wifiIconFor keep the glyphs, for the bar face).
  function heroSprite(kind) {
    if (kind === "wifi") return icons.wifiHero
    if (kind === "ethernet") return icons.ethernetHero
    return icons.offlineHero
  }
  // The fan's lit arcs for a signal in percent: the bar skin's steps, so the
  // popup and the bar say the same.
  function wifiLevel(strength) {
    if (strength === undefined || strength === null || strength < 0) return 0
    return strength < 25 ? 1 : (strength < 60 ? 2 : 3)
  }

  // The hero's two lines: the stock heroSsid and heroMeta texts.
  readonly property string heroTitle: {
    var title
    if (root.info.type === "wifi") title = root.info.ssid || "Wi-Fi"
    else if (root.info.type === "ethernet") title = "Ethernet"
    else title = root.info.iface || (root.kind === "disconnected" ? "Disconnected" : "No connection")
    var detail = root.headerDetail()
    return detail !== "" ? title + " (" + detail + ")" : title
  }
  readonly property string heroStatus: {
    if (root.info.type === "wifi") {
      if (root.canDisconnect) return root.connectionPhrase.toUpperCase()
      if (root.kind === "disconnected") return "NOT CONNECTED"
      return ""
    }
    if (root.info.type === "ethernet") return root.connectionPhrase.toUpperCase()
    if (root.kind === "disconnected") return "NOT CONNECTED"
    return ""
  }

  readonly property var dnsTips: ({
    "DHCP": "Use DNS from DHCP",
    "Cloudflare": "Set DNS to Cloudflare",
    "Google": "Set DNS to Google",
    "Custom": "Set custom DNS servers"
  })

  // The Wi-Fi list as runs under a section title: where
  // Model.wifiSectionTitle starts a section (KNOWN NETWORKS, OTHER NETWORKS),
  // a group starts. Rows keep their index into wifiNetworks.
  readonly property var wifiGroups: {
    var groups = []
    for (var i = 0; i < wifiNetworks.length; i++) {
      var title = wifiSectionTitle(i)
      if (title !== "" || groups.length === 0)
        groups.push({ title: title || "NETWORKS", start: i, count: 0 })
      groups[groups.length - 1].count++
    }
    return groups
  }

  // The band group is there for any Wi-Fi link (see the view).
  readonly property bool bandGroupShown: root.kind === "wifi" || root.bandBusy

  readonly property string emptyListText: root.scanning
    ? "Scanning…"
    : (Networking.wifiEnabled ? "No networks found" : "Wi-Fi is off")

  // Width of cell `i` of `n` across `total` with `spacing` between: whole vpx,
  // the last taking what is left so the row ends flush.
  function cellWidth(total, n, spacing, i) {
    if (n <= 0) return 0
    var cell = panel.g.floor((total - spacing * (n - 1)) / n)
    return i === n - 1 ? total - (n - 1) * (cell + spacing) : cell
  }
  // A tooltip's x under an item at `itemX`, `itemW` wide, in a row `room`
  // wide: centred, but kept inside the row.
  function tipX(itemX, itemW, tipW, room) {
    var centred = panel.g.centre(itemW, tipW)
    return Math.max(-itemX, Math.min(centred, room - itemX - tipW))
  }

  QPopup {
    id: panel
    anchorItem: button
    owner: root
    bar: root.bar
    open: root.opened
    focusTarget: keyCatcher
    // 54 cells of body, and the list's scroll gutter beside them.
    contentWidth: panel.fittedContentWidth(panel.cardWidth(54) + panel.g.px(4))
    contentHeight: panel.fittedContentHeight(column.implicitHeight)
    // The rows come from Repeaters, which a Column lays out only once the
    // window is up: hold the card until its size has stopped moving.
    settleCount: 4

    // Catches all unhandled keys for keyboard navigation. AfterItem priority
    // lets the passphrase field (a child via focus chain) get its keys
    // first; only events the focused subtree ignores bubble back here.
    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      // Freeze the cursor model while the inline password prompt is open;
      // the field inside owns input until Esc/Enter/Cancel.
      blocked: root.passwordSsid !== ""

      onMoveRequested: function(dx, dy) {
        if (!root.cursorActive) {
          root.cursorActive = true
          if (dy >= 0) return
        }
        if (dy !== 0) {
          // Vertical order is header ⇄ band ⇄ DNS ⇄ wifi, with the band section
          // dropping out of the chain entirely when it isn't on screen.
          if (root.focusSection === "header") {
            if (dy > 0) {
              if (root.canSelectBand) {
                root.focusSection = "band"
                root.bandAutoFocused = true
              } else {
                root.focusSection = "dns"
              }
            }
          } else if (root.focusSection === "band") {
            // Automatic on the header line, then the pills -- which collapse
            // away under Automatic, leaving a single row to walk.
            if (dy < 0) {
              if (!root.bandAutoFocused) {
                root.bandAutoFocused = true
              } else if (root.headerActionCount > 0) {
                root.focusSection = "header"
                root.headerIndex = 0
              }
            } else if (root.bandAutoFocused && root.bandPillsVisible) {
              root.bandAutoFocused = false
            } else {
              root.focusSection = "dns"
            }
          } else if (root.focusSection === "dns") {
            // k from DNS moves up into the band section when it's on screen,
            // then the disconnect button; otherwise stays put. j drops into the
            // wifi list if there's anywhere to land.
            if (dy < 0) {
              if (root.canSelectBand) {
                root.focusSection = "band"
                root.bandAutoFocused = !root.bandPillsVisible
              } else if (root.headerActionCount > 0) {
                root.focusSection = "header"
                root.headerIndex = 0
              }
            } else if (root.wifiNetworks.length > 0) {
              root.focusSection = "wifi"
              if (root.selectedIndex < 0) root.selectedIndex = 0
            }
          } else {  // wifi
            // k from the top row escapes back up to the DNS row rather than
            // wrapping around to the bottom of the list.
            if (dy < 0 && root.selectedIndex <= 0) {
              root.focusSection = "dns"
              root.wifiActionFocused = false
            }
            else root.selectByDelta(dy)
          }
        }
        if (dx !== 0) {
          if (root.focusSection === "header") root.selectHeaderByDelta(dx)
          else if (root.focusSection === "band") { if (!root.bandAutoFocused) root.selectBandByDelta(dx) }
          else if (root.focusSection === "dns") root.selectDnsByDelta(dx)
          else if (root.focusSection === "wifi") root.selectWifiActionByDelta(dx)
        }
      }
      onActivateRequested: {
        if (root.cursorActive) {
          if (root.focusSection === "header") root.activateHeader()
          else if (root.focusSection === "band") root.activateBand()
          else if (root.focusSection === "dns") root.activateDns()
          else root.activateSelected()
        }
      }
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) {
        if (t === "r" || t === "R") root.refresh()
        else if (t === "w" || t === "W") root.toggleNetwork()
      }

      Column {
        id: column
        width: parent.width
        spacing: panel.g.px(6)

        // Everything but the list stops where the list's scroll gutter starts,
        // so the right edges line up down the card.
        readonly property real body: width - scroll.gutter
        // What sits above the list, so the list can take the rest of the screen.
        // (From the state, not from `visible`: that is false for everything
        // in the card while it is closed.)
        readonly property real fixedHeight: hero.height
          + (!!root.info.iface ? spacing + stats.height : 0)
          + (root.bandGroupShown ? spacing + bandGroup.height : 0)
          + spacing + dnsGroup.height

        // ---------- Hero: link · name and state · actions ----------
        // Status only; the switch owns the radio, mouse and keyboard alike.
        // (The switch keeps two vpx of air at its right for its brackets: the
        // hero reaches into the gutter by that much, so the box ends flush
        // with everything below.)
        Item {
          id: hero
          z: 4
          width: column.body + (powerSwitch.visible ? panel.g.px(2) : 0)
          height: 2 * panel.g.line

          readonly property real iconSpace: panel.g.px(15 + 4)
          readonly property real trailSpace: heroActions.width > 0 ? heroActions.width + panel.g.px(3) : 0
          readonly property int columns: panel.g.columns(width - iconSpace - trailSpace)

          // The link, drawn at the size of the two lines beside it.
          Sprite {
            y: panel.g.centre(hero.height, height)
            rows: root.heroSprite(root.kind)
            level: root.kind === "wifi" ? root.wifiLevel(root.signalStrength) : 1
            color: root.networkManagerAvailable ? Role.ink : Role.faint
            dim: root.kind === "ethernet" ? Role.muted : Role.faint
          }

          Column {
            x: hero.iconSpace
            PixelText { text: root.heroTitle; ink: Role.ink; columns: hero.columns }
            PixelText { text: root.heroStatus; ink: Role.muted; columns: hero.columns }
          }

          Row {
            id: heroActions
            anchors.right: parent.right
            y: panel.g.centre(hero.height, height)
            spacing: panel.g.px(3)

            // Sharing belongs to the connected-network hero rather than the
            // scan result row. (The buttons sit a vpx low so they and the
            // switch's box both centre on the two lines of text.)
            QButton {
              id: qrAction
              y: panel.g.px(1)
              visible: root.canShareWifi
              icon: icons.qr
              hasCursor: root.qrHeaderHasCursor
              onHovered: function(on) { if (on) root.setHeaderCursor(root.qrHeaderIndex) }
              onClicked: root.summonWifiQr()
              QTip { text: "Show QR code"; shown: qrAction.hot; x: qrAction.width - width }
            }

            QButton {
              id: speedAction
              y: panel.g.px(1)
              visible: root.canRunSpeedTest
              icon: icons.gauge
              hasCursor: root.speedHeaderHasCursor
              onHovered: function(on) { if (on) root.setHeaderCursor(root.speedHeaderIndex) }
              onClicked: root.summonSpeedTest()
              QTip { text: "Run a speed test"; shown: speedAction.hot; x: speedAction.width - width }
            }

            QSwitch {
              id: powerSwitch
              visible: root.canToggleWifi
              checked: Networking.wifiEnabled
              hasCursor: root.toggleHeaderHasCursor
              onHovered: function(on) { if (on) root.setHeaderCursor(root.toggleHeaderIndex) }
              onToggled: root.toggleNetwork()
              QTip { text: root.toggleHint; shown: powerSwitch.containsMouse; x: powerSwitch.width - panel.g.px(2) - width }
            }
          }
        }

        // ---------- Readings: transfer first, then IP and gateway ----------
        // Every reading stays mounted whether or not there is data behind it,
        // so a sample arriving late never reflows the grid.
        Row {
          id: stats
          z: 3
          visible: !!root.info.iface
          width: column.body
          spacing: panel.g.px(8)

          readonly property real half: panel.g.floor((width - spacing) / 2)

          Column {
            width: stats.half
            QReading {
              width: parent.width
              label: "Ping"
              value: root.formatPingLatency(root.internetPingLatency)
              valueInk: root.internetPingPacketLoss > 0 ? Role.alarm : Role.ink
            }
            QReading {
              width: parent.width
              label: "Receiving"
              value: root.hasTransferStats ? root.formatRate(root.downloadRate) : "--"
            }
            QReading {
              width: parent.width
              label: "Downloaded"
              value: root.hasTransferStats ? root.formatBytes(parseFloat(root.info.rx_bytes || "0")) : "--"
            }
            CopyReading {
              width: parent.width
              label: "IP address"
              value: root.info.ip || "--"
              copyable: !!root.info.ip
              tipText: "Copy IP"
            }
          }

          Column {
            width: stats.half
            QReading {
              width: parent.width
              label: "Packet loss"
              value: root.formatPacketLoss(root.internetPingPacketLoss)
              valueInk: root.internetPingPacketLoss > 0 ? Role.alarm : Role.ink
            }
            QReading {
              width: parent.width
              label: "Sending"
              value: root.hasTransferStats ? root.formatRate(root.uploadRate) : "--"
            }
            QReading {
              width: parent.width
              label: "Uploaded"
              value: root.hasTransferStats ? root.formatBytes(parseFloat(root.info.tx_bytes || "0")) : "--"
            }
            CopyReading {
              width: parent.width
              label: "Gateway"
              value: root.info.gateway || "--"
              copyable: !!root.info.gateway
              tipText: "Copy gateway"
            }
          }
        }

        // ---------- Wi-Fi band ----------
        // On Wi-Fi. "Automatic" heads the group; under it the bands, which are
        // there only while one is pinned. The stock hid the section until the
        // network answered on two bands, which a scan can find out a few
        // seconds after the card is up: the group is here for any Wi-Fi link,
        // and until there is a choice it says so in place of the switch (the
        // keyboard still skips it, as canSelectBand says).
        Group {
          id: bandGroup
          z: 2
          visible: root.bandGroupShown
          name: root.bandSectionTitle
          width: column.body
          height: root.bandGroupShown ? implicitHeight : 0

          Column {
            width: parent.width
            spacing: panel.g.px(2)

            Item {
              visible: !root.canSelectBand
              width: parent.width
              height: panel.g.px(13)
              PixelText {
                y: panel.g.px(1)
                text: "One band in range"
                ink: Role.muted
              }
            }

            Item {
              visible: root.canSelectBand
              width: parent.width
              height: panel.g.px(13)

              PixelText {
                y: panel.g.px(1)
                text: "Automatic"
                ink: Role.ink
              }
              // A change in flight, or a pinned band the radio is not on.
              PixelText {
                x: bandAutoSwitch.x - panel.g.px(1) - width
                y: panel.g.px(1)
                text: root.bandBusy ? "Switching…"
                  : (root.bandPinned && root.bandCurrent !== "" && root.bandCurrent !== root.bandEffective
                     ? "On " + root.bandLabel(root.bandCurrent) : "")
                ink: Role.muted
              }
              QSwitch {
                id: bandAutoSwitch
                x: parent.width - width + panel.g.px(2)
                checked: !root.bandPinned
                busy: root.bandBusy
                hasCursor: root.cursorActive && root.focusSection === "band" && root.bandAutoFocused
                onToggled: root.toggleBandAuto()
                onHovered: function(isHovered) {
                  if (!isHovered) return
                  root.cursorActive = true
                  root.focusSection = "band"
                  root.bandAutoFocused = true
                }
                QTip {
                  text: root.bandPinned ? "Let Wi-Fi pick the band" : "Stay on " + root.bandLabel(root.bandCurrent)
                  shown: bandAutoSwitch.containsMouse
                  x: bandAutoSwitch.width - panel.g.px(2) - width
                }
              }
            }

            // The bands. The one in force (or asked for, while the change is
            // in flight) is the inverse block.
            Row {
              id: bandRow
              visible: root.bandPillsVisible
              width: parent.width
              spacing: panel.g.px(4)

              Repeater {
                model: root.bandAvailable
                QButton {
                  id: bandPill
                  required property var modelData
                  required property int index
                  fixedWidth: root.cellWidth(bandRow.width, root.bandAvailable.length, bandRow.spacing, index)
                  text: root.bandLabel(modelData)
                  active: root.bandEffective === modelData
                  hasCursor: root.cursorActive && root.focusSection === "band"
                    && !root.bandAutoFocused && root.bandIndex === index
                  onClicked: root.setBand(modelData)
                  onHovered: function(isHovered) {
                    if (!isHovered) return
                    root.cursorActive = true
                    root.focusSection = "band"
                    root.bandIndex = bandPill.index
                  }
                  QTip {
                    text: root.bandTooltip(bandPill.modelData)
                    shown: bandPill.hot
                    x: root.tipX(bandPill.x, bandPill.width, width, bandRow.width)
                  }
                }
              }
            }
          }
        }

        // ---------- DNS provider ----------
        Group {
          id: dnsGroup
          z: 1
          name: "DNS PROVIDER"
          width: column.body
          height: implicitHeight

          Row {
            id: dnsRow
            width: parent.width
            spacing: panel.g.px(4)

            Repeater {
              model: root.dnsProviders
              QButton {
                id: dnsPill
                required property string modelData
                required property int index
                fixedWidth: root.cellWidth(dnsRow.width, root.dnsProviders.length, dnsRow.spacing, index)
                text: modelData
                active: root.dnsProvider === modelData
                hasCursor: root.cursorActive && root.focusSection === "dns" && root.dnsIndex === index
                onClicked: root.setDns(modelData)
                onHovered: function(isHovered) {
                  if (!isHovered) return
                  root.cursorActive = true
                  root.focusSection = "dns"
                  root.dnsIndex = dnsPill.index
                }
                QTip {
                  text: root.dnsTips[dnsPill.modelData] || ""
                  shown: dnsPill.hot
                  x: root.tipX(dnsPill.x, dnsPill.width, width, dnsRow.width)
                }
              }
            }
          }
        }

        // ---------- Wi-Fi networks (only if a Wi-Fi station is available) ----
        // Capped so a busy neighbourhood does not push the card off the
        // screen; the keyboard cursor is kept in view (NetRow).
        QScroll {
          id: scroll
          visible: root.wifiStationAvailable
          width: parent.width
          // A fixed height, not the list's: the networks come in over the
          // first second the card is open (the scan restarts on opening), and
          // they fill this room instead of growing the card. Ten rows, as
          // much as the screen allows; with the radio off nothing will
          // arrive, and the empty row is enough (turning it on sizes the list
          // again, at the click).
          height: !root.wifiStationAvailable ? 0
            : (Networking.wifiEnabled ? Math.min(listCap, panel.g.px(16 + 10 * 14 + 9 * 2)) : panel.g.px(16 + 14))
          contentHeight: wifiColumn.implicitHeight

          // At most the stock's 240, and no more than the screen has room for.
          readonly property real listCap: Math.max(panel.g.px(3 * 14),
            Math.min(panel.g.px(240),
              panel.g.floor(panel.availableCardHeight - panel.verticalContentInset
                - column.fixedHeight - column.spacing)))

          Column {
            id: wifiColumn
            width: parent.width
            spacing: panel.g.px(6)

            // Counts, not lists, are the models: a Repeater over an array that is a new
            // array each time rebuilds every row (and every glyph of every row) when
            // one access point comes or goes, which was most of the time a scan took.
            // A row, and its text, are made once and updated where they stand.
            Repeater {
              model: root.wifiGroups.length

              delegate: Item {
                id: section
                required property int index
                readonly property var group: root.wifiGroups[index] || ({ title: "", start: 0, count: 0 })
                width: wifiColumn.width
                height: sectionGroup.height

                Group {
                  id: sectionGroup
                  name: section.group.title
                  width: parent.width
                  height: implicitHeight

                  Column {
                    width: parent.width
                    spacing: panel.g.px(2)

                    Repeater {
                      model: section.group.count
                      NetRow {
                        required property int index
                        width: parent.width
                        rowIndex: section.group.start + index
                        net: root.wifiNetworks[section.group.start + index]
                      }
                    }
                  }
                }

                // A scan in flight: a muted word on the first rule, at its
                // right, so nothing moves when it comes and goes.
                Item {
                  visible: section.index === 0 && root.scanning
                  x: parent.width - 2 * panel.g.cellW - width
                  width: scanWord.width
                  height: panel.g.line
                  Rectangle {
                    x: -(panel.g.cellW - panel.g.unit)
                    y: sectionGroup.ruleY
                    width: parent.width + 2 * (panel.g.cellW - panel.g.unit)
                    height: panel.g.hair
                    antialiasing: false
                    color: Role.ground
                  }
                  PixelText { id: scanWord; text: "SCANNING"; ink: Role.muted }
                }
              }
            }

            // Nothing to list: say why, on a row's own height.
            Group {
              visible: root.wifiNetworks.length === 0
              name: "NETWORKS"
              width: parent.width
              height: root.wifiNetworks.length === 0 ? implicitHeight : 0
              QEmpty {
                width: parent.width
                icon: Sprites.wifi
                text: root.emptyListText
              }
            }
          }
        }
      }
    }
  }

  // A reading whose value can be copied (IP, gateway): click to copy.
  component CopyReading: QReading {
    id: reading
    property bool copyable: false
    property string tipText: "Copy to clipboard"

    MouseArea {
      id: copyMouse
      anchors.fill: parent
      enabled: reading.copyable && reading.value !== ""
      hoverEnabled: enabled
      cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onClicked: root.copyToClipboard(reading.value)
    }
    QTip {
      text: reading.tipText
      shown: copyMouse.enabled && copyMouse.containsMouse
      x: reading.width - width
    }
  }

  // A single Wi-Fi network entry: the signal fan, the name (two lines, then an
  // ellipsis), the state on a second line when there is one, a lock for a
  // network that asks for credentials and a forget button for a saved one.
  // The network joined is the inverse block. It opens inline into a passphrase
  // prompt when the user picks a network that requires credentials we do not
  // have. Clicking a connected row disconnects.
  component NetRow: Item {
    id: row
    required property var net
    required property int rowIndex

    readonly property var g: panel.g

    // ---- the stock NetworkRow's state ----
    readonly property bool isConnected: net && net.connected
    readonly property bool isKnown: !!(net && net.known)
    readonly property bool requiresCredentials: net ? root.requiresCredentials(net.security) : false
    readonly property bool isEnterprise: net
      ? (net.security === WifiSecurityType.Wpa2Eap || net.security === WifiSecurityType.WpaEap)
      : false
    readonly property bool canForget: root.canForgetNetwork(net)
    readonly property bool isSelected: root.focusSection === "wifi" && root.selectedIndex === rowIndex
    readonly property bool forgetFocused: isSelected && root.wifiActionFocused && canForget
    readonly property bool hasCursor: root.cursorActive && isSelected && !root.wifiActionFocused
    // Gate on the matching *Kind/*Reason being non-empty so a hidden-SSID
    // row (ssid == "") doesn't match the "" defaults of actionSsid etc.
    readonly property bool isBusy: root.actionKind !== "" && root.actionSsid === (net ? net.ssid : "")
    readonly property bool isFailed: root.failureReason !== "" && root.failureSsid === (net ? net.ssid : "")
    readonly property bool isPasswordOpen: root.passwordSsid !== "" && root.passwordSsid === (net ? net.ssid : "")

    function submitCredentials() {
      if (!net || root.busy || root.passwordText.length === 0) return
      if (!isEnterprise) return root.connectWithPassphrase(net.ssid, root.passwordText)
      if (root.identityText.length > 0) root.connectEnterprise(net.ssid, root.identityText, root.passwordText)
    }

    Connections {
      target: row.net ? root.networkForSsid(row.net.ssid) : null
      function onConnectionFailed(reason) {
        // Background auto-connect retries fire this too; only reprompt for
        // the connect started from this panel. Checked before
        // failNetworkAction, which clears the action state.
        var ours = root.actionKind === "connect" && root.actionSsid === (row.net.ssid || "")
        root.failNetworkAction(root.networkForSsid(row.net.ssid), reason)
        if (ours && root.shouldRepromptPassphrase(reason, row.requiresCredentials)) root.openPasswordPrompt(row.net.ssid)
      }
      function onConnectedChanged() {
        if (row.net) root.checkActionCompletion(root.networkForSsid(row.net.ssid))
      }
      function onKnownChanged() {
        if (row.net) root.checkActionCompletion(root.networkForSsid(row.net.ssid))
      }
      function onStateChangingChanged() {
        if (row.net) root.checkActionCompletion(root.networkForSsid(row.net.ssid))
      }
    }

    readonly property string statusText: {
      if (!net) return ""
      if (isPasswordOpen) return ""
      if (isBusy && root.actionKind === "connect") return "Connecting…"
      if (isBusy && root.actionKind === "disconnect") return "Disconnecting…"
      if (isBusy && root.actionKind === "forget") return "Forgetting…"
      if (isFailed) return root.failureReason || "Failed"
      if (isConnected) return "Connected"
      return ""
    }

    Timer {
      id: failureTimer
      interval: 2000
      running: row.isFailed && row.isPasswordOpen
      onTriggered: {
        root.failureSsid = ""
        root.failureReason = ""
        pwField.forceActiveFocus()
      }
    }

    // ---- the view ----
    readonly property bool hot: rowMouse.containsMouse
    readonly property color ink: isConnected ? Role.onAccent : Role.ink
    readonly property color statusInk: isFailed ? Role.alarm
      : (isConnected ? Role.onAccent : (isBusy ? Role.ink : Role.muted))
    readonly property real pad: g.px(3)
    readonly property real textX: pad + g.px(7 + 3)
    // The lock's place is kept on every row so the locks line up down the
    // list; a forget button stands left of it on a saved network.
    readonly property real lockX: width - pad - g.px(7)
    readonly property real textRight: canForget ? forgetButton.x - g.px(3) : lockX - g.px(3)
    readonly property int columns: Math.max(1, g.columns(textRight - textX))
    readonly property var lines: Glyphs.wrap(net ? (net.ssid || "Hidden") : "", columns, 2)
    readonly property real bodyHeight: (lines.length + (statusText !== "" ? 1 : 0)) * g.line + 2 * g.hair

    implicitHeight: bodyHeight + (isPasswordOpen ? g.px(2) + prompt.height : 0)
    height: implicitHeight

    // Keep the keyboard cursor in view. A row the pointer is on is in view
    // already, and scrolling under the pointer would move the cursor on.
    onHasCursorChanged: if (hasCursor && !rowMouse.containsMouse) scroll.ensureItemVisible(row)
    onIsPasswordOpenChanged: if (isPasswordOpen) Qt.callLater(function() { if (row) scroll.ensureItemVisible(row) })

    Rectangle {
      width: parent.width
      height: row.bodyHeight
      antialiasing: false
      color: row.isConnected ? Role.accent : (row.hot ? Role.raised : "transparent")
    }

    MouseArea {
      id: rowMouse
      width: parent.width
      height: row.bodyHeight
      hoverEnabled: true
      acceptedButtons: Qt.LeftButton
      cursorShape: Qt.PointingHandCursor
      enabled: !root.busy

      // Move the cursor here when the mouse enters; mouse leaving doesn't
      // clear it (so the cursor stays where the mouse last was and
      // subsequent j/k pick up from this row).
      onContainsMouseChanged: if (containsMouse) { root.cursorActive = true; root.focusSection = "wifi"; root.selectedIndex = row.rowIndex; root.wifiActionFocused = false }

      onClicked: {
        if (!row.net) return
        // Resync cursor in case keyboard nav moved it away while the mouse
        // stayed parked on this row — the click target is unambiguously here.
        root.cursorActive = true
        root.focusSection = "wifi"
        root.selectedIndex = row.rowIndex
        root.wifiActionFocused = false
        if (row.isConnected) {
          root.disconnectRow(row.net.ssid)
          return
        }
        if (row.requiresCredentials && !row.isKnown) {
          root.openPasswordPrompt(row.net.ssid)
          return
        }
        root.connectDirectly(row.net.ssid)
      }
    }

    // The signal. On the inverse block the unlit arcs are left out.
    Sprite {
      x: row.pad
      y: row.g.hair + row.g.onCaps(7)
      rows: Sprites.wifi
      level: root.wifiLevel(row.net ? row.net.signal : -1)
      color: row.isFailed ? Role.alarm : row.ink
      dim: row.isConnected ? "transparent" : Role.faint
    }

    Column {
      x: row.textX
      y: row.g.hair
      Repeater {
        model: row.lines
        PixelText {
          required property string modelData
          text: modelData
          ink: row.ink
        }
      }
      PixelText {
        visible: row.statusText !== ""
        text: row.statusText
        ink: row.statusInk
        columns: row.columns
      }
    }

    Sprite {
      visible: row.requiresCredentials
      x: row.lockX
      y: row.g.hair + row.g.onCaps(7)
      rows: Sprites.lock
      color: row.isConnected ? Role.onAccent : Role.muted
    }

    QButton {
      id: forgetButton
      visible: row.canForget
      x: row.lockX - row.g.px(3) - width
      y: row.g.hair
      icon: icons.forget
      danger: true
      enabledState: !root.busy
      hasCursor: row.forgetFocused
      onHovered: function(on) {
        if (!on) return
        root.cursorActive = true
        root.focusSection = "wifi"
        root.selectedIndex = row.rowIndex
        root.wifiActionFocused = true
      }
      onClicked: if (row.net) root.forget(row.net)
      QTip {
        text: "Forget network"
        shown: forgetButton.hot || row.forgetFocused
        x: -width - row.g.px(2)
        y: 0
      }
    }

    Loader {
      width: parent.width
      height: row.bodyHeight
      active: row.hasCursor
      sourceComponent: Brackets { arm: 3; color: row.isConnected ? Role.ink : Role.accent }
    }

    // Inline passphrase prompt — shown when we hit a protected network we
    // don't have saved credentials for, or when a connect fails because the
    // saved passphrase is wrong. Submitting (Enter or Connect) fires connect;
    // Esc or Cancel closes it.
    Item {
      id: prompt
      visible: row.isPasswordOpen
      x: row.textX
      y: row.bodyHeight + row.g.px(2)
      width: row.width - x - row.pad
      height: (idField.visible ? row.g.px(14 + 2) : 0) + row.g.px(14)

      QField {
        id: idField
        visible: row.isEnterprise && !row.isBusy && !row.isFailed
        width: connectButton.x - row.g.px(3)
        placeholderText: "Identity (user@domain)"
        text: row.isPasswordOpen ? root.identityText : ""

        onAccepted: pwField.forceActiveFocus()
        onTextChanged: if (row.isPasswordOpen && text !== root.identityText) root.identityText = text
        onKeyPressed: function(event) {
          if (event.key === Qt.Key_Escape) { root.cancelPasswordPrompt(); event.accepted = true }
        }

        onVisibleChanged: if (visible) Qt.callLater(function() { idField.forceActiveFocus() })
        Component.onCompleted: if (visible) Qt.callLater(function() { idField.forceActiveFocus() })
      }

      QField {
        id: pwField
        visible: !row.isBusy && !row.isFailed
        y: idField.visible ? row.g.px(14 + 2) : 0
        width: connectButton.x - row.g.px(3)
        password: true
        placeholderText: "Passphrase"
        text: row.isPasswordOpen ? root.passwordText : ""

        onAccepted: row.submitCredentials()
        onTextChanged: if (row.isPasswordOpen && text !== root.passwordText) root.passwordText = text
        onKeyPressed: function(event) {
          if (event.key === Qt.Key_Escape) { root.cancelPasswordPrompt(); event.accepted = true }
        }

        onVisibleChanged: if (visible && !row.isEnterprise) Qt.callLater(function() { pwField.forceActiveFocus() })
        Component.onCompleted: if (visible && !row.isEnterprise) Qt.callLater(function() { pwField.forceActiveFocus() })
      }

      QButton {
        id: cancelButton
        visible: !row.isBusy && !row.isFailed
        x: prompt.width - width
        y: pwField.y + row.g.hair
        text: "Cancel"
        onClicked: root.cancelPasswordPrompt()
      }

      QButton {
        id: connectButton
        visible: !row.isBusy && !row.isFailed
        x: cancelButton.x - row.g.px(3) - width
        y: pwField.y + row.g.hair
        text: "Connect"
        enabledState: !!(row.net && pwField.text.length > 0 && (!row.isEnterprise || idField.text.length > 0))
        onClicked: row.submitCredentials()
      }

      // While it connects, or after it failed: the state in the field's box.
      Item {
        visible: row.isBusy || row.isFailed
        width: parent.width
        height: row.g.px(14)
        Rectangle {
          anchors.fill: parent
          antialiasing: false
          color: Role.edge
          Rectangle {
            anchors.fill: parent
            anchors.margins: row.g.hair
            antialiasing: false
            color: Role.ground
          }
        }
        PixelText {
          x: row.g.px(3)
          y: row.g.hair
          text: row.isFailed ? "Wrong password" : "Connecting…"
          ink: row.isFailed ? Role.alarm : Role.ink
        }
      }
    }
  }
}
