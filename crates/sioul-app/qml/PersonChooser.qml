// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A person of the household chosen for something (docs/persons.md): an
// agenda's "Whose name", a child agenda's "The other parent". Nobody said
// first (or the name an older text gave, when it names nobody here), then the
// persons of the right relation, then "New person…", which the caller opens.

import QtQuick

PlainComboBox {
    id: chooser

    required property var sioul
    required property var theme
    // {id, name}, those this may be.
    property var persons: []
    // The person chosen now, by id; "" none.
    property string personId: ""
    // A name said with no person (written by an older Sioul): shown as it is.
    property string fallback: ""

    signal chosen(string id)
    signal newAsked

    readonly property var entries: [{ id: "", name: chooser.personId === "" && chooser.fallback !== "" ? chooser.fallback : chooser.sioul.text("agenda-person-none") }].concat(chooser.persons).concat([{ id: "+", name: chooser.sioul.text("person-card-new") }])

    model: chooser.entries.map(e => chooser.theme.plain(e.name))
    currentIndex: Math.max(0, chooser.entries.findIndex(e => e.id === chooser.personId))
    onActivated: index => {
        const entry = chooser.entries[index]
        if (entry.id === "+") {
            chooser.currentIndex = Qt.binding(() => Math.max(0, chooser.entries.findIndex(e => e.id === chooser.personId)))
            chooser.newAsked()
        } else if (entry.id !== chooser.personId) {
            chooser.chosen(entry.id)
        }
    }
}
