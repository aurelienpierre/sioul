// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The medicines and the prescriptions, on the Health page: content, as tasks
// are, not settings. Each medicine: its name and dose, its times ("08:00 ·
// 20:00"; each take with its amount when they differ, "08:00 · 1 tablet,
// 20:00 · 2 tablets"; every few days or hours in words), until when; paused or past
// its last day, quieter. Each prescription: who wrote it, the next visit to
// the pharmacy and the day to renew it by, in words, the medicines it
// covers, and "Fetched today". Add and Edit open their forms, the page's
// (HealthPage.qml). Today's doses, and whether they were taken, are in the
// day's list, not here.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    // As the page's view gives them (`health::medicine_rows`, `prescription_rows`).
    property var medicines: []
    property var prescriptions: []

    // A form to open: a medicine's, a prescription's (null: a new one).
    signal openMedicine(var medicine)
    signal openPrescription(var prescription)
    // A prescription's medicines fetched at the pharmacy today.
    signal fetched(string id)
    // Shown read-only to a doctor or a pharmacist: a prescription's (its id), or all taken now ("").
    signal showProfessional(string id)

    spacing: 2

    Heading {
        text: section.sioul.text("health-medicines")
        adding: section.sioul.text("health-add-medicine")
        onAdd: section.openMedicine(null)
    }
    Repeater {
        model: section.medicines

        delegate: RowLayout {
            id: medicine

            required property var modelData

            Layout.fillWidth: true
            Layout.leftMargin: 6
            spacing: 10

            ColumnLayout {
                // What is left of the line, however long the name: never wider than the screen.
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                spacing: 0

                // Whole, on two lines if need be: a name cut short could be another medicine's.
                Label {
                    Layout.fillWidth: true
                    text: [medicine.modelData.precise || medicine.modelData.name, medicine.modelData.amount || "", medicine.modelData.paused ? section.sioul.text("health-paused") : ""].filter(t => t !== "").join("  ·  ")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: medicine.modelData.quiet ? section.theme.muted : section.theme.text
                }
                Label {
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: [medicine.modelData.when, medicine.modelData.ends].filter(t => t !== "").join("  ·  ")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    font.features: { "tnum": 1 }
                    color: section.theme.muted
                }
            }
            Button {
                flat: true
                implicitWidth: implicitContentWidth + leftPadding + rightPadding
                text: section.sioul.text("ui-edit")
                Accessible.description: medicine.modelData.name
                onClicked: section.openMedicine(medicine.modelData)
            }
        }
    }

    // All the medicines taken now, for a doctor or a pharmacist to read.
    Button {
        visible: section.medicines.length > 0
        Layout.leftMargin: 6
        flat: true
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        text: section.sioul.text("health-pro-show")
        onClicked: section.showProfessional("")
    }

    Heading {
        Layout.topMargin: 10
        text: section.sioul.text("health-prescriptions")
        adding: section.sioul.text("health-add-prescription")
        onAdd: section.openPrescription(null)
    }
    Repeater {
        model: section.prescriptions

        // Its words, then its buttons beside them; under them, right, when the
        // section is narrow (a phone, a wide window's column): the words get the
        // whole width, a name is not split, the dates take two lines, not four.
        delegate: GridLayout {
            id: prescription

            required property var modelData

            Layout.fillWidth: true
            Layout.leftMargin: 6
            columns: section.width < 460 ? 1 : 2
            columnSpacing: 10
            rowSpacing: 0

            ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredWidth: 1
                spacing: 0

                Label {
                    Layout.fillWidth: true
                    text: [prescription.modelData.title, prescription.modelData.prescriber || ""].filter(t => t !== "").join("  ·  ")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: section.theme.text
                }
                // The next visit to the pharmacy, the day to renew it by: the soonest first.
                Label {
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: prescription.modelData.next.join("  ·  ")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: section.theme.muted
                }
                Label {
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: prescription.modelData.covers
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: section.theme.muted
                }
                // Its medicines precisely, when a generic name or a strength is given.
                Repeater {
                    model: prescription.modelData.lines || []

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        font.features: { "tnum": 1 }
                        color: section.theme.muted
                    }
                }
                // This prescription, for a doctor or a pharmacist to read: a line
                // of its own, under its words, however narrow the section.
                Button {
                    visible: (prescription.modelData.medicines || []).length > 0
                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: section.sioul.text("health-pro-show")
                    Accessible.description: prescription.modelData.title
                    onClicked: section.showProfessional(prescription.modelData.id)
                }
            }
            RowLayout {
                Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
                spacing: 10

                // Fetched at the pharmacy: the next visit is counted from today.
                // The buttons as wide as their names.
                Button {
                    visible: prescription.modelData.refill_days !== undefined
                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: section.sioul.text("health-refilled")
                    icon.name: prescription.modelData.fetched_today ? "task-complete" : ""
                    icon.color: section.theme.text
                    onClicked: section.fetched(prescription.modelData.id)
                }
                Button {
                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: section.sioul.text("ui-edit")
                    Accessible.description: prescription.modelData.title
                    onClicked: section.openPrescription(prescription.modelData)
                }

            }
        }
    }

    // A group's name, and the button that adds to it.
    component Heading: RowLayout {
        id: heading

        property alias text: name.text
        property alias adding: add.text

        signal add

        Layout.fillWidth: true
        Layout.leftMargin: 6
        spacing: 8

        Label {
            id: name

            Layout.fillWidth: true
            elide: Text.ElideRight
            font.weight: Font.DemiBold
            color: section.theme.text
        }
        Button {
            id: add

            flat: true
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            icon.name: "list-add"
            icon.color: section.theme.text
            onClicked: heading.add()
        }
    }
}
