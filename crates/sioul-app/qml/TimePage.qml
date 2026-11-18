// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Time spent: a week, a month or a year, as bars stacked by project, the
// hours of each project and what is left to bill, then each stretch of time,
// newest first. Time is noted by hand here too; a click changes any stretch
// not billed (its start, its end, what, its task), its menu takes it out.
// The Projects page's filter by area (AreaFilter.qml, its ticks shared):
// time on a project not ticked folds; one project chosen shows whatever the
// ticks (docs/areas.md, "A page's filter by area").

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // "week", "month", "year".
    property string period: "week"
    property date anchor: page.wall(Date.now())
    property string project: ""
    property var shown: null
    property var projects: []
    property string problem: ""
    readonly property var periods: ["week", "month", "year"]
    // A narrow screen (a phone): the title and the choices under the arrows,
    // each stretch of time on two lines.
    readonly property bool narrow: page.width < 600

    // For the window's tests: the first stretch, being changed.
    property alias changing: timeDialog

    function changeFirst() {
        if (page.shown && page.shown.entries.length > 0)
            timeDialog.now().change(page.shown.entries[0])
    }

    // An instant (ms) as a date whose own clock reads the time zone Sioul shows
    // (`zone::current`, Settings ▸ Agenda ▸ Time zone; `zoneOffset`, JavaScript's own offset back).
    function wall(ms) {
        return new Date(ms + (typeof page.sioul.zoneOffset === "function" ? page.sioul.zoneOffset(ms / 1000) + new Date(ms).getTimezoneOffset() * 60000 : 0))
    }

    function iso(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // Every area ticked, whatever the hour (the window's pictures): what the
    // projects' filter keeps until the hours change, as a tick of yours.
    function showAll() {
        page.sioul.tickProjectAreas("work+admin+leisure")
        page.reload()
    }

    // Time spent, noted by hand.
    function startNew() {
        timeDialog.now().begin(page.project)
    }

    // The period asked: made off the window's thread, shown when it comes (`timeView`).
    function reload() {
        page.projects = JSON.parse(page.sioul.projectRows() || "[]")
        page.sioul.showTimePage(page.period, page.iso(page.anchor), page.project)
    }

    function move(steps) {
        let d = new Date(page.anchor)
        if (page.period === "week") {
            d.setDate(d.getDate() + 7 * steps)
        } else {
            // A month or a year away, on the same day or that month's last:
            // from 31 October, back is 30 September, never 1 October again.
            const months = page.period === "year" ? 12 * steps : steps
            const last = new Date(d.getFullYear(), d.getMonth() + months + 1, 0).getDate()
            d = new Date(d.getFullYear(), d.getMonth() + months, Math.min(d.getDate(), last))
        }
        page.anchor = d
        page.reload()
    }

    function colour(index) {
        return page.theme.chartColors[index % page.theme.chartColors.length]
    }

    // A bar's day (or month, over a year) in your language, and its length as
    // the core writes lengths: "45 min", "2 h", "2 h 05" (timereport::duration).
    function barTip(bar) {
        const day = new Date(bar.date + "T12:00:00").toLocaleDateString(page.window.sioulLocale, page.period === "year" ? "MMMM yyyy" : "dddd d MMMM")
        const hours = Math.floor(bar.minutes / 60)
        const minutes = bar.minutes % 60
        const length = hours === 0 ? minutes + " min" : minutes === 0 ? hours + " h" : hours + " h " + String(minutes).padStart(2, "0")
        return day + "  ·  " + length
    }

    onVisibleChanged: {
        if (page.visible)
            page.reload()
    }
    onPeriodChanged: page.reload()
    Component.onCompleted: page.reload()

    Connections {
        target: page.sioul

        // The view asked, made.
        function onTimeViewChanged() {
            page.shown = JSON.parse(page.sioul.timeView || "null")
        }
        function onLinksChanged() {
            page.reload()
        }
        // The projects time is noted for.
        function onProjectsChanged() {
            if (page.visible)
                page.reload()
        }
        // An invoice issued, paid or changed, here or elsewhere: the time billed and to bill.
        function onInvoicesChanged() {
            if (page.visible)
                page.reload()
        }
        // Time noted on another device, come through the sharing.
        function onSharedIn(stores) {
            if (page.visible && stores.indexOf("data/time/") >= 0)
                page.reload()
        }
    }
    // The day turned (the window's clock): today's bar is the new day's.
    Connections {
        target: page.window

        function onTodayChanged() {
            if (page.visible)
                page.reload()
        }
        // The time changed (work's hours over, a meal, the night): the filter's ticks follow.
        function onMomentChanged() {
            if (page.visible)
                page.reload()
        }
    }

    // The week, the month or the year: tabs across the page (Tabs.qml), which stay as the page scrolls.
    Tabs {
        id: periods

        objectName: "timePeriods"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: page.theme.gap
        theme: page.theme
        name: page.sioul.text("ui-time")
        model: page.periods.map(p => ({ id: p, text: page.sioul.text("time-period-" + p) }))
        current: page.period
        onChosen: id => page.period = id
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        anchors.margins: page.theme.gap
        anchors.topMargin: periods.y + periods.height + 12
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 12

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    text: page.sioul.text("agenda-today")
                    onClicked: {
                        page.anchor = page.wall(Date.now())
                        page.reload()
                    }
                }
                Button {
                    implicitWidth: 40
                    text: "◂"
                    Accessible.name: page.sioul.text("agenda-earlier")
                    onClicked: page.move(-1)
                }
                Button {
                    implicitWidth: 40
                    text: "▸"
                    Accessible.name: page.sioul.text("agenda-later")
                    onClicked: page.move(1)
                }
                Label {
                    visible: !page.narrow
                    Layout.fillWidth: true
                    text: page.shown ? page.shown.title : ""
                    textFormat: Text.PlainText
                    font.pixelSize: 19
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                Item {
                    visible: page.narrow
                    Layout.fillWidth: true
                }
                ProjectChoice {
                    visible: !page.narrow
                    Layout.preferredWidth: 180
                }
                Button {
                    Layout.preferredWidth: page.narrow ? 40 : -1
                    text: page.sioul.text("time-note")
                    icon.name: "chronometer-start"
                    icon.color: page.theme.text
                    display: page.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                    Accessible.name: page.sioul.text("time-note")
                    onClicked: timeDialog.now().begin(page.project)
                }
                // A project's billable time, as a spreadsheet.
                ToolButton {
                    icon.name: "document-export"
                    icon.color: page.theme.text
                    Accessible.name: page.sioul.text("time-export")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("time-export")
                    ToolTip.delay: 400
                    onClicked: timeExport.now().begin(page.project)
                }
            }
            Label {
                visible: page.narrow
                Layout.fillWidth: true
                text: page.shown ? page.shown.title : ""
                textFormat: Text.PlainText
                font.pixelSize: 19
                wrapMode: Text.Wrap
                color: page.theme.text
            }
            ProjectChoice {
                visible: page.narrow
                Layout.fillWidth: true
            }

            // What shows: the time on work's, admin's, leisure's projects, as the hours say until
            // you tick others; nothing else explains it. Hidden while one project is chosen.
            AreaFilter {
                objectName: "timeAreas"
                visible: page.project === "" && page.shown !== null && !!page.shown.filter
                sioul: page.sioul
                theme: page.theme
                shown: page.shown ? page.shown.filter || null : null
                help: page.sioul.text("time-filter-help")
                onTicked: picked => {
                    page.sioul.tickProjectAreas(picked)
                    page.reload()
                }
                onAsHours: {
                    page.sioul.projectAreasAsHours()
                    page.reload()
                }
            }
            Label {
                Layout.fillWidth: true
                // What the filter shows, with the whole beside it when it folds some ("12 h shown, 31 h in all").
                text: page.shown ? (page.shown.sentence !== "" ? page.shown.sentence : page.shown.total_line || page.sioul.textWith("time-total", "time", page.shown.total)) : ""
                textFormat: Text.PlainText
                font.pixelSize: 16
                wrapMode: Text.Wrap
                color: page.shown && page.shown.sentence !== "" ? page.theme.muted : page.theme.text
            }
            Label {
                visible: page.problem !== ""
                Layout.fillWidth: true
                text: page.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.warm
            }

            // A bar a day, or a month: stacked by project, the tallest the most worked.
            Item {
                visible: page.shown !== null && page.shown.most > 0
                Layout.fillWidth: true
                Layout.preferredHeight: 180

                Row {
                    id: bars

                    anchors.fill: parent
                    spacing: 4

                    Repeater {
                        model: page.shown ? page.shown.bars : []

                        delegate: Item {
                            id: bar

                            required property var modelData

                            width: Math.max(4, (bars.width - bars.spacing * ((page.shown ? page.shown.bars.length : 1) - 1)) / Math.max(1, page.shown ? page.shown.bars.length : 1))
                            height: bars.height
                            ToolTip.visible: hover.hovered && bar.modelData.minutes > 0
                            ToolTip.text: page.theme.plain(page.barTip(bar.modelData))
                            ToolTip.delay: 200

                            HoverHandler {
                                id: hover
                            }

                            Column {
                                anchors.bottom: dayLabel.top
                                anchors.bottomMargin: 4
                                width: parent.width

                                Repeater {
                                    // The largest part at the bottom: drawn last.
                                    model: bar.modelData.parts.map((m, i) => ({ minutes: m, index: i })).filter(p => p.minutes > 0).reverse()

                                    delegate: Rectangle {
                                        required property var modelData

                                        width: bar.width
                                        height: (bars.height - 24) * modelData.minutes / page.shown.most
                                        color: page.colour(modelData.index)
                                    }
                                }
                            }
                            Label {
                                id: dayLabel

                                anchors.bottom: parent.bottom
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: bar.modelData.label
                                textFormat: Text.PlainText
                                font.pixelSize: 11
                                font.weight: bar.modelData.today ? Font.Bold : Font.Normal
                                color: bar.modelData.today ? page.theme.accent : page.theme.muted
                            }
                        }
                    }
                }
            }

            // Each project: its time, and what is left to bill (under the
            // title on a phone).
            Repeater {
                model: page.shown ? page.shown.projects : []

                delegate: GridLayout {
                    id: line

                    required property var modelData

                    Layout.fillWidth: true
                    columnSpacing: 10
                    rowSpacing: 2

                    Rectangle {
                        Layout.row: 0
                        Layout.column: 0
                        Layout.preferredWidth: 12
                        Layout.preferredHeight: 12
                        radius: 3
                        color: page.colour(line.modelData.index)
                    }
                    Label {
                        Layout.row: 0
                        Layout.column: 1
                        Layout.fillWidth: true
                        text: line.modelData.title
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: page.theme.text
                    }
                    Label {
                        Layout.row: 0
                        Layout.column: 2
                        text: line.modelData.minutes > 0 ? line.modelData.time : ""
                        textFormat: Text.PlainText
                        color: page.theme.text
                    }
                    RowLayout {
                        visible: line.modelData.unbilled !== ""
                        Layout.row: page.narrow ? 1 : 0
                        Layout.column: page.narrow ? 1 : 3
                        Layout.columnSpan: page.narrow ? 2 : 1
                        Layout.fillWidth: page.narrow
                        spacing: 10

                        Label {
                            Layout.fillWidth: page.narrow
                            text: page.sioul.textWith("project-to-bill", "time", line.modelData.unbilled) + (line.modelData.unbilled_amount !== "" ? " (" + line.modelData.unbilled_amount + ")" : "")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.warm
                        }
                        Button {
                            visible: line.modelData.billable
                            flat: true
                            text: page.sioul.text("invoice-make")
                            onClicked: {
                                page.window.openProject(line.modelData.id)
                            }
                        }
                    }
                }
            }

            // Each stretch of time, newest first.
            Repeater {
                model: page.shown ? page.shown.entries : []

                delegate: ItemDelegate {
                    id: entry

                    required property var modelData

                    Layout.fillWidth: true
                    padding: 4

                    background: Rectangle {
                        color: entry.hovered ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                    }

                    // Any stretch not billed: changed with a click (its start, its end, what,
                    // its task), or taken out from its menu.
                    onClicked: {
                        if (entry.modelData.invoice === "")
                            timeDialog.now().change(entry.modelData)
                    }
                    // On a touch screen, the menu comes at a long press; letting go then opens nothing.
                    onPressAndHold: {
                        entry.Window.window.menuAt = entry.mapToItem(null, entry.pressX, entry.pressY)
                        if (entry.modelData.invoice !== "")
                            return
                        entryMenu.key = entry.modelData.key
                        entryMenu.entry = entry.modelData
                        entryMenu.popup()
                    }
                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        // A touch has no buttons: on a touch screen, the row's long press.
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        enabled: entry.modelData.invoice === ""
                        onTapped: {
                            entryMenu.key = entry.modelData.key
                            entryMenu.entry = entry.modelData
                            entryMenu.popup()
                        }
                    }

                    contentItem: GridLayout {
                        columnSpacing: 10
                        rowSpacing: 2

                        Label {
                            Layout.row: 0
                            Layout.column: 0
                            Layout.preferredWidth: page.narrow ? -1 : 150
                            Layout.fillWidth: page.narrow
                            text: entry.modelData.date
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                        Label {
                            Layout.row: 0
                            Layout.column: 1
                            Layout.preferredWidth: 60
                            text: entry.modelData.time
                            textFormat: Text.PlainText
                            color: page.theme.text
                        }
                        ColumnLayout {
                            Layout.row: page.narrow ? 1 : 0
                            Layout.column: page.narrow ? 0 : 2
                            Layout.columnSpan: page.narrow ? 2 : 1
                            Layout.fillWidth: true
                            spacing: 0

                            Label {
                                Layout.fillWidth: true
                                text: entry.modelData.title
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            // Its project, its note, how its minutes were known (timed, noted by hand,
                            // timed then corrected; quietly, docs/capacity.md), its invoice.
                            Label {
                                Layout.fillWidth: true
                                text: [entry.modelData.project_title, entry.modelData.note, entry.modelData.kind_said || "", entry.modelData.invoice !== "" ? page.sioul.textWith("time-on-invoice", "number", entry.modelData.invoice) : ""].filter(t => t !== "").join("  ·  ")
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                        }
                    }
                }
            }
        }
    }

    // The project shown: beside the title, or under it on a phone.
    component ProjectChoice: PlainComboBox {
        readonly property var choices: [{ id: "", title: page.sioul.text("time-all-projects") }].concat(page.projects)

        model: choices.map(c => page.theme.plain(c.title))
        currentIndex: Math.max(0, choices.findIndex(c => c.id === page.project))
        onActivated: index => {
            page.project = choices[index].id
            page.reload()
        }
    }

    SioulMenu {
        id: entryMenu

        property string key: ""
        property var entry: null

        MenuItem {
            text: page.sioul.text("time-change")
            onTriggered: timeDialog.now().change(entryMenu.entry)
        }
        MenuItem {
            text: page.sioul.text("time-remove")
            onTriggered: page.problem = page.sioul.removeTime(entryMenu.key)
        }
    }

    Later {
        id: timeDialog

        sourceComponent: Component {
            TimeDialog {
                id: timeDialogForm

                sioul: page.sioul
                theme: page.theme
                onSaved: page.reload()
            }
        }
    }

    Later {
        id: timeExport

        sourceComponent: Component {
            TimeExport {
                id: timeExportForm

                sioul: page.sioul
                theme: page.theme
                window: page.window
                projects: page.projects
            }
        }
    }
}
