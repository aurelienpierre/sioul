// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The agenda: what comes, from today, as a calm list of days; the week and
// the month one switch away. Nothing is late or overdue: the past is not
// shown unless you go back to it. An event opens on the right with its time
// and place; notes, repetition and guests are folded. Deleting waits ten
// seconds with "Undo"; a repeating event asks whether it is this time or all.

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

    function takeShown() {
        if (page.visible)
            page.agendaText = page.sioul.agenda
        // An event asked for by a link, once its days are shown: its details.
        if (page.wanted !== "" && page.showWanted())
            page.wanted = ""
    }

    // An event asked for by a link, outside the days shown: its days are read first.
    property string wanted: ""

    // Its details, when it is in the days shown.
    function showWanted() {
        for (const day of page.shown.days)
            for (const event of day.events)
                if (event.key === page.wanted) {
                    page.opened = event
                    return true
                }
        return false
    }

    // Its days never came: its form, as before.
    Timer {
        id: wantedLate

        interval: 3000
        onTriggered: {
            if (page.wanted === "")
                return
            const key = page.wanted
            page.wanted = ""
            eventDialog.now().edit(key, "")
        }
    }

    Connections {
        target: page.sioul

        function onAgendaChanged() {
            page.takeShown()
        }
    }
    onVisibleChanged: {
        page.takeShown()
        if (page.visible)
            page.readAgain()
    }
    // "agenda", "day", "week", "month".
    property string mode: "agenda"
    // A narrow screen (a phone): the title under the arrows, events on two lines.
    readonly property bool narrow: page.width < 600
    readonly property var modes: ["agenda", "day", "week", "month"]
    property var opened: null
    // When the open event is reminded, and its own choice (docs/reminders.md): {line, remind, writable}.
    property var reminder: ({})
    readonly property var reminds: page.reminder.writable ? JSON.parse(page.sioul.reminderChoices(page.reminder.remind || "") || "[]") : []
    function readReminder() {
        page.reminder = page.opened ? JSON.parse(page.sioul.eventReminder(page.opened.key, page.opened.start) || "{}") : ({})
    }
    onOpenedChanged: page.readReminder()
    // The event the menu or the deletion asks about.
    property var menuTarget: null
    // On a phone, the event open takes the page; Back closes it (main.qml).
    readonly property bool canGoBack: page.opened !== null
    function back() {
        page.opened = null
    }
    property bool moreShown: false
    // The open event, as what new things are tied to.
    readonly property var source: page.opened && page.opened.uid ? { uri: "sioul:event/" + encodeURIComponent(page.opened.uid), kind: "event", key: page.opened.key, title: page.opened.summary, start: page.opened.start } : null

    function today() {
        return new Date()
    }

    function iso(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
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

        function onNowChanged() {
            if (page.visible && (page.window.today !== page.readOn || Math.floor(page.window.now / 300) !== Math.floor(page.readAt / 300)))
                page.readAgain()
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

    // For the window's tests.
    function fillEvent(text, from, to, how) {
        eventDialog.now().fill(text, from, to, how)
    }

    function saveEvent() {
        eventDialog.now().accept()
    }

    // A new event made from something else: a message, a task.
    function makeFrom(text, note, link) {
        eventDialog.now().makeFrom(text, note, link, page.iso(new Date()))
    }

    // An event by its file, from a link: its details, as from the agenda;
    // outside the days shown, its days are shown first. Its form ("Edit")
    // only when it cannot be found.
    function openEvent(key) {
        page.wanted = key
        if (page.showWanted()) {
            page.wanted = ""
            return
        }
        const found = JSON.parse(page.sioul.event(key) || "null")
        if (!found || !found.edit || !found.edit.start) {
            page.wanted = ""
            eventDialog.now().edit(key, "")
            return
        }
        page.anchor = new Date(found.edit.start.slice(0, 10) + "T12:00:00")
        page.load()
        wantedLate.restart()
    }

    onModeChanged: page.load()
    Component.onCompleted: {
        page.takeShown()
        page.readAgain()
    }

    Shortcut {
        sequence: "Escape"
        enabled: page.visible && page.opened !== null
        onActivated: page.opened = null
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        ColumnLayout {
            visible: !(page.window.compact && page.opened !== null)
            Layout.fillHeight: true
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: page.opened === null ? columns.width : Math.round((columns.width - columns.spacing) * 0.6)
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
                                return page.sioul.textArgs("overlap-day", JSON.stringify({ day: day, first: one(clash.modelData.first), second: one(clash.modelData.second) }))
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
                    text: page.title()
                    textFormat: Text.PlainText
                    font.pixelSize: 19
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                // The day, the week and the month, one switch away; on a phone,
                // in the title's room.
                ComboBox {
                    Layout.fillWidth: page.narrow
                    Layout.preferredWidth: page.narrow ? -1 : 150
                    model: page.modes.map(m => page.sioul.text("agenda-mode-" + m))
                    currentIndex: page.modes.indexOf(page.mode)
                    onActivated: index => page.mode = page.modes[index]
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
                SettingsButton {
                    sioul: page.sioul
                    theme: page.theme
                    view: "agenda"
                }
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
                            selected: page.opened !== null && page.opened.key === modelData.key && page.opened.start === modelData.start
                            onOpen: page.opened = modelData
                            onMenu: {
                                page.menuTarget = modelData
                                eventMenu.now().popup()
                            }
                        }
                    }
                }
            }

            // The week as a planning: the hours down, each event as long as it lasts.
            WeekPlanning {
                visible: page.mode === "week" || page.mode === "day"
                Layout.fillWidth: true
                Layout.fillHeight: true
                sioul: page.sioul
                theme: page.theme
                days: page.mode === "week" || page.mode === "day" ? page.shown.days : []
                locale: page.locale
                // The window's clock: the line at now and today's column follow it.
                now: page.window.now
                today: page.window.today
                opened: page.opened
                onOpen: event => page.opened = event
                onMenu: event => {
                    page.menuTarget = event
                    eventMenu.now().popup()
                }
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

                                    delegate: Label {
                                        id: monthEvent

                                        required property var modelData

                                        Layout.fillWidth: true
                                        text: monthEvent.modelData.summary
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        font.pixelSize: 12
                                        font.strikeout: monthEvent.modelData.cancelled
                                        color: page.theme.text

                                        TapHandler {
                                            onTapped: page.opened = monthEvent.modelData
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

        // The event, read.
        Panel {
            visible: page.opened !== null
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.4)
            theme: page.theme

            ColumnLayout {
                anchors.fill: parent
                spacing: 10

                Label {
                    Layout.fillWidth: true
                    text: page.opened ? page.opened.summary : ""
                    textFormat: Text.PlainText
                    font.pixelSize: 20
                    font.strikeout: page.opened !== null && page.opened.cancelled
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Label {
                    Layout.fillWidth: true
                    text: page.opened ? new Date(page.opened.start * 1000).toLocaleDateString(page.locale, "dddd d MMMM") + " · " + page.opened.when : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Label {
                    visible: page.opened !== null && page.opened.location !== ""
                    Layout.fillWidth: true
                    text: page.opened ? page.opened.location : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                RowLayout {
                    spacing: 6

                    Rectangle {
                        Layout.preferredWidth: 10
                        Layout.preferredHeight: 10
                        radius: 5
                        color: page.opened && page.opened.color ? page.opened.color : page.theme.accent
                    }
                    Label {
                        text: page.opened ? page.opened.calendar + (page.opened.recurring ? "  ·  " + page.sioul.text("agenda-repeats") : "") : ""
                        textFormat: Text.PlainText
                        color: page.theme.muted
                    }
                }
                // When it is reminded; changed here for every time it comes.
                Label {
                    visible: (page.reminder.line || "") !== ""
                    Layout.fillWidth: true
                    text: page.reminder.line || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                ComboBox {
                    visible: page.reminds.length > 0
                    Layout.fillWidth: page.narrow
                    Layout.preferredWidth: page.narrow ? -1 : 260
                    textRole: "label"
                    model: page.reminds
                    currentIndex: Math.max(0, page.reminds.findIndex(c => c.value === (page.reminder.remind || "")))
                    Accessible.name: page.sioul.text("event-remind")
                    onActivated: index => {
                        const problem = page.sioul.setEventReminder(page.opened.key, page.reminds[index].value)
                        if (problem === "")
                            page.readReminder()
                    }
                }
                Button {
                    readonly property bool any: page.opened !== null && (page.opened.notes !== "" || page.opened.organizer !== "" || page.opened.attendees.length > 0)

                    visible: any
                    flat: true
                    text: (page.moreShown ? "▾  " : "▸  ") + page.sioul.text("ui-more-details")
                    onClicked: page.moreShown = !page.moreShown
                }
                Label {
                    visible: page.moreShown && page.opened !== null && page.opened.notes !== ""
                    Layout.fillWidth: true
                    text: page.opened ? page.opened.notes : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Label {
                    visible: page.moreShown && page.opened !== null && page.opened.organizer !== ""
                    Layout.fillWidth: true
                    text: page.opened ? page.sioul.textWith("agenda-organizer", "name", page.opened.organizer) : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                Repeater {
                    model: page.moreShown && page.opened ? page.opened.attendees : []

                    delegate: Label {
                        id: guest

                        required property var modelData

                        Layout.fillWidth: true
                        text: (guest.modelData.name || guest.modelData.address) + "  ·  " + page.sioul.text("answer-" + guest.modelData.answer)
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: page.theme.muted
                        font.pixelSize: 13
                    }
                }
                // What it is tied to, and what can be made from it.
                RelatedList {
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    sioul: page.sioul
                    theme: page.theme
                    title: page.sioul.text("related-title")
                    uri: page.source ? page.source.uri : ""
                    onOpenThing: item => page.window.openThing(item)
                }
                ThingActions {
                    visible: page.source !== null
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    source: page.source
                }
                Item {
                    Layout.fillHeight: true
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: page.theme.gap

                    Button {
                        visible: page.opened !== null && !page.opened.read_only
                        text: page.sioul.text("ui-edit")
                        onClicked: eventDialog.now().edit(page.opened.key, "")
                    }
                    Button {
                        visible: page.opened !== null && !page.opened.read_only
                        flat: true
                        text: page.sioul.text("ui-delete")
                        onClicked: {
                            page.menuTarget = page.opened
                            deleteChoice.now().open()
                        }
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    Button {
                        text: page.sioul.text("ui-close")
                        onClicked: page.opened = null
                    }
                }
            }
        }
    }

    // Right click on an event.
    Later {
        id: eventMenu

        sourceComponent: Component {
            SioulMenu {
                id: eventMenuForm

                readonly property var source: page.menuTarget && page.menuTarget.uid ? { uri: "sioul:event/" + encodeURIComponent(page.menuTarget.uid), kind: "event", key: page.menuTarget.key, title: page.menuTarget.summary, start: page.menuTarget.start } : null

                // Its details first, as a click gives them; then its form.
                MenuItem {
                    enabled: page.menuTarget !== null
                    text: page.sioul.text("ui-details")
                    onTriggered: page.opened = page.menuTarget
                }
                MenuItem {
                    enabled: page.menuTarget !== null && !page.menuTarget.read_only
                    text: page.sioul.text("ui-edit")
                    onTriggered: eventDialog.now().edit(page.menuTarget.key, "")
                }
                MenuItem {
                    enabled: page.menuTarget !== null && !page.menuTarget.read_only
                    text: page.sioul.text("ui-delete")
                    onTriggered: deleteChoice.now().open()
                }
                MenuSeparator {}
                AddMenu {
                    sioul: page.sioul
                    window: page.window
                    source: eventMenuForm.source
                }
                MenuItem {
                    enabled: eventMenuForm.source !== null
                    text: page.sioul.text("ui-link-existing")
                    onTriggered: page.window.linkFrom(eventMenuForm.source)
                }
            }
        }
    }

    // A repeating event: this time, or every time. A single one: no question.
    Later {
        id: deleteChoice

        sourceComponent: Component {
            Dialog {
                id: deleteChoiceForm

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, page.width - 2 * page.theme.gap)
                title: page.menuTarget ? page.theme.plain(page.menuTarget.summary) : ""
                onAboutToShow: {
                    if (page.menuTarget && !page.menuTarget.recurring) {
                        page.sioul.deleteEvent(page.menuTarget.key, page.menuTarget.start, false)
                        page.opened = null
                        Qt.callLater(() => deleteChoiceForm.close())
                    }
                }

                ColumnLayout {
                    width: parent.width
                    spacing: 8

                    Button {
                        Layout.fillWidth: true
                        text: page.sioul.text("agenda-delete-this")
                        onClicked: {
                            page.sioul.deleteEvent(page.menuTarget.key, page.menuTarget.start, true)
                            page.opened = null
                            deleteChoiceForm.close()
                        }
                    }
                    Button {
                        Layout.fillWidth: true
                        text: page.sioul.text("agenda-delete-all")
                        onClicked: {
                            page.sioul.deleteEvent(page.menuTarget.key, page.menuTarget.start, false)
                            page.opened = null
                            deleteChoiceForm.close()
                        }
                    }
                    Button {
                        Layout.alignment: Qt.AlignRight
                        text: page.sioul.text("ui-cancel")
                        onClicked: deleteChoiceForm.close()
                    }
                }
            }
        }
    }

    Later {
        id: eventDialog

        sourceComponent: Component {
            EventDialog {
                id: eventDialogForm

                sioul: page.sioul
                theme: page.theme
                onSaved: page.opened = null
            }
        }
    }
}
