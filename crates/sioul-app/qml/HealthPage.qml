// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Health: its day as a list, ‹ › a day at a time (docs/health.md, "The
// page"): its meals, naps, night and doses in time order, each a tap away
// from what can change it, that day only: a quarter of an hour later,
// another time, other times, not that day, taken out; and one can be added
// for that day alone. The same day in time, among the rest, is the agenda's
// Day, and its week the agenda's Week (the unified agenda, part 5): Week here
// opens it, for one release. Today's doses say whether they are marked taken,
// and the doubt when another device may know: never "not taken". The
// medicines and the prescriptions are the page's content, after the day:
// under its list, or in a column of their own on a wide window
// (MedicinesSection.qml); their forms open from there. What is set once (the
// usual meals and night, the pauses, where the errands go) is behind the ⚙.
// Nothing counts what was not done.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // The page's view (`health::page`): the week of the day shown, its days' lists, and what only today says.
    property var shown: ({ week: { monday: "", today: "", days: [] }, missed: [], shared_note: "", reminded_there: "", unreadable: "", later: 15, any: true, medicines: [], prescriptions: [] })
    // The medicines, the prescriptions and the week's days, taken from the view
    // only when they change: the view comes again each minute, and their rows
    // stay as they are.
    property var medicines: []
    property var prescriptions: []
    property var weekDays: []
    // The day asked for ("2026-10-07"); the day shown is the view's, once it has it.
    property string wanted: page.iso(new Date())
    property var day: null
    // Shown from today: the page follows the days as they turn.
    property bool followsToday: true
    // A row under the pointer, or chosen ("date|key"): lit.
    property string hovered: ""
    property string chosen: ""
    // A row to bring into view, asked from the agenda (its key).
    property string reveal: ""
    signal revealed
    // A block's menu open (the window's pictures take it).
    property bool menuOpen: false
    // What a doctor or a pharmacist is shown, open over the window (ShowMedicines.qml).
    property bool professionalOpen: false
    // Back closes it first.
    readonly property bool canGoBack: page.professionalOpen
    function back() {
        if (page.professionalOpen)
            professional.close()
    }
    // A phone, or a window as narrow: the header on two lines.
    readonly property bool narrow: page.width < 760
    // A window wide enough for a third column: the medicines and prescriptions
    // beside the day (and the week); else under the day's list.
    readonly property bool wide: page.width >= 1400
    readonly property int sideWidth: Math.round(Math.max(340, Math.min(440, page.width * 0.24)))
    readonly property var locale: Qt.locale(page.sioul.text("qt-locale"))
    readonly property bool onToday: page.day !== null && page.day.today

    function iso(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // Where the usual meals, naps and night are set: the settings, opened there
    // (the Porch's card, Settings ▸ Hours, the window's pictures).
    function showNeeds() {
        settings.now().showNeeds()
    }

    // The week: the agenda's Week, where it is drawn in time among the rest (for one
    // release, then the link goes); the day: this page (the window's pictures too).
    function showWeek() {
        page.window.showAgendaWeek()
    }

    function showDay() {
        page.window.page = 10
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
        page.medicinesWanted()
    }

    // A meal, a nap, the night or a dose of `date` (its key: "meal:1",
    // "sleep", a dose's), from the agenda: that day shown, its row lit and
    // brought into view once the day is there.
    function openRow(date, key) {
        page.goTo(date)
        page.chosen = date + "|" + key
        page.reveal = key
        page.revealOn = date
        page.pick()
    }

    // The day a row waits for (`openRow`): revealed when the view has it.
    property string revealOn: ""

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

    // A prescription's medicines (its id), or all those taken now (""),
    // read-only over the whole window, for a doctor or a pharmacist.
    function showProfessional(id) {
        professional.now().show(page.sioul.healthForProfessional(id))
    }

    // The view, for the window's pictures; closed.
    function professionalView() {
        return professional.now()
    }

    function closeProfessional() {
        professional.close()
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

    // A dose writer's answer, said in the status line when it says something
    // went wrong (a dose of a day to come, a record that could not be written).
    function said(problem) {
        if (problem !== "")
            page.sioul.status = problem
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
        const days = page.shown.week.days || []
        if (JSON.stringify(days) !== JSON.stringify(page.weekDays))
            page.weekDays = days
        page.pick()
    }

    // The day asked for, once the view has its week; until then the one shown
    // stays. The same as before, it stays as it is: its timeline and its rows
    // are not made again each minute for nothing.
    function pick() {
        const days = page.weekDays
        const found = days.find(d => d.date === page.wanted)
        if (found) {
            if (JSON.stringify(found) !== JSON.stringify(page.day))
                page.day = found
        } else if (page.day === null && days.length > 0) {
            page.day = days.find(d => d.today) || days[0]
        }
        // A row asked for from the agenda, once its day is the one shown: brought into view.
        if (page.revealOn !== "" && page.day !== null && page.day.date === page.revealOn) {
            page.revealOn = ""
            Qt.callLater(page.revealed)
        }
    }

    function goTo(date) {
        page.wanted = date
        page.followsToday = date === page.window.today
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

    // A day earlier or later.
    function move(steps) {
        const d = new Date(page.wanted + "T12:00:00")
        d.setDate(d.getDate() + steps)
        const first = page.earliest()
        page.goTo(page.iso(d < first ? first : d))
    }

    readonly property bool earlierAllowed: new Date(page.wanted + "T12:00:00") > page.earliest()

    function title() {
        return page.day ? page.day.title : ""
    }

    // A day's block changed, that day only (`health::change_need`): the page and the plan follow.
    function change(date, key, action, more) {
        const problem = page.sioul.changeNeed(JSON.stringify(Object.assign({ date: date, key: key, action: action }, more || {})))
        if (problem !== "")
            page.sioul.status = problem
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

    Connections {
        target: page.sioul

        function onHealthViewChanged() {
            page.takeView()
        }

        // A dose marked, a medicine or a day changed on another device: shown at once.
        function onSharedIn(stores) {
            if (page.visible && (stores.indexOf("state/health-state.toml") >= 0 || stores.indexOf("state/health-doses.toml") >= 0 || stores.indexOf("data/health.toml") >= 0 || stores.indexOf("data/health-days.toml") >= 0))
                page.reload()
        }
    }

    // Another day when the clock turns past midnight, if the page showed today.
    function follow() {
        if (page.followsToday && page.wanted !== page.window.today)
            page.goTo(page.window.today)
        else
            page.reload()
    }

    onVisibleChanged: {
        if (page.visible)
            page.follow()
    }
    Component.onCompleted: {
        page.takeView()
        page.reload()
    }

    // Doses pass their time, and what is known of them from your other devices
    // ages: made again at each minute's turn of the window's clock while open,
    // and at once when the app comes back.
    Connections {
        target: page.window

        function onNowChanged() {
            if (page.visible)
                page.follow()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: 8

        // ‹ The day › … Week (the agenda's); the settings. On a phone, on two lines.
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
                onClicked: page.goTo(page.window.today)
            }
            WeekLink {
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

            WeekLink {}
            Button {
                visible: !page.onToday
                flat: true
                text: page.sioul.text("agenda-today")
                onClicked: page.goTo(page.window.today)
            }
            Item {
                Layout.fillWidth: true
            }
            SettingsGear {}
        }

        // The health file could not be read here: said above the day and the
        // week alike, on every day, never read as no medicines (review of 36002ea, can-wait 12).
        Label {
            objectName: "health-unreadable"
            visible: (page.shown.unreadable || "") !== ""
            Layout.fillWidth: true
            text: page.shown.unreadable || ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: page.theme.warm
        }

        // Doses due while Sioul ran on none of your computers: a question on the
        // past, answered once; never a reminder to take one now. Made only then.
        Loader {
            active: page.shown.missed.length > 0
            visible: active
            Layout.fillWidth: true
            sourceComponent: missedComponent
        }

        // The day's list; on a wide window, the medicines and the prescriptions beside it.
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: page.theme.gap

            // Made a moment after the first screen, without holding the window.
            Loader {
                id: listLoader

                Layout.fillWidth: true
                Layout.fillHeight: true
                asynchronous: true
                sourceComponent: dayListComponent
            }

            // Wide: their own column, its width kept from the start (nothing
            // moves when it comes). Made after the first screen, without holding the window.
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

    // The week, drawn in time among the rest: the agenda's (for one release, then it goes).
    component WeekLink: Button {
        objectName: "healthWeekLink"
        flat: true
        text: page.sioul.text("health-week-in-agenda")
        onClicked: page.showWeek()
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
        onShowProfessional: id => page.showProfessional(id)
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
                        // Chosen, its block lit; a meal, a nap or the night: its card, where you are.
                        TapHandler {
                            onTapped: {
                                page.chosen = row.ident
                                const kind = row.modelData.kind === "sleep" ? "night" : row.modelData.kind
                                if (kind !== "dose")
                                    page.window.openCard({ kind: kind, srcDate: scroll.day.date, srcKey: row.modelData.key, title: row.modelData.name })
                            }
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
                                    text: row.isDose ? row.modelData.from : row.modelData.from + " – " + row.modelData.to
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
                                    text: page.theme.plain(row.modelData.taken !== "" ? page.sioul.textWith("health-taken-at", "time", row.modelData.taken) : page.sioul.text(row.modelData.late && !row.modelData.choose ? "health-taken-when" : "health-taken"))
                                    icon.name: row.modelData.taken !== "" ? "task-complete" : ""
                                    icon.color: page.theme.text
                                    onClicked: {
                                        // Answers that differ on your devices: you choose.
                                        if (row.modelData.choose) {
                                            page.said(page.sioul.doseChoose(row.modelData.key, true))
                                            page.reload()
                                            return
                                        }
                                        if (row.modelData.taken === "" && row.modelData.late) {
                                            page.window.askDose(row.modelData.key)
                                            return
                                        }
                                        page.said(page.sioul.setDoseTaken(row.modelData.key, row.modelData.taken === ""))
                                        page.reload()
                                    }
                                }
                                Button {
                                    visible: row.isDose && scroll.day.today && row.modelData.choose === true
                                    flat: true
                                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                                    text: page.sioul.text("health-not-taken")
                                    onClicked: {
                                        page.said(page.sioul.doseChoose(row.modelData.key, false))
                                        page.reload()
                                    }
                                }
                                // The quickest change, at hand: a quarter of an hour later; put back when taken out.
                                Button {
                                    visible: row.editable
                                    flat: true
                                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                                    text: page.theme.plain(row.modelData.off ? page.sioul.text("need-put-back") : page.sioul.textWith("need-later-n", "minutes", String(page.shown.later)))
                                    onClicked: page.change(scroll.day.date, row.modelData.key, row.modelData.off ? "on" : "later")
                                }
                                // On a past day, a night not over (after midnight): its alarm alone.
                                ToolButton {
                                    id: more

                                    visible: (row.editable || (row.modelData.kind === "sleep" && row.modelData.alarm_open === true)) && !row.modelData.off
                                    text: "⋯"
                                    Accessible.name: page.sioul.textWith("need-menu", "name", row.modelData.name)
                                    ToolTip.visible: hovered
                                    ToolTip.text: page.theme.plain(Accessible.name)
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
                            // Not marked here, and not known whether it was taken on another device: said,
                            // never guessed; "This device is off" for each device it names.
                            DoubtLine {
                                Layout.fillWidth: true
                                Layout.leftMargin: timeWidth.implicitWidth + 12
                                Layout.preferredWidth: 1
                                sioul: page.sioul
                                theme: page.theme
                                doubt: row.modelData.doubt
                                offs: row.modelData.doubt_off || []
                                onSaidOff: page.reload()
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
                Item {
                    Layout.preferredHeight: page.theme.gap
                }
            }
        }
    }

    // Doses neither marked nor reminded: due while Sioul ran nowhere, or while it
    // ran but their reminder could not be shown. Did you take them?
    Component {
        id: missedComponent

        Panel {
            theme: page.theme

            ColumnLayout {
                anchors.fill: parent
                spacing: 6

                Repeater {
                    model: page.shown.missed

                    delegate: ColumnLayout {
                        id: missed

                        required property var modelData
                        required property int index

                        Layout.fillWidth: true
                        spacing: 2

                        // The question, above the first dose of its kind: Sioul closed,
                        // or its reminder not shown (health.rs, `missed_rows`).
                        Label {
                            visible: missed.index === 0 || page.shown.missed[missed.index - 1].question !== missed.modelData.question
                            Layout.fillWidth: true
                            Layout.topMargin: missed.index === 0 ? 0 : 6
                            Layout.bottomMargin: 4
                            text: missed.modelData.question
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.weight: Font.DemiBold
                            color: page.theme.text
                        }
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
                        // Whether it was taken on another device is not known here: said, never
                        // guessed; "This device is off" for each device it names.
                        DoubtLine {
                            Layout.fillWidth: true
                            sioul: page.sioul
                            theme: page.theme
                            doubt: missed.modelData.doubt
                            offs: missed.modelData.doubt_off || []
                            onSaidOff: page.reload()
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
            NeedMenu {
                id: menu

                sioul: page.sioul
                theme: page.theme
                later: page.shown.later
                onActed: action => page.change(menu.day.date, menu.item.key, action)
                onFormAsked: mode => dayForm.now().edit(mode, menu.day, menu.item)
                onOpened: page.menuOpen = true
                onClosed: page.menuOpen = false
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

    // What a doctor or a pharmacist is shown, made the first time.
    Later {
        id: professional

        sourceComponent: Component {
            ShowMedicines {
                sioul: page.sioul
                theme: page.theme
                onOpened: page.professionalOpen = true
                onClosed: page.professionalOpen = false
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
