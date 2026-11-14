// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What can change a meal, a nap or the night that day, one menu for Health's
// list, the agenda and its card (the unified agenda, part 3): had already,
// just now or at a time, for a meal or a nap of today not begun; later by
// Health's minutes, moved, its times, no notice that day or back, the alarm
// at the end of that night or that nap (a switch, on or off for it alone),
// back to usual, taken out that day or put back.
// It changes nothing itself: `acted(action)`, `formAsked(mode)` and
// `hadAsked(at)` say what was chosen, and the one who opened it writes that
// day's change (`changeNeed`, `hadNeed`).

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
    // "Had it": `at` "now", or "pick" for a time picked (HadAt.qml).
    signal hadAsked(string at)

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

    // A meal or a nap of today not begun yet (`needItem`'s `ahead`): had already.
    MenuItem {
        objectName: "needHadNow"
        visible: menu.day !== null && menu.day.ahead === true && !menu.off && !menu.alarmOnly
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("need-had-now")
        onTriggered: menu.hadAsked("now")
    }
    MenuItem {
        objectName: "needHadAt"
        visible: menu.day !== null && menu.day.ahead === true && !menu.off && !menu.alarmOnly
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("need-had-at")
        onTriggered: menu.hadAsked("pick")
    }
    MenuSeparator {
        visible: menu.day !== null && menu.day.ahead === true && !menu.off && !menu.alarmOnly
        height: visible ? implicitHeight : 0
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
    // The alarm at its end, this night or this nap alone, whatever the usual days say: a switch,
    // ticked when it rings (docs/health.md, "The alarm at waking"). A block known only from the
    // timeline (the week before's) says no waking: none offered.
    MenuItem {
        id: alarmSwitch

        objectName: "needAlarm"
        visible: menu.item !== null && (menu.item.kind === "sleep" || menu.item.kind === "nap") && (menu.item.waking || "") !== "" && !menu.off
        height: visible ? implicitHeight : 0
        checkable: true
        checked: menu.item !== null && menu.item.rings === true
        text: menu.theme.plain(menu.item === null ? "" : menu.sioul.textWith(menu.item.kind === "sleep" ? "wake-this-night" : "wake-this-nap", "time", menu.item.waking || ""))
        onTriggered: {
            menu.act(menu.item.rings === true ? "no-alarm" : "alarm")
            // As the block says, once it is read again: never the tick a press left.
            alarmSwitch.checked = Qt.binding(() => menu.item !== null && menu.item.rings === true)
        }
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
