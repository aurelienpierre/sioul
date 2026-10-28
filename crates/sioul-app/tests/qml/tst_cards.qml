// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Every object's card where you are (CardHost.qml, EventCard.qml,
// NeedCard.qml; the unified agenda, part 3), on a stand-in for Sioul that
// records what each card asks: which card a reference opens (a dose opens
// none here: its day in Health); an appointment's card read from its file,
// Edit offered where its writer allows it, the reason in its place where
// nothing can change; Delete through its own writer; the card you came from
// kept one level deep, Back going there and then closing; a meal's card and
// its actions through that day's writer, a day gone by said and offering
// nothing; what you typed in a form closed without Save given back at its
// next opening, gone with Cancel. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 800

    Theme {
        id: testTheme
    }

    // Made up: no real calendar, no real person.
    QtObject {
        id: mock

        property string status: ""
        property var calls: []
        property bool past: false

        // What the cards follow.
        signal agendaChanged()
        signal tasksChanged()
        signal linksChanged()
        signal healthViewChanged()
        property var events: ({
            "/c/dentist.ics": { id: "event:c/dentist.ics", key: "/c/dentist.ics", uid: "dentist", start: 4000000000, end: 4000003600, all_day: false, summary: "Dentist", when: "Monday · 09:00 – 10:00", before: 15, after: 0, location: "Rue Blanche", calendar: "Personal", color: null, recurring: false, cancelled: false, tentative: false, read_only: false, with_others: false, task: "", list: "", notes: "", organizer: "", attendees: [], why: "" },
            "/c/holidays.ics": { id: "event:c/holidays.ics", key: "/c/holidays.ics", uid: "holidays", start: 4000086400, end: 4000090000, all_day: false, summary: "Town fair", when: "Tuesday · 09:00 – 10:00", before: 0, after: 0, location: "", calendar: "Holidays", color: null, recurring: false, cancelled: false, tentative: false, read_only: true, with_others: false, task: "", list: "", notes: "", organizer: "", attendees: [], why: "This calendar can only be read." }
        })

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key }
        function eventCard(key, start) {
            const event = mock.events[key]
            return event ? JSON.stringify(event) : "{}"
        }
        function eventReminder(key, start) { return "{}" }
        function reminderChoices(current) { return "[]" }
        function eventPairing(key) { return "null" }
        function related(uri) { return "[]" }
        function deleteEvent(key, start, onlyThis) {
            mock.calls.push({ what: "deleteEvent", key: key, start: start, onlyThis: onlyThis })
            return ""
        }
        function needItem(date, key) {
            if (key !== "meal:1")
                return "{}"
            return JSON.stringify({ date: date, title: "Monday 5 October", today: !mock.past, past: mock.past, later: 15, item: { key: "meal:1", kind: "meal", name: "Lunch", from: "12:10", to: "13:00", at: "12:30", detail: "eating from 12:30", note: "", changed: false, added: false, quiet: false, off: false, past: mock.past, minutes: 30, before: 20, after: 0, alarm: "", alarm_skipped: false, alarm_open: false } })
        }
        function changeNeed(edit) {
            mock.calls.push({ what: "changeNeed", edit: JSON.parse(edit) })
            return ""
        }
        function taskCardChoices() { return "{}" }
        function calendars() { return JSON.stringify([{ id: "local/personal", name: "Personal", color: null }]) }
        function eventForm(key) {
            mock.calls.push({ what: "eventForm", key: key })
            return JSON.stringify({ edit: { title: "Dentist", location: "Rue Blanche", notes: "", start: "2096-10-01T09:00", end: "2096-10-01T10:00", all_day: false, repeat: "", margins: { before: 15, after: 0 }, demands: {}, remind: "" }, calendar: "local/personal", calendar_name: "Personal", here_only: "", stays: "", read_only: false })
        }
        function saveEvent(key, edit, calendar) {
            mock.calls.push({ what: "saveEvent", key: key, edit: JSON.parse(edit) })
            return ""
        }
    }

    // The window, as the cards see it.
    QtObject {
        id: shell

        property bool compact: false
        property var opened: []
        function openThing(item, fromCard) { shell.opened.push(item) }
        function linkFrom(source) {}
        function addLinked(kind, source) {}
    }

    Component {
        id: hostComponent

        CardHost {
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    Component {
        id: dialogComponent

        EventDialog {
            sioul: mock
            theme: testTheme
        }
    }

    Component {
        id: needFormComponent

        NeedDayForm {
            sioul: mock
            theme: testTheme
        }
    }

    QtObject {
        id: keeper

        property var forms: ({})
    }

    function find(item, name) {
        if (!item)
            return null
        if (item.objectName === name)
            return item
        const kids = item.children || []
        for (let i = 0; i < kids.length; ++i) {
            const found = root.find(kids[i], name)
            if (found)
                return found
        }
        if (item.contentItem && item.contentItem !== item)
            return root.find(item.contentItem, name)
        return null
    }

    // Every object under `item` that `test` takes, its children and its popups' own searched.
    function all(item, test, out) {
        out = out || []
        if (!item)
            return out
        const kids = (item.children || []).concat(item.data || [])
        const seen = []
        for (let i = 0; i < kids.length; ++i) {
            const kid = kids[i]
            if (!kid || seen.indexOf(kid) >= 0)
                continue
            seen.push(kid)
            if (test(kid))
                out.push(kid)
            if (kid.children !== undefined || kid.data !== undefined)
                root.all(kid, test, out)
            // A Loader's form (a Dialog is no child of it).
            if (kid.item !== undefined && kid.item !== null && typeof kid.item === "object" && kid.status !== undefined) {
                if (test(kid.item))
                    out.push(kid.item)
                root.all(kid.item, test, out)
            }
        }
        return out
    }

    // Every item under `item`, counted (the objects a card keeps alive).
    function count(item) {
        let n = 1
        for (let i = 0; i < item.children.length; ++i)
            n += root.count(item.children[i])
        return n
    }

    TestCase {
        name: "Cards"
        when: windowShown

        function init() {
            mock.calls = []
            mock.status = ""
            mock.past = false
            shell.opened = []
            keeper.forms = {}
        }

        function makeHost() {
            const host = createTemporaryObject(hostComponent, root, { width: 520, height: 760 })
            waitForRendering(host)
            return host
        }

        // Each reference opens its kind's card; a dose none here (its day in Health, where its medicine is named).
        function test_a_reference_opens_its_kind_s_card() {
            const host = makeHost()
            compare(host.cardOf({ kind: "event" }), "event")
            compare(host.cardOf({ kind: "block", owner: "" }), "event", "a block of a task not known here: its event")
            compare(host.cardOf({ kind: "block", owner: "t1" }), "task", "a block opens its task")
            compare(host.cardOf({ kind: "step", owner: "t1" }), "task")
            compare(host.cardOf({ kind: "done", owner: "t1" }), "task")
            compare(host.cardOf({ kind: "meal" }), "need")
            compare(host.cardOf({ kind: "night" }), "need")
            compare(host.cardOf({ kind: "dose" }), "")
            verify(!host.show({ kind: "dose", srcKey: "lumiel@1" }), "no dose card")
            verify(!host.open)
        }

        // An appointment's card from its file: Edit where its writer allows it,
        // the reason in its place where it cannot change; Delete by its own writer.
        function test_an_event_s_card() {
            const host = makeHost()
            verify(host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 }))
            const card = host.eventCard
            verify(card !== null)
            compare(card.title, "Dentist")
            verify(root.find(card, "cardEdit").visible, "Edit offered")
            verify(!root.find(card, "cardWhy").visible)
            compare(card.stateLine, "agenda-kept", "the time kept before it, said")
            // Edit opens its own form, read by its own key.
            root.find(card, "cardEdit").clicked()
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare(mock.calls[0].what + " " + mock.calls[0].key, "eventForm /c/dentist.ics")
            mock.calls = []
            card.askDelete()
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare([mock.calls[0].what, mock.calls[0].key, mock.calls[0].start, mock.calls[0].onlyThis].join(" "), "deleteEvent /c/dentist.ics 4000000000 false")
            verify(!host.open, "deleted: the card closes")
            // A calendar that can only be read: no Edit, its reason in Edit's place, no Delete.
            host.show({ kind: "event", srcKey: "/c/holidays.ics", srcStart: 4000086400 })
            wait(20)
            verify(!root.find(host.eventCard, "cardEdit").visible)
            const why = root.find(host.eventCard, "cardWhy")
            verify(why.visible)
            compare(why.text, "This calendar can only be read.")
            verify(!root.find(host.eventCard, "eventCardDelete").visible)
            // Gone from its file: said so, nothing offered.
            host.show({ kind: "event", srcKey: "/c/gone.ics", srcStart: 1 })
            wait(20)
            compare(host.eventCard.title, "card-gone")
            verify(!root.find(host.eventCard, "cardEdit").visible)
        }

        // The card you came from, one level deep: Back goes to it, then closes.
        function test_back_goes_to_the_card_it_came_from() {
            const host = makeHost()
            host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
            wait(20)
            host.follow({ kind: "meal", srcDate: "2096-10-01", srcKey: "meal:1" })
            wait(20)
            compare(host.kind, "need")
            verify(host.previous !== null && host.previous.card === "event")
            verify(root.find(host.needCard, "cardBack").visible, "‹ Dentist, on a computer")
            host.back()
            compare(host.kind, "event")
            compare(host.eventCard.title, "Dentist")
            verify(host.previous === null)
            host.back()
            verify(!host.open, "then closed")
            // Opened from a page: nothing kept behind it.
            host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
            host.show({ kind: "meal", srcDate: "2096-10-01", srcKey: "meal:1" })
            verify(host.previous === null)
            verify(host.eventCard !== null, "a computer keeps the cards it made")
        }

        // A phone keeps the card shown and the one Back goes to, no other; and none once closed.
        function test_a_phone_keeps_what_it_shows() {
            shell.compact = true
            try {
                const host = makeHost()
                host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
                wait(20)
                host.follow({ kind: "meal", srcDate: "2096-10-01", srcKey: "meal:1" })
                wait(20)
                verify(host.eventCard !== null && host.needCard !== null, "Back's card kept")
                host.show({ kind: "meal", srcDate: "2096-10-01", srcKey: "meal:1" })
                wait(20)
                verify(host.eventCard === null, "opened from a page: the appointment's card unloaded")
                verify(host.needCard !== null)
                host.close()
                wait(20)
                verify(host.needCard === null, "closed: unloaded")
            } finally {
                shell.compact = false
            }
        }

        // A meal's card: that day's row, its actions by that day's writer; a day gone by said, nothing offered.
        function test_a_meal_s_card() {
            const host = makeHost()
            host.show({ kind: "meal", srcDate: "2096-10-01", srcKey: "meal:1" })
            wait(20)
            const card = host.needCard
            compare(card.title, "Lunch")
            verify(card.when.indexOf("12:10 – 13:00") >= 0, card.when)
            verify(root.find(card, "cardEdit").visible)
            const later = root.find(card, "needCardLater")
            verify(later.visible)
            // Edit opens that day's form for its times.
            root.find(card, "cardEdit").clicked()
            wait(20)
            const form = root.all(card, c => c.keptKey !== undefined)
            compare(form.length, 1)
            compare(form[0].keptKey, "need:2096-10-01|meal:1|times")
            verify(form[0].visible || form[0].opened, "its form open")
            form[0].close()
            later.clicked()
            compare(mock.calls.length, 1, JSON.stringify(mock.calls))
            compare([mock.calls[0].what, mock.calls[0].edit.date, mock.calls[0].edit.key, mock.calls[0].edit.action].join(" "), "changeNeed 2096-10-01 meal:1 later")
            // A day gone by: as it was, said so; no Edit, no action.
            mock.past = true
            card.read()
            verify(!root.find(card, "cardEdit").visible)
            verify(root.find(card, "cardWhy").visible)
            compare(root.find(card, "cardWhy").text, "need-past")
            verify(!root.find(card, "needCardLater").visible)
        }

        // What you typed in a form closed without Save: given back at its next
        // opening, said so; Save and Cancel forget it.
        function test_typed_text_is_kept() {
            const dialog = createTemporaryObject(dialogComponent, root, { keeper: keeper })
            dialog.edit("/c/dentist.ics", "")
            compare(dialog.keptLine, "")
            dialog.applyForm(Object.assign(dialog.form(), { title: "Dentist, the new one" }))
            dialog.close()
            verify(keeper.forms["/c/dentist.ics"] !== undefined, "kept when closed without Save")
            dialog.edit("/c/dentist.ics", "")
            compare(dialog.form().title, "Dentist, the new one", "given back")
            verify(dialog.keptLine.indexOf("card-typed-back") === 0, dialog.keptLine)
            dialog.save()
            compare(mock.calls[mock.calls.length - 1].edit.title, "Dentist, the new one")
            verify(keeper.forms["/c/dentist.ics"] === undefined, "saved: forgotten")
            dialog.edit("/c/dentist.ics", "")
            compare(dialog.form().title, "Dentist")
            dialog.applyForm(Object.assign(dialog.form(), { title: "Not this" }))
            const cancel = root.find(dialog.footer, "eventCancel")
            verify(cancel !== null)
            cancel.clicked()
            verify(keeper.forms["/c/dentist.ics"] === undefined, "cancelled: forgotten")
            // A meal's form for its day, the same.
            const form = createTemporaryObject(needFormComponent, root, { keeper: keeper })
            const day = { date: "2096-10-01", title: "Monday", today: true }
            const lunch = JSON.parse(mock.needItem("2096-10-01", "meal:1")).item
            form.edit("times", day, lunch)
            form.close()
            verify(keeper.forms["need:2096-10-01|meal:1|times"] === undefined, "nothing typed: nothing kept")
            form.edit("times", day, lunch)
            // Typed: its fields no longer what it read.
            form.loadedForm = ""
            form.close()
            verify(keeper.forms["need:2096-10-01|meal:1|times"] !== undefined, "typed (its fields differ from what it read): kept")
            form.edit("times", day, lunch)
            verify(form.keptLine.indexOf("card-typed-back") === 0)
        }

        // The objects a card keeps alive (Later 9 of the review of 3a2fbd4): the
        // host with an appointment's card open, then a meal's, within the counts
        // measured on 28 October 2026 and a tenth more.
        function test_the_cards_stay_within_their_budget() {
            const host = makeHost()
            host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
            wait(20)
            const event = root.count(host)
            host.show({ kind: "meal", srcDate: "2096-10-01", srcKey: "meal:1" })
            wait(20)
            const both = root.count(host)
            console.warn("sioul-objects: host with an event's card " + event + ", with a meal's too " + both)
            verify(event <= Math.ceil(budget.event * 1.1), "event: " + event)
            verify(both <= Math.ceil(budget.both * 1.1), "both: " + both)
        }
    }

    QtObject {
        id: budget

        // Measured on the stand-ins above, 28 October 2026.
        readonly property int event: 96
        readonly property int both: 156
    }
}
