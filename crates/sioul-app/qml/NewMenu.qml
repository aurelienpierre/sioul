// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Something new, of any kind, from anywhere: a message, a task, an event, a
// contact, a note, a budget movement, time spent, a project. One button and
// one key (Ctrl+N) for all of them; each opens where that kind lives.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

SioulMenu {
    id: newMenu

    required property var sioul
    required property var window


    // [kind, words, icon].
    readonly property var kinds: [
        ["mail", "new-mail", "mail-message-new"],
        ["task", "new-task", "view-task"],
        ["event", "new-event", "view-calendar-day"],
        ["contact", "new-contact", "contact-new"],
        ["note", "new-note", "view-pim-notes"],
        ["movement", "new-movement", "view-financial-account-add"],
        ["paper", "new-paper", "document-new"],
        ["time", "new-time", "chronometer-start"],
        ["project", "new-project", "folder-new"],
        // One line on where you stopped, for when you are back.
        ["stopped", "new-stopped", "bookmarks-organize"]
    ]

    Repeater {
        model: newMenu.kinds

        delegate: MenuItem {
            required property var modelData

            text: newMenu.sioul.text(modelData[1])
            icon.name: modelData[2]
            onTriggered: newMenu.window.newThing(modelData[0])
        }
    }
}
