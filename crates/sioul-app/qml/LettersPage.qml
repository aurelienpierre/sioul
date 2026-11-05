// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Letters page (docs/porch.md, "Paper letters"): your paper post, as
// scans read on a computer. It opens on the letters waiting, each a card
// (LettersSection.qml), then those filed, by year, the newest first. Its bar
// adds a scan (copied into the folder scans arrive in), links scans where
// they are (never copied: a linked scan that moved or went says so) and
// opens that folder; its ⚙ holds where scans arrive, and what installs the
// reading programs when this computer lacks them. A phone reads none: it
// shows the cards a computer read, without adding or linking.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var shown: ({ inbox: "", letters: [], filed: [], projects: [], missing: "", reader: "" })
    // On a computer, the place and its bar: the folder, adding, linking, the settings.
    readonly property bool place: Qt.platform.os !== "android"
    // The letter to bring into view once read (`openLetter`), by its id.
    property string wanted: ""
    property alias section: section

    function reload() {
        page.shown = JSON.parse(page.sioul.letters())
        if (page.wanted !== "")
            Qt.callLater(page.reveal)
    }

    // A scan chosen here, copied into the folder (the New menu's "A paper letter (scan)…").
    function addScan() {
        section.addScan()
    }

    // Scans kept elsewhere, linked where they are.
    function linkScans() {
        section.linkScans()
    }

    function openSettings() {
        settingsButton.show(true)
    }

    // For the window's images: the settings panel, floating over the page.
    function grabSettings(path) {
        settingsButton.grab(path)
    }

    // A letter brought into view: its card, or its row among those filed.
    function openLetter(id) {
        page.wanted = id
        page.reload()
    }

    function reveal() {
        const id = page.wanted
        page.wanted = ""
        const found = page.find(column, id)
        if (found !== null)
            scroll.contentItem.contentY = Math.max(0, Math.min(found.mapToItem(column, 0, 0).y, column.height - scroll.availableHeight))
    }

    // The first item under `item` showing the letter `id` (its `letterId`).
    function find(item, id) {
        if (item === null)
            return null
        if (item.letterId !== undefined && item.letterId === id)
            return item
        for (let i = 0; i < item.children.length; i++) {
            const found = page.find(item.children[i], id)
            if (found !== null)
                return found
        }
        return null
    }

    onVisibleChanged: if (visible) page.reload()
    Component.onCompleted: page.reload()

    // A scan read, linked or filed, here or by another device: shown at once.
    Connections {
        target: page.sioul

        function onLettersChanged() {
            if (page.visible)
                page.reload()
        }

        // Whom each letter is for, and the persons to choose.
        function onPersonsChanged() {
            if (page.visible)
                page.reload()
        }
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        anchors.margins: page.theme.gap
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: column

            width: Math.min(scroll.availableWidth, 760)
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("ui-letters")
                    font.pixelSize: 22
                    color: page.theme.text
                }
                SettingsButton {
                    id: settingsButton

                    visible: page.place
                    sioul: page.sioul
                    theme: page.theme
                    view: "letters"
                }
            }
            // Where scans go: one quiet line; no count, nothing that nags.
            Label {
                visible: page.place
                Layout.fillWidth: true
                text: page.shown.inbox !== "" ? page.sioul.textWith("letters-place", "folder", page.shown.inbox) : page.sioul.text("letters-place-unset")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            // The bar: a scan added, scans linked, the folder opened; without a folder, where to choose one.
            Flow {
                visible: page.place
                Layout.fillWidth: true
                spacing: 6

                Button {
                    objectName: "lettersAdd"
                    text: page.sioul.text("letters-add-scan")
                    icon.name: "list-add"
                    onClicked: page.addScan()
                }
                Button {
                    objectName: "lettersLink"
                    text: page.sioul.text("letters-link")
                    icon.name: "insert-link"
                    onClicked: page.linkScans()
                }
                Button {
                    visible: page.shown.inbox !== ""
                    flat: true
                    text: page.sioul.text("letters-open-folder")
                    icon.name: "document-open"
                    onClicked: {
                        const folder = page.sioul.lettersFolder()
                        if (folder !== "")
                            Qt.openUrlExternally(page.theme.fileUrl(folder))
                    }
                }
                Button {
                    visible: page.shown.inbox === ""
                    flat: true
                    text: page.sioul.text("letters-choose-folder")
                    onClicked: page.openSettings()
                }
            }
            // Scans waiting, unread: what installs the reader, its command to copy.
            CommandText {
                visible: page.place && page.shown.missing !== ""
                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                text: page.sioul.textWith("letters-no-ocr", "hint", page.shown.missing)
                color: page.theme.warm
            }
            Label {
                visible: page.shown.letters.length === 0 && (page.shown.filed || []).length === 0
                Layout.fillWidth: true
                Layout.topMargin: 12
                text: page.sioul.text("letters-none")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            // The letters waiting: their cards.
            LettersSection {
                id: section

                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                window: page.window
                shown: page.shown
                onChanged: page.reload()
                onSettingsWanted: page.openSettings()
            }

            // Then those filed, by year, the newest first: who, what, when, the scan one click away.
            Label {
                visible: (page.shown.filed || []).length > 0
                Layout.fillWidth: true
                Layout.topMargin: 12
                text: page.sioul.text("letters-filed")
                font.pixelSize: 17
                font.weight: Font.DemiBold
                color: page.theme.text
            }
            Repeater {
                model: page.shown.filed || []

                delegate: ColumnLayout {
                    id: year

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        Layout.topMargin: 6
                        text: year.modelData.year
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        color: page.theme.muted
                    }
                    Repeater {
                        model: year.modelData.letters

                        delegate: ColumnLayout {
                            id: filed

                            required property var modelData
                            readonly property string letterId: filed.modelData.id

                            Layout.fillWidth: true
                            spacing: 2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                Icon {
                                    iconName: "mail-mark-unread"
                                }
                                // What the scan says (its letterhead) is plain text, as wide as the row lets it.
                                Label {
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 0
                                    text: filed.modelData.sender + " · " + filed.modelData.kind_label
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                                Label {
                                    text: filed.modelData.received
                                    textFormat: Text.PlainText
                                    font.pixelSize: 12
                                    color: page.theme.muted
                                }
                                Button {
                                    visible: filed.modelData.gone !== true
                                    flat: true
                                    text: page.sioul.text("letters-scan")
                                    onClicked: Qt.openUrlExternally(page.theme.fileUrl(filed.modelData.file))
                                }
                            }
                            Label {
                                visible: filed.modelData.gone === true || filed.modelData.linked === true
                                Layout.fillWidth: true
                                text: filed.modelData.gone === true ? page.sioul.textWith("letters-gone", "file", filed.modelData.file) : page.sioul.textWith("letters-linked-here", "file", filed.modelData.file)
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                        }
                    }
                }
            }
        }
    }
}
