// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One cell of a contact's postal addresses, in its card's details
// (ContactsPage.qml): the address's label, or the address itself with
// Directions beside it (the owner's word, 6 November 2026: "geo: links should
// also be implemented for contact's address"), the way there opened as an
// event's place opens it (Directions.qml): the point the contacts' map placed
// it at when it has (`placeOf`), else its words. On a phone (`theme.compact`)
// Directions goes under the address, so that the row stays within the screen.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

GridLayout {
    id: line

    required property var theme
    // Sioul's backend: `text`, `placeOf`, `geoHandler`.
    required property var sioul
    // What the cell says: the label, or the address.
    property string text: ""
    // The address, when this cell holds one; "" for its label's cell.
    property string address: ""
    // The way there (Directions.qml, shared with the event card).
    property Directions way: Directions {
        sioul: line.sioul
    }

    columns: line.theme.compact ? 1 : 2
    columnSpacing: 6
    rowSpacing: 0

    // Directions pressed: what was opened (`Directions.open`).
    function directions() {
        const at = line.sioul.placeOf(line.address)
        return line.way.open(at !== "" ? JSON.parse(at) : null, line.address)
    }

    Label {
        Layout.fillWidth: line.address !== ""
        Layout.alignment: Qt.AlignTop
        text: line.text
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: line.address !== "" ? line.theme.text : line.theme.muted
    }
    Button {
        objectName: "contactDirections"
        visible: line.address !== ""
        Layout.alignment: Qt.AlignTop | Qt.AlignLeft
        flat: true
        icon.name: "go-next"
        text: line.sioul.text("map-directions")
        onClicked: line.directions()
    }
}
