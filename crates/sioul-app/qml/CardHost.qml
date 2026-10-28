// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The card host (the unified agenda, part 3): every object's card where you
// are, one host per window, beside the page on a computer (over its right
// part where the page would lose its room), the whole page on a phone. It
// opens by a reference that every row carries (`window.openCard(ref)`: its
// kind, its source's key and start or date, its task), and holds one card
// per kind, each made the first time it is needed (on a phone, an
// appointment's and a meal's unloaded once closed): an appointment's
// (EventCard), a task's (the window's one TaskPanel: a step, a time block
// and a task done open their task), a meal's, a nap's or the night's
// (NeedCard), a date asked's (DatedCard; a task's date opens its task). A dose keeps opening its day in Health, where its medicine is
// named. It keeps the card you came from, one level deep (`follow`,
// `back`); Back and Escape close it first, and on a phone a place opened
// closes it (`leave`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

Item {
    id: host

    required property var sioul
    required property var theme
    required property var window

    // The card open: {card: "event" | "task" | "need", kind, srcKey, srcStart,
    // srcDate, owner, date, itemId, title, when, at, until, certainty, making}; else null.
    property var ref: null
    // The card it was opened from, one level deep: Back goes there.
    property var previous: null
    readonly property bool open: host.ref !== null
    readonly property string kind: host.open ? host.ref.card : ""
    // As wide as the card reads: the task's form's own, else about seventy characters.
    readonly property real naturalWidth: host.kind === "task" && host.taskPanel ? host.taskPanel.naturalWidth : 520
    // The kinds' cards made so far (on a phone, an appointment's and a meal's unloaded once closed).
    property var made: ({})
    readonly property var taskPanel: taskLoader.item
    readonly property EventCard eventCard: eventLoader.item as EventCard
    readonly property NeedCard needCard: needLoader.item as NeedCard
    readonly property DatedCard datedCard: datedLoader.item as DatedCard
    // What you typed in a card's form closed without Save, by thing, until you save or cancel.
    readonly property QtObject keeper: QtObject {
        property var forms: ({})
    }
    // What a task's card offers to choose from (`taskCardChoices`).
    property var choices: ({ projects: [], kinds: [], categories: [] })

    // An event's file and the time a reminder named ("…/yoga.ics#1793520000",
    // `remind::event_time`): {key, start}; the file alone, start 0 (its next time).
    function eventTime(key) {
        const named = /^(.*)#(\d+)$/.exec(key || "")
        return named ? { key: named[1], start: Number(named[2]) } : { key: key || "", start: 0 }
    }

    // The card a reference opens, by its kind; "" for none here.
    function cardOf(ref) {
        if (ref.kind === "event" || (ref.kind === "block" && (ref.owner || "") === ""))
            return "event"
        if (ref.kind === "task" || ref.kind === "step" || ref.kind === "block" || ref.kind === "done")
            return (ref.owner || "") !== "" || ref.making === true ? "task" : ""
        if (ref.kind === "meal" || ref.kind === "nap" || ref.kind === "night")
            return "need"
        // A date asked: a task's opens its task; the others their dated card.
        if (ref.kind === "dated")
            return ref.datedKind === "task" ? ((ref.owner || "") !== "" ? "task" : "") : ((ref.datedKind || "") !== "" && (ref.srcKey || ref.key || "") !== "" ? "dated" : "")
        return ""
    }

    // A plain copy of what the card needs (a timeline's row is a live model row).
    function copyOf(ref, card) {
        return {
            card: card,
            kind: ref.kind || "",
            srcKey: ref.srcKey || ref.key || "",
            srcStart: ref.srcStart || ref.start || 0,
            srcDate: ref.srcDate || ref.date || "",
            owner: ref.owner || "",
            date: ref.date || "",
            itemId: ref.itemId || "",
            title: ref.title || "",
            when: ref.when || "",
            at: ref.at || 0,
            certainty: ref.certainty || "",
            datedKind: ref.datedKind || "",
            making: ref.making === true
        }
    }

    // Opened from a page: the card shown, nothing kept behind it.
    function show(ref) {
        return host.place(ref, false)
    }

    // Opened from a card (its task, its appointment): the one it came from kept, one level deep.
    function follow(ref) {
        return host.place(ref, host.open)
    }

    function place(ref, keep) {
        const card = host.cardOf(ref)
        if (card === "")
            return false
        host.leaveTask()
        host.previous = keep ? host.ref : null
        const made = Object.assign({}, host.made)
        made[card] = true
        // A phone keeps the card shown and the one Back goes to, no other; but
        // the task's card, once made, as the Tasks page kept its panel (made
        // again, some 700 items, at every tap on a step: review of 590eb18, later 1).
        if (host.window.compact) {
            for (const other of ["event", "need", "dated"]) {
                if (other !== card && (host.previous === null || host.previous.card !== other))
                    made[other] = false
            }
        }
        host.made = made
        if (card === "task")
            taskLoader.make()
        host.opened++
        host.ref = host.copyOf(ref, card)
        if (card === "task")
            host.readChoices()
        return true
    }

    // Each opening counted: a card's late answer is for the one it was asked of.
    property int opened: 0

    // Its kind's own words for the head, once its card read them: Back's "‹ Doctor", the agenda's outline.
    // (After the read that asked it: never while a card's own bindings are being made;
    // and only for the card still open, never one opened since.)
    function named(itemId, title) {
        Qt.callLater(host.rename, host.opened, itemId, title)
    }

    function rename(opened, itemId, title) {
        if (!host.open || opened !== host.opened)
            return
        if ((itemId !== "" && host.ref.itemId !== itemId) || (title !== "" && host.ref.title !== title))
            host.ref = Object.assign({}, host.ref, itemId !== "" ? { itemId: itemId } : {}, title !== "" ? { title: title } : {})
    }

    // Back: to the card it came from, else closed.
    function back() {
        if (host.previous !== null) {
            host.leaveTask()
            host.opened++
            host.ref = host.previous
            host.previous = null
        } else {
            host.close()
        }
    }

    function close() {
        host.leaveTask()
        const was = host.kind
        host.opened++
        host.ref = null
        host.previous = null
        // A phone keeps what is shown only: the card made again at its next
        // opening; the task's card kept once made (`place`).
        if (host.window.compact && was !== "" && was !== "task") {
            const made = Object.assign({}, host.made)
            made[was] = false
            host.made = made
        }
    }

    // The page changes under the card, or a place opens (a message, a note, a
    // contact, a project): on a phone, where the card covers the page, it
    // closes, so that what opens shows; on a computer it stays beside the page
    // (review of 590eb18, M3).
    function leave() {
        if (host.window.compact && host.open)
            host.close()
    }

    // The task's card left (another card, another task, Back, closed): a new
    // task's form with a title typed makes the task first; a task's field being
    // typed in is saved first.
    function leaveTask() {
        if (host.ref === null || host.ref.card !== "task" || !host.taskPanel)
            return
        if (host.ref.making)
            host.taskPanel.leave()
        else
            host.taskPanel.commit()
    }

    // A new task's full form, its title first; from `source` (a card's Add ▾ ▸ A task), its tie given.
    function startNewTask(source) {
        host.place({ kind: "task", making: true }, false)
        const problem = host.taskPanel.startNew(source || null)
        if (problem !== "") {
            host.close()
            host.sioul.status = problem
        }
        return problem
    }

    // The task card shows the task the reference names (set, not bound: the
    // card goes to another task by itself, its bigger task, a step).
    // (By the reference itself: `kind`, bound to it, may not have followed yet.)
    function taskFollows() {
        if (host.ref !== null && host.ref.card === "task" && host.taskPanel && !host.ref.making && host.taskPanel.uid !== host.ref.owner)
            host.taskPanel.uid = host.ref.owner
    }

    onRefChanged: host.taskFollows()

    // An appointment's menu (a right click, a long press), from any view.
    function eventMenu(ref) {
        eventMenuLater.now().ask(host.copyOf(ref, "event"))
    }

    // A meal's, a nap's or the night's menu, from any view: that day's change, or its form in its card.
    function needMenu(ref) {
        const read = JSON.parse(host.sioul.needItem(ref.srcDate || ref.date || "", ref.srcKey || "") || "{}")
        if (!read.item)
            return
        const menu = needMenuLater.now()
        menu.ref = host.copyOf(ref, "need")
        menu.later = read.later || 15
        const alarmOnly = read.past && read.item.kind === "sleep" && read.item.alarm_open === true && (read.item.alarm || "") !== ""
        if ((read.past || read.item.past) && !alarmOnly)
            return
        menu.show(read, read.item, null, alarmOnly)
    }

    function readChoices() {
        const read = JSON.parse(host.sioul.taskCardChoices() || "{}")
        host.choices = { projects: read.projects || [], kinds: read.kinds || [], categories: read.categories || [] }
        if (host.taskPanel) {
            host.taskPanel.projects = host.choices.projects
            host.taskPanel.kinds = host.choices.kinds
            host.taskPanel.categories = host.choices.categories
        }
    }

    Connections {
        target: host.sioul

        function onTasksChanged() {
            if (host.kind === "task")
                host.readChoices()
        }
    }

    // The card's ground: the window's colour, a line on its left where it meets the page.
    Rectangle {
        anchors.fill: parent
        color: host.theme.background
    }
    Rectangle {
        visible: !host.window.compact
        width: 1
        height: parent.height
        color: host.theme.line
    }

    // Where the card stands: a step's place in the plan, a block's pin; before its task.
    Label {
        id: placeLine

        objectName: "cardPlace"
        readonly property string day: host.open && host.ref.at > 0 ? new Date(host.ref.at * 1000).toLocaleDateString(Qt.locale(host.sioul.text("qt-locale")), "dddd d MMMM") + ", " + host.ref.when : ""

        visible: host.kind === "task" && placeLine.text !== ""
        x: host.theme.gap
        y: host.theme.gap
        width: parent.width - 2 * host.theme.gap
        text: !host.open || placeLine.day === "" ? "" : host.ref.kind === "block" || host.ref.certainty === "held" ? host.sioul.textWith("card-pinned", "when", placeLine.day) : host.ref.kind === "step" ? host.sioul.textWith("card-plan-places", "when", placeLine.day) : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: host.theme.muted
    }

    Loader {
        id: eventLoader

        anchors.fill: parent
        anchors.margins: host.window.compact ? 0 : host.theme.gap
        active: host.made.event === true
        visible: host.kind === "event"

        sourceComponent: Component {
            EventCard {
                sioul: host.sioul
                theme: host.theme
                window: host.window
                host: host
                key: host.kind === "event" ? host.ref.srcKey : ""
                start: host.kind === "event" ? host.ref.srcStart : 0
            }
        }
    }

    // The window's one task card (TaskPanel.qml), made the first time a task's card
    // opens; loaded by its file, so that the host itself never needs the window's
    // own types, which the task card imports (the QML tests load the host alone).
    Loader {
        id: taskLoader

        function make() {
            if (taskLoader.status === Loader.Null)
                taskLoader.setSource("TaskPanel.qml", { theme: host.theme, sioul: host.sioul, window: host.window, projects: host.choices.projects, kinds: host.choices.kinds, categories: host.choices.categories })
        }

        anchors.fill: parent
        anchors.topMargin: placeLine.visible ? placeLine.height + 2 * host.theme.gap : 0
        visible: host.kind === "task"
        onLoaded: {
            const panel = taskLoader.item
            panel.closed.connect(host.close)
            panel.focusRequested.connect((uid, minutes) => host.sioul.focusStart(uid, minutes))
            // The form's title given: the task made goes on in its card, unless it was being closed.
            panel.made.connect(uid => {
                if (host.ref !== null && host.ref.card === "task" && host.ref.making)
                    host.ref = Object.assign({}, host.ref, { owner: uid, making: false })
            })
            // Gone to another task by itself (its bigger task, a step, what it waits for): the reference follows.
            panel.uidChanged.connect(() => {
                if (host.ref !== null && host.ref.card === "task" && panel.uid !== "" && panel.uid !== host.ref.owner)
                    host.ref = Object.assign({}, host.ref, { owner: panel.uid, title: "", kind: "task", at: 0 })
            })
            host.taskFollows()
        }
    }

    Later {
        id: eventMenuLater

        sourceComponent: Component {
            EventMenu {
                sioul: host.sioul
                window: host.window
                host: host
            }
        }
    }

    Later {
        id: needMenuLater

        sourceComponent: Component {
            NeedMenu {
                id: needMenuForm

                property var ref: null

                sioul: host.sioul
                theme: host.theme
                onActed: action => {
                    const problem = host.sioul.changeNeed(JSON.stringify({ date: needMenuForm.day.date, key: needMenuForm.item.key, action: action }))
                    if (problem !== "")
                        host.sioul.status = problem
                }
                onFormAsked: mode => {
                    host.show(needMenuForm.ref)
                    if (host.needCard)
                        host.needCard.edit(mode)
                }
            }
        }
    }

    Loader {
        id: datedLoader

        anchors.fill: parent
        anchors.margins: host.window.compact ? 0 : host.theme.gap
        active: host.made.dated === true
        visible: host.kind === "dated"

        sourceComponent: Component {
            DatedCard {
                sioul: host.sioul
                theme: host.theme
                window: host.window
                host: host
                kind: host.kind === "dated" ? host.ref.datedKind : ""
                key: host.kind === "dated" ? host.ref.srcKey : ""
            }
        }
    }

    Loader {
        id: needLoader

        anchors.fill: parent
        anchors.margins: host.window.compact ? 0 : host.theme.gap
        active: host.made.need === true
        visible: host.kind === "need"

        sourceComponent: Component {
            NeedCard {
                sioul: host.sioul
                theme: host.theme
                window: host.window
                host: host
                date: host.kind === "need" ? host.ref.srcDate || host.ref.date : ""
                key: host.kind === "need" ? host.ref.srcKey : ""
            }
        }
    }
}
