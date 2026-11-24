// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A preset, a rule that comes with Sioul (Settings ▸ Rules ▸ Presets;
// docs/rules.md, "Presets"): what Sioul does by itself today, in the same
// sentence as a rule. Its card: the sentence; its switch and its details
// where they are settings today (a list, a number of days, a level), each
// drawn and saved as that setting is everywhere (SettingRow.qml): one truth,
// the setting itself, never a copy; a line when its details cannot be
// changed yet (it moves into the rules' engine in a later version); "Copy
// to make my own" when the engine can already run what it says, which
// opens the editor on a rule of your own made from it. Made the first time
// it opens (RulesTab.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: card

    objectName: "presetCard"

    required property var sioul
    required property var theme

    // The preset's line as the list gives it: {id, sentence, preset: {offered, …}, settings: [rows]}.
    property var line: null
    property string problem: ""

    // A setting changed: the list reads its presets again.
    signal changed
    // "Copy to make my own": a rule of yours from it.
    signal copied(string id)

    function show(line) {
        card.line = line
        card.problem = ""
        card.open()
    }

    function save(key, value, shown) {
        card.problem = shown === undefined ? card.sioul.setSetting(key, JSON.stringify(value)) : card.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
        card.changed()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, (parent ? card.roomWidth : 560) - 2 * card.theme.gap)
    title: card.sioul.text("rules-preset-title")

    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(column.implicitHeight, (card.parent ? card.roomHeight : 700) - 170)
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: card.theme.sectionGap

            Label {
                objectName: "presetSentence"
                Layout.fillWidth: true
                text: card.line ? card.line.sentence : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 16
                lineHeight: 1.2
                color: card.theme.text
            }
            Label {
                visible: card.problem !== ""
                Layout.fillWidth: true
                text: card.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: card.theme.warm
            }

            // Its switch and its details, as the settings they are today.
            Section {
                objectName: "presetSettings"
                visible: card.line !== null && (card.line.settings || []).length > 0
                Layout.fillWidth: true
                theme: card.theme
                title: card.sioul.text("rules-preset-details")
                body.spacing: card.theme.rowGap

                Repeater {
                    model: card.line ? card.line.settings || [] : []

                    delegate: SettingRow {
                        required property var modelData

                        Layout.fillWidth: true
                        setting: modelData
                        sioul: card.sioul
                        theme: card.theme
                        onSave: (key, value, shown) => card.save(key, value, shown)
                    }
                }
            }

            // Not in the rules' engine yet: said once, plainly.
            Label {
                objectName: "presetLater"
                visible: card.line !== null && card.line.preset && !card.line.preset.offered
                Layout.fillWidth: true
                text: card.sioul.text("rules-preset-later")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: card.theme.muted
            }
        }
    }

    footer: DialogFoot {
        sioul: card.sioul
        theme: card.theme
        cancelText: card.sioul.text("ui-close")
        mainText: card.line && card.line.preset && card.line.preset.offered ? card.sioul.text("rules-preset-copy") : ""
        onAccepted: {
            card.close()
            card.copied(card.line.id)
        }
        onRejected: card.close()
    }
}
