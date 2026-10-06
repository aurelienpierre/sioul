// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The usual meals, naps and night, set first, in the Health page's settings:
// times kept free of tasks, the work planned around them (docs/health.md,
// "Meals, rest and sleep"). Each block named as you like, on the weekdays
// you choose, its notices on or off; a day that differs is changed on the
// page itself, that day only. Nothing about eating or sleeping is recorded
// here, and nothing is said when one passes.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    // {needs, usual, gaps, wake}, as the backend gives it.
    property var shown: ({ needs: { meals_on: false, meals: [], naps_on: false, naps: [], sleep_on: false, sleep: { bed: "23:00", wake: "07:00", wind_down: 60, notices: true, alarm: [false, false, false, false, false, false, false] }, heads_up: 15, later: 15 }, usual: { meals: [], nap: "", sleep: "" }, gaps: [], wake: { phone: false, names: [], short: [], next: "", exact: true, screen: true, notifications: true } })
    property string problem: ""
    // A narrow screen: each block's numbers under its name.
    readonly property bool narrow: section.width < 560

    // Saved: the page's days follow.
    signal changed

    function reload() {
        section.shown = JSON.parse(section.sioul.needs())
    }

    // The settings changed by `change` (on a copy), saved, and read again.
    function edit(change) {
        const needs = JSON.parse(JSON.stringify(section.shown.needs))
        change(needs)
        section.problem = section.sioul.saveNeeds(JSON.stringify(needs))
        section.reload()
        section.changed()
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
        font.pixelSize: 16
        font.weight: Font.DemiBold
        color: section.theme.accent
    }
    Label {
        Layout.fillWidth: true
        text: section.sioul.text("needs-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: section.theme.muted
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

    // The alarm at waking, rung by a phone: the mornings it rings on, in the
    // locale's order (docs/health.md, "The alarm at waking"). Kept with the
    // night, so set from any device; a desktop never rings.
    ColumnLayout {
        id: alarm

        // The weekdays in the locale's order, as their places Monday first (0 to 6).
        readonly property var order: {
            const first = Qt.locale(section.sioul.text("qt-locale")).firstDayOfWeek
            const start = (first + 6) % 7
            return [0, 1, 2, 3, 4, 5, 6].map(i => (start + i) % 7)
        }
        readonly property var wake: section.shown.wake || { phone: false, names: [], short: [], next: "", exact: true, screen: true, notifications: true }
        readonly property var mornings: section.shown.needs.sleep.alarm || [false, false, false, false, false, false, false]
        readonly property bool ticked: alarm.mornings.some(on => on)

        visible: section.shown.needs.sleep_on
        Layout.fillWidth: true
        spacing: 4

        // Back from Android's page where it was allowed: said as it is now.
        Connections {
            target: section.sioul
            enabled: alarm.wake.phone

            function onAwayChanged() {
                if (!section.sioul.away) {
                    trial.answer = null
                    section.reload()
                }
            }
        }
        Label {
            text: section.sioul.text("wake-title")
            color: section.theme.muted
        }
        Flow {
            Layout.fillWidth: true
            spacing: 4

            Repeater {
                model: alarm.order

                delegate: Button {
                    id: morning

                    required property int modelData

                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    checkable: true
                    checked: alarm.mornings[morning.modelData]
                    flat: !checked
                    text: alarm.wake.short[morning.modelData] || ""
                    Accessible.name: alarm.wake.names[morning.modelData] || ""
                    onToggled: {
                        const on = checked
                        section.edit(needs => {
                            needs.sleep.alarm = needs.sleep.alarm || [false, false, false, false, false, false, false]
                            needs.sleep.alarm[morning.modelData] = on
                        })
                        // As saved, here or on another device: the binding back.
                        morning.checked = Qt.binding(() => alarm.mornings[morning.modelData])
                    }
                }
            }
        }
        Label {
            Layout.fillWidth: true
            text: section.sioul.text("wake-help") + (alarm.ticked && alarm.wake.next !== "" ? " " + alarm.wake.next : "")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: section.theme.muted
        }
        // On a phone, what Android refuses it, and its page to allow it (once only: the try below may say it already).
        Repeater {
            model: alarm.wake.phone && alarm.ticked ? [["exact", "wake-exact-off", "wake-allow-exact"], ["notifications", "wake-notifications-off", "wake-allow-notifications"], ["screen", "wake-screen-off", "wake-allow-screen"]].filter(refused => !alarm.wake[refused[0]] && !(trial.answer !== null && trial.answer.fix === refused[0])) : []

            delegate: ColumnLayout {
                id: refusal

                required property var modelData

                Layout.fillWidth: true
                spacing: 2

                Label {
                    Layout.fillWidth: true
                    text: section.sioul.text(refusal.modelData[1])
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    // It would not ring, or not stop: said warm; the screen unlit, muted.
                    color: refusal.modelData[0] === "screen" ? section.theme.muted : section.theme.warm
                }
                Button {
                    flat: true
                    text: section.sioul.text(refusal.modelData[2])
                    onClicked: section.sioul.wakeSettings(refusal.modelData[0])
                }
            }
        }
    }

    // On a phone, the alarm tried: rung ten seconds on, as a waking would be,
    // over the lock screen; nothing written, the next waking as it is
    // (crates/sioul-app/src/wake.rs). Under the mornings; the night off, here
    // all the same, so that trying it changes nothing of the night.
    ColumnLayout {
        id: trial

        // Its answer: {rings, line, fix, button}; null before.
        property var answer: null

        visible: alarm.wake.phone
        Layout.fillWidth: true
        spacing: 4

        Button {
            text: section.sioul.text("wake-try")
            onClicked: trial.answer = JSON.parse(section.sioul.tryWake())
        }
        Label {
            visible: trial.answer !== null
            Layout.fillWidth: true
            text: trial.answer === null ? "" : trial.answer.line
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            // It will ring: muted; refused, why: warm.
            color: trial.answer !== null && !trial.answer.rings ? section.theme.warm : section.theme.muted
        }
        Button {
            visible: trial.answer !== null && trial.answer.fix !== ""
            flat: true
            text: trial.answer === null ? "" : trial.answer.button
            onClicked: section.sioul.wakeSettings(trial.answer.fix)
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
