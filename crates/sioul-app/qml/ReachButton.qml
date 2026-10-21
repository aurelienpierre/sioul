// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The one button of "What reaches you" (part 5; the proposal's 3.5 and 3.6),
// a button of the status line (LineButton.qml): on a phone at the bottom, on
// a computer in the title bar. It took the place of do-not-disturb's switch
// and Expecting a call's. Its icon says the state: a bell; crossed out under
// do-not-disturb; with waves while Reachable. The accent marks any state but
// As usual, and where there is room it shows its words then ("Reachable
// until 18:00."). Its name for screen readers says the state and its end. A
// click or a tap opens the card "What reaches you now" (ReachCard.qml); a
// right click or a long press, a short menu: Reachable for an hour, Do not
// disturb for an hour, As usual.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

LineButton {
    id: button

    required property var sioul
    // The status line's moment (main.qml): its `dnd` part (everywhere.rs's `moment`), its `calls` part (calls.rs's).
    required property var moment
    // At the window's top (a computer's title bar): the card opens under it.
    property bool below: false
    readonly property var dnd: button.moment && button.moment.dnd ? button.moment.dnd : ({ on: false, line: "", why_line: "", details: [], end_time: "", state: "usual", reachable: null, paused: false })
    readonly property bool on: button.dnd.on === true
    // Reachable now, on any device.
    readonly property bool reachable: button.dnd.state === "reachable" && !button.on && button.dnd.reachable !== null && button.dnd.reachable !== undefined
    // "reachable", "usual" or "dnd".
    readonly property string reachState: button.on ? "dnd" : button.reachable ? "reachable" : "usual"
    // The state and its end, in a sentence.
    readonly property string stateLine: button.on ? button.dnd.line : button.reachable ? button.dnd.reachable.line : button.sioul.text("reach-usual")
    // The card, once made (tests, the documentation's pictures).
    readonly property alias cardLoader: cardLoader
    readonly property alias menuLoader: menuLoader

    switchedOn: button.on || button.reachable
    // A long press opens its menu.
    tipOnHold: false
    icon.name: button.on ? "notification-disabled" : button.reachable ? "notification-active" : "notification-inactive"
    display: button.compact || !button.switchedOn ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
    text: button.switchedOn ? button.stateLine : button.sioul.text("reach-card-title")
    textColor: button.theme.muted
    name: button.theme.plain(button.sioul.textWith("reach-button-name", "state", button.stateLine))
    menuOpen: (cardLoader.item !== null && (cardLoader.item as Popup).opened) || (menuLoader.item !== null && (menuLoader.item as SioulMenu).opened)

    function openCard() {
        cardLoader.active = true
        const card = cardLoader.item as ReachCard
        card.read()
        card.open()
    }

    function openMenu() {
        menuLoader.active = true
        const menu = menuLoader.item as SioulMenu
        menu.popup()
    }

    // As usual: do-not-disturb off (a session it holds, held off until it ends), or Reachable ended.
    function usual() {
        if (button.on)
            button.sioul.dndToggle(false, 0)
        else if (button.reachable)
            button.sioul.reachChoose("usual", "")
    }

    onChosen: button.openCard()

    TapHandler {
        acceptedButtons: Qt.RightButton
        // A touch has no buttons: on a touch screen, the long press below.
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: button.openMenu()
    }
    TapHandler {
        id: hold

        acceptedDevices: PointerDevice.TouchScreen
        onLongPressed: {
            // Letting go then changes nothing.
            button.held = true
            const window = button.Window.window
            if (window)
                window.menuAt = hold.point.scenePosition
            button.openMenu()
        }
    }

    // Made the first time it opens.
    Loader {
        id: cardLoader

        active: false
        sourceComponent: ReachCard {
            sioul: button.sioul
            theme: button.theme
            anchor: button
            below: button.below
            dnd: button.dnd
            reachState: button.reachState
            stateLine: button.stateLine
            moment: button.moment
        }
    }
    Loader {
        id: menuLoader

        active: false
        sourceComponent: SioulMenu {
            MenuItem {
                objectName: "menu-reachable"
                visible: button.dnd.paused !== true && !button.reachable
                height: visible ? implicitHeight : 0
                text: button.dnd.reach_capped !== undefined && button.dnd.reach_capped[1] === true ? button.theme.plain(button.sioul.textWith("reach-for-60-latest", "time", button.dnd.reach_tonight_time || "")) : button.sioul.text("reach-for-60")
                onTriggered: button.sioul.reachChoose("reachable", "60")
            }
            MenuItem {
                objectName: "menu-dnd"
                text: button.sioul.text("reach-menu-dnd")
                onTriggered: button.sioul.dndToggle(true, 60)
            }
            MenuItem {
                objectName: "menu-usual"
                visible: button.reachState !== "usual"
                height: visible ? implicitHeight : 0
                text: button.sioul.text("reach-usual")
                onTriggered: button.usual()
            }
        }
    }
}
