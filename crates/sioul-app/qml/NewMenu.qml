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


    // Each kind, its words and its icon (named `icon`, as the test of the
    // bundled icons reads them: crates/sioul-app/tests/icons.rs).
    readonly property var kinds: [
        { kind: "mail", words: "new-mail", icon: "mail-message-new" },
        { kind: "task", words: "new-task", icon: "view-task" },
        { kind: "event", words: "new-event", icon: "view-calendar-day" },
        { kind: "contact", words: "new-contact", icon: "contact-new" },
        { kind: "note", words: "new-note", icon: "view-pim-notes" },
        { kind: "movement", words: "new-movement", icon: "view-financial-account-add" },
        { kind: "paper", words: "new-paper", icon: "document-new" },
        { kind: "time", words: "new-time", icon: "chronometer-start" },
        { kind: "project", words: "new-project", icon: "folder-new" },
        // One line on where you stopped, for when you are back.
        { kind: "stopped", words: "new-stopped", icon: "bookmarks-organize" }
    ]

    Repeater {
        model: newMenu.kinds

        delegate: MenuItem {
            required property var modelData

            text: newMenu.sioul.text(modelData.words)
            icon.name: modelData.icon
            onTriggered: newMenu.window.newThing(modelData.kind)
        }
    }
}
