// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A contact's form: the name, addresses and numbers in view, each with what
// it is for; the organisation; then, folded, postal addresses, the birthday,
// notes and web sites. `edit()` gives what the backend saves.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: form

    required property var sioul
    required property var theme
    property bool moreShown: false
    readonly property var labels: ["", "work", "home", "cell"]

    function load(person) {
        name.text = person.name
        org.text = person.org
        title.text = person.title
        birthday.text = person.birthday
        notes.text = person.notes
        urls.text = person.urls.join("\n")
        emails.clear()
        for (const e of person.emails)
            emails.append({ label: e.label.split(",")[0].trim(), value: e.value })
        if (emails.count === 0)
            emails.append({ label: "", value: "" })
        phones.clear()
        for (const p of person.phones)
            phones.append({ label: p.label.split(",")[0].trim(), value: p.value })
        if (phones.count === 0)
            phones.append({ label: "", value: "" })
        addresses.clear()
        for (const a of person.addresses)
            addresses.append({ label: a.label.split(",")[0].trim(), value: a.value })
        form.moreShown = person.addresses.length > 0 || person.birthday !== "" || person.notes !== "" || person.urls.length > 0
        name.forceActiveFocus()
    }

    function rows(model) {
        const out = []
        for (let i = 0; i < model.count; i++)
            if (model.get(i).value.trim() !== "")
                out.push({ label: model.get(i).label, value: model.get(i).value })
        return out
    }

    function edit() {
        return {
            name: name.text,
            emails: form.rows(emails),
            phones: form.rows(phones),
            org: org.text,
            title: title.text,
            addresses: form.rows(addresses),
            birthday: birthday.text,
            notes: notes.text,
            urls: urls.text.split("\n").map(u => u.trim()).filter(u => u)
        }
    }

    spacing: 8

    ListModel {
        id: emails
    }
    ListModel {
        id: phones
    }
    ListModel {
        id: addresses
    }

    Label {
        text: form.sioul.text("contact-name")
        color: form.theme.muted
    }
    TextField {
        id: name

        Layout.fillWidth: true
    }
    Label {
        text: form.sioul.text("contact-emails")
        color: form.theme.muted
    }
    LabeledRows {
        sioul: form.sioul
        theme: form.theme
        labels: form.labels
        model: emails
        addText: form.sioul.text("contact-add-email")
    }
    Label {
        text: form.sioul.text("contact-phones")
        color: form.theme.muted
    }
    LabeledRows {
        sioul: form.sioul
        theme: form.theme
        labels: form.labels
        model: phones
        addText: form.sioul.text("contact-add-phone")
    }
    Label {
        text: form.sioul.text("contact-org")
        color: form.theme.muted
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 6

        TextField {
            id: org

            Layout.fillWidth: true
            placeholderText: form.sioul.text("contact-org")
        }
        TextField {
            id: title

            Layout.fillWidth: true
            placeholderText: form.sioul.text("contact-title")
        }
    }

    Button {
        flat: true
        text: (form.moreShown ? "▾  " : "▸  ") + form.sioul.text("ui-more-details")
        onClicked: form.moreShown = !form.moreShown
    }
    ColumnLayout {
        visible: form.moreShown
        Layout.fillWidth: true
        spacing: 8

        Label {
            text: form.sioul.text("contact-address")
            color: form.theme.muted
        }
        LabeledRows {
            sioul: form.sioul
            theme: form.theme
            labels: form.labels
            model: addresses
            multiline: true
            addText: form.sioul.text("contact-add-address")
        }
        Label {
            text: form.sioul.text("contact-birthday")
            color: form.theme.muted
        }
        TextField {
            id: birthday

            Layout.preferredWidth: 160
            placeholderText: "1984-05-12"
        }
        Label {
            text: form.sioul.text("contact-notes")
            color: form.theme.muted
        }
        TextArea {
            id: notes

            // A field shows where it is: a border, darker when it has the focus.
            background: Rectangle {
                color: form.theme.surface
                radius: form.theme.radius
                border.color: notes.activeFocus ? form.theme.focus : form.theme.line
            }
            Layout.fillWidth: true
            Layout.preferredHeight: 90
            wrapMode: TextArea.Wrap
        }
        Label {
            text: form.sioul.text("contact-web")
            color: form.theme.muted
        }
        TextArea {
            id: urls

            // A field shows where it is: a border, darker when it has the focus.
            background: Rectangle {
                color: form.theme.surface
                radius: form.theme.radius
                border.color: urls.activeFocus ? form.theme.focus : form.theme.line
            }
            Layout.fillWidth: true
            Layout.preferredHeight: 60
            placeholderText: "https://…"
        }
    }
}
