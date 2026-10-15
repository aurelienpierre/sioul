// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Microsoft's mail (Outlook.com, Hotmail, Live, MSN; Microsoft 365 for work
// or school) in the mail form: Microsoft takes no password from mail
// programs, app passwords included, so this form asks none. "Sign in with
// Microsoft" opens Microsoft's page in the system's browser, and Sioul keeps
// the access in the system keyring. A personal address is told of
// Outlook.com's IMAP switch, off until turned on; a work or school one, that
// its organisation may have to approve Sioul once, with what its
// administrator does, to copy and forward. An application of your own: its
// id, and its directory when it is registered for one organisation. When
// only the mailbox of another login than the address opened (the one
// Microsoft's page signed in), the form asks before anything is kept. Made
// the first time an address turns out to be Microsoft's (AccountsPage.qml).
// docs/microsoft.md.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: form

    required property var sioul
    required property var theme
    // The address, and what was found for it (backend's FoundView).
    property string address: ""
    property var found: null
    // An account signed in again: its id; empty when adding one.
    property string againId: ""
    property bool ownApp: false
    property bool stepsShown: false
    property bool adminShown: false
    // "What your administrator does" unfolded from the start (the page's captures).
    property bool unfoldAdmin: false
    readonly property bool appKept: form.found !== null && form.found.microsoft_key === true
    readonly property bool builtIn: form.sioul.microsoftBuiltIn()
    // "personal", "work" or "unknown", as discovery found it.
    readonly property string kind: form.found && form.found.microsoft_kind ? form.found.microsoft_kind : "unknown"
    readonly property bool phone: Qt.platform.os === "android"
    // An application's id is asked: none built in, or yours chosen, and none kept here.
    readonly property bool asksApp: !form.appKept && (!form.builtIn || form.ownApp)
    // Microsoft's page signed in another login, and only its mailbox opened: the question, until answered.
    property string question: ""
    // The same question put in from the page (its captures).
    property string asked: ""

    // Something was asked of the backend: the page shows its answer under this form.
    signal started

    // A new address: its fields emptied.
    function reset() {
        form.ownApp = false
        form.stepsShown = false
        form.adminShown = form.unfoldAdmin
        clientId.text = ""
        tenantId.text = ""
    }

    Component.onCompleted: form.reset()
    // Another address found: its fields emptied.
    onFoundChanged: form.reset()
    onUnfoldAdminChanged: form.adminShown = form.unfoldAdmin
    onAskedChanged: form.question = form.asked

    Connections {
        target: form.sioul

        function onMicrosoftQuestion(question) {
            form.question = question
        }

        // The sign-in ended (Cancel, five minutes): its question with it.
        function onFormBusyChanged() {
            if (!form.sioul.formBusy)
                form.question = ""
        }
    }

    spacing: 10

    Label {
        Layout.fillWidth: true
        text: form.appKept ? form.sioul.text("ui-microsoft-app-kept") : form.sioul.text("ui-microsoft-sign-in-note")
        wrapMode: Text.Wrap
        color: form.theme.text
    }
    // What Sioul does with the mail, and its terms: within reach, as Microsoft's terms ask.
    Label {
        Layout.fillWidth: true
        text: form.sioul.text("ui-microsoft-privacy")
        textFormat: Text.MarkdownText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: form.theme.muted
        onLinkActivated: link => Qt.openUrlExternally(link)
    }

    // Outlook.com keeps IMAP off until you turn it on.
    ColumnLayout {
        visible: form.kind !== "work"
        Layout.fillWidth: true
        spacing: 6

        Label {
            Layout.fillWidth: true
            text: form.sioul.text("ui-microsoft-imap-switch")
            wrapMode: Text.Wrap
            color: form.theme.text
        }
        Button {
            text: form.sioul.text("ui-microsoft-imap-how")
            icon.name: "internet-web-browser"
            icon.color: form.theme.text
            onClicked: Qt.openUrlExternally("https://support.microsoft.com/en-us/office/pop-imap-and-smtp-settings-for-outlook-com-d088b986-291d-42b8-9564-9c414e2aa040")
        }
    }

    // A work or school address: its organisation may have to approve Sioul once.
    ColumnLayout {
        visible: form.kind !== "personal"
        Layout.fillWidth: true
        spacing: 6

        Label {
            Layout.fillWidth: true
            text: form.sioul.text("ui-microsoft-work-note")
            wrapMode: Text.Wrap
            color: form.theme.text
        }
        // Its sentence wraps: on one line, it was wider than a phone.
        Button {
            id: adminButton

            Layout.fillWidth: true
            flat: true
            text: (form.adminShown ? "▾  " : "▸  ") + form.sioul.text("ui-microsoft-admin")
            contentItem: Label {
                text: adminButton.text
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: form.theme.text
            }
            onClicked: form.adminShown = !form.adminShown
        }
        // Read-only, to select and copy: its link is long, and goes to the administrator.
        TextArea {
            id: adminText

            visible: form.adminShown && adminText.text !== ""
            Layout.fillWidth: true
            readOnly: true
            selectByMouse: true
            wrapMode: TextArea.WrapAtWordBoundaryOrAnywhere
            textFormat: TextEdit.PlainText
            // For an id typed here, asked now (no keyring); else as discovery found it.
            text: !form.adminShown ? "" : form.asksApp ? form.sioul.microsoftAdminText(form.address, clientId.text) : (form.found && form.found.microsoft_admin ? form.found.microsoft_admin : "")
            font.pixelSize: 13
            color: form.theme.text
            background: Rectangle {
                color: form.theme.surface
                radius: form.theme.radius
                border.color: adminText.activeFocus ? form.theme.focus : form.theme.line
            }
        }
        Button {
            visible: form.adminShown && adminText.text !== ""
            flat: true
            icon.name: "edit-copy"
            icon.color: form.theme.text
            text: copied.running ? form.sioul.text("ui-copied") : form.sioul.text("ui-copy")
            onClicked: {
                adminText.selectAll()
                adminText.copy()
                adminText.deselect()
                copied.restart()
            }

            Timer {
                id: copied

                interval: 2000
            }
        }
    }

    // Sioul's own application in this build, or yours.
    WrapCheckBox {
        visible: !form.appKept && form.builtIn
        Layout.fillWidth: true
        text: form.sioul.text("ui-microsoft-own-app")
        checked: form.ownApp
        onToggled: form.ownApp = checked
    }
    Label {
        visible: !form.appKept && !form.builtIn
        Layout.fillWidth: true
        text: form.sioul.text("ui-microsoft-no-app")
        wrapMode: Text.Wrap
        color: form.theme.text
    }
    Button {
        id: stepsButton

        visible: form.asksApp
        Layout.fillWidth: true
        flat: true
        text: (form.stepsShown ? "▾  " : "▸  ") + form.sioul.text("ui-microsoft-steps")
        contentItem: Label {
            text: stepsButton.text
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: form.theme.text
        }
        onClicked: form.stepsShown = !form.stepsShown
    }
    Label {
        visible: form.stepsShown && form.asksApp
        Layout.fillWidth: true
        text: form.sioul.text("ui-microsoft-steps-text")
        textFormat: Text.MarkdownText
        wrapMode: Text.Wrap
        color: form.theme.text
        onLinkActivated: link => Qt.openUrlExternally(link)
    }
    GridLayout {
        visible: form.asksApp
        Layout.fillWidth: true
        columns: 2
        columnSpacing: form.theme.gap
        rowSpacing: 8

        Label {
            text: form.sioul.text("ui-microsoft-client-id")
            color: form.theme.muted
        }
        TextField {
            id: clientId

            Layout.fillWidth: true
            placeholderText: "00000000-0000-0000-0000-000000000000"
            inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
        }
        Label {
            text: form.sioul.text("ui-microsoft-tenant-id")
            color: form.theme.muted
        }
        TextField {
            id: tenantId

            Layout.fillWidth: true
            placeholderText: form.sioul.text("ui-microsoft-tenant-optional")
            inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
        }
    }
    RowLayout {
        spacing: 8

        Button {
            text: form.sioul.text("ui-microsoft-sign-in")
            enabled: !form.sioul.formBusy && form.found !== null && (form.appKept || !form.asksApp || clientId.text.trim().length > 0)
            onClicked: {
                form.started()
                form.sioul.addMicrosoftMail(form.address, form.kind, form.asksApp ? clientId.text : "", form.asksApp ? tenantId.text : "", form.againId)
            }
        }
        Button {
            visible: form.sioul.formBusy
            flat: true
            text: form.sioul.text("ui-cancel")
            onClicked: form.sioul.cancelMicrosoft()
        }
    }
    Label {
        visible: form.sioul.formBusy && form.question === ""
        Layout.fillWidth: true
        text: form.phone ? form.sioul.text("ui-microsoft-waiting-phone") : form.sioul.text("ui-microsoft-waiting")
        wrapMode: Text.Wrap
        color: form.theme.text
    }
    // Kept on your Yes alone: both addresses named (backend's microsoft_question).
    ColumnLayout {
        visible: form.question !== ""
        Layout.fillWidth: true
        spacing: 6

        Label {
            Layout.fillWidth: true
            text: form.question
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: form.theme.text
        }
        Flow {
            Layout.fillWidth: true
            spacing: 8

            Button {
                text: form.sioul.text("ui-microsoft-other-use")
                onClicked: {
                    form.question = ""
                    form.sioul.answerMicrosoft(true)
                }
            }
            Button {
                flat: true
                text: form.sioul.text("ui-microsoft-other-not")
                onClicked: {
                    form.question = ""
                    form.sioul.answerMicrosoft(false)
                }
            }
        }
    }
}
