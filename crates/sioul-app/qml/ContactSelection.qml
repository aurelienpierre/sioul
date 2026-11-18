// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Contacts page's selection bar (wave D): how many are selected, "Select
// the 132 shown", none, then what to do with them: move to an address book,
// add to a group or take out of one, move the title into its field, delete.
// Each asks first, in its dialog, how many it will change and why some
// cannot (`contactsBatchPlan`); runs off the window's thread with its
// progress and a Stop (`contactsBatch`); ends by naming what it could not
// do, card by card, and is undone as a whole from the status line.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: bar

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The keys selected (file paths).
    property var keys: []
    // How many the filters show.
    property int shownCount: 0
    // The writable address books, to move to: [{place, name}].
    property var books: []
    // The groups in use, to choose from.
    property var groups: []
    // The batch running or last ended (`contactsBatch`): {running, done, total, line, failed}.
    readonly property var batch: bar.sioul.contactsBatch ? JSON.parse(bar.sioul.contactsBatch) : ({ running: false, failed: [] })

    signal selectShown
    signal selectNone
    signal finished
    // A batch ended: the cards it moved or deleted are no longer where the selection named them.
    signal ended

    property bool wasRunning: false
    onBatchChanged: {
        if (bar.wasRunning && !bar.batch.running)
            bar.ended()
        bar.wasRunning = bar.batch.running === true
    }

    // The dialog of an action opened ("move", "group-add", "group-remove", "title", "delete").
    function act(action) {
        const dialog = askDialog.now()
        dialog.begin(action)
        return dialog
    }

    objectName: "contactSelection"
    spacing: 4

    Flow {
        Layout.fillWidth: true
        spacing: 6

        Label {
            objectName: "contactSelectionCount"
            height: selectShownButton.height
            verticalAlignment: Text.AlignVCenter
            text: bar.sioul.textArgs("contacts-selected", JSON.stringify({ n: bar.keys.length }))
            font.weight: Font.DemiBold
            color: bar.theme.text
        }
        Button {
            id: selectShownButton

            objectName: "contactSelectShown"
            flat: true
            enabled: !bar.batch.running
            text: bar.theme.plain(bar.sioul.textArgs("contacts-select-shown", JSON.stringify({ n: bar.shownCount })))
            onClicked: bar.selectShown()
        }
        Button {
            visible: bar.keys.length > 0
            flat: true
            enabled: !bar.batch.running
            text: bar.sioul.text("contacts-select-none")
            onClicked: bar.selectNone()
        }
        Button {
            id: actionsButton

            objectName: "contactSelectionActions"
            enabled: bar.keys.length > 0 && !bar.batch.running
            text: bar.sioul.text("batch-actions")
            icon.name: "go-down"
            icon.color: bar.theme.text
            onClicked: actions.now().popup(actionsButton, 0, actionsButton.height)
        }
        Button {
            objectName: "contactSelectionDone"
            flat: true
            enabled: !bar.batch.running
            text: bar.sioul.text("contacts-select-done")
            onClicked: bar.finished()
        }
    }

    // The batch running: its count, a bar, Stop.
    RowLayout {
        visible: bar.batch.running === true
        Layout.fillWidth: true
        spacing: 8

        ProgressBar {
            objectName: "contactBatchProgress"
            Layout.fillWidth: true
            from: 0
            to: Math.max(1, bar.batch.total || 1)
            value: bar.batch.done || 0
        }
        Label {
            text: bar.batch.line || ""
            textFormat: Text.PlainText
            color: bar.theme.muted
        }
        Button {
            objectName: "contactBatchStop"
            text: bar.sioul.text("batch-stop")
            onClicked: bar.sioul.stopContactsBatch()
        }
    }

    // What the last batch could not do, card by card.
    Label {
        visible: !bar.batch.running && (bar.batch.failed || []).length > 0
        Layout.fillWidth: true
        text: bar.batch.line || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: bar.theme.text
    }
    Repeater {
        model: bar.batch.running ? [] : (bar.batch.failed || [])

        delegate: Label {
            required property var modelData

            objectName: "contactBatchFailed"
            Layout.fillWidth: true
            Layout.leftMargin: 12
            text: "•  " + modelData.name + " — " + modelData.why
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: bar.theme.muted
        }
    }

    Later {
        id: actions

        sourceComponent: Component {
            SioulMenu {
                MenuItem {
                    objectName: "batchMove"
                    text: bar.sioul.text("batch-move")
                    enabled: bar.books.length > 0
                    onTriggered: bar.act("move")
                }
                MenuItem {
                    text: bar.sioul.text("batch-group-add")
                    onTriggered: bar.act("group-add")
                }
                MenuItem {
                    text: bar.sioul.text("batch-group-remove")
                    onTriggered: bar.act("group-remove")
                }
                MenuItem {
                    objectName: "batchTitle"
                    text: bar.sioul.text("batch-title")
                    onTriggered: bar.act("title")
                }
                MenuItem {
                    objectName: "batchExport"
                    text: bar.sioul.text("batch-export")
                    onTriggered: exportFile.open()
                }
                MenuSeparator {}
                MenuItem {
                    text: bar.sioul.text("batch-delete")
                    onTriggered: bar.act("delete")
                }
            }
        }
    }

    // The cards selected as one vCard file, where you choose.
    FileDialog {
        id: exportFile

        fileMode: FileDialog.SaveFile
        defaultSuffix: "vcf"
        nameFilters: [bar.sioul.text("books-export-kind") + " (*.vcf)"]
        onAccepted: {
            const problem = bar.sioul.exportContacts(JSON.stringify({ keys: bar.keys }), exportFile.selectedFile.toString())
            if (problem !== "")
                bar.sioul.status = problem
        }
    }

    // Asked first: where or which group; then the count and why some cannot; then Go.
    Later {
        id: askDialog

        sourceComponent: Component {
            SafeDialog {
                id: ask

                property string action: ""
                property var plan: ({ count: 0, sentence: "", note: "", losses: "", cannot_line: "", cannot: [], previews: [] })

                function asked() {
                    const book = ask.action === "move" && bar.books.length > 0 ? bar.books[Math.max(0, bookChoice.currentIndex)].place : ""
                    const group = ask.action === "title" ? (alsoGroup.checked ? groupField.text : "") : groupField.text
                    return JSON.stringify({ keys: bar.keys, action: ask.action, book: book, group: group })
                }

                function replan() {
                    ask.plan = JSON.parse(bar.sioul.contactsBatchPlan(ask.asked()) || "{}")
                }

                function begin(action) {
                    ask.action = action
                    groupField.text = ""
                    alsoGroup.checked = false
                    bookChoice.currentIndex = 0
                    ask.replan()
                    ask.open()
                }

                objectName: "contactBatchDialog"
                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(520, ask.roomWidth - 2 * bar.theme.gap)
                title: bar.sioul.text("batch-" + (ask.action === "group-add" ? "group-add" : ask.action === "group-remove" ? "group-remove" : ask.action))

                contentItem: ScrollView {
                    implicitHeight: Math.min(askColumn.implicitHeight, ask.roomHeight * 0.6)
                    contentWidth: availableWidth
                    clip: true

                    ColumnLayout {
                        id: askColumn

                        width: parent ? parent.width : 400
                        spacing: 8

                        // Where they go.
                        PlainComboBox {
                            id: bookChoice

                            objectName: "batchBook"
                            visible: ask.action === "move"
                            Layout.fillWidth: true
                            model: bar.books.map(b => bar.theme.plain(b.name))
                            Accessible.name: bar.sioul.text("batch-book")
                            onActivated: ask.replan()
                        }
                        // Which group: typed, or one in use.
                        CheckBox {
                            id: alsoGroup

                            objectName: "batchAlsoGroup"
                            visible: ask.action === "title"
                            text: bar.sioul.text("batch-also-group")
                            onToggled: ask.replan()
                        }
                        RowLayout {
                            visible: ask.action === "group-add" || ask.action === "group-remove" || (ask.action === "title" && alsoGroup.checked)
                            Layout.fillWidth: true
                            spacing: 4

                            TextField {
                                id: groupField

                                objectName: "batchGroup"
                                Layout.fillWidth: true
                                placeholderText: bar.sioul.text("batch-group-name")
                                onTextEdited: replanning.restart()
                            }
                            ToolButton {
                                id: usedGroups

                                visible: bar.groups.length > 0
                                text: "▾"
                                Accessible.name: bar.sioul.text("contact-category-used")
                                onClicked: groupsMenu.now().popup(usedGroups, 0, usedGroups.height)
                            }
                            Timer {
                                id: replanning

                                interval: 300
                                onTriggered: ask.replan()
                            }
                        }
                        Label {
                            objectName: "batchSentence"
                            Layout.fillWidth: true
                            text: ask.plan.sentence || ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.weight: Font.DemiBold
                            color: bar.theme.text
                        }
                        Label {
                            visible: (ask.plan.note || "") !== ""
                            Layout.fillWidth: true
                            text: ask.plan.note || ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: bar.theme.muted
                        }
                        Label {
                            visible: (ask.plan.losses || "") !== ""
                            Layout.fillWidth: true
                            text: ask.plan.losses || ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: bar.theme.warm
                        }
                        // Before and after, for every card whose title moves.
                        Repeater {
                            model: ask.action === "title" ? (ask.plan.previews || []) : []

                            delegate: Label {
                                required property var modelData

                                objectName: "batchPreview"
                                Layout.fillWidth: true
                                text: modelData[0] + "  →  " + modelData[1]
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: bar.theme.text
                            }
                        }
                        Label {
                            visible: (ask.plan.cannot_line || "") !== ""
                            Layout.fillWidth: true
                            text: ask.plan.cannot_line || ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: bar.theme.muted
                        }
                        Repeater {
                            model: ask.plan.cannot || []

                            delegate: Label {
                                required property var modelData

                                objectName: "batchCannot"
                                Layout.fillWidth: true
                                Layout.leftMargin: 12
                                text: "•  " + modelData.name + " — " + modelData.why
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: bar.theme.muted
                            }
                        }
                    }
                }

                footer: DialogButtonBox {
                    Button {
                        text: bar.sioul.text("ui-cancel")
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                    Button {
                        objectName: "batchGo"
                        enabled: (ask.plan.count || 0) > 0
                        highlighted: true
                        text: bar.sioul.text(ask.action === "delete" ? "batch-go-delete" : "batch-go")
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                }

                onAccepted: {
                    const problem = bar.sioul.runContactsBatch(ask.asked())
                    if (problem !== "")
                        bar.sioul.status = problem
                }

                Later {
                    id: groupsMenu

                    sourceComponent: Component {
                        SioulMenu {
                            id: used

                            Instantiator {
                                model: bar.groups

                                delegate: MenuItem {
                                    id: usedLine

                                    required property string modelData

                                    text: bar.theme.plain(usedLine.modelData.replace(/&/g, "&&"))
                                    onTriggered: {
                                        groupField.text = usedLine.modelData
                                        ask.replan()
                                    }
                                }
                                onObjectAdded: (index, object) => used.insertItem(index, object)
                                onObjectRemoved: (index, object) => used.removeItem(object)
                            }
                        }
                    }
                }
            }
        }
    }
}
