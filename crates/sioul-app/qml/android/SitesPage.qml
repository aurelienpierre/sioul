// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Sites page on Android. The computer's list, menu and forms
// (SitesList.qml, SiteMenu.qml, SiteEditDialog.qml, SiteAddDialog.qml): a
// site added from the usual sites or by hand, changed, removed, its type and
// what it is for, the filter, the order, each site's icon and news. Qt
// WebEngine has no Android version: a site opens in a screen of Sioul's own
// around the system's web view (SiteScreen.java), or in the browser where
// this device's web view cannot carry it; each line says which, and why
// (siteview.rs). Same name and same calls as the computer's page, as far as
// the window uses them (open, clickNotice, openSettings, sites, menuId,
// siteMenu, presetsMenu, filterMenu).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
// The window's own files, beside this folder.
import ".."

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var sites: []
    // What went wrong last, said above the list.
    property string problem: ""
    // The site the menu acts on: the one long-pressed.
    property string menuId: ""
    readonly property var menuSite: page.sites.find(s => s.id === page.menuId) || null
    readonly property int menuIndex: page.sites.findIndex(s => s.id === page.menuId)
    property alias siteMenu: siteMenu
    property alias presetsMenu: list.presetsMenu
    property alias filterMenu: list.filterMenu
    property alias laterShown: list.laterShown

    function reload() {
        page.sites = JSON.parse(page.sioul.siteList() || "[]")
    }

    // A site asked for, by its line, a link, the Porch or a notice: in Sioul's
    // own screen, over the window, or in the browser (siteview.rs decides). Its
    // news is seen either way. The screen's bar takes Sioul's colours.
    function open(id) {
        const answer = JSON.parse(page.sioul.siteOpen(id, JSON.stringify(page.look)) || "{}")
        page.problem = answer.problem || ""
        if (answer.browser)
            page.outside(answer.browser)
        if (answer.browser || answer.here === true)
            page.sioul.siteSeen(id)
    }
    // The browser, given an address (a test stands one in).
    property var outside: address => Qt.openUrlExternally(address)
    readonly property var look: ({ dark: page.theme.dark, background: String(page.theme.background), surface: String(page.theme.surface), text: String(page.theme.text), muted: String(page.theme.muted), line: String(page.theme.line), accent: String(page.theme.accent) })

    // A site's notice clicked: its web notice is a computer's; the site alone opens here.
    function clickNotice(id, key) {
        return false
    }
    // This page has no ⚙ on Android: calmer colours are a computer's, and the vault's fill comes later.
    function openSettings(open, key) {
    }

    Component.onCompleted: page.reload()
    onVisibleChanged: if (page.visible) page.reload()

    Connections {
        target: page.sioul

        function onSitesChanged() {
            page.reload()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        Label {
            visible: page.problem !== ""
            Layout.fillWidth: true
            text: page.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: page.theme.warm
        }
        SitesList {
            id: list

            Layout.fillWidth: true
            Layout.fillHeight: true
            sioul: page.sioul
            theme: page.theme
            sites: page.sites
            note: page.sioul.text("site-android")
            onChosen: id => page.open(id)
            onMenuAsked: id => {
                page.menuId = id
                siteMenu.popup()
            }
            onAddAsked: adding.now().open()
            onPinned: problem => {
                page.problem = problem
                page.reload()
            }
        }
    }

    // A site's menu, at a long press: what only Qt WebEngine carries is a computer's.
    SiteMenu {
        id: siteMenu

        sioul: page.sioul
        window: page.window
        site: page.menuSite
        index: page.menuIndex
        total: page.sites.length
        calls: false
        onSaid: problem => page.problem = problem
        onChanged: page.reload()
        onEditAsked: editing.now().begin(page.menuSite)
        onRemoveAsked: removing.now().ask(page.menuSite ? page.menuSite.name : "", page.sioul.text("site-remove-ask-android"), page.sioul.text("site-remove"))
    }

    // The forms, made the first time they are asked for (Later.qml).
    Later {
        id: editing

        sourceComponent: Component {
            SiteEditDialog {
                sioul: page.sioul
                theme: page.theme
                room: page.width
                onSaved: page.reload()
            }
        }
    }
    Later {
        id: adding

        sourceComponent: Component {
            SiteAddDialog {
                sioul: page.sioul
                theme: page.theme
                room: page.width
                roomTall: page.height
                onAdded: id => {
                    page.reload()
                    page.open(id)
                }
            }
        }
    }
    // Out of Sioul, and its sign-in on this device forgotten as far as the
    // web view can: the line under the title says what stays (review K1).
    Later {
        id: removing

        sourceComponent: Component {
            ConfirmDialog {
                sioul: page.sioul
                theme: page.theme
                onConfirmed: {
                    page.problem = page.sioul.removeSite(page.menuId)
                    page.reload()
                }
            }
        }
    }
}
