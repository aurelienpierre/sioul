// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Where a bank account's movements go (docs/accounting.md, "Bank accounts"):
// a movement whose label holds one of a rule's words goes where the rule
// says, a budget, a reserve (money moved with a savings account), or another
// of your accounts (in no budget). The first rule that holds wins; each is
// saved as soon as it is changed.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    property var budgets: []
    property var reserves: []
    property var accounts: []
    // The bank account's card, from the bank view.
    property var account: null
    property string problem: ""
    readonly property var directions: ["", "debit", "credit"]
    // Where a movement can go, for every rule.
    readonly property var places: dialog.budgets.map(b => ({ id: "budget:" + b.id, title: b.title }))
        .concat(dialog.reserves.map(r => ({ id: "reserve:" + r.id, title: dialog.sioul.textWith("bank-rule-to-reserve", "reserve", r.title) })))
        .concat(dialog.accounts.filter(a => dialog.account === null || a.id !== dialog.account.id).map(a => ({ id: "transfer:" + a.id, title: dialog.sioul.textWith("bank-place-transfer", "account", a.title) })))

    signal changed

    function show(account) {
        dialog.account = account
        dialog.problem = ""
        newWords.text = ""
        dialog.open()
    }

    // The card again, once the rules were written.
    function refresh(accounts) {
        if (dialog.account !== null)
            dialog.account = accounts.find(a => a.id === dialog.account.id) || null
    }

    function words(text) {
        return text.split(",").map(w => w.trim()).filter(w => w !== "")
    }

    // `rule`: the rule as shown (named by its id, or its place while it holds the same); null for a new one.
    function save(rule, wordsText, direction, to, everywhere) {
        const edit = { account: everywhere ? "" : dialog.account.id, words: dialog.words(wordsText), direction: direction, to: to }
        dialog.problem = dialog.sioul.saveBankRule(rule === null ? "" : JSON.stringify(rule), JSON.stringify(edit))
        if (dialog.problem === "")
            dialog.changed()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(760, (parent ? parent.width : 760) - 2 * dialog.theme.gap)
    height: Math.min(620, (parent ? parent.height : 620) - 2 * dialog.theme.gap)
    title: dialog.account === null ? "" : dialog.theme.plain(dialog.sioul.textWith("bank-rules-title", "account", dialog.account.title))

    contentItem: ColumnLayout {
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: dialog.sioul.text("bank-rules-help")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: dialog.theme.muted
        }
        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                width: parent.width
                spacing: 6

                Repeater {
                    model: dialog.account === null ? [] : dialog.account.rules

                    delegate: RowLayout {
                        id: rule

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 6

                        TextField {
                            id: ruleWords

                            Layout.fillWidth: true
                            text: rule.modelData.words.join(", ")
                            Accessible.name: dialog.sioul.text("bank-rule-words")
                            onEditingFinished: {
                                if (text !== rule.modelData.words.join(", "))
                                    dialog.save(rule.modelData, text, rule.modelData.direction, rule.modelData.to, rule.modelData.everywhere)
                            }
                        }
                        PlainComboBox {
                            Layout.preferredWidth: 180
                            model: dialog.directions.map(d => dialog.sioul.text("bank-rule-direction-" + (d === "" ? "any" : d)))
                            currentIndex: Math.max(0, dialog.directions.indexOf(rule.modelData.direction))
                            onActivated: index => dialog.save(rule.modelData, ruleWords.text, dialog.directions[index], rule.modelData.to, rule.modelData.everywhere)
                        }
                        PlainComboBox {
                            Layout.preferredWidth: 220
                            model: dialog.places.map(p => dialog.theme.plain(p.title))
                            currentIndex: Math.max(0, dialog.places.findIndex(p => p.id === rule.modelData.to))
                            onActivated: index => dialog.save(rule.modelData, ruleWords.text, rule.modelData.direction, dialog.places[index].id, rule.modelData.everywhere)
                        }
                        Label {
                            Layout.preferredWidth: 110
                            text: dialog.sioul.text(rule.modelData.everywhere ? "bank-rule-everywhere" : "bank-rule-here")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            color: dialog.theme.muted
                        }
                        ToolButton {
                            text: "×"
                            Accessible.name: dialog.sioul.text("ui-delete")
                            onClicked: {
                                dialog.problem = dialog.sioul.removeBankRule(JSON.stringify(rule.modelData))
                                if (dialog.problem === "")
                                    dialog.changed()
                            }
                        }
                    }
                }
                Label {
                    visible: dialog.account !== null && dialog.account.rules.length === 0
                    Layout.fillWidth: true
                    text: dialog.sioul.text("bank-rules-none")
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
            }
        }
        // A new rule.
        RowLayout {
            Layout.fillWidth: true
            spacing: 6

            TextField {
                id: newWords

                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("bank-rule-words")
                // Enter adds the rule only when "Add" could: words or a way, and somewhere to go.
                onAccepted: {
                    if (addRule.enabled)
                        addRule.clicked()
                }
            }
            PlainComboBox {
                id: newDirection

                Layout.preferredWidth: 180
                model: dialog.directions.map(d => dialog.sioul.text("bank-rule-direction-" + (d === "" ? "any" : d)))
            }
            PlainComboBox {
                id: newPlace

                Layout.preferredWidth: 220
                model: dialog.places.map(p => dialog.theme.plain(p.title))
            }
            Button {
                id: addRule

                // Without words, a rule takes every movement one way.
                enabled: (dialog.words(newWords.text).length > 0 || newDirection.currentIndex > 0) && dialog.places.length > 0
                text: dialog.sioul.text("ui-add")
                onClicked: {
                    dialog.save(null, newWords.text, dialog.directions[newDirection.currentIndex], dialog.places[newPlace.currentIndex].id, false)
                    if (dialog.problem === "")
                        newWords.clear()
                }
            }
        }
        Label {
            visible: dialog.problem !== ""
            Layout.fillWidth: true
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
    }

    footer: DialogButtonBox {
        Button {
            text: dialog.sioul.text("ui-close")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: dialog.close()
    }
}
