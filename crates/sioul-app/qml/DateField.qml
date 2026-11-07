// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A date, typed as 2026-10-05 or picked from a month: the calendar button or
// Alt+Down opens the month, a click on a day closes it, a click outside keeps
// the date and Esc gives back the earlier one, as ClockField's panel does. Under
// the field on a computer, the focus left in the field; a bottom sheet on a touch
// screen, with no soft keyboard. Tab goes past the button: the field does all it
// does. Beside an hour (`hourField`), a day picked moves on to that hour.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: field

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The window's words, for the month arrows' names; without them, the month alone.
    property var sioul: null
    // Month and day names in Sioul's language.
    property var locale: Qt.locale()
    // "2026-10-05".
    property string date: ""
    // The calendar button's name, said on hover and to screen readers.
    property string pickLabel: ""
    // The month shown by the picker (month 0 is January).
    property int shownYear: 2026
    // The month the picker shows, 0 for January, in `shownYear`.
    property int shownMonth: 0
    // The ClockField beside this date, if any (EventDialog, the task form): a day picked moves on to it.
    property var hourField: null
    // The date when the month opened, for Esc.
    property string earlier: ""
    // Fingers rather than a mouse: a bottom sheet with no keyboard.
    readonly property bool sheet: field.theme.touch === true
    // The month's popup, once made (null until it first opens), and whether it shows.
    readonly property Popup monthPopup: pickerMaker.item as Popup
    readonly property bool monthOpen: field.monthPopup !== null && field.monthPopup.visible

    // The date was typed or picked: the form saves it.
    signal edited

    // A number on two digits, "05".
    function pad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    // A date as 2026-10-05.
    function iso(d) {
        return d.getFullYear() + "-" + field.pad(d.getMonth() + 1) + "-" + field.pad(d.getDate())
    }

    // A month arrow's name, for a screen reader and its tip: "Previous month:
    // September 2026" (`step` -1), "Next month: November 2026" (+1).
    function arrowName(step) {
        const month = new Date(field.shownYear, field.shownMonth + step, 1).toLocaleDateString(field.locale, "MMMM yyyy")
        return field.sioul ? field.sioul.textWith(step < 0 ? "date-month-previous" : "date-month-next", "month", month) : month
    }

    function openMonth() {
        const shown = field.date ? new Date(field.date + "T12:00:00") : new Date(Date.now() + (field.sioul && typeof field.sioul.zoneOffset === "function" ? field.sioul.zoneOffset(Date.now() / 1000) + new Date(Date.now()).getTimezoneOffset() * 60000 : 0))
        field.shownYear = shown.getFullYear()
        field.shownMonth = shown.getMonth()
        field.earlier = field.date
        // A phone's keyboard comes with the focus, and comes back with it when the sheet closes.
        if (field.sheet)
            text.focus = false
        else
            text.forceActiveFocus()
        pickerMaker.active = true
        field.monthPopup.open()
    }

    // Esc: the earlier date back, the month closed.
    function cancelMonth() {
        if (field.date !== field.earlier) {
            field.date = field.earlier
            field.edited()
        }
        field.monthPopup.close()
    }

    implicitWidth: row.implicitWidth
    implicitHeight: row.implicitHeight

    // The form closed or the field hidden, the month goes with it.
    onVisibleChanged: {
        if (!field.visible && field.monthOpen)
            field.monthPopup.close()
    }

    RowLayout {
        id: row

        anchors.fill: parent
        spacing: 4

        TextField {
            id: text

            objectName: "dateText"
            // A phone's row has an hour beside it, and a date needs no more.
            Layout.preferredWidth: field.sheet ? 110 : 130
            text: field.date
            placeholderText: "2026-10-05"
            inputMask: "9999-99-99"
            onTextEdited: {
                field.date = text.text
                field.edited()
            }
            // By Tab the whole date is selected, so that the first digit starts over: the
            // mask keeps every place and a cursor at its end would swallow what is typed.
            onActiveFocusChanged: {
                if (text.activeFocus && (text.focusReason === Qt.TabFocusReason || text.focusReason === Qt.BacktabFocusReason))
                    text.selectAll()
                else if (!text.activeFocus && !field.sheet && field.monthOpen)
                    field.monthPopup.close()
            }
            // Esc belongs to the month while it is open, not to the dialog around the field.
            Keys.onShortcutOverride: event => event.accepted = field.monthOpen && event.key === Qt.Key_Escape
            Keys.onPressed: event => {
                if (event.key === Qt.Key_Down && (event.modifiers & Qt.AltModifier) !== 0 && !field.monthOpen) {
                    field.openMonth()
                    event.accepted = true
                } else if (event.key === Qt.Key_Escape && field.monthOpen) {
                    field.cancelMonth()
                    event.accepted = true
                }
            }
        }
        ToolButton {
            objectName: "dateButton"
            focusPolicy: Qt.NoFocus
            activeFocusOnTab: false
            icon.name: "view-calendar"
            icon.color: field.theme.text
            Accessible.name: field.pickLabel
            ToolTip.visible: hovered
            ToolTip.text: field.theme.plain(field.pickLabel)
            onClicked: field.monthOpen ? field.monthPopup.close() : field.openMonth()
        }
    }

    // Made the first time it opens, then kept: a page of forms holds many dates.
    Loader {
        id: pickerMaker

        active: false

        sourceComponent: Component {
            SafePopup {
                id: picker

                parent: field.sheet ? Overlay.overlay : field
                // A sheet stands alone over the dimmed form. Under the field, the form stays
                // in reach: the press that closes the month also reaches what is under it.
                modal: field.sheet
                dim: field.sheet
                focus: field.sheet
                safeGap: field.sheet ? 0 : 8
                x: field.sheet ? (picker.parent.width - picker.width) / 2 : 0
                y: field.sheet ? picker.parent.height - picker.height - (picker.safeArea ? picker.safeArea.bottom : 0) : field.height
                width: field.sheet ? picker.roomWidth : implicitWidth
                padding: 8
                // A press on the field or its button leaves the month to them: the button
                // closes what it opened. A sheet holds the focus: Esc, and a phone's Back,
                // close it, and it has changed nothing until a day closes it.
                closePolicy: field.sheet ? Popup.CloseOnEscape | Popup.CloseOnPressOutside : Popup.CloseOnPressOutsideParent

                background: Rectangle {
                    color: field.theme.surface
                    border.color: field.theme.line
                    radius: field.theme.radius
                }

                // The month is made when the picker opens: a month is 42 days worked out in local time.
                Loader {
                    active: picker.visible
                    width: field.sheet ? picker.availableWidth : implicitWidth

                    sourceComponent: Component {
                        ColumnLayout {
                            spacing: 4

                            RowLayout {
                                Layout.fillWidth: true

                                ToolButton {
                                    focusPolicy: Qt.NoFocus
                                    text: "◂"
                                    Accessible.name: field.arrowName(-1)
                                    ToolTip.visible: hovered
                                    ToolTip.text: field.theme.plain(field.arrowName(-1))
                                    onClicked: {
                                        if (field.shownMonth === 0) {
                                            field.shownMonth = 11
                                            field.shownYear -= 1
                                        } else {
                                            field.shownMonth -= 1
                                        }
                                    }
                                }
                                Label {
                                    Layout.fillWidth: true
                                    horizontalAlignment: Text.AlignHCenter
                                    text: new Date(field.shownYear, field.shownMonth, 1).toLocaleDateString(field.locale, "MMMM yyyy")
                                    textFormat: Text.PlainText
                                    color: field.theme.text
                                }
                                ToolButton {
                                    objectName: "dateNextMonth"
                                    focusPolicy: Qt.NoFocus
                                    text: "▸"
                                    Accessible.name: field.arrowName(1)
                                    ToolTip.visible: hovered
                                    ToolTip.text: field.theme.plain(field.arrowName(1))
                                    onClicked: {
                                        if (field.shownMonth === 11) {
                                            field.shownMonth = 0
                                            field.shownYear += 1
                                        } else {
                                            field.shownMonth += 1
                                        }
                                    }
                                }
                            }
                            DayOfWeekRow {
                                Layout.fillWidth: true
                                locale: grid.locale
                            }
                            MonthGrid {
                                id: grid

                                objectName: "dateMonth"
                                Layout.fillWidth: true
                                // Rows a fingertip tall on a phone, within a screen held sideways with the month's head.
                                Layout.preferredHeight: field.sheet ? Math.min(6 * 44 + 5 * spacing + topPadding + bottomPadding, picker.roomHeight - 110) : implicitHeight
                                locale: field.locale
                                year: field.shownYear
                                month: field.shownMonth
                                onClicked: day => {
                                    field.date = field.iso(day)
                                    field.edited()
                                    picker.close()
                                    if (field.hourField)
                                        field.hourField.moveHere()
                                }

                                delegate: Label {
                                    id: cell

                                    required property var model

                                    horizontalAlignment: Text.AlignHCenter
                                    verticalAlignment: Text.AlignVCenter
                                    padding: 6
                                    text: cell.model.day
                                    textFormat: Text.PlainText
                                    opacity: cell.model.month === grid.month ? 1 : 0.35
                                    font.weight: field.iso(cell.model.date) === field.date ? Font.Bold : Font.Normal
                                    color: field.iso(cell.model.date) === field.date ? field.theme.accent : field.theme.text
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
