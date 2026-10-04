// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Sites page on Android. Qt WebEngine, which keeps sites inside Sioul on
// a computer (qml/SitesPage.qml), has no Android version: your sites are
// listed here, and each opens in the browser. Same name and same calls as
// the computer's page, as far as the window uses them (open).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var sites: []

    function reload() {
        page.sites = JSON.parse(page.sioul.siteList() || "[]")
    }

    // A site asked for, by a link or from the Porch: in the browser.
    function open(id) {
        const site = page.sites.find(s => s.id === id)
        if (!site)
            return
        page.sioul.siteSeen(id)
        Qt.openUrlExternally(site.url)
    }

    Component.onCompleted: page.reload()
    onVisibleChanged: if (visible) page.reload()

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        Label {
            Layout.fillWidth: true
            text: page.sioul.text("ui-sites")
            font.pixelSize: 20
            color: page.theme.text
        }
        Label {
            Layout.fillWidth: true
            text: page.sioul.text("site-android")
            wrapMode: Text.Wrap
            color: page.theme.muted
        }
        Label {
            visible: page.sites.length === 0
            Layout.fillWidth: true
            text: page.sioul.text("site-none")
            wrapMode: Text.Wrap
            color: page.theme.muted
        }
        ListView {
            id: list

            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: page.sites

            delegate: ItemDelegate {
                id: row

                required property var modelData

                width: list.width
                // A site's name is yours or a preset's: plain text in a list line.
                text: page.theme.plain(row.modelData.name || row.modelData.url)
                onClicked: page.open(row.modelData.id)
            }
        }
    }
}
