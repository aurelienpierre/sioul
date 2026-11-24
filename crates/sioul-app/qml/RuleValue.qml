// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The value of a rule's condition or parameter, its control chosen by what
// the core declares the value to be (`value` in the catalogue, docs/rules.md):
// words, addresses or domains as one field, several separated by commas;
// a text, a size or a time as one field; a date; days of the week to tick;
// a number; a choice among declared things (a recognised kind, a circle, a
// kind of file) or among your own (a project, a topic, an address, a task
// list, a model of task, a level of notice, a contact) as a drop-down,
// what is not available yet greyed with "later". A test "between" gives a
// second control for where it ends. "none" (a test such as "exists") shows
// nothing. It says `edited(value, until)`, the value always a list; the
// editor (RuleEditor.qml) keeps it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: slot

    required property var sioul
    required property var theme
    // What the value is: "text", "words", "addresses", "domains", "choice", "days",
    // "time", "date", "size", "number", "topic", "project", "contact", "account",
    // "list", "model", "level", "file-type", "paper-kind", "person", "pattern", "none".
    property string kind: "text"
    // What a choice chooses among: [{id, name, available}].
    property var things: []
    // The value as the rule holds it: a list (any of them holds).
    property var value: []
    // Where a "between" ends.
    property string until: ""
    property bool between: false
    // One value only (a parameter): a field holds it whole, commas and all.
    property bool single: false
    // Its name, for a screen reader.
    property string name: ""

    signal edited(var value, string until)

    readonly property bool typed: ["text", "words", "addresses", "domains", "size", "time", "pattern"].includes(slot.kind)
    readonly property bool listed: !slot.single && ["words", "addresses", "domains"].includes(slot.kind)
    readonly property bool chosen: ["choice", "topic", "project", "contact", "account", "list", "model", "level", "file-type", "paper-kind", "person"].includes(slot.kind)
    // Days by their ids, Monday first, when the core gives none.
    readonly property var days: slot.things.length > 0 ? slot.things : [1, 2, 3, 4, 5, 6, 0].map((d, i) => ({ id: ["mon", "tue", "wed", "thu", "fri", "sat", "sun"][i], name: Qt.locale(slot.sioul.text("qt-locale")).dayName(d, Locale.LongFormat), available: true }))

    // What a field holds, as the value: a list split at commas, trimmed, domains in small letters.
    function read(text) {
        if (!slot.listed)
            return text.trim() === "" ? [] : [text.trim()]
        const parts = text.split(",").map(p => p.trim()).filter(p => p !== "")
        return slot.kind === "domains" || slot.kind === "addresses" ? parts.map(p => p.toLowerCase()) : parts
    }

    visible: slot.kind !== "none" && slot.kind !== ""
    spacing: slot.theme.labelGap

    // Words, addresses, domains, a text, a size, a time: one field (two for "between").
    RowLayout {
        visible: slot.typed
        Layout.fillWidth: true
        spacing: slot.theme.labelGap

        TextField {
            id: typedField

            objectName: "ruleValueText"
            Layout.fillWidth: true
            text: slot.value.join(slot.listed ? ", " : "")
            placeholderText: slot.sioul.text("rules-value-" + (slot.typed ? slot.kind : "text"))
            Accessible.name: slot.name
            // Each control speaks only for the kind it is shown for: the others are hidden, never asked.
            onEditingFinished: {
                if (!slot.typed)
                    return
                const read = slot.read(typedField.text)
                if (JSON.stringify(read) !== JSON.stringify(slot.value))
                    slot.edited(read, slot.until)
            }
        }
        Label {
            visible: slot.between
            text: slot.sioul.text("rules-value-and")
            color: slot.theme.muted
        }
        TextField {
            id: untilField

            visible: slot.between
            Layout.fillWidth: true
            text: slot.until
            placeholderText: slot.sioul.text("rules-value-" + (slot.typed ? slot.kind : "text"))
            Accessible.name: slot.sioul.textWith("rules-value-until", "name", slot.name)
            onEditingFinished: {
                if (slot.typed && slot.between && untilField.text.trim() !== slot.until)
                    slot.edited(slot.value, untilField.text.trim())
            }
        }
    }

    // A date (two for "between").
    Flow {
        visible: slot.kind === "date"
        Layout.fillWidth: true
        spacing: slot.theme.labelGap

        DateField {
            id: dateField

            theme: slot.theme
            sioul: slot.sioul
            locale: Qt.locale(slot.sioul.text("qt-locale"))
            date: slot.value.length > 0 ? slot.value[0] : ""
            Accessible.name: slot.name
            onEdited: {
                if (slot.kind === "date")
                    slot.edited(dateField.date === "" ? [] : [dateField.date], slot.until)
            }
        }
        Label {
            visible: slot.between
            height: dateField.height
            verticalAlignment: Text.AlignVCenter
            text: slot.sioul.text("rules-value-and")
            color: slot.theme.muted
        }
        DateField {
            id: untilDate

            visible: slot.between
            theme: slot.theme
            sioul: slot.sioul
            locale: Qt.locale(slot.sioul.text("qt-locale"))
            date: slot.until
            Accessible.name: slot.sioul.textWith("rules-value-until", "name", slot.name)
            onEdited: {
                if (slot.kind === "date" && slot.between)
                    slot.edited(slot.value, untilDate.date)
            }
        }
    }

    // Days of the week: several to tick.
    Flow {
        visible: slot.kind === "days"
        Layout.fillWidth: true
        spacing: 4

        Repeater {
            model: slot.kind === "days" ? slot.days : []

            delegate: CheckBox {
                required property var modelData

                text: slot.theme.plain(modelData.name)
                checked: slot.value.includes(modelData.id)
                onToggled: {
                    const kept = slot.days.map(d => d.id).filter(id => id === modelData.id ? checked : slot.value.includes(id))
                    slot.edited(kept, slot.until)
                }
            }
        }
    }

    // A number.
    SpinBox {
        id: number

        objectName: "ruleValueNumber"
        visible: slot.kind === "number"
        from: 0
        to: 365
        editable: true
        value: slot.value.length > 0 ? Number(slot.value[0]) : 0
        Accessible.name: slot.name
        onValueModified: {
            if (slot.kind === "number" && Number.isFinite(number.value))
                slot.edited([number.value], slot.until)
        }
    }

    // A choice among declared things, or among your own.
    PlainComboBox {
        id: choice

        readonly property int at: slot.things.findIndex(t => slot.value.length > 0 && t.id === slot.value[0])

        objectName: "ruleValueChoice"
        visible: slot.chosen
        Layout.fillWidth: true
        model: slot.things.map(t => ({ text: t.available === false ? slot.sioul.textWith("rules-later", "name", t.name) : t.name, available: t.available !== false }))
        textRole: "text"
        currentIndex: choice.at
        displayText: choice.at < 0 ? slot.sioul.text(slot.things.length === 0 ? "rules-value-none-yet" : "rules-choose") : choice.currentText
        implicitContentWidthPolicy: ComboBox.WidestText
        Accessible.name: slot.name
        onActivated: index => {
            if (!slot.chosen)
                return
            const thing = slot.things[index]
            if (thing && thing.available !== false)
                slot.edited([thing.id], slot.until)
            choice.currentIndex = Qt.binding(() => choice.at)
        }

        delegate: ItemDelegate {
            id: item

            required property var modelData
            required property int index

            width: ListView.view.width
            enabled: item.modelData.available
            text: String(item.modelData.text).replace(/&/g, "&&").replace(/</g, "<⁠")
            palette.text: item.enabled ? choice.palette.text : slot.theme.muted
            font.weight: choice.currentIndex === item.index ? Font.DemiBold : Font.Normal
            highlighted: choice.highlightedIndex === item.index
        }
    }
}
