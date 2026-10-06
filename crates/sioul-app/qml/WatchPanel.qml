// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The watch on the Health page: when it last gave data, then in words the
// day's steps, resting heart rate (and its usual), the night's sleep (its
// length and its hours, no stages), Body Battery, the week; then today's
// curves, plain, no colours as grades. Where its files come from, and whether
// it may offer a pause or a walk between tasks.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: panel

    required property var sioul
    required property var theme
    // The page's `watch`: {any, synced, folder, offers, steps, resting, slept, battery, week, heart_rate, stress, battery_curve, from}.
    property var watch: null
    // "data": what the watch says; "settings": where its files come from and whether it may offer (the page's ⚙).
    property string part: "data"

    signal setting(string key, var value)

    spacing: 6

    Label {
        Layout.topMargin: panel.part === "data" ? 8 : 0
        text: panel.sioul.text("watch-title")
        font.weight: Font.DemiBold
        color: panel.part === "settings" ? panel.theme.accent : panel.theme.text
        // In the settings, as large as their other groups' names.
        Component.onCompleted: if (panel.part === "settings") font.pixelSize = 16
    }
    Label {
        visible: panel.part === "settings" && (panel.watch === null || !panel.watch.any)
        Layout.fillWidth: true
        text: panel.sioul.text("watch-none")
        wrapMode: Text.Wrap
        color: panel.theme.muted
    }
    Label {
        visible: panel.part === "data" && panel.watch !== null && panel.watch.any
        Layout.fillWidth: true
        text: panel.watch ? panel.watch.synced : ""
        textFormat: Text.PlainText
        font.pixelSize: 13
        color: panel.theme.muted
    }
    // In words, one line each.
    Repeater {
        model: panel.part === "data" && panel.watch ? [panel.watch.steps, panel.watch.resting, panel.watch.slept, panel.watch.battery, panel.watch.week].filter(t => t !== "") : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: panel.theme.text
        }
    }
    // Today's curves: heart rate (40–180), stress as Garmin scores it and Body Battery (0–100).
    Label {
        visible: panel.part === "data" && curves.any
        Layout.topMargin: 4
        text: panel.sioul.text("watch-curves")
        font.pixelSize: 13
        color: panel.theme.muted
    }
    Canvas {
        id: curves

        readonly property bool any: panel.watch !== null && (panel.watch.heart_rate.length > 1 || panel.watch.stress.length > 1 || panel.watch.battery_curve.length > 1)

        visible: panel.part === "data" && curves.any
        Layout.fillWidth: true
        Layout.preferredHeight: 110
        onWidthChanged: requestPaint()
        onVisibleChanged: requestPaint()

        Connections {
            target: panel

            function onWatchChanged() {
                curves.requestPaint()
            }
        }

        onPaint: {
            const ctx = getContext("2d")
            ctx.reset()
            if (!panel.watch)
                return
            const from = panel.watch.from
            const x = at => (at - from) / 86400 * width
            const line = (points, low, high, colour, dashed) => {
                if (points.length < 2)
                    return
                ctx.beginPath()
                ctx.strokeStyle = colour
                ctx.lineWidth = 1.5
                ctx.setLineDash(dashed ? [4, 3] : [])
                points.forEach((p, i) => {
                    const y = height - 4 - (Math.min(high, Math.max(low, p[1])) - low) / (high - low) * (height - 8)
                    if (i === 0)
                        ctx.moveTo(x(p[0]), y)
                    else
                        ctx.lineTo(x(p[0]), y)
                })
                ctx.stroke()
            }
            // Noon and the hour now, faint.
            ctx.strokeStyle = panel.theme.line
            ctx.setLineDash([])
            ctx.beginPath()
            ctx.moveTo(width / 2, 0)
            ctx.lineTo(width / 2, height)
            ctx.stroke()
            line(panel.watch.battery_curve, 0, 100, panel.theme.accent, false)
            line(panel.watch.stress, 0, 100, panel.theme.muted, true)
            line(panel.watch.heart_rate, 40, 180, panel.theme.text, false)
        }
    }
    // Where its files come from, and whether it may offer.
    RowLayout {
        visible: panel.part === "settings"
        Layout.fillWidth: true
        spacing: 8

        Label {
            text: panel.sioul.text("watch-folder")
            color: panel.theme.muted
        }
        Label {
            Layout.fillWidth: true
            text: panel.watch && panel.watch.folder !== "" ? panel.watch.folder : "—"
            textFormat: Text.PlainText
            elide: Text.ElideMiddle
            color: panel.theme.text
        }
        Button {
            text: panel.sioul.text("watch-folder-choose")
            onClicked: folderPicker.open()
        }
    }
    WrapCheckBox {
        visible: panel.part === "settings"
        Layout.fillWidth: true
        text: panel.sioul.text("watch-offers")
        checked: panel.watch !== null && panel.watch.offers
        onToggled: panel.setting("watch_offers", checked)
    }

    FolderDialog {
        id: folderPicker

        // A path, as the core keeps it: "C:/…" on Windows, not "/C:/…".
        onAccepted: panel.setting("watch_folder", panel.theme.localPath(folderPicker.selectedFolder.toString()))
    }
}
