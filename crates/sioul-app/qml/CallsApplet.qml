// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "Let every call through" (docs/android.md, "Calls"): a switch for the
// status line, a button of the line (LineButton.qml), shown on every device
// while a phone of yours screens calls,
// for when you expect a call. One click lets every call ring on that phone
// for an hour; a click while it is on screens calls again; a right click or
// a long press chooses until when. Its words say until when; no colour but
// the accent while it is on. Self-contained: the status line places it
// (main.qml), beside do-not-disturb's switch.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

LineButton {
    id: applet

    required property var sioul
    // The status line's moment (backend.rs's `mode_json`), its `calls` part (calls.rs's `moment`).
    required property var moment
    readonly property var calls: applet.moment && applet.moment.calls ? applet.moment.calls : ({ shown: false, through: false, line: "" })
    readonly property bool on: applet.calls.through === true

    visible: applet.calls.shown === true
    checkable: true
    checked: applet.on
    switchedOn: applet.on
    // A long press opens its menu.
    tipOnHold: false
    icon.name: "call-start"
    display: applet.compact || !applet.on ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
    text: applet.on ? applet.calls.line : applet.sioul.text("calls-switch")
    tip: applet.tipText()
    menuOpen: menuLoader.item !== null && (menuLoader.item as SioulMenu).opened

    function tipText(): string {
        return applet.on ? applet.sioul.textWith("calls-switch-tip-on", "line", applet.calls.line) : applet.sioul.text("calls-switch-tip-off")
    }

    function openMenu() {
        menuLoader.active = true
        const menu = menuLoader.item as SioulMenu
        menu.popup()
    }

    onChosen: {
        // Off: every call for an hour, the usual wait for a call; on: screened again.
        applet.sioul.callsThrough(!applet.on, applet.on ? 0 : 60)
        applet.checked = Qt.binding(() => applet.on)
    }

    contentItem: RowLayout {
        spacing: 4

        Icon {
            iconName: applet.icon.name
            color: applet.on ? applet.theme.accent : applet.theme.muted
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
            MenuItem {
                visible: applet.on
                height: visible ? implicitHeight : 0
                enabled: false
                text: applet.theme.plain(applet.calls.line)
            }
            MenuItem {
                visible: applet.on
                height: visible ? implicitHeight : 0
                text: applet.sioul.text("calls-note-again")
                onTriggered: applet.sioul.callsThrough(false, 0)
            }
            MenuItem {
                text: applet.sioul.text("calls-for-60")
                onTriggered: applet.sioul.callsThrough(true, 60)
            }
            MenuItem {
                text: applet.sioul.text("calls-until-off")
                onTriggered: applet.sioul.callsThrough(true, 0)
            }
            MenuItem {
                visible: Qt.platform.os === "android"
                height: visible ? implicitHeight : 0
                text: applet.sioul.text("calls-open-settings")
                onTriggered: {
                    const window = applet.Window.window
                    if (window && window.showParameters)
                        window.showParameters("calls")
                }
            }
        }
    }
}
