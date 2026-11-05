// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Share with…" on an availability's card (AvailabilitySetup.qml): only who
// is asked (a person, a group, anyone with a link, a booking tool, and for a
// person or a group their name); Sioul then names the way it will use and
// why, in one sentence, its server's own first (`availability_ways`), the
// others one press away; a way set up on your server's own page (a booking
// page, Fastmail's link) opens that page and takes the link you paste back.
// Share makes it (`share_availability`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: chooser

    required property var sioul
    required property var theme
    // The availability's id.
    property string availability: ""
    // Its server's own page for a way you set up there (AvailabilitySetup's `pageOf`).
    property var pageOf: null
    property string audience: "person"
    property var choices: []
    property int chosen: 0
    property string problem: ""
    readonly property var choice: chooser.choices.length > 0 ? chooser.choices[Math.min(chooser.chosen, chooser.choices.length - 1)] : null
    // A way whose link you paste from your server's page.
    readonly property bool pasted: chooser.choice !== null && ["appointments", "google-schedule", "fastmail-link"].indexOf(chooser.choice.way) >= 0
    readonly property bool named: chooser.audience === "person" || chooser.audience === "group"

    signal done(bool shared)

    // A name given as if typed, and the ways asked for it (the window's pictures).
    function askWho(name) {
        whoField.text = name
        chooser.ask()
    }

    // The ways asked again: the audience, or the name, changed.
    function ask() {
        const who = chooser.named ? whoField.text.trim() : ""
        if (chooser.named && who === "") {
            chooser.choices = []
            chooser.problem = ""
            return
        }
        let answer = {}
        try {
            answer = JSON.parse(chooser.sioul.availabilityWays(JSON.stringify({ id: chooser.availability, audience: chooser.audience, who: who })) || "{}")
        } catch (e) {
            answer = {}
        }
        chooser.choices = answer.choices || []
        chooser.chosen = 0
        chooser.problem = answer.problem || ""
    }

    objectName: "availabilityChooser"
    spacing: 4
    Component.onCompleted: chooser.ask()

    Flow {
        Layout.fillWidth: true
        spacing: 4

        Repeater {
            model: ["person", "group", "link", "booking"]

            delegate: RadioButton {
                id: kind

                required property string modelData

                objectName: "availabilityAudience"
                text: chooser.sioul.text("availability-audience-" + kind.modelData)
                checked: chooser.audience === kind.modelData
                onToggled: {
                    if (checked) {
                        chooser.audience = kind.modelData
                        chooser.ask()
                    }
                }
            }
        }
    }
    TextField {
        id: whoField

        objectName: "availabilityWho"
        visible: chooser.named
        Layout.fillWidth: true
        placeholderText: chooser.sioul.text(chooser.audience === "group" ? "availability-who-group" : "availability-who-person")
        Accessible.name: placeholderText
        onTextEdited: asking.restart()
    }
    Timer {
        id: asking

        interval: 400
        onTriggered: chooser.ask()
    }
    Label {
        objectName: "availabilityWay"
        visible: chooser.choice !== null
        Layout.fillWidth: true
        text: chooser.choice !== null ? chooser.choice.sentence : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: chooser.theme.text
    }
    // A booking tool can also read your calendars on their server itself: nothing to share from here.
    Label {
        objectName: "availabilityBookingDirect"
        visible: chooser.audience === "booking"
        Layout.fillWidth: true
        text: chooser.sioul.text("availability-booking-direct")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: chooser.theme.muted
    }
    Button {
        objectName: "availabilityAnotherWay"
        visible: chooser.choices.length > 1
        text: chooser.sioul.text("availability-another-way")
        onClicked: chooser.chosen = (chooser.chosen + 1) % chooser.choices.length
    }
    RowLayout {
        visible: chooser.pasted
        Layout.fillWidth: true
        spacing: 6

        Button {
            objectName: "availabilityOpenPage"
            text: chooser.sioul.text("ui-open")
            onClicked: {
                const page = chooser.pageOf && chooser.choice !== null ? chooser.pageOf(chooser.choice.way, chooser.choice.account) : ""
                if (page !== "")
                    Qt.openUrlExternally(page)
            }
        }
        TextField {
            id: pastedField

            objectName: "availabilityPasted"
            Layout.fillWidth: true
            Layout.preferredWidth: 1
            placeholderText: chooser.sioul.text("availability-paste-link")
            Accessible.name: placeholderText
        }
    }
    Label {
        objectName: "availabilityChooserProblem"
        visible: chooser.problem !== ""
        Layout.fillWidth: true
        text: chooser.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: chooser.theme.warm
    }
    RowLayout {
        spacing: 8

        Button {
            objectName: "availabilityShare"
            enabled: chooser.choice !== null && (!chooser.pasted || pastedField.text.trim() !== "")
            text: chooser.sioul.text("availability-share")
            onClicked: {
                const c = chooser.choice
                chooser.problem = chooser.sioul.shareAvailability(JSON.stringify({ id: chooser.availability, audience: chooser.audience, who: c.who, way: c.way, account: c.account, link: chooser.pasted ? pastedField.text.trim() : "" }))
                if (chooser.problem === "")
                    chooser.done(true)
            }
        }
        Button {
            flat: true
            text: chooser.sioul.text("ui-cancel")
            onClicked: chooser.done(false)
        }
    }
}
