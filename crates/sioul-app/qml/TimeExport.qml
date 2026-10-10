// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A project's billable time over a stretch of days, as a spreadsheet: what was
// done, how many hours, at what rate, for how much, and the total. This week,
// last week, this month, last month, everything, or from one day to another.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Dialog {
    id: exporting

    required property var sioul
    required property var theme
    required property var window
    // The projects that can be chosen: {id, title}.
    property var projects: []
    readonly property var ranges: ["this-week", "last-week", "this-month", "last-month", "all", "custom"]
    property string problem: ""

    function iso(d) {
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // The first and last day of a range, around today.
    function bounds(range) {
        const today = new Date()
        const monday = new Date(today.getFullYear(), today.getMonth(), today.getDate() - (today.getDay() + 6) % 7)
        const shift = (d, days) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + days)
        if (range === "this-week")
            return [exporting.iso(monday), exporting.iso(shift(monday, 6))]
        if (range === "last-week")
            return [exporting.iso(shift(monday, -7)), exporting.iso(shift(monday, -1))]
        if (range === "this-month")
            return [exporting.iso(new Date(today.getFullYear(), today.getMonth(), 1)), exporting.iso(new Date(today.getFullYear(), today.getMonth() + 1, 0))]
        if (range === "last-month")
            return [exporting.iso(new Date(today.getFullYear(), today.getMonth() - 1, 1)), exporting.iso(new Date(today.getFullYear(), today.getMonth(), 0))]
        if (range === "all")
            return ["1970-01-01", "9999-12-31"]
        return [fromDay.date, toDay.date]
    }

    function begin(project) {
        exporting.problem = ""
        projectChoice.currentIndex = Math.max(0, exporting.projects.findIndex(p => p.id === project))
        rangeChoice.currentIndex = 3
        fromDay.date = exporting.iso(new Date())
        toDay.date = exporting.iso(new Date())
        exporting.open()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(480, (parent ? parent.width : 480) - 2 * exporting.theme.gap)
    title: exporting.sioul.text("time-export")
    // Qt's standard "Cancel" is not translated here: Sioul's own.
    footer: DialogButtonBox {
        Button {
            text: exporting.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
    }

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: exporting.sioul.text("time-export-project")
            color: exporting.theme.muted
        }
        PlainComboBox {
            id: projectChoice

            Layout.fillWidth: true
            model: exporting.projects.map(p => exporting.theme.plain(p.title))
        }
        Label {
            text: exporting.sioul.text("time-export-range")
            color: exporting.theme.muted
        }
        PlainComboBox {
            id: rangeChoice

            Layout.fillWidth: true
            model: exporting.ranges.map(r => exporting.sioul.text("time-range-" + r))
        }
        Label {
            visible: rangeChoice.currentIndex === exporting.ranges.indexOf("custom")
            text: exporting.sioul.text("time-export-from")
            color: exporting.theme.muted
        }
        DateField {
            id: fromDay

            visible: rangeChoice.currentIndex === exporting.ranges.indexOf("custom")
            theme: exporting.theme
            sioul: exporting.sioul
            locale: exporting.window.sioulLocale
            pickLabel: exporting.sioul.text("event-pick-day")
        }
        Label {
            visible: rangeChoice.currentIndex === exporting.ranges.indexOf("custom")
            text: exporting.sioul.text("time-export-to")
            color: exporting.theme.muted
        }
        DateField {
            id: toDay

            visible: rangeChoice.currentIndex === exporting.ranges.indexOf("custom")
            theme: exporting.theme
            sioul: exporting.sioul
            locale: exporting.window.sioulLocale
            pickLabel: exporting.sioul.text("event-pick-day")
        }
        Label {
            visible: exporting.problem !== ""
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: exporting.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: exporting.theme.warm
        }
        Button {
            Layout.columnSpan: 2
            Layout.alignment: Qt.AlignRight
            enabled: projectChoice.currentIndex >= 0 && exporting.projects.length > 0
            highlighted: true
            text: exporting.sioul.text("time-export-save")
            onClicked: saving.open()
        }
    }

    FileDialog {
        id: saving

        fileMode: FileDialog.SaveFile
        nameFilters: ["CSV (*.csv)"]
        defaultSuffix: "csv"
        onAccepted: {
            const range = exporting.bounds(exporting.ranges[rangeChoice.currentIndex])
            const project = exporting.projects[projectChoice.currentIndex]
            exporting.problem = exporting.sioul.exportTimeCsv(project.id, range[0], range[1], saving.selectedFile.toString())
            if (exporting.problem === "")
                exporting.close()
        }
    }
}
