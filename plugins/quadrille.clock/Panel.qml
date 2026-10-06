import QtQuick
import Quickshell
import qs.Commons
import qs.Ui
import "Model.js" as Model
import "Q"

// The clock's calendar popup: a month grid with ISO week numbers, built to
// sit beside the weather panel — same hero-over-detail composition, same
// spacing scale, same small-caps labels.
//
// The grid is a read-out rather than a picker: today is the only marked
// day, and the only thing that moves is which month is on screen —
// chevrons, the scroll wheel, and the arrow keys all step it.
//
// BarWidget.qml owns the bar label and hands this panel the button to
// anchor against.
Panel {
  id: root
  moduleName: "omarchy.clock"
  ipcTarget: "omarchy.clock"
  manageIpc: false

  property var anchorItem: null

  // The bar tracks the widget mounted in its slot — BarWidget.qml — not this
  // nested panel. Everything the bar identifies a panel by has to be that
  // widget: the popout coordinator (and with it the open-panel dot under the
  // pill) compares against `slot.activeItem`, and switchPanelFrom looks the
  // slot up the same way.
  property var hostWidget: null
  readonly property var barIdentity: hostWidget || root

  // ---- Today. SystemClock keeps this honest across midnight so the
  //      highlight rolls over without the panel being reopened.
  property date today: new Date()
  readonly property string todayKey: Model.keyForDate(today)

  // The month on screen. Stepping moves this and nothing else: the grid is
  // a read-out, not a picker, so there is no per-day cursor to keep in sync.
  property int viewYear: today.getFullYear()
  property int viewMonth: today.getMonth()

  readonly property date viewDate: new Date(viewYear, viewMonth, 1)
  readonly property bool viewingCurrentMonth: viewYear === today.getFullYear() && viewMonth === today.getMonth()

  // Pinned to today, not to the month being browsed — stepping through the
  // calendar does not change how much of the year is gone.
  readonly property real yearDone: Model.yearProgress(today.getFullYear(), today.getMonth(), today.getDate())
  readonly property int yearDonePercent: Model.yearProgressPercent(today.getFullYear(), today.getMonth(), today.getDate())

  // Memento mori, for anyone who goes looking: double-tapping the year bar
  // asks for a birth year and a life expectancy, and a second bar tracks one
  // against the other. A birth year rather than an age, so it keeps counting
  // on its own. Without one the bar stays hidden.
  readonly property int birthYear: Model.parseBirthYear(setting("birthYear", 0), today.getFullYear())
  readonly property int age: Model.ageFromBirthYear(birthYear, today.getFullYear())
  readonly property int lifeExpectancy: Model.parseLifeExpectancy(setting("lifeExpectancy", 0))
  readonly property real lifeDone: Model.lifeProgress(age, lifeExpectancy)
  readonly property int lifeDonePercent: Model.lifeProgressPercent(age, lifeExpectancy)
  property bool editingLife: false

  // Unset falls through to the locale's own first day, so a fresh install
  // starts out matching the rest of the desktop rather than a hardcoded
  // convention. Clicking the grid's "W" heading writes the choice back to
  // shell.json.
  readonly property int weekStart: Model.normalizedWeekStart(setting("weekStartDay", null), Qt.locale().firstDayOfWeek)
  // The interface is English throughout, so day names are not taken from the
  // system locale. Where the week starts still is: that is a regional
  // convention rather than a translation, and it stays overridable above.
  readonly property var labelLocale: Qt.locale("en_US")
  readonly property string nextWeekStartLabel: labelLocale.dayName(Model.toggledWeekStart(weekStart), Locale.LongFormat)
  readonly property var weekdays: Model.weekdayOrder(weekStart)
  readonly property var weeks: Model.monthGrid(viewYear, viewMonth, weekStart, todayKey)


  // Guarded so the widget renders before the bar is injected (the bar-widget
  // contract instantiates it bare).
  readonly property color contentForeground: bar ? bar.foreground : Color.foreground
  readonly property string contentFontFamily: bar ? bar.fontFamily : Style.font.family

  readonly property int cellWidth: Style.space(52)
  readonly property int cellHeight: Style.space(34)
  readonly property int cellSpacing: Style.space(2)
  readonly property int weekColumnWidth: Style.space(32)
  readonly property int gutterWidth: Style.space(14)

  function open() {
    refresh()
    root.controller.show()
    // Set after showing, not before: showing hands the popout coordinator
    // over, which closes whichever panel was open, and that close clears the
    // shared flag. Deferring means the panel taking over always wins, while
    // a handoff to a panel that does not manage the flag still leaves it
    // cleared rather than stuck on.
    Qt.callLater(function() {
      if (root.opened) setCenterHoverRevealSuppressed(true)
    })
  }

  function close() {
    setCenterHoverRevealSuppressed(false)
    // Dismissing the panel mid-edit would otherwise leave the inputs up,
    // waiting behind a closed popup for the next time it opens.
    if (root.editingLife) root.cancelEditingLife()
    root.controller.hide()
  }

  function toggle() {
    if (root.opened) root.close()
    else root.open()
  }

  function switchPanel(direction) {
    if (root.bar && typeof root.bar.switchPanelFrom === "function")
      return root.bar.switchPanelFrom(root.barIdentity, direction)
    return false
  }

  // Summoning by hotkey moves no pointer, so a hover the bar was still
  // holding must not keep the center indicators revealed behind the panel.
  function setCenterHoverRevealSuppressed(value) {
    if (root.bar && typeof root.bar.setCenterHoverRevealSuppressed === "function")
      root.bar.setCenterHoverRevealSuppressed(value)
    else if (root.bar && "centerHoverRevealSuppressed" in root.bar)
      root.bar.centerHoverRevealSuppressed = value
  }

  function refresh() {
    root.today = new Date()
    root.goToToday()
  }

  function goToToday() {
    root.viewYear = today.getFullYear()
    root.viewMonth = today.getMonth()
  }

  function moveMonth(delta) {
    var next = Model.stepMonth(viewYear, viewMonth, delta)
    root.viewYear = next.year
    root.viewMonth = next.month
  }

  function moveYear(delta) {
    moveMonth(delta * 12)
  }

  // Applied locally first so the panel redraws on the click itself; the
  // shell.json write comes back through the bar as the same value. With no
  // writable entry (the widget is not in the layout) it stays a session-only
  // preference rather than doing nothing. The host widget builds its own
  // entry when the label format is cycled, so it has to be kept in step or
  // it would write this key straight back out from a stale copy.
  function persistSettings(values) {
    var entry = { id: root.moduleName }
    for (var existing in root.settings) if (existing !== "id") entry[existing] = root.settings[existing]
    for (var key in values) entry[key] = values[key]

    root.settings = entry
    if (root.hostWidget && "settings" in root.hostWidget) root.hostWidget.settings = entry
    if (root.bar && root.bar.shell && typeof root.bar.shell.updateEntryInline === "function")
      root.bar.shell.updateEntryInline(root.moduleName, entry)
  }

  function setWeekStart(day) {
    var next = Model.normalizedWeekStart(day, root.weekStart)
    if (next === root.weekStart) return
    persistSettings({ weekStartDay: Model.weekStartSettingName(next) })
  }

  function startEditingLife() {
    root.editingLife = true
    Qt.callLater(function() {
      bornField.text = root.birthYear > 0 ? String(root.birthYear) : ""
      expectancyField.text = String(root.lifeExpectancy)
      bornField.selectAll()
      bornField.forceActiveFocus()
    })
  }

  function cancelEditingLife() {
    root.editingLife = false
    Qt.callLater(function() { if (keyCatcher) keyCatcher.forceActiveFocus() })
  }

  // Shared by both fields: Tab hops to the other one, Enter commits the pair,
  // Escape drops the lot.
  function handleLifeKey(event, other) {
    if (event.key === Qt.Key_Escape) {
      root.cancelEditingLife()
      event.accepted = true
    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
      root.commitLife()
      event.accepted = true
    } else if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
      other.selectAll()
      other.forceActiveFocus()
      event.accepted = true
    }
  }

  // Double-tapping the life bar puts it away again. The expectancy stays in
  // the config so setting a birth year again brings your own number back
  // rather than the default.
  function clearLife() {
    if (root.birthYear <= 0) return
    persistSettings({ birthYear: 0 })
  }

  function commitLife() {
    var born = Model.parseBirthYear(bornField.text, today.getFullYear())
    var span = Model.parseLifeExpectancy(expectancyField.text)
    if (born !== root.birthYear || span !== root.lifeExpectancy)
      persistSettings({ birthYear: born, lifeExpectancy: span })
    cancelEditingLife()
  }

  function toggleWeekStart() {
    setWeekStart(Model.toggledWeekStart(root.weekStart))
  }

  // English short day names, matching the rest of the interface.
  function weekdayLabel(weekday) {
    return String(labelLocale.dayName(weekday, Locale.ShortFormat)).toUpperCase()
  }

  SystemClock {
    id: clock
    precision: SystemClock.Minutes
    onDateChanged: {
      if (Model.keyForDate(clock.date) === String(root.todayKey)) return
      var followToday = root.viewingCurrentMonth
      root.today = clock.date
      if (followToday) root.goToToday()
    }
  }

  Icons { id: icons }

  // Geometry of the grid, in virtual pixels: a day is 20 x 14 (two figures and a
  // hair of air), a pitch of 22 across and 16 down; the week numbers have a
  // 16-wide column and a 6-wide gutter with a hairline in it.
  readonly property int dayW: 20
  readonly property int dayH: 14
  readonly property int pitchX: 22
  readonly property int pitchY: 16
  readonly property int weekW: 16
  readonly property int gutterW: 6
  readonly property int gridW: weekW + gutterW + 7 * pitchX - 2

  QPopup {
    id: panel
    anchorItem: root.anchorItem
    owner: root.barIdentity
    bar: root.bar
    open: root.opened
    centerOnBar: true
    focusTarget: keyCatcher
    contentWidth: panel.fittedContentWidth(panel.cardWidth(30))
    contentHeight: panel.fittedContentHeight(calendarColumn.implicitHeight)

    PanelKeyCatcher {
      id: keyCatcher
      anchors.fill: parent
      blocked: root.editingLife
      onMoveRequested: function(dx, dy) {
        if (dx !== 0) root.moveMonth(dx)
        if (dy !== 0) root.moveYear(dy)
      }
      onActivateRequested: root.goToToday()
      onCloseRequested: root.close()
      onTabRequested: function(direction) { root.switchPanel(direction) }
      onTextKey: function(t) {
        if (t === "[") root.moveMonth(-1)
        else if (t === "]") root.moveMonth(1)
        else if (t === "{") root.moveYear(-1)
        else if (t === "}") root.moveYear(1)
        else if (t === "t" || t === "T") root.goToToday()
        else if (t === "w" || t === "W") root.toggleWeekStart()
      }

      Column {
        id: calendarColumn
        width: parent.width
        spacing: panel.g.px(6)

        // ---- Hero: today, centred. Once the view has stepped back it is also
        //      the way home: clicking the date beats hunting for a reset button.
        Item {
          width: parent.width
          height: panel.g.px(24)

          Row {
            id: heroRow
            x: panel.g.centre(parent.width, width)
            spacing: panel.g.px(6)

            Sprite {
              rows: icons.calendar15
              accent: Role.accent
              color: heroMouse.containsMouse ? Role.accent : Role.ink
              y: panel.g.centre(24 * panel.g.unit, height)
            }
            BigText {
              text: Qt.formatDate(root.today, "MMMM d")
              ink: heroMouse.containsMouse ? Role.accent : Role.ink
            }
          }

          MouseArea {
            id: heroMouse
            x: heroRow.x; y: heroRow.y
            width: heroRow.width; height: heroRow.height
            enabled: !root.viewingCurrentMonth
            hoverEnabled: enabled
            cursorShape: Qt.PointingHandCursor
            onClicked: root.goToToday()

            QTip { shown: heroMouse.containsMouse; text: "Back to today" }
          }
        }

        // ---- Year progress: the year as a stepped bar, days done over days in
        //      the year. Double-tap asks for a birth year and a lifespan.
        Item {
          id: yearBlock
          x: panel.g.centre(parent.width, root.gridW * panel.g.unit)
          width: root.gridW * panel.g.unit
          height: panel.g.line

          TapHandler {
            enabled: !root.editingLife
            onDoubleTapped: root.startEditingLife()
          }

          Row {
            visible: root.editingLife
            spacing: panel.g.px(3)

            PixelText { text: "BORN"; ink: Role.muted; y: panel.g.px(0) }
            QField {
              id: bornField
              width: panel.g.px(42)
              y: panel.g.centre(panel.g.line, height)
              placeholderText: "year"
              inputMethodHints: Qt.ImhDigitsOnly
              onKeyPressed: function(event) { root.handleLifeKey(event, expectancyField) }
            }
            Item { width: panel.g.px(3); height: 1 }
            PixelText { text: "LIVE TO"; ink: Role.muted }
            QField {
              id: expectancyField
              width: panel.g.px(30)
              y: panel.g.centre(panel.g.line, height)
              placeholderText: "90"
              inputMethodHints: Qt.ImhDigitsOnly
              onKeyPressed: function(event) { root.handleLifeKey(event, bornField) }
            }
          }

          Item {
            visible: !root.editingLife
            anchors.fill: parent

            PixelText { text: String(root.today.getFullYear()); ink: Role.muted }
            PixelText {
              x: parent.width - width
              text: root.yearDonePercent + "%"
              ink: Role.ink
            }
            QMeter {
              x: panel.g.px(4 * 6 + 4)
              y: panel.g.onCaps(3)
              width: parent.width - panel.g.px(2 * (4 * 6 + 4))
              value: root.yearDone
              fill: Role.accent
            }
          }
        }

        // ---- Memento mori: only here once someone has given an age.
        Item {
          visible: root.birthYear > 0
          x: panel.g.centre(parent.width, root.gridW * panel.g.unit)
          width: root.gridW * panel.g.unit
          height: visible ? panel.g.line : 0

          PixelText { text: "LIFE"; ink: Role.muted }
          PixelText {
            x: parent.width - width
            text: root.lifeDonePercent + "%"
            ink: Role.ink
          }
          QMeter {
            x: panel.g.px(4 * 6 + 4)
            y: panel.g.onCaps(3)
            width: parent.width - panel.g.px(2 * (4 * 6 + 4))
            value: root.lifeDone
            fill: Role.ink
          }
          TapHandler { onDoubleTapped: root.clearLife() }
          MouseArea {
            id: lifeMouse
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.NoButton
            QTip { shown: lifeMouse.containsMouse; text: "Memento Mori" }
          }
        }

        // ---- Month grid: week numbers down a gutter on the left, then the seven
        //      day columns. Always six rows, so the popup is exactly as tall in
        //      February as in August.
        Item {
          id: gridBlock
          width: parent.width
          height: (root.dayH + 2) * panel.g.unit + 6 * root.pitchY * panel.g.unit

          WheelHandler {
            onWheel: function(event) {
              // Horizontal wheels and touchpad side-scrolls report y === 0.
              if (event.angleDelta.y === 0) return
              root.moveMonth(event.angleDelta.y > 0 ? -1 : 1)
            }
          }

          Item {
            id: grid
            x: panel.g.centre(parent.width, root.gridW * panel.g.unit)
            width: root.gridW * panel.g.unit
            height: parent.height

            // The week-number heading doubles as the week-start toggle.
            Item {
              x: 0; y: 0
              width: root.weekW * panel.g.unit
              height: root.dayH * panel.g.unit
              Rectangle {
                anchors.fill: parent
                visible: weekStartMouse.containsMouse
                color: Role.raised
                antialiasing: false
              }
              PixelText {
                x: panel.g.centre(parent.width, width)
                y: panel.g.centre(parent.height, height)
                text: "W"
                ink: weekStartMouse.containsMouse ? Role.ink : Role.faint
              }
              MouseArea {
                id: weekStartMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.toggleWeekStart()
              }
              QTip { shown: weekStartMouse.containsMouse; text: "Start weeks on " + root.nextWeekStartLabel }
            }

            Repeater {
              model: root.weekdays
              PixelText {
                required property var modelData
                required property int index
                x: (root.weekW + root.gutterW + index * root.pitchX) * panel.g.unit
                y: panel.g.centre(root.dayH * panel.g.unit, height)
                text: root.weekdayLabel(modelData)
                ink: Role.muted
              }
            }

            // Hairline down the week-number gutter, beside the day rows only.
            Rectangle {
              x: (root.weekW + root.gutterW / 2) * panel.g.unit
              y: root.pitchY * panel.g.unit
              width: panel.g.hair
              height: 6 * root.pitchY * panel.g.unit - 2 * panel.g.unit
              color: Role.edge
              antialiasing: false
            }

            Repeater {
              model: root.weeks
              Item {
                id: weekRow
                required property var modelData
                required property int index
                y: (index + 1) * root.pitchY * panel.g.unit
                width: parent.width
                height: root.dayH * panel.g.unit

                PixelText {
                  x: panel.g.floor((root.weekW * panel.g.unit - width) / 2)
                  y: panel.g.centre(parent.height, height)
                  text: String(weekRow.modelData.week)
                  ink: Role.faint
                }

                Repeater {
                  model: weekRow.modelData.days
                  Item {
                    id: dayCell
                    required property var modelData
                    required property int index
                    x: (root.weekW + root.gutterW + index * root.pitchX) * panel.g.unit
                    width: root.dayW * panel.g.unit
                    height: root.dayH * panel.g.unit

                    // Today is the inverse block.
                    Rectangle {
                      anchors.fill: parent
                      visible: dayCell.modelData.today
                      color: Role.accent
                      antialiasing: false
                    }
                    PixelText {
                      x: panel.g.centre(parent.width, width)
                      y: panel.g.centre(parent.height, height)
                      text: String(dayCell.modelData.day)
                      ink: dayCell.modelData.today ? Role.onAccent
                        : (dayCell.modelData.inMonth ? (dayCell.modelData.weekend ? Role.muted : Role.ink) : Role.faint)
                    }
                  }
                }
              }
            }
          }
        }

        // ---- Month stepping, spanning the grid it drives.
        Item {
          x: panel.g.centre(parent.width, root.gridW * panel.g.unit)
          width: root.gridW * panel.g.unit
          height: panel.g.px(14)

          QButton {
            icon: Sprites.chevronLeft
            fixedWidth: panel.g.px(20)
            onClicked: root.moveMonth(-1)
            QTip { shown: parent.hot; text: "Previous month" }
          }
          PixelText {
            x: panel.g.centre(parent.width, width)
            y: panel.g.centre(parent.height, height)
            text: Qt.formatDate(root.viewDate, "MMMM yyyy").toUpperCase()
            ink: Role.muted
          }
          QButton {
            x: parent.width - width
            icon: Sprites.chevronRight
            fixedWidth: panel.g.px(20)
            onClicked: root.moveMonth(1)
            QTip { shown: parent.hot; text: "Next month" }
          }
        }
      }
    }
  }
}
