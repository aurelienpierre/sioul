// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A folder chosen in Sioul itself, on Android: the system's picker refuses
// the phone's storage (Murena's, among others) and gives addresses, not
// paths. The phone's folders are read through "All files access"; one your
// other devices already share through says so.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: browser

    required property var sioul
    required property var theme
    property var shown: ({ path: "", parent: "", folders: [], readable: true })
    property bool access: true

    signal chosen(string path)

    // Opens in `path`, else in the phone's storage.
    function begin(path) {
        browser.access = browser.sioul.filesAccess()
        browser.go(path || "")
        browser.open()
    }

    function go(path) {
        browser.shown = JSON.parse(browser.sioul.foldersIn(path))
        list.positionViewAtBeginning()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * browser.theme.gap)
    height: Math.min(560, (parent ? parent.height : 560) - 2 * browser.theme.gap)
    title: browser.sioul.text("folder-browser-title")

    // Back from Android's switch for "All files access": the folders read again.
    Connections {
        target: Qt.application

        function onStateChanged() {
            if (browser.opened && Qt.application.state === Qt.ApplicationActive) {
                browser.access = browser.sioul.filesAccess()
                browser.go(browser.shown.path)
            }
        }
    }

    contentItem: ColumnLayout {
        spacing: 8

        Label {
            visible: !browser.access
            Layout.fillWidth: true
            text: browser.sioul.text("share-files-access")
            wrapMode: Text.Wrap
            color: browser.theme.text
        }
        Button {
            visible: !browser.access
            highlighted: true
            text: browser.sioul.text("share-files-allow")
            onClicked: browser.sioul.askFilesAccess()
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            ToolButton {
                enabled: browser.shown.parent !== ""
                icon.name: "go-up"
                icon.color: browser.theme.text
                display: AbstractButton.IconOnly
                Accessible.name: browser.sioul.text("folder-browser-up")
                onClicked: browser.go(browser.shown.parent)
            }
            // A folder's name is plain text.
            Label {
                Layout.fillWidth: true
                text: browser.shown.path
                textFormat: Text.PlainText
                elide: Text.ElideLeft
                color: browser.theme.muted
            }
        }
        ListView {
            id: list

            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: browser.shown.folders
            boundsBehavior: Flickable.StopAtBounds

            delegate: ItemDelegate {
                id: row

                required property var modelData

                width: ListView.view.width
                icon.name: "folder"
                icon.color: browser.theme.text
                text: browser.theme.plain(row.modelData.sealed ? row.modelData.name + "  ·  " + browser.sioul.text("folder-browser-shared") : row.modelData.name)
                onClicked: browser.go(row.modelData.path)
            }
        }
        Label {
            visible: list.count === 0
            Layout.fillWidth: true
            text: browser.sioul.text(browser.shown.readable ? "folder-browser-empty" : "folder-browser-unreadable")
            wrapMode: Text.Wrap
            color: browser.theme.muted
        }
    }

    footer: DialogButtonBox {
        Button {
            text: browser.sioul.text("folder-browser-choose")
            highlighted: true
            enabled: browser.shown.path !== "" && browser.shown.readable
            onClicked: {
                browser.close()
                browser.chosen(browser.shown.path)
            }
        }
        Button {
            text: browser.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: browser.close()
    }
}
