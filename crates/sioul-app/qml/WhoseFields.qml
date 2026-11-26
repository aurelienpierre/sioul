// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Whose an agenda is, and whose name (docs/client.md, "Several agendas"): the
// one control for it, used on each agenda's line in Agenda ⚙ ▸ Your agendas,
// which writes at once, and in the forms that make a calendar (Agenda ⚙'s
// "New agenda…", "Subscribe to a calendar…", and Settings ▸ Extensions when an
// extension's agenda gets a new calendar), which write the same fields once
// the calendar is made (`agendas::set_field`, `persons::set_agenda_person`).
// One uniform list of roles: yours, a child's, another parent's, a partner's,
// someone else's (a co-worker's, a friend's), shared. A child's, a partner's
// or another parent's: one of your people of that relation, chosen in the
// list or made with "New person…" (the caller opens the person's form);
// someone else's: any of your people, or a name typed, or nobody; a shared
// one's: a name typed. It says what it was told and keeps nothing: the host
// writes, then gives it back its new state.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: fields

    required property var sioul
    required property var theme
    // The persons each role may name, `agenda::setup`'s `persons`: {child: [{id, name}], partner: […], parent: […], someone: […]}.
    property var persons: ({})
    // "yours", "child", "parent", "partner", "someone" or "shared".
    property string whose: "yours"
    // Whose it is as a person, by id; "" none.
    property string personId: ""
    // A name said with no person (an older Sioul's), or a shared agenda's name.
    property string name: ""

    // Its role chosen; one of your people chosen ("" none); a shared one's name typed;
    // "New person…" asked, with the relation the person's form starts from.
    signal whoseChosen(string whose)
    signal personChosen(string id)
    signal nameGiven(string name)
    signal newPersonAsked(string relation)

    readonly property var choices: ["yours", "child", "parent", "partner", "someone", "shared"]
    // The roles that name one of your people (docs/persons.md).
    readonly property bool namesPerson: ["child", "partner", "parent", "someone"].indexOf(fields.whose) >= 0
    // The role's own control shown; off where the role is chosen beside it (an agenda's row,
    // its card then holding whose name alone: AgendaCard.qml).
    property bool roleShown: true

    spacing: fields.theme.labelGap

    Label {
        visible: fields.roleShown
        text: fields.sioul.text("agenda-whose")
        color: fields.theme.text
    }
    PlainComboBox {
        objectName: "agendaWhose"
        visible: fields.roleShown
        Layout.fillWidth: true
        model: fields.choices.map(w => fields.sioul.text("agenda-whose-" + w))
        currentIndex: Math.max(0, fields.choices.indexOf(fields.whose))
        Accessible.name: fields.sioul.text("agenda-whose")
        onActivated: index => {
            if (fields.choices[index] !== fields.whose)
                fields.whoseChosen(fields.choices[index])
        }
    }
    // Whose name: a person of the household (docs/persons.md).
    PersonChooser {
        objectName: "agendaPerson"
        visible: fields.namesPerson
        Layout.fillWidth: true
        sioul: fields.sioul
        theme: fields.theme
        persons: (fields.persons || {})[fields.whose] || []
        personId: fields.personId
        // Someone else's name typed shows in its field below, not here.
        fallback: fields.whose === "someone" ? "" : fields.name
        Accessible.name: fields.sioul.text(fields.whose === "child" ? "agenda-person-child" : "agenda-person")
        onChosen: id => fields.personChosen(id)
        onNewAsked: fields.newPersonAsked(({ child: "child", partner: "partner", parent: "other-parent" })[fields.whose] || "")
    }
    // A shared agenda's, or someone else's with none of your people chosen: whose name, typed.
    TextField {
        objectName: "agendaSharedName"
        visible: fields.whose === "shared" || (fields.whose === "someone" && fields.personId === "")
        Layout.fillWidth: true
        text: fields.name
        placeholderText: fields.sioul.text("agenda-person")
        Accessible.name: placeholderText
        onEditingFinished: if (text.trim() !== fields.name)
            fields.nameGiven(text.trim())
    }
    // What another person's agenda does, said where it is chosen (review of 3346ff96, 6).
    Label {
        objectName: "agendaWhoseTheirs"
        visible: ["partner", "parent", "someone"].indexOf(fields.whose) >= 0
        Layout.fillWidth: true
        text: fields.sioul.text("extension-agenda-theirs")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: fields.theme.muted
    }
}
