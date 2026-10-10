// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A contract, new or changed: what it is, with whom, your number with them,
// the payment that pays it, when it renews and the notice it needs, how to
// stop it, what it covers. The notice proposed is the one usually asked for
// its kind; yours is what your contract says.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: form

    required property var sioul
    required property var theme
    required property var window
    // {id, label, notice}.
    property var kinds: []
    // {id, title}: the recurring payments to tie it to.
    property var presets: []
    property string contractId: ""
    property string problem: ""
    // The notice was written by hand: a kind chosen after does not change it.
    property bool noticeTouched: false

    signal saved

    function begin(start) {
        form.contractId = ""
        form.fill(start || {})
        form.noticeTouched = false
        form.open()
        titleField.forceActiveFocus()
    }

    function edit(contract) {
        form.contractId = contract.id
        form.fill(contract)
        form.noticeTouched = true
        form.open()
    }

    function fill(c) {
        form.problem = ""
        kindChoice.currentIndex = Math.max(0, form.kinds.findIndex(k => k.id === (c.kind || "other")))
        titleField.text = c.title || ""
        partyField.text = c.party || ""
        referenceField.text = c.reference || ""
        presetChoice.currentIndex = Math.max(0, presetChoice.choices.findIndex(p => p.id === (c.preset || "")))
        startedField.date = c.started || ""
        renewsField.date = c.renews || ""
        everyChoice.currentIndex = Math.max(0, ["", "month", "year"].indexOf(c.every || ""))
        noticeBox.value = c.notice_days !== undefined ? c.notice_days : (form.kinds[kindChoice.currentIndex] ? form.kinds[kindChoice.currentIndex].notice : 0)
        cancelField.text = c.cancel || ""
        coversArea.text = c.covers || ""
        notesArea.text = c.notes || ""
        endedField.date = c.ended || ""
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(600, (parent ? parent.width : 600) - 2 * form.theme.gap)
    title: form.sioul.text(form.contractId === "" ? "contracts-new" : "contracts-change")

    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(grid.implicitHeight, (form.parent ? form.parent.height : 700) - 200)
        contentWidth: availableWidth
        clip: true

        GridLayout {
            id: grid

            width: scroll.availableWidth
            columns: 2
            columnSpacing: 10
            rowSpacing: 8

            Label {
                text: form.sioul.text("papers-kind")
                color: form.theme.muted
            }
            PlainComboBox {
                id: kindChoice

                Layout.fillWidth: true
                model: form.kinds.map(k => form.theme.plain(k.label))
                onActivated: index => {
                    if (!form.noticeTouched)
                        noticeBox.value = form.kinds[index].notice
                }
            }
            Label {
                text: form.sioul.text("papers-title")
                color: form.theme.muted
            }
            TextField {
                id: titleField

                Layout.fillWidth: true
                placeholderText: form.sioul.text("contracts-title-hint")
            }
            Label {
                text: form.sioul.text("contracts-party")
                color: form.theme.muted
            }
            TextField {
                id: partyField

                Layout.fillWidth: true
            }
            Label {
                text: form.sioul.text("contracts-reference")
                color: form.theme.muted
            }
            TextField {
                id: referenceField

                Layout.fillWidth: true
            }
            Label {
                text: form.sioul.text("contracts-preset")
                color: form.theme.muted
            }
            PlainComboBox {
                id: presetChoice

                readonly property var choices: [{ id: "", title: "—" }].concat(form.presets)

                Layout.fillWidth: true
                model: choices.map(p => form.theme.plain(p.title))
            }
            Label {
                text: form.sioul.text("contracts-started")
                color: form.theme.muted
            }
            DateField {
                id: startedField

                theme: form.theme
                sioul: form.sioul
                locale: form.window.sioulLocale
                pickLabel: form.sioul.text("event-pick-day")
            }
            Label {
                text: form.sioul.text("contracts-renews")
                color: form.theme.muted
            }
            RowLayout {
                spacing: 6

                DateField {
                    id: renewsField

                    theme: form.theme
                    sioul: form.sioul
                    locale: form.window.sioulLocale
                    pickLabel: form.sioul.text("event-pick-day")
                }
                PlainComboBox {
                    id: everyChoice

                    Layout.preferredWidth: 170
                    model: [form.sioul.text("contracts-every-none"), form.sioul.text("contracts-every-month"), form.sioul.text("contracts-every-year")]
                }
            }
            Label {
                text: form.sioul.text("contracts-notice")
                color: form.theme.muted
            }
            RowLayout {
                spacing: 6

                SpinBox {
                    id: noticeBox

                    from: 0
                    to: 365
                    editable: true
                    onValueModified: form.noticeTouched = true
                }
                Label {
                    text: form.sioul.text("contracts-days")
                    color: form.theme.muted
                }
            }
            Label {
                Layout.columnSpan: 2
                Layout.fillWidth: true
                text: form.sioul.text("contract-rule-" + (form.kinds[kindChoice.currentIndex] ? form.kinds[kindChoice.currentIndex].id : "other"))
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: form.theme.muted
            }
            Label {
                text: form.sioul.text("contracts-cancel")
                color: form.theme.muted
            }
            TextField {
                id: cancelField

                Layout.fillWidth: true
                placeholderText: form.sioul.text("contracts-cancel-hint")
            }
            Label {
                Layout.alignment: Qt.AlignTop
                text: form.sioul.text("contracts-covers")
                color: form.theme.muted
            }
            TextArea {
                id: coversArea

                Layout.fillWidth: true
                Layout.preferredHeight: 60
                wrapMode: TextEdit.Wrap
                placeholderText: form.sioul.text("contracts-covers-hint")
                background: Rectangle {
                    color: form.theme.surface
                    border.color: coversArea.activeFocus ? form.theme.focus : form.theme.line
                    radius: 3
                }
            }
            Label {
                Layout.alignment: Qt.AlignTop
                text: form.sioul.text("papers-notes")
                color: form.theme.muted
            }
            TextArea {
                id: notesArea

                Layout.fillWidth: true
                Layout.preferredHeight: 60
                wrapMode: TextEdit.Wrap
                background: Rectangle {
                    color: form.theme.surface
                    border.color: notesArea.activeFocus ? form.theme.focus : form.theme.line
                    radius: 3
                }
            }
            Label {
                text: form.sioul.text("contracts-ended")
                color: form.theme.muted
            }
            DateField {
                id: endedField

                theme: form.theme
                sioul: form.sioul
                locale: form.window.sioulLocale
                pickLabel: form.sioul.text("event-pick-day")
            }
            Label {
                visible: form.problem !== ""
                Layout.columnSpan: 2
                Layout.fillWidth: true
                text: form.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: form.theme.warm
            }
            RowLayout {
                Layout.columnSpan: 2
                Layout.fillWidth: true
                spacing: 8

                Button {
                    visible: form.contractId !== ""
                    flat: true
                    text: form.sioul.text("papers-remove")
                    onClicked: removing.ask(titleField.text, form.sioul.text("contracts-remove-ask"), form.sioul.text("papers-remove"))
                }
                Item {
                    Layout.fillWidth: true
                }
                Button {
                    text: form.sioul.text("ui-cancel")
                    onClicked: form.close()
                }
                Button {
                    highlighted: true
                    text: form.sioul.text("ui-save")
                    onClicked: {
                        const edit = {
                            kind: form.kinds[kindChoice.currentIndex].id,
                            title: titleField.text,
                            party: partyField.text,
                            reference: referenceField.text,
                            preset: presetChoice.choices[presetChoice.currentIndex].id,
                            started: startedField.date,
                            renews: renewsField.date,
                            every: ["", "month", "year"][everyChoice.currentIndex],
                            notice_days: noticeBox.value,
                            cancel: cancelField.text,
                            covers: coversArea.text,
                            notes: notesArea.text,
                            ended: endedField.date
                        }
                        form.problem = form.sioul.saveContract(form.contractId, JSON.stringify(edit))
                        if (form.problem === "") {
                            form.close()
                            form.saved()
                        }
                    }
                }
            }
        }
    }

    ConfirmDialog {
        id: removing

        sioul: form.sioul
        theme: form.theme
        onConfirmed: {
            form.problem = form.sioul.removeContract(form.contractId)
            if (form.problem === "") {
                form.close()
                form.saved()
            }
        }
    }
}
