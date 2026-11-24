// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Tasks ⚙ ▸ Task lists (one canonical place per setting, 24 November 2026):
// each list that holds only tasks, with what Agenda ⚙ ▸ Your agendas offered
// for it before, by the same calls: yours renamed here and on its server at
// the next sync (`renameAgenda`, `servers::rename`, with Undo); removed only
// on a second press, after one sentence says what goes and what its server
// keeps (`agendaRemoval`), a dated copy of the whole list written first
// (`removeAgenda`, `servers::remove`: the same guard and the same copy as
// for a calendar); someone else's stopped showing, or hidden on this device
// when its server does not say whose it is. A calendar that takes events
// too is renamed and removed on its line in Your agendas; whose a list is,
// what it is for and who sees it stay there with the other agendas.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: lists

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The agendas as Your agendas reads them (`agendasSetup`): {agendas: [{id, name, place, colour, events, tasks, server}]}.
    property var view: ({ agendas: [] })
    property string problem: ""
    // The list whose removal is asked, by its place; and what removing it would do ({act, sentence, note}).
    property string removing: ""
    property var removal: ({})
    // The lists that hold only tasks; a calendar of busy blocks Sioul keeps is never one.
    readonly property var shown: (lists.view.agendas || []).filter(a => a.tasks === true && a.events !== true && a.mirror !== true)

    function read() {
        try {
            lists.view = JSON.parse(lists.sioul.agendasSetup() || "{}")
        } catch (e) {
            lists.view = { agendas: [] }
        }
        if (!lists.view.agendas)
            lists.view = { agendas: [] }
    }

    // The removal asked first, in one sentence; done on the second press (as in Your agendas).
    function askRemoval(id) {
        lists.removal = JSON.parse(lists.sioul.agendaRemoval(id) || "{}")
        lists.removing = id
    }
    function remove(id) {
        lists.problem = lists.sioul.removeAgenda(id)
        lists.removing = ""
        lists.read()
    }

    objectName: "taskListsSetup"
    spacing: 8
    Component.onCompleted: lists.read()

    Connections {
        target: lists.sioul

        // A list made, renamed, removed or changed, here or on another device.
        function onAgendasChanged() {
            lists.read()
        }
    }

    Repeater {
        model: lists.shown

        delegate: ColumnLayout {
            id: one

            required property var modelData
            readonly property bool own: one.modelData.server !== undefined && one.modelData.server !== null && one.modelData.server.own === true

            objectName: "taskListLine"
            Layout.fillWidth: true
            spacing: 4

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Rectangle {
                    Layout.preferredWidth: 10
                    Layout.preferredHeight: 10
                    radius: 5
                    color: one.modelData.colour || lists.theme.accent
                }
                // Yours: its name to change, sent by the next sync (with Undo on the status line).
                TextField {
                    objectName: "taskListRename"
                    visible: one.own
                    Layout.fillWidth: true
                    text: one.modelData.name
                    Accessible.name: lists.sioul.text("agenda-new-name")
                    onEditingFinished: if (text.trim() !== "" && text.trim() !== one.modelData.name) {
                        lists.problem = lists.sioul.renameAgenda(one.modelData.id, text.trim())
                        lists.read()
                    }
                }
                Label {
                    objectName: "taskListName"
                    visible: !one.own
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    text: one.modelData.name
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: lists.theme.text
                }
            }
            Label {
                visible: (one.modelData.place || "") !== ""
                Layout.fillWidth: true
                Layout.leftMargin: 16
                text: one.modelData.place || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: lists.theme.muted
            }
            Button {
                objectName: "taskListRemove"
                visible: one.modelData.server && one.modelData.server.server !== undefined && (one.own || one.modelData.server.server !== "local") && lists.removing !== one.modelData.id
                Layout.leftMargin: 16
                text: lists.sioul.text(one.own ? "agenda-remove" : "agenda-stop-showing")
                onClicked: lists.askRemoval(one.modelData.id)
            }
            ColumnLayout {
                visible: lists.removing === one.modelData.id
                Layout.fillWidth: true
                Layout.leftMargin: 16
                spacing: 4

                Label {
                    objectName: "taskListRemoveQuestion"
                    Layout.fillWidth: true
                    text: lists.removal.sentence || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: lists.theme.text
                }
                Label {
                    visible: (lists.removal.note || "") !== ""
                    Layout.fillWidth: true
                    text: lists.removal.note || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: lists.theme.muted
                }
                RowLayout {
                    spacing: 8

                    Button {
                        objectName: "taskListRemoveConfirm"
                        visible: lists.removal.act === "remove" || lists.removal.act === "leave" || lists.removal.act === "hide"
                        text: lists.sioul.text(lists.removal.act === "remove" ? "agenda-remove-confirm" : "agenda-leave-confirm")
                        onClicked: lists.remove(one.modelData.id)
                    }
                    Button {
                        flat: true
                        text: lists.sioul.text("agenda-keep")
                        onClicked: lists.removing = ""
                    }
                }
            }
        }
    }
    Label {
        visible: lists.problem !== ""
        Layout.fillWidth: true
        text: lists.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: lists.theme.warm
    }
}
