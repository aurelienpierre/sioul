// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The end of the work day, or of the whole day before sleep (docs/reviews.md).
// First, in words, how the day started and what the plan did with it; for the
// night, the end of work as said then, tomorrow's first step and what the day
// asked and gave, never a number. Then how the day felt, its mix, a note in
// Markdown: each answer optional, and closing without any is as plain as with.
// "Close the work day" closes it as "Done for today" always did, and the
// screen after it says where everything went; "Not now" changes nothing. The
// notes of the last days, folded, in the words said then: no count, no
// streak, no calendar. Made the first time it is asked for (main.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: sheet

    required property var sioul
    required property var theme
    required property var window
    // The sheet's view (`reviews::view`): kind, date, title, lines, balance, the answers given, back.
    property var view: null
    // The answers being given: "" for none.
    property string felt: ""
    property string mix: ""
    property bool previewing: false
    property bool backShown: false

    // "work" or "night": the sheet for it, with the answers already given, or
    // those being given when it was left with "Not now" the same day.
    function show(kind) {
        const view = JSON.parse(sheet.sioul.dayReview(kind) || "null")
        if (!view)
            return
        if (sheet.view === null || sheet.view.kind !== view.kind || sheet.view.date !== view.date) {
            sheet.felt = view.felt
            sheet.mix = view.mix
            memo.text = view.memo
            sheet.previewing = false
            sheet.backShown = false
        }
        sheet.view = view
        sheet.open()
    }

    // Kept; the work day closed as "Done for today" closes it, its screen after.
    function closeIt() {
        if (sheet.view === null)
            return
        const closing = sheet.sioul.closeReview(sheet.view.kind, sheet.felt, sheet.mix, memo.text)
        // Given: the next time starts from what is kept.
        sheet.view = null
        sheet.close()
        if (closing !== "")
            sheet.window.showClosing(JSON.parse(closing))
    }

    // For the window's pictures: a note written, the sheet as an image.
    function write(text) {
        memo.text = text
    }

    function grab(path) {
        const item = sheet.contentItem.parent || sheet.contentItem
        item.grabToImage(result => result.saveToFile(path))
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(520, (parent ? parent.width : 520) - 2 * sheet.theme.gap)
    // Taller than the window: its content scrolls, the buttons stay.
    height: Math.min(sheet.implicitHeight, (parent ? parent.height : 900) - 2 * sheet.theme.gap)
    title: sheet.view ? sheet.view.title : ""
    onOpened: closeButton.forceActiveFocus()

    // A long title wraps on a phone rather than ending in "…".
    header: Label {
        text: sheet.title
        visible: sheet.title !== ""
        wrapMode: Text.Wrap
        textFormat: Text.PlainText
        font.bold: true
        padding: 12
        background: Rectangle {
            x: 1
            y: 1
            width: parent.width - 2
            height: parent.height - 1
            color: sheet.palette.window
        }
    }

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 10

            // How the day started and went, in words: only what was said.
            Repeater {
                model: sheet.view ? sheet.view.lines : []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: sheet.theme.text
                }
            }

            // The night: what the day asked and gave, in words.
            ColumnLayout {
                visible: sheet.view !== null && sheet.view.balance.length > 0
                Layout.fillWidth: true
                spacing: 2

                Label {
                    Layout.fillWidth: true
                    text: sheet.sioul.text("review-balance-title")
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: sheet.theme.muted
                }
                Repeater {
                    model: sheet.view ? sheet.view.balance : []

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: sheet.theme.text
                    }
                }
            }

            // How the day felt: one word at most; tapped again, none.
            Label {
                Layout.fillWidth: true
                Layout.topMargin: 4
                text: sheet.sioul.text("review-felt-label")
                wrapMode: Text.Wrap
                color: sheet.theme.muted
            }
            Flow {
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: ["light", "usual", "heavy", "gave_back"]

                    delegate: Button {
                        id: feltChip

                        required property string modelData

                        text: sheet.sioul.text("review-felt-" + feltChip.modelData.replace("_", "-"))
                        checkable: true
                        checked: sheet.felt === feltChip.modelData
                        onClicked: {
                            sheet.felt = sheet.felt === feltChip.modelData ? "" : feltChip.modelData
                            feltChip.checked = Qt.binding(() => sheet.felt === feltChip.modelData)
                        }
                    }
                }
            }

            // The mix of duty and what is yours.
            Label {
                Layout.fillWidth: true
                text: sheet.sioul.text("review-mix-label")
                wrapMode: Text.Wrap
                color: sheet.theme.muted
            }
            Flow {
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: ["too_much", "about_right", "too_empty"]

                    delegate: Button {
                        id: mixChip

                        required property string modelData

                        text: sheet.sioul.text("review-mix-" + mixChip.modelData.replace("_", "-"))
                        checkable: true
                        checked: sheet.mix === mixChip.modelData
                        onClicked: {
                            sheet.mix = sheet.mix === mixChip.modelData ? "" : mixChip.modelData
                            mixChip.checked = Qt.binding(() => sheet.mix === mixChip.modelData)
                        }
                    }
                }
            }

            // A note, in Markdown; its preview as Notes and mail show it.
            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    text: sheet.sioul.text("review-memo-label")
                    wrapMode: Text.Wrap
                    color: sheet.theme.muted
                }
                Button {
                    visible: sheet.previewing || memo.text.trim() !== ""
                    flat: true
                    text: sheet.sioul.text(sheet.previewing ? "ui-edit" : "ui-preview")
                    onClicked: sheet.previewing = !sheet.previewing
                }
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(96, sheet.previewing ? preview.implicitHeight + 24 : memo.implicitHeight + 2)
                color: sheet.theme.surface
                radius: sheet.theme.radius
                border.color: memo.activeFocus ? sheet.theme.focus : sheet.theme.line

                TextArea {
                    id: memo

                    visible: !sheet.previewing
                    anchors.fill: parent
                    anchors.margins: 1
                    wrapMode: TextArea.Wrap
                    selectByMouse: true
                    placeholderText: sheet.sioul.text("review-memo-hint")
                    color: sheet.theme.text
                    font.family: sheet.theme.readingFamily || font.family
                    font.pixelSize: sheet.theme.readingSize
                    padding: 10
                    background: null
                }
                TextEdit {
                    id: preview

                    visible: sheet.previewing
                    anchors.fill: parent
                    anchors.margins: 12
                    // rich on purpose: Markdown made HTML in Rust (compose::markdown_html): HTML written in it shows as words, a picture from elsewhere as a link.
                    text: sheet.previewing ? sheet.theme.spaced(sheet.sioul.preview(memo.text).replace(/<a href=/g, '<a style="color:' + sheet.theme.accent + '" href=')) : ""
                    textFormat: TextEdit.RichText
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    color: sheet.theme.text
                    font.family: sheet.theme.readingFamily || font.family
                    font.pixelSize: sheet.theme.readingSize
                    onLinkActivated: link => Qt.openUrlExternally(link)
                }
            }

            // The notes of the last days, as they were said: folded, read only.
            Button {
                visible: sheet.view !== null && sheet.view.back.length > 0
                Layout.topMargin: 4
                flat: true
                text: (sheet.backShown ? "▾  " : "▸  ") + sheet.sioul.text("review-back-title")
                onClicked: sheet.backShown = !sheet.backShown
            }
            Repeater {
                model: sheet.backShown && sheet.view ? sheet.view.back : []

                delegate: ColumnLayout {
                    id: past

                    required property var modelData

                    Layout.fillWidth: true
                    Layout.leftMargin: 8
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        text: past.modelData.day
                        textFormat: Text.PlainText
                        font.pixelSize: 13
                        color: sheet.theme.muted
                    }
                    Label {
                        visible: past.modelData.line !== ""
                        Layout.fillWidth: true
                        text: past.modelData.line
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: sheet.theme.text
                    }
                    Repeater {
                        model: past.modelData.memos

                        delegate: TextEdit {
                            required property string modelData

                            Layout.fillWidth: true
                            // rich on purpose: Markdown made HTML in Rust (compose::markdown_html): HTML written in it shows as words, a picture from elsewhere as a link.
                            text: sheet.theme.spaced(modelData.replace(/<a href=/g, '<a style="color:' + sheet.theme.accent + '" href='))
                            textFormat: TextEdit.RichText
                            readOnly: true
                            selectByMouse: true
                            wrapMode: TextEdit.Wrap
                            color: sheet.theme.text
                            onLinkActivated: link => Qt.openUrlExternally(link)
                        }
                    }
                }
            }
        }
    }

    // Closing first, the way out beside it; on a narrow phone, one under the other.
    footer: Item {
        implicitHeight: buttons.implicitHeight + 24

        Flow {
            id: buttons

            anchors.fill: parent
            anchors.margins: 12
            layoutDirection: Qt.RightToLeft
            spacing: 8

            Button {
                id: closeButton

                text: sheet.theme.plain(sheet.view ? sheet.view.close : "")
                highlighted: true
                onClicked: sheet.closeIt()
                Keys.onReturnPressed: sheet.closeIt()
                Keys.onEnterPressed: sheet.closeIt()
            }
            Button {
                flat: true
                text: sheet.sioul.text("review-not-now")
                onClicked: sheet.close()
            }
        }
    }
}
