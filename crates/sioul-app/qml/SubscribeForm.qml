// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Agenda ⚙ ▸ Your agendas ▸ "Subscribe to a calendar…" (calendars by their
// address, 18 November 2026): a field for the address (https:// or
// webcal://), two lines saying where Moodle and PRONOTE show theirs, in the
// words of their own screens; or one of the official calendars of holidays
// (23 November 2026, `subscribed::offer`): the person's country's first, its
// public holidays then its school holidays; "Another country…" for the
// others, chosen by their names; where the person's country has none here,
// one sentence says so and the others are offered at once. Then its name,
// whose it is (a child's timetable goes to that child), what it is for, and
// whether it holds holidays: public ones count as your days off unless you
// say not, school ones do not unless you say so; the Ministry's school
// holidays begin after class. A calendar of events may be said your own busy
// time as it is added, off unless ticked, as its line says it afterwards
// (`engagements`, F41); a calendar of holidays never shows it. The address is a setting like any other,
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
    // `subscriptions::officials`: {home, home_name, countries: [{code, name}],
    // officials: [{id, country, holidays, name, hint, url, after_class, taken}]}, the person's country first.
    property var offer: ({})
    property var officials: []
    property string problem: ""
    // The official calendar chosen, its id; "" an address typed.
    property string official: ""
    // The country whose official calendars show; "" none chosen yet.
    property string country: ""
    // The countries offered to choose from: "Another country…" pressed, or the person's own has none here.
    property bool elsewhere: false
    readonly property var whoseChoices: ["yours", "child", "parent", "partner", "shared"]
    readonly property var kinds: ["", "public", "school"]
    readonly property var countries: form.offer.countries || []
    readonly property string home: form.offer.home || ""
    // The person's country has official calendars here: offered first.
    readonly property bool homeHas: form.home !== "" && form.countries.length > 0 && form.countries[0].code === form.home
    readonly property var shown: form.officials.filter(o => o.country === form.country)
    readonly property var publics: form.shown.filter(o => o.holidays === "public")
    readonly property var schools: form.shown.filter(o => o.holidays === "school")

    // Subscribed: its id; or put away.
    signal done(string id)
    signal cancelled

    // Opened again: the official calendars read, the person's country's shown, every field emptied.
    function begin() {
        try {
            form.offer = JSON.parse(form.sioul.subscribeOfficials() || "{}")
        } catch (e) {
            form.offer = {}
        }
        form.officials = form.offer.officials || []
        form.elsewhere = !form.homeHas
        form.country = form.homeHas ? form.home : ""
        countryChoice.currentIndex = 0
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
        busy.checked = false
        address.forceActiveFocus()
    }

    // Another country's calendars: none chosen until one is, the person's own among the countries.
    function showCountry(code) {
        form.country = code
        publicChoice.currentIndex = 0
        schoolChoice.currentIndex = 0
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
        // As its publisher shapes them: the Ministry's school holidays begin after class.
        afterClass.checked = item.holidays === "school" && item.after_class === true
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
            engagements: holidays === "" ? busy.checked : null,
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
    // The official calendars: for the person's country ("France"), by country, or none for theirs.
    Label {
        objectName: "subscribeOfficial"
        visible: form.countries.length > 0
        Layout.fillWidth: true
        Layout.topMargin: 4
        text: !form.elsewhere ? form.sioul.textWith("subscribe-official", "country", form.offer.home_name || "") : form.home !== "" && !form.homeHas ? form.sioul.textWith("subscribe-official-none", "country", form.offer.home_name || "") : form.sioul.text("subscribe-official-by-country")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: form.theme.text
    }
    PlainComboBox {
        id: countryChoice

        objectName: "subscribeCountry"
        visible: form.elsewhere && form.countries.length > 0
        Layout.fillWidth: true
        model: [form.sioul.text("subscribe-choose-country")].concat(form.countries.map(c => form.theme.plain(c.name)))
        Accessible.name: form.sioul.text("subscribe-choose-country")
        onActivated: index => form.showCountry(index > 0 ? form.countries[index - 1].code : "")
    }
    PlainComboBox {
        id: publicChoice

        objectName: "subscribePublic"
        visible: form.publics.length > 0
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
        visible: form.schools.length > 0
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
    Button {
        objectName: "subscribeAnotherCountry"
        visible: !form.elsewhere && form.countries.length > 1
        flat: true
        text: form.sioul.text("subscribe-another-country")
        onClicked: {
            form.elsewhere = true
            countryChoice.currentIndex = 0
            form.showCountry("")
        }
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
            const chosen = form.officials.find(o => o.id === form.official)
            afterClass.checked = form.kinds[index] === "school" && chosen !== undefined && chosen.after_class === true
        }
    }
    WrapCheckBox {
        id: daysOff

        objectName: "subscribeDaysOff"
        visible: kind.currentIndex > 0
        Layout.fillWidth: true
        text: form.sioul.text("subscribe-days-off")
    }
    // Its events your own busy time: a calendar of events alone, off unless ticked (F41).
    WrapCheckBox {
        id: busy

        objectName: "subscribeBusy"
        visible: kind.currentIndex === 0
        Layout.fillWidth: true
        text: form.sioul.text("subscribe-engagements")
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
