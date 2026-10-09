// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Bitwarden unlock dialog (VaultUnlock.qml), on a stand-in for Sioul
// that answers as Bitwarden's server would: later, by ticket, as the window
// does off its thread (sites.rs, `bitwarden_later`). The dialog says it
// waits, asks nothing more meanwhile, takes its own answer and no other, and
// forgets it once closed. A second step it cannot take: a security key from
// a dialog that cannot ask one (Accounts, a phone) says where the key works;
// Duo alone, as before. Run by tools/qml-test.sh.

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
            "bitwarden-factor-key-sites": "Your key, from the Sites page.",
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

    // As Accounts has it: no page to ask a security key from.
    VaultUnlock {
        id: unlock

        sioul: mock
        theme: testTheme
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

        function test_a_key_it_cannot_ask_says_where_it_can() {
            unlock.tryIt()
            mock.bitwardenAnswered(1, JSON.stringify({ factor: [7], key: { page: "https://vault.example.org/", script: "" } }))
            compare(unlock.offered.length, 0)
            compare(unlock.problem, Qt.platform.os === "android" ? "Your key, not on a phone." : "Your key, from the Sites page.")
            unlock.begin()
            unlock.tryIt()
            mock.bitwardenAnswered(2, JSON.stringify({ factor: [2] }))
            compare(unlock.problem, "Duo, not yet.", "Duo alone")
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
