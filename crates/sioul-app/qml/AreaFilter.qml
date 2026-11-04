// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A page's filter by area (docs/areas.md, "A page's filter by area"): a
// button saying what shows ("Admin · Leisure") that unfolds three boxes,
// Work, Admin and Leisure, ticked as the hours say until you tick others;
// "As the hours say" sets them back, while your ticks hold; one "?" says how
// it works. Rust keeps the ticks and decides when they end (`areafilter`);
// this lays them out. Mail's All addresses uses it; Budgets will.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: filter

    required property var sioul
    required property var theme
    // What Rust says of it (`areafilter::Shown`): {work, admin, leisure, label, own}.
    property var shown: null
    // The "?"'s words: what this page's filter does, once.
    property string help: ""
    readonly property alias unfolded: list.opened

    // The three areas as they are now ticked, as written ("work+leisure", "" for none).
    signal ticked(string areas)
    // "As the hours say".
    signal asHours

    implicitWidth: button.implicitWidth
    implicitHeight: button.implicitHeight

    // One box changed: the three as they now stand, said at once.
    function tick(which, on) {
        const now = { work: !!filter.shown && filter.shown.work === true, admin: !!filter.shown && filter.shown.admin === true, leisure: !!filter.shown && filter.shown.leisure === true }
        now[which] = on
        filter.ticked(["work", "admin", "leisure"].filter(a => now[a]).join("+"))
    }

    // Folded again (the window's pictures).
    function fold() {
        list.close()
    }

    // Unfolded under its button (the window's pictures and tests too).
    function unfold() {
        const at = button.mapToItem(list.parent, 0, button.height)
        list.x = Math.max(list.leftMargin, Math.min(at.x, list.parent.width - list.width - list.rightMargin))
        list.y = at.y + 2
        list.open()
    }

    Button {
        id: button

        objectName: "areaFilter"
        anchors.fill: parent
        flat: true
        // The words come from Rust, ours: plain text all the same.
        text: filter.theme.plain(filter.shown ? filter.shown.label : "") + "  ▾"
        Accessible.name: filter.sioul.textWith("area-filter-name", "areas", filter.shown ? filter.shown.label : "")
        // A press on it while its list is open closes the list (outside it): that
        // same click does not open it again.
        onClicked: {
            if (list.opened)
                list.close()
            else if (Date.now() - list.closedAt > 300)
                filter.unfold()
        }
    }

    SafePopup {
        id: list

        // How it works, said under the boxes while the "?" is pressed.
        property bool helpShown: false
        // When it last closed (ms), for its button's click.
        property real closedAt: 0

        parent: Overlay.overlay
        modal: false
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
        padding: 10
        // Its boxes' width; never past the window less its margins (a phone's 412 points).
        width: Math.min(Math.max(boxes.implicitWidth + 2 * list.padding, 240), (list.parent ? list.roomWidth : 412) - 2 * list.safeGap)
        onClosed: {
            list.helpShown = false
            list.closedAt = Date.now()
        }

        background: Rectangle {
            color: filter.theme.surface
            radius: filter.theme.radius
            border.color: filter.theme.line
        }

        contentItem: ColumnLayout {
            id: boxes

            spacing: 2

            RowLayout {
                Layout.fillWidth: true
                spacing: 4

                CheckBox {
                    id: workBox

                    objectName: "area:work"
                    Layout.fillWidth: true
                    text: filter.sioul.text("area-filter-work")
                    checked: !!filter.shown && filter.shown.work === true
                    onToggled: {
                        filter.tick("work", workBox.checked)
                        workBox.checked = Qt.binding(() => !!filter.shown && filter.shown.work === true)
                    }
                }
                ToolButton {
                    objectName: "areaFilterHelp"
                    visible: filter.help !== ""
                    text: "?"
                    checkable: true
                    checked: list.helpShown
                    Accessible.name: filter.sioul.text("area-filter-how")
                    ToolTip.visible: hovered && !list.helpShown
                    ToolTip.text: filter.sioul.text("area-filter-how")
                    ToolTip.delay: 300
                    onClicked: list.helpShown = !list.helpShown
                }
            }
            CheckBox {
                id: adminBox

                objectName: "area:admin"
                Layout.fillWidth: true
                text: filter.sioul.text("area-filter-admin")
                checked: !!filter.shown && filter.shown.admin === true
                onToggled: {
                    filter.tick("admin", adminBox.checked)
                    adminBox.checked = Qt.binding(() => !!filter.shown && filter.shown.admin === true)
                }
            }
            CheckBox {
                id: leisureBox

                objectName: "area:leisure"
                Layout.fillWidth: true
                text: filter.sioul.text("area-filter-leisure")
                checked: !!filter.shown && filter.shown.leisure === true
                onToggled: {
                    filter.tick("leisure", leisureBox.checked)
                    leisureBox.checked = Qt.binding(() => !!filter.shown && filter.shown.leisure === true)
                }
            }
            // Your ticks hold: the hours' back, in one tap.
            Button {
                objectName: "areaFilterAsHours"
                visible: !!filter.shown && filter.shown.own === true
                flat: true
                font.pixelSize: 13
                text: filter.sioul.text("area-filter-as-hours")
                onClicked: filter.asHours()
            }
            Label {
                objectName: "areaFilterHelpText"
                visible: list.helpShown
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                Layout.topMargin: 4
                text: filter.help
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: filter.theme.muted
            }
        }
    }
}
