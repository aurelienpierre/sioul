// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Do not disturb, under its settings (docs/do-not-disturb.md):
// what this device's system lets Sioul do, with the pages it needs; the list
// of people who may reach you during do-not-disturb, the same on every
// device; on a phone, who on it is starred (Sioul never stars anyone: it
// opens their contact), and Sioul kept in step in the background.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme
    // everywhere.rs's `setup`: {here: {on, line, lines, consent, offers}, gnome, people, problem, said, android, stars, steps}.
    property var shown: ({ here: { line: "", lines: [], consent: "", offers: [] }, gnome: false, people: [], problem: "", said: "", android: false })
    // The person being written by hand, or changed: its id ("" new), or none.
    property var editing: null
    property var found: []
    property string none: ""

    function reload() {
        setup.shown = JSON.parse(setup.sioul.dndSetup() || "null") || setup.shown
    }

    // Someone to write by hand ("" new) or to change: the fields filled anew.
    function edit(person) {
        setup.editing = person
        nameField.text = person ? person.name : ""
        phonesField.text = person ? person.phones : ""
        emailsField.text = person ? person.emails : ""
    }

    function act(verb, args) {
        const answer = JSON.parse(setup.sioul.dndChange(verb, JSON.stringify(args || {})) || "null")
        if (answer)
            setup.shown = answer
    }

    spacing: 6
    Component.onCompleted: setup.reload()
    onVisibleChanged: if (visible) setup.reload()

    // ---------------------------------------------------------------- this device
    Label {
        Layout.fillWidth: true
        Layout.topMargin: 18
        text: setup.sioul.text("dnd-setup-here")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: setup.theme.accent
    }
    Repeater {
        model: [setup.shown.here.line].concat(setup.shown.here.lines || []).filter(l => l !== "")

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: setup.theme.text
        }
    }
    Flow {
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: (setup.shown.here.offers || []).filter(o => o.key !== "desktop")

            delegate: Button {
                id: offer

                required property var modelData

                width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                text: offer.modelData.label
                onClicked: setup.act("open", { key: offer.modelData.key })
            }
        }
    }
    // GNOME: Sioul switches its Do Not Disturb only with this yes, the same as the pauses'.
    WrapCheckBox {
        visible: (setup.shown.here.offers || []).some(o => o.key === "desktop")
        Layout.fillWidth: true
        text: setup.shown.here.consent || ""
        checked: setup.shown.gnome === true
        onToggled: setup.act("gnome", { on: checked })
    }

    // ---------------------------------------------------------------- the list
    Label {
        Layout.fillWidth: true
        Layout.topMargin: 18
        text: setup.sioul.text("dnd-setup-list")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: setup.theme.accent
    }
    Label {
        Layout.fillWidth: true
        text: setup.sioul.text("dnd-setup-list-help")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.problem !== "" || setup.shown.said !== ""
        Layout.fillWidth: true
        text: setup.shown.problem !== "" ? setup.shown.problem : setup.shown.said
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        visible: setup.shown.people.length === 0
        Layout.fillWidth: true
        text: setup.sioul.text("dnd-setup-empty")
        color: setup.theme.muted
    }
    Repeater {
        model: setup.shown.people

        delegate: ColumnLayout {
            id: person

            required property var modelData

            Layout.fillWidth: true
            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    text: person.modelData.name !== "" ? person.modelData.name : (person.modelData.emails[0] || person.modelData.phones[0] || "")
                    textFormat: Text.PlainText
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    color: setup.theme.text
                }
                Button {
                    flat: true
                    text: setup.sioul.text("dnd-setup-edit")
                    onClicked: setup.edit({ id: person.modelData.id, name: person.modelData.name, phones: person.modelData.phones.join("\n"), emails: person.modelData.emails.join("\n") })
                }
                Button {
                    flat: true
                    text: setup.sioul.text("dnd-setup-remove")
                    onClicked: setup.act("remove", { id: person.modelData.id })
                }
            }
            Label {
                Layout.fillWidth: true
                text: person.modelData.phones.concat(person.modelData.emails).join(" · ")
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                color: setup.theme.muted
            }
            Repeater {
                model: person.modelData.notes

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: setup.theme.muted
                    font.pixelSize: 12
                }
            }
        }
    }
    Flow {
        Layout.fillWidth: true
        Layout.topMargin: 6
        spacing: 6

        Button {
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            text: setup.sioul.text("dnd-setup-add-safe")
            onClicked: setup.act("add-safe", {})
        }
        Button {
            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
            text: setup.sioul.text("dnd-setup-add-person")
            onClicked: setup.edit({ id: "", name: "", phones: "", emails: "" })
        }
    }
    // A contact picked from your address books.
    TextField {
        id: search

        Layout.fillWidth: true
        placeholderText: setup.sioul.text("dnd-setup-add-contact")
        Accessible.name: setup.sioul.text("dnd-setup-search")
        onTextEdited: {
            if (search.text.trim().length < 2) {
                setup.found = []
                setup.none = ""
                return
            }
            const answer = JSON.parse(setup.sioul.dndChange("search", JSON.stringify({ query: search.text })) || "{}")
            setup.found = answer.found || []
            setup.none = answer.none || ""
        }
    }
    Label {
        visible: setup.none !== "" && search.text.trim().length >= 2
        Layout.fillWidth: true
        text: setup.none
        color: setup.theme.muted
    }
    Repeater {
        model: setup.found

        delegate: Button {
            id: contact

            required property var modelData

            Layout.fillWidth: true
            flat: true
            text: contact.modelData.name + (contact.modelData.detail !== "" ? " · " + contact.modelData.detail : "")
            contentItem: Label {
                text: contact.text
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: setup.theme.text
            }
            onClicked: {
                setup.act("add-contact", { uid: contact.modelData.uid })
                search.text = ""
                setup.found = []
            }
        }
    }
    // Someone written by hand, or changed: a name, numbers and addresses, one per line.
    ColumnLayout {
        visible: setup.editing !== null
        Layout.fillWidth: true
        spacing: 4

        TextField {
            id: nameField

            Layout.fillWidth: true
            placeholderText: setup.sioul.text("dnd-setup-name")
            Accessible.name: setup.sioul.text("dnd-setup-name")
        }
        TextArea {
            id: phonesField

            Layout.fillWidth: true
            placeholderText: setup.sioul.text("dnd-setup-phones")
            Accessible.name: setup.sioul.text("dnd-setup-phones")
            wrapMode: TextArea.Wrap
            background: Rectangle {
                color: setup.theme.surface
                radius: setup.theme.radius
                border.color: phonesField.activeFocus ? setup.theme.focus : setup.theme.line
            }
        }
        TextArea {
            id: emailsField

            Layout.fillWidth: true
            placeholderText: setup.sioul.text("dnd-setup-emails")
            Accessible.name: setup.sioul.text("dnd-setup-emails")
            wrapMode: TextArea.Wrap
            background: Rectangle {
                color: setup.theme.surface
                radius: setup.theme.radius
                border.color: emailsField.activeFocus ? setup.theme.focus : setup.theme.line
            }
        }
        RowLayout {
            spacing: 6

            Button {
                text: setup.sioul.text("dnd-setup-save")
                onClicked: {
                    const editing = setup.editing
                    setup.act(editing.id === "" ? "add" : "edit", { id: editing.id, name: nameField.text, phones: phonesField.text, emails: emailsField.text })
                    setup.edit(null)
                }
            }
            Button {
                flat: true
                text: setup.sioul.text("dnd-setup-cancel")
                onClicked: setup.edit(null)
            }
        }
    }

    // ---------------------------------------------------------------- on a phone: starred, and in the background
    Label {
        visible: setup.shown.android === true
        Layout.fillWidth: true
        Layout.topMargin: 18
        text: setup.sioul.text("dnd-stars-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: setup.theme.accent
    }
    Label {
        visible: setup.shown.android === true && setup.shown.stars !== undefined
        Layout.fillWidth: true
        text: setup.shown.stars ? setup.shown.stars.summary : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Label {
        visible: setup.shown.android === true && setup.shown.stars !== undefined && setup.shown.stars.permission === true && setup.shown.stars.missing > 0
        Layout.fillWidth: true
        text: setup.sioul.text("dnd-stars-why")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Button {
        visible: setup.shown.android === true && setup.shown.stars !== undefined && setup.shown.stars.permission !== true
        text: setup.sioul.text("dnd-stars-allow")
        onClicked: setup.act("allow-contacts", {})
    }
    Repeater {
        model: setup.shown.android === true && setup.shown.stars ? setup.shown.stars.people : []

        delegate: RowLayout {
            id: starred

            required property var modelData

            Layout.fillWidth: true
            spacing: 6

            Label {
                Layout.fillWidth: true
                text: starred.modelData.name + " · " + setup.sioul.text("dnd-stars-" + starred.modelData.state)
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: starred.modelData.state === "starred" ? setup.theme.muted : setup.theme.text
            }
            Button {
                visible: starred.modelData.state === "not-starred" && starred.modelData.contact !== ""
                text: setup.sioul.text("dnd-stars-open")
                onClicked: setup.act("open-contact", { contact: starred.modelData.contact })
            }
            Button {
                visible: starred.modelData.state === "unknown"
                text: setup.sioul.text("dnd-stars-add")
                onClicked: setup.act("add-to-contacts", { id: starred.modelData.id })
            }
        }
    }
    Button {
        visible: setup.shown.android === true
        flat: true
        text: setup.sioul.text("dnd-stars-again")
        onClicked: setup.reload()
    }
    Label {
        visible: setup.shown.android === true
        Layout.fillWidth: true
        text: setup.sioul.text("dnd-stars-repeat")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }

    Label {
        visible: setup.shown.android === true
        Layout.fillWidth: true
        Layout.topMargin: 18
        text: setup.sioul.text("dnd-steps-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: setup.theme.accent
    }
    WrapCheckBox {
        visible: setup.shown.android === true
        Layout.fillWidth: true
        text: setup.sioul.text("set-dnd-background")
        checked: setup.shown.steps ? setup.shown.steps.on === true : false
        onToggled: setup.act("background", { on: checked })
    }
    Label {
        visible: setup.shown.android === true
        Layout.fillWidth: true
        text: setup.sioul.text("set-dnd-background-help")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.shown.android === true && setup.shown.steps !== undefined
        Layout.fillWidth: true
        text: setup.shown.steps ? setup.shown.steps.line + " " + setup.shown.steps.battery_line : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    Button {
        visible: setup.shown.android === true && setup.shown.steps !== undefined && setup.shown.steps.battery !== true
        Layout.bottomMargin: 24
        text: setup.sioul.text("dnd-steps-allow")
        onClicked: setup.act("battery", {})
    }
}
