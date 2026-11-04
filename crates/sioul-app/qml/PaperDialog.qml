// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A paper of the wallet, new or changed: what it is, its file, from when and
// until when it holds, whom it is for (you, or persons of your household,
// several at once: docs/persons.md). A file from elsewhere is copied into
// the wallet's folder when saved; an end is proposed from the issue for a
// passport, an identity card or a warranty.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

SafeDialog {
    id: form

    required property var sioul
    required property var theme
    required property var window
    // The kinds: {id, label}.
    property var kinds: []
    property string paperId: ""
    property string problem: ""
    // Its file: a path, or a file:// address just chosen.
    property string file: ""
    // Whom it may be for, {id, name}, you first (papers::view's `choices`).
    property var choices: []
    // Whom it is for, by person id ("me" for you), and texts naming nobody, kept as written.
    property var chosen: ["me"]
    property var texts: []

    function readChoices() {
        form.choices = (JSON.parse(form.sioul.papers() || "{}").choices) || []
    }

    function toggle(id, on) {
        const others = form.chosen.filter(c => c !== id)
        form.chosen = on ? others.concat([id]) : others
    }

    signal saved

    // A new paper, perhaps from a file (an attachment kept, a scan), perhaps for someone ("forIds").
    function begin(file, title, kind, forIds) {
        form.paperId = ""
        form.readChoices()
        form.chosen = forIds && forIds.length > 0 ? forIds : ["me"]
        form.texts = []
        form.problem = ""
        kindChoice.currentIndex = Math.max(0, form.kinds.findIndex(k => k.id === (kind || "other")))
        titleField.text = title || ""
        form.file = file || ""
        issuedField.date = ""
        untilField.date = ""
        notesArea.text = ""
        form.open()
        titleField.forceActiveFocus()
    }

    // A paper of the wallet, changed.
    function edit(paper) {
        form.paperId = paper.id
        form.readChoices()
        form.chosen = (paper.persons || []).length > 0 ? paper.persons : ((paper.texts || []).length > 0 ? [] : ["me"])
        form.texts = paper.texts || []
        form.problem = ""
        kindChoice.currentIndex = Math.max(0, form.kinds.findIndex(k => k.id === paper.kind))
        titleField.text = paper.title
        form.file = paper.file
        issuedField.date = paper.issued
        untilField.date = paper.until
        notesArea.text = paper.notes
        form.open()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, (parent ? form.roomWidth : 560) - 2 * form.theme.gap)
    title: form.sioul.text(form.paperId === "" ? "papers-new" : "papers-change")

    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(grid.implicitHeight, (form.parent ? form.parent.height : 700) - 200)
        contentWidth: availableWidth
        clip: true

        GridLayout {
            id: grid

            width: scroll.availableWidth
            columns: 2
            columnSpacing: 10
            rowSpacing: 8

            Label {
                text: form.sioul.text("papers-kind")
                color: form.theme.muted
            }
            PlainComboBox {
                id: kindChoice

                Layout.fillWidth: true
                model: form.kinds.map(k => form.theme.plain(k.label))
            }
            Label {
                text: form.sioul.text("papers-title")
                color: form.theme.muted
            }
            TextField {
                id: titleField

                Layout.fillWidth: true
                placeholderText: form.sioul.text("papers-title-hint")
            }
            Label {
                text: form.sioul.text("papers-file")
                color: form.theme.muted
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    text: form.file === "" ? "—" : (form.file.startsWith("file:") ? form.theme.localPath(form.file) : form.file).split(/[\/\\]/).pop()
                    textFormat: Text.PlainText
                    elide: Text.ElideMiddle
                    color: form.theme.text
                }
                Button {
                    visible: form.file !== ""
                    text: form.sioul.text("ui-open")
                    onClicked: Qt.openUrlExternally(form.file.startsWith("file:") ? form.file : form.theme.fileUrl(form.file))
                }
                Button {
                    text: form.sioul.text("papers-choose")
                    onClicked: picker.open()
                }
            }
            Label {
                text: form.sioul.text("papers-issued")
                color: form.theme.muted
            }
            DateField {
                id: issuedField

                theme: form.theme
                sioul: form.sioul
                locale: form.window.sioulLocale
                pickLabel: form.sioul.text("event-pick-day")
            }
            Label {
                text: form.sioul.text("papers-until")
                color: form.theme.muted
            }
            DateField {
                id: untilField

                theme: form.theme
                sioul: form.sioul
                locale: form.window.sioulLocale
                pickLabel: form.sioul.text("event-pick-day")
            }
            Label {
                Layout.columnSpan: 2
                Layout.fillWidth: true
                text: form.sioul.text("papers-until-hint")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: form.theme.muted
            }
            Label {
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: 6
                text: form.sioul.text("papers-for")
                color: form.theme.muted
            }
            // You, the household's persons, several at once; a text naming nobody stays until unticked.
            Flow {
                objectName: "paperFor"
                Layout.fillWidth: true
                spacing: 4

                Repeater {
                    model: form.choices.concat(form.texts.map(t => ({ id: "", name: t, text: true })))

                    delegate: CheckBox {
                        id: forBox

                        required property var modelData

                        objectName: "paperFor:" + forBox.modelData.name
                        text: form.theme.plain(forBox.modelData.name)
                        checked: forBox.modelData.text ? true : form.chosen.indexOf(forBox.modelData.id) >= 0
                        onToggled: {
                            if (forBox.modelData.text)
                                form.texts = form.texts.filter(t => t !== forBox.modelData.name)
                            else
                                form.toggle(forBox.modelData.id, forBox.checked)
                        }
                    }
                }
                Button {
                    objectName: "paperForNew"
                    flat: true
                    text: form.sioul.text("papers-for-new")
                    onClicked: {
                        const dialog = newPerson.now()
                        dialog.relations = (JSON.parse(form.sioul.household() || "{}").relations || []).filter(r => r.id !== "other-parent")
                        dialog.begin("", "", "")
                    }
                }
            }
            Label {
                Layout.alignment: Qt.AlignTop
                text: form.sioul.text("papers-notes")
                color: form.theme.muted
            }
            TextArea {
                id: notesArea

                Layout.fillWidth: true
                Layout.preferredHeight: 70
                wrapMode: TextEdit.Wrap
                background: Rectangle {
                    color: form.theme.surface
                    border.color: notesArea.activeFocus ? form.theme.focus : form.theme.line
                    radius: 3
                }
            }
            Label {
                visible: form.problem !== ""
                Layout.columnSpan: 2
                Layout.fillWidth: true
                text: form.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: form.theme.warm
            }
            RowLayout {
                Layout.columnSpan: 2
                Layout.fillWidth: true
                spacing: 8

                Button {
                    visible: form.paperId !== ""
                    flat: true
                    text: form.sioul.text("papers-remove")
                    onClicked: removing.ask(titleField.text, form.sioul.text("papers-remove-ask"), form.sioul.text("papers-remove"))
                }
                Item {
                    Layout.fillWidth: true
                }
                Button {
                    text: form.sioul.text("ui-cancel")
                    onClicked: form.close()
                }
                Button {
                    highlighted: true
                    text: form.sioul.text("ui-save")
                    onClicked: {
                        const edit = { kind: form.kinds[kindChoice.currentIndex].id, title: titleField.text, file: form.file, issued: issuedField.date, until: untilField.date, for: form.chosen.length > 0 || form.texts.length > 0 ? form.chosen : ["me"], texts: form.texts, notes: notesArea.text }
                        form.problem = form.sioul.savePaper(form.paperId, JSON.stringify(edit))
                        if (form.problem === "") {
                            form.close()
                            form.saved()
                        }
                    }
                }
            }
        }
    }

    // A new person, from "For": ticked once saved.
    Later {
        id: newPerson

        sourceComponent: Component {
            PersonDialog {
                sioul: form.sioul
                theme: form.theme
                window: form.window
                onSaved: id => {
                    form.readChoices()
                    if (id !== "")
                        form.toggle(id, true)
                }
            }
        }
    }

    FileDialog {
        id: picker

        fileMode: FileDialog.OpenFile
        onAccepted: form.file = picker.selectedFile.toString()
    }

    ConfirmDialog {
        id: removing

        sioul: form.sioul
        theme: form.theme
        onConfirmed: {
            form.problem = form.sioul.removePaper(form.paperId)
            if (form.problem === "") {
                form.close()
                form.saved()
            }
        }
    }
}
