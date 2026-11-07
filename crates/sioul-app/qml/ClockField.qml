// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A time of day, by keyboard alone or by pointer alone: every form's one field
// for a time (the unified agenda, part 5; a medicine's takes are picked from
// lists, TimeField.qml). Its form reads `text`, "14:30", as from the field it
// replaces, and hears `textEdited`, `accepted` and `editingFinished`.
//
// Keys: `1430`, `14:30`, `14h30`, `930` (09:30), `14` then Tab (14:00), read by
// `readClock` when the field is left. What does not read ("25:00") is left as
// typed, for its form to say. Up and Down move it by five minutes, to the next
// whole five, and Page Up and Page Down by an hour, around midnight; an end
// field (`end`) stays within its day. Alt+Down opens the panel, and Tab goes
// past the button and never enters the grids: the field does all they do.
// A step edits the text as typing does, so a `text:` binding holds: over the
// whole text selected, `insert` writes through the input mask, as a key typed
// does, which every use gives ("99:99"); without a mask it would add to the
// text, so a field without one is written by assignment.
//
// Pointer: the clock button at the field's end opens the panel, as DateField's
// calendar button opens its month. 24 hours, 12 minute steps, -1 and +1. Each
// click applies at once, an hour with no minutes yet gives :00, a click outside
// keeps the value, Esc gives back the earlier one, and there is no OK. On a
// computer the panel sits beside the field, or under it where the window is too
// narrow, with the focus left in the field, so that typing goes on beside it, and
// a press on the form both closes it and reaches what it was aimed at. On a touch
// screen it is a bottom sheet of cells a fingertip wide, the two grids side by
// side where the sheet is wide enough, and the soft keyboard stays away until the
// field itself is tapped. A form that saves on `editingFinished` hears it once,
// when the panel closes.
//
// A screen reader finds a spin button: its readable value in `value` (Qt reads
// a property of that name for this role), its steps by the increase and decrease
// actions; the cells are radio buttons in two named groups. Without `theme`
// there is no panel and the field is typed only. docs/design.md says why a grid
// over a dial or wheels, on what evidence.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

