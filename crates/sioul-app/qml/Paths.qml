// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Where a thing moved on the agenda's timeline goes: each kind by its own
// writer, as its item says (`timeline::Can::drag`), never another. An
// appointment or a time block moves through its own file (`moveEvent`), a
// step of the plan is pinned there (`pinTask`), a meal, a nap or the night
// changes that day (`dragNeed`). Nothing else is ever sent: what may not move
// says so where it would. Each returns what went wrong, else "" (Rust's
// writers refuse what their rule refuses, whoever asks).

import QtQuick

QtObject {
    id: paths

    required property var sioul

    // "09:30", on the clock, from Unix seconds.
    function clock(seconds) {
        return Qt.formatTime(new Date(seconds * 1000), "HH:mm")
    }

    // "2026-10-06T14:30", a time given, from Unix seconds.
    function local(seconds) {
        const d = new Date(seconds * 1000)
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()) + "T" + pad(d.getHours()) + ":" + pad(d.getMinutes())
    }

    // `thing` (a row of the timeline: its mover, `can.drag`; its source, its times in Unix
    // seconds) moved: `to` is {start, end} of the thing itself in Unix
    // seconds, `part` "body", "top" or "bottom", `onlyThis` for one time of
    // a repeating event. Returns what went wrong, else "".
    function drop(thing, part, to, onlyThis) {
        if (thing.mover === "event")
            return paths.sioul.moveEvent(thing.srcKey, thing.srcStart, to.start, to.end, onlyThis === true)
        if (thing.mover === "pin")
            return paths.sioul.pinTask(thing.owner, paths.local(to.start), Math.max(5, Math.round((to.end - to.start) / 60)))
        if (thing.mover === "need")
            return paths.sioul.dragNeed(JSON.stringify(paths.needEdit(thing, part, to)))
        return ""
    }

    // What the day is asked (`health::drag_need`): a meal or a nap moved
    // from its getting ready, or its times; the night's bedtime (its winding
    // down kept as long) or its waking. `to` holds the new start and end of
    // what was taken: for a meal or a nap its whole time, getting ready and
    // coming back included; for the night its winding down and waking.
    function needEdit(thing, part, to) {
        const asked = { date: thing.srcDate, key: thing.srcKey }
        if (thing.kind === "night") {
            const bed = thing.at + (to.start - thing.start)
            if (part === "top")
                return Object.assign(asked, { action: "times", from: paths.clock(to.start), bed: paths.clock(bed), to: paths.clock(thing.until) })
            return Object.assign(asked, { action: "times", from: paths.clock(thing.start), bed: paths.clock(thing.at), to: paths.clock(to.end) })
        }
        if (part === "bottom")
            return Object.assign(asked, { action: "times", from: paths.clock(to.start), to: paths.clock(to.end) })
        return Object.assign(asked, { action: "move", from: paths.clock(to.start) })
    }
}
