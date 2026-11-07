// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What is coming (research/budgets.md §6, recommendation 3): every movement
// expected over the next six weeks, or until the income after next, in date
// order and grouped by week; each with who, the amount or "about", its
// budget, its state in words and what tells of it. Money in looks as money
// out does. A month grid on demand, beside the list, never instead of it
// (a computer's; a phone keeps the list). A row's menu (a long press, a
// right click, its "⋯") moves it, splits it, says it is not the same.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window
    // The page's view (`view::BudgetsView`).
    property var view: null
    // The month grid beside the list.
    property bool month: false
    // The first day of the month the grid shows.
    property date monthShown: new Date(new Date().getFullYear(), new Date().getMonth(), 1)
    readonly property bool compact: section.window.compact === true
    readonly property var rows: section.view && section.view.coming ? section.view.coming.reduce((all, w) => all.concat(w.rows), []) : []
    // The days' column: as wide as its widest day, in this language ("Wed 18 Nov", "mer. 18 nov.").
    readonly property real dateWidth: Math.ceil(section.rows.reduce((widest, r) => Math.max(widest, dateFont.advanceWidth(r.date)), 0)) + 4

    // Its effect said by the page; asked once there too.
    signal said(string text, var ask)

    spacing: 6

    Label {
        id: dateProbe

        visible: false
        font.features: { "tnum": 1 }
    }
    FontMetrics {
        id: dateFont

        font: dateProbe.font
    }

    // A row's menu, at the finger on a phone.
    function menuOf(row, item, x, y) {
        if (item && x !== undefined)
            item.Window.window.menuAt = item.mapToItem(null, x, y)
        actions.show(row)
    }

    // The days of the month shown, Monday first: [{day: "2026-11-02", number, inMonth}].
    function monthDays() {
        const first = section.monthShown
        const start = new Date(first.getFullYear(), first.getMonth(), 1 - ((first.getDay() + 6) % 7))
        const days = []
        for (let i = 0; i < 42; ++i) {
            const d = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i)
            if (i >= 35 && d.getMonth() !== first.getMonth())
                break
            const iso = d.getFullYear() + "-" + String(d.getMonth() + 1).padStart(2, "0") + "-" + String(d.getDate()).padStart(2, "0")
            days.push({ day: iso, number: d.getDate(), inMonth: d.getMonth() === first.getMonth() })
        }
        return days
    }

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: section.theme.gap
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: section.view && section.view.coming_title ? section.view.coming_title : ""
            textFormat: Text.PlainText
            font.pixelSize: 19
            color: section.theme.text
        }
        ToolButton {
            text: "?"
            Accessible.name: section.sioul.text("coming-help")
            ToolTip.visible: hovered || pressed
            ToolTip.text: section.sioul.text("coming-help")
            ToolTip.delay: 300
        }
        Button {
            objectName: "comingMonth"
            visible: !section.compact
            flat: true
            text: section.month ? section.sioul.text("coming-month-hide") : section.sioul.text("coming-month-show")
            onClicked: section.month = !section.month
        }
    }
    Label {
        visible: section.rows.length === 0
        Layout.fillWidth: true
        text: section.sioul.text("coming-none")
        color: section.theme.muted
    }

    RowLayout {
        Layout.fillWidth: true
        spacing: section.theme.gap

        // The list, week by week.
        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
            spacing: 4

            Repeater {
                model: section.view && section.view.coming ? section.view.coming : []

                delegate: ColumnLayout {
                    id: week

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        Layout.topMargin: 6
                        text: week.modelData.title
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        color: section.theme.muted
                    }
                    Repeater {
                        model: week.modelData.rows

                        delegate: ItemDelegate {
                            id: line

                            required property var modelData

                            objectName: "comingRow"
                            Layout.fillWidth: true
                            padding: 4
                            leftPadding: 0
                            Accessible.name: line.modelData.date + ", " + line.modelData.who + ", " + line.modelData.way_words + " " + line.modelData.amount + ", " + line.modelData.state
                            onPressAndHold: section.menuOf(line.modelData, line, line.pressX, line.pressY)

                            TapHandler {
                                acceptedButtons: Qt.RightButton
                                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                onTapped: section.menuOf(line.modelData)
                            }

                            contentItem: ColumnLayout {
                                spacing: 0

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8

                                    Label {
                                        Layout.preferredWidth: section.dateWidth
                                        text: line.modelData.date
                                        textFormat: Text.PlainText
                                        font.features: { "tnum": 1 }
                                        color: section.theme.muted
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: line.modelData.who
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: section.theme.text
                                    }
                                    // Money in as plain as money out: its sign alone tells.
                                    Label {
                                        text: (line.modelData.way === "in" ? "+" : "") + line.modelData.amount
                                        textFormat: Text.PlainText
                                        horizontalAlignment: Text.AlignRight
                                        font.features: { "tnum": 1 }
                                        font.weight: line.modelData.way === "in" ? Font.DemiBold : Font.Normal
                                        color: section.theme.text
                                    }
                                    // A computer's; a long press on a phone.
                                    Loader {
                                        visible: active
                                        active: !section.compact

                                        sourceComponent: ToolButton {
                                            text: "⋯"
                                            Accessible.name: section.sioul.text("bank-movement-where")
                                            onClicked: section.menuOf(line.modelData)
                                        }
                                    }
                                }
                                Label {
                                    Layout.fillWidth: true
                                    Layout.leftMargin: section.dateWidth + 8
                                    text: [line.modelData.state, line.modelData.budget].filter(t => t !== "").join("  ·  ")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 12
                                    color: section.theme.muted
                                }
                                // What tells of it, together: the mail, the letter, the contract, the bank's line.
                                Label {
                                    visible: line.modelData.sources.length > 1 || line.modelData.probable
                                    Layout.fillWidth: true
                                    Layout.leftMargin: section.dateWidth + 8
                                    text: line.modelData.from + (line.modelData.probable ? "  " + section.sioul.text("coming-probably") : "")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 12
                                    color: section.theme.muted
                                }
                            }
                        }
                    }
                }
            }
            Label {
                visible: !!section.view && !!section.view.unusual
                Layout.fillWidth: true
                Layout.topMargin: 6
                text: section.view && section.view.unusual ? section.view.unusual : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: section.theme.muted
            }
        }

        // The same movements on a month's calendar, beside the list; made when asked for.
        Loader {
            objectName: "comingGrid"
            visible: active
            active: section.month && !section.compact
            Layout.alignment: Qt.AlignTop
            Layout.preferredWidth: 7 * 62

            sourceComponent: ColumnLayout {
                    spacing: 4

                    RowLayout {
                        Layout.fillWidth: true

                        Button {
                            flat: true
                            text: "◂"
                            Accessible.name: section.sioul.text("agenda-earlier")
                            onClicked: section.monthShown = new Date(section.monthShown.getFullYear(), section.monthShown.getMonth() - 1, 1)
                        }
                        Label {
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                            text: section.monthShown.toLocaleDateString(Qt.locale(section.sioul.text("qt-locale")), "MMMM yyyy")
                            color: section.theme.text
                        }
                        Button {
                            flat: true
                            text: "▸"
                            Accessible.name: section.sioul.text("agenda-later")
                            onClicked: section.monthShown = new Date(section.monthShown.getFullYear(), section.monthShown.getMonth() + 1, 1)
                        }
                    }
                    GridLayout {
                        Layout.fillWidth: true
                        columns: 7
                        columnSpacing: 2
                        rowSpacing: 2

                        Repeater {
                            model: section.month ? section.monthDays() : []

                            delegate: Rectangle {
                                id: cell

                                required property var modelData
                                readonly property var here: section.rows.filter(r => r.day === cell.modelData.day)

                                Layout.preferredWidth: 60
                                Layout.preferredHeight: Math.max(48, entries.implicitHeight + 6)
                                radius: 3
                                color: "transparent"
                                border.color: section.theme.line
                                opacity: cell.modelData.inMonth ? 1 : 0.45

                                ColumnLayout {
                                    id: entries

                                    anchors.fill: parent
                                    anchors.margins: 3
                                    spacing: 0

                                    Label {
                                        text: cell.modelData.number
                                        textFormat: Text.PlainText
                                        font.pixelSize: 11
                                        color: section.theme.muted
                                    }
                                    Repeater {
                                        model: cell.here

                                        delegate: Label {
                                            required property var modelData

                                            Layout.fillWidth: true
                                            text: (modelData.way === "in" ? "+" : "") + modelData.amount + " " + modelData.who
                                            textFormat: Text.PlainText
                                            elide: Text.ElideRight
                                            font.pixelSize: 10
                                            font.features: { "tnum": 1 }
                                            color: section.theme.text
                                        }
                                    }
                                }
                            }
                        }
                    }
            }
        }
    }

    MovementActions {
        id: actions

        sioul: section.sioul
        theme: section.theme
        window: section.window
        budgets: section.view && section.view.choices ? section.view.choices : []
        onSaid: (text, ask) => section.said(text, ask)
    }
}
