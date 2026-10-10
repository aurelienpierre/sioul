// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The mail filters, under "Filters" in Mail ▸ ⚙ (crates/sioul-app/src/filters.rs,
// docs/client.md, "Filters"): one list for every address, each filter in
// words, narrowed to one address when you choose one; its editor opens in
// place, one condition and one action at first, the rest when asked for,
// each field once its kind is chosen; every change saved at once, as every
// setting. "Try it" counts what a filter takes in the inboxes now; "Run
// them on the inboxes…" says first what would change, then waits ten
// seconds with Undo. On a phone, the same in one column.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: filters

    required property var sioul
    required property var theme
    // What filters are, in a sentence: the setting's own (settings.rs), said first.
    property string about: ""
    // filters.rs's View: {filters, accounts, folders, form}.
    property var shown: ({ filters: [], accounts: [], folders: [], form: { fields: [], actions: [], kinds: [], who: [], days: [] } })
    // The address the list is narrowed to; "" for all of them.
    property string narrowed: ""
    // The filter open in the editor, by its place in the list; -1 for none.
    property int editing: -1
    // That filter as the editor changes it, saved at each change.
    property var draft: null
    // Its sentence and what keeps it from running, said as it changes.
    property string draftSaid: ""
    property string draftProblem: ""
    property string draftOnly: ""
    // What went wrong writing the list, else "".
    property string problem: ""
    // The filter last deleted and its place, for "Undo".
    property var deleted: null
    // What "Try it" (kind "try") or the preview (kind "preview") found; looking meanwhile.
    property var found: null
    property bool looking: false
    // Under the editor of a filter a search made: what of the search a filter does not read.
    property string note: ""
    // The editor's frame, while one is open: shown whole in the panel when a search opens it.
    property Item editorItem: null
    readonly property var locale: Qt.locale(filters.sioul.text("qt-locale"))
    readonly property bool narrow: filters.width < 460

    // The list read again; a filter a search just made (`open`), opened in the editor.
    function read() {
        filters.shown = JSON.parse(filters.sioul.mailFilters() || "null") || filters.shown
        if (filters.shown.open !== undefined && filters.shown.open >= 0 && filters.shown.open < filters.shown.filters.length) {
            filters.narrowed = ""
            filters.editing = -1
            filters.open(filters.shown.open)
            filters.note = filters.shown.note || ""
            revealTimer.restart()
        }
    }

    // An item shown whole (the editor open, a preview's result), its top under
    // the panel's top when it does not fit: the panel's own scrolling.
    function reveal(item) {
        let up = filters.parent
        while (up && (up as Flickable) === null)
            up = up.parent
        const flick = up as Flickable
        if (flick === null || !item)
            return
        const top = item.mapToItem(flick.contentItem, 0, 0).y - 60
        if (top >= flick.contentY && top + item.height + 60 <= flick.contentY + flick.height)
            return
        flick.contentY = Math.max(0, Math.min(top, flick.contentHeight - flick.height))
    }

    // "Try it": what the filter open takes in the inboxes now.
    function tryIt() {
        filters.found = null
        filters.looking = true
        filters.sioul.mailFilterTry(JSON.stringify(filters.draft))
    }

    // "Run them on the inboxes…": what would change, said first.
    function preview() {
        filters.found = null
        filters.looking = true
        filters.sioul.mailFiltersPreview()
    }

    // The list written, in its order, from the one shown: its change set over the
    // filters as they are now; read again when `again` (its sentences said anew).
    function write(list, again) {
        filters.problem = filters.sioul.setMailFilters(JSON.stringify(list), JSON.stringify(filters.shown.filters))
        if (filters.problem === "" && again)
            filters.read()
    }

    function copy(value) {
        return JSON.parse(JSON.stringify(value))
    }

    function shownHere(filter) {
        return filters.narrowed === "" || filter.accounts.length === 0 || filter.accounts.indexOf(filters.narrowed) >= 0
    }

    function fieldOf(id) {
        return filters.shown.form.fields.find(f => f.id === id) || null
    }

    function testOf(fieldId, testId) {
        const field = filters.fieldOf(fieldId)
        return field ? (field.tests.find(t => t.id === testId) || null) : null
    }

    // The draft saved into its place, its sentence said anew; `rebuilt`: the
    // editor's parts follow (a part added, taken away, of another kind).
    function saveDraft(rebuilt) {
        const list = filters.copy(filters.shown.filters)
        list[filters.editing] = filters.draft
        filters.write(list, false)
        filters.shown.filters[filters.editing] = filters.draft
        const said = JSON.parse(filters.sioul.mailFilterSaid(JSON.stringify(filters.draft)) || "{}")
        filters.draftSaid = said.said || ""
        filters.draftProblem = said.problem || ""
        filters.draftOnly = said.only || ""
        filters.found = null
        if (rebuilt)
            filters.draft = filters.copy(filters.draft)
    }

    function open(index) {
        if (filters.editing >= 0)
            filters.close()
        filters.editing = index
        filters.draft = filters.copy(filters.shown.filters[index])
        filters.draftSaid = filters.draft.said
        filters.draftProblem = filters.draft.problem
        filters.draftOnly = filters.draft.only
        filters.found = null
    }

    function close() {
        filters.editing = -1
        filters.draft = null
        filters.found = null
        filters.note = ""
        filters.read()
    }

    // A new filter: one condition and one action, to be filled; on the address the list is narrowed to.
    function add() {
        const list = filters.copy(filters.shown.filters)
        list.push({ name: "", enabled: true, accounts: filters.narrowed === "" ? [] : [filters.narrowed], any: false, conditions: [{ field: "from", test: "contains", value: "", until: "" }], actions: [{ do: "move", name: "" }], stop: false })
        filters.write(list, true)
        if (filters.problem === "")
            filters.open(filters.shown.filters.length - 1)
    }

    function remove(index) {
        const list = filters.copy(filters.shown.filters)
        filters.deleted = { filter: list[index], at: index }
        list.splice(index, 1)
        filters.editing = -1
        filters.draft = null
        filters.write(list, true)
        undoTimer.restart()
    }

    function bringBack() {
        if (filters.deleted === null)
            return
        const list = filters.copy(filters.shown.filters)
        const at = Math.min(filters.deleted.at, list.length)
        list.splice(at, 0, filters.deleted.filter)
        // A filter open below it keeps its editor: one place further.
        if (filters.editing >= at)
            filters.editing += 1
        filters.deleted = null
        filters.write(list, true)
    }

    // The filter next to it in the list shown (narrowed to an address, the next of that address), -1 for none.
    function neighbour(index, by) {
        for (let at = index + by; at >= 0 && at < filters.shown.filters.length; at += by) {
            if (filters.shownHere(filters.shown.filters[at]))
                return at
        }
        return -1
    }

    // Asked earlier or later: in the place of the filter above or below it, as shown.
    function move(index, by) {
        const list = filters.copy(filters.shown.filters)
        const to = filters.neighbour(index, by)
        if (to < 0)
            return
        const one = list.splice(index, 1)[0]
        list.splice(to, 0, one)
        if (filters.editing >= 0)
            filters.close()
        filters.write(list, true)
    }

    function toggle(index, on) {
        const list = filters.copy(filters.shown.filters)
        list[index].enabled = on
        // The filter open in the editor: its next change keeps the switch as set.
        if (index === filters.editing && filters.draft !== null)
            filters.draft.enabled = on
        filters.write(list, true)
    }

    // A condition's field chosen: its usual test, and a value of its kind.
    function setField(at, fieldId) {
        const field = filters.fieldOf(fieldId)
        const condition = filters.draft.conditions[at]
        condition.field = fieldId
        if (!field.tests.some(t => t.id === condition.test)) {
            condition.test = field.tests[0].id
            condition.value = ""
            condition.until = ""
        }
        const kind = filters.testOf(fieldId, condition.test).value
        if (kind === "kind" && filters.shown.form.kinds.every(k => k.id !== condition.value))
            condition.value = filters.shown.form.kinds[0].id
        if (kind === "who" && filters.shown.form.who.every(w => w.id !== condition.value))
            condition.value = filters.shown.form.who[0].id
        filters.saveDraft(true)
    }

    function setTest(at, testId) {
        const condition = filters.draft.conditions[at]
        const before = filters.testOf(condition.field, condition.test)
        condition.test = testId
        const now = filters.testOf(condition.field, testId)
        if (before && now && before.value !== now.value && !(before.value === "date" && now.value === "dates") && !(before.value === "time" && now.value === "times"))
            condition.value = ""
        if (now && now.value !== "dates" && now.value !== "times")
            condition.until = ""
        filters.saveDraft(true)
    }

    // Who has a folder: its addresses, as the filter's addresses see it.
    function folderNote(name) {
        const folder = filters.shown.folders.find(f => f.name.toLowerCase() === name.toLowerCase())
        const wanted = filters.draft.accounts.length > 0 ? filters.draft.accounts : filters.shown.accounts.map(a => a.id)
        if (!folder || wanted.every(id => folder.accounts.indexOf(id) >= 0))
            return ""
        const having = filters.shown.accounts.filter(a => folder.accounts.indexOf(a.id) >= 0 && wanted.indexOf(a.id) >= 0).map(a => a.label)
        return filters.sioul.textWith("filter-ui-folder-some", "accounts", having.join(", "))
    }

    // Found by the window's pictures (main.qml, "mail-filters").
    objectName: "mailFilters"
    spacing: 6
    Component.onCompleted: filters.read()
    // Shown again: the list as it is now, and a filter a search made meanwhile opened.
    onVisibleChanged: if (visible) filters.read()

    Connections {
        target: filters.sioul

        function onMailFiltersFound(found) {
            filters.looking = false
            filters.found = JSON.parse(found || "null")
            // A preview's result below the list: brought into view.
            if (filters.found && filters.found.kind === "preview") {
                revealTimer.item = runBlock
                revealTimer.restart()
            }
        }
    }

    Timer {
        id: undoTimer

        interval: 10000
        onTriggered: filters.deleted = null
    }

    // Once the panel is laid out with it: the editor open, else `item`.
    Timer {
        id: revealTimer

        property Item item: null

        interval: 150
        onTriggered: {
            filters.reveal(revealTimer.item || filters.editorItem)
            revealTimer.item = null
        }
    }

    Label {
        visible: filters.about !== ""
        Layout.fillWidth: true
        text: filters.about
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.25
        color: filters.theme.muted
    }

    // Narrowed to one address, when there are several.
    RowLayout {
        visible: filters.shown.accounts.length > 1 && filters.shown.filters.length > 0
        Layout.fillWidth: true
        spacing: 8

        Label {
            text: filters.sioul.text("filter-ui-narrow")
            color: filters.theme.muted
        }
        ComboBox {
            Layout.fillWidth: true
            model: [filters.sioul.text("filter-ui-all-addresses")].concat(filters.shown.accounts.map(a => filters.theme.plain(a.label)))
            currentIndex: filters.narrowed === "" ? 0 : 1 + filters.shown.accounts.findIndex(a => a.id === filters.narrowed)
            Accessible.name: filters.sioul.text("filter-ui-narrow")
            onActivated: index => filters.narrowed = index === 0 ? "" : filters.shown.accounts[index - 1].id
        }
    }

    Label {
        visible: filters.shown.filters.length === 0 || !filters.shown.filters.some(f => filters.shownHere(f))
        Layout.fillWidth: true
        text: filters.sioul.text(filters.shown.filters.length === 0 ? "filter-ui-none" : "filter-ui-none-here")
        wrapMode: Text.Wrap
        color: filters.theme.muted
    }

    // The list: each filter in words, its switch, its place.
    Repeater {
        model: filters.shown.filters

        delegate: ColumnLayout {
            id: row

            required property var modelData
            required property int index
            readonly property bool open: filters.editing === row.index

            visible: filters.shownHere(row.modelData)
            Layout.fillWidth: true
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Switch {
                    Layout.alignment: Qt.AlignTop
                    checked: row.modelData.enabled
                    Accessible.name: filters.sioul.text("filter-ui-switch")
                    ToolTip.visible: hovered
                    ToolTip.text: filters.sioul.text("filter-ui-switch")
                    ToolTip.delay: 400
                    onToggled: filters.toggle(row.index, checked)
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 1

                    Label {
                        visible: row.modelData.name !== ""
                        Layout.fillWidth: true
                        text: row.modelData.name
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        wrapMode: Text.Wrap
                        color: row.modelData.enabled ? filters.theme.text : filters.theme.muted
                    }
                    Label {
                        Layout.fillWidth: true
                        text: row.open ? filters.draftSaid : row.modelData.said
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        lineHeight: 1.2
                        color: row.modelData.enabled ? filters.theme.text : filters.theme.muted

                        TapHandler {
                            onTapped: row.open ? filters.close() : filters.open(row.index)
                        }
                    }
                    Label {
                        visible: text !== ""
                        Layout.fillWidth: true
                        text: row.open ? filters.draftOnly : row.modelData.only
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: filters.theme.muted
                    }
                    Label {
                        visible: text !== "" && !row.open
                        Layout.fillWidth: true
                        text: row.modelData.problem
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: filters.theme.warm
                    }
                }
                ToolButton {
                    Layout.alignment: Qt.AlignTop
                    icon.name: row.open ? "go-up" : "document-edit"
                    icon.color: filters.theme.text
                    Accessible.name: filters.sioul.text(row.open ? "filter-ui-done" : "filter-ui-edit")
                    ToolTip.visible: hovered
                    ToolTip.text: filters.theme.plain(Accessible.name)
                    ToolTip.delay: 400
                    onClicked: row.open ? filters.close() : filters.open(row.index)
                }
                ColumnLayout {
                    Layout.alignment: Qt.AlignTop
                    visible: filters.neighbour(row.index, -1) >= 0 || filters.neighbour(row.index, 1) >= 0
                    spacing: 0

                    ToolButton {
                        text: "▴"
                        enabled: filters.neighbour(row.index, -1) >= 0
                        implicitHeight: 22
                        Accessible.name: filters.sioul.text("filter-ui-up")
                        ToolTip.visible: hovered
                        ToolTip.text: filters.theme.plain(Accessible.name)
                        ToolTip.delay: 400
                        onClicked: filters.move(row.index, -1)
                    }
                    ToolButton {
                        text: "▾"
                        enabled: filters.neighbour(row.index, 1) >= 0
                        implicitHeight: 22
                        Accessible.name: filters.sioul.text("filter-ui-down")
                        ToolTip.visible: hovered
                        ToolTip.text: filters.theme.plain(Accessible.name)
                        ToolTip.delay: 400
                        onClicked: filters.move(row.index, 1)
                    }
                }
            }

            // Its editor, in place.
            Loader {
                active: row.open && filters.draft !== null
                visible: active
                Layout.fillWidth: true
                Layout.topMargin: 4
                Layout.bottomMargin: 8
                sourceComponent: editor
            }
        }
    }

    Label {
        visible: filters.problem !== ""
        Layout.fillWidth: true
        text: filters.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: filters.theme.warm
    }

    // A filter just deleted: back with "Undo", for ten seconds.
    RowLayout {
        visible: filters.deleted !== null
        Layout.fillWidth: true

        Label {
            Layout.fillWidth: true
            text: filters.deleted === null ? "" : filters.sioul.textWith("filter-ui-deleted", "name", filters.deleted.filter.name !== "" ? filters.deleted.filter.name : filters.deleted.filter.said)
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: filters.theme.muted
        }
        Button {
            flat: true
            text: filters.sioul.text("ui-undo")
            onClicked: filters.bringBack()
        }
    }

    Button {
        flat: true
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        text: "+  " + filters.sioul.text("filter-ui-add")
        onClicked: filters.add()
    }

    // On the inboxes now, read mail too: said first, then done with ten seconds to undo.
    ColumnLayout {
        id: runBlock

        visible: filters.shown.filters.some(f => f.enabled && f.problem === "")
        Layout.fillWidth: true
        Layout.topMargin: 10
        spacing: 4

        Label {
            Layout.fillWidth: true
            text: filters.sioul.text("filter-ui-run-help")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            lineHeight: 1.25
            color: filters.theme.muted
        }
        Button {
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            text: filters.sioul.text("filter-ui-run")
            enabled: !filters.looking
            onClicked: filters.preview()
        }
        Label {
            visible: filters.looking && filters.editing < 0
            text: filters.sioul.text("filter-ui-looking")
            color: filters.theme.muted
        }
        ColumnLayout {
            visible: filters.found !== null && filters.found.kind === "preview"
            Layout.fillWidth: true
            spacing: 3

            Label {
                Layout.fillWidth: true
                text: filters.found ? filters.found.text : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: filters.theme.text
            }
            Repeater {
                model: filters.found && filters.found.kind === "preview" ? filters.found.lines : []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: "•  " + modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    lineHeight: 1.2
                    color: filters.theme.text
                }
            }
            Flow {
                visible: filters.found !== null && filters.found.count > 0
                Layout.fillWidth: true
                spacing: 6

                Button {
                    text: filters.sioul.text("filter-ui-run-now")
                    onClicked: {
                        filters.problem = filters.sioul.mailFiltersRun()
                        filters.found = null
                    }
                }
                Button {
                    flat: true
                    text: filters.sioul.text("filter-ui-not-now")
                    onClicked: filters.found = null
                }
            }
        }
    }

    // The editor of the filter open: its name, its conditions, its actions, the rest folded.
    Component {
        id: editor

        Rectangle {
            id: box

            implicitHeight: form.implicitHeight + 20
            radius: filters.theme.radius
            color: "transparent"
            border.color: filters.theme.line
            Component.onCompleted: filters.editorItem = box
            Component.onDestruction: {
                if (filters.editorItem === box)
                    filters.editorItem = null
            }

            ColumnLayout {
                id: form

                anchors.fill: parent
                anchors.margins: 10
                spacing: 6

                // Made from a search: what of it a filter does not read.
                Label {
                    visible: filters.note !== ""
                    Layout.fillWidth: true
                    text: filters.note
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    lineHeight: 1.2
                    color: filters.theme.muted
                }

                TextField {
                    Layout.fillWidth: true
                    text: filters.draft.name
                    placeholderText: filters.sioul.text("filter-ui-name")
                    Accessible.name: placeholderText
                    onEditingFinished: {
                        if (text.trim() !== filters.draft.name) {
                            filters.draft.name = text.trim()
                            filters.saveDraft(false)
                        }
                    }
                }

                // If: every condition, or one of them (asked once there are two).
                RowLayout {
                    Layout.fillWidth: true

                    Label {
                        text: filters.sioul.text("filter-ui-if")
                        font.weight: Font.DemiBold
                        color: filters.theme.text
                    }
                    ComboBox {
                        visible: filters.draft.conditions.length > 1
                        Layout.fillWidth: true
                        model: [filters.sioul.text("filter-ui-all"), filters.sioul.text("filter-ui-any")]
                        currentIndex: filters.draft.any ? 1 : 0
                        Accessible.name: filters.sioul.text("filter-ui-if")
                        onActivated: index => {
                            filters.draft.any = index === 1
                            filters.saveDraft(true)
                        }
                    }
                }
                Repeater {
                    model: filters.draft.conditions

                    // A condition: what it reads and how it compares, × when there are
                    // others; under them, its value, as its kind asks. On a phone, one
                    // column: what it reads and ×, then how it compares, then the value.
                    delegate: GridLayout {
                        id: condition

                        required property var modelData
                        required property int index
                        readonly property var field: filters.fieldOf(condition.modelData.field)
                        readonly property var test: filters.testOf(condition.modelData.field, condition.modelData.test)
                        readonly property string kind: condition.test ? condition.test.value : "text"
                        // ×, when there are others; without it, the line is the others' alone.
                        readonly property bool removable: filters.draft.conditions.length > 1

                        function setValue(value, until) {
                            const one = filters.draft.conditions[condition.index]
                            if (one.value === value && (until === undefined || one.until === until))
                                return
                            one.value = value
                            if (until !== undefined)
                                one.until = until
                            filters.saveDraft(false)
                        }

                        Layout.fillWidth: true
                        columns: filters.narrow ? 2 : 3
                        columnSpacing: 6
                        rowSpacing: 4

                        ComboBox {
                            Layout.row: 0
                            Layout.column: 0
                            Layout.columnSpan: filters.narrow && !condition.removable ? 2 : 1
                            Layout.fillWidth: filters.narrow
                            implicitContentWidthPolicy: ComboBox.WidestText
                            model: filters.shown.form.fields.map(f => filters.theme.plain(f.label))
                            currentIndex: filters.shown.form.fields.findIndex(f => f.id === condition.modelData.field)
                            Accessible.name: filters.sioul.text("filter-ui-field")
                            onActivated: index => filters.setField(condition.index, filters.shown.form.fields[index].id)
                        }
                        ComboBox {
                            Layout.row: filters.narrow ? 1 : 0
                            Layout.column: filters.narrow ? 0 : 1
                            Layout.columnSpan: filters.narrow || !condition.removable ? 2 : 1
                            Layout.fillWidth: true
                            implicitContentWidthPolicy: ComboBox.WidestText
                            model: condition.field ? condition.field.tests.map(t => filters.theme.plain(t.label)) : []
                            currentIndex: condition.field ? condition.field.tests.findIndex(t => t.id === condition.modelData.test) : -1
                            Accessible.name: filters.sioul.text("filter-ui-test")
                            onActivated: index => filters.setTest(condition.index, condition.field.tests[index].id)
                        }
                        ToolButton {
                            Layout.row: 0
                            Layout.column: filters.narrow ? 1 : 2
                            visible: condition.removable
                            text: "×"
                            Accessible.name: filters.sioul.text("filter-ui-remove")
                            ToolTip.visible: hovered
                            ToolTip.text: filters.theme.plain(Accessible.name)
                            ToolTip.delay: 400
                            onClicked: {
                                filters.draft.conditions.splice(condition.index, 1)
                                filters.saveDraft(true)
                            }
                        }
                        // The value, as its kind asks: words take the whole line.
                        Flow {
                            id: valueRow

                            visible: condition.kind !== "nothing"
                            Layout.row: filters.narrow ? 2 : 1
                            Layout.column: 0
                            Layout.columnSpan: filters.narrow ? 2 : 3
                            Layout.fillWidth: true
                            spacing: 6

                            TextField {
                                visible: condition.kind === "text" || condition.kind === "size"
                                width: condition.kind === "size" && !filters.narrow ? 160 : valueRow.width
                                text: condition.modelData.value || ""
                                placeholderText: filters.sioul.text(condition.kind === "size" ? "filter-ui-size" : "filter-ui-value")
                                Accessible.name: placeholderText
                                onEditingFinished: condition.setValue(text.trim())
                            }
                            DateField {
                                visible: condition.kind === "date" || condition.kind === "dates"
                                theme: filters.theme
                                sioul: filters.sioul
                                locale: filters.locale
                                date: condition.modelData.value || ""
                                pickLabel: filters.sioul.text("event-pick-day")
                                onEdited: {
                                    if (date.length === 10)
                                        condition.setValue(date)
                                }
                            }
                            Label {
                                visible: condition.kind === "dates" || condition.kind === "times"
                                height: 36
                                verticalAlignment: Text.AlignVCenter
                                text: filters.sioul.text("filter-ui-and")
                                color: filters.theme.muted
                            }
                            DateField {
                                visible: condition.kind === "dates"
                                theme: filters.theme
                                sioul: filters.sioul
                                locale: filters.locale
                                date: condition.modelData.until || ""
                                pickLabel: filters.sioul.text("event-pick-day")
                                onEdited: {
                                    if (date.length === 10)
                                        condition.setValue(filters.draft.conditions[condition.index].value, date)
                                }
                            }
                            TextField {
                                visible: condition.kind === "time" || condition.kind === "times"
                                width: 80
                                text: condition.modelData.value || ""
                                placeholderText: "18:00"
                                Accessible.name: filters.sioul.text("filter-field-hour")
                                onEditingFinished: condition.setValue(text.trim())
                            }
                            TextField {
                                visible: condition.kind === "times"
                                width: 80
                                text: condition.modelData.until || ""
                                placeholderText: "06:00"
                                Accessible.name: filters.sioul.text("filter-field-hour")
                                onEditingFinished: condition.setValue(filters.draft.conditions[condition.index].value, text.trim())
                            }
                            // Days of the week: each ticked or not.
                            Repeater {
                                model: condition.kind === "days" ? filters.shown.form.days : []

                                delegate: CheckBox {
                                    required property var modelData

                                    readonly property var picked: (condition.modelData.value || "").split(",").filter(d => d !== "")

                                    text: filters.theme.plain(modelData.label)
                                    checked: picked.indexOf(modelData.id) >= 0
                                    onToggled: {
                                        const days = filters.shown.form.days.map(d => d.id).filter(id => id === modelData.id ? checked : picked.indexOf(id) >= 0)
                                        condition.setValue(days.join(","))
                                    }
                                }
                            }
                            ComboBox {
                                visible: condition.kind === "kind" || condition.kind === "who"
                                width: Math.min(implicitWidth, valueRow.width)
                                implicitContentWidthPolicy: ComboBox.WidestText
                                readonly property var choices: condition.kind === "kind" ? filters.shown.form.kinds : condition.kind === "who" ? filters.shown.form.who : []

                                model: choices.map(c => filters.theme.plain(c.label))
                                currentIndex: choices.findIndex(c => c.id === condition.modelData.value)
                                Accessible.name: filters.sioul.text("filter-ui-value")
                                onActivated: index => condition.setValue(choices[index].id)
                            }
                        }
                    }
                }
                Button {
                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: "+  " + filters.sioul.text("filter-ui-add-condition")
                    onClicked: {
                        filters.draft.conditions.push({ field: "subject", test: "contains", value: "", until: "" })
                        filters.saveDraft(true)
                    }
                }

                // Then: what is done, each action with what it names.
                Label {
                    text: filters.sioul.text("filter-ui-then")
                    font.weight: Font.DemiBold
                    color: filters.theme.text
                }
                Repeater {
                    model: filters.draft.actions

                    // An action and what it names (a folder, a keyword), × when there are
                    // others; on a phone, what it names on a line of its own.
                    delegate: GridLayout {
                        id: action

                        required property var modelData
                        required property int index
                        readonly property bool names: action.modelData.do === "move" || action.modelData.do === "keyword"
                        // ×, when there are others; without it, the line is the others' alone.
                        readonly property bool removable: filters.draft.actions.length > 1

                        function setName(name) {
                            const one = filters.draft.actions[action.index]
                            if (one.name === name)
                                return
                            one.name = name
                            filters.saveDraft(true)
                        }

                        Layout.fillWidth: true
                        columns: filters.narrow ? 2 : 3
                        columnSpacing: 6
                        rowSpacing: 4

                        ComboBox {
                            Layout.row: 0
                            Layout.column: 0
                            Layout.columnSpan: filters.narrow && !action.removable ? 2 : 1
                            Layout.fillWidth: filters.narrow
                            implicitContentWidthPolicy: ComboBox.WidestText
                            model: filters.shown.form.actions.map(a => filters.theme.plain(a.label))
                            currentIndex: filters.shown.form.actions.findIndex(a => a.id === action.modelData.do)
                            Accessible.name: filters.sioul.text("filter-ui-then")
                            onActivated: index => {
                                const one = filters.draft.actions[action.index]
                                one.do = filters.shown.form.actions[index].id
                                one.name = ""
                                filters.saveDraft(true)
                            }
                        }
                        ComboBox {
                            readonly property var names: filters.shown.folders.map(f => f.name)
                            // A folder another device chose, not listed here yet: shown all the same.
                            readonly property var choices: (action.modelData.name !== "" && names.every(n => n.toLowerCase() !== action.modelData.name.toLowerCase()) ? [action.modelData.name] : []).concat(names)

                            visible: action.modelData.do === "move"
                            Layout.row: filters.narrow ? 1 : 0
                            Layout.column: filters.narrow ? 0 : 1
                            Layout.columnSpan: filters.narrow || !action.removable ? 2 : 1
                            Layout.fillWidth: true
                            Layout.minimumWidth: 120
                            model: choices.map(c => filters.theme.plain(c))
                            currentIndex: action.modelData.name === "" ? -1 : choices.findIndex(n => n.toLowerCase() === action.modelData.name.toLowerCase())
                            displayText: currentIndex < 0 ? filters.sioul.text("filter-ui-folder") : currentText
                            Accessible.name: filters.sioul.text("filter-ui-folder")
                            onActivated: index => action.setName(choices[index])
                        }
                        TextField {
                            visible: action.modelData.do === "keyword"
                            Layout.row: filters.narrow ? 1 : 0
                            Layout.column: filters.narrow ? 0 : 1
                            Layout.columnSpan: filters.narrow || !action.removable ? 2 : 1
                            Layout.fillWidth: true
                            Layout.minimumWidth: 120
                            text: action.modelData.name || ""
                            placeholderText: filters.sioul.text("filter-ui-keyword")
                            Accessible.name: placeholderText
                            onEditingFinished: action.setName(text.trim())
                        }
                        // Nothing named: × at the line's end all the same.
                        Item {
                            visible: !filters.narrow && !action.names
                            Layout.row: 0
                            Layout.column: 1
                            Layout.columnSpan: action.removable ? 1 : 2
                            Layout.fillWidth: true
                        }
                        ToolButton {
                            Layout.row: 0
                            Layout.column: filters.narrow ? 1 : 2
                            visible: action.removable
                            text: "×"
                            Accessible.name: filters.sioul.text("filter-ui-remove")
                            ToolTip.visible: hovered
                            ToolTip.text: filters.theme.plain(Accessible.name)
                            ToolTip.delay: 400
                            onClicked: {
                                filters.draft.actions.splice(action.index, 1)
                                filters.saveDraft(true)
                            }
                        }
                        // A folder some of its addresses lack: said, calmly.
                        Label {
                            readonly property string note: action.modelData.do === "move" && action.modelData.name !== "" ? filters.folderNote(action.modelData.name) : ""

                            visible: note !== ""
                            Layout.row: filters.narrow ? 2 : 1
                            Layout.column: 0
                            Layout.columnSpan: filters.narrow ? 2 : 3
                            Layout.fillWidth: true
                            text: note
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: filters.theme.muted
                        }
                    }
                }
                Button {
                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: "+  " + filters.sioul.text("filter-ui-add-action")
                    onClicked: {
                        filters.draft.actions.push({ do: "read", name: "" })
                        filters.saveDraft(true)
                    }
                }

                // Folded: on which addresses, and whether the filters below are still asked.
                Button {
                    id: more

                    property bool unfolded: filters.draft.stop || filters.draft.accounts.length > 0

                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: (more.unfolded ? "▾  " : "▸  ") + filters.sioul.text("filter-ui-more")
                    onClicked: more.unfolded = !more.unfolded
                }
                ColumnLayout {
                    visible: more.unfolded
                    Layout.fillWidth: true
                    Layout.leftMargin: 12
                    spacing: 2

                    Label {
                        visible: filters.shown.accounts.length > 1
                        text: filters.sioul.text("filter-ui-addresses")
                        color: filters.theme.muted
                    }
                    WrapCheckBox {
                        visible: filters.shown.accounts.length > 1
                        Layout.fillWidth: true
                        text: filters.sioul.text("filter-ui-every-address")
                        checked: filters.draft.accounts.length === 0
                        onToggled: {
                            filters.draft.accounts = checked ? [] : filters.shown.accounts.slice(0, 1).map(a => a.id)
                            filters.saveDraft(true)
                        }
                    }
                    Repeater {
                        model: filters.shown.accounts.length > 1 && filters.draft.accounts.length > 0 ? filters.shown.accounts : []

                        delegate: WrapCheckBox {
                            required property var modelData

                            Layout.fillWidth: true
                            Layout.leftMargin: 16
                            text: modelData.label
                            checked: filters.draft.accounts.indexOf(modelData.id) >= 0
                            onToggled: {
                                const chosen = filters.shown.accounts.map(a => a.id).filter(id => id === modelData.id ? checked : filters.draft.accounts.indexOf(id) >= 0)
                                filters.draft.accounts = chosen
                                filters.saveDraft(true)
                            }
                        }
                    }
                    WrapCheckBox {
                        Layout.fillWidth: true
                        text: filters.sioul.text("filter-ui-stop")
                        checked: filters.draft.stop
                        onToggled: {
                            filters.draft.stop = checked
                            filters.saveDraft(true)
                        }
                    }
                }

                // What keeps it from running, said.
                Label {
                    visible: filters.draftProblem !== ""
                    Layout.fillWidth: true
                    text: filters.draftProblem
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: filters.theme.warm
                }

                // "Try it": how many messages of the inboxes it takes now, and a few of them.
                Flow {
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        text: filters.sioul.text("filter-ui-try")
                        enabled: !filters.looking && filters.draftProblem === ""
                        onClicked: filters.tryIt()
                    }
                    Button {
                        flat: true
                        text: filters.sioul.text("filter-ui-done")
                        onClicked: filters.close()
                    }
                    Button {
                        flat: true
                        text: filters.sioul.text("filter-ui-delete")
                        onClicked: filters.remove(filters.editing)
                    }
                }
                Label {
                    visible: filters.looking
                    text: filters.sioul.text("filter-ui-looking")
                    color: filters.theme.muted
                }
                Label {
                    visible: filters.found !== null && filters.found.kind === "try"
                    Layout.fillWidth: true
                    text: filters.found ? filters.found.text : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: filters.theme.text
                }
                Repeater {
                    model: filters.found && filters.found.kind === "try" ? filters.found.lines : []

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: "•  " + modelData
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 13
                        color: filters.theme.muted
                    }
                }
            }
        }
    }
}
