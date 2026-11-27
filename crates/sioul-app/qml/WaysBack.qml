// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The ways back to what Sioul keeps on this device (docs/store.md, "Ways
// back"): a passphrase, a recovery code, the person's security key, each
// with what it protects against, each an option, none needed. With none
// kept, what a lost keyring entry would cost, said plainly. Shown once at the
// first sealing (StoreGateWindow.qml) and on the card "What Sioul keeps here"
// (StoreCard.qml); `sioul` is the screen's object or the backend, which both
// answer `storeWays`, `keepPassphraseWay`, `makeRecoveryCode`,
// `keepCardWay`, `forgetWay` and `text`.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: ways

    required property var sioul
    required property var theme
    // What is kept, read again after each change (`storeWays`).
    property var kept: ({ ways: [], sharing: false })
    // A recovery code just made: shown once, here.
    property string code: ""
    // What the last press said ("" when done quietly).
    property string said: ""

    function reread() {
        ways.kept = JSON.parse(ways.sioul.storeWays() || "{}")
    }
    function has(kind) {
        return (ways.kept.ways || []).indexOf(kind) >= 0
    }

    spacing: 10
    Component.onCompleted: ways.reread()

    Label {
        Layout.fillWidth: true
        text: ways.sioul.text("store-ways-help")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        lineHeight: 1.25
        color: ways.theme.text
    }
    // None kept: what a lost keyring entry would cost, plainly.
    Label {
        objectName: "storeWaysNone"
        visible: (ways.kept.ways || []).length === 0
        Layout.fillWidth: true
        text: ways.sioul.text(ways.kept.sharing ? "store-ways-none" : "store-ways-none-alone")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        lineHeight: 1.25
        color: ways.theme.warm
    }

    // A passphrase.
    Label {
        Layout.fillWidth: true
        Layout.topMargin: 6
        text: ways.sioul.text("store-way-passphrase") + (ways.has("passphrase") ? " · " + ways.sioul.text("store-way-kept") : "")
        font.bold: true
        color: ways.theme.text
    }
    Label {
        Layout.fillWidth: true
        text: ways.sioul.text("store-way-passphrase-help")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: ways.theme.text
    }
    // Which one it is: a passphrase way stays the one it was kept with.
    Label {
        objectName: "storeWayPassphraseKept"
        visible: (ways.kept.passphrase || "") !== ""
        Layout.fillWidth: true
        text: ways.kept.passphrase || ""
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: ways.theme.text
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 6

        TextField {
            id: passphrase

            objectName: "storeWayPassphrase"
            Layout.fillWidth: true
            echoMode: TextInput.Password
            placeholderText: ways.sioul.text("store-way-passphrase")
        }
        Button {
            objectName: "storeWayPassphraseKeep"
            enabled: passphrase.text !== ""
            text: ways.sioul.text("store-way-keep")
            onClicked: {
                ways.said = ways.sioul.keepPassphraseWay(passphrase.text, sharing.checked)
                if (ways.said === "")
                    passphrase.text = ""
                ways.reread()
            }
        }
    }
    CheckBox {
        id: sharing

        visible: ways.kept.sharing === true
        text: ways.sioul.text("store-way-passphrase-sharing")
    }

    // A recovery code.
    Label {
        Layout.fillWidth: true
        Layout.topMargin: 6
        text: ways.sioul.text("store-way-code") + (ways.has("code") ? " · " + ways.sioul.text("store-way-kept") : "")
        font.bold: true
        color: ways.theme.text
    }
    Label {
        Layout.fillWidth: true
        text: ways.sioul.text("store-way-code-help")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: ways.theme.text
    }
    Button {
        objectName: "storeWayCodeMake"
        text: ways.sioul.text("store-way-code-make")
        onClicked: {
            ways.code = ways.sioul.makeRecoveryCode()
            ways.reread()
        }
    }
    Label {
        visible: ways.code !== ""
        Layout.fillWidth: true
        text: ways.sioul.text("store-way-code-shown")
        wrapMode: Text.WordWrap
        color: ways.theme.warm
    }
    TextEdit {
        objectName: "storeWayCode"
        visible: ways.code !== ""
        Layout.fillWidth: true
        readOnly: true
        selectByMouse: true
        text: ways.code
        font.family: "monospace"
        font.pixelSize: 18
        wrapMode: TextEdit.WrapAnywhere
        color: ways.theme.text
    }

    // A security key.
    Label {
        Layout.fillWidth: true
        Layout.topMargin: 6
        text: ways.sioul.text("store-way-card") + (ways.has("card") ? " · " + ways.sioul.text("store-way-kept") : "")
        font.bold: true
        color: ways.theme.text
    }
    Label {
        Layout.fillWidth: true
        text: ways.sioul.text("store-way-card-help")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: ways.theme.text
    }
    // Reset, or given a new encryption key since: its way opens nothing now.
    Label {
        objectName: "storeWayCardStale"
        visible: (ways.kept.stale || []).length > 0
        Layout.fillWidth: true
        text: ways.sioul.text("store-way-card-stale")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: ways.theme.warm
    }
    Button {
        objectName: "storeWayCardKeep"
        text: ways.sioul.text("store-way-keep")
        onClicked: {
            ways.said = ways.sioul.keepCardWay()
            ways.reread()
        }
    }

    // Each kept: taken out again, on its own.
    Flow {
        visible: (ways.kept.ways || []).length > 0
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: ways.kept.ways || []

            Button {
                required property var modelData

                text: ways.sioul.text("store-way-forget") + " · " + ways.sioul.text("store-way-" + modelData)
                onClicked: {
                    ways.said = ways.sioul.forgetWay(modelData)
                    ways.reread()
                }
            }
        }
    }
    Label {
        objectName: "storeWaysSaid"
        visible: ways.said !== ""
        Layout.fillWidth: true
        text: ways.said
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: ways.theme.text
    }
}
