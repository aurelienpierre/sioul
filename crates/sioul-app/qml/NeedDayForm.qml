// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One day's meal, nap or night, for that day only (docs/health.md): moved
// to another time, its times changed (from, to; a meal's minutes to get it
// ready, a nap's to come back, the night's bedtime), or one added for that
// day alone, a meal or a rest, named as you like. The usual ones stay as
// they are, in the page's settings. Nothing about food is asked.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: form

    required property var sioul
    required property var theme
    // "move", "times" or "add".
    property string mode: "times"
    // The day (`health::DayView`) and its row (`DayItem`); none for "add".
    property var day: null
    property var item: null
    // "meal" or "nap": the kind of the one added.
    property string kind: "meal"
    property string problem: ""
    readonly property bool night: form.item !== null && form.item.kind === "sleep"
    readonly property string shownKind: form.mode === "add" ? form.kind : form.item ? form.item.kind : "meal"

    // Saved: the page reads its days again.
    signal saved

    function edit(mode, day, item) {
        form.mode = mode
        form.day = day
        form.item = item
        form.kind = "meal"
        form.problem = ""
        // One added: from the next quarter of an hour today, else noon; for half an hour.
        let start = 12 * 60
        if (!item && day.today) {
            const now = new Date()
            start = Math.min(23 * 60, Math.ceil((now.getHours() * 60 + now.getMinutes() + 1) / 15) * 15)
        }
        const hm = minutes => {
            const pad = n => n < 10 ? "0" + n : String(n)
            return pad(Math.floor(minutes / 60) % 24) + ":" + pad(minutes % 60)
        }
        from.text = item ? item.from : hm(start)
        to.text = item ? item.to : hm(start + 30)
        bed.text = item && item.kind === "sleep" ? item.at : ""
        name.text = ""
        before.value = item ? item.before : 10
        after.value = item ? item.after : 10
        form.open()
        if (mode === "add")
            name.forceActiveFocus()
        else
            from.forceActiveFocus()
    }

    // "8:5" as typed, "08:05" as kept; "" when it reads as no time.
    function clock(text) {
        const parts = text.split(":")
        const h = Number(parts[0]), m = Number(parts[1])
        return parts.length === 2 && parts[0].trim() !== "" && h >= 0 && h < 24 && m >= 0 && m < 60 ? (h < 10 ? "0" + h : String(h)) + ":" + (m < 10 ? "0" + m : String(m)) : ""
    }

    function save() {
        const edit = {
            date: form.day.date,
            key: form.item ? form.item.key : "",
            action: form.mode,
            from: form.clock(from.text),
            to: form.clock(to.text),
            bed: form.clock(bed.text),
            before: before.value,
            after: after.value,
            kind: form.kind,
            name: name.text
        }
        form.problem = form.sioul.changeNeed(JSON.stringify(edit))
        if (form.problem !== "")
            return
        form.close()
        form.saved()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(440, (parent ? parent.width : 440) - 2 * form.theme.gap)
    title: form.theme.plain(form.mode === "add" ? (form.day ? form.sioul.textWith("need-add-title", "day", form.day.title) : "") : form.item && form.day ? form.item.name + "  ·  " + form.day.title : "")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        // One added: a meal or a rest, and its name.
        Row {
            visible: form.mode === "add"
            Layout.columnSpan: 2
            spacing: 6

            Repeater {
                model: ["meal", "nap"]

                delegate: Button {
                    id: kindButton

                    required property string modelData

                    text: form.sioul.text("need-kind-" + kindButton.modelData)
                    checkable: true
                    checked: form.kind === kindButton.modelData
                    flat: form.kind !== kindButton.modelData
                    onClicked: {
                        form.kind = kindButton.modelData
                        kindButton.checked = Qt.binding(() => form.kind === kindButton.modelData)
                    }
                }
            }
        }
        Label {
            visible: form.mode === "add"
            text: form.sioul.text("health-field-name")
            color: form.theme.muted
        }
        TextField {
            id: name

            visible: form.mode === "add"
            Layout.fillWidth: true
            placeholderText: form.sioul.text(form.kind === "nap" ? "need-added-nap" : "need-added-meal")
            Accessible.name: form.sioul.text("health-field-name")
        }

        // From, as the row says it: getting it ready, winding down included.
        Label {
            text: form.sioul.text(form.mode === "move" ? "need-move-to" : "need-form-from")
            color: form.theme.muted
        }
        TextField {
            id: from

            Layout.preferredWidth: 80
            inputMask: "99:99"
            inputMethodHints: Qt.ImhTime
            Accessible.name: form.sioul.text(form.mode === "move" ? "need-move-to" : "need-form-from")
            onAccepted: form.save()
        }
        // The night: bedtime between winding down and waking.
        Label {
            visible: form.mode === "times" && form.night
            text: form.sioul.text("needs-bed")
            color: form.theme.muted
        }
        TextField {
            id: bed

            visible: form.mode === "times" && form.night
            Layout.preferredWidth: 80
            inputMask: "99:99"
            inputMethodHints: Qt.ImhTime
            Accessible.name: form.sioul.text("needs-bed")
        }
        Label {
            visible: form.mode !== "move"
            text: form.sioul.text(form.night ? "needs-wake" : "need-form-to")
            color: form.theme.muted
        }
        TextField {
            id: to

            visible: form.mode !== "move"
            Layout.preferredWidth: 80
            inputMask: "99:99"
            inputMethodHints: Qt.ImhTime
            Accessible.name: form.sioul.text(form.night ? "needs-wake" : "need-form-to")
            onAccepted: form.save()
        }
        // A meal's minutes to get it ready, a nap's to come back: kept free with it.
        Label {
            visible: form.mode !== "move" && form.shownKind === "meal"
            text: form.sioul.text("needs-prep")
            color: form.theme.muted
        }
        SpinBox {
            id: before

            visible: form.mode !== "move" && form.shownKind === "meal"
            from: 0
            to: 180
            stepSize: 5
            editable: true
            Accessible.name: form.sioul.text("needs-prep")
        }
        Label {
            visible: form.mode !== "move" && form.shownKind === "nap"
            text: form.sioul.text("needs-after")
            color: form.theme.muted
        }
        SpinBox {
            id: after

            visible: form.mode !== "move" && form.shownKind === "nap"
            from: 0
            to: 180
            stepSize: 5
            editable: true
            Accessible.name: form.sioul.text("needs-after")
        }
        Label {
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: form.sioul.text("need-form-help")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: form.theme.muted
        }
        Label {
            visible: form.problem !== ""
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: form.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: form.theme.warm
        }
    }

    // The buttons inside an Item: a DialogButtonBox as the footer itself closes
    // the dialog on "Save" even when saving fails, and what went wrong is never read.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                text: form.sioul.text("ui-save")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: form.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onAccepted: form.save()
            onRejected: form.close()
        }
    }
}
