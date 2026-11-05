// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An event's form: the title, whole days or hours, when, where; folded
// underneath, notes, how it repeats, the time kept before and after it
// (getting there, getting ready, coming back), when it is reminded (as
// usual, not this one, or so long before: docs/reminders.md), what it costs
// and gives back (four costs and a gain as tiles, 0 to 10, unsaid until
// said: CostTiles.qml), and the calendar when there are several. Opened from an event's details ("Edit"),
// or new.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme
    // The event's file; empty for a new one.
    property string key: ""
    // Its place's point, "48.856600;2.352200" (`GEO`), set when a suggested address is chosen,
    // gone when the place is typed again; "" none.
    property string geo: ""
    // The addresses suggested for the place typed: [{label, lat, lon}] (`suggestAddresses`),
    // and the one Up and Down are on (-1: none, the field's own text).
    property var suggestions: []
    property int suggestionAt: -1
    // "What you type in a place is sent to … to suggest addresses.", the first time only.
    property string suggestSaid: ""

    // A guest added from what is typed or a suggestion ("Anne <anne@example.org>",
    // or an address): once each; what does not read as an address stays typed.
    function addGuest(typed) {
        const text = String(typed || "").trim()
        const angled = text.match(/^(.*)<([^<>\s]+@[^<>\s]+)>$/)
        const address = angled ? angled[2].trim() : text.replace(/^mailto:/i, "")
        const name = angled ? angled[1].trim().replace(/^"|"$/g, "") : ""
        if (!/^[^@\s,;<>"]+@[^@\s,;<>"]+\.[^@\s,;<>"]+$/.test(address))
            return false
        if (!dialog.guests.some(g => g.address.toLowerCase() === address.toLowerCase()))
            dialog.guests = dialog.guests.concat([{ address: address, name: name }])
        dialog.guestSuggestions = []
        dialog.guestAt = -1
        return true
    }

    function removeGuest(index) {
        dialog.guests = dialog.guests.filter((g, i) => i !== index)
    }

    // For the window's tests: as if typed in "Invite…".
    function typeGuest(text) {
        invite.forceActiveFocus()
        invite.text = text
        invite.textEdited()
    }

    // A suggestion chosen: its words in the place, its point kept on this device, and
    // written into the event (its GEO) when the event is yours to write.
    function choosePlace(suggestion) {
        place.text = suggestion.label
        dialog.suggestions = []
        if (suggestion.lat === undefined)
            return
        dialog.sioul.keepPoint(suggestion.label, suggestion.lat, suggestion.lon)
        if (!dialog.withOthers)
            dialog.geo = suggestion.lat.toFixed(6) + ";" + suggestion.lon.toFixed(6)
    }
    // A task being made an appointment ("Make it an appointment", 21 October
    // 2026): its UID, and the file of the time block it was opened from.
    property string taskUid: ""
    property string taskBlock: ""
    // A leg of the appointment in `legOf` (part E5: "Add a ride or a walk…"),
    // `legTitle` from `legAt` to `legUntil` (Unix seconds), from a timetable or
    // not. Its side is its times': after the appointment once it starts from its
    // end, else before (review of ab7daa4, later 7); `legLine` says it ("Before
    // Doctor (14:00)").
    property string legOf: ""
    property string legTitle: ""
    property real legAt: 0
    property real legUntil: 0
    property bool legTimetable: false
    // The steps the leg may be the time of (part 7): {offered, steps: [{uid, title}], why}; the one chosen, "" for none.
    property var legSteps: ({ offered: false, steps: [], why: "" })
    property string legStep: ""
    readonly property string legSide: dialog.legOf !== "" && dialog.legStart() >= dialog.legUntil ? "after" : "before"
    readonly property string legLine: dialog.legOf === "" ? "" : dialog.sioul.textArgs(dialog.legSide === "after" ? "leg-after" : "leg-before", JSON.stringify({ appointment: dialog.legTitle, time: dialog.clockOf(dialog.legSide === "after" ? dialog.legUntil : dialog.legAt) }))

    // An instant (ms) as a date whose own clock reads the time zone Sioul shows
    // (`zone::current`, Settings ▸ Agenda ▸ Time zone; `zoneOffset`, JavaScript's own offset back).
    function wall(ms) {
        return new Date(ms + (typeof dialog.sioul.zoneOffset === "function" ? dialog.sioul.zoneOffset(ms / 1000) + new Date(ms).getTimezoneOffset() * 60000 : 0))
    }

    // The start typed, in Unix seconds, on the form's clock: the time zone Sioul shows,
    // or the one a leg's form is on (review of 4e065b1, L1); NaN while it does not read.
    function legStart() {
        if (dialog.clock !== "" && typeof dialog.sioul.wallInstant === "function")
            return dialog.sioul.wallInstant(startDay.date + "T" + startTime.text, dialog.clock)
        const typed = new Date(startDay.date + "T" + startTime.text).getTime()
        return (typed - (dialog.wall(typed).getTime() - typed)) / 1000
    }

    function clockOf(seconds) {
        const d = dialog.wall(seconds * 1000)
        return dialog.pad(d.getHours()) + ":" + dialog.pad(d.getMinutes())
    }
    // What the task cannot keep as an appointment, said before Save (F5); else "".
    property string losses: ""
    property var calendars: []
    // The calendar of the event opened ("account/id"), its name, and whether it stays on this device alone.
    property string currentCalendar: ""
    property string calendarName: ""
    property string hereOnly: ""
    // Why it stays in its calendar: a meeting with others (F2, 22 October 2026); else "".
    property string stays: ""
    // A meeting with others (Rust's one rule, `agenda::may_change`): its time,
    // title, place, notes and repeat are its organizer's and are not offered;
    // your time around it, its costs and your reminder are.
    property bool withOthers: false
    // A meeting you organize (decision 7 of the event card): every field is
    // yours, and its guests, written through its own door.
    property bool organized: false
    // Its guests, as "Invite…" lists them: [{address, name}].
    property var guests: []
    // The guests suggested for what is typed (`guestCompletions`, this
    // device's address books and Your people), and the one Up and Down are on.
    property var guestSuggestions: []
    property int guestAt: -1
    // Saving tells someone (`inviteSaid`): asked once, then saved.
    property bool inviteConfirmed: false
    property bool moreShown: false
    property string problem: ""
    // The clock its times are on (`EventEdit::clock`): "" the time zone shown
    // (`zoneName`), "UTC", or another zone by its name, kept by an edit;
    // `clockOther` while "Another time zone…" is chosen, picked or not yet.
    property string clock: ""
    property bool clockOther: false
    property string zoneName: ""
    // What a new event is made from: links it carries ("mid:…", "sioul:task/…").
    property var links: []
    readonly property var repeats: ["", "daily", "weekly", "monthly", "yearly"]
    readonly property var marginMinutes: [0, 5, 10, 15, 20, 30, 45, 60, 90, 120]
    // The choices, with the event's own when another program wrote one not
    // among them: what it keeps always shows, never as "unsaid" (22 October 2026).
    readonly property var marginChoices: dialog.marginMinutes.concat([dialog.around.before, dialog.around.after].filter(m => m > 0 && dialog.marginMinutes.indexOf(m) < 0)).sort((a, b) => a - b).filter((m, i, all) => all.indexOf(m) === i)
    // Minutes before and after; the four costs and the gain, 0 to 10 or unsaid (null).
    property var around: ({ before: 0, after: 0 })
    property var demands: ({ cognitive: null, emotional: null, anxiety: null, body: null, gain: null })
    // Its reminder before it: "" as usual, "none", or minutes ("15"); and the choices offered.
    property string remind: ""
    property var reminds: []

    signal saved

    // What you typed, kept when the form closes without Save (Escape, Back, a
    // press outside) and given back at its next Edit, until you save or cancel
    // (the unified agenda, part 3): the card host's, {forms: {key: {form, at}}}.
    // Only the fields typed, over the event as it is read then: a time moved
    // since (a drag, the other device, its organizer) stays where it went, but
    // for times you typed yourself (review of 590eb18, M2).
    property var keeper: null
    property string loadedForm: ""
    property bool closedBySave: false
    property bool cancelled: false
    property string keptLine: ""

    // The fields of `form` that differ from what was read (`loadedForm`); its
    // start, its end and "all day" one group: one typed, the three kept.
    function typedFields(form) {
        const loaded = JSON.parse(dialog.loadedForm || "{}")
        const typed = {}
        for (const name in form) {
            if (JSON.stringify(form[name]) !== JSON.stringify(loaded[name]))
                typed[name] = form[name]
        }
        if (typed.start !== undefined || typed.end !== undefined || typed.all_day !== undefined || typed.clock !== undefined) {
            typed.start = form.start
            typed.end = form.end
            typed.all_day = form.all_day
            typed.clock = form.clock
        }
        return typed
    }

    function keep() {
        if (!dialog.keeper || dialog.key === "" || dialog.loadedForm === "" || dialog.closedBySave || dialog.cancelled)
            return
        const typed = dialog.typedFields(dialog.form())
        // Typed back to what it read: nothing kept.
        if (Object.keys(typed).length === 0) {
            dialog.forget()
            return
        }
        const forms = Object.assign({}, dialog.keeper.forms)
        forms[dialog.key] = { form: typed, at: Qt.formatTime(dialog.wall(Date.now()), "HH:mm") }
        dialog.keeper.forms = forms
    }

    function forget() {
        if (!dialog.keeper || !dialog.keeper.forms[dialog.key])
            return
        const forms = Object.assign({}, dialog.keeper.forms)
        delete forms[dialog.key]
        dialog.keeper.forms = forms
    }

    // Fields of a form as `form()` gives them, put back over the form as it is.
    function applyForm(given) {
        const f = Object.assign(dialog.form(), given)
        title.text = f.title
        place.text = f.location
        notes.text = f.notes
        allDay.checked = f.all_day
        startDay.date = f.start.slice(0, 10)
        startTime.text = f.all_day ? "09:00" : f.start.slice(11, 16)
        endDay.date = f.end.slice(0, 10)
        endTime.text = f.all_day ? "10:00" : f.end.slice(11, 16)
        repeat.currentIndex = Math.max(0, dialog.repeats.indexOf(f.repeat))
        every.value = Math.max(1, f.every || 1)
        dialog.clock = f.clock || ""
        dialog.clockOther = dialog.clock !== "" && dialog.clock !== "UTC"
        dialog.around = f.margins
        dialog.demands = f.demands
        dialog.remind = f.remind
        dialog.links = f.links || []
        dialog.guests = f.guests || dialog.guests
        dialog.moreShown = true
    }

    onClosed: dialog.keep()

    function pad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    function minutesText(m) {
        return m < 60 ? m + " min" : Math.floor(m / 60) + " h" + (m % 60 ? " " + dialog.pad(m % 60) : "")
    }

    // A cost or the gain, 0 to 10, or null when unsaid; kept until Save.
    function rate(name, value) {
        const demands = Object.assign({}, dialog.demands)
        demands[name] = value
        dialog.demands = demands
    }

    function rating(name) {
        const value = dialog.demands[name]
        return value === undefined ? null : value
    }

    // Opens on an event, or on a new one on `day` ("2026-10-05"), at `hour` or the next full hour.
    function edit(key, day, hour) {
        dialog.key = key
        dialog.legOf = ""
        dialog.legTitle = ""
        dialog.legAt = 0
        dialog.legUntil = 0
        dialog.legTimetable = false
        dialog.legSteps = ({ offered: false, steps: [], why: "" })
        dialog.legStep = ""
        dialog.taskUid = ""
        dialog.taskBlock = ""
        dialog.losses = ""
        dialog.problem = ""
        dialog.links = []
        dialog.suggestions = []
        dialog.geo = ""
        dialog.guests = []
        dialog.guestSuggestions = []
        dialog.organized = false
        dialog.inviteConfirmed = false
        dialog.calendars = JSON.parse(dialog.sioul.calendars() || "[]")
        dialog.zoneName = dialog.sioul.zoneName()
        dialog.clock = ""
        dialog.clockOther = false
        if (key) {
            const found = JSON.parse(dialog.sioul.eventForm(key) || "null")
            if (!found)
                return
            const e = found.edit
            title.text = e.title
            place.text = e.location
            notes.text = e.notes
            allDay.checked = e.all_day
            startDay.date = e.start.slice(0, 10)
            startTime.text = e.all_day ? "09:00" : e.start.slice(11, 16)
            endDay.date = e.end.slice(0, 10)
            endTime.text = e.all_day ? "10:00" : e.end.slice(11, 16)
            repeat.currentIndex = Math.max(0, dialog.repeats.indexOf(e.repeat))
            every.value = Math.max(1, e.every || 1)
            dialog.clock = e.clock || ""
            dialog.clockOther = dialog.clock !== "" && dialog.clock !== "UTC"
            calendar.currentIndex = Math.max(0, dialog.calendars.findIndex(c => c.id === found.calendar))
            dialog.currentCalendar = found.calendar || ""
            dialog.calendarName = found.calendar_name || ""
            dialog.hereOnly = found.here_only || ""
            dialog.stays = found.stays || ""
            dialog.withOthers = found.with_others === true
            dialog.organized = found.organized === true
            dialog.guests = found.guests || []
            dialog.around = Object.assign({ before: 0, after: 0 }, e.margins)
            dialog.demands = Object.assign({ cognitive: null, emotional: null, anxiety: null, body: null, gain: null }, e.demands)
            dialog.geo = e.geo || ""
            dialog.remind = e.remind || ""
            dialog.reminds = JSON.parse(dialog.sioul.reminderChoices(dialog.remind) || "[]")
            dialog.moreShown = e.notes !== "" || e.repeat !== "" || dialog.around.before > 0 || dialog.around.after > 0 || dialog.remind !== "" || ["cognitive", "emotional", "anxiety", "body", "gain"].some(n => dialog.rating(n) !== null)
            // What was typed here before, closed without Save: given back, said so.
            dialog.loadedForm = JSON.stringify(dialog.form())
            const kept = dialog.keeper ? dialog.keeper.forms[key] : null
            if (kept)
                dialog.applyForm(kept.form)
            dialog.keptLine = kept ? dialog.sioul.textWith("card-typed-back", "time", kept.at) : ""
        } else {
            dialog.loadedForm = ""
            dialog.keptLine = ""
            const now = dialog.wall(Date.now())
            hour = hour === undefined || hour < 0 ? Math.min(now.getHours() + 1, 23) : hour
            title.text = ""
            place.text = ""
            notes.text = ""
            allDay.checked = false
            startDay.date = day
            endDay.date = day
            startTime.text = dialog.pad(hour) + ":00"
            // As long as a new event lasts (Settings ▸ Agenda), within its day.
            const end = Math.min(hour * 60 + dialog.newLength(), 23 * 60 + 59)
            endTime.text = dialog.pad(Math.floor(end / 60)) + ":" + dialog.pad(end % 60)
            repeat.currentIndex = 0
            every.value = 1
            calendar.currentIndex = 0
            dialog.currentCalendar = ""
            dialog.calendarName = ""
            dialog.hereOnly = ""
            dialog.stays = ""
            dialog.withOthers = false
            dialog.around = { before: 0, after: 0 }
            dialog.demands = { cognitive: null, emotional: null, anxiety: null, body: null, gain: null }
            dialog.remind = ""
            dialog.reminds = JSON.parse(dialog.sioul.reminderChoices("") || "[]")
            dialog.moreShown = false
        }
        dialog.closedBySave = false
        dialog.cancelled = false
        dialog.open()
        title.forceActiveFocus()
    }

    // How long a new event lasts, in minutes (Settings ▸ Agenda; 30 unset).
    function newLength() {
        const minutes = typeof dialog.sioul.newEventMinutes === "function" ? dialog.sioul.newEventMinutes() : 30
        return minutes > 0 ? minutes : 30
    }

    // A new event on `day` ("2026-10-05"), from `from` to `to`, minutes from its
    // midnight (the timeline's "+"); past midnight, the next day's times.
    function editRange(day, from, to) {
        dialog.edit("", day, Math.floor(from / 60) % 24)
        const at = (base, minutes) => {
            const d = new Date(base + "T12:00:00")
            d.setDate(d.getDate() + Math.floor(minutes / 1440))
            const pad = n => n < 10 ? "0" + n : String(n)
            return { day: d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()), time: pad(Math.floor(minutes % 1440 / 60)) + ":" + pad(minutes % 60) }
        }
        const start = at(day, from)
        const end = at(day, to)
        startDay.date = start.day
        startTime.text = start.time
        endDay.date = end.day
        endTime.text = end.time
    }

    // For the window's pictures and tests: `text` as if typed in its place.
    function typePlace(text) {
        place.forceActiveFocus()
        place.text = text
        dialog.geo = ""
        placePause.restart()
    }

    // A new event made from something: its title and notes given, the link kept.
    function makeFrom(text, note, link, day) {
        dialog.edit("", day)
        title.text = text
        notes.text = note
        dialog.links = link ? [link] : []
        dialog.moreShown = note !== ""
    }

    // A task made an appointment: the form a new event's, filled from the task
    // at its block's time (`start`, `end`), in the calendar for new events or
    // another chosen; saved, the event replaces the task (`appointments`).
    function makeFromTask(uid, block, start, end) {
        const made = JSON.parse(dialog.sioul.taskAsEvent(uid, start, end) || "{}")
        if (made.error) {
            dialog.sioul.status = made.error
            return
        }
        dialog.edit("", made.start.slice(0, 10))
        dialog.taskUid = uid
        dialog.taskBlock = block
        dialog.losses = made.losses || ""
        title.text = made.title
        place.text = made.location
        notes.text = made.notes
        startDay.date = made.start.slice(0, 10)
        startTime.text = made.start.slice(11, 16)
        endDay.date = made.end.slice(0, 10)
        endTime.text = made.end.slice(11, 16)
        repeat.currentIndex = Math.max(0, dialog.repeats.indexOf(made.repeat))
        every.value = Math.max(1, made.every || 1)
        dialog.around = Object.assign({ before: 0, after: 0 }, made.margins)
        dialog.demands = Object.assign({ cognitive: null, emotional: null, anxiety: null, body: null, gain: null }, made.demands)
        dialog.moreShown = made.notes !== "" || made.repeat !== "" || dialog.around.before > 0 || dialog.around.after > 0
    }

    // A leg of the appointment in `key` (part E5), `title` from `at` to `until`
    // (Unix seconds), its own times given ("YYYY-MM-DD", "HH:MM"). Saved, it
    // moves with it. No repeat, no calendar to choose: the appointment's.
    function makeLeg(key, title, at, until, day, from, to) {
        dialog.edit("", day)
        dialog.legOf = key
        dialog.legSteps = JSON.parse(dialog.sioul.legSteps(key) || "{}")
        dialog.legTitle = title
        dialog.legAt = at
        dialog.legUntil = until
        startDay.date = day
        endDay.date = day
        startTime.text = from
        endTime.text = to
    }

    // For the window's pictures: the costs and the gain in view.
    function showRatings() {
        dialog.moreShown = true
        Qt.callLater(() => {
            const top = tiles.mapToItem(scroll.contentItem.contentItem, 0, 0).y
            scroll.contentItem.contentY = Math.max(0, Math.min(top - 12, scroll.contentItem.contentHeight - scroll.height))
        })
    }

    // For the window's tests: as if typed, then saved.
    function fill(text, from, to, how) {
        title.text = text
        startTime.text = from
        endTime.text = to
        repeat.currentIndex = Math.max(0, dialog.repeats.indexOf(how))
        every.value = 1
        dialog.moreShown = true
    }

    function save() {
        // An event of a calendar not offered (read only) stays where it is.
        const offered = dialog.calendars.length > 0 ? dialog.calendars[Math.max(0, calendar.currentIndex)].id : ""
        const where = dialog.key !== "" && dialog.calendars.findIndex(c => c.id === dialog.currentCalendar) < 0 ? dialog.currentCalendar : offered
        // Guests invited, taken off, or told of a change: said first, saved on Save.
        if (dialog.legOf === "" && dialog.taskUid === "" && !dialog.inviteConfirmed && typeof dialog.sioul.inviteSaid === "function") {
            const said = JSON.parse(dialog.sioul.inviteSaid(dialog.key, JSON.stringify(dialog.form()), where) || "{}")
            if (said.ask) {
                inviteAsk.ask(dialog.sioul.text("invite-guests"), said.ask, dialog.sioul.text("invite-save"))
                return
            }
        }
        dialog.inviteConfirmed = false
        const problem = dialog.legOf !== "" ? dialog.sioul.saveLeg(dialog.legOf, JSON.stringify(dialog.form()), dialog.legSide, dialog.legTimetable, dialog.legStep) : dialog.taskUid !== "" ? dialog.sioul.taskToEvent(dialog.taskUid, dialog.taskBlock, JSON.stringify(dialog.form()), offered) : dialog.sioul.saveEvent(dialog.key, JSON.stringify(dialog.form()), where)
        if (problem) {
            dialog.problem = problem
            return
        }
        dialog.closedBySave = true
        dialog.forget()
        dialog.close()
        dialog.saved()
    }

    onAccepted: dialog.save()

    function form() {
        return {
            title: title.text,
            location: place.text,
            notes: notes.text,
            start: allDay.checked ? startDay.date : startDay.date + "T" + startTime.text,
            end: allDay.checked ? endDay.date : endDay.date + "T" + endTime.text,
            all_day: allDay.checked,
            clock: allDay.checked ? "" : dialog.clock,
            repeat: dialog.repeats[repeat.currentIndex],
            every: dialog.repeats[repeat.currentIndex] === "" ? 1 : every.value,
            margins: dialog.around,
            demands: dialog.demands,
            remind: dialog.remind,
            geo: dialog.geo,
            links: dialog.links,
            guests: dialog.guests
        }
    }

    // Over the whole window, whatever item made it (a Later's is 0 by 0).
    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    modal: true
    // As wide as what it holds, without a cap of its own (docs/qt-quick.md, "An
    // editing card's width"): its fields at about 70 characters of the body
    // font, a readable line, or its tiles side by side, whichever is wider;
    // never past the window less the usual margins, all of a phone's.
    width: Math.min(Math.ceil(Math.max(70 * body.averageCharacterWidth, tiles.naturalWidth)) + dialog.leftPadding + dialog.rightPadding + 12, (parent ? dialog.roomWidth : 800) - 2 * dialog.theme.gap)
    // Taller than the window, every detail unfolded: the form scrolls, its buttons stay.
    height: Math.min(implicitHeight, dialog.roomHeight - 32)
    // A meeting with others: your own time around it alone, kept in your own file (decision 18).
    title: dialog.legOf !== "" ? dialog.sioul.text("appointment-add-leg") : dialog.taskUid !== "" ? dialog.sioul.text("agenda-make-appointment") : dialog.key && dialog.withOthers ? dialog.sioul.text("event-own-time") : dialog.key ? dialog.sioul.text("ui-edit-event") : dialog.sioul.text("ui-new-event")

    // For the window's tests: the question asked before saving.
    readonly property alias inviteQuestion: inviteAsk

    // Who hears of it, said before it is saved (`inviteSaid`): Save, or Cancel.
    ConfirmDialog {
        id: inviteAsk

        objectName: "eventInviteAsk"
        sioul: dialog.sioul
        theme: dialog.theme
        onConfirmed: {
            dialog.inviteConfirmed = true
            dialog.save()
        }
    }

    FontMetrics {
        id: body

        font: dialog.font
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 10

            Label {
                objectName: "eventKept"
                visible: dialog.keptLine !== ""
                Layout.fillWidth: true
                text: dialog.keptLine
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.muted
            }
            // A leg: which appointment, which side of it, and whether its time is a timetable's.
            ColumnLayout {
                objectName: "eventLeg"
                visible: dialog.legOf !== ""
                Layout.fillWidth: true
                spacing: 4

                Label {
                    Layout.fillWidth: true
                    text: dialog.legLine
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
                CheckBox {
                    objectName: "legTimetable"
                    text: dialog.sioul.text("leg-timetable")
                    checked: dialog.legTimetable
                    onToggled: dialog.legTimetable = checked
                }
                // Its step (part 7): the leg is that step's time, once every device keeps such a leg.
                Label {
                    visible: dialog.legSteps.offered === true
                    text: dialog.sioul.text("leg-step")
                    textFormat: Text.PlainText
                    color: dialog.theme.muted
                }
                PlainComboBox {
                    objectName: "legStep"
                    visible: dialog.legSteps.offered === true
                    Layout.fillWidth: true
                    model: [dialog.sioul.text("leg-step-none")].concat((dialog.legSteps.steps || []).map(s => dialog.theme.plain(s.title)))
                    currentIndex: Math.max(0, (dialog.legSteps.steps || []).findIndex(s => s.uid === dialog.legStep) + 1)
                    onActivated: index => dialog.legStep = index > 0 ? dialog.legSteps.steps[index - 1].uid : ""
                }
                Label {
                    objectName: "legStepWhy"
                    visible: !!dialog.legSteps.why
                    Layout.fillWidth: true
                    text: dialog.legSteps.why || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
            }
            Label {
                objectName: "event-losses"
                visible: dialog.losses !== ""
                Layout.fillWidth: true
                text: dialog.losses
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.muted
            }
            TextField {
                id: title

                objectName: "eventTitle"
                enabled: !dialog.withOthers
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("event-title")
            }
            CheckBox {
                id: allDay

                objectName: "eventAllDay"
                enabled: !dialog.withOthers
                text: dialog.sioul.text("agenda-all-day")
            }
            GridLayout {
                enabled: !dialog.withOthers
                columns: 3
                columnSpacing: 8
                rowSpacing: 6

                Label {
                    text: dialog.sioul.text("event-starts")
                    color: dialog.theme.muted
                }
                DateField {
                    id: startDay

                    theme: dialog.theme
                    sioul: dialog.sioul
                    locale: Qt.locale(dialog.sioul.text("qt-locale"))
                    pickLabel: dialog.sioul.text("event-pick-day")
                    // The end follows the start when it would come before it.
                    onEdited: if (endDay.date < startDay.date) endDay.date = startDay.date
                }
                ClockField {
                    id: startTime

                    visible: !allDay.checked
                    Layout.preferredWidth: 80
                }
                Label {
                    text: dialog.sioul.text("event-ends")
                    color: dialog.theme.muted
                }
                DateField {
                    id: endDay

                    theme: dialog.theme
                    sioul: dialog.sioul
                    locale: Qt.locale(dialog.sioul.text("qt-locale"))
                    pickLabel: dialog.sioul.text("event-pick-day")
                }
                ClockField {
                    id: endTime
                    // An end: Up stops at 23:55 (ClockField's `end`).
                    end: true

                    visible: !allDay.checked
                    Layout.preferredWidth: 80
                }
            }
            // The clock its times are on: local time (the time zone shown), UTC,
            // or another zone found by its name; on another, its times here beside.
            RowLayout {
                objectName: "eventClockRow"
                visible: !allDay.checked && dialog.legOf === ""
                enabled: !dialog.withOthers
                Layout.fillWidth: true
                spacing: 8

                Label {
                    text: dialog.sioul.text("event-clock")
                    color: dialog.theme.muted
                }
                PlainComboBox {
                    objectName: "eventClock"
                    Layout.fillWidth: true
                    // Fixed while the form is open: a model changed under it would put the box back on its first line.
                    model: [dialog.sioul.textWith("event-clock-local", "zone", dialog.zoneName), dialog.sioul.text("event-clock-utc"), dialog.sioul.text("zone-other")].map(t => dialog.theme.plain(t))
                    currentIndex: dialog.clock === "UTC" ? 1 : dialog.clockOther ? 2 : 0
                    Accessible.name: dialog.sioul.text("event-clock")
                    onActivated: index => {
                        dialog.clockOther = index === 2
                        if (index < 2)
                            dialog.clock = index === 1 ? "UTC" : ""
                        else if (dialog.clock === "UTC")
                            dialog.clock = ""
                    }
                }
            }
            ZonePicker {
                objectName: "eventZone"
                visible: dialog.clockOther && !allDay.checked && dialog.legOf === ""
                enabled: !dialog.withOthers
                Layout.fillWidth: true
                sioul: dialog.sioul
                theme: dialog.theme
                zone: dialog.clockOther ? dialog.clock : ""
                onPicked: name => dialog.clock = name
            }
            Label {
                objectName: "eventClockLine"
                visible: text !== ""
                Layout.fillWidth: true
                text: dialog.clock === "" || allDay.checked ? "" : dialog.sioul.clockLine(startDay.date + "T" + startTime.text, endDay.date + "T" + endTime.text, dialog.clock)
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.muted
            }
            TextField {
                id: place

                objectName: "eventPlace"
                enabled: !dialog.withOthers
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("event-where")
                // Typed again: the point it had no longer holds; suggestions after a pause.
                onTextEdited: {
                    dialog.geo = ""
                    placePause.restart()
                }
                Keys.onEscapePressed: event => {
                    event.accepted = dialog.suggestions.length > 0
                    dialog.suggestions = []
                }
                // Down and Up through the suggestions, Enter chooses one; the field keeps the keyboard.
                Keys.onDownPressed: event => {
                    event.accepted = dialog.suggestions.length > 0
                    dialog.suggestionAt = Math.min(dialog.suggestions.length - 1, dialog.suggestionAt + 1)
                }
                Keys.onUpPressed: event => {
                    event.accepted = dialog.suggestions.length > 0
                    dialog.suggestionAt = Math.max(-1, dialog.suggestionAt - 1)
                }
                Keys.onReturnPressed: event => {
                    event.accepted = dialog.suggestionAt >= 0 && dialog.suggestionAt < dialog.suggestions.length
                    if (event.accepted)
                        dialog.choosePlace(dialog.suggestions[dialog.suggestionAt])
                }

                // After 300 ms without typing: the places you used before, then, with
                // three characters and the switch on (Settings ▸ Agenda), the service's.
                Timer {
                    id: placePause

                    interval: 300
                    onTriggered: {
                        const typed = place.text.trim()
                        if (typed.length < 2) {
                            dialog.suggestions = []
                            return
                        }
                        if (typed.length >= 3 && dialog.suggestSaid === "" && dialog.sioul.suggestsAddresses() && dialog.sioul.firstTime("suggest"))
                            dialog.suggestSaid = dialog.sioul.textWith("suggest-said", "service", dialog.sioul.addressService())
                        dialog.sioul.suggestAddresses(typed)
                    }
                }
                Connections {
                    target: dialog.sioul

                    function onAddressesSuggested(typed, suggested) {
                        if (typed !== place.text.trim() || !place.activeFocus)
                            return
                        const read = JSON.parse(suggested || "{}")
                        const out = []
                        for (const label of read.used || [])
                            if (!out.some(o => o.label.toLowerCase() === label.toLowerCase()))
                                out.push({ label: label })
                        for (const found of read.found || [])
                            if (!out.some(o => o.label.toLowerCase() === found.label.toLowerCase()))
                                out.push(found)
                        dialog.suggestions = out.slice(0, 8)
                        dialog.suggestionAt = -1
                    }
                }
            }
            // The addresses suggested: places used before first, then the service's; a
            // tap chooses one. Where what is typed goes, said once.
            Repeater {
                model: place.activeFocus ? dialog.suggestions : []

                delegate: ItemDelegate {
                    id: suggestion

                    required property var modelData
                    required property int index

                    objectName: "eventPlaceSuggestion"
                    // The field keeps the keyboard (and the list), its press chooses.
                    focusPolicy: Qt.NoFocus
                    highlighted: suggestion.index === dialog.suggestionAt
                    Layout.fillWidth: true
                    text: dialog.theme.plain(suggestion.modelData.label)
                    icon.name: suggestion.modelData.lat !== undefined ? "mark-location" : "document-open-recent"
                    onClicked: dialog.choosePlace(suggestion.modelData)
                }
            }
            Label {
                objectName: "eventPlaceSaid"
                visible: dialog.suggestSaid !== ""
                Layout.fillWidth: true
                text: dialog.suggestSaid
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.muted
                font.pixelSize: 13
            }

            // Its guests (decision 7 of the event card): an event of yours, or a meeting
            // you organize; suggested from this device's address books and Your
            // people, nothing asked of a network. Saving says first who hears of it.
            Label {
                visible: invite.shown && dialog.guests.length > 0
                text: dialog.sioul.text("invite-guests")
                color: dialog.theme.muted
            }
            Repeater {
                model: invite.shown ? dialog.guests : []

                delegate: RowLayout {
                    id: guestRow

                    required property var modelData
                    required property int index

                    objectName: "eventGuest"
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        Layout.fillWidth: true
                        text: guestRow.modelData.name ? dialog.theme.plain(guestRow.modelData.name) + " · " + dialog.theme.plain(guestRow.modelData.address) : dialog.theme.plain(guestRow.modelData.address)
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                    }
                    ToolButton {
                        objectName: "eventGuestOff"
                        icon.name: "edit-clear"
                        display: AbstractButton.IconOnly
                        text: dialog.theme.plain(dialog.sioul.textWith("invite-take-off", "who", guestRow.modelData.name || guestRow.modelData.address))
                        Accessible.name: text
                        ToolTip.visible: hovered
                        ToolTip.text: dialog.theme.plain(text)
                        onClicked: dialog.removeGuest(guestRow.index)
                    }
                }
            }
            TextField {
                id: invite

                objectName: "eventInvite"
                // Not a meeting someone else organizes, a leg or a task being made an appointment.
                readonly property bool shown: !dialog.withOthers && dialog.legOf === "" && dialog.taskUid === ""
                visible: invite.shown
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("invite-field")
                inputMethodHints: Qt.ImhEmailCharactersOnly | Qt.ImhNoAutoUppercase
                onTextEdited: {
                    // A comma or a semicolon ends an address.
                    if (/[,;]$/.test(invite.text) && dialog.addGuest(invite.text.slice(0, -1))) {
                        invite.text = ""
                        return
                    }
                    dialog.guestSuggestions = invite.text.trim().length >= 2 && typeof dialog.sioul.guestCompletions === "function" ? JSON.parse(dialog.sioul.guestCompletions(invite.text) || "[]") : []
                    dialog.guestAt = -1
                }
                Keys.onEscapePressed: event => {
                    event.accepted = dialog.guestSuggestions.length > 0
                    dialog.guestSuggestions = []
                }
                Keys.onDownPressed: event => {
                    event.accepted = dialog.guestSuggestions.length > 0
                    dialog.guestAt = Math.min(dialog.guestSuggestions.length - 1, dialog.guestAt + 1)
                }
                Keys.onUpPressed: event => {
                    event.accepted = dialog.guestSuggestions.length > 0
                    dialog.guestAt = Math.max(-1, dialog.guestAt - 1)
                }
                // Enter adds the suggestion chosen, else what is typed; the form is saved only from an empty field.
                Keys.onReturnPressed: event => {
                    const typed = dialog.guestAt >= 0 && dialog.guestAt < dialog.guestSuggestions.length ? dialog.guestSuggestions[dialog.guestAt] : invite.text
                    event.accepted = invite.text.trim() !== ""
                    if (event.accepted && dialog.addGuest(typed))
                        invite.text = ""
                }
            }
            Repeater {
                model: invite.activeFocus ? dialog.guestSuggestions : []

                delegate: ItemDelegate {
                    id: guestSuggestion

                    required property var modelData
                    required property int index

                    objectName: "eventGuestSuggestion"
                    focusPolicy: Qt.NoFocus
                    highlighted: guestSuggestion.index === dialog.guestAt
                    Layout.fillWidth: true
                    text: dialog.theme.plain(guestSuggestion.modelData)
                    icon.name: "contact-new"
                    onClicked: {
                        if (dialog.addGuest(guestSuggestion.modelData))
                            invite.text = ""
                    }
                }
            }

            // Its calendar, said, and another chosen: a new event's, or an event
            // moved there, on the phone too. In sight, never folded under "More
            // details": folded there, it was never found (22 October 2026).
            GridLayout {
                objectName: "event-calendar-row"
                // A leg goes with its appointment, said once saved.
                visible: dialog.legOf === ""
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 8
                rowSpacing: 6

                Label {
                    visible: dialog.calendars.length > 1 || dialog.calendarName !== ""
                    text: dialog.sioul.text("event-calendar")
                    color: dialog.theme.muted
                }
                PlainComboBox {
                    id: calendar

                    objectName: "event-calendar"
                    visible: dialog.calendars.length > 1 && dialog.stays === "" && (dialog.key === "" || dialog.calendars.some(c => c.id === dialog.currentCalendar))
                    Layout.fillWidth: true
                    model: dialog.calendars.map(c => dialog.theme.plain(c.name))
                    Accessible.name: dialog.sioul.text("event-calendar")
                }
                Label {
                    objectName: "event-calendar-name"
                    visible: dialog.calendarName !== "" && !calendar.visible
                    Layout.fillWidth: true
                    text: dialog.theme.plain(dialog.sioul.textWith("event-calendar-in", "name", dialog.calendarName))
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                }
                Label {
                    objectName: "event-stays"
                    visible: dialog.stays !== ""
                    Layout.columnSpan: 2
                    Layout.fillWidth: true
                    text: dialog.stays
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: dialog.theme.muted
                }
                Label {
                    objectName: "event-here-only"
                    visible: dialog.hereOnly !== ""
                    Layout.columnSpan: 2
                    Layout.fillWidth: true
                    text: dialog.hereOnly
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: dialog.theme.muted
                }
            }

            Button {
                flat: true
                text: (dialog.moreShown ? "▾  " : "▸  ") + dialog.sioul.text("ui-more-details")
                onClicked: dialog.moreShown = !dialog.moreShown
            }
            GridLayout {
                visible: dialog.moreShown
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 8
                rowSpacing: 6

                Label {
                    Layout.alignment: Qt.AlignTop
                    text: dialog.sioul.text("contact-notes")
                    color: dialog.theme.muted
                }
                TextArea {
                    id: notes

                    // Its organizer's words: read, never changed here.
                    readOnly: dialog.withOthers
                    // A field shows where it is: a border, darker when it has the focus.
                    background: Rectangle {
                        color: dialog.theme.surface
                        radius: dialog.theme.radius
                        border.color: notes.activeFocus ? dialog.theme.focus : dialog.theme.line
                    }
                    Layout.fillWidth: true
                    Layout.preferredHeight: 80
                    placeholderText: dialog.sioul.text("task-field-notes")
                    wrapMode: TextArea.Wrap
                }
                // A list to tick, in the notes: what to bring (docs/tasks.md, "Checklists").
                Item {
                    implicitWidth: 1
                    implicitHeight: 1
                }
                Button {
                    objectName: "eventAddChecklist"
                    enabled: !dialog.withOthers
                    flat: true
                    text: "+  " + dialog.sioul.text("checklist-add")
                    onClicked: {
                        const before = notes.text.replace(/\s+$/, "")
                        const afterItem = /(^|\n)\s*([-*+]|\d+[.)])\s+\[[ xX-]\][^\n]*$/.test(before)
                        notes.text = before === "" ? "- [ ] " : before + (afterItem ? "\n" : "\n\n") + "- [ ] "
                        notes.forceActiveFocus()
                        notes.cursorPosition = notes.length
                    }
                }
                Label {
                    visible: dialog.legOf === ""
                    text: dialog.sioul.text("event-repeat")
                    color: dialog.theme.muted
                }
                PlainComboBox {
                    id: repeat

                    objectName: "eventRepeat"
                    // A leg happens once: one that repeated would count other days as before it.
                    visible: dialog.legOf === ""
                    enabled: !dialog.withOthers
                    Layout.fillWidth: true
                    model: dialog.repeats.map(r => dialog.sioul.text("repeat-" + (r || "none")))
                }
                // Every how many of them, from its first day: every other day is 2.
                Label {
                    visible: dialog.legOf === "" && repeat.currentIndex > 0
                    text: dialog.sioul.textArgs("event-repeat-every", JSON.stringify({ unit: dialog.repeats[repeat.currentIndex] }))
                    textFormat: Text.PlainText
                    color: dialog.theme.muted
                }
                RowLayout {
                    visible: dialog.legOf === "" && repeat.currentIndex > 0
                    spacing: 6

                    SpinBox {
                        id: every

                        objectName: "eventEvery"
                        enabled: !dialog.withOthers
                        from: 1
                        to: 99
                        value: 1
                        editable: true
                    }
                    Label {
                        text: dialog.sioul.textArgs("event-repeat-unit", JSON.stringify({ unit: dialog.repeats[repeat.currentIndex], n: every.value }))
                        textFormat: Text.PlainText
                        color: dialog.theme.muted
                    }
                }
                // Getting there and back, getting ready: kept free around it, never a pause.
                Label {
                    Layout.maximumWidth: 160
                    text: dialog.sioul.text("task-field-before")
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
                PlainComboBox {
                    Layout.fillWidth: true
                    objectName: "event-before"
                    model: dialog.marginChoices.map(m => m === 0 ? dialog.sioul.text("task-rating-unsaid") : dialog.theme.plain(dialog.minutesText(m)))
                    currentIndex: Math.max(0, dialog.marginChoices.indexOf(dialog.around.before))
                    onActivated: index => dialog.around = Object.assign({}, dialog.around, { before: dialog.marginChoices[index] })
                }
                Label {
                    Layout.maximumWidth: 160
                    text: dialog.sioul.text("task-field-after")
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
                PlainComboBox {
                    Layout.fillWidth: true
                    objectName: "event-after"
                    model: dialog.marginChoices.map(m => m === 0 ? dialog.sioul.text("task-rating-unsaid") : dialog.theme.plain(dialog.minutesText(m)))
                    currentIndex: Math.max(0, dialog.marginChoices.indexOf(dialog.around.after))
                    onActivated: index => dialog.around = Object.assign({}, dialog.around, { after: dialog.marginChoices[index] })
                }
                // Its reminder: as usual (Settings ▸ Reminders and notifications),
                // not this one, or so long before it and its time to get there.
                Label {
                    Layout.maximumWidth: 160
                    text: dialog.sioul.text("event-remind")
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
                PlainComboBox {
                    Layout.fillWidth: true
                    textRole: "label"
                    model: dialog.reminds.map(c => ({ value: c.value, label: dialog.theme.plain(c.label) }))
                    currentIndex: Math.max(0, dialog.reminds.findIndex(c => c.value === dialog.remind))
                    onActivated: index => dialog.remind = dialog.reminds[index].value
                }
                // What it costs, and what it gives back: four tiles and a row, 0 to 10
                // each, unsaid until said (CostTiles.qml); kept until Save.
                CostTiles {
                    id: tiles

                    Layout.columnSpan: 2
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    Layout.bottomMargin: 4
                    sioul: dialog.sioul
                    theme: dialog.theme
                    values: dialog.demands
                    onEdited: (name, value) => dialog.rate(name, value)
                }
            }
        }
    }

    footer: ColumnLayout {
        spacing: 6

        Label {
            visible: dialog.problem !== ""
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            Layout.bottomMargin: 12

            Item {
                Layout.fillWidth: true
            }
            // Cancel: what was typed goes, as it never was.
            Button {
                objectName: "eventCancel"
                text: dialog.sioul.text("ui-cancel")
                onClicked: {
                    dialog.cancelled = true
                    dialog.forget()
                    dialog.close()
                }
            }
            Button {
                text: dialog.sioul.text("ui-save")
                highlighted: true
                onClicked: dialog.save()
            }
        }
    }
}
