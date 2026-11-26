// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A credentials sign-in form, framed entirely by Sioul (docs/extensions.md,
// "A credentials sign-in"): no text comes from the extension itself; the
// signed manifest's words alone show beside Sioul's, the extension's name and,
// where its publisher gave them, a field's label and a line of help under it
// (Planity's "iCal private link" and where Planity Pro shows it), each as
// plain text, chosen in Rust (`cred_label`, `Offer::fields`); else every
// field's label is Sioul's own fixed word for that kind. The password is masked.
// A plain dialog in Sioul's process on every device, with no web view. One
// sign-in at a time. Sign in hands the values back once
// (`extensionCredentials`); Cancel, and closing the form, hand nothing back;
// the backend clears the form (hide) when the wait ends or the extension is
// removed.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: form

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme

    // What the backend asked: {id, name, fields:[{kind, label, help, masked}], french}.
    property var asked: null
    // The values typed, by field kind, gathered as each field changes.
    property var typed: ({})
    // Handed back once: the values on Sign in, or "" when cancelled; the
    // backend's clearing (hide) hands nothing back.
    property bool answered: false

    // Shown with what the backend asked.
    function show(what) {
        form.typed = {}
        form.asked = what
        form.answered = false
        form.open()
    }
    // Closed because the wait ended or the extension was removed: nothing handed back.
    function hide() {
        form.answered = true
        form.close()
    }
    // A field's value as it changes, kept by its kind until Sign in.
    function record(kind, value) {
        form.typed[kind] = value
    }
    // Sign in: the values handed back once, the form closed. A phone's keyboard
    // may still hold the word being typed (its suggestion underlined, not yet in
    // the field's text): it is committed first, and each field's text read as it
    // is now, in the fields' order, not only as its last change said.
    function sendIn() {
        if (form.answered)
            return
        Qt.inputMethod.commit()
        const values = {}
        for (let i = 0; i < repeater.count; ++i) {
            const field = repeater.itemAt(i)
            if (field)
                values[field.modelData.kind] = field.value()
        }
        form.answered = true
        form.sioul.extensionCredentials(form.asked.id, JSON.stringify(values))
        form.close()
    }
    // Cancel, or a close that is not a deliberate Sign in: nothing handed back.
    function cancel() {
        if (form.answered)
            return
        form.answered = true
        form.sioul.extensionCredentials(form.asked ? form.asked.id : "", "")
    }

    objectName: "extensionSignInForm"
    title: form.asked ? form.theme.plain(form.sioul.textWith("extension-sign-in-title", "name", form.asked.name)) : ""
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    // Only Sign in, Cancel, or the backend's clearing closes it: a credential
    // is not lost by a tap outside.
    closePolicy: Popup.NoAutoClose
    width: Math.min(420, form.roomWidth - 2 * form.theme.gap)
    onRejected: form.cancel()
    onClosed: form.cancel()

    contentItem: ColumnLayout {
        spacing: 10

        Repeater {
            id: repeater

            model: form.asked ? form.asked.fields : []

            delegate: ColumnLayout {
                id: field

                required property var modelData

                // The field's text as it is now (`sendIn`); what its last change said, before it loads.
                function value() {
                    return input.item ? String(input.item.text) : (form.typed[field.modelData.kind] || "")
                }

                Layout.fillWidth: true
                spacing: 2

                Label {
                    objectName: "extensionCredLabel:" + field.modelData.kind
                    Layout.fillWidth: true
                    text: field.modelData.label
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: form.theme.text
                }
                // Where the service shows what to type, in its publisher's words (the signed manifest's).
                Label {
                    objectName: "extensionCredHelp:" + field.modelData.kind
                    Layout.fillWidth: true
                    visible: text !== ""
                    text: field.modelData.help || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: form.theme.muted
                }
                // A masked field for a password (PasswordField, with its
                // show/hide eye), a plain one otherwise. Its kind names it, and
                // each change is kept by the form.
                Loader {
                    id: input

                    objectName: "extensionCred:" + field.modelData.kind
                    Layout.fillWidth: true
                    sourceComponent: field.modelData.masked ? masked : plain
                }
                Component {
                    id: masked

                    PasswordField {
                        sioul: form.sioul
                        onTextChanged: form.record(field.modelData.kind, text)
                        Component.onCompleted: form.record(field.modelData.kind, text)
                    }
                }
                Component {
                    id: plain

                    // A phone's keyboard for an email or a link; no correction in any field.
                    TextField {
                        inputMethodHints: field.modelData.kind === "email" ? (Qt.ImhEmailCharactersOnly | Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText) : field.modelData.kind === "web-address" ? (Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText) : Qt.ImhNoPredictiveText
                        onTextChanged: form.record(field.modelData.kind, text)
                        Component.onCompleted: form.record(field.modelData.kind, text)
                    }
                }
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            objectName: "extensionSignInDo"
            text: form.sioul.text("extension-sign-in-now")
            highlighted: true
            onClicked: form.sendIn()
        }
        Button {
            objectName: "extensionSignInCancel"
            text: form.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
    }
}
