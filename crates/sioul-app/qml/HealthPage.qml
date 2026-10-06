// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Health: the day at a glance, or its week (docs/health.md, "The page").
// A day: its meals, naps, night and doses down the hours, and beside them
// (under them on a phone) the same as a list, the day's events and planned
// steps faded behind, for context. Each block is a tap away from what can
// change it, that day only: a quarter of an hour later, another time, other
// times, not that day, taken out; and one can be added for that day alone.
// The week: seven columns, the same blocks and the same taps, to fix ahead a
// day that will differ. Today's doses say whether they are marked taken,
// and the doubt when another device may know: never "not taken". The
// medicines and the prescriptions are the page's content, after the day:
// under its list, or in a column of their own on a wide window, which the
// week keeps too (MedicinesSection.qml); their forms open from there. What
// is set once (the usual meals and night, the watch, the pauses, where the
// errands go) is behind the ⚙. Nothing counts what was not done.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // The page's view (`health::page`): the week of the day shown, and what only today says.
    property var shown: ({ week: { monday: "", today: "", from_minute: 420, to_minute: 1380, days: [] }, missed: [], shared_note: "", reminded_there: "", watch: null, later: 15, any: true, medicines: [], prescriptions: [] })
    // The medicines and the prescriptions, taken from the view only when they
    // change: the view comes again each minute, and their rows stay as they are.
    property var medicines: []
    property var prescriptions: []
    // "day" or "week"; the week made the first time it is asked for.
    property string mode: "day"
    property bool weekMade: false
    // The day asked for ("2026-10-07"); the day shown is the view's, once it has it.
    property string wanted: page.iso(new Date())
    property var day: null
    // Shown from today: the page follows the days as they turn.
    property bool followsToday: true
    // A block or a dose under the pointer, or chosen ("date|key"): its row and its block light up together.
    property string hovered: ""
    property string chosen: ""
    // A row to bring into view, chosen in the timeline (its key).
    property string reveal: ""
    signal revealed
    // A block's menu open (the window's pictures take it).
    property bool menuOpen: false
    // The timeline opened readable by a long press, a block a finger can take
    // (HealthTimeline.qml): on a phone it takes the page, the list behind it,
    // until "Done" or Back.
    property bool readable: false
    readonly property bool canGoBack: page.readable
    function back() {
        page.readable = false
    }
    // For the window's pictures: opened readable, as a long press opens it.
    function showReadable(on) {
        page.readable = on
    }
    // Escape folds the readable timeline, as "Done" and Back do.
    Shortcut {
        sequence: "Escape"
        enabled: page.visible && page.readable
        onActivated: page.readable = false
    }
    // A phone, or a window as narrow: the list under the day, the header on two lines.
    readonly property bool narrow: page.width < 760
    // A window wide enough for a third column: the medicines and prescriptions
    // beside the day (and the week); else under the day's list.
    readonly property bool wide: page.width >= 1400
    readonly property int sideWidth: Math.round(Math.max(340, Math.min(440, page.width * 0.24)))
    readonly property var locale: Qt.locale(page.sioul.text("qt-locale"))
    readonly property bool onToday: page.mode === "week" ? page.shown.week.days.some(d => d.today) : page.day !== null && page.day.today

    function iso(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // Where the usual meals, naps and night are set: the settings, opened there
    // (the Porch's card, Settings ▸ Hours, the window's pictures).
    function showNeeds() {
        settings.now().showNeeds()
    }

    // The week, or the day, in view; the settings, for their picture (the window's pictures).
    function showWeek() {
        page.setMode("week")
    }

    function showDay() {
        page.setMode("day")
    }

    function settingsPanel() {
        return settings.now()
    }

    function closeSettings() {
        settings.close()
    }

    // The medicines and the prescriptions brought into view: under the day's
    // list, scrolled to; in their own column, already there (the window's pictures).
    signal medicinesWanted

    function showMedicines() {
        page.setMode("day")
        page.medicinesWanted()
    }

    // A block of the day shown: its menu, or its form ("move", "times"); what is open; all closed.
    function showMenu(key) {
        const item = page.day ? page.day.items.find(i => i.key === key) : null
        if (item)
            needMenu.now().show(page.day, item, null)
    }

    function showChange(key, mode) {
        const item = page.day ? page.day.items.find(i => i.key === key) : null
        if (item)
            dayForm.now().edit(mode, page.day, item)
    }

    function openPopup() {
        const opened = form => form.item && form.item.opened
        if (opened(medicineForm))
            return medicineForm.item
        if (opened(prescriptionForm))
            return prescriptionForm.item
        return page.menuOpen ? needMenu.item : dayForm.item
    }

    function closePopups() {
        needMenu.close()
        dayForm.close()
        medicineForm.close()
        prescriptionForm.close()
    }

    // A medicine or a prescription, new (null) or changed: its form, made the
    // first time. The prescriptions it can come with as they are now: the
    // view coming again each minute leaves an open form alone.
    function editMedicine(medicine) {
        medicineForm.now().edit(medicine, page.prescriptions)
    }

    function editPrescription(prescription) {
        prescriptionForm.now().edit(prescription)
    }

    // Fetched at the pharmacy today: the next visit is counted from today.
    function fetched(id) {
        const problem = page.sioul.refilled(id)
        if (problem !== "")
            page.sioul.status = problem
        page.reload()
    }

    // The week of the day asked for, made again off the window's thread (`healthView`).
    function reload() {
        page.sioul.showHealthWeek(page.wanted)
    }

    function takeView() {
        if (page.sioul.healthView === "")
            return
        page.shown = JSON.parse(page.sioul.healthView)
        const medicines = page.shown.medicines || []
        const prescriptions = page.shown.prescriptions || []
        if (JSON.stringify(medicines) !== JSON.stringify(page.medicines))
            page.medicines = medicines
        if (JSON.stringify(prescriptions) !== JSON.stringify(page.prescriptions))
            page.prescriptions = prescriptions
        page.pick()
    }

    // The day asked for, once the view has its week; until then the one shown stays.
    function pick() {
        const days = page.shown.week.days
        const found = days.find(d => d.date === page.wanted)
        if (found)
            page.day = found
        else if (page.day === null && days.length > 0)
            page.day = days.find(d => d.today) || days[0]
    }

    function goTo(date) {
        page.wanted = date
        page.followsToday = date === page.iso(new Date())
        page.chosen = ""
        page.pick()
        page.reload()
    }

    // Past days are as they were: a week back at most.
    function earliest() {
        const d = new Date()
        d.setHours(12, 0, 0, 0)
        d.setDate(d.getDate() - 7)
        return d
    }

    // A day, or a week, earlier or later.
    function move(steps) {
        const d = new Date(page.wanted + "T12:00:00")
        d.setDate(d.getDate() + steps * (page.mode === "week" ? 7 : 1))
        const first = page.earliest()
        page.goTo(page.iso(d < first ? first : d))
    }

    readonly property bool earlierAllowed: {
        const d = new Date(page.wanted + "T12:00:00")
        if (page.mode === "week")
            d.setDate(d.getDate() - (d.getDay() + 6) % 7)
        return d > page.earliest()
    }

    function setMode(mode) {
        if (mode === "week")
            page.weekMade = true
        page.mode = mode
    }

    function title() {
        if (page.mode === "week") {
            const monday = page.shown.week.monday !== "" ? new Date(page.shown.week.monday + "T12:00:00") : new Date()
            return page.sioul.textWith("agenda-week-of", "day", monday.toLocaleDateString(page.locale, "d MMMM"))
        }
        return page.day ? page.day.title : ""
    }

    // A day's block changed, that day only (`health::change_need`): the page and the plan follow.
    function change(date, key, action, more) {
        const problem = page.sioul.changeNeed(JSON.stringify(Object.assign({ date: date, key: key, action: action }, more || {})))
        if (problem !== "")
            page.sioul.status = problem
    }

    // A block's day and row: those of the week shown, else (a night ending
    // the week's first morning, the week before's) what the timeline knows.
    function rowOf(segment) {
        const day = page.shown.week.days.find(d => d.date === segment.date)
        const item = day ? day.items.find(i => i.key === segment.key) : null
        if (day && item)
            return { day: day, item: item }
        return { day: { date: segment.date, title: "", today: false, past: segment.past }, item: { key: segment.key, kind: segment.kind, name: segment.name, quiet: segment.quiet, off: false, changed: false, added: false, past: segment.past, from: "", to: "", at: "", before: 0, after: 0 } }
    }

    // What can change a block that day: under `under` (its ⋯), else where the
    // pointer or the finger is (window.menuAt). Nothing for a past one, nor a
    // dose; but a night begun on a past day and not over (after midnight), its
    // alarm at waking alone, which the core takes until it rings (`alarm_open`).
    function openMenu(day, item, under) {
        if (item.kind === "dose")
            return
        const alarmOnly = (day.past || item.past) && item.kind === "sleep" && item.alarm_open === true && (item.alarm || "") !== ""
        if ((day.past || item.past) && !alarmOnly)
            return
        needMenu.now().show(day, item, under || null, alarmOnly)
    }

    // A block or a dose tapped in the timeline: chosen, its row shown; in the
    // week, a block's menu at once, a dose's day opened.
    function tapped(segment, at) {
        page.chosen = segment.date + "|" + segment.key
        // The morning part of a night begun the day before is not in this day's
        // list (its row there is tonight's): its menu instead, as in the week.
        if (page.mode === "day" && (page.day === null || segment.date === page.day.date)) {
            page.reveal = segment.key
            page.revealed()
            return
        }
        if (segment.kind === "dose") {
            page.setMode("day")
            page.goTo(segment.date)
            return
        }
        const row = page.rowOf(segment)
        page.window.menuAt = at
        page.openMenu(row.day, row.item, null)
    }

    function held(segment, at) {
        const row = page.rowOf(segment)
        page.window.menuAt = at
        page.openMenu(row.day, row.item, null)
    }

    Connections {
        target: page.sioul

        function onHealthViewChanged() {
            page.takeView()
        }

        // A dose marked, a medicine or a day changed on another device: shown at once.
        function onSharedIn(stores) {
            if (page.visible && (stores.indexOf("state/health-state.toml") >= 0 || stores.indexOf("data/health.toml") >= 0 || stores.indexOf("data/health-days.toml") >= 0))
                page.reload()
        }
    }

    // Another day when the clock turns past midnight, if the page showed today.
    function follow() {
        if (page.followsToday && page.wanted !== page.iso(new Date()))
            page.goTo(page.iso(new Date()))
        else
            page.reload()
    }

    onVisibleChanged: {
        if (page.visible)
            page.follow()
        else
            page.readable = false
    }
    Component.onCompleted: {
        page.takeView()
        page.reload()
    }

    // Doses pass their time, the now line moves: made again each minute while open.
    Timer {
        interval: 60000
        running: page.visible && !page.sioul.away
        repeat: true
        onTriggered: page.follow()
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: 8

        // ‹ The day, or the week › … Day, Week; the settings. On a phone, on two lines.
        RowLayout {
            Layout.fillWidth: true
            spacing: 2

            ToolButton {
                text: "‹"
                font.pixelSize: 20
                enabled: page.earlierAllowed
                Accessible.name: page.sioul.text("agenda-earlier")
                onClicked: page.move(-1)
            }
            // As wide whatever the day: the arrows stay where they are.
            Label {
                Layout.fillWidth: page.narrow
                Layout.preferredWidth: page.narrow ? -1 : 300
                horizontalAlignment: Text.AlignHCenter
                text: page.title()
                textFormat: Text.PlainText
                font.pixelSize: 19
                elide: Text.ElideRight
                color: page.theme.text
            }
            ToolButton {
                text: "›"
                font.pixelSize: 20
                Accessible.name: page.sioul.text("agenda-later")
                onClicked: page.move(1)
            }
            Item {
                visible: !page.narrow
                Layout.fillWidth: true
            }
            // Back to today, from another day: nothing else moves when it shows.
            Button {
                visible: !page.narrow && !page.onToday
                flat: true
                text: page.sioul.text("agenda-today")
                onClicked: page.goTo(page.iso(new Date()))
            }
            ModeSwitch {
                visible: !page.narrow
            }
            SettingsGear {
                visible: !page.narrow
            }
        }
        RowLayout {
            visible: page.narrow
            Layout.fillWidth: true
            spacing: 4

            ModeSwitch {}
            Button {
                visible: !page.onToday
                flat: true
                text: page.sioul.text("agenda-today")
                onClicked: page.goTo(page.iso(new Date()))
            }
            Item {
                Layout.fillWidth: true
            }
            SettingsGear {}
        }

        // Doses due while Sioul ran on none of your computers: a question on the
        // past, answered once; never a reminder to take one now. Made only then.
        Loader {
            active: page.shown.missed.length > 0
            visible: active
            Layout.fillWidth: true
            sourceComponent: missedComponent
        }

        // The day or the week; on a wide window, the medicines and the prescriptions beside them.
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: page.theme.gap

            // The day: its timeline, and its list beside it (under it on a phone).
            Item {
                id: body

                visible: page.mode === "day"
                Layout.fillWidth: true
                Layout.fillHeight: true

                HealthTimeline {
                    id: dayLine

                    width: page.narrow ? body.width : Math.round((body.width - page.theme.gap) * 0.55)
                    height: page.narrow && !page.readable ? Math.round(body.height * 0.38) : body.height
                    sioul: page.sioul
                    theme: page.theme
                    days: page.day ? [page.day] : []
                    fromMinute: page.shown.week.from_minute
                    toMinute: page.shown.week.to_minute
                    lit: page.hovered
                    chosen: page.chosen
                    readable: page.readable
                    onHover: ident => page.hovered = ident
                    onTapped: (segment, at) => page.tapped(segment, at)
                    onMenu: (segment, at) => page.held(segment, at)
                    onReadableAsked: page.readable = true
                    onReadableDone: page.readable = false
                }
                // Made a moment after the first screen, without holding the window.
                Loader {
                    id: listLoader

                    // On a phone, behind the timeline opened readable.
                    visible: !(page.narrow && page.readable)
                    x: page.narrow ? 0 : dayLine.width + page.theme.gap
                    y: page.narrow ? dayLine.height + 8 : 0
                    width: body.width - x
                    height: body.height - y
                    asynchronous: true
                    sourceComponent: dayListComponent
                }
            }

            // The week: seven columns, the same blocks, the same taps. Made the first time.
            Loader {
                visible: page.mode === "week"
                active: page.weekMade
                Layout.fillWidth: true
                Layout.fillHeight: true

                sourceComponent: HealthTimeline {
                    sioul: page.sioul
                    theme: page.theme
                    titles: true
                    days: page.shown.week.days
                    fromMinute: page.shown.week.from_minute
                    toMinute: page.shown.week.to_minute
                    lit: page.hovered
                    chosen: page.chosen
                    readable: page.readable
                    onHover: ident => page.hovered = ident
                    onTapped: (segment, at) => page.tapped(segment, at)
                    onMenu: (segment, at) => page.held(segment, at)
                    onReadableAsked: page.readable = true
                    onReadableDone: page.readable = false
                    onOpenDay: date => {
                        page.setMode("day")
                        page.goTo(date)
                    }
                }
            }

            // Wide: their own column, its width kept from the start (nothing
            // moves when it comes), the same in the day and the week. Made
            // after the first screen, without holding the window.
            Loader {
                visible: page.wide
                active: page.wide
                asynchronous: true
                Layout.preferredWidth: page.sideWidth
                Layout.fillHeight: true

                sourceComponent: Component {
                    ScrollView {
                        id: side

                        contentWidth: availableWidth
                        clip: true

                        Medicines {
                            width: side.availableWidth
                        }
                    }
                }
            }
        }
    }

    // Day, Week.
    component ModeSwitch: Row {
        spacing: 4

        Repeater {
            model: ["day", "week"]

            delegate: Button {
                id: modeButton

                required property string modelData

                text: page.sioul.text("agenda-mode-" + modeButton.modelData)
                checkable: true
                checked: page.mode === modeButton.modelData
                flat: page.mode !== modeButton.modelData
                // A click on the way shown unticks it: its binding ticks it again.
                onClicked: {
                    page.setMode(modeButton.modelData)
                    modeButton.checked = Qt.binding(() => page.mode === modeButton.modelData)
                }
            }
        }
    }

    // The page's settings: what is set once (SettingsButton's look).
    component SettingsGear: ToolButton {
        icon.name: "settings-configure"
        icon.color: page.theme.text
        display: AbstractButton.IconOnly
        Accessible.name: page.sioul.text("ui-settings")
        ToolTip.visible: hovered
        ToolTip.text: page.sioul.text("ui-settings")
        ToolTip.delay: 300
        onClicked: settings.now().open()
    }

    // The medicines and the prescriptions, as the page lists them; their forms are the page's.
    component Medicines: MedicinesSection {
        sioul: page.sioul
        theme: page.theme
        medicines: page.medicines
        prescriptions: page.prescriptions
        onOpenMedicine: medicine => page.editMedicine(medicine)
        onOpenPrescription: prescription => page.editPrescription(prescription)
        onFetched: id => page.fetched(id)
    }

    // The day's list: its whole-day events and errands, then its meals, naps,
    // night and doses in time order, each with what changes it; then one to add.
    Component {
        id: dayListComponent

        ScrollView {
            id: scroll

            readonly property var day: page.day
            readonly property bool editable: scroll.day !== null && !scroll.day.past

            // A row brought into view, chosen in the timeline.
            function showRow(key) {
                const index = scroll.day ? scroll.day.items.findIndex(i => i.key === key) : -1
                const row = index >= 0 ? rows.itemAt(index) : null
                const flick = scroll.contentItem as Flickable
                if (!row || !flick)
                    return
                const top = row.y + listColumn.y
                if (top < flick.contentY || top + row.height > flick.contentY + flick.height)
                    flick.contentY = Math.max(0, Math.min(top - 8, flick.contentHeight - flick.height))
            }

            Connections {
                target: page

                function onRevealed() {
                    scroll.showRow(page.reveal)
                }

                function onMedicinesWanted() {
                    const flick = scroll.contentItem as Flickable
                    if (flick && medicinesHere.visible)
                        flick.contentY = Math.max(0, Math.min(medicinesHere.y + listColumn.y - 8, flick.contentHeight - flick.height))
                }
            }

            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                id: listColumn

                width: scroll.availableWidth
                spacing: 2

                // The widest time, measured in the rows' own type: the names line up.
                Label {
                    id: timeWidth

                    visible: false
                    text: "00:00–00:00"
                    font.features: { "tnum": 1 }
                }

                // Whole-day events; a refill or a renewal from that day on.
                Repeater {
                    model: scroll.day ? scroll.day.lines : []

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        Layout.bottomMargin: 4
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: page.theme.muted
                    }
                }
                // Nothing set yet: where to set it.
                Label {
                    visible: scroll.day !== null && scroll.day.items.length === 0
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    text: page.sioul.text(page.shown.any ? "health-day-nothing" : "health-day-empty")
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }

                Repeater {
                    id: rows

                    model: scroll.day ? scroll.day.items : []

                    delegate: Rectangle {
                        id: row

                        required property var modelData
                        readonly property string ident: scroll.day.date + "|" + row.modelData.key
                        readonly property bool lit: page.hovered === row.ident || page.chosen === row.ident
                        readonly property bool isDose: row.modelData.kind === "dose"
                        // A meal, a nap or the night not over yet, on a day not past.
                        readonly property bool editable: !row.isDose && !row.modelData.past && scroll.editable
                        readonly property color ink: row.modelData.off || (row.modelData.past && row.modelData.taken === "") ? page.theme.muted : page.theme.text

                        Layout.fillWidth: true
                        implicitHeight: line.implicitHeight + 10
                        radius: 4
                        color: row.lit ? page.theme.hover : "transparent"

                        HoverHandler {
                            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                            onHoveredChanged: {
                                if (hovered)
                                    page.hovered = row.ident
                                else if (page.hovered === row.ident)
                                    page.hovered = ""
                            }
                        }
                        TapHandler {
                            onTapped: page.chosen = row.ident
                        }
                        TapHandler {
                            acceptedButtons: Qt.RightButton
                            // A touch has no buttons: on a touch screen, the long press below.
                            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                            onTapped: eventPoint => {
                                page.window.menuAt = eventPoint.scenePosition
                                page.openMenu(scroll.day, row.modelData, null)
                            }
                        }
                        TapHandler {
                            id: rowHold

                            acceptedDevices: PointerDevice.TouchScreen
                            onLongPressed: {
                                page.window.menuAt = rowHold.point.scenePosition
                                page.openMenu(scroll.day, row.modelData, null)
                            }
                        }

                        // The first line: its time, its name, what changes it; under the
                        // name, how that day has it, and the doubt on a dose when there is one.
                        ColumnLayout {
                            id: line

                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.leftMargin: 6
                            anchors.rightMargin: 2
                            spacing: 0

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                // Kept from–to (getting it ready, winding down included); a dose, its time.
                                Label {
                                    Layout.preferredWidth: timeWidth.implicitWidth + 2
                                    text: row.isDose ? row.modelData.from : row.modelData.from + "–" + row.modelData.to
                                    textFormat: Text.PlainText
                                    font.features: { "tnum": 1 }
                                    color: row.ink
                                }
                                // What is left of the line, however long the name: never wider than the screen.
                                Label {
                                    Layout.fillWidth: true
                                    Layout.preferredWidth: 1
                                    text: row.modelData.name + (row.modelData.dose !== "" ? "  ·  " + row.modelData.dose : "")
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    color: row.ink
                                }
                                // Taken: when, said plainly; one click takes it back. More than
                                // half an hour late, when it was taken is asked (DoseTaken.qml).
                                Button {
                                    visible: row.isDose && scroll.day.today
                                    flat: row.modelData.taken !== ""
                                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                                    text: row.modelData.taken !== "" ? page.sioul.textWith("health-taken-at", "time", row.modelData.taken) : page.sioul.text(row.modelData.late ? "health-taken-when" : "health-taken")
                                    icon.name: row.modelData.taken !== "" ? "task-complete" : ""
                                    icon.color: page.theme.text
                                    onClicked: {
                                        if (row.modelData.taken === "" && row.modelData.late) {
                                            page.window.askDose(row.modelData.key)
                                            return
                                        }
                                        page.sioul.setDoseTaken(row.modelData.key, row.modelData.taken === "")
                                        page.reload()
                                    }
                                }
                                // The quickest change, at hand: a quarter of an hour later; put back when taken out.
                                Button {
                                    visible: row.editable
                                    flat: true
                                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                                    text: row.modelData.off ? page.sioul.text("need-put-back") : page.sioul.textWith("need-later-n", "minutes", String(page.shown.later))
                                    onClicked: page.change(scroll.day.date, row.modelData.key, row.modelData.off ? "on" : "later")
                                }
                                // On a past day, a night not over (after midnight): its alarm alone.
                                ToolButton {
                                    id: more

                                    visible: (row.editable || (row.modelData.kind === "sleep" && row.modelData.alarm_open === true)) && !row.modelData.off
                                    text: "⋯"
                                    Accessible.name: page.sioul.textWith("need-menu", "name", row.modelData.name)
                                    ToolTip.visible: hovered
                                    ToolTip.text: Accessible.name
                                    ToolTip.delay: 400
                                    onClicked: page.openMenu(scroll.day, row.modelData, more)
                                }
                            }
                            Label {
                                visible: text !== ""
                                Layout.fillWidth: true
                                Layout.leftMargin: timeWidth.implicitWidth + 12
                                Layout.preferredWidth: 1
                                text: [row.modelData.detail, row.modelData.note].filter(t => t !== "").join("  ·  ")
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                            // Not marked here, and not known whether it was taken on another device: said, never guessed.
                            Label {
                                visible: row.modelData.doubt !== ""
                                Layout.fillWidth: true
                                Layout.leftMargin: timeWidth.implicitWidth + 12
                                Layout.preferredWidth: 1
                                text: row.modelData.doubt
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: page.theme.warm
                            }
                        }
                    }
                }

                // One for this day only: a meal or a rest.
                Button {
                    visible: scroll.editable
                    Layout.topMargin: 4
                    flat: true
                    icon.name: "list-add"
                    icon.color: page.theme.text
                    text: page.sioul.text("need-add")
                    onClicked: dayForm.now().edit("add", scroll.day, null)
                }

                // How the day went, in the words said then; today in the evening, "Close the day" (DayReviewLine.qml, docs/reviews.md).
                DayReviewLine {
                    Layout.fillWidth: true
                    Layout.topMargin: 6
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    date: scroll.day !== null ? scroll.day.date : ""
                }

                // Today: what is not known of the doses marked elsewhere; where reminders come.
                Label {
                    readonly property string said: [page.shown.shared_note, page.shown.reminded_there].filter(t => t !== "").join(" ")

                    visible: scroll.day !== null && scroll.day.today && said !== ""
                    Layout.fillWidth: true
                    Layout.topMargin: 6
                    text: said
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: page.shown.shared_note !== "" ? page.theme.warm : page.theme.muted
                }

                // The medicines and the prescriptions, after the day (their own
                // column on a wide window); below the first screen of a phone.
                Loader {
                    id: medicinesHere

                    active: !page.wide
                    visible: active
                    Layout.fillWidth: true
                    Layout.topMargin: 14
                    asynchronous: true
                    sourceComponent: Component {
                        Medicines {}
                    }
                }

                // Today: what the watch says, once one is set up; made after the
                // rest, the medicines above it first, so that it does not move.
                Loader {
                    active: scroll.day !== null && scroll.day.today && page.shown.watch !== null && page.shown.watch.any && (page.wide || medicinesHere.status === Loader.Ready)
                    visible: active
                    Layout.fillWidth: true
                    asynchronous: true
                    sourceComponent: WatchPanel {
                        sioul: page.sioul
                        theme: page.theme
                        watch: page.shown.watch
                    }
                }
                Item {
                    Layout.preferredHeight: page.theme.gap
                }
            }
        }
    }

    // Doses due while Sioul ran nowhere: did you take them?
    Component {
        id: missedComponent

        Panel {
            theme: page.theme

            ColumnLayout {
                anchors.fill: parent
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("health-missed-question")
                    wrapMode: Text.Wrap
                    font.weight: Font.DemiBold
                    color: page.theme.text
                }
                Repeater {
                    model: page.shown.missed

                    delegate: ColumnLayout {
                        id: missed

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 2

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 12

                            Label {
                                Layout.preferredWidth: 90
                                text: missed.modelData.time
                                textFormat: Text.PlainText
                                font.features: { "tnum": 1 }
                                color: page.theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: missed.modelData.name + (missed.modelData.dose !== "" ? "  ·  " + missed.modelData.dose : "")
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            // When it was taken, asked (DoseTaken.qml).
                            Button {
                                text: page.sioul.text("health-taken-when")
                                onClicked: page.window.askDose(missed.modelData.key)
                            }
                            Button {
                                flat: true
                                text: page.sioul.text("health-not-taken")
                                onClicked: {
                                    page.sioul.doseNotTaken(missed.modelData.key)
                                    page.reload()
                                }
                            }
                        }
                        // Whether it was taken on another device is not known here: said, never guessed.
                        Label {
                            visible: missed.modelData.doubt !== ""
                            Layout.fillWidth: true
                            text: missed.modelData.doubt
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: page.theme.warm
                        }
                    }
                }
            }
        }
    }

    // What can change a block, that day only.
    Later {
        id: needMenu

        sourceComponent: Component {
            SioulMenu {
                id: menu

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
                    page.change(menu.day.date, menu.item.key, action)
                }

                onOpened: page.menuOpen = true
                onClosed: page.menuOpen = false

                MenuItem {
                    visible: !menu.off && !menu.alarmOnly
                    height: visible ? implicitHeight : 0
                    text: page.sioul.textWith("need-later-n", "minutes", String(page.shown.later))
                    onTriggered: menu.act("later")
                }
                MenuItem {
                    visible: !menu.off && !menu.alarmOnly
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("need-move-to") + "…"
                    onTriggered: dayForm.now().edit("move", menu.day, menu.item)
                }
                MenuItem {
                    visible: !menu.off && !menu.alarmOnly
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("need-change-times")
                    onTriggered: dayForm.now().edit("times", menu.day, menu.item)
                }
                // No notice that day, kept free all the same; or back.
                MenuItem {
                    visible: !menu.off && !menu.alarmOnly
                    height: visible ? implicitHeight : 0
                    text: menu.item === null ? "" : page.sioul.text(menu.item.quiet ? (menu.today ? "need-unskip" : "need-unskip-day") : (menu.today ? "needs-not-today" : "need-not-that-day"))
                    onTriggered: menu.act(menu.item.quiet ? "loud" : "quiet")
                }
                // The alarm at waking, the morning this night ends: none that morning, or back (docs/health.md).
                // A night known only from the timeline (the week before's) has no alarm said: none offered.
                MenuItem {
                    visible: menu.item !== null && menu.item.kind === "sleep" && (menu.item.alarm || "") !== "" && !menu.off
                    height: visible ? implicitHeight : 0
                    text: menu.item === null ? "" : page.sioul.textWith(menu.item.alarm_skipped ? "wake-unskip" : "wake-skip", "time", menu.item.alarm || "")
                    onTriggered: menu.act(menu.item.alarm_skipped ? "alarm" : "no-alarm")
                }
                MenuItem {
                    visible: menu.item !== null && menu.item.changed && !menu.off && !menu.alarmOnly
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("need-as-usual")
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
                    text: page.sioul.text(menu.off ? "need-put-back" : menu.added ? "needs-remove" : menu.today ? "need-remove-today" : "need-remove-day")
                    onTriggered: menu.act(menu.off ? "on" : "off")
                }
            }
        }
    }

    // A day's block moved, its times changed, or one added for that day.
    Later {
        id: dayForm

        sourceComponent: Component {
            NeedDayForm {
                sioul: page.sioul
                theme: page.theme
            }
        }
    }

    // A medicine, a prescription: new, changed, taken out. The page's own, so
    // that a form open stays open when their list moves to another column.
    Later {
        id: medicineForm

        sourceComponent: Component {
            MedicineDialog {
                sioul: page.sioul
                theme: page.theme
                onSaved: page.reload()
            }
        }
    }
    Later {
        id: prescriptionForm

        sourceComponent: Component {
            PrescriptionDialog {
                sioul: page.sioul
                theme: page.theme
                onSaved: page.reload()
            }
        }
    }

    // What is set once: behind the ⚙.
    Later {
        id: settings

        sourceComponent: Component {
            HealthSettings {
                sioul: page.sioul
                theme: page.theme
                onChanged: page.reload()
            }
        }
    }
}
