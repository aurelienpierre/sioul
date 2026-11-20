// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An office's opening hours on its card (Contacts; opening.rs, offices.rs):
// what is true now ("Open until 12:00", "Closed: opens Monday at 9:00"),
// the hours day by day, when it answers the phone if that differs, the
// free line for what Sioul cannot count, and where the hours come from and
// when they were checked. Its buttons: "Look them up…" (only on a card that
// says it is an organisation's or already holds hours: never a person's
// name; nothing sent before the person reads what goes), "Change"
// (OpeningHoursForm.qml) and "Call back…", a task that follows these hours.
// On any other card, one flat link, "This is an office, a shop or a
// practice", marks it as an organisation's (`markOffice`); the lookup is
// offered from then on. Read from the backend when the card opens, when the
// contacts change, and each minute while it shows, since what is true now
// moves on. Words from the card or a source are drawn as plain text.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Section {
    id: hours

    required property var sioul
    required property var window
    // The card's file.
    property string key: ""
    // What the backend said last (`contactHours`): null before it answers, or for a card gone.
    property var shown: null
    readonly property bool has: hours.shown !== null && (hours.shown.open !== null || hours.shown.phone !== null || (hours.shown.note || "") !== "")
    readonly property bool writable: hours.shown !== null && !hours.shown.read_only
    readonly property bool stale: hours.shown !== null && ((hours.shown.open && hours.shown.open.stale) || (hours.shown.phone && hours.shown.phone.stale))

    // "Change" and "Look them up…" pressed: the page opens their dialog.
    signal changeAsked(var shown)
    signal lookUpAsked(var shown)
    // "Call back…" made its task: {uid} or {error}.
    signal calledBack(var answer)
    // "This is an office…" pressed: {ok} or {error}.
    signal marked(var answer)

    // A second press of "Call back…" within a second makes nothing (two quick presses, one task).
    function callBack() {
        if (calledAgain.running)
            return
        calledAgain.start()
        hours.calledBack(JSON.parse(hours.sioul.callBack(hours.key) || "{}"))
    }

    function reload() {
        // Its key may come before the backend while the section is made: read once both are there.
        const text = hours.key !== "" && hours.sioul ? hours.sioul.contactHours(hours.key) : ""
        hours.shown = text ? JSON.parse(text) : null
    }

    objectName: "openingHours"
    title: hours.sioul.text("hours-title")

    onKeyChanged: hours.reload()
    onVisibleChanged: {
        if (hours.visible)
            hours.reload()
    }
    Component.onCompleted: hours.reload()

    Connections {
        target: hours.sioul

        function onContactsChanged() {
            hours.reload()
        }
    }
    Timer {
        id: calledAgain

        interval: 1000
    }
    // What is true now changes with the clock.
    Timer {
        interval: 60000
        repeat: true
        running: hours.visible && hours.key !== ""
        onTriggered: hours.reload()
    }

    Label {
        objectName: "openingHoursNone"
        visible: hours.shown !== null && !hours.has
        Layout.fillWidth: true
        text: hours.sioul.text("hours-none")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: hours.theme.muted
    }

    // When it is open, then when it answers the phone, each the same way.
    Repeater {
        model: hours.shown === null ? [] : [{ part: hours.shown.open, phone: false }, { part: hours.shown.phone, phone: true }].filter(p => p.part !== null && p.part !== undefined)

        delegate: ColumnLayout {
            id: part

            required property var modelData

            Layout.fillWidth: true
            spacing: 4

            Label {
                visible: part.modelData.phone
                Layout.fillWidth: true
                text: hours.sioul.text("hours-phone-title")
                textFormat: Text.PlainText
                font.weight: Font.DemiBold
                color: hours.theme.text
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }
            Label {
                objectName: part.modelData.phone ? "openingHoursPhoneNow" : "openingHoursNow"
                visible: part.modelData.part.now !== ""
                Layout.fillWidth: true
                text: part.modelData.part.now
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 15
                font.weight: Font.DemiBold
                color: hours.theme.text
            }
            Repeater {
                model: part.modelData.part.lines

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: hours.theme.text
                }
            }
            // Hours Sioul cannot count: as written, and said so.
            Label {
                visible: part.modelData.part.unread
                Layout.fillWidth: true
                text: part.modelData.part.value
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: hours.theme.text
            }
            // Why it counts none of it, when there is more to say ("Di" in doubt); else that it cannot;
            // public holidays as it reads them, and when Sioul knows none, where to add them.
            Label {
                visible: part.modelData.part.unread || part.modelData.part.lines.length > 0
                Layout.fillWidth: true
                text: part.modelData.part.unread ? (part.modelData.part.doubt || hours.sioul.text("hours-unread")) : hours.sioul.text(!part.modelData.part.holidays_off ? "hours-holidays-open" : hours.shown.holidays_known === false ? "hours-holidays-unknown" : "hours-holidays")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: hours.theme.muted
            }
            Label {
                visible: part.modelData.part.rest.length > 0 && !part.modelData.part.unread
                Layout.fillWidth: true
                text: part.modelData.part.rest.join("; ")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: hours.theme.muted
            }
            Label {
                objectName: part.modelData.phone ? "openingHoursPhoneSource" : "openingHoursSource"
                Layout.fillWidth: true
                text: part.modelData.part.source + (part.modelData.part.stale ? " " + hours.sioul.text("hours-stale") : "")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: hours.theme.muted
            }
        }
    }
    // A card of a book that can only be read: its hours shown, never written here.
    Label {
        objectName: "openingHoursReadOnly"
        visible: hours.shown !== null && hours.shown.read_only === true
        Layout.fillWidth: true
        text: hours.sioul.text("hours-read-only")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        color: hours.theme.muted
    }
    // The free line: shown, never counted.
    Label {
        objectName: "openingHoursNote"
        visible: hours.shown !== null && (hours.shown.note || "") !== ""
        Layout.fillWidth: true
        text: hours.shown ? hours.shown.note || "" : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: hours.theme.text
    }
    // A card that says neither that it is an organisation's nor holds hours: why no lookup, and the one link that says so.
    Label {
        objectName: "openingHoursMarkWhy"
        visible: hours.shown !== null && hours.shown.mark === true
        Layout.fillWidth: true
        text: hours.sioul.text("hours-mark-why")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        color: hours.theme.muted
    }
    Flow {
        visible: hours.shown !== null
        Layout.fillWidth: true
        spacing: 6

        Button {
            objectName: "openingHoursLookUp"
            // Only on a card that says it is an organisation's or holds hours, and that can be written.
            visible: hours.writable && hours.shown.asked !== null
            text: hours.sioul.text(hours.stale ? "hours-look-up-again" : "hours-look-up")
            icon.name: "edit-find"
            icon.color: hours.theme.text
            onClicked: hours.lookUpAsked(hours.shown)
        }
        Button {
            objectName: "openingHoursChange"
            visible: hours.writable
            text: hours.sioul.text(hours.has ? "hours-change" : "hours-add")
            icon.name: "document-edit"
            icon.color: hours.theme.text
            onClicked: hours.changeAsked(hours.shown)
        }
        Button {
            objectName: "openingHoursCallBack"
            visible: hours.shown !== null && (hours.shown.numbers || []).length > 0
            text: hours.sioul.text("hours-call-back")
            icon.name: "call-start"
            icon.color: hours.theme.text
            ToolTip.visible: hovered
            ToolTip.text: hours.shown ? hours.theme.plain(hours.sioul.textWith("hours-call-back-help", "name", hours.shown.name)) : ""
            ToolTip.delay: 400
            Accessible.description: ToolTip.text
            onClicked: hours.callBack()
        }
    }
    // Its words are long in French: on their own line, wrapped within the section, never past its edge.
    Button {
        id: markLink

        objectName: "openingHoursMark"
        visible: hours.writable && hours.shown.mark === true
        Layout.maximumWidth: hours.body.width
        flat: true
        text: hours.sioul.text("hours-mark-office")
        Accessible.name: text
        onClicked: hours.marked(JSON.parse(hours.sioul.markOffice(hours.key) || "{}"))

        contentItem: RowLayout {
            spacing: 6

            Icon {
                iconName: "go-next"
                size: 16
            }
            Label {
                Layout.fillWidth: true
                text: markLink.text
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: hours.theme.text
            }
        }
    }
}
