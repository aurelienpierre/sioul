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
            "/c/bus.ics": { id: "event:c/bus.ics", key: "/c/bus.ics", uid: "bus", start: 3999997800, end: 3999999300, all_day: false, summary: "Bus 24", when: "Monday · 08:30 – 08:55", before: 0, after: 0, location: "", calendar: "Personal", color: null, recurring: false, cancelled: false, tentative: false, read_only: false, with_others: false, task: "", list: "", notes: "", organizer: "", attendees: [], why: "" },
            "/c/dentist.ics": { id: "event:c/dentist.ics", key: "/c/dentist.ics", uid: "dentist", start: 4000000000, end: 4000003600, all_day: false, summary: "Dentist", when: "Monday · 09:00 – 10:00", before: 15, after: 0, location: "Rue Blanche", calendar: "Personal", color: null, recurring: false, cancelled: false, tentative: false, read_only: false, with_others: false, task: "", list: "", notes: "", organizer: "", attendees: [], why: "" },
            "/c/board.ics": { id: "event:c/board.ics", key: "/c/board.ics", uid: "board", start: 4000172800, end: 4000176400, all_day: false, summary: "Board", when: "Wednesday · 09:00 – 10:00", before: 0, after: 0, location: "", calendar: "Personal", color: null, recurring: false, cancelled: false, tentative: false, read_only: false, with_others: true, task: "", list: "", notes: "", organizer: "chair@example.org", attendees: [], why: "" },
            "/c/holidays.ics": { id: "event:c/holidays.ics", key: "/c/holidays.ics", uid: "holidays", start: 4000086400, end: 4000090000, all_day: false, summary: "Town fair", when: "Tuesday · 09:00 – 10:00", before: 0, after: 0, location: "", calendar: "Holidays", color: null, recurring: false, cancelled: false, tentative: false, read_only: true, with_others: false, task: "", list: "", notes: "", organizer: "", attendees: [], why: "This calendar can only be read." }
        })

        function text(key) { return key }
        function zoneName() { return "Europe/Paris" }
        function clockLine(start, end, clock) { return "" }
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
        // Part F's view of an appointment: the dentist's, when to leave and its next step.
        function appointmentView(key, start) {
            // A leg's own card: what it is to its appointment (part E).
            if (key === "/c/bus.ics")
                return JSON.stringify({ uid: "bus", key: key, title: "Bus 24", line: "", before: [], more: 0, bring: [], getting_there: [], coming_back: [], steps_after: [], also: [], leg_of: { uid: "dentist", key: "/c/dentist.ics", title: "Dentist", start: 4000000000, side: "before", unconfirmed: true, line: "Before Dentist (09:00) · moves with it" } })
            if (key !== "/c/dentist.ics")
                return "{}"
            return JSON.stringify({ uid: "dentist", key: key, title: "Dentist", leave_at: 3999999100, line: "Leave at 08:45 · Next: Find the card", preparation: { uid: "prep", title: "Prepare the dentist" }, before: [{ uid: "card", title: "Find the card" }], more: 2, bring: [{ name: "Bring", items: [{ task: "prep", line: 3, text: "Carte Vitale", done: false }], more: 0 }], getting_there: mock.withLegs ? [{ uid: "bus", key: "/c/bus.ics", title: "Bus 24", start: 3999997800, end: 3999999300, when: "08:30 – 08:55", unconfirmed: true }] : [], coming_back: mock.withLegs ? [{ uid: "home", key: "/c/home.ics", title: "Walk home", start: 4000004200, end: 4000005400, when: "10:10 – 10:30", unconfirmed: false }] : [], steps_after: mock.withLegs ? [{ uid: "next", title: "Book the next one" }] : [], also: mock.withLegs ? [{ uid: "papers", title: "The papers" }] : [], set_aside: mock.withAside ? [{ uid: "early", title: "Send the prescription" }] : [], leg_of: null })
        }
        property bool withLegs: false
        property bool withAside: false
        function bringBack(uid) {
            mock.calls.push({ what: "bringBack", uid: uid })
            return JSON.stringify({ uid: uid })
        }
        function saveLeg(key, edit, side, timetable, step) {
            mock.calls.push({ what: "saveLeg", key: key, edit: JSON.parse(edit), side: side, timetable: timetable, step: step })
            return ""
        }
        // The steps a leg may be the time of (part 7): offered once every device keeps such a leg.
        property bool stepsOffered: false
        function legSteps(key) {
            const steps = [{ uid: "s1", title: "Take the bus" }]
            return JSON.stringify(mock.stepsOffered ? { offered: true, steps: steps, why: "" } : { offered: false, steps: [], why: "leg-step-not-yet" })
        }
        function untieLeg(key) {
            mock.calls.push({ what: "untieLeg", key: key })
            return ""
        }
        function invitationOf(key) { return key === "/c/board.ics" ? "/mail/board.eml" : "" }
        function answerInvitation(key, answer) { mock.calls.push({ what: "answerInvitation", key: key, answer: answer }) }
        function checkItem(kind, key, line, state) {
            mock.calls.push({ what: "checkItem", kind: kind, key: key, line: line, state: state })
            return ""
        }
        // A date asked's card (`datedCard`): a letter read from its scan, a paper, an invoice.
        function datedCard(kind, key) {
            if (kind === "letter" && key === "caf-1")
                return JSON.stringify({ there: true, kind: kind, key: key, title: "CAF", mark: "Date asked · four days left", when: "Friday 30 October", filled: true, read: true, lines: ["A request", "By Friday 30 October (“within two months”)"], edit: mock.letterTask ? "task" : "letter-dates", task: mock.letterTask, task_open: mock.letterTask !== "", into_agenda: true, can_task: true, file: "/notes/letters/caf.pdf", project: "", item: { deadline: "2096-10-30", appointment: "", time: "" } })
            if (kind === "invoice" && key === "2026-002")
                return JSON.stringify({ there: true, kind: kind, key: key, title: "2026-002 · Fernhill · €480", mark: "Invoice due · tomorrow", when: "Tuesday", filled: false, lines: [], edit: "none", why: "dated-invoice-record", number: "2026-002", paid: false })
            return JSON.stringify({ there: false })
        }
        property string letterTask: ""
        function letterDates(id, edit) {
            mock.calls.push({ what: "letterDates", id: id, edit: JSON.parse(edit) })
            return ""
        }
        function setInvoicePaid(number, paid) {
            mock.calls.push({ what: "setInvoicePaid", number: number, paid: paid })
            return ""
        }

        // Two tasks, as `task(uid)` reads them; a task made from the form is "t9".
        function taskDetail(uid, title) {
            return { card: { title: title, status: "needs-action", read_only: false, due: "", estimate: 30, has_steps: false, spent: 0, stopped: "", tight: false, unblocks: [], waits: [], done_on: "" }, edit: { title: title, notes: "", area: "", billable: false, categories: [], demands: {}, due: "", energy: "", estimate: 30, felt: {}, kind: "", margins: { before: 0, after: 0 }, office_hours: "", office_times: [], parent: "", projects: [], repeat: "", start: "" }, steps: [], steps_total: "", waits_for: [], frees: [], sessions: [], related: [], area_tags: "", level: "", felt_on: "", ratio_line: "", pin: null, pin_minutes: 0, list: "Tasks", lists: [], parent_title: "", appointment: null, budget: null, limited: null, proposed: null, source: null, keeps_ties: false }
        }
        function task(uid) {
            const titles = { t1: "Write the letter", t2: "Call the office", t9: "Book the room", t3: "Going to the doctor" }
            if (uid === "t3") {
                // A composed task (§7.2): its appointment, two steps, a list to tick in its notes.
                const detail = mock.taskDetail(uid, titles[uid])
                detail.card.has_steps = true
                detail.edit.notes = "Bring:\n- [ ] Health card"
                detail.edit.margins = { before: 10, after: 0 }
                detail.card.due = "Asked for Thursday 29 October"
                const step = (id, title) => ({ uid: id, title: title, status: "needs-action", due: "", estimate: "", steps: "", waits: "", stopped: "", list: "", done_on: "", read_only: false, has_steps: false })
                detail.steps = [step("s1", "Brush teeth"), step("s2", "Check the bag")]
                detail.appointment = { uid: "dentist", key: "/c/dentist.ics", title: "Doctor", when: "on Thursday 29 October at 14:00", here: true, leave_at: 0, leave: "Leave at 13:10 · Bus 24", line: "" }
                return JSON.stringify(detail)
            }
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
        function descriptionHtml(markdown, tickable) { return "<p>" + markdown + "</p>" }
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
        property int agendaAsked: 0
        function openThing(item, fromCard) { shell.opened.push(item) }
        function agendaAgain() { shell.agendaAsked += 1 }
        function openPage(kind, key) { shell.opened.push({ kind: kind, key: key }) }
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
        id: taskRowComponent

        TaskRow {
            theme: testTheme
            sioul: mock
        }
    }

    Component {
        id: timelineComponent

        TaskTimeline {
            sioul: mock
            theme: testTheme
            today: "2099-01-05"
            opened: ""
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
            mock.withLegs = false
            mock.withAside = false
            mock.letterTask = ""
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
            // Its times typed: start, end, all day and the clock they are on kept as one.
            dialog.edit("/c/dentist.ics", "")
            compare(dialog.form().title, "Dentist")
            dialog.applyForm({ end: "2096-10-01T12:30" })
            dialog.close()
            compare(JSON.stringify(Object.keys(keeper.forms["/c/dentist.ics"].form).sort()), JSON.stringify(["all_day", "clock", "end", "start"]))
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

        // A composed task's details (§7.2): its appointment at the top with when to leave, its
        // steps in order, the checklist under them; Edit turns the steps into drag handles.
        function test_a_composed_task_s_details() {
            const host = makeHost()
            host.show({ kind: "step", owner: "t3" })
            wait(50)
            const panel = host.taskPanel
            const place = name => { const item = root.find(panel, name); return item ? item.mapToItem(panel, 0, 0).y : -1 }
            const steps = root.all(panel, c => c.objectName === "taskStep" && c.visible)
            compare(steps.length, 2)
            const leave = root.find(panel, "taskAppointmentLeave")
            verify(leave.visible && leave.text === "Leave at 13:10 · Bus 24", "when to leave")
            const notes = root.all(panel, c => c.markdown !== undefined && c.visible)[0]
            verify(notes, "its notes")
            const at = item => item.mapToItem(panel, 0, 0).y
            verify(place("taskAppointment") < at(steps[0]), "the appointment above the steps")
            verify(at(steps[1]) < at(notes), "the checklist under the steps")
            const fact = root.all(panel, c => c.text === "task-field-before" && c.visible)[0]
            verify(fact && at(notes) < at(fact), "right under them, before its details")
            const said = root.all(panel, c => c.text === "Asked for Thursday 29 October" && c.visible)[0]
            verify(said && place("taskAppointment") < at(said), "at the top, above what it says of itself")
            verify(!root.find(panel, "taskRowHandle"), "no handle outside Edit")
            panel.editing = true
            wait(20)
            compare(root.all(panel, c => c.objectName === "taskRowHandle" && c.visible).filter((c, i, all) => all.indexOf(c) === i).length, 2, "Edit: a handle on each step")
            panel.editing = false
        }

        // A date asked's card (the unified agenda, part 4): a letter's, read from
        // its scan, its mark filled within seven days; Edit opens its own dates'
        // form while it has neither task nor event, saved by `letterDates`; with
        // its task, Edit opens the task's card. A task's date opens its task.
        // An invoice is its own record: no Edit, its reason; Paid by its writer.
        function test_a_date_asked_s_card() {
            const host = makeHost()
            compare(host.cardOf({ kind: "dated", datedKind: "task", owner: "t1", srcKey: "t1" }), "task", "a task's date opens its task")
            compare(host.cardOf({ kind: "dated", datedKind: "letter", srcKey: "caf-1" }), "dated")
            verify(host.show({ kind: "dated", datedKind: "letter", srcKey: "caf-1" }))
            wait(20)
            const card = host.datedCard
            verify(card !== null)
            compare(card.title, "CAF")
            verify(root.find(card, "datedMark").color !== "transparent", "filled within seven days")
            verify(root.find(card, "cardEdit").visible)
            root.find(card, "cardEdit").clicked()
            wait(20)
            const form = root.all(card, c => c.letterId !== undefined)[0]
            verify(form !== undefined && form.opened, "its own dates' form")
            const deadline = root.all(form.contentItem, c => c.objectName === "letterDeadline")[0]
            compare(deadline.text, "2096-10-30")
            deadline.text = "2096-11-03"
            root.all(form.footer, c => c.objectName === "letterDatesSave")[0].clicked()
            const asked = mock.calls.filter(c => c.what === "letterDates")
            compare(asked.length, 1, JSON.stringify(mock.calls))
            compare(asked[0].id + " " + asked[0].edit.deadline, "caf-1 2096-11-03")
            verify(shell.agendaAsked > 0, "the agenda's marks follow")
            // With its task: Edit opens the task's card.
            mock.letterTask = "t7"
            card.read()
            root.find(card, "cardEdit").clicked()
            compare(host.kind, "task")
            compare(host.ref.owner, "t7")
            // An invoice: no Edit, its reason in its place; Paid by its own writer.
            host.show({ kind: "dated", datedKind: "invoice", srcKey: "2026-002" })
            wait(20)
            verify(!root.find(host.datedCard, "cardEdit").visible)
            compare(root.find(host.datedCard, "cardWhy").text, "dated-invoice-record")
            verify(root.find(host.datedCard, "datedMark").color === Qt.rgba(0, 0, 0, 0) || String(root.find(host.datedCard, "datedMark").color) === "#00000000", "never filled")
        }

        // A meeting's card (part F's agenda half): when to leave and its next
        // step, Before and Bring, a tick by its task's own writer; Answer… while
        // its invitation is in your mail.
        // Part E's legs on an appointment's card: Getting there and Coming back in time
        // order, a doubt said, the steps after it and another task tied to it; "Add a ride
        // or a walk…" opens the event form as a leg, saved through `saveLeg`; a leg's own
        // card says what it is to its appointment, and × unties it.
        function test_an_appointment_s_legs() {
            mock.withLegs = true
            const dentist = mock.events["/c/dentist.ics"]
            const host = makeHost()
            host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
            wait(20)
            const card = host.eventCard
            // Each once: a Loader lists what it made among its children twice.
            const legs = root.all(card, c => c.objectName === "eventCardLeg" && c.visible).filter((l, i, all) => all.indexOf(l) === i)
            compare(legs.map(l => l.text).join(" | "), "08:30 – 08:55  legs-unconfirmed:Bus 24 | 10:10 – 10:30  Walk home")
            const once = list => list.filter((l, i, all) => all.indexOf(l) === i)
            compare(once(root.all(card, c => c.text === "·  Book the next one" && c.visible)).length, 1, "the step after, under Coming back")
            compare(once(root.all(card, c => c.text === "appointment-also-tied:The papers" && c.visible)).length, 1)
            const add = root.find(card, "eventCardAddLeg")
            verify(add.visible)
            add.clicked()
            wait(20)
            const dialog = root.all(card, c => typeof c.now === "function").map(l => l.item).find(d => d && typeof d.makeLeg === "function" && d.legOf !== "")
            verify(dialog !== undefined, "the form, as a leg")
            compare([dialog.legOf, dialog.legSide].join(" "), "/c/dentist.ics before")
            // A leg happens once, with its appointment: no repeat, no calendar to choose (review of ab7daa4, later 7).
            dialog.moreShown = true
            verify(!root.find(dialog.contentItem, "eventRepeat").visible && !root.find(dialog.contentItem, "event-calendar-row").visible)
            root.find(dialog.contentItem, "eventTitle").text = "Bus 24"
            root.find(dialog.contentItem, "legTimetable").toggle()
            root.find(dialog.contentItem, "legTimetable").toggled()
            // Not every device keeps a leg that is a step's time yet: no step offered, why said (part 7).
            verify(!root.find(dialog.contentItem, "legStep").visible, "no step offered")
            compare(root.find(dialog.contentItem, "legStepWhy").text, "leg-step-not-yet")
            // Its times after the appointment's end: after it, as its times say.
            const two = n => (n < 10 ? "0" : "") + n
            const local = s => { const d = new Date(s * 1000); return d.getFullYear() + "-" + two(d.getMonth() + 1) + "-" + two(d.getDate()) + "T" + two(d.getHours()) + ":" + two(d.getMinutes()) }
            dialog.applyForm({ start: local(4000003600 + 600), end: local(4000003600 + 1800) })
            compare(dialog.legSide, "after")
            verify(dialog.legLine.indexOf("leg-after") === 0, dialog.legLine)
            dialog.save()
            const saved = mock.calls.filter(c => c.what === "saveLeg")
            compare(saved.length, 1, JSON.stringify(mock.calls))
            compare([saved[0].key, saved[0].edit.title, saved[0].side, saved[0].timetable, saved[0].step].join(" "), "/c/dentist.ics Bus 24 after true ")
            // Every device keeps it: the leg may be a step's time, chosen in its form.
            mock.stepsOffered = true
            dialog.makeLeg("/c/dentist.ics", "Dentist", 4000000000, 4000003600, "2096-10-02", "08:30", "08:55")
            const combo = root.find(dialog.contentItem, "legStep")
            verify(combo.visible, "its step offered")
            compare(combo.count, 2, "none, or the step")
            combo.activated(1)
            compare(dialog.legStep, "s1")
            root.find(dialog.contentItem, "eventTitle").text = "Bus 24"
            dialog.save()
            compare(mock.calls.filter(c => c.what === "saveLeg").map(c => c.step).join(","), ",s1")
            mock.stepsOffered = false
            // The leg's own card.
            host.show({ kind: "leg", srcKey: "/c/bus.ics", srcStart: 3999997800 })
            wait(20)
            const of = root.find(host.eventCard, "eventCardLegOf")
            verify(of.visible)
            verify(root.all(of, c => c.text === "Before Dentist (09:00) · moves with it").length >= 1)
            verify(!root.find(host.eventCard, "eventCardAddLeg").visible, "a leg takes no leg")
            root.find(host.eventCard, "eventCardUntieLeg").clicked()
            compare(mock.calls.filter(c => c.what === "untieLeg").map(c => c.key).join(" "), "/c/bus.ics")
            // Nor an appointment cancelled or declined, nor a block opened as an event (review of ab7daa4, later 15).
            for (const changed of [{ cancelled: true }, { declined: true }, { task: "t1" }]) {
                mock.events["/c/dentist.ics"] = Object.assign({}, dentist, changed)
                host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
                wait(20)
                verify(!root.find(host.eventCard, "eventCardAddLeg").visible, JSON.stringify(changed))
            }
            mock.events["/c/dentist.ics"] = dentist
            mock.withLegs = false
        }

        // The way back (review of ab7daa4, M1): what an appointment over set aside by itself is
        // listed on its card, each with Bring back, which asks for it by its own UID.
        function test_what_an_appointment_set_aside_comes_back() {
            mock.withAside = true
            const host = makeHost()
            host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
            wait(20)
            const back = root.all(host.eventCard, c => c.objectName === "eventCardBringBack" && c.visible)
            verify(back.length >= 1, "Bring back offered")
            verify(root.all(host.eventCard, c => c.text === "Send the prescription" && c.visible).length >= 1)
            back[0].clicked()
            compare(mock.calls.filter(c => c.what === "bringBack").map(c => c.uid).join(" "), "early")
        }

        // The way back that needs no card (review of 4d7bab4, later 8): a task set aside by an
        // appointment over, listed with the done tasks, says so and brings itself back; a task
        // done, or one you only read, offers nothing.
        function test_a_task_set_aside_comes_back_from_the_done_tasks() {
            const task = { uid: "early", title: "Find the card", status: "cancelled", due: "", estimate: "", steps: "", waits: "", stopped: "", list: "", done_on: "", read_only: false, has_steps: false, set_aside: "Set aside Thursday 29 October, its appointment over" }
            const row = createTemporaryObject(taskRowComponent, root, { task: task, width: 600 })
            const back = root.find(row, "taskRowBringBack")
            verify(back.visible, "Bring back offered")
            verify(root.all(row, c => c.text === task.set_aside && c.visible).length >= 1, "said so")
            const before = mock.calls.length
            back.clicked()
            compare(mock.calls.slice(before).filter(c => c.what === "bringBack").map(c => c.uid).join(" "), "early")
            const done = createTemporaryObject(taskRowComponent, root, { task: Object.assign({}, task, { status: "completed", set_aside: "", done_on: "Done Thursday 29 October" }), width: 600 })
            verify(!root.find(done, "taskRowBringBack"), "done: nothing to bring back, nothing made")
            const read = createTemporaryObject(taskRowComponent, root, { task: Object.assign({}, task, { read_only: true }), width: 600 })
            verify(!root.find(read, "taskRowBringBack").visible, "read only: nothing")
        }

        // The fold (composed tasks §7.2): a task with steps is one row in List and Board, its line
        // its appointment's time and its next step, never a count; its arrow opens its steps; the
        // Gantt's bar has its own; a view that does not fold makes no arrow.
        function test_a_task_with_steps_folds_into_one_row() {
            const task = { uid: "prep", title: "Going to the doctor", status: "needs-action", due: "", estimate: "", steps: "2 of 3 steps left", waits: "", stopped: "", list: "", done_on: "", read_only: false, has_steps: true, folded: true, fold_line: "Mon 5 Jan 14:00 · next: Brush teeth" }
            const row = createTemporaryObject(taskRowComponent, root, { task: task, width: 600, folds: true })
            const folded = []
            row.fold.connect((uid, open) => folded.push(uid + " " + open))
            const arrow = root.find(row, "taskRowFold")
            verify(arrow && arrow.visible, "its arrow")
            compare(arrow.text, "▸")
            verify(root.all(row, c => c.text === task.fold_line && c.visible).length >= 1, "its line")
            verify(root.all(row, c => typeof c.text === "string" && c.text.indexOf("steps left") >= 0 && c.visible).length === 0, "no count")
            arrow.clicked()
            compare(folded.join(","), "prep true")
            const plain = createTemporaryObject(taskRowComponent, root, { task: task, width: 600 })
            verify(!root.find(plain, "taskRowFold"), "a view that does not fold: no arrow")
            const bars = createTemporaryObject(timelineComponent, root, { width: 800, height: 300, timeline: { days: [{ date: "2099-01-05", day: "5", weekday: "Mon", month: "January 2099", rest: false }], rows: [{ uid: "prep", key: "", title: "Going to the doctor", depth: 0, start: 0, length: 1, due: null, tight: false, column: "ready", has_steps: true, folded: true, color: null, detail: "" }], note: "" } })
            const opened = []
            bars.fold.connect((uid, open) => opened.push(uid + " " + open))
            wait(20)
            const barArrow = root.find(bars, "timelineFold")
            verify(barArrow, "the bar's arrow")
            barArrow.clicked()
            compare(opened.join(","), "prep true")
        }

        function test_an_appointment_s_part_f() {
            const host = makeHost()
            host.show({ kind: "event", srcKey: "/c/dentist.ics", srcStart: 4000000000 })
            wait(20)
            const card = host.eventCard
            compare(root.find(card, "eventCardLeave").text, "Leave at 08:45 · Next: Find the card")
            const bring = root.all(card, c => c.objectName === "eventCardBring")
            compare(bring.length, 1)
            bring[0].toggle()
            bring[0].toggled()
            const ticked = mock.calls.filter(c => c.what === "checkItem")
            compare(ticked.length, 1, JSON.stringify(mock.calls))
            compare([ticked[0].kind, ticked[0].key, ticked[0].line, ticked[0].state].join(" "), "task prep 3 done")
            verify(!root.find(card, "eventCardAnswer").visible, "no invitation in the mail")
            // A meeting with others whose invitation is in the mail: Answer… shows, and sends
            // your answer to that message (review of part 4, later 12).
            host.show({ kind: "event", srcKey: "/c/board.ics", srcStart: 4000172800 })
            wait(20)
            const answer = root.find(host.eventCard, "eventCardAnswer")
            verify(answer.visible, "its invitation in the mail")
            answer.clicked()
            wait(20)
            const later = root.all(answer, c => typeof c.now === "function")
            compare(later.length, 1)
            const menu = later[0].item
            verify(menu !== null && menu.count === 3, "accept, maybe, decline")
            menu.itemAt(0).triggered()
            const answered = mock.calls.filter(c => c.what === "answerInvitation")
            compare(answered.length, 1, JSON.stringify(mock.calls))
            compare([answered[0].key, answered[0].answer].join(" "), "/mail/board.eml accepted")
            menu.close()
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

        // Measured on the stand-ins above, 28 October 2026; the appointment's card
        // with part 4's Leave at, Before (one step), Bring (one group) and Answer…
        // (its menu made at its first press): 96 before them.
        readonly property int event: 118
        readonly property int both: 178
        readonly property int phoneWithTask: 677
    }
}
