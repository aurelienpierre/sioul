// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

pragma ComponentBehavior: Bound

// One group of usual sites (a country's offices, its banks…): all of them
// pinned at once, or one (docs/sites.md, "Presets").

import QtQuick
import QtQuick.Controls.Basic

SioulMenu {
    id: groupMenu

    required property var sioul
    // {"name", "sites": [row]}; a row says whether it is already "kept".
    property var group: ({ name: "", sites: [] })

    signal pin(var rows)

    // "&" marks a shortcut in a menu: "&&" is one ("PG&E"). A word joiner
    // after each "<" keeps a name plain text (Theme.qml's `plain`).
    title: groupMenu.group.name.replace(/</g, "<\u2060").replace(/&/g, "&&")

    MenuItem {
        text: groupMenu.sioul.textWith("site-presets-pin-all", "count", String(groupMenu.group.sites.filter(r => !r.kept).length))
        enabled: groupMenu.group.sites.some(r => !r.kept)
        onTriggered: groupMenu.pin(groupMenu.group.sites.filter(r => !r.kept))
    }
    MenuSeparator {}

    Instantiator {
        model: groupMenu.group.sites

        delegate: MenuItem {
            id: siteLine

            required property var modelData

            text: siteLine.modelData.name.replace(/</g, "<\u2060").replace(/&/g, "&&") + (siteLine.modelData.kept ? "  ✓" : "")
            enabled: !siteLine.modelData.kept
            onTriggered: groupMenu.pin([siteLine.modelData])
        }
        onObjectAdded: (index, object) => groupMenu.insertItem(index + 2, object)
        onObjectRemoved: (index, object) => groupMenu.removeItem(object)
    }
}
