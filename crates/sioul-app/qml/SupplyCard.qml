// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A supply's card (docs/supplies.md), modal, its sections headed: What it
// is (its fields, each saved as it changes), In stock (the number, with
// Received… and Count again…), Planned (the open tasks that will use it, a
// flat list of links), History (its movements, newest first, each saying
// why, a task's as a link; the whole ledger one press away), and Archive.
// A new supply: its fields and its first count, made at Save. No colour of
// alarm anywhere: "low" and "none left" are quiet words. The supply's name,
// its category and the tasks' titles are the person's own words, drawn plain.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: card

    required property var sioul
    required property var theme
    required property var window
    // The categories in use, to choose one again; the projects: [{id, title}].
    property var categories: []
    property var projects: []
    // The supply shown; "" for a new one.
    property string supplyId: ""
    // Its card as Rust gives it (`supplies::card`); null for a new one.
    property var shown: null
    property string problem: ""
    // The whole history, past its last ten lines.
    property bool historyAll: false
    // The movement asked in place: "received", "count", "arrived" (a task to resupply done elsewhere), or "".
    property string asking: ""
    readonly property bool fresh: card.supplyId === ""
    // Ten lines of history at first.
    readonly property int historyShort: 10

    // Something was saved: the page reads the supplies again.
    signal saved

    // A new supply, its fields empty.
    function begin() {
        card.supplyId = ""
        card.shown = null
        card.fill()
        card.open()
        nameField.forceActiveFocus()
    }

    // A supply's card, by its id.
    function show(id) {
        card.supplyId = id
        card.historyAll = false
        card.reload()
        card.open()
    }

    function reload() {
        if (card.fresh)
            return
        const text = card.sioul.supplyCard(card.supplyId)
        card.shown = text === "null" ? null : JSON.parse(text)
        if (card.shown === null) {
            card.close()
            return
        }
        card.fill()
    }

    function fill() {
        const s = card.shown
        card.problem = ""
        card.asking = ""
        nameField.text = s ? s.name : ""
        categoryField.text = s ? s.category : ""
        areaSwitches.value = s ? s.area : ""
        unitField.text = s ? s.unit : ""
        buyField.text = s ? s.buy : ""
        sellField.text = s ? s.sell : ""
        lowField.text = s ? s.low_at : ""
        boughtField.text = s ? s.bought_by : ""
        firstField.text = ""
        projectBox.currentIndex = Math.max(0, projectBox.choices.findIndex(p => p.id === (s ? s.project : "")))
    }

    // One field changed: saved at once on a supply's card; kept in the form of a new one until Save.
    function change(field, value) {
        if (card.fresh)
            return
        const change = {}
        change[field] = value
        const answer = JSON.parse(card.sioul.saveSupply(card.supplyId, JSON.stringify(change), ""))
        card.problem = answer.error || ""
        if (card.problem === "") {
            card.reload()
            card.saved()
        }
    }

    // A new supply made of its form, with its first count.
    function make() {
        const change = { name: nameField.text, category: categoryField.text, area: areaSwitches.value, unit: unitField.text, buy: buyField.text, sell: sellField.text, low_at: lowField.text, bought_by: boughtField.text, project: projectBox.choices[projectBox.currentIndex].id }
        const answer = JSON.parse(card.sioul.saveSupply("", JSON.stringify(change), firstField.text))
        if (answer.error) {
            card.problem = answer.error
            return
        }
        card.saved()
        card.supplyId = answer.id
        card.reload()
    }

    // Received… or Count again…, said in place: its field first.
    function ask(what) {
        card.asking = what
        movedField.text = ""
        movedField.forceActiveFocus()
    }

    function moved() {
        const answer = card.asking === "arrived" ? card.sioul.supplyArrived(card.shown.resupplied.uid, movedField.text) : card.sioul.supplyMoved(card.supplyId, card.asking, movedField.text)
        card.problem = answer
        if (answer === "") {
            card.asking = ""
            card.reload()
            card.saved()
        }
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, (parent ? card.roomWidth : 560) - 2 * card.theme.gap)
    height: Math.min(implicitHeight, (parent ? card.roomHeight : 800) - 2 * card.theme.gap)
    title: card.fresh ? card.sioul.text("supplies-new") : card.theme.plain(card.shown ? card.shown.name : "")

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 14

            // What it is: its fields, changed in place.
            Section {
                theme: card.theme
                title: card.sioul.text("supplies-card-what")
                Layout.fillWidth: true

                GridLayout {
                    Layout.fillWidth: true
                    columns: card.width < 420 ? 1 : 2
                    columnSpacing: 10
                    rowSpacing: 6

                    Label {
                        text: card.sioul.text("supplies-field-name")
                        color: card.theme.muted
                    }
                    TextField {
                        id: nameField

                        objectName: "supplyName"
                        Layout.fillWidth: true
                        placeholderText: card.sioul.text("supplies-field-name-hint")
                        onEditingFinished: {
                            if (card.shown && nameField.text.trim() !== "" && nameField.text !== card.shown.name)
                                card.change("name", nameField.text)
                        }
                    }
                    Label {
                        text: card.sioul.text("supplies-field-category")
                        color: card.theme.muted
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        TextField {
                            id: categoryField

                            objectName: "supplyCategory"
                            Layout.fillWidth: true
                            placeholderText: card.sioul.text("supplies-field-category-hint")
                            onEditingFinished: {
                                if (card.shown && categoryField.text !== card.shown.category)
                                    card.change("category", categoryField.text)
                            }
                        }
                        // The categories in use, one press each: one kind of category, the person's words.
                        Flow {
                            visible: card.categories.length > 0
                            Layout.fillWidth: true
                            spacing: 4

                            Repeater {
                                model: card.categories

                                delegate: Button {
                                    required property string modelData

                                    flat: true
                                    font.pixelSize: 12
                                    text: card.theme.plain(modelData)
                                    onClicked: {
                                        categoryField.text = modelData
                                        if (card.shown && modelData !== card.shown.category)
                                            card.change("category", modelData)
                                    }
                                }
                            }
                        }
                    }
                    Label {
                        text: card.sioul.text("supplies-field-for")
                        color: card.theme.muted
                    }
                    AreaSwitches {
                        id: areaSwitches

                        Layout.fillWidth: true
                        sioul: card.sioul
                        onPicked: value => {
                            areaSwitches.value = value
                            card.change("area", value)
                        }
                    }
                    Label {
                        text: card.sioul.text("supplies-field-unit")
                        color: card.theme.muted
                    }
                    TextField {
                        id: unitField

                        Layout.fillWidth: true
                        placeholderText: card.sioul.text("supplies-field-unit-hint")
                        onEditingFinished: {
                            if (card.shown && unitField.text !== card.shown.unit)
                                card.change("unit", unitField.text)
                        }
                    }
                    Label {
                        text: card.sioul.text("supplies-field-buy")
                        color: card.theme.muted
                    }
                    TextField {
                        id: buyField

                        Layout.fillWidth: true
                        inputMethodHints: Qt.ImhFormattedNumbersOnly
                        onEditingFinished: {
                            if (card.shown && buyField.text !== card.shown.buy)
                                card.change("buy", buyField.text)
                        }
                    }
                    Label {
                        text: card.sioul.text("supplies-field-sell")
                        color: card.theme.muted
                    }
                    TextField {
                        id: sellField

                        Layout.fillWidth: true
                        inputMethodHints: Qt.ImhFormattedNumbersOnly
                        placeholderText: card.sioul.text("supplies-field-optional")
                        onEditingFinished: {
                            if (card.shown && sellField.text !== card.shown.sell)
                                card.change("sell", sellField.text)
                        }
                    }
                    Label {
                        text: card.sioul.text("supplies-field-low-at")
                        color: card.theme.muted
                    }
                    TextField {
                        id: lowField

                        objectName: "supplyLowAt"
                        Layout.fillWidth: true
                        inputMethodHints: Qt.ImhFormattedNumbersOnly
                        placeholderText: card.sioul.text("supplies-field-low-at-hint")
                        onEditingFinished: {
                            if (card.shown && lowField.text !== card.shown.low_at)
                                card.change("low_at", lowField.text)
                        }
                    }
                    Label {
                        text: card.sioul.text("supplies-field-bought-by")
                        color: card.theme.muted
                    }
                    TextField {
                        id: boughtField

                        Layout.fillWidth: true
                        inputMethodHints: Qt.ImhFormattedNumbersOnly
                        placeholderText: card.sioul.text("supplies-field-optional")
                        onEditingFinished: {
                            if (card.shown && boughtField.text !== card.shown.bought_by)
                                card.change("bought_by", boughtField.text)
                        }
                    }
                    Label {
                        text: card.sioul.text("supplies-field-project")
                        color: card.theme.muted
                    }
                    PlainComboBox {
                        id: projectBox

                        readonly property var choices: [{ id: "", title: "—" }].concat(card.projects)

                        Layout.fillWidth: true
                        model: projectBox.choices.map(p => card.theme.plain(p.title))
                        onActivated: index => card.change("project", projectBox.choices[index].id)
                    }
                    Label {
                        visible: card.fresh
                        text: card.sioul.text("supplies-first-count")
                        color: card.theme.muted
                    }
                    TextField {
                        id: firstField

                        objectName: "supplyFirstCount"
                        visible: card.fresh
                        Layout.fillWidth: true
                        inputMethodHints: Qt.ImhFormattedNumbersOnly
                        placeholderText: "0"
                        onAccepted: card.make()
                    }
                }
            }

            // In stock: the number, and the two ways to change it.
            Section {
                visible: !card.fresh && card.shown !== null
                theme: card.theme
                title: card.sioul.text("supplies-card-stock")
                Layout.fillWidth: true

                Label {
                    objectName: "supplyStock"
                    Layout.fillWidth: true
                    text: card.shown ? card.shown.stock_text : ""
                    textFormat: Text.PlainText
                    font.pixelSize: 22
                    color: card.theme.text
                }
                Label {
                    Layout.fillWidth: true
                    text: card.shown ? card.shown.line : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
                // A stock under zero, said as it is: counting again sets it right.
                Label {
                    visible: card.shown !== null && card.shown.under_zero === true
                    Layout.fillWidth: true
                    text: card.sioul.text("supplies-under-zero")
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
                // A task to resupply done elsewhere, its arrival not said: asked here, once.
                Label {
                    visible: card.shown !== null && card.shown.resupplied !== null && card.shown.resupplied !== undefined
                    Layout.fillWidth: true
                    text: card.shown && card.shown.resupplied ? card.sioul.textArgs("supplies-resupplied", JSON.stringify({ title: card.shown.resupplied.title, day: card.shown.resupplied.day })) : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.text
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        objectName: "supplyReceived"
                        text: card.sioul.text("supplies-received")
                        onClicked: card.ask("received")
                    }
                    Button {
                        objectName: "supplyCountAgain"
                        text: card.sioul.text("supplies-count-again")
                        onClicked: card.ask("count")
                    }
                    Button {
                        visible: card.shown !== null && card.shown.resupplied !== null && card.shown.resupplied !== undefined
                        text: card.sioul.text("supplies-arrived-title")
                        onClicked: card.ask("arrived")
                    }
                }
                // The number asked in place.
                RowLayout {
                    visible: card.asking !== ""
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        Layout.fillWidth: true
                        text: card.asking === "count" ? card.sioul.text("supplies-count-ask") : card.sioul.text("supplies-received-ask")
                        wrapMode: Text.Wrap
                        color: card.theme.muted
                    }
                    TextField {
                        id: movedField

                        objectName: "supplyMovedField"
                        Layout.preferredWidth: 90
                        inputMethodHints: Qt.ImhFormattedNumbersOnly
                        horizontalAlignment: Text.AlignRight
                        onAccepted: card.moved()
                    }
                    Button {
                        objectName: "supplyMovedOk"
                        text: card.sioul.text("ui-ok")
                        highlighted: true
                        onClicked: card.moved()
                    }
                }
            }

            // Planned: the open tasks that will use it, a flat list of links.
            Section {
                visible: !card.fresh && card.shown !== null
                theme: card.theme
                title: card.sioul.text("supplies-card-planned")
                Layout.fillWidth: true

                Label {
                    visible: card.shown !== null && card.shown.planned_tasks.length === 0
                    Layout.fillWidth: true
                    text: card.sioul.text("supplies-card-planned-none")
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
                Repeater {
                    model: card.shown ? card.shown.planned_tasks : []

                    delegate: Button {
                        required property var modelData

                        Layout.fillWidth: true
                        flat: true
                        text: card.theme.plain(modelData.title + " · " + modelData.quantity)
                        onClicked: {
                            card.close()
                            card.window.openTask(modelData.uid)
                        }
                    }
                }
                Button {
                    visible: card.shown !== null && card.shown.resupply !== null && card.shown.resupply !== undefined
                    Layout.fillWidth: true
                    flat: true
                    text: card.shown && card.shown.resupply ? card.theme.plain(card.shown.resupply.title) : ""
                    onClicked: {
                        card.close()
                        card.window.openTask(card.shown.resupply.uid)
                    }
                }
            }

            // History: its movements, newest first, each saying why; the whole of it one press away.
            Section {
                visible: !card.fresh && card.shown !== null
                theme: card.theme
                title: card.sioul.text("supplies-card-history")
                Layout.fillWidth: true

                Label {
                    visible: card.shown !== null && card.shown.history.length === 0
                    Layout.fillWidth: true
                    text: card.sioul.text("supplies-history-none")
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
                Repeater {
                    model: card.shown ? (card.historyAll ? card.shown.history : card.shown.history.slice(0, card.historyShort)) : []

                    delegate: ColumnLayout {
                        id: moved

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: moved.modelData.when + " · " + moved.modelData.line + " → " + moved.modelData.after
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: card.theme.text
                        }
                        Button {
                            visible: moved.modelData.task !== "" && moved.modelData.task_title !== ""
                            flat: true
                            font.pixelSize: 12
                            text: card.theme.plain(moved.modelData.task_title)
                            onClicked: {
                                card.close()
                                card.window.openTask(moved.modelData.task)
                            }
                        }
                    }
                }
                Button {
                    visible: card.shown !== null && !card.historyAll && card.shown.history.length > card.historyShort
                    flat: true
                    text: card.sioul.text("supplies-history-all")
                    onClicked: card.historyAll = true
                }
            }

            Button {
                objectName: "supplyArchive"
                visible: !card.fresh && card.shown !== null
                flat: true
                text: card.shown && card.shown.archived ? card.sioul.text("supplies-unarchive") : card.sioul.text("supplies-archive")
                icon.name: "archive-insert"
                icon.color: card.theme.text
                onClicked: card.change("archived", !card.shown.archived)
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

    // The buttons inside an Item: a DialogButtonBox as the footer itself closes
    // the dialog on "Save" even when saving fails, and what went wrong is never read.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                objectName: "supplySave"
                visible: card.fresh
                text: card.sioul.text("ui-save")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: card.fresh ? card.sioul.text("ui-cancel") : card.sioul.text("ui-close")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onAccepted: card.make()
            onRejected: card.close()
        }
    }
}
