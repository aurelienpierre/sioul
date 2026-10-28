// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A meal's, a nap's or the night's card, where you are (the unified agenda,
// part 3): that day's row as Health's list has it (`needItem(date, key)`),
// read from the health file and the days' changes alone, never a dose
// record. Its name, its times ("12:10 – 13:00, eating from 12:30"), how that
// day has it; Edit opens its form for that day (NeedDayForm); its own actions
// are the menu's (NeedMenu): later, moved, no notice that day, the alarm,
// back to usual. A day gone by, or one over: as it was, said so. Health
// things have no address: no "Tied to it", no Add, no Link.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CardFrame {
    id: card

    required property var window
    // Its day ("2026-10-28") and its key ("meal:1", "nap:0", "sleep", "added-…").
    property string date: ""
    property string key: ""
    // What `needItem` read: {date, title, today, past, later, item: {…}}.
    property var need: null
    readonly property var item: card.need && card.need.item ? card.need.item : null
    readonly property bool changeable: card.item !== null && !card.need.past && !card.item.past
    // A night begun the day before and not over: its alarm alone.
    readonly property bool alarmOnly: card.item !== null && card.item.kind === "sleep" && !card.changeable && card.item.alarm_open === true && (card.item.alarm || "") !== ""

    function read() {
        const read = card.date !== "" && card.key !== "" ? JSON.parse(card.sioul.needItem(card.date, card.key) || "{}") : {}
        card.need = read.item ? read : null
        if (card.item)
            card.host.named("", card.item.name)
    }

    onDateChanged: card.read()
    onKeyChanged: card.read()
    Component.onCompleted: card.read()

    // That day changed (here, from Health, on another device): read again.
    Connections {
        target: card.sioul

        function onHealthViewChanged() {
            if (card.visible)
                card.read()
        }
        function onAgendaChanged() {
            if (card.visible)
                card.read()
        }
    }

    // That day's change (`changeNeed`): the card, Health and the plan follow.
    function act(action) {
        const problem = card.sioul.changeNeed(JSON.stringify({ date: card.date, key: card.key, action: action }))
        if (problem !== "")
            card.sioul.status = problem
        else
            card.read()
    }

    function edit(mode) {
        if (card.need)
            dayForm.now().edit(mode, card.need, card.item)
    }

    // Its kind as the timeline names it (the health file's "sleep" is the night).
    readonly property string kind: card.item === null ? "" : card.item.kind === "sleep" ? "night" : card.item.kind

    iconName: card.theme.kindIcons[card.kind] || ""
    dot: card.theme.chartColors[card.item && card.item.kind === "nap" ? 2 : 1]
    title: card.item ? card.item.name : card.sioul.text("card-gone")
    when: card.need ? card.need.title + " · " + card.item.from + " – " + card.item.to + (card.item.detail ? ", " + card.item.detail : "") : ""
    from: card.item ? card.item.note : ""
    editable: card.changeable && !card.item.off
    why: card.item === null ? "" : card.changeable ? "" : card.sioul.text(card.need.past ? "need-past" : "need-over")
    onEditAsked: card.edit("times")

    Label {
        objectName: "needCardUsual"
        Layout.fillWidth: true
        text: card.sioul.text("card-needs-usual")
        textFormat: Text.PlainText
        color: card.theme.accent

        TapHandler {
            cursorShape: Qt.PointingHandCursor
            onTapped: card.window.openThing({ kind: "needs", uri: "", key: "" })
        }
    }

    actions: [
        Button {
            objectName: "needCardLater"
            visible: card.changeable && !card.item.off && card.item.kind !== "sleep"
            flat: true
            text: card.theme.plain(card.sioul.textWith("need-later-n", "minutes", String(card.need ? card.need.later : 15)))
            onClicked: card.act("later")
        },
        Button {
            visible: card.changeable && !card.item.off && card.item.kind !== "sleep"
            flat: true
            text: card.sioul.text("need-move-to") + "…"
            onClicked: card.edit("move")
        },
        // No notice that day, kept free all the same; or back.
        Button {
            visible: card.changeable && !card.item.off && card.item.kind !== "sleep"
            flat: true
            text: card.item === null ? "" : card.sioul.text(card.item.quiet ? (card.need.today ? "need-unskip" : "need-unskip-day") : (card.need.today ? "needs-not-today" : "need-not-that-day"))
            onClicked: card.act(card.item.quiet ? "loud" : "quiet")
        },
        // The alarm at waking, the morning this night ends: none that morning, or back.
        Button {
            objectName: "needCardAlarm"
            visible: card.item !== null && card.item.kind === "sleep" && (card.item.alarm || "") !== "" && !card.item.off && (card.changeable || card.alarmOnly)
            flat: true
            text: card.item === null ? "" : card.theme.plain(card.sioul.textWith(card.item.alarm_skipped ? "wake-unskip" : "wake-skip", "time", card.item.alarm || ""))
            onClicked: card.act(card.item.alarm_skipped ? "alarm" : "no-alarm")
        },
        Button {
            visible: card.changeable && card.item.changed && !card.item.off
            flat: true
            text: card.sioul.text("need-as-usual")
            onClicked: card.act("usual")
        },
        // Taken out that day (its time free for other things), or put back.
        Button {
            visible: card.changeable
            flat: true
            text: card.item === null ? "" : card.sioul.text(card.item.off ? "need-put-back" : card.item.added ? "needs-remove" : card.need.today ? "need-remove-today" : "need-remove-day")
            onClicked: card.act(card.item.off ? "on" : "off")
        }
    ]

    Later {
        id: dayForm

        sourceComponent: Component {
            NeedDayForm {
                sioul: card.sioul
                theme: card.theme
                keeper: card.host.keeper
            }
        }
    }
}
