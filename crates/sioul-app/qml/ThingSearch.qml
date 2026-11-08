// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Search tab of Tasks and of the Agenda (8 November 2026): tasks and
// events found by words in their title, notes or place, and filtered by
// project, kind and category (each with "none"), dates (anytime or a range),
// an event's guests and status, a task's done and date asked. One engine in
// the core decides (`sioul_core::thingsearch`); this lays it out. A full list
// is a tool: everything found is shown, whatever the hour, grouped as what
// comes, the past and what has no date.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: search

    required property var sioul
    required property var theme
    property var window: null
    // What is searched first: "tasks" in Tasks, "events" in the Agenda; "both" chosen there.
    property string which: "both"
    // On a phone: each filter half a line.
    readonly property bool narrow: search.width < 640
    readonly property int half: Math.floor((filters.width - filters.spacing) / 2)

    // The query as the core reads it (`thingsearch::Query`).
    property string words: ""
    property string project: ""
    property string kind: ""
    property string category: ""
    property bool ranged: false
    property string from: ""
    property string to: ""
    // "", "yes", "no".
    property string guests: ""
    property var statuses: []
    property string done: ""
    property string deadline: ""

    // What the core answered: the choices and what was found.
    property var shown: ({ projects: [], kinds: [], categories: [], found: [] })
    readonly property var found: search.shown.found || []
    readonly property bool asksEvents: search.which !== "tasks"
    readonly property bool asksTasks: search.which !== "events"

    function query() {
        const yesNo = v => v === "yes" ? true : v === "no" ? false : null
        // Read from `which` itself: asked from its own change, before the bindings follow.
        const events = search.which !== "tasks"
        const tasks = search.which !== "events"
        return {
            words: search.words,
            which: search.which,
            project: search.project,
            kind: search.kind,
            category: search.category,
            from: search.ranged ? search.from : "",
            to: search.ranged ? search.to : "",
            guests: events ? yesNo(search.guests) : null,
            status: events ? search.statuses : [],
            done: tasks ? yesNo(search.done) : null,
            deadline: tasks ? yesNo(search.deadline) : null
        }
    }

    // Asked again at once; words typed wait for a pause.
    function run() {
        if (!search.visible)
            return
        const text = search.sioul.findThings(JSON.stringify(search.query()))
        search.shown = text ? JSON.parse(text) : { projects: [], kinds: [], categories: [], found: [] }
    }

    function open(thing) {
        if (!search.window)
            return
        if (thing.thing === "task")
            search.window.openCard({ kind: "task", owner: thing.uid })
        else
            search.window.openCard({ kind: "event", srcKey: thing.key, srcStart: thing.at || 0, title: thing.title })
    }

    function toggleStatus(status, on) {
        const now = search.statuses.filter(s => s !== status)
        if (on)
            now.push(status)
        search.statuses = now
        search.run()
    }

    // A row's line: its time, its projects, its place, and its status when it says something.
    function line(thing) {
        const status = thing.thing === "event" ? (thing.status === "confirmed" ? "" : search.sioul.text("things-status-" + thing.status)) : thing.done ? search.sioul.text("things-row-done") : ""
        return [thing.date, thing.projects.join(", "), thing.place, status].filter(t => t !== "").join("  ·  ")
    }

    onVisibleChanged: search.run()
    onWhichChanged: search.run()
    Component.onCompleted: search.run()

    // What changed elsewhere shows here too.
    Connections {
        target: search.sioul

        function onTasksChanged() {
            search.run()
        }
    }

    Timer {
        id: typing

        interval: 250
        onTriggered: search.run()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        TextField {
            objectName: "thingsWords"
            Layout.fillWidth: true
            placeholderText: search.sioul.text("things-words")
            text: search.words
            onTextEdited: {
                search.words = text
                typing.restart()
            }
        }

        // The filters, on as many lines as the width asks, never past its edge.
        Flow {
            id: filters

            Layout.fillWidth: true
            spacing: 6

            PlainComboBox {
                objectName: "thingsWhich"

                readonly property var ways: ["both", "tasks", "events"]

                width: search.narrow ? search.half : 170
                model: ways.map(w => search.sioul.text("things-which-" + w))
                currentIndex: Math.max(0, ways.indexOf(search.which))
                onActivated: index => search.which = ways[index]
            }
            PlainComboBox {
                objectName: "thingsProject"

                readonly property var choices: [{ id: "", label: search.sioul.text("things-any-project") }, { id: ":none", label: search.sioul.text("things-no-project") }].concat(search.shown.projects || [])

                width: search.narrow ? search.half : 180
                model: choices.map(c => search.theme.plain(c.label))
                currentIndex: Math.max(0, choices.findIndex(c => c.id === search.project))
                onActivated: index => {
                    search.project = choices[index].id
                    search.run()
                }
            }
            PlainComboBox {
                objectName: "thingsKind"

                readonly property var choices: [{ id: "", label: search.sioul.text("things-any-kind") }, { id: ":none", label: search.sioul.text("things-no-kind") }].concat(search.shown.kinds || [])

                width: search.narrow ? search.half : 170
                model: choices.map(c => search.theme.plain(c.label))
                currentIndex: Math.max(0, choices.findIndex(c => c.id === search.kind))
                onActivated: index => {
                    search.kind = choices[index].id
                    search.run()
                }
            }
            PlainComboBox {
                objectName: "thingsCategory"

                readonly property var choices: [{ id: "", label: search.sioul.text("things-any-category") }, { id: ":none", label: search.sioul.text("things-no-category") }].concat((search.shown.categories || []).map(c => ({ id: c, label: c })))

                width: search.narrow ? search.half : 180
                model: choices.map(c => search.theme.plain(c.label))
                currentIndex: Math.max(0, choices.findIndex(c => c.id === search.category))
                onActivated: index => {
                    search.category = choices[index].id
                    search.run()
                }
            }
            PlainComboBox {
                objectName: "thingsDates"

                width: search.narrow ? search.half : 150
                model: [search.sioul.text("things-anytime"), search.sioul.text("things-range")]
                currentIndex: search.ranged ? 1 : 0
                onActivated: index => {
                    search.ranged = index === 1
                    search.run()
                }
            }
            DateField {
                objectName: "thingsFrom"
                visible: search.ranged
                width: search.narrow ? search.half : 150
                theme: search.theme
                sioul: search.sioul
                pickLabel: search.sioul.text("things-from")
                date: search.from
                onEdited: {
                    search.from = date
                    search.run()
                }
            }
            DateField {
                objectName: "thingsTo"
                visible: search.ranged
                width: search.narrow ? search.half : 150
                theme: search.theme
                sioul: search.sioul
                pickLabel: search.sioul.text("things-to")
                date: search.to
                onEdited: {
                    search.to = date
                    search.run()
                }
            }
            // Events: with guests or not, and their status.
            PlainComboBox {
                objectName: "thingsGuests"

                readonly property var ways: ["", "yes", "no"]

                visible: search.asksEvents
                width: search.narrow ? search.half : 200
                model: [search.sioul.text("things-guests-any"), search.sioul.text("things-guests-yes"), search.sioul.text("things-guests-no")]
                currentIndex: Math.max(0, ways.indexOf(search.guests))
                onActivated: index => {
                    search.guests = ways[index]
                    search.run()
                }
            }
            Repeater {
                model: search.asksEvents ? ["confirmed", "tentative", "cancelled"] : []

                delegate: CheckBox {
                    required property string modelData

                    objectName: "thingsStatus-" + modelData
                    text: search.sioul.text("things-status-" + modelData)
                    checked: search.statuses.indexOf(modelData) >= 0
                    onToggled: search.toggleStatus(modelData, checked)
                }
            }
            // Tasks: done or not, with a date asked or not.
            PlainComboBox {
                objectName: "thingsDone"

                readonly property var ways: ["", "no", "yes"]

                visible: search.asksTasks
                width: search.narrow ? search.half : 170
                model: [search.sioul.text("things-done-any"), search.sioul.text("things-done-no"), search.sioul.text("things-done-yes")]
                currentIndex: Math.max(0, ways.indexOf(search.done))
                onActivated: index => {
                    search.done = ways[index]
                    search.run()
                }
            }
            PlainComboBox {
                objectName: "thingsDeadline"

                readonly property var ways: ["", "yes", "no"]

                visible: search.asksTasks
                width: search.narrow ? search.half : 230
                model: [search.sioul.text("things-due-any"), search.sioul.text("things-due-yes"), search.sioul.text("things-due-no")]
                currentIndex: Math.max(0, ways.indexOf(search.deadline))
                onActivated: index => {
                    search.deadline = ways[index]
                    search.run()
                }
            }
        }

        Label {
            visible: search.found.length === 0
            Layout.fillWidth: true
            text: search.sioul.text("things-none-found")
            wrapMode: Text.Wrap
            color: search.theme.muted
        }

        ListView {
            id: results

            objectName: "thingsFound"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 2
            model: search.found
            section.property: "when"
            section.delegate: Label {
                required property string section

                topPadding: 8
                text: search.sioul.text("things-" + section)
                font.weight: Font.DemiBold
                color: search.theme.text
            }
            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: row

                required property var modelData
                required property int index

                objectName: "thingRow"
                width: results.width - 12
                onClicked: search.open(row.modelData)
                Accessible.name: row.modelData.title

                background: Rectangle {
                    color: row.hovered ? search.theme.surface : "transparent"
                    radius: search.theme.radius
                    border.color: row.visualFocus ? search.theme.focus : "transparent"
                }
                contentItem: RowLayout {
                    spacing: 10

                    Icon {
                        Layout.alignment: Qt.AlignTop
                        iconName: row.modelData.thing === "task" ? (row.modelData.done ? "checkbox" : "view-calendar-tasks") : "view-calendar-day"
                        size: 16
                        color: search.theme.muted
                        tip: search.sioul.text(row.modelData.thing === "task" ? "things-a-task" : "things-an-event")
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: row.modelData.title
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.strikeout: row.modelData.status === "cancelled"
                            color: row.modelData.done ? search.theme.muted : search.theme.text
                        }
                        Label {
                            readonly property string said: search.line(row.modelData)

                            visible: said !== ""
                            Layout.fillWidth: true
                            text: said
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: search.theme.muted
                        }
                    }
                }
            }
        }
    }
}
