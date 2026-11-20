// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The question a file a site offers asks, over the site or over the pop-up
// that offered it (docs/sites.md, "Downloads"). Headed in two sections. The
// file: its name in a field that can be changed (one plain file name, as
// Rust made it), its size when the site says it and the host it comes from,
// one calm line saying it waits in Sioul's own folder until a place is
// chosen and that Cancel deletes it, and a line of caution for a program.
// Where: the five places, one chosen (the one chosen last for this site
// first), each with a line saying where it goes or why it cannot take this
// file; they follow the name typed. A site that had a file refused, or
// offers several, gets a flat link to refuse every file from it until it is
// closed. Save and Cancel; Escape and a phone's back key are Cancel. By
// keyboard: Tab through it; the arrows move the choice among the places;
// Enter on a place chooses it and saves; Enter on Cancel cancels. What the
// card shows is `downloadOffer`'s (downloads::OfferView); what is pressed
// goes to its page (SiteDownloads.qml). Words from the site (the name, the
// host) are drawn as plain text, already cut short by Rust; a problem is
// announced to a screen reader.

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
    property var offer: ({ name: "", host: "", from: "", waits: "", program: false, caution: "", places: [], first: "downloads", folder: "" })
    // Whether the link to refuse every file from this site shows.
    property bool canRefuseSite: false
    // Each place's button by its id, as the rows made them (for the arrows).
    property var radios: ({})
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
    // "Refuse every file from this site".
    signal siteRefused()
    // The name in the field changed, by you.
    signal renamed(string name)

    // The question for one file; `problem`, why its last place refused it ("" none).
    function ask(offer, problem, canRefuseSite) {
        card.offer = offer
        card.place = offer.first || "downloads"
        card.answered = false
        card.canRefuseSite = canRefuseSite === true
        nameField.text = offer.name || ""
        card.open()
        nameField.forceActiveFocus()
        // Its name selected without its extension, as a "Save as" window does.
        const dot = nameField.text.lastIndexOf(".")
        nameField.select(0, dot > 0 ? dot : nameField.text.length)
        card.say(problem || "")
    }

    // What the places can take for the name typed: the choice kept while it can take it.
    function update(offer) {
        const place = card.place
        card.offer = Object.assign({}, card.offer, { program: offer.program, caution: offer.caution, places: offer.places })
        const still = (offer.places || []).some(p => p.id === place && p.enabled)
        card.place = still ? place : "downloads"
    }

    // The keys back in the card, its name's field first (the system's "Save as" window closed).
    function refocus() {
        card.forceActiveFocus()
        nameField.forceActiveFocus()
    }

    // A problem shown under the places, and said to a screen reader.
    function say(problem) {
        card.problem = problem
        if (problem !== "" && typeof problemLine.Accessible.announce === "function")
            problemLine.Accessible.announce(problem)
    }

    // The arrows: the next place that can take the file, chosen and focused.
    function moveChoice(step) {
        const places = card.offer.places || []
        let at = places.findIndex(p => p.id === card.place)
        for (let i = 0; i < places.length; ++i) {
            at = (at + step + places.length) % places.length
            if (places[at].enabled) {
                card.place = places[at].id
                const radio = card.radios[places[at].id]
                if (radio) {
                    radio.checked = true
                    radio.forceActiveFocus()
                }
                return
            }
        }
    }

    // Save: a name is needed; the page closes the card once the place is taken.
    function save() {
        if (nameField.text.trim() === "") {
            card.say(card.sioul.text("download-no-name"))
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
                    onTextEdited: renaming.restart()
                }
                // The places asked again a moment after the name stops changing.
                Timer {
                    id: renaming

                    interval: 300
                    onTriggered: card.renamed(nameField.text)
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
                // Where it waits meanwhile, and what Cancel does.
                Label {
                    objectName: "downloadWaits"
                    visible: (card.offer.waits || "") !== ""
                    Layout.fillWidth: true
                    text: card.offer.waits || ""
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
                            Component.onCompleted: card.radios[choice.modelData.id] = radio
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
                            // Enter on a place chooses it, then saves; the arrows move the choice.
                            Keys.onReturnPressed: choice.chooseAndSave()
                            Keys.onEnterPressed: choice.chooseAndSave()
                            Keys.onUpPressed: card.moveChoice(-1)
                            Keys.onDownPressed: card.moveChoice(1)
                            Keys.onLeftPressed: card.moveChoice(-1)
                            Keys.onRightPressed: card.moveChoice(1)
                        }
                        function chooseAndSave() {
                            card.place = choice.modelData.id
                            radio.checked = true
                            card.save()
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
                id: problemLine

                objectName: "downloadProblem"
                visible: card.problem !== ""
                Layout.fillWidth: true
                text: card.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: card.theme.warm
                Accessible.role: Accessible.AlertMessage
                Accessible.name: card.problem
            }

            // A site that keeps offering files: refused as a whole until it is closed or reloaded.
            Button {
                id: refuseLink

                objectName: "downloadRefuseSite"
                visible: card.canRefuseSite
                Layout.fillWidth: true
                flat: true
                text: card.sioul.text("download-refuse-site")
                Accessible.name: card.sioul.text("download-refuse-site")
                // A link: no frame, its words alone.
                background: null
                leftPadding: 0
                rightPadding: 0
                contentItem: Label {
                    text: card.sioul.text("download-refuse-site")
                    wrapMode: Text.Wrap
                    color: card.theme.accent
                    font.underline: true
                    // Its focus seen by keyboard, as a flat link has no frame.
                    font.bold: refuseLink.visualFocus
                }
                onClicked: {
                    if (card.answered)
                        return
                    card.answered = true
                    card.close()
                    card.siteRefused()
                }
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
                Keys.onReturnPressed: card.cancel()
                Keys.onEnterPressed: card.cancel()
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
