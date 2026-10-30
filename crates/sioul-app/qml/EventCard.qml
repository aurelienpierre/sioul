// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An appointment's card, where you are (the unified agenda, part 3): read
// from its own file by its reference (`eventCard(key, start)`), whatever
// page opened it. Its time and the time kept around it, its place, its
// calendar and "repeats", its reminder, its notes with their list, its
// organizer and guests, what prepares it (part D's pairing), what it is tied
// to. Edit opens its form; Delete asks this time or every time, never for a
// meeting with others; "Make it a task" and back. A time block of a task
// known here says the task it is, and opens it. Gone, or its deletion
// waiting for "Undo": it says so, and offers nothing.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CardFrame {
    id: card

    required property var window
    // Its file and the time asked (Unix seconds); 0: the next time it comes.
    property string key: ""
    property real start: 0
    // What `eventCard` read: {id, key, uid, start, end, summary, when, before,
    // after, location, calendar, color, recurring, cancelled, tentative,
    // read_only, with_others, task, list, notes, organizer, attendees, why}.
    property var event: null
    readonly property bool there: card.event !== null && card.event.key !== undefined
    readonly property bool isTask: card.there && (card.event.task || "") !== ""
    readonly property bool narrow: card.width < 480
    // When it is reminded, and its own choice: {line, remind, writable}.
    property var reminder: ({})
    readonly property var reminds: card.reminder.writable ? JSON.parse(card.sioul.reminderChoices(card.reminder.remind || "") || "[]") : []
    property bool moreShown: false
    // The event being made a task, asked about first.
    property string converting: ""
    // The open event, as what new things are tied to.
    readonly property var source: card.there && card.event.uid ? { uri: "sioul:event/" + encodeURIComponent(card.event.uid), kind: "event", key: card.event.key, title: card.event.summary, start: card.event.start } : null

    function read() {
        if (card.key === "") {
            card.event = null
            return
        }
        let read = JSON.parse(card.sioul.eventCard(card.key, card.start) || "{}")
        // A single event moved since it opened: the same event, at its new time.
        if (read.key === undefined && card.event !== null && card.event.recurring === false)
            read = JSON.parse(card.sioul.eventCard(card.key, 0) || "{}")
        card.event = read.key === undefined ? null : read
        // The time read stays in `event.start`, which every action uses: `start`
        // is the host's binding to what was tapped, never assigned here (review
        // of 590eb18, M1).
        if (card.event !== null)
            card.host.named(card.event.id, card.event.summary)
        card.reminder = card.event ? JSON.parse(card.sioul.eventReminder(card.event.key, card.event.start) || "{}") : ({})
        pairing.read()
        part.read()
    }

    function readReminder() {
        card.reminder = card.event ? JSON.parse(card.sioul.eventReminder(card.event.key, card.event.start) || "{}") : ({})
    }

    onKeyChanged: {
        card.moreShown = false
        card.read()
    }
    onStartChanged: {
        if (card.event === null || card.event.start !== card.start)
            card.read()
    }
    Component.onCompleted: card.read()

    // Read again as the agenda changes (its list ticked, its time moved, its file gone).
    Connections {
        target: card.sioul

        function onAgendaChanged() {
            if (card.visible)
                card.read()
        }
        function onTasksChanged() {
            pairing.read()
            part.read()
        }
    }

    // A writer's answer, said in the status line when something stops it.
    function said(problem) {
        if (problem !== "")
            card.sioul.status = problem
    }

    // An event of your own that could be a task instead: not a task's time
    // already, not a meeting with others (others expect you there), not read only.
    readonly property bool canBeTask: card.there && !card.isTask && !card.event.read_only && !card.event.with_others

    // A task made an appointment (the event form, its block's time), or an appointment made a task.
    function changeKind() {
        if (card.isTask) {
            eventDialog.now().makeFromTask(card.event.task, card.event.key, card.event.start, card.event.end)
            return
        }
        // What it cannot keep as a task, asked first.
        const losses = card.sioul.eventToTaskLosses(card.event.key)
        if (losses !== "") {
            card.converting = card.event.key
            convertChoice.now().ask(card.event.summary, losses, card.sioul.text("agenda-make-task"))
            return
        }
        card.toTask(card.event.key)
    }

    function toTask(key) {
        const problem = card.sioul.eventToTask(key)
        if (problem === "")
            card.host.close()
        else
            card.sioul.status = problem
    }

    // Deleted: a repeating one asks this time or every time; a single one goes at once.
    function askDelete() {
        if (!card.event.recurring) {
            card.deleteNow(false)
            return
        }
        deleteChoice.now().ask(card.event.summary, "")
    }

    function deleteNow(onlyThis) {
        if (!card.there)
            return
        const problem = card.sioul.deleteEvent(card.event.key, card.event.start, onlyThis)
        card.said(problem)
        if (problem === "")
            card.host.close()
    }

    function edit() {
        if (card.there)
            eventDialog.now().edit(card.event.key, "")
    }

    title: card.there ? card.event.summary : card.sioul.text("card-gone")
    struck: card.there && card.event.cancelled
    iconName: card.isTask ? "view-task" : ""
    dot: card.there && card.event.color ? card.event.color : card.theme.accent
    when: card.there ? card.event.when : ""
    from: !card.there ? "" : (card.isTask ? card.sioul.textWith("agenda-task-of", "list", card.event.list) : card.event.calendar) + (card.event.recurring ? "  ·  " + card.sioul.text("agenda-repeats") : "")
    stateLine: card.there && (card.event.before > 0 || card.event.after > 0) ? card.sioul.textArgs("agenda-kept", JSON.stringify({ before: card.event.before, after: card.event.after })) : ""
    editable: card.there && !card.event.read_only
    why: card.there ? card.event.why || "" : ""
    onEditAsked: card.edit()

    Label {
        visible: card.there && card.event.location !== ""
        Layout.fillWidth: true
        text: card.there ? card.event.location : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: card.theme.text
    }
    // When it is reminded; changed here for every time it comes.
    Label {
        visible: (card.reminder.line || "") !== ""
        Layout.fillWidth: true
        text: card.reminder.line || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: card.theme.muted
    }
    PlainComboBox {
        visible: card.reminds.length > 0
        Layout.fillWidth: card.narrow
        Layout.preferredWidth: card.narrow ? -1 : 260
        textRole: "label"
        model: card.reminds.map(c => ({ value: c.value, label: card.theme.plain(c.label) }))
        currentIndex: Math.max(0, card.reminds.findIndex(c => c.value === (card.reminder.remind || "")))
        Accessible.name: card.sioul.text("event-remind")
        onActivated: index => {
            const problem = card.sioul.setEventReminder(card.event.key, card.reminds[index].value)
            if (problem === "")
                card.readReminder()
            else
                card.sioul.status = problem
        }
    }
    Button {
        readonly property bool any: card.there && ((card.event.notes || "") !== "" || (card.event.organizer || "") !== "" || (card.event.attendees || []).length > 0)

        visible: any
        flat: true
        text: (card.moreShown ? "▾  " : "▸  ") + card.sioul.text("ui-more-details")
        onClicked: card.moreShown = !card.moreShown
    }
    // Its notes, their Markdown drawn: an invitation's are its sender's words.
    Description {
        visible: card.moreShown && card.there && (card.event.notes || "") !== ""
        Layout.fillWidth: true
        sioul: card.sioul
        theme: card.theme
        window: card.window
        markdown: card.there ? card.event.notes || "" : ""
        // Its checklist ticked here: a task's that can be changed, an event's of
        // a calendar you write to, never a meeting with others' (its organizer's words).
        tickable: card.there && !card.event.read_only && (card.isTask || card.event.with_others !== true)
        // An item made a task of its own; a step, of a task's list.
        converts: tickable
        steps: card.isTask
        onConvert: (line, to) => card.said(card.isTask ? card.sioul.convertItem("task", card.event.task, line, to) : card.sioul.convertItem("event", card.event.key, line, to))
        onCheck: (line, state) => card.said(card.isTask ? card.sioul.checkItem("task", card.event.task, line, state) : card.sioul.checkItem("event", card.event.key, line, state))
    }
    Label {
        visible: card.moreShown && card.there && (card.event.organizer || "") !== ""
        Layout.fillWidth: true
        text: card.there ? card.sioul.textWith("agenda-organizer", "name", card.event.organizer || "") : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: card.theme.muted
    }
    Repeater {
        model: card.moreShown && card.there ? card.event.attendees || [] : []

        delegate: Label {
            id: guest

            required property var modelData

            Layout.fillWidth: true
            text: (guest.modelData.name || guest.modelData.address) + "  ·  " + card.sioul.text("answer-" + guest.modelData.answer)
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: card.theme.muted
            font.pixelSize: 13
        }
    }
    // What prepares it: its task, with its steps and list (opened by a tap); else steps to
    // add, or a task made from it to use as its steps (docs/tasks.md, "An appointment and its steps").
    ColumnLayout {
        id: pairing

        // {task, adoptable, can}, read again when the event or the tasks change.
        property var shown: null

        function read() {
            pairing.shown = card.there && !card.isTask ? JSON.parse(card.sioul.eventPairing(card.event.key) || "null") : null
        }

        visible: pairing.shown !== null
        Layout.fillWidth: true
        spacing: 4

        Label {
            objectName: "agendaPairing"
            visible: !!pairing.shown && !!pairing.shown.task
            Layout.fillWidth: true
            text: pairing.shown && pairing.shown.task ? card.sioul.textWith("pair-prepared-by", "title", pairing.shown.task.title) + (pairing.shown.task.next ? "  ·  " + pairing.shown.task.next : "") : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: card.theme.text

            TapHandler {
                onTapped: card.host.follow({ kind: "task", owner: pairing.shown.task.uid, title: pairing.shown.task.title })
            }
        }
        Button {
            objectName: "agendaAddSteps"
            visible: !!pairing.shown && pairing.shown.can
            flat: true
            text: "+  " + card.sioul.text("pair-add-steps")
            onClicked: {
                const answer = JSON.parse(card.sioul.stepsForEvent(card.event.key) || "{}")
                if (answer.error)
                    card.sioul.status = answer.error
                else
                    card.host.follow({ kind: "task", owner: answer.uid })
            }
        }
        Repeater {
            model: pairing.shown ? pairing.shown.adoptable : []

            delegate: Button {
                id: adopt

                required property var modelData

                flat: true
                text: card.theme.plain(card.sioul.textWith("pair-adopt", "title", adopt.modelData.title))
                onClicked: {
                    card.said(card.sioul.attachToEvent(adopt.modelData.uid, card.event.key))
                    pairing.read()
                }
            }
        }
    }
    // When to leave, Before, Getting there, Bring and Coming back (composed tasks'
    // part F, the unified agenda's parts 4 and 6): one next step named, the others
    // folded; the legs as they will be once moved with it, a doubt said plainly;
    // nothing asks to tick, and a step left open costs nothing. A leg's own card:
    // what it is to its appointment, and × to untie it.
    ColumnLayout {
        id: part

        // `appointmentView`: {leave_at, line, preparation, before, more, bring,
        // getting_there, coming_back, steps_after, also, leg_of}.
        property var shown: null
        readonly property var legOf: part.shown && part.shown.leg_of ? part.shown.leg_of : null
        // Never on an appointment that will not be (cancelled, declined), nor a task's block opened
        // as an event (review of ab7daa4, later 15).
        readonly property bool legsTaken: card.there && !card.isTask && part.legOf === null && card.event.recurring !== true && card.event.all_day !== true && card.event.cancelled !== true && card.event.declined !== true && (card.event.task || "") === ""

        function read() {
            part.shown = card.there && !card.isTask ? JSON.parse(card.sioul.appointmentView(card.event.key, card.event.start) || "{}") : null
        }

        // "13:10 – 13:35 Bus 24", its doubt said.
        function legText(leg) {
            return leg.when + "  " + (leg.unconfirmed ? card.sioul.textWith("legs-unconfirmed", "leg", leg.title) : leg.title)
        }

        // "Add a ride or a walk…": the event form, its times before the appointment,
        // on the time zone Sioul shows (`zoneShift`).
        function addLeg() {
            const two = n => (n < 10 ? "0" : "") + n
            const shown = card.event.start * 1000
            const at = new Date(shown + (typeof card.sioul.zoneShift === "function" ? card.sioul.zoneShift(card.event.start) : 0))
            const end = new Date(at.getTime() - 5 * 60000)
            const begin = new Date(end.getTime() - 25 * 60000)
            const clock = d => two(d.getHours()) + ":" + two(d.getMinutes())
            const day = d => d.getFullYear() + "-" + two(d.getMonth() + 1) + "-" + two(d.getDate())
            legDialog.now().makeLeg(card.event.key, card.event.summary, card.event.start, card.event.end, day(begin), clock(begin), clock(end))
        }

        visible: part.shown !== null && ((part.shown.set_aside || []).length > 0 || (part.shown.line || "") !== "" || (part.shown.before || []).length > 0 || (part.shown.bring || []).length > 0 || (part.shown.getting_there || []).length > 0 || (part.shown.coming_back || []).length > 0 || (part.shown.steps_after || []).length > 0 || (part.shown.also || []).length > 0 || part.legOf !== null || part.legsTaken)
        Layout.fillWidth: true
        spacing: 4

        Label {
            objectName: "eventCardLeave"
            visible: !!part.shown && (part.shown.line || "") !== ""
            Layout.fillWidth: true
            text: part.shown ? part.shown.line || "" : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: card.theme.text
        }
        // A leg: "Before Doctor (14:00) · moves with it", opened by a tap; its doubt; × unties it.
        // Made only on a leg's card.
        Loader {
            objectName: "eventCardLegPart"
            active: part.legOf !== null
            visible: active
            Layout.fillWidth: true

            sourceComponent: Component {
                ColumnLayout {
                    spacing: 4

                    RowLayout {
                        objectName: "eventCardLegOf"
                        visible: part.legOf !== null
                        Layout.fillWidth: true

                        Label {
                            Layout.fillWidth: true
                            text: part.legOf ? part.legOf.line : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: card.theme.accent

                            TapHandler {
                                cursorShape: Qt.PointingHandCursor
                                onTapped: card.host.follow({ kind: "event", srcKey: part.legOf.key, srcStart: part.legOf.start, title: part.legOf.title })
                            }
                        }
                        ToolButton {
                            objectName: "eventCardUntieLeg"
                            visible: card.editable
                            text: "×"
                            Accessible.name: card.sioul.text("leg-untie")
                            ToolTip.visible: hovered
                            ToolTip.text: card.sioul.text("leg-untie")
                            onClicked: {
                                card.said(card.sioul.untieLeg(card.event.key))
                                part.read()
                            }
                        }
                    }
                    Label {
                        visible: part.legOf !== null && part.legOf.unconfirmed === true
                        Layout.fillWidth: true
                        text: card.sioul.text("leg-unconfirmed")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: card.theme.muted
                    }

                }
            }
        }
        Label {
            visible: !!part.shown && (part.shown.before || []).length > 0
            Layout.topMargin: 4
            text: card.sioul.text("appointment-before")
            textFormat: Text.PlainText
            font.bold: true
            color: card.theme.text
        }
        Repeater {
            model: part.shown ? part.shown.before || [] : []

            delegate: Label {
                id: step

                required property var modelData

                Layout.fillWidth: true
                text: "·  " + step.modelData.title
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: card.theme.accent

                TapHandler {
                    cursorShape: Qt.PointingHandCursor
                    onTapped: card.host.follow({ kind: "task", owner: step.modelData.uid, title: step.modelData.title })
                }
            }
        }
        Label {
            visible: !!part.shown && (part.shown.more || 0) > 0
            text: card.sioul.textArgs("appointment-more", JSON.stringify({ n: part.shown ? part.shown.more || 0 : 0 }))
            textFormat: Text.PlainText
            color: card.theme.muted
        }
        // Getting there: the legs before it, in time order; a tap opens the leg. Made only when it has some.
        Loader {
            active: !!part.shown && (part.shown.getting_there || []).length > 0
            visible: active
            Layout.fillWidth: true

            sourceComponent: Component {
                ColumnLayout {
                    spacing: 4

                    Label {
                        visible: !!part.shown && (part.shown.getting_there || []).length > 0
                        Layout.topMargin: 4
                        text: card.sioul.text("appointment-getting-there")
                        textFormat: Text.PlainText
                        font.bold: true
                        color: card.theme.text
                    }
                    Repeater {
                        model: part.shown ? part.shown.getting_there || [] : []

                        delegate: Label {
                            id: going

                            required property var modelData

                            objectName: "eventCardLeg"
                            Layout.fillWidth: true
                            text: part.legText(going.modelData)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: card.theme.accent
                            font.italic: going.modelData.unconfirmed

                            TapHandler {
                                cursorShape: Qt.PointingHandCursor
                                onTapped: card.host.follow({ kind: "event", srcKey: going.modelData.key, srcStart: going.modelData.start, title: going.modelData.title })
                            }
                        }
                    }
                }
            }
        }
        Repeater {
            model: part.shown ? part.shown.bring || [] : []

            delegate: ColumnLayout {
                id: group

                required property var modelData

                Layout.fillWidth: true
                spacing: 2

                Label {
                    Layout.topMargin: 4
                    text: group.modelData.name !== "" ? group.modelData.name : card.sioul.text("appointment-bring")
                    textFormat: Text.PlainText
                    font.bold: true
                    color: card.theme.text
                }
                Repeater {
                    model: group.modelData.items

                    delegate: CheckBox {
                        id: bring

                        required property var modelData

                        objectName: "eventCardBring"
                        Layout.fillWidth: true
                        text: card.theme.plain(bring.modelData.text)
                        checked: bring.modelData.done
                        // Ticked in its task's notes, through its own writer.
                        onToggled: {
                            card.said(card.sioul.checkItem("task", bring.modelData.task, bring.modelData.line, bring.checked ? "done" : "open"))
                            part.read()
                        }
                    }
                }
                Label {
                    visible: group.modelData.more > 0
                    text: card.sioul.textArgs("appointment-more", JSON.stringify({ n: group.modelData.more }))
                    textFormat: Text.PlainText
                    color: card.theme.muted
                }
            }
        }
        // Coming back: the legs after it, then the steps that come after it; another task tied
        // to it. Made only when there is one of them.
        Loader {
            active: !!part.shown && ((part.shown.coming_back || []).length > 0 || (part.shown.steps_after || []).length > 0 || (part.shown.also || []).length > 0)
            visible: active
            Layout.fillWidth: true

            sourceComponent: Component {
                ColumnLayout {
                    spacing: 4

                    Label {
                        visible: !!part.shown && ((part.shown.coming_back || []).length > 0 || (part.shown.steps_after || []).length > 0)
                        Layout.topMargin: 4
                        text: card.sioul.text("appointment-coming-back")
                        textFormat: Text.PlainText
                        font.bold: true
                        color: card.theme.text
                    }
                    Repeater {
                        model: part.shown ? part.shown.coming_back || [] : []

                        delegate: Label {
                            id: back

                            required property var modelData

                            objectName: "eventCardLeg"
                            Layout.fillWidth: true
                            text: part.legText(back.modelData)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: card.theme.accent
                            font.italic: back.modelData.unconfirmed

                            TapHandler {
                                cursorShape: Qt.PointingHandCursor
                                onTapped: card.host.follow({ kind: "event", srcKey: back.modelData.key, srcStart: back.modelData.start, title: back.modelData.title })
                            }
                        }
                    }
                    Repeater {
                        model: part.shown ? part.shown.steps_after || [] : []

                        delegate: Label {
                            id: later

                            required property var modelData

                            Layout.fillWidth: true
                            text: "·  " + later.modelData.title
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: card.theme.accent

                            TapHandler {
                                cursorShape: Qt.PointingHandCursor
                                onTapped: card.host.follow({ kind: "task", owner: later.modelData.uid, title: later.modelData.title })
                            }
                        }
                    }
                    // Another open task tied to it besides its preparation (part E0.10): named, opened by a tap.
                    Repeater {
                        model: part.shown ? part.shown.also || [] : []

                        delegate: Label {
                            id: also

                            required property var modelData

                            Layout.fillWidth: true
                            text: card.sioul.textWith("appointment-also-tied", "title", also.modelData.title)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: card.theme.muted

                            TapHandler {
                                cursorShape: Qt.PointingHandCursor
                                onTapped: card.host.follow({ kind: "task", owner: also.modelData.uid, title: also.modelData.title })
                            }
                        }
                    }
                }
            }
        }
        // What it set aside by itself once over (its steps before it left open, its task): each
        // can come back, and is never set aside again by it. Made only when there is one.
        Loader {
            objectName: "eventCardSetAside"
            active: !!part.shown && (part.shown.set_aside || []).length > 0
            visible: active
            Layout.fillWidth: true

            sourceComponent: Component {
                ColumnLayout {
                    spacing: 4

                    Label {
                        Layout.topMargin: 4
                        text: card.sioul.text("appointment-set-aside")
                        textFormat: Text.PlainText
                        font.bold: true
                        color: card.theme.text
                    }
                    Repeater {
                        model: part.shown ? part.shown.set_aside || [] : []

                        delegate: RowLayout {
                            id: aside

                            required property var modelData

                            Layout.fillWidth: true

                            Label {
                                Layout.fillWidth: true
                                text: aside.modelData.title
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: card.theme.muted
                            }
                            Button {
                                objectName: "eventCardBringBack"
                                flat: true
                                text: card.sioul.text("appointment-bring-back")
                                onClicked: {
                                    const answer = JSON.parse(card.sioul.bringBack(aside.modelData.uid) || "{}")
                                    card.said(answer.error || "")
                                    part.read()
                                }
                            }
                        }
                    }
                }
            }
        }
        Button {
            objectName: "eventCardAddLeg"
            visible: part.legsTaken
            flat: true
            text: "+  " + card.sioul.text("appointment-add-leg")
            onClicked: part.addLeg()
        }
    }
    // What it is tied to, and what can be made from it.
    RelatedList {
        visible: card.source !== null
        Layout.fillWidth: true
        Layout.topMargin: 8
        sioul: card.sioul
        theme: card.theme
        title: card.sioul.text("related-title")
        uri: card.source ? card.source.uri : ""
        onOpenThing: item => card.window.openThing(item, true)
    }
    ThingActions {
        visible: card.source !== null
        sioul: card.sioul
        theme: card.theme
        window: card.window
        source: card.source
    }

    actions: [
        // Never a meeting with others: its server would cancel or decline it for them.
        Button {
            objectName: "eventCardDelete"
            visible: card.there && !card.event.read_only && card.event.with_others !== true
            flat: true
            text: card.sioul.text("ui-delete")
            onClicked: card.askDelete()
        },
        // Which it is, a task or an appointment, is yours to say.
        Button {
            objectName: "agenda-change-kind"
            visible: card.isTask || card.canBeTask
            flat: true
            text: card.isTask ? card.sioul.text("agenda-make-appointment") : card.sioul.text("agenda-make-task")
            onClicked: card.changeKind()
        },
        Button {
            objectName: "eventCardTask"
            visible: card.isTask
            flat: true
            text: card.sioul.text("agenda-open-task")
            onClicked: card.host.follow({ kind: "task", owner: card.event.task, title: card.event.summary })
        },
        // A meeting with others whose invitation is still in your mail: your answer, from here.
        Button {
            id: answerButton

            objectName: "eventCardAnswer"
            visible: card.invitation !== ""
            flat: true
            text: card.sioul.text("event-answer") + "…"
            onClicked: answerMenu.now().popup(answerButton, 0, answerButton.height)

            Later {
                id: answerMenu

                sourceComponent: Component {
                    SioulMenu {
                        MenuItem {
                            text: card.sioul.text("invite-accept")
                            onTriggered: card.sioul.answerInvitation(card.invitation, "accepted")
                        }
                        MenuItem {
                            text: card.sioul.text("invite-maybe")
                            onTriggered: card.sioul.answerInvitation(card.invitation, "tentative")
                        }
                        MenuItem {
                            text: card.sioul.text("invite-decline")
                            onTriggered: card.sioul.answerInvitation(card.invitation, "declined")
                        }
                    }
                }
            }
        }
    ]

    // The message that brought its invitation, while it is in your mail (`invitationOf`); "" for none.
    readonly property string invitation: card.there && card.event.with_others === true ? card.sioul.invitationOf(card.event.key) : ""

    // A repeating event deleted: this time, or every time.
    Later {
        id: deleteChoice

        sourceComponent: Component {
            WhichTimes {
                sioul: card.sioul
                theme: card.theme
                onChosen: onlyThis => card.deleteNow(onlyThis)
            }
        }
    }

    Later {
        id: convertChoice

        sourceComponent: Component {
            ConfirmDialog {
                sioul: card.sioul
                theme: card.theme
                onConfirmed: card.toTask(card.converting)
            }
        }
    }

    Later {
        id: eventDialog

        sourceComponent: Component {
            EventDialog {
                sioul: card.sioul
                theme: card.theme
                keeper: card.host.keeper
            }
        }
    }

    // "Add a ride or a walk…": the event form for a leg, its own (never the one an edit keeps).
    Later {
        id: legDialog

        sourceComponent: Component {
            EventDialog {
                sioul: card.sioul
                theme: card.theme
                onSaved: part.read()
            }
        }
    }
}
