// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The settings of one page, on the right of it: each with what it changes,
// in a sentence. A change is saved at once, into the configuration file, its
// comments kept. Escape or a click outside closes it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafePopup {
    id: panel

    // A sheet: edge to edge within the safe area, as it was within the window.
    safeGap: 0

    required property var sioul
    required property var theme
    // "porch", "mail", "agenda", "tasks", "notes", "contacts", "time", "general".
    property string view: ""
    property var rows: []
    property string problem: ""
    // The rows, gathered by heading, in their order: one frame each (Section.qml).
    readonly property var groups: {
        const groups = []
        for (const row of panel.rows) {
            const last = groups.length > 0 ? groups[groups.length - 1] : null
            if (last !== null && last.group === row.group)
                last.rows.push(row)
            else
                groups.push({ group: row.group, rows: [row] })
        }
        return groups
    }

    function reload() {
        panel.rows = panel.view === "" ? [] : JSON.parse(panel.sioul.settings(panel.view))
    }

    // A setting at the top of the panel, its group's title above it (a door by
    // its key, the window's images). The first time the panel opens its rows are
    // not laid out yet: the setting asked for is kept in view while they are,
    // then left to the hand (the first move of the hand ends it at once).
    property string wanted: ""

    function scrollTo(key) {
        panel.wanted = key
        panel.place()
        settled.restart()
    }

    // The setting's own row, or, when it is the first of its heading, its frame with the heading.
    function placeOf(key) {
        const at = panel.groups.findIndex(group => group.rows.some(row => row.key === key))
        const section = at < 0 ? null : shownGroups.itemAt(at)
        if (section === null)
            return null
        if (panel.groups[at].rows[0].key === key)
            return section
        const find = item => {
            if (item.objectName === "setting:" + key)
                return item
            for (let i = 0; i < item.children.length; i++) {
                const found = find(item.children[i])
                if (found)
                    return found
            }
            return null
        }
        return find(section)
    }

    function place() {
        const row = panel.placeOf(panel.wanted)
        const flick = scroll.contentItem as Flickable
        if (row === null || flick === null)
            return
        const top = row.mapToItem(flick.contentItem, 0, 0).y
        flick.contentY = Math.max(0, Math.min(top - 8, flick.contentHeight - flick.height))
    }

    Timer {
        id: settled

        interval: 400
        onTriggered: panel.wanted = ""
    }

    // Placed again at each frame of the layout's work, a row's place moving after
    // the panel's height said it changed; the setting kept a moment once still.
    Timer {
        interval: 40
        repeat: true
        running: panel.wanted !== "" && panel.opened
        onTriggered: panel.place()
    }

    Connections {
        target: scroll.contentItem as Flickable

        function onContentHeightChanged() {
            if (panel.wanted !== "")
                settled.restart()
        }
        function onMovementStarted() {
            panel.wanted = ""
        }
    }

    // An item of the panel (a part's own line) at the top of the panel (the window's images).
    function scrollToItem(item) {
        const flick = scroll.contentItem as Flickable
        if (item === null || flick === null)
            return
        const top = item.mapToItem(flick.contentItem, 0, 0).y
        flick.contentY = Math.max(0, Math.min(top - 8, flick.contentHeight - flick.height))
    }

    // `shown`: the value the row showed: a list's change is set over the files as they are now.
    function save(key, value, shown) {
        panel.problem = shown === undefined ? panel.sioul.setSetting(key, JSON.stringify(value)) : panel.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
        if (panel.problem === "")
            panel.reload()
    }

    parent: Overlay.overlay
    x: parent ? parent.width - width : 0
    y: 0
    // Two fifths of the window and a little more, 520 to 760 pixels; all of a phone's (Theme.qml).
    // A panel that holds a table at a glance (Agenda ⚙'s agendas, 26 November 2026) spans the
    // view, a margin left on the page under it; its other settings keep a readable width.
    readonly property bool wide: panel.rows.some(row => row.kind === "agendas")
    width: !parent ? 520 : panel.wide && panel.roomWidth >= 720 ? panel.roomWidth - (panel.roomWidth >= 1100 ? 48 : 0) : panel.theme.sideWidth(panel.roomWidth)
    height: parent ? panel.roomHeight : 600
    padding: panel.theme.gap
    modal: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    onAboutToShow: panel.reload()
    onOpened: if (panel.wanted !== "") panel.place()

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
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.theme.warm
            }

            // Each heading's rows in a frame of their own (Section.qml), as Settings shows them.
            Repeater {
                id: shownGroups

                model: panel.groups

                delegate: Section {
                    id: group

                    required property var modelData
                    required property int index

                    // A group with no heading whose rows draw nothing (Mail ⚙'s row that leaves the
                    // lists of the mail you called spam, while there are none): no empty frame.
                    visible: group.title !== "" || rowsShown.count !== 1 || rowsShown.itemAt(0) === null || !(rowsShown.itemAt(0) as SettingRow).blank
                    Layout.fillWidth: true
                    Layout.topMargin: group.index === 0 ? 6 : 12
                    theme: panel.theme
                    title: panel.theme.plain(group.modelData.group)
                    body.spacing: 12

                    Repeater {
                        id: rowsShown

                        model: group.modelData.rows

                        delegate: SettingRow {
                            required property var modelData

                            objectName: "setting:" + modelData.key
                            Layout.fillWidth: true
                            // Beside a table at a glance, a setting of one value keeps a readable width.
                            Layout.maximumWidth: panel.wide && ["agendas", "conflicts"].indexOf(modelData.kind) < 0 ? panel.theme.readable : -1
                            setting: modelData
                            sioul: panel.sioul
                            theme: panel.theme
                            onSave: (key, value, shown) => panel.save(key, value, shown)
                            onGoTo: key => panel.scrollTo(key)
                            onChangedElsewhere: panel.reload()
                        }
                    }
                }
            }
        }
    }
}
