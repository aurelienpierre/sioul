// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An office's opening hours changed by hand (opening.rs, offices.rs): a
// week to fill in, day by day, or the short form typed ("mo-fr 9-12,
// 14-17; sa 9-12"), each following the other; what Sioul reads in it said
// under it. When it answers the phone at other hours, those too; the free
// line for what Sioul cannot count. Public holidays are closed, as Sioul
// assumes for an office. Saved through the contacts' own writer: what Sioul
// counts is written in OpenStreetMap's syntax, what it cannot goes to the
// free line, nothing dropped.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: form

    required property var sioul
    required property var theme
    // The card's file.
    property string key: ""
    property string name: ""
    property string problem: ""
    // What the short form reads as (`readHours`), and the phone's.
    property var reading: ({ written: "", lines: [], days: ["", "", "", "", "", "", ""], rest: [] })
    property var phoneReading: ({ written: "", lines: [], days: [], rest: [] })
    readonly property var dayKeys: ["mo", "tu", "we", "th", "fr", "sa", "su"]

    signal saved

    // Opens on a card's hours as its section showed them.
    function edit(key, shown) {
        form.key = key
        form.name = shown ? shown.name || "" : ""
        form.problem = ""
        shortForm.text = shown && shown.open ? shown.open.value : ""
        phoneDiffers.checked = shown !== null && shown.phone !== null && shown.phone !== undefined
        phoneForm.text = shown && shown.phone ? shown.phone.value : ""
        note.text = shown ? shown.note || "" : ""
        form.readShort()
        form.readPhone()
        form.open()
    }

    // A day's field (its row's second item), or null before its row is made.
    function dayField(d) {
        const row = dayRows.itemAt(d)
        return row ? row.children[1] as TextField : null
    }

    // The short form read again, and the days filled from it.
    function readShort() {
        form.reading = JSON.parse(form.sioul.readHours(shortForm.text) || "{}")
        const days = form.reading.days || []
        for (let d = 0; d < 7; d++) {
            const field = form.dayField(d)
            if (field)
                field.text = days[d] || ""
        }
    }

    function readPhone() {
        form.phoneReading = JSON.parse(form.sioul.readHours(phoneForm.text) || "{}")
    }

    // A day changed: the short form made again from the seven days, what it
    // held that Sioul cannot count kept after them.
    function fromDays() {
        const parts = []
        for (let d = 0; d < 7; d++) {
            const field = form.dayField(d)
            const text = field ? field.text.trim() : ""
            if (text !== "")
                parts.push(form.dayKeys[d] + " " + text)
        }
        shortForm.text = parts.concat(form.reading.rest || []).join("; ")
        form.readShort()
    }

    function save() {
        const typed = { open: shortForm.text, phone: phoneDiffers.checked ? phoneForm.text : "", note: note.text }
        const answer = JSON.parse(form.sioul.saveContactHours(form.key, JSON.stringify(typed)) || "{}")
        if (answer.error) {
            form.problem = answer.error
            return
        }
        form.close()
        form.saved()
    }

    objectName: "openingHoursForm"
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(520, (parent ? form.roomWidth : 520) - 2 * form.theme.gap)
    height: Math.min(implicitHeight, (parent ? form.roomHeight : 800) - 2 * form.theme.gap)
    modal: true
    title: form.theme.plain(form.sioul.textWith("hours-form-title", "name", form.name))

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 14

            Section {
                Layout.fillWidth: true
                theme: form.theme
                title: form.sioul.text("hours-form-open")

                Label {
                    Layout.fillWidth: true
                    text: form.sioul.text("hours-form-short")
                    color: form.theme.muted
                }
                TextField {
                    id: shortForm

                    objectName: "openingHoursFormShort"
                    Layout.fillWidth: true
                    placeholderText: "mo-fr 9-12, 14-17; sa 9-12"
                    Accessible.name: form.sioul.text("hours-form-short")
                    Accessible.description: form.sioul.text("hours-form-short-help")
                    onEditingFinished: form.readShort()
                }
                Label {
                    Layout.fillWidth: true
                    text: form.sioul.text("hours-form-short-help")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: form.theme.muted
                }
                // The week, day by day: each row its hours, empty when closed.
                Repeater {
                    id: dayRows

                    model: 7

                    delegate: RowLayout {
                        id: dayRow

                        required property int index

                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            readonly property string day: form.sioul.text("weekday-" + (dayRow.index + 1))

                            Layout.preferredWidth: 96
                            // A day starts its row with a capital, in French too ("lundi" → "Lundi").
                            text: day.charAt(0).toUpperCase() + day.slice(1)
                            textFormat: Text.PlainText
                            color: form.theme.text
                            elide: Text.ElideRight
                        }
                        TextField {
                            id: dayField

                            objectName: "openingHoursFormDay" + dayRow.index
                            Layout.fillWidth: true
                            placeholderText: form.sioul.text("hours-form-closed")
                            Accessible.name: form.sioul.textWith("hours-form-day", "day", form.sioul.text("weekday-" + (dayRow.index + 1)))
                            onEditingFinished: form.fromDays()
                        }
                    }
                }
                Label {
                    objectName: "openingHoursFormReads"
                    Layout.fillWidth: true
                    text: (form.reading.lines || []).length > 0 ? form.sioul.textWith("hours-form-reads", "hours", (form.reading.lines || []).join("; ")) : shortForm.text.trim() !== "" ? form.sioul.text("hours-form-unread") : ""
                    visible: text !== ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: form.theme.text
                }
                Label {
                    Layout.fillWidth: true
                    visible: (form.reading.lines || []).length > 0 && (form.reading.rest || []).length > 0
                    text: form.sioul.textWith("hours-lookup-free", "text", (form.reading.rest || []).join("; "))
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: form.theme.muted
                }
                Label {
                    Layout.fillWidth: true
                    text: form.sioul.text("hours-form-holidays")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: form.theme.muted
                }
            }

            Section {
                Layout.fillWidth: true
                theme: form.theme
                title: form.sioul.text("hours-phone-title")

                CheckBox {
                    id: phoneDiffers

                    objectName: "openingHoursFormPhoneDiffers"
                    Layout.fillWidth: true
                    text: form.sioul.text("hours-form-phone")
                }
                TextField {
                    id: phoneForm

                    objectName: "openingHoursFormPhone"
                    visible: phoneDiffers.checked
                    Layout.fillWidth: true
                    placeholderText: "mo-th 9-12"
                    Accessible.name: form.sioul.text("hours-form-phone-hours")
                    onEditingFinished: form.readPhone()
                }
                Label {
                    visible: phoneDiffers.checked && text !== ""
                    Layout.fillWidth: true
                    text: (form.phoneReading.lines || []).length > 0 ? form.sioul.textWith("hours-form-reads", "hours", form.phoneReading.lines.join("; ")) : phoneForm.text.trim() !== "" ? form.sioul.text("hours-form-unread") : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: form.theme.text
                }
            }

            Section {
                Layout.fillWidth: true
                theme: form.theme
                title: form.sioul.text("hours-form-note")

                TextArea {
                    id: note

                    objectName: "openingHoursFormNote"
                    Layout.fillWidth: true
                    placeholderText: form.sioul.text("hours-form-note-help")
                    wrapMode: TextEdit.Wrap
                    Accessible.name: form.sioul.text("hours-form-note")
                }
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

    footer: DialogButtonBox {
        Button {
            objectName: "openingHoursFormSave"
            text: form.sioul.text("ui-save")
            highlighted: true
            onClicked: form.save()
        }
        Button {
            text: form.sioul.text("ui-cancel")
            onClicked: form.close()
        }
    }
}
