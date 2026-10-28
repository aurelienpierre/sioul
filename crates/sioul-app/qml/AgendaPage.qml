// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The agenda: what comes, from today, as a calm list of days; the day, the
// week and the month one switch away. The Day and the Week are one timeline
// of everything that has a time (WeekPlanning.qml): appointments, the plan's
// steps, meals, naps, the night, doses. Nothing is late or overdue: the past
// is not shown unless you go back to it. Anything tapped opens its card where
// you are (`window.openCard`, CardHost.qml), beside the page; the open thing
// is outlined. New events are made here.

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
    }

    // The card open, as the views outline it: an appointment by its file and start.
    readonly property var cardRef: page.window.cardRef
    function isOpen(event) {
        return page.cardRef !== null && page.cardRef.card === "event" && page.cardRef.srcKey === event.key && (page.cardRef.srcStart === event.start || page.cardRef.itemId === event.id)
    }

    // An appointment of the list or the month: its card, where you are.
    function openEventRow(event) {
        page.window.openCard({ kind: "event", srcKey: event.key, srcStart: event.start, itemId: event.id, title: event.summary })
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
    // What the page keeps beside an open card: the Week's grid, else a list's room (main.qml).
    readonly property real cardRoom: page.mode === "week" ? 830 : 480
    // The step whose menu is open (a row of the timeline).
    property var stepTarget: null
    // "Show anyway": the steps quiet time folds on coming days, drawn.
    property bool showAnyway: false
    // What the Day and the Week show on this device ("Show", agenda-view.toml):
    // {steps, needs, doses, calendars: [{id, name}]}, a calendar by its place
    // ("account/id"), two of one name apart; all shown until you say.
    property var view: JSON.parse(page.sioul.agendaView() || "{}")
    readonly property var hiddenCalendars: (page.view.calendars || []).map(c => c.id)
    readonly property var hide: ({ steps: page.view.steps === false, needs: page.view.needs === false, doses: page.view.doses === false, calendars: page.hiddenCalendars })
    // The calendars of the days shown, and those Show hides: Show's list, {id, name}.
    readonly property var calendarsShown: {
        const out = (page.view.calendars || []).slice()
        for (const day of page.shown.days)
            for (const row of (day.head || []).concat(day.timed || [])) {
                const source = row.item.source || {}
                if (source.type === "calendar" && source.collection && !out.some(c => c.id === source.collection))
                    out.push({ id: source.collection, name: source.calendar || source.collection })
            }
        return out.sort((a, b) => a.name.localeCompare(b.name) || a.id.localeCompare(b.id))
    }
    // Whether a day shown has steps quiet time folds: "Show anyway" offered.
    readonly property bool anyFolded: page.shown.days.some(d => (d.bands || []).length > 0)

    // Show changed: `part` ("steps", "needs", "doses") on or off, kept on this device.
    function showPart(part, on) {
        const changed = Object.assign({}, page.view)
        changed[part] = on
        page.saveView(changed)
    }

    function showCalendar(calendar, on) {
        const hidden = (page.view.calendars || []).filter(c => c.id !== calendar.id)
        if (!on)
            hidden.push({ id: calendar.id, name: calendar.name })
        page.saveView(Object.assign({}, page.view, { calendars: hidden }))
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

    // A thing of the timeline tapped (a row: WeekPlanning.qml): its card, where you are.
    function openRow(row) {
        // A band of steps quiet time folds: those steps, drawn ("Show anyway").
        if (row.fold === "band") {
            page.showAnyway = true
            return
        }
        page.window.openCard(row)
    }

    // Its menu (a right click, a long press): an appointment's or a block's, a
    // step's, a meal's, a nap's or the night's; a dose's opens its day in Health.
    function menuRow(row) {
        if (row.kind === "event" || row.kind === "block") {
            page.window.cardHost.eventMenu(row)
        } else if ((row.kind === "step" || row.kind === "done") && row.owner !== "") {
            page.stepTarget = { owner: row.owner, title: row.title, held: row.certainty === "held" }
            stepMenu.now().popup()
        } else if (row.kind === "meal" || row.kind === "nap" || row.kind === "night") {
            page.window.cardHost.needMenu(row)
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

    // An event by its file, from a link: its card, where you are (the next time it comes).
    function openEvent(key) {
        page.window.openCard({ kind: "event", srcKey: key, srcStart: 0 })
    }

    onModeChanged: page.load()
    Component.onCompleted: {
        page.takeShown()
        page.readAgain()
    }


    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        ColumnLayout {
            Layout.fillHeight: true
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: columns.width
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
                readonly property var chips: headLine.day && page.narrow ? (headLine.day.head || []).filter(p => !(p.item.source && p.item.source.type === "calendar" && page.hiddenCalendars.indexOf(p.item.source.collection) >= 0)) : []

                visible: (page.mode === "day" || page.mode === "week") && (page.headNow !== null || headLine.chips.length > 0)
                Layout.fillWidth: true
                spacing: 8

                // A date asked comes first in a day's head: on a phone its diamond and its words
                // ("CAF form · Date asked · four days left"), as the computer's Day says it (later 2).
                DayHead {
                    sioul: page.sioul
                    theme: page.theme
                    planning: timeline
                    chips: headLine.chips
                    Layout.maximumWidth: headLine.width * (page.headNow !== null ? 0.4 : 1)
                    onOpened: page.openRow(timeline.rowOf(headLine.chips[0], headLine.day, 0, ""))
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
                            selected: page.isOpen(modelData)
                            onOpen: page.openEventRow(modelData)
                            onMenu: page.window.cardHost.eventMenu({ kind: "event", srcKey: modelData.key, srcStart: modelData.start, itemId: modelData.id, title: modelData.summary })
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
                chosenId: page.cardRef ? page.cardRef.itemId || "" : ""
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
                                            onTapped: page.openEventRow(monthEvent.modelData)
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

                        required property var modelData

                        // "&" marks a shortcut in a menu: "&&" is one.
                        text: page.theme.plain(calendarLine.modelData.name.replace(/&/g, "&&"))
                        checkable: true
                        checked: page.hiddenCalendars.indexOf(calendarLine.modelData.id) < 0
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

    Later {
        id: eventDialog

        sourceComponent: Component {
            EventDialog {
                sioul: page.sioul
                theme: page.theme
            }
        }
    }
}
