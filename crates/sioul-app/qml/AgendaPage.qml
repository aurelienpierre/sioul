// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The agenda: what comes, from today, as a calm list of days; the day, the
// week and the month one switch away. The Day and the Week are one timeline
// of everything that has a time (WeekPlanning.qml): appointments, the plan's
// steps, meals, naps, the night, doses. Nothing is late or overdue: the past
// is not shown unless you go back to it. An event opens on the right with its
// time and place; notes, repetition and guests are folded; anything else
// opens where it lives (`window.openCard`). Deleting waits ten seconds with
// "Undo"; a repeating event asks whether it is this time or all.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // Month and day names in Sioul's language, not the system's.
    readonly property var locale: Qt.locale(page.sioul.text("qt-locale"))
    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string agendaText: ""
    readonly property var shown: page.agendaText ? page.eventsOfDays(JSON.parse(page.agendaText)) : ({ days: [], sentence: "", from: "" })

    // The agenda's days (version 3): each thing an item of the one model of
    // time as its day places it (`timeline::Placed`), with an event's details.
    // The list and the month draw each day's events and time blocks, its whole
    // days first, each with its details, its place in the day and what may
    // change it; the Day and the Week draw every kind, from the days as they come.
    function eventOf(row) {
        const item = row.item
        const source = item.source || {}
        const time = item.time || {}
        const span = time.type === "span"
        // What the job leaves out when false or empty, said: every reader finds its field.
        const detail = Object.assign({ end: span ? time.until : source.start, location: "", cancelled: false, tentative: false, read_only: false, with_others: false, task: "", list: "" }, row.detail || {})
        return Object.assign(detail, {
            id: item.id,
            key: source.key,
            uid: source.uid,
            start: source.start,
            recurring: source.recurring === true,
            calendar: source.calendar || "",
            color: item.colour || null,
            when: row.when,
            summary: item.title,
            all_day: row.all_day === true,
            from_minute: row.from_minute,
            to_minute: row.to_minute,
            column: row.column || 0,
            columns: row.columns || 1,
            // The time kept before and after it: its lighter ends.
            before: span ? Math.round((time.at - time.start) / 60) : 0,
            after: span ? Math.round((time.end - time.until) / 60) : 0,
            can: item.can
        })
    }

    function eventsOfDays(agenda) {
        return Object.assign({}, agenda, { days: (agenda.days || []).map(day => Object.assign({}, day, { events: (day.head || []).concat(day.timed || []).filter(row => row.detail).map(page.eventOf) })) })
    }

    function takeShown() {
        if (page.visible)
            page.agendaText = page.sioul.agenda
        // The open event read again, with what changed in it (its list ticked): by its file and start.
        if (page.opened !== null) {
            const key = page.opened.key
            const start = page.opened.start
            for (const day of page.shown.days)
                for (const event of day.events)
                    if (event.key === key && event.start === start)
                        page.opened = event
            page.readDetails()
        }
        // An event asked for by a link, once its days are shown: its details.
        if (page.wanted !== "" && page.showWanted())
            page.wanted = ""
    }

    // An event asked for by a link, outside the days shown: its days are read first.
    property string wanted: ""

    // Its details, when it is in the days shown.
    function showWanted() {
        for (const day of page.shown.days)
            for (const event of day.events)
                if (event.key === page.wanted) {
                    page.opened = event
                    return true
                }
        return false
    }

    // Its days never came: its form, as before.
    Timer {
        id: wantedLate

        interval: 3000
        onTriggered: {
            if (page.wanted === "")
                return
            const key = page.wanted
            page.wanted = ""
            eventDialog.now().edit(key, "")
        }
    }

    Connections {
        target: page.sioul

        function onAgendaChanged() {
            page.takeShown()
        }
    }
    onVisibleChanged: {
        page.takeShown()
        if (page.visible) {
            page.readAgain()
            if (page.drawnToday)
                page.sioul.refreshHealth()
        }
    }
    // "agenda", "day", "week", "month".
    property string mode: "agenda"
    // A narrow screen (a phone): the title under the arrows, events on two lines.
    readonly property bool narrow: page.width < 600
    readonly property var modes: ["agenda", "day", "week", "month"]
    property var opened: null
    // The event being made a task, asked about first.
    property string converting: ""
    // The one opened is a task's time block: shown as the task it is.
    readonly property bool openedTask: page.opened !== null && (page.opened.task || "") !== ""
    // When the open event is reminded, and its own choice (docs/reminders.md): {line, remind, writable}.
    property var reminder: ({})
    readonly property var reminds: page.reminder.writable ? JSON.parse(page.sioul.reminderChoices(page.reminder.remind || "") || "[]") : []
    function readReminder() {
        page.reminder = page.opened ? JSON.parse(page.sioul.eventReminder(page.opened.key, page.opened.start) || "{}") : ({})
    }
    // Its notes, organizer and guests, read from its file when it opens: the agenda's
    // days carry none of them, so that a week of long invitations stays light.
    property var details: ({})
    function readDetails() {
        page.details = page.opened ? JSON.parse(page.sioul.eventDetails(page.opened.key, page.opened.start) || "{}") : ({})
    }
    onOpenedChanged: {
        page.readReminder()
        page.readDetails()
    }
    // The event the menu or the deletion asks about.
    property var menuTarget: null
    // The step whose menu is open (a row of the timeline).
    property var stepTarget: null
    // "Show anyway": the steps quiet time folds on coming days, drawn.
    property bool showAnyway: false
    // What the Day and the Week show on this device ("Show", agenda-view.toml):
    // {steps, needs, doses, calendars_hidden}; all shown until you say.
    property var view: JSON.parse(page.sioul.agendaView() || "{}")
    readonly property var hide: ({ steps: page.view.steps === false, needs: page.view.needs === false, doses: page.view.doses === false, calendars: page.view.calendars_hidden || [] })
    // The calendars of the days shown, and those Show hides: Show's list.
    readonly property var calendarsShown: {
        const names = (page.view.calendars_hidden || []).slice()
        for (const day of page.shown.days)
            for (const row of (day.head || []).concat(day.timed || []))
                if (row.item.source && row.item.source.type === "calendar" && row.item.source.calendar && names.indexOf(row.item.source.calendar) < 0)
                    names.push(row.item.source.calendar)
        return names.sort((a, b) => a.localeCompare(b))
    }
    // Whether a day shown has steps quiet time folds: "Show anyway" offered.
    readonly property bool anyFolded: page.shown.days.some(d => (d.bands || []).length > 0)

    // Show changed: `part` ("steps", "needs", "doses") on or off, kept on this device.
    function showPart(part, on) {
        const changed = Object.assign({}, page.view)
        changed[part] = on
        page.saveView(changed)
    }

    function showCalendar(name, on) {
        const hidden = (page.view.calendars_hidden || []).filter(n => n !== name)
        if (!on)
            hidden.push(name)
        page.saveView(Object.assign({}, page.view, { calendars_hidden: hidden }))
    }

    function saveView(changed) {
        const problem = page.sioul.setAgendaView(JSON.stringify(changed))
        if (problem !== "")
            page.sioul.status = problem
        else
            page.view = changed
    }

    // Today's head line now (`timeline::head_line`, given ahead to midnight): its
    // frame by the window's clock, which counts whole minutes (the first frame
    // starts at the second the job ran).
    readonly property var headNow: {
        const now = page.window.now
        let found = null
        for (const frame of page.shown.head || [])
            if (frame.from <= now + 59 && now < frame.until)
                found = frame
        return found
    }
    // The Day or the Week shows today: the doses' marks and today's steps follow the clock.
    readonly property bool drawnToday: (page.mode === "day" || page.mode === "week") && page.shown.days.some(d => d.date === page.window.today)
    // On a phone, the event open takes the page; Back closes it (main.qml).
    readonly property bool canGoBack: page.opened !== null
    function back() {
        page.opened = null
    }
    property bool moreShown: false
    // The open event, as what new things are tied to.
    readonly property var source: page.opened && page.opened.uid ? { uri: "sioul:event/" + encodeURIComponent(page.opened.uid), kind: "event", key: page.opened.key, title: page.opened.summary, start: page.opened.start } : null

    function today() {
        return new Date()
    }

    function iso(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // A writer's answer, said in the status line when something stops it
    // (a calendar you only read, a meeting with others: Rust's one rule).
    function said(problem) {
        if (problem !== "")
            page.sioul.status = problem
    }

    // The first day a mode shows around `day`: the day itself, its Monday, the Monday before the 1st.
    function startOf(day, mode) {
        const d = new Date(day.getFullYear(), day.getMonth(), day.getDate(), 12)
        if (mode === "week")
            d.setDate(d.getDate() - (d.getDay() + 6) % 7)
        if (mode === "month") {
            d.setDate(1)
            d.setDate(d.getDate() - (d.getDay() + 6) % 7)
        }
        return d
    }

    function length(mode) {
        return mode === "day" ? 1 : mode === "week" ? 7 : mode === "month" ? 42 : 15
    }

    property date anchor: new Date()

    // Shown from today: the page follows the days as they turn.
    property bool followsToday: true

    // Read again at each five minutes' turn of the window's clock while shown,
    // when shown again, when the app comes back (the clock jumps) and when the
    // day turns: events over leave the list, what another device or the
    // server changed comes in, the days shown from today follow midnight.
    property real readAt: 0
    property string readOn: ""

    function readAgain() {
        if (page.followsToday && page.iso(page.anchor) !== page.window.today)
            page.anchor = new Date(page.window.today + "T12:00:00")
        page.readAt = page.window.now
        page.readOn = page.window.today
        page.load()
    }

    Connections {
        target: page.window

        // Back on the screen while today is drawn: the doses read again at once, not at the
        // next minute's turn (until then, their marks say they are being read).
        function onBackAtChanged() {
            if (page.visible && page.drawnToday)
                page.sioul.refreshHealth()
        }

        function onNowChanged() {
            if (!page.visible)
                return
            // While today is drawn: the doses' marks read again at each minute (the Health job, one run for all who ask).
            if (page.drawnToday)
                page.sioul.refreshHealth()
            if (page.window.today === page.readOn && Math.floor(page.window.now / 300) === Math.floor(page.readAt / 300))
                return
            // Today laid again from now, cheaply (the plan as last made): its publish brings the agenda's days.
            if (page.drawnToday && page.window.today === page.readOn) {
                page.readAt = page.window.now
                page.sioul.relayToday()
            } else {
                page.readAgain()
            }
        }
    }

    function load() {
        page.followsToday = page.iso(page.anchor) === page.window.today
        // From today, the list is what comes: an event over is shown only when you go back (◂).
        const upcoming = page.mode === "agenda" && page.followsToday
        page.sioul.showDays(page.iso(page.startOf(page.anchor, page.mode)), page.length(page.mode), upcoming)
    }

    function move(steps) {
        let d = new Date(page.anchor)
        if (page.mode === "month") {
            // The same day of the next month, or its last: from 31 January, February, never March.
            const last = new Date(d.getFullYear(), d.getMonth() + steps + 1, 0).getDate()
            d = new Date(d.getFullYear(), d.getMonth() + steps, Math.min(d.getDate(), last))
        } else {
            d.setDate(d.getDate() + steps * (page.mode === "day" ? 1 : page.mode === "week" ? 7 : 15))
        }
        page.anchor = d
        page.load()
    }

    function title() {
        // Today as the window's clock has it: the title turns with it at midnight.
        const today = page.window.today
        if (page.mode === "agenda")
            return page.iso(page.anchor) === today ? page.sioul.text("agenda-next") : page.sioul.textWith("agenda-from-day", "day", page.anchor.toLocaleDateString(page.locale, "dddd d MMMM"))
        if (page.mode === "day") {
            // "Today, Saturday 3 October"; another day by its date alone.
            const named = page.anchor.toLocaleDateString(page.locale, "dddd d MMMM")
            const tomorrow = new Date(today + "T12:00:00")
            tomorrow.setDate(tomorrow.getDate() + 1)
            if (page.iso(page.anchor) === today)
                return page.sioul.text("agenda-today") + ", " + named
            if (page.iso(page.anchor) === page.iso(tomorrow))
                return page.sioul.text("agenda-tomorrow") + ", " + named
            return named
        }
        if (page.mode === "week")
            return page.sioul.textWith("agenda-week-of", "day", page.startOf(page.anchor, "week").toLocaleDateString(page.locale, "d MMMM"))
        return page.anchor.toLocaleDateString(page.locale, "MMMM yyyy")
    }

    // A new event on `day`: "2026-10-05", a date, or nothing for the day shown.
    function newEvent(day) {
        const shown = day instanceof Date ? page.iso(day) : day
        eventDialog.now().edit("", shown || page.iso(page.anchor))
    }

    // For the window's pictures: Show's menu open (it is given back), then closed.
    function openShow() {
        const menu = showMenu.now()
        const under = page.narrow ? showButtonNarrow : showButton
        if (!menu.opened)
            menu.popup(under, 0, under.height)
        return menu
    }

    function closeShow() {
        showMenu.close()
    }

    // For the window's tests.
    function fillEvent(text, from, to, how) {
        eventDialog.now().fill(text, from, to, how)
    }

    function saveEvent() {
        eventDialog.now().accept()
    }

    // For the window's pictures: the event form's costs and gain in view.
    function showEventRatings() {
        eventDialog.now().showRatings()
    }

    // A new event made from something else: a message, a task.
    function makeFrom(text, note, link) {
        eventDialog.now().makeFrom(text, note, link, page.iso(new Date()))
    }

    // An event of your own that could be a task instead: not a task's time
    // already, not a meeting with others (Rust's one rule: others expect you
    // there), not read only.
    function canBeTask(event) {
        return event !== null && (event.task || "") === "" && !event.read_only && !event.with_others
    }

    // A task made an appointment (the event form, its block's time, then
    // saved), or an appointment made a task, at once (21 October 2026).
    function changeKind(event) {
        if ((event.task || "") !== "") {
            eventDialog.now().makeFromTask(event.task, event.key, event.start, event.end)
            return
        }
        // What it cannot keep as a task, asked first (F5, 22 October 2026).
        const losses = page.sioul.eventToTaskLosses(event.key)
        if (losses !== "") {
            page.converting = event.key
            convertChoice.now().ask(event.summary, losses, page.sioul.text("agenda-make-task"))
            return
        }
        page.toTask(event.key)
    }

    function toTask(key) {
        const problem = page.sioul.eventToTask(key)
        if (problem === "")
            page.opened = null
        else
            page.sioul.status = problem
    }

    // A thing of the timeline tapped (a row: WeekPlanning.qml): an appointment
    // opens its details here; anything else where it opens elsewhere, until
    // each has its card (`window.openCard`).
    function openRow(row) {
        // A band of steps quiet time folds: those steps, drawn ("Show anyway").
        if (row.fold === "band") {
            page.showAnyway = true
            return
        }
        if (row.kind === "event" || (row.kind === "block" && row.owner === "")) {
            page.opened = page.eventOf(JSON.parse(row.json))
            return
        }
        page.window.openCard(row)
    }

    // Its menu (a right click, a long press): an appointment's or a block's
    // here; a step's; for a meal, a nap, the night or a dose, its day in Health.
    function menuRow(row) {
        if (row.kind === "event" || row.kind === "block") {
            page.menuTarget = page.eventOf(JSON.parse(row.json))
            eventMenu.now().popup()
        } else if ((row.kind === "step" || row.kind === "done") && row.owner !== "") {
            page.stepTarget = { owner: row.owner, title: row.title, held: row.certainty === "held" }
            stepMenu.now().popup()
        } else if (row.kind !== "gain" && row.kind !== "slack") {
            page.window.openCard(row)
        }
    }

    // Ctrl+E: its form; one that cannot be changed here opens where it lives.
    function editRow(row) {
        if ((row.kind === "event" || row.kind === "block") && !row.readOnly)
            eventDialog.now().edit(row.srcKey, "")
        else
            page.openRow(row)
    }

    // The days shown as one day (a day's title in the week).
    function showDay(date) {
        page.anchor = new Date(date + "T12:00:00")
        if (page.mode === "day")
            page.load()
        else
            page.mode = "day"
    }

    // Deleted: a repeating event asks whether it is this time or every time; a single one goes at once.
    function askDelete(target) {
        page.menuTarget = target
        if (!target.recurring) {
            page.deleteNow(false)
            return
        }
        deleteChoice.now().ask(target.summary, "")
    }

    function deleteNow(onlyThis) {
        if (page.menuTarget === null)
            return
        page.said(page.sioul.deleteEvent(page.menuTarget.key, page.menuTarget.start, onlyThis))
        page.opened = null
    }

    // An event by its file, from a link: its details, as from the agenda;
    // outside the days shown, its days are shown first. Its form ("Edit")
    // only when it cannot be found.
    function openEvent(key) {
        page.wanted = key
        if (page.showWanted()) {
            page.wanted = ""
            return
        }
        const found = JSON.parse(page.sioul.eventForm(key) || "null")
        if (!found || !found.edit || !found.edit.start) {
            page.wanted = ""
            eventDialog.now().edit(key, "")
            return
        }
        page.anchor = new Date(found.edit.start.slice(0, 10) + "T12:00:00")
        page.load()
        wantedLate.restart()
    }

    onModeChanged: page.load()
    Component.onCompleted: {
        page.takeShown()
        page.readAgain()
    }

    Shortcut {
        sequence: "Escape"
        enabled: page.visible && page.opened !== null
        onActivated: page.opened = null
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        ColumnLayout {
            visible: !(page.window.compact && page.opened !== null)
            Layout.fillHeight: true
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: page.opened === null ? columns.width : Math.round((columns.width - columns.spacing) * 0.6)
            spacing: 8

            // Two events at once in the next two weeks, the time to get there and
            // back counted: said above the agenda until you say to leave it.
            ColumnLayout {
                id: overlapsList

                // Made with the agenda, off the window's thread.
                readonly property var rows: JSON.parse(page.sioul.overlaps || "[]")

                visible: overlapsList.rows.length > 0
                Layout.fillWidth: true
                spacing: 4

                Repeater {
                    model: overlapsList.rows

                    delegate: RowLayout {
                        id: clash

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: {
                                const one = e => e.title + " (" + e.from + "–" + e.to + ")"
                                const day = new Date(clash.modelData.day + "T12:00:00").toLocaleDateString(page.locale, "dddd d MMMM")
                                // At once only when their times overlap; else too close, the time you set named with its place (22 October 2026).
                                return page.sioul.textArgs(clash.modelData.at_once ? "overlap-day" : "overlap-day-close", JSON.stringify({ day: day, first: one(clash.modelData.first), second: one(clash.modelData.second), gap: clash.modelData.gap, needed: clash.modelData.needed, source: clash.modelData.source }))
                            }
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                        Button {
                            flat: true
                            text: page.sioul.text("overlap-set-aside")
                            onClicked: page.sioul.setOverlapAside(clash.modelData.key)
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    text: page.sioul.text("agenda-today")
                    onClicked: {
                        page.anchor = new Date(page.window.today + "T12:00:00")
                        page.load()
                        // The same days: back to now all the same.
                        timeline.scrollToStart(true)
                    }
                }
                ToolButton {
                    text: "◂"
                    Accessible.name: page.sioul.text("agenda-earlier")
                    onClicked: page.move(-1)
                }
                ToolButton {
                    text: "▸"
                    Accessible.name: page.sioul.text("agenda-later")
                    onClicked: page.move(1)
                }
                Label {
                    visible: !page.narrow
                    Layout.fillWidth: true
                    // What is left of the line, its words cut short: it never widens the line.
                    Layout.preferredWidth: 0
                    text: page.title()
                    textFormat: Text.PlainText
                    font.pixelSize: 19
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                // The day, the week and the month, one switch away; on a phone,
                // in the title's room.
                PlainComboBox {
                    Layout.fillWidth: page.narrow
                    Layout.preferredWidth: page.narrow ? -1 : 150
                    model: page.modes.map(m => page.sioul.text("agenda-mode-" + m))
                    currentIndex: page.modes.indexOf(page.mode)
                    onActivated: index => page.mode = page.modes[index]
                }
                // What the Day and the Week show here: the plan's steps, meals and the night, doses, each calendar.
                // Words beside its icon on a wide window; on a phone, on the title's line.
                Button {
                    id: showButton

                    objectName: "agendaShow"
                    visible: !page.narrow && (page.mode === "day" || page.mode === "week")
                    Layout.preferredWidth: page.width < 1000 ? 40 : -1
                    text: page.sioul.text("agenda-show")
                    icon.name: "view-filter"
                    icon.color: page.theme.text
                    display: page.width < 1000 ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                    Accessible.name: page.sioul.text("agenda-show")
                    onClicked: page.openShow()
                }
                Button {
                    visible: page.shown.can_add !== false
                    Layout.preferredWidth: page.narrow ? 40 : -1
                    text: page.sioul.text("ui-new-event")
                    icon.name: "appointment-new"
                    icon.color: page.theme.text
                    display: page.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                    Accessible.name: page.sioul.text("ui-new-event")
                    onClicked: page.newEvent("")
                }
                SettingsButton {
                    visible: !page.narrow
                    sioul: page.sioul
                    theme: page.theme
                    view: "agenda"
                }
            }
            RowLayout {
                visible: page.narrow
                Layout.fillWidth: true
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    text: page.title()
                    textFormat: Text.PlainText
                    font.pixelSize: 19
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Button {
                    id: showButtonNarrow

                    visible: page.mode === "day" || page.mode === "week"
                    Layout.preferredWidth: 40
                    text: page.sioul.text("agenda-show")
                    icon.name: "view-filter"
                    icon.color: page.theme.text
                    display: AbstractButton.IconOnly
                    Accessible.name: page.sioul.text("agenda-show")
                    onClicked: page.openShow()
                }
                SettingsButton {
                    sioul: page.sioul
                    theme: page.theme
                    view: "agenda"
                }
            }

            // Today's head line, under the header: what now is for, what comes next.
            // On a phone's Day, the day's whole days first (it keeps three lines above its hours).
            RowLayout {
                id: headLine

                readonly property var day: page.mode === "day" && page.shown.days.length === 1 ? page.shown.days[0] : null
                readonly property var chips: headLine.day && page.narrow ? (headLine.day.head || []).filter(p => !(p.item.source && p.item.source.type === "calendar" && (page.view.calendars_hidden || []).indexOf(p.item.source.calendar) >= 0)) : []

                visible: (page.mode === "day" || page.mode === "week") && (page.headNow !== null || headLine.chips.length > 0)
                Layout.fillWidth: true
                spacing: 8

                Label {
                    objectName: "agendaDayChips"
                    visible: headLine.chips.length > 0
                    Layout.maximumWidth: headLine.width * (page.headNow !== null ? 0.4 : 1)
                    text: headLine.chips.length === 0 ? "" : headLine.chips[0].item.title + (headLine.chips.length > 1 ? "  " + page.sioul.textArgs("agenda-chips-more", JSON.stringify({ n: headLine.chips.length - 1 })) : "")
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: page.theme.muted

                    TapHandler {
                        onTapped: page.openRow(timeline.rowOf(headLine.chips[0], headLine.day, 0, ""))
                    }
                }
                Label {
                    objectName: "agendaHeadLine"
                    visible: page.headNow !== null
                    Layout.fillWidth: true
                    text: page.headNow ? page.headNow.text : ""
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: page.theme.text
                    Accessible.role: Accessible.StaticText
                    Accessible.name: page.headNow ? page.headNow.said : ""
                }
            }
            // The medicines could not be read here: said where their doses would be.
            Label {
                visible: (page.mode === "day" || page.mode === "week") && (page.shown.doses_note || "") !== ""
                Layout.fillWidth: true
                text: page.shown.doses_note || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            Label {
                visible: page.shown.sentence !== "" && page.mode === "agenda"
                Layout.fillWidth: true
                text: page.shown.sentence
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            // The days that hold something, and today.
            ListView {
                id: daysList

                visible: page.mode === "agenda"
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 12
                model: page.shown.days.filter(d => d.events.length > 0 || d.today)
                ScrollBar.vertical: ScrollBar {}
                // Opened on today (its first day, from today).
                onCountChanged: Qt.callLater(() => {
                    const today = daysList.model.findIndex(d => d.today)
                    if (today >= 0)
                        daysList.positionViewAtIndex(today, ListView.Beginning)
                })

                delegate: ColumnLayout {
                    id: day

                    required property var modelData

                    width: daysList.width - 12
                    spacing: 2

                    Label {
                        text: day.modelData.title
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        font.pixelSize: 16
                        color: day.modelData.date < page.window.today ? page.theme.muted : page.theme.text
                    }
                    Label {
                        visible: day.modelData.events.length === 0
                        text: page.sioul.text("agenda-free")
                        color: page.theme.muted
                    }
                    Repeater {
                        model: day.modelData.events

                        delegate: EventRow {
                            required property var modelData

                            Layout.fillWidth: true
                            compact: page.narrow
                            event: modelData
                            theme: page.theme
                            sioul: page.sioul
                            selected: page.opened !== null && page.opened.key === modelData.key && page.opened.start === modelData.start
                            onOpen: page.opened = modelData
                            onMenu: {
                                page.menuTarget = modelData
                                eventMenu.now().popup()
                            }
                        }
                    }
                }
            }

            // The Day and the Week: one timeline, the hours down, each thing as long as it lasts.
            WeekPlanning {
                id: timeline

                visible: page.mode === "week" || page.mode === "day"
                Layout.fillWidth: true
                Layout.fillHeight: true
                sioul: page.sioul
                theme: page.theme
                days: page.mode === "week" || page.mode === "day" ? page.shown.days : []
                frame: page.shown.frame || ({ from: 7 * 60, to: 22 * 60 })
                locale: page.locale
                // The window's clock: the line at now, today's column and its doses' marks follow it.
                now: page.window.now
                today: page.window.today
                marks: page.sioul.doseMarks ? JSON.parse(page.sioul.doseMarks) : null
                readAt: page.sioul.doseReadAt
                backAt: page.window.backAt
                showAnyway: page.showAnyway
                hide: page.hide
                // A phone's Day says its whole days on its head line.
                chipsHere: !(page.narrow && page.mode === "day")
                chosenId: page.opened ? page.opened.id || "" : ""
                onOpen: row => page.openRow(row)
                onMenu: row => page.menuRow(row)
                onEdit: row => page.editRow(row)
                onTurn: steps => page.move(steps)
                onOpenDay: date => page.showDay(date)
                onNewAt: (day, hour) => eventDialog.now().edit("", day, hour)
            }

            // The month: the days in a grid, three events at most in each, then "…".
            ColumnLayout {
                visible: page.mode === "month"
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 4

                DayOfWeekRow {
                    Layout.fillWidth: true
                    locale: page.locale
                }
                GridLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    columns: 7
                    columnSpacing: 4
                    rowSpacing: 4

                    Repeater {
                        model: page.mode === "month" ? page.shown.days : []

                        delegate: Rectangle {
                            id: cell

                            required property var modelData
                            readonly property bool inMonth: Number(cell.modelData.date.slice(5, 7)) === page.anchor.getMonth() + 1
                            // Today as the window's clock has it: it moves at midnight.
                            readonly property bool today: cell.modelData.date === page.window.today

                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            Layout.preferredWidth: 1
                            Layout.preferredHeight: 1
                            color: cell.today ? page.theme.surface : "transparent"
                            radius: page.theme.radius
                            border.color: page.theme.line
                            opacity: cell.inMonth ? 1 : 0.5

                            TapHandler {
                                onDoubleTapped: page.newEvent(cell.modelData.date)
                            }

                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 4
                                spacing: 2

                                Label {
                                    text: Number(cell.modelData.date.slice(8, 10))
                                    textFormat: Text.PlainText
                                    font.weight: cell.today ? Font.Bold : Font.Normal
                                    color: cell.today ? page.theme.accent : page.theme.text
                                }
                                Repeater {
                                    model: cell.modelData.events.slice(0, 3)

                                    delegate: RowLayout {
                                        id: monthEvent

                                        required property var modelData

                                        Layout.fillWidth: true
                                        spacing: 3

                                        // A task's time: the task mark (21 October 2026).
                                        Icon {
                                            visible: monthEvent.modelData.task !== ""
                                            iconName: "view-task"
                                            size: 12
                                            tip: page.sioul.text("agenda-task-mark")
                                        }
                                        Label {
                                            Layout.fillWidth: true
                                            text: monthEvent.modelData.summary
                                            textFormat: Text.PlainText
                                            elide: Text.ElideRight
                                            font.pixelSize: 12
                                            font.strikeout: monthEvent.modelData.cancelled
                                            color: page.theme.text
                                        }
                                        TapHandler {
                                            onTapped: page.opened = monthEvent.modelData
                                        }
                                    }
                                }
                                Label {
                                    visible: cell.modelData.events.length > 3
                                    text: "…"
                                    color: page.theme.muted
                                }
                                Item {
                                    Layout.fillHeight: true
                                }
                            }
                        }
                    }
                }
            }
        }

        // The event, read.
        Panel {
            visible: page.opened !== null
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.4)
            theme: page.theme

            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                Label {
                    Layout.fillWidth: true
                    text: page.opened ? page.opened.summary : ""
                    textFormat: Text.PlainText
                    font.pixelSize: 20
                    font.strikeout: page.opened !== null && page.opened.cancelled
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Label {
                    Layout.fillWidth: true
                    text: page.opened ? new Date(page.opened.start * 1000).toLocaleDateString(page.locale, "dddd d MMMM") + " · " + page.opened.when : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                // The time it keeps before and after it, its own, said: never hidden (22 October 2026).
                Label {
                    objectName: "agenda-opened-kept"
                    visible: page.opened !== null && ((page.opened.before || 0) > 0 || (page.opened.after || 0) > 0)
                    Layout.fillWidth: true
                    text: page.opened ? page.sioul.textArgs("agenda-kept", JSON.stringify({ before: page.opened.before || 0, after: page.opened.after || 0 })) : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                Label {
                    visible: page.opened !== null && page.opened.location !== ""
                    Layout.fillWidth: true
                    text: page.opened ? page.opened.location : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                RowLayout {
                    spacing: 6

                    Rectangle {
                        visible: !page.openedTask
                        Layout.preferredWidth: 10
                        Layout.preferredHeight: 10
                        radius: 5
                        color: page.opened && page.opened.color ? page.opened.color : page.theme.accent
                    }
                    // A task's time block says the task it is, and its list (21 October 2026).
                    Icon {
                        visible: page.openedTask
                        iconName: "view-task"
                        size: 16
                        tip: page.sioul.text("agenda-task-mark")
                    }
                    Label {
                        objectName: "agenda-opened-kind"
                        text: !page.opened ? "" : (page.openedTask ? page.sioul.textWith("agenda-task-of", "list", page.opened.list) : page.opened.calendar) + (page.opened.recurring ? "  ·  " + page.sioul.text("agenda-repeats") : "")
                        textFormat: Text.PlainText
                        color: page.theme.muted
                    }
                }
                // When it is reminded; changed here for every time it comes.
                Label {
                    visible: (page.reminder.line || "") !== ""
                    Layout.fillWidth: true
                    text: page.reminder.line || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                PlainComboBox {
                    visible: page.reminds.length > 0
                    Layout.fillWidth: page.narrow
                    Layout.preferredWidth: page.narrow ? -1 : 260
                    textRole: "label"
                    model: page.reminds.map(c => ({ value: c.value, label: page.theme.plain(c.label) }))
                    currentIndex: Math.max(0, page.reminds.findIndex(c => c.value === (page.reminder.remind || "")))
                    Accessible.name: page.sioul.text("event-remind")
                    onActivated: index => {
                        const problem = page.sioul.setEventReminder(page.opened.key, page.reminds[index].value)
                        if (problem === "")
                            page.readReminder()
                        else
                            page.sioul.status = problem
                    }
                }
                Button {
                    readonly property bool any: page.opened !== null && ((page.details.notes || "") !== "" || (page.details.organizer || "") !== "" || (page.details.attendees || []).length > 0)

                    visible: any
                    flat: true
                    text: (page.moreShown ? "▾  " : "▸  ") + page.sioul.text("ui-more-details")
                    onClicked: page.moreShown = !page.moreShown
                }
                // Its notes, their Markdown drawn: an invitation's are its sender's words.
                Description {
                    visible: page.moreShown && page.opened !== null && (page.details.notes || "") !== ""
                    Layout.fillWidth: true
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    markdown: page.details.notes || ""
                    // Its checklist ticked here: a task's that can be changed, an event's of
                    // a calendar you write to, never a meeting with others' (its organizer's words).
                    tickable: page.opened !== null && !page.opened.read_only && (page.openedTask || page.opened.with_others !== true)
                    // An item made a task of its own; a step, of a task's list.
                    converts: tickable
                    steps: page.openedTask
                    onConvert: (line, to) => {
                        const problem = page.openedTask ? page.sioul.convertItem("task", page.opened.task, line, to) : page.sioul.convertItem("event", page.opened.key, line, to)
                        if (problem !== "")
                            page.sioul.status = problem
                    }
                    onCheck: (line, state) => {
                        const problem = page.openedTask ? page.sioul.checkItem("task", page.opened.task, line, state) : page.sioul.checkItem("event", page.opened.key, line, state)
                        if (problem !== "")
                            page.sioul.status = problem
                    }
                }
                Label {
                    visible: page.moreShown && page.opened !== null && (page.details.organizer || "") !== ""
                    Layout.fillWidth: true
                    text: page.opened ? page.sioul.textWith("agenda-organizer", "name", page.details.organizer || "") : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                Repeater {
                    model: page.moreShown && page.opened ? page.details.attendees || [] : []

                    delegate: Label {
                        id: guest

                        required property var modelData

                        Layout.fillWidth: true
                        text: (guest.modelData.name || guest.modelData.address) + "  ·  " + page.sioul.text("answer-" + guest.modelData.answer)
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: page.theme.muted
                        font.pixelSize: 13
                    }
                }
                // Which it is, a task or an appointment, is yours to say (21 October 2026).
                Button {
                    objectName: "agenda-change-kind"
                    visible: page.opened !== null && (page.openedTask || page.canBeTask(page.opened))
                    flat: true
                    text: page.openedTask ? page.sioul.text("agenda-make-appointment") : page.sioul.text("agenda-make-task")
                    onClicked: page.changeKind(page.opened)
                }
                // What prepares it: its task, with its steps and list (opened by a tap); else steps to
                // add, or a task made from it to use as its steps (docs/tasks.md, "An appointment and its steps").
                ColumnLayout {
                    id: pairing

                    // {task, adoptable, can}, read again when the event or the tasks change.
                    property var shown: null

                    function read() {
                        pairing.shown = page.opened && !page.openedTask ? JSON.parse(page.sioul.eventPairing(page.opened.key) || "null") : null
                    }

                    visible: pairing.shown !== null
                    Layout.fillWidth: true
                    spacing: 4

                    Connections {
                        target: page
                        function onOpenedChanged() {
                            pairing.read()
                        }
                    }
                    Connections {
                        target: page.sioul
                        function onTasksChanged() {
                            pairing.read()
                        }
                    }
                    Label {
                        objectName: "agendaPairing"
                        visible: !!pairing.shown && !!pairing.shown.task
                        Layout.fillWidth: true
                        text: pairing.shown && pairing.shown.task ? page.sioul.textWith("pair-prepared-by", "title", pairing.shown.task.title) + (pairing.shown.task.next ? "  ·  " + pairing.shown.task.next : "") : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: page.theme.text

                        TapHandler {
                            onTapped: page.window.openThing({ kind: "task", uri: "sioul:task/" + encodeURIComponent(pairing.shown.task.uid), key: pairing.shown.task.uid })
                        }
                    }
                    Button {
                        objectName: "agendaAddSteps"
                        visible: !!pairing.shown && pairing.shown.can
                        flat: true
                        text: "+  " + page.sioul.text("pair-add-steps")
                        onClicked: {
                            const answer = JSON.parse(page.sioul.stepsForEvent(page.opened.key) || "{}")
                            if (answer.error)
                                page.sioul.status = answer.error
                            else
                                page.window.openThing({ kind: "task", uri: "sioul:task/" + encodeURIComponent(answer.uid), key: answer.uid })
                        }
                    }
                    Repeater {
                        model: pairing.shown ? pairing.shown.adoptable : []

                        delegate: Button {
                            id: adopt

                            required property var modelData

                            flat: true
                            text: page.theme.plain(page.sioul.textWith("pair-adopt", "title", adopt.modelData.title))
                            onClicked: {
                                const problem = page.sioul.attachToEvent(adopt.modelData.uid, page.opened.key)
                                if (problem !== "")
                                    page.sioul.status = problem
                                pairing.read()
                            }
                        }
                    }
                }
                // What it is tied to, and what can be made from it.
                RelatedList {
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    sioul: page.sioul
                    theme: page.theme
                    title: page.sioul.text("related-title")
                    uri: page.source ? page.source.uri : ""
                    onOpenThing: item => page.window.openThing(item)
                }
                ThingActions {
                    visible: page.source !== null
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    source: page.source
                }
                Item {
                    Layout.fillHeight: true
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: page.theme.gap

                    Button {
                        visible: page.opened !== null && !page.opened.read_only
                        text: page.sioul.text("ui-edit")
                        onClicked: eventDialog.now().edit(page.opened.key, "")
                    }
                    // Never a meeting with others: its server would cancel or decline it for them.
                    Button {
                        visible: page.opened !== null && !page.opened.read_only && page.opened.with_others !== true
                        flat: true
                        text: page.sioul.text("ui-delete")
                        onClicked: page.askDelete(page.opened)
                    }
                    Button {
                        visible: page.openedTask
                        flat: true
                        text: page.sioul.text("agenda-open-task")
                        onClicked: page.window.openTask(page.opened.task)
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    Button {
                        text: page.sioul.text("ui-close")
                        onClicked: page.opened = null
                    }
                }
            }
        }
    }

    // Right click on an event.
    Later {
        id: eventMenu

        sourceComponent: Component {
            SioulMenu {
                id: eventMenuForm

                readonly property var source: page.menuTarget && page.menuTarget.uid ? { uri: "sioul:event/" + encodeURIComponent(page.menuTarget.uid), kind: "event", key: page.menuTarget.key, title: page.menuTarget.summary, start: page.menuTarget.start } : null

                // Its details first, as a click gives them; then its form.
                MenuItem {
                    enabled: page.menuTarget !== null
                    text: page.sioul.text("ui-details")
                    onTriggered: page.opened = page.menuTarget
                }
                MenuItem {
                    enabled: page.menuTarget !== null && !page.menuTarget.read_only
                    text: page.sioul.text("ui-edit")
                    onTriggered: eventDialog.now().edit(page.menuTarget.key, "")
                }
                MenuItem {
                    enabled: page.menuTarget !== null && !page.menuTarget.read_only && page.menuTarget.with_others !== true
                    text: page.sioul.text("ui-delete")
                    onTriggered: page.askDelete(page.menuTarget)
                }
                MenuItem {
                    readonly property bool task: page.menuTarget !== null && (page.menuTarget.task || "") !== ""

                    visible: task || page.canBeTask(page.menuTarget)
                    height: visible ? implicitHeight : 0
                    text: task ? page.sioul.text("agenda-make-appointment") : page.sioul.text("agenda-make-task")
                    onTriggered: page.changeKind(page.menuTarget)
                }
                MenuSeparator {}
                AddMenu {
                    sioul: page.sioul
                    window: page.window
                    source: eventMenuForm.source
                }
                MenuItem {
                    enabled: eventMenuForm.source !== null
                    text: page.sioul.text("ui-link-existing")
                    onTriggered: page.window.linkFrom(eventMenuForm.source)
                }
            }
        }
    }

    // Show: what the Day and the Week draw on this device.
    Later {
        id: showMenu

        sourceComponent: Component {
            SioulMenu {
                id: showForm

                MenuItem {
                    text: page.sioul.text("agenda-show-steps")
                    checkable: true
                    checked: page.view.steps !== false
                    onTriggered: page.showPart("steps", checked)
                }
                MenuItem {
                    text: page.sioul.text("agenda-show-needs")
                    checkable: true
                    checked: page.view.needs !== false
                    onTriggered: page.showPart("needs", checked)
                }
                MenuItem {
                    text: page.sioul.text("agenda-show-doses")
                    checkable: true
                    checked: page.view.doses !== false
                    onTriggered: page.showPart("doses", checked)
                }
                // Quiet time's bands on coming days, drawn as the steps they hold: for now, not kept.
                MenuItem {
                    visible: page.anyFolded
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("agenda-show-quiet")
                    checkable: true
                    checked: page.showAnyway
                    onTriggered: page.showAnyway = checked
                }
                MenuSeparator {
                    visible: page.calendarsShown.length > 0
                    height: visible ? implicitHeight : 0
                }
                Instantiator {
                    model: page.calendarsShown

                    delegate: MenuItem {
                        id: calendarLine

                        required property string modelData

                        // "&" marks a shortcut in a menu: "&&" is one.
                        text: page.theme.plain(calendarLine.modelData.replace(/&/g, "&&"))
                        checkable: true
                        checked: (page.view.calendars_hidden || []).indexOf(calendarLine.modelData) < 0
                        onTriggered: page.showCalendar(calendarLine.modelData, checked)
                    }
                    onObjectAdded: (index, object) => showForm.insertItem(5 + index, object)
                    onObjectRemoved: (index, object) => showForm.removeItem(object)
                }
            }
        }
    }

    // A step's menu: open its task; given its time by hand, back to the plan.
    Later {
        id: stepMenu

        sourceComponent: Component {
            SioulMenu {
                MenuItem {
                    text: page.sioul.text("ui-open")
                    onTriggered: page.window.openTask(page.stepTarget.owner)
                }
                MenuItem {
                    visible: page.stepTarget !== null && page.stepTarget.held
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("day-let-plan")
                    onTriggered: page.said(page.sioul.setTaskAt(page.stepTarget.owner, ""))
                }
            }
        }
    }

    // A repeating event deleted: this time, or every time (WhichTimes.qml, one with moving).
    Later {
        id: deleteChoice

        sourceComponent: Component {
            WhichTimes {
                sioul: page.sioul
                theme: page.theme
                onChosen: onlyThis => page.deleteNow(onlyThis)
            }
        }
    }

    Later {
        id: convertChoice

        sourceComponent: Component {
            ConfirmDialog {
                sioul: page.sioul
                theme: page.theme
                onConfirmed: page.toTask(page.converting)
            }
        }
    }

    Later {
        id: eventDialog

        sourceComponent: Component {
            EventDialog {
                id: eventDialogForm

                sioul: page.sioul
                theme: page.theme
                onSaved: page.opened = null
            }
        }
    }
}
