// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A date, typed as 2026-10-05 or picked from a month: the calendar button
// opens the month, a click on a day closes it.

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

    implicitWidth: row.implicitWidth
    implicitHeight: row.implicitHeight

    RowLayout {
        id: row

        anchors.fill: parent
        spacing: 4

        TextField {
            id: text

            Layout.preferredWidth: 130
            text: field.date
            placeholderText: "2026-10-05"
            inputMask: "9999-99-99"
            onTextEdited: {
                field.date = text.text
                field.edited()
            }
        }
        ToolButton {
            icon.name: "view-calendar"
            icon.color: field.theme.text
            Accessible.name: field.pickLabel
            ToolTip.visible: hovered
            ToolTip.text: field.theme.plain(field.pickLabel)
            onClicked: {
                const shown = field.date ? new Date(field.date + "T12:00:00") : new Date()
                field.shownYear = shown.getFullYear()
                field.shownMonth = shown.getMonth()
                picker.open()
            }
        }
    }

    Popup {
        id: picker

        y: field.height
        padding: 8

        background: Rectangle {
            color: field.theme.surface
            border.color: field.theme.line
            radius: field.theme.radius
        }

        // The month is made when the picker opens: a page of forms holds
        // many dates, and a month is 42 days worked out in local time.
        Loader {
            active: picker.visible

            sourceComponent: Component {
                ColumnLayout {
                    spacing: 4

                    RowLayout {
                        Layout.fillWidth: true

                        ToolButton {
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

                        Layout.fillWidth: true
                        locale: field.locale
                        year: field.shownYear
                        month: field.shownMonth
                        onClicked: day => {
                            field.date = field.iso(day)
                            field.edited()
                            picker.close()
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
