// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What a thing is for: work, your admin, leisure; any of them together, as
// three switches, on more lines when there is little room (docs/areas.md).
// An address's card and a device's (SettingRow.qml, DevicesTab.qml) show the
// same three: what was ticked is given back as it is written in files
// ("work+admin"; "" for none), and the switches follow `value` again. In a
// narrow column (`stacked`), one under the other, each as wide as the
// column, its word wrapping.

pragma ComponentBehavior: Bound

import QtQuick

Flow {
    id: areas

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The areas ticked, as written in files: "work+admin"; "" for none.
    property string value: ""
    readonly property var on: areas.value === "" ? [] : areas.value.split("+")
    // One switch a line, each as wide as this: for a table's column.
    property bool stacked: false

    // A switch turned: every area ticked now, as written in files.
    signal picked(string value)

    spacing: areas.stacked ? 0 : 12

    Repeater {
        model: ["work", "admin", "leisure"]

        delegate: WrapCheckBox {
            id: area

            required property string modelData

            width: areas.stacked ? areas.width : implicitWidth
            text: areas.sioul.text("area-" + modelData)
            checked: areas.on.indexOf(modelData) >= 0
            onToggled: {
                const was = areas.on
                const now = ["work", "admin", "leisure"].filter(a => a === modelData ? checked : was.indexOf(a) >= 0)
                areas.picked(now.join("+"))
                area.checked = Qt.binding(() => areas.on.indexOf(area.modelData) >= 0)
            }
        }
    }
}
