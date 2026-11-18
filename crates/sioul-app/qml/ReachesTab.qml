// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ What reaches you (« Ce qui vous joint »; docs/attention.md, "The
// interface"): Sioul decides what gets through, and when; Android's
// do-not-disturb only decides whether it makes a sound. On top, the sentence
// for now, the three states, and Start from (the presets). Two tabs:
// - Your rules: People (a choice per circle for mail and for messages), who
//   is in which circle, Sioul's own notices (four groups) and Moments (the
//   three states, what turns do-not-disturb on, who holds what on a phone);
//   one line counting what is set elsewhere, and one saying that calls are
//   set per phone, in Devices.
// - What comes when: the whole grid, a word in each cell, the columns read now
//   marked (AttentionGrid.qml); a press changes a cell.
// On a phone each table is a list of lines, each opening its choices. The
// words come from the window's `reachesView` (reaches.rs, read from the matrix
// by sioul_core::behaviours); a choice is saved at once, its rows whole
// (`reachesChoose`), a cell of the grid by its row (`reachesSet`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: tab

    objectName: "reachesTab"

    required property var sioul
    required property var theme
    // Settings' rows: this page's own switches (section "attention").
    required property var rows

    // A switch changed (Settings saves it, then reads its rows again).
    signal save(string key, var value, var shown)
    // Another tab shown: the page scrolled back to its top.
    signal toTop
    // A part asked for (a door from elsewhere): the page scrolled to it.
    signal reveal(var item)

    property var shown: ({ now: [], state: "usual", column: "", now_columns: [], presets: [], changed: "", people: { channels: [], rows: [], blocked: { label: "", help: "", word: "" }, notes: [], own_help: "" }, own: [], moments: { columns: [], rows: [], holds_title: "", holds_columns: [], holds: [], principle: "" }, elsewhere: { line: "", links: [] }, calls: "", rows: [], grid: { columns: [], marks: [], rows: [] }, phone: false })
    // "rules" or "table".
    property string view: "rules"
    // On a phone, What comes when: the row whose times are open; "" for the list.
    property string rowOpen: ""
    property string problem: ""
    // A preset pressed while your own changes stand: asked first.
    property string presetAsked: ""
    // Who is in which circle (Settings' "senders").
    property var lists: []
    readonly property bool narrow: tab.width > 0 && tab.width < 560
    // The window (main.qml): its status line's button and a person's sheet.
    readonly property var host: tab.Window.window
    // What comes when's rows, as the grid has them.
    readonly property var gridRows: tab.shown.grid.rows.filter(r => tab.shown.rows.indexOf(r.id) >= 0)
    readonly property var openRowData: tab.gridRows.find(r => r.id === tab.rowOpen) || null
    // People's cells, each at its place in the table: its circle's line, its channel's column.
    readonly property var peopleCells: {
        const out = []
        tab.shown.people.rows.forEach((circle, row) => circle.cells.forEach((cell, column) => out.push({ circle: circle, cell: cell, row: row + 1, column: column + 1 })))
        return out
    }

    function reload() {
        tab.shown = JSON.parse(tab.sioul.reachesView() || "null") || tab.shown
    }

    function setting(key) {
        return tab.rows.find(r => r.key === key) || null
    }

    // A behaviour, a group's choice or a care switch (`what`: "people.mail.safe",
    // "group.reminders", "care.move"), its rows written whole.
    function choose(what, value) {
        tab.problem = tab.sioul.reachesChoose(what, value)
        tab.reload()
    }

    // A cell of What comes when changed, its row saved whole; only the cell
    // changed is set over the row as the configuration holds it now.
    function chooseCell(row, column, value) {
        const word = (c, v) => v === "now" ? c.column : c.column + ":" + v
        const words = row.cells.map(c => word(c, c.column === column ? value : c.value))
        tab.problem = tab.sioul.reachesSet(row.id, JSON.stringify(words), JSON.stringify(row.cells.map(c => word(c, c.value))))
        tab.reload()
    }

    // A preset over your own changes is asked first, but from "Back to As Sioul does now" (`direct`).
    function preset(id, direct) {
        if (tab.shown.changed !== "" && tab.presetAsked !== id && !direct) {
            tab.presetAsked = id
            return
        }
        tab.presetAsked = ""
        tab.problem = tab.sioul.reachesPreset(id)
        tab.reload()
    }

    // A tab, or a part of Your rules, from a link elsewhere or the pictures'
    // steps. The older views' names lead to their part: "person", a channel
    // and "exceptions" to People and the circles, "own" to Sioul's own
    // notices, "dnd" to Moments, "calls" to the calls' line; "table", and a
    // time or a layer ("pause", "sleep"), to What comes when.
    function show(view, detail) {
        const parts = { "person": "people", "mail": "people", "messages": "people", "calls": "calls", "exceptions": "circles", "own": "own", "dnd": "moments", "rules": "", "time": "" }
        tab.rowOpen = ""
        if (view === "table" || (view === "time" && (detail || "") !== "")) {
            tab.view = "table"
            return
        }
        tab.view = "rules"
        const part = parts[detail || ""] !== undefined && (detail || "") !== "" ? parts[detail] : parts[view] || ""
        if (part !== "")
            revealer.ask(part)
    }

    function readLists() {
        tab.lists = JSON.parse(tab.sioul.settings("senders") || "[]")
    }

    // A cell's choices in What comes when, opened by its row's and column's ids: for the pictures.
    function openCell(rowId, columnId) {
        const grid = tab.findNamed(tab, "attentionGrid")
        if (grid && tab.view === "table")
            grid.openAt(rowId, columnId)
    }

    // A circle's choices in Your rules (a computer's cell, a phone's line): for the pictures.
    function openChoice(circle, channel) {
        const item = tab.findNamed(tab, "choice-" + circle + "-" + channel) || tab.findNamed(tab, "circle-" + circle)
        if (item)
            item.clicked()
    }

    function closeChoices() {
        behaviours.close()
        cellChoices.close()
        const grid = tab.findNamed(tab, "attentionGrid")
        if (grid)
            grid.closeChoices()
    }

    // A shown item under this one, by its objectName.
    function findNamed(item, name) {
        if (!item)
            return null
        if (item.objectName === name && item.visible)
            return item
        for (let i = 0; i < item.children.length; i++) {
            const found = tab.findNamed(item.children[i], name)
            if (found)
                return found
        }
        return null
    }

    spacing: 8
    Component.onCompleted: {
        tab.reload()
        tab.readLists()
    }
    onVisibleChanged: {
        if (visible) {
            tab.reload()
            tab.readLists()
        }
    }
    onViewChanged: {
        tab.rowOpen = ""
        tab.toTop()
    }
    onRowOpenChanged: tab.toTop()

    // A part asked for, once laid out.
    Timer {
        id: revealer

        property string part: ""

        function ask(part) {
            revealer.part = part
            revealer.restart()
        }

        interval: 120
        onTriggered: {
            const item = tab.findNamed(tab, "part-" + revealer.part)
            if (item)
                tab.reveal(item)
        }
    }

    component Said: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        lineHeight: 1.2
        color: tab.theme.text
    }
    component Note: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: tab.theme.muted
    }
    component Strong: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.weight: Font.DemiBold
        color: tab.theme.text
    }
    // A link to another place: its words and an arrow.
    component GoButton: Button {
        flat: true
        icon.name: "go-next"
        icon.color: tab.theme.text
    }
    // A table's cell of words, wrapping in its share of the width.
    component Cell: Label {
        id: cell

        property bool strong: false
        property real share: 1

        Layout.fillWidth: true
        Layout.preferredWidth: cell.share
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.weight: cell.strong ? Font.DemiBold : Font.Normal
        color: tab.theme.text
    }

    // ---------------------------------------------------------------- now, the three states, where you started from
    Said {
        objectName: "reachesNow"
        text: tab.shown.now.join(" ")
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 6

        Flow {
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: [{ id: "usual", label: "reach-moment-usual" }, { id: "dnd", label: "reach-moment-dnd" }, { id: "reachable", label: "reach-moment-reachable" }]

                delegate: Label {
                    id: stateWord

                    required property var modelData
                    readonly property bool current: tab.shown.state === stateWord.modelData.id

                    objectName: "state-" + stateWord.modelData.id
                    leftPadding: 10
                    rightPadding: 10
                    topPadding: 5
                    bottomPadding: 5
                    text: tab.sioul.text(stateWord.modelData.label)
                    textFormat: Text.PlainText
                    font.weight: stateWord.current ? Font.DemiBold : Font.Normal
                    color: stateWord.current ? tab.theme.accentText : tab.theme.muted
                    Accessible.name: stateWord.text
                    background: Rectangle {
                        radius: tab.theme.radius
                        color: stateWord.current ? tab.theme.accent : "transparent"
                        border.width: stateWord.current ? 0 : 1
                        border.color: tab.theme.line
                    }
                }
            }
        }
        // The states are pressed where they always are: the status line's button and its card.
        Button {
            objectName: "reachesStates"
            text: tab.sioul.text("attention-change")
            onClicked: {
                if (tab.host && tab.host.statusLine && tab.host.statusLine.reachButton)
                    tab.host.statusLine.reachButton.openCard()
            }
        }
    }
    Flow {
        Layout.fillWidth: true
        spacing: 6

        Label {
            height: implicitHeight + 12
            verticalAlignment: Text.AlignVCenter
            text: tab.sioul.text("reach-start")
            color: tab.theme.muted
        }
        Repeater {
            model: tab.shown.presets

            delegate: Button {
                id: presetButton

                required property var modelData

                objectName: "preset-" + presetButton.modelData.id
                width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                text: tab.theme.plain(presetButton.modelData.label)
                checkable: true
                checked: presetButton.modelData.current
                highlighted: presetButton.modelData.current
                flat: !presetButton.modelData.current
                onClicked: {
                    tab.preset(presetButton.modelData.id, false)
                    presetButton.checked = Qt.binding(() => presetButton.modelData.current)
                }
            }
        }
    }
    RowLayout {
        visible: tab.shown.changed !== ""
        Layout.fillWidth: true
        spacing: 6

        Note {
            text: tab.shown.changed
        }
        Button {
            flat: true
            text: tab.sioul.text("attention-back-to-usual")
            onClicked: tab.preset("usual", true)
        }
    }
    // A preset over your own changes: said, then chosen or not.
    ColumnLayout {
        visible: tab.presetAsked !== ""
        Layout.fillWidth: true
        spacing: 4

        Said {
            text: {
                const chosen = tab.shown.presets.find(p => p.id === tab.presetAsked)
                return chosen ? tab.sioul.textWith("attention-preset-replaces", "preset", chosen.label) : ""
            }
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Button {
                text: tab.sioul.text("attention-preset-take")
                onClicked: tab.preset(tab.presetAsked, true)
            }
            Button {
                flat: true
                text: tab.sioul.text("attention-preset-keep")
                onClicked: tab.presetAsked = ""
            }
        }
    }
    Label {
        visible: tab.problem !== ""
        Layout.fillWidth: true
        text: tab.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.warm
    }

    Tabs {
        objectName: "reachesTabs"
        Layout.fillWidth: true
        Layout.topMargin: 6
        theme: tab.theme
        name: tab.sioul.text("settings-tab-attention")
        model: [{ id: "rules", text: tab.sioul.text("reach-tab-rules") }, { id: "table", text: tab.sioul.text("reach-tab-table") }]
        current: tab.view
        onChosen: id => tab.view = id
    }

    // ================================================================ Your rules
    // ---------------------------------------------------------------- People
    Section {
        objectName: "part-people"
        visible: tab.view === "rules"
        Layout.fillWidth: true
        Layout.topMargin: 6
        theme: tab.theme
        title: tab.sioul.text("reach-people")

        // A computer: circles down, mail and messages across, a choice in each cell.
        GridLayout {
            visible: !tab.narrow
            Layout.fillWidth: true
            columns: 3
            columnSpacing: 14
            rowSpacing: 6

            Item {
                Layout.row: 0
                Layout.column: 0
                Layout.fillWidth: true
                Layout.preferredWidth: 1
            }
            Repeater {
                model: tab.narrow ? [] : tab.shown.people.channels

                delegate: ColumnLayout {
                    id: head

                    required property var modelData
                    required property int index

                    Layout.row: 0
                    Layout.column: head.index + 1
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    spacing: 0

                    Strong {
                        text: head.modelData.label
                        color: tab.theme.muted
                    }
                    Note {
                        visible: head.modelData.note !== ""
                        text: head.modelData.note
                        font.pixelSize: 12
                    }
                }
            }
            Repeater {
                model: tab.narrow ? [] : tab.shown.people.rows

                delegate: ColumnLayout {
                    id: circleName

                    required property var modelData
                    required property int index

                    Layout.row: circleName.index + 1
                    Layout.column: 0
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    spacing: 0

                    Label {
                        Layout.fillWidth: true
                        text: circleName.modelData.label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: tab.theme.text
                    }
                    Note {
                        text: circleName.modelData.help
                        font.pixelSize: 12
                    }
                }
            }
            Repeater {
                model: tab.narrow ? [] : tab.peopleCells

                delegate: Button {
                    id: cellButton

                    required property var modelData

                    objectName: "choice-" + cellButton.modelData.circle.id + "-" + cellButton.modelData.cell.channel
                    Layout.row: cellButton.modelData.row
                    Layout.column: cellButton.modelData.column
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    Layout.alignment: Qt.AlignTop
                    text: tab.theme.plain(cellButton.modelData.cell.label) + "  ▾"
                    font.italic: cellButton.modelData.cell.value === "own"
                    Accessible.name: tab.theme.plain(cellButton.modelData.circle.label + ", " + (tab.shown.people.channels[cellButton.modelData.column - 1] || { label: "" }).label + ": " + cellButton.modelData.cell.label)
                    onClicked: behaviours.ask(cellButton.modelData.circle, [cellButton.modelData.cell], cellButton)
                }
            }
            Label {
                Layout.row: tab.shown.people.rows.length + 1
                Layout.column: 0
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                text: tab.shown.people.blocked.label
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: tab.theme.text
            }
            Repeater {
                model: tab.narrow ? [] : tab.shown.people.channels.length

                delegate: Label {
                    required property int index

                    Layout.row: tab.shown.people.rows.length + 1
                    Layout.column: index + 1
                    Layout.fillWidth: true
                    Layout.preferredWidth: 1
                    leftPadding: 12
                    text: tab.shown.people.blocked.word
                    textFormat: Text.PlainText
                    color: tab.theme.muted
                }
            }
        }
        // A phone: a line per circle, its two choices said under it; a press opens them.
        Repeater {
            model: tab.narrow ? tab.shown.people.rows : []

            delegate: BareButton {
                id: circleLine

                required property var modelData

                objectName: "circle-" + circleLine.modelData.id
                Layout.fillWidth: true
                implicitHeight: circleText.implicitHeight + 14
                Accessible.name: tab.theme.plain(circleLine.modelData.label + ": " + circleLine.modelData.cells.map((c, i) => (tab.shown.people.channels[i] || { label: "" }).label + " " + c.label).join(", "))
                contentItem: ColumnLayout {
                    id: circleText

                    spacing: 1

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            Layout.fillWidth: true
                            text: circleLine.modelData.label
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: tab.theme.text
                        }
                        Label {
                            text: "›"
                            color: tab.theme.muted
                        }
                    }
                    Repeater {
                        model: circleLine.modelData.cells

                        delegate: Note {
                            required property var modelData
                            required property int index

                            text: (tab.shown.people.channels[index] || { label: "" }).label + ": " + modelData.label
                            font.italic: modelData.value === "own"
                        }
                    }
                }
                onClicked: behaviours.ask(circleLine.modelData, circleLine.modelData.cells, circleLine)
            }
        }
        RowLayout {
            visible: tab.narrow
            Layout.fillWidth: true
            Layout.leftMargin: 6

            Label {
                Layout.fillWidth: true
                text: tab.shown.people.blocked.label
                textFormat: Text.PlainText
                color: tab.theme.text
            }
            Label {
                text: tab.shown.people.blocked.word
                textFormat: Text.PlainText
                color: tab.theme.muted
            }
        }
        Repeater {
            model: tab.shown.people.notes

            delegate: Note {
                required property string modelData

                Layout.topMargin: 2
                text: modelData
            }
        }
        // Mail's own: new mail told at all, newsletters.
        Repeater {
            model: ["reminders.mail", "reminders.mail_newsletters"].map(k => tab.setting(k)).filter(s => s !== null)

            delegate: SettingRow {
                required property var modelData

                Layout.fillWidth: true
                setting: modelData
                sioul: tab.sioul
                theme: tab.theme
                onSave: (key, value, shown) => tab.save(key, value, shown)
            }
        }
    }

    // Who is in which circle: Always through, then the three lists, the people on them, the groups.
    Section {
        objectName: "part-circles"
        visible: tab.view === "rules"
        Layout.fillWidth: true
        theme: tab.theme
        title: tab.sioul.text("reach-circles")

        Note {
            text: tab.sioul.text("reach-circles-help")
        }
        Strong {
            Layout.topMargin: 6
            text: tab.sioul.text("reach-circle-always")
        }
        Loader {
            active: tab.view === "rules"
            visible: active
            Layout.fillWidth: true

            sourceComponent: Component {
                DndSetup {
                    sioul: tab.sioul
                    theme: tab.theme
                    part: "list"
                }
            }
        }
        Note {
            Layout.topMargin: 6
            text: tab.sioul.text("senders-help")
        }
        Repeater {
            model: tab.view === "rules" ? tab.lists : []

            delegate: ColumnLayout {
                id: listRow

                required property var modelData
                required property int index
                readonly property bool newGroup: listRow.modelData.group !== "" && (listRow.index === 0 || tab.lists[listRow.index - 1].group !== listRow.modelData.group)

                Layout.fillWidth: true
                Layout.topMargin: listRow.newGroup ? 12 : 4
                spacing: 3

                Strong {
                    visible: listRow.newGroup
                    text: listRow.modelData.group
                }
                SettingRow {
                    Layout.fillWidth: true
                    setting: listRow.modelData
                    sioul: tab.sioul
                    theme: tab.theme
                    onSave: (key, value, shown) => {
                        const problem = tab.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
                        if (problem === "")
                            tab.readLists()
                        else
                            tab.sioul.status = problem
                    }
                }
                // Someone placed on a list by their card: their sheet, one press away.
                GoButton {
                    visible: listRow.modelData.key.startsWith("contact:")
                    text: tab.theme.plain(tab.sioul.textWith("attention-sheet-title", "name", listRow.modelData.label))
                    onClicked: {
                        if (tab.host && tab.host.openPersonSheet)
                            tab.host.openPersonSheet(listRow.modelData.key.slice("contact:".length), "")
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- Sioul's own notices
    Section {
        objectName: "part-own"
        visible: tab.view === "rules"
        Layout.fillWidth: true
        theme: tab.theme
        title: tab.sioul.text("reach-own-notices")

        Repeater {
            model: tab.shown.own

            delegate: ColumnLayout {
                id: group

                required property var modelData
                required property int index

                objectName: "group-" + group.modelData.id
                Layout.fillWidth: true
                Layout.topMargin: group.index > 0 ? 10 : 0
                spacing: 3

                Strong {
                    text: group.modelData.label
                }
                Note {
                    text: group.modelData.holds
                }
                // One choice: a radio each, none ticked while the rows are your own.
                Flow {
                    visible: group.modelData.choices.length > 0
                    Layout.fillWidth: true
                    spacing: 10

                    Repeater {
                        model: group.modelData.choices

                        delegate: RadioButton {
                            id: groupChoice

                            required property var modelData

                            objectName: "group-" + group.modelData.id + "-" + groupChoice.modelData.id
                            text: tab.theme.plain(groupChoice.modelData.label)
                            checked: group.modelData.value === groupChoice.modelData.id
                            onClicked: {
                                if (group.modelData.value !== groupChoice.modelData.id)
                                    tab.choose("group." + group.modelData.id, groupChoice.modelData.id)
                                groupChoice.checked = Qt.binding(() => group.modelData.value === groupChoice.modelData.id)
                            }
                        }
                    }
                }
                // Care: a switch each; one set cell by cell says so.
                Repeater {
                    model: group.modelData.switches

                    delegate: RowLayout {
                        id: careLine

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 6

                        Switch {
                            id: careSwitch

                            objectName: "care-" + careLine.modelData.kind
                            Layout.fillWidth: true
                            text: tab.theme.plain(careLine.modelData.label)
                            checked: careLine.modelData.on === true
                            onToggled: tab.choose("care." + careLine.modelData.kind, careSwitch.checked ? "on" : "off")
                        }
                        Label {
                            visible: careLine.modelData.on === null
                            text: tab.sioul.text("reach-own")
                            textFormat: Text.PlainText
                            font.pixelSize: 13
                            font.italic: true
                            color: tab.theme.muted
                        }
                    }
                }
                Note {
                    visible: group.modelData.value === "own" && group.modelData.choices.length > 0
                    text: tab.sioul.text("reach-own") + ": " + tab.shown.people.own_help
                    font.italic: true
                }
                Note {
                    visible: group.modelData.gathered !== "" && tab.setting("reminders.gathered") === null
                    text: group.modelData.gathered
                }
                // The gathered times themselves, under Sites and apps.
                SettingRow {
                    visible: group.modelData.id === "sites" && tab.setting("reminders.gathered") !== null
                    Layout.fillWidth: true
                    setting: tab.setting("reminders.gathered") || { key: "", kind: "note", label: "", help: "", value: [], choices: [] }
                    sioul: tab.sioul
                    theme: tab.theme
                    onSave: (key, value, shown) => tab.save(key, value, shown)
                }
            }
        }
        // How long a notice to know lasts, one length for all (docs/notifications.md): a computer's for now.
        SettingRow {
            visible: !tab.shown.phone && tab.setting("notices.informative_minutes") !== null
            Layout.fillWidth: true
            Layout.topMargin: 8
            setting: tab.setting("notices.informative_minutes") || { key: "", kind: "note", label: "", help: "", value: 0, choices: [] }
            sioul: tab.sioul
            theme: tab.theme
            onSave: (key, value, shown) => tab.save(key, value, shown)
        }
    }

    // ---------------------------------------------------------------- Moments
    Section {
        objectName: "part-moments"
        visible: tab.view === "rules"
        Layout.fillWidth: true
        theme: tab.theme
        title: tab.sioul.text("reach-moments")

        Said {
            text: tab.shown.moments.principle
            font.weight: Font.DemiBold
        }
        // A computer: the three moments down; who passes, what turns them on, the phone across.
        GridLayout {
            visible: !tab.narrow
            Layout.fillWidth: true
            Layout.topMargin: 4
            columns: 4
            columnSpacing: 14
            rowSpacing: 6

            Item {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
            }
            Repeater {
                model: tab.narrow ? [] : tab.shown.moments.columns

                delegate: Cell {
                    required property string modelData

                    share: 2
                    strong: true
                    text: modelData
                    color: tab.theme.muted
                }
            }
            Repeater {
                model: tab.narrow ? [] : tab.shown.moments.rows.reduce((out, r) => out.concat([{ text: r.label, strong: true }, { text: r.who }, { text: r.by }, { text: r.phone }]), [])

                delegate: Cell {
                    required property var modelData

                    share: modelData.strong ? 1 : 2
                    strong: modelData.strong === true
                    text: modelData.text
                }
            }
        }
        // A phone: each moment, its three lines under it.
        Repeater {
            model: tab.narrow ? tab.shown.moments.rows : []

            delegate: ColumnLayout {
                id: moment

                required property var modelData

                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 1

                Strong {
                    text: moment.modelData.label
                }
                Repeater {
                    model: [moment.modelData.who, moment.modelData.by, moment.modelData.phone]

                    delegate: Note {
                        required property string modelData
                        required property int index

                        text: (tab.shown.moments.columns[index] || "") + ": " + modelData
                    }
                }
            }
        }
        // What turns do-not-disturb on: the switch, a pause or Free time, the night, while busy.
        Strong {
            Layout.topMargin: 8
            text: tab.sioul.text("attention-dnd-turns-on")
        }
        Repeater {
            model: ["dnd.button", "dnd.pauses", "dnd.sleep", "dnd.focus", "free_time.nothing"].map(k => tab.setting(k)).filter(s => s !== null)

            delegate: SettingRow {
                required property var modelData

                Layout.fillWidth: true
                setting: modelData
                sioul: tab.sioul
                theme: tab.theme
                onSave: (key, value, shown) => tab.save(key, value, shown)
            }
        }
        // Who holds what on a phone: Sioul's part and Android's.
        Strong {
            objectName: "reachesHolds"
            Layout.topMargin: 8
            text: tab.shown.moments.holds_title
        }
        GridLayout {
            visible: !tab.narrow
            Layout.fillWidth: true
            columns: 3
            columnSpacing: 14
            rowSpacing: 6

            Item {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
            }
            Repeater {
                model: tab.narrow ? [] : tab.shown.moments.holds_columns

                delegate: Cell {
                    required property string modelData

                    strong: true
                    text: modelData
                    color: tab.theme.muted
                }
            }
            Repeater {
                model: tab.narrow ? [] : tab.shown.moments.holds.reduce((out, h) => out.concat([{ text: h.label, strong: true }, { text: h.sioul }, { text: h.android }]), [])

                delegate: Cell {
                    required property var modelData

                    strong: modelData.strong === true
                    text: modelData.text
                }
            }
        }
        Repeater {
            model: tab.narrow ? tab.shown.moments.holds : []

            delegate: ColumnLayout {
                id: hold

                required property var modelData

                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 1

                Strong {
                    text: hold.modelData.label
                }
                Note {
                    text: (tab.shown.moments.holds_columns[0] || "") + ": " + hold.modelData.sioul
                }
                Note {
                    text: (tab.shown.moments.holds_columns[1] || "") + ": " + hold.modelData.android
                }
            }
        }
        // What this device's system lets Sioul do.
        Loader {
            active: tab.view === "rules"
            visible: active
            Layout.fillWidth: true
            Layout.topMargin: 6

            sourceComponent: Component {
                DndSetup {
                    sioul: tab.sioul
                    theme: tab.theme
                    part: "device"
                }
            }
        }
    }

    // What is set elsewhere, counted, and where; calls, set per phone.
    ColumnLayout {
        objectName: "part-calls"
        visible: tab.view === "rules"
        Layout.fillWidth: true
        Layout.topMargin: 6
        Layout.bottomMargin: 18
        spacing: 2

        Note {
            objectName: "reachesElsewhere"
            text: tab.shown.elsewhere.line
        }
        Flow {
            visible: tab.shown.elsewhere.links.length > 0
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: tab.shown.elsewhere.links

                delegate: GoButton {
                    required property var modelData

                    text: tab.theme.plain(modelData.label)
                    onClicked: tab.sioul.reminderOpened(modelData.id, "", "")
                }
            }
        }
        Note {
            objectName: "reachesCalls"
            Layout.topMargin: 6
            text: tab.shown.calls
        }
        GoButton {
            objectName: "reachesDevices"
            text: tab.sioul.text("reach-calls-open")
            // This phone's card in Devices, on a phone; Devices on a computer (ParametersPage.qml's door).
            onClicked: tab.sioul.reminderOpened("settings:devices.here", "", "")
        }
    }

    // ================================================================ What comes when
    Loader {
        active: tab.view === "table" && !tab.narrow
        visible: active
        Layout.fillWidth: true
        Layout.bottomMargin: 18

        sourceComponent: Component {
            AttentionGrid {
                setting: { "grid": { columns: tab.shown.grid.columns, marks: tab.shown.grid.marks, rows: tab.gridRows } }
                sioul: tab.sioul
                theme: tab.theme
                words: true
                now: tab.shown.now_columns
                onSave: (key, value, shown) => {
                    tab.problem = tab.sioul.reachesSet(key.slice("attention.".length), JSON.stringify(value), JSON.stringify(shown))
                    tab.reload()
                }
            }
        }
    }
    // A phone: a line per row, now's value in a word; a press opens its nine values.
    Repeater {
        model: tab.view === "table" && tab.narrow && tab.openRowData === null ? tab.gridRows : []

        delegate: BareButton {
            id: rowButton

            required property var modelData
            required property int index
            readonly property bool newGroup: rowButton.index === 0 || tab.gridRows[rowButton.index - 1].group !== rowButton.modelData.group

            objectName: "row-" + rowButton.modelData.id
            Layout.fillWidth: true
            Layout.topMargin: rowButton.newGroup && rowButton.index > 0 ? 10 : 0
            implicitHeight: rowCol.implicitHeight + 10
            Accessible.name: tab.theme.plain(rowButton.modelData.group + ", " + rowButton.modelData.label)
            contentItem: ColumnLayout {
                id: rowCol

                spacing: 1

                Label {
                    visible: rowButton.newGroup
                    Layout.fillWidth: true
                    text: rowButton.modelData.group
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    color: tab.theme.accent
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        Layout.fillWidth: true
                        text: rowButton.modelData.label + (rowButton.modelData.changed ? "  •" : "")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: tab.theme.text
                    }
                    Label {
                        text: {
                            const now = tab.shown.now_columns
                            const cell = rowButton.modelData.cells.find(c => now.indexOf(c.column) >= 0)
                            return cell ? cell.word : ""
                        }
                        textFormat: Text.PlainText
                        color: tab.theme.muted
                    }
                    Label {
                        text: "›"
                        color: tab.theme.muted
                    }
                }
            }
            onClicked: tab.rowOpen = rowButton.modelData.id
        }
    }
    ColumnLayout {
        visible: tab.view === "table" && tab.narrow && tab.openRowData !== null
        Layout.fillWidth: true
        Layout.bottomMargin: 18
        spacing: 2

        BareButton {
            objectName: "rowBack"
            text: tab.theme.plain("‹  " + (tab.openRowData ? tab.openRowData.group + " · " + tab.openRowData.label : ""))
            onClicked: tab.rowOpen = ""
        }
        Note {
            text: tab.openRowData ? tab.openRowData.help : ""
        }
        Repeater {
            model: tab.openRowData ? tab.openRowData.cells : []

            delegate: BareButton {
                id: valueLine

                required property var modelData
                readonly property bool fixed: (valueLine.modelData.locked || "") !== ""
                readonly property bool current: tab.shown.now_columns.indexOf(valueLine.modelData.column) >= 0

                objectName: "line-" + tab.rowOpen + "-" + valueLine.modelData.column
                Layout.fillWidth: true
                implicitHeight: Math.max(36, valueRow.implicitHeight + 8)
                Accessible.name: valueLine.modelData.said
                contentItem: RowLayout {
                    id: valueRow

                    spacing: 8

                    Label {
                        Layout.fillWidth: true
                        text: (tab.shown.grid.columns.find(c => c.id === valueLine.modelData.column) || { label: valueLine.modelData.column }).label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.weight: valueLine.current ? Font.DemiBold : Font.Normal
                        color: valueLine.current ? tab.theme.accent : tab.theme.text
                    }
                    Label {
                        text: valueLine.modelData.word + (valueLine.modelData.changed === true ? "  •" : "")
                        textFormat: Text.PlainText
                        color: valueLine.fixed ? tab.theme.muted : tab.theme.text
                    }
                }
                onClicked: cellChoices.ask(tab.openRowData, valueLine.modelData, valueLine)
            }
        }
    }

    // A circle's behaviours, for one channel (a computer's cell) or both (a
    // phone's line): each channel's name, then its choices, the one it has
    // in bold; "Your own" said when none.
    SioulMenu {
        id: behaviours

        property var circle: null
        property var cells: []
        readonly property var lines: {
            const out = []
            for (const cell of behaviours.cells) {
                const channel = tab.shown.people.channels.find(c => c.id === cell.channel) || { label: cell.channel }
                out.push({ head: (behaviours.circle ? behaviours.circle.label + " · " : "") + channel.label })
                if (cell.value === "own")
                    out.push({ note: tab.sioul.text("reach-own") + ": " + tab.shown.people.own_help })
                for (const choice of cell.choices)
                    out.push({ id: choice.id, label: choice.label, help: choice.help || "", channel: cell.channel, current: choice.id === cell.value })
            }
            return out
        }

        function ask(circle, cells, item) {
            behaviours.circle = circle
            behaviours.cells = cells
            behaviours.popup(item, 0, item.height)
        }

        Instantiator {
            model: behaviours.lines

            delegate: MenuItem {
                id: behaviourLine

                required property var modelData
                readonly property bool said: behaviourLine.modelData.head !== undefined || behaviourLine.modelData.note !== undefined

                objectName: behaviourLine.said ? "" : "behaviour-" + behaviourLine.modelData.channel + "-" + behaviourLine.modelData.id
                enabled: !behaviourLine.said
                Accessible.checkable: !behaviourLine.said
                Accessible.checked: behaviourLine.modelData.current === true
                contentItem: ColumnLayout {
                    spacing: 0

                    Label {
                        Layout.fillWidth: true
                        Layout.maximumWidth: Math.min(420, Math.max(160, (tab.Window.window ? tab.Window.window.width : 400) - 80))
                        text: behaviourLine.modelData.head !== undefined ? behaviourLine.modelData.head : behaviourLine.modelData.note !== undefined ? behaviourLine.modelData.note : behaviourLine.modelData.label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: behaviourLine.modelData.note !== undefined ? 12 : 14
                        font.weight: behaviourLine.modelData.head !== undefined || behaviourLine.modelData.current === true ? Font.DemiBold : Font.Normal
                        color: behaviourLine.modelData.note !== undefined || behaviourLine.modelData.head !== undefined ? tab.theme.muted : behaviourLine.modelData.current === true ? tab.theme.accent : tab.theme.text
                    }
                    Label {
                        visible: (behaviourLine.modelData.help || "") !== ""
                        Layout.fillWidth: true
                        Layout.maximumWidth: Math.min(420, Math.max(160, (tab.Window.window ? tab.Window.window.width : 400) - 80))
                        text: behaviourLine.modelData.help || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 12
                        color: tab.theme.muted
                    }
                }
                onTriggered: {
                    if (behaviourLine.modelData.current !== true)
                        tab.choose("people." + behaviourLine.modelData.channel + "." + behaviours.circle.id, behaviourLine.modelData.id)
                }
            }
            onObjectAdded: (index, object) => behaviours.insertItem(index, object)
            onObjectRemoved: (index, object) => behaviours.removeItem(object)
        }
    }

    // A cell's choices on a phone's What comes when, in words: its row and
    // time, what the row is, then the values it may take; fixed, why.
    SioulMenu {
        id: cellChoices

        property var row: null
        property var cell: null
        readonly property var lines: {
            if (!cellChoices.row || !cellChoices.cell)
                return []
            const named = tab.shown.grid.columns.find(c => c.id === cellChoices.cell.column)
            const out = [{ head: cellChoices.row.label + " · " + (named ? named.label : "") }, { note: cellChoices.row.help }]
            if (cellChoices.cell.locked)
                return out.concat([{ note: cellChoices.cell.locked }])
            return out.concat(cellChoices.row.choices.filter(c => cellChoices.cell.choices.indexOf(c.id) >= 0))
        }

        function ask(row, cell, item) {
            cellChoices.row = row
            cellChoices.cell = cell
            cellChoices.popup(item, 0, item.height)
        }

        Instantiator {
            model: cellChoices.lines

            delegate: MenuItem {
                id: choiceLine

                required property var modelData
                readonly property bool said: choiceLine.modelData.head !== undefined || choiceLine.modelData.note !== undefined
                readonly property bool current: !choiceLine.said && cellChoices.cell !== null && cellChoices.cell.value === choiceLine.modelData.id

                enabled: !choiceLine.said
                Accessible.checkable: !choiceLine.said
                Accessible.checked: choiceLine.current
                contentItem: Label {
                    width: Math.min(420, Math.max(160, (tab.Window.window ? tab.Window.window.width : 400) - 80))
                    text: choiceLine.modelData.head !== undefined ? choiceLine.modelData.head : choiceLine.modelData.note !== undefined ? choiceLine.modelData.note : choiceLine.modelData.label
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: choiceLine.modelData.note !== undefined ? 12 : 14
                    font.weight: choiceLine.modelData.head !== undefined || choiceLine.current ? Font.DemiBold : Font.Normal
                    color: choiceLine.modelData.note !== undefined ? tab.theme.muted : choiceLine.current ? tab.theme.accent : tab.theme.text
                }
                onTriggered: tab.chooseCell(cellChoices.row, cellChoices.cell.column, choiceLine.modelData.id)
            }
            onObjectAdded: (index, object) => cellChoices.insertItem(index, object)
            onObjectRemoved: (index, object) => cellChoices.removeItem(object)
        }
    }
}
