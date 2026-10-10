// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A prescription, new or changed, from the Health page: what it is for, who
// wrote it, until when it is valid, how many days the pharmacy gives at a
// time and when it was last fetched (its errands come from these), a note;
// then its medicines as rows: each its name, its generic name (INN) and
// strength (beside it on a computer; folded under it on a phone, a tap away),
// its dose and its takes, each
// take its time and its own amount when it differs (TakesEditor.qml). Add a
// medicine adds a row; a row taken out takes its medicine out on saving,
// said in the row with "Keep it" until then. A medicine taken every few days
// or hours says when, its schedule changed in its own form. Saved whole, or
// not at all (`health::apply_prescription`). The prescription itself taken
// out after one question, its medicines kept. Made the first time it opens
// (HealthPage.qml).

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
    // A row just added: its name field takes the focus once it is made.
    property bool focusNew: false
    readonly property var dateLocale: Qt.locale(form.sioul.text("qt-locale"))

    // Saved or taken out: the page reads its prescriptions again.
    signal saved

    // `prescription` as the page lists it, with its medicines; null for a new one.
    function edit(prescription) {
        form.prescriptionId = prescription ? prescription.id : ""
        form.problem = ""
        what.text = prescription ? prescription.title : ""
        prescriber.text = prescription ? prescription.prescriber || "" : ""
        validUntil.date = prescription && prescription.until ? prescription.until : ""
        refillDays.value = prescription && prescription.refill_days ? prescription.refill_days : 0
        lastRefill.date = prescription && prescription.last_refill ? prescription.last_refill : ""
        note.text = prescription ? prescription.note || "" : ""
        medicines.clear()
        for (const m of (prescription && prescription.medicines ? prescription.medicines : []))
            medicines.append({ medicineId: m.id, name: m.name, generic: m.generic || "", strength: m.strength || "", dose: m.usual !== undefined ? m.usual : m.dose || "", every: m.schedule.every, when: m.when || "", removed: false, takes: JSON.stringify(m.takes || []) })
        form.open()
        what.forceActiveFocus()
    }

    // A row for a new medicine, at set times each day, its name to type.
    function addMedicine() {
        form.focusNew = true
        medicines.append({ medicineId: "", name: "", generic: "", strength: "", dose: "", every: "day", when: "", removed: false, takes: "[]" })
    }

    // The rows as the form gives them (`health::PrescribedEdit`): each row writes
    // what is typed into the list as it is typed.
    function save() {
        const rowsNow = []
        for (let i = 0; i < medicines.count; i++) {
            const m = medicines.get(i)
            rowsNow.push({ id: m.medicineId, name: m.name, generic: m.generic, strength: m.strength, dose: m.dose, takes: m.every === "day" ? JSON.parse(m.takes) : [], removed: m.removed })
        }
        const edit = { title: what.text, prescriber: prescriber.text, until: validUntil.date, refill_days: refillDays.value, last_refill: lastRefill.date, note: note.text, medicines: rowsNow }
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
    width: Math.min(560, (parent ? parent.width : 560) - 2 * form.theme.gap)
    title: form.prescriptionId === "" ? form.sioul.text("health-add-prescription") : form.sioul.text("health-prescription")

    // Its medicines, a small window: the form scrolls, its buttons stay.
    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(column.implicitHeight, (form.parent ? form.parent.height : 700) - 160)
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: 12

            GridLayout {
                Layout.fillWidth: true
                // On a phone, each label above its field: the fields keep the width.
                columns: form.width < 480 ? 1 : 2
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
                    sioul: form.sioul
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
                    sioul: form.sioul
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
            }

            // Its medicines: a row each, made, changed and taken out here.
            Label {
                Layout.topMargin: 4
                text: form.sioul.text("health-medicines")
                font.weight: Font.DemiBold
                color: form.theme.text
            }
            ListModel {
                id: medicines
            }
            Repeater {
                id: rows

                model: medicines

                delegate: Rectangle {
                    id: medicine

                    required property int index
                    required property string medicineId
                    required property string name
                    required property string dose
                    required property string every
                    required property string when
                    required property bool removed
                    required property string takes
                    required property string generic
                    required property string strength
                    // On a phone, the generic name and strength unfolded by a tap.
                    property bool unfolded: false

                    Layout.fillWidth: true
                    implicitHeight: inside.implicitHeight + 16
                    radius: 6
                    color: "transparent"
                    border.color: form.theme.line

                    ColumnLayout {
                        id: inside

                        anchors.fill: parent
                        anchors.margins: 8
                        spacing: 6

                        // Taken out on saving: said, with "Keep it" until then.
                        RowLayout {
                            visible: medicine.removed
                            Layout.fillWidth: true
                            spacing: 8

                            Label {
                                Layout.fillWidth: true
                                text: form.sioul.textWith("health-row-removed", "name", nameField.text.trim() || medicine.name)
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: form.theme.muted
                            }
                            Button {
                                flat: true
                                implicitWidth: implicitContentWidth + leftPadding + rightPadding
                                text: form.sioul.text("health-keep-it")
                                onClicked: medicines.setProperty(medicine.index, "removed", false)
                            }
                        }
                        RowLayout {
                            visible: !medicine.removed
                            Layout.fillWidth: true
                            spacing: 6

                            TextField {
                                id: nameField

                                Layout.fillWidth: true
                                Layout.minimumWidth: 80
                                text: medicine.name
                                placeholderText: form.sioul.text("health-field-brand")
                                Accessible.name: form.sioul.text("health-field-brand")
                                onTextEdited: medicines.setProperty(medicine.index, "name", nameField.text)
                                // A row just added: its name first.
                                Component.onCompleted: {
                                    if (form.focusNew && medicine.index === medicines.count - 1) {
                                        form.focusNew = false
                                        nameField.forceActiveFocus()
                                    }
                                }
                            }
                            TextField {
                                id: doseField

                                Layout.preferredWidth: Math.min(140, medicine.width / 3)
                                text: medicine.dose
                                placeholderText: form.sioul.text("health-field-dose")
                                Accessible.name: form.sioul.text("health-field-dose")
                                onTextEdited: medicines.setProperty(medicine.index, "dose", doseField.text)
                            }
                            // A row made here goes at once; one saved before, on saving.
                            ToolButton {
                                icon.name: "user-trash"
                                icon.color: form.theme.text
                                ToolTip.delay: 400
                                Accessible.name: form.sioul.text("health-remove")
                                Accessible.description: nameField.text
                                ToolTip.visible: hovered
                                ToolTip.text: form.sioul.text("health-remove")
                                onClicked: {
                                    if (medicine.medicineId === "")
                                        medicines.remove(medicine.index)
                                    else
                                        medicines.setProperty(medicine.index, "removed", true)
                                }
                            }
                        }
                        // On a phone, folded under the name: what is given, or what can be, a tap away.
                        Button {
                            visible: !medicine.removed && form.width < 480 && !medicine.unfolded
                            flat: true
                            implicitWidth: implicitContentWidth + leftPadding + rightPadding
                            text: form.theme.plain([medicine.generic, medicine.strength].filter(t => t.trim() !== "").join(" ") || form.sioul.text("health-row-precise") + "…")
                            onClicked: {
                                medicine.unfolded = true
                                genericField.forceActiveFocus()
                            }
                        }
                        // Its generic name and strength, for a doctor or a pharmacist.
                        GridLayout {
                            visible: !medicine.removed && (form.width >= 480 || medicine.unfolded)
                            Layout.fillWidth: true
                            columns: form.width < 480 ? 1 : 2
                            columnSpacing: 6
                            rowSpacing: 6

                            TextField {
                                id: genericField

                                Layout.fillWidth: true
                                Layout.preferredWidth: 1
                                text: medicine.generic
                                placeholderText: form.sioul.text("health-field-generic")
                                Accessible.name: form.sioul.text("health-field-generic")
                                onTextEdited: medicines.setProperty(medicine.index, "generic", genericField.text)
                            }
                            TextField {
                                id: strengthField

                                Layout.fillWidth: true
                                Layout.preferredWidth: 1
                                text: medicine.strength
                                placeholderText: form.sioul.text("health-field-strength")
                                Accessible.name: form.sioul.text("health-field-strength")
                                onTextEdited: medicines.setProperty(medicine.index, "strength", strengthField.text)
                            }
                        }
                        TakesEditor {
                            id: takesEditor

                            visible: !medicine.removed && medicine.every === "day"
                            Layout.fillWidth: true
                            sioul: form.sioul
                            theme: form.theme
                            usual: doseField.text.trim()
                            Component.onCompleted: takesEditor.load(JSON.parse(medicine.takes))
                            onEdited: medicines.setProperty(medicine.index, "takes", JSON.stringify(takesEditor.takes()))
                        }
                        // Every few days or hours: when, in words; its form changes it.
                        Label {
                            visible: !medicine.removed && medicine.every !== "day"
                            Layout.fillWidth: true
                            text: form.sioul.textWith("health-row-other-schedule", "when", medicine.when)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: form.theme.muted
                        }
                    }
                }
            }
            Button {
                flat: true
                implicitWidth: implicitContentWidth + leftPadding + rightPadding
                icon.name: "list-add"
                icon.color: form.theme.text
                text: form.sioul.text("health-add-medicine")
                onClicked: form.addMedicine()
            }
            Label {
                visible: form.problem !== ""
                Layout.fillWidth: true
                text: form.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: form.theme.warm
            }
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
