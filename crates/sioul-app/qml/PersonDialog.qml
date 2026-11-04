// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A person of the household, new or changed (docs/persons.md): their name,
// how papers write it, who they are to you, their date of birth (the ages
// papers follow, nothing else), whether they live with you, and for a
// child's other parent, whose. A new name rewrites the papers' texts that
// named the old one.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: form

    required property var sioul
    required property var theme
    required property var window
    // {id, label}, from persons::household.
    property var relations: []
    // The household's children, {id, name}: whose other parent this may be.
    property var children: []
    property string personId: ""
    // A card to link at once (a contact's "This is a person in your household…").
    property string card: ""
    property string problem: ""
    property var chosenOf: []

    // Saved: the person's id.
    signal saved(string id)

    // A new person, perhaps from a contact's card (its name, its UID).
    function begin(name, card, relation) {
        form.personId = ""
        form.card = card || ""
        form.problem = ""
        nameField.text = name || ""
        fullField.text = ""
        relationChoice.currentIndex = Math.max(0, form.relations.findIndex(r => r.id === (relation || "")))
        bornField.date = ""
        livesBox.checked = false
        form.chosenOf = []
        form.open()
        nameField.forceActiveFocus()
    }

    // A person's page's record, changed.
    function edit(person) {
        form.personId = person.id
        form.card = ""
        form.problem = ""
        nameField.text = person.name
        fullField.text = person.full_name || ""
        relationChoice.currentIndex = Math.max(0, form.relations.findIndex(r => r.id === person.relation))
        bornField.date = person.born || ""
        livesBox.checked = person.lives_with === true
        form.chosenOf = (person.of || []).map(c => c.id)
        form.open()
        nameField.forceActiveFocus()
    }

    readonly property string relation: form.relations.length > 0 && relationChoice.currentIndex >= 0 ? form.relations[relationChoice.currentIndex].id : ""

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(480, (parent ? form.roomWidth : 480) - 2 * form.theme.gap)
    title: form.sioul.text(form.personId === "" ? "person-new" : "person-change")

    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(grid.implicitHeight, (form.parent ? form.parent.height : 700) - 200)
        contentWidth: availableWidth
        clip: true

        GridLayout {
            id: grid

            width: scroll.availableWidth
            columns: 2
            columnSpacing: 10
            rowSpacing: 8

            Label {
                text: form.sioul.text("person-name")
                color: form.theme.muted
            }
            TextField {
                id: nameField

                objectName: "personName"
                Layout.fillWidth: true
                placeholderText: form.sioul.text("person-name-hint")
            }
            Label {
                text: form.sioul.text("person-relation")
                color: form.theme.muted
            }
            PlainComboBox {
                id: relationChoice

                Layout.fillWidth: true
                model: form.relations.map(r => form.theme.plain(r.label))
            }
            Label {
                visible: form.relation === "other-parent" && form.children.length > 0
                Layout.alignment: Qt.AlignTop
                text: form.sioul.text("person-of")
                color: form.theme.muted
            }
            Flow {
                visible: form.relation === "other-parent" && form.children.length > 0
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: form.children

                    delegate: CheckBox {
                        id: child

                        required property var modelData

                        text: form.theme.plain(child.modelData.name)
                        checked: form.chosenOf.indexOf(child.modelData.id) >= 0
                        onToggled: form.chosenOf = child.checked ? form.chosenOf.concat([child.modelData.id]) : form.chosenOf.filter(id => id !== child.modelData.id)
                    }
                }
            }
            Label {
                text: form.sioul.text("person-full-name")
                color: form.theme.muted
            }
            TextField {
                id: fullField

                Layout.fillWidth: true
                placeholderText: form.sioul.text("person-full-name-hint")
            }
            Label {
                text: form.sioul.text("person-born")
                color: form.theme.muted
            }
            DateField {
                id: bornField

                theme: form.theme
                sioul: form.sioul
                locale: form.window.sioulLocale || Qt.locale()
                pickLabel: form.sioul.text("event-pick-day")
            }
            Item {
                implicitWidth: 1
            }
            CheckBox {
                id: livesBox

                Layout.fillWidth: true
                text: form.sioul.text("person-lives-with-label")
            }
            Label {
                visible: form.problem !== ""
                Layout.columnSpan: 2
                Layout.fillWidth: true
                text: form.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: form.theme.warm
            }
            RowLayout {
                Layout.columnSpan: 2
                Layout.fillWidth: true
                spacing: 8

                Item {
                    Layout.fillWidth: true
                }
                Button {
                    text: form.sioul.text("ui-cancel")
                    onClicked: form.close()
                }
                Button {
                    objectName: "personSave"
                    highlighted: true
                    text: form.sioul.text("ui-save")
                    onClicked: {
                        const edit = { name: nameField.text, full_name: fullField.text, relation: form.relation, born: bornField.date, lives_with: livesBox.checked, of: form.relation === "other-parent" ? form.chosenOf : [], card: form.card }
                        const answer = JSON.parse(form.sioul.savePerson(form.personId, JSON.stringify(edit)) || "{}")
                        if (answer.error) {
                            form.problem = answer.error
                            return
                        }
                        form.close()
                        form.saved(answer.id || "")
                    }
                }
            }
        }
    }
}
