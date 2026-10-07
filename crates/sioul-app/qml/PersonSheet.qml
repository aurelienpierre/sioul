// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// How someone reaches you (« Comment Camille vous joint »; docs/attention.md),
// from a contact's card or a message's sender: their list and why ("Safe, as
// the category Friends says"), the list chosen for them, Always through, and
// what each channel does with them at each time, in sentences. Blocked and
// Always through exclude each other: choosing one takes them off the other,
// and the sheet says so. No times of their own: a person has a list and
// Always through, the rest is the lists' (What reaches you ▸ By person).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: sheet

    required property var sioul
    required property var theme
    // A card's file or UID, or "" for a sender known by their addresses alone.
    property string key: ""
    // A sender's addresses, a JSON array; "" for a card.
    property string addresses: ""
    // reaches.rs's `person`: {title, name, who, said, choice, choices, own, always, always_help, lines, note}.
    property var shown: ({ title: "", name: "", who: "", said: "", choice: "", choices: [], own: [], always: false, always_help: "", lines: [], note: "" })

    // The sheet of someone: a card ("" addresses), or a sender's addresses ("" key).
    function ask(key, addresses) {
        sheet.key = key || ""
        sheet.addresses = addresses || ""
        sheet.reload()
        sheet.open()
    }

    function reload() {
        sheet.shown = JSON.parse(sheet.sioul.personSheet(sheet.key, sheet.addresses) || "null") || sheet.shown
    }

    function change(verb, value) {
        const answer = JSON.parse(sheet.sioul.personSheetChange(sheet.key, sheet.addresses, verb, value) || "null")
        if (answer && answer.sheet)
            sheet.shown = answer.sheet
        sheet.changed()
    }

    // Their list or Always through changed: the page that opened the sheet reads them again.
    signal changed

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(520, (parent ? parent.width : 520) - 2 * sheet.theme.gap)
    height: Math.min(implicitHeight, (parent ? parent.height : 640) - 2 * sheet.theme.gap)
    title: sheet.shown.title
    objectName: "personSheet"

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: sheet.shown.said
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: sheet.theme.text
            }
            // Their list: as their categories say, or one of the four.
            GridLayout {
                Layout.fillWidth: true
                columns: sheet.width < 380 ? 1 : 2
                columnSpacing: 8
                rowSpacing: 2

                Label {
                    text: sheet.sioul.text("person-standing")
                    color: sheet.theme.muted
                }
                ComboBox {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    model: (sheet.shown.choices || []).map(c => c.label)
                    currentIndex: Math.max(0, (sheet.shown.choices || []).findIndex(c => c.value === sheet.shown.choice))
                    Accessible.name: sheet.sioul.text("person-standing")
                    onActivated: index => sheet.change("list", sheet.shown.choices[index].value)
                }
            }
            Repeater {
                model: sheet.shown.own || []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: sheet.theme.muted
                }
            }
            // Always through: on or off; blocked, turning it on unblocks them, said.
            WrapCheckBox {
                id: always

                Layout.fillWidth: true
                Layout.topMargin: 4
                text: sheet.sioul.text("attention-sheet-always")
                checked: sheet.shown.always === true
                onToggled: {
                    sheet.change("always", always.checked ? "on" : "off")
                    always.checked = Qt.binding(() => sheet.shown.always === true)
                }
            }
            Label {
                Layout.fillWidth: true
                text: sheet.shown.always_help
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: sheet.theme.muted
            }
            // What each channel does with them, at each time.
            Label {
                Layout.fillWidth: true
                Layout.topMargin: 6
                text: sheet.sioul.text("attention-sheet-channels")
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                color: sheet.theme.accent
            }
            Repeater {
                model: sheet.shown.lines || []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    lineHeight: 1.2
                    color: sheet.theme.text
                }
            }
            Label {
                visible: sheet.shown.note !== ""
                Layout.fillWidth: true
                text: sheet.shown.note
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: sheet.theme.accent
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            text: sheet.sioul.text("ui-close")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: sheet.close()
    }
}
