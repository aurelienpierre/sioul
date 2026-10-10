// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A contact's form: the name, addresses and numbers in view, each with what
// it is for; the organisation; its categories, chosen among those in use or
// typed; then, folded, postal addresses, the birthday, notes and web sites.
// `edit()` gives what the backend saves.

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
    // The categories the cards have (the page's list), to choose from.
    property var known: []
    // This contact's categories, as edited.
    property var chosen: []
    // Those in use it does not have yet, to choose from.
    readonly property var others: form.known.filter(k => !form.has(k))

    // Categories compare as the sender lists compare them: case and accents aside.
    function folded(name) {
        return name.trim().toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "")
    }

    function has(name) {
        return form.chosen.some(c => form.folded(c) === form.folded(name))
    }

    // The categories with one more: the spelling already in use when there is one.
    function plus(list, name) {
        const typed = name.trim()
        if (typed === "" || list.some(c => form.folded(c) === form.folded(typed)))
            return list
        const used = form.known.find(k => form.folded(k) === form.folded(typed))
        return list.concat([used !== undefined ? used : typed])
    }

    function addCategory(name) {
        form.chosen = form.plus(form.chosen, name)
    }

    function load(person) {
        name.text = person.name
        org.text = person.org
        title.text = person.title
        birthday.text = person.birthday
        notes.text = person.notes
        urls.text = person.urls.join("\n")
        form.chosen = (person.categories || []).slice()
        newCategory.clear()
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
            urls: urls.text.split("\n").map(u => u.trim()).filter(u => u),
            // A category typed and not yet added counts too.
            categories: form.plus(form.chosen, newCategory.text)
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

    Label {
        text: form.sioul.text("contact-categories")
        color: form.theme.muted
    }
    // Its categories: each taken off with ×, another chosen among those in use or typed.
    Flow {
        visible: form.chosen.length > 0
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: form.chosen

            delegate: Rectangle {
                id: tag

                required property string modelData

                implicitWidth: tagRow.implicitWidth + 12
                implicitHeight: tagRow.implicitHeight + 4
                radius: height / 2
                color: "transparent"
                border.color: form.theme.line

                Row {
                    id: tagRow

                    anchors.centerIn: parent
                    spacing: 2

                    Label {
                        anchors.verticalCenter: parent.verticalCenter
                        leftPadding: 4
                        text: tag.modelData
                        textFormat: Text.PlainText
                        font.pixelSize: 13
                        color: form.theme.text
                    }
                    ToolButton {
                        implicitWidth: 22
                        implicitHeight: 22
                        text: "×"
                        Accessible.name: form.sioul.textWith("contact-category-remove", "category", tag.modelData)
                        onClicked: form.chosen = form.chosen.filter(c => c !== tag.modelData)
                    }
                }
            }
        }
    }
    // Another category: typed, or chosen among those in use (▾).
    RowLayout {
        Layout.fillWidth: true
        spacing: 4

        TextField {
            id: newCategory

            Layout.fillWidth: true
            Layout.maximumWidth: 260
            placeholderText: "+ " + form.sioul.text("contact-category-add")
            Accessible.name: form.sioul.text("contact-category-add")
            onAccepted: {
                form.addCategory(newCategory.text)
                newCategory.clear()
            }
        }
        ToolButton {
            id: usedButton

            visible: form.others.length > 0
            text: "▾"
            Accessible.name: form.sioul.text("contact-category-used")
            ToolTip.visible: hovered
            ToolTip.text: form.sioul.text("contact-category-used")
            onClicked: usedMenu.now().popup(usedButton, 0, usedButton.height)
        }
        Item {
            Layout.fillWidth: true
        }
    }
    Later {
        id: usedMenu

        sourceComponent: Component {
            SioulMenu {
                id: menu

                Instantiator {
                    model: form.others

                    delegate: MenuItem {
                        id: usedLine

                        required property string modelData

                        // "&" marks a shortcut in a menu: "&&" is one.
                        text: form.theme.plain(usedLine.modelData.replace(/&/g, "&&"))
                        onTriggered: form.addCategory(usedLine.modelData)
                    }
                    onObjectAdded: (index, object) => menu.insertItem(index, object)
                    onObjectRemoved: (index, object) => menu.removeItem(object)
                }
            }
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
