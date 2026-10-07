// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Writing a message, in Markdown. In view: to whom, the subject, the text,
// "Attach" and "Send". Cc and Bcc are folded, the sending account shows only
// when you have several. Everything is saved as you type, so closing the
// window loses nothing: the draft waits in Drafts. "Send" waits ten seconds
// with "Undo" in the main window before the message leaves. A message your
// security key signs is signed here first, at Send, with its PIN and maybe a
// touch (SecurityKeyBand.qml); the ten seconds come after.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import com.aurelienpierre.sioul

SioulWindow {
    id: compose

    required property var sioul
    required property string draftId
    property var draft: JSON.parse(compose.sioul.draft(compose.draftId) || "null")
    property bool previewing: false
    property bool copiesShown: compose.draft !== null && (compose.draft.cc !== "" || compose.draft.bcc !== "")
    // "Saved at 14:02", or what stops sending.
    property string note: ""
    property bool problem: false
    // Sent or discarded: the main window says it, with "Undo".
    property bool leaving: false
    // What OpenPGP allows: signing with your key, encrypting to every recipient's.
    property var protection: compose.draft ? compose.draft.protection : ({ can_sign: false, can_encrypt: false, missing: [] })
    // The security key that signs for this address has an expired certificate.
    readonly property bool expired: compose.protection.expired !== undefined && compose.protection.expired !== ""
    // "Attaching 2 files…" while files shared from another application are copied (outside.rs).
    property string attaching: compose.draft && compose.draft.attaching ? compose.draft.attaching : ""

    signal finished(string id)

    // What is written, as the backend saves it.
    function edit() {
        return JSON.stringify({
            id: compose.draftId,
            account: compose.draft && compose.draft.accounts.length > 1 ? from.currentValue : (compose.draft ? compose.draft.account : ""),
            to: to.text,
            cc: cc.text,
            bcc: bcc.text,
            subject: subject.text,
            body: body.text,
            sign: sign.checked,
            encrypt: encrypt.checked
        })
    }

    function save() {
        autosave.stop()
        compose.note = compose.sioul.saveDraft(compose.edit())
        compose.problem = false
        compose.readProtection()
    }

    // Recipients changed, or keys were found: what can be signed and encrypted, again.
    function readProtection() {
        const fresh = JSON.parse(compose.sioul.draft(compose.draftId) || "null")
        if (fresh)
            compose.protection = fresh.protection
    }

    Connections {
        target: compose.sioul

        function onKeysChanged() {
            compose.readProtection()
        }

        // The files shared from another application came: attached, or said why not.
        function onDraftFilesArrived(id, line, problem) {
            if (id !== compose.draftId)
                return
            compose.attaching = ""
            compose.reloadAttachments()
            compose.note = line
            compose.problem = problem
        }
    }

    // Attachments changed: read them again, the text stays as typed.
    function reloadAttachments() {
        const fresh = JSON.parse(compose.sioul.draft(compose.draftId) || "null")
        if (fresh)
            attachments.model = fresh.attachments.concat(fresh.forwarded.map(f => Object.assign({ forwarded: true }, f)))
    }

    function attach(urls) {
        compose.save()
        for (const url of urls) {
            const problem = compose.sioul.attach(compose.draftId, url.toString())
            if (problem) {
                compose.note = problem
                compose.problem = true
            }
        }
        compose.reloadAttachments()
    }

    function send() {
        // Signing already: once is enough.
        if (keyBand.said.state === "working" || keyBand.said.state === "touch")
            return
        compose.save()
        // Signed with your security key: the key is asked here, before the ten seconds of "Undo".
        const key = JSON.parse(compose.sioul.securityKeyForDraft(compose.draftId) || "null")
        if (key && key.expired) {
            keyBand.say(key.expired)
            return
        }
        if (key) {
            keyBand.start()
            return
        }
        const problem = compose.sioul.send(compose.draftId)
        if (problem) {
            compose.note = problem
            compose.problem = true
            return
        }
        compose.leaving = true
        compose.close()
    }

    function discard() {
        autosave.stop()
        compose.sioul.discard(compose.draftId)
        compose.leaving = true
        compose.close()
    }

    // For the window's images: the writing window as it opens.
    function grab(path) {
        frame.grabToImage(result => result.saveToFile(path))
    }

    // "Write" from a contact: the address already there.
    function prefill(recipients) {
        to.text = recipients
        compose.save()
        subject.forceActiveFocus()
    }

    // For the window's pictures: as if "Sign" were ticked.
    function setSigned(on) {
        sign.checked = on
        compose.save()
    }

    // For the window's tests: as if typed.
    function fill(recipients, title, text) {
        to.text = recipients
        subject.text = title
        body.text = text + body.text
        compose.save()
    }

    width: 760
    height: 680
    minimumWidth: 480
    minimumHeight: 420
    visible: true
    title: subject.text !== "" ? subject.text : (compose.draft ? compose.draft.title : "")


    Component.onCompleted: {
        compose.reloadAttachments()
        // An answer starts in the text, above the quote; anything else with whom it goes to.
        if (compose.draft && compose.draft.to !== "") {
            body.forceActiveFocus()
            body.cursorPosition = 0
        } else {
            to.forceActiveFocus()
        }
    }

    onClosing: {
        if (!compose.leaving) {
            // A key plugged in later sends nothing for a window that is gone.
            compose.sioul.securityKeyNotNow(compose.draftId)
            compose.save()
            compose.sioul.draftClosed(compose.draftId)
        }
        compose.finished(compose.draftId)
    }

    Timer {
        id: autosave

        interval: 800
        onTriggered: compose.save()
    }

    Shortcut {
        sequences: ["Ctrl+Return", "Ctrl+Enter"]
        onActivated: compose.send()
    }
    Shortcut {
        sequence: "Ctrl+P"
        onActivated: compose.previewing = !compose.previewing
    }

    FileDialog {
        id: picker

        title: compose.sioul.text("ui-attach")
        fileMode: FileDialog.OpenFiles
        onAccepted: compose.attach(picker.selectedFiles)
    }

    Rectangle {
        id: frame

        anchors.fill: parent
        color: compose.theme.background

        // Files dropped anywhere on the window are attached.
        DropArea {
            anchors.fill: parent
            onDropped: drop => {
                if (drop.hasUrls)
                    compose.attach(drop.urls)
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: compose.theme.gap
            spacing: 10

            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 12
                rowSpacing: 8

                Label {
                    visible: compose.draft !== null && compose.draft.accounts.length > 1
                    text: compose.sioul.text("compose-from")
                    color: compose.theme.muted
                }
                ComboBox {
                    id: from

                    visible: compose.draft !== null && compose.draft.accounts.length > 1
                    Layout.fillWidth: true
                    model: compose.draft ? compose.draft.accounts : []
                    textRole: "label"
                    valueRole: "id"
                    Component.onCompleted: from.currentIndex = compose.draft ? from.indexOfValue(compose.draft.account) : 0
                    onActivated: autosave.restart()
                }

                Label {
                    text: compose.sioul.text("compose-to")
                    color: compose.theme.muted
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    AddressField {
                        id: to

                        Layout.fillWidth: true
                        sioul: compose.sioul
                        theme: compose.theme
                        text: compose.draft ? compose.draft.to : ""
                        placeholderText: compose.sioul.text("ui-to-placeholder")
                        onTextEdited: autosave.restart()
                        onPicked: autosave.restart()
                    }
                    Button {
                        visible: !compose.copiesShown
                        flat: true
                        text: compose.sioul.text("ui-cc-bcc")
                        onClicked: compose.copiesShown = true
                    }
                }

                Label {
                    visible: compose.copiesShown
                    text: compose.sioul.text("ui-cc")
                    color: compose.theme.muted
                }
                AddressField {
                    id: cc

                    visible: compose.copiesShown
                    Layout.fillWidth: true
                    sioul: compose.sioul
                    theme: compose.theme
                    text: compose.draft ? compose.draft.cc : ""
                    onTextEdited: autosave.restart()
                    onPicked: autosave.restart()
                }
                Label {
                    visible: compose.copiesShown
                    text: compose.sioul.text("ui-bcc")
                    color: compose.theme.muted
                }
                AddressField {
                    id: bcc

                    visible: compose.copiesShown
                    Layout.fillWidth: true
                    sioul: compose.sioul
                    theme: compose.theme
                    text: compose.draft ? compose.draft.bcc : ""
                    placeholderText: compose.sioul.text("ui-bcc-note")
                    onTextEdited: autosave.restart()
                    onPicked: autosave.restart()
                }

                Label {
                    text: compose.sioul.text("compose-subject")
                    color: compose.theme.muted
                }
                TextField {
                    id: subject

                    Layout.fillWidth: true
                    text: compose.draft ? compose.draft.subject : ""
                    onTextEdited: autosave.restart()
                }
            }

            // The text, in Markdown, or what it becomes.
            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                color: compose.theme.surface
                radius: compose.theme.radius
                border.color: body.activeFocus ? compose.theme.focus : compose.theme.line

                ScrollView {
                    id: editor

                    visible: !compose.previewing
                    anchors.fill: parent
                    anchors.margins: 1

                    TextArea {
                        id: body

                        text: compose.draft ? compose.draft.body : ""
                        wrapMode: TextArea.Wrap
                        selectByMouse: true
                        color: compose.theme.text
                        font.family: compose.theme.readingFamily || font.family
                        font.pixelSize: compose.theme.readingSize
                        padding: 12
                        background: null
                        onTextChanged: if (body.activeFocus) autosave.restart()

                        TextSpacing {
                            document: body.textDocument
                            spacing: compose.theme.readingSpacing
                        }
                    }
                }
                ScrollView {
                    id: previewScroll

                    visible: compose.previewing
                    anchors.fill: parent
                    anchors.margins: 12
                    contentWidth: availableWidth

                    TextEdit {
                        width: previewScroll.availableWidth
                        text: compose.previewing ? compose.sioul.preview(body.text).replace(/<a href=/g, '<a style="color:' + compose.theme.accent + '" href=') : ""
                        textFormat: TextEdit.RichText
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.Wrap
                        color: compose.theme.text
                        font: compose.font
                    }
                }
            }

            // What goes below the text when it is sent: it names the sender answered, plain text.
            Label {
                visible: compose.draft !== null && compose.draft.below !== ""
                Layout.fillWidth: true
                text: compose.draft ? compose.draft.below : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: compose.theme.muted
                font.pixelSize: 13
            }

            // Attachments: those you added, and those a forward carries.
            Flow {
                Layout.fillWidth: true
                spacing: 6
                visible: attachments.count > 0

                Repeater {
                    id: attachments

                    model: []

                    delegate: Rectangle {
                        id: chip

                        required property var modelData

                        width: chipRow.implicitWidth + 16
                        height: chipRow.implicitHeight + 8
                        radius: compose.theme.radius
                        color: compose.theme.surface
                        border.color: compose.theme.line

                        RowLayout {
                            id: chipRow

                            anchors.centerIn: parent
                            spacing: 6

                            Icon {
                                iconName: "mail-attachment"
                                size: 16
                            }
                            Label {
                                text: chip.modelData.name + (chip.modelData.size ? "  ·  " + chip.modelData.size : "")
                                textFormat: Text.PlainText
                                color: compose.theme.text
                                font.pixelSize: 13
                            }
                            ToolButton {
                                text: "×"
                                font.pixelSize: 14
                                Accessible.name: compose.sioul.text("ui-remove")
                                ToolTip.visible: hovered
                                ToolTip.text: compose.sioul.text("ui-remove")
                                onClicked: {
                                    compose.sioul.detach(compose.draftId, chip.modelData.index, !!chip.modelData.forwarded)
                                    compose.reloadAttachments()
                                }
                            }
                        }
                    }
                }
            }

            // Signing and encrypting: there once you or they have a key, never in the way before.
            RowLayout {
                visible: compose.protection.can_sign || compose.protection.can_encrypt || encrypt.checked || compose.expired
                Layout.fillWidth: true
                spacing: 8

                // A security key whose certificate expired still offers "Sign": ticked, it says why it cannot.
                CheckBox {
                    id: sign

                    text: compose.sioul.text("ui-sign")
                    enabled: compose.protection.can_sign || compose.expired
                    checked: compose.draft !== null && compose.draft.sign
                    onToggled: autosave.restart()
                }
                CheckBox {
                    id: encrypt

                    text: compose.sioul.text("ui-encrypt")
                    enabled: compose.protection.can_encrypt || encrypt.checked
                    checked: compose.draft !== null && compose.draft.encrypt
                    onToggled: autosave.restart()
                }
                Label {
                    visible: compose.protection.missing.length > 0
                    Layout.fillWidth: true
                    text: compose.sioul.textWith("ui-no-key-for", "addresses", compose.protection.missing.join(", "))
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: encrypt.checked ? compose.theme.warm : compose.theme.muted
                    font.pixelSize: 13
                }
                Button {
                    visible: compose.protection.missing.length > 0
                    flat: true
                    text: compose.sioul.text("ui-look-for-keys")
                    onClicked: {
                        compose.save()
                        compose.sioul.pgpLookup(compose.draftId)
                    }
                }
            }
            // The security key that signs for this address has expired: when, and how to renew it,
            // said once you ask to sign, not in every message.
            Label {
                visible: sign.checked && compose.expired
                Layout.fillWidth: true
                text: compose.protection.expired || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: compose.theme.warm
                font.pixelSize: 13
            }

            // Your security key, asked at Send.
            SecurityKeyBand {
                id: keyBand

                Layout.fillWidth: true
                sioul: compose.sioul
                theme: compose.theme
                context: compose.draftId
                purpose: "sign"
                onGo: pin => compose.sioul.signAndSend(compose.draftId, pin)
                onUnsigned: {
                    sign.checked = false
                    compose.send()
                }
                // Signed: the main window says "Sending…" with "Undo".
                onDone: {
                    compose.leaving = true
                    compose.close()
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Button {
                    text: compose.sioul.text("ui-attach")
                    icon.name: "mail-attachment"
                    icon.color: compose.theme.text
                    onClicked: picker.open()
                }
                // A paper of the wallet, attached: an identity card, the last tax notice, a rent receipt.
                Button {
                    id: paperButton

                    property var papers: []

                    text: compose.sioul.text("papers-attach")
                    onClicked: {
                        const shown = JSON.parse(compose.sioul.papers())
                        paperButton.papers = shown.families.flatMap(f => f.papers).filter(p => p.file_there)
                        paperMenu.popup(paperButton, 0, paperButton.height)
                    }

                    SioulMenu {
                        id: paperMenu

                        MenuItem {
                            visible: paperButton.papers.length === 0
                            height: visible ? implicitHeight : 0
                            enabled: false
                            text: compose.sioul.text("papers-none-to-attach")
                        }
                        // One list of lines: a Repeater's lines land out of order in a
                        // menu once their model changes, and it changes at each click.
                        Instantiator {
                            model: paperButton.papers

                            delegate: MenuItem {
                                id: paperLine

                                required property var modelData

                                // An old one says so: "less than three months old" is often asked.
                                // "&" marks a shortcut in a menu: "&&" is one.
                                text: (paperLine.modelData.standing === "old" || paperLine.modelData.standing === "ended" ? paperLine.modelData.title + " — " + paperLine.modelData.line : paperLine.modelData.title).replace(/&/g, "&&")
                                // Its path made an address as the file dialog's are: "#", "%" in a
                                // name stay in it, and Windows' "C:\…" is read back whole.
                                onTriggered: compose.attach([compose.theme.fileUrl(paperLine.modelData.file)])
                            }
                            onObjectAdded: (index, object) => paperMenu.insertItem(index + 1, object)
                            onObjectRemoved: (index, object) => paperMenu.removeItem(object)
                        }
                    }
                }
                Button {
                    flat: true
                    text: compose.previewing ? compose.sioul.text("ui-edit") : compose.sioul.text("ui-preview")
                    onClicked: compose.previewing = !compose.previewing
                }
                Label {
                    Layout.fillWidth: true
                    // Files still coming are said until they are there, unless something stops sending.
                    text: compose.attaching !== "" && !compose.problem ? compose.attaching : (compose.note !== "" ? compose.note : compose.sioul.text("ui-markdown-hint"))
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: compose.problem ? compose.theme.warm : compose.theme.muted
                    font.pixelSize: 13
                }
                Button {
                    flat: true
                    text: compose.sioul.text("ui-discard")
                    onClicked: compose.discard()
                }
                Button {
                    text: compose.sioul.text("ui-send")
                    icon.name: "mail-send"
                    icon.color: compose.theme.accentText
                    highlighted: true
                    enabled: keyBand.said.state !== "working" && keyBand.said.state !== "touch"
                    onClicked: compose.send()
                }
            }
        }
    }
}
