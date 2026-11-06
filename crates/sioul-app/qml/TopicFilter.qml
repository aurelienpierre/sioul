// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Mail's filter by topic (docs/topics.md, "Where topics show"), beside the
// filter by area: a button saying which topic shows ("All topics" until one
// is chosen) that unfolds the topics, one tap each. A topic is a way to
// look at the one mailbox, never a place: nothing moves.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: filter

    required property var sioul
    required property var theme
    // The topic shown, by id; "" for all.
    property string chosen: ""
    // The topics (`topics::view`), read again each time the list unfolds.
    property var topics: []

    signal picked(string topic)

    readonly property var chosenTopic: filter.topics.find(t => t.id === filter.chosen) || null

    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight
    Component.onCompleted: filter.topics = JSON.parse(filter.sioul.topics() || "[]")

    // A topic renamed or removed, here or on another device: its name on the button follows.
    Connections {
        target: filter.sioul

        function onTopicsChanged() {
            filter.topics = JSON.parse(filter.sioul.topics() || "[]")
        }
    }

    function unfold() {
        filter.topics = JSON.parse(filter.sioul.topics() || "[]")
        const at = button.mapToItem(list.parent, 0, button.height)
        list.x = Math.max(list.leftMargin, Math.min(at.x, list.parent.width - list.width - list.rightMargin))
        list.y = at.y + 2
        list.open()
    }

    Button {
        id: button

        objectName: "topicFilter"
        anchors.fill: parent
        flat: true
        // Your own words: plain text all the same.
        text: (filter.chosenTopic ? filter.theme.plain(filter.chosenTopic.name) : filter.sioul.text("topic-filter-all")) + "  ▾"
        Accessible.name: filter.chosenTopic ? filter.sioul.textWith("topic-filter-name", "topic", filter.chosenTopic.name) : filter.sioul.text("topic-filter-all")
        onClicked: {
            if (list.opened)
                list.close()
            else if (Date.now() - list.closedAt > 300)
                filter.unfold()
        }
    }

    SafePopup {
        id: list

        property real closedAt: 0

        parent: Overlay.overlay
        modal: false
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        padding: 8
        width: Math.min(Math.max(choices.implicitWidth + 2 * list.padding, 200), (list.parent ? list.roomWidth : 412) - 2 * list.safeGap)
        onClosed: list.closedAt = Date.now()

        background: Rectangle {
            color: filter.theme.surface
            radius: filter.theme.radius
            border.color: filter.theme.line
        }

        contentItem: ColumnLayout {
            id: choices

            spacing: 2

            Button {
                objectName: "topicFilter:all"
                Layout.fillWidth: true
                flat: true
                highlighted: filter.chosen === ""
                text: filter.sioul.text("topic-filter-all")
                onClicked: {
                    filter.picked("")
                    list.close()
                }
            }
            Repeater {
                model: filter.topics

                delegate: Button {
                    id: one

                    required property var modelData

                    objectName: "topicFilter:" + one.modelData.id
                    Layout.fillWidth: true
                    flat: true
                    highlighted: filter.chosen === one.modelData.id
                    contentItem: Label {
                        text: one.modelData.name
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: filter.theme.text
                    }
                    onClicked: {
                        filter.picked(one.modelData.id)
                        list.close()
                    }
                }
            }
        }
    }
}
