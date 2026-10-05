// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contracts and subscriptions, on the Budgets page: what you are bound to,
// each with when it renews and the last day to stop it, in words; the
// recurring payments that have no contract yet, one click from being one;
// stopping one: its own cancel page, or the letter drafted. No offers.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window
    property var shown: ({ store: false, problem: "", kinds: [], contracts: [], suggestions: [], presets: [] })
    property string problem: ""
    property alias dialog: contractDialog

    function reload() {
        section.shown = JSON.parse(section.sioul.contracts())
    }

    // A contract to say, from a mail or a payment.
    function start(prefill) {
        section.reload()
        contractDialog.now().begin(prefill)
    }

    function open(id) {
        section.reload()
        const contract = section.shown.contracts.find(c => c.id === id)
        if (contract)
            contractDialog.now().edit(contract)
    }

    spacing: 8
    Component.onCompleted: section.reload()
    onVisibleChanged: if (visible) section.reload()

    RowLayout {
        Layout.fillWidth: true
        Layout.topMargin: section.theme.gap
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: section.sioul.text("contracts")
            font.pixelSize: 19
            color: section.theme.text
        }
        Button {
            enabled: section.shown.store
            text: section.sioul.text("contracts-add")
            icon.name: "list-add"
            onClicked: section.start({})
        }
    }
    Label {
        visible: section.shown.problem !== "" || section.problem !== ""
        Layout.fillWidth: true
        text: section.problem !== "" ? section.problem : section.shown.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: section.theme.warm
    }
    Label {
        visible: section.shown.store && section.shown.contracts.length === 0 && section.shown.suggestions.length === 0
        Layout.fillWidth: true
        text: section.sioul.text("contracts-none")
        wrapMode: Text.Wrap
        color: section.theme.muted
    }

    Repeater {
        model: section.shown.contracts

        delegate: Rectangle {
            id: row

            required property var modelData
            readonly property bool ended: row.modelData.ended !== ""

            Layout.fillWidth: true
            implicitHeight: rowLayout.implicitHeight + 16
            color: section.theme.surface
            border.color: section.theme.line
            radius: 4
            opacity: row.ended ? 0.6 : 1

            RowLayout {
                id: rowLayout

                anchors.fill: parent
                anchors.margins: 8
                spacing: 10

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        text: row.modelData.party === "" || row.modelData.party === row.modelData.title ? row.modelData.title : row.modelData.title + " · " + row.modelData.party
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: section.theme.text
                    }
                    Label {
                        Layout.fillWidth: true
                        text: [row.modelData.kind_label, row.modelData.cost].filter(t => t !== "").join(" · ")
                        textFormat: Text.PlainText
                        font.pixelSize: 13
                        color: section.theme.muted
                    }
                    Label {
                        Layout.fillWidth: true
                        text: row.modelData.line
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: section.theme.muted
                    }
                    Label {
                        visible: row.modelData.covers !== ""
                        Layout.fillWidth: true
                        text: section.sioul.textWith("contracts-covers-line", "covers", row.modelData.covers)
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: section.theme.muted
                    }
                }
                Button {
                    flat: true
                    text: section.sioul.text("routines-change")
                    onClicked: contractDialog.now().edit(row.modelData)
                }
                Button {
                    visible: !row.ended
                    text: section.sioul.text("contracts-stop")
                    onClicked: stopMenu.now().popup()

                    Later {
                        id: stopMenu

                        sourceComponent: Component {
                            SioulMenu {
                                id: stopMenuForm

                                MenuItem {
                                    visible: row.modelData.cancel_is_page
                                    height: visible ? implicitHeight : 0
                                    text: section.sioul.text("contracts-stop-page")
                                    onTriggered: Qt.openUrlExternally(row.modelData.cancel)
                                }
                                MenuItem {
                                    text: section.sioul.text("contracts-stop-letter")
                                    onTriggered: {
                                        const made = JSON.parse(section.sioul.contractLetter(row.modelData.id))
                                        if (made.draft)
                                            section.window.openDraft(made.draft)
                                        else
                                            section.problem = made.error
                                    }
                                }
                                MenuItem {
                                    text: section.sioul.text("contracts-stop-ended")
                                    onTriggered: {
                                        const edit = Object.assign({}, row.modelData, { ended: Qt.formatDate(new Date(), "yyyy-MM-dd") })
                                        section.problem = section.sioul.saveContract(row.modelData.id, JSON.stringify(edit))
                                        section.reload()
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Recurring payments without a contract: one click from being one.
    Label {
        visible: section.shown.suggestions.length > 0
        Layout.fillWidth: true
        Layout.topMargin: 6
        text: section.sioul.text("contracts-suggestions")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: section.theme.muted
    }
    Repeater {
        model: section.shown.suggestions

        delegate: RowLayout {
            id: suggestion

            required property var modelData

            Layout.fillWidth: true
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: suggestion.modelData.cost === "" ? suggestion.modelData.title : suggestion.modelData.title + " · " + suggestion.modelData.cost
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: section.theme.text
            }
            Button {
                flat: true
                text: section.sioul.text("contracts-note-it")
                onClicked: section.start({ title: suggestion.modelData.title, preset: suggestion.modelData.preset, kind: suggestion.modelData.kind })
            }
        }
    }

    Later {
        id: contractDialog

        sourceComponent: Component {
            ContractDialog {
                id: contractDialogForm

                sioul: section.sioul
                theme: section.theme
                window: section.window
                kinds: section.shown.kinds
                presets: section.shown.presets
                onSaved: section.reload()
            }
        }
    }
}
