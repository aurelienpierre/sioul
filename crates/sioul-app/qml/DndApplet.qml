// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Do-not-disturb's switch, in the status line beside the sounds
// (docs/do-not-disturb.md), a button of the line (LineButton.qml): one click
// turns it on until you turn it off, on every device, or off; a right click
// or a long press says until when, and where it holds. Its words say where it
// holds ("on every device", "here only") and why; no colour but the accent
// while it is on. Apart from the pauses' buttons, at the line's end, so that
// the two are never taken for one another. Both ways: turned off in this
// device's own system, unheard, it says so, muted, with "Silence this device
// again"; off while the system still silences this device, it says so, with
// Android's page; the last "off" made in a system is named in its tip.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

LineButton {
    id: applet

    required property var sioul
    // The status line's moment (main.qml), its `dnd` part (everywhere.rs's `moment`).
    required property var moment
    readonly property var dnd: applet.moment && applet.moment.dnd ? applet.moment.dnd : ({ on: false, button: true, line: "", why_line: "", details: [], manual: false, end_time: "", here_off: false, here_offers: [], system_on: false, system_line: "", system_offers: [], last_off: "" })
    readonly property bool on: applet.dnd.on === true
    // On elsewhere, turned off in this device's own system and left so (both ways).
    readonly property bool hereOff: applet.on && applet.dnd.here_off === true
    // The buttons beside what this device's own system holds: Android's page.
    readonly property var offers: applet.hereOff ? (applet.dnd.here_offers || []) : (!applet.on ? (applet.dnd.system_offers || []) : [])

    // The switch hidden in the settings shows only while something holds it on.
    visible: applet.dnd.button !== false || applet.on
    checkable: true
    checked: applet.on
    switchedOn: applet.on
    // A long press opens its menu.
    tipOnHold: false
    icon.name: applet.on ? "notification-disabled" : "notification-inactive"
    display: applet.compact || !applet.on ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
    text: applet.on ? applet.dnd.line : applet.sioul.text("dnd-switch")
    tip: applet.tipText()
    menuOpen: menuLoader.item !== null && (menuLoader.item as SioulMenu).opened

    function tipText(): string {
        if (!applet.on)
            return [applet.sioul.text("dnd-switch-tip-off"), applet.dnd.system_line || "", applet.dnd.last_off || ""].filter(line => line !== "").join("\n")
        return applet.sioul.textArgs("dnd-switch-tip-on", JSON.stringify({ line: applet.dnd.line, why: applet.dnd.why_line }))
    }

    // Until when, and where it holds: at a right click or a long press.
    function openMenu() {
        menuLoader.active = true
        const menu = menuLoader.item as SioulMenu
        menu.popup()
    }

    onChosen: {
        // On (from anything): off, on every device; off: on until turned off.
        applet.sioul.dndToggle(!applet.on, 0)
        applet.checked = Qt.binding(() => applet.on)
    }

    contentItem: RowLayout {
        spacing: 4

        Icon {
            iconName: applet.icon.name
            color: applet.on && !applet.hereOff ? applet.theme.accent : applet.theme.muted
            size: 16
        }
        Label {
            visible: applet.display === AbstractButton.TextBesideIcon && applet.text !== ""
            Layout.maximumWidth: 320
            text: applet.text
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: applet.theme.muted
        }
    }

    TapHandler {
        acceptedButtons: Qt.RightButton
        // A touch has no buttons: on a touch screen, the long press below.
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: applet.openMenu()
    }
    TapHandler {
        id: hold

        acceptedDevices: PointerDevice.TouchScreen
        onLongPressed: {
            // Letting go then changes nothing.
            applet.held = true
            const window = applet.Window.window
            if (window)
                window.menuAt = hold.point.scenePosition
            applet.openMenu()
        }
    }

    // Made the first time it is opened.
    Loader {
        id: menuLoader

        active: false
        sourceComponent: SioulMenu {
            // Where it holds, and why: words only.
            MenuItem {
                visible: applet.on
                height: visible ? implicitHeight : 0
                enabled: false
                text: applet.theme.plain(applet.dnd.why_line)
            }
            Repeater {
                model: applet.on ? applet.dnd.details : []

                delegate: MenuItem {
                    required property string modelData

                    enabled: false
                    text: applet.theme.plain(modelData)
                }
            }
            // Off, while this device's own system still silences it; the last "off" made in a system.
            MenuItem {
                visible: !applet.on && (applet.dnd.system_line || "") !== ""
                height: visible ? implicitHeight : 0
                enabled: false
                text: applet.theme.plain(applet.dnd.system_line || "")
            }
            MenuItem {
                visible: !applet.on && (applet.dnd.last_off || "") !== ""
                height: visible ? implicitHeight : 0
                enabled: false
                text: applet.theme.plain(applet.dnd.last_off || "")
            }
            MenuItem {
                visible: applet.hereOff
                height: visible ? implicitHeight : 0
                text: applet.sioul.text("dnd-again")
                onTriggered: applet.sioul.dndChange("again", "{}")
            }
            Repeater {
                model: applet.offers

                delegate: MenuItem {
                    required property var modelData

                    text: applet.theme.plain(modelData.label)
                    onTriggered: applet.sioul.openDnd(modelData.key)
                }
            }
            MenuItem {
                visible: applet.on
                height: visible ? implicitHeight : 0
                text: applet.sioul.text("dnd-turn-off")
                onTriggered: applet.sioul.dndToggle(false, 0)
            }
            Repeater {
                model: [30, 60, 120]

                delegate: MenuItem {
                    required property int modelData

                    text: applet.sioul.text("dnd-for-" + modelData)
                    onTriggered: applet.sioul.dndToggle(true, modelData)
                }
            }
            MenuItem {
                visible: applet.dnd.end_time !== ""
                height: visible ? implicitHeight : 0
                text: applet.theme.plain(applet.sioul.textWith("dnd-until-time", "time", applet.dnd.end_time))
                onTriggered: applet.sioul.dndToggle(true, -1)
            }
            MenuItem {
                text: applet.sioul.text("dnd-until-off")
                onTriggered: applet.sioul.dndToggle(true, 0)
            }
            MenuItem {
                text: applet.sioul.text("dnd-open-settings")
                onTriggered: {
                    const window = applet.Window.window
                    if (window && window.showParameters)
                        window.showParameters("dnd.button")
                }
            }
        }
    }
}
