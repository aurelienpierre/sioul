// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What Android has not allowed Sioul yet on a phone, on the Porch
// (AccessCard.qml), on a stand-in for Sioul: each thing in a line with
// Allow in Android…, and Sioul's app info beside where Android may keep
// restricted settings from Sioul installed from a file; Not now puts the
// card away for those things for good; a thing Android allows leaves it;
// nothing to say, no card; Android's state read only while the Porch is in
// view. Which lines are said is tested in the app (access.rs). Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 420
    height: 900

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // What the card asked, in order: verb and what.
        property var asked: []
        property var flags: ({})
        // Made up: the owner's new phone of 16 October 2026, as access.rs would say it.
        property var lines: [
            { what: "texts", said: "Texts: Android has not allowed them yet. Press Allow in Android… once, then Sioul's app info.", info: true },
            { what: "notifications", said: "Other apps' notifications: Android has not given Sioul notification access yet.", info: false }
        ]

        signal porchChanged()

        function text(key) {
            return ({ "access-title": "Android has not allowed all that Sioul needs here", "access-not-now": "Not now", "appnotes-open-info": "Sioul's app info", "texts-setup-ask": "Allow in Android…" })[key] || key
        }
        function viewFlag(name) {
            return mock.flags[name] === true
        }
        function setViewFlag(name, value) {
            const flags = Object.assign({}, mock.flags)
            flags[name] = value
            mock.flags = flags
        }
        function texts(verb, json) {
            const args = JSON.parse(json)
            mock.asked = mock.asked.concat([verb + (args.what ? " " + args.what : "")])
            return JSON.stringify({ lines: mock.lines })
        }
    }

    AccessCard {
        id: card

        width: root.width - 32
        x: 16
        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "AccessCard"
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
        function seen(item) {
            for (let at = item; at && at !== card; at = at.parent) {
                if (!at.visible)
                    return false
            }
            return card.visible
        }
        function buttons(text) {
            return walk(card, i => i.text === text && i.clicked !== undefined && seen(i), [])
        }
        function shown(text) {
            return walk(card, i => i.text === text && i.clicked === undefined && seen(i), []).length > 0
        }

        function init() {
            mock.asked = []
            mock.flags = {}
            mock.lines = [
                { what: "texts", said: "Texts: Android has not allowed them yet. Press Allow in Android… once, then Sioul's app info.", info: true },
                { what: "notifications", said: "Other apps' notifications: Android has not given Sioul notification access yet.", info: false }
            ]
            card.active = true
            card.reload()
        }

        // Each line with what Android wants: app info beside where restricted settings may keep it.
        function test_each_thing_with_what_android_wants() {
            verify(card.visible)
            verify(shown("Android has not allowed all that Sioul needs here"))
            verify(shown(mock.lines[0].said) && shown(mock.lines[1].said))
            compare(buttons("Sioul's app info").length, 1, "only where Android may keep restricted settings")
            compare(buttons("Allow in Android…").length, 2)
            buttons("Sioul's app info")[0].clicked()
            buttons("Allow in Android…")[0].clicked()
            buttons("Allow in Android…")[1].clicked()
            compare(mock.asked.filter(a => a !== "access"), ["access-info", "access-ask texts", "access-ask notifications"])
        }

        // Not now: away for good, for those things; one Android allows leaves the card.
        function test_not_now_puts_it_away_for_good() {
            mock.lines = [mock.lines[1]]
            card.reload()
            compare(buttons("Allow in Android…").length, 1)
            buttons("Not now")[0].clicked()
            verify(!card.visible, "put away")
            mock.porchChanged()
            verify(!card.visible, "still away")
            // Another thing later: said.
            mock.lines = [{ what: "contacts", said: "Contacts: Android has not allowed Sioul to read them.", info: false }]
            mock.porchChanged()
            verify(card.visible && shown("Contacts: Android has not allowed Sioul to read them."))
            // Allowed: the card goes.
            mock.lines = []
            mock.porchChanged()
            verify(!card.visible)
        }

        // The Porch out of view: Android's state not read, then read as it comes back.
        function test_read_only_while_the_porch_is_in_view() {
            card.active = false
            mock.asked = []
            mock.porchChanged()
            compare(mock.asked.length, 0, "out of view: nothing asked")
            card.active = true
            compare(mock.asked, ["access"], "in view again: read")
        }
    }
}
