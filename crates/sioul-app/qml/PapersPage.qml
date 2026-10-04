// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The papers wallet: the papers asked again and again, by family (identity,
// health, home, money, warranties), each with where it stands in words (valid
// until, time to renew, older than what is usually asked), its file one click
// away, its renewal planned as a task. No red, no count.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var shown: ({ store: false, problem: "", kinds: [], families: [] })
    property string problem: ""
    // The paper to show once the page is read: its id.
    property string wanted: ""
    property alias dialog: paperDialog

    function reload() {
        page.shown = JSON.parse(page.sioul.papers())
        if (page.wanted !== "") {
            const id = page.wanted
            page.wanted = ""
            page.open(id)
        }
    }

    // A new paper, from nothing.
    function startNew() {
        page.reload()
        paperDialog.begin("", "", "other")
    }

    // A paper made from a file kept (an attachment): what it is, asked.
    function startFrom(file, title, kind) {
        page.reload()
        paperDialog.begin(file, title, kind)
    }

    // A paper of the wallet, its form open.
    function open(id) {
        for (const family of page.shown.families)
            for (const paper of family.papers)
                if (paper.id === id) {
                    paperDialog.edit(paper)
                    return
                }
        page.wanted = id
    }

    onVisibleChanged: if (visible) page.reload()
    Component.onCompleted: page.reload()

    ScrollView {
        id: scroll

        anchors.fill: parent
        anchors.margins: page.theme.gap
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: Math.min(scroll.availableWidth, 760)
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("ui-papers")
                    font.pixelSize: 22
                    color: page.theme.text
                }
                Button {
                    enabled: page.shown.store
                    text: page.sioul.text("papers-add")
                    icon.name: "list-add"
                    onClicked: page.startNew()
                }
            }
            Label {
                Layout.fillWidth: true
                text: page.sioul.text("papers-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            Label {
                visible: page.shown.problem !== "" || page.problem !== ""
                Layout.fillWidth: true
                text: page.problem !== "" ? page.problem : page.shown.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.warm
            }
            Label {
                visible: page.shown.store && page.shown.families.length === 0 && page.shown.problem === ""
                Layout.fillWidth: true
                Layout.topMargin: 12
                text: page.sioul.text("papers-none")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            Repeater {
                model: page.shown.families

                delegate: ColumnLayout {
                    id: family

                    required property var modelData

                    Layout.fillWidth: true
                    Layout.topMargin: 12
                    spacing: 6

                    Label {
                        text: family.modelData.label
                        textFormat: Text.PlainText
                        font.pixelSize: 17
                        font.weight: Font.DemiBold
                        color: page.theme.accent
                    }
                    Repeater {
                        model: family.modelData.papers

                        delegate: Rectangle {
                            id: row

                            required property var modelData
                            readonly property bool attention: ["renew", "ended", "old"].includes(row.modelData.standing)

                            Layout.fillWidth: true
                            implicitHeight: rowLayout.implicitHeight + 16
                            color: area.containsMouse ? page.theme.hover : page.theme.surface
                            border.color: page.theme.line
                            radius: 4

                            MouseArea {
                                id: area

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: paperDialog.edit(row.modelData)
                            }
                            RowLayout {
                                id: rowLayout

                                anchors.fill: parent
                                anchors.margins: 8
                                spacing: 10

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    Label {
                                        Layout.fillWidth: true
                                        text: row.modelData.holder === "" ? row.modelData.title : row.modelData.title + " · " + row.modelData.holder
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: page.theme.text
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        // The kind, unless the name says it already.
                                        readonly property string kind: row.modelData.title.toLowerCase().startsWith(row.modelData.kind_label.toLowerCase()) ? "" : row.modelData.kind_label

                                        text: [kind, row.modelData.line].filter(t => t !== "").join(" · ")
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        font.pixelSize: 13
                                        color: row.attention ? page.theme.warm : page.theme.muted
                                    }
                                    Label {
                                        visible: row.modelData.file !== "" && !row.modelData.file_there
                                        Layout.fillWidth: true
                                        text: page.sioul.text("papers-file-missing")
                                        wrapMode: Text.Wrap
                                        font.pixelSize: 13
                                        color: page.theme.muted
                                    }
                                }
                                Button {
                                    visible: row.modelData.can_renew && row.attention
                                    text: page.sioul.text("papers-plan-renewal")
                                    onClicked: {
                                        page.problem = page.sioul.planRenewal(row.modelData.id)
                                        page.reload()
                                    }
                                }
                                Button {
                                    visible: row.modelData.renewal !== ""
                                    flat: true
                                    text: page.sioul.text("papers-renewal-planned")
                                    onClicked: page.window.openTask(row.modelData.renewal)
                                }
                                Button {
                                    visible: row.modelData.file_there
                                    text: page.sioul.text("ui-open")
                                    onClicked: Qt.openUrlExternally(page.theme.fileUrl(row.modelData.file))
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    PaperDialog {
        id: paperDialog

        sioul: page.sioul
        theme: page.theme
        window: page.window
        kinds: page.shown.kinds
        onSaved: page.reload()
    }
}
