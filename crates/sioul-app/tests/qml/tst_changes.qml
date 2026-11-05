// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Every view that reads its kind itself reads it again when that kind
// changed elsewhere (docs/client.md, "Every view shows a change at once"):
// the contracts' and the bank's sections on money, the Papers page on
// papers and persons, the Time page on projects; each while it shows, and
// never while it is hidden. A stand-in for Sioul says the change, as the
// window's listener does; fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
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

        signal moneyChanged
        signal papersChanged
        signal personsChanged
        signal projectsChanged
        signal linksChanged
        signal sharedIn(string stores)

        // How often each view was read.
        property var reads: ({})

        function read(what) {
            const next = Object.assign({}, mock.reads)
            next[what] = (next[what] || 0) + 1
            mock.reads = next
        }
        function count(what) {
            return mock.reads[what] || 0
        }

        function text(key) { return key }
        function textWith(key, name, value) { return key + ": " + value }
        function textArgs(key, args) { return key }
        function textCounted(key, n) { return key }
        function zoneOffset(seconds) { return 0 }
        function contracts() {
            mock.read("contracts")
            return JSON.stringify({ store: true, problem: "", contracts: [], kinds: [], presets: [], suggestions: [] })
        }
        function bank() {
            mock.read("bank")
            return JSON.stringify({ store: true, problem: "", accounts: [], balance: "", budgets: [], findings: [], reserves: [] })
        }
        function papers() {
            mock.read("papers")
            return JSON.stringify({ store: true, problem: "", kinds: [], families: [], choices: [], chips: [] })
        }
        function projectRows() {
            mock.read("projects")
            return "[]"
        }
        function timePage(period, anchor, project) {
            return JSON.stringify({ title: "", sentence: "", total: "", most: "", bars: [], entries: [], projects: [] })
        }
        function viewFlag(key) { return false }
    }

    QtObject {
        id: shell

        property var moment: ({ quiet: false, line: "" })
        property string today: "2099-01-05"
        property string sioulLocale: "en_GB"
        function openCard(ref) {}
        function openTask(uid) {}
        function openDraft(id) {}
        function openProject(id) {}
    }

    Component {
        id: contractsComponent

        ContractsSection {
            width: root.width
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    Component {
        id: bankComponent

        BankSection {
            width: root.width
            sioul: mock
            theme: testTheme
        }
    }

    Component {
        id: papersComponent

        PapersPage {
            width: root.width
            height: root.height
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    Component {
        id: timeComponent

        TimePage {
            width: root.width
            height: root.height
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    TestCase {
        id: test

        name: "Changes"
        when: windowShown

        function init() {
            mock.reads = {}
        }

        // Said once, a view reads again while it shows; hidden, it waits for
        // its return, which reads it (its own onVisibleChanged).
        function heard(component, what, say) {
            const view = test.createTemporaryObject(component, root)
            test.waitForRendering(view)
            const first = mock.count(what)
            verify(first >= 1, what + " read as it opens")
            say()
            compare(mock.count(what), first + 1, what + ": read again at once")
            view.visible = false
            say()
            compare(mock.count(what), first + 1, what + ": nothing read while hidden")
            view.visible = true
            compare(mock.count(what), first + 2, what + ": read on its return")
            view.destroy()
            wait(0)
        }

        function test_money_reaches_the_contracts_and_the_bank() {
            heard(contractsComponent, "contracts", () => mock.moneyChanged())
            heard(bankComponent, "bank", () => mock.moneyChanged())
        }

        function test_papers_and_persons_reach_the_papers_page() {
            heard(papersComponent, "papers", () => mock.papersChanged())
            heard(papersComponent, "papers", () => mock.personsChanged())
        }

        function test_projects_reach_the_time_page() {
            heard(timeComponent, "projects", () => mock.projectsChanged())
        }
    }
}
