// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The agenda's Day and Week (the unified agenda, part 2): one timeline that
// draws everything that has a time, from the one model (`timeline::Item`,
// placed and dressed in Rust: `timeline::days`, `timeline::dress`). The
// hours shown are Health's rule (from half an hour before waking to half an
// hour after bedtime), the same for every day, so that the hour lines stay
// in place; the night shades both ends. On each day: its hours by what they
// are for, faint behind; its doses on a narrow lane of their own; and one
// lane of cards, side by side where things overlap: appointments solid with
// their calendar's colour, a pinned task with its pin, the plan's steps
// outlined (the next one filled, the one under way marked), tasks done
// dimmed, meals and naps tinted (named in the Day, bands without words in
// the Week), what is kept around each thing (getting there and back,
// getting ready, coming back, winding down) lighter, time for you and the
// time kept free as quiet words; in quiet time, a coming day's steps hidden
// now as one band per stretch, "Show anyway" drawing them. A dose's mark
// says what is known of it by the window's clock (the Health job's rows:
// `doseMarks`), never "not taken"; a day to come's are ticks, a day gone by
// has none. What falls inside the night shows at the edge with its time.
//
// Each day's things live in one model keyed by their id, changed in place
// at each publish (`apply`): only what changed is made again, and nothing
// scrolls under you. A tap opens the thing (the page says where, by its
// kind); a right click or a long press asks for its menu; a drag moves it
// when its item says it may (`can.drag`), by its own writer (Paths.qml):
// an appointment or a block by its file, a step pinned there, a meal, a nap
// or the night that day. With a mouse, press and drag; with a finger, a long
// press picks it up (a plain swipe scrolls). One set of handlers for the
// whole timeline (TimeDrag.qml): the cards carry none.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: planning

    required property var sioul
    required property var theme
    required property var locale
    // The days shown (the agenda's JSON, version 3), and the hours they show.
    property var days: []
    property var frame: ({ from: 7 * 60, to: 22 * 60 })
    // Today's doses as the Health job last read them (`doseMarks`, parsed),
    // when it read them (`doseReadAt`), and when Sioul came back on the screen.
    property var marks: null
    property real readAt: 0
    property real backAt: 0
    // "Show anyway": quiet time's bands drawn as the steps they hold.
    property bool showAnyway: false
    // What Show leaves out on this device: {steps, needs, doses, calendars: [places, "account/id"]}.
    property var hide: ({})
    // The thing open beside the timeline, by its item's id: shown chosen (on each day it covers).
    property string chosenId: ""
    // The thing the keyboard is on (Up and Down), by its row's key.
    property string focusKey: ""
    // The Day: one day, its things named; the Week: seven, meals and naps as bands without words.
    readonly property bool single: daysModel.count === 1
    // The Week as a list of days, in time order: on a phone, or wherever seven
    // columns of about 110 points do not fit (§4.9). Only the shown way is made.
    // Set by `apply` (and when the width changes), never in the middle of one.
    property bool listed: false
    // The day's whole days drawn above the hours (a phone's Day says them on its head line instead).
    property bool chipsHere: true
    // A phone's width: the hours' column narrower, so the days get the room.
    readonly property bool narrow: planning.width < 600
    readonly property int gutter: planning.narrow ? 40 : 52
    // The doses' lane, left of each day's cards.
    readonly property int lane: planning.narrow ? 16 : 18
    readonly property int hoursShown: Math.max(1, Math.round((planning.frame.to - planning.frame.from) / 60))
    readonly property real dayWidth: Math.max(40, (planning.width - planning.gutter - 12) / Math.max(1, daysModel.count))
    readonly property real slotWidth: planning.dayWidth - planning.lane - 4

    // Now on the clock (Unix seconds) and today ("2026-10-06"): the window's clock.
    property real now: Date.now() / 1000
    property string today: {
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }
    // Each publish applied (`apply`): what reads the models' rows follows it.
    property int revision: 0
    // Today's column among the days shown; -1 when today is not shown.
    readonly property int todayIndex: {
        planning.revision
        for (let i = 0; i < daysModel.count; i++)
            if (daysModel.get(i).date === planning.today)
                return i
        return -1
    }
    // Now in minutes from today's midnight, for its line.
    readonly property int minuteNow: {
        const d = new Date(planning.now * 1000)
        return d.getHours() * 60 + d.getMinutes()
    }
    // Today's step under way and the next one (`follow`), by their rows' keys.
    property string underKey: ""
    property string nextKey: ""

    signal open(var row)
    signal menu(var row)
    // Ctrl+E on the thing the keyboard is on: its own form.
    signal edit(var row)
    signal newAt(string day, int hour)
    signal openDay(string date)
    // Page Up and Page Down: the day or the week before, the next.
    signal turn(int steps)

    // The cards' type, measured: the lines a title takes at its card's width.
    readonly property real titleLine: Math.ceil(Math.max(titleFont.height, titleFont.lineSpacing))
    readonly property real timeLine: Math.ceil(Math.max(timeFont.height, timeFont.lineSpacing))
    readonly property int cardPadding: 4
    readonly property real cardLeast: planning.cardPadding + planning.titleLine
    // An hour at most this tall: past it, the shortest cards end their title with "…".
    readonly property real zoomMost: 160
    // A phone's Day: an hour of 56 points at least (§4.9).
    readonly property real hourLeast: planning.narrow && planning.single ? 56 : 30

    // The lines `text` takes at `width`, as a card's title wraps it.
    function linesOf(text, width) {
        if (width <= 0)
            return 1
        const space = titleFont.advanceWidth(" ")
        let lines = 1
        let used = 0
        for (const word of String(text).split(/\s+/)) {
            if (word === "")
                continue
            let w = titleFont.advanceWidth(word)
            if (used > 0 && used + space + w <= width) {
                used += space + w
                continue
            }
            if (used > 0)
                lines += 1
            while (w > width) {
                lines += 1
                w -= width
            }
            used = w
        }
        return lines
    }

    // How tall an hour must be for every appointment and pinned task to show its
    // whole title (§4.4: those alone, so that a crowded week does not grow without end).
    readonly property real legibleHour: {
        planning.revision
        let most = 0
        for (let i = 0; i < things.count; i++) {
            const row = things.get(i)
            if (row.edge !== "" || !(row.kind === "event" || row.kind === "block" || (row.kind === "step" && row.certainty === "held")))
                continue
            const lines = planning.linesOf(row.title, planning.slotWidth / Math.max(1, row.columns) - 12)
            if (lines < 2)
                continue
            const length = Math.max(5, row.to - row.from) / 60
            most = Math.max(most, (planning.cardPadding + lines * planning.titleLine + 1) / length)
        }
        return most
    }
    readonly property real hourHeight: Math.max(planning.hourLeast, hours.height / planning.hoursShown, Math.min(planning.zoomMost, planning.legibleHour))

    // Where a minute of a day sits in the hours' content, and the minute at `y`.
    function yOf(minute) {
        return (minute - planning.frame.from) / 60 * planning.hourHeight
    }

    function minuteAt(y) {
        return planning.frame.from + y / planning.hourHeight * 60
    }

    function columnAt(x) {
        return Math.floor((x - planning.gutter) / planning.dayWidth)
    }

    // ---- The rows, from the days: one flat model, keyed, changed in place.

    ListModel {
        id: things
    }

    ListModel {
        id: daysModel
    }

    ListModel {
        id: marksModel
    }

    ListModel {
        id: listRows
    }

    // A placed item of `day` (its index `index`) as a flat row: what a card draws and a drag moves.
    function rowOf(placed, day, index, fold) {
        const item = placed.item
        const src = item.source || {}
        const time = item.time || {}
        const can = item.can || {}
        const detail = placed.detail || {}
        const span = time.type === "span"
        const at = span ? time.at : (time.at || day.start)
        const minute = t => Math.round((t - day.start) / 60)
        return {
            key: item.id + "|" + day.date + (fold ? "|" + fold : ""),
            itemId: item.id,
            day: index,
            date: day.date,
            kind: item.kind,
            fold: fold || "",
            title: item.title || "",
            line: item.line || "",
            when: placed.when || "",
            from: placed.from_minute,
            to: placed.to_minute,
            lead: span ? Math.max(-1440, minute(time.start)) : placed.from_minute,
            tail: span ? Math.min(2880, minute(time.end)) : placed.to_minute,
            column: placed.column || 0,
            columns: placed.columns || 1,
            edge: placed.edge || "",
            certainty: typeof item.certainty === "string" ? item.certainty : (item.certainty === undefined ? "fixed" : "dose"),
            tone: item.tone || "plain",
            colour: item.colour || "",
            // What a drag does (`can.drag`); not named `drag`, which the cards read as the timeline's TimeDrag.
            mover: can.drag || "none",
            ends: can.ends === true,
            edit: can.edit || "none",
            why: can.why || "",
            owner: item.owner || "",
            srcKey: src.key || "",
            srcStart: src.start || 0,
            recurring: src.recurring === true,
            srcDate: src.date || "",
            calendar: src.calendar || "",
            collection: src.collection || "",
            // A date asked: its kind ("letter", "notice"…), which its card reads.
            datedKind: src.type === "dated" ? src.kind || "" : "",
            cancelled: detail.cancelled === true,
            tentative: detail.tentative === true,
            readOnly: detail.read_only === true,
            withOthers: detail.with_others === true,
            location: detail.location || "",
            list: detail.list || "",
            start: span ? time.start : at,
            at: at,
            until: span ? time.until : at,
            end: span ? time.end : at,
            json: JSON.stringify(placed)
        }
    }

    // A date asked's words beside its diamond: its title, its hour when it has one, its kind and the time left.
    function datedWords(row) {
        const at = row.when !== "" && row.when !== planning.sioul.text("agenda-all-day") ? " · " + row.when : ""
        return row.title + at + (row.line ? " · " + row.line : "")
    }

    // What Show leaves out on this device.
    function shown(row) {
        const hide = planning.hide || {}
        if (hide.steps && (row.kind === "step" || row.kind === "done" || row.kind === "gain" || row.kind === "slack"))
            return false
        if (hide.needs && (row.kind === "meal" || row.kind === "nap" || row.kind === "night"))
            return false
        // A calendar by its place, never its name: two of one name stay apart.
        if ((row.kind === "event" || row.kind === "block" || row.kind === "leg") && (hide.calendars || []).indexOf(row.collection) >= 0)
            return false
        return true
    }

    // `model` changed to hold `rows`, in order, each by its key: what stayed
    // is changed in place (only the roles that differ), what went is taken
    // out, what came is put in. Nothing is made again for nothing.
    function diff(model, given) {
        // One row a key: a second copy, in the rows given or in the model, is dropped, never left as a ghost.
        const wanted = {}
        const rows = []
        for (const row of given) {
            if (wanted[row.key])
                continue
            wanted[row.key] = true
            rows.push(row)
        }
        const kept = {}
        for (let i = 0; i < model.count; i++) {
            const key = model.get(i).key
            if (!wanted[key] || kept[key])
                model.remove(i--)
            else
                kept[key] = true
        }
        for (let i = 0; i < rows.length; i++) {
            const row = rows[i]
            if (i < model.count && model.get(i).key === row.key) {
                planning.update(model, i, row)
                continue
            }
            let found = -1
            for (let j = i + 1; j < model.count; j++) {
                if (model.get(j).key === row.key) {
                    found = j
                    break
                }
            }
            if (found >= 0) {
                model.move(found, i, 1)
                planning.update(model, i, row)
            } else {
                model.insert(i, row)
            }
        }
    }

    function update(model, i, row) {
        const now = model.get(i)
        for (const role in row)
            if (now[role] !== row[role])
                model.setProperty(i, role, row[role])
    }

    // The days read again (a publish, Show, Show anyway): every model changed in place.
    function apply() {
        const days = planning.days || []
        planning.listed = planning.listsDays(days.length)
        // The hours hidden: shown again, they open on their start.
        if (planning.listed)
            planning.scrolledFor = ""
        const dayRows = []
        const rows = []
        const doses = []
        for (let i = 0; i < days.length; i++) {
            const day = days[i]
            // The night's parts: the shading at both ends.
            for (const placed of day.edges || []) {
                const row = planning.rowOf(placed, day, i, "")
                if (planning.shown(row))
                    rows.push(row)
            }
            dayRows.push({
                key: day.date,
                date: day.date,
                title: day.title || "",
                today: day.today === true,
                start: day.start,
                hours: JSON.stringify(day.hours || []),
                chips: JSON.stringify((day.head || []).filter(p => planning.shown(planning.rowOf(p, day, i, "")))),
                plan: day.plan || "",
                more: day.more || "",
                said: (day.said || []).join(" ")
            })
            for (const placed of day.timed || []) {
                const row = planning.rowOf(placed, day, i, "")
                if (planning.shown(row))
                    rows.push(row)
            }
            // Quiet time: a coming day's hidden steps as bands, or, shown anyway, as themselves.
            for (const placed of (planning.showAnyway ? day.hidden : day.bands) || []) {
                const row = planning.rowOf(placed, day, i, planning.showAnyway ? "hidden" : "band")
                if (planning.shown(row))
                    rows.push(row)
            }
            if (!(planning.hide || {}).doses)
                for (const placed of day.doses || [])
                    doses.push(planning.rowOf(placed, day, i, ""))
        }
        const marks = planning.marksOf(doses)
        planning.diff(daysModel, dayRows)
        planning.diff(things, planning.listed ? [] : rows)
        planning.diff(listRows, planning.listed ? planning.listOf(days, rows, marks) : [])
        planning.diff(marksModel, marks)
        planning.revision++
        planning.follow()
        // Other days shown: opened on their first hour (today's, on now), the list on today; the same days, never moved.
        Qt.callLater(planning.scrollToStart, false)
        Qt.callLater(planning.listToToday)
    }

    Component.onCompleted: planning.apply()
    onWidthChanged: Qt.callLater(planning.relist)

    // Whether `count` days are a list at this width (`listed`).
    function listsDays(count) {
        return count > 1 && planning.width > 0 && planning.width < planning.gutter + count * 110
    }

    // The width changed: the days drawn the other way, when they must.
    function relist() {
        if (planning.listsDays(daysModel.count) !== planning.listed)
            planning.apply()
    }

    // The Week as a list (`listed`): each day's title (and its whole days),
    // its things in time order, today's doses on one line, the lists' line,
    // what today's plan could not place; "Nothing planned." for a free day.
    function listOf(days, rows, marks) {
        const out = []
        for (let i = 0; i < days.length; i++) {
            const day = days[i]
            const line = (kind, title, more) => Object.assign(planning.rowOf({ item: { id: kind, kind: kind, title: title, time: { type: "at", at: day.start }, can: {}, source: {} }, when: "", from_minute: 0, to_minute: 0 }, day, i, ""), { key: kind + "|" + day.date }, more || {})
            const heads = (day.head || []).map(p => planning.rowOf(p, day, i, "")).filter(planning.shown)
            // Whole days under the day's title; a date asked on a line of its own, its diamond and its words (review of part 4, later 2).
            out.push(line("day-title", day.title || day.date, { line: heads.filter(r => r.kind !== "dated").map(r => r.title).join(" · ") }))
            for (const r of heads.filter(r => r.kind === "dated"))
                out.push(Object.assign({}, r, { key: r.key + "|list", line: planning.datedWords(r) }))
            if (day.today && marks.some(m => m.date === day.date))
                out.push(line("doses-line", ""))
            // The night by its part: waking, at its time; winding down, from its start.
            const said = r => r.kind !== "night" ? r : planning.morning(r) ? Object.assign({}, r, { title: planning.sioul.text("agenda-wake"), when: planning.clockOf(r.to) }) : Object.assign({}, r, { when: planning.clockOf(r.lead) })
            // A leg is said in its appointment's line here, never a row of its own (the unified agenda, §4.7).
            const own = rows.filter(r => r.day === i && r.kind !== "leg").sort((a, b) => a.from - b.from || a.column - b.column).map(said)
            out.push(...own)
            if (day.plan)
                out.push(line("plan-line", day.plan))
            if (day.more)
                out.push(line("more-line", day.more))
            if (own.length === 0 && !day.more)
                out.push(line("free", planning.sioul.text("agenda-free")))
        }
        return out
    }

    onDaysChanged: {
        // What was let go is drawn until the days come back with it; never
        // forgotten while "only this time or every time" waits for its answer.
        if (!(which.item && which.item.opened))
            planning.landed = null
        planning.apply()
    }
    onHideChanged: planning.apply()
    onShowAnywayChanged: planning.apply()
    onMarksChanged: planning.diff(marksModel, planning.marksOf(planning.doseRows()))
    onNowChanged: planning.follow()

    // ---- Doses: one mark a minute, in the state that asks the most care.

    // The dose rows last laid (`apply`), for the marks made again when the Health job reads.
    function doseRows() {
        const rows = []
        const days = planning.days || []
        if ((planning.hide || {}).doses)
            return rows
        for (let i = 0; i < days.length; i++)
            for (const placed of days[i].doses || [])
                rows.push(planning.rowOf(placed, days[i], i, ""))
        return rows
    }

    // A reading's row of a dose, by its key (`doseMarks`).
    function markOf(key) {
        const rows = planning.marks && planning.marks.rows ? planning.marks.rows : []
        return rows.find(r => r.key === key) || null
    }

    // What is known of a dose at `now` (seconds), as `health::dose_state` says
    // it: a reading of another day is no reading; taken stays taken; none since
    // Sioul came back: being read; a reading older than its hold turns an open
    // dose into "check"; else its state given ahead for now. Never "not taken".
    function doseState(key, day, now) {
        const marks = planning.marks
        const reading = { state: "reading", words: marks ? marks.reading || "" : "" }
        if (day !== planning.today)
            return { state: "ahead", words: "" }
        const row = planning.markOf(key)
        if (!marks || !row || row.day !== planning.today)
            return reading
        let given = null
        for (const g of row.states || [])
            if (g.from <= now)
                given = g
        if (!given)
            given = (row.states || [])[0]
        if (!given)
            return reading
        const state = given.state.state
        if (state === "taken")
            return { state: "taken", words: given.words }
        // Back at or after the reading's start (both to the millisecond): it may be older than the return.
        if (planning.backAt >= planning.readAt)
            return reading
        if (state === "open" && now - planning.readAt > (marks.holds || 120))
            return { state: "check", words: marks.old || "" }
        if (state === "open")
            return { state: given.state.come ? "come" : "not-come", words: given.words }
        return { state: state === "differ" || state === "asked" ? "check" : state, words: given.words }
    }

    // The order of care: what a mark of several doses shows (§8.2).
    readonly property var care: ["reading", "check", "come", "not-come", "taken", "ahead"]

    // The doses as marks: one per day and minute, each with the keys it holds;
    // today's taken from the reading too, those it holds and the schedule no
    // longer has (taken at another time) included.
    function marksOf(rows) {
        const byMinute = {}
        const add = (day, date, minute, key, edge) => {
            const mark = date + "|" + minute
            if (!byMinute[mark])
                byMinute[mark] = { key: mark, day: day, date: date, minute: minute, keys: [], edge: edge }
            if (byMinute[mark].keys.indexOf(key) < 0)
                byMinute[mark].keys.push(key)
        }
        // A day gone by has no mark (none could say what it was); in the seconds after midnight,
        // before the next publish, yesterday's doses are still in the days.
        for (const row of rows)
            if (row.date >= planning.today)
                add(row.day, row.date, row.from, row.srcKey, row.edge)
        const today = planning.todayIndex
        if (today >= 0 && planning.marks && !(planning.hide || {}).doses)
            for (const row of planning.marks.rows || [])
                if (row.day === planning.today)
                    add(today, planning.today, row.minute, row.key, row.minute < planning.frame.from ? "top" : row.minute >= planning.frame.to ? "bottom" : "")
        return Object.values(byMinute).sort((a, b) => a.date.localeCompare(b.date) || a.minute - b.minute).map(m => Object.assign({}, m, { keys: JSON.stringify(m.keys) }))
    }

    // A mark's state: of its doses, the one asking the most care.
    function markState(keysJson, date) {
        let best = null
        for (const key of JSON.parse(keysJson)) {
            const state = planning.doseState(key, date, planning.now)
            if (best === null || planning.care.indexOf(state.state) < planning.care.indexOf(best.state))
                best = state
        }
        return best || { state: "reading", words: "" }
    }

    // ---- Now: the step under way and the next one, today.

    function follow() {
        let under = ""
        let next = ""
        let nextAt = Infinity
        for (let i = 0; i < things.count; i++) {
            const row = things.get(i)
            if (row.date !== planning.today || row.kind !== "step" || row.fold === "band")
                continue
            if (row.at <= planning.now && planning.now < row.until)
                under = row.key
            else if (row.at > planning.now && row.at < nextAt) {
                next = row.key
                nextAt = row.at
            }
        }
        planning.underKey = under
        planning.nextKey = next
    }

    // ---- Where things are drawn, and what lies under the hand.

    // A row's card: {x, y, width, height} in the hours' content. The night's
    // parts take the day's width; the rest its slot among what overlaps it; a
    // thing inside the night sits at the edge, a line high.
    function boxOf(row) {
        const left = planning.gutter + row.day * planning.dayWidth
        if (row.kind === "night") {
            const top = planning.morning(row) ? planning.frame.from : Math.max(planning.frame.from, row.lead)
            const bottom = planning.morning(row) ? Math.min(planning.frame.to, row.to) : planning.frame.to
            return { x: left + 1, y: planning.yOf(top), width: planning.dayWidth - 2, height: Math.max(0, planning.yOf(bottom) - planning.yOf(top)) }
        }
        const slot = planning.slotWidth / Math.max(1, row.columns)
        const x = left + planning.lane + 2 + row.column * slot
        if (row.edge !== "") {
            const h = planning.cardLeast
            return { x: x, y: row.edge === "top" ? 0 : planning.yOf(planning.frame.to) - h, width: slot - 2, height: h }
        }
        // A step, an event: what is kept around it, lighter, above and below.
        const top = Math.max(planning.frame.from, Math.min(row.from, row.lead))
        const bottom = Math.min(planning.frame.to, Math.max(row.to, row.tail))
        return { x: x, y: planning.yOf(top), width: slot - 2, height: Math.max(planning.cardLeast, planning.yOf(bottom) - planning.yOf(top) - 1) }
    }

    // The night's part that wakes this day (gone to bed before noon: last
    // evening, or after midnight), not tonight's: `timeline::frame_of`'s rule.
    function morning(row) {
        return row.kind === "night" && row.from < 12 * 60
    }

    // Its end is on this day (not cut at midnight): its bottom edge is its end.
    function endsHere(row) {
        return row.tail <= 1440
    }

    // What the hand may move: what its item says, never over, never a thing at the edge.
    function movable(row) {
        return row.mover !== "none" && row.edge === "" && row.fold !== "band"
    }

    // What lies at `p` (the hours' content): {row, index, part: "body", "top"
    // (the night's bedtime) or "bottom" (an end: an event's, a meal's, the
    // night's waking)}; cards over the night's shading; else null.
    function thingAt(p) {
        let found = null
        let behind = null
        let leg = null
        for (let i = 0; i < things.count; i++) {
            const row = things.get(i)
            const box = planning.boxOf(row)
            if (p.x < box.x || p.x > box.x + box.width || p.y < box.y - 4 || p.y > box.y + box.height + 4)
                continue
            // A leg is a line in its appointment's lead or tail: in the Week, a press there is the
            // appointment's, never the leg's, and its body's, never its end's handle; in the Day,
            // the leg's, over the box it lies in (reviews of ab7daa4, later 8, and 4d7bab4, later 14).
            if (row.kind === "leg") {
                if (p.y >= box.y && p.y <= box.y + box.height && leg === null)
                    leg = { row: row, index: i, part: "body" }
                continue
            }
            const inside = Math.max(4, Math.min(8, box.height / 3))
            let part = "body"
            if (row.kind === "night") {
                if (!planning.morning(row) && row.ends && p.y <= box.y + inside)
                    part = "top"
                else if (planning.morning(row) && row.ends && p.y >= box.y + box.height - inside)
                    part = "bottom"
                if (p.y < box.y || p.y > box.y + box.height)
                    continue
                behind = { row: row, index: i, part: part }
                continue
            }
            if (row.ends && planning.endsHere(row) && p.y >= box.y + box.height - inside)
                part = "bottom"
            if (p.y < box.y || p.y > box.y + box.height + (part === "bottom" ? 4 : 0))
                continue
            found = { row: row, index: i, part: part }
        }
        if (leg !== null && planning.single)
            return leg
        if (leg !== null && found !== null && found.part === "bottom")
            found = { row: found.row, index: found.index, part: "body" }
        return found || leg || behind
    }

    // ---- The drag: a thing taken by the hand, where it would go, let go.

    // {row, part, pressMinute, column}; where the pointer is now.
    property var grab: null
    property point pointer: Qt.point(0, 0)
    // Let go, or waiting for "only this time / every time": drawn where it goes until the days come back.
    property var landed: null

    // Midnight of the day shown at column `i`, Unix seconds.
    function dayStart(i) {
        return i >= 0 && i < daysModel.count ? daysModel.get(i).start : 0
    }

    // Where the thing taken would go, by whole five minutes: {start, end, column}
    // of the thing itself, Unix seconds (a meal, a nap or the night: with what is
    // kept around it, as its writer moves it; a step and an event: proper).
    function target(grab, p) {
        const row = grab.row
        const shift = Math.round((planning.minuteAt(p.y) - grab.pressMinute) / 5) * 5 * 60
        const snap = s => Math.round(s / 300) * 300
        const whole = row.kind === "meal" || row.kind === "nap" || row.kind === "night"
        const from = whole ? row.start : row.at
        const to = whole ? row.end : row.until
        if (grab.part === "bottom")
            return { start: from, end: Math.max(from + 300, snap(to + shift)), column: row.day }
        if (grab.part === "top")
            return { start: snap(from + shift), end: to, column: row.day }
        // A meal, a nap and the night stay on their day; an event or a step goes to another day of the week.
        const column = !whole && daysModel.count > 1 ? Math.max(0, Math.min(daysModel.count - 1, planning.columnAt(p.x))) : row.day
        const moved = planning.dayStart(column) - planning.dayStart(row.day)
        let start = snap(from + moved + shift)
        // A step is given a time from now on (its writer's rule, `blocks::pin`).
        if (row.mover === "pin")
            start = Math.max(start, Math.ceil(planning.now / 300) * 300)
        return { start: start, end: start + (to - from), column: column }
    }

    function clock(seconds) {
        return Qt.formatTime(new Date(seconds * 1000), "HH:mm")
    }

    // "09:00 – 10:30"; on another day, its day first: "Tue 6, 09:00 – 10:30".
    function said(grab, to) {
        const times = planning.clock(to.start) + " – " + planning.clock(to.end)
        if (to.column === grab.row.day)
            return times
        return new Date(to.start * 1000).toLocaleDateString(planning.locale, "ddd d") + ", " + times
    }

    // A row of the model as a plain object: what stays after a publish changes the model
    // (a drag under way, a question on a repeating event waiting for its answer).
    function copyOf(row) {
        const out = {}
        for (const role in planning.rowOf({ item: { id: "", kind: "", time: {}, can: {}, source: {} } }, { date: "", start: 0 }, 0, ""))
            out[role] = row[role]
        return out
    }

    function take(thing, p) {
        planning.grab = { row: planning.copyOf(thing.row), part: thing.part, pressMinute: planning.minuteAt(p.y) }
        planning.pointer = p
        planning.landed = null
    }

    function drop(p) {
        const grab = planning.grab
        planning.grab = null
        if (!grab)
            return
        const to = planning.target(grab, p)
        const row = grab.row
        const whole = row.kind === "meal" || row.kind === "nap" || row.kind === "night"
        // Let go where it was: nothing changes, nothing to undo.
        if (to.start === (whole ? row.start : row.at) && to.end === (whole ? row.end : row.until))
            return
        planning.landed = { row: row, part: grab.part, column: to.column, start: to.start, end: to.end, text: planning.said(grab, to) }
        // A repeating event, moved or its end dragged: this time alone, or every time.
        if (row.mover === "event" && row.recurring) {
            which.now().ask(row.title, planning.sioul.textWith("agenda-move-which", "time", planning.landed.text))
            return
        }
        planning.save(false)
    }

    // The thing let go written, by its own writer (Paths.qml): that time alone, or every time.
    function save(onlyThis) {
        const landed = planning.landed
        if (!landed)
            return
        landedLate.restart()
        const problem = paths.drop(landed.row, landed.part, { start: landed.start, end: landed.end }, onlyThis)
        if (problem !== "") {
            planning.landed = null
            planning.sioul.status = problem
        }
    }

    Paths {
        id: paths

        sioul: planning.sioul
    }

    // What was let go stays drawn where it went until the days come back with
    // it, while its re-plan is due (the days wait for it: `sioul.replanDue`),
    // then `landedFor` more at most (review of 590eb18, later 2).
    property int landedFor: 4000

    Timer {
        id: landedLate

        interval: planning.landedFor
        onTriggered: {
            if (planning.sioul.replanDue === true)
                landedLate.restart()
            else
                planning.landed = null
        }
    }

    // ---- The keyboard: from thing to thing in time order (Up and Down).

    function ordered() {
        const out = []
        for (let i = 0; i < things.count; i++) {
            const row = things.get(i)
            if (row.kind !== "night" && row.fold !== "band")
                out.push(row)
        }
        out.sort((a, b) => a.day - b.day || a.from - b.from || a.column - b.column)
        return out
    }

    // The next thing (`delta` 1) or the one before (-1), from the one the keyboard is on.
    function stepFocus(delta) {
        const all = planning.ordered()
        if (all.length === 0)
            return null
        const at = all.findIndex(r => r.key === planning.focusKey)
        const next = at < 0 ? (delta > 0 ? 0 : all.length - 1) : Math.max(0, Math.min(all.length - 1, at + delta))
        planning.focusKey = all[next].key
        planning.reveal(all[next])
        return all[next]
    }

    function focused() {
        for (let i = 0; i < things.count; i++)
            if (things.get(i).key === planning.focusKey)
                return things.get(i)
        return null
    }

    // A thing brought into view, when the keyboard goes to it.
    function reveal(row) {
        const box = planning.boxOf(row)
        if (box.y < hours.contentY)
            hours.contentY = Math.max(0, box.y - 8)
        else if (box.y + box.height > hours.contentY + hours.height)
            hours.contentY = Math.min(hours.contentHeight - hours.height, box.y + box.height - hours.height + 8)
    }

    // ---- Opened on now: today an hour before now, another day at its first hour;
    // again only when the days shown change or Today is pressed, never on a publish.

    property string scrolledFor: ""

    // The dates shown, as one word: what the views follow to open on their start.
    function scrolledDays() {
        const shownDays = []
        for (let i = 0; i < daysModel.count; i++)
            shownDays.push(daysModel.get(i).date)
        return shownDays.join(",")
    }

    // Opened on its start once the hours have their height (they had none, hidden).
    property bool scrollPending: false

    function scrollToStart(always) {
        // Out of sight, or not laid out yet: when it shows, its height known.
        if (!planning.visible || !hours.visible || hours.height <= 0) {
            planning.scrollPending = true
            return
        }
        planning.scrollPending = false
        const what = planning.scrolledDays()
        if (!always && what === planning.scrolledFor)
            return
        planning.scrolledFor = what
        const minutes = planning.todayIndex >= 0 ? Math.max(planning.frame.from, planning.minuteNow - 60) : planning.frame.from
        hours.contentY = Math.max(0, Math.min(planning.yOf(minutes) - 4, hours.contentHeight - hours.height))
    }

    onVisibleChanged: {
        if (planning.visible) {
            Qt.callLater(planning.scrollToStart, false)
            Qt.callLater(planning.listToToday)
        }
    }

    spacing: 4
    activeFocusOnTab: true
    Accessible.role: Accessible.Table
    Accessible.name: planning.sioul.text(planning.single ? "agenda-mode-day" : "agenda-mode-week")

    // The keyboard, while the timeline has it: Up and Down from thing to thing
    // in time order, Enter opens it, Ctrl+E its form, Page Up and Page Down
    // the day or the week before and after.
    Keys.onUpPressed: planning.stepFocus(-1)
    Keys.onDownPressed: planning.stepFocus(1)
    Keys.onReturnPressed: planning.openFocused()
    Keys.onEnterPressed: planning.openFocused()
    Keys.onPressed: event => {
        if (event.key === Qt.Key_E && (event.modifiers & Qt.ControlModifier)) {
            const row = planning.focused()
            if (row)
                planning.edit(row)
            event.accepted = true
        } else if (event.key === Qt.Key_PageUp || event.key === Qt.Key_PageDown) {
            planning.turn(event.key === Qt.Key_PageUp ? -1 : 1)
            event.accepted = true
        }
    }

    function openFocused() {
        const row = planning.focused()
        if (row)
            planning.open(row)
    }

    // The cards' type, for measuring their titles and times. Never shown.
    Label {
        id: titleType

        visible: false
        font.pixelSize: 12
    }
    Label {
        id: timeType

        visible: false
        font.pixelSize: 11
    }
    FontMetrics {
        id: titleFont

        font: titleType.font
    }
    FontMetrics {
        id: timeFont

        font: timeType.font
    }
    // The list's times (`listed`), measured as they are drawn.
    FontMetrics {
        id: listTimeFont

        font.pixelSize: 13
        font.features: { "tnum": 1 }
    }

    // The days' titles (the Week): a tap opens that day.
    RowLayout {
        visible: !planning.single && !planning.listed
        Layout.fillWidth: true
        spacing: 0

        Item {
            Layout.preferredWidth: planning.gutter
        }
        Repeater {
            model: daysModel

            delegate: Label {
                id: dayTitle

                required property string date

                Layout.preferredWidth: planning.dayWidth
                horizontalAlignment: Text.AlignHCenter
                // Narrow days (a phone's week): the weekday above its number.
                text: planning.dayWidth < 64 ? new Date(dayTitle.date + "T12:00:00").toLocaleDateString(planning.locale, "ddd") + "\n" + Number(dayTitle.date.slice(8, 10)) : new Date(dayTitle.date + "T12:00:00").toLocaleDateString(planning.locale, "ddd d")
                textFormat: Text.PlainText
                font.weight: dayTitle.date === planning.today ? Font.Bold : Font.DemiBold
                color: dayTitle.date === planning.today ? planning.theme.accent : dayTitle.date < planning.today ? planning.theme.muted : planning.theme.text
                Accessible.role: Accessible.Button
                Accessible.name: new Date(dayTitle.date + "T12:00:00").toLocaleDateString(planning.locale, "dddd d MMMM")

                TapHandler {
                    cursorShape: Qt.PointingHandCursor
                    onTapped: planning.openDay(dayTitle.date)
                }
            }
        }
    }

    // Whole days, days off: chips at each day's head, two rows at most, then how many more.
    RowLayout {
        visible: chipsAny.any && planning.chipsHere && !planning.listed
        Layout.fillWidth: true
        spacing: 0

        QtObject {
            id: chipsAny

            readonly property bool any: {
                planning.revision
                for (let i = 0; i < daysModel.count; i++)
                    if (daysModel.get(i).chips !== "[]")
                        return true
                return false
            }
        }
        Item {
            Layout.preferredWidth: planning.gutter
        }
        Repeater {
            model: daysModel

            delegate: ColumnLayout {
                id: band

                required property string chips
                required property string date
                required property int index
                readonly property var rows: JSON.parse(band.chips)

                Layout.preferredWidth: planning.dayWidth
                Layout.alignment: Qt.AlignTop
                spacing: 2

                Repeater {
                    model: band.rows.slice(0, 2)

                    delegate: Rectangle {
                        id: chip

                        required property var modelData
                        readonly property var item: chip.modelData.item

                        // A date asked: a diamond, filled within seven days (`tone` plain),
                        // outlined beyond; in the Day its words too ("CAF form · Date
                        // asked · four days left"), in the Week its title alone (§7.3).
                        readonly property bool dated: chip.item.kind === "dated"
                        // Its hour too, when it has one ("CAF form · 17:00 · Date asked · today"; later 10).
                        readonly property string hour: chip.dated && chip.item.time && chip.item.time.type === "at" ? chip.modelData.when || "" : ""
                        readonly property string words: chip.dated && planning.single ? planning.datedWords({ title: chip.item.title, when: chip.hour, line: chip.item.line || "" }) : chip.dated && chip.hour !== "" ? chip.item.title + " · " + chip.hour : chip.item.title

                        Layout.fillWidth: true
                        Layout.rightMargin: 3
                        implicitHeight: chipLabel.implicitHeight + 6
                        radius: 4
                        color: chip.item.kind === "day-off" ? planning.theme.button : planning.theme.surface
                        border.color: chip.item.colour ? chip.item.colour : planning.theme.line
                        Accessible.role: Accessible.Button
                        Accessible.name: chip.words

                        Rectangle {
                            objectName: "datedDiamond"
                            visible: chip.dated
                            x: 6
                            anchors.verticalCenter: parent.verticalCenter
                            width: 7
                            height: 7
                            rotation: 45
                            color: chip.dated && chip.item.tone !== "quiet" ? planning.theme.text : "transparent"
                            border.color: planning.theme.text
                            border.width: 1
                        }
                        Label {
                            id: chipLabel

                            anchors.fill: parent
                            anchors.margins: 3
                            anchors.leftMargin: chip.dated ? 17 : 3
                            text: chip.words
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            color: planning.theme.text
                        }
                        TapHandler {
                            onTapped: planning.open(planning.rowOf(chip.modelData, { date: band.date, start: planning.dayStart(band.index) }, band.index, ""))
                        }
                        TapHandler {
                            acceptedButtons: Qt.RightButton
                            // A touch has no buttons: on a touch screen, the long press below.
                            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                            onTapped: planning.menu(planning.rowOf(chip.modelData, { date: band.date, start: planning.dayStart(band.index) }, band.index, ""))
                        }
                        // On a touch screen, the menu at a long press.
                        TapHandler {
                            id: chipHold

                            acceptedDevices: PointerDevice.TouchScreen
                            onLongPressed: {
                                chip.Window.window.menuAt = chipHold.point.scenePosition
                                planning.menu(planning.rowOf(chip.modelData, { date: band.date, start: planning.dayStart(band.index) }, band.index, ""))
                            }
                        }
                    }
                }
                Label {
                    visible: band.rows.length > 2
                    text: planning.sioul.textArgs("agenda-chips-more", JSON.stringify({ n: band.rows.length - 2 }))
                    textFormat: Text.PlainText
                    font.pixelSize: 11
                    color: planning.theme.muted
                }
            }
        }
    }

    // What the day's hours are for, as the bands behind (the Day, on a computer).
    Flow {
        id: legend

        readonly property var kinds: {
            planning.revision
            if (!planning.single || planning.narrow || daysModel.count === 0)
                return []
            const seen = []
            for (const stretch of JSON.parse(daysModel.get(0).hours))
                for (const kind of stretch.kind.split("+"))
                    if (seen.indexOf(kind) < 0)
                        seen.push(kind)
            return ["work", "admin", "leisure"].filter(k => seen.indexOf(k) >= 0)
        }

        visible: legend.kinds.length > 1
        Layout.fillWidth: true
        Layout.leftMargin: planning.gutter
        spacing: 14

        Repeater {
            model: legend.kinds

            delegate: Row {
                id: kindMark

                required property string modelData

                spacing: 6

                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 4
                    height: 14
                    radius: 2
                    color: planning.areaColor(kindMark.modelData)
                }
                Label {
                    text: planning.sioul.text("area-" + kindMark.modelData)
                    textFormat: Text.PlainText
                    font.pixelSize: 12
                    color: planning.theme.muted
                }
            }
        }
    }

    function areaColor(kind) {
        return planning.theme.chartColors[Math.max(0, ["work", "admin", "leisure"].indexOf(kind))]
    }

    function tint(color, alpha) {
        return Qt.rgba(color.r, color.g, color.b, alpha)
    }

    // A thing's colour by its kind: a meal, a nap and the night as Health tints them (never red).
    function kindColor(row) {
        const colors = planning.theme.chartColors
        if (row.kind === "meal")
            return colors[1]
        if (row.kind === "nap")
            return colors[2]
        if (row.kind === "night")
            return colors[4]
        return row.colour !== "" ? row.colour : planning.theme.accent
    }

    // The hours.
    Flickable {
        id: hours

        visible: !planning.listed
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        contentWidth: width
        onHeightChanged: {
            if (planning.scrollPending)
                Qt.callLater(planning.scrollToStart, false)
        }
        contentHeight: planning.hoursShown * planning.hourHeight + 1
        boundsBehavior: Flickable.StopAtBounds
        // A mouse drags things, not the view: the wheel scrolls it.
        acceptedButtons: Qt.NoButton
        ScrollBar.vertical: ScrollBar {}

        Item {
            id: canvas

            width: hours.width
            height: hours.contentHeight

            // The hours' lines.
            Repeater {
                model: planning.listed ? 0 : planning.hoursShown

                delegate: Item {
                    id: hour

                    required property int index
                    readonly property int clock: planning.frame.from / 60 + hour.index

                    y: hour.index * planning.hourHeight
                    width: parent.width
                    height: planning.hourHeight

                    Label {
                        width: planning.gutter - 8
                        horizontalAlignment: Text.AlignRight
                        text: (hour.clock % 24 < 10 ? "0" : "") + hour.clock % 24 + ":00"
                        textFormat: Text.PlainText
                        font.pixelSize: 11
                        color: planning.theme.muted
                    }
                    Rectangle {
                        x: planning.gutter
                        width: parent.width - planning.gutter
                        height: 1
                        color: planning.theme.line
                    }
                }
            }

            // Each day: today's lighter ground, its hours by what they are for, faint.
            Repeater {
                model: planning.listed ? null : daysModel

                delegate: Item {
                    id: column

                    required property int index
                    required property string date
                    required property string hours
                    required property real start
                    readonly property var stretches: JSON.parse(column.hours)

                    x: planning.gutter + column.index * planning.dayWidth
                    width: planning.dayWidth
                    height: parent.height

                    Rectangle {
                        anchors.fill: parent
                        visible: column.date === planning.today && daysModel.count > 1
                        color: planning.theme.surface
                        opacity: 0.6
                    }
                    Rectangle {
                        width: 1
                        height: parent.height
                        color: planning.theme.line
                    }
                    Repeater {
                        model: column.stretches

                        delegate: Rectangle {
                            id: stretch

                            required property var modelData
                            readonly property int from: Math.round((stretch.modelData.start - column.start) / 60)
                            readonly property int to: Math.round((stretch.modelData.end - column.start) / 60)

                            x: planning.lane
                            y: planning.yOf(Math.max(planning.frame.from, stretch.from))
                            width: planning.dayWidth - planning.lane - 1
                            height: Math.max(0, planning.yOf(Math.min(planning.frame.to, stretch.to)) - stretch.y)
                            color: planning.areaColor(stretch.modelData.kind.split("+")[0])
                            opacity: 0.06
                        }
                    }
                }
            }

            // Every thing of every day: one delegate each, changed in place.
            Repeater {
                objectName: "planningThings"
                model: things

                delegate: Item {
                    id: card

                    required property string key
                    required property string itemId
                    required property string kind
                    required property string fold
                    required property string title
                    required property string when
                    required property int day
                    required property int from
                    required property int to
                    required property int lead
                    required property int tail
                    required property int column
                    required property int columns
                    required property string edge
                    required property string certainty
                    required property string tone
                    required property string colour
                    required property bool ends
                    required property bool cancelled
                    required property bool tentative
                    required property string location
                    required property string list
                    required property string date
                    required property real start
                    required property real at
                    required property real until
                    required property real end
                    required property string owner

                    readonly property var row: ({ kind: card.kind, day: card.day, from: card.from, to: card.to, lead: card.lead, tail: card.tail, column: card.column, columns: card.columns, edge: card.edge })
                    readonly property var box: planning.boxOf(card.row)
                    readonly property bool isNight: card.kind === "night"
                    readonly property bool isNeed: card.kind === "meal" || card.kind === "nap"
                    readonly property bool isStep: card.kind === "step" && card.fold !== "band"
                    readonly property bool isBand: card.fold === "band"
                    readonly property bool isQuiet: card.kind === "gain" || card.kind === "slack"
                    // A ride or a walk around an appointment (part E): a quiet line in its lead, never a card.
                    readonly property bool isLeg: card.kind === "leg"
                    readonly property bool pinned: card.kind === "step" && card.certainty === "held"
                    readonly property bool done: card.kind === "done"
                    readonly property bool chosen: planning.chosenId !== "" && planning.chosenId === card.itemId
                    readonly property bool focused: planning.activeFocus && planning.focusKey === card.key
                    readonly property bool underWay: planning.underKey === card.key
                    readonly property bool comingNext: planning.nextKey === card.key
                    // Taken by the hand: outlined; moved, or let go: faint where it was.
                    readonly property bool taken: planning.grab !== null && planning.grab.row.key === card.key
                    readonly property bool away: (card.taken && drag.dragging) || (planning.landed !== null && planning.landed.row.key === card.key)
                    readonly property color hue: card.isNight || card.isNeed ? planning.kindColor({ kind: card.kind, colour: card.colour }) : (card.colour !== "" ? card.colour : planning.theme.accent)
                    // The thing itself within its card: what is kept around it lighter.
                    readonly property real properTop: planning.yOf(Math.max(planning.frame.from, card.from)) - card.box.y
                    readonly property real properBottom: planning.yOf(Math.min(planning.frame.to, card.to)) - card.box.y

                    x: card.box.x
                    y: card.box.y
                    width: card.box.width
                    height: card.box.height
                    z: card.isNight ? 0 : card.isQuiet ? 1 : card.isLeg ? 3 : 2
                    opacity: card.away ? 0.3 : card.tentative || card.done ? 0.6 : 1
                    Accessible.role: Accessible.Button
                    Accessible.name: card.title + ", " + card.when

                    // Two layers for every kind, so that a week of cards stays light: the
                    // whole, lighter (what is kept around the thing: getting there and back,
                    // getting ready, coming back, winding down; the night's shading)...
                    Rectangle {
                        visible: !card.isQuiet && !card.isLeg && (card.isNight || card.isNeed || card.properTop > 1 || card.properBottom < card.height - 1)
                        anchors.fill: parent
                        radius: card.isNight ? 0 : 3
                        color: planning.tint(card.hue, card.isNight ? 0.06 : card.isNeed && card.tone === "quiet" ? 0.05 : 0.1)
                        border.width: card.isNeed ? (card.chosen || card.taken ? 2 : 1) : 0
                        border.color: card.chosen || card.focused || card.taken ? planning.theme.accent : planning.tint(card.hue, 0.45)
                    }
                    // ... and the thing itself: an appointment, a block, a step, a task
                    // done, a band as a card; a meal's or a nap's stronger tint; the
                    // night's sleep, from bedtime on (its morning part: all of it).
                    // A leg: a thin line down its time, its calendar's colour; fainter while its
                    // timetable was not checked (moved with its appointment).
                    Rectangle {
                        objectName: "planning-leg"
                        visible: card.isLeg
                        x: 2
                        width: 2
                        height: parent.height
                        radius: 1
                        color: card.hue
                        opacity: card.certainty === "unconfirmed" ? 0.45 : 0.9
                    }
                    Rectangle {
                        readonly property bool asCard: !card.isNight && !card.isNeed

                        visible: !card.isQuiet && !card.isLeg
                        x: card.isNeed ? 1 : 0
                        y: card.isNight && planning.morning(card.row) ? 0 : Math.max(0, Math.min(card.properTop, card.height - planning.cardLeast))
                        width: parent.width - (card.isNeed ? 2 : 0)
                        height: card.isNight ? parent.height - y : Math.max(card.isNeed ? 0 : planning.cardLeast, Math.min(parent.height, card.properBottom) - y)
                        radius: asCard ? 4 : 0
                        // The plan's proposal outlined, never a promise; the next one filled.
                        color: card.isNight ? planning.tint(card.hue, 0.12) : card.isNeed ? planning.tint(card.hue, card.tone === "quiet" ? 0.14 : 0.28) : card.isBand ? "transparent" : card.isStep && !card.pinned ? (card.comingNext ? planning.tint(planning.theme.accent, 0.16) : planning.theme.background) : card.chosen ? planning.theme.line : planning.theme.background
                        border.width: !asCard ? 0 : card.chosen || card.taken || card.underWay || card.focused ? 2 : 1
                        border.color: card.taken || card.chosen || card.underWay || card.comingNext ? planning.theme.accent : card.focused ? planning.theme.focus : card.isBand || card.done ? planning.theme.line : card.kind === "event" || card.kind === "block" ? card.hue : planning.theme.accent
                    }
                    // Its calendar's colour, as a bar: an appointment, a block.
                    Rectangle {
                        visible: card.kind === "event" || card.kind === "block"
                        y: Math.max(0, card.properTop)
                        width: 3
                        height: Math.max(0, card.properBottom - card.properTop)
                        radius: 2
                        color: card.hue
                    }
                    // A pinned step's pin, a block's task mark: made only there.
                    Loader {
                        active: card.pinned || card.kind === "block"
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.topMargin: Math.max(0, card.properTop) + 2
                        anchors.rightMargin: 2

                        sourceComponent: Component {
                            Icon {
                                objectName: "planning-pin"
                                iconName: card.pinned ? "pin" : "view-task"
                                size: 12
                                color: planning.theme.accent
                                tip: planning.sioul.text("agenda-task-mark")
                            }
                        }
                    }
                    ColumnLayout {
                        // Meals and naps in the Week are bands without words; time for you, quiet words.
                        visible: !card.isNight && !(card.isNeed && !planning.single)
                        anchors.fill: parent
                        anchors.leftMargin: 6
                        anchors.rightMargin: 2 + (card.pinned || card.kind === "block" ? 14 : 0)
                        anchors.topMargin: Math.max(planning.cardPadding / 2, Math.min(card.properTop, card.height - planning.cardLeast))
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: card.edge !== "" ? card.when + " · " + card.title : card.done ? "✓  " + card.title : card.title
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            wrapMode: Text.Wrap
                            maximumLineCount: Math.max(1, Math.floor((card.height - planning.cardPadding) / planning.titleLine))
                            font.pixelSize: card.isLeg ? 11 : 12
                            font.italic: card.isQuiet || card.isBand || (card.isLeg && card.certainty === "unconfirmed")
                            font.strikeout: card.cancelled
                            font.weight: card.underWay ? Font.DemiBold : Font.Normal
                            color: card.isQuiet || card.isBand || card.done || card.isLeg ? planning.theme.muted : planning.theme.text
                        }
                        Label {
                            readonly property string beside: card.location !== "" ? card.location : card.list !== "" ? planning.sioul.textWith("agenda-task-of", "list", card.list) : ""
                            readonly property string line: card.when + (beside !== "" ? " · " + beside : "")

                            // A band's title says its time already.
                            visible: card.edge === "" && !card.isQuiet && !card.isBand && card.height >= planning.cardPadding + planning.titleLine + planning.timeLine
                            Layout.fillWidth: true
                            text: timeFont.advanceWidth(line) <= card.width - 8 ? line : card.when
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 11
                            color: planning.theme.muted
                        }
                    }
                }
            }

            // The doses: a mark on their lane at each minute (today's by the
            // window's clock, as the Health job read them), a tick on a day to
            // come; at the frame's edge with its time when inside the night.
            Repeater {
                model: planning.listed ? null : marksModel

                delegate: Item {
                    id: mark

                    required property int day
                    required property string date
                    required property int minute
                    required property string keys
                    required property string edge
                    readonly property var known: planning.markState(mark.keys, mark.date)

                    x: planning.gutter + mark.day * planning.dayWidth + 1
                    y: mark.edge === "top" ? 0 : mark.edge === "bottom" ? planning.yOf(planning.frame.to) - 16 : planning.yOf(mark.minute) - 8
                    width: planning.lane
                    height: 16
                    z: 3
                    Accessible.role: Accessible.Button
                    Accessible.name: planning.sioul.textWith("agenda-dose-at", "time", (mark.minute / 60 < 10 ? "0" : "") + Math.floor(mark.minute / 60) + ":" + (mark.minute % 60 < 10 ? "0" : "") + mark.minute % 60) + (mark.known.words ? ": " + mark.known.words : "")

                    DoseMark {
                        width: planning.lane
                        height: parent.height
                        theme: planning.theme
                        known: mark.known.state
                    }
                    Label {
                        visible: mark.edge !== ""
                        x: planning.lane
                        anchors.verticalCenter: parent.verticalCenter
                        text: (mark.minute / 60 < 10 ? "0" : "") + Math.floor(mark.minute / 60) + ":" + (mark.minute % 60 < 10 ? "0" : "") + mark.minute % 60
                        font.pixelSize: 10
                        color: planning.theme.muted
                    }
                }
            }

            // Now: a quiet line in today's column; the clock moves it alone.
            Rectangle {
                objectName: "nowLine"
                visible: planning.todayIndex >= 0 && planning.minuteNow >= planning.frame.from && planning.minuteNow <= planning.frame.to
                x: planning.gutter + Math.max(0, planning.todayIndex) * planning.dayWidth
                y: planning.yOf(planning.minuteNow)
                width: planning.dayWidth
                height: 2
                z: 4
                color: planning.theme.accent
            }

            // Where the thing taken goes, and a tag with its times above it, out
            // of the finger's way (under it at the top of the view).
            Rectangle {
                id: ghost

                readonly property var going: planning.grab !== null && drag.dragging ? planning.target(planning.grab, planning.pointer) : null
                readonly property var shownAt: ghost.going ? { row: planning.grab.row, column: ghost.going.column, start: ghost.going.start, end: ghost.going.end, text: planning.said(planning.grab, ghost.going) } : planning.landed
                readonly property real dayStart: ghost.shownAt ? planning.dayStart(ghost.shownAt.column) : 0
                readonly property real from: ghost.shownAt ? Math.max(planning.frame.from, (ghost.shownAt.start - ghost.dayStart) / 60) : 0
                readonly property real until: ghost.shownAt ? Math.min(planning.frame.to, (ghost.shownAt.end - ghost.dayStart) / 60) : 0

                visible: ghost.shownAt !== null && ghost.until > ghost.from
                x: ghost.shownAt ? planning.gutter + ghost.shownAt.column * planning.dayWidth + planning.lane + 2 : 0
                y: planning.yOf(ghost.from)
                width: planning.slotWidth
                height: Math.max(planning.cardLeast, (ghost.until - ghost.from) / 60 * planning.hourHeight - 1)
                radius: 4
                color: Qt.rgba(planning.theme.accent.r, planning.theme.accent.g, planning.theme.accent.b, 0.16)
                border.color: planning.theme.accent
                border.width: 2
                z: 5
            }
            Rectangle {
                id: tag

                readonly property bool above: ghost.y - tag.height - 4 >= hours.contentY + 2

                visible: ghost.visible
                x: Math.max(planning.gutter, Math.min(canvas.width - tag.width - 2, ghost.x))
                y: tag.above ? ghost.y - tag.height - 4 : ghost.y + ghost.height + 4
                width: tagText.implicitWidth + 12
                height: tagText.implicitHeight + 8
                radius: 4
                color: planning.theme.surface
                border.color: planning.theme.accent
                z: 6

                Label {
                    id: tagText

                    x: 6
                    y: 4
                    text: ghost.shownAt ? ghost.shownAt.text : ""
                    textFormat: Text.PlainText
                    font.features: { "tnum": 1 }
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    color: planning.theme.text
                }
            }

            TimeDrag {
                id: drag

                anchors.fill: parent
                z: 7
                flick: hours
                onHovered: position => {
                    const thing = position.x < 0 ? null : planning.thingAt(position)
                    drag.cursorShape = !thing ? Qt.ArrowCursor : !planning.movable(thing.row) ? Qt.PointingHandCursor : thing.part !== "body" ? Qt.SizeVerCursor : Qt.OpenHandCursor
                }
                onTapped: position => {
                    planning.forceActiveFocus()
                    const thing = planning.thingAt(position)
                    // A dose's mark, over the night's shading too (its lane lies in it): the dose.
                    const dose = !thing || thing.row.kind === "night" ? planning.markAt(position) : null
                    if (dose) {
                        planning.open(dose)
                    } else if (thing) {
                        planning.focusKey = thing.row.key
                        planning.open(thing.row)
                    }
                }
                onDoubleTapped: position => {
                    const column = planning.columnAt(position.x)
                    const thing = planning.thingAt(position)
                    // Never over a fixed thing: an appointment, a pinned task, a meal, a nap.
                    if (column >= 0 && column < daysModel.count && (!thing || thing.row.kind === "night" || (thing.row.kind === "step" && thing.row.certainty !== "held") || thing.row.kind === "gain" || thing.row.kind === "slack"))
                        planning.newAt(daysModel.get(column).date, Math.floor(planning.minuteAt(position.y) / 60))
                }
                onMenuAsked: (position, scenePosition) => {
                    planning.grab = null
                    const thing = planning.thingAt(position)
                    if (!thing)
                        return
                    planning.Window.window.menuAt = scenePosition
                    planning.menu(thing.row)
                }
                onHeld: (position, scenePosition) => {
                    const thing = planning.thingAt(position)
                    if (!thing)
                        return
                    if (planning.movable(thing.row)) {
                        planning.take(thing, position)
                        drag.pickUp(true)
                    } else {
                        planning.Window.window.menuAt = scenePosition
                        planning.menu(thing.row)
                    }
                }
                onStarted: position => {
                    // A finger took it when held; a mouse takes what it pressed on.
                    if (drag.holding)
                        return
                    const thing = planning.thingAt(position)
                    if (thing && planning.movable(thing.row))
                        planning.take(thing, position)
                    else
                        planning.grab = null
                }
                onMoved: position => planning.pointer = position
                onReleased: position => planning.drop(position)
                onCanceled: planning.grab = null
            }

            // A phone's Day: a swipe across goes to the next day, or the one
            // before (here, under the view, which holds the finger from its press).
            DragHandler {
                id: swipe

                // How far the finger went across and down, while it swipes.
                property point went: Qt.point(0, 0)

                enabled: planning.single && planning.narrow && planning.grab === null && !drag.holding
                target: null
                acceptedDevices: PointerDevice.TouchScreen
                xAxis.enabled: true
                yAxis.enabled: false
                onCentroidChanged: {
                    if (swipe.active)
                        swipe.went = Qt.point(swipe.centroid.scenePosition.x - swipe.centroid.scenePressPosition.x, swipe.centroid.scenePosition.y - swipe.centroid.scenePressPosition.y)
                }
                onActiveChanged: {
                    if (swipe.active) {
                        swipe.went = Qt.point(0, 0)
                        return
                    }
                    if (Math.abs(swipe.went.x) > 60 && Math.abs(swipe.went.x) > 2 * Math.abs(swipe.went.y))
                        planning.turn(swipe.went.x < 0 ? 1 : -1)
                }
            }
        }
    }

    // The Week as a list of days (`listed`): made as it scrolls, its rows changed in place.
    ListView {
        id: list

        visible: planning.listed
        Layout.fillWidth: true
        Layout.fillHeight: true
        clip: true
        spacing: 2
        model: planning.listed ? listRows : null
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}
        // Opened on today, when today is shown; else on the first day.
        onCountChanged: Qt.callLater(planning.listToToday)

        delegate: Item {
            id: line

            required property int index
            required property string key
            required property string kind
            required property string title
            required property string line
            required property string when
            required property string date
            required property string colour
            required property string certainty
            required property string location
            required property string list
            required property string fold
            required property string tone
            readonly property bool isTitle: line.kind === "day-title"
            readonly property bool isLine: line.kind === "doses-line" || line.kind === "plan-line" || line.kind === "more-line" || line.kind === "free"
            readonly property bool quiet: line.isLine || line.kind === "gain" || line.kind === "slack" || line.kind === "night" || line.fold === "band" || line.kind === "done"

            width: ListView.view.width - 12
            height: shown.implicitHeight + (line.isTitle ? 14 : 8)
            Accessible.role: line.isLine ? Accessible.StaticText : Accessible.Button
            Accessible.name: line.isTitle || line.isLine ? line.title : line.title + ", " + line.when

            ColumnLayout {
                id: shown

                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 4
                spacing: 2

                // A day: its title (today's in the accent), its whole days under it.
                Label {
                    visible: line.isTitle
                    Layout.fillWidth: true
                    text: line.title
                    textFormat: Text.PlainText
                    font.pixelSize: 16
                    font.weight: line.date === planning.today ? Font.Bold : Font.DemiBold
                    color: line.date === planning.today ? planning.theme.accent : line.date < planning.today ? planning.theme.muted : planning.theme.text
                }
                Label {
                    visible: line.isTitle && line.line !== ""
                    Layout.fillWidth: true
                    text: line.line
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: planning.theme.muted
                }
                // Today's doses on one line: their marks and times, never their medicine.
                Flow {
                    visible: line.kind === "doses-line"
                    Layout.fillWidth: true
                    spacing: 10

                    Label {
                        text: planning.sioul.text("agenda-show-doses")
                        textFormat: Text.PlainText
                        font.pixelSize: 12
                        color: planning.theme.muted
                    }
                    Repeater {
                        model: line.kind === "doses-line" ? planning.marksOn(line.date) : []

                        delegate: Row {
                            id: listMark

                            required property var modelData
                            readonly property var known: planning.markState(listMark.modelData.keys, listMark.modelData.date)

                            spacing: 4
                            Accessible.role: Accessible.StaticText
                            Accessible.name: planning.sioul.textWith("agenda-dose-at", "time", planning.clockOf(listMark.modelData.minute)) + (listMark.known.words ? ": " + listMark.known.words : "")

                            DoseMark {
                                anchors.verticalCenter: parent.verticalCenter
                                theme: planning.theme
                                known: listMark.known.state
                            }
                            Label {
                                text: planning.clockOf(listMark.modelData.minute)
                                textFormat: Text.PlainText
                                font.pixelSize: 12
                                font.features: { "tnum": 1 }
                                color: planning.theme.muted
                            }
                        }
                    }
                }
                // A thing: its time, its title; what kind it is by its mark.
                RowLayout {
                    visible: !line.isTitle && !line.isLine
                    Layout.fillWidth: true
                    spacing: 8

                    // Its kind's bar; a date asked's diamond, filled within seven days (one
                    // object either way: a phone's week list has one per row).
                    Rectangle {
                        readonly property bool diamond: line.kind === "dated"

                        objectName: diamond ? "listDiamond" : ""
                        Layout.preferredWidth: diamond ? 7 : 4
                        Layout.preferredHeight: diamond ? 7 : -1
                        Layout.fillHeight: !diamond
                        Layout.minimumHeight: diamond ? 7 : 16
                        Layout.alignment: Qt.AlignVCenter
                        rotation: diamond ? 45 : 0
                        radius: diamond ? 0 : 2
                        color: diamond ? (line.tone !== "quiet" ? planning.theme.text : "transparent") : line.kind === "step" && line.certainty !== "held" ? "transparent" : planning.kindColor({ kind: line.kind, colour: line.colour })
                        border.width: diamond || (line.kind === "step" && line.certainty !== "held") ? 1 : 0
                        border.color: diamond ? planning.theme.text : planning.theme.accent
                        opacity: !diamond && line.quiet ? 0.5 : 1
                    }
                    Label {
                        visible: line.kind !== "dated"
                        Layout.preferredWidth: listTimeFont.advanceWidth("00:00 – 00:00") + 4
                        Layout.alignment: Qt.AlignTop
                        text: line.when
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 13
                        font.features: { "tnum": 1 }
                        color: planning.theme.muted
                    }
                    Label {
                        Layout.fillWidth: true
                        // Its words wrap at the row's width: they never widen it.
                        Layout.preferredWidth: 0
                        // An appointment's line under it (Leave at, its legs, its next step: the unified agenda, §4.7).
                        text: line.kind === "dated" ? line.line : (line.kind === "done" ? "✓  " : "") + line.title + (line.location !== "" ? "  ·  " + line.location : "") + (line.kind === "event" && line.line !== "" ? "\n" + line.line : "")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 14
                        font.italic: line.kind === "gain" || line.kind === "slack" || line.fold === "band"
                        color: line.quiet ? planning.theme.muted : planning.theme.text
                    }
                }
                Label {
                    visible: line.isLine && line.kind !== "doses-line"
                    Layout.fillWidth: true
                    text: line.title
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: planning.theme.muted
                }
            }
            TapHandler {
                id: lineTap

                enabled: !line.isLine
                onTapped: {
                    if (line.isTitle)
                        planning.openDay(line.date)
                    else
                        planning.open(listRows.get(line.index))
                }
                // A long press: its menu, where the finger is.
                onLongPressed: {
                    if (line.isTitle)
                        return
                    line.Window.window.menuAt = lineTap.point.scenePosition
                    planning.menu(listRows.get(line.index))
                }
            }
            TapHandler {
                enabled: !line.isLine && !line.isTitle
                acceptedButtons: Qt.RightButton
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                onTapped: eventPoint => {
                    line.Window.window.menuAt = eventPoint.scenePosition
                    planning.menu(listRows.get(line.index))
                }
            }
        }
    }

    // The list opened on today's title (its days changed), never moved on a publish.
    property string listedFor: ""

    function listToToday() {
        const what = planning.scrolledDays()
        if (!planning.listed || list.count === 0 || what === planning.listedFor)
            return
        planning.listedFor = what
        for (let i = 0; i < listRows.count; i++) {
            if (listRows.get(i).key === "day-title|" + planning.today) {
                list.positionViewAtIndex(i, ListView.Beginning)
                return
            }
        }
        list.positionViewAtBeginning()
    }

    // Today's marks, for the list's doses line.
    function marksOn(date) {
        const out = []
        for (let i = 0; i < marksModel.count; i++)
            if (marksModel.get(i).date === date)
                out.push(marksModel.get(i))
        return out
    }

    // "08:30", from minutes of the day.
    function clockOf(minute) {
        const m = ((Math.round(minute) % 1440) + 1440) % 1440
        return (m < 600 ? "0" : "") + Math.floor(m / 60) + ":" + (m % 60 < 10 ? "0" : "") + m % 60
    }

    // What today's plan could not place, and the words of how today is laid:
    // under the hours, in the Day or the Week that shows today.
    Label {
        readonly property string said: {
            planning.revision
            for (let i = 0; i < daysModel.count; i++) {
                const day = daysModel.get(i)
                if (day.date === planning.today)
                    return [day.more, day.said].filter(t => t !== "").join("  ")
            }
            return ""
        }

        objectName: "planningSaid"
        visible: !planning.listed && said !== ""
        Layout.fillWidth: true
        Layout.leftMargin: planning.gutter
        text: said
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: planning.theme.muted
    }

    // A dose's mark under the hand: {kind: "dose", date, key} for its card's day, else null.
    function markAt(p) {
        for (let i = 0; i < marksModel.count; i++) {
            const mark = marksModel.get(i)
            const x = planning.gutter + mark.day * planning.dayWidth + 1
            const y = mark.edge === "top" ? 8 : mark.edge === "bottom" ? planning.yOf(planning.frame.to) - 8 : planning.yOf(mark.minute)
            // A 30-point tap, whatever the mark's size (§4.9).
            if (Math.abs(p.x - (x + planning.lane / 2)) <= 15 && Math.abs(p.y - y) <= 15)
                return { kind: "dose", date: mark.date, srcKey: JSON.parse(mark.keys)[0] || "", srcDate: mark.date, key: mark.key, json: "{}" }
        }
        return null
    }

    // A repeating event let go: this time alone, or every time. Made the first time.
    Later {
        id: which

        sourceComponent: Component {
            WhichTimes {
                sioul: planning.sioul
                theme: planning.theme
                onChosen: onlyThis => planning.save(onlyThis)
                onDropped: planning.landed = null
            }
        }
    }
}
