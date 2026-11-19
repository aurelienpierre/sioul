// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A site's menu, on a computer and on Android alike (SitesPage.qml,
// android/SitesPage.qml): what it is (its type), its ties, what it is for
// (the hours it comes forward in), its place in your order, a link, its
// name and address, Remove. On a computer also what only Qt WebEngine
// carries: kept open to hear from it, the microphone, the camera, sharing
// the screen, the devices of calls, another login from Bitwarden (`calls`,
// `canChoose`). Every change is written at once (`sioul.setSite`); what went
// wrong is said (`said`), and the page reads its sites again (`changed`).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

SioulMenu {
    id: menu

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The main window (main.qml): `openChain`, `linkFrom`.
    required property var window
    // The site the menu acts on, as `sioul.siteList()` gives it; null when none.
    property var site: null
    // Its place in your order, and how many sites there are.
    property int index: -1
    property int total: 0
    // What only a computer's page carries: kept open, the call switches, the devices of calls.
    property bool calls: true
    // "Choose a login…": the site is the one open, on a computer.
    property bool canChoose: false

    // What went wrong, else "".
    signal said(string problem)
    // A setting changed: the page reads its sites again.
    signal changed()
    signal editAsked()
    signal removeAsked()
    signal devicesAsked()
    signal chooseAsked()

    readonly property string siteId: menu.site !== null ? menu.site.id : ""
    readonly property var kinds: ["mailbox", "chat", "video", "social", "dating", "other"]

    function set(field, value) {
        menu.said(menu.sioul.setSite(menu.siteId, field, value))
        menu.changed()
    }

    Repeater {
        model: menu.kinds

        delegate: MenuItem {
            required property string modelData

            text: menu.sioul.text("site-kind-" + modelData)
            checkable: true
            checked: menu.site !== null && menu.site.kind === modelData
            onTriggered: menu.set("site", modelData)
        }
    }
    MenuSeparator {}
    // Its ties as a chain (ChainPage.qml): the person it is opened for, its mail.
    MenuItem {
        text: menu.sioul.text("chain-its-ties")
        onTriggered: menu.window.openChain("sioul:site/" + encodeURIComponent(menu.siteId))
    }
    MenuItem {
        visible: menu.calls
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("site-background")
        checkable: true
        checked: menu.site !== null && menu.site.background === true
        onTriggered: menu.set("background", String(checked))
    }
    // What it is for: the hours it comes forward in, any of them together (docs/areas.md).
    // None ticked: as its kind goes.
    SioulMenu {
        title: menu.sioul.text("site-area")

        Repeater {
            model: ["work", "admin", "leisure"]

            delegate: MenuItem {
                required property string modelData

                readonly property var on: menu.site !== null ? String(menu.site.area).split("+") : []

                text: menu.sioul.text("area-" + modelData)
                checkable: true
                checked: on.indexOf(modelData) >= 0
                onTriggered: {
                    const was = on
                    const now = ["work", "admin", "leisure"].filter(a => a === modelData ? checked : was.indexOf(a) >= 0)
                    menu.set("area", now.join("+"))
                }
            }
        }
    }
    MenuSeparator {
        visible: menu.calls
        height: visible ? implicitHeight : 0
    }
    // For calls: what the site may use, each asked again at every call.
    MenuItem {
        visible: menu.calls
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("site-microphone")
        checkable: true
        checked: menu.site !== null && menu.site.microphone === true
        onTriggered: menu.set("microphone", String(checked))
    }
    MenuItem {
        visible: menu.calls
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("site-camera")
        checkable: true
        checked: menu.site !== null && menu.site.camera === true
        onTriggered: menu.set("camera", String(checked))
    }
    MenuItem {
        visible: menu.calls
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("site-screen")
        checkable: true
        checked: menu.site !== null && menu.site.screen === true
        onTriggered: menu.set("screen", String(checked))
    }
    // Which camera, microphone and speaker calls use: the same for every site.
    MenuItem {
        visible: menu.calls
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("site-devices") + "…"
        onTriggered: menu.devicesAsked()
    }
    MenuSeparator {}
    // Your order: the list keeps it (by type, within each).
    MenuItem {
        text: menu.sioul.text("site-move-up")
        enabled: menu.index > 0
        onTriggered: {
            menu.said(menu.sioul.moveSite(menu.siteId, -1))
            menu.changed()
        }
    }
    MenuItem {
        text: menu.sioul.text("site-move-down")
        enabled: menu.index >= 0 && menu.index < menu.total - 1
        onTriggered: {
            menu.said(menu.sioul.moveSite(menu.siteId, 1))
            menu.changed()
        }
    }
    // Tied to a task, a note, a contact: a site is a correspondent too.
    MenuItem {
        text: menu.sioul.text("site-link")
        onTriggered: menu.window.linkFrom({ uri: "sioul:site/" + encodeURIComponent(menu.siteId), kind: "site", key: menu.siteId, title: menu.site !== null ? menu.site.name : "" })
    }
    MenuSeparator {}
    // Another login than the site's own, or one of several: the vault searched (the site open).
    MenuItem {
        visible: menu.canChoose
        height: visible ? implicitHeight : 0
        text: menu.sioul.text("bitwarden-choose")
        onTriggered: menu.chooseAsked()
    }
    MenuItem {
        text: menu.sioul.text("site-edit")
        onTriggered: menu.editAsked()
    }
    MenuItem {
        text: menu.sioul.text("site-remove")
        onTriggered: menu.removeAsked()
    }
}
