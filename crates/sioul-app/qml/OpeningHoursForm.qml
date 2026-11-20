// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An office's opening hours changed by hand (opening.rs, offices.rs): a
// week to fill in, day by day, or the short form typed in the window's
// words ("lu-ve 9-12, 14-17; sa 9-12" in French), each following the
// other; what Sioul reads in it said under it. A day Sioul cannot count
// keeps its text in its field, said under it, and goes to the free line at
// Save; ";" typed in a day is read as ",". One tick, "Open on public
// holidays" (unticked: closed on those Sioul knows), which `24/7` or hours
// of their own on public holidays tick. When it answers the phone at other
// hours, those too; the free line for what Sioul cannot count. Save reads
// what was typed last first, a word still being composed by a phone's
// keyboard included. Saved through the contacts' own writer: what Sioul
// counts is written in OpenStreetMap's syntax, what it cannot goes to the
// free line, nothing dropped; hours that count the same as the card's are
// left as they are.

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
    property var reading: ({ written: "", short: "", lines: [], days: ["", "", "", "", "", "", ""], rest: [] })
    property var phoneReading: ({ written: "", lines: [], days: [], rest: [] })
    // What the card holds that Sioul cannot count, kept where it is while its hours do not change.
    property string cardRest: ""
    // The days Sioul cannot count as typed, Monday first: their text stays in their field.
    property var dayDoubts: [false, false, false, false, false, false, false]
    // A day's field or the short form typed into since they were last read: Save reads them first.
    property bool daysTyped: false
    property bool shortTyped: false
    // The days' abbreviations in the window's words ("lu ma me je ve sa di").
    readonly property var dayKeys: {
        const words = form.sioul.text("hours-short-days").split(" ").filter(w => w !== "")
        return words.length === 7 ? words : ["mo", "tu", "we", "th", "fr", "sa", "su"]
    }

    signal saved

    // Opens on a card's hours as its section showed them.
    function edit(key, shown) {
        form.key = key
        form.name = shown ? shown.name || "" : ""
        form.problem = ""
        const open = shown && shown.open ? shown.open : null
        // As the window's words say them; hours Sioul cannot count, as written, to be corrected.
        shortForm.text = open ? (open.unread ? open.value : open.short) : ""
        form.cardRest = open && !open.unread ? (open.rest || []).join("; ") : ""
        holidaysOpen.checked = open !== null && open.holidays_off === false
        phoneDiffers.checked = shown !== null && shown.phone !== null && shown.phone !== undefined
        phoneForm.text = shown && shown.phone ? (shown.phone.unread ? shown.phone.value : shown.phone.short) : ""
        note.text = shown ? shown.note || "" : ""
        form.dayDoubts = [false, false, false, false, false, false, false]
        form.readShort()
        form.readPhone()
        form.daysTyped = false
        form.shortTyped = false
        form.open()
    }

    // A day's field (its row's line's second item), or null before its row is made.
    function dayField(d) {
        const row = dayRows.itemAt(d)
        return row ? (row.children[0] as RowLayout).children[1] as TextField : null
    }

    // The short form read again, and the days filled from it: a day Sioul could
    // not count keeps its text (`keep`, when the days made the short form).
    function readShort(keep) {
        form.reading = JSON.parse(form.sioul.readHours(shortForm.text) || "{}")
        // 24/7, or hours of their own on public holidays: the tick as the hours say it.
        if (form.reading.holidays_said === true)
            holidaysOpen.checked = form.reading.holidays_off === false
        const days = form.reading.days || []
        for (let d = 0; d < 7; d++) {
            const field = form.dayField(d)
            if (field && !(keep && form.dayDoubts[d]))
                field.text = days[d] || ""
        }
        if (!keep)
            form.dayDoubts = [false, false, false, false, false, false, false]
        form.shortTyped = false
    }

    function readPhone() {
        form.phoneReading = JSON.parse(form.sioul.readHours(phoneForm.text) || "{}")
    }

    // A day changed: the short form made again from the days Sioul counts, in
    // the window's words; ";" typed in a day read as ","; a day it cannot
    // count kept in its field and said; what the short form held that Sioul
    // cannot count (a sentence) kept after them.
    function fromDays() {
        const parts = []
        const doubts = []
        for (let d = 0; d < 7; d++) {
            const field = form.dayField(d)
            const text = field ? field.text.trim().replace(/;/g, ",") : ""
            if (field && field.text !== text && text !== "")
                field.text = text
            let counted = text === ""
            if (!counted) {
                const day = JSON.parse(form.sioul.readHours(form.dayKeys[d] + " " + text) || "{}")
                counted = (day.lines || []).length > 0
                if (counted)
                    parts.push(form.dayKeys[d] + " " + text)
            }
            doubts.push(!counted)
        }
        form.dayDoubts = doubts
        const kept = (form.reading.lines || []).length > 0 ? form.reading.rest || [] : []
        shortForm.text = parts.concat(kept).join("; ")
        form.readShort(true)
        // The same hours, days with the same hours together ("lu-ve 9:00-12:00").
        if ((form.reading.lines || []).length > 0)
            shortForm.text = [form.reading.short].concat(kept).join("; ")
        form.daysTyped = false
    }

    // The days Sioul cannot count, as "Tuesday: 9-12 and 2-5", for the free line.
    function doubtfulDays() {
        const out = []
        for (let d = 0; d < 7; d++) {
            const field = form.dayField(d)
            if (form.dayDoubts[d] && field && field.text.trim() !== "") {
                const day = form.sioul.text("weekday-" + (d + 1))
                out.push(form.sioul.textArgs("hours-day-line", JSON.stringify({ days: day.charAt(0).toUpperCase() + day.slice(1), hours: field.text.trim() })))
            }
        }
        return out
    }

    function save() {
        // The field typed in last gives up the focus: a word still being composed by a phone's
        // keyboard is committed and its editing ends, read as it ends; else read here.
        scroll.forceActiveFocus()
        if (form.daysTyped)
            form.fromDays()
        else if (form.shortTyped)
            form.readShort()
        let free = note.text.trim()
        for (const day of form.doubtfulDays()) {
            if (free.indexOf(day) < 0)
                free = free === "" ? day : free + "; " + day
        }
        const typed = { open: shortForm.text, phone: phoneDiffers.checked ? phoneForm.text : "", note: free, holidays_open: holidaysOpen.checked }
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
                    textFormat: Text.PlainText
                    color: form.theme.muted
                }
                TextField {
                    id: shortForm

                    objectName: "openingHoursFormShort"
                    Layout.fillWidth: true
                    placeholderText: form.sioul.text("hours-form-short-placeholder")
                    Accessible.name: form.sioul.text("hours-form-short")
                    Accessible.description: form.sioul.text("hours-form-short-help")
                    onTextEdited: form.shortTyped = true
                    onEditingFinished: {
                        if (form.shortTyped)
                            form.readShort()
                    }
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

                    // One day: its name, its hours, and under them, when Sioul cannot count them, that it cannot.
                    delegate: ColumnLayout {
                        id: dayRow

                        required property int index

                        Layout.fillWidth: true
                        spacing: 2

                        RowLayout {
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
                                objectName: "openingHoursFormDay" + dayRow.index
                                Layout.fillWidth: true
                                placeholderText: form.sioul.text("hours-form-closed")
                                Accessible.name: form.sioul.textWith("hours-form-day", "day", form.sioul.text("weekday-" + (dayRow.index + 1)))
                                onTextEdited: form.daysTyped = true
                                onEditingFinished: {
                                    if (form.daysTyped)
                                        form.fromDays()
                                }
                            }
                        }
                        Label {
                            objectName: "openingHoursFormDayDoubt" + dayRow.index
                            visible: form.dayDoubts[dayRow.index] === true
                            Layout.fillWidth: true
                            Layout.leftMargin: 104
                            text: form.sioul.text("hours-form-day-unread")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: form.theme.warm
                        }
                    }
                }
                Label {
                    objectName: "openingHoursFormReads"
                    Layout.fillWidth: true
                    text: (form.reading.lines || []).length > 0 ? form.sioul.textWith("hours-form-reads", "hours", (form.reading.lines || []).join("; ")) : shortForm.text.trim() !== "" ? (form.reading.doubt || form.sioul.text("hours-form-unread")) : ""
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
                // What the card holds that Sioul cannot count: it stays where it is while these hours do not change.
                Label {
                    objectName: "openingHoursFormCardRest"
                    Layout.fillWidth: true
                    visible: form.cardRest !== ""
                    text: form.sioul.textWith("hours-form-card-rest", "text", form.cardRest)
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: form.theme.muted
                }
                CheckBox {
                    id: holidaysOpen

                    objectName: "openingHoursFormHolidays"
                    Layout.fillWidth: true
                    text: form.sioul.text("hours-form-holidays")
                }
                Label {
                    Layout.fillWidth: true
                    text: form.sioul.text("hours-form-holidays-help")
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
                    placeholderText: form.sioul.text("hours-form-short-placeholder")
                    Accessible.name: form.sioul.text("hours-form-phone-hours")
                    onEditingFinished: form.readPhone()
                }
                Label {
                    visible: phoneDiffers.checked && text !== ""
                    Layout.fillWidth: true
                    text: (form.phoneReading.lines || []).length > 0 ? form.sioul.textWith("hours-form-reads", "hours", form.phoneReading.lines.join("; ")) : phoneForm.text.trim() !== "" ? (form.phoneReading.doubt || form.sioul.text("hours-form-unread")) : ""
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
