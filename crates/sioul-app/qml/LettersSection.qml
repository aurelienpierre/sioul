// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Paper letters waiting, on the Letters page (LettersPage.qml): each scan
// read, as a card. Who sent it, what it is, how much, by when (legal delays
// counted into a date); a task for that date, its appointment in the agenda,
// its scan one click away; done, it is filed with its project. A scan linked
// where it is says so; one moved or deleted since says so, calmly. The
// page's bar adds and links scans (`addScan`, `linkScans`); a phone reads
// none (a computer does): it shows the cards alone.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window
    // The letters as `letters::view` gives them, read by the page.
    property var shown: ({ inbox: "", letters: [], filed: [], projects: [], missing: "", reader: "" })
    property string problem: ""

    // No folder for scans yet: the page's settings, where one is chosen.
    signal settingsWanted
    // A letter acted on: the page reads the letters again.
    signal changed

    // A scan chosen on this computer, copied into the folder (the New menu's
    // "A paper letter (scan)…" too); without a folder, where to choose one.
    function addScan() {
        if (section.sioul.lettersFolder() === "")
            section.settingsWanted()
        else
            scanPicker.open()
    }

    // Scans kept elsewhere, linked where they are: never copied.
    function linkScans() {
        linkPicker.open()
    }

    function reload() {
        section.changed()
    }

    spacing: 8

    Label {
        visible: section.problem !== ""
        Layout.fillWidth: true
        text: section.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: section.theme.warm
    }

    Repeater {
        model: section.shown.letters

        delegate: Panel {
            id: card

            required property var modelData
            // Found by the page, to bring it into view (LettersPage.qml, `openLetter`).
            readonly property string letterId: card.modelData.id

            Layout.fillWidth: true
            theme: section.theme

            ColumnLayout {
                anchors.fill: parent
                spacing: 6

                // Never wider than the card: its title wraps (a phone's 412 points, review of 1017fa4, L3).
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    Icon {
                        iconName: "mail-mark-unread"
                    }
                    // What the scan says (its letterhead, its words) is plain text; its
                    // width the row's room, never its words' (a phone's 412 points).
                    Label {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: card.modelData.sender + " · " + card.modelData.kind_label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 16
                        color: section.theme.text
                    }
                    Label {
                        text: card.modelData.received
                        textFormat: Text.PlainText
                        font.pixelSize: 12
                        color: section.theme.muted
                    }
                }
                Repeater {
                    model: card.modelData.lines

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.text
                    }
                }
                Label {
                    visible: card.modelData.problem !== ""
                    Layout.fillWidth: true
                    text: section.sioul.textWith("letters-unread", "problem", card.modelData.problem)
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: section.theme.muted
                }
                // Linked where it is, not read yet, or no longer there: said calmly, once.
                Label {
                    visible: card.modelData.gone === true || card.modelData.waiting === true || card.modelData.linked === true
                    Layout.fillWidth: true
                    text: card.modelData.gone === true ? section.sioul.textWith("letters-gone", "file", card.modelData.file)
                        : card.modelData.waiting === true ? section.sioul.text("letters-waiting-read")
                        : section.sioul.textWith("letters-linked-here", "file", card.modelData.file)
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: section.theme.muted
                }
                // Its actions, on more lines where the card is narrow.
                Flow {
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        text: section.sioul.text("letters-scan")
                        // Its path made an address: a "#" or "%" in a scan's name stays in it; Windows' paths too.
                        onClicked: Qt.openUrlExternally(section.theme.fileUrl(card.modelData.file))
                    }
                    Button {
                        visible: card.modelData.deadline !== "" && card.modelData.task === ""
                        highlighted: true
                        text: section.sioul.text("letters-task")
                        onClicked: {
                            section.problem = section.sioul.letterTask(card.modelData.id)
                            section.reload()
                        }
                    }
                    Button {
                        visible: card.modelData.task !== ""
                        flat: true
                        text: section.sioul.text("letters-task-made")
                        onClicked: section.window.openTask(card.modelData.task)
                    }
                    Button {
                        visible: card.modelData.appointment
                        text: section.sioul.text("letters-event")
                        onClicked: section.problem = section.sioul.letterEvent(card.modelData.id)
                    }
                    PlainComboBox {
                        id: projectChoice

                        readonly property var choices: [{ id: "", title: section.sioul.text("letters-no-project") }].concat(section.shown.projects)

                        width: 170
                        model: choices.map(c => section.theme.plain(c.title))
                        currentIndex: Math.max(0, choices.findIndex(c => c.id === card.modelData.project))
                    }
                    Button {
                        text: section.sioul.text("letters-done")
                        onClicked: {
                            section.problem = section.sioul.letterDone(card.modelData.id, projectChoice.choices[projectChoice.currentIndex].id)
                            section.reload()
                        }
                    }
                }
            }
        }
    }

    // PDF and pictures, the kinds Sioul reads; several at once. Each copied,
    // the chosen file left where it was; what happened said on the status line.
    FileDialog {
        id: scanPicker

        title: section.sioul.text("letters-add-scan-title")
        fileMode: FileDialog.OpenFiles
        nameFilters: [section.sioul.text("letters-add-scan-filter") + " (*.pdf *.png *.jpg *.jpeg *.tif *.tiff *.webp)"]
        onAccepted: section.sioul.status = section.sioul.lettersAdd(JSON.stringify(Array.from(scanPicker.selectedFiles, f => f.toString())))
    }
    // The same kinds, linked where they are: recorded there, read there, never copied.
    FileDialog {
        id: linkPicker

        title: section.sioul.text("letters-link-title")
        fileMode: FileDialog.OpenFiles
        nameFilters: [section.sioul.text("letters-add-scan-filter") + " (*.pdf *.png *.jpg *.jpeg *.tif *.tiff *.webp)"]
        onAccepted: {
            section.sioul.status = section.sioul.lettersLink(JSON.stringify(Array.from(linkPicker.selectedFiles, f => f.toString())))
            section.changed()
        }
    }
}
