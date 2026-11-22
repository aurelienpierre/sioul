// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's part of a phone's fill from Bitwarden (SiteFill.qml), on a
// stand-in for Sioul: a fill asked opens the unlock card when the vault is
// locked, then fills the site's only login or opens the chooser; Choose a
// login… opens the chooser; the login chosen and the cancel are each handed
// back once, never both, with the request's number; a request told twice is
// begun once; a request waiting as the window starts is begun then. The
// vault's own rules are Rust's (sitefill.rs, sites.rs); here no password
// ever reaches the stand-in. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 891

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // What the vault is: "missing", "locked" or "unlocked".
        property string state: "locked"
        // The request Rust keeps for the window, as JSON; "" when none.
        property string waiting: ""
        // The site's only login, filled at once; "" when there is a choice to make.
        property string only: ""
        property int tickets: 0
        property int unlocks: 0
        // What the window handed back, in order: ["chosen", serial, id] or ["cancelled", serial].
        property var handed: []
        // Made up: no real vault, no real person.
        property var logins: [
            { id: "a", name: "Forum", username: "jane", site: "forum.example.net", elsewhere: false },
            { id: "b", name: "Forum, other", username: "j.doe", site: "forum.example.net", elsewhere: false }
        ]

        signal siteFillAsked
        signal bitwardenAnswered(int ticket, string answer)
        signal bitwardenFound(int ticket, string found)
        signal bitwardenLocked

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key
        }
        function viewFlag(name) {
            return false
        }
        function siteFillWaiting() {
            return mock.waiting
        }
        function siteFillChosen(serial, id) {
            mock.handed = mock.handed.concat([["chosen", serial, id]])
        }
        function siteFillCancelled(serial) {
            mock.handed = mock.handed.concat([["cancelled", serial]])
        }
        function bitwardenState() {
            return mock.state
        }
        function bitwardenSite(url) {
            return JSON.stringify({ domain: "example.net", chosen: "", only: mock.only })
        }
        function bitwardenSearch(url, site, user) {
            return JSON.stringify({ found: mock.logins, more: 0 })
        }
        function bitwardenUnlock(password, provider, code) {
            mock.unlocks += 1
            mock.tickets += 1
            return mock.tickets
        }
        function bitwardenSendCode(password) {
            mock.tickets += 1
            return mock.tickets
        }
        function bitwardenPasskeyBegin() {
            mock.tickets += 1
            return mock.tickets
        }
        function bitwardenPasskey(answer) {
            mock.tickets += 1
            return mock.tickets
        }
    }

    SiteFill {
        id: fill

        sioul: mock
        theme: testTheme
    }

    Component {
        id: starting

        SiteFill {
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "SiteFill"
        when: windowShown

        property int serial: 0

        // A fill asked from a site's screen, as Rust tells it.
        function ask(mode) {
            serial += 1
            mock.waiting = JSON.stringify({ serial: serial, url: "https://forum.example.net/sign-in", mode: mode })
            mock.siteFillAsked()
        }
        // The vault opened in the unlock card, as Bitwarden's server would answer.
        function open() {
            fill.unlock.tryIt()
            mock.bitwardenAnswered(mock.tickets, JSON.stringify({ ok: true }))
            mock.state = "unlocked"
        }
        // What was handed back once every closing is over.
        function settled() {
            wait(50)
            return mock.handed
        }

        function init() {
            mock.handed = []
            mock.state = "locked"
            mock.only = ""
            mock.unlocks = 0
        }
        function cleanup() {
            fill.unlock.close()
            fill.chooser.close()
            tryCompare(fill.unlock, "visible", false, 2000)
            wait(50)
        }

        function test_the_vault_locked_opens_the_card_then_fills_the_site_s_one_login() {
            mock.only = "a"
            ask("fill")
            tryCompare(fill.unlock, "opened", true)
            compare(mock.handed.length, 0, "nothing handed back while the card asks")
            open()
            tryCompare(fill.unlock, "visible", false)
            compare(settled(), [["chosen", serial, "a"]], "the site's one login, handed back once, no cancel after it")
            verify(!(fill.chooser.item && fill.chooser.item.visible), "no chooser for one login")
        }

        function test_the_vault_locked_then_several_logins_open_the_chooser() {
            ask("fill")
            tryCompare(fill.unlock, "opened", true)
            open()
            tryVerify(() => fill.chooser.item !== null && fill.chooser.item.opened, 2000, "the chooser after the card")
            compare(settled(), [], "nothing handed back while the chooser asks")
            fill.chooser.item.take(1)
            compare(settled(), [["chosen", serial, "b"]], "the login chosen, once, no cancel after it")
        }

        function test_choose_a_login_opens_the_chooser_at_once_and_its_cancel_is_handed_back_once() {
            mock.state = "unlocked"
            mock.only = "a"
            ask("choose")
            tryVerify(() => fill.chooser.item !== null && fill.chooser.item.opened, 2000, "Choose a login…: the chooser, even with one login")
            verify(!fill.unlock.visible, "the vault open: no card")
            fill.chooser.item.close()
            compare(settled(), [["cancelled", serial]])
            fill.chooser.item.take(0)
            compare(settled(), [["cancelled", serial]], "answered already: nothing more")
        }

        function test_the_unlock_card_cancelled_hands_back_nothing_filled() {
            ask("fill")
            tryCompare(fill.unlock, "opened", true)
            fill.unlock.close()
            compare(settled(), [["cancelled", serial]])
            fill.unlock.close()
            compare(settled(), [["cancelled", serial]], "once")
        }

        function test_a_request_told_twice_is_begun_once() {
            ask("fill")
            tryCompare(fill.unlock, "opened", true)
            fill.unlock.tryIt()
            compare(mock.unlocks, 1)
            mock.siteFillAsked()
            verify(fill.unlock.busy, "the same request: the card kept as it was, still waiting for Bitwarden")
            compare(settled(), [])
            ask("choose")
            tryCompare(fill.unlock, "opened", true)
            verify(!fill.unlock.busy, "a newer request begins afresh")
            compare(settled(), [], "the older one needs no answer: Rust keeps the newer alone")
            fill.unlock.close()
            compare(settled(), [["cancelled", serial]], "the newer one's cancel, with its own number")
        }

        function test_a_request_waiting_as_the_window_starts_is_begun_then() {
            serial += 1
            mock.waiting = JSON.stringify({ serial: serial, url: "https://forum.example.net/", mode: "fill" })
            const made = starting.createObject(root)
            verify(made !== null)
            tryCompare(made.unlock, "opened", true)
            made.unlock.close()
            compare(settled(), [["cancelled", serial]])
            made.destroy()
        }

        function test_no_account_any_more_hands_back_nothing_filled() {
            mock.state = "missing"
            ask("fill")
            compare(settled(), [["cancelled", serial]])
            verify(!fill.unlock.visible)
        }
    }
}
