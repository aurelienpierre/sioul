// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A calendar subscribed by its address (SubscribeForm.qml, AgendasSetup.qml;
// 18 November 2026) on a stand-in for Sioul, at a computer's width and at a
// phone's (412): Agenda ⚙ ▸ Your agendas offers "Subscribe to a calendar…",
// whose form says where Moodle and PRONOTE show their address, offers the
// official calendars of holidays, the person's country's first and another
// country's on asking, or says their country has none and offers the others
// (one chosen fills the address, the name and the kind; the Ministry's school
// holidays begin after class), sends what was said, and shows the server's
// refusal. A subscribed agenda's line says when
// it was read, and offers Refresh now, Change the address and Stop (asked
// once), its name and whether it holds holidays; nothing of a server's.
// Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1344
    height: 2600

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal agendaShared(string id, string answer)
        signal agendasChanged
        signal personsChanged

        property var calls: []
        // What the next subscription answers.
        property string refusal: ""
        // The person's country, as the settings say it: "FR"; "DE", one with no official calendar here.
        property string home: "FR"

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function agendasSetup() { return JSON.stringify(root.view) }
        function setAgendaField(id, field, value) { return "" }
        function readAgendaSharing(id) { mock.calls = mock.calls.concat([["sharing", id]]) }
        // As `subscriptions::officials` orders them: the person's country first, when it has some.
        function subscribeOfficials() {
            const france = { code: "FR", name: "France" }
            const uk = { code: "GB", name: "United Kingdom" }
            return JSON.stringify({
                home: mock.home,
                home_name: mock.home === "FR" ? "France" : mock.home === "GB" ? "United Kingdom" : "Germany",
                countries: mock.home === "GB" ? [uk, france] : [france, uk],
                officials: [
                    { id: "fr-public-metropole", country: "FR", holidays: "public", name: "Public holidays · Métropole", hint: "", url: "https://etalab.github.io/jours-feries-france-data/ics/jours_feries_metropole.ics", after_class: false, taken: false },
                    { id: "fr-public-guyane", country: "FR", holidays: "public", name: "Public holidays · Guyane", hint: "", url: "https://etalab.github.io/jours-feries-france-data/ics/jours_feries_guyane.ics", after_class: false, taken: false },
                    { id: "fr-school-zone-a", country: "FR", holidays: "school", name: "School holidays · Zone A", hint: "Besançon, Bordeaux", url: "https://fr.ftp.opendatasoft.com/openscol/fr-en-calendrier-scolaire/Zone-A.ics", after_class: true, taken: false },
                    { id: "gb-public-scotland", country: "GB", holidays: "public", name: "Public holidays · Scotland", hint: "", url: "https://www.gov.uk/bank-holidays/scotland.ics", after_class: false, taken: false }
                ]
            })
        }
        function subscribeCalendar(text) {
            mock.calls = mock.calls.concat([["subscribe", JSON.parse(text)]])
            return JSON.stringify(mock.refusal === "" ? { id: "timetable", problem: "" } : { id: "", problem: mock.refusal })
        }
        function setSubscriptionField(id, field, value) { mock.calls = mock.calls.concat([["field", id, field, value]]); return "" }
        function refreshSubscription(id) { mock.calls = mock.calls.concat([["refresh", id]]); return "" }
        function stopSubscription(id) { mock.calls = mock.calls.concat([["stop", id]]); return "" }
    }

    readonly property var row: ({ colour: "#4c6b5c", events: true, tasks: false, whose: "yours", person: "", area: "admin+leisure", reaches: "all-day", access: "read", server_read_only: true, for_work: false, said: false, kept_hidden: false, parent: "", parent_address: "", parent_answers: "mail", standing: [], titles: false })
    readonly property var view: ({
        agendas: [
            Object.assign({}, root.row, { id: "subscribed/timetable", name: "Lycée timetable", place: "example.org", server: null, subscription: { id: "timetable", url: "https://school.example.org/calendar/export_execute.php?userid=7&authtoken=0123", name: "", holidays: "", days_off: false, after_class: false, line: "Read Wed 19 Nov 08:00." } }),
            Object.assign({}, root.row, { id: "subscribed/zone-a", name: "Zone A", place: "opendatasoft.com", server: null, subscription: { id: "zone-a", url: "https://fr.ftp.opendatasoft.com/openscol/fr-en-calendrier-scolaire/Zone-A.ics", name: "Zone A", holidays: "school", days_off: false, after_class: true, line: "Not read on this device yet." } }),
            // An entry that does not read here (F20): its line, and Stop alone.
            Object.assign({}, root.row, { id: "subscribed/unread", name: "Unread", place: "example.org", server: null, subscription: { id: "unread", url: "", name: "", holidays: "", days_off: false, after_class: false, engagements: false, unread: true, line: "This calendar's settings do not read here." } })
        ],
        pairs: [],
        children: { kinds: [], yours: [], follow_ups: true, any: false },
        accounts: [{ id: "local", name: "this computer", google: false }]
    })

    Component {
        id: setupComponent

        AgendasSetup {
            width: root.width
            sioul: mock
            theme: testTheme
        }
    }

    function all(item, test, out) {
        out = out || []
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            root.all(child, test, out)
        }
        return out
    }

    function named(item, name) {
        return root.all(item, c => c.objectName === name)
    }

    function one(item, name) {
        const found = root.named(item, name)
        return found.length > 0 ? found[0] : null
    }

    // Nothing wider than the panel: the form and the lines fit a phone.
    function fits(item) {
        const wide = root.all(item, c => c.visible && c.width > 0 && c.mapToItem(item, 0, 0).x + c.width > item.width + 1)
        return wide.filter(c => !(c instanceof Repeater))
    }

    TestCase {
        name: "Subscribe"
        when: windowShown

        function init() {
            mock.calls = []
            mock.refusal = ""
            mock.home = "FR"
            root.width = 1344
        }

        function test_the_form_offers_an_address_and_the_official_calendars_data() {
            return [{ tag: "computer", width: 1344 }, { tag: "phone", width: 412 }]
        }

        function test_the_form_offers_an_address_and_the_official_calendars(data) {
            root.width = data.width
            const setup = createTemporaryObject(setupComponent, root)
            const open = root.one(setup, "agendaSubscribe")
            verify(open.visible, "Subscribe to a calendar… beside New agenda…")
            mouseClick(open)
            const form = root.one(setup, "subscribeForm")
            verify(form.visible)
            verify(!root.one(setup, "agendaSubscribe").visible, "the button folds away while the form is open")
            compare(root.one(form, "subscribeWhereMoodle").text, "subscribe-where-moodle")
            compare(root.one(form, "subscribeWherePronote").text, "subscribe-where-pronote")
            // The person's country's official calendars: public holidays by territory, school holidays by zone.
            compare(root.one(form, "subscribeOfficial").text, "subscribe-official:France")
            verify(!root.one(form, "subscribeCountry").visible, "the person's country first, no list of countries")
            const publics = root.one(form, "subscribePublic")
            const schools = root.one(form, "subscribeSchool")
            compare(publics.count, 3, "a line to choose, then two territories")
            compare(schools.count, 2)
            schools.currentIndex = 1
            schools.activated(1)
            compare(root.one(form, "subscribeAddress").text, "https://fr.ftp.opendatasoft.com/openscol/fr-en-calendrier-scolaire/Zone-A.ics")
            compare(root.one(form, "subscribeName").text, "School holidays · Zone A")
            compare(root.one(form, "subscribeKind").currentIndex, 2, "school holidays")
            verify(root.one(form, "subscribeAfterClass").visible && root.one(form, "subscribeAfterClass").checked, "the Ministry's begin after class")
            verify(!root.one(form, "subscribeBusy").visible, "holidays are never your busy time")
            verify(!root.one(form, "subscribeDaysOff").checked, "school holidays are no days off unless said")
            root.one(form, "subscribeWhose").currentIndex = 1
            root.one(form, "subscribePerson").text = "Léa"
            mouseClick(root.one(form, "subscribeMake"))
            const sent = mock.calls.find(c => c[0] === "subscribe")[1]
            compare(sent.url, "https://fr.ftp.opendatasoft.com/openscol/fr-en-calendrier-scolaire/Zone-A.ics")
            compare(sent.id, "fr-school-zone-a", "kept under the official calendar's id")
            compare(sent.holidays, "school")
            compare(sent.after_class, true)
            compare(sent.days_off, false)
            compare(sent.engagements, null, "holidays: never said busy")
            compare(sent.whose, "child", "a child's timetable goes to that child")
            compare(sent.person, "Léa")
            verify(!form.visible, "done: the form folds away")
            compare(root.fits(setup).length, 0, "nothing wider than the panel")
        }

        function test_another_country_s_calendars_data() {
            return [{ tag: "computer", width: 1344 }, { tag: "phone", width: 412 }]
        }

        // "Another country…": the countries by name, one chosen shows its calendars alone, one of
        // them chosen fills the form; a country with no school holidays here offers none.
        function test_another_country_s_calendars(data) {
            root.width = data.width
            const setup = createTemporaryObject(setupComponent, root)
            mouseClick(root.one(setup, "agendaSubscribe"))
            const form = root.one(setup, "subscribeForm")
            const another = root.one(form, "subscribeAnotherCountry")
            verify(another.visible)
            mouseClick(another)
            verify(!another.visible)
            compare(root.one(form, "subscribeOfficial").text, "subscribe-official-by-country")
            const countries = root.one(form, "subscribeCountry")
            verify(countries.visible)
            compare(countries.count, 3, "a line to choose, then each country by its name")
            compare(countries.textAt(2), "United Kingdom")
            verify(!root.one(form, "subscribePublic").visible && !root.one(form, "subscribeSchool").visible, "no country chosen yet")
            countries.currentIndex = 2
            countries.activated(2)
            const publics = root.one(form, "subscribePublic")
            verify(publics.visible)
            compare(publics.count, 2, "the United Kingdom's alone")
            verify(!root.one(form, "subscribeSchool").visible, "no school holidays offered there")
            publics.currentIndex = 1
            publics.activated(1)
            compare(root.one(form, "subscribeAddress").text, "https://www.gov.uk/bank-holidays/scotland.ics")
            compare(root.one(form, "subscribeName").text, "Public holidays · Scotland")
            compare(root.one(form, "subscribeKind").currentIndex, 1)
            verify(root.one(form, "subscribeDaysOff").checked, "public holidays count as days off by default")
            // Back to the person's own country, among the others.
            countries.currentIndex = 1
            countries.activated(1)
            compare(root.one(form, "subscribePublic").count, 3)
            verify(root.one(form, "subscribeSchool").visible)
            countries.currentIndex = 2
            countries.activated(2)
            root.one(form, "subscribePublic").currentIndex = 1
            root.one(form, "subscribePublic").activated(1)
            mouseClick(root.one(form, "subscribeMake"))
            const sent = mock.calls.find(c => c[0] === "subscribe")[1]
            compare(sent.id, "gb-public-scotland")
            compare(sent.holidays, "public")
            compare(sent.days_off, true)
            compare(root.fits(setup).length, 0, "nothing wider than the panel")
            // Opened again: the person's country first once more.
            mouseClick(root.one(setup, "agendaSubscribe"))
            verify(!root.one(form, "subscribeCountry").visible && root.one(form, "subscribeAnotherCountry").visible)
            compare(root.one(form, "subscribePublic").count, 3)
        }

        // The person's country first, whichever it is: the United Kingdom's own, France's on asking.
        function test_the_person_s_country_comes_first() {
            mock.home = "GB"
            const setup = createTemporaryObject(setupComponent, root)
            mouseClick(root.one(setup, "agendaSubscribe"))
            const form = root.one(setup, "subscribeForm")
            compare(root.one(form, "subscribeOfficial").text, "subscribe-official:United Kingdom")
            compare(root.one(form, "subscribePublic").count, 2)
            compare(root.one(form, "subscribePublic").textAt(1), "Public holidays · Scotland")
            verify(!root.one(form, "subscribeSchool").visible)
            mouseClick(root.one(form, "subscribeAnotherCountry"))
            compare(root.one(form, "subscribeCountry").textAt(1), "United Kingdom", "the person's own first among the countries")
        }

        // A country with no official calendar here: one sentence says so, the others offered at once.
        function test_a_country_with_none_is_said_and_the_others_offered() {
            root.width = 412
            mock.home = "DE"
            const setup = createTemporaryObject(setupComponent, root)
            mouseClick(root.one(setup, "agendaSubscribe"))
            const form = root.one(setup, "subscribeForm")
            compare(root.one(form, "subscribeOfficial").text, "subscribe-official-none:Germany")
            verify(!root.one(form, "subscribeAnotherCountry").visible, "the countries are offered already")
            const countries = root.one(form, "subscribeCountry")
            verify(countries.visible)
            compare(countries.count, 3)
            compare(countries.textAt(1), "France")
            verify(!root.one(form, "subscribePublic").visible)
            countries.currentIndex = 1
            countries.activated(1)
            compare(root.one(form, "subscribePublic").count, 3)
            compare(root.one(form, "subscribeSchool").count, 2)
            compare(root.fits(setup).length, 0, "nothing wider than the panel")
        }

        function test_a_calendar_of_events_may_be_your_busy_time_data() {
            return [{ tag: "computer", width: 1344 }, { tag: "phone", width: 412 }]
        }

        // The review's F41, in the form: a calendar of events may be said your own busy time as it
        // is added, off unless ticked, and sent; one of holidays never shows the tick.
        function test_a_calendar_of_events_may_be_your_busy_time(data) {
            root.width = data.width
            const setup = createTemporaryObject(setupComponent, root)
            mouseClick(root.one(setup, "agendaSubscribe"))
            const form = root.one(setup, "subscribeForm")
            const busy = root.one(form, "subscribeBusy")
            verify(busy.visible, "a calendar of events, as the form opens")
            verify(!busy.checked, "off unless ticked")
            compare(busy.text, "subscribe-engagements")
            root.one(form, "subscribeAddress").text = "https://calendar.example.org/appointments.ics"
            mouseClick(busy)
            verify(busy.checked)
            // Said to hold holidays: the tick goes, and nothing busy is sent.
            const kind = root.one(form, "subscribeKind")
            kind.currentIndex = 1
            kind.activated(1)
            verify(!busy.visible)
            mouseClick(root.one(form, "subscribeMake"))
            compare(mock.calls[mock.calls.length - 1][1].engagements, null)
            // Opened again, a calendar of events ticked: sent so.
            mouseClick(root.one(setup, "agendaSubscribe"))
            verify(busy.visible && !busy.checked, "opened again: off")
            root.one(form, "subscribeAddress").text = "https://calendar.example.org/appointments.ics"
            // Laid out again before the click lands on it.
            waitForItemPolished(setup)
            waitForItemPolished(form)
            mouseClick(busy)
            verify(busy.checked)
            mouseClick(root.one(form, "subscribeMake"))
            const sent = mock.calls[mock.calls.length - 1][1]
            compare(sent.holidays, "")
            compare(sent.engagements, true)
            // Unticked: sent as not busy.
            mouseClick(root.one(setup, "agendaSubscribe"))
            root.one(form, "subscribeAddress").text = "https://calendar.example.org/classes.ics"
            mouseClick(root.one(form, "subscribeMake"))
            compare(mock.calls[mock.calls.length - 1][1].engagements, false)
            compare(root.fits(setup).length, 0, "nothing wider than the panel")
        }

        // An address typed: no official id; the server's refusal said, the form kept.
        function test_a_refusal_is_said_and_the_form_stays() {
            root.width = 412
            const setup = createTemporaryObject(setupComponent, root)
            mouseClick(root.one(setup, "agendaSubscribe"))
            const form = root.one(setup, "subscribeForm")
            const publics = root.one(form, "subscribePublic")
            publics.currentIndex = 1
            publics.activated(1)
            compare(root.one(form, "subscribeKind").currentIndex, 1)
            verify(root.one(form, "subscribeDaysOff").checked, "public holidays count as days off by default")
            verify(!root.one(form, "subscribeAfterClass").visible)
            const address = root.one(form, "subscribeAddress")
            address.text = ""
            verify(!root.one(form, "subscribeMake").enabled, "nothing to subscribe to")
            address.forceActiveFocus()
            keyClick("h")
            address.text = "http://calendar.example.org/a.ics"
            mock.refusal = "Only an address beginning with https:// or webcal:// is read."
            mouseClick(root.one(form, "subscribeMake"))
            compare(mock.calls[0][1].id, "", "typed by hand: no official id")
            compare(root.one(form, "subscribeProblem").text, mock.refusal)
            verify(form.visible, "the form stays, to change the address")
            mouseClick(root.one(form, "subscribeCancel"))
            verify(!form.visible && root.one(setup, "agendaSubscribe").visible)
        }

        function test_a_subscribed_agenda_s_line_data() {
            return [{ tag: "computer", width: 1344 }, { tag: "phone", width: 412 }]
        }

        // Its line on this device, Refresh now, Change the address, Stop asked once; no server's block.
        function test_a_subscribed_agenda_s_line(data) {
            root.width = data.width
            const setup = createTemporaryObject(setupComponent, root)
            setup.toggle("subscribed/timetable")
            compare(mock.calls.filter(c => c[0] === "sharing").length, 0, "no server asked who sees it")
            const lines = root.named(setup, "subscriptionLine").filter(l => l.visible)
            compare(lines.length, 1)
            compare(lines[0].text, "Read Wed 19 Nov 08:00.")
            verify(root.named(setup, "agendaRename").every(f => !f.visible) && root.named(setup, "agendaRemove").every(b => !b.visible), "no server's rename nor removal")
            mouseClick(root.named(setup, "subscriptionRefresh").find(b => b.visible))
            compare(mock.calls[mock.calls.length - 1], ["refresh", "timetable"])
            mouseClick(root.named(setup, "subscriptionChange").find(b => b.visible))
            const address = root.named(setup, "subscriptionAddress").find(f => f.visible)
            compare(address.text, "https://school.example.org/calendar/export_execute.php?userid=7&authtoken=0123", "your own address, to change")
            address.text = "https://school.example.org/calendar/export_execute.php?userid=7&authtoken=4567"
            mouseClick(root.named(setup, "subscriptionAddressSave").find(b => b.visible))
            compare(mock.calls[mock.calls.length - 1], ["field", "timetable", "url", "https://school.example.org/calendar/export_execute.php?userid=7&authtoken=4567"])
            mouseClick(root.named(setup, "subscriptionStop").find(b => b.visible))
            compare(mock.calls.filter(c => c[0] === "stop").length, 0, "asked first")
            mouseClick(root.named(setup, "subscriptionStopConfirm").find(b => b.visible))
            compare(mock.calls[mock.calls.length - 1], ["stop", "timetable"])
            compare(root.fits(setup).length, 0, "nothing wider than the panel")
        }

        // A calendar of school holidays: its kind, the switch of days off, after class.
        function test_a_holidays_calendar_says_its_kind() {
            const setup = createTemporaryObject(setupComponent, root)
            setup.toggle("subscribed/zone-a")
            const kind = root.named(setup, "subscriptionKind").find(k => k.visible)
            compare(kind.currentIndex, 2)
            const after = root.named(setup, "subscriptionAfterClass").find(b => b.visible)
            verify(after.checked)
            const daysOff = root.named(setup, "subscriptionDaysOff").find(b => b.visible)
            verify(!daysOff.checked)
            mouseClick(daysOff)
            compare(mock.calls[mock.calls.length - 1], ["field", "zone-a", "days_off", "true"])
            compare(root.named(setup, "subscriptionLine").find(l => l.visible).text, "Not read on this device yet.")
        }

        // The review's F41: "These are my own engagements" offered on a calendar of events,
        // never on one of holidays, off until ticked, then sent; its colour chosen here.
        function test_own_engagements_and_its_colour() {
            const setup = createTemporaryObject(setupComponent, root)
            setup.toggle("subscribed/timetable")
            const boxes = root.named(setup, "subscriptionEngagements").filter(b => b.visible)
            compare(boxes.length, 1, "on the calendar of events alone")
            verify(!boxes[0].checked, "off unless ticked")
            mouseClick(boxes[0])
            verify(mock.calls.some(c => c[0] === "field" && c[1] === "timetable" && c[2] === "engagements" && c[3] === "true"), JSON.stringify(mock.calls))
            const dots = root.named(setup, "subscriptionColour").filter(d => d.visible)
            verify(dots.length > 0)
            mouseClick(dots[1])
            verify(mock.calls.some(c => c[0] === "field" && c[1] === "timetable" && c[2] === "colour"), JSON.stringify(mock.calls))
            setup.toggle("subscribed/timetable")
            setup.toggle("subscribed/zone-a")
            compare(root.named(setup, "subscriptionEngagements").filter(b => b.visible).length, 0, "holidays act through the days off alone")
        }

        // The review's F19 and F20: settings that do not read: the line says so, and Stop alone is offered.
        function test_a_calendar_whose_settings_do_not_read() {
            root.width = 412
            const setup = createTemporaryObject(setupComponent, root)
            setup.toggle("subscribed/unread")
            const lines = root.named(setup, "subscriptionLine").filter(l => l.visible)
            compare(lines.length, 1)
            compare(lines[0].text, "This calendar's settings do not read here.")
            compare(root.named(setup, "subscriptionRefresh").filter(b => b.visible).length, 0)
            compare(root.named(setup, "subscriptionChange").filter(b => b.visible).length, 0)
            compare(root.named(setup, "subscriptionName").filter(b => b.visible).length, 0)
            compare(root.named(setup, "subscriptionStop").filter(b => b.visible).length, 1, "Stop stays")
            compare(root.fits(setup).length, 0, "nothing wider than the panel")
        }
    }
}
