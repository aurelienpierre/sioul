// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A medicine, new or changed, from the Health page: its name (its brand, or
// your word for it), its generic name (INN) and strength, optional, for a
// doctor or a pharmacist; its dose, when
// it is taken (at set times each day, its takes as rows, each its time and
// its own amount when it differs: TakesEditor.qml; every few days; every few
// hours), until when, the prescription it comes with, paused for now; taken
// out after one question. Made the first time it opens (HealthPage.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: form

    required property var sioul
    required property var theme
    // The prescriptions it can come with, {id, title}, as they were when it
    // opened: not bound to the page's view, which a minute's change would
    // give again and the list's choice with it.
    property var prescriptions: []
    property string medicineId: ""
    property string problem: ""
    readonly property var everies: ["day", "days", "hours"]
    readonly property var dateLocale: Qt.locale(form.sioul.text("qt-locale"))

    // Saved or taken out: the page reads its medicines again.
    signal saved

    // `medicine` as the page lists it, null for a new one; the prescriptions it can come with.
    function edit(medicine, prescriptions) {
        form.prescriptions = prescriptions
        form.medicineId = medicine ? medicine.id : ""
        form.problem = ""
        medicineName.text = medicine ? medicine.name : ""
        generic.text = medicine ? medicine.generic || "" : ""
        strength.text = medicine ? medicine.strength || "" : ""
        since.date = medicine && medicine.since ? medicine.since : ""
        // Its usual amount: the one most takes have, each other take saying its own.
        medicineDose.text = medicine ? (medicine.usual !== undefined ? medicine.usual : medicine.dose) || "" : ""
        const schedule = medicine ? medicine.schedule : { every: "day", times: ["08:00"] }
        every.currentIndex = Math.max(0, form.everies.indexOf(schedule.every))
        takes.load(medicine && schedule.every === "day" ? medicine.takes : [])
        everyDays.value = schedule.every === "days" ? schedule.days : 2
        everyHours.value = schedule.every === "hours" ? schedule.hours : 6
        dayTime.text = schedule.every === "days" ? schedule.time : "08:00"
        // Today on this device's clock: toISOString would give the day in UTC, yesterday after midnight.
        from.date = schedule.every === "days" ? schedule.from : Qt.formatDate(new Date(), "yyyy-MM-dd")
        const start = schedule.every === "hours" ? new Date(schedule.from * 1000) : new Date()
        hourFrom.text = start.toTimeString().slice(0, 5)
        until.date = medicine && medicine.until ? medicine.until : ""
        link.currentIndex = medicine && medicine.prescription ? Math.max(0, form.prescriptions.findIndex(p => p.id === medicine.prescription) + 1) : 0
        paused.checked = medicine ? medicine.paused === true : false
        form.open()
        medicineName.forceActiveFocus()
    }

    function save() {
        const kind = form.everies[every.currentIndex]
        const today = Qt.formatDate(new Date(), "yyyy-MM-dd")
        const edit = {
            name: medicineName.text,
            generic: generic.text,
            strength: strength.text,
            since: since.date,
            dose: medicineDose.text,
            every: kind,
            takes: takes.takes(),
            days: everyDays.value,
            hours: everyHours.value,
            time: dayTime.text,
            from: kind === "hours" ? today + "T" + hourFrom.text : from.date,
            until: until.date,
            prescription: link.currentIndex > 0 ? form.prescriptions[link.currentIndex - 1].id : "",
            paused: paused.checked
        }
        const answer = JSON.parse(form.sioul.saveMedicine(form.medicineId, JSON.stringify(edit)))
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
    title: form.medicineId === "" ? form.sioul.text("health-add-medicine") : form.sioul.text("health-medicine")

    // Many takes, a small window: the form scrolls, its buttons stay.
    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(grid.implicitHeight, (form.parent ? form.parent.height : 700) - 160)
        contentWidth: availableWidth
        clip: true

        GridLayout {
            id: grid

            width: scroll.availableWidth
            // On a phone, each label above its field: the takes keep the width.
            columns: form.width < 480 ? 1 : 2
            columnSpacing: 10
            rowSpacing: 8

            // Long labels wrap, in a column no wider than this: the fields keep the width.
            Label {
                Layout.maximumWidth: 170
                text: form.sioul.text("health-field-brand")
                wrapMode: Text.Wrap
                color: form.theme.muted
            }
            TextField {
                id: medicineName

                Layout.fillWidth: true
                placeholderText: form.sioul.text("health-field-brand-hint")
            }
            // For a doctor or a pharmacist: what is in it, whatever the brand.
            Label {
                Layout.maximumWidth: 170
                text: form.sioul.text("health-field-generic")
                wrapMode: Text.Wrap
                color: form.theme.muted
            }
            TextField {
                id: generic

                Layout.fillWidth: true
                placeholderText: form.sioul.text("health-field-generic-hint")
            }
            Label {
                Layout.maximumWidth: 170
                text: form.sioul.text("health-field-strength")
                wrapMode: Text.Wrap
                color: form.theme.muted
            }
            TextField {
                id: strength

                Layout.fillWidth: true
                placeholderText: form.sioul.text("health-field-strength-hint")
            }
            Label {
                text: form.sioul.text("health-field-dose")
                color: form.theme.muted
            }
            TextField {
                id: medicineDose

                Layout.fillWidth: true
                placeholderText: form.sioul.text("health-field-dose-hint")
            }
            Label {
                text: form.sioul.text("health-field-when")
                color: form.theme.muted
            }
            ComboBox {
                id: every

                Layout.fillWidth: true
                model: form.everies.map(e => form.sioul.text("health-every-" + e + "-choice"))
            }
            Label {
                visible: every.currentIndex === 0
                Layout.alignment: Qt.AlignTop
                Layout.topMargin: 10
                text: form.sioul.text("health-field-takes")
                color: form.theme.muted
            }
            TakesEditor {
                id: takes

                visible: every.currentIndex === 0
                Layout.fillWidth: true
                sioul: form.sioul
                theme: form.theme
                usual: medicineDose.text.trim()
            }
            Label {
                visible: every.currentIndex === 1
                text: form.sioul.text("health-field-every-days")
                color: form.theme.muted
            }
            RowLayout {
                visible: every.currentIndex === 1
                spacing: 6

                SpinBox {
                    id: everyDays

                    from: 1
                    to: 90
                    editable: true
                }
                Label {
                    text: form.sioul.text("health-field-at")
                    color: form.theme.muted
                }
                TextField {
                    id: dayTime

                    Layout.preferredWidth: 70
                    inputMask: "99:99"
                }
            }
            Label {
                visible: every.currentIndex === 1
                text: form.sioul.text("health-field-from")
                color: form.theme.muted
            }
            DateField {
                id: from

                visible: every.currentIndex === 1
                theme: form.theme
                locale: form.dateLocale
            }
            Label {
                visible: every.currentIndex === 2
                text: form.sioul.text("health-field-every-hours")
                color: form.theme.muted
            }
            RowLayout {
                visible: every.currentIndex === 2
                spacing: 6

                SpinBox {
                    id: everyHours

                    from: 1
                    to: 48
                    editable: true
                }
                Label {
                    text: form.sioul.text("health-field-from-time")
                    color: form.theme.muted
                }
                TextField {
                    id: hourFrom

                    Layout.preferredWidth: 70
                    inputMask: "99:99"
                }
            }

            Label {
                text: form.sioul.text("health-field-until")
                color: form.theme.muted
            }
            DateField {
                id: until

                theme: form.theme
                locale: form.dateLocale
            }
            // When it was first taken, if you know: how long, said to a professional.
            Label {
                Layout.maximumWidth: 170
                text: form.sioul.text("health-field-since")
                wrapMode: Text.Wrap
                color: form.theme.muted
            }
            DateField {
                id: since

                theme: form.theme
                locale: form.dateLocale
            }
            Label {
                text: form.sioul.text("health-field-prescription")
                color: form.theme.muted
            }
            ComboBox {
                id: link

                Layout.fillWidth: true
                model: [form.sioul.text("health-no-prescription")].concat(form.prescriptions.map(p => form.theme.plain(p.title)))
            }
            Item {
                visible: grid.columns > 1
                Layout.preferredHeight: 1
            }
            CheckBox {
                id: paused

                visible: form.medicineId !== ""
                text: form.sioul.text("health-pause")
            }
            Label {
                visible: form.problem !== ""
                Layout.columnSpan: grid.columns
                Layout.fillWidth: true
                text: form.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: form.theme.warm
            }
        }
    }

    // The buttons inside an Item: a DialogButtonBox as the footer itself closes
    // the dialog on "Save" even when saving fails, and what went wrong is never read.
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
                visible: form.medicineId !== ""
                flat: true
                text: form.sioul.text("health-remove")
                DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                onClicked: removeAsk.now().ask(medicineName.text, form.sioul.text("health-remove-medicine-ask"), form.sioul.text("health-remove"))
            }
            onAccepted: form.save()
            onRejected: form.close()
        }
    }

    // Taken out, once asked (out of the layout: Later is hidden).
    Later {
        id: removeAsk

        sourceComponent: Component {
            ConfirmDialog {
                sioul: form.sioul
                theme: form.theme
                onConfirmed: {
                    const problem = form.sioul.removeHealth(form.medicineId)
                    if (problem !== "")
                        form.sioul.status = problem
                    form.close()
                    form.saved()
                }
            }
        }
    }
}