TextField {
    id: field

    // An end of something (its "to", its "until"): Up stops at 23:55 and Down at 00:00,
    // never around midnight to the same day's start (review of 21436bf, later 10).
    property bool end: false
    // The window's colours, sizes and fonts (Theme.qml).
    property var theme: null
    // The window's words, for the names a screen reader says and the button's tip.
    property var sioul: null
    // The locale the cells are labelled in: 24 cells in any case, "12 AM" to "11 PM"
    // where its short time has a period. The field itself stays on "14:30".
    property var timeLocale: field.sioul ? Qt.locale(field.sioul.text("qt-locale")) : Qt.locale()

    // What the text reads as, half typed or not ("14:30"); "" while it reads as no time.
    readonly property string clock: field.readClock(field.text)
    // Its hour and minute, -1 while it reads as no time.
    readonly property int hour: field.clock === "" ? -1 : parseInt(field.clock.slice(0, 2), 10)
    readonly property int minute: field.clock === "" ? -1 : parseInt(field.clock.slice(3), 10)
    readonly property bool twelveHours: field.timeLocale.timeFormat(Locale.ShortFormat).toLowerCase().indexOf("ap") >= 0
    // Fingers rather than a mouse: a bottom sheet with no keyboard, cells a fingertip wide.
    readonly property bool sheet: field.theme !== null && field.theme.touch === true
    // Numbers said with their units, in your language; without words, the numbers alone.
    readonly property bool hasWords: field.sioul !== null && typeof field.sioul.textCounted === "function"
    // What a screen reader says of the field: "14 hours 30".
    readonly property string value: field.spoken(field.hour, field.minute)
    // The panel, once made (null until it first opens), and whether it shows.
    readonly property Popup panel: panelMaker.item as Popup
    readonly property bool panelOpen: field.panel !== null && field.panel.visible
    // The text when the panel opened, for Esc.
    property string earlier: ""

    // A number on two digits, "05".
    function pad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    // A time as typed, read as "14:30"; "" when it reads as no time. "1430", "14:30",
    // "14h30", "930" (three digits are H:MM), "14", "14:", "9:30". An input mask leaves
    // blanks and its colon in what was typed: "93:0" is 930 and "14:" is 14. One minute
    // digit after a separator of its own ("9:5") could mean :05 or :50 and reads as no time.
    function readClock(typed) {
        const kept = String(typed).replace(/[\s_]/g, "")
        let parts = /^(\d{1,2})(?:[:hH.](\d\d)?)?$/.exec(kept)
        let hours = parts ? parseInt(parts[1], 10) : -1
        let minutes = parts ? parseInt(parts[2] || "0", 10) : -1
        if (!parts) {
            parts = /^(\d)(\d):?(\d)$/.exec(kept) || /^()(\d\d):?(\d\d)$/.exec(kept)
            if (!parts)
                return ""
            const three = parts[1] !== ""
            hours = parseInt(three ? parts[1] : parts[2], 10)
            minutes = parseInt(three ? parts[2] + parts[3] : parts[3], 10)
        }
        return hours > 23 || minutes > 59 ? "" : field.pad(hours) + ":" + field.pad(minutes)
    }

    // An hour's cell: "14", or "2 PM" where the locale counts twelve.
    function hourLabel(hour) {
        if (!field.twelveHours)
            return field.pad(hour)
        return ((hour + 11) % 12 + 1) + " " + (hour < 12 ? field.timeLocale.amText : field.timeLocale.pmText)
    }

    // An hour as a screen reader says it, "14 hours", or "2 PM" where the locale counts twelve.
    function hourName(hour) {
        if (field.twelveHours)
            return field.hourLabel(hour)
        return field.hasWords ? field.sioul.textCounted("clock-hour-name", hour) : field.pad(hour)
    }

    // A minute step as a screen reader says it.
    function minuteName(minute) {
        return field.hasWords ? field.sioul.textCounted("clock-minute-name", minute) : field.pad(minute)
    }

    // A time as a screen reader says it: "14 hours 30", "14 hours", "2:30 PM". "" for no time.
    function spoken(hour, minute) {
        if (hour < 0)
            return ""
        if (field.twelveHours)
            return ((hour + 11) % 12 + 1) + ":" + field.pad(minute) + " " + (hour < 12 ? field.timeLocale.amText : field.timeLocale.pmText)
        return field.hourName(hour) + (minute > 0 ? " " + field.pad(minute) : "")
    }

    // Puts `time` in the field, as typing does: the form hears `textEdited`, a binding holds.
    function write(time) {
        if (field.inputMask === "") {
            field.text = time
            field.textEdited()
            return
        }
        if (time.length !== 5) {
            // A half-typed text given back (Esc): `insert` over a whole text would leave the rest.
            field.clear()
            field.insert(0, time)
            field.textEdited()
            return
        }
        // Through the field's own editing, never an assignment: a `text:` binding holds, and
        // the field follows its model when that changes (review of 21436bf, later 10). The
        // edit says itself as typed (`textEdited`).
        field.selectAll()
        field.insert(0, time)
        field.deselect()
        field.cursorPosition = field.length
    }

    // What was typed read whole: "14:" becomes "14:00". A field left half typed hears no
    // `editingFinished` from Qt, so whoever calls this says it when it returns true.
    function settle() {
        if (field.inputMask !== "" && field.acceptableInput)
            return false
        const read = field.readClock(field.text)
        if (read === "" || read === field.text)
            return false
        field.write(read)
        return true
    }

    // The text as minutes from midnight when it reads whole; -1 while "9:" or "25:00".
    function wholeMinutes() {
        const parts = field.text.split(":")
        if (parts.length !== 2 || parts[0].trim().length !== 2 || parts[1].trim().length !== 2)
            return -1
        const hour = parseInt(parts[0], 10)
        const minute = parseInt(parts[1], 10)
        return isNaN(hour) || isNaN(minute) || hour > 23 || minute > 59 ? -1 : hour * 60 + minute
    }

    // `at` minutes from midnight, written. It goes around midnight, but an end field stays
    // between 00:00 and `top` (or where it is, typed past `top`).
    function moveTo(now, at, top) {
        const moved = field.end ? Math.max(0, Math.min(Math.max(top, now), at)) : (at + 1440) % 1440
        if (moved !== now)
            field.write(field.pad(Math.floor(moved / 60)) + ":" + field.pad(moved % 60))
    }

    // Five minutes later (`step` 1) or earlier (-1), from the time typed; what does not
    // read whole ("9:", "25:00") stays as typed.
    function stepBy(step) {
        const now = field.wholeMinutes()
        if (now >= 0)
            field.moveTo(now, step > 0 ? Math.floor(now / 5) * 5 + 5 : Math.ceil(now / 5) * 5 - 5, 23 * 60 + 55)
    }

    // An hour later (`step` 1) or earlier (-1), on the same minute.
    function hourBy(step) {
        const now = field.wholeMinutes()
        if (now >= 0)
            field.moveTo(now, now + step * 60, 23 * 60 + 55)
    }

    // One minute later or earlier, for the panel's -1 and +1: an end field reaches 23:59.
    function minuteBy(step) {
        const now = field.hour * 60 + field.minute
        if (field.hour >= 0)
            field.moveTo(now, now + step, 23 * 60 + 59)
    }

    // An hour's cell clicked: on the minutes there are, else :00.
    function pickHour(hour) {
        const time = field.pad(hour) + ":" + field.pad(Math.max(0, field.minute))
        if (time !== field.text)
            field.write(time)
    }

    // A minute's cell clicked; it waits for an hour, it never makes one up.
    function pickMinute(minute) {
        const time = field.pad(field.hour) + ":" + field.pad(minute)
        if (field.hour >= 0 && time !== field.text)
            field.write(time)
    }

    function openPanel() {
        if (field.theme === null || !field.enabled)
            return
        panelMaker.active = true
        if (field.panel.visible)
            return
        field.settle()
        field.earlier = field.text
        // A phone's keyboard comes with the focus, and comes back with it when the sheet closes.
        if (field.sheet)
            field.focus = false
        else
            field.forceActiveFocus()
        field.panel.open()
    }

    function closePanel() {
        if (field.panelOpen)
            field.panel.close()
    }

    // Esc: the earlier value back, the panel closed.
    function cancelPanel() {
        if (field.text !== field.earlier)
            field.write(field.earlier)
        field.closePanel()
    }

    // A day was just picked beside this field (DateField's `hourField`): on to the hour.
    // Typing goes to the field at once and the pointer carries on in the panel; a phone
    // opens its sheet alone. A field with no hour yet (a task's "No hour") takes the focus
    // and stays closed: a day alone is a whole answer there.
    function moveHere() {
        if (!field.visible || !field.enabled)
            return
        if (!field.sheet)
            field.forceActiveFocus()
        if (field.clock !== "")
            field.openPanel()
    }

    // As wide as a time and the button, whatever a form's old width said: "23:59" is never cut.
    implicitWidth: Math.ceil(digits.advanceWidth("00:00")) + leftPadding + rightPadding + 2
    Layout.minimumWidth: implicitWidth
    leftPadding: 8
    rightPadding: field.theme !== null ? clockButton.width + 2 : 8
    inputMask: "99:99"
    inputMethodHints: Qt.ImhPreferNumbers
    font.features: { "tnum": 1 }
    Accessible.role: Accessible.SpinBox
    Accessible.onIncreaseAction: field.stepBy(1)
    Accessible.onDecreaseAction: field.stepBy(-1)
    // The field activated by a screen reader, as tapped: the keyboard's turn.
    Accessible.onToggleAction: field.forceActiveFocus()

    // Into the field the whole text is selected, so that the first digit starts over: the
    // mask keeps every place and a cursor at its end would swallow what is typed. A click's
    // own cursor comes after the focus, hence later. Out of it, what was typed is read.
    onActiveFocusChanged: {
        if (field.activeFocus) {
            if (field.focusReason === Qt.TabFocusReason || field.focusReason === Qt.BacktabFocusReason || field.focusReason === Qt.OtherFocusReason)
                field.selectAll()
            else if (field.focusReason === Qt.MouseFocusReason)
                Qt.callLater(field.selectAll)
            return
        }
        if (field.settle())
            field.editingFinished()
        if (!field.sheet)
            field.closePanel()
    }

    // The form closed or the field hidden, the panel goes with it.
    onVisibleChanged: {
        if (!field.visible)
            field.closePanel()
    }

    // Esc belongs to the panel while it is open, not to the dialog around the field.
    Keys.onShortcutOverride: event => event.accepted = field.panelOpen && event.key === Qt.Key_Escape
    Keys.onPressed: event => {
        const alt = (event.modifiers & Qt.AltModifier) !== 0
        if (event.key === Qt.Key_Down && alt)
            field.openPanel()
        else if (event.key === Qt.Key_Up)
            field.stepBy(1)
        else if (event.key === Qt.Key_Down)
            field.stepBy(-1)
        else if (event.key === Qt.Key_PageUp)
            field.hourBy(1)
        else if (event.key === Qt.Key_PageDown)
            field.hourBy(-1)
        else if (event.key === Qt.Key_Escape && field.panelOpen)
            field.cancelPanel()
        else {
            // Enter reads what was typed first: Qt then says `accepted`, whole. A digit with
            // the mask full starts over, or it would go nowhere (after a click, a step, a time typed).
            if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                field.settle()
                field.closePanel()
            } else if (/^\d$/.test(event.text) && field.inputMask !== "" && field.cursorPosition === field.length && field.selectedText === "") {
                field.selectAll()
            }
            event.accepted = false
            return
        }
        event.accepted = true
    }

    FontMetrics {
        id: digits

        font: field.font
    }

    ToolButton {
        id: clockButton

        objectName: "clockButton"
        visible: field.theme !== null
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: field.sheet ? 34 : 26
        padding: 2
        focusPolicy: Qt.NoFocus
        activeFocusOnTab: false
        display: AbstractButton.IconOnly
        icon.name: "clock"
        icon.width: 16
        icon.height: 16
        icon.color: field.theme !== null ? field.theme.text : "transparent"
        Accessible.name: field.sioul ? field.sioul.text("clock-pick") : ""
        ToolTip.visible: hovered && field.sioul !== null
        ToolTip.text: field.sioul ? field.sioul.text("clock-pick") : ""
        onClicked: field.panelOpen ? field.closePanel() : field.openPanel()
    }

    // Made the first time it opens, then kept: a page of forms holds many times.
    Loader {
        id: panelMaker

        active: false

        sourceComponent: Component {
            SafePopup {
                id: panel

                // Room between two cells, and around the grids.
                readonly property int cellGap: field.sheet ? 4 : 2
                readonly property int edge: 8
                // A fingertip across on the narrowest phone, on two grids where they fit.
                readonly property bool sideBySide: field.sheet && panel.roomWidth >= 2 * (6 * 44 + 5 * panel.cellGap) + 3 * panel.edge
                readonly property real cellWidth: field.sheet ? Math.floor(Math.min(64, ((panel.sideBySide ? (panel.roomWidth - panel.edge) / 2 : panel.roomWidth) - 2 * panel.edge - 5 * panel.cellGap) / 6)) : (field.twelveHours ? 46 : 36)
                readonly property real cellHeight: field.sheet ? 44 : 32
                // Beside the field where the window has room, or else under it: the form's next
                // row stays in view and in reach, an event's end under its start, which follows it.
                property bool beside: false

                parent: field.sheet ? Overlay.overlay : field
                // A sheet stands alone over the dimmed form. Under the field, the form stays
                // in reach: the press that closes the panel also reaches what is under it.
                modal: field.sheet
                dim: field.sheet
                focus: field.sheet
                safeGap: field.sheet ? 0 : 8
                x: field.sheet ? (panel.parent.width - panel.width) / 2 : panel.beside ? field.width + 4 : 0
                y: field.sheet ? panel.parent.height - panel.height - (panel.safeArea ? panel.safeArea.bottom : 0) : panel.beside ? 0 : field.height + 2
                width: field.sheet ? panel.roomWidth : implicitWidth
                padding: panel.edge
                // A press on the field or its button leaves the panel to them: the text is
                // edited beside it, and the button closes what it opened.
                closePolicy: field.sheet ? Popup.CloseOnPressOutside : Popup.CloseOnPressOutsideParent

                onAboutToShow: {
                    const overlay = panel.Overlay.overlay
                    panel.beside = !field.sheet && overlay !== null && field.mapToItem(null, field.width, 0).x + 4 + panel.implicitWidth + panel.safeGap <= overlay.width
                }

                // Said here rather than for each way of closing: a form that saves on
                // `editingFinished` hears the value kept, and Esc leaves nothing to say.
                onClosed: {
                    if (field.text !== field.earlier)
                        field.editingFinished()
                }

                background: Rectangle {
                    color: field.theme.surface
                    border.color: field.theme.line
                    radius: field.theme.radius
                }

                contentItem: GridLayout {
                    focus: field.sheet
                    columns: panel.sideBySide ? 2 : 1
                    columnSpacing: panel.edge
                    rowSpacing: panel.edge

                    // A sheet holds the focus: Esc, and a phone's Back, give back the earlier one.
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Escape || event.key === Qt.Key_Back) {
                            field.cancelPanel()
                            event.accepted = true
                        }
                    }

                    Grid {
                        objectName: "clockHours"
                        Layout.alignment: Qt.AlignHCenter | Qt.AlignTop
                        columns: 6
                        spacing: panel.cellGap
                        Accessible.role: Accessible.Grouping
                        Accessible.name: field.sioul ? field.sioul.text("clock-hours") : ""

                        Repeater {
                            model: 24

                            delegate: ClockCell {
                                required property int index

                                objectName: "clockHour" + index
                                theme: field.theme
                                width: panel.cellWidth
                                height: panel.cellHeight
                                text: field.theme.plain(field.hourLabel(index))
                                chosen: field.hour === index
                                Accessible.role: Accessible.RadioButton
                                Accessible.name: field.hourName(index)
                                onClicked: field.pickHour(index)
                            }
                        }
                    }
                    ColumnLayout {
                        Layout.alignment: Qt.AlignHCenter | Qt.AlignTop
                        spacing: panel.edge

                        Grid {
                            objectName: "clockMinutes"
                            Layout.alignment: Qt.AlignHCenter
                            columns: 6
                            spacing: panel.cellGap
                            Accessible.role: Accessible.Grouping
                            Accessible.name: field.sioul ? field.sioul.text("clock-minutes") : ""

                            Repeater {
                                model: 12

                                delegate: ClockCell {
                                    required property int index

                                    objectName: "clockMinute" + index * 5
                                    theme: field.theme
                                    width: panel.cellWidth
                                    height: panel.cellHeight
                                    enabled: field.hour >= 0
                                    text: field.theme.plain(":" + field.pad(index * 5))
                                    chosen: field.minute === index * 5
                                    Accessible.role: Accessible.RadioButton
                                    Accessible.name: field.minuteName(index * 5)
                                    onClicked: field.pickMinute(index * 5)
                                }
                            }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: panel.cellGap

                            ClockCell {
                                objectName: "clockMinus"
                                theme: field.theme
                                filled: true
                                Layout.preferredWidth: panel.cellWidth
                                Layout.preferredHeight: panel.cellHeight
                                enabled: field.hour >= 0
                                text: "−1"
                                Accessible.role: Accessible.Button
                                Accessible.name: field.sioul ? field.sioul.text("clock-earlier") : text
                                onClicked: field.minuteBy(-1)
                            }
                            // The time as it stands: on a phone the field is under the sheet's dimmer.
                            Label {
                                objectName: "clockNow"
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                text: field.hour < 0 ? "––:––" : field.twelveHours ? field.spoken(field.hour, field.minute) : field.clock
                                textFormat: Text.PlainText
                                font.features: { "tnum": 1 }
                                color: field.theme.text
                                Accessible.ignored: true
                            }
                            ClockCell {
                                objectName: "clockPlus"
                                theme: field.theme
                                filled: true
                                Layout.preferredWidth: panel.cellWidth
                                Layout.preferredHeight: panel.cellHeight
                                enabled: field.hour >= 0
                                text: "+1"
                                Accessible.role: Accessible.Button
                                Accessible.name: field.sioul ? field.sioul.text("clock-later") : text
                                onClicked: field.minuteBy(1)
                            }
                        }
                    }
                }
            }
        }
    }

    // A cell of the panel, reached by pointer and screen reader alone: the field does by
    // keys all that the cells do, so no Tab stops here.
    component ClockCell: AbstractButton {
        id: cell

        // The window's colours (an inline component does not see the file's ids).
        required property var theme
        // The field's time is this one.
        property bool chosen: false
        // A button among the cells (-1, +1): filled, where a cell is a place to pick.
        property bool filled: false

        focusPolicy: Qt.NoFocus
        activeFocusOnTab: false
        padding: 0
        font.features: { "tnum": 1 }
        Accessible.checkable: Accessible.role === Accessible.RadioButton
        Accessible.checked: cell.chosen
        // A radio button is toggled by a screen reader, where a button is pressed: both are a click.
        Accessible.onToggleAction: if (cell.enabled) cell.clicked()
        Accessible.onPressAction: if (cell.enabled) cell.clicked()

        contentItem: Label {
            text: cell.text
            textFormat: Text.PlainText
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            font: cell.font
            opacity: cell.enabled ? 1 : 0.4
            color: cell.chosen ? cell.theme.accentText : cell.theme.text
        }
        background: Rectangle {
            radius: cell.theme.radius
            color: cell.chosen ? cell.theme.accent : cell.down ? cell.theme.pressed : cell.hovered ? cell.theme.hover : cell.filled ? cell.theme.button : "transparent"
        }
    }
}
