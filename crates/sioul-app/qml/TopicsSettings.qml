// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Mail ▸ ⚙ ▸ Topics (docs/topics.md): the topics, each with its seeds in
// words and its senders' rules (× takes one off), Rename and Remove; "Add a
// topic"; and on a computer the groups Sioul noticed in the mail no word of
// yours names yet (NMF on their words: crates/sioul-learn/src/groups.rs),
// each with its words, its size and two subjects, and "Name it", which makes
// a topic and gives it to the whole group in one act. Every act is said with
// Undo; nothing here is counted, nothing asks.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: page

    required property var sioul
    required property var theme
    // What it is, in a sentence: the setting's own, said first.
    property string about: ""
    // The topics (`topics::view`): [{id, name, seeds, senders: [[address, "always" | "never"]]}].
    property var topics: []
    // The groups found (`topicGroups`): {state, groups: [{size, senders, names, subjects, proposed, proposed_by}], detail}.
    readonly property var groups: page.sioul.topicGroups ? JSON.parse(page.sioul.topicGroups) : null
    property string problem: ""
    // The topic being renamed, by id; "" for none.
    property string renaming: ""
    // The group being named, by its place; -1 for none.
    property int naming: -1
    // The groups are looked for on a computer: a phone never reads all the mail for them.
    readonly property bool computer: Qt.platform.os !== "android"

    function read() {
        page.topics = JSON.parse(page.sioul.topics() || "[]")
        page.offered = JSON.parse(page.sioul.topicPresets() || "[]")
    }

    spacing: 8
    Component.onCompleted: page.read()
    onVisibleChanged: if (visible) page.read()

    // An act said in the status line (with Undo) has changed the list: read it again.
    Connections {
        target: page.sioul

        function onUndoLineChanged() {
            page.read()
        }
    }

    Label {
        visible: page.about !== ""
        Layout.fillWidth: true
        Layout.preferredWidth: 0
        text: page.about
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.25
        color: page.theme.muted
    }
    Label {
        visible: page.topics.length === 0
        Layout.fillWidth: true
        Layout.preferredWidth: 0
        text: page.sioul.text("topics-settings-none")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: page.theme.muted
    }

    Repeater {
        model: page.topics

        delegate: ColumnLayout {
            id: topic

            required property var modelData

            objectName: "topicRow:" + topic.modelData.id
            Layout.fillWidth: true
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Label {
                    visible: page.renaming !== topic.modelData.id
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    text: topic.modelData.name
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.weight: Font.DemiBold
                    color: page.theme.text
                }
                TextField {
                    id: newName

                    visible: page.renaming === topic.modelData.id
                    Layout.fillWidth: true
                    text: topic.modelData.name
                    onAccepted: {
                        page.problem = page.sioul.renameTopic(topic.modelData.id, newName.text)
                        if (page.problem === "")
                            page.renaming = ""
                    }
                }
                Button {
                    flat: true
                    text: page.sioul.text("topics-rename")
                    onClicked: {
                        if (page.renaming === topic.modelData.id) {
                            page.problem = page.sioul.renameTopic(topic.modelData.id, newName.text)
                            if (page.problem === "")
                                page.renaming = ""
                        } else {
                            page.renaming = topic.modelData.id
                            newName.forceActiveFocus()
                        }
                    }
                }
                Button {
                    objectName: "topicRemove:" + topic.modelData.id
                    flat: true
                    text: page.sioul.text("topics-remove")
                    onClicked: page.sioul.removeTopic(topic.modelData.id)
                }
            }
            Label {
                visible: topic.modelData.seeds !== ""
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: topic.modelData.seeds
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            Repeater {
                model: topic.modelData.senders

                delegate: RowLayout {
                    id: rule

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: page.sioul.textWith(rule.modelData[1] === "always" ? "topics-sender-always" : "topics-sender-never", "address", rule.modelData[0])
                        textFormat: Text.PlainText
                        elide: Text.ElideMiddle
                        font.pixelSize: 13
                        color: page.theme.muted
                    }
                    ToolButton {
                        text: "×"
                        Accessible.name: page.sioul.text("topics-sender-off")
                        ToolTip.visible: hovered
                        ToolTip.text: page.sioul.text("topics-sender-off")
                        onClicked: page.sioul.forgetSenderTopic(rule.modelData[0], topic.modelData.id)
                    }
                }
            }
        }
    }

    // The topics Sioul knows, not started yet (`presets/topics.json`): one tap starts one.
    property var offered: []
    Label {
        visible: page.offered.length > 0
        Layout.topMargin: 6
        text: page.sioul.text("topics-offered")
        textFormat: Text.PlainText
        font.pixelSize: 13
        color: page.theme.muted
    }
    Flow {
        visible: page.offered.length > 0
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: page.offered

            delegate: Button {
                id: preset

                required property var modelData

                objectName: "topicPreset:" + preset.modelData.id
                text: preset.modelData.name
                onClicked: {
                    page.problem = page.sioul.startTopicPreset(preset.modelData.id)
                    page.read()
                }
            }
        }
    }
    // A topic from its name alone: messages are given it from the mail.
    RowLayout {
        Layout.fillWidth: true
        spacing: 6

        TextField {
            id: addName

            objectName: "topicAddName"
            Layout.fillWidth: true
            placeholderText: page.sioul.text("topic-new-name")
            onAccepted: addButton.clicked()
        }
        Button {
            id: addButton

            objectName: "topicAdd"
            enabled: addName.text.trim() !== ""
            text: page.sioul.text("topics-add")
            onClicked: {
                page.problem = page.sioul.newTopic("[]", addName.text)
                if (page.problem === "")
                    addName.text = ""
            }
        }
    }
    Label {
        visible: page.problem !== ""
        Layout.fillWidth: true
        Layout.preferredWidth: 0
        text: page.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: page.theme.warm
    }

    // The groups Sioul noticed, on a computer: looked for when you ask.
    Label {
        visible: page.computer
        Layout.topMargin: 8
        text: page.sioul.text("topics-groups")
        font.weight: Font.DemiBold
        color: page.theme.text
    }
    Button {
        objectName: "topicGroupsLook"
        visible: page.computer
        enabled: page.groups === null || page.groups.state !== "looking"
        text: page.groups !== null && page.groups.state === "looking" ? page.sioul.text("topics-groups-looking") : page.sioul.text("topics-groups-look")
        onClicked: page.sioul.findTopicGroups()
    }
    Label {
        visible: page.groups !== null && (page.groups.state === "failed" || (page.groups.state === "done" && page.groups.groups.length === 0))
        Layout.fillWidth: true
        Layout.preferredWidth: 0
        text: page.groups === null ? "" : page.groups.state === "failed" ? page.sioul.textWith("topics-groups-failed", "detail", page.groups.detail || "") : page.sioul.text("topics-groups-none")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: page.theme.muted
    }
    Repeater {
        model: page.groups !== null && page.groups.state === "done" ? page.groups.groups : []

        delegate: ColumnLayout {
            id: group

            required property var modelData
            required property int index

            objectName: "topicGroup:" + group.index
            Layout.fillWidth: true
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                // Its senders, as they name themselves: the mail's words, plain text.
                Label {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    text: group.modelData.names.join(", ")
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                Label {
                    text: page.sioul.textCounted("topics-groups-size", group.modelData.size)
                    textFormat: Text.PlainText
                    font.pixelSize: 13
                    color: page.theme.muted
                }
                Button {
                    objectName: "topicGroupName:" + group.index
                    visible: page.naming !== group.index
                    text: page.sioul.text("topics-name-it")
                    onClicked: {
                        page.naming = group.index
                        groupName.text = group.modelData.proposed || ""
                        groupName.forceActiveFocus()
                        groupName.selectAll()
                    }
                }
            }
            Label {
                visible: group.modelData.subjects.length > 0
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: group.modelData.subjects.join(" · ")
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: 13
                color: page.theme.muted
            }
            // How the name offered was found, in one sentence.
            Label {
                visible: page.naming === group.index && group.modelData.proposed_by !== ""
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: group.modelData.proposed_by === "sites" ? page.sioul.text("topics-named-sites") : page.sioul.text("topics-named-sender")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            RowLayout {
                visible: page.naming === group.index
                Layout.fillWidth: true
                spacing: 6

                TextField {
                    id: groupName

                    objectName: "topicGroupField:" + group.index
                    Layout.fillWidth: true
                    placeholderText: page.sioul.text("topic-new-name")
                    onAccepted: nameIt.clicked()
                }
                Button {
                    id: nameIt

                    objectName: "topicGroupMake:" + group.index
                    enabled: groupName.text.trim() !== ""
                    text: page.sioul.text("topic-make")
                    onClicked: {
                        page.problem = page.sioul.nameTopicGroup(group.index, groupName.text)
                        if (page.problem === "")
                            page.naming = -1
                    }
                }
            }
        }
    }
}
