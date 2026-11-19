// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One site in the Sites page's list (SitesList.qml), on a computer and on
// Android alike: its own icon (its type's until fetched), its name, a quiet
// mark when silenced, a dot when it has news (never a number). Folded to
// its icon on a computer's narrow column. On Android, where it opens: in
// Sioul, or in the browser and why (`opens_line`, siteview.rs). A tap
// chooses it; its menu comes at a right click, or at a long press on a
// touch screen.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ItemDelegate {
    id: row

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The site, as `sioul.siteList()` gives it.
    property var site: ({})
    // The column folded to the sites' icons.
    property bool narrow: false
    // A site for other hours, shown unfolded: quieter.
    property bool later: false
    // Its type's icon, until its own is fetched (sioul_core::sites::TYPES).
    readonly property var kindIcons: ({ "mailbox": "mail-folder-inbox", "chat": "im-user", "video": "camera-video", "social": "system-users", "dating": "emblem-favorite" })
    // Where it opens, on Android: "" on a computer, where every site opens here.
    readonly property string where: !row.narrow ? (row.site.opens_line || "") : ""

    // Chosen: the page opens it.
    signal chosen()
    // Its menu asked for, where it is.
    signal menuAsked()

    opacity: row.later ? 0.6 : 1
    padding: row.narrow ? 4 : row.theme.touch ? 10 : 6
    onClicked: row.chosen()

    // On a touch screen, the menu comes at a long press, where the finger is; letting go then opens nothing.
    onPressAndHold: {
        row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
        row.menuAsked()
    }
    TapHandler {
        acceptedButtons: Qt.RightButton
        // A touch has no buttons: on a touch screen, the row's long press.
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: row.menuAsked()
    }
    ToolTip.visible: row.narrow && row.hovered
    ToolTip.text: row.theme.plain(row.site.name || "")
    ToolTip.delay: 400
    Accessible.name: row.site.name || ""

    background: Rectangle {
        color: row.highlighted || row.hovered ? row.theme.surface : "transparent"
        radius: row.theme.radius
        border.color: row.visualFocus ? row.theme.focus : row.highlighted ? row.theme.line : "transparent"
    }

    contentItem: RowLayout {
        spacing: 6

        Item {
            Layout.preferredWidth: 18
            Layout.preferredHeight: 18
            Layout.alignment: row.where !== "" ? Qt.AlignTop : Qt.AlignVCenter

            // Its own icon, kept on this device; its type's until then, and when it cannot be drawn.
            Image {
                id: favicon

                visible: favicon.status === Image.Ready
                anchors.centerIn: parent
                width: 16
                height: 16
                source: row.site.icon || ""
                sourceSize: Qt.size(32, 32)
                fillMode: Image.PreserveAspectFit
                smooth: true
                asynchronous: true
            }
            Icon {
                visible: favicon.status !== Image.Ready
                anchors.centerIn: parent
                iconName: row.kindIcons[row.site.kind] || "internet-web-browser"
                size: 16
            }
            // Something notified, not seen yet: a dot, never a number.
            Rectangle {
                visible: row.site.news === true && row.narrow
                anchors.right: parent.right
                anchors.top: parent.top
                width: 7
                height: 7
                radius: 3.5
                color: row.theme.accent
            }
        }
        ColumnLayout {
            visible: !row.narrow
            Layout.fillWidth: true
            spacing: 0

            Label {
                Layout.fillWidth: true
                // A site without a name shows its address, as Android's page did (review C2).
                text: row.site.name || row.site.url || ""
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: row.theme.text
            }
            // Android: in Sioul, or in the browser and why.
            Label {
                visible: row.where !== ""
                Layout.fillWidth: true
                text: row.where
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: row.theme.muted
            }
        }
        // Quiet, not red as Breeze draws it.
        Icon {
            visible: row.site.muted === true && !row.narrow
            iconName: "audio-volume-muted"
            color: row.theme.muted
            size: 14
        }
        Rectangle {
            visible: row.site.news === true && !row.narrow
            Layout.preferredWidth: 7
            Layout.preferredHeight: 7
            radius: 3.5
            color: row.theme.accent
        }
    }
}
