// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ What reaches you (« Ce qui vous joint »; docs/attention.md): one
// place for when each thing reaches you. On top, the moment now in sentences
// and where you started from (the presets; one press back). Then five views:
// By time, a card per time in words, Change… opening that time's rows; By
// person, the channels' grids (Mail, Calls, Messages) with your lists under
// them; Sioul's own, the kinds' grid; Exceptions, Always through and each
// conversation, app or site with a choice of its own; Do not disturb, what
// turns it on, what it holds, what each device does. On a phone, never the
// whole grid: cards, then one time's rows, or one row's times, each value in
// words. The words come from the window's `reachesView` (reaches.rs); a
// change is saved at once, its row whole (`reachesSet`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: tab

    objectName: "reachesTab"

    required property var sioul
    required property var theme
    // Settings' rows: this tab's own switches (section "attention").
    required property var rows

    // A switch changed (Settings saves it, then reads its rows again).
    signal save(string key, var value)
    // Another view, time or row shown: the page scrolled back to its top.
    signal toTop

    property var shown: ({ now: [], column: "", presets: [], changed: "", cards: [], switches: [], channels: [], rows: [], grid: { columns: [], marks: [], rows: [] }, gathered: "", mail: { spam: "", areas: "" }, exceptions: { always: [], sites: [] }, phone: false })
    // "time", "person", "own", "exceptions", "dnd".
    property string view: "time"
    // By time: the time whose rows are open; "" for the cards.
    property string time: ""
    // By person: the channel shown.
    property string channel: "mail"
    // On a phone (By person, Sioul's own): the row whose times are open; "" for the list.
    property string rowOpen: ""
    property string problem: ""
    // A preset pressed while your own changes stand: asked first.
    property string presetAsked: ""
    // Who is on which list (Settings' "senders"), read when By person is first shown.
    property var lists: []
    readonly property bool narrow: tab.width > 0 && tab.width < 560
    // On a phone, a time's rows or a row's values open: the moment and the
    // presets, which are the whole matrix's, give them the screen.
    readonly property bool detail: tab.narrow && (tab.time !== "" || tab.rowOpen !== "")
    // The rows that act here, as the grid has them.
    readonly property var gridRows: tab.shown.grid.rows.filter(r => tab.shown.rows.indexOf(r.id) >= 0)
    readonly property var channelRows: tab.gridRows.filter(r => r.group_id === tab.channel)
    readonly property var ownRows: tab.gridRows.filter(r => ["mail", "calls", "messages"].indexOf(r.group_id) < 0)
    readonly property var openCard: tab.shown.cards.find(c => c.id === tab.time) || null
    readonly property var openRowData: tab.gridRows.find(r => r.id === tab.rowOpen) || null
    readonly property var channelShown: tab.shown.channels.find(c => c.id === tab.channel) || ({ id: "mail", label: "", note: "" })
    // On a phone, one row's times open: that alone, under the view's buttons.
    readonly property bool rowShown: tab.narrow && tab.openRowData !== null

    function reload() {
        tab.shown = JSON.parse(tab.sioul.reachesView() || "null") || tab.shown
    }

    function setting(key) {
        return tab.rows.find(r => r.key === key) || null
    }

    // A row with one cell changed, saved whole.
    function choose(row, column, value) {
        const words = row.cells.map(c => {
            const v = c.column === column ? value : c.value
            return v === "now" ? c.column : c.column + ":" + v
        })
        tab.saveRow(row.id, words)
    }

    function saveRow(id, words) {
        tab.problem = tab.sioul.reachesSet(id, JSON.stringify(words))
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

    // A view, and in it a time, a channel or a row: from a link elsewhere
    // ("attention.pause", "attention.calls"), or the pictures' steps.
    function show(view, detail) {
        tab.view = view
        tab.time = view === "time" ? (detail || "") : ""
        tab.rowOpen = ""
        if (view === "person" && detail)
            tab.channel = detail
    }

    function readLists() {
        tab.lists = JSON.parse(tab.sioul.settings("senders") || "[]")
    }

    // A cell's choices in words, opened by its row's and column's ids: for the pictures.
    function openCell(rowId, columnId) {
        const row = tab.gridRows.find(r => r.id === rowId)
        if (!row)
            return
        const grid = tab.findGrid(tab)
        if (grid && !tab.narrow && tab.time === "") {
            grid.openAt(rowId, columnId)
            return
        }
        // Under its line, as a press opens them.
        choices.ask(row, columnId, tab.findNamed(tab, "line-" + rowId + "-" + columnId) || tab)
    }

    function closeChoices() {
        choices.close()
        const grid = tab.findGrid(tab)
        if (grid)
            grid.closeChoices()
    }

    function findGrid(item) {
        return tab.findNamed(item, "attentionGrid")
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

    // A value in the row's own words ("Shown, not told", "Voicemail, listed later").
    function valueWords(row, value) {
        const choice = row.choices.find(c => c.id === value)
        return choice ? choice.label : value
    }

    spacing: 8
    Component.onCompleted: tab.reload()
    onVisibleChanged: if (visible) tab.reload()
    onViewChanged: {
        tab.rowOpen = ""
        if (tab.view === "person" && tab.lists.length === 0)
            tab.readLists()
        tab.toTop()
    }
    onTimeChanged: tab.toTop()
    onRowOpenChanged: tab.toTop()
    onChannelChanged: tab.toTop()

    component Heading: Label {
        Layout.fillWidth: true
        Layout.topMargin: 14
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: tab.theme.accent
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

    // ---------------------------------------------------------------- now, and where you started from
    Said {
        visible: !tab.detail
        objectName: "reachesNow"
        text: tab.shown.now.join(" ")
    }
    Flow {
        visible: !tab.detail
        Layout.fillWidth: true
        spacing: 6

        Label {
            height: implicitHeight + 12
            verticalAlignment: Text.AlignVCenter
            text: tab.sioul.text("attention-start-from")
            color: tab.theme.muted
        }
        Repeater {
            model: tab.shown.presets

            delegate: Button {
                id: presetButton

                required property var modelData

                width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                text: presetButton.modelData.label
                checkable: true
                checked: presetButton.modelData.current
                flat: !presetButton.modelData.current
                onClicked: {
                    tab.preset(presetButton.modelData.id, false)
                    presetButton.checked = Qt.binding(() => presetButton.modelData.current)
                }
            }
        }
    }
    RowLayout {
        visible: tab.shown.changed !== "" && !tab.detail
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

    // The views, as the page's tabs are drawn.
    Flow {
        Layout.fillWidth: true
        Layout.topMargin: 6
        spacing: 6

        Repeater {
            model: ["time", "person", "own", "exceptions", "dnd"]

            delegate: Button {
                id: viewButton

                required property string modelData

                text: tab.sioul.text("attention-view-" + viewButton.modelData)
                checkable: true
                checked: tab.view === viewButton.modelData
                flat: tab.view !== viewButton.modelData
                onClicked: {
                    tab.show(viewButton.modelData, "")
                    viewButton.checked = Qt.binding(() => tab.view === viewButton.modelData)
                }
            }
        }
    }

    // ---------------------------------------------------------------- By time: a card per time
    GridLayout {
        visible: tab.view === "time" && tab.time === ""
        Layout.fillWidth: true
        columns: tab.width >= 900 ? 3 : tab.width >= 560 ? 2 : 1
        columnSpacing: tab.theme.gap
        rowSpacing: tab.theme.gap

        Repeater {
            model: tab.shown.cards

            delegate: Panel {
                id: card

                required property var modelData

                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.preferredWidth: 1
                objectName: "card-" + card.modelData.id
                theme: tab.theme
                accent: card.modelData.now

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 5

                    Label {
                        Layout.fillWidth: true
                        text: card.modelData.label + (card.modelData.now ? " · " + tab.sioul.text("attention-now-mark") : "")
                        font.pixelSize: 16
                        font.weight: Font.DemiBold
                        wrapMode: Text.Wrap
                        color: tab.theme.text
                    }
                    Note {
                        text: card.modelData.when
                    }
                    Repeater {
                        model: card.modelData.lines

                        delegate: Said {
                            required property string modelData

                            text: modelData
                        }
                    }
                    Note {
                        text: card.modelData.system
                    }
                    Repeater {
                        model: card.modelData.also

                        delegate: Note {
                            required property string modelData

                            text: modelData
                        }
                    }
                    // Free time's own choice: nothing at all, not even your safe senders.
                    SettingRow {
                        visible: card.modelData.id === "free" && tab.setting("free_time.nothing") !== null
                        Layout.fillWidth: true
                        setting: tab.setting("free_time.nothing") || { key: "", kind: "note", label: "", help: "", value: false, choices: [] }
                        sioul: tab.sioul
                        theme: tab.theme
                        onSave: (key, value) => tab.save(key, value)
                    }
                    Item {
                        Layout.fillHeight: true
                    }
                    Button {
                        Layout.alignment: Qt.AlignRight
                        flat: true
                        text: tab.sioul.text("attention-change")
                        Accessible.name: tab.sioul.text("attention-change") + " " + card.modelData.label
                        onClicked: tab.time = card.modelData.id
                    }
                }
            }
        }
    }
    // Let every call through, Real time: a line each.
    Repeater {
        model: tab.view === "time" && tab.time === "" ? tab.shown.switches : []

        delegate: Note {
            required property string modelData

            text: modelData
        }
    }

    // ---------------------------------------------------------------- a time's rows
    ColumnLayout {
        visible: tab.view === "time" && tab.openCard !== null
        Layout.fillWidth: true
        spacing: 4

        Button {
            flat: true
            text: "‹  " + tab.sioul.text("attention-all-times")
            onClicked: tab.time = ""
        }
        Label {
            Layout.fillWidth: true
            text: tab.openCard ? tab.openCard.label : ""
            font.pixelSize: 17
            font.weight: Font.DemiBold
            wrapMode: Text.Wrap
            color: tab.theme.text
        }
        Note {
            text: tab.openCard ? tab.openCard.when : ""
        }
        Note {
            text: tab.sioul.text("attention-rows-help")
        }
        Repeater {
            model: tab.view === "time" && tab.openCard !== null ? tab.gridRows : []

            delegate: ColumnLayout {
                id: timeRow

                required property var modelData
                required property int index
                readonly property var cell: timeRow.modelData.cells.find(c => c.column === tab.time) || ({ value: "", locked: "", changed: false, said: "", choices: [] })
                readonly property bool newGroup: timeRow.index === 0 || tab.gridRows[timeRow.index - 1].group !== timeRow.modelData.group

                Layout.fillWidth: true
                spacing: 0

                Label {
                    visible: timeRow.newGroup
                    Layout.fillWidth: true
                    Layout.topMargin: 10
                    text: timeRow.modelData.group
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    color: tab.theme.accent
                }
                ValueLine {
                    Layout.fillWidth: true
                    objectName: "line-" + timeRow.modelData.id + "-" + tab.time
                    name: timeRow.modelData.label
                    row: timeRow.modelData
                    cell: timeRow.cell
                    onPicked: item => choices.ask(timeRow.modelData, tab.time, item)
                }
            }
        }
    }

    // A row's line: its name, its mark and its value in words, a dot when
    // changed; a press opens its choices (fixed: why).
    component ValueLine: AbstractButton {
        id: line

        required property string name
        required property var row
        required property var cell
        signal picked(var item)

        readonly property bool fixed: (line.cell.locked || "") !== ""

        implicitHeight: Math.max(36, lineRow.implicitHeight + 8)
        hoverEnabled: true
        Accessible.role: Accessible.Button
        Accessible.name: line.cell.said
        background: Rectangle {
            radius: tab.theme.radius
            color: line.down ? tab.theme.pressed : line.hovered || line.visualFocus ? tab.theme.hover : "transparent"
        }
        contentItem: RowLayout {
            id: lineRow

            spacing: 8

            Label {
                Layout.fillWidth: true
                // Its own width counts for nothing: it takes what is left, and wraps.
                Layout.preferredWidth: 1
                Layout.leftMargin: 6
                text: line.name
                wrapMode: Text.Wrap
                color: tab.theme.text
            }
            LevelMark {
                value: line.cell.value
                always: line.row.id.endsWith(".always")
                tint: line.fixed ? tab.theme.muted : tab.theme.text
                hole: tab.theme.background
                opacity: line.fixed ? 0.6 : 1
            }
            // The values in one column, their marks before them, aligned.
            Label {
                Layout.preferredWidth: Math.round(line.width * 0.42)
                text: tab.valueWords(line.row, line.cell.value)
                wrapMode: Text.Wrap
                color: line.fixed ? tab.theme.muted : tab.theme.text
            }
            Rectangle {
                Layout.preferredWidth: 6
                Layout.preferredHeight: 6
                Layout.rightMargin: 6
                radius: 3
                color: line.cell.changed === true ? tab.theme.accent : "transparent"
            }
        }
        onClicked: line.picked(line)
    }

    // A list of rows (a phone's By person and Sioul's own): each opens its times.
    component RowList: ColumnLayout {
        id: list

        required property var model

        spacing: 0

        Repeater {
            model: list.model

            delegate: AbstractButton {
                id: rowButton

                required property var modelData
                required property int index
                readonly property bool newGroup: rowButton.index === 0 || list.model[rowButton.index - 1].group !== rowButton.modelData.group

                Layout.fillWidth: true
                Layout.topMargin: rowButton.newGroup && rowButton.index > 0 ? 10 : 0
                implicitHeight: rowCol.implicitHeight + 10
                hoverEnabled: true
                Accessible.role: Accessible.Button
                Accessible.name: rowButton.modelData.label
                background: Rectangle {
                    radius: tab.theme.radius
                    color: rowButton.down ? tab.theme.pressed : rowButton.hovered || rowButton.visualFocus ? tab.theme.hover : "transparent"
                }
                contentItem: ColumnLayout {
                    id: rowCol

                    spacing: 1

                    Label {
                        visible: rowButton.newGroup
                        Layout.fillWidth: true
                        Layout.leftMargin: 6
                        text: rowButton.modelData.group
                        font.pixelSize: 13
                        font.weight: Font.DemiBold
                        color: tab.theme.accent
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            Layout.leftMargin: 6
                            text: rowButton.modelData.label + (rowButton.modelData.changed ? "  •" : "")
                            wrapMode: Text.Wrap
                            color: tab.theme.text
                        }
                        Label {
                            Layout.rightMargin: 6
                            text: "›"
                            color: tab.theme.muted
                        }
                    }
                }
                onClicked: tab.rowOpen = rowButton.modelData.id
            }
        }
    }

    // A row's nine values, on a phone: its times, then its two layers.
    component RowValues: ColumnLayout {
        id: values

        required property var row

        spacing: 2

        Button {
            flat: true
            text: "‹  " + values.row.group + " · " + values.row.label
            onClicked: tab.rowOpen = ""
        }
        Note {
            text: values.row.help
        }
        Repeater {
            model: values.row.cells

            delegate: ValueLine {
                id: valueLine

                required property var modelData

                Layout.fillWidth: true
                objectName: "line-" + values.row.id + "-" + valueLine.modelData.column
                name: (tab.shown.grid.columns.find(c => c.id === valueLine.modelData.column) || { label: valueLine.modelData.column }).label
                row: values.row
                cell: valueLine.modelData
                onPicked: item => choices.ask(values.row, valueLine.modelData.column, item)
            }
        }
    }

    // ---------------------------------------------------------------- By person
    Flow {
        visible: tab.view === "person"
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: tab.shown.channels

            delegate: Button {
                id: channelButton

                required property var modelData

                text: channelButton.modelData.label
                checkable: true
                checked: tab.channel === channelButton.modelData.id
                flat: !checked
                onClicked: {
                    tab.channel = channelButton.modelData.id
                    tab.rowOpen = ""
                    channelButton.checked = Qt.binding(() => tab.channel === channelButton.modelData.id)
                }
            }
        }
    }
    Note {
        visible: tab.view === "person" && !tab.rowShown && tab.channelShown.note !== ""
        text: tab.channelShown.note
    }
    // Mail's own: new mail told at all, newsletters; what the spam filter and an address's area hold.
    Repeater {
        model: tab.view === "person" && !tab.rowShown && tab.channel === "mail" ? ["reminders.mail", "reminders.mail_newsletters"].map(k => tab.setting(k)).filter(s => s !== null) : []

        delegate: SettingRow {
            required property var modelData

            Layout.fillWidth: true
            setting: modelData
            sioul: tab.sioul
            theme: tab.theme
            onSave: (key, value) => tab.save(key, value)
        }
    }
    Note {
        visible: tab.view === "person" && !tab.rowShown && tab.channel === "mail"
        text: tab.shown.mail.spam
    }
    Note {
        visible: tab.view === "person" && !tab.rowShown && tab.channel === "mail"
        text: tab.shown.mail.areas
    }
    Note {
        visible: tab.view === "person" && (!tab.narrow || tab.openRowData === null)
        text: tab.sioul.text(tab.narrow ? "attention-person-list-help" : "attention-person-grid-help")
    }
    Loader {
        active: tab.view === "person" && !tab.narrow
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            AttentionGrid {
                setting: { "grid": { columns: tab.shown.grid.columns, marks: tab.shown.grid.marks, rows: tab.channelRows } }
                sioul: tab.sioul
                theme: tab.theme
                onSave: (key, value) => tab.saveRow(key.slice("attention.".length), value)
            }
        }
    }
    Loader {
        active: tab.view === "person" && tab.narrow
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            ColumnLayout {
                spacing: 0

                RowList {
                    visible: tab.openRowData === null
                    Layout.fillWidth: true
                    model: tab.channelRows
                }
                RowValues {
                    visible: tab.openRowData !== null
                    Layout.fillWidth: true
                    row: tab.openRowData || { id: "", label: "", group: "", help: "", cells: [], choices: [] }
                }
            }
        }
    }
    // Who is on which list: the four lists, the people on a list, your contacts' categories.
    Heading {
        visible: tab.view === "person" && !tab.rowShown
        text: tab.sioul.text("attention-lists")
    }
    Note {
        visible: tab.view === "person" && !tab.rowShown
        text: tab.sioul.text("senders-help")
    }
    Repeater {
        model: tab.view === "person" && !tab.rowShown ? tab.lists : []

        delegate: ColumnLayout {
            id: listRow

            required property var modelData
            required property int index
            readonly property bool newGroup: listRow.modelData.group !== "" && (listRow.index === 0 || tab.lists[listRow.index - 1].group !== listRow.modelData.group)

            Layout.fillWidth: true
            Layout.topMargin: listRow.newGroup ? 14 : 6
            spacing: 3

            Label {
                visible: listRow.newGroup
                Layout.fillWidth: true
                text: listRow.modelData.group
                font.pixelSize: 15
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                color: tab.theme.accent
            }
            SettingRow {
                Layout.fillWidth: true
                setting: listRow.modelData
                sioul: tab.sioul
                theme: tab.theme
                onSave: (key, value) => {
                    const problem = tab.sioul.setSetting(key, JSON.stringify(value))
                    if (problem === "")
                        tab.readLists()
                    else
                        tab.sioul.status = problem
                }
            }
            // Someone placed on a list by their card: their sheet, one press away.
            Button {
                visible: listRow.modelData.key.startsWith("contact:")
                flat: true
                text: tab.sioul.textWith("attention-sheet-title", "name", listRow.modelData.label)
                icon.name: "go-next"
                icon.color: tab.theme.text
                onClicked: {
                    const window = tab.Window.window
                    if (window && window.openPersonSheet)
                        window.openPersonSheet(listRow.modelData.key.slice("contact:".length), "")
                }
            }
        }
    }

    // ---------------------------------------------------------------- Sioul's own
    Note {
        visible: tab.view === "own" && (!tab.narrow || tab.openRowData === null)
        text: tab.sioul.text(tab.narrow ? "attention-own-help-list" : "attention-own-help")
    }
    SettingRow {
        visible: tab.view === "own" && !tab.rowShown && !tab.shown.phone && tab.setting("reminders.gather") !== null
        Layout.fillWidth: true
        setting: tab.setting("reminders.gather") || { key: "", kind: "note", label: "", help: "", value: false, choices: [] }
        sioul: tab.sioul
        theme: tab.theme
        onSave: (key, value) => tab.save(key, value)
    }
    Note {
        visible: tab.view === "own" && !tab.rowShown
        text: tab.shown.gathered
    }
    Loader {
        active: tab.view === "own" && !tab.narrow
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            AttentionGrid {
                setting: { "grid": { columns: tab.shown.grid.columns, marks: tab.shown.grid.marks, rows: tab.ownRows } }
                sioul: tab.sioul
                theme: tab.theme
                onSave: (key, value) => tab.saveRow(key.slice("attention.".length), value)
            }
        }
    }
    Loader {
        active: tab.view === "own" && tab.narrow
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            ColumnLayout {
                spacing: 0

                RowList {
                    visible: tab.openRowData === null
                    Layout.fillWidth: true
                    model: tab.ownRows
                }
                RowValues {
                    visible: tab.openRowData !== null
                    Layout.fillWidth: true
                    row: tab.openRowData || { id: "", label: "", group: "", help: "", cells: [], choices: [] }
                }
            }
        }
    }

    // ---------------------------------------------------------------- Exceptions
    Heading {
        visible: tab.view === "exceptions"
        text: tab.sioul.text("attention-person-always")
    }
    Repeater {
        model: tab.view === "exceptions" ? tab.shown.exceptions.always : []

        delegate: Said {
            required property string modelData

            text: modelData
        }
    }
    Loader {
        active: tab.view === "exceptions"
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
    // A phone: each conversation, app and browser site with a choice of its own.
    Loader {
        active: tab.view === "exceptions" && tab.shown.phone
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            AppNotesSetup {
                sioul: tab.sioul
                theme: tab.theme
                part: "lists"
            }
        }
    }
    // A computer: each site's own choice, changed on the site.
    Heading {
        visible: tab.view === "exceptions" && !tab.shown.phone
        text: tab.sioul.text("attention-exceptions-sites")
    }
    Repeater {
        model: tab.view === "exceptions" && !tab.shown.phone ? tab.shown.exceptions.sites : []

        delegate: Said {
            required property string modelData

            text: modelData
        }
    }
    Note {
        visible: tab.view === "exceptions" && !tab.shown.phone
        text: tab.sioul.text("attention-exceptions-sites-help")
    }
    Button {
        visible: tab.view === "exceptions" && !tab.shown.phone
        flat: true
        text: tab.sioul.text("attention-open-sites")
        icon.name: "go-next"
        icon.color: tab.theme.text
        onClicked: tab.sioul.reminderOpened("sites", "", "")
    }
    Heading {
        visible: tab.view === "exceptions"
        text: tab.sioul.text("attention-exceptions-events")
    }
    Said {
        visible: tab.view === "exceptions"
        text: tab.sioul.text("attention-exceptions-events-help")
    }
    Button {
        visible: tab.view === "exceptions"
        flat: true
        text: tab.sioul.text("attention-open-agenda")
        icon.name: "go-next"
        icon.color: tab.theme.text
        onClicked: tab.sioul.reminderOpened("agenda", "", "")
    }
    Heading {
        visible: tab.view === "exceptions"
        text: tab.sioul.text("attention-exceptions-health")
    }
    Said {
        visible: tab.view === "exceptions"
        text: tab.sioul.text("attention-exceptions-health-help")
    }
    Button {
        visible: tab.view === "exceptions"
        Layout.bottomMargin: 18
        flat: true
        text: tab.sioul.text("attention-open-health")
        icon.name: "go-next"
        icon.color: tab.theme.text
        onClicked: tab.sioul.reminderOpened("needs", "", "")
    }

    // ---------------------------------------------------------------- Do not disturb
    Heading {
        visible: tab.view === "dnd"
        text: tab.sioul.text("attention-dnd-turns-on")
    }
    Repeater {
        model: tab.view === "dnd" ? ["dnd.button", "dnd.focus", "dnd.pauses", "dnd.sleep"].map(k => tab.setting(k)).filter(s => s !== null) : []

        delegate: SettingRow {
            required property var modelData

            Layout.fillWidth: true
            Layout.topMargin: 4
            setting: modelData
            sioul: tab.sioul
            theme: tab.theme
            onSave: (key, value) => tab.save(key, value)
        }
    }
    Heading {
        visible: tab.view === "dnd"
        text: tab.sioul.text("attention-dnd-while")
    }
    Repeater {
        model: tab.view === "dnd" ? (tab.shown.cards.find(c => c.id === "dnd") || { lines: [] }).lines : []

        delegate: Said {
            required property string modelData

            text: modelData
        }
    }
    Note {
        visible: tab.view === "dnd"
        text: (tab.shown.cards.find(c => c.id === "dnd") || { system: "" }).system
    }
    Button {
        visible: tab.view === "dnd"
        flat: true
        text: tab.sioul.text("attention-change")
        onClicked: tab.show("time", "dnd")
    }
    Loader {
        active: tab.view === "dnd"
        visible: active
        Layout.fillWidth: true
        Layout.bottomMargin: 18

        sourceComponent: Component {
            DndSetup {
                sioul: tab.sioul
                theme: tab.theme
                part: "device"
            }
        }
    }

    // A cell's choices, in words: its row and time, what the row is, then the
    // values it may take, the one it has in bold; fixed, why.
    SioulMenu {
        id: choices

        property var row: null
        property string column: ""
        readonly property var cell: choices.row ? choices.row.cells.find(c => c.column === choices.column) || null : null
        readonly property var lines: {
            if (!choices.row || !choices.cell)
                return []
            const named = tab.shown.grid.columns.find(c => c.id === choices.column)
            const channel = ["mail", "calls", "messages"].indexOf(choices.row.group_id) >= 0 ? choices.row.group + " · " : ""
            const out = [{ head: channel + choices.row.label + " · " + (named ? named.label : "") }, { note: choices.row.help }]
            if (choices.cell.locked)
                return out.concat([{ note: choices.cell.locked }])
            return out.concat(choices.row.choices.filter(c => choices.cell.choices.indexOf(c.id) >= 0))
        }

        // Under the line: below its value on a wide screen, from its start on a phone.
        function ask(row, column, item) {
            choices.row = row
            choices.column = column
            choices.popup(item, tab.narrow || item === tab ? 0 : Math.round(item.width / 2), item.height)
        }

        Instantiator {
            model: choices.lines

            delegate: MenuItem {
                id: choiceLine

                required property var modelData
                readonly property bool said: choiceLine.modelData.head !== undefined || choiceLine.modelData.note !== undefined
                readonly property bool current: !choiceLine.said && choices.cell !== null && choices.cell.value === choiceLine.modelData.id

                enabled: !choiceLine.said
                Accessible.checkable: !choiceLine.said
                Accessible.checked: choiceLine.current
                contentItem: RowLayout {
                    spacing: 8

                    LevelMark {
                        visible: !choiceLine.said
                        value: choiceLine.said ? "" : choiceLine.modelData.id
                        always: choices.row !== null && choices.row.id.endsWith(".always")
                        tint: choiceLine.current ? tab.theme.accent : tab.theme.text
                        hole: tab.theme.surface
                    }
                    Label {
                        Layout.fillWidth: true
                        Layout.maximumWidth: Math.min(420, Math.max(160, (tab.Window.window ? tab.Window.window.width : 400) - 110))
                        text: choiceLine.modelData.head !== undefined ? choiceLine.modelData.head : choiceLine.modelData.note !== undefined ? choiceLine.modelData.note : choiceLine.modelData.label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: choiceLine.modelData.note !== undefined ? 12 : 14
                        font.weight: choiceLine.modelData.head !== undefined || choiceLine.current ? Font.DemiBold : Font.Normal
                        color: choiceLine.modelData.note !== undefined ? tab.theme.muted : choiceLine.current ? tab.theme.accent : tab.theme.text
                    }
                }
                onTriggered: tab.choose(choices.row, choices.column, choiceLine.modelData.id)
            }
            onObjectAdded: (index, object) => choices.insertItem(index, object)
            onObjectRemoved: (index, object) => choices.removeItem(object)
        }
    }
}
