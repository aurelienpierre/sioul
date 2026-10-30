// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A time zone found by typing part of its name ("paris", "new y", "utc"):
// the matches listed under the field, at most twelve, those whose city
// starts with it first; a press picks one (`picked`), Enter the first.
// Settings ▸ Agenda ▸ Time zone and an event's own clock (EventDialog.qml)
// use it; the names are the time zone database's (`zone::matching`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: picker

    required property var sioul
    required property var theme
    // The zone chosen, by its name; "" none yet.
    property string zone: ""
    // What is typed, read as it changes: the field's own text.
    property string typed: ""
    readonly property var matches: picker.typed.trim().length >= 2 && picker.typed !== picker.zone ? JSON.parse(picker.sioul.zoneNames(picker.typed) || "[]") : []

    signal picked(string name)

    function pick(name) {
        picker.zone = name
        query.text = name
        picker.typed = name
        picker.picked(name)
    }

    spacing: 2
    onZoneChanged: {
        if (query.text !== picker.zone) {
            query.text = picker.zone
            picker.typed = picker.zone
        }
    }

    TextField {
        id: query

        objectName: "zoneQuery"
        Layout.fillWidth: true
        text: picker.zone
        placeholderText: picker.sioul.text("zone-search")
        Accessible.name: picker.sioul.text("zone-search")
        onTextEdited: picker.typed = query.text
        onAccepted: {
            if (picker.matches.length > 0)
                picker.pick(picker.matches[0])
        }
    }
    Repeater {
        model: picker.matches

        delegate: ItemDelegate {
            id: match

            required property string modelData

            objectName: "zoneMatch"
            Layout.fillWidth: true
            text: picker.theme.plain(match.modelData.replace(/_/g, " "))
            onClicked: picker.pick(match.modelData)
        }
    }
}
