// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The settings of one page, on the right of it: each with what it changes,
// in a sentence. A change is saved at once, into the configuration file, its
// comments kept. Escape or a click outside closes it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: panel

    required property var sioul
    required property var theme
    // "porch", "mail", "agenda", "tasks", "notes", "reading", "contacts", "time", "general".
    property string view: ""
    property var rows: []
    property string problem: ""

    function reload() {
        panel.rows = panel.view === "" ? [] : JSON.parse(panel.sioul.settings(panel.view))
    }

    // A setting at the top of the panel, its group's title above it (the window's images).
    function scrollTo(key) {
        const at = panel.rows.findIndex(row => row.key === key)
        const row = at < 0 ? null : shownRows.itemAt(at)
        const flick = scroll.contentItem as Flickable
        if (row === null || flick === null)
            return
        const top = row.mapToItem(flick.contentItem, 0, 0).y
        flick.contentY = Math.max(0, Math.min(top - 8, flick.contentHeight - flick.height))
    }

    function save(key, value) {
        panel.problem = panel.sioul.setSetting(key, JSON.stringify(value))
        if (panel.problem === "")
            panel.reload()
    }

    parent: Overlay.overlay
    x: parent ? parent.width - width : 0
    y: 0
    // Half the window, at most 480 pixels; all of a phone's.
    width: !parent ? 480 : parent.width < 720 ? parent.width : Math.min(480, parent.width * 0.5)
    height: parent ? parent.height : 600
    padding: panel.theme.gap
    modal: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    onAboutToShow: panel.reload()

    background: Rectangle {
        color: panel.theme.surface
        border.color: panel.theme.line

        Rectangle {
            width: 1
            height: parent.height
            color: panel.theme.line
        }
    }

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 6

            RowLayout {
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: panel.sioul.text("ui-settings")
                    font.pixelSize: 18
                    color: panel.theme.text
                }
                ToolButton {
                    text: "×"
                    Accessible.name: panel.sioul.text("ui-close")
                    onClicked: panel.close()
                }
            }
            Label {
                visible: panel.problem !== ""
                Layout.fillWidth: true
                text: panel.problem
                wrapMode: Text.Wrap
                color: panel.theme.warm
            }

            Repeater {
                id: shownRows

                model: panel.rows

                delegate: ColumnLayout {
                    id: row

                    required property var modelData
                    required property int index
                    readonly property bool newGroup: row.modelData.group !== "" && (row.index === 0 || panel.rows[row.index - 1].group !== row.modelData.group)

                    Layout.fillWidth: true
                    Layout.topMargin: row.newGroup ? 14 : 8
                    spacing: 3

                    Label {
                        visible: row.newGroup
                        Layout.fillWidth: true
                        text: row.modelData.group
                        font.pixelSize: 16
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                        color: panel.theme.accent
                    }
                    SettingRow {
                        Layout.fillWidth: true
                        setting: row.modelData
                        sioul: panel.sioul
                        theme: panel.theme
                        onSave: (key, value) => panel.save(key, value)
                    }
                }
            }
        }
    }
}
