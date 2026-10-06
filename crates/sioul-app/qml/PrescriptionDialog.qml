// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A prescription, new or changed, from the Health page: what it is for, who
// wrote it, until when it is valid, how many days the pharmacy gives at a
// time and when it was last fetched (its errands come from these), a note;
// taken out after one question, its medicines kept. Made the first time it
// opens (HealthPage.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: form

    required property var sioul
    required property var theme
    property string prescriptionId: ""
    property string problem: ""
    readonly property var dateLocale: Qt.locale(form.sioul.text("qt-locale"))

    // Saved or taken out: the page reads its prescriptions again.
    signal saved

    // `prescription` as the page lists it; null for a new one.
    function edit(prescription) {
        form.prescriptionId = prescription ? prescription.id : ""
        form.problem = ""
        what.text = prescription ? prescription.title : ""
        prescriber.text = prescription ? prescription.prescriber || "" : ""
        validUntil.date = prescription && prescription.until ? prescription.until : ""
        refillDays.value = prescription && prescription.refill_days ? prescription.refill_days : 0
        lastRefill.date = prescription && prescription.last_refill ? prescription.last_refill : ""
        note.text = prescription ? prescription.note || "" : ""
        form.open()
        what.forceActiveFocus()
    }

    function save() {
        const edit = { title: what.text, prescriber: prescriber.text, until: validUntil.date, refill_days: refillDays.value, last_refill: lastRefill.date, note: note.text }
        const answer = JSON.parse(form.sioul.savePrescription(form.prescriptionId, JSON.stringify(edit)))
        if (answer.error) {
            form.problem = answer.error
            return
        }
        form.close()
        form.saved()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(520, (parent ? parent.width : 520) - 2 * form.theme.gap)
    title: form.prescriptionId === "" ? form.sioul.text("health-add-prescription") : form.sioul.text("health-prescription")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: form.sioul.text("health-field-what")
            color: form.theme.muted
        }
        TextField {
            id: what

            Layout.fillWidth: true
            placeholderText: form.sioul.text("health-field-what-hint")
        }
        Label {
            text: form.sioul.text("health-field-prescriber")
            color: form.theme.muted
        }
        TextField {
            id: prescriber

            Layout.fillWidth: true
        }
        Label {
            text: form.sioul.text("health-field-valid-until")
            color: form.theme.muted
        }
        DateField {
            id: validUntil

            theme: form.theme
            locale: form.dateLocale
        }
        Label {
            text: form.sioul.text("health-field-refill-days")
            color: form.theme.muted
        }
        RowLayout {
            spacing: 6

            SpinBox {
                id: refillDays

                from: 0
                to: 365
                editable: true
            }
            Label {
                text: form.sioul.text("health-days-at-a-time")
                color: form.theme.muted
            }
        }
        Label {
            text: form.sioul.text("health-field-last-refill")
            color: form.theme.muted
        }
        DateField {
            id: lastRefill

            theme: form.theme
            locale: form.dateLocale
        }
        Label {
            text: form.sioul.text("health-field-note")
            color: form.theme.muted
        }
        TextField {
            id: note

            Layout.fillWidth: true
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
    }

    // The buttons inside an Item, as the medicine's: saving that fails keeps the form open.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                text: form.sioul.text("ui-save")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: form.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            Button {
                visible: form.prescriptionId !== ""
                flat: true
                text: form.sioul.text("health-remove")
                DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                onClicked: removeAsk.now().ask(what.text, form.sioul.text("health-remove-prescription-ask"), form.sioul.text("health-remove"))
            }
            onAccepted: form.save()
            onRejected: form.close()
        }
    }

    // Taken out, once asked; its medicines stay, no longer tied to it (out of the layout: Later is hidden).
    Later {
        id: removeAsk

        sourceComponent: Component {
            ConfirmDialog {
                sioul: form.sioul
                theme: form.theme
                onConfirmed: {
                    const problem = form.sioul.removeHealth(form.prescriptionId)
                    if (problem !== "")
                        form.sioul.status = problem
                    form.close()
                    form.saved()
                }
            }
        }
    }
}
