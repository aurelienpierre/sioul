// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Meals, naps and the night, set first: times kept free of tasks, the work
// planned around them (docs/health.md, "Meals, rest and sleep"). Today's
// first, each a few minutes later, at another time or not today, without a
// word asked; then each block, named as you like, on the weekdays you
// choose, its notices on or off. Nothing about eating or sleeping is
// recorded here, and nothing is said when one passes.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window
    // Today's, as they are now: moved, skipped.
    property var today: []
    // {needs, usual, gaps, skipped, moved}, as the backend gives it.
    property var shown: ({ needs: { meals_on: false, meals: [], naps_on: false, naps: [], sleep_on: false, sleep: { bed: "23:00", wake: "07:00", wind_down: 60, notices: true }, heads_up: 15, later: 15 }, usual: { meals: [], nap: "", sleep: "" }, gaps: [], skipped: [], moved: {} })
    property string problem: ""
    // A narrow screen: each block's numbers under its name.
    readonly property bool narrow: section.width < 560

    function reload() {
        section.shown = JSON.parse(section.sioul.needs())
        section.today = JSON.parse(section.sioul.needsToday() || "[]")
    }

    // The settings changed by `change` (on a copy), saved, and read again.
    function edit(change) {
        const needs = JSON.parse(JSON.stringify(section.shown.needs))
        change(needs)
        section.problem = section.sioul.saveNeeds(JSON.stringify(needs))
        section.reload()
    }

    function skip(key, skip) {
        section.problem = section.sioul.skipNeed(key, skip)
        section.reload()
    }

    function later(key) {
        section.problem = section.sioul.moveNeed(key, 0, "")
        section.reload()
    }

    // "08:30" as typed, or nothing when it is no time.
    function clock(text) {
        const parts = text.split(":")
        const h = Number(parts[0]), m = Number(parts[1])
        return parts.length === 2 && h >= 0 && h < 24 && m >= 0 && m < 60 ? (h < 10 ? "0" + h : String(h)) + ":" + (m < 10 ? "0" + m : String(m)) : ""
    }

    Component.onCompleted: section.reload()
    spacing: 8

    Label {
        Layout.topMargin: 12
        text: section.sioul.text("needs-title")
        font.weight: Font.DemiBold
        color: section.theme.text
    }
    Label {
        Layout.fillWidth: true
        text: section.sioul.text("needs-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: section.theme.muted
    }

    // Today's, as they are now: a few minutes later, at another time, or not today.
    Label {
        visible: section.today.length > 0
        Layout.topMargin: 4
        text: section.sioul.text("needs-today")
        font.weight: Font.DemiBold
        color: section.theme.text
    }
    Repeater {
        model: section.today

        // The buttons beside it, or under it on a phone.
        delegate: GridLayout {
            id: row

            required property var modelData

            Layout.fillWidth: true
            columns: section.narrow ? 1 : 2
            columnSpacing: 10
            rowSpacing: 2

            Label {
                Layout.fillWidth: true
                text: row.modelData.from + "–" + row.modelData.to + "   " + row.modelData.name + (row.modelData.moved !== 0 ? "   · " + section.sioul.textWith("needs-moved", "minutes", String(row.modelData.moved)) : "")
                textFormat: Text.PlainText
                font.strikeout: row.modelData.skipped
                wrapMode: Text.Wrap
                color: row.modelData.past || row.modelData.skipped ? section.theme.muted : section.theme.text
            }
            Flow {
                visible: !row.modelData.past
                Layout.alignment: Qt.AlignRight
                spacing: 6

                Button {
                    visible: !row.modelData.skipped
                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: section.sioul.textWith("need-later-n", "minutes", String(section.shown.needs.later))
                    onClicked: section.later(row.modelData.key)
                }
                Button {
                    visible: !row.modelData.skipped
                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: section.sioul.text("need-move-to") + "…"
                    onClicked: section.window.askNeed(row.modelData.key)
                }
                Button {
                    flat: true
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: section.sioul.text(row.modelData.skipped ? "need-unskip" : "needs-not-today")
                    onClicked: section.skip(row.modelData.key, !row.modelData.skipped)
                }
            }
        }
    }

    // Meals.
    Switch {
        text: section.sioul.text("needs-meals")
        checked: section.shown.needs.meals_on
        onToggled: section.edit(needs => needs.meals_on = checked)
    }
    Repeater {
        model: section.shown.needs.meals_on ? section.shown.needs.meals : []

        delegate: BlockEditor {
            required property var modelData
            required property int index

            kind: "meal"
            block: modelData
            position: index
            usual: section.shown.usual.meals[index] || ""
        }
    }
    Label {
        visible: section.shown.needs.meals_on && section.shown.gaps.length > 0
        Layout.fillWidth: true
        text: section.shown.gaps.join(" ")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: section.theme.muted
    }
    Button {
        visible: section.shown.needs.meals_on
        flat: true
        icon.name: "list-add"
        icon.color: section.theme.text
        text: section.sioul.text("needs-add-meal")
        onClicked: section.edit(needs => needs.meals.push({ at: "16:00", minutes: 15, before: 5, after: 0, days: [true, true, true, true, true, true, true], on: true, notices: true }))
    }

    // Naps.
    Switch {
        text: section.sioul.text("needs-naps")
        checked: section.shown.needs.naps_on
        onToggled: section.edit(needs => needs.naps_on = checked)
    }
    Repeater {
        model: section.shown.needs.naps_on ? section.shown.needs.naps : []

        delegate: BlockEditor {
            required property var modelData
            required property int index

            kind: "nap"
            block: modelData
            position: index
            usual: section.shown.usual.nap
        }
    }
    Button {
        visible: section.shown.needs.naps_on
        flat: true
        icon.name: "list-add"
        icon.color: section.theme.text
        text: section.sioul.text("needs-add-nap")
        onClicked: section.edit(needs => needs.naps.push({ at: "14:00", minutes: 20, before: 0, after: 15, days: [true, true, true, true, true, true, true], on: true, notices: true }))
    }

    // The night.
    Switch {
        text: section.sioul.text("needs-sleep")
        checked: section.shown.needs.sleep_on
        onToggled: section.edit(needs => needs.sleep_on = checked)
    }
    Flow {
        visible: section.shown.needs.sleep_on
        Layout.fillWidth: true
        spacing: 10

        LabeledTime {
            label: section.sioul.text("needs-bed")
            value: section.shown.needs.sleep.bed
            onChosen: time => section.edit(needs => needs.sleep.bed = time)
        }
        LabeledTime {
            label: section.sioul.text("needs-wake")
            value: section.shown.needs.sleep.wake
            onChosen: time => section.edit(needs => needs.sleep.wake = time)
        }
        LabeledMinutes {
            label: section.sioul.text("needs-wind-down")
            value: section.shown.needs.sleep.wind_down
            onChosen: minutes => section.edit(needs => needs.sleep.wind_down = minutes)
        }
        CheckBox {
            text: section.sioul.text("needs-notices")
            checked: section.shown.needs.sleep.notices
            onToggled: section.edit(needs => needs.sleep.notices = checked)
        }
    }

    // When the notices come, for all of them.
    Flow {
        visible: section.shown.needs.meals_on || section.shown.needs.naps_on || section.shown.needs.sleep_on
        Layout.fillWidth: true
        spacing: 10

        LabeledMinutes {
            label: section.sioul.text("needs-heads-up")
            value: section.shown.needs.heads_up
            onChosen: minutes => section.edit(needs => needs.heads_up = minutes)
        }
        LabeledMinutes {
            label: section.sioul.text("needs-later-by")
            value: section.shown.needs.later
            onChosen: minutes => section.edit(needs => needs.later = minutes)
        }
    }
    Label {
        visible: section.problem !== ""
        Layout.fillWidth: true
        text: section.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: section.theme.warm
    }

    // A time, typed: saved once it reads as one.
    component LabeledTime: RowLayout {
        id: timeField

        property string label
        property string value

        signal chosen(string time)

        spacing: 6

        Label {
            text: timeField.label
            color: section.theme.muted
        }
        TextField {
            Layout.preferredWidth: 70
            inputMask: "99:99"
            inputMethodHints: Qt.ImhTime
            text: timeField.value
            Accessible.name: timeField.label
            onEditingFinished: {
                const time = section.clock(text)
                if (time !== "" && time !== timeField.value)
                    timeField.chosen(time)
            }
        }
    }

    // Minutes, chosen.
    component LabeledMinutes: RowLayout {
        id: minutesField

        property string label
        property int value

        signal chosen(int minutes)

        spacing: 6

        Label {
            text: minutesField.label
            color: section.theme.muted
        }
        SpinBox {
            from: 0
            to: 180
            stepSize: 5
            editable: true
            value: minutesField.value
            Accessible.name: minutesField.label
            onValueModified: minutesField.chosen(value)
        }
    }

    // One meal or nap: its name, its time and minutes, its weekdays, its notices.
    component BlockEditor: Panel {
        id: editor

        property string kind
        property var block
        property int position
        property string usual
        readonly property string key: editor.kind + ":" + editor.position
        readonly property var list: editor.kind === "meal" ? "meals" : "naps"

        function change(apply) {
            section.edit(needs => apply(needs[editor.list][editor.position]))
        }

        Layout.fillWidth: true
        theme: section.theme

        ColumnLayout {
            anchors.fill: parent
            spacing: 6

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                TextField {
                    Layout.fillWidth: true
                    text: editor.block.name || ""
                    placeholderText: editor.usual
                    Accessible.name: section.sioul.text("needs-name-hint")
                    onEditingFinished: {
                        if (text.trim() !== (editor.block.name || ""))
                            editor.change(block => block.name = text.trim())
                    }
                }
                ToolButton {
                    icon.name: "user-trash"
                    icon.color: section.theme.text
                    Accessible.name: section.sioul.text("needs-remove")
                    ToolTip.visible: hovered
                    ToolTip.text: section.sioul.text("needs-remove")
                    ToolTip.delay: 400
                    onClicked: section.edit(needs => needs[editor.list].splice(editor.position, 1))
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 10

                LabeledTime {
                    label: section.sioul.text("needs-at")
                    value: editor.block.at
                    onChosen: time => editor.change(block => block.at = time)
                }
                LabeledMinutes {
                    label: section.sioul.text(editor.kind === "meal" ? "needs-eat" : "needs-nap-minutes")
                    value: editor.block.minutes
                    onChosen: minutes => editor.change(block => block.minutes = minutes)
                }
                LabeledMinutes {
                    label: section.sioul.text(editor.kind === "meal" ? "needs-prep" : "needs-after")
                    value: editor.kind === "meal" ? editor.block.before : editor.block.after
                    onChosen: minutes => editor.change(block => {
                        if (editor.kind === "meal")
                            block.before = minutes
                        else
                            block.after = minutes
                    })
                }
            }
            Flow {
                Layout.fillWidth: true
                spacing: 4

                Repeater {
                    model: 7

                    // Its weekdays, Monday first.
                    delegate: Button {
                        id: day

                        required property int index

                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                        checkable: true
                        checked: editor.block.days[day.index]
                        flat: !checked
                        text: section.sioul.text("weekday-" + (day.index + 1)).slice(0, 3)
                        Accessible.name: section.sioul.text("weekday-" + (day.index + 1))
                        onToggled: editor.change(block => block.days[day.index] = checked)
                    }
                }
                CheckBox {
                    text: section.sioul.text("needs-notices")
                    checked: editor.block.notices
                    onToggled: editor.change(block => block.notices = checked)
                }
            }
        }
    }
}
