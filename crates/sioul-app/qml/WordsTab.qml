// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Words (« Mots »; docs/words.md): the words Sioul recognises
// things by. On top, the languages read and the countries whose names are
// read, ticked. Then one line for each thing recognised, in four groups
// (mail; money and papers; the phone; tasks and a public address), each with
// how many words it holds and how many you changed; a line opens its own
// page, on a computer as on a phone (never a fold that pushes the list
// down): what its words do, its lists as word rows (SettingRow.qml: chips
// with ×, a field to add one), "Back to the defaults". The rows come from
// Settings (`settings::words_rows`, section "words"): each line's note
// (`words.about.<line>`, its group's name in `unit`), then its lists
// (`words.<list>`); a change is saved at once, written as its difference
// from the packs.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: tab

    objectName: "wordsTab"

    required property var sioul
    required property var theme
    // Settings' rows of this tab (section "words").
    required property var rows

    // A list changed, or a line taken back (Settings saves it, then reads its rows again).
    // `shown`: the value the row showed (`changeSetting`).
    signal save(string key, var value, var shown)
    // A line opened or closed: the page scrolled back to its top.
    signal toTop

    // The line whose page is open ("codes", "voicemail"…); "" for the list of lines.
    property string open: ""
    readonly property var ticked: tab.rows.filter(r => r.key === "words.languages" || r.key === "words.countries")
    readonly property var notes: tab.rows.filter(r => r.kind === "note" && r.key.startsWith("words.about."))
    readonly property var openNote: tab.notes.find(n => n.key === "words.about." + tab.open) || null
    // The open line's lists: the rows of its group, its note aside.
    readonly property var openRows: tab.openNote === null ? [] : tab.rows.filter(r => r.group === tab.openNote.group && r.kind !== "note")

    // The line holding a list ("words.voicemail.operators"), opened: where another page sends you.
    function show(key) {
        const row = tab.rows.find(r => r.key === key)
        const note = row ? tab.notes.find(n => n.group === row.group) : null
        tab.open = note ? note.key.slice("words.about.".length) : ""
        tab.toTop()
    }

    Layout.fillWidth: true
    spacing: 6

    // ---------------------------------------------------------------- the list of lines

    Repeater {
        model: tab.open === "" ? tab.ticked : []

        delegate: SettingRow {
            required property var modelData

            Layout.fillWidth: true
            Layout.topMargin: 8
            setting: modelData
            sioul: tab.sioul
            theme: tab.theme
            onSave: (key, value, shown) => tab.save(key, value, shown)
        }
    }

    Repeater {
        model: tab.open === "" ? tab.notes : []

        delegate: ColumnLayout {
            id: line

            required property var modelData
            required property int index
            // The first line of its group: the group's name above it.
            readonly property bool newFamily: line.index === 0 || tab.notes[line.index - 1].unit !== line.modelData.unit

            Layout.fillWidth: true
            Layout.topMargin: line.newFamily ? 18 : 0
            spacing: 2

            Label {
                visible: line.newFamily
                Layout.fillWidth: true
                text: line.modelData.unit
                textFormat: Text.PlainText
                font.pixelSize: 17
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                color: tab.theme.accent
            }
            ItemDelegate {
                Layout.fillWidth: true
                Accessible.name: line.modelData.group + ". " + line.modelData.help
                onClicked: {
                    tab.open = line.modelData.key.slice("words.about.".length)
                    tab.toTop()
                }

                contentItem: RowLayout {
                    spacing: 8

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1

                        Label {
                            Layout.fillWidth: true
                            text: line.modelData.group
                            textFormat: Text.PlainText
                            font.weight: Font.DemiBold
                            wrapMode: Text.Wrap
                            color: tab.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: line.modelData.help
                            textFormat: Text.PlainText
                            font.pixelSize: 13
                            wrapMode: Text.Wrap
                            color: tab.theme.muted
                        }
                    }
                    Label {
                        text: "›"
                        font.pixelSize: 20
                        color: tab.theme.muted
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- one line's page

    RowLayout {
        visible: tab.openNote !== null
        Layout.fillWidth: true
        spacing: 4

        ToolButton {
            icon.name: "go-previous"
            icon.color: tab.theme.text
            text: tab.sioul.text("settings-tab-words")
            display: AbstractButton.TextBesideIcon
            Accessible.name: tab.sioul.text("ui-back")
            onClicked: {
                tab.open = ""
                tab.toTop()
            }
        }
    }
    Label {
        visible: tab.openNote !== null
        Layout.fillWidth: true
        text: tab.openNote === null ? "" : tab.openNote.group
        textFormat: Text.PlainText
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: tab.theme.accent
    }
    // What its words do, and what taking one away does.
    Label {
        visible: tab.openNote !== null
        Layout.fillWidth: true
        text: tab.openNote === null ? "" : tab.openNote.label
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        lineHeight: 1.25
        color: tab.theme.text
    }
    Label {
        visible: tab.openNote !== null
        Layout.fillWidth: true
        text: tab.sioul.text("set-words-whole")
        font.pixelSize: 13
        wrapMode: Text.Wrap
        color: tab.theme.muted
    }

    Repeater {
        model: tab.openRows

        delegate: SettingRow {
            required property var modelData

            Layout.fillWidth: true
            Layout.topMargin: 10
            setting: modelData
            sioul: tab.sioul
            theme: tab.theme
            onSave: (key, value, shown) => tab.save(key, value, shown)
        }
    }

    Button {
        visible: tab.openNote !== null
        Layout.topMargin: 14
        Layout.bottomMargin: 24
        text: tab.sioul.text("set-words-reset")
        onClicked: tab.save("words.reset." + tab.open, true)
    }
}
