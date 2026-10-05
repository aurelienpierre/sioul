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
    }

    Connections {
        target: page.sioul

        function onAgendaChanged() {
            page.takeShown()
        }
    }
    onVisibleChanged: page.takeShown()
    // "agenda", "day", "week", "month".
    property string mode: "agenda"
    // A narrow screen (a phone): the title under the arrows, events on two lines.
    readonly property bool narrow: page.width < 600
    readonly property var modes: ["agenda", "day", "week", "month"]
    property var opened: null
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

    // Read again every five minutes while shown, and when the app comes back:
    // events over leave the list, what another device or the server changed comes in.
    Timer {
        interval: 5 * 60000
        running: page.visible && !page.sioul.away
        repeat: true
        onRunningChanged: if (running) triggered()
        onTriggered: {
            if (page.followsToday && page.iso(page.anchor) !== page.iso(new Date()))
                page.anchor = new Date()
            page.load()
        }
    }

    function load() {
        page.followsToday = page.iso(page.anchor) === page.iso(new Date())
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
        if (page.mode === "agenda")
            return page.iso(page.anchor) === page.iso(new Date()) ? page.sioul.text("agenda-next") : page.sioul.textWith("agenda-from-day", "day", page.anchor.toLocaleDateString(page.locale, "dddd d MMMM"))
        if (page.mode === "day") {
            // "Today, Saturday 3 October"; another day by its date alone.
            const named = page.anchor.toLocaleDateString(page.locale, "dddd d MMMM")
            const tomorrow = new Date()
            tomorrow.setDate(tomorrow.getDate() + 1)
            if (page.iso(page.anchor) === page.iso(new Date()))
                return page.sioul.text("agenda-today") + ", " + named
            if (page.iso(page.anchor) === page.iso(tomorrow))
                return page.sioul.text("agenda-tomorrow") + ", " + named
            return named
        }
        if (page.mode === "week")
            return page.sioul.textWith("agenda-week-of", "day", page.startOf(page.anchor, "week").toLocaleDateString(page.locale, "d MMMM"))
        return page.anchor.toLocaleDateString(page.locale, "MMMM yyyy")
    }

    function newEvent(day) {
        eventDialog.edit("", day || page.iso(page.anchor))
    }

    // For the window's tests.
    function fillEvent(text, from, to, how) {
        eventDialog.fill(text, from, to, how)
    }

    function saveEvent() {
        eventDialog.accept()
    }

    // A new event made from something else: a message, a task.
    function makeFrom(text, note, link) {
        eventDialog.makeFrom(text, note, link, page.iso(new Date()))
    }

    // An event by its file, from a link: shown when it is in the days shown, else its form.
    function openEvent(key) {
        for (const day of page.shown.days)
            for (const event of day.events)
                if (event.key === key) {
                    page.opened = event
                    return
                }
        eventDialog.edit(key, "")
    }

    onModeChanged: page.load()
    Component.onCompleted: {
        page.takeShown()
        page.load()
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
                        page.anchor = new Date()
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
                        color: day.modelData.date < page.iso(new Date()) ? page.theme.muted : page.theme.text
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
                                eventMenu.target = modelData
                                eventMenu.popup()
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
                opened: page.opened
                onOpen: event => page.opened = event
                onMenu: event => {
                    eventMenu.target = event
                    eventMenu.popup()
                }
                onNewAt: (day, hour) => eventDialog.edit("", day, hour)
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

                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            Layout.preferredWidth: 1
                            Layout.preferredHeight: 1
                            color: cell.modelData.today ? page.theme.surface : "transparent"
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
                                    font.weight: cell.modelData.today ? Font.Bold : Font.Normal
                                    color: cell.modelData.today ? page.theme.accent : page.theme.text
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
                        onClicked: eventDialog.edit(page.opened.key, "")
                    }
                    Button {
                        visible: page.opened !== null && !page.opened.read_only
                        flat: true
                        text: page.sioul.text("ui-delete")
                        onClicked: {
                            eventMenu.target = page.opened
                            deleteChoice.open()
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
    SioulMenu {
        id: eventMenu

        property var target: null
        readonly property var source: eventMenu.target && eventMenu.target.uid ? { uri: "sioul:event/" + encodeURIComponent(eventMenu.target.uid), kind: "event", key: eventMenu.target.key, title: eventMenu.target.summary, start: eventMenu.target.start } : null

        MenuItem {
            enabled: eventMenu.target !== null && !eventMenu.target.read_only
            text: page.sioul.text("ui-edit")
            onTriggered: eventDialog.edit(eventMenu.target.key, "")
        }
        MenuItem {
            enabled: eventMenu.target !== null && !eventMenu.target.read_only
            text: page.sioul.text("ui-delete")
            onTriggered: deleteChoice.open()
        }
        MenuSeparator {}
        AddMenu {
            sioul: page.sioul
            window: page.window
            source: eventMenu.source
        }
        MenuItem {
            enabled: eventMenu.source !== null
            text: page.sioul.text("ui-link-existing")
            onTriggered: page.window.linkFrom(eventMenu.source)
        }
    }

    // A repeating event: this time, or every time. A single one: no question.
    Dialog {
        id: deleteChoice

        anchors.centerIn: parent
        modal: true
        width: Math.min(440, page.width - 2 * page.theme.gap)
        title: eventMenu.target ? page.theme.plain(eventMenu.target.summary) : ""
        onAboutToShow: {
            if (eventMenu.target && !eventMenu.target.recurring) {
                page.sioul.deleteEvent(eventMenu.target.key, eventMenu.target.start, false)
                page.opened = null
                Qt.callLater(() => deleteChoice.close())
            }
        }

        ColumnLayout {
            width: parent.width
            spacing: 8

            Button {
                Layout.fillWidth: true
                text: page.sioul.text("agenda-delete-this")
                onClicked: {
                    page.sioul.deleteEvent(eventMenu.target.key, eventMenu.target.start, true)
                    page.opened = null
                    deleteChoice.close()
                }
            }
            Button {
                Layout.fillWidth: true
                text: page.sioul.text("agenda-delete-all")
                onClicked: {
                    page.sioul.deleteEvent(eventMenu.target.key, eventMenu.target.start, false)
                    page.opened = null
                    deleteChoice.close()
                }
            }
            Button {
                Layout.alignment: Qt.AlignRight
                text: page.sioul.text("ui-cancel")
                onClicked: deleteChoice.close()
            }
        }
    }

    EventDialog {
        id: eventDialog

        sioul: page.sioul
        theme: page.theme
        onSaved: page.opened = null
    }
}
