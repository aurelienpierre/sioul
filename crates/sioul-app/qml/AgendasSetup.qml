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
// the follow-up tasks (`children`).

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
    }

    // What may be chosen of what you may do with it: never writing where its server keeps it read only.
    function accessChoices(agenda) {
        return agenda.server_read_only ? ["read", "availability"] : ["write", "read", "availability"]
    }

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
}
