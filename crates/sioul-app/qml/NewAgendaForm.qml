// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The form that makes a calendar, with its role (several agendas, part 5):
// its account, its name, whether it holds events, tasks or both, its colour,
// whose it is (WhoseFields.qml, the control of each agenda's line), what it
// is for and when a personal one reaches you. Used by Agenda ⚙ ▸ Your agendas
// ("New agenda…") and by Settings ▸ Extensions, when an extension's agenda
// gets a new calendar (its account and its name proposed, events only). Made
// here and on its server by the next sync (`servers::make`, which writes the
// role's fields, `agendas::set_field`); a person chosen in the list is
// written as the line writes it (`setAgendaPerson`). `made` gives its place.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: form

    required property var sioul
    required property var theme
    // [{id, name, google}]: the accounts a calendar can be made on (`agenda::setup`'s `accounts`).
    property var accounts: []
    // The persons each role may name (`agenda::setup`'s `persons`).
    property var persons: ({})
    // For an extension's agenda: events alone, its tasks not offered.
    property bool eventsOnly: false
    property string problem: ""
    property string colour: form.colours[0]
    // Whose it is, until Make: the role, the person chosen, a shared one's name.
    property string whose: "yours"
    property string personId: ""
    property string name: ""
    readonly property var colours: ["#4c6b5c", "#3f6f8f", "#7a5c99", "#b07d3f", "#a3504f", "#5f7f3a", "#5b6470", "#c06c8d"]
    readonly property var account: form.accounts[accountChoice.currentIndex] || null

    // Made: its place. Kept: nothing made.
    signal made(string place)
    signal kept

    // Opened afresh: on `account` (by id, "" the first) and named `name`.
    function begin(account, name) {
        accountChoice.currentIndex = Math.max(0, form.accounts.findIndex(a => a.id === account))
        newName.text = name || ""
        newEvents.checked = true
        newTasks.checked = false
        form.colour = form.colours[0]
        form.whose = "yours"
        form.personId = ""
        form.name = ""
        newWork.checked = false
        newAdmin.checked = true
        newLeisure.checked = true
        newReaches.currentIndex = 0
        form.problem = ""
    }

    // Sioul's window, with its own properties (its locale), for the person's form.
    function sioulWindow(): var {
        return form.Window.window
    }

    function make() {
        const areas = [[newWork, "work"], [newAdmin, "admin"], [newLeisure, "leisure"]].filter(p => p[0].checked).map(p => p[1]).join("+")
        const named = form.whose !== "yours" && form.personId === ""
        const answer = JSON.parse(form.sioul.newAgenda(JSON.stringify({ account: form.account.id, name: newName.text.trim(), colour: form.colour, events: form.eventsOnly || newEvents.checked, tasks: !form.eventsOnly && newTasks.enabled && newTasks.checked, whose: form.whose, person: named ? form.name : "", area: areas, reaches: newWork.checked || newReaches.currentIndex === 0 ? "" : newReaches.choices[newReaches.currentIndex] })) || "{}")
        form.problem = answer.problem || ""
        if (form.problem !== "")
            return
        // One of your people chosen: written as the agenda's line writes it.
        if (form.whose !== "yours" && form.personId !== "")
            form.problem = form.sioul.setAgendaPerson(answer.place, "person", form.personId) || ""
        newName.text = ""
        form.made(answer.place)
    }

    spacing: 4

    Label {
        text: form.sioul.text("agenda-new-account")
        color: form.theme.text
    }
    PlainComboBox {
        id: accountChoice

        objectName: "agendaNewAccount"
        Layout.fillWidth: true
        model: form.accounts.map(a => form.theme.plain(a.name))
        Accessible.name: form.sioul.text("agenda-new-account")
    }
    TextField {
        id: newName

        objectName: "agendaNewName"
        Layout.fillWidth: true
        placeholderText: form.sioul.text("agenda-new-name")
        Accessible.name: placeholderText
    }
    Flow {
        visible: !form.eventsOnly
        Layout.fillWidth: true
        spacing: 12

        CheckBox {
            id: newEvents

            text: form.sioul.text("agenda-new-events")
            checked: true
        }
        CheckBox {
            id: newTasks

            // Google's task lists are Google Tasks': a Google agenda holds events.
            enabled: !(form.account && form.account.google)
            text: form.sioul.text("agenda-new-tasks")
        }
    }
    Flow {
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: form.colours

            delegate: Rectangle {
                id: swatch

                required property string modelData

                width: 26
                height: 26
                radius: 13
                color: swatch.modelData
                border.width: form.colour === swatch.modelData ? 3 : 0
                border.color: form.theme.text
                Accessible.role: Accessible.Button
                Accessible.name: form.sioul.text("agenda-colour") + " " + swatch.modelData

                TapHandler {
                    onTapped: form.colour = swatch.modelData
                }
            }
        }
    }
    // Whose it is: the control of each agenda's line, its answers kept until Make.
    WhoseFields {
        objectName: "agendaNewWhose"
        Layout.fillWidth: true
        sioul: form.sioul
        theme: form.theme
        persons: form.persons
        whose: form.whose
        personId: form.personId
        name: form.name
        onWhoseChosen: whose => {
            form.whose = whose
            form.personId = ""
        }
        onPersonChosen: id => form.personId = id
        onNameGiven: name => form.name = name
        onNewPersonAsked: relation => {
            const household = JSON.parse(form.sioul.household() || "{}")
            const dialog = personForm.now()
            dialog.relations = household.relations || []
            dialog.children = (household.persons || []).filter(p => p.relation === "child").map(p => ({ id: p.id, name: p.name }))
            dialog.begin("", "", relation, [])
        }
    }
    Flow {
        id: newAreas

        Layout.fillWidth: true
        spacing: 12

        CheckBox {
            id: newWork

            text: form.sioul.text("area-work")
        }
        CheckBox {
            id: newAdmin

            text: form.sioul.text("area-admin")
            checked: true
        }
        CheckBox {
            id: newLeisure

            text: form.sioul.text("area-leisure")
            checked: true
        }
    }
    // A personal one: all day, or outside work hours; one for work follows the work hours.
    PlainComboBox {
        id: newReaches

        readonly property var choices: ["all-day", "outside-work"]

        objectName: "agendaNewReaches"
        visible: !newWork.checked
        Layout.fillWidth: true
        model: choices.map(r => form.sioul.text("agenda-reaches-" + r))
        Accessible.name: form.sioul.text("agenda-reaches")
    }
    Label {
        visible: form.problem !== ""
        Layout.fillWidth: true
        text: form.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: form.theme.warm
    }
    RowLayout {
        spacing: 8

        Button {
            objectName: "agendaNewMake"
            enabled: newName.text.trim() !== "" && form.account !== null
            text: form.sioul.text("agenda-new-make")
            onClicked: form.make()
        }
        Button {
            objectName: "agendaNewKeep"
            flat: true
            text: form.sioul.text("agenda-keep")
            onClicked: form.kept()
        }
    }

    // A person's form, from "New person…": chosen for the new agenda once saved.
    Later {
        id: personForm

        sourceComponent: Component {
            PersonDialog {
                sioul: form.sioul
                theme: form.theme
                window: form.sioulWindow()
                onSaved: id => {
                    if (id !== "")
                        form.personId = id
                }
            }
        }
    }
}
