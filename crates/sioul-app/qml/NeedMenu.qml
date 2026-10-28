// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What can change a meal, a nap or the night that day, one menu for Health's
// list, the agenda and its card (the unified agenda, part 3): later by Health's
// minutes, moved, its times, no notice that day or back, the alarm the morning
// the night ends, back to usual, taken out that day or put back. It changes
// nothing itself: `acted(action)` and `formAsked(mode)` say what was chosen,
// and the one who opened it writes that day's change (`changeNeed`).

import QtQuick
import QtQuick.Controls.Basic

SioulMenu {
    id: menu

    required property var sioul
    required property var theme
    // "N min later", as Health's settings say.
    property int later: 15

    signal acted(string action)
    signal formAsked(string mode)

    property var day: null
    property var item: null
    // A night begun on a past day, not over: its alarm alone (the rest of that day is as it was).
    property bool alarmOnly: false
    readonly property bool today: menu.day !== null && menu.day.today
    readonly property bool off: menu.item !== null && menu.item.off
    readonly property bool added: menu.item !== null && menu.item.added

    function show(day, item, under, alarmOnly) {
        menu.day = day
        menu.item = item
        menu.alarmOnly = alarmOnly === true
        if (under)
            menu.popup(under, 0, under.height)
        else
            menu.popup()
    }

    function act(action) {
        menu.acted(action)
    }

    MenuItem {
        visible: !menu.off && !menu.alarmOnly
        height: visible ? implicitHeight : 0
        text: menu.theme.plain(menu.sioul.textWith("need-later-n", "minutes", String(menu.later)))
        onTriggered: menu.act("later")
    }
    MenuItem {
        visible: !menu.off && !menu.alarmOnly
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("need-move-to") + "…"
        onTriggered: menu.formAsked("move")
    }
    MenuItem {
        visible: !menu.off && !menu.alarmOnly
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("need-change-times")
        onTriggered: menu.formAsked("times")
    }
    // No notice that day, kept free all the same; or back.
    MenuItem {
        visible: !menu.off && !menu.alarmOnly
        height: visible ? implicitHeight : 0
        text: menu.item === null ? "" : menu.sioul.text(menu.item.quiet ? (menu.today ? "need-unskip" : "need-unskip-day") : (menu.today ? "needs-not-today" : "need-not-that-day"))
        onTriggered: menu.act(menu.item.quiet ? "loud" : "quiet")
    }
    // The alarm at waking, the morning this night ends: none that morning, or back (docs/health.md).
    // A night known only from the timeline (the week before's) has no alarm said: none offered.
    MenuItem {
        visible: menu.item !== null && menu.item.kind === "sleep" && (menu.item.alarm || "") !== "" && !menu.off
        height: visible ? implicitHeight : 0
        text: menu.theme.plain(menu.item === null ? "" : menu.sioul.textWith(menu.item.alarm_skipped ? "wake-unskip" : "wake-skip", "time", menu.item.alarm || ""))
        onTriggered: menu.act(menu.item.alarm_skipped ? "alarm" : "no-alarm")
    }
    MenuItem {
        visible: menu.item !== null && menu.item.changed && !menu.off && !menu.alarmOnly
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("need-as-usual")
        onTriggered: menu.act("usual")
    }
    MenuSeparator {
        visible: !menu.alarmOnly
        height: visible ? implicitHeight : 0
    }
    // Taken out that day (its time free for other things), or put back; one added that day, gone.
    MenuItem {
        visible: !menu.alarmOnly
        height: visible ? implicitHeight : 0
        text: menu.sioul.text(menu.off ? "need-put-back" : menu.added ? "needs-remove" : menu.today ? "need-remove-today" : "need-remove-day")
        onTriggered: menu.act(menu.off ? "on" : "off")
    }
}
