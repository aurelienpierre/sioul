// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One exception of What reaches you, added or changed (ReachesTab.qml; the
// redesign of 24 November 2026, docs/attention.md "The page"): four headed
// drop-downs, What, From whom, Where and When it reaches you, the first
// three starting at "anything", "anyone" and "every device". "A person…"
// opens a field that finds them by name in your address books; "Your own
// times…" alone opens the nine moments, each with its drop-down. The
// choices of the first three come from the page (`choices`, reaches.rs's
// `card`); those of When, for what is chosen, from the backend
// (`reachesAct("offered")`). Saved through `reachesAct("save")`, the
// exception before it given too when it is changed; a save that fails
// keeps the card open and says why (DialogFoot.qml: Cancel, then the main
// action, at the right).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: card

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The page's choices for What, From whom and Where: each [{value, label, search?}].
    property var choices: ({ what: [], whom: [], where: [] })
    // The exception changed, as the page lists it (its store with it); null for a new one.
    property var old: null
    // What is chosen, each a value as the core reads it (sioul_core::exceptions).
    property var what: null
    property var whom: null
    property var where: null
    property var when: null
    // When's choices for what is chosen: {when: [{value, label, cells}], own, moments: [{column, label, levels}]}.
    property var offered: ({ when: [], own: false, moments: [] })
    // "Your own times…" chosen: a level per moment, W A L M S P F Y D.
    property bool ownChosen: false
    property var cells: []
    // "A person…" chosen and nobody found yet: the field shows.
    property bool searching: false
    property var found: []
    property string problem: ""
    // Narrow (a phone): each moment's drop-down under its name.
    readonly property bool narrow: card.width < 480

    // Saved: the page reads itself again.
    signal saved

    // A heading over each choice: what it chooses.
    component Head: Label {
        Layout.fillWidth: true
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.weight: Font.DemiBold
        color: card.theme.text
        Accessible.role: Accessible.Heading
    }

    // Two values the same (the core writes each in one order).
    function same(a, b) {
        return JSON.stringify(a) === JSON.stringify(b)
    }

    // A value's place among `list`'s choices; -1 when none.
    function indexIn(list, value) {
        for (let i = 0; i < list.length; i++) {
            if (card.same(list[i].value, value))
                return i
        }
        return -1
    }

    // `exception`, as the page lists it, to change; or a new one, from
    // `start` (a cell of What comes when) or from the first choices.
    function edit(exception, start) {
        card.problem = ""
        card.old = exception || null
        const from = exception || start || null
        card.what = from ? from.what : (card.choices.what[0] || { value: null }).value
        card.whom = from ? from.whom : (card.choices.whom[0] || { value: null }).value
        card.where = from ? from.where : (card.choices.where[0] || { value: null }).value
        card.searching = false
        card.found = []
        card.ownChosen = from !== null && from.when !== undefined && from.when.kind === "own"
        card.cells = card.ownChosen ? from.when.cells.slice() : []
        card.when = from && !card.ownChosen ? from.when : null
        card.refresh()
        card.open()
    }

    // When's choices read again for what is chosen; the choice kept when it is still offered.
    // Changing an exception, its own When comes among them even where the card could not
    // offer it (a phone's table set cell by cell), so that saving it as it was writes nothing.
    function refresh() {
        const asked = { what: card.what, whom: card.whom, where: card.where, old: card.old }
        const ready = card.what !== null && card.whom !== null && card.where !== null && !(card.whom.kind === "person" && (card.whom.id || "") === "")
        const none = { when: [], own: false, moments: [], why: "" }
        card.offered = ready ? (JSON.parse(card.sioul.reachesAct("offered", JSON.stringify(asked)) || "null") || none) : none
        if (card.ownChosen && !card.offered.own)
            card.ownChosen = false
        // Each moment keeps its level where it is still offered there, else takes the first.
        if (card.ownChosen)
            card.cells = card.offered.moments.map((m, i) => m.levels.some(l => l.value === card.cells[i]) ? card.cells[i] : m.levels[0].value)
        if (!card.ownChosen && card.indexIn(card.offered.when, card.when) < 0)
            card.when = card.offered.when.length > 0 ? card.offered.when[0].value : null
    }

    // Enter takes the card's main action (a drop-down open takes it to choose); in the
    // field of a name, the one person found.
    function takeEnter(event) {
        if (findField.activeFocus) {
            if (card.found.length === 1) {
                card.whom = card.found[0].value
                card.searching = false
                card.found = []
                findField.text = ""
                card.refresh()
            }
            event.accepted = true
            return
        }
        if (card.ownChosen || card.when !== null) {
            event.accepted = true
            card.save()
        }
    }

    // "Your own times…" chosen: each moment starts as the pattern chosen before says, else its first level.
    function chooseOwn() {
        const at = card.indexIn(card.offered.when, card.when)
        const from = at >= 0 ? card.offered.when[at].cells : null
        card.cells = card.offered.moments.map((m, i) => from && m.levels.some(l => l.value === from[i]) ? from[i] : m.levels[0].value)
        card.ownChosen = true
    }

    // People of your address books by name, for "A person…".
    function find(text) {
        card.found = JSON.parse(card.sioul.reachesAct("find", JSON.stringify({ text: text })) || "[]")
    }

    function save() {
        const when = card.ownChosen ? { kind: "own", cells: card.cells } : card.when
        if (when === null) {
            card.problem = card.sioul.text("reach-when-pick")
            return
        }
        const answer = JSON.parse(card.sioul.reachesAct("save", JSON.stringify({ old: card.old, new: { what: card.what, whom: card.whom, where: card.where, when: when } })) || "null")
        if (answer && answer.problem) {
            card.problem = answer.problem
            return
        }
        card.close()
        card.saved()
    }

    objectName: "exceptionCard"
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    focus: true
    width: Math.min(560, (parent ? card.roomWidth : 560) - 2 * card.theme.gap)
    title: card.sioul.text(card.old ? "reach-card-edit" : "reach-card-add")

    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(column.implicitHeight, (card.parent ? card.roomHeight : 700) - 160)
        contentWidth: availableWidth
        clip: true
        focus: true
        Keys.onReturnPressed: event => card.takeEnter(event)
        Keys.onEnterPressed: event => card.takeEnter(event)

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: card.theme.rowGap

            Head {
                text: card.sioul.text("reach-what")
            }
            PlainComboBox {
                objectName: "exceptionWhat"
                Layout.fillWidth: true
                model: card.choices.what.map(c => card.theme.plain(c.label))
                currentIndex: card.indexIn(card.choices.what, card.what)
                Accessible.name: card.sioul.text("reach-what")
                onActivated: index => {
                    card.what = card.choices.what[index].value
                    card.refresh()
                }
            }

            Head {
                text: card.sioul.text("reach-whom")
            }
            PlainComboBox {
                id: whomBox

                objectName: "exceptionWhom"
                Layout.fillWidth: true
                model: card.choices.whom.map(c => card.theme.plain(c.label))
                // A person chosen: the entry "A person…", their name in the box.
                currentIndex: card.whom && card.whom.kind === "person" ? card.choices.whom.findIndex(c => c.search === true) : card.indexIn(card.choices.whom, card.whom)
                displayText: card.whom && card.whom.kind === "person" && (card.whom.name || "") !== "" ? card.whom.name : currentText
                Accessible.name: card.sioul.text("reach-whom")
                onActivated: index => {
                    const chosen = card.choices.whom[index]
                    card.searching = chosen.search === true
                    card.whom = chosen.value
                    card.found = []
                    card.refresh()
                    if (card.searching)
                        findField.forceActiveFocus()
                }
            }
            // A person, found by name.
            TextField {
                id: findField

                objectName: "exceptionFind"
                visible: card.searching || (card.whom !== null && card.whom.kind === "person" && (card.whom.id || "") === "")
                Layout.fillWidth: true
                placeholderText: card.sioul.text("reach-find")
                Accessible.name: card.sioul.text("reach-find")
                onTextEdited: card.find(text)
            }
            Repeater {
                model: findField.visible ? card.found : []

                delegate: Button {
                    id: person

                    required property var modelData

                    Layout.fillWidth: true
                    text: card.theme.plain(person.modelData.label)
                    onClicked: {
                        card.whom = person.modelData.value
                        card.searching = false
                        card.found = []
                        findField.text = ""
                        card.refresh()
                    }
                }
            }
            Label {
                visible: findField.visible && findField.text.length >= 2 && card.found.length === 0
                Layout.fillWidth: true
                text: card.sioul.text("reach-find-none")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: card.theme.muted
            }

            Head {
                text: card.sioul.text("reach-where")
            }
            PlainComboBox {
                objectName: "exceptionWhere"
                Layout.fillWidth: true
                model: card.choices.where.map(c => card.theme.plain(c.label))
                currentIndex: card.indexIn(card.choices.where, card.where)
                Accessible.name: card.sioul.text("reach-where")
                onActivated: index => {
                    card.where = card.choices.where[index].value
                    card.refresh()
                }
            }

            Head {
                text: card.sioul.text("reach-when")
            }
            PlainComboBox {
                id: whenBox

                // When's choices, and "Your own times…" last where they may be set.
                readonly property var shown: card.offered.when.concat(card.offered.own ? [{ value: { kind: "own" }, label: card.sioul.text("reach-when-own"), own: true }] : [])

                objectName: "exceptionWhen"
                visible: whenBox.shown.length > 0
                Layout.fillWidth: true
                model: whenBox.shown.map(c => card.theme.plain(c.label))
                currentIndex: card.ownChosen ? whenBox.shown.length - 1 : card.indexIn(card.offered.when, card.when)
                Accessible.name: card.sioul.text("reach-when")
                onActivated: index => {
                    if (whenBox.shown[index].own === true)
                        card.chooseOwn()
                    else {
                        card.ownChosen = false
                        card.when = whenBox.shown[index].value
                    }
                }
            }
            // Nothing to choose: why (a device that cannot keep it, what and from whom not chosen yet).
            Label {
                objectName: "exceptionWhy"
                visible: !whenBox.visible
                Layout.fillWidth: true
                text: card.offered.why || card.sioul.text("reach-when-pick")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: card.theme.muted
            }
            // Your own times: the nine moments, each its drop-down.
            GridLayout {
                objectName: "exceptionMoments"
                visible: card.ownChosen
                Layout.fillWidth: true
                Layout.leftMargin: card.theme.indent
                columns: card.narrow ? 1 : 2
                columnSpacing: 10
                rowSpacing: card.narrow ? 2 : 6

                Repeater {
                    model: card.ownChosen ? card.offered.moments : []

                    delegate: ColumnLayout {
                        id: moment

                        required property var modelData
                        required property int index

                        Layout.fillWidth: true
                        spacing: 2

                        Label {
                            Layout.fillWidth: true
                            text: moment.modelData.label
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: card.theme.muted
                        }
                        PlainComboBox {
                            objectName: "exceptionMoment-" + moment.modelData.column
                            Layout.fillWidth: true
                            model: moment.modelData.levels.map(l => card.theme.plain(l.label))
                            currentIndex: moment.modelData.levels.findIndex(l => l.value === card.cells[moment.index])
                            Accessible.name: moment.modelData.label
                            onActivated: at => {
                                const cells = card.cells.slice()
                                cells[moment.index] = moment.modelData.levels[at].value
                                card.cells = cells
                            }
                        }
                    }
                }
            }
            Label {
                visible: card.problem !== ""
                Layout.fillWidth: true
                text: card.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: card.theme.warm
            }
        }
    }

    footer: DialogFoot {
        sioul: card.sioul
        theme: card.theme
        mainText: card.sioul.text(card.old ? "ui-save" : "reach-save")
        mainEnabled: card.ownChosen || card.when !== null
        onAccepted: card.save()
        onRejected: card.close()
    }
}
