// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A drop-down of things a rule's editor offers (RuleEditor.qml), as the core
// declares them ({id, name, available}): a type, what happens, a field, a
// test, an action, where a parameter is taken from. What is not available
// yet is in the list greyed, its name with "later", and cannot be chosen.
// Nothing chosen yet, it says `placeholder`. It says `picked(id)`; the
// editor changes the rule, and `chosen` follows it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

PlainComboBox {
    id: combo

    required property var sioul
    required property var theme
    // [{id, name, available}]
    property var things: []
    // The id chosen; "" none.
    property string chosen: ""
    // Said while nothing is chosen.
    property string placeholder: ""

    signal picked(string id)

    // A thing's name, with "later" when it cannot be chosen yet.
    function named(thing) {
        return thing.available === false ? combo.sioul.textWith("rules-later", "name", thing.name) : thing.name
    }

    model: combo.things.map(t => ({ text: combo.named(t), available: t.available !== false }))
    textRole: "text"
    currentIndex: combo.things.findIndex(t => t.id === combo.chosen)
    displayText: combo.currentIndex < 0 ? combo.placeholder : combo.named(combo.things[combo.currentIndex])
    implicitContentWidthPolicy: ComboBox.WidestText
    onActivated: index => {
        const thing = combo.things[index]
        if (thing && thing.available !== false)
            combo.picked(thing.id)
        // Greyed, or chosen: the box shows what the rule holds.
        combo.currentIndex = Qt.binding(() => combo.things.findIndex(t => t.id === combo.chosen))
    }

    delegate: ItemDelegate {
        id: choice

        required property var modelData
        required property int index

        width: ListView.view.width
        enabled: choice.modelData.available
        // The "&" doubled and a word joiner after each "<", as PlainComboBox's own.
        text: String(choice.modelData.text).replace(/&/g, "&&").replace(/</g, "<⁠")
        palette.text: choice.enabled ? combo.palette.text : combo.theme.muted
        font.weight: combo.currentIndex === choice.index ? Font.DemiBold : Font.Normal
        highlighted: combo.highlightedIndex === choice.index
    }
}
