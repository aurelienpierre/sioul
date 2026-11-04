// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The agenda filter (several agendas): one button that opens the list of the
// agendas, grouped by whose they are (yours, each child, another parent,
// partners, shared), each group foldable, with a box for the group and one
// for each agenda. Unless you say, it shows what the hours admit (a work
// agenda in work hours only); your word holds until work's hours change, and
// "As the hours say" gives them back. One filter for every view: the
// agenda's Day, Week, Month and What comes, and the Tasks page's lists, which
// show the same list of their own agendas. On a computer under its button;
// on a phone a sheet from the bottom, as wide as the screen. Named sets of
// agendas come first (part 4): chosen by hand, or brought by Sioul's own
// times, never by a place; "Keep these as a set" names what shows. Beside
// each child's agenda, "Who does what" opens its week (`WhoDoesWhat`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Button {
    id: filter

    required property var sioul
    required property var theme
    // The filter as its list shows it (`agenda::FilterShown`): {chosen, until,
    // groups: [{key, title, agendas: [{id, name, colour, on, usual}]}]}.
    property var shown: ({ chosen: false, groups: [] })
    // A phone: the list a sheet from the bottom, the button its icon alone.
    property bool narrow: false
    // The button its icon alone, its name in a tip (a narrow window, a crowded line).
    property bool iconOnly: filter.narrow
    // The groups folded on this page, by key, while it lives.
    property var folded: ({})
    // The window, for the who-does-what rows' cards; none on the Tasks page.
    property var window: null
    // "Keep these as a set" opened.
    property bool naming: false

    readonly property var groups: filter.shown && filter.shown.groups ? filter.shown.groups : []
    // Some agenda the hours, or your word, leave out now: the button says so by its look alone.
    readonly property bool narrowed: filter.groups.some(g => g.agendas.some(a => !a.on))

    objectName: "agendaFilter"
    visible: filter.groups.length > 0
    text: filter.sioul.text("agendas-filter")
    icon.name: "view-calendar"
    icon.color: filter.theme.text
    display: filter.iconOnly ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
    font.weight: filter.narrowed ? Font.DemiBold : Font.Normal
    ToolTip.visible: filter.iconOnly && filter.hovered
    ToolTip.text: filter.sioul.text("agendas-filter")
    ToolTip.delay: 300
    Accessible.name: filter.text
    onClicked: filter.open()

    // The list, made the first time it opens; given back (for the window's pictures).
    function open() {
        const list = sheet.now()
        if (!list.opened)
            list.open()
        return list
    }

    function close() {
        sheet.close()
    }

    // Agendas turned on or off by hand: your word, kept until work's hours change.
    function turn(ids, on) {
        const problem = filter.sioul.turnAgendas(JSON.stringify(ids), on)
        if (problem !== "")
            filter.sioul.status = problem
    }

    // A set chosen, kept or forgotten (part 4).
    function useSet(name) {
        const problem = filter.sioul.useAgendaSet(name)
        if (problem !== "")
            filter.sioul.status = problem
    }
    function keepSet(name, times) {
        const ids = []
        for (const group of filter.groups) {
            for (const agenda of group.agendas) {
                if (agenda.on)
                    ids.push(agenda.id)
            }
        }
        const problem = filter.sioul.keepAgendaSet(name, JSON.stringify(ids), JSON.stringify(times))
        if (problem !== "")
            filter.sioul.status = problem
        filter.naming = false
    }
    function forgetSet(name) {
        const problem = filter.sioul.keepAgendaSet(name, "null", "[]")
        if (problem !== "")
            filter.sioul.status = problem
    }
    // A child's week, who does what.
    function whoDoesWhat(child) {
        const panel = week.now()
        panel.openOn(child)
        return panel
    }

    Later {
        id: week

        sourceComponent: Component {
            WhoDoesWhat {
                sioul: filter.sioul
                theme: filter.theme
                window: filter.window
                narrow: filter.narrow
            }
        }
    }

    function asTheHoursSay() {
        const problem = filter.sioul.agendasAsHours()
        if (problem !== "")
            filter.sioul.status = problem
    }

    // A group's box: all its agendas shown, none, or some.
    function groupState(group) {
        const on = group.agendas.filter(a => a.on).length
        return on === 0 ? Qt.Unchecked : on === group.agendas.length ? Qt.Checked : Qt.PartiallyChecked
    }

    function fold(key) {
        const next = Object.assign({}, filter.folded)
        next[key] = !next[key]
        filter.folded = next
    }

    Later {
        id: sheet

        sourceComponent: Component {
            SafePopup {
                id: list

                // Where the button is in the window, when the list opens on a computer.
                property point at: Qt.point(0, 0)

                parent: Overlay.overlay
                modal: filter.narrow
                focus: true
                padding: filter.theme.gap
                closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
                width: filter.narrow ? list.roomWidth - 2 * list.safeGap : Math.min(360, list.roomWidth - 2 * list.safeGap)
                height: Math.min(implicitHeight, list.roomHeight * (filter.narrow ? 0.7 : 0.8))
                x: filter.narrow ? (list.safeArea ? list.safeArea.left : 0) + list.safeGap : Math.max(list.safeGap, Math.min(list.at.x, (parent ? parent.width : 800) - list.width - list.safeGap))
                y: filter.narrow ? (parent ? parent.height : 800) - list.height - list.bottomMargin : list.at.y
                onAboutToShow: list.at = filter.mapToItem(null, 0, filter.height + 4)

                background: Rectangle {
                    color: filter.theme.surface
                    border.color: filter.theme.line
                    radius: filter.theme.radius
                }

                contentItem: ScrollView {
                    id: scroll

                    contentWidth: availableWidth
                    clip: true

                    ColumnLayout {
                        width: scroll.availableWidth
                        spacing: 4

                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 6

                            Label {
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: filter.sioul.text("agendas-filter")
                                textFormat: Text.PlainText
                                font.pixelSize: 16
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                                color: filter.theme.text
                            }
                            Button {
                                objectName: "agendasAsHours"
                                visible: filter.shown.chosen === true
                                flat: true
                                text: filter.sioul.text("agendas-filter-hours")
                                ToolTip.visible: hovered && (filter.shown.until || "") !== ""
                                ToolTip.text: filter.theme.plain(filter.sioul.textWith("agendas-filter-until", "time", filter.shown.until || ""))
                                ToolTip.delay: 300
                                onClicked: filter.asTheHoursSay()
                            }
                            ToolButton {
                                text: "×"
                                Accessible.name: filter.sioul.text("ui-close")
                                onClicked: list.close()
                            }
                        }

                        // Named sets (part 4): each a button, the one showing marked, × forgets it.
                        Flow {
                            visible: (filter.shown.sets || []).length > 0
                            Layout.fillWidth: true
                            spacing: 6

                            Repeater {
                                model: filter.shown.sets || []

                                delegate: Row {
                                    id: set

                                    required property var modelData

                                    spacing: 0

                                    Button {
                                        objectName: "agendaSet"
                                        text: filter.theme.plain(set.modelData.name) + (set.modelData.by_time ? " · " + filter.sioul.text("agendas-set-now") : "")
                                        highlighted: set.modelData.showing === true
                                        onClicked: filter.useSet(set.modelData.name)
                                    }
                                    ToolButton {
                                        text: "×"
                                        Accessible.name: filter.sioul.textWith("agendas-set-forget", "name", set.modelData.name)
                                        onClicked: filter.forgetSet(set.modelData.name)
                                    }
                                }
                            }
                        }
                        Button {
                            objectName: "agendasKeepSet"
                            visible: !filter.naming
                            flat: true
                            text: filter.sioul.text("agendas-set-save")
                            onClicked: filter.naming = true
                        }
                        ColumnLayout {
                            visible: filter.naming
                            Layout.fillWidth: true
                            spacing: 4

                            TextField {
                                id: setName

                                objectName: "agendasSetName"
                                Layout.fillWidth: true
                                placeholderText: filter.sioul.text("agendas-set-name")
                                Accessible.name: placeholderText
                                onAccepted: if (text.trim() !== "")
                                    filter.keepSet(text.trim(), setTimes.chosen())
                            }
                            Label {
                                text: filter.sioul.text("agendas-set-times")
                                color: filter.theme.muted
                            }
                            Flow {
                                id: setTimes

                                function chosen() {
                                    return [[setWork, "work"], [setAdmin, "admin"], [setLeisure, "leisure"]].filter(p => p[0].checked).map(p => p[1])
                                }

                                Layout.fillWidth: true
                                spacing: 8

                                CheckBox {
                                    id: setWork

                                    text: filter.sioul.text("area-work")
                                }
                                CheckBox {
                                    id: setAdmin

                                    text: filter.sioul.text("area-admin")
                                }
                                CheckBox {
                                    id: setLeisure

                                    text: filter.sioul.text("area-leisure")
                                }
                            }
                            Button {
                                enabled: setName.text.trim() !== ""
                                text: filter.sioul.text("agendas-set-keep")
                                onClicked: filter.keepSet(setName.text.trim(), setTimes.chosen())
                            }
                        }

                        Repeater {
                            model: filter.groups

                            delegate: ColumnLayout {
                                id: group

                                required property var modelData
                                readonly property bool shut: filter.folded[group.modelData.key] === true

                                Layout.fillWidth: true
                                spacing: 0

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    ToolButton {
                                        text: group.shut ? "▸" : "▾"
                                        Accessible.name: group.shut ? filter.sioul.textWith("agendas-unfold", "group", group.modelData.title) : filter.sioul.textWith("agendas-fold", "group", group.modelData.title)
                                        onClicked: filter.fold(group.modelData.key)
                                    }
                                    CheckBox {
                                        objectName: "agendaGroup"
                                        Layout.fillWidth: true
                                        Layout.preferredWidth: 0
                                        tristate: true
                                        text: filter.theme.plain(group.modelData.title)
                                        font.weight: Font.DemiBold
                                        checkState: filter.groupState(group.modelData)
                                        // All shown: all off; otherwise all on (the box never stops half-ticked by a click).
                                        nextCheckState: () => filter.groupState(group.modelData) === Qt.Checked ? Qt.Unchecked : Qt.Checked
                                        onClicked: {
                                            filter.turn(group.modelData.agendas.map(a => a.id), checkState === Qt.Checked)
                                            checkState = Qt.binding(() => filter.groupState(group.modelData))
                                        }
                                    }
                                }
                                Repeater {
                                    model: group.shut ? [] : group.modelData.agendas

                                    delegate: RowLayout {
                                        id: line

                                        required property var modelData

                                        Layout.fillWidth: true
                                        Layout.leftMargin: 34
                                        spacing: 6

                                        Rectangle {
                                            Layout.preferredWidth: 10
                                            Layout.preferredHeight: 10
                                            radius: 5
                                            color: line.modelData.colour || filter.theme.accent
                                        }
                                        CheckBox {
                                            objectName: "agendaLine"
                                            Layout.fillWidth: true
                                            Layout.preferredWidth: 0
                                            // Kept hidden on this device: unticked whatever the hours; ticked, shown until they change.
                                            text: filter.theme.plain(line.modelData.name) + (line.modelData.kept === true ? " · " + filter.sioul.text("agendas-kept-hidden") : "")
                                            checked: line.modelData.on === true
                                            onToggled: {
                                                filter.turn([line.modelData.id], checked)
                                                checked = Qt.binding(() => line.modelData.on === true)
                                            }
                                        }
                                        // A child's agenda: its week, who does what (part 4).
                                        ToolButton {
                                            objectName: "agendaWhoDoesWhat"
                                            visible: String(group.modelData.key).startsWith("child:")
                                            icon.name: "view-list-details"
                                            icon.color: filter.theme.text
                                            display: AbstractButton.IconOnly
                                            Accessible.name: filter.sioul.text("holder-who-does-what")
                                            ToolTip.visible: hovered
                                            ToolTip.text: filter.sioul.text("holder-who-does-what")
                                            ToolTip.delay: 300
                                            onClicked: {
                                                filter.close()
                                                filter.whoDoesWhat(line.modelData.id)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
