// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "What Sioul keeps here" (docs/store.md, "Ways back"), in Settings ▸ Your
// folder and sharing: whether what Sioul keeps on this device is sealed, how
// many earlier versions of each file it keeps (n), and the ways back to it
// (WaysBack.qml), each an option. On a profile in clear (a phone, no
// keyring), it says so and offers nothing.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: card

    required property var sioul
    required property var theme
    readonly property var kept: typeof card.sioul.storeWays === "function" ? JSON.parse(card.sioul.storeWays() || "{}") : ({})

    spacing: 8

    Label {
        Layout.fillWidth: true
        Layout.topMargin: 12
        text: card.sioul.text("store-card-title")
        font.bold: true
        color: card.theme.text
    }
    Label {
        objectName: "storeCardState"
        Layout.fillWidth: true
        text: card.sioul.text(card.kept.sealed ? "store-card-sealed" : "store-card-plain")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: card.theme.text
    }
    // Sioul's lists still in the notes folder: in clear there until they move to Sioul's own folder.
    Label {
        objectName: "storeCardListsClear"
        visible: card.kept.lists_clear === true
        Layout.fillWidth: true
        text: card.sioul.text("store-lists-clear")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        color: card.theme.warm
    }
    RowLayout {
        visible: card.kept.sealed === true
        Layout.fillWidth: true
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: card.sioul.text("store-versions")
            wrapMode: Text.WordWrap
            color: card.theme.text
        }
        SpinBox {
            objectName: "storeCardVersions"
            from: 0
            to: 20
            value: card.kept.versions === undefined ? 2 : card.kept.versions
            onValueModified: card.sioul.setVersionsKept(value)
        }
    }
    Label {
        visible: card.kept.sealed === true
        Layout.fillWidth: true
        text: card.sioul.text("store-versions-help")
        textFormat: Text.PlainText
        wrapMode: Text.WordWrap
        font.pixelSize: 13
        color: card.theme.muted
    }
    Loader {
        active: card.kept.sealed === true
        visible: active
        Layout.fillWidth: true

        sourceComponent: WaysBack {
            sioul: card.sioul
            theme: card.theme
        }
    }
}
