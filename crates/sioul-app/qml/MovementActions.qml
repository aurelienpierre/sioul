// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What a money movement's row offers (docs/accounting.md, "Moving a
// movement", "Movements tab"): Change… in the fields its kind allows,
// where it goes (one budget, or split between two), "The same as…", "Not
// the same payment" beside a source joined as probably the same, Tie to…,
// its removal by what it is made from (Delete, asked once; Not this time;
// Leave out; Not a payout), Put back for one removed, and what tells of it
// opened (the mail, the letter, the contract). Change…, Tie to… and the
// removal come with a row that says what it allows (the Movements tab's).
// Opened by a long press on a phone, a right click or its "⋯" button on a
// computer. Each act says its effect, with Undo, and a move asks once
// whether the payee's next movements go there too: your act is the consent.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: actions

    required property var sioul
    required property var theme
    required property var window
    // Every budget, whatever the hour: [{id, title}].
    property var budgets: []
    // The bank accounts, for Change…: [{id, title}].
    property var accounts: []
    // What an act on a movement could not do, in words; "" for nothing.
    property string problem: ""
    // The form of Change…, once made (the window's tests).
    readonly property var changing: changeForm.item

    // An act done: its effect in words, and what to ask once ({payee, budget, title, way}, or null).
    signal said(string text, var ask)

    visible: false

    // The menu of a row: {key, who, amount, cents, budget_id, sources: [{kind, words, address, probable, key}], probable};
    // from the Movements tab also {address, gone, allowed: {made, fields, removal}, data}.
    function show(row) {
        menu.now().show(row)
    }

    // An act's answer ({said} or {problem}): its line is the status line's, with Undo.
    function done(text) {
        const answer = JSON.parse(text || "{}")
        actions.problem = answer.problem || ""
        actions.said(answer.problem || "", null)
    }

    // Every menu and form of it closed (the window's pictures).
    function closeAll() {
        menu.close()
        changeForm.close()
    }

    // Change… from elsewhere than the menu (a movement's pane).
    function changeNow(row) {
        changeForm.now().change(row)
    }

    // The row Delete asks about, until its answer.
    property var deleting: null

    // The removal of its kind: Delete asked once, the others at once (each with Undo).
    function removeRow(row) {
        actions.deleting = row
        if (row.allowed.removal === "delete")
            confirmDelete.now().ask(actions.theme.plain(row.who + "  ·  " + row.amount), actions.sioul.text("movement-delete-ask"), actions.sioul.text("movement-removal-delete"))
        else
            actions.done(actions.sioul.removeMovement(row.address))
    }

    // What tells of it opened, as a link to it opens.
    function open(address) {
        if (address === "")
            return
        actions.window.openThing(JSON.parse(actions.sioul.linkTarget("", address) || "null") || { uri: address, kind: address.startsWith("mid:") ? "mail" : "", key: "" })
    }

    function answered(text) {
        const done = JSON.parse(text)
        actions.said(done.problem || done.said || "", done.ask || null)
    }

    Later {
        id: menu

        sourceComponent: Component {
            SioulMenu {
                id: rowMenu

                property var row: null
                readonly property var probable: rowMenu.row ? rowMenu.row.sources.filter(s => s.probable) : []
                readonly property var openable: rowMenu.row ? rowMenu.row.sources.filter(s => s.address !== "") : []

                readonly property var allowed: rowMenu.row && rowMenu.row.allowed ? rowMenu.row.allowed : null

                function show(row) {
                    rowMenu.row = row
                    rowMenu.popup()
                }

                MenuItem {
                    objectName: "movementChange"
                    visible: rowMenu.allowed !== null && rowMenu.allowed.fields.length > 0 && rowMenu.row.gone !== true
                    height: visible ? implicitHeight : 0
                    text: actions.sioul.text("movement-change")
                    onTriggered: changeForm.now().change(rowMenu.row)
                }
                MenuItem {
                    objectName: "movementPlace"
                    text: actions.sioul.text("bank-movement-where") + "…"
                    onTriggered: place.now().edit(rowMenu.row, false)
                }
                MenuItem {
                    objectName: "movementSplit"
                    text: actions.sioul.text("movement-split")
                    onTriggered: place.now().edit(rowMenu.row, true)
                }
                MenuItem {
                    objectName: "movementSame"
                    text: actions.sioul.text("movement-same")
                    onTriggered: same.now().edit(rowMenu.row)
                }
                Instantiator {
                    model: rowMenu.probable

                    delegate: MenuItem {
                        required property var modelData

                        text: actions.sioul.text("movement-not-same") + " · " + actions.theme.plain(modelData.words)
                        onTriggered: actions.said(actions.sioul.notSameMovement(rowMenu.row.key, modelData.key), null)
                    }
                    onObjectAdded: (index, item) => rowMenu.insertItem(4 + index, item)
                    onObjectRemoved: (index, item) => rowMenu.removeItem(item)
                }
                // Tied to a contact, a task, a project…
                MenuItem {
                    objectName: "movementTie"
                    visible: rowMenu.allowed !== null && rowMenu.row.gone !== true
                    height: visible ? implicitHeight : 0
                    text: actions.sioul.text("movement-tie")
                    onTriggered: tie.now().show({ uri: rowMenu.row.address }, "")
                }
                MenuItem {
                    objectName: "movementRemove"
                    visible: rowMenu.allowed !== null && !!rowMenu.allowed.removal && rowMenu.row.gone !== true
                    height: visible ? implicitHeight : 0
                    text: rowMenu.allowed && rowMenu.allowed.removal ? actions.sioul.text("movement-removal-" + rowMenu.allowed.removal) : ""
                    onTriggered: actions.removeRow(rowMenu.row)
                }
                MenuItem {
                    objectName: "movementPutBack"
                    visible: rowMenu.row !== null && rowMenu.row.gone === true
                    height: visible ? implicitHeight : 0
                    text: actions.sioul.text("movement-put-back-button")
                    onTriggered: actions.done(actions.sioul.putBackMovement(rowMenu.row.address))
                }
                Instantiator {
                    model: rowMenu.openable

                    delegate: MenuItem {
                        required property var modelData

                        text: actions.sioul.text("ui-open") + " · " + actions.theme.plain(modelData.words)
                        onTriggered: actions.open(modelData.address)
                    }
                    onObjectAdded: (index, item) => rowMenu.addItem(item)
                    onObjectRemoved: (index, item) => rowMenu.removeItem(item)
                }
            }
        }
    }

    // Where it goes: one budget; split, a part in one and the rest in another.
    Later {
        id: place

        sourceComponent: Component {
            SafeDialog {
                id: placeForm

                property var row: null
                property bool splitting: false
                property string problem: ""

                function edit(row, splitting) {
                    placeForm.row = row
                    placeForm.splitting = splitting
                    placeForm.problem = ""
                    rest.currentIndex = Math.max(0, actions.budgets.findIndex(b => b.id === row.budget_id))
                    part.currentIndex = -1
                    partAmount.text = ""
                    placeForm.open()
                }

                function save() {
                    const amount = Number(partAmount.text.replace(/[\s€]/g, "").replace(",", ".").replace("−", "-"))
                    if (placeForm.splitting && (part.currentIndex < 0 || Number.isNaN(amount) || amount === 0)) {
                        placeForm.problem = actions.sioul.textWith("amount-unreadable", "text", partAmount.text.trim())
                        return
                    }
                    const parts = placeForm.splitting ? [{ budget: actions.budgets[part.currentIndex].id, amount: Math.abs(amount) }] : []
                    const text = actions.sioul.moveMovement(placeForm.row.key, "budget:" + actions.budgets[rest.currentIndex].id, JSON.stringify(parts))
                    const done = JSON.parse(text)
                    if (done.problem) {
                        placeForm.problem = done.problem
                        return
                    }
                    placeForm.close()
                    actions.answered(text)
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(420, (parent ? placeForm.roomWidth : 420) - 2 * actions.theme.gap)
                title: placeForm.row ? actions.theme.plain(placeForm.row.who + "  ·  " + placeForm.row.amount) : ""

                contentItem: GridLayout {
                    columns: 2
                    columnSpacing: 10
                    rowSpacing: 8

                    Label {
                        visible: placeForm.splitting
                        text: actions.sioul.text("movement-split")
                        color: actions.theme.muted
                    }
                    RowLayout {
                        visible: placeForm.splitting
                        Layout.fillWidth: true
                        spacing: 8

                        PlainComboBox {
                            id: part

                            Layout.fillWidth: true
                            model: actions.budgets.map(b => actions.theme.plain(b.title))
                            displayText: currentIndex < 0 ? "…" : currentText
                        }
                        TextField {
                            id: partAmount

                            Layout.preferredWidth: 90
                            horizontalAlignment: Text.AlignRight
                            inputMethodHints: Qt.ImhFormattedNumbersOnly
                            font.features: { "tnum": 1 }
                        }
                    }
                    Label {
                        text: actions.sioul.text("bank-movement-where")
                        color: actions.theme.muted
                    }
                    PlainComboBox {
                        id: rest

                        Layout.fillWidth: true
                        model: actions.budgets.map(b => actions.theme.plain(b.title))
                    }
                    Label {
                        visible: placeForm.problem !== ""
                        Layout.columnSpan: 2
                        Layout.fillWidth: true
                        text: placeForm.problem
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: actions.theme.warm
                    }
                }

                footer: DialogButtonBox {
                    Button {
                        text: actions.sioul.text("ui-save")
                        highlighted: true
                        onClicked: placeForm.save()
                    }
                    Button {
                        text: actions.sioul.text("ui-cancel")
                        onClicked: placeForm.close()
                    }
                }
            }
        }
    }

    // The same payment as another movement: the nearest of the same way, to choose from.
    Later {
        id: same

        sourceComponent: Component {
            SafeDialog {
                id: sameForm

                property var row: null
                property var candidates: []
                property int chosen: -1
                property string problem: ""

                function edit(row) {
                    sameForm.row = row
                    sameForm.candidates = JSON.parse(actions.sioul.sameCandidates(row.key) || "[]")
                    sameForm.chosen = -1
                    sameForm.problem = ""
                    sameForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? sameForm.roomWidth : 440) - 2 * actions.theme.gap)
                title: actions.sioul.text("movement-same-ask")

                contentItem: ColumnLayout {
                    spacing: 4

                    Repeater {
                        model: sameForm.candidates

                        delegate: RadioButton {
                            required property var modelData
                            required property int index

                            Layout.fillWidth: true
                            checked: sameForm.chosen === index
                            text: actions.theme.plain(modelData.date + "  ·  " + modelData.label + "  ·  " + modelData.amount)
                            onClicked: sameForm.chosen = index
                        }
                    }
                    Label {
                        visible: sameForm.candidates.length === 0
                        text: actions.sioul.text("coming-none")
                        color: actions.theme.muted
                    }
                    Label {
                        visible: sameForm.problem !== ""
                        Layout.fillWidth: true
                        text: sameForm.problem
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: actions.theme.warm
                    }
                }

                footer: DialogButtonBox {
                    Button {
                        enabled: sameForm.chosen >= 0
                        text: actions.sioul.text("movement-same")
                        highlighted: true
                        onClicked: {
                            const problem = actions.sioul.sameMovement(sameForm.row.key, sameForm.candidates[sameForm.chosen].key)
                            if (problem !== "") {
                                sameForm.problem = problem
                                return
                            }
                            sameForm.close()
                            actions.said("", null)
                        }
                    }
                    Button {
                        text: actions.sioul.text("ui-cancel")
                        onClicked: sameForm.close()
                    }
                }
            }
        }
    }

    // Change…: the movement's form, the fields its kind allows.
    Later {
        id: changeForm

        sourceComponent: Component {
            MovementDialog {
                sioul: actions.sioul
                theme: actions.theme
                budgets: actions.budgets
                accounts: actions.accounts
                onChanged: text => actions.done(text)
            }
        }
    }

    // Tie to…: anything, by a few words of its title.
    Later {
        id: tie

        sourceComponent: Component {
            LinkPicker {
                sioul: actions.sioul
                theme: actions.theme
            }
        }
    }

    // Delete, asked once (with Undo after).
    Later {
        id: confirmDelete

        sourceComponent: Component {
            ConfirmDialog {
                sioul: actions.sioul
                theme: actions.theme
                onConfirmed: actions.done(actions.sioul.removeMovement(actions.deleting.address))
            }
        }
    }
}
