// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Search tab of Tasks and of the Agenda (8 November 2026): tasks and
// events found by words in their title, notes or place, and filtered by
// project, kind and category (each with "none"), dates (anytime or a range),
// an event's guests and status, a task's done and date asked. One engine in
// the core decides (`sioul_core::thingsearch`); this lays it out. A full list
// is a tool: everything found is shown, whatever the hour, grouped as what
// comes, the past and what has no date.
//
// One view in both pages (20 November 2026): the page decides what is
// searched (Tasks its tasks, the Agenda its events), and nothing else
// differs. The words and Select on the first line; then one line of filters,
// project, type, category, dates, then the kind's own; on a phone and a
// narrow window they fold under one button that says how many are set,
// open while one is. Tasks gives the project, type and category of its
// other tabs (`upstream`): the search starts on them, and what is changed
// here stays here. The page's agenda filter holds here as in its other full
// views; what it leaves out and the search would have found is counted on
// one quiet line under the results, with a link that shows it all for this
// visit of the tab.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: search

    required property var sioul
    required property var theme
    property var window: null
    // What is searched, the page's: "tasks" in Tasks, "events" in the Agenda.
    property string which: "tasks"
    // The page's own project, type and category (Tasks' other tabs): each one
    // chosen there since the search last took it is the search's when the tab
    // shows; the others stay as the search has them.
    property var upstream: ({ project: "", kind: "", category: "" })
    property var taken: ({ project: "", kind: "", category: "" })
    // On a phone and a narrow window: each filter half a line, all of them folded.
    readonly property bool narrow: search.width < 640
    // From the tab's width, never the filters' own: folded, a layout sizes them no more, and
    // their width would follow what their filters ask, half of it each, without end.
    readonly property int half: Math.floor((search.width - 6) / 2)

    // A filter's width: as wide as its longest choice, `most` at most, so that the line holds them
    // all at a computer's width; on a narrow width half a line, or the whole of it when its words
    // need more ("Avec ou sans date demandée" was cut in half a phone's line).
    function fit(wanted, most) {
        return search.narrow ? (wanted <= search.half ? search.half : search.width) : Math.min(wanted, most)
    }

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
    // Every agenda, whatever the page's filter ("Show them all"): until the tab
    // is left, the link takes it back, or the filter is changed.
    property bool everyAgenda: false
    // The filter's agendas left out, as last read: a change is the person's word.
    property string filterSeen: ""

    // What the core answered: the choices, what was found, what the agendas
    // left out hold, and the page's agenda filter as its full views show it.
    property var shown: ({ projects: [], kinds: [], categories: [], found: [], left_out: 0, agendas: { chosen: false, groups: [] } })
    readonly property var found: search.shown.found || []
    readonly property int leftOut: search.shown.left_out || 0
    readonly property var agendas: search.shown.agendas || ({ chosen: false, groups: [] })
    // What is selected for a batch edit, by file: its Select button, then a
    // click or a tap on each row; Ctrl or Shift with a click, a long press on
    // a touch screen, pick too.
    property var picked: ({})
    property int anchor: -1
    readonly property var pickedList: Object.keys(search.picked).map(k => search.picked[k])
    // Select is on; choosing, either that or something picked another way:
    // each row has its box, a click picks, the bar shows.
    property bool selecting: false
    readonly property bool choosing: search.selecting || search.pickedList.length > 0
    readonly property bool asksEvents: search.which !== "tasks"
    readonly property bool asksTasks: search.which !== "events"
    // The filters set, said on the fold's button.
    readonly property int filtersSet: [search.project !== "", search.kind !== "", search.category !== "", search.ranged, search.asksEvents && search.guests !== "", search.asksEvents && search.statuses.length > 0, search.asksTasks && search.done !== "", search.asksTasks && search.deadline !== ""].filter(set => set).length
    // The fold, by its button (true open, false closed); null, open while a filter is set.
    property var unfolded: null
    readonly property bool folded: search.narrow && (search.unfolded === null ? search.filtersSet === 0 : !search.unfolded)

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
            deadline: tasks ? yesNo(search.deadline) : null,
            every_agenda: search.everyAgenda
        }
    }

    // The tab shown: the page's choices made since the search last took them
    // are the search's, shown as set; the others stay as they were here.
    function takeUpstream() {
        const page = search.upstream || {}
        const taken = Object.assign({}, search.taken)
        for (const key of ["project", "kind", "category"]) {
            const value = page[key] || ""
            if (value !== taken[key]) {
                search[key] = value
                taken[key] = value
            }
        }
        search.taken = taken
    }

    // "Show them all" (true) or "Back to the agendas ticked" (false), for this visit of the tab.
    function showEveryAgenda(on) {
        search.everyAgenda = on
        search.run()
    }

    // Asked again at once; words typed wait for a pause. Whole first: a Loader shows its item
    // before its bindings give it Sioul.
    function run() {
        if (!search.visible || !search.whole)
            return
        const text = search.sioul.findThings(JSON.stringify(search.query()))
        const answer = text ? JSON.parse(text) : { projects: [], kinds: [], categories: [], found: [], left_out: 0, agendas: { chosen: false, groups: [] } }
        search.shown = answer
        // The page's filter changed while every agenda showed: the person's word, followed again.
        // Read from the answer itself, before the bindings follow.
        const off = JSON.stringify(((answer.agendas || {}).groups || []).map(g => g.agendas.filter(a => !a.on).map(a => a.id)))
        const changed = search.filterSeen !== "" && off !== search.filterSeen
        search.filterSeen = off
        if (changed && search.everyAgenda) {
            search.showEveryAgenda(false)
            return
        }
        // What is picked and still found stays picked, as it now is.
        const kept = {}
        for (const thing of search.found)
            if (search.picked[thing.key])
                kept[thing.key] = thing
        search.picked = kept
        search.anchor = -1
    }

    function open(thing) {
        if (!search.window)
            return
        if (thing.thing === "task")
            search.window.openCard({ kind: "task", owner: thing.uid })
        else
            search.window.openCard({ kind: "event", srcKey: thing.key, srcStart: thing.at || 0, title: thing.title })
    }

    // A row picked or let go; with `range`, every row from the last one picked.
    function pick(index, range) {
        const now = Object.assign({}, search.picked)
        const thing = search.found[index]
        if (!thing)
            return
        if (range && search.anchor >= 0) {
            const [from, to] = [Math.min(search.anchor, index), Math.max(search.anchor, index)]
            for (let i = from; i <= to; i++)
                now[search.found[i].key] = search.found[i]
        } else if (now[thing.key]) {
            delete now[thing.key]
        } else {
            now[thing.key] = thing
        }
        search.anchor = index
        search.picked = now
    }

    // The selection emptied and Select off: after an act, or asked.
    function stopChoosing() {
        search.picked = ({})
        search.anchor = -1
        search.selecting = false
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

    // Made, its bindings set: its handlers act from here on.
    property bool whole: false
    onVisibleChanged: {
        if (!search.whole)
            return
        if (search.visible) {
            search.takeUpstream()
        } else {
            search.everyAgenda = false
        }
        search.run()
    }
    onWhichChanged: search.run()
    Component.onCompleted: {
        search.whole = true
        search.takeUpstream()
        search.run()
    }

    // What changed elsewhere shows here too; the agenda filter turned shows in the
    // agenda's days, and in the tasks' views only when a list of tasks turned.
    Connections {
        target: search.sioul
        ignoreUnknownSignals: true

        function onTasksChanged() {
            search.run()
        }

        function onAgendaChanged() {
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

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            TextField {
                objectName: "thingsWords"
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                placeholderText: search.sioul.text("things-words")
                text: search.words
                onTextEdited: {
                    search.words = text
                    typing.restart()
                }
            }
            // On a phone and a narrow window: the filters folded; how many are set, in its words.
            Button {
                id: foldButton

                objectName: "thingsFilters"
                visible: search.narrow
                checkable: true
                checked: !search.folded
                flat: true
                text: search.sioul.textCounted("task-filters", search.filtersSet)
                icon.name: "view-filter"
                icon.color: checked ? search.theme.accentText : search.theme.text
                onClicked: {
                    search.unfolded = search.folded
                    // A click flips its own tick: it stays the fold's.
                    foldButton.checked = Qt.binding(() => !search.folded)
                }
            }
            // Choosing what to change together: a box on each row, the bar at once.
            Button {
                objectName: "thingsSelect"
                checkable: true
                checked: search.choosing
                text: search.sioul.text("batch-select")
                ToolTip.visible: hovered
                ToolTip.delay: 600
                ToolTip.text: search.sioul.text("batch-select-tip")
                onToggled: {
                    if (checked)
                        search.selecting = true
                    else
                        search.stopChoosing()
                    checked = Qt.binding(() => search.choosing)
                }
            }
        }

        // The filters, in one order in both pages: project, type, category, dates, then the
        // kind's own; on as many lines as the width asks, never past its edge. Folded, no room.
        Flow {
            id: filters

            visible: !search.folded
            Layout.fillWidth: true
            spacing: 6

            PlainComboBox {
                objectName: "thingsProject"

                readonly property var choices: [{ id: "", label: search.sioul.text("things-any-project") }, { id: ":none", label: search.sioul.text("things-no-project") }].concat(search.shown.projects || [])

                implicitContentWidthPolicy: ComboBox.WidestText
                width: search.fit(implicitWidth, 240)
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

                implicitContentWidthPolicy: ComboBox.WidestText
                width: search.fit(implicitWidth, 200)
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

                implicitContentWidthPolicy: ComboBox.WidestText
                width: search.fit(implicitWidth, 220)
                model: choices.map(c => search.theme.plain(c.label))
                currentIndex: Math.max(0, choices.findIndex(c => c.id === search.category))
                onActivated: index => {
                    search.category = choices[index].id
                    search.run()
                }
            }
            PlainComboBox {
                objectName: "thingsDates"

                implicitContentWidthPolicy: ComboBox.WidestText
                width: search.fit(implicitWidth, 200)
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
                implicitContentWidthPolicy: ComboBox.WidestText
                width: search.fit(implicitWidth, 240)
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
                implicitContentWidthPolicy: ComboBox.WidestText
                width: search.fit(implicitWidth, 200)
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
                implicitContentWidthPolicy: ComboBox.WidestText
                width: search.fit(implicitWidth, 280)
                model: [search.sioul.text("things-due-any"), search.sioul.text("things-due-yes"), search.sioul.text("things-due-no")]
                currentIndex: Math.max(0, ways.indexOf(search.deadline))
                onActivated: index => {
                    search.deadline = ways[index]
                    search.run()
                }
            }
        }

        BatchBar {
            objectName: "thingsBatch"
            Layout.fillWidth: true
            sioul: search.sioul
            theme: search.theme
            things: search.pickedList
            selecting: search.choosing
            offers: search.which
            projects: search.shown.projects || []
            kinds: search.shown.kinds || []
            categories: search.shown.categories || []
            lists: search.shown.lists || []
            onCleared: search.stopChoosing()
        }

        Label {
            objectName: "thingsNone"
            visible: search.found.length === 0
            Layout.fillWidth: true
            // Nothing in the agendas ticked, something elsewhere: said so, the line under it says how much.
            text: search.leftOut > 0 && !search.everyAgenda ? search.sioul.text("things-none-found-filtered") : search.sioul.text("things-none-found")
            wrapMode: Text.Wrap
            color: search.theme.muted
        }

        // Found again whenever tasks change: the list keeps its place (ListPlace.qml); other words
        // or filters, a list from its top.
        ListPlace {
            id: resultsPlace

            view: results
            source: search.found
            scope: search.query()
        }
        ListView {
            id: results

            objectName: "thingsFound"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            spacing: 2
            model: resultsPlace.rows
            section.property: "when"
            section.delegate: Label {
                required property string section

                topPadding: 8
                text: search.sioul.text("things-" + section)
                font.weight: Font.DemiBold
                color: search.theme.text
            }
            ScrollBar.vertical: ScrollBar {}

            // Under the results, one quiet line: what the page's agenda filter leaves out
            // and the search would have found, with a link that shows it all; or that
            // every agenda shows, with the link back. Absence proves nothing.
            footer: Flow {
                objectName: "thingsLeftOut"
                visible: search.leftOut > 0 || search.everyAgenda
                width: results.width - 12
                height: visible ? implicitHeight : 0
                topPadding: 8
                spacing: 8

                Label {
                    objectName: "thingsLeftOutLine"
                    width: Math.min(implicitWidth, parent.width)
                    text: search.everyAgenda ? search.sioul.text("things-every-agenda") : search.sioul.textCounted("things-left-out", search.leftOut)
                    wrapMode: Text.Wrap
                    color: search.theme.muted
                }
                // A link: no frame, its words alone.
                Button {
                    id: showAll

                    objectName: "thingsShowAll"
                    text: search.everyAgenda ? search.sioul.text("things-agendas-shown") : search.sioul.text("things-show-all")
                    Accessible.name: showAll.text
                    background: null
                    leftPadding: 0
                    rightPadding: 0
                    topPadding: 0
                    bottomPadding: 0
                    contentItem: Label {
                        text: search.everyAgenda ? search.sioul.text("things-agendas-shown") : search.sioul.text("things-show-all")
                        wrapMode: Text.Wrap
                        color: search.theme.accent
                        font.underline: true
                        // Its focus seen by keyboard, as a flat link has no frame.
                        font.bold: showAll.visualFocus
                    }
                    onClicked: search.showEveryAgenda(!search.everyAgenda)
                }
            }

            delegate: ItemDelegate {
                id: row

                required property var modelData
                required property int index

                readonly property bool picked: search.picked[row.modelData.key] !== undefined
                // Pressed with Ctrl ("one") or Shift ("range"): the click that follows picks.
                property string modified: ""

                objectName: "thingRow"
                width: results.width - 12
                highlighted: row.picked
                onClicked: {
                    // A click with Ctrl or Shift picks (its handlers below), never opens.
                    if (row.modified !== "") {
                        search.pick(row.index, row.modified === "range")
                        row.modified = ""
                        return
                    }
                    // Choosing: a click or a tap picks it too.
                    if (search.choosing)
                        search.pick(row.index, false)
                    else
                        search.open(row.modelData)
                }
                onPressAndHold: search.pick(row.index, false)
                Accessible.name: row.modelData.title

                // Ctrl or Shift with a click: picked, never opened. The handlers only
                // see the press; the row's own click picks.
                TapHandler {
                    acceptedModifiers: Qt.ControlModifier
                    onPressedChanged: if (pressed) row.modified = "one"
                }
                TapHandler {
                    acceptedModifiers: Qt.ShiftModifier
                    onPressedChanged: if (pressed) row.modified = "range"
                }

                background: Rectangle {
                    color: row.picked || row.hovered ? search.theme.surface : "transparent"
                    radius: search.theme.radius
                    border.color: row.visualFocus ? search.theme.focus : row.picked ? search.theme.accent : "transparent"
                }
                contentItem: RowLayout {
                    spacing: 10

                    // Choosing what to change: its box, ticked while it is picked. Made only then.
                    Loader {
                        active: search.choosing
                        visible: active
                        Layout.alignment: Qt.AlignTop
                        sourceComponent: CheckBox {
                            objectName: "thingPick"
                            // Its tick follows the selection alone: a click asks, the list picks.
                            checkable: false
                            checked: row.picked
                            padding: 0
                            Accessible.name: row.modelData.title
                            onClicked: search.pick(row.index, false)
                        }
                    }
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
