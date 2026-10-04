// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Something new, tied to the thing it is made from: a task, an event, a
// note, a message; a contact from a message's sender. One menu for every
// kind of thing, so a card keeps one button however many kinds there are.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

SioulMenu {
    id: addMenu

    required property var sioul
    required property var window
    // What the new thing is tied to: {uri, kind, key, title, start, name, address}.
    property var source: null
    readonly property string from: addMenu.source ? addMenu.source.kind : ""

    function offers(kind) {
        if (addMenu.source === null)
            return false
        if (kind === "task")
            return addMenu.from !== "task"
        if (kind === "note")
            return addMenu.from !== "note"
        if (kind === "contact")
            return addMenu.from === "mail" && !!addMenu.source.address && !addMenu.source.known
        return true
    }

    title: addMenu.sioul.text("ui-add-new")

    MenuItem {
        visible: addMenu.offers("task")
        height: visible ? implicitHeight : 0
        text: addMenu.sioul.text("add-task")
        icon.name: "view-task"
        onTriggered: addMenu.window.addLinked("task", addMenu.source)
    }
    MenuItem {
        visible: addMenu.offers("event")
        height: visible ? implicitHeight : 0
        text: addMenu.sioul.text("add-event")
        icon.name: "view-calendar-day"
        onTriggered: addMenu.window.addLinked("event", addMenu.source)
    }
    MenuItem {
        visible: addMenu.offers("note")
        height: visible ? implicitHeight : 0
        text: addMenu.sioul.text("add-note")
        icon.name: "view-pim-notes"
        onTriggered: addMenu.window.addLinked("note", addMenu.source)
    }
    MenuItem {
        visible: addMenu.offers("mail")
        height: visible ? implicitHeight : 0
        text: addMenu.from === "mail" ? addMenu.sioul.text("add-reply") : addMenu.sioul.text("add-mail")
        icon.name: addMenu.from === "mail" ? "mail-reply-sender" : "mail-message-new"
        onTriggered: addMenu.window.addLinked("mail", addMenu.source)
    }
    MenuItem {
        visible: addMenu.offers("contact")
        height: visible ? implicitHeight : 0
        text: addMenu.sioul.text("add-contact")
        icon.name: "contact-new"
        onTriggered: addMenu.window.addLinked("contact", addMenu.source)
    }
}
