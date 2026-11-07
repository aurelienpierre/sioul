// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An appointment's card, where you are (the unified agenda, part 3): read
// from its own file by its reference (`eventCard(key, start)`), whatever
// page opened it. Its time and the time kept around it, its place, with its
// map and the way there on a press (nothing is asked of a server before), its
// own status (chosen here when it is yours) and your answer to an invitation,
// its calendar and "repeats", its reminder, its notes with their list, the
// people it names and their answers, what prepares it (part D's pairing), what it is tied
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
    // Its map, once Map was pressed: {lat, lon} found, "asking", "missing", or "" not asked.
    property var point: null
    property string mapState: ""
    // "To draw the map, the address goes to OpenStreetMap's servers.", the first time only.
    property string mapSaid: ""

    // Map pressed: its point from its own GEO or the places kept, else asked of
    // OpenStreetMap's geocoder once (`locatePlace`); the map drawn when it is known.
    function showMap() {
        if (!card.there || card.event.location === "")
            return
        if (card.mapSaid === "" && card.sioul.firstTime("map"))
            card.mapSaid = card.sioul.text("map-said")
        card.readPoint(true)
    }

    // Its point as known now; `ask`: asked of the geocoder when it is not.
    function readPoint(ask) {
        const found = JSON.parse(card.sioul.eventPoint(card.event.geo || "", card.event.location) || "{}")
        if (found.lat !== undefined) {
            card.point = { lat: found.lat, lon: found.lon }
            card.mapState = "found"
        } else if (found.asked) {
            card.mapState = "missing"
        } else if (ask) {
            card.mapState = "asking"
            card.said(card.sioul.locatePlace(card.event.location))
        }
    }

    // The way there (Directions.qml, shared with a contact's addresses).
    readonly property Directions way: Directions {
        sioul: card.sioul
    }

    // The way there, for `os` (Qt.platform.os): {geo, page} (`Directions.uris`).
    function directionsOf(point, place, os) {
        return card.way.uris(point, place, os)
    }

    // Directions pressed: its point when known (its GEO, the places kept), else its words (`Directions.open`).
    function directions() {
        if (!card.there || card.event.location === "")
            return
        card.way.open(card.point || JSON.parse(card.sioul.eventPoint(card.event.geo || "", card.event.location) || "{}"), card.event.location)
    }

    // Its own status set ("confirmed", "tentative", "cancelled"), by its own writer.
    function setStatus(status) {
        if (!card.there || status === (card.status || "confirmed"))
            return
        card.said(card.sioul.setEventStatus(card.event.key, status))
        card.read()
    }

    // Its own status (`STATUS`), chosen here for an event you may write.
    readonly property var statuses: ["confirmed", "tentative", "cancelled"]
    readonly property string status: !card.there ? "" : card.event.status ? card.event.status : card.event.cancelled ? "cancelled" : card.event.tentative ? "tentative" : ""
    readonly property bool statusChosen: card.there && !card.event.read_only && card.event.with_others !== true && !card.isTask

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
        card.point = null
        card.mapState = ""
        card.mapSaid = ""
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
        // The geocoder answered: the map drawn, or "not found" said.
        function onPlacesChanged() {
            if (card.mapState === "asking")
                card.readPoint(false)
        }
    }

    // A writer's answer, said in the status line when something stops it.
    function said(problem) {
        if (problem !== "")
            card.sioul.status = problem
    }

    // Who takes a child's event (several agendas, part 3): "me", "other", "accept", "decline".
    function take(who) {
        card.said(card.sioul.takeChild(card.event.key, card.event.recurring ? card.event.start : 0, who))
        card.read()
    }

    // What you may answer of a child's event, by where its asking stands.
    function childAnswers(info) {
        if (!info)
            return []
        const other = info.other || ""
        switch (info.state) {
        case "nobody":
        case "reopened":
        case "declined":
            return [{ who: "me", label: card.sioul.text("holder-me"), proposed: info.proposal === "me" }].concat(other !== "" ? [{ who: "other", label: other, proposed: info.proposal === "other" }] : [])
        case "proposed":
            return [{ who: "accept", label: card.sioul.text("holder-take-it"), proposed: false }, { who: "decline", label: card.sioul.text("holder-not-this-time"), proposed: false }]
        case "yours":
            return (other !== "" ? [{ who: "other", label: card.sioul.textWith("holder-ask-other", "who", other), proposed: false }] : []).concat([{ who: "decline", label: card.sioul.text("holder-not-this-time"), proposed: false }])
        default:
            return [{ who: "me", label: card.sioul.text("holder-i-take-it"), proposed: false }]
        }
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

    // Deleted: a repeating one asks this time or every time; a single one goes at once;
    // a meeting you organize says first that every guest is told, and goes whole.
    function askDelete() {
        if (card.event.organized === true) {
            tellsDelete.now().ask(card.event.summary, card.sioul.text("invite-delete-tells"), card.sioul.text("ui-delete"))
            return
        }
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
    // Its own status (chosen here when it is yours), your answer to an invitation, its
    // place's map and the way there, on a press: nothing is asked of a server before.
    // Made only for an event with one of them to say.
    Loader {
        objectName: "eventCardPlace"
        active: card.there && (card.event.location !== "" || card.statusChosen || card.status === "tentative" || card.status === "cancelled" || (card.event.own_answer || "") !== "")
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            ColumnLayout {
                spacing: 4

                Flow {
                    Layout.fillWidth: true
                    spacing: 8

                    // Its status: a menu of three where it is yours, else said when it is not "confirmed".
                    Button {
                        id: statusButton

                        objectName: "eventCardStatus"
                        visible: card.statusChosen
                        flat: true
                        text: card.sioul.text("event-status-" + (card.status || "confirmed")) + "  ▾"
                        Accessible.name: card.sioul.text("event-status") + ": " + card.sioul.text("event-status-" + (card.status || "confirmed"))
                        onClicked: statusMenu.now().popup(statusButton, 0, statusButton.height)

                        Later {
                            id: statusMenu

                            sourceComponent: Component {
                                SioulMenu {
                                    MenuItem {
                                        objectName: "eventCardStatus-confirmed"
                                        text: card.sioul.text("event-status-confirmed")
                                        checkable: true
                                        checked: (card.status || "confirmed") === "confirmed"
                                        onTriggered: card.setStatus("confirmed")
                                    }
                                    MenuItem {
                                        objectName: "eventCardStatus-tentative"
                                        text: card.sioul.text("event-status-tentative")
                                        checkable: true
                                        checked: card.status === "tentative"
                                        onTriggered: card.setStatus("tentative")
                                    }
                                    MenuItem {
                                        objectName: "eventCardStatus-cancelled"
                                        text: card.sioul.text("event-status-cancelled")
                                        checkable: true
                                        checked: card.status === "cancelled"
                                        onTriggered: card.setStatus("cancelled")
                                    }
                                }
                            }
                        }
                    }
                    Label {
                        visible: !card.statusChosen && (card.status === "tentative" || card.status === "cancelled")
                        text: card.status !== "" ? card.sioul.text("event-status-" + card.status) : ""
                        textFormat: Text.PlainText
                        color: card.theme.muted
                    }
                    Button {
                        objectName: "eventCardMap"
                        visible: card.event.location !== "" && card.mapState === ""
                        flat: true
                        icon.name: "mark-location"
                        text: card.sioul.text("map-show")
                        onClicked: card.showMap()
                    }
                    Button {
                        objectName: "eventCardDirections"
                        visible: card.event.location !== ""
                        flat: true
                        icon.name: "go-next"
                        text: card.sioul.text("map-directions")
                        onClicked: card.directions()
                    }
                }
                Label {
                    objectName: "eventCardYourAnswer"
                    visible: (card.event.own_answer || "") !== ""
                    Layout.fillWidth: true
                    text: (card.event.own_answer || "") !== "" ? card.sioul.text("event-you-" + card.event.own_answer) : ""
                    textFormat: Text.PlainText
                    color: card.theme.muted
                }
                Label {
                    objectName: "eventCardMapSaid"
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: card.mapState === "asking" ? [card.mapSaid, card.sioul.text("map-finding")].filter(t => t !== "").join(" ") : card.mapState === "missing" ? card.sioul.text("map-not-found") : card.mapState === "found" ? card.mapSaid : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                    font.pixelSize: 13
                }
                // A small map of its point, its tiles from the tile server set (OpenStreetMap's).
                Loader {
                    objectName: "eventCardMapView"
                    active: card.mapState === "found" && card.point !== null
                    visible: active
                    Layout.fillWidth: true
                    Layout.preferredHeight: 200

                    sourceComponent: Component {
                        ContactMap {
                            theme: card.theme
                            tiles: typeof card.sioul.mapTiles === "function" ? card.sioul.mapTiles() : ""
                            still: true
                            zoomOne: 16
                            attribution: card.sioul.text("map-attribution")
                            pins: card.point ? [{ key: card.event.key, name: card.event.summary, address: card.event.location, lat: card.point.lat, lon: card.point.lon }] : []
                        }
                    }
                }
            }
        }
    }
    // You in two places (several agendas): one sentence each, said here and nowhere else.
    Repeater {
        model: card.there ? card.event.conflicts || [] : []

        delegate: Label {
            id: conflictLine

            required property string modelData

            objectName: "eventConflict"
            Layout.fillWidth: true
            text: conflictLine.modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: card.theme.text
        }
    }
    // A child's event (part 3): who takes it and since when, who brings the child back,
    // what Sioul proposes and why, your answers, its kind. Made only for a child's event.
    Loader {
        active: card.there && !!card.event.child
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            ColumnLayout {
                id: child

                readonly property var info: card.there && card.event.child ? card.event.child : null
                readonly property bool open: child.info !== null && ["nobody", "reopened", "declined"].indexOf(child.info.state) >= 0

                spacing: 4

                Label {
                    objectName: "eventHolder"
                    Layout.fillWidth: true
                    text: child.info ? child.info.line : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.text
                }
                Label {
                    visible: child.info !== null && child.info.back !== ""
                    Layout.fillWidth: true
                    text: child.info ? child.info.back : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
                Label {
                    visible: child.open && (child.info.why !== "" || (child.info.busy || "") !== "")
                    Layout.fillWidth: true
                    text: child.info ? [child.info.why, child.info.busy || ""].filter(w => w !== "").join(" ") : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
                Flow {
                    visible: child.info !== null && child.info.writable === true
                    Layout.fillWidth: true
                    spacing: 8

                    Repeater {
                        model: card.childAnswers(child.info)

                        delegate: Button {
                            id: answer

                            required property var modelData

                            objectName: "eventHolderAnswer"
                            text: card.theme.plain(answer.modelData.label)
                            highlighted: answer.modelData.proposed === true
                            onClicked: card.take(answer.modelData.who)
                        }
                    }
                }
                // Who brings the child back, when another than who takes it (PART=BACK).
                PlainComboBox {
                    readonly property var choices: ["none", "me", "other"]

                    objectName: "eventChildBack"
                    visible: child.info !== null && child.info.writable === true && !child.open && (child.info.other || "") !== ""
                    Layout.fillWidth: card.narrow
                    Layout.preferredWidth: card.narrow ? -1 : 260
                    model: child.info ? [card.sioul.text("holder-back-same"), card.sioul.text("holder-me"), card.theme.plain(child.info.other || "")] : []
                    currentIndex: child.info ? Math.max(0, choices.indexOf(child.info.back_who || "none")) : 0
                    Accessible.name: child.info ? card.sioul.textWith("holder-back", "child", child.info.child || "") : ""
                    onActivated: index => card.take("back-" + choices[index])
                }
                PlainComboBox {
                    readonly property var choices: child.info ? [""].concat(child.info.kinds.map(k => k.id)) : [""]

                    objectName: "eventChildKind"
                    visible: child.info !== null && child.info.writable === true
                    Layout.fillWidth: card.narrow
                    Layout.preferredWidth: card.narrow ? -1 : 260
                    model: child.info ? [card.sioul.text("holder-kind-none")].concat(child.info.kinds.map(k => card.theme.plain(k.label))) : []
                    currentIndex: child.info ? Math.max(0, choices.indexOf(child.info.kind)) : 0
                    Accessible.name: card.sioul.text("holder-kind")
                    onActivated: index => {
                        card.said(card.sioul.setChildKind(card.event.key, choices[index]))
                        card.read()
                    }
                }
            }
        }
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
        readonly property bool any: card.there && (card.event.notes || "") !== ""

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
    // The people it names: its organizer first, then each guest with their answer, "optional"
    // when they are; by their contact's name when a card holds the address, which a tap opens.
    // Read only, on every event.
    Repeater {
        model: card.there ? card.event.people || [] : []

        delegate: Label {
            id: guest

            required property var modelData
            readonly property string role: guest.modelData.organizer ? card.sioul.text("event-organizer-role") : guest.modelData.optional ? card.sioul.text("event-optional") : ""
            readonly property string answer: guest.modelData.answer !== "" ? card.sioul.text("answer-" + guest.modelData.answer) : ""

            objectName: "eventCardPerson"
            Layout.fillWidth: true
            text: [guest.modelData.name, guest.role, guest.answer].filter(t => t !== "").join("  ·  ")
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: guest.modelData.contact !== "" ? card.theme.accent : card.theme.muted
            font.pixelSize: 13
            Accessible.role: guest.modelData.contact !== "" ? Accessible.Link : Accessible.StaticText
            Accessible.name: guest.text + (guest.modelData.name !== guest.modelData.address ? ", " + guest.modelData.address : "")

            TapHandler {
                enabled: guest.modelData.contact !== ""
                cursorShape: Qt.PointingHandCursor
                onTapped: card.window.openContact(guest.modelData.contact)
            }
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
                // A repeating event: for this time of it (`pairs::new_for`).
                const answer = JSON.parse(card.sioul.stepsForEvent(card.event.key, part.shown ? part.shown.rid || "" : "") || "{}")
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
                    card.said(card.sioul.attachToEvent(adopt.modelData.uid, card.event.key, part.shown ? part.shown.rid || "" : ""))
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
        // to_do_before, getting_there, coming_back, steps_after, also, leg_of}.
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
        // on the time zone Sioul shows (`zoneOffset`).
        function addLeg() {
            const two = n => (n < 10 ? "0" : "") + n
            const shown = card.event.start * 1000
            const at = new Date(shown + (typeof card.sioul.zoneOffset === "function" ? card.sioul.zoneOffset(card.event.start) + new Date(shown).getTimezoneOffset() * 60000 : 0))
            const end = new Date(at.getTime() - 5 * 60000)
            const begin = new Date(end.getTime() - 25 * 60000)
            const clock = d => two(d.getHours()) + ":" + two(d.getMinutes())
            const day = d => d.getFullYear() + "-" + two(d.getMonth() + 1) + "-" + two(d.getDate())
            legDialog.now().makeLeg(card.event.key, card.event.summary, card.event.start, card.event.end, day(begin), clock(begin), clock(end))
        }

        visible: part.shown !== null && ((part.shown.set_aside || []).length > 0 || (part.shown.line || "") !== "" || (part.shown.to_do_before || []).length > 0 || (part.shown.before || []).length > 0 || (part.shown.bring || []).length > 0 || (part.shown.getting_there || []).length > 0 || (part.shown.coming_back || []).length > 0 || (part.shown.steps_after || []).length > 0 || (part.shown.also || []).length > 0 || part.legOf !== null || part.legsTaken)
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
        // The tasks tied to this time of a repeating event, this time alone, a changed one too.
        Label {
            visible: !!part.shown && (part.shown.to_do_before || []).length > 0
            Layout.topMargin: 4
            text: card.sioul.text("appointment-to-do-before")
            textFormat: Text.PlainText
            font.bold: true
            color: card.theme.text
        }
        Repeater {
            model: part.shown ? part.shown.to_do_before || [] : []

            delegate: Label {
                id: toDo

                required property var modelData

                objectName: "eventCardToDoBefore"
                Layout.fillWidth: true
                text: "·  " + toDo.modelData.title
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: card.theme.accent

                TapHandler {
                    cursorShape: Qt.PointingHandCursor
                    onTapped: card.host.follow({ kind: "task", owner: toDo.modelData.uid, title: toDo.modelData.title })
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
        // Never a meeting someone else organizes: its server would cancel or decline it for them.
        Button {
            objectName: "eventCardDelete"
            visible: card.there && !card.event.read_only && (card.event.with_others !== true || card.event.organized === true)
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
        // The message that invited you, in Mail's reader through the window's one opener,
        // by the event's UID as the mail's index knows it (the unified mailbox's part 2).
        // Made only while its invitation is in your mail: one object on any other card.
        Loader {
            active: card.invitation !== ""
            visible: active

            sourceComponent: Component {
                Button {
                    objectName: "eventCardInvitation"
                    flat: true
                    text: card.sioul.text("event-its-invitation")
                    onClicked: card.window.openMail({ invitation: card.event.uid || "", organizer: card.event.organizer || "", file: card.invitation })
                }
            }
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

    // A meeting you organize deleted: "Deleting it tells every guest that it is cancelled." Made the first time.
    Later {
        id: tellsDelete

        sourceComponent: Component {
            ConfirmDialog {
                objectName: "eventCardDeleteTells"
                sioul: card.sioul
                theme: card.theme
                onConfirmed: card.deleteNow(false)
            }
        }
    }

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
