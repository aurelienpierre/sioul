// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Hours ▸ Vacations and holidays, under your vacations (18
// November 2026): each calendar of holidays you subscribed to, with its
// switch "Its days count as my days off" (on by default for public
// holidays, off for school holidays), and what this device knows of it: a
// device that has not read it yet does not know its days, and says so (the
// calendar's events never travel between devices; `subscriptions::holidays`).
// This switch is the calendar's days off's one place (Agenda ⚙ ▸ Your agendas
// says what kind of holidays it holds, not whether they count: 24 November
// 2026). "Subscribe to a calendar of holidays…" opens the subscribe form
// (SubscribeForm.qml, the one Agenda ⚙ ▸ Your agendas shows) over Settings,
// here, rather than sending you to the Agenda's ⚙: the act stays where the
// need is, and the subscription lands in Your agendas all the same.

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
    // The subscribe form open over Settings (its tests).
    property bool subscribing: false

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
                // One sentence, in each language's own order (`vacations-counts-line`).
                text: calendar.modelData.counts_line
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
        onClicked: subscribeDialog.now().ask()
    }

    // The subscribe form, over the window (made the first time it is asked for).
    Later {
        id: subscribeDialog

        sourceComponent: Component {
            SafePopup {
                id: dialog

                function ask() {
                    dialog.open()
                    form.begin()
                }

                objectName: "holidaysSubscribeDialog"
                onAboutToShow: block.subscribing = true
                onClosed: block.subscribing = false
                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(560, dialog.roomWidth - 2 * block.theme.gap)
                height: Math.min(implicitHeight, dialog.roomHeight - 2 * block.theme.gap)
                padding: 16

                contentItem: ScrollView {
                    contentWidth: availableWidth

                    SubscribeForm {
                        id: form

                        width: dialog.availableWidth
                        sioul: block.sioul
                        theme: block.theme
                        onDone: id => {
                            dialog.close()
                            block.read()
                        }
                        onCancelled: dialog.close()
                    }
                }
            }
        }
    }
}
