// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An appointment's menu (a right click, a long press), one for every view
// and its card (the unified agenda, part 3): Details first, as a tap gives
// them, then Edit, Delete (never a meeting with others), "Make it a task" or
// "Make it an appointment", Add ▾ and Link to…. What it offers is what its
// writers allow, read from its file (`eventCard`); each action opens its
// card and acts there, so that a form and a question have one place.

import QtQuick
import QtQuick.Controls.Basic

SioulMenu {
    id: menu

    required property var sioul
    required property var window
    required property var host
    // The row it was asked on, and its event as its file says it now.
    property var ref: null
    property var event: null
    readonly property bool there: menu.event !== null && menu.event.key !== undefined
    readonly property bool isTask: menu.there && (menu.event.task || "") !== ""
    readonly property var source: menu.there && menu.event.uid ? { uri: "sioul:event/" + encodeURIComponent(menu.event.uid), kind: "event", key: menu.event.key, title: menu.event.summary, start: menu.event.start } : null

    function ask(ref) {
        menu.ref = ref
        const read = JSON.parse(menu.sioul.eventCard(ref.srcKey || "", ref.srcStart || 0) || "{}")
        menu.event = read.key === undefined ? null : read
        if (menu.there)
            menu.popup()
    }

    // Its card, then what was asked there.
    function inCard(then) {
        menu.host.show(Object.assign({}, menu.ref, { kind: "event" }))
        if (menu.host.eventCard)
            then(menu.host.eventCard)
    }

    MenuItem {
        text: menu.sioul.text("ui-details")
        onTriggered: menu.inCard(() => {})
    }
    MenuItem {
        enabled: menu.there && !menu.event.read_only
        text: menu.sioul.text("ui-edit")
        onTriggered: menu.inCard(card => card.edit())
    }
    MenuItem {
        enabled: menu.there && !menu.event.read_only && menu.event.with_others !== true
        text: menu.sioul.text("ui-delete")
        onTriggered: menu.inCard(card => card.askDelete())
    }
    MenuItem {
        visible: menu.isTask || (menu.there && !menu.event.read_only && !menu.event.with_others)
        height: visible ? implicitHeight : 0
        text: menu.isTask ? menu.sioul.text("agenda-make-appointment") : menu.sioul.text("agenda-make-task")
        onTriggered: menu.inCard(card => card.changeKind())
    }
    MenuSeparator {}
    AddMenu {
        sioul: menu.sioul
        window: menu.window
        source: menu.source
    }
    MenuItem {
        enabled: menu.source !== null
        text: menu.sioul.text("ui-link-existing")
        onTriggered: menu.window.linkFrom(menu.source)
    }
}
