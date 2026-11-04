// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The household at the head of the Contacts page (« Proches »;
// docs/persons.md): one row of chips, you first, then the others in their
// order, children's other parents after a thin space, "Add a person" at the
// end. A chip opens the person's page; right click, or a long press on a
// touch screen, opens its menu. The row wraps rather than running past the
// screen's edge. One "?" says what a person is, once.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: household

    required property var sioul
    required property var theme
    // persons::household: {persons: [{id, name, relation, line}], others: [...], relations: [{id, label}]}.
    property var shown: ({ persons: [], others: [], relations: [] })
    // The person whose page is open, to mark their chip.
    property string chosen: ""
    property bool helpShown: false

    signal openPerson(string id)
    signal addPerson
    signal renamePerson(string id)

    // Sioul's window, with its own properties (not only a window's): where a long press opens a menu.
    function sioulWindow(): var {
        return household.Window.window
    }

    function reload() {
        household.shown = JSON.parse(household.sioul.household() || "null") || ({ persons: [], others: [], relations: [] })
    }

    spacing: 4
    Component.onCompleted: household.reload()

    RowLayout {
        Layout.fillWidth: true
        spacing: 6

        Label {
            Layout.fillWidth: true
            text: household.sioul.text("persons-household")
            font.pixelSize: 13
            font.weight: Font.DemiBold
            color: household.theme.muted
        }
        ToolButton {
            objectName: "householdHelp"
            text: "?"
            checkable: true
            checked: household.helpShown
            Accessible.name: household.sioul.text("persons-household")
            ToolTip.visible: hovered && !household.helpShown
            ToolTip.text: household.theme.plain(household.sioul.text("persons-household-help"))
            ToolTip.delay: 300
            onClicked: household.helpShown = !household.helpShown
        }
    }
    Label {
        visible: household.helpShown
        Layout.fillWidth: true
        text: household.sioul.text("persons-household-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: household.theme.muted
    }
    Flow {
        objectName: "householdChips"
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: household.shown.persons.concat(household.shown.others.length > 0 ? [{ id: "", gap: true }] : []).concat(household.shown.others)

            delegate: Item {
                id: slot

                required property var modelData

                implicitWidth: slot.modelData.gap ? 6 : chip.implicitWidth
                implicitHeight: chip.implicitHeight

                Button {
                    id: chip

                    visible: !slot.modelData.gap
                    objectName: "householdChip:" + (slot.modelData.name || "")
                    // As wide as its name: the style's buttons are 100 pixels at least.
                    implicitWidth: Math.min(implicitContentWidth + leftPadding + rightPadding, 220)
                    // The one open: a firmer border and its name in bold, never the style's filled button.
                    readonly property bool chosen: household.chosen === slot.modelData.id

                    flat: true
                    text: household.theme.plain(slot.modelData.name || "")
                    font.pixelSize: 13
                    font.weight: chip.chosen ? Font.DemiBold : Font.Normal
                    ToolTip.visible: hovered && (slot.modelData.line || "") !== ""
                    ToolTip.text: household.theme.plain(slot.modelData.line || "")
                    ToolTip.delay: 500
                    background: Rectangle {
                        radius: height / 2
                        color: chip.chosen || chip.hovered ? household.theme.surface : "transparent"
                        border.color: chip.visualFocus ? household.theme.focus : chip.chosen ? household.theme.accent : household.theme.line
                    }
                    onClicked: household.openPerson(slot.modelData.id)
                    // On a touch screen, the menu comes at a long press.
                    onPressAndHold: {
                        const window = household.sioulWindow()
                        if (window && window.menuAt !== undefined)
                            window.menuAt = chip.mapToItem(null, chip.pressX, chip.pressY)
                        menu.target = slot.modelData
                        menu.popup()
                    }
                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onTapped: {
                            menu.target = slot.modelData
                            menu.popup()
                        }
                    }
                }
            }
        }
        Button {
            objectName: "householdAdd"
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            flat: true
            text: household.sioul.text("persons-add")
            icon.name: "list-add"
            icon.color: household.theme.text
            font.pixelSize: 13
            onClicked: household.addPerson()
        }
    }

    SioulMenu {
        id: menu

        property var target: null

        MenuItem {
            text: household.sioul.text("person-open")
            onTriggered: household.openPerson(menu.target.id)
        }
        MenuItem {
            // You have nothing to rename: your name is your accounts'.
            enabled: menu.target !== null && menu.target.id !== "me"
            text: household.sioul.text("person-rename")
            onTriggered: household.renamePerson(menu.target.id)
        }
    }
}
