// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sharing with your other computers, on the Parameters page: what travels and
// how, the folder a sync carries, a passphrase typed once on each computer
// (twice on the first), then where things stand.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: panel

    required property var sioul
    required property var theme

    property var status: ({ on: false, folder: "", sealed: false, lines: [], problems: [] })
    property string problem: ""
    // Android: no folder dialog (it hands out content:// addresses, not paths);
    // the path typed, or one of the folders your other devices share through.
    readonly property bool android: Qt.platform.os === "android"
    property bool filesAccess: true
    property var candidates: []

    function reload() {
        panel.filesAccess = panel.sioul.filesAccess()
        panel.status = JSON.parse(panel.sioul.shareStatus(folderField.text))
        if (!folderField.activeFocus)
            folderField.text = panel.status.folder
        panel.candidates = panel.status.on || !panel.filesAccess ? [] : JSON.parse(panel.sioul.shareCandidates() || "[]").filter(c => c !== folderField.text)
    }

    // A folder typed or picked: what it holds is said at once.
    function choose(folder) {
        folderField.text = folder
        panel.reload()
    }

    spacing: 6
    onVisibleChanged: if (visible) panel.reload()
    Component.onCompleted: panel.reload()

    // While shown: news of the other computers as it comes.
    Timer {
        interval: 20000
        running: panel.visible
        repeat: true
        onTriggered: panel.reload()
    }

    Label {
        Layout.topMargin: 18
        Layout.fillWidth: true
        text: panel.sioul.text("share-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: panel.theme.accent
    }
    Label {
        Layout.fillWidth: true
        text: panel.sioul.text("share-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.muted
    }
    Repeater {
        model: panel.status.lines

        // Where things stand, your other computers' names among it: plain text.
        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: panel.theme.text
        }
    }
    Repeater {
        model: panel.status.problems

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: panel.theme.warm
        }
    }
    // Android: the folder your sync app carries is read by its path, once Android allows it.
    Label {
        visible: panel.android && !panel.filesAccess
        Layout.fillWidth: true
        text: panel.sioul.text("share-files-access")
        wrapMode: Text.Wrap
        color: panel.theme.text
    }
    Button {
        visible: panel.android && !panel.filesAccess
        text: panel.sioul.text("share-files-allow")
        onClicked: panel.sioul.askFilesAccess()
    }
    // The folders your other devices already share through: one tap.
    Label {
        visible: panel.candidates.length > 0
        Layout.fillWidth: true
        text: panel.sioul.text("share-found")
        wrapMode: Text.Wrap
        color: panel.theme.muted
    }
    Repeater {
        model: panel.candidates

        delegate: Button {
            required property string modelData

            Layout.fillWidth: true
            flat: true
            text: modelData
            onClicked: panel.choose(modelData)
        }
    }
    // Not shared yet: the folder and the passphrase.
    GridLayout {
        visible: !panel.status.on
        Layout.fillWidth: true
        columns: 3
        columnSpacing: 8
        rowSpacing: 6

        Label {
            text: panel.sioul.text("share-folder")
            color: panel.theme.muted
        }
        TextField {
            id: folderField

            Layout.fillWidth: true
            placeholderText: panel.android ? "/storage/emulated/0/Documents/Sioul" : "~/Nextcloud/Sioul"
            onEditingFinished: panel.reload()
        }
        Button {
            text: panel.sioul.text("share-choose")
            onClicked: panel.android ? panel.browse() : folderPicker.open()
        }
        Label {
            text: panel.sioul.text("share-passphrase")
            color: panel.theme.muted
        }
        PasswordField {
            id: passphrase

            Layout.columnSpan: 2
            Layout.fillWidth: true
            sioul: panel.sioul
        }
        Label {
            visible: !panel.status.sealed
            text: panel.sioul.text("share-again")
            color: panel.theme.muted
        }
        PasswordField {
            id: again

            visible: !panel.status.sealed
            Layout.columnSpan: 2
            Layout.fillWidth: true
            sioul: panel.sioul
        }
        Label {
            Layout.columnSpan: 3
            Layout.fillWidth: true
            text: panel.sioul.text(panel.status.sealed ? "share-passphrase-known" : "share-passphrase-hint")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: panel.theme.muted
        }
    }
    Label {
        visible: panel.problem !== ""
        Layout.fillWidth: true
        text: panel.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: panel.theme.warm
    }
    RowLayout {
        spacing: 8

        Button {
            visible: !panel.status.on
            highlighted: true
            text: panel.sioul.text("share-start")
            onClicked: {
                panel.problem = panel.sioul.startSharing(folderField.text, passphrase.text, again.text)
                if (panel.problem === "") {
                    passphrase.clear()
                    again.clear()
                }
                panel.reload()
            }
        }
        Button {
            visible: panel.status.on
            text: panel.sioul.text("share-now")
            onClicked: {
                panel.sioul.shareNow()
                panel.reload()
            }
        }
        Button {
            visible: panel.status.on
            flat: true
            text: panel.sioul.text("share-stop")
            onClicked: {
                panel.problem = panel.sioul.stopSharing()
                panel.reload()
            }
        }
    }
    Label {
        Layout.fillWidth: true
        text: panel.sioul.text("share-not-shared")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.muted
    }

    // On Android, Sioul's own browser (FolderBrowser.qml): the system's picker
    // refuses the phone's storage. It opens in Documents, where Murena's eDrive
    // keeps what it syncs.
    function browse() {
        if (browser.item === null)
            browser.setSource("FolderBrowser.qml", { sioul: panel.sioul, theme: panel.theme })
        browser.item.begin(folderField.text !== "" ? folderField.text : "/storage/emulated/0/Documents")
    }

    Loader {
        id: browser
    }
    Connections {
        target: browser.item

        function onChosen(path) {
            panel.choose(path)
        }
    }

    FolderDialog {
        id: folderPicker

        // On Windows the address is file:///C:/…: its path is C:/…, not /C:/…;
        // on Android, a folder of the phone's storage (Theme.localPath).
        onAccepted: {
            const path = panel.theme.localPath(folderPicker.selectedFolder)
            if (path === "")
                panel.problem = panel.sioul.text("folder-not-on-device")
            else
                panel.choose(path)
        }
    }
}
