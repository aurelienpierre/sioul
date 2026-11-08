// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Topic…" (docs/topics.md, "Your word"): the topics to give one message,
// or several chosen at once, the one they have ticked; "No topic"; "New
// topic…", which makes the topic and gives it in one act; for one message,
// "Always <topic> for this sender" and "Never…"; "Rename or remove…", the
// topics themselves changed in the same place (8 November 2026), each name a
// field, Enter renaming it, Remove beside it. Each act is Rust's
// (crates/sioul-app/src/topics.rs), said in the status line with Undo.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafePopup {
    id: menu

    required property var sioul
    required property var theme
    // The messages: a JSON list of keys (a mailbox row's keys as a list within it).
    property string keys: "[]"
    // One message alone: its key, for its sender's rules; "" for several.
    property string key: ""
    // The topic it has now, by id; "" for none or several.
    property string current: ""
    // The topics, as Rust lists them (`topics::view`): [{id, name, seeds, senders}].
    property var topics: []
    // Why "New topic…" did not make one, else "".
    property string problem: ""
    property bool naming: false
    // Renaming or removing topics instead of giving one.
    property bool editing: false

    // Opened under `item` (a button), or in the window's middle without one; the topics read again.
    function openAt(item, keys, key, current) {
        menu.keys = keys
        menu.key = key || ""
        menu.current = current || ""
        menu.topics = JSON.parse(menu.sioul.topics() || "[]")
        menu.problem = ""
        menu.naming = false
        menu.editing = false
        nameField.text = ""
        if (item) {
            const at = item.mapToItem(menu.parent, 0, item.height)
            menu.x = Math.max(menu.leftMargin, Math.min(at.x, menu.parent.width - menu.width - menu.rightMargin))
            menu.y = at.y + 2
        } else {
            menu.x = Math.max(menu.leftMargin, (menu.parent.width - menu.width) / 2)
            menu.y = Math.max(menu.topMargin, menu.parent.height / 3)
        }
        menu.open()
    }

    function give(id) {
        menu.sioul.tagMessages(menu.keys, id)
        menu.close()
    }

    function make() {
        const name = nameField.text.trim()
        if (name === "")
            return
        menu.problem = menu.sioul.newTopic(menu.keys, name)
        if (menu.problem === "")
            menu.close()
    }

    // A topic renamed from its field: what went wrong said, the topics read again.
    function rename(id, name) {
        const topic = menu.topics.find(t => t.id === id)
        if (!topic || name.trim() === "" || name.trim() === topic.name)
            return
        menu.problem = menu.sioul.renameTopic(id, name.trim())
        menu.topics = JSON.parse(menu.sioul.topics() || "[]")
    }

    // A topic removed (its word leaves every message; Undo in the status line puts it back).
    function remove(id) {
        menu.sioul.removeTopic(id)
        menu.problem = ""
        if (menu.current === id)
            menu.current = ""
        menu.topics = JSON.parse(menu.sioul.topics() || "[]")
        if (menu.topics.length === 0)
            menu.editing = false
    }

    readonly property var currentTopic: menu.topics.find(t => t.id === menu.current) || null

    parent: Overlay.overlay
    modal: false
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    padding: 8
    // Never past the window less its margins (a phone's 412 points).
    width: Math.min(Math.max(column.implicitWidth + 2 * menu.padding, 240), (menu.parent ? menu.roomWidth : 412) - 2 * menu.safeGap)

    background: Rectangle {
        color: menu.theme.surface
        radius: menu.theme.radius
        border.color: menu.theme.line
    }

    contentItem: ColumnLayout {
        id: column

        spacing: 2

        Repeater {
            model: menu.editing ? [] : menu.topics

            delegate: Button {
                id: choice

                required property var modelData

                objectName: "topicChoice:" + choice.modelData.id
                Layout.fillWidth: true
                flat: true
                checkable: true
                checked: choice.modelData.id === menu.current
                // Your own words: plain text all the same.
                text: menu.theme.plain(choice.modelData.name)
                contentItem: Label {
                    text: (choice.checked ? "✓  " : "    ") + choice.text
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: menu.theme.text
                }
                onClicked: menu.give(choice.modelData.id)
            }
        }
        // Renaming or removing: each topic's name a field, Enter renames it; Remove beside it.
        Repeater {
            model: menu.editing ? menu.topics : []

            delegate: RowLayout {
                id: edited

                required property var modelData

                Layout.fillWidth: true
                spacing: 6

                TextField {
                    objectName: "topicRename:" + edited.modelData.id
                    Layout.fillWidth: true
                    Layout.minimumWidth: 140
                    text: edited.modelData.name
                    selectByMouse: true
                    Accessible.name: menu.sioul.text("topics-rename")
                    onAccepted: menu.rename(edited.modelData.id, text)
                    onEditingFinished: menu.rename(edited.modelData.id, text)
                }
                Button {
                    objectName: "topicRemove:" + edited.modelData.id
                    flat: true
                    text: menu.sioul.text("topics-remove")
                    onClicked: menu.remove(edited.modelData.id)
                }
            }
        }
        Button {
            objectName: "topicEditDone"
            visible: menu.editing
            Layout.fillWidth: true
            flat: true
            text: menu.sioul.text("topic-edit-done")
            onClicked: {
                menu.editing = false
                menu.problem = ""
            }
        }
        Button {
            objectName: "topicNone"
            visible: !menu.editing && (menu.current !== "" || menu.key === "")
            Layout.fillWidth: true
            flat: true
            text: menu.sioul.text("topic-none")
            onClicked: menu.give("")
        }
        Rectangle {
            visible: menu.topics.length > 0 && !menu.editing
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: menu.theme.line
        }
        Button {
            objectName: "topicNew"
            visible: !menu.naming && !menu.editing
            Layout.fillWidth: true
            flat: true
            text: menu.sioul.text("topic-new")
            onClicked: {
                menu.naming = true
                nameField.forceActiveFocus()
            }
        }
        RowLayout {
            visible: menu.naming && !menu.editing
            Layout.fillWidth: true
            spacing: 6

            TextField {
                id: nameField

                objectName: "topicNewName"
                Layout.fillWidth: true
                placeholderText: menu.sioul.text("topic-new-name")
                onAccepted: menu.make()
            }
            Button {
                objectName: "topicMake"
                enabled: nameField.text.trim() !== ""
                text: menu.sioul.text("topic-make")
                onClicked: menu.make()
            }
        }
        Label {
            visible: menu.problem !== ""
            Layout.fillWidth: true
            Layout.preferredWidth: 0
            text: menu.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: menu.theme.warm
        }
        // The topics themselves, renamed or removed in the same place.
        Button {
            objectName: "topicEdit"
            visible: !menu.editing && menu.topics.length > 0
            Layout.fillWidth: true
            flat: true
            text: menu.sioul.text("topic-edit")
            onClicked: {
                menu.naming = false
                menu.problem = ""
                menu.editing = true
            }
        }
        // One message with a topic: its sender's rules for it.
        Rectangle {
            visible: !menu.editing && menu.key !== "" && menu.currentTopic !== null
            Layout.fillWidth: true
            Layout.preferredHeight: 1
            color: menu.theme.line
        }
        Button {
            objectName: "topicAlways"
            visible: !menu.editing && menu.key !== "" && menu.currentTopic !== null
            Layout.fillWidth: true
            flat: true
            text: menu.currentTopic ? menu.theme.plain(menu.sioul.textWith("topic-always", "topic", menu.currentTopic.name)) : ""
            onClicked: {
                menu.sioul.senderTopic(menu.key, menu.current, "always")
                menu.close()
            }
        }
        Button {
            objectName: "topicNever"
            visible: !menu.editing && menu.key !== "" && menu.currentTopic !== null
            Layout.fillWidth: true
            flat: true
            text: menu.currentTopic ? menu.theme.plain(menu.sioul.textWith("topic-never", "topic", menu.currentTopic.name)) : ""
            onClicked: {
                menu.sioul.senderTopic(menu.key, menu.current, "never")
                menu.close()
            }
        }
    }
}
