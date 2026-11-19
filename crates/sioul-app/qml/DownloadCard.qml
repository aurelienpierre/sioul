// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The question a file a site offers asks, over the site (docs/sites.md,
// "Downloads"): nothing is saved before it is answered. Headed in two
// sections. The file: its name in a field that can be changed (one plain
// file name, as Rust made it), its size when the site says it and the host
// it comes from, and a line of caution for a program. Where: the five places,
// one chosen (the one chosen last for this site first), each with a line
// saying where it goes or why it cannot take this file. Save and Cancel;
// Escape and a phone's back key are Cancel. What the card shows is
// `downloadOffer`'s (downloads::OfferView); what is pressed goes to its page
// (SiteDownloads.qml), which starts the download, or asks the folder first
// in the system's "Save as" window. Words from the site (the name, the host)
// are drawn as plain text, already cut short by Rust.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: card

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // What `downloadOffer` says of the file.
    property var offer: ({ name: "", host: "", from: "", program: false, caution: "", places: [], first: "downloads", folder: "" })
    // The place chosen here: "downloads", "folder", "notes", "papers", "letters".
    property string place: "downloads"
    // Why the file could not start, from its page; "" none.
    property string problem: ""
    // Answered (Save or Cancel): closing then says nothing more.
    property bool answered: false

    // Save pressed: the place, and the name in the field.
    signal saved(string place, string name)
    // Cancel, Escape, a phone's back key: nothing saved.
    signal cancelled()

    // The question for one file.
    function ask(offer) {
        card.offer = offer
        card.place = offer.first || "downloads"
        card.problem = ""
        card.answered = false
        nameField.text = offer.name || ""
        card.open()
        nameField.forceActiveFocus()
        // Its name selected without its extension, as a "Save as" window does.
        const dot = nameField.text.lastIndexOf(".")
        nameField.select(0, dot > 0 ? dot : nameField.text.length)
    }

    // Save: a name is needed; the page closes the card once the file starts.
    function save() {
        if (nameField.text.trim() === "") {
            card.problem = card.sioul.text("download-no-name")
            return
        }
        card.problem = ""
        card.saved(card.place, nameField.text)
    }

    // Closed by its page once answered: nothing more said.
    function done() {
        card.answered = true
        card.close()
    }

    function cancel() {
        if (card.answered)
            return
        card.answered = true
        card.close()
        card.cancelled()
    }

    // However it closes unanswered (Escape, a phone's back key), nothing is saved;
    // a close heard once the next question opened belongs to the one before.
    onClosed: {
        if (!card.visible)
            card.cancel()
    }

    objectName: "downloadCard"
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    // The keys come here while it shows: Tab through it, Enter to save, Escape to cancel.
    focus: true
    closePolicy: Popup.CloseOnEscape
    width: Math.min(520, card.roomWidth - 2 * card.safeGap)
    title: card.sioul.text("download-title")

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true
        implicitHeight: Math.min(column.implicitHeight, card.roomHeight - 160)

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: 12

            // What comes: its name, which you may change, its size and its host, a program's caution.
            Section {
                objectName: "downloadWhat"
                Layout.fillWidth: true
                theme: card.theme
                title: card.sioul.text("download-what")

                Label {
                    Layout.fillWidth: true
                    text: card.sioul.text("download-name")
                    color: card.theme.muted
                }
                TextField {
                    id: nameField

                    objectName: "downloadName"
                    Layout.fillWidth: true
                    selectByMouse: true
                    Accessible.name: card.sioul.text("download-name")
                    onAccepted: card.save()
                }
                Label {
                    objectName: "downloadFrom"
                    visible: (card.offer.from || "") !== ""
                    Layout.fillWidth: true
                    text: card.offer.from || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
                RowLayout {
                    visible: card.offer.program === true
                    Layout.fillWidth: true
                    spacing: 8

                    Icon {
                        Layout.alignment: Qt.AlignTop
                        iconName: "dialog-warning"
                    }
                    Label {
                        objectName: "downloadCaution"
                        Layout.fillWidth: true
                        text: card.offer.caution || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: card.theme.warm
                    }
                }
            }

            // Where it goes: one place, each saying where, what follows, or why not this file.
            Section {
                objectName: "downloadWhere"
                Layout.fillWidth: true
                theme: card.theme
                title: card.sioul.text("download-where")

                ButtonGroup {
                    id: places
                }
                Repeater {
                    model: card.offer.places || []

                    delegate: ColumnLayout {
                        id: choice

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 0

                        RadioButton {
                            id: radio

                            objectName: "downloadPlace:" + choice.modelData.id
                            Layout.fillWidth: true
                            ButtonGroup.group: places
                            enabled: choice.modelData.enabled === true
                            checked: choice.modelData.id === card.place
                            text: card.theme.plain(choice.modelData.label)
                            Accessible.name: choice.modelData.label
                            Accessible.description: choice.modelData.detail
                            onToggled: {
                                if (radio.checked)
                                    card.place = choice.modelData.id
                            }
                            Keys.onReturnPressed: card.save()
                            Keys.onEnterPressed: card.save()
                        }
                        Label {
                            Layout.fillWidth: true
                            Layout.leftMargin: radio.indicator ? radio.indicator.width + radio.spacing + radio.leftPadding : 0
                            text: choice.modelData.detail
                            textFormat: Text.PlainText
                            // At words; a long folder's path anywhere it must.
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: card.theme.muted
                            opacity: radio.enabled ? 1 : 0.8
                        }
                    }
                }
            }

            Label {
                objectName: "downloadProblem"
                visible: card.problem !== ""
                Layout.fillWidth: true
                text: card.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: card.theme.warm
            }
        }
    }

    // Save and Cancel under what scrolls, always in view, a phone's height too.
    footer: Pane {
        background: null

        RowLayout {
            anchors.left: parent.left
            anchors.right: parent.right
            spacing: 8

            Item {
                Layout.fillWidth: true
            }
            Button {
                objectName: "downloadCancel"
                text: card.sioul.text("ui-cancel")
                onClicked: card.cancel()
            }
            Button {
                objectName: "downloadSave"
                highlighted: true
                text: card.sioul.text("ui-save")
                onClicked: card.save()
                Keys.onReturnPressed: card.save()
                Keys.onEnterPressed: card.save()
            }
        }
    }
}
