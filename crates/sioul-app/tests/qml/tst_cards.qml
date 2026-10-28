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
// next opening over what changed since, gone with Cancel; a phone's card
// closing for a place opened; the task's card (TaskPanel, the window's
// module stood in for) saving a field typed in before it moves on, and
// making a new task. Run by tools/qml-test.sh.

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
        // A weekly appointment's two times, s1 and s2: its card read by its time.
        readonly property real s1: 4000172800
        readonly property real s2: 4000777600
        // What the event's form reads now (moved since, by a drag or another device).
        property string formStart: "2096-10-01T09:00"
        property string formEnd: "2096-10-01T10:00"
        // The meal's times that day as they are now.
        property string mealFrom: "12:10"
        property string mealTo: "13:00"
        property var events: ({
            "/c/dentist.ics": { id: "event:c/dentist.ics", key: "/c/dentist.ics", uid: "dentist", start: 4000000000, end: 4000003600, all_day: false, summary: "Dentist", when: "Monday · 09:00 – 10:00", before: 15, after: 0, location: "Rue Blanche", calendar: "Personal", color: null, recurring: false, cancelled: false, tentative: false, read_only: false, with_others: false, task: "", list: "", notes: "", organizer: "", attendees: [], why: "" },
            "/c/holidays.ics": { id: "event:c/holidays.ics", key: "/c/holidays.ics", uid: "holidays", start: 4000086400, end: 4000090000, all_day: false, summary: "Town fair", when: "Tuesday · 09:00 – 10:00", before: 0, after: 0, location: "", calendar: "Holidays", color: null, recurring: false, cancelled: false, tentative: false, read_only: true, with_others: false, task: "", list: "", notes: "", organizer: "", attendees: [], why: "This calendar can only be read." }
        })

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key }
        // An event's time by its start (0: its next), as `agenda::event_card` reads it.
        function eventCard(key, start) {
            if (key === "/c/yoga.ics") {
                const at = start === 0 ? mock.s1 : start
                if (at !== mock.s1 && at !== mock.s2)
                    return "{}"
                return JSON.stringify({ id: "event:c/yoga.ics@" + at, key: key, uid: "yoga", start: at, end: at + 3600, all_day: false, summary: "Yoga", when: at === mock.s1 ? "Wednesday" : "Next Wednesday", before: 0, after: 0, location: "", calendar: "Personal", color: null, recurring: true, cancelled: false, tentative: false, read_only: false, with_others: false, task: "", list: "", notes: "", organizer: "", attendees: [], why: "" })
            }
            const event = mock.events[key]
            return event && (start === 0 || start === event.start) ? JSON.stringify(event) : "{}"
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
            return JSON.stringify({ date: date, title: "Monday 5 October", today: !mock.past, past: mock.past, later: 15, item: { key: "meal:1", kind: "meal", name: "Lunch", from: mock.mealFrom, to: mock.mealTo, at: "12:30", detail: "eating from 12:30", note: "", changed: false, added: false, quiet: false, off: false, past: mock.past, minutes: 30, before: 20, after: 0, alarm: "", alarm_skipped: false, alarm_open: false } })
        }
        function changeNeed(edit) {
            mock.calls.push({ what: "changeNeed", edit: JSON.parse(edit) })
            return ""
        }
        function taskCardChoices() { return "{}" }
        function calendars() { return JSON.stringify([{ id: "local/personal", name: "Personal", color: null }]) }
        function eventForm(key) {
            mock.calls.push({ what: "eventForm", key: key })
            return JSON.stringify({ edit: { title: "Dentist", location: "Rue Blanche", notes: "", start: mock.formStart, end: mock.formEnd, all_day: false, repeat: "", margins: { before: 15, after: 0 }, demands: {}, remind: "" }, calendar: "local/personal", calendar_name: "Personal", here_only: "", stays: "", read_only: false })
        }
        function saveEvent(key, edit, calendar) {
            mock.calls.push({ what: "saveEvent", key: key, edit: JSON.parse(edit) })
            return ""
        }
        function eventToTaskLosses(key) { return "" }

        // Two tasks, as `task(uid)` reads them; a task made from the form is "t9".
        function taskDetail(uid, title) {
            return { card: { title: title, status: "needs-action", read_only: false, due: "", estimate: 30, has_steps: false, spent: 0, stopped: "", tight: false, unblocks: [], waits: [], done_on: "" }, edit: { title: title, notes: "", area: "", billable: false, categories: [], demands: {}, due: "", energy: "", estimate: 30, felt: {}, kind: "", margins: { before: 0, after: 0 }, office_hours: "", office_times: [], parent: "", projects: [], repeat: "", start: "" }, steps: [], steps_total: "", waits_for: [], frees: [], sessions: [], related: [], area_tags: "", level: "", felt_on: "", ratio_line: "", pin: null, pin_minutes: 0, list: "Tasks", lists: [], parent_title: "", appointment: null, budget: null, limited: null, proposed: null, source: null, keeps_ties: false }
        }
        function task(uid) {
            const titles = { t1: "Write the letter", t2: "Call the office", t9: "Book the room" }
            return titles[uid] ? JSON.stringify(mock.taskDetail(uid, titles[uid])) : ""
        }
        function newTaskForm(uri, key, start, list) {
            const blank = mock.taskDetail("", "")
            blank.card = null
            return JSON.stringify(blank)
        }
        function saveTask(uid, edit, list) {
            mock.calls.push({ what: "saveTask", uid: uid, edit: JSON.parse(edit) })
            return JSON.stringify(uid === "" ? { uid: "t9" } : {})
        }
        function searchTasks(text) { return "[]" }
        function searchThings(text) { return "[]" }
        function textCounted(key, n) { return key }
        function focusStart(uid, minutes) {}
    }

    // The window, as the cards see it.
    QtObject {
        id: shell

        property bool compact: false
        property var opened: []
        readonly property var sioulLocale: Qt.locale("en_GB")
        function openThing(item, fromCard) { shell.opened.push(item) }
        function menuAt(menu, item) {}
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

        // A phone keeps the card shown and the one Back goes to, no other, and
        // none once closed; but the task's card, once made, as the Tasks page
        // kept its panel (review of 590eb18, later 1).
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
                host.show({ kind: "step", owner: "t1" })
                wait(20)
                const panel = host.taskPanel
                verify(panel !== null)
                host.show({ kind: "meal", srcDate: "2096-10-01", srcKey: "meal:1" })
                wait(20)
                verify(host.taskPanel === panel, "the task's card kept beside another card")
                host.close()
                wait(20)
                verify(host.taskPanel === panel, "and once closed")
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
        // opening, said so, over the event as it is then (a time moved since
        // stays where it went: review of 590eb18, M2); Save and Cancel forget it.
        function test_typed_text_is_kept() {
            const dialog = createTemporaryObject(dialogComponent, root, { keeper: keeper })
            dialog.edit("/c/dentist.ics", "")
            compare(dialog.keptLine, "")
            const title = root.find(dialog.contentItem, "eventTitle")
            verify(title !== null)
            title.text = "Dentist, the new one"
            dialog.close()
            verify(keeper.forms["/c/dentist.ics"] !== undefined, "kept when closed without Save")
            const typedAlone = JSON.stringify(Object.keys(keeper.forms["/c/dentist.ics"].form))
            // Moved meanwhile (a drag, the other device): the new times stay.
            mock.formStart = "2096-10-01T11:00"
            mock.formEnd = "2096-10-01T12:00"
            dialog.edit("/c/dentist.ics", "")
            compare(dialog.form().title, "Dentist, the new one", "given back")
            compare(dialog.form().start, "2096-10-01T11:00", "its new time, not the one read before")
            verify(dialog.keptLine.indexOf("card-typed-back") === 0, dialog.keptLine)
            compare(typedAlone, JSON.stringify(["title"]), "the field typed alone kept")
            dialog.save()
            const saved = mock.calls[mock.calls.length - 1].edit
            compare(saved.title + " " + saved.start + " " + saved.end, "Dentist, the new one 2096-10-01T11:00 2096-10-01T12:00")
            verify(keeper.forms["/c/dentist.ics"] === undefined, "saved: forgotten")
            // Its times typed: start, end and all day kept as one.
            dialog.edit("/c/dentist.ics", "")
            compare(dialog.form().title, "Dentist")
            dialog.applyForm({ end: "2096-10-01T12:30" })
            dialog.close()
            compare(JSON.stringify(Object.keys(keeper.forms["/c/dentist.ics"].form).sort()), JSON.stringify(["all_day", "end", "start"]))
            dialog.edit("/c/dentist.ics", "")
            compare(dialog.form().end, "2096-10-01T12:30")
            const cancel = root.find(dialog.footer, "eventCancel")
            verify(cancel !== null)
            cancel.clicked()
            verify(keeper.forms["/c/dentist.ics"] === undefined, "cancelled: forgotten")
            // A meal's form for its day, the same.
            const form = createTemporaryObject(needFormComponent, root, { keeper: keeper })
            const day = { date: "2096-10-01", title: "Monday", today: true }
            const lunch = () => JSON.parse(mock.needItem("2096-10-01", "meal:1")).item
            form.edit("times", day, lunch())
            form.close()
            verify(keeper.forms["need:2096-10-01|meal:1|times"] === undefined, "nothing typed: nothing kept")
            form.edit("times", day, lunch())
            const before = root.all(form.contentItem, c => c.objectName === "needBefore")[0]
            verify(before !== undefined)
            before.value = before.value + 5
            form.close()
            const kept = keeper.forms["need:2096-10-01|meal:1|times"]
            verify(kept !== undefined, "typed: kept")
            // Moved meanwhile by a drag: its new times, the minutes typed.
            mock.mealFrom = "13:10"
            mock.mealTo = "14:00"
            form.edit("times", day, lunch())
            verify(form.keptLine.indexOf("card-typed-back") === 0)
            compare(form.typed().from + " " + form.typed().to + " " + form.typed().before, "13:10 14:00 25", "its new times, the minutes typed")
            compare(JSON.stringify(Object.keys(kept.form)), JSON.stringify(["before"]), "the field typed alone kept")
        }

        // On a phone the card covers the page: a place opened from it (a tie,
        // a notification) closes it, so that the place shows; on a computer it
        // stays beside the page (review of 590eb18, M3).
        function test_a_phone_s_card_leaves_for_a_place() {
            const host = makeHost()
            host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
            host.leave()
            verify(host.open, "a computer keeps it beside the page")
            shell.compact = true
            try {
                host.leave()
                verify(!host.open, "a phone closes it")
            } finally {
                shell.compact = false
            }
        }

        // A repeating appointment's card shows the time tapped, whatever card
        // was shown before; its menu acts on that time (review of 590eb18, M1).
        function test_an_appointment_s_card_follows_the_time_tapped() {
            const host = makeHost()
            host.show({ kind: "event", srcKey: "/c/yoga.ics", srcStart: mock.s2 })
            wait(20)
            compare(host.eventCard.event.start, mock.s2)
            host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
            wait(20)
            compare(host.eventCard.title, "Dentist")
            host.close()
            host.show({ kind: "event", srcKey: "/c/yoga.ics", srcStart: mock.s1 })
            wait(20)
            verify(host.eventCard.event !== null, "not gone")
            compare(host.eventCard.event.start, mock.s1)
            // Its menu on the other time, Delete, only this time: in the card, at that time.
            host.eventMenu({ srcKey: "/c/yoga.ics", srcStart: mock.s2 })
            const menu = root.all(host, c => c.inCard !== undefined && c.ask !== undefined)[0]
            verify(menu !== undefined)
            menu.close()
            menu.inCard(card => {
                compare(card.event.start, mock.s2, "the menu's own time in the card")
                card.deleteNow(true)
            })
            const asked = mock.calls.filter(c => c.what === "deleteEvent")
            compare(asked.length, 1, JSON.stringify(mock.calls))
            compare([asked[0].key, asked[0].start, asked[0].onlyThis].join(" "), "/c/yoga.ics " + mock.s2 + " true")
            // A reminder names its file and its time (`remind::event_time`; review of 590eb18, later 6).
            compare(JSON.stringify(host.eventTime("/c/yoga.ics#" + mock.s2)), JSON.stringify({ key: "/c/yoga.ics", start: mock.s2 }))
            compare(JSON.stringify(host.eventTime("/c/dentist.ics")), JSON.stringify({ key: "/c/dentist.ics", start: 0 }), "its file alone: its next time")
        }

        // The task's card: a field being typed in is saved before the card goes
        // to another task (review of 590eb18, later 9); a new task's form makes
        // the task when its title is given, and goes on as its card.
        function test_the_task_s_card() {
            const host = makeHost()
            host.show({ kind: "step", owner: "t1" })
            wait(50)
            const panel = host.taskPanel
            verify(panel !== null, "TaskPanel loaded")
            compare(panel.uid, "t1")
            panel.editing = true
            wait(20)
            const title = root.find(panel, "taskTitle")
            verify(title !== null && title.visible)
            title.forceActiveFocus()
            title.text = "Write the long letter"
            host.show({ kind: "step", owner: "t2" })
            wait(20)
            const saved = mock.calls.filter(c => c.what === "saveTask")
            compare(saved.length, 1, JSON.stringify(mock.calls))
            compare(saved[0].uid + " " + saved[0].edit.title, "t1 Write the long letter")
            compare(panel.uid, "t2")
            // A new task, from the host: made as its title is given.
            mock.calls = []
            compare(host.startNewTask(null), "")
            verify(host.ref.making)
            wait(20)
            title.text = "Book the room"
            title.editingFinished()
            compare(mock.calls.filter(c => c.what === "saveTask" && c.uid === "").length, 1, JSON.stringify(mock.calls))
            compare(host.ref.owner, "t9")
            verify(!host.ref.making)
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
            // A phone's host with a meal's card, the task's card kept behind it.
            shell.compact = true
            let phone = 0
            try {
                const small = makeHost()
                small.show({ kind: "step", owner: "t1" })
                wait(50)
                small.show({ kind: "meal", srcDate: "2096-10-01", srcKey: "meal:1" })
                wait(20)
                phone = root.count(small)
            } finally {
                shell.compact = false
            }
            console.warn("sioul-objects: host with an event's card " + event + ", with a meal's too " + both + "; a phone's with a meal's and the task's kept " + phone)
            verify(event <= Math.ceil(budget.event * 1.1), "event: " + event)
            verify(both <= Math.ceil(budget.both * 1.1), "both: " + both)
            verify(phone <= Math.ceil(budget.phoneWithTask * 1.1), "phone: " + phone)
        }
    }

    QtObject {
        id: budget

        // Measured on the stand-ins above, 28 October 2026.
        readonly property int event: 96
        readonly property int both: 156
        readonly property int phoneWithTask: 676
    }
}
