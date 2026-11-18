// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The sites' one web profile, and Bitwarden's page that asks a security
// key, held by the window on computers (main.qml's `sitesWeb`), made the
// first time either is wanted: a site's view (SitesPage.qml), or a key
// asked by a vault's dialog, from Sites or from Accounts (VaultUnlock.qml),
// so that both pages open the vault the same way. One profile for every
// site and for the vault's page: logins kept, files cached on disk. Its
// prototype, once complete, starts Chromium (some 25 MB in Sioul's process
// and 45 MB in its zygotes): made only when a site's view or a key's page
// needs it. A key signs only for the page it is asked from, and Bitwarden
// takes signatures for its vault's address only: its own light page is
// loaded, held unseen in the dialog; the key's PIN, when it has one, comes
// in Sioul's own dialog, above (WebAuthDialog.qml). Android has neither:
// no Qt WebEngine there, and no key asked (build.rs lists this file for
// computers only).

pragma ComponentBehavior: Bound

import QtQuick
import QtWebEngine

Item {
    id: web

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme

    // In every page, before its own scripts: Qt 6.10–6.11 never answers
    // PublicKeyCredential.getClientCapabilities(), and sites asking for a
    // security key wait for it forever (QTBUG-149575); without it they ask
    // the key directly. Injection point 2: DocumentCreation; world 0: the page's.
    readonly property var webauthnFix: ({
        name: "sioul-webauthn",
        sourceCode: "if (window.PublicKeyCredential && PublicKeyCredential.getClientCapabilities) { delete PublicKeyCredential.getClientCapabilities; }",
        injectionPoint: 2,
        worldId: 0,
        runsOnSubFrames: true
    })

    // The profile, once made (`profile()`).
    property WebEngineProfile made: null

    Component {
        id: prototypeKind

        WebEngineProfilePrototype {
            storageName: "sioul-sites"
            httpCacheType: WebEngineProfile.DiskHttpCache
            httpCacheMaximumSize: 512 * 1024 * 1024
            persistentCookiesPolicy: WebEngineProfile.ForcePersistentCookies
            // What a site was allowed is kept: notifications for good (the Sites
            // page's `keepPermissions`), a microphone, a camera, a screen while its switch is on.
            persistentPermissionsPolicy: WebEngineProfile.PersistentPermissionsPolicy.StoreOnDisk
        }
    }

    // The profile, made from its prototype the first time, complete once made:
    // no view exists before it.
    function profile() {
        if (web.made !== null)
            return web.made
        const made = (prototypeKind.createObject(web) as WebEngineProfilePrototype).instance()
        // Without Qt's name in it: some chats refuse browsers they do not know.
        made.httpUserAgent = made.httpUserAgent.replace(/ QtWebEngine\/[\d.]+/, "")
        // Nor in the brands it tells sites: Google refuses sign-in to browsers it takes for embedded.
        const brands = made.clientHints.fullVersionList
        const kept = {}
        for (const name in brands) {
            if (!/qt/i.test(name))
                kept[name] = brands[name]
        }
        made.clientHints.fullVersionList = kept
        web.made = made
        return made
    }

    // Bitwarden's page that asks a security key, for a vault's dialog
    // (VaultUnlock.qml's `keyView`), the profile made first: a new page for
    // each ask, given what it asks ({mode, page, script}) once made.
    function keyPage() {
        web.profile()
        return keyPageKind
    }

    Component {
        id: keyPageKind

        WebEngineView {
            id: view

            // What the dialog asks: {mode, page, script} (VaultUnlock.qml's `keyAsk`).
            property var ask: null

            profile: web.made
            // The scripts first, then the page: injection point 1, once its document is ready.
            onAskChanged: {
                if (view.ask === null)
                    return
                view.userScripts.collection = [web.webauthnFix, {
                    name: "sioul-key",
                    sourceCode: view.ask.script,
                    injectionPoint: 1,
                    worldId: 0,
                    runsOnSubFrames: false
                }]
                view.url = view.ask.page
            }
            onWebAuthUxRequested: request => webAuth.show(request)
        }
    }

    WebAuthDialog {
        id: webAuth

        sioul: web.sioul
        theme: web.theme
    }
}
