// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Budgets page's From mail tab (MoneyMailTab.qml; docs/accounting.md,
// "From mail"), on a stand-in for Sioul: the page asked off the window's
// thread and taken when it comes, the rows by day, each saying what became
// of it; a mail not tied to any movement offers Add and Not a payment, one
// kept out by its shape says so once with Count it again, a joined one's
// menu offers Not the same; older mail still being read said in one calm
// line; a filter in use as words; at a phone's 412 points nothing passes
// the edge. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 760

    Theme {
        id: testTheme
    }

    // Made up: fiction names, addresses and amounts.
    readonly property var rows: [
        { key: "mid:a@pay.example.org", file: "/demo/Mail/cur/a", section: "Thursday 12 November", day: "2026-11-12", kind: "Received", who: "Client A", amount: "+€520", cents: 52000, fee: "fees €15.43", subject: "You received a payment", became: "part of the payout “PayPal” of Thu 12 Nov", outcome: "carried", movement: "sioul:movement/bank%3Amain%2FP1", shape: false, doubtful: false, domain: "pay.example.org", sender: "Pay" },
        { key: "mid:b@shop.example.org", file: "/demo/Mail/cur/b", section: "Tuesday 10 November", day: "2026-11-10", kind: "Order", who: "Shop", amount: "−€45", cents: -4500, fee: "", subject: "Your order 1234", became: "not tied to any movement", outcome: "untied", movement: "", shape: false, doubtful: false, domain: "shop.example.org", sender: "Shop" },
        { key: "mid:c@news.example.org", file: "/demo/Mail/cur/c", section: "Monday 9 November", day: "2026-11-09", kind: "Bill", who: "News", amount: "−€9", cents: -900, fee: "", subject: "Your weekly offers", became: "not counted: mail like this from News is no longer counted", outcome: "ignored", movement: "", shape: true, doubtful: false, domain: "news.example.org", sender: "News" }
    ]

    QtObject {
        id: mock

        signal moneyChanged

        property string moneyMail: ""
        property var asked: []
        property var notPayments: []
        property var counted: []
        property var apart: []
        property bool waiting: true

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function askMoneyMail(asked) {
            mock.asked = mock.asked.concat([JSON.parse(asked)])
            const chips = JSON.parse(asked).tied === false ? [{ id: "tied", words: "not tied to any movement" }] : []
            // Off the window's thread: it comes a moment later.
            Qt.callLater(() => mock.moneyMail = JSON.stringify({ asked: asked, rows: root.rows, page: 0, more: false, waiting: mock.waiting ? "Older mail is still being read for money; it comes as it is read." : "", chips: chips, filter: null, accounts: [], budgets: [], addresses: [] }))
        }
        function notAPayment(named) {
            mock.notPayments = mock.notPayments.concat([JSON.parse(named)])
            return JSON.stringify({ said: "Mail like this from Shop is no longer counted." })
        }
        function countAgain(named) {
            mock.counted = mock.counted.concat([JSON.parse(named)])
            return JSON.stringify({ said: "" })
        }
        function mailNotSame(named) {
            mock.apart = mock.apart.concat([JSON.parse(named)])
            return JSON.stringify({ said: "" })
        }
        function mailCandidates(day, cents) {
            return "[]"
        }
        function linkTarget(source, href) {
            return "null"
        }
    }

    QtObject {
        id: fakeWindow

        property bool compact: false
        property var menuAt: null
        property var opened: []
        property var payments: []

        function openThing(item) {
            fakeWindow.opened = fakeWindow.opened.concat([item])
        }
        function askPayment(kind, key, trust) {
            fakeWindow.payments = fakeWindow.payments.concat([key])
        }
    }

    Component {
        id: tabComponent

        MoneyMailTab {
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    TestCase {
        name: "MoneyMail"
        when: windowShown

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item))
                found.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, found)
            if (item.contentItem && item.contentItem !== item && (item.children || []).indexOf(item.contentItem) < 0)
                all(item.contentItem, test, found)
            return found
        }

        function named(item, name) {
            return all(item, i => i.objectName === name)
        }

        function visibleNamed(item, name) {
            return named(item, name).filter(i => i.visible)
        }

        function texts(item) {
            return all(item, i => i.text !== undefined && typeof i.text === "string" && i.visible).map(i => i.text)
        }

        function cleanup() {
            fakeWindow.compact = false
            fakeWindow.opened = []
            fakeWindow.payments = []
            mock.asked = []
            mock.notPayments = []
            mock.counted = []
            mock.apart = []
            mock.waiting = true
        }

        // The page comes off the window's thread; each row says what became of it.
        function test_each_mail_says_what_became_of_it() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "moneyMailRow").length === 3, 5000)
            const said = texts(tab)
            verify(said.indexOf("part of the payout “PayPal” of Thu 12 Nov") >= 0, said)
            verify(said.indexOf("+€520  ·  fees €15.43") >= 0, said)
            verify(said.indexOf("“Your order 1234”") >= 0, said)
            verify(visibleNamed(tab, "moneyMailWaiting").length === 1, "older mail still being read, said calmly")
            compare(mock.asked[0].page, 0)
            compare(mock.asked[0].tied, null, "tied or not is never the default")
        }

        // A mail not tied: Add and Not a payment; one kept out by its shape: Count it again.
        function test_the_rows_own_acts() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "moneyMailRow").length === 3, 5000)
            compare(visibleNamed(tab, "moneyMailNot").length, 1)
            compare(visibleNamed(tab, "moneyMailAdd").length, 1)
            compare(visibleNamed(tab, "moneyMailCountAgain").length, 1)
            visibleNamed(tab, "moneyMailNot")[0].clicked()
            compare(mock.notPayments.length, 1)
            compare(mock.notPayments[0].key, "mid:b@shop.example.org")
            compare(mock.notPayments[0].domain, "shop.example.org", "its shape learnt by its sender's domain")
            visibleNamed(tab, "moneyMailCountAgain")[0].clicked()
            compare(mock.counted[0].key, "mid:c@news.example.org")
            visibleNamed(tab, "moneyMailAdd")[0].clicked()
            compare(fakeWindow.payments, ["/demo/Mail/cur/b"], "A payment… from its file")
        }

        // A joined mail's menu: Not the same, its movement opened.
        function test_a_joined_mails_menu() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "moneyMailRow").length === 3, 5000)
            tab.menuOf(root.rows[0])
            const notSame = () => all(tab.Overlay.overlay, i => i.objectName === "moneyMailNotSame")[0]
            tryVerify(() => notSame() !== undefined && notSame().visible, 3000)
            verify(!all(tab.Overlay.overlay, i => i.objectName === "moneyMailSame")[0].visible, "no The same as… for a joined mail")
            notSame().triggered()
            compare(mock.apart[0].movement, "sioul:movement/bank%3Amain%2FP1")
        }

        // A row opens its message; a filter in use said as words.
        function test_a_row_opens_its_message_and_filters_say_themselves() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "moneyMailRow").length === 3, 5000)
            named(tab, "moneyMailRow").find(r => r.who === "Shop").clicked()
            compare(fakeWindow.opened.length, 1)
            compare(fakeWindow.opened[0].key, "/demo/Mail/cur/b")
            mock.waiting = false
            tab.ask({ tied: false })
            tryVerify(() => visibleNamed(tab, "moneyMailChip").length === 1, 3000)
            compare(visibleNamed(tab, "moneyMailChip")[0].text, "not tied to any movement  ×")
            verify(visibleNamed(tab, "moneyMailWaiting").length === 0)
        }

        // At a phone's 412 points: nothing past the edge.
        function test_a_phone_keeps_every_row_within_its_width() {
            fakeWindow.compact = true
            const tab = createTemporaryObject(tabComponent, root, { width: 412 - 32, height: 700 })
            tryVerify(() => named(tab, "moneyMailRow").length === 3, 5000)
            for (const row of named(tab, "moneyMailRow")) {
                const right = row.mapToItem(root, row.width, 0).x
                verify(right <= 412, "a row within the phone's width: " + right)
            }
        }
    }
}
