// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A site's name, address (https only), type and categories, and whom you
// open it for, changed: on a computer and on Android alike (SitesPage.qml,
// android/SitesPage.qml). Saved, it closes; refused, it stays open with
// why, what you typed kept.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: editing

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The width it keeps within: the page's.
    property real room: editing.roomWidth

    // Saved: the page reads its sites again.
    signal saved()

    // The site changed, as `sioul.siteList()` gives it.
    property var site: null
    // What was refused (an address without https…): said here, the dialog kept open.
    property string problem: ""
    property var persons: []
    property string personId: ""
    readonly property var kinds: ["mailbox", "chat", "video", "social", "dating", "other"]

    function begin(site) {
        editing.site = site
        editName.text = site ? site.name : ""
        editUrl.text = site ? site.url : ""
        editKind.currentIndex = site ? Math.max(0, editing.kinds.indexOf(site.kind)) : 0
        editCategories.text = site ? (site.categories || []).join(", ") : ""
        // Whom you open it for ("Odile's ameli account"): a person of the household, or nobody said.
        editing.persons = (JSON.parse(editing.sioul.household() || "{}").persons || []).filter(p => p.id !== "me")
        editing.personId = ((site ? JSON.parse(editing.sioul.sitePerson(site.id) || "null") : null) || { id: "" }).id
        editing.problem = ""
        editing.open()
    }

    // Saved, it closes; refused, it stays open with why, what you typed kept.
    function save() {
        const id = editing.site ? editing.site.id : ""
        const categories = editCategories.text.split(",").map(c => c.trim()).filter(c => c !== "")
        const problems = [editing.sioul.setSite(id, "name", editName.text), editing.sioul.setSite(id, "url", editUrl.text), editing.sioul.setSite(id, "site", editing.kinds[editKind.currentIndex]), editing.sioul.setSite(id, "categories", JSON.stringify(categories)), editing.sioul.setSiteFor(id, editing.personId)].filter(p => p !== "")
        editing.saved()
        editing.problem = problems.join(" ")
        if (editing.problem === "")
            editing.close()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, Math.min(editing.room, editing.roomWidth) - 2 * editing.theme.gap)
    title: editing.sioul.text("site-edit")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: editing.sioul.text("site-field-name")
            color: editing.theme.muted
        }
        TextField {
            id: editName

            Layout.fillWidth: true
        }
        Label {
            text: editing.sioul.text("site-field-url")
            color: editing.theme.muted
        }
        TextField {
            id: editUrl

            Layout.fillWidth: true
            inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoPredictiveText
            onAccepted: editing.save()
        }
        Label {
            text: editing.sioul.text("site-field-kind")
            color: editing.theme.muted
        }
        PlainComboBox {
            id: editKind

            Layout.fillWidth: true
            model: editing.kinds.map(k => editing.sioul.text("site-kind-" + k))
        }
        Label {
            text: editing.sioul.text("site-field-categories")
            color: editing.theme.muted
        }
        TextField {
            id: editCategories

            Layout.fillWidth: true
            placeholderText: editing.sioul.text("site-categories-help")
            onAccepted: editing.save()
        }
        Label {
            visible: editing.persons.length > 0
            text: editing.sioul.text("site-field-for")
            color: editing.theme.muted
        }
        PlainComboBox {
            readonly property var choices: [{ id: "", name: editing.sioul.text("site-for-you") }].concat(editing.persons)

            visible: editing.persons.length > 0
            Layout.fillWidth: true
            model: choices.map(c => editing.theme.plain(c.name))
            currentIndex: Math.max(0, choices.findIndex(c => c.id === editing.personId))
            Accessible.name: editing.sioul.text("site-field-for")
            onActivated: index => editing.personId = choices[index].id
        }
        Label {
            visible: editing.problem !== ""
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: editing.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: editing.theme.warm
        }
    }

    // Out of the footer's own box: a box there closes the dialog on "Save"
    // before saving could be refused. Sioul's own words, translated.
    footer: Item {
        implicitWidth: editButtons.implicitWidth
        implicitHeight: editButtons.implicitHeight

        DialogButtonBox {
            id: editButtons

            anchors.fill: parent

            Button {
                text: editing.sioul.text("ui-save")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: editing.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onAccepted: editing.save()
            onRejected: editing.close()
        }
    }
}
