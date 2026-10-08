// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Health page's settings (its ⚙), set once or rarely, made the first
// time they open: where the pharmacy and renewal errands go, the usual
// meals, naps and night, the pause to move, the limit on chats. The
// medicines and the prescriptions are the page's content, changed there
// (MedicinesSection.qml), as is a day that differs, that day only. On the
// right of the window, all of a phone's; Escape or a click outside closes it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: panel

    required property var sioul
    required property var theme
    // As the backend gives it (`health::settings_view`).
    property var shown: ({ movement: { enabled: true, minutes: 45 }, chats: { enabled: false, minutes: 60, locked_minutes: 30 }, errands_list: "", lists: [] })

    // Something changed that the page shows (the usual meals, naps and night): it reads its days again.
    signal changed

    function reload() {
        panel.shown = JSON.parse(panel.sioul.healthSettings())
        needs.reload()
    }

    // A setting changed.
    function saved() {
        panel.reload()
        panel.changed()
    }

    function setting(key, value) {
        const problem = panel.sioul.setHealth(key, String(value))
        if (problem !== "")
            panel.sioul.status = problem
        panel.saved()
    }

    // Opened where the usual meals, naps and night are set (the Porch's "Set
    // my night", Settings ▸ Hours): scrolled there once open, and kept there a
    // moment while the rows above it are laid out, then left to the hand.
    property bool toNeeds: false

    function showNeeds() {
        panel.toNeeds = true
        if (panel.opened)
            panel.scrollToNeeds()
        else
            panel.open()
    }

    function scrollToNeeds() {
        const flick = scroll.contentItem as Flickable
        if (flick)
            flick.contentY = Math.max(0, Math.min(needs.y - 8, column.implicitHeight - flick.height))
        needsSettled.restart()
    }

    onOpened: if (panel.toNeeds) panel.scrollToNeeds()

    // The panel's end brought into view: the night's last rows, the pause to
    // move and the limit on chats (the window's pictures).
    function showEnd() {
        panel.toNeeds = false
        const flick = scroll.contentItem as Flickable
        if (flick)
            flick.contentY = Math.max(0, column.implicitHeight - flick.height)
    }

    Connections {
        target: needs

        function onYChanged() {
            if (panel.toNeeds && panel.opened)
                panel.scrollToNeeds()
        }
    }

    Timer {
        id: needsSettled

        interval: 400
        onTriggered: panel.toNeeds = false
    }

    parent: Overlay.overlay
    x: parent ? parent.width - width : 0
    y: 0
    // As every page's settings: two fifths of the window and a little more, 520 to 760 pixels; all of a phone's (Theme.qml).
    width: !parent ? 560 : panel.theme.sideWidth(parent.width)
    height: parent ? parent.height : 600
    padding: panel.theme.gap
    modal: true
    dim: false
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
            id: column

            width: scroll.availableWidth
            spacing: 8

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
                Layout.fillWidth: true
                text: panel.sioul.text("health-local")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }

            // Where the pharmacy and the renewals go: a list your phone has.
            RowLayout {
                visible: panel.shown.lists.length > 0
                Layout.fillWidth: true
                spacing: 8

                Label {
                    text: panel.sioul.text("health-errands-list")
                    color: panel.theme.muted
                }
                ComboBox {
                    readonly property var lists: panel.shown.lists

                    Layout.fillWidth: true
                    model: lists.map(l => panel.theme.plain(l.name))
                    currentIndex: Math.max(0, lists.findIndex(l => l.id === panel.shown.errands_list))
                    onActivated: index => panel.setting("errands_list", lists[index].id)
                }
            }

            // The usual meals, naps and night.
            NeedsSection {
                id: needs

                Layout.fillWidth: true
                Layout.topMargin: 6
                sioul: panel.sioul
                theme: panel.theme
                onChanged: panel.changed()
            }

            // A pause to move, while focusing.
            Label {
                Layout.topMargin: 12
                text: panel.sioul.text("health-moving")
                font.pixelSize: 16
                font.weight: Font.DemiBold
                color: panel.theme.accent
            }
            // The minutes under the sentence when the screen is narrow (a phone).
            Flow {
                id: movingRow

                Layout.fillWidth: true
                spacing: 8

                // A sentence longer than the row wraps in it, rather than run past the screen.
                WrapCheckBox {
                    width: Math.min(implicitWidth, movingRow.width)
                    text: panel.sioul.text("health-moving-every")
                    checked: panel.shown.movement.enabled
                    onToggled: panel.setting("movement.enabled", checked)
                }
                SpinBox {
                    id: movingMinutes

                    from: 10
                    to: 240
                    stepSize: 5
                    editable: true
                    value: panel.shown.movement.minutes
                    enabled: panel.shown.movement.enabled
                    Accessible.name: panel.sioul.text("health-moving-every")
                    onValueModified: panel.setting("movement.minutes", value)
                }
                Label {
                    height: movingMinutes.height
                    verticalAlignment: Text.AlignVCenter
                    text: panel.sioul.text("health-minutes")
                    color: panel.theme.muted
                }
            }
            Label {
                Layout.fillWidth: true
                text: panel.sioul.text("health-moving-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }

            // Chats, a limit a day.
            Label {
                Layout.topMargin: 12
                text: panel.sioul.text("health-chats")
                font.pixelSize: 16
                font.weight: Font.DemiBold
                color: panel.theme.accent
            }
            WrapCheckBox {
                Layout.fillWidth: true
                text: panel.sioul.text("health-chats-limit")
                checked: panel.shown.chats.enabled
                onToggled: panel.setting("chats.enabled", checked)
            }
            GridLayout {
                enabled: panel.shown.chats.enabled
                columns: 3
                columnSpacing: 8
                rowSpacing: 4

                Label {
                    text: panel.sioul.text("health-chats-after")
                    color: panel.theme.muted
                }
                SpinBox {
                    from: 5
                    to: 600
                    stepSize: 5
                    editable: true
                    value: panel.shown.chats.minutes || 60
                    onValueModified: panel.setting("chats.minutes", value)
                }
                Label {
                    text: panel.sioul.text("health-minutes-a-day")
                    color: panel.theme.muted
                }
                Label {
                    text: panel.sioul.text("health-chats-for")
                    color: panel.theme.muted
                }
                SpinBox {
                    from: 5
                    to: 600
                    stepSize: 5
                    editable: true
                    value: panel.shown.chats.locked_minutes || 30
                    onValueModified: panel.setting("chats.locked_minutes", value)
                }
                Label {
                    text: panel.sioul.text("health-minutes")
                    color: panel.theme.muted
                }
            }
            Label {
                Layout.fillWidth: true
                text: panel.sioul.text("health-chats-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            Item {
                Layout.preferredHeight: panel.theme.gap
            }
        }
    }
}
