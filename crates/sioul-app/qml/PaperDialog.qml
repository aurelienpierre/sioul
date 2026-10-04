// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A paper of the wallet, new or changed: what it is, its file, from when and
// until when it holds, whose it is. A file from elsewhere is copied into the
// wallet's folder when saved; an end is proposed from the issue for a passport,
// an identity card or a warranty.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Dialog {
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

    signal saved

    // A new paper, perhaps from a file (an attachment kept, a scan).
    function begin(file, title, kind) {
        form.paperId = ""
        form.problem = ""
        kindChoice.currentIndex = Math.max(0, form.kinds.findIndex(k => k.id === (kind || "other")))
        titleField.text = title || ""
        form.file = file || ""
        issuedField.date = ""
        untilField.date = ""
        holderField.text = ""
        notesArea.text = ""
        form.open()
        titleField.forceActiveFocus()
    }

    // A paper of the wallet, changed.
    function edit(paper) {
        form.paperId = paper.id
        form.problem = ""
        kindChoice.currentIndex = Math.max(0, form.kinds.findIndex(k => k.id === paper.kind))
        titleField.text = paper.title
        form.file = paper.file
        issuedField.date = paper.issued
        untilField.date = paper.until
        holderField.text = paper.holder
        notesArea.text = paper.notes
        form.open()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, (parent ? parent.width : 560) - 2 * form.theme.gap)
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
            ComboBox {
                id: kindChoice

                Layout.fillWidth: true
                model: form.kinds.map(k => k.label)
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
                text: form.sioul.text("papers-holder")
                color: form.theme.muted
            }
            TextField {
                id: holderField

                Layout.fillWidth: true
                placeholderText: form.sioul.text("papers-holder-hint")
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
                        const edit = { kind: form.kinds[kindChoice.currentIndex].id, title: titleField.text, file: form.file, issued: issuedField.date, until: untilField.date, holder: holderField.text, notes: notesArea.text }
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
