// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The places, on the left of the window (main.qml): New, then the places
// where your things live, then, apart at the bottom, the accounts, the
// settings and one button that refreshes everything; at its foot, level with
// the status line, the button that hides them (F9). One icon each
// (RailButton.qml), their names beside them when `named`; scrolled when the
// window is too short for them all. Made the first time they show: a phone's
// drawer is not made before ☰ is pressed, nor a column hidden as Sioul starts.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

Item {
    id: column

    required property var window
    required property var theme
    required property var sioul
    // Names beside the icons: in a phone's drawer, else as Settings ▸ Display says.
    required property bool named
    // A row: a finger's on a phone or a touch screen, a pointer's elsewhere.
    readonly property int row: column.window.compact || Qt.platform.os === "android" ? 44 : 36
    // The buttons' margin on each side.
    readonly property int inset: column.named ? 8 : 6
    // What the window reaches: New (its menu comes under it), the button that
    // hides the places (the keyboard's focus comes back to it), the places.
    readonly property alias addButton: newButton
    readonly property alias hideButton: hidePlaces
    readonly property alias list: placesList
    readonly property alias repeater: placesRepeater

    // Something new, of any kind: always here, whatever the page.
    RailButton {
        id: newButton

        x: column.inset
        y: 8
        width: column.width - 2 * column.inset
        height: column.row
        theme: column.theme
        iconName: "list-add"
        name: column.sioul.text("ui-new")
        keys: "Ctrl+N"
        after: "  ▾"
        named: column.named
        filled: true
        sayTip: column.window.sayRailTip
        onChosen: column.window.showNewMenu()
    }

    // Where your things live; scrolled when the window is too short for them all.
    Flickable {
        id: placesList

        // A button reached by Tab, brought into view.
        function reveal(item) {
            if (item.y < placesList.contentY)
                placesList.contentY = item.y
            else if (item.y + item.height > placesList.contentY + placesList.height)
                placesList.contentY = item.y + item.height - placesList.height
        }

        y: newButton.y + newButton.height + 10
        width: column.width
        height: Math.max(0, Math.min(placesList.contentHeight, foot.y - 10 - placesList.y))
        contentHeight: placesColumn.height
        flickableDirection: Flickable.VerticalFlick
        boundsBehavior: Flickable.StopAtBounds
        interactive: placesList.contentHeight > placesList.height + 1
        clip: placesList.interactive
        // More of them than the window shows: a thin bar says so.
        ScrollBar.vertical: ScrollBar {
            interactive: false
            policy: placesList.interactive ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
        }

        Column {
            id: placesColumn

            x: column.inset
            width: column.width - 2 * column.inset
            spacing: 2

            Repeater {
                id: placesRepeater

                // In the order of their keys, Ctrl+1 to Ctrl+0; the papers'
                // page came after the others, its place is by the budgets.
                model: [
                    { page: 0, name: "ui-porch", iconName: "mail-folder-inbox" },
                    { page: 1, name: "ui-tasks", iconName: "view-calendar-tasks" },
                    { page: 2, name: "ui-mail", iconName: "mail-message" },
                    { page: 3, name: "ui-sites", iconName: "globe" },
                    { page: 4, name: "ui-agenda", iconName: "view-calendar" },
                    { page: 5, name: "ui-contacts", iconName: "user-properties" },
                    { page: 6, name: "ui-notes", iconName: "view-pim-notes" },
                    { page: 7, name: "ui-projects", iconName: "folder" },
                    { page: 8, name: "ui-time", iconName: "clock" },
                    { page: 9, name: "ui-budgets", iconName: "wallet-open" },
                    { page: 13, name: "ui-papers", iconName: "view-certificate" },
                    { page: 10, name: "ui-health", iconName: "love" }
                ]

                delegate: RailButton {
                    id: placeButton

                    required property var modelData

                    width: placesColumn.width
                    height: column.row
                    theme: column.theme
                    iconName: placeButton.modelData.iconName
                    name: column.sioul.text(placeButton.modelData.name)
                    // The first ten pages' keys.
                    keys: placeButton.modelData.page <= 9 ? "Ctrl+" + (placeButton.modelData.page + 1) % 10 : ""
                    named: column.named
                    place: true
                    current: column.window.page === placeButton.modelData.page
                    sayTip: column.window.sayRailTip
                    onChosen: {
                        column.window.page = placeButton.modelData.page
                        column.window.placesOpen = false
                    }
                    onActiveFocusChanged: {
                        if (placeButton.activeFocus)
                            placesList.reveal(placeButton)
                    }
                }
            }
        }
    }

    // Sioul itself, apart: the accounts, the settings, and everything fetched
    // again; in a phone's drawer, one row of icons.
    Column {
        id: foot

        x: column.inset
        width: column.width - 2 * column.inset
        anchors.bottom: parent.bottom
        anchors.bottomMargin: (strip.visible ? strip.height : 0) + 8
        spacing: 8

        Rectangle {
            width: foot.width
            height: 1
            color: column.theme.line
        }
        Grid {
            id: footGrid

            // Their names beside them when asked for; a phone's drawer keeps its row of icons.
            readonly property bool named: column.named && !column.window.compact

            anchors.horizontalCenter: parent.horizontalCenter
            columns: column.window.compact ? 3 : 1
            spacing: column.window.compact ? 4 : 2

            Repeater {
                model: [{ page: 11, name: "ui-accounts", iconName: "user-identity" }, { page: 12, name: "ui-parameters", iconName: "settings-configure" }]

                delegate: RailButton {
                    id: sioulButton

                    required property var modelData

                    width: column.window.compact ? 44 : foot.width
                    height: column.row
                    theme: column.theme
                    iconName: sioulButton.modelData.iconName
                    name: column.sioul.text(sioulButton.modelData.name)
                    named: footGrid.named
                    place: true
                    current: column.window.page === sioulButton.modelData.page
                    sayTip: column.window.sayRailTip
                    onChosen: {
                        column.window.page = sioulButton.modelData.page
                        column.window.placesOpen = false
                    }
                }
            }
            // Mail, agenda, tasks, contacts and the rest, fetched again at once.
            RailButton {
                id: refreshButton

                width: column.window.compact ? 44 : foot.width
                height: column.row
                theme: column.theme
                iconName: "view-refresh"
                name: column.sioul.text("ui-refresh-short")
                keys: "F5"
                tip: column.sioul.busy ? column.sioul.text("ui-refreshing") : column.sioul.text("ui-refresh-all")
                named: footGrid.named
                enabled: !column.sioul.busy
                sayTip: column.window.sayRailTip
                onChosen: column.sioul.syncNow()
            }
        }
    }
    // Turning slowly while it fetches.
    RotationAnimator {
        target: refreshButton.glyph
        running: column.sioul.busy
        from: 0
        to: 360
        duration: 1600
        loops: Animation.Infinite
        onRunningChanged: if (!running) refreshButton.glyph.rotation = 0
    }

    // Its foot, level with the status line and under its line: the button that
    // hides the places, where the one that shows them again comes (main.qml).
    Item {
        id: strip

        visible: !column.window.compact
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 36

        Rectangle {
            width: parent.width
            height: 1
            color: column.theme.line
        }
        RailButton {
            id: hidePlaces

            x: column.inset
            anchors.verticalCenter: parent.verticalCenter
            width: strip.width - 2 * column.inset
            height: 28
            theme: column.theme
            iconName: "sidebar-collapse-left"
            name: column.sioul.text("ui-places-hide")
            keys: "F9"
            named: column.named
            sayTip: column.window.sayRailTip
            onChosen: column.window.togglePlaces(hidePlaces.visualFocus)
        }
    }
}
