// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A person's page, in the place of a contact's card (docs/persons.md): their
// name and one line ("Child · 12 · lives with you"), then each section
// only when it holds something: their papers with where each stands, and a
// paper to add for them; the contact cards that are them, and one to link;
// at the foot, quiet, "Open to AI agents". One "?" says what a person is.
// On a phone it takes the page, as a card does.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: page

    required property var sioul
    required property var window
    property string personId: ""
    // persons::page: {id, me, name, full_name, relation, line, born, lives_with, agents, holds_papers, of, papers, cards}.
    property var shown: null
    property string problem: ""
    property bool helpShown: false

    signal closeAsked
    signal editAsked(var person)
    signal openContact(string key)
    signal changed
    // "What Sioul keeps about…" and "Ends…": the page holding this one asks where (ContactsPage.qml).
    signal keepsAsked(var person)
    signal endAsked(var person)

    // Read again; `kept`, for a change heard from elsewhere: what went wrong stays
    // said, and so does the hand-over line while the page shows that person.
    function reload(kept) {
        const before = page.shown
        page.shown = page.personId !== "" ? JSON.parse(page.sioul.personPage(page.personId) || "null") : null
        if (kept !== true)
            page.problem = ""
        if (kept === true && before && page.shown && before.id === page.shown.id && (before.handover || "") !== "") {
            page.shown = Object.assign({}, page.shown, { handover: before.handover, handover_age: before.handover_age })
            return
        }
        // A hand-over line shown is said for good: never again, on any device.
        if (page.shown && (page.shown.handover || "") !== "")
            page.sioul.personHandoverSaid(page.personId, page.shown.handover_age)
    }

    onPersonIdChanged: page.reload()

    // The person, or their agendas, changed here or on another device: read again.
    Connections {
        target: page.sioul

        function onPersonsChanged() {
            if (page.personId !== "")
                page.reload(true)
        }

        function onAgendasChanged() {
            if (page.personId !== "")
                page.reload(true)
        }

        // Their papers, letters, budgets, sites and ties.
        function onPapersChanged() {
            if (page.personId !== "")
                page.reload(true)
        }

        function onLettersChanged() {
            if (page.personId !== "")
                page.reload(true)
        }

        function onMoneyChanged() {
            if (page.personId !== "")
                page.reload(true)
        }

        function onSitesChanged() {
            if (page.personId !== "")
                page.reload(true)
        }

        function onLinksChanged() {
            if (page.personId !== "")
                page.reload(true)
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 10

        ScrollView {
            id: scroll

            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth

            ColumnLayout {
                width: scroll.availableWidth
                spacing: 8

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        objectName: "personTitle"
                        Layout.fillWidth: true
                        text: page.shown ? page.shown.name : ""
                        textFormat: Text.PlainText
                        font.pixelSize: 22
                        wrapMode: Text.Wrap
                        color: page.theme.text
                    }
                    // Their ties as a chain (ChainPage.qml): papers, letters, agendas, cards, sites.
                    ToolButton {
                        objectName: "personTies"
                        visible: page.shown !== null && (page.shown.id || "") !== ""
                        icon.name: "view-list-tree"
                        icon.color: page.theme.text
                        Accessible.name: page.sioul.text("ui-ties")
                        ToolTip.visible: hovered
                        ToolTip.text: page.sioul.text("ui-ties")
                        ToolTip.delay: 300
                        onClicked: page.window.openChain("sioul:person/" + encodeURIComponent(page.shown.id))
                    }
                    ToolButton {
                        objectName: "personHelp"
                        text: "?"
                        checkable: true
                        checked: page.helpShown
                        Accessible.name: page.sioul.text("persons-household")
                        ToolTip.visible: hovered && !page.helpShown
                        ToolTip.text: page.theme.plain(page.sioul.text("person-help"))
                        ToolTip.delay: 300
                        onClicked: page.helpShown = !page.helpShown
                    }
                }
                Label {
                    visible: page.shown !== null && page.shown.line !== ""
                    Layout.fillWidth: true
                    text: page.shown ? page.shown.line : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                // Once, at 16 and at 18, in an admin window: what passes to them; at 18, what Sioul keeps can be handed over.
                Label {
                    objectName: "personHandover"
                    visible: page.shown !== null && (page.shown.handover || "") !== ""
                    Layout.fillWidth: true
                    text: page.shown ? (page.shown.handover || "") : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Button {
                    visible: page.shown !== null && page.shown.handover_age === 18
                    flat: true
                    text: page.shown ? page.theme.plain(page.sioul.textWith("person-keeps", "name", page.shown.name)) : ""
                    onClicked: page.keepsAsked(page.shown)
                }
                Label {
                    visible: page.helpShown
                    Layout.fillWidth: true
                    text: page.sioul.text("person-help")
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: page.theme.muted
                }

                // Their papers, each with where it stands, as on the Papers page.
                Label {
                    visible: page.shown !== null && page.shown.holds_papers && page.shown.papers.length > 0
                    Layout.topMargin: 8
                    text: page.sioul.text("person-papers")
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                    color: page.theme.accent
                }
                Repeater {
                    model: page.shown && page.shown.holds_papers ? page.shown.papers : []

                    delegate: ItemDelegate {
                        id: paperRow

                        required property var modelData
                        readonly property bool attention: ["renew", "ended", "old"].includes(paperRow.modelData.standing)

                        Layout.fillWidth: true
                        padding: 8
                        onClicked: page.window.openCard({ kind: "dated", datedKind: "paper", srcKey: paperRow.modelData.id, title: paperRow.modelData.title })
                        background: Rectangle {
                            color: paperRow.hovered ? page.theme.hover : "transparent"
                            radius: page.theme.radius
                            border.color: paperRow.visualFocus ? page.theme.focus : page.theme.line
                        }
                        contentItem: ColumnLayout {
                            spacing: 2

                            Label {
                                Layout.fillWidth: true
                                text: paperRow.modelData.title
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: [paperRow.modelData.kind_label, paperRow.modelData.line].filter(t => t !== "").join(" · ")
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: paperRow.attention ? page.theme.warm : page.theme.muted
                            }
                        }
                    }
                }
                Button {
                    objectName: "personAddPaper"
                    visible: page.shown !== null && page.shown.holds_papers
                    flat: true
                    icon.name: "list-add"
                    icon.color: page.theme.text
                    text: page.shown ? (page.shown.me ? page.sioul.text("papers-add") : page.theme.plain(page.sioul.textWith("person-add-paper", "name", page.shown.name))) : ""
                    onClicked: {
                        const kinds = JSON.parse(page.sioul.papers() || "{}").kinds || []
                        const dialog = paperDialog.now()
                        dialog.kinds = kinds
                        dialog.begin("", "", "other", [page.personId])
                    }
                }

                // Their agendas, each with who does what; a child's other parent's, the children's they share.
                Label {
                    visible: page.shown !== null && (page.shown.agendas || []).length > 0
                    Layout.topMargin: 8
                    text: page.sioul.text("person-agendas")
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                    color: page.theme.accent
                }
                Repeater {
                    model: page.shown ? (page.shown.agendas || []) : []

                    delegate: RowLayout {
                        id: agendaRow

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            Layout.fillWidth: true
                            text: agendaRow.modelData.whose ? agendaRow.modelData.name + " · " + agendaRow.modelData.whose : agendaRow.modelData.name
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                        Button {
                            objectName: "personWhoDoesWhat"
                            visible: agendaRow.modelData.child
                            flat: true
                            text: page.sioul.text("holder-who-does-what")
                            onClicked: whoDoesWhat.now().openOn(agendaRow.modelData.id)
                        }
                    }
                }

                // Letters addressed to them (their address block's name, or a card's "For").
                Label {
                    visible: page.shown !== null && (page.shown.letters || []).length > 0
                    Layout.topMargin: 8
                    text: page.sioul.text("person-letters")
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                    color: page.theme.accent
                }
                Repeater {
                    model: page.shown ? (page.shown.letters || []) : []

                    delegate: ItemDelegate {
                        id: letterRow

                        required property var modelData

                        Layout.fillWidth: true
                        padding: 6
                        onClicked: page.window.openLetter(letterRow.modelData.id)
                        contentItem: Label {
                            text: [letterRow.modelData.sender, letterRow.modelData.kind_label, letterRow.modelData.received].filter(t => t !== "").join(" · ")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                    }
                }

                // Money: the budgets for them; the sites you open for them.
                Label {
                    visible: page.shown !== null && ((page.shown.budgets || []).length > 0 || (page.shown.sites || []).length > 0)
                    Layout.topMargin: 8
                    text: page.sioul.text("person-money")
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                    color: page.theme.accent
                }
                Repeater {
                    model: page.shown ? (page.shown.budgets || []) : []

                    delegate: ItemDelegate {
                        id: budgetRow

                        required property var modelData

                        objectName: "personBudget"
                        Layout.fillWidth: true
                        padding: 6
                        onClicked: page.window.openPage("budget", budgetRow.modelData.id)
                        contentItem: Label {
                            text: budgetRow.modelData.title
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                    }
                }
                Repeater {
                    model: page.shown ? (page.shown.sites || []) : []

                    delegate: ItemDelegate {
                        id: siteRow

                        required property var modelData

                        objectName: "personSite"
                        Layout.fillWidth: true
                        padding: 6
                        onClicked: page.window.openThing({ kind: "site", uri: "sioul:site/" + encodeURIComponent(siteRow.modelData.id), key: siteRow.modelData.id, title: siteRow.modelData.name })
                        contentItem: Label {
                            text: page.theme.plain(page.sioul.textWith("person-site", "name", siteRow.modelData.name))
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                    }
                }

                // The cards that are them, and one to link.
                Label {
                    visible: page.shown !== null && !page.shown.me && page.shown.cards.length > 0
                    Layout.topMargin: 8
                    text: page.sioul.text("person-contacts")
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                    color: page.theme.accent
                }
                Repeater {
                    model: page.shown ? page.shown.cards : []

                    delegate: RowLayout {
                        id: cardRow

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 8

                        ItemDelegate {
                            Layout.fillWidth: true
                            padding: 6
                            onClicked: page.openContact(cardRow.modelData.key)
                            contentItem: ColumnLayout {
                                spacing: 2

                                Label {
                                    Layout.fillWidth: true
                                    text: cardRow.modelData.name
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    color: page.theme.text
                                }
                                Label {
                                    visible: cardRow.modelData.detail !== ""
                                    Layout.fillWidth: true
                                    text: cardRow.modelData.detail
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: 13
                                    color: page.theme.muted
                                }
                            }
                        }
                        Button {
                            flat: true
                            text: page.sioul.text("person-unlink-card")
                            onClicked: {
                                page.problem = page.sioul.linkPersonCard(page.personId, cardRow.modelData.uid, false)
                                page.reload()
                                page.changed()
                            }
                        }
                    }
                }
                Button {
                    objectName: "personLinkCard"
                    visible: page.shown !== null && !page.shown.me
                    flat: true
                    icon.name: "contact-new"
                    icon.color: page.theme.text
                    text: page.sioul.text("person-link-card")
                    onClicked: picker.ask()
                }

                Label {
                    visible: page.problem !== ""
                    Layout.fillWidth: true
                    text: page.problem
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.warm
                }

                // At the foot, quiet: what Sioul keeps about them, the relation's end, and what agents may read.
                Flow {
                    visible: page.shown !== null && !page.shown.me
                    Layout.topMargin: 12
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        objectName: "personKeeps"
                        flat: true
                        text: page.shown ? page.theme.plain(page.sioul.textWith("person-keeps", "name", page.shown.name)) : ""
                        onClicked: page.keepsAsked(page.shown)
                    }
                    Button {
                        objectName: "personEnds"
                        visible: page.shown !== null && page.shown.can_end === true
                        flat: true
                        text: page.sioul.text("person-ends")
                        onClicked: page.endAsked(page.shown)
                    }
                    Button {
                        objectName: "personSameAs"
                        visible: page.shown !== null && (page.shown.same_as || []).length > 0
                        flat: true
                        text: page.sioul.text("person-same-as")
                        onClicked: sameMenu.popup()
                    }
                    Button {
                        objectName: "personNotAPerson"
                        flat: true
                        text: page.sioul.text("person-not-a-person")
                        onClicked: {
                            page.problem = page.sioul.notAPerson(page.personId)
                            if (page.problem === "") {
                                page.changed()
                                page.closeAsked()
                            }
                        }
                    }
                }
                Label {
                    visible: page.shown !== null && (page.shown.ended || "") !== ""
                    Layout.fillWidth: true
                    text: page.shown ? page.sioul.textWith("person-ended", "date", page.shown.ended || "") : ""
                    textFormat: Text.PlainText
                    color: page.theme.muted
                }
                // At the foot, quiet: what agents may read. You are always open, as before.
                Switch {
                    objectName: "personAgents"
                    visible: page.shown !== null && !page.shown.me
                    Layout.topMargin: 12
                    Layout.fillWidth: true
                    text: page.sioul.text("person-agents")
                    checked: page.shown !== null && page.shown.agents === true
                    onToggled: {
                        page.problem = page.sioul.setPersonAgents(page.personId, checked)
                        page.reload()
                    }
                }
                // What it means, in one sentence under it (as a project's page says it).
                Label {
                    objectName: "personAgentsHelp"
                    visible: page.shown !== null && !page.shown.me
                    Layout.fillWidth: true
                    text: page.sioul.text("person-agents-help")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: page.theme.muted
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: page.theme.gap

            Button {
                visible: page.shown !== null && !page.shown.me
                text: page.sioul.text("ui-edit")
                onClicked: page.editAsked(page.shown)
            }
            Item {
                Layout.fillWidth: true
            }
            Button {
                text: page.sioul.text("ui-close")
                onClicked: page.closeAsked()
            }
        }
    }

    // A paper for them: the wallet's form, "For" set to them.
    Later {
        id: paperDialog

        sourceComponent: Component {
            PaperDialog {
                sioul: page.sioul
                theme: page.theme
                window: page.window
                onSaved: {
                    page.reload()
                    page.changed()
                }
            }
        }
    }

    // "Same as…": the person chosen joined into this one (Léa and "Léa M.").
    SioulMenu {
        id: sameMenu

        Instantiator {
            model: page.shown ? (page.shown.same_as || []) : []

            delegate: MenuItem {
                required property var modelData

                text: page.theme.plain(modelData.name)
                onTriggered: {
                    page.problem = page.sioul.samePerson(page.personId, modelData.id)
                    page.reload()
                    page.changed()
                }
            }
            onObjectAdded: (index, object) => sameMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => sameMenu.removeItem(object)
        }
    }

    // Who does what, a week of one child's agenda (`children::who_does_what`).
    Later {
        id: whoDoesWhat

        sourceComponent: Component {
            WhoDoesWhat {
                sioul: page.sioul
                theme: page.theme
                window: page.window
                narrow: page.width < 560
            }
        }
    }

    // A contact card to link: by the words typed.
    SafePopup {
        id: picker

        property var rows: []

        function ask() {
            search.text = ""
            picker.rows = JSON.parse(page.sioul.personCardChoices("") || "[]")
            picker.open()
            search.forceActiveFocus()
        }

        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(420, picker.roomWidth - 16)
        height: Math.min(480, picker.roomHeight - 16)

        contentItem: ColumnLayout {
            spacing: 6

            TextField {
                id: search

                Layout.fillWidth: true
                placeholderText: page.sioul.text("ui-search")
                onTextEdited: picker.rows = JSON.parse(page.sioul.personCardChoices(search.text) || "[]")
            }
            ListView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: picker.rows

                delegate: ItemDelegate {
                    id: choice

                    required property var modelData

                    width: ListView.view.width
                    padding: 6
                    onClicked: {
                        page.problem = page.sioul.linkPersonCard(page.personId, choice.modelData.uid, true)
                        picker.close()
                        page.reload()
                        page.changed()
                    }
                    contentItem: ColumnLayout {
                        spacing: 2

                        Label {
                            Layout.fillWidth: true
                            text: choice.modelData.name
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                        Label {
                            readonly property string of: choice.modelData.of ? page.theme.plain(page.sioul.textWith("person-card-of", "name", choice.modelData.of)) : ""

                            visible: text !== ""
                            Layout.fillWidth: true
                            text: [choice.modelData.detail, of].filter(t => t !== "").join(" · ")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                    }
                }
            }
            Button {
                Layout.alignment: Qt.AlignRight
                text: page.sioul.text("ui-cancel")
                onClicked: picker.close()
            }
        }
    }
}
