// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A sign-in page an extension asked for, on a computer (docs/extensions.md,
// "A sign-in"): a window of Sioul's own with Qt WebEngine, the site's name
// always in sight above the page, so that you see where a password is
// typed. Its profile is its own and kept in memory only (no storage name:
// off the record), gone with the window. Only https pages load; when a
// navigation reaches the address the extension watches for (an
// application's own scheme, such as `skoapp-prod://…`), it is stopped there,
// never loaded, and given back (`signInReturned`); closing the window gives
// back nothing. No scheme is registered with the system. Android has its own
// screen (SignIn.java); build.rs lists this file for computers only.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtWebEngine

Window {
    id: window

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // What the extension asked: {open, watch, host}.
    required property var page

    // The host of the page shown now: what is said above it.
    property string shownHost: window.page.host || ""
    // The answer given once: the address returned, or "" when closed.
    property bool answered: false
    // The page's own profile, once made.
    property WebEngineProfile made: null

    function hostOf(address) {
        const rest = String(address).replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
        return rest.split(/[\/?#]/)[0].replace(/^.*@/, "").toLowerCase()
    }

    function answer(address) {
        if (window.answered)
            return
        window.answered = true
        window.sioul.signInReturned(address)
    }

    // Whether a navigation may load: https only; inside a page, its blank frames too.
    function allowed(address, mainFrame) {
        const url = String(address)
        if (url.startsWith("https://"))
            return true
        return !mainFrame && (url === "about:blank" || url === "about:srcdoc")
    }

    width: 560
    height: 760
    minimumWidth: 360
    minimumHeight: 420
    visible: true
    color: window.theme.background
    title: window.sioul.textWith("sign-in-window-title", "host", window.shownHost)
    onClosing: window.answer("")

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // The site's name, always in sight.
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: bar.implicitHeight + 16
            color: window.theme.surface

            RowLayout {
                id: bar

                anchors.fill: parent
                anchors.margins: 8
                spacing: 10

                Label {
                    objectName: "signInSite"
                    Layout.fillWidth: true
                    text: window.sioul.textWith("sign-in-site", "host", window.shownHost)
                    textFormat: Text.PlainText
                    elide: Text.ElideMiddle
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    color: window.theme.text
                }
                Button {
                    objectName: "signInClose"
                    text: window.sioul.text("ui-cancel")
                    onClicked: window.close()
                }
            }
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 1
            color: window.theme.line
        }
        Item {
            id: holder

            Layout.fillWidth: true
            Layout.fillHeight: true
        }
    }

    Component {
        id: viewKind

        WebEngineView {
            id: view

            anchors.fill: parent
            onUrlChanged: window.shownHost = window.hostOf(view.url)
            onNavigationRequested: request => {
                const address = request.url.toString()
                // The address watched for: never loaded, given back whole.
                if (request.isMainFrame && window.page.watch && address.startsWith(window.page.watch)) {
                    request.reject()
                    window.answer(address)
                    window.close()
                    return
                }
                if (window.allowed(address, request.isMainFrame))
                    request.accept()
                else
                    request.reject()
            }
            // A link that would open another window opens here, https only.
            onNewWindowRequested: request => {
                if (String(request.requestedUrl).startsWith("https://"))
                    view.url = request.requestedUrl
            }
            // A redirect to the watched address that ends as a failed load rather than a navigation.
            onLoadingChanged: info => {
                const address = info.url.toString()
                if (window.page.watch && address.startsWith(window.page.watch)) {
                    view.stop()
                    window.answer(address)
                    window.close()
                }
            }
        }
    }

    Component {
        id: prototypeKind

        // No storage name: an off-the-record profile, in memory only, gone with the window.
        WebEngineProfilePrototype {
            persistentCookiesPolicy: WebEngineProfile.NoPersistentCookies
        }
    }

    Component.onCompleted: {
        const open = String(window.page.open || "")
        if (!open.startsWith("https://")) {
            window.answer("")
            window.close()
            return
        }
        const made = (prototypeKind.createObject(window) as WebEngineProfilePrototype).instance()
        // Without Qt's name: sign-in services refuse browsers they take for embedded (SitesWeb.qml).
        made.httpUserAgent = made.httpUserAgent.replace(/ QtWebEngine\/[\d.]+/, "")
        window.made = made
        viewKind.createObject(holder, { profile: made, url: open })
    }
}
