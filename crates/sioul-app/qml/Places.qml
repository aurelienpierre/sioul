// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The places, on the left of the window (main.qml): New, then the places
// where your things live, then, apart at the bottom, closing the work day or
// the day (at any hour), the accounts, the settings and one button that
// refreshes everything; at its foot, level with the status line where that
// line is at the bottom, the button that shows their names or keeps their
// icons only (F9; on a computer, at the title bar's left end). One icon each
// (RailButton.qml), their names beside them when `named`; scrolled when the
// window is too short for them all. Made the first time they show: a phone's
// drawer is not made before ☰ is pressed.

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
    // shows the names or keeps the icons (the keyboard's focus stays on it), the places.
    readonly property alias addButton: newButton
    readonly property alias namesButton: namesToggle
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
                    { page: 3, name: "ui-sites", iconName: "sioul-web" },
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

    // Apart: closing the work day, and the whole day, named where the places'
    // names show; then Sioul itself, the accounts, the settings and everything
    // fetched again, their icons alone, on one row where the column is wide
    // enough (with names, a phone's drawer), else one under the other.
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
        // The work day, and the whole day, closed at any hour (docs/reviews.md):
        // each review asks how it went, all of it optional; the status line
        // offers each only at its time. Each tip says, on a line of its own,
        // what closing does.
        Column {
            width: foot.width
            spacing: 2

            Repeater {
                model: [{ kind: "work", iconName: "task-complete" }, { kind: "night", iconName: "system-suspend" }]

                delegate: RailButton {
                    id: closeButton

                    required property var modelData

                    width: foot.width
                    height: column.row
                    theme: column.theme
                    iconName: closeButton.modelData.iconName
                    name: column.sioul.text("review-close-" + closeButton.modelData.kind)
                    tip: closeButton.name + "\n" + column.sioul.text("review-close-" + closeButton.modelData.kind + "-tip")
                    named: column.named
                    sayTip: column.window.sayRailTip
                    onChosen: {
                        column.window.placesOpen = false
                        column.window.reviewDay(closeButton.modelData.kind)
                    }
                }
            }
        }
        Grid {
            id: footGrid

            // Three icons side by side where they fit; else stacked (the icons' column).
            readonly property bool row: foot.width >= 3 * 44 + 2 * 4

            anchors.horizontalCenter: parent.horizontalCenter
            columns: footGrid.row ? 3 : 1
            spacing: footGrid.row ? 4 : 2

            Repeater {
                model: [{ page: 11, name: "ui-accounts", iconName: "user-identity" }, { page: 12, name: "ui-parameters", iconName: "settings-configure" }]

                delegate: RailButton {
                    id: sioulButton

                    required property var modelData

                    width: footGrid.row ? 44 : foot.width
                    height: column.row
                    theme: column.theme
                    iconName: sioulButton.modelData.iconName
                    name: column.sioul.text(sioulButton.modelData.name)
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

                width: footGrid.row ? 44 : foot.width
                height: column.row
                theme: column.theme
                iconName: "view-refresh"
                name: column.sioul.text("ui-refresh-short")
                keys: "F5"
                tip: column.sioul.busy ? column.sioul.text("ui-refreshing") : column.sioul.text("ui-refresh-all")
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

    // Its foot, level with the status line and under its line, where that line
    // is at the bottom (a tablet): the button that shows the places' names
    // beside their icons, or keeps the icons only (F9). On a computer, that
    // button is at the title bar's left end, above the places (TitleBar.qml).
    Item {
        id: strip

        visible: !column.window.compact && !column.window.ownTitleBar
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
            id: namesToggle

            x: column.inset
            anchors.verticalCenter: parent.verticalCenter
            width: strip.width - 2 * column.inset
            height: 28
            theme: column.theme
            iconName: column.named ? "sidebar-collapse-left" : "sidebar-expand-left"
            name: column.sioul.text(column.named ? "ui-places-names-hide" : "ui-places-names-show")
            keys: "F9"
            named: column.named
            sayTip: column.window.sayRailTip
            onChosen: column.window.togglePlaces(namesToggle.visualFocus)
        }
    }
}
