// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Agenda ⚙'s block of the agendas (several agendas): each calendar and task
// list, folded to a line, opened to say whose it is (yours, a child's,
// another parent's, a partner's, shared with you) and whose name, what it is
// for (work, your admin, leisure), when a personal one reaches you (all day,
// or outside work hours), and what you may do with it (read and write, read
// only, its availability only; never writing where its server keeps it read
// only). What Sioul saw decides until you say; each change is saved at once,
// into the configuration, and travels with the settings (`agendas::set_field`).
// "Keep hidden" hides one for good on this device, whatever the hours, until
// "Show again" (`agenda::keep_hidden`, this device's own). A child's agenda
// names the other parent who may take its events, how they answer (an
// invitation in their mail, or their own Sioul), and who usually takes each
// kind (part 3); below the agendas, where your stand-ins of each kind go and
// the follow-up tasks (`children`). On its server (part 5, `servers`): "New
// agenda…" makes one with its role; yours are renamed, recoloured and
// removed (asked in one sentence, a dated copy first), someone else's
// stopped showing; each says who sees it, shared with a person or by a link
// where its server shares; then "Share my availability…".

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme
    // The row's sentence, said once above the agendas.
    property string about: ""
    // `agenda::setup`: {agendas: [{id, name, colour, place, events, tasks, whose, person,
    // area, reaches, access, server_read_only, for_work, said}], pairs}.
    property var view: ({ agendas: [], pairs: [], children: { kinds: [], yours: [], follow_ups: true, any: false } })
    property string problem: ""
    // The agendas opened, by place, while the panel lives.
    property var opened: ({})
    // Each agenda's sharing as its server last answered (`agendaShared`), by place.
    property var sharing: ({})
    // The removal asked of an agenda: its place, and what it would do (`agendaRemoval`).
    property string removing: ""
    property var removal: ({})
    // "New agenda…" open; "Share my availability…" open, and what it is now.
    property bool making: false
    property bool sharingAvailability: false
    property var availability: ({ current: null, line: "", agendas: [], accounts: [] })
    readonly property var colours: ["#4c6b5c", "#3f6f8f", "#7a5c99", "#b07d3f", "#a3504f", "#5f7f3a", "#5b6470", "#c06c8d"]

    readonly property var whoseChoices: ["yours", "child", "parent", "partner", "shared"]
    readonly property var areas: ["work", "admin", "leisure"]

    function read() {
        try {
            setup.view = JSON.parse(setup.sioul.agendasSetup())
        } catch (e) {
            setup.view = { agendas: [], pairs: [], children: { kinds: [], yours: [], follow_ups: true, any: false } }
        }
    }

    // One field written ("" back to what Sioul saw); the agendas read again.
    function save(id, field, value) {
        setup.problem = setup.sioul.setAgendaField(id, field, value)
        setup.read()
    }

    function toggle(id) {
        const next = Object.assign({}, setup.opened)
        next[id] = !next[id]
        setup.opened = next
        // Who sees it, asked of its server when its line opens.
        const agenda = setup.view.agendas.find(a => a.id === id)
        if (next[id] && agenda && agenda.server && agenda.server.server !== "local")
            setup.sioul.readAgendaSharing(id)
    }

    function readAvailability() {
        try {
            setup.availability = JSON.parse(setup.sioul.availability() || "{}")
        } catch (e) {
            setup.availability = { current: null, line: "", agendas: [], accounts: [] }
        }
    }

    // The removal asked first, in one sentence; done on the second press.
    function askRemoval(id) {
        setup.removal = JSON.parse(setup.sioul.agendaRemoval(id) || "{}")
        setup.removing = id
    }
    function remove(id) {
        setup.problem = setup.sioul.removeAgenda(id)
        setup.removing = ""
        setup.read()
    }

    Connections {
        target: setup.sioul

        function onAgendaShared(id, answer) {
            if (id === "availability") {
                setup.readAvailability()
                return
            }
            const next = Object.assign({}, setup.sharing)
            next[id] = JSON.parse(answer || "{}")
            setup.sharing = next
        }
    }

    // What may be chosen of what you may do with it: never writing where its server keeps it read only.
    function accessChoices(agenda) {
        return agenda.server_read_only ? ["read", "availability"] : ["write", "read", "availability"]
    }

    objectName: "agendasSetup"
    spacing: 4
    Component.onCompleted: {
        setup.read()
        setup.readAvailability()
    }

    Label {
        visible: setup.about !== ""
        Layout.fillWidth: true
        text: setup.about
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.problem !== ""
        Layout.fillWidth: true
        text: setup.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.warm
    }

    // A new agenda, with its role (part 5): made here, and on its server by the next sync.
    Button {
        objectName: "agendaNew"
        visible: !setup.making && (setup.view.accounts || []).length > 0
        text: setup.sioul.text("agenda-new")
        onClicked: setup.making = true
    }
    ColumnLayout {
        id: making

        property string colour: setup.colours[0]
        readonly property var account: (setup.view.accounts || [])[accountChoice.currentIndex] || null

        visible: setup.making
        Layout.fillWidth: true
        spacing: 4

        Label {
            text: setup.sioul.text("agenda-new-account")
            color: setup.theme.text
        }
        PlainComboBox {
            id: accountChoice

            objectName: "agendaNewAccount"
            Layout.fillWidth: true
            model: (setup.view.accounts || []).map(a => setup.theme.plain(a.name))
            Accessible.name: setup.sioul.text("agenda-new-account")
        }
        TextField {
            id: newName

            objectName: "agendaNewName"
            Layout.fillWidth: true
            placeholderText: setup.sioul.text("agenda-new-name")
            Accessible.name: placeholderText
        }
        Flow {
            Layout.fillWidth: true
            spacing: 12

            CheckBox {
                id: newEvents

                text: setup.sioul.text("agenda-new-events")
                checked: true
            }
            CheckBox {
                id: newTasks

                // Google's task lists are Google Tasks': a Google agenda holds events.
                enabled: !(making.account && making.account.google)
                text: setup.sioul.text("agenda-new-tasks")
            }
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: setup.colours

                delegate: Rectangle {
                    id: swatch

                    required property string modelData

                    width: 26
                    height: 26
                    radius: 13
                    color: swatch.modelData
                    border.width: making.colour === swatch.modelData ? 3 : 0
                    border.color: setup.theme.text
                    Accessible.role: Accessible.Button
                    Accessible.name: setup.sioul.text("agenda-colour") + " " + swatch.modelData

                    TapHandler {
                        onTapped: making.colour = swatch.modelData
                    }
                }
            }
        }
        PlainComboBox {
            id: newWhose

            Layout.fillWidth: true
            model: setup.whoseChoices.map(w => setup.sioul.text("agenda-whose-" + w))
            Accessible.name: setup.sioul.text("agenda-whose")
        }
        TextField {
            id: newPerson

            visible: newWhose.currentIndex > 0
            Layout.fillWidth: true
            placeholderText: setup.sioul.text(newWhose.currentIndex === 1 ? "agenda-person-child" : "agenda-person")
            Accessible.name: placeholderText
        }
        Flow {
            id: newAreas

            Layout.fillWidth: true
            spacing: 12

            CheckBox {
                id: newWork

                text: setup.sioul.text("area-work")
            }
            CheckBox {
                id: newAdmin

                text: setup.sioul.text("area-admin")
                checked: true
            }
            CheckBox {
                id: newLeisure

                text: setup.sioul.text("area-leisure")
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
            model: choices.map(r => setup.sioul.text("agenda-reaches-" + r))
            Accessible.name: setup.sioul.text("agenda-reaches")
        }
        RowLayout {
            spacing: 8

            Button {
                objectName: "agendaNewMake"
                enabled: newName.text.trim() !== "" && making.account !== null
                text: setup.sioul.text("agenda-new-make")
                onClicked: {
                    const areas = [[newWork, "work"], [newAdmin, "admin"], [newLeisure, "leisure"]].filter(p => p[0].checked).map(p => p[1]).join("+")
                    const answer = JSON.parse(setup.sioul.newAgenda(JSON.stringify({ account: making.account.id, name: newName.text.trim(), colour: making.colour, events: newEvents.checked, tasks: newTasks.enabled && newTasks.checked, whose: setup.whoseChoices[newWhose.currentIndex], person: newPerson.text.trim(), area: areas, reaches: newWork.checked || newReaches.currentIndex === 0 ? "" : newReaches.choices[newReaches.currentIndex] })) || "{}")
                    setup.problem = answer.problem || ""
                    if (setup.problem === "") {
                        setup.making = false
                        newName.text = ""
                        setup.read()
                    }
                }
            }
            Button {
                flat: true
                text: setup.sioul.text("agenda-keep")
                onClicked: setup.making = false
            }
        }
    }

    Repeater {
        model: setup.view.agendas

        delegate: ColumnLayout {
            id: one

            required property var modelData
            readonly property bool open: setup.opened[one.modelData.id] === true
            readonly property var on: String(one.modelData.area).split("+")

            objectName: "agendaSetup"
            Layout.fillWidth: true
            spacing: 4

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                ToolButton {
                    text: one.open ? "▾" : "▸"
                    Accessible.name: setup.sioul.textWith(one.open ? "agendas-fold" : "agendas-unfold", "group", one.modelData.name)
                    onClicked: setup.toggle(one.modelData.id)
                }
                Rectangle {
                    Layout.preferredWidth: 10
                    Layout.preferredHeight: 10
                    radius: 5
                    color: one.modelData.colour || setup.theme.accent
                }
                Label {
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    text: one.modelData.name + " · " + setup.sioul.text("agenda-whose-" + one.modelData.whose) + (one.modelData.person !== "" ? " (" + one.modelData.person + ")" : "") + (one.modelData.kept_hidden === true ? " · " + setup.sioul.text("agendas-kept-hidden") : "")
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: setup.theme.text

                    TapHandler {
                        onTapped: setup.toggle(one.modelData.id)
                    }
                }
            }

            GridLayout {
                visible: one.open
                Layout.fillWidth: true
                Layout.leftMargin: 34
                columns: 1
                rowSpacing: 4

                Label {
                    text: one.modelData.place
                    textFormat: Text.PlainText
                    color: setup.theme.muted
                }
                // On its server (part 5): yours renamed, recoloured, removed; someone else's stopped showing.
                TextField {
                    objectName: "agendaRename"
                    visible: one.modelData.server && one.modelData.server.own === true
                    Layout.fillWidth: true
                    text: one.modelData.name
                    Accessible.name: setup.sioul.text("agenda-new-name")
                    onEditingFinished: if (text.trim() !== "" && text.trim() !== one.modelData.name) {
                        setup.problem = setup.sioul.renameAgenda(one.modelData.id, text.trim())
                        setup.read()
                    }
                }
                Flow {
                    visible: one.modelData.server && one.modelData.server.own === true
                    Layout.fillWidth: true
                    spacing: 6

                    Repeater {
                        model: setup.colours

                        delegate: Rectangle {
                            id: dot

                            required property string modelData

                            objectName: "agendaColour"
                            width: 22
                            height: 22
                            radius: 11
                            color: dot.modelData
                            border.width: (one.modelData.colour || "").toLowerCase().startsWith(dot.modelData) ? 3 : 0
                            border.color: setup.theme.text
                            Accessible.role: Accessible.Button
                            Accessible.name: setup.sioul.text("agenda-colour") + " " + dot.modelData

                            TapHandler {
                                onTapped: {
                                    setup.problem = setup.sioul.recolourAgenda(one.modelData.id, dot.modelData)
                                    setup.read()
                                }
                            }
                        }
                    }
                }
                Button {
                    objectName: "agendaRemove"
                    visible: one.modelData.server && one.modelData.server.server !== undefined && (one.modelData.server.own === true || one.modelData.server.server !== "local") && setup.removing !== one.modelData.id
                    text: setup.sioul.text(one.modelData.server && one.modelData.server.own === true ? "agenda-remove" : "agenda-stop-showing")
                    onClicked: setup.askRemoval(one.modelData.id)
                }
                ColumnLayout {
                    visible: setup.removing === one.modelData.id
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        objectName: "agendaRemoveQuestion"
                        Layout.fillWidth: true
                        text: setup.removal.sentence || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: setup.theme.text
                    }
                    Label {
                        visible: (setup.removal.note || "") !== ""
                        Layout.fillWidth: true
                        text: setup.removal.note || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: setup.theme.muted
                    }
                    RowLayout {
                        spacing: 8

                        Button {
                            objectName: "agendaRemoveConfirm"
                            visible: setup.removal.act === "remove" || setup.removal.act === "leave"
                            text: setup.sioul.text(setup.removal.act === "leave" ? "agenda-leave-confirm" : "agenda-remove-confirm")
                            onClicked: setup.remove(one.modelData.id)
                        }
                        Button {
                            flat: true
                            text: setup.sioul.text("agenda-keep")
                            onClicked: setup.removing = ""
                        }
                    }
                }
                // Who sees it, as its server says; shared with a person, or by a link.
                ColumnLayout {
                    id: share

                    readonly property var now: setup.sharing[one.modelData.id] || null
                    readonly property bool can: share.now !== null && share.now.sharer !== undefined && share.now.sharer !== "none" && one.modelData.server.own === true

                    visible: one.modelData.server && one.modelData.server.server !== "local"
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        text: setup.sioul.text("agenda-sharing")
                        color: setup.theme.text
                    }
                    Label {
                        objectName: "agendaSharingLine"
                        Layout.fillWidth: true
                        text: share.now === null ? setup.sioul.text("agenda-sharing-reading") : (share.now.problem || share.now.line || "")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: share.now !== null && (share.now.problem || "") !== "" ? setup.theme.warm : setup.theme.text
                    }
                    Repeater {
                        model: share.can ? share.now.people : []

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
                                color: setup.theme.text
                            }
                            ToolButton {
                                objectName: "agendaUnshare"
                                text: "×"
                                Accessible.name: setup.sioul.textWith("agenda-unshare", "who", sharee.modelData.name)
                                onClicked: setup.sioul.unshareAgenda(one.modelData.id, sharee.modelData.id)
                            }
                        }
                    }
                    // The name on a line of its own, then how and Share, wrapping on a phone.
                    TextField {
                        id: shareWith

                        objectName: "agendaShareWith"
                        visible: share.can
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        placeholderText: setup.sioul.text("agenda-share-with")
                        Accessible.name: placeholderText
                    }
                    Flow {
                        visible: share.can
                        Layout.fillWidth: true
                        spacing: 6

                        CheckBox {
                            id: shareGroup

                            objectName: "agendaShareGroup"
                            visible: share.now !== null && share.now.sharer === "nextcloud"
                            text: setup.sioul.text("agenda-share-group")
                        }
                        PlainComboBox {
                            id: shareAccess

                            readonly property var choices: share.now && share.now.sharer === "google" ? ["read", "write", "busy"] : ["read", "write"]

                            width: 170
                            model: choices.map(c => setup.sioul.text("agenda-share-" + c))
                            Accessible.name: setup.sioul.text("agenda-sharing")
                        }
                        Button {
                            objectName: "agendaShare"
                            enabled: shareWith.text.trim() !== ""
                            text: setup.sioul.text("agenda-share")
                            onClicked: {
                                setup.sioul.shareAgenda(one.modelData.id, (shareGroup.visible && shareGroup.checked ? "group:" : "") + shareWith.text.trim(), shareAccess.choices[shareAccess.currentIndex])
                                shareWith.text = ""
                                shareGroup.checked = false
                            }
                        }
                    }
                    Flow {
                        visible: share.can && share.now.links === true
                        Layout.fillWidth: true
                        spacing: 6

                        CheckBox {
                            objectName: "agendaLink"
                            text: setup.sioul.text("agenda-link-on")
                            checked: share.now !== null && !!share.now.link
                            onToggled: setup.sioul.agendaLink(one.modelData.id, checked)
                        }
                        Button {
                            visible: share.now !== null && !!share.now.link
                            flat: true
                            text: setup.sioul.text("agenda-link-copy")
                            onClicked: setup.sioul.copyRequested(share.now.link)
                        }
                    }
                    Label {
                        visible: share.now !== null && ((share.now.said || "") !== "" || (share.now.note || "") !== "")
                        Layout.fillWidth: true
                        text: share.now === null ? "" : [share.now.said || "", share.now.note || ""].filter(w => w !== "").join(" ")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: setup.theme.muted
                    }
                }
                Label {
                    text: setup.sioul.text("agenda-whose")
                    color: setup.theme.text
                }
                PlainComboBox {
                    objectName: "agendaWhose"
                    Layout.fillWidth: true
                    model: setup.whoseChoices.map(w => setup.sioul.text("agenda-whose-" + w))
                    currentIndex: Math.max(0, setup.whoseChoices.indexOf(one.modelData.whose))
                    Accessible.name: setup.sioul.text("agenda-whose")
                    onActivated: index => setup.save(one.modelData.id, "whose", setup.whoseChoices[index])
                }
                TextField {
                    visible: one.modelData.whose !== "yours"
                    Layout.fillWidth: true
                    text: one.modelData.person
                    placeholderText: setup.sioul.text(one.modelData.whose === "child" ? "agenda-person-child" : "agenda-person")
                    Accessible.name: placeholderText
                    onEditingFinished: if (text.trim() !== one.modelData.person)
                        setup.save(one.modelData.id, "name", text.trim())
                }
                Label {
                    text: setup.sioul.text("agenda-for")
                    color: setup.theme.text
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 12

                    Repeater {
                        model: setup.areas

                        delegate: CheckBox {
                            id: area

                            required property string modelData

                            text: setup.sioul.text("area-" + area.modelData)
                            checked: one.on.indexOf(area.modelData) >= 0
                            onToggled: {
                                const now = setup.areas.filter(a => a === area.modelData ? checked : one.on.indexOf(a) >= 0)
                                setup.save(one.modelData.id, "area", now.join("+"))
                                area.checked = Qt.binding(() => one.on.indexOf(area.modelData) >= 0)
                            }
                        }
                    }
                }
                // A personal agenda: all day, or outside work hours. One for work follows the work hours.
                Label {
                    visible: one.on.indexOf("work") < 0
                    text: setup.sioul.text("agenda-reaches")
                    color: setup.theme.text
                }
                PlainComboBox {
                    readonly property var choices: ["all-day", "outside-work"]

                    visible: one.on.indexOf("work") < 0
                    Layout.fillWidth: true
                    model: choices.map(r => setup.sioul.text("agenda-reaches-" + r))
                    currentIndex: Math.max(0, choices.indexOf(one.modelData.reaches))
                    Accessible.name: setup.sioul.text("agenda-reaches")
                    onActivated: index => setup.save(one.modelData.id, "reaches", choices[index])
                }
                Label {
                    text: setup.sioul.text("agenda-access")
                    color: setup.theme.text
                }
                PlainComboBox {
                    objectName: "agendaAccess"
                    readonly property var choices: setup.accessChoices(one.modelData)

                    Layout.fillWidth: true
                    model: choices.map(a => setup.sioul.text("agenda-access-" + a))
                    currentIndex: Math.max(0, choices.indexOf(one.modelData.access))
                    Accessible.name: setup.sioul.text("agenda-access")
                    onActivated: index => setup.save(one.modelData.id, "access", choices[index])
                }
                // A child's: the other parent who may take its events, how they answer, who usually takes each kind.
                Label {
                    visible: one.modelData.whose === "child"
                    text: setup.sioul.text("agenda-parent")
                    color: setup.theme.text
                }
                TextField {
                    objectName: "agendaParent"
                    visible: one.modelData.whose === "child"
                    Layout.fillWidth: true
                    text: one.modelData.parent || ""
                    placeholderText: setup.sioul.text("agenda-parent-name")
                    Accessible.name: placeholderText
                    onEditingFinished: if (text.trim() !== (one.modelData.parent || ""))
                        setup.save(one.modelData.id, "parent", text.trim())
                }
                TextField {
                    objectName: "agendaParentAddress"
                    visible: one.modelData.whose === "child"
                    Layout.fillWidth: true
                    text: one.modelData.parent_address || ""
                    placeholderText: setup.sioul.text("agenda-parent-address")
                    inputMethodHints: Qt.ImhEmailCharactersOnly
                    Accessible.name: placeholderText
                    onEditingFinished: if (text.trim() !== (one.modelData.parent_address || ""))
                        setup.save(one.modelData.id, "parent_address", text.trim())
                }
                Label {
                    visible: one.modelData.whose === "child"
                    text: setup.sioul.text("agenda-parent-answers")
                    color: setup.theme.text
                }
                PlainComboBox {
                    readonly property var choices: ["mail", "sioul"]

                    visible: one.modelData.whose === "child"
                    Layout.fillWidth: true
                    model: choices.map(c => setup.sioul.text("agenda-parent-answers-" + c))
                    currentIndex: Math.max(0, choices.indexOf(one.modelData.parent_answers))
                    Accessible.name: setup.sioul.text("agenda-parent-answers")
                    onActivated: index => setup.save(one.modelData.id, "parent_answers", choices[index])
                }
                Label {
                    visible: one.modelData.whose === "child"
                    text: setup.sioul.text("agenda-standing")
                    color: setup.theme.text
                }
                Repeater {
                    model: one.modelData.whose === "child" ? setup.view.children.kinds : []

                    delegate: RowLayout {
                        id: standing

                        required property var modelData
                        readonly property var choices: ["", "me", "other"]
                        readonly property string now: (one.modelData.standing || {})[standing.modelData.id] || ""

                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            Layout.preferredWidth: 110
                            text: standing.modelData.label
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: setup.theme.text
                        }
                        PlainComboBox {
                            objectName: "agendaStanding"
                            Layout.fillWidth: true
                            model: standing.choices.map(c => c === "" ? setup.sioul.text("agenda-standing-ask") : c === "me" ? setup.sioul.text("holder-me") : setup.theme.plain(one.modelData.parent || setup.sioul.text("agenda-parent")))
                            currentIndex: Math.max(0, standing.choices.indexOf(standing.now))
                            Accessible.name: setup.sioul.text("agenda-standing") + " · " + standing.modelData.label
                            onActivated: index => {
                                setup.problem = setup.sioul.setStandingHolder(one.modelData.id, standing.modelData.id, standing.choices[index])
                                setup.read()
                            }
                        }
                    }
                }
                // For availability: what its events say shown, off by default (part 4).
                CheckBox {
                    objectName: "agendaTitles"
                    visible: one.modelData.access === "availability"
                    Layout.fillWidth: true
                    text: setup.sioul.text("agenda-titles")
                    checked: one.modelData.titles === true
                    onToggled: setup.save(one.modelData.id, "titles", checked ? "shown" : "")
                }
                Label {
                    visible: one.modelData.server_read_only === true
                    Layout.fillWidth: true
                    text: setup.sioul.text("agenda-server-read-only")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: setup.theme.muted
                }
                // Hidden for good on this device, whatever the hours, until shown again.
                Button {
                    objectName: "agendaKeepHidden"
                    text: setup.sioul.text(one.modelData.kept_hidden === true ? "agenda-show-again" : "agenda-keep-hidden")
                    onClicked: {
                        setup.problem = setup.sioul.keepAgendaHidden(one.modelData.id, one.modelData.kept_hidden !== true)
                        setup.read()
                    }
                }
            }
        }
    }
    // Children's events (part 3): where your stand-in of each kind goes, the follow-up tasks.
    ColumnLayout {
        visible: setup.view.children.any === true
        Layout.fillWidth: true
        Layout.topMargin: 8
        spacing: 4

        Label {
            text: setup.sioul.text("children-events")
            font.bold: true
            color: setup.theme.text
        }
        Label {
            text: setup.sioul.text("children-stand-ins")
            color: setup.theme.text
        }
        Repeater {
            model: setup.view.children.kinds

            delegate: RowLayout {
                id: kind

                required property var modelData
                readonly property var choices: [""].concat(setup.view.children.yours.map(a => a.id))

                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.preferredWidth: 110
                    text: kind.modelData.label
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: setup.theme.text
                }
                PlainComboBox {
                    objectName: "childrenStandIn"
                    Layout.fillWidth: true
                    model: [setup.sioul.text("children-stand-in-usual")].concat(setup.view.children.yours.map(a => setup.theme.plain(a.name)))
                    currentIndex: Math.max(0, kind.choices.indexOf(kind.modelData.stand_in))
                    Accessible.name: setup.sioul.text("children-stand-ins") + " · " + kind.modelData.label
                    onActivated: index => {
                        setup.problem = setup.sioul.setStandInAgenda(kind.modelData.id, kind.choices[index])
                        setup.read()
                    }
                }
                SpinBox {
                    visible: kind.modelData.usual_months > 0 || kind.modelData.months > 0
                    from: 0
                    to: 60
                    value: kind.modelData.months
                    Accessible.name: setup.sioul.text("children-months") + " · " + kind.modelData.label
                    onValueModified: {
                        setup.problem = setup.sioul.setRhythm(kind.modelData.id, value === kind.modelData.usual_months ? -1 : value)
                        setup.read()
                    }
                }
            }
        }
        CheckBox {
            objectName: "childrenFollowUps"
            Layout.fillWidth: true
            text: setup.sioul.text("children-follow-ups")
            checked: setup.view.children.follow_ups === true
            onToggled: {
                setup.problem = setup.sioul.setFollowUps(checked)
                setup.read()
            }
        }
    }

    // Your availability (part 5): your busy times shared with one person, never what fills them.
    ColumnLayout {
        id: busy

        readonly property var current: setup.availability.current || null
        readonly property var account: (setup.availability.accounts || [])[busyAccount.currentIndex] || null
        property var picked: ({})
        property var words: ({})

        visible: (setup.availability.accounts || []).length > 0
        Layout.fillWidth: true
        Layout.topMargin: 8
        spacing: 4

        Label {
            text: setup.sioul.text("availability-title")
            font.bold: true
            color: setup.theme.text
        }
        Label {
            objectName: "availabilityLine"
            visible: busy.current !== null
            Layout.fillWidth: true
            text: setup.availability.line || ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: setup.theme.text
        }
        Button {
            objectName: "availabilityStop"
            visible: busy.current !== null
            text: setup.sioul.text("availability-stop")
            onClicked: {
                setup.problem = setup.sioul.stopAvailability()
                setup.readAvailability()
            }
        }
        Button {
            objectName: "availabilityOpen"
            visible: busy.current === null && !setup.sharingAvailability
            text: setup.sioul.text("availability-title")
            onClicked: setup.sharingAvailability = true
        }
        ColumnLayout {
            visible: busy.current === null && setup.sharingAvailability
            Layout.fillWidth: true
            spacing: 4

            PlainComboBox {
                id: busyAccount

                Layout.fillWidth: true
                model: (setup.availability.accounts || []).map(a => setup.theme.plain(a.name))
                Accessible.name: setup.sioul.text("availability-from")
            }
            Repeater {
                model: (setup.availability.agendas || []).filter(a => !busy.account || !busy.account.google || a.account === busy.account.id)

                delegate: RowLayout {
                    id: source

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 6

                    CheckBox {
                        objectName: "availabilityAgenda"
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: setup.theme.plain(source.modelData.name)
                        onToggled: {
                            const next = Object.assign({}, busy.picked)
                            next[source.modelData.id] = checked
                            busy.picked = next
                        }
                    }
                    TextField {
                        visible: busy.picked[source.modelData.id] === true && !(busy.account && busy.account.google)
                        Layout.preferredWidth: 140
                        placeholderText: setup.sioul.text("availability-word")
                        Accessible.name: placeholderText
                        onTextEdited: {
                            const next = Object.assign({}, busy.words)
                            next[source.modelData.id] = text
                            busy.words = next
                        }
                    }
                }
            }
            TextField {
                id: busyWith

                objectName: "availabilityWith"
                Layout.fillWidth: true
                placeholderText: setup.sioul.text("availability-with")
                Accessible.name: placeholderText
            }
            CheckBox {
                id: busyLink

                visible: !(busy.account && busy.account.google)
                text: setup.sioul.text("availability-link")
            }
            RowLayout {
                spacing: 8

                Button {
                    objectName: "availabilityShare"
                    text: setup.sioul.text("availability-share")
                    onClicked: {
                        const agendas = Object.keys(busy.picked).filter(k => busy.picked[k])
                        setup.problem = setup.sioul.shareAvailability(JSON.stringify({ account: busy.account ? busy.account.id : "", agendas: agendas, with: busyWith.text.trim(), link: busyLink.visible && busyLink.checked, words: busy.words }))
                        if (setup.problem === "") {
                            setup.sharingAvailability = false
                            setup.readAvailability()
                        }
                    }
                }
                Button {
                    flat: true
                    text: setup.sioul.text("agenda-keep")
                    onClicked: setup.sharingAvailability = false
                }
            }
        }
    }
}
