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
    readonly property var shown: page.agendaText ? JSON.parse(page.agendaText) : ({ days: [], sentence: "", from: "" })

    // What comes' rows of a day (version 3's `comes`, `timeline::comes`), in time order:
    // its whole-day things first, tonight's night last; today's doses by the minute, as the
    // Porch lists them: from their time on until marked, with their mark and what is known
    // (the window's rows, `timeline.markState`), those still to come plainly; never a name.
    function comesRows(day) {
        const comes = day.comes || { rows: [] }
        const out = []
        for (const placed of (comes.rows || []).filter(p => page.shownHere(p.item))) {
            const item = placed.item
            const detail = placed.detail || {}
            out.push({ sort: item.kind === "night" ? 3000 : placed.all_day === true ? -1 : placed.from_minute, id: item.id, when: placed.when, title: item.title, line: [placed.both || "", item.line || "", detail.location || ""].filter(t => t !== "").join(" · "), kind: item.kind, tone: item.tone || "plain", colour: item.colour || "transparent", known: "", cancelled: detail.cancelled === true, tentative: detail.tentative === true, placed: placed, keys: [] })
        }
        // Show leaves the doses out here as in the Day and the Week (review of 21436bf, later 2).
        for (const dose of page.hide.doses ? [] : comes.doses || []) {
            let known = ""
            let words = ""
            if (day.today) {
                const state = timeline.markState(JSON.stringify(dose.keys), day.date)
                if (state.state === "taken")
                    continue
                if (state.state !== "not-come") {
                    known = state.state
                    words = state.words || ""
                }
            }
            out.push({ sort: dose.minute, id: "doses:" + day.date + "/" + dose.minute, when: dose.when, title: dose.title, line: words, kind: "dose", tone: "plain", colour: "transparent", known: known, cancelled: false, tentative: false, placed: null, keys: dose.keys })
        }
        return out.concat(page.spanRows(day)).sort((a, b) => a.sort - b.sort)
    }

    // The events over several days shown, each once, by its item's id
    // (`spans`, `ongoing::spans`): what a list's row of one says, what its line opens.
    readonly property var spanEvents: {
        const out = {}
        for (const placed of page.shown.spans || [])
            out[placed.item.id] = placed
        return out
    }
    // The room the lines take at a list's left edge, while a day shown has one.
    readonly property int lineRoom: page.shown.days.some(d => ((d.through || {}).lines || []).length > 0) ? 26 : 0

    // A list's rows of a day for its events over several days (`ongoing::Through`): an
    // event's row on its first day shown, its whole span under its title ("Ongoing
    // since …" when the list starts after it began), its end mark's row on its last.
    function spanRows(day) {
        const out = []
        for (const r of (day.through || {}).rows || []) {
            const placed = page.spanEvents[r.id]
            if (!placed || !page.shownHere(placed.item))
                continue
            const item = placed.item
            const detail = placed.detail || {}
            const place = r.mark === "end" ? "" : detail.location || ""
            out.push({ sort: r.minute, id: item.id, when: r.when, title: item.title, line: [r.line, place].filter(t => t !== "").join(" · "), kind: item.kind, tone: item.tone || "plain", colour: item.colour || "transparent", known: "", cancelled: detail.cancelled === true, tentative: detail.tentative === true, placed: placed, keys: [], mark: r.mark })
        }
        return out
    }

    // A phone's Month list's rows of a day: its lines (`month_lines`) but the copies of an
    // event over several days, which it writes once (`spanRows`), in time order.
    function monthRows(day) {
        return (day.lines || []).filter(p => page.shownHere(p.item) && p.spans !== true).map(page.monthRow).concat(page.spanRows(day)).sort((a, b) => a.sort - b.sort)
    }
    // The room the lines take at the phone's Month list's left edge.
    readonly property int monthLineRoom: page.mode === "month" && page.narrow && page.shown.days.some(d => ((d.through || {}).lines || []).length > 0) ? 26 : 0

    // The Month's bars (`ongoing::Bar`), those of a calendar Show hides left out; how many
    // lanes each week's row takes; a bar's height.
    readonly property var monthBars: (page.shown.bars || []).filter(b => {
        const placed = page.spanEvents[b.id]
        return !placed || page.shownHere(placed.item)
    })
    readonly property var monthBarLanes: {
        const out = [0, 0, 0, 0, 0, 0]
        for (const b of page.monthBars)
            out[b.row] = Math.max(out[b.row] || 0, b.lane + 1)
        return out
    }
    readonly property int monthBarHeight: 15

    // An event over several days' lines through a day of a list (`lines`, `ongoing::Line`),
    // made only for a day that has some, so that the lists cost nothing more without
    // them: SpanLine.qml on `parent` (the list's content), from `from(line)` to
    // `to(line)` there, opening with `open(line)`; those `made` before destroyed.
    function layLines(made, parent, lines, from, to, open) {
        for (const item of made || [])
            item.destroy()
        const out = []
        for (const l of lines) {
            const placed = page.spanEvents[l.id]
            const item = spanLine.createObject(parent, { theme: page.theme, span: l, title: placed ? placed.item.title : "", fromY: Qt.binding(() => from(l)), toY: Qt.binding(() => to(l)) })
            item.open.connect(() => open(l))
            out.push(item)
        }
        return out
    }

    Component {
        id: spanLine

        SpanLine {}
    }

    // An event over several days opened from its line: its card where you are.
    function openSpan(id, day) {
        const placed = page.spanEvents[id]
        if (placed)
            page.openListed({ kind: placed.item.kind, placed: placed }, day)
    }

    // What Show keeps on this device, in What comes and the Month as in the Day and the Week
    // (WeekPlanning's `shown`): the plan's steps, meals and rest, a calendar by its place
    // (review of 21436bf, later 2). The dates asked are never left out (decision 8).
    function shownHere(item) {
        const hide = page.hide
        if (hide.steps && (item.kind === "step" || item.kind === "done"))
            return false
        if (hide.needs && (item.kind === "meal" || item.kind === "nap" || item.kind === "night"))
            return false
        if ((item.kind === "event" || item.kind === "block" || item.kind === "leg") && hide.calendars.indexOf((item.source || {}).collection) >= 0)
            return false
        return true
    }

    // The plan's line of a day in What comes, unless Show leaves the steps out.
    function comesPlan(day) {
        return page.hide.steps || !day.comes ? "" : day.comes.plan || ""
    }

    // A row of What comes or of a phone's Month list: its card where you are; a dose's, its day in Health.
    function openListed(row, day) {
        if (row.kind === "dose")
            page.window.openCard({ kind: "dose", date: day.date, srcKey: row.keys[0] || "" })
        else
            page.openRow(timeline.rowOf(row.placed, day, 0, ""))
    }

    function menuListed(row, day) {
        if (row.kind !== "dose")
            page.menuRow(timeline.rowOf(row.placed, day, 0, ""))
    }

    // A Month's line, as the lists' rows are.
    function monthRow(placed) {
        const item = placed.item
        const detail = placed.detail || {}
        return { sort: placed.all_day === true ? -1 : placed.from_minute, id: item.id, when: placed.when, title: item.title, line: placed.both || "", kind: item.kind, tone: item.tone || "plain", colour: item.colour || "transparent", known: "", cancelled: detail.cancelled === true, tentative: detail.tentative === true, placed: placed, keys: [] }
    }

    function takeShown() {
        if (page.visible)
            page.agendaText = page.sioul.agenda
    }

    // The card open, as the views outline it: its thing by its id.
    readonly property var cardRef: page.window.cardRef
    function isOpen(row) {
        return page.cardRef !== null && (page.cardRef.itemId || "") === row.id
    }

    // Children's events asked about now (several agendas, part 3: `children::questions`), the nearest first.
    property var questions: []
    function readQuestions() {
        try {
            page.questions = page.visible ? JSON.parse(page.sioul.holderQuestions() || "[]") : []
        } catch (e) {
            page.questions = []
        }
    }
    // A question answered: "me", "other", "accept", "decline" or "later".
    function answerQuestion(question, who) {
        const problem = page.sioul.takeChild(question.key, question.start, who)
        if (problem !== "")
            page.sioul.status = problem
        page.readQuestions()
    }

    Connections {
        target: page.sioul

        function onAgendaChanged() {
            page.takeShown()
            page.readQuestions()
        }
    }
    onVisibleChanged: {
        page.takeShown()
        page.readQuestions()
        if (page.visible) {
            page.readAgain()
            if (page.dosesAsked)
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
    // What every view shows on this device ("Show", agenda-view.toml):
    // {steps, needs, doses, legs}; all shown until you say.
    property var view: JSON.parse(page.sioul.agendaView() || "{}")
    // What the views leave out: Show's parts, and the agendas the filter leaves out now, by
    // place ("account/id", the agenda's `hidden`: what the hours admit, or your word until
    // work's hours change; AgendaFilter.qml).
    readonly property var hide: ({ steps: page.view.steps === false, needs: page.view.needs === false, doses: page.view.doses === false, calendars: page.shown.hidden || [] })
    // Whether a day shown has steps quiet time folds: "Show anyway" offered.
    readonly property bool anyFolded: page.shown.days.some(d => (d.bands || []).length > 0)

    // Show changed: `part` ("steps", "needs", "doses") on or off, kept on this device.
    function showPart(part, on) {
        const changed = Object.assign({}, page.view)
        changed[part] = on
        page.saveView(changed)
    }

    function saveView(changed) {
        const problem = page.sioul.setAgendaView(JSON.stringify(changed))
        if (problem !== "") {
            page.sioul.status = problem
        } else {
            page.view = changed
            // Rides as events of their own are made in Rust: laid out again.
            page.readAgain()
        }
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
    // Today shown, in the Day, the Week or What comes: the doses' marks and rows, and today's
    // steps, follow the clock; the doses' reading asked at each minute, on showing and on
    // coming back, today laid again at each five minutes' turn (review of 21436bf, M2).
    readonly property bool todayShown: (page.mode === "day" || page.mode === "week" || page.mode === "agenda") && page.shown.days.some(d => d.date === page.window.today)
    // The doses' reading asked of the Health job while today is shown, but not in What comes
    // while Show leaves the doses out there: rows it would not show (review of 679f96a, later 9).
    readonly property bool dosesAsked: page.todayShown && !(page.mode === "agenda" && page.hide.doses)

    // An instant (ms) as a date whose own clock reads the time zone Sioul shows
    // (`zone::current`, Settings ▸ Agenda ▸ Time zone; `zoneOffset`, JavaScript's own offset back).
    function wall(ms) {
        return new Date(ms + (typeof page.sioul.zoneOffset === "function" ? page.sioul.zoneOffset(ms / 1000) + new Date(ms).getTimezoneOffset() * 60000 : 0))
    }

    function today() {
        return page.wall(Date.now())
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

    property date anchor: page.wall(Date.now())

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
            if (page.visible && page.dosesAsked)
                page.sioul.refreshHealth()
        }

        function onNowChanged() {
            if (!page.visible)
                return
            // While today is drawn: the doses' marks read again at each minute (the Health job, one run for all who ask).
            if (page.dosesAsked)
                page.sioul.refreshHealth()
            if (page.window.today === page.readOn && Math.floor(page.window.now / 300) === Math.floor(page.readAt / 300))
                return
            // Today laid again from now, cheaply (the plan as last made): its publish brings the agenda's days.
            if (page.todayShown && page.window.today === page.readOn) {
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

    // For the window's pictures: Agenda ⚙, open at one of its settings or closed, and as an image.
    function openSettings(open, key) {
        const button = page.narrow ? agendaSettingsNarrow : agendaSettings
        button.show(open)
        if (open && key)
            Qt.callLater(() => button.scrollTo(key))
    }

    // For the window's images: one agenda's line opened in Agenda ⚙ ▸ Your agendas.
    function openAgendaLine(id) {
        const setup = (page.narrow ? agendaSettingsNarrow : agendaSettings).part("agendasSetup")
        if (setup !== null)
            setup.toggle(id)
    }

    function grabSettings(path) {
        (page.narrow ? agendaSettingsNarrow : agendaSettings).grab(path)
    }

    // For the window's pictures: the agenda filter's list open (it is given back), then closed.
    function openFilter() {
        return (page.narrow ? filterButtonNarrow : filterButton).open()
    }

    function closeFilter() {
        filterButton.close()
        filterButtonNarrow.close()
    }

    // A child's week, who does what (part 4), as the filter opens it beside its agenda.
    function whoDoesWhat(child) {
        return (page.narrow ? filterButtonNarrow : filterButton).whoDoesWhat(child)
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
        eventDialog.now().makeFrom(text, note, link, page.iso(page.wall(Date.now())))
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
        if (row.kind === "event" || row.kind === "block" || row.kind === "leg") {
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
        if ((row.kind === "event" || row.kind === "block" || row.kind === "leg") && !row.readOnly)
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
                // Which agendas every view draws (several agendas): the one filter, on a phone on the title's line.
                AgendaFilter {
                    id: filterButton

                    visible: !page.narrow && (page.shown.filter || { groups: [] }).groups.length > 0
                    sioul: page.sioul
                    theme: page.theme
                    shown: page.shown.filter || ({ chosen: false, groups: [] })
                    window: page.window
                    iconOnly: page.width < 1000
                    Layout.preferredWidth: page.width < 1000 ? 40 : -1
                }
                // What every view shows here: the plan's steps, meals and the night, doses, rides.
                // Words beside its icon on a wide window; on a phone, on the title's line.
                Button {
                    id: showButton

                    objectName: "agendaShow"
                    visible: !page.narrow
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
                    id: agendaSettings

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
                    // What the buttons leave of the line, its words wrapping: it never widens the line.
                    Layout.preferredWidth: 0
                    text: page.title()
                    textFormat: Text.PlainText
                    font.pixelSize: 19
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                AgendaFilter {
                    id: filterButtonNarrow

                    visible: (page.shown.filter || { groups: [] }).groups.length > 0
                    Layout.preferredWidth: 40
                    sioul: page.sioul
                    theme: page.theme
                    shown: page.shown.filter || ({ chosen: false, groups: [] })
                    window: page.window
                    narrow: true
                }
                Button {
                    id: showButtonNarrow

                    visible: true
                    Layout.preferredWidth: 40
                    text: page.sioul.text("agenda-show")
                    icon.name: "view-filter"
                    icon.color: page.theme.text
                    display: AbstractButton.IconOnly
                    Accessible.name: page.sioul.text("agenda-show")
                    onClicked: page.openShow()
                }
                SettingsButton {
                    id: agendaSettingsNarrow

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
                readonly property var chips: headLine.day && page.narrow ? (headLine.day.head || []).filter(p => !(p.item.source && p.item.source.type === "calendar" && page.hide.calendars.indexOf(p.item.source.collection) >= 0)) : []

                // Above What comes too, while it shows today (§4.2).
                visible: (page.mode === "day" || page.mode === "week" || page.mode === "agenda") && (page.headNow !== null || headLine.chips.length > 0)
                Layout.fillWidth: true
                spacing: 8

                // A date asked comes first in a day's head: on a phone its diamond and its words
                // ("CAF form · Date asked · four days left"), as the computer's Day says it (later 2).
                DayHead {
                    sioul: page.sioul
                    theme: page.theme
                    planning: timeline
                    chips: headLine.chips
                    // Its share of the page, never of the line it sits in: a layout in a layout fills
                    // by default, and a width read from the line rearranged the line again and again.
                    Layout.fillWidth: page.headNow === null
                    Layout.maximumWidth: Math.max(0, (page.width - 2 * page.theme.gap) * (page.headNow !== null ? 0.4 : 1))
                    onOpened: page.openRow(timeline.rowOf(headLine.chips[0], headLine.day, 0, ""))
                }
                Label {
                    objectName: "agendaHeadLine"
                    visible: page.headNow !== null
                    Layout.fillWidth: true
                    // What is left of the line, its words cut short: it never widens the page (found in the phone's pictures).
                    Layout.preferredWidth: 0
                    text: page.headNow ? page.headNow.text : ""
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: page.theme.text
                    Accessible.role: Accessible.StaticText
                    Accessible.name: page.headNow ? page.headNow.said : ""
                }
            }
            // One question about a child's event at a time, asked once (part 3): who takes it.
            ColumnLayout {
                id: question

                readonly property var asked: page.questions.length > 0 ? page.questions[0] : null

                objectName: "agendaQuestion"
                visible: question.asked !== null
                Layout.fillWidth: true
                spacing: 4

                Label {
                    Layout.fillWidth: true
                    text: question.asked ? question.asked.line : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Label {
                    visible: question.asked !== null && question.asked.why !== ""
                    Layout.fillWidth: true
                    text: question.asked ? question.asked.why : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 8

                    Repeater {
                        model: !question.asked ? [] : question.asked.asking === "proposed"
                            ? [{ who: "accept", label: page.sioul.text("holder-take-it") }, { who: "decline", label: page.sioul.text("holder-not-this-time") }, { who: "later", label: page.sioul.text("holder-later") }]
                            : [{ who: "me", label: page.sioul.text("holder-me") }].concat(question.asked.other !== "" ? [{ who: "other", label: question.asked.other }] : []).concat([{ who: "later", label: page.sioul.text("holder-later") }])

                        delegate: Button {
                            id: questionAnswer

                            required property var modelData

                            objectName: "agendaQuestionAnswer"
                            text: page.theme.plain(questionAnswer.modelData.label)
                            highlighted: question.asked !== null && question.asked.proposal === questionAnswer.modelData.who
                            onClicked: page.answerQuestion(question.asked, questionAnswer.modelData.who)
                        }
                    }
                }
            }
            // The medicines could not be read here: said where their doses would be.
            Label {
                visible: (page.mode === "day" || page.mode === "week" || (page.mode === "agenda" && !page.hide.doses)) && (page.shown.doses_note || "") !== ""
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

            // What comes: the days that hold something, and today (the unified agenda, §4.6).
            ListView {
                id: daysList

                visible: page.mode === "agenda"
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 12
                model: page.mode === "agenda" ? page.shown.days.filter(d => d.today || page.comesRows(d).length > 0 || page.comesPlan(d) !== "") : []
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
                    readonly property var rows: page.comesRows(day.modelData)
                    readonly property string plan: page.comesPlan(day.modelData)
                    readonly property var through: day.modelData.through || ({ lines: [], more: 0, rows: [] })
                    // Its events over several days' lines, made only while it has some (`layLines`).
                    property var spans: []

                    // The middle of the day's row of an event over several days (its first
                    // row, or its end mark's), in the day: where its line starts or ends.
                    function middleOf(id, end) {
                        const count = dayRows.count
                        for (let i = 0; i < count; i++) {
                            const r = day.rows[i]
                            if (r && r.id === id && r.mark !== undefined && (r.mark === "end") === end) {
                                const item = dayRows.itemAt(i)
                                return item ? item.y + item.height / 2 : 0
                            }
                        }
                        return end ? day.height : 0
                    }

                    // Its lines on the list's content: from its row on its first day (else through
                    // the gap above), to its end mark on its last (else the day's bottom).
                    function lay() {
                        day.spans = page.layLines(day.spans, daysList.contentItem, day.through.lines,
                                                  l => day.y + (l.first ? day.middleOf(l.id, false) : -daysList.spacing),
                                                  l => day.y + (l.last ? day.middleOf(l.id, true) : day.height),
                                                  l => page.openSpan(l.id, day.modelData))
                    }

                    width: daysList.width - 12
                    spacing: 2
                    onThroughChanged: day.lay()
                    Component.onCompleted: day.lay()
                    Component.onDestruction: page.layLines(day.spans, null, [], null, null, null)

                    // Its title; beyond three lines of events over several days at once, how many more ("+2").
                    Label {
                        Layout.leftMargin: page.lineRoom
                        text: day.modelData.title + (day.through.more > 0 ? "   " + page.sioul.textArgs("agenda-lines-more", JSON.stringify({ count: day.through.more })) : "")
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        font.pixelSize: 16
                        color: day.modelData.date < page.window.today ? page.theme.muted : page.theme.text
                    }
                    Label {
                        visible: day.rows.length === 0 && day.plan === "" && day.through.lines.length === 0
                        Layout.leftMargin: page.lineRoom
                        text: page.sioul.text("agenda-free")
                        color: page.theme.muted
                    }
                    Repeater {
                        id: dayRows

                        model: day.rows

                        delegate: ItemRow {
                            id: listed

                            required property var modelData

                            Layout.fillWidth: true
                            Layout.leftMargin: page.lineRoom
                            theme: page.theme
                            compact: page.narrow
                            timeWidth: page.narrow ? 76 : 116
                            when: listed.modelData.when
                            title: listed.modelData.title
                            line: listed.modelData.line
                            kind: listed.modelData.kind
                            tone: listed.modelData.tone
                            colour: listed.modelData.colour
                            known: listed.modelData.known
                            cancelled: listed.modelData.cancelled
                            tentative: listed.modelData.tentative
                            ending: listed.modelData.mark === "end"
                            selected: page.isOpen(listed.modelData)
                            onOpen: page.openListed(listed.modelData, day.modelData)
                            onMenu: page.menuListed(listed.modelData, day.modelData)
                        }
                    }
                    // The plan in one line; a tap opens that day.
                    Label {
                        objectName: "comesPlan"
                        visible: day.plan !== ""
                        Layout.fillWidth: true
                        Layout.leftMargin: page.lineRoom + 6
                        text: day.plan
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: page.theme.muted

                        TapHandler {
                            onTapped: page.showDay(day.modelData.date)
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
                spans: page.mode === "week" || page.mode === "day" ? page.shown.spans || [] : []
                bars: page.mode === "week" || page.mode === "day" ? page.shown.bars || [] : []
                barsMore: page.mode === "week" || page.mode === "day" ? page.shown.bars_more || [] : []
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

            // The Month (§4.5): six weeks of seven cells. On a computer each cell its
            // dates first, then its appointments, three lines, then "…"; on a phone its
            // number and two marks at most, the month's dates and appointments listed
            // under the grid from today on. Days off are shaded; no step, meal or dose.
            ColumnLayout {
                id: month

                visible: page.mode === "month"
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 4

                // The month's days from today on that hold something: a phone's list.
                readonly property var listed: page.mode === "month" && page.narrow ? page.shown.days.filter(d => Number(d.date.slice(5, 7)) === page.anchor.getMonth() + 1 && d.date >= page.window.today && page.monthRows(d).length > 0) : []

                DayOfWeekRow {
                    Layout.fillWidth: true
                    locale: page.locale
                }
                // The weeks' cells, and over them the bars of the events of 24 hours or more and
                // of several whole days (`ongoing::Bar`): one bar a week's row, its title once in it.
                Item {
                    Layout.fillWidth: true
                    Layout.fillHeight: !page.narrow
                    Layout.preferredHeight: page.narrow ? 6 * 44 + 5 * 4 : -1
                    implicitHeight: monthGrid.implicitHeight

                    GridLayout {
                        id: monthGrid

                        anchors.fill: parent
                        columns: 7
                        columnSpacing: 4
                        rowSpacing: 4

                        Repeater {
                            id: monthCells

                            model: page.mode === "month" ? page.shown.days : []

                            delegate: Rectangle {
                                id: cell

                                required property var modelData
                                required property int index
                                readonly property bool inMonth: Number(cell.modelData.date.slice(5, 7)) === page.anchor.getMonth() + 1
                                // Today as the window's clock has it: it moves at midnight.
                                readonly property bool today: cell.modelData.date === page.window.today
                                readonly property var lines: (cell.modelData.lines || []).filter(p => page.shownHere(p.item))
                                readonly property var dated: cell.lines.find(p => p.item.kind === "dated") || null
                                // The bars' lanes in the cell's week row, and where they start in it.
                                readonly property int barLanes: page.narrow ? 0 : page.monthBarLanes[Math.floor(cell.index / 7)] || 0
                                readonly property real barsTop: 4 + cellHead.y + cellHead.height + 2

                                objectName: "monthCell"
                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                Layout.preferredWidth: 1
                                Layout.preferredHeight: 1
                                color: cell.today ? page.theme.surface : cell.modelData.off === true ? timeline.tint(page.theme.line, 0.35) : "transparent"
                                radius: page.theme.radius
                                border.color: page.theme.line
                                opacity: cell.inMonth ? 1 : 0.5

                                TapHandler {
                                    // On a phone, the day's place in the list; double, a new event there.
                                    onTapped: if (page.narrow) {
                                        const at = month.listed.findIndex(d => d.date >= cell.modelData.date)
                                        if (at >= 0)
                                            monthList.positionViewAtIndex(at, ListView.Beginning)
                                    }
                                    onDoubleTapped: page.newEvent(cell.modelData.date)
                                }

                                ColumnLayout {
                                    anchors.fill: parent
                                    anchors.margins: 4
                                    spacing: 2

                                    // Its number and a phone's marks; under them, room for the bars over the row
                                    // (a margin, not an object more in each of the 42 cells).
                                    RowLayout {
                                        id: cellHead

                                        Layout.bottomMargin: cell.barLanes * (page.monthBarHeight + 2)
                                        spacing: 3

                                        Label {
                                            objectName: "monthDay"
                                            text: Number(cell.modelData.date.slice(8, 10))
                                            textFormat: Text.PlainText
                                            font.weight: cell.today ? Font.Bold : Font.Normal
                                            color: cell.today ? page.theme.accent : page.theme.text

                                            // Its number: that day in the Day (a computer; a phone's cell takes the tap).
                                            TapHandler {
                                                enabled: !page.narrow
                                                onTapped: page.showDay(cell.modelData.date)
                                            }
                                        }
                                        // A phone's two marks: a diamond for a date asked, a dot for appointments
                                        // (an event over several days among them).
                                        Rectangle {
                                            objectName: "monthDiamond"
                                            visible: page.narrow && cell.dated !== null
                                            Layout.preferredWidth: 6
                                            Layout.preferredHeight: 6
                                            rotation: 45
                                            color: cell.dated && cell.dated.item.tone !== "quiet" ? page.theme.text : "transparent"
                                            border.color: page.theme.text
                                            border.width: 1
                                        }
                                        Rectangle {
                                            objectName: "monthDot"
                                            visible: page.narrow && (cell.lines.some(p => p.item.kind !== "dated") || ((cell.modelData.through || {}).lines || []).length > 0)
                                            Layout.preferredWidth: 6
                                            Layout.preferredHeight: 6
                                            radius: 3
                                            color: page.theme.accent
                                        }
                                    }
                                    Repeater {
                                        model: page.narrow ? [] : cell.lines.slice(0, Math.max(1, 3 - cell.barLanes))

                                        delegate: RowLayout {
                                            id: monthLine

                                            required property var modelData

                                            objectName: "monthLine"
                                            Layout.fillWidth: true
                                            spacing: 3

                                            // A date's diamond, filled within seven days; an appointment's colour dot.
                                            Rectangle {
                                                readonly property bool diamond: monthLine.modelData.item.kind === "dated"

                                                Layout.preferredWidth: diamond ? 6 : 7
                                                Layout.preferredHeight: diamond ? 6 : 7
                                                Layout.alignment: Qt.AlignVCenter
                                                rotation: diamond ? 45 : 0
                                                radius: diamond ? 0 : 3.5
                                                color: diamond ? (monthLine.modelData.item.tone !== "quiet" ? page.theme.text : "transparent") : monthLine.modelData.item.colour || page.theme.accent
                                                border.width: diamond ? 1 : 0
                                                border.color: page.theme.text
                                            }
                                            Label {
                                                Layout.fillWidth: true
                                                text: monthLine.modelData.item.title
                                                textFormat: Text.PlainText
                                                elide: Text.ElideRight
                                                font.pixelSize: 12
                                                font.strikeout: (monthLine.modelData.detail || {}).cancelled === true
                                                color: page.theme.text
                                            }
                                            TapHandler {
                                                onTapped: page.openRow(timeline.rowOf(monthLine.modelData, cell.modelData, 0, ""))
                                            }
                                        }
                                    }
                                    Label {
                                        visible: !page.narrow && (cell.lines.length > Math.max(1, 3 - cell.barLanes) || (page.shown.bars_more || [])[cell.index] > 0)
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
                    Repeater {
                        model: page.mode === "month" && !page.narrow ? page.monthBars : []

                        delegate: Rectangle {
                            id: monthBar

                            required property var modelData
                            readonly property Item first: monthCells.count > 0 ? monthCells.itemAt(monthBar.modelData.row * 7 + monthBar.modelData.from_column) : null
                            readonly property Item last: monthCells.count > 0 ? monthCells.itemAt(monthBar.modelData.row * 7 + monthBar.modelData.to_column) : null
                            readonly property color hue: monthBar.modelData.colour ? monthBar.modelData.colour : (page.spanEvents[monthBar.modelData.id] || { item: {} }).item.kind === "day-off" ? page.theme.muted : page.theme.accent

                            objectName: "monthBar"
                            visible: monthBar.first !== null && monthBar.last !== null
                            x: monthBar.first ? monthBar.first.x + 3 : 0
                            y: monthBar.first ? monthBar.first.y + monthBar.first.barsTop + monthBar.modelData.lane * (page.monthBarHeight + 2) : 0
                            width: monthBar.last && monthBar.first ? monthBar.last.x + monthBar.last.width - monthBar.first.x - 6 : 0
                            height: page.monthBarHeight
                            topLeftRadius: monthBar.modelData.cut_before ? 0 : 3
                            bottomLeftRadius: monthBar.modelData.cut_before ? 0 : 3
                            topRightRadius: monthBar.modelData.cut_after ? 0 : 3
                            bottomRightRadius: monthBar.modelData.cut_after ? 0 : 3
                            color: timeline.tint(monthBar.hue, 0.22)
                            border.width: 1
                            border.color: timeline.tint(monthBar.hue, 0.55)
                            Accessible.role: Accessible.Button
                            Accessible.name: monthBar.modelData.line

                            Label {
                                anchors.fill: parent
                                anchors.leftMargin: 4
                                anchors.rightMargin: 4
                                verticalAlignment: Text.AlignVCenter
                                text: monthBar.modelData.title
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: 11
                                color: page.theme.text
                            }
                            TapHandler {
                                onTapped: page.openSpan(monthBar.modelData.id, page.shown.days[monthBar.modelData.row * 7 + monthBar.modelData.from_column] || {})
                            }
                        }
                    }
                }
                // A phone: the month's dates and appointments, day by day, from today on.
                Label {
                    visible: page.narrow
                    Layout.topMargin: 6
                    text: page.sioul.text("agenda-month-list")
                    textFormat: Text.PlainText
                    font.weight: Font.DemiBold
                    color: page.theme.text
                }
                ListView {
                    id: monthList

                    visible: page.narrow
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 8
                    model: month.listed

                    delegate: ColumnLayout {
                        id: listedDay

                        required property var modelData
                        readonly property var rows: page.monthRows(listedDay.modelData)
                        readonly property var through: listedDay.modelData.through || ({ lines: [], more: 0, rows: [] })
                        // Its events over several days' lines, made only while it has some (`layLines`).
                        property var spans: []

                        // The middle of the day's row of an event over several days: where its line starts or ends.
                        function middleOf(id, end) {
                            for (let i = 0; i < monthRowsShown.count; i++) {
                                const r = listedDay.rows[i]
                                if (r && r.id === id && r.mark !== undefined && (r.mark === "end") === end) {
                                    const item = monthRowsShown.itemAt(i)
                                    return item ? item.y + item.height / 2 : 0
                                }
                            }
                            return end ? listedDay.height : 0
                        }

                        function lay() {
                            listedDay.spans = page.layLines(listedDay.spans, monthList.contentItem, listedDay.through.lines,
                                                            l => listedDay.y + (l.first ? listedDay.middleOf(l.id, false) : -monthList.spacing),
                                                            l => listedDay.y + (l.last ? listedDay.middleOf(l.id, true) : listedDay.height),
                                                            l => page.openSpan(l.id, listedDay.modelData))
                        }

                        width: monthList.width
                        spacing: 1
                        onThroughChanged: listedDay.lay()
                        Component.onCompleted: listedDay.lay()
                        Component.onDestruction: page.layLines(listedDay.spans, null, [], null, null, null)

                        Label {
                            Layout.leftMargin: page.monthLineRoom
                            text: listedDay.modelData.title + (listedDay.through.more > 0 ? "   " + page.sioul.textArgs("agenda-lines-more", JSON.stringify({ count: listedDay.through.more })) : "")
                            textFormat: Text.PlainText
                            font.weight: Font.DemiBold
                            color: page.theme.text

                            TapHandler {
                                onTapped: page.showDay(listedDay.modelData.date)
                            }
                        }
                        Repeater {
                            id: monthRowsShown

                            model: listedDay.rows

                            delegate: ItemRow {
                                id: monthItem

                                required property var modelData

                                Layout.fillWidth: true
                                Layout.leftMargin: page.monthLineRoom
                                theme: page.theme
                                compact: true
                                timeWidth: 76
                                when: monthItem.modelData.when
                                title: monthItem.modelData.title
                                // An event over several days: its span, "Ongoing since …", "until 10:15".
                                line: monthItem.modelData.mark !== undefined ? monthItem.modelData.line : ""
                                kind: monthItem.modelData.kind
                                tone: monthItem.modelData.tone
                                colour: monthItem.modelData.colour
                                cancelled: monthItem.modelData.cancelled
                                tentative: monthItem.modelData.tentative
                                ending: monthItem.modelData.mark === "end"
                                selected: page.isOpen(monthItem.modelData)
                                onOpen: page.openListed(monthItem.modelData, listedDay.modelData)
                                onMenu: page.menuListed(monthItem.modelData, listedDay.modelData)
                            }
                        }
                    }
                }
            }
        }
    }

    // Show: what every view draws on this device (the Day, the Week, What comes and the Month);
    // which agendas they draw is the filter's (AgendaFilter.qml).
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
                // Each ride or walk as an event of its own, rather than a quiet line in its
                // appointment's lead (part E, D25): made in Rust, so the days are read again.
                MenuItem {
                    objectName: "agendaShowLegs"
                    text: page.sioul.text("agenda-show-legs")
                    checkable: true
                    checked: page.view.legs === true
                    onTriggered: {
                        page.showPart("legs", checked)
                        page.readAgain()
                    }
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
