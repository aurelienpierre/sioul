// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ What reaches you (« Ce qui vous joint »; docs/attention.md, "The
// page"; the redesign of 24 November 2026): the rule once, then only the
// exceptions. On top, the sentence for now and the three states. Then:
// - The usual rule: a sentence per circle, with how many people and groups
//   it holds, and the one drop-down "Start from"; its explanation behind "?".
// - Your exceptions: a sentence each, read from every store as differences
//   from the usual (sioul_core::exceptions), with Edit and ✕ (Undo in the
//   status line), and where it does nothing here, a line saying so; "Add an
//   exception…" opens the one card (ExceptionCard.qml).
// - Do not disturb comes on by itself: its switches.
// - Other apps on your phones (on a computer, its own notifications): the
//   switches of this phone for other apps, the gathered times.
// - Folded: Who is in which circle (Always through, your people by name,
//   the lists of addresses and numbers), and What comes when, worked out:
//   the whole grid, read-only, a press on a cell saying which line decides
//   it, with "Add an exception for this…" (AttentionGrid.qml on a computer,
//   rows that open on a phone).
// The words come from the window's `reachesView` (reaches.rs); what is asked
// and changed goes through `reachesAct`.

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
    // The page scrolled back to its top.
    signal toTop
    // A part asked for (a door from elsewhere): the page scrolled to it.
    signal reveal(var item)

    property var shown: ({ now: [], state: "usual", phone: false, start: { id: "usual", choices: [] }, rule: [], idle: [], exceptions: [], card: { what: [], whom: [], where: [] }, now_columns: [], grid: { columns: [], marks: [], rows: [] }, circle: { title: "", help: "", people: [] } })
    // "rules", or "table" while What comes when, worked out, is open (Settings is wider then).
    property string view: "rules"
    // On a phone, What comes when: the row whose moments are open; "" for the list.
    property string rowOpen: ""
    property string problem: ""
    // An exception whose ✕ asks first (a phone's table of calls): its place in the list; -1 for none.
    property int removeAsked: -1
    // A start that replaces exceptions, asked first: its id and the question.
    property string startAsked: ""
    property string startQuestion: ""
    // The usual rule's explanation, behind its "?".
    property bool helpOpen: false
    // Who is in which circle's lists (Settings' "senders"): the addresses, numbers and domains.
    property var lists: []
    // On a phone, its own switches for other apps (appnotes.rs's `setup`: hold, people, shade).
    property var apps: null
    readonly property bool narrow: tab.width > 0 && tab.width < 560
    // The window (main.qml): its status line's button and a person's sheet.
    readonly property var host: tab.Window.window
    // What comes when's rows, and the one open on a phone.
    readonly property var gridRows: tab.shown.grid.rows || []
    readonly property var openRowData: tab.gridRows.find(r => r.id === tab.rowOpen) || null
    // The card that adds or changes an exception, once made.
    readonly property var card: cardLoader.item

    function reload() {
        tab.shown = JSON.parse(tab.sioul.reachesView() || "null") || tab.shown
        if (tab.shown.phone)
            tab.apps = JSON.parse(tab.sioul.appNotesSetup() || "null")
    }

    function setting(key) {
        return tab.rows.find(r => r.key === key) || null
    }

    // Asked of the backend (`reachesAct`): its answer, read; a change reads the page again.
    function act(verb, args) {
        const answer = JSON.parse(tab.sioul.reachesAct(verb, JSON.stringify(args || {})) || "null") || {}
        tab.problem = answer.problem || ""
        return answer
    }

    // An exception taken out: said in the status line with Undo. One that takes more with it
    // (a phone's whole table of calls: its groups' rows, its switches) asks first, under its line.
    function remove(line, index, sure) {
        if ((line.ask || "") !== "" && sure !== true) {
            tab.removeAsked = index
            return
        }
        tab.removeAsked = -1
        tab.act("remove", { value: line.value })
        tab.reload()
    }

    // The card, for `exception` (as listed) or a new one starting from `start` (a cell's).
    function openCard(exception, start) {
        cardLoader.active = true
        const card = cardLoader.item as ExceptionCard
        if (card)
            card.edit(exception || null, start || null)
    }

    // A start chosen: asked first when it replaces some of your exceptions (`sure` once asked).
    function chooseStart(id, sure) {
        const answer = tab.act("start", { id: id, sure: sure === true })
        tab.startAsked = answer.ask ? id : ""
        tab.startQuestion = answer.ask || ""
        tab.reload()
    }

    // A part of the page, from a door elsewhere or the pictures' steps. The
    // older views' names lead to their part: "person", "people", a channel and
    // "rules" to the usual rule; "own" and "exceptions" to your exceptions;
    // "dnd" to do-not-disturb; "circles" to Who is in which circle, unfolded;
    // "table", and a time or a layer ("pause", "sleep"), to What comes when,
    // worked out, unfolded (on a phone at a row, by its id).
    function show(view, detail) {
        const parts = { "person": "rule", "people": "rule", "mail": "rule", "messages": "rule", "calls": "rule", "own": "exceptions", "exceptions": "exceptions", "dnd": "dnd", "apps": "apps", "gathered": "apps", "circles": "circles" }
        tab.rowOpen = ""
        if (view === "table" || (view === "time" && (detail || "") !== "")) {
            workedOut.want(true)
            if (tab.gridRows.some(r => r.id === detail) && tab.narrow)
                tab.rowOpen = detail
            revealer.ask("worked-out")
            return
        }
        const part = parts[detail || ""] || parts[view] || ""
        workedOut.want(false)
        if (part === "circles")
            circles.want(true)
        // The page itself ("rules", or nothing said): its top, with now and the three states.
        if (part !== "")
            revealer.ask(part)
        else
            tab.toTop()
    }

    // The status line's card, where the three states are pressed with their lengths.
    function openCard_() {
        if (tab.host && tab.host.statusLine && tab.host.statusLine.reachButton)
            tab.host.statusLine.reachButton.openCard()
    }

    // One of the lists of addresses, numbers and domains changed, over the file as it is now.
    function saveList(key, value, shown) {
        const problem = tab.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
        if (problem === "")
            tab.readLists()
        else
            tab.sioul.status = problem
    }

    function readLists() {
        // The lists of addresses, numbers and domains alone: a person's list is on their sheet, a group's an exception.
        tab.lists = JSON.parse(tab.sioul.settings("senders") || "[]").filter(r => ["safe", "restricted", "blocked", "neutral"].indexOf(r.key) >= 0)
    }

    // A cell of What comes when, opened by its row's and column's ids: for the pictures.
    function openCell(rowId, columnId) {
        workedOut.want(true)
        const grid = tab.findNamed(tab, "attentionGrid")
        if (grid)
            grid.openAt(rowId, columnId)
    }

    function closeChoices() {
        const grid = tab.findNamed(tab, "attentionGrid")
        if (grid)
            grid.closeChoices()
        lineWhy.close()
        if (cardLoader.item)
            (cardLoader.item as ExceptionCard).close()
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

    spacing: tab.theme.sectionGap
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
    // A phone's row of What comes when opened, or closed: the fold brought into view.
    onRowOpenChanged: revealer.ask("worked-out")

    // Read again when the moment changes (an Undo in the status line among the rest).
    Connections {
        target: tab.sioul
        ignoreUnknownSignals: true
        function onModeChanged() {
            if (tab.visible)
                tab.reload()
        }
    }

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
    // One of this page's settings (Settings' rows), by its key; none when the build has none.
    component Row_: SettingRow {
        required property string key

        objectName: "setting:" + key
        visible: tab.setting(key) !== null
        Layout.fillWidth: true
        setting: tab.setting(key) || { key: "", kind: "note", label: "", help: "", value: false, choices: [] }
        sioul: tab.sioul
        theme: tab.theme
        onSave: (key, value, shown) => tab.save(key, value, shown)
    }

    // ---------------------------------------------------------------- now, the three states
    ColumnLayout {
        Layout.fillWidth: true
        spacing: tab.theme.rowGap

        Said {
            objectName: "reachesNow"
            text: tab.shown.now.join(" ")
        }
        // The three states, the one on marked; the status line's card, where they are set, opens
        // from Change… on a computer, from the row itself on a phone.
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            // On a phone they wrap rather than pass the screen's edge.
            Flow {
                Layout.fillWidth: true
                spacing: tab.narrow ? 4 : 6
                Accessible.role: tab.narrow ? Accessible.Button : Accessible.Grouping
                Accessible.name: tab.narrow ? tab.sioul.text("attention-change") : ""

                Repeater {
                    model: [{ id: "usual", label: "reach-moment-usual" }, { id: "dnd", label: "reach-moment-dnd" }, { id: "reachable", label: "reach-moment-reachable" }]

                    delegate: Label {
                        id: stateWord

                        required property var modelData
                        readonly property bool current: tab.shown.state === stateWord.modelData.id

                        objectName: "state-" + stateWord.modelData.id
                        leftPadding: tab.narrow ? 7 : 10
                        rightPadding: tab.narrow ? 7 : 10
                        topPadding: 5
                        bottomPadding: 5
                        // The one on said by a mark and to a screen reader, not by its colour alone.
                        text: (stateWord.current ? "✓ " : "") + tab.sioul.text(stateWord.modelData.label)
                        textFormat: Text.PlainText
                        font.pixelSize: tab.narrow ? 13 : 14
                        font.weight: stateWord.current ? Font.DemiBold : Font.Normal
                        color: stateWord.current ? tab.theme.text : tab.theme.muted
                        Accessible.name: tab.sioul.text(stateWord.modelData.label) + (stateWord.current ? ", " + tab.sioul.text("reach-state-now") : "")
                        // Selected (the rulebook's rule 5): the accent's frame and tint, bold, a ✓. The
                        // other two are words read, never framed as buttons: they are set in the status
                        // line's card (Change…), not here (rule 11).
                        background: Rectangle {
                            radius: tab.theme.radius
                            color: stateWord.current ? tab.theme.chosenTint : "transparent"
                            border.width: stateWord.current ? 2 : 0
                            border.color: tab.theme.accent
                        }
                    }
                }
                TapHandler {
                    enabled: tab.narrow
                    onTapped: tab.openCard_()
                }
            }
            Button {
                objectName: "reachesStates"
                visible: !tab.narrow
                text: tab.sioul.text("attention-change")
                onClicked: tab.openCard_()
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
    }

    // ---------------------------------------------------------------- the usual rule
    ColumnLayout {
        objectName: "part-rule"
        Layout.fillWidth: true
        spacing: tab.theme.labelGap

        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: tab.sioul.text("reach-rule")
                textFormat: Text.PlainText
                font.pixelSize: 17
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                color: tab.theme.accent
                Accessible.role: Accessible.Heading
            }
            // Its explanation, more than two lines, behind a "?" (the rulebook's rule 13).
            ToolButton {
                objectName: "reachesRuleHelp"
                text: "?"
                checkable: true
                checked: tab.helpOpen
                Accessible.name: tab.sioul.text("reach-rule")
                Accessible.description: tab.sioul.text("reach-rule-help")
                onToggled: tab.helpOpen = checked
            }
        }
        Pane {
            Layout.fillWidth: true
            padding: tab.theme.cardPadding

            background: Rectangle {
                color: tab.theme.surface
                radius: tab.theme.radius
                border.color: tab.theme.line
                border.width: 1
            }

            ColumnLayout {
                width: parent.width
                spacing: tab.theme.rowGap

                Note {
                    objectName: "reachesRuleHelpText"
                    visible: tab.helpOpen
                    text: tab.sioul.text("reach-rule-help")
                }
                // Start from: the one drop-down of the rule.
                GridLayout {
                    Layout.fillWidth: true
                    columns: tab.narrow ? 1 : 2
                    columnSpacing: 10
                    rowSpacing: tab.theme.labelGap

                    Label {
                        text: tab.sioul.text("reach-start")
                        textFormat: Text.PlainText
                        color: tab.theme.text
                    }
                    PlainComboBox {
                        objectName: "reachesStartFrom"
                        // As wide as its longest choice in either language, never wider than the card.
                        Layout.fillWidth: true
                        Layout.maximumWidth: tab.narrow ? -1 : 380
                        model: tab.shown.start.choices.map(c => tab.theme.plain(c.label))
                        currentIndex: tab.shown.start.choices.findIndex(c => c.id === tab.shown.start.id)
                        Accessible.name: tab.sioul.text("reach-start")
                        onActivated: index => tab.chooseStart(tab.shown.start.choices[index].id, false)
                    }
                }
                // A start that replaces exceptions: said, then taken or not.
                ColumnLayout {
                    objectName: "reachesStartAsk"
                    visible: tab.startQuestion !== ""
                    Layout.fillWidth: true
                    spacing: 4

                    Said {
                        text: tab.startQuestion
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            text: tab.sioul.text("attention-preset-take")
                            highlighted: true
                            onClicked: tab.chooseStart(tab.startAsked, true)
                        }
                        Button {
                            text: tab.sioul.text("attention-preset-keep")
                            onClicked: {
                                tab.startAsked = ""
                                tab.startQuestion = ""
                                tab.reload()
                            }
                        }
                    }
                }
                // A sentence per circle, and how many it holds.
                Repeater {
                    model: tab.shown.rule

                    delegate: ColumnLayout {
                        id: ruleLine

                        required property var modelData

                        objectName: "rule-" + ruleLine.modelData.circle
                        Layout.fillWidth: true
                        spacing: 0

                        Said {
                            text: ruleLine.modelData.text
                        }
                        Note {
                            visible: ruleLine.modelData.count !== ""
                            text: ruleLine.modelData.count
                        }
                    }
                }
                // Where the rules of a channel do nothing here, said there.
                Repeater {
                    model: tab.shown.idle

                    delegate: Note {
                        required property string modelData

                        objectName: "reachesIdle"
                        text: modelData
                        color: tab.theme.warm
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- your exceptions
    Section {
        objectName: "part-exceptions"
        Layout.fillWidth: true
        theme: tab.theme
        title: tab.sioul.text("reach-exceptions")

        Note {
            visible: tab.shown.exceptions.length === 0
            text: tab.sioul.text("reach-none")
        }
        Repeater {
            model: tab.shown.exceptions

            delegate: ColumnLayout {
                id: exception

                required property var modelData
                required property int index

                objectName: "exception-" + exception.index
                Layout.fillWidth: true
                spacing: 2

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Said {
                        // Its share of the row: it wraps rather than pushing the buttons off a phone's screen.
                        Layout.preferredWidth: 1
                        text: exception.modelData.text
                    }
                    Button {
                        objectName: "exceptionEdit-" + exception.index
                        Layout.alignment: Qt.AlignTop
                        text: tab.sioul.text("reach-edit")
                        Accessible.description: exception.modelData.text
                        onClicked: tab.openCard(exception.modelData.value, null)
                    }
                    ToolButton {
                        objectName: "exceptionRemove-" + exception.index
                        Layout.alignment: Qt.AlignTop
                        icon.name: "window-close"
                        icon.color: tab.theme.text
                        Accessible.name: tab.sioul.textWith("reach-remove-name", "line", exception.modelData.text)
                        ToolTip.visible: hovered
                        ToolTip.text: tab.sioul.text("reach-remove")
                        ToolTip.delay: 600
                        onClicked: tab.remove(exception.modelData, exception.index, false)
                    }
                }
                // Where it does nothing here, said beside it.
                Note {
                    visible: (exception.modelData.note || "") !== ""
                    text: exception.modelData.note || ""
                    color: tab.theme.warm
                }
                // ✕ on what takes more with it: said first, then taken out or kept.
                ColumnLayout {
                    objectName: "exceptionAsk-" + exception.index
                    visible: tab.removeAsked === exception.index
                    Layout.fillWidth: true
                    spacing: 4

                    Said {
                        text: exception.modelData.ask || ""
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            objectName: "exceptionAskTake-" + exception.index
                            text: tab.sioul.text("reach-remove-phone-take")
                            onClicked: tab.remove(exception.modelData, exception.index, true)
                        }
                        Button {
                            objectName: "exceptionAskKeep-" + exception.index
                            text: tab.sioul.text("reach-remove-phone-keep")
                            onClicked: tab.removeAsked = -1
                        }
                    }
                }
            }
        }
        Button {
            objectName: "reachesAdd"
            Layout.topMargin: 4
            text: tab.sioul.text("reach-add")
            highlighted: true
            onClicked: tab.openCard(null, null)
        }
    }

    // ---------------------------------------------------------------- do not disturb
    Section {
        objectName: "part-dnd"
        Layout.fillWidth: true
        theme: tab.theme
        title: tab.sioul.text("reach-dnd")

        Row_ {
            key: "dnd.sleep"
        }
        Row_ {
            key: "dnd.pauses"
        }
        Row_ {
            key: "dnd.focus"
        }
        Row_ {
            key: "dnd.button"
        }
        Row_ {
            key: "free_time.nothing"
        }
        // What this computer's system lets Sioul do (on a phone, its card in Devices says it: audit 25).
        Loader {
            active: !tab.shown.phone
            visible: active
            Layout.fillWidth: true

            sourceComponent: Component {
                DndSetup {
                    sioul: tab.sioul
                    theme: tab.theme
                    part: "device"
                }
            }
        }
    }

    // ---------------------------------------------------------------- other apps, and what is gathered
    Section {
        objectName: "part-apps"
        Layout.fillWidth: true
        theme: tab.theme
        title: tab.sioul.text(tab.shown.phone ? "reach-apps" : "reach-apps-computer")

        // This phone's own switches for other apps (app-notes.toml), as its card in Devices wrote them.
        Repeater {
            model: tab.shown.phone && tab.apps ? ["hold", "people", "shade"].map(k => tab.apps[k]).filter(r => r !== null && r !== undefined) : []

            delegate: SettingRow {
                required property var modelData

                objectName: "apps-" + modelData.key
                visible: modelData.key === "hold" || (tab.apps.hold && tab.apps.hold.value === true)
                Layout.fillWidth: true
                setting: modelData
                sioul: tab.sioul
                theme: tab.theme
                onSave: (key, value) => {
                    tab.apps = JSON.parse(tab.sioul.appNotesChange("set", JSON.stringify({ key: key, value: value })) || "null") || tab.apps
                    tab.reload()
                }
            }
        }
        Row_ {
            key: "reminders.gathered"
        }
        // How long a notice to know stays, one length for all: a computer's.
        Row_ {
            key: "notices.informative_minutes"
            visible: !tab.shown.phone && tab.setting("notices.informative_minutes") !== null
        }
    }

    // ---------------------------------------------------------------- who is in which circle, folded
    Fold {
        id: circles

        objectName: "part-circles"
        Layout.fillWidth: true
        theme: tab.theme
        title: tab.sioul.text("reach-circles")
        framed: true

        ColumnLayout {
            spacing: tab.theme.rowGap

            Note {
                text: tab.sioul.text("reach-circles-help")
            }
            Strong {
                text: tab.sioul.text("reach-circle-always")
            }
            DndSetup {
                Layout.fillWidth: true
                sioul: tab.sioul
                theme: tab.theme
                part: "list"
            }
            // Your people, read from Contacts ▸ Your people (never edited here): each by name, by the
            // row they are judged on, what holds one of them out said with its door.
            Strong {
                objectName: "circlePeople"
                visible: tab.shown.circle.people.length > 0
                text: tab.shown.circle.title
            }
            Note {
                visible: tab.shown.circle.people.length > 0
                text: tab.shown.circle.help
            }
            Repeater {
                model: tab.shown.circle.people

                delegate: ColumnLayout {
                    id: close

                    required property var modelData

                    objectName: "close-" + close.modelData.id
                    Layout.fillWidth: true
                    spacing: 0

                    Label {
                        Layout.fillWidth: true
                        text: close.modelData.name
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: tab.theme.text
                    }
                    Repeater {
                        model: close.modelData.notes || []

                        delegate: ColumnLayout {
                            id: held

                            required property var modelData
                            required property int index

                            Layout.fillWidth: true
                            spacing: 0

                            Note {
                                objectName: "closeNote-" + close.modelData.id + "-" + held.index
                                text: held.modelData.text
                            }
                            Link {
                                objectName: "closeDoor-" + close.modelData.id + "-" + held.index
                                visible: held.modelData.door !== ""
                                theme: tab.theme
                                text: held.modelData.label
                                onActivated: {
                                    if (held.modelData.door === "page")
                                        tab.sioul.reminderOpened("person", "", held.modelData.key)
                                    else if (tab.host && tab.host.openPersonSheet)
                                        tab.host.openPersonSheet(held.modelData.key, "")
                                }
                            }
                        }
                    }
                }
            }
            Note {
                text: tab.sioul.text("senders-help")
            }
            Repeater {
                model: tab.lists

                delegate: ColumnLayout {
                    id: listRow

                    required property var modelData
                    // The addresses, numbers and domains named as your contacts: one fold, opened to read and remove them.
                    readonly property bool named: listRow.modelData.key === "neutral"

                    Layout.fillWidth: true
                    spacing: 3

                    SettingRow {
                        visible: !listRow.named
                        Layout.fillWidth: true
                        setting: listRow.modelData
                        sioul: tab.sioul
                        theme: tab.theme
                        onSave: (key, value, shown) => tab.saveList(key, value, shown)
                    }
                    Fold {
                        objectName: "namedContacts"
                        visible: listRow.named
                        Layout.fillWidth: true
                        theme: tab.theme
                        title: listRow.modelData.label

                        SettingRow {
                            objectName: "namedContactsList"
                            setting: listRow.modelData
                            sioul: tab.sioul
                            theme: tab.theme
                            onSave: (key, value, shown) => tab.saveList(key, value, shown)
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- what comes when, worked out, folded
    Fold {
        id: workedOut

        objectName: "part-worked-out"
        Layout.fillWidth: true
        theme: tab.theme
        title: tab.sioul.text("reach-worked-out")
        framed: true
        onToggled: open => {
            tab.view = open ? "table" : "rules"
            tab.rowOpen = ""
        }

        ColumnLayout {
            spacing: tab.theme.rowGap

            // A computer: the grid, a word per cell, a press saying which line decides it. Held in an
            // item of the frame's width, so that its own measure lays it out (the names above their
            // words, or the times upright) rather than it pushing past the frame.
            Item {
                visible: !tab.narrow
                Layout.fillWidth: true
                implicitHeight: grid.implicitHeight

                AttentionGrid {
                    id: grid

                    width: Math.min(parent.width, grid.nameWidth + grid.flatWidth + 24)
                    setting: { "grid": tab.shown.grid }
                    sioul: tab.sioul
                    theme: tab.theme
                    words: true
                    explains: true
                    now: tab.shown.now_columns
                    onExcept: start => tab.openCard(null, start)
                }
            }
            // A phone: a line per row, now's value in a word; a press opens its nine moments.
            Repeater {
                model: tab.narrow && tab.openRowData === null ? tab.gridRows : []

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
                                text: rowButton.modelData.label + (rowButton.modelData.cells.some(c => c.changed === true) ? "  •" : "")
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: tab.theme.text
                            }
                            Label {
                                text: {
                                    const cell = rowButton.modelData.cells.find(c => tab.shown.now_columns.indexOf(c.column) >= 0)
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
                visible: tab.narrow && tab.openRowData !== null
                Layout.fillWidth: true
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
                                color: tab.theme.text
                            }
                        }
                        onClicked: lineWhy.ask(tab.openRowData, valueLine.modelData, valueLine)
                    }
                }
            }
            Note {
                objectName: "reachesGridKey"
                text: tab.sioul.text("reach-grid-key")
            }
        }
    }

    // A phone's moment of a row, pressed: which line decides it, and "Add an exception for this…".
    SioulMenu {
        id: lineWhy

        property var row: null
        property var cell: null

        function ask(row, cell, item) {
            lineWhy.row = row
            lineWhy.cell = cell
            lineWhy.popup(item, 0, item.height)
        }

        MenuItem {
            enabled: false
            contentItem: Label {
                width: Math.min(380, Math.max(160, tab.width - 60))
                text: lineWhy.cell ? lineWhy.cell.by || "" : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: tab.theme.text
            }
        }
        MenuItem {
            objectName: "lineWhyAdd"
            enabled: lineWhy.cell !== null && lineWhy.cell.except !== null && lineWhy.cell.except !== undefined
            text: tab.sioul.text("reach-add-for-this")
            onTriggered: tab.openCard(null, lineWhy.cell.except)
        }
    }

    // The one card that adds or changes an exception, made when first opened.
    Loader {
        id: cardLoader

        active: false

        sourceComponent: Component {
            ExceptionCard {
                sioul: tab.sioul
                theme: tab.theme
                choices: tab.shown.card
                onSaved: tab.reload()
            }
        }
    }
}
