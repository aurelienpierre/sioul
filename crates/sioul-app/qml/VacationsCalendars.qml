// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Hours ▸ Vacations and holidays, under your vacations (18
// November 2026): each calendar of holidays you subscribed to, with its
// switch "Its days count as my days off" (on by default for public
// holidays, off for school holidays), and what this device knows of it: a
// device that has not read it yet does not know its days, and says so (the
// calendar's events never travel between devices; `subscriptions::holidays`).
// "Subscribe to a calendar of holidays…" opens Agenda ⚙ ▸ Your agendas, its
// form open (`subscribeAsked`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: block

    required property var sioul
    required property var theme
    // `subscriptions::holidays`: {calendars: [{id, name, holidays, days_off, after_class, known, line}]}.
    property var view: ({ calendars: [] })
    property string problem: ""

    signal subscribeAsked

    function read() {
        try {
            block.view = JSON.parse(block.sioul.holidaysCalendars() || "{}")
        } catch (e) {
            block.view = { calendars: [] }
        }
        if (!block.view.calendars)
            block.view = { calendars: [] }
    }

    objectName: "vacationsCalendars"
    spacing: 4
    Component.onCompleted: block.read()

    Connections {
        target: block.sioul

        // A calendar subscribed, read, switched or stopped, here or on another device.
        function onAgendasChanged() {
            block.read()
        }
    }

    Label {
        Layout.fillWidth: true
        text: block.sioul.text("vacations-calendars")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: block.theme.text
    }
    Label {
        objectName: "vacationsCalendarsNone"
        visible: block.view.calendars.length === 0
        Layout.fillWidth: true
        text: block.sioul.text("vacations-calendars-none")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: block.theme.muted
    }
    Repeater {
        model: block.view.calendars

        delegate: ColumnLayout {
            id: calendar

            required property var modelData

            objectName: "holidaysCalendar"
            Layout.fillWidth: true
            spacing: 2

            WrapCheckBox {
                objectName: "holidaysCounts"
                Layout.fillWidth: true
                text: calendar.modelData.name + " · " + block.sioul.text("subscribe-holidays-" + calendar.modelData.holidays) + " — " + block.sioul.text("vacations-counts")
                checked: calendar.modelData.days_off === true
                onToggled: {
                    block.problem = block.sioul.setSubscriptionField(calendar.modelData.id, "days_off", checked ? "true" : "false")
                    block.read()
                }
            }
            Label {
                objectName: "holidaysLine"
                Layout.fillWidth: true
                Layout.leftMargin: 28
                text: calendar.modelData.line
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: calendar.modelData.known === true ? block.theme.muted : block.theme.warm
            }
        }
    }
    Label {
        visible: block.problem !== ""
        Layout.fillWidth: true
        text: block.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: block.theme.warm
    }
    Button {
        objectName: "holidaysSubscribe"
        text: block.sioul.text("vacations-subscribe")
        onClicked: block.subscribeAsked()
    }
}
