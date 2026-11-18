// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Bitwarden unlock dialog (VaultUnlock.qml), on a stand-in for Sioul
// that answers as Bitwarden's server would: later, by ticket, as the window
// does off its thread (sites.rs, `bitwarden_later`). The dialog says it
// waits, asks nothing more meanwhile, takes its own answer and no other, and
// forgets it once closed. A security key as second step: a dialog given the
// window's key page (every dialog on a computer, from Sites or from
// Accounts: SitesWeb.qml) offers it and asks it there; one without (a
// phone) says what to do instead; Duo alone says it cannot be taken. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 700
    height: 600

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property int tickets: 0
        // What the dialog asked, in order: [verb, …].
        property var asked: []
        readonly property var words: ({
            "bitwarden-waiting": "Waiting for Bitwarden…",
            "bitwarden-open": "Open",
            "bitwarden-factor-unsupported": "Duo, not yet.",
            "bitwarden-factor-key-phone": "Your key, not on a phone.",
            "bitwarden-code-sent": "The code was sent."
        })

        signal bitwardenAnswered(int ticket, string answer)

        function text(key) {
            return mock.words[key] || key
        }
        function textWith(key, name, value) {
            return mock.text(key)
        }
        function viewFlag(name) {
            return false
        }
        function ask(what) {
            mock.tickets += 1
            mock.asked = mock.asked.concat([what])
            return mock.tickets
        }
        function bitwardenUnlock(password, provider, code) {
            return mock.ask(["unlock", password, provider, code])
        }
        function bitwardenSendCode(password) {
            return mock.ask(["code", password])
        }
        function bitwardenPasskeyBegin() {
            return mock.ask(["passkey-begin"])
        }
        function bitwardenPasskey(answer) {
            return mock.ask(["passkey", answer])
        }
    }

    // As a phone has it: no page to ask a security key from.
    VaultUnlock {
        id: unlock

        sioul: mock
        theme: testTheme
    }

    // As every dialog has it on a computer, Accounts' as Sites': the window's
    // key page (SitesWeb.qml's `keyPage`), here a stand-in that records what it
    // is asked and answers nothing yet.
    property int keyPagesMade: 0
    Component {
        id: keyPageStandIn

        Item {
            property var ask: null

            Component.onCompleted: root.keyPagesMade += 1
            function runJavaScript(script, done) {
                done("")
            }
        }
    }
    VaultUnlock {
        id: withKey

        sioul: mock
        theme: testTheme
        keys: () => keyPageStandIn
    }

    SignalSpy {
        id: unlockedSpy

        target: unlock
        signalName: "unlocked"
    }

    TestCase {
        name: "VaultUnlock"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                walk(children[i], test, found)
            if (item.contentItem && item.contentItem !== item && children.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function shown(text) {
            return walk(unlock.contentItem.parent, i => i.text === text && i.visible, [])[0] || null
        }
        function openButton() {
            return walk(unlock.footer, i => i.text === "Open", [])[0] || null
        }

        function init() {
            mock.tickets = 0
            mock.asked = []
            unlockedSpy.clear()
            unlock.begin()
            tryCompare(unlock, "opened", true)
        }
        function cleanup() {
            unlock.close()
            tryCompare(unlock, "visible", false, 2000)
        }

        function test_it_waits_for_its_own_answer() {
            unlock.tryIt()
            compare(mock.asked.length, 1)
            compare(mock.asked[0][0], "unlock")
            verify(unlock.busy)
            verify(shown("Waiting for Bitwarden…") !== null, "said while it waits")
            verify(!openButton().enabled, "Open greyed meanwhile")
            unlock.tryIt()
            compare(mock.asked.length, 1, "nothing more asked while it waits")
            mock.bitwardenAnswered(99, JSON.stringify({ ok: true }))
            verify(unlock.busy, "another ticket's answer is not this one's")
            compare(unlockedSpy.count, 0)
            mock.bitwardenAnswered(1, JSON.stringify({ ok: true }))
            verify(!unlock.busy)
            compare(unlockedSpy.count, 1)
            tryCompare(unlock, "visible", false)
        }

        function test_what_went_wrong_comes_back() {
            unlock.tryIt()
            mock.bitwardenAnswered(1, JSON.stringify({ error: "No network." }))
            compare(unlock.problem, "No network.")
            verify(!unlock.busy)
            verify(shown("Waiting for Bitwarden…") === null)
            verify(openButton().enabled)
        }

        function test_the_code_by_mail_is_sent_and_waited_for() {
            unlock.tryIt()
            mock.bitwardenAnswered(1, JSON.stringify({ factor: [1] }))
            compare(unlock.provider, 1)
            compare(mock.asked[1][0], "code", "the e-mail's code asked at once")
            verify(unlock.busy)
            mock.bitwardenAnswered(2, "")
            verify(unlock.sent)
            compare(unlock.note, "The code was sent.")
            verify(!unlock.busy)
        }

        function test_a_key_it_cannot_ask_says_what_to_do_instead() {
            unlock.tryIt()
            mock.bitwardenAnswered(1, JSON.stringify({ factor: [7], key: { page: "https://vault.example.org/", script: "" } }))
            compare(unlock.offered.length, 0)
            compare(unlock.problem, "Your key, not on a phone.")
            unlock.begin()
            unlock.tryIt()
            mock.bitwardenAnswered(2, JSON.stringify({ factor: [2] }))
            compare(unlock.problem, "Duo, not yet.", "Duo alone")
        }

        // On a computer, from Accounts as from Sites: the key offered, asked at once
        // from the window's page, given what to ask; no sentence sends elsewhere.
        function test_a_dialog_given_the_window_s_key_page_asks_the_key() {
            unlock.close()
            root.keyPagesMade = 0
            withKey.begin()
            tryCompare(withKey, "opened", true)
            verify(withKey.keyCapable)
            withKey.tryIt()
            const ticket = mock.tickets
            mock.bitwardenAnswered(ticket, JSON.stringify({ factor: [7], key: { page: "https://vault.example.org/", script: "/* ask */" } }))
            compare(withKey.offered, [7])
            compare(withKey.provider, 7)
            compare(withKey.problem, "", "nothing said of another page")
            tryVerify(() => withKey.keyView.item !== null, 2000, "the key's page made")
            compare(withKey.keyView.item.ask.page, "https://vault.example.org/")
            compare(withKey.keyView.item.ask.script, "/* ask */")
            compare(root.keyPagesMade, 1)
            withKey.close()
            tryCompare(withKey, "visible", false, 2000)
            unlock.begin()
        }

        function test_closed_it_waits_no_more() {
            unlock.tryIt()
            unlock.close()
            verify(!unlock.busy)
            mock.bitwardenAnswered(1, JSON.stringify({ ok: true }))
            compare(unlockedSpy.count, 0, "the vault opens all the same; this dialog says nothing")
        }
    }
}
