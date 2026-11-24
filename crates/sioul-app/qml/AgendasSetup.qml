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
// where its server shares. Your availability is its own block, below
// (AvailabilitySetup.qml).

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
    property string viewText: ""
    property string problem: ""
    // The agendas opened, by place, while the panel lives.
    property var opened: ({})
    // Each agenda's sharing as its server last answered (`agendaShared`), by place.
    property var sharing: ({})
    // The removal asked of an agenda: its place, and what it would do (`agendaRemoval`).
    property string removing: ""
    property var removal: ({})
    // "New agenda…" open.
    property bool making: false
    // "Subscribe to a calendar…" open (SubscribeForm.qml).
    property bool subscribing: false
    readonly property var colours: ["#4c6b5c", "#3f6f8f", "#7a5c99", "#b07d3f", "#a3504f", "#5f7f3a", "#5b6470", "#c06c8d"]

    // A calendar of busy blocks pressed: Agenda ⚙ ▸ Your availability, where it is kept, shown.
    signal availabilityAsked

    readonly property var areas: ["work", "admin", "leisure"]

    // Read again; `kept`, the same answer changes nothing (its rows, their open menus, stay).
    function read(kept) {
        const text = setup.sioul.agendasSetup()
        if (kept === true && text === setup.viewText)
            return
        setup.viewText = text
        try {
            setup.view = JSON.parse(text)
        } catch (e) {
            setup.view = { agendas: [], pairs: [], children: { kinds: [], yours: [], follow_ups: true, any: false } }
        }
    }

    // One field written ("" back to what Sioul saw); the agendas read again.
    function save(id, field, value) {
        setup.problem = setup.sioul.setAgendaField(id, field, value)
        setup.read()
    }

    // Sioul's window, with its own properties (its locale), for the person's form.
    function sioulWindow(): var {
        return setup.Window.window
    }

    // A person chosen for an agenda ("person", "parent_person"): the key and the name beside it.
    function setPerson(id, field, person) {
        setup.problem = setup.sioul.setAgendaPerson(id, field, person)
        setup.read()
    }

    // "New person…" from an agenda's chooser: the person's form, its relation the agenda's; chosen once saved.
    function newPerson(agenda, field) {
        const household = JSON.parse(setup.sioul.household() || "{}")
        const dialog = personForm.now()
        dialog.relations = household.relations || []
        dialog.children = (household.persons || []).filter(p => p.relation === "child").map(p => ({ id: p.id, name: p.name }))
        dialog.place = agenda.id
        dialog.field = field
        const relation = field === "parent_person" ? "other-parent" : ({ child: "child", partner: "partner", parent: "other-parent" })[agenda.whose] || ""
        dialog.begin("", "", relation, field === "parent_person" && agenda.person_id ? [agenda.person_id] : [])
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

        // An agenda's role or person, or a person, changed here or on another device:
        // the agendas and their choosers read again.
        function onAgendasChanged() {
            setup.read(true)
        }

        function onPersonsChanged() {
            setup.read(true)
        }

        function onAgendaShared(id, answer) {
            if (id === "availability")
                return
            const next = Object.assign({}, setup.sharing)
            next[id] = JSON.parse(answer || "{}")
            setup.sharing = next
        }
    }

    // "New agenda…" opened afresh, its role said (the window's pictures: someone else's, a name typed).
    function askNew(whose, name) {
        setup.subscribing = false
        making.begin("", "")
        making.whose = whose
        making.name = name
        setup.making = true
    }

    // "Subscribe to a calendar…": its form, opened afresh (also from Settings ▸ Hours, through AgendaPage's `askSubscribe`).
    function askSubscribe() {
        setup.making = false
        setup.subscribing = true
        subscribeForm.begin()
    }

    // One field of a subscribed calendar written; the agendas read again.
    function saveSubscription(id, field, value) {
        setup.problem = setup.sioul.setSubscriptionField(id, field, value)
        setup.read()
    }

    // A list of tasks alone (it takes no events): renamed and removed in Tasks ⚙ (TaskListsSetup.qml), not here.
    function tasksOnly(agenda) {
        return agenda.tasks === true && agenda.events !== true
    }

    // What may be chosen of what you may do with it: never writing where its server keeps it read only.
    function accessChoices(agenda) {
        return agenda.server_read_only ? ["read", "availability"] : ["write", "read", "availability"]
    }

    objectName: "agendasSetup"
    spacing: 4
    Component.onCompleted: setup.read()

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

    // A new agenda, with its role (part 5): made here, and on its server by the next sync;
    // or a calendar subscribed by its address, read by each device (SubscribeForm.qml).
    Flow {
        visible: !setup.making && !setup.subscribing
        Layout.fillWidth: true
        spacing: 8

        Button {
            objectName: "agendaNew"
            visible: (setup.view.accounts || []).length > 0
            text: setup.sioul.text("agenda-new")
            onClicked: {
                making.begin("", "")
                setup.making = true
            }
        }
        Button {
            objectName: "agendaSubscribe"
            text: setup.sioul.text("subscribe-open")
            onClicked: setup.askSubscribe()
        }
    }
    SubscribeForm {
        id: subscribeForm

        persons: setup.view.persons || ({})
        visible: setup.subscribing
        Layout.fillWidth: true
        sioul: setup.sioul
        theme: setup.theme
        onDone: id => {
            setup.subscribing = false
            setup.read()
            const opened = Object.assign({}, setup.opened)
            opened["subscribed/" + id] = true
            setup.opened = opened
        }
        onCancelled: setup.subscribing = false
    }
    NewAgendaForm {
        id: making

        visible: setup.making
        Layout.fillWidth: true
        sioul: setup.sioul
        theme: setup.theme
        accounts: setup.view.accounts || []
        persons: setup.view.persons || ({})
        onMade: place => {
            setup.making = false
            setup.read()
        }
        onKept: setup.making = false
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
                    // A calendar of busy blocks Sioul keeps: nothing of its own to choose; it opens Your availability.
                    readonly property bool mirror: one.modelData.mirror === true

                    objectName: "agendaFold"
                    text: mirror ? "→" : one.open ? "▾" : "▸"
                    Accessible.name: mirror ? setup.sioul.text("agenda-mirror-open") : setup.sioul.textWith(one.open ? "agendas-fold" : "agendas-unfold", "group", one.modelData.name)
                    onClicked: mirror ? setup.availabilityAsked() : setup.toggle(one.modelData.id)
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
                    objectName: "agendaLineLabel"
                    text: one.modelData.mirror === true
                        ? one.modelData.name + " · " + (one.modelData.kept_for ? setup.sioul.textWith("agenda-mirror-kept-for", "name", one.modelData.kept_for) : setup.sioul.text("agenda-mirror-kept"))
                        : one.modelData.name + " · " + setup.sioul.text("agenda-whose-" + one.modelData.whose) + (one.modelData.person !== "" ? " (" + one.modelData.person + ")" : "") + (one.modelData.kept_hidden === true ? " · " + setup.sioul.text("agendas-kept-hidden") : "")
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: setup.theme.text

                    TapHandler {
                        onTapped: one.modelData.mirror === true ? setup.availabilityAsked() : setup.toggle(one.modelData.id)
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
                // A calendar subscribed by its address: when this device last read it, or why not;
                // Refresh now, Change the address, Stop; its name; whether it holds holidays.
                ColumnLayout {
                    id: feed

                    readonly property var sub: one.modelData.subscription || null
                    // Its settings do not read here (F19, F20): its line says so, Stop alone stays.
                    readonly property bool unread: feed.sub !== null && feed.sub.unread === true
                    property bool changing: false
                    property bool stopping: false

                    visible: feed.sub !== null
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        objectName: "subscriptionLine"
                        Layout.fillWidth: true
                        text: feed.sub ? feed.sub.line : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: setup.theme.text
                    }
                    Flow {
                        visible: !feed.changing && !feed.stopping
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            objectName: "subscriptionRefresh"
                            visible: !feed.unread
                            text: setup.sioul.text("subscribe-refresh")
                            onClicked: setup.problem = setup.sioul.refreshSubscription(feed.sub.id)
                        }
                        Button {
                            objectName: "subscriptionChange"
                            visible: !feed.unread
                            text: setup.sioul.text("subscribe-change")
                            onClicked: {
                                newAddress.text = feed.sub.url
                                feed.changing = true
                            }
                        }
                        Button {
                            objectName: "subscriptionStop"
                            text: setup.sioul.text("subscribe-stop")
                            onClicked: feed.stopping = true
                        }
                    }
                    TextField {
                        id: newAddress

                        objectName: "subscriptionAddress"
                        visible: feed.changing
                        Layout.fillWidth: true
                        inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
                        Accessible.name: setup.sioul.text("subscribe-address")
                    }
                    Flow {
                        visible: feed.changing
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            objectName: "subscriptionAddressSave"
                            enabled: newAddress.text.trim() !== ""
                            text: setup.sioul.text("subscribe-change-save")
                            onClicked: {
                                setup.saveSubscription(feed.sub.id, "url", newAddress.text.trim())
                                if (setup.problem === "")
                                    feed.changing = false
                            }
                        }
                        Button {
                            flat: true
                            text: setup.sioul.text("agenda-keep")
                            onClicked: feed.changing = false
                        }
                    }
                    Label {
                        visible: feed.stopping
                        Layout.fillWidth: true
                        text: setup.sioul.text("subscribe-stop-question")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: setup.theme.text
                    }
                    Flow {
                        visible: feed.stopping
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            objectName: "subscriptionStopConfirm"
                            text: setup.sioul.text("subscribe-stop-confirm")
                            onClicked: {
                                setup.problem = setup.sioul.stopSubscription(feed.sub.id)
                                feed.stopping = false
                                setup.read()
                            }
                        }
                        Button {
                            flat: true
                            text: setup.sioul.text("agenda-keep")
                            onClicked: feed.stopping = false
                        }
                    }
                    TextField {
                        objectName: "subscriptionName"
                        visible: !feed.unread
                        Layout.fillWidth: true
                        text: feed.sub ? feed.sub.name : ""
                        placeholderText: setup.theme.plain(one.modelData.name)
                        Accessible.name: setup.sioul.text("subscribe-name")
                        onEditingFinished: if (feed.sub && text.trim() !== feed.sub.name)
                            setup.saveSubscription(feed.sub.id, "name", text.trim())
                    }
                    PlainComboBox {
                        readonly property var kinds: ["", "public", "school"]

                        objectName: "subscriptionKind"
                        visible: !feed.unread
                        Layout.fillWidth: true
                        model: kinds.map(k => setup.sioul.text("subscribe-holidays-" + (k === "" ? "none" : k)))
                        currentIndex: feed.sub ? Math.max(0, kinds.indexOf(feed.sub.holidays)) : 0
                        Accessible.name: setup.sioul.text("subscribe-holidays")
                        onActivated: index => setup.saveSubscription(feed.sub.id, "holidays", kinds[index])
                    }
                    // Whether its days count as your days off is Settings ▸ Hours' alone (VacationsCalendars.qml).
                    // Its events mark you busy and hold your time only when you say they are yours (F41):
                    // the same words, and the same line under them, as an extension agenda's switch.
                    WrapCheckBox {
                        id: engagements

                        objectName: "subscriptionEngagements"
                        // Not for another person's (a co-worker's): never your time.
                        visible: !feed.unread && feed.sub !== null && feed.sub.holidays === "" && ["partner", "parent", "someone"].indexOf(one.modelData.whose) < 0
                        Layout.fillWidth: true
                        text: setup.sioul.text("agenda-busy-time")
                        checked: feed.sub !== null && feed.sub.engagements === true
                        onToggled: setup.saveSubscription(feed.sub.id, "engagements", checked ? "true" : "false")
                    }
                    Label {
                        objectName: "subscriptionEngagementsHelp"
                        visible: engagements.visible
                        Layout.fillWidth: true
                        text: setup.sioul.text("agenda-busy-time-help")
                        wrapMode: Text.Wrap
                        color: setup.theme.muted
                    }
                    WrapCheckBox {
                        objectName: "subscriptionAfterClass"
                        visible: !feed.unread && feed.sub !== null && feed.sub.holidays === "school"
                        Layout.fillWidth: true
                        text: setup.sioul.text("subscribe-after-class")
                        checked: feed.sub !== null && feed.sub.after_class === true
                        onToggled: setup.saveSubscription(feed.sub.id, "after_class", checked ? "true" : "false")
                    }
                    // Its colour in the agenda, kept with its settings (`colour`): no server to ask.
                    Flow {
                        visible: !feed.unread
                        Layout.fillWidth: true
                        spacing: 6

                        Repeater {
                            model: setup.colours

                            delegate: Rectangle {
                                id: subDot

                                required property string modelData

                                objectName: "subscriptionColour"
                                width: 22
                                height: 22
                                radius: 11
                                color: subDot.modelData
                                border.width: (one.modelData.colour || "").toLowerCase().startsWith(subDot.modelData) ? 3 : 0
                                border.color: setup.theme.text
                                Accessible.role: Accessible.Button
                                Accessible.name: setup.sioul.text("agenda-colour") + " " + subDot.modelData

                                TapHandler {
                                    onTapped: setup.saveSubscription(feed.sub.id, "colour", subDot.modelData)
                                }
                            }
                        }
                    }
                }
                // On its server (part 5): yours renamed, recoloured, removed; someone else's stopped showing.
                // A list of tasks alone is renamed and removed (or stopped showing) in Tasks ⚙ ▸ Task
                // lists, its one place, by the same calls (TaskListsSetup.qml).
                TextField {
                    objectName: "agendaRename"
                    visible: one.modelData.server && one.modelData.server.own === true && !setup.tasksOnly(one.modelData)
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
                    visible: one.modelData.server && one.modelData.server.server !== undefined && (one.modelData.server.own === true || one.modelData.server.server !== "local") && !setup.tasksOnly(one.modelData) && setup.removing !== one.modelData.id
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
                            visible: setup.removal.act === "remove" || setup.removal.act === "leave" || setup.removal.act === "hide"
                            text: setup.sioul.text(setup.removal.act === "remove" ? "agenda-remove-confirm" : "agenda-leave-confirm")
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
                    }
                    // The link itself, to select or copy.
                    LinkLine {
                        objectName: "agendaLinkLine"
                        visible: share.can && share.now.links === true && !!share.now.link
                        Layout.fillWidth: true
                        sioul: setup.sioul
                        theme: setup.theme
                        link: share.now !== null ? share.now.link || "" : ""
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
                // Whose it is, and whose name: the one control, as the forms that make a calendar have it.
                WhoseFields {
                    Layout.fillWidth: true
                    sioul: setup.sioul
                    theme: setup.theme
                    persons: setup.view.persons || ({})
                    whose: one.modelData.whose
                    personId: one.modelData.person_id || ""
                    name: one.modelData.person || ""
                    onWhoseChosen: whose => setup.save(one.modelData.id, "whose", whose)
                    onPersonChosen: id => setup.setPerson(one.modelData.id, "person", id)
                    onNameGiven: name => setup.save(one.modelData.id, "name", name)
                    onNewPersonAsked: setup.newPerson(one.modelData, "person")
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
                // The other parent: a person, a child's other parent; their address stays this agenda's (where invitations go).
                PersonChooser {
                    objectName: "agendaParent"
                    visible: one.modelData.whose === "child"
                    Layout.fillWidth: true
                    sioul: setup.sioul
                    theme: setup.theme
                    persons: (setup.view.persons || {}).parent || []
                    personId: one.modelData.parent_person || ""
                    fallback: one.modelData.parent || ""
                    Accessible.name: setup.sioul.text("agenda-parent-name")
                    onChosen: id => setup.setPerson(one.modelData.id, "parent_person", id)
                    onNewAsked: setup.newPerson(one.modelData, "parent_person")
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
                    // A calendar read by its address has no server to say so: its own words.
                    text: setup.sioul.text(one.modelData.subscription ? "agenda-subscribed-read-only" : "agenda-server-read-only")
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

    // A person's form, from an agenda's "New person…": chosen for that agenda once saved.
    Later {
        id: personForm

        sourceComponent: Component {
            PersonDialog {
                id: agendaPersonForm

                property string place: ""
                property string field: ""

                sioul: setup.sioul
                theme: setup.theme
                window: setup.sioulWindow()
                onSaved: id => {
                    if (id !== "" && agendaPersonForm.place !== "")
                        setup.setPerson(agendaPersonForm.place, agendaPersonForm.field, id)
                }
            }
        }
    }
}
