// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Who does what (several agendas, part 4): a week of the children's
// agendas, one child or all, day by day, each event and task with who takes
// it ("You", "Sam", "Sam, asked", "Nobody yet"), and nothing more: no total
// per parent, no history of who did more (research/agendas.md §6,
// recommendation 11). "What I take" keeps those you take, across children.
// A tap opens the event's or the task's card, where it is answered.
// `children::who_does_what`; opened from the agenda filter, beside each
// child's agenda. On a phone, the same list in a sheet as wide as the screen.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafePopup {
    id: week

    required property var sioul
    required property var theme
    // The window, to open a card; none (the Tasks page's filter): rows read only.
    property var window: null
    property bool narrow: false
    // One child's agenda by its place, "" every child; a day of the week, "" this one.
    property string child: ""
    property string from: ""
    property bool mine: false
    // `children::who_does_what`: {from, previous, next, week, children: [{id, name}], days: [{label, items}]}.
    property var shown: ({ days: [], children: [], week: "" })

    function read() {
        try {
            week.shown = JSON.parse(week.sioul.whoDoesWhat(week.from, week.child, week.mine) || "{}")
        } catch (e) {
            week.shown = { days: [], children: [], week: "" }
        }
    }

    // Opened on one child (or all), this week.
    function openOn(child) {
        week.child = child
        week.from = ""
        week.read()
        week.open()
    }

    function openRow(row) {
        if (!week.window)
            return
        week.close()
        week.window.openThing(row.kind === "task" ? { kind: "task", uri: row.uri, key: row.key, title: row.title } : { kind: "event", uri: "", key: row.key, start: row.start, title: row.title })
    }

    objectName: "whoDoesWhat"
    parent: Overlay.overlay
    modal: true
    focus: true
    padding: week.theme.gap
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    width: week.narrow ? week.roomWidth - 2 * week.safeGap : Math.min(460, week.roomWidth - 2 * week.safeGap)
    height: Math.min(implicitHeight, week.roomHeight * 0.85)
    x: ((parent ? parent.width : 800) - week.width) / 2
    y: week.narrow ? (parent ? parent.height : 800) - week.height - week.bottomMargin : Math.max(week.topMargin, ((parent ? parent.height : 800) - week.height) / 3)
    onMineChanged: week.read()
    onChildChanged: week.read()

    background: Rectangle {
        color: week.theme.surface
        border.color: week.theme.line
        radius: week.theme.radius
    }

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 6

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    text: week.sioul.text("holder-who-does-what")
                    textFormat: Text.PlainText
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    color: week.theme.text
                }
                ToolButton {
                    text: "×"
                    Accessible.name: week.sioul.text("ui-close")
                    onClicked: week.close()
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 4

                ToolButton {
                    objectName: "whoPrevious"
                    text: "◀"
                    Accessible.name: week.sioul.text("holder-previous-week")
                    onClicked: {
                        week.from = week.shown.previous || ""
                        week.read()
                    }
                }
                Label {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    horizontalAlignment: Text.AlignHCenter
                    text: week.shown.week || ""
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: week.theme.text
                }
                ToolButton {
                    objectName: "whoNext"
                    text: "▶"
                    Accessible.name: week.sioul.text("holder-next-week")
                    onClicked: {
                        week.from = week.shown.next || ""
                        week.read()
                    }
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 8

                PlainComboBox {
                    readonly property var choices: [""].concat((week.shown.children || []).map(c => c.id))

                    objectName: "whoChild"
                    visible: (week.shown.children || []).length > 1
                    width: Math.min(220, scroll.availableWidth)
                    model: [week.sioul.text("holder-all-children")].concat((week.shown.children || []).map(c => week.theme.plain(c.name)))
                    currentIndex: Math.max(0, choices.indexOf(week.child))
                    Accessible.name: week.sioul.text("holder-who-does-what")
                    onActivated: index => week.child = choices[index]
                }
                CheckBox {
                    objectName: "whoMine"
                    text: week.sioul.text("holder-what-i-take")
                    checked: week.mine
                    onToggled: week.mine = checked
                }
            }
            Label {
                visible: (week.shown.days || []).length === 0
                Layout.fillWidth: true
                text: week.sioul.text("holder-nothing-this-week")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: week.theme.muted
            }
            Repeater {
                model: week.shown.days || []

                delegate: ColumnLayout {
                    id: day

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        Layout.topMargin: 4
                        text: day.modelData.label
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                        color: week.theme.text
                    }
                    Repeater {
                        model: day.modelData.items

                        delegate: RowLayout {
                            id: row

                            required property var modelData

                            objectName: "whoRow"
                            Layout.fillWidth: true
                            spacing: 8

                            Label {
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: [row.modelData.time, row.modelData.title, week.child === "" ? row.modelData.child : ""].filter(w => w !== "").join(" · ")
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: week.theme.text
                            }
                            Label {
                                Layout.maximumWidth: scroll.availableWidth * 0.4
                                text: row.modelData.who
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.weight: row.modelData.yours ? Font.DemiBold : Font.Normal
                                color: row.modelData.nobody ? week.theme.muted : week.theme.text
                            }

                            TapHandler {
                                enabled: week.window !== null
                                onTapped: week.openRow(row.modelData)
                            }
                        }
                    }
                }
            }
        }
    }
}
