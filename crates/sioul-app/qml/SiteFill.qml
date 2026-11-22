// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A phone's site screen asks the window for a login from Bitwarden
// (SiteScreen.java, sitefill.rs). The window comes forward with its own
// cards, the very ones a computer's Sites page and the accounts' forms use:
// the vault's unlock card when it is locked (VaultUnlock.qml: the master
// password and a code; no security key on a phone), then, for Fill from
// Bitwarden, the site's only login at once, else the chooser
// (LoginChooser.qml; Choose a login… always opens it). The login chosen,
// or the cancel, goes back to Rust once (`siteFillChosen`,
// `siteFillCancelled`), which brings the site back forward: only the
// login's id leaves here, never its password. A request is read as this is
// made (Android may have ended Sioul meanwhile, the site's screen alone made
// again) and at each `siteFillAsked`. A card closes once its request no
// longer waits (review of 22 November 2026, finding 8): its screen gone or
// another site opened (Rust forgot it), or its ten minutes over (Rust then
// brings the site back, saying that no login was chosen in time). Made on
// Android only (main.qml).

pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: fill

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme

    // The request answered now: {serial, url, mode ("fill" or "choose")}; null when none.
    property var asked: null
    // Its answer handed back already: a card closing after it hands nothing more.
    property bool answered: true
    // For the window's tests: the unlock card and the chooser's holder.
    property alias unlock: unlock
    property alias chooser: chooser

    // The request waiting, begun unless it is the one under way. An older
    // one still open has its cards closed and needs no answer: Rust keeps
    // the newer one alone. One that no longer waits, none newer coming,
    // has its cards closed and is handed back as cancelled, once: Rust
    // takes that only while it still holds the request (its ten minutes
    // over), and then brings the site back.
    function take() {
        const said = fill.sioul.siteFillWaiting()
        const request = said ? JSON.parse(said) : null
        if (fill.asked !== null && request !== null && fill.asked.serial === request.serial) {
            fill.expireIn(request)
            return
        }
        if (fill.asked !== null) {
            if (request === null)
                fill.cancel()
            fill.asked = null
            unlock.close()
            chooser.close()
        }
        if (request === null) {
            expiry.stop()
            return
        }
        fill.asked = request
        fill.expireIn(request)
        fill.answered = false
        const state = fill.sioul.bitwardenState()
        if (state === "locked")
            unlock.begin()
        else if (state === "unlocked")
            fill.next()
        else
            fill.cancel()
    }

    // The vault open: Fill from Bitwarden takes the site's only login at
    // once (unless another was chosen there last); else the chooser.
    function next() {
        if (fill.asked === null || fill.answered)
            return
        if (fill.asked.mode === "fill") {
            const site = JSON.parse(fill.sioul.bitwardenSite(fill.asked.url))
            if (!site.error && site.only) {
                fill.choose(site.only)
                return
            }
        }
        chooser.now().begin(fill.asked.url)
    }

    // The request read again once its ten minutes are over ("left", in seconds).
    function expireIn(request) {
        expiry.interval = Math.max(1, request.left || 0) * 1000 + 250
        expiry.restart()
    }

    // A login chosen, by its id: handed back once.
    function choose(item) {
        if (fill.asked === null || fill.answered)
            return
        fill.answered = true
        fill.sioul.siteFillChosen(fill.asked.serial, item)
    }

    // Nothing chosen: handed back once; the site comes back with nothing filled.
    function cancel() {
        if (fill.asked === null || fill.answered)
            return
        fill.answered = true
        fill.sioul.siteFillCancelled(fill.asked.serial)
    }

    // A card closed: cancelled once the closing is over, unless a login was
    // chosen meanwhile, the next card opened (the chooser after the vault
    // opened), or another request came.
    function closed() {
        const request = fill.asked
        Qt.callLater(() => {
            if (fill.asked === request && !unlock.visible && !(chooser.item !== null && (chooser.item as LoginChooser).visible))
                fill.cancel()
        })
    }

    Component.onCompleted: fill.take()

    // A request's ten minutes: then it is read again, and closed when it no longer waits.
    Timer {
        id: expiry

        onTriggered: fill.take()
    }

    Connections {
        target: fill.sioul

        function onSiteFillAsked() {
            fill.take()
        }
    }

    // Bitwarden, opened here for the session, as from Sites or Accounts.
    VaultUnlock {
        id: unlock

        sioul: fill.sioul
        theme: fill.theme
        onUnlocked: fill.next()
    }
    Connections {
        target: unlock

        function onClosed() {
            fill.closed()
        }
    }

    // Which login: the site's own first, any of the vault's found by site and user name.
    Later {
        id: chooser

        sourceComponent: Component {
            LoginChooser {
                sioul: fill.sioul
                theme: fill.theme
                onChosen: item => fill.choose(item)
            }
        }
    }
    Connections {
        target: chooser.item

        function onClosed() {
            fill.closed()
        }
    }
}
