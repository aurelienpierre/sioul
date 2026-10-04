// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Which of the vault's logins to fill in a site: the site's own, the one
// chosen last for it first; or any login of the vault, searched by its
// name, user name or site. Names only here: a password leaves the vault for
// the one login chosen. A login made for another site says which.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: chooser

    required property var sioul
    required property var theme
    property string url: ""
    property var shown: ({ site: "", matches: [], found: [] })
    // For the window's tests: the words searched.
    property alias query: search.text
    readonly property bool searching: search.text.trim() !== ""
    readonly property var choices: chooser.searching ? chooser.shown.found : chooser.shown.matches
    // What taking a login does: "Fill" in a site, "Use" for an account's password.
    property string takeText: chooser.sioul.text("bitwarden-choose-fill")

    signal chosen(string item)

    // `query`, when given, is searched at once: an account's address.
    function begin(url, query) {
        chooser.url = url
        search.text = query || ""
        chooser.reload()
        chooser.open()
        search.forceActiveFocus()
    }

    function reload() {
        const shown = JSON.parse(chooser.sioul.bitwardenLogins(chooser.url, search.text))
        chooser.shown = shown.error ? { site: "", matches: [], found: [] } : shown
        list.currentIndex = 0
    }

    function take(index) {
        const choice = chooser.choices[index]
        if (!choice)
            return
        chooser.close()
        chooser.chosen(choice.id)
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * chooser.theme.gap)
    title: chooser.sioul.text("bitwarden-choose-title")

    contentItem: ColumnLayout {
        spacing: 8

        TextField {
            id: search

            Layout.fillWidth: true
            placeholderText: chooser.sioul.text("bitwarden-choose-search")
            inputMethodHints: Qt.ImhNoPredictiveText
            onTextChanged: chooser.reload()
            onAccepted: chooser.take(list.currentIndex)
            Keys.onDownPressed: list.currentIndex = Math.min(list.count - 1, list.currentIndex + 1)
            Keys.onUpPressed: list.currentIndex = Math.max(0, list.currentIndex - 1)
        }
        Label {
            Layout.fillWidth: true
            text: chooser.searching
                  ? chooser.sioul.text(chooser.choices.length > 0 ? "bitwarden-choose-found" : "bitwarden-choose-nothing")
                  : chooser.sioul.textWith(chooser.choices.length > 0 ? "bitwarden-choose-site" : "bitwarden-choose-none", "site", chooser.shown.site)
            wrapMode: Text.Wrap
            color: chooser.theme.muted
        }
        ListView {
            id: list

            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(list.contentHeight, 320)
            clip: true
            model: chooser.choices
            boundsBehavior: Flickable.StopAtBounds

            delegate: ItemDelegate {
                id: row

                required property var modelData
                required property int index

                width: ListView.view.width
                highlighted: ListView.isCurrentItem
                Accessible.name: row.modelData.name
                onClicked: chooser.take(row.index)

                contentItem: ColumnLayout {
                    spacing: 2

                    // A vault's items may be shared with you by others: plain text.
                    Label {
                        Layout.fillWidth: true
                        text: row.modelData.name
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.weight: Font.DemiBold
                        color: chooser.theme.text
                    }
                    Label {
                        visible: text !== ""
                        Layout.fillWidth: true
                        // Made for another domain: which, so a look-alike site shows.
                        text: [row.modelData.username, row.modelData.elsewhere && row.modelData.site !== "" ? chooser.sioul.textWith("bitwarden-choose-other-site", "site", row.modelData.site) : ""].filter(t => t !== "").join(" · ")
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: chooser.theme.muted
                    }
                }
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            text: chooser.takeText
            highlighted: true
            enabled: chooser.choices.length > 0
            onClicked: chooser.take(list.currentIndex)
        }
        Button {
            text: chooser.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: chooser.close()
    }
}
