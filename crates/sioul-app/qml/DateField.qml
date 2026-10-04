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

    required property var theme
    // Month and day names in Sioul's language.
    property var locale: Qt.locale()
    // "2026-10-05".
    property string date: ""
    property string pickLabel: ""

    signal edited

    function pad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    function iso(d) {
        return d.getFullYear() + "-" + field.pad(d.getMonth() + 1) + "-" + field.pad(d.getDate())
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
            ToolTip.text: field.pickLabel
            onClicked: {
                const shown = field.date ? new Date(field.date + "T12:00:00") : new Date()
                grid.year = shown.getFullYear()
                grid.month = shown.getMonth()
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

        ColumnLayout {
            spacing: 4

            RowLayout {
                Layout.fillWidth: true

                ToolButton {
                    text: "◂"
                    onClicked: {
                        if (grid.month === 0) {
                            grid.month = 11
                            grid.year -= 1
                        } else {
                            grid.month -= 1
                        }
                    }
                }
                Label {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: new Date(grid.year, grid.month, 1).toLocaleDateString(field.locale, "MMMM yyyy")
                    textFormat: Text.PlainText
                    color: field.theme.text
                }
                ToolButton {
                    text: "▸"
                    onClicked: {
                        if (grid.month === 11) {
                            grid.month = 0
                            grid.year += 1
                        } else {
                            grid.month += 1
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
