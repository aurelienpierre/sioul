// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An address field that completes from the contacts: what is typed after the
// last comma is looked up by name and address; Up and Down go through the
// suggestions, Enter or Tab takes one, Escape leaves them.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: field

    required property var sioul
    required property var theme
    property var suggestions: []

    // A suggestion was taken: the text changed without typing.
    signal picked

    function choose(index) {
        const parts = field.text.split(/[,;]/)
        parts[parts.length - 1] = (parts.length > 1 ? " " : "") + field.suggestions[index]
        field.text = parts.join(",") + ", "
        suggestionsPopup.close()
        field.picked()
    }

    onTextEdited: {
        field.suggestions = JSON.parse(field.sioul.completions(field.text) || "[]")
        if (field.suggestions.length > 0 && field.activeFocus) {
            list.currentIndex = 0
            suggestionsPopup.open()
        } else {
            suggestionsPopup.close()
        }
    }
    onActiveFocusChanged: if (!activeFocus) suggestionsPopup.close()
    Keys.onDownPressed: event => {
        if (suggestionsPopup.opened)
            list.incrementCurrentIndex()
        else
            event.accepted = false
    }
    Keys.onUpPressed: event => {
        if (suggestionsPopup.opened)
            list.decrementCurrentIndex()
        else
            event.accepted = false
    }
    Keys.onReturnPressed: event => {
        if (suggestionsPopup.opened && list.currentIndex >= 0)
            field.choose(list.currentIndex)
        else
            event.accepted = false
    }
    Keys.onTabPressed: event => {
        if (suggestionsPopup.opened && list.currentIndex >= 0)
            field.choose(list.currentIndex)
        else
            event.accepted = false
    }
    Keys.onEscapePressed: event => {
        if (suggestionsPopup.opened)
            suggestionsPopup.close()
        else
            event.accepted = false
    }

    Popup {
        id: suggestionsPopup

        y: field.height
        width: field.width
        padding: 2
        // The field keeps the keyboard.
        focus: false
        closePolicy: Popup.CloseOnPressOutsideParent

        background: Rectangle {
            color: field.theme.surface
            border.color: field.theme.line
            radius: field.theme.radius
        }

        ListView {
            id: list

            implicitHeight: contentHeight
            width: parent.width
            model: field.suggestions
            interactive: false

            delegate: ItemDelegate {
                id: suggestion

                required property string modelData
                required property int index

                width: list.width
                // A contact's name may come from a sender's: plain text in a list line.
                text: field.theme.plain(suggestion.modelData)
                highlighted: list.currentIndex === suggestion.index
                onClicked: field.choose(suggestion.index)
            }
        }
    }
}
