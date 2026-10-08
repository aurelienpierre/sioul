// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Link to…": finds anything, by a few words of its title, of one kind or
// all, and ties it to the thing open. What is already tied says so.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: picker

    required property var sioul
    required property var theme
    property var source: null
    property string kind: ""
    property var found: []
    readonly property var kinds: ["", "task", "event", "mail", "note", "contact", "budget", "project", "site"]

    // `kind`: what to look among first ("task", "event", "project"…), else all.
    function show(source, kind) {
        picker.source = source
        picker.kind = kind || ""
        query.text = ""
        picker.search()
        picker.open()
        query.forceActiveFocus()
    }

    function search() {
        picker.found = picker.source ? JSON.parse(picker.sioul.searchThings(query.text, picker.kind, picker.source.uri)) : []
    }

    // For the window's tests: as if typed, and its picture (the dialog floats outside the page's).
    function type(text) {
        query.text = text
        picker.search()
    }

    function grab(path) {
        picker.contentItem.grabToImage(result => result.saveToFile(path))
    }

    function chooseFirst() {
        if (picker.found.length > 0)
            picker.choose(picker.found[0])
    }

    function choose(item) {
        if (item.linked || picker.source === null)
            return
        picker.sioul.linkThings(picker.source.uri, item.uri)
        picker.close()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(580, (parent ? parent.width : 580) - 2 * picker.theme.gap)
    height: Math.min(660, (parent ? parent.height : 660) - 2 * picker.theme.gap)
    // The title may be a message's subject: plain text, a "<" opens no tag.
    title: picker.source ? picker.theme.plain(picker.sioul.textWith("link-title", "title", picker.source.title || "")) : ""

    Timer {
        id: searching

        interval: 250
        onTriggered: picker.search()
    }

    contentItem: ColumnLayout {
        spacing: 8

        TextField {
            id: query

            Layout.fillWidth: true
            placeholderText: picker.sioul.text("link-search")
            onTextEdited: searching.restart()
            Keys.onDownPressed: results.forceActiveFocus()
            onAccepted: {
                if (picker.found.length > 0)
                    picker.choose(picker.found[0])
            }
        }
        // One kind, or all.
        Flow {
            Layout.fillWidth: true
            spacing: 4

            Repeater {
                model: picker.kinds

                delegate: Button {
                    id: chip

                    required property string modelData

                    flat: picker.kind !== chip.modelData
                    highlighted: picker.kind === chip.modelData
                    padding: 6
                    text: picker.sioul.text(chip.modelData === "" ? "link-kind-all" : "link-kind-" + chip.modelData)
                    font.pixelSize: 13
                    onClicked: {
                        picker.kind = chip.modelData
                        picker.search()
                    }
                }
            }
        }
        ListView {
            id: results

            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            model: picker.found
            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: thing

                required property var modelData

                width: results.width - 12
                padding: 6
                Accessible.name: thing.modelData.title
                enabled: !thing.modelData.linked
                onClicked: picker.choose(thing.modelData)
                Keys.onReturnPressed: picker.choose(thing.modelData)

                contentItem: RowLayout {
                    spacing: 8

                    Icon {
                        iconName: picker.theme.kindIcons[thing.modelData.kind] || "insert-link"
                        size: 16
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: thing.modelData.title
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: picker.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: [picker.sioul.text("link-kind-" + thing.modelData.kind), thing.modelData.detail, thing.modelData.when].filter(t => t !== "").join("  ·  ")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            color: picker.theme.muted
                        }
                    }
                    Label {
                        visible: thing.modelData.linked
                        text: picker.sioul.text("link-already")
                        font.pixelSize: 12
                        color: picker.theme.accent
                    }
                }
            }
        }
        Label {
            visible: picker.found.length === 0
            Layout.fillWidth: true
            text: picker.sioul.text("link-nothing")
            wrapMode: Text.Wrap
            color: picker.theme.muted
        }
    }

    // Sioul's own button, in your language: Qt's standard ones are not translated.
    footer: DialogButtonBox {
        Button {
            text: picker.sioul.text("ui-close")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: picker.close()
    }
}
