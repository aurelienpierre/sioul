// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The ways back to what Sioul keeps (WaysBack.qml), on a stand-in for Sioul:
// with none kept, what a lost key would cost is said (shared or not); a
// passphrase kept is said kept and its field emptied; a recovery code made is
// shown once; a way taken out goes; which passphrase a way wants is said,
// and a security key's way gone stale (the key reset or renewed) is said.
// Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 600
    height: 900

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var ways: []
        property bool sharing: false
        property var stale: []
        property string passphrase: ""

        function text(key) {
            return key
        }
        function storeWays() {
            return JSON.stringify({ sealed: true, ways: mock.ways, stale: mock.stale, passphrase: mock.passphrase, sharing: mock.sharing, versions: 2 })
        }
        function keepPassphraseWay(passphrase, sharing) {
            if (passphrase.length < 12)
                return "store-way-passphrase-short"
            mock.ways = mock.ways.concat(["passphrase"])
            return ""
        }
        function makeRecoveryCode() {
            mock.ways = mock.ways.concat(["code"])
            return "ABCD-EFGH-JKMN"
        }
        function keepCardWay() {
            return "store-way-card-none"
        }
        function forgetWay(kind) {
            mock.ways = mock.ways.filter(w => w !== kind)
            return ""
        }
    }

    WaysBack {
        id: ways

        width: parent.width
        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "WaysBack"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            return null
        }

        function init() {
            mock.ways = []
            mock.sharing = false
            mock.stale = []
            mock.passphrase = ""
            ways.code = ""
            ways.said = ""
            ways.reread()
        }

        function test_none_kept_says_what_a_lost_key_would_cost() {
            const none = find(ways, "storeWaysNone")
            verify(none.visible)
            compare(none.text, "store-ways-none-alone")
            mock.sharing = true
            ways.reread()
            compare(none.text, "store-ways-none", "the sharing brings back what it carries: said")
        }

        function test_a_passphrase_kept_and_one_too_short() {
            const field = find(ways, "storeWayPassphrase")
            field.text = "short"
            find(ways, "storeWayPassphraseKeep").clicked()
            compare(ways.said, "store-way-passphrase-short")
            field.text = "four long words of mine"
            find(ways, "storeWayPassphraseKeep").clicked()
            compare(ways.said, "")
            compare(field.text, "", "never left in its field")
            verify(!find(ways, "storeWaysNone").visible)
        }

        function test_a_code_is_shown_once_and_a_way_taken_out() {
            find(ways, "storeWayCodeMake").clicked()
            compare(find(ways, "storeWayCode").text, "ABCD-EFGH-JKMN")
            verify(find(ways, "storeWayCode").visible)
            compare(mock.forgetWay("code"), "")
            ways.reread()
            verify(find(ways, "storeWaysNone").visible)
        }

        function test_no_security_key_known_is_said() {
            find(ways, "storeWayCardKeep").clicked()
            compare(ways.said, "store-way-card-none")
        }

        function test_which_passphrase_and_a_stale_security_key_are_said() {
            verify(!find(ways, "storeWayPassphraseKept").visible)
            verify(!find(ways, "storeWayCardStale").visible)
            mock.ways = ["passphrase", "card"]
            mock.passphrase = "The passphrase you kept on 27 November 2026."
            mock.stale = ["0006:00000007"]
            ways.reread()
            verify(find(ways, "storeWayPassphraseKept").visible)
            compare(find(ways, "storeWayPassphraseKept").text, "The passphrase you kept on 27 November 2026.")
            verify(find(ways, "storeWayCardStale").visible, "a card reset or renewed: its way said stale")
            compare(find(ways, "storeWayCardStale").text, "store-way-card-stale")
        }
    }
}
