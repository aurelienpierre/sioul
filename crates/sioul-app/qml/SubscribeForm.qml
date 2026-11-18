// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Agenda ⚙ ▸ Your agendas ▸ "Subscribe to a calendar…" (calendars by their
// address, 18 November 2026): a field for the address (https:// or
// webcal://), two lines saying where Moodle and PRONOTE show theirs, in the
// words of their own screens; or one of the official French calendars,
// public holidays by territory, school holidays by zone. Then its name,
// whose it is (a child's timetable goes to that child), what it is for, and
// whether it holds holidays: public ones count as your days off unless you
// say not, school ones do not unless you say so; the Ministry's school
// holidays begin after class. The address is a setting like any other,
// typed once and carried to your other devices; each device reads it by
// itself (`subscriptions::subscribe`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: form

    required property var sioul
    required property var theme
    // `subscriptions::officials`: [{id, holidays, name, hint, url, taken}].
    property var officials: []
    property string problem: ""
    // The official calendar chosen, its id; "" an address typed.
    property string official: ""
    readonly property var whoseChoices: ["yours", "child", "parent", "partner", "shared"]
    readonly property var kinds: ["", "public", "school"]
    readonly property var publics: form.officials.filter(o => o.holidays === "public")
    readonly property var schools: form.officials.filter(o => o.holidays === "school")

    // Subscribed: its id; or put away.
    signal done(string id)
    signal cancelled

    // Opened again: the official calendars read, every field emptied.
    function begin() {
        try {
            form.officials = JSON.parse(form.sioul.subscribeOfficials() || "{}").officials || []
        } catch (e) {
            form.officials = []
        }
        form.problem = ""
        form.official = ""
        address.text = ""
        name.text = ""
        publicChoice.currentIndex = 0
        schoolChoice.currentIndex = 0
        kind.currentIndex = 0
        whose.currentIndex = 0
        person.text = ""
        daysOff.checked = true
        afterClass.checked = false
        address.forceActiveFocus()
    }

    // An official calendar chosen: its address, its name and its kind filled in.
    function choose(item) {
        if (!item)
            return
        form.official = item.id
        address.text = item.url
        name.text = item.name
        kind.currentIndex = form.kinds.indexOf(item.holidays)
        daysOff.checked = item.holidays === "public"
        afterClass.checked = item.holidays === "school"
    }

    function make() {
        const holidays = form.kinds[kind.currentIndex]
        const areas = [[work, "work"], [admin, "admin"], [leisure, "leisure"]].filter(p => p[0].checked).map(p => p[1]).join("+")
        const asked = {
            url: address.text.trim(),
            name: name.text.trim(),
            holidays: holidays,
            days_off: holidays === "" ? null : daysOff.checked,
            after_class: holidays === "school" ? afterClass.checked : null,
            id: form.official,
            whose: form.whoseChoices[whose.currentIndex],
            person: person.text.trim(),
            area: areas
        }
        const answer = JSON.parse(form.sioul.subscribeCalendar(JSON.stringify(asked)) || "{}")
        form.problem = answer.id ? "" : (answer.problem || "")
        if (answer.id)
            form.done(answer.id)
    }

    objectName: "subscribeForm"
    spacing: 6

    Label {
        Layout.fillWidth: true
        text: form.sioul.text("subscribe-title")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: form.theme.text
        Accessible.role: Accessible.Heading
    }
    Label {
        text: form.sioul.text("subscribe-address")
        color: form.theme.text
    }
    TextField {
        id: address

        objectName: "subscribeAddress"
        Layout.fillWidth: true
        placeholderText: form.sioul.text("subscribe-address-hint")
        inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
        Accessible.name: form.sioul.text("subscribe-address")
        // Typed by hand: no longer the official one chosen.
        onTextEdited: form.official = ""
    }
    // Where Moodle and PRONOTE show their address, in their own screens' words.
    Label {
        objectName: "subscribeWhereMoodle"
        Layout.fillWidth: true
        text: form.sioul.text("subscribe-where-moodle")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: form.theme.muted
    }
    Label {
        objectName: "subscribeWherePronote"
        Layout.fillWidth: true
        text: form.sioul.text("subscribe-where-pronote")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: form.theme.muted
    }
    Label {
        Layout.fillWidth: true
        Layout.topMargin: 4
        text: form.sioul.text("subscribe-official")
        wrapMode: Text.Wrap
        color: form.theme.text
    }
    PlainComboBox {
        id: publicChoice

        objectName: "subscribePublic"
        Layout.fillWidth: true
        model: [form.sioul.text("subscribe-choose-public")].concat(form.publics.map(o => form.theme.plain(o.name)))
        Accessible.name: form.sioul.text("subscribe-choose-public")
        onActivated: index => {
            if (index > 0) {
                schoolChoice.currentIndex = 0
                form.choose(form.publics[index - 1])
            }
        }
    }
    PlainComboBox {
        id: schoolChoice

        objectName: "subscribeSchool"
        Layout.fillWidth: true
        model: [form.sioul.text("subscribe-choose-school")].concat(form.schools.map(o => form.theme.plain(o.name)))
        Accessible.name: form.sioul.text("subscribe-choose-school")
        onActivated: index => {
            if (index > 0) {
                publicChoice.currentIndex = 0
                form.choose(form.schools[index - 1])
            }
        }
    }
    // A zone's académies, under its name.
    Label {
        readonly property var chosen: schoolChoice.currentIndex > 0 ? form.schools[schoolChoice.currentIndex - 1] : null

        visible: chosen !== null && chosen.hint !== ""
        Layout.fillWidth: true
        text: chosen ? chosen.hint : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: form.theme.muted
    }
    TextField {
        id: name

        objectName: "subscribeName"
        Layout.fillWidth: true
        placeholderText: form.sioul.text("subscribe-name")
        Accessible.name: placeholderText
    }
    // Whose it is: a child's timetable goes to that child.
    PlainComboBox {
        id: whose

        objectName: "subscribeWhose"
        Layout.fillWidth: true
        model: form.whoseChoices.map(w => form.sioul.text("agenda-whose-" + w))
        Accessible.name: form.sioul.text("agenda-whose")
    }
    TextField {
        id: person

        objectName: "subscribePerson"
        visible: whose.currentIndex > 0
        Layout.fillWidth: true
        placeholderText: form.sioul.text(whose.currentIndex === 1 ? "agenda-person-child" : "agenda-person")
        Accessible.name: placeholderText
    }
    Flow {
        Layout.fillWidth: true
        spacing: 12

        CheckBox {
            id: work

            text: form.sioul.text("area-work")
        }
        CheckBox {
            id: admin

            text: form.sioul.text("area-admin")
            checked: true
        }
        CheckBox {
            id: leisure

            text: form.sioul.text("area-leisure")
            checked: true
        }
    }
    PlainComboBox {
        id: kind

        objectName: "subscribeKind"
        Layout.fillWidth: true
        model: form.kinds.map(k => form.sioul.text("subscribe-holidays-" + (k === "" ? "none" : k)))
        Accessible.name: form.sioul.text("subscribe-holidays")
        onActivated: index => {
            daysOff.checked = form.kinds[index] === "public"
            afterClass.checked = form.kinds[index] === "school" && form.official !== ""
        }
    }
    WrapCheckBox {
        id: daysOff

        objectName: "subscribeDaysOff"
        visible: kind.currentIndex > 0
        Layout.fillWidth: true
        text: form.sioul.text("subscribe-days-off")
    }
    WrapCheckBox {
        id: afterClass

        objectName: "subscribeAfterClass"
        visible: form.kinds[kind.currentIndex] === "school"
        Layout.fillWidth: true
        text: form.sioul.text("subscribe-after-class")
    }
    Label {
        objectName: "subscribeProblem"
        visible: form.problem !== ""
        Layout.fillWidth: true
        text: form.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: form.theme.warm
    }
    Flow {
        Layout.fillWidth: true
        spacing: 8

        Button {
            objectName: "subscribeMake"
            enabled: address.text.trim() !== ""
            text: form.sioul.text("subscribe-make")
            onClicked: form.make()
        }
        Button {
            objectName: "subscribeCancel"
            flat: true
            text: form.sioul.text("agenda-keep")
            onClicked: form.cancelled()
        }
    }
}
