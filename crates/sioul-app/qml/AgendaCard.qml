// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One agenda's card (Agenda ⚙ ▸ Your agendas, its row's Details…; 26 November
// 2026): what needs more room than its row, in headed sections (the owner's
// rule 3), each setting in this one place, never one its row changes (its
// colour, whose it is, what for, when it reaches you, what you may do, its
// busy time). Its name (yours, on its server; a calendar read by its
// address keeps its name in its settings); whose name, for a role that names
// a person; for its availability only, whether its events' titles show;
// a child's other parent and who usually takes each kind;
// who sees it, as its server says, shared with a person, a group or by a
// link; a calendar read by its address: its line, Refresh now, its address,
// its name, whether it holds holidays; kept hidden on this device; Remove…
// (or Stop showing…), asked in one sentence first. A calendar of busy blocks
// Sioul keeps says for which availability, opens it, and is removed after
// its question. Every
// change is saved at once through the page (AgendasSetup.qml), which reads
// itself again: the card shows the agenda as the page has it now.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: card

    // The page that opened it (AgendasSetup.qml): its words, its saves, its view.
    required property var setup
    // The agenda shown, by its place.
    property string agendaId: ""
    // The agenda as the page reads it now; {} when it is gone.
    readonly property var agenda: (card.setup.view.agendas || []).find(a => a.id === card.agendaId) || ({})
    readonly property var on: String(card.agenda.area || "").split("+")
    readonly property var sub: card.agenda.subscription || null
    readonly property bool mirror: card.agenda.mirror === true
    // Its server answered who sees it (`agendaShared`); null while asked.
    readonly property var share: card.setup.sharing[card.agendaId] || null
    readonly property bool canShare: card.share !== null && card.share.sharer !== undefined && card.share.sharer !== "none" && !!card.agenda.server && card.agenda.server.own === true
    readonly property bool onServer: !!card.agenda.server && card.agenda.server.server !== "local"
    readonly property bool own: !!card.agenda.server && card.agenda.server.own === true
    // A subscription's address being changed, or its stop asked.
    property bool changing: false
    property bool stopping: false

    function show(id) {
        card.agendaId = id
        card.changing = false
        card.stopping = false
        card.setup.removing = ""
        card.open()
    }

    objectName: "agendaCard"
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(640, (parent ? card.roomWidth : 640) - 2 * card.setup.theme.gap)
    title: card.setup.theme.plain(card.agenda.name || "")

    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(column.implicitHeight, (card.parent ? card.roomHeight : 700) - 160)
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: card.setup.theme.rowGap

            Label {
                visible: card.setup.problem !== ""
                Layout.fillWidth: true
                text: card.setup.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: card.setup.theme.warm
            }

            // A calendar of busy blocks Sioul keeps: for which availability, and the way there.
            Section {
                visible: card.mirror
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-card-mirror")

                Label {
                    Layout.fillWidth: true
                    text: card.agenda.kept_for ? card.setup.sioul.textWith("agenda-mirror-kept-for", "name", card.agenda.kept_for) : card.setup.sioul.text("agenda-mirror-kept")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.setup.theme.text
                }
                Button {
                    objectName: "agendaMirrorOpen"
                    text: card.setup.sioul.text("agenda-mirror-open")
                    onClicked: {
                        card.close()
                        card.setup.availabilityAsked()
                    }
                }
            }

            // Its name: yours on its server, or a calendar read by its address (its colour is changed on its row).
            Section {
                visible: !card.mirror && ((card.own && !card.setup.tasksOnly(card.agenda)) || (card.sub !== null && card.sub.unread !== true))
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-card-name")

                TextField {
                    objectName: "agendaRename"
                    visible: card.own && card.sub === null && !card.setup.tasksOnly(card.agenda)
                    Layout.fillWidth: true
                    text: card.agenda.name || ""
                    Accessible.name: card.setup.sioul.text("agenda-new-name")
                    onEditingFinished: if (text.trim() !== "" && text.trim() !== card.agenda.name) {
                        card.setup.problem = card.setup.sioul.renameAgenda(card.agendaId, text.trim())
                        card.setup.read()
                    }
                }
                // A calendar read by its address: its own name, its domain's shown when none is given.
                TextField {
                    objectName: "subscriptionName"
                    visible: card.sub !== null
                    Layout.fillWidth: true
                    text: card.sub ? card.sub.name : ""
                    placeholderText: card.setup.theme.plain(card.agenda.name || "")
                    Accessible.name: card.setup.sioul.text("subscribe-name")
                    onEditingFinished: if (card.sub && text.trim() !== card.sub.name)
                        card.setup.saveSubscription(card.sub.id, "name", text.trim())
                }
            }

            // Whose name, for a role that names a person: the role itself is chosen on its row.
            Section {
                visible: !card.mirror && ["child", "partner", "parent", "someone"].indexOf(card.agenda.whose) >= 0
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-whose")

                WhoseFields {
                    Layout.fillWidth: true
                    roleShown: false
                    sioul: card.setup.sioul
                    theme: card.setup.theme
                    persons: card.setup.view.persons || ({})
                    whose: card.agenda.whose || "yours"
                    personId: card.agenda.person_id || ""
                    name: card.agenda.person || ""
                    onWhoseChosen: whose => card.setup.save(card.agendaId, "whose", whose)
                    onPersonChosen: id => card.setup.setPerson(card.agendaId, "person", id)
                    onNameGiven: name => card.setup.save(card.agendaId, "name", name)
                    onNewPersonAsked: card.setup.newPerson(card.agenda, "person")
                }
            }

            // What you may do with it, chosen on its row: here, for its availability only, what its
            // events say shown; and why its server lets you read it only.
            Section {
                visible: !card.mirror && (card.agenda.access === "availability" || card.agenda.server_read_only === true)
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-access")

                CheckBox {
                    objectName: "agendaTitles"
                    visible: card.agenda.access === "availability"
                    Layout.fillWidth: true
                    text: card.setup.sioul.text("agenda-titles")
                    checked: card.agenda.titles === true
                    onToggled: card.setup.save(card.agendaId, "titles", checked ? "shown" : "")
                }
                Label {
                    visible: card.agenda.server_read_only === true
                    Layout.fillWidth: true
                    // A calendar read by its address has no server to say so: its own words.
                    text: card.setup.sioul.text(card.sub !== null ? "agenda-subscribed-read-only" : "agenda-server-read-only")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.setup.theme.muted
                }
            }

            // A child's: the other parent who may take its events, how they answer, who usually takes each kind.
            Section {
                visible: !card.mirror && card.agenda.whose === "child"
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-parent")

                // The other parent: a person, a child's other parent; their address stays this agenda's (where invitations go).
                PersonChooser {
                    objectName: "agendaParent"
                    Layout.fillWidth: true
                    sioul: card.setup.sioul
                    theme: card.setup.theme
                    persons: (card.setup.view.persons || {}).parent || []
                    personId: card.agenda.parent_person || ""
                    fallback: card.agenda.parent || ""
                    Accessible.name: card.setup.sioul.text("agenda-parent-name")
                    onChosen: id => card.setup.setPerson(card.agendaId, "parent_person", id)
                    onNewAsked: card.setup.newPerson(card.agenda, "parent_person")
                }
                TextField {
                    objectName: "agendaParentAddress"
                    Layout.fillWidth: true
                    text: card.agenda.parent_address || ""
                    placeholderText: card.setup.sioul.text("agenda-parent-address")
                    inputMethodHints: Qt.ImhEmailCharactersOnly
                    Accessible.name: placeholderText
                    onEditingFinished: if (text.trim() !== (card.agenda.parent_address || ""))
                        card.setup.save(card.agendaId, "parent_address", text.trim())
                }
                Label {
                    text: card.setup.sioul.text("agenda-parent-answers")
                    color: card.setup.theme.text
                }
                PlainComboBox {
                    readonly property var choices: ["mail", "sioul"]

                    Layout.fillWidth: true
                    model: choices.map(c => card.setup.sioul.text("agenda-parent-answers-" + c))
                    currentIndex: Math.max(0, choices.indexOf(card.agenda.parent_answers))
                    Accessible.name: card.setup.sioul.text("agenda-parent-answers")
                    onActivated: index => card.setup.save(card.agendaId, "parent_answers", choices[index])
                }
                Label {
                    text: card.setup.sioul.text("agenda-standing")
                    color: card.setup.theme.text
                }
                Repeater {
                    model: card.agenda.whose === "child" ? card.setup.view.children.kinds : []

                    delegate: RowLayout {
                        id: standing

                        required property var modelData
                        readonly property var choices: ["", "me", "other"]
                        readonly property string now: (card.agenda.standing || {})[standing.modelData.id] || ""

                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            Layout.preferredWidth: 110
                            text: standing.modelData.label
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: card.setup.theme.text
                        }
                        PlainComboBox {
                            objectName: "agendaStanding"
                            Layout.fillWidth: true
                            model: standing.choices.map(c => c === "" ? card.setup.sioul.text("agenda-standing-ask") : c === "me" ? card.setup.sioul.text("holder-me") : card.setup.theme.plain(card.agenda.parent || card.setup.sioul.text("agenda-parent")))
                            currentIndex: Math.max(0, standing.choices.indexOf(standing.now))
                            Accessible.name: card.setup.sioul.text("agenda-standing") + " · " + standing.modelData.label
                            onActivated: index => {
                                card.setup.problem = card.setup.sioul.setStandingHolder(card.agendaId, standing.modelData.id, standing.choices[index])
                                card.setup.read()
                            }
                        }
                    }
                }
            }

            // Who sees it, as its server says; shared with a person, a group, or by a link.
            Section {
                visible: !card.mirror && card.onServer && card.sub === null
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-sharing")

                Label {
                    objectName: "agendaSharingLine"
                    Layout.fillWidth: true
                    text: card.share === null ? card.setup.sioul.text("agenda-sharing-reading") : (card.share.problem || card.share.line || "")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.share !== null && (card.share.problem || "") !== "" ? card.setup.theme.warm : card.setup.theme.text
                }
                Repeater {
                    model: card.canShare ? card.share.people : []

                    delegate: RowLayout {
                        id: sharee

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            text: sharee.modelData.name
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: card.setup.theme.text
                        }
                        ToolButton {
                            objectName: "agendaUnshare"
                            text: "×"
                            Accessible.name: card.setup.sioul.textWith("agenda-unshare", "who", sharee.modelData.name)
                            onClicked: card.setup.sioul.unshareAgenda(card.agendaId, sharee.modelData.id)
                        }
                    }
                }
                // The name on a line of its own, then how and Share, wrapping on a phone.
                TextField {
                    id: shareWith

                    objectName: "agendaShareWith"
                    visible: card.canShare
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    placeholderText: card.setup.sioul.text("agenda-share-with")
                    Accessible.name: placeholderText
                }
                Flow {
                    visible: card.canShare
                    Layout.fillWidth: true
                    spacing: 6

                    CheckBox {
                        id: shareGroup

                        objectName: "agendaShareGroup"
                        visible: card.share !== null && card.share.sharer === "nextcloud"
                        text: card.setup.sioul.text("agenda-share-group")
                    }
                    PlainComboBox {
                        id: shareAccess

                        readonly property var choices: card.share && card.share.sharer === "google" ? ["read", "write", "busy"] : ["read", "write"]

                        width: 170
                        model: choices.map(c => card.setup.sioul.text("agenda-share-" + c))
                        Accessible.name: card.setup.sioul.text("agenda-sharing")
                    }
                    Button {
                        objectName: "agendaShare"
                        enabled: shareWith.text.trim() !== ""
                        text: card.setup.sioul.text("agenda-share")
                        onClicked: {
                            card.setup.sioul.shareAgenda(card.agendaId, (shareGroup.visible && shareGroup.checked ? "group:" : "") + shareWith.text.trim(), shareAccess.choices[shareAccess.currentIndex])
                            shareWith.text = ""
                            shareGroup.checked = false
                        }
                    }
                }
                CheckBox {
                    objectName: "agendaLink"
                    visible: card.canShare && card.share.links === true
                    text: card.setup.sioul.text("agenda-link-on")
                    checked: card.share !== null && !!card.share.link
                    onToggled: card.setup.sioul.agendaLink(card.agendaId, checked)
                }
                // The link itself, to select or copy.
                LinkLine {
                    objectName: "agendaLinkLine"
                    visible: card.canShare && card.share.links === true && !!card.share.link
                    Layout.fillWidth: true
                    sioul: card.setup.sioul
                    theme: card.setup.theme
                    link: card.share !== null ? card.share.link || "" : ""
                }
                Label {
                    visible: card.share !== null && ((card.share.said || "") !== "" || (card.share.note || "") !== "")
                    Layout.fillWidth: true
                    text: card.share === null ? "" : [card.share.said || "", card.share.note || ""].filter(w => w !== "").join(" ")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.setup.theme.muted
                }
            }

            // A calendar subscribed by its address: when this device last read it, or why not;
            // Refresh now, Change the address, Stop; whether it holds holidays.
            Section {
                visible: card.sub !== null
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-card-address")

                Label {
                    objectName: "subscriptionLine"
                    Layout.fillWidth: true
                    text: card.sub ? card.sub.line : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.setup.theme.text
                }
                Flow {
                    visible: !card.changing && !card.stopping
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        objectName: "subscriptionRefresh"
                        visible: card.sub !== null && card.sub.unread !== true
                        text: card.setup.sioul.text("subscribe-refresh")
                        onClicked: card.setup.problem = card.setup.sioul.refreshSubscription(card.sub.id)
                    }
                    Button {
                        objectName: "subscriptionChange"
                        visible: card.sub !== null && card.sub.unread !== true
                        text: card.setup.sioul.text("subscribe-change")
                        onClicked: {
                            newAddress.text = card.sub.url
                            card.changing = true
                        }
                    }
                    Button {
                        objectName: "subscriptionStop"
                        text: card.setup.sioul.text("subscribe-stop")
                        onClicked: card.stopping = true
                    }
                }
                TextField {
                    id: newAddress

                    objectName: "subscriptionAddress"
                    visible: card.changing
                    Layout.fillWidth: true
                    inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                    Accessible.name: card.setup.sioul.text("subscribe-address")
                }
                Flow {
                    visible: card.changing
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        objectName: "subscriptionAddressSave"
                        enabled: newAddress.text.trim() !== ""
                        text: card.setup.sioul.text("subscribe-change-save")
                        onClicked: {
                            card.setup.saveSubscription(card.sub.id, "url", newAddress.text.trim())
                            if (card.setup.problem === "")
                                card.changing = false
                        }
                    }
                    Button {
                        flat: true
                        text: card.setup.sioul.text("agenda-keep")
                        onClicked: card.changing = false
                    }
                }
                Label {
                    visible: card.stopping
                    Layout.fillWidth: true
                    text: card.setup.sioul.text("subscribe-stop-question")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.setup.theme.text
                }
                Flow {
                    visible: card.stopping
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        objectName: "subscriptionStopConfirm"
                        text: card.setup.sioul.text("subscribe-stop-confirm")
                        onClicked: {
                            card.setup.problem = card.setup.sioul.stopSubscription(card.sub.id)
                            card.stopping = false
                            card.setup.read()
                            if (card.setup.problem === "")
                                card.close()
                        }
                    }
                    Button {
                        flat: true
                        text: card.setup.sioul.text("agenda-keep")
                        onClicked: card.stopping = false
                    }
                }
                PlainComboBox {
                    readonly property var kinds: ["", "public", "school"]

                    objectName: "subscriptionKind"
                    visible: card.sub !== null && card.sub.unread !== true
                    Layout.fillWidth: true
                    model: kinds.map(k => card.setup.sioul.text("subscribe-holidays-" + (k === "" ? "none" : k)))
                    currentIndex: card.sub ? Math.max(0, kinds.indexOf(card.sub.holidays)) : 0
                    Accessible.name: card.setup.sioul.text("subscribe-holidays")
                    onActivated: index => card.setup.saveSubscription(card.sub.id, "holidays", kinds[index])
                }
                // Whether its days count as your days off is Settings ▸ Hours' alone (VacationsCalendars.qml);
                // whether its events count as busy time, its row's switch.
                WrapCheckBox {
                    objectName: "subscriptionAfterClass"
                    visible: card.sub !== null && card.sub.unread !== true && card.sub.holidays === "school"
                    Layout.fillWidth: true
                    text: card.setup.sioul.text("subscribe-after-class")
                    checked: card.sub !== null && card.sub.after_class === true
                    onToggled: card.setup.saveSubscription(card.sub.id, "after_class", checked ? "true" : "false")
                }
            }

            // On this device: hidden for good, whatever the hours, until shown again.
            Section {
                visible: !card.mirror
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-card-device")

                Button {
                    objectName: "agendaKeepHidden"
                    text: card.setup.sioul.text(card.agenda.kept_hidden === true ? "agenda-show-again" : "agenda-keep-hidden")
                    onClicked: {
                        card.setup.problem = card.setup.sioul.keepAgendaHidden(card.agendaId, card.agenda.kept_hidden !== true)
                        card.setup.read()
                    }
                }
            }

            // Removed (yours) or no longer shown (someone else's), after one sentence; a list of
            // tasks alone in Tasks ⚙, its one place; a mirror says whom it stops sharing with.
            Section {
                readonly property bool can: card.mirror || (!!card.agenda.server && card.agenda.server.server !== undefined && (card.own || card.onServer) && !card.setup.tasksOnly(card.agenda) && card.sub === null)

                visible: can
                Layout.fillWidth: true
                theme: card.setup.theme
                title: card.setup.sioul.text("agenda-card-remove")

                Button {
                    objectName: card.mirror ? "agendaMirrorRemove" : "agendaRemove"
                    visible: card.setup.removing !== card.agendaId
                    text: card.setup.sioul.text(card.mirror || card.own ? "agenda-remove" : "agenda-stop-showing")
                    onClicked: card.setup.askRemoval(card.agendaId)
                }
                Label {
                    objectName: card.mirror ? "agendaMirrorRemoveQuestion" : "agendaRemoveQuestion"
                    visible: card.setup.removing === card.agendaId
                    Layout.fillWidth: true
                    text: card.setup.removal.sentence || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.setup.theme.text
                }
                Label {
                    visible: card.setup.removing === card.agendaId && (card.setup.removal.note || "") !== ""
                    Layout.fillWidth: true
                    text: card.setup.removal.note || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.setup.theme.muted
                }
                Flow {
                    visible: card.setup.removing === card.agendaId
                    Layout.fillWidth: true
                    spacing: 8

                    Button {
                        objectName: card.mirror ? "agendaMirrorRemoveConfirm" : "agendaRemoveConfirm"
                        visible: ["remove", "leave", "hide", "mirror"].indexOf(card.setup.removal.act) >= 0
                        text: card.setup.sioul.text(card.setup.removal.act === "mirror" ? "agenda-remove-mirror-confirm" : card.setup.removal.act === "remove" ? "agenda-remove-confirm" : "agenda-leave-confirm")
                        onClicked: {
                            card.setup.remove(card.agendaId)
                            if (card.setup.problem === "")
                                card.close()
                        }
                    }
                    Button {
                        flat: true
                        text: card.setup.sioul.text("agenda-keep")
                        onClicked: card.setup.removing = ""
                    }
                }
            }
        }
    }

    footer: DialogFoot {
        sioul: card.setup.sioul
        theme: card.setup.theme
        cancelText: card.setup.sioul.text("ui-close")
        onRejected: card.close()
    }
}
