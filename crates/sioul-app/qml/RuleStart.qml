// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Add a rule…" (Settings ▸ Rules; docs/rules.md, "The view"): three ways
// to start, in one small card. From nothing; from a message of the last 30
// days a rule can see (never what the Porch holds), found off the window's
// thread, then by a few words of its subject or its sender (the same draft
// as its menu's "Make a rule from this…": its sender's proven domain, what
// Sioul recognises in it, its project); from a preset the engine can run
// now, said by its sentence (none yet: one quiet line says it comes later). A choice closes the card and says
// `chosen(kind, id)`; the tab opens the editor on it. Made the first time
// it opens (RulesTab.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: start

    objectName: "ruleStart"

    required property var sioul
    required property var theme

    // {messages: [{key, subject, from, day}], presets: [{id, sentence}]}, found off the window's thread.
    property var offered: ({ messages: [], presets: [] })
    // Still looking for the messages a rule can see.
    property bool looking: false
    // A few words to find a message by.
    property string find: ""
    // The messages the words find, a dozen at most.
    readonly property var found: {
        const words = start.find.trim().toLowerCase()
        const all = start.offered.messages || []
        return (words === "" ? all : all.filter(m => (m.subject + " " + m.from).toLowerCase().includes(words))).slice(0, 12)
    }

    // "nothing" (id ""), "message" (its file), "preset" (its id).
    signal chosen(string kind, string id)

    function ask() {
        start.offered = { messages: [], presets: [] }
        start.looking = true
        start.find = ""
        start.sioul.rulesStartsAsk()
        start.open()
    }

    Connections {
        target: start.sioul

        function onRulesStartsFound(found) {
            start.offered = JSON.parse(found || "null") || { messages: [], presets: [] }
            start.looking = false
        }
    }

    function choose(kind, id) {
        start.close()
        start.chosen(kind, id)
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, (parent ? start.roomWidth : 560) - 2 * start.theme.gap)
    title: start.sioul.text("rules-add-title")

    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(column.implicitHeight, (start.parent ? start.roomHeight : 700) - 170)
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: start.theme.sectionGap

            Button {
                objectName: "ruleStartBlank"
                Layout.alignment: Qt.AlignLeft
                text: start.sioul.text("rules-start-blank")
                onClicked: start.choose("nothing", "")
            }

            Section {
                objectName: "ruleStartMessages"
                Layout.fillWidth: true
                theme: start.theme
                title: start.sioul.text("rules-start-message")
                body.spacing: 2

                TextField {
                    objectName: "ruleStartFind"
                    Layout.fillWidth: true
                    Layout.bottomMargin: start.theme.labelGap
                    placeholderText: start.sioul.text("rules-start-find")
                    Accessible.name: placeholderText
                    onTextEdited: start.find = text
                }
                Label {
                    visible: start.found.length === 0
                    Layout.fillWidth: true
                    text: start.sioul.text(start.looking ? "rules-start-looking" : (start.offered.messages || []).length === 0 ? "rules-start-no-message" : "rules-start-none-found")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: start.theme.muted
                }
                Repeater {
                    model: start.found

                    delegate: ItemDelegate {
                        id: message

                        required property var modelData

                        Layout.fillWidth: true
                        Accessible.name: message.modelData.subject + ", " + message.modelData.from
                        onClicked: start.choose("message", message.modelData.key)

                        contentItem: ColumnLayout {
                            spacing: 1

                            Label {
                                Layout.fillWidth: true
                                text: message.modelData.subject
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: start.theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: message.modelData.from + " · " + message.modelData.day
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: 13
                                color: start.theme.muted
                            }
                        }
                    }
                }
            }

            // No preset the engine runs yet: said once, quietly, where they would be.
            Label {
                objectName: "ruleStartPresetsLater"
                visible: !start.looking && (start.offered.presets || []).length === 0
                Layout.fillWidth: true
                text: start.sioul.text("rules-start-preset-later")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: start.theme.muted
            }

            Section {
                objectName: "ruleStartPresets"
                visible: (start.offered.presets || []).length > 0
                Layout.fillWidth: true
                theme: start.theme
                title: start.sioul.text("rules-start-preset")
                body.spacing: 2

                Repeater {
                    model: start.offered.presets || []

                    delegate: ItemDelegate {
                        id: preset

                        required property var modelData

                        Layout.fillWidth: true
                        Accessible.name: preset.modelData.sentence
                        onClicked: start.choose("preset", preset.modelData.id)

                        contentItem: Label {
                            text: preset.modelData.sentence
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: start.theme.text
                        }
                    }
                }
            }
        }
    }

    footer: DialogFoot {
        sioul: start.sioul
        theme: start.theme
        onRejected: start.close()
    }
}
