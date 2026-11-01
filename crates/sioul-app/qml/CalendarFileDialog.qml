// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A calendar file (.ics) opened in Sioul (calendarfile.rs): its events, the
// agenda they go into and the categories they take, then Add to the agenda
// or Cancel. What the file says (titles, places, its organizer) is drawn as
// plain text; nothing in it is followed. One already in the chosen agenda is
// written over, and the dialog says so before.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme

    // calendarfile.rs's view: {problem, events: [{uid, title, when, there, place, organizer, notes}], calendars: [{id, name}], categories}.
    property var shown: ({ problem: "", events: [], calendars: [], categories: [] })
    // The categories chosen for its events.
    property var tags: []
    // The UIDs the chosen agenda holds already.
    property var known: []
    property string problem: ""
    property bool added: false

    function show(json) {
        dialog.shown = JSON.parse(json || "{}")
        dialog.tags = []
        dialog.problem = ""
        dialog.added = false
        agenda.currentIndex = 0
        dialog.readKnown()
        dialog.open()
    }

    function calendarId() {
        const all = dialog.shown.calendars || []
        return all.length > 0 ? all[Math.max(0, agenda.currentIndex)].id : ""
    }

    function readKnown() {
        const id = dialog.calendarId()
        dialog.known = id === "" ? [] : JSON.parse(dialog.sioul.calendarFileKnown(id) || "[]")
    }

    function addTag(text) {
        const tag = text.trim().replace(/^#/, "")
        if (tag !== "" && dialog.tags.findIndex(t => t.toLowerCase() === tag.toLowerCase()) < 0)
            dialog.tags = dialog.tags.concat([tag])
        newTag.editText = ""
        newTag.currentIndex = -1
    }

    function add() {
        const problem = dialog.sioul.addCalendarFile(dialog.calendarId(), JSON.stringify(dialog.tags))
        if (problem) {
            dialog.problem = problem
            return
        }
        dialog.added = true
        dialog.close()
    }

    // Closed without Add (Cancel, Escape, a tap beside it): nothing written, the next file shown.
    onClosed: {
        if (!dialog.added)
            dialog.sioul.closeCalendarFile()
    }

    // Over the whole window, whatever item made it (a Later's is 0 by 0).
    parent: Overlay.overlay
    anchors.centerIn: Overlay.overlay
    modal: true
    // About 60 characters of the body font; never past the window less the usual margins, all of a phone's.
    width: Math.min(Math.ceil(60 * body.averageCharacterWidth) + dialog.leftPadding + dialog.rightPadding + 12, (parent ? dialog.roomWidth : 800) - 2 * dialog.theme.gap)
    height: Math.min(implicitHeight, dialog.roomHeight - 32)
    title: dialog.sioul.text("calendar-file-title")

    FontMetrics {
        id: body

        font: dialog.font
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 10

            // Why it does not open, calmly.
            Label {
                objectName: "calendarFileProblem"
                visible: (dialog.shown.problem || "") !== ""
                Layout.fillWidth: true
                text: dialog.shown.problem || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.muted
            }

            Repeater {
                model: dialog.shown.events || []

                delegate: ColumnLayout {
                    id: event

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        text: event.modelData.title
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.bold: true
                        color: dialog.theme.text
                    }
                    Label {
                        Layout.fillWidth: true
                        text: event.modelData.when
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: dialog.theme.text
                    }
                    // On its own clock, both times.
                    Label {
                        visible: event.modelData.there !== ""
                        Layout.fillWidth: true
                        text: event.modelData.there
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: dialog.theme.muted
                    }
                    Label {
                        visible: event.modelData.place !== ""
                        Layout.fillWidth: true
                        text: event.modelData.place
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: dialog.theme.muted
                    }
                    Label {
                        visible: event.modelData.organizer !== ""
                        Layout.fillWidth: true
                        text: event.modelData.organizer
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: dialog.theme.muted
                    }
                    Label {
                        visible: event.modelData.notes !== ""
                        Layout.fillWidth: true
                        text: event.modelData.notes
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        maximumLineCount: 4
                        elide: Text.ElideRight
                        font.pixelSize: 13
                        color: dialog.theme.muted
                    }
                    // Already in the agenda chosen: said before it is written over.
                    Label {
                        objectName: "calendarFileKnown"
                        visible: dialog.known.indexOf(event.modelData.uid) >= 0
                        Layout.fillWidth: true
                        text: dialog.sioul.text("calendar-file-known")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: dialog.theme.warm
                    }
                }
            }

            Label {
                visible: (dialog.shown.events || []).length > 0
                text: dialog.sioul.text("calendar-file-agenda")
                color: dialog.theme.muted
            }
            PlainComboBox {
                id: agenda

                objectName: "calendarFileAgenda"
                visible: (dialog.shown.events || []).length > 0
                Layout.fillWidth: true
                model: (dialog.shown.calendars || []).map(c => dialog.theme.plain(c.name))
                Accessible.name: dialog.sioul.text("calendar-file-agenda")
                onActivated: dialog.readKnown()
            }
            Label {
                visible: (dialog.shown.events || []).length > 0 && (dialog.shown.calendars || []).length === 0
                Layout.fillWidth: true
                text: dialog.sioul.text("dav-no-calendar")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.warm
            }

            Label {
                visible: (dialog.shown.events || []).length > 0
                text: dialog.sioul.text("calendar-file-tags")
                color: dialog.theme.muted
            }
            // Its tags: each taken off with ×, one more typed or chosen among those in use.
            Flow {
                visible: (dialog.shown.events || []).length > 0
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: dialog.tags

                    delegate: Rectangle {
                        id: tag

                        required property string modelData

                        implicitWidth: tagRow.implicitWidth + 12
                        implicitHeight: tagRow.implicitHeight + 4
                        radius: height / 2
                        color: "transparent"
                        border.color: dialog.theme.line

                        Row {
                            id: tagRow

                            anchors.centerIn: parent
                            spacing: 2

                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                text: "#" + tag.modelData
                                textFormat: Text.PlainText
                                font.pixelSize: 13
                                color: dialog.theme.text
                            }
                            ToolButton {
                                implicitWidth: 22
                                implicitHeight: 22
                                text: "×"
                                Accessible.name: dialog.sioul.textWith("task-tag-remove", "tag", tag.modelData)
                                onClicked: dialog.tags = dialog.tags.filter(t => t !== tag.modelData)
                            }
                        }
                    }
                }
                PlainComboBox {
                    id: newTag

                    readonly property var others: (dialog.shown.categories || []).filter(c => dialog.tags.indexOf(c) < 0)

                    width: 170
                    editable: true
                    model: newTag.others.map(c => dialog.theme.plain(c))
                    currentIndex: -1
                    displayText: ""
                    Accessible.name: dialog.sioul.text("task-tag-add")

                    onAccepted: dialog.addTag(newTag.editText)
                    onActivated: index => dialog.addTag(newTag.others[index])

                    contentItem: TextField {
                        text: newTag.editText
                        placeholderText: "+ " + dialog.sioul.text("task-tag-add")
                        font.pixelSize: 13
                        background: Item {}
                        onTextEdited: newTag.editText = text
                        onAccepted: dialog.addTag(text)
                    }
                }
            }
        }
    }

    footer: ColumnLayout {
        spacing: 6

        Label {
            visible: dialog.problem !== ""
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 12
            Layout.rightMargin: 12
            Layout.bottomMargin: 12

            Item {
                Layout.fillWidth: true
            }
            Button {
                objectName: "calendarFileCancel"
                text: (dialog.shown.events || []).length > 0 ? dialog.sioul.text("ui-cancel") : dialog.sioul.text("ui-close")
                onClicked: dialog.close()
            }
            Button {
                objectName: "calendarFileAdd"
                visible: (dialog.shown.events || []).length > 0
                enabled: (dialog.shown.calendars || []).length > 0
                text: dialog.sioul.text("calendar-file-add")
                highlighted: true
                onClicked: dialog.add()
            }
        }
    }
}
