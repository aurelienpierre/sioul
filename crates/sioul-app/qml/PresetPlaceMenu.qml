// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

pragma ComponentBehavior: Bound

// A country's usual sites, or a state's: "All of them" first, then its groups;
// a place with a single group lists its sites at once (docs/sites.md, "Presets").
// A country's states or provinces are added after, by the page.

import QtQuick
import QtQuick.Controls.Basic

SioulMenu {
    id: placeMenu

    required property var sioul
    // {"name", "groups": [{"name", "sites"}]}.
    property var place: ({ name: "", groups: [] })
    // The sites of its own groups, not its states'.
    readonly property var everything: placeMenu.place.groups.reduce((all, g) => all.concat(g.sites), [])
    readonly property bool single: placeMenu.place.groups.length === 1

    signal pin(var rows)

    // "&" marks a shortcut in a menu: "&&" is one ("PG&E"). A word joiner
    // after each "<" keeps a name plain text (Theme.qml's `plain`).
    title: placeMenu.place.name.replace(/</g, "<\u2060").replace(/&/g, "&&")

    MenuItem {
        text: placeMenu.sioul.textWith("site-presets-pin-all", "count", String(placeMenu.everything.filter(r => !r.kept).length))
        enabled: placeMenu.everything.some(r => !r.kept)
        onTriggered: placeMenu.pin(placeMenu.everything.filter(r => !r.kept))
    }
    MenuSeparator {}

    Instantiator {
        model: placeMenu.single ? [] : placeMenu.place.groups

        delegate: PresetGroupMenu {
            id: groupMenu

            required property var modelData

            sioul: placeMenu.sioul
            group: groupMenu.modelData
            onPin: rows => placeMenu.pin(rows)
        }
        onObjectAdded: (index, object) => placeMenu.insertMenu(index + 2, object)
        onObjectRemoved: (index, object) => placeMenu.removeMenu(object)
    }
    Instantiator {
        model: placeMenu.single ? placeMenu.place.groups[0].sites : []

        delegate: MenuItem {
            id: siteLine

            required property var modelData

            text: siteLine.modelData.name.replace(/</g, "<\u2060").replace(/&/g, "&&") + (siteLine.modelData.kept ? "  ✓" : "")
            enabled: !siteLine.modelData.kept
            onTriggered: placeMenu.pin([siteLine.modelData])
        }
        onObjectAdded: (index, object) => placeMenu.insertItem(index + 2, object)
        onObjectRemoved: (index, object) => placeMenu.removeItem(object)
    }
}
