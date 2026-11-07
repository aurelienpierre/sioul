// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The budgets page from one income to the next (BudgetsPage.qml,
// ComingSection.qml; docs/accounting.md, "The budgets page"), on a stand-in
// for Sioul: each account's sentences at the top, the area filter mounted
// as Mail's (its ticks said to Rust), What is coming by week with money in
// as plain as money out, the month beside the list on a computer only, a
// row's menu; at a phone's 412 points nothing passes the window's edge; a
// phone's page holds no more than 400 objects as it opens, as Mail's.
// Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1100
    height: 800

    Theme {
        id: testTheme
    }

    // Made up: fiction names and amounts.
    readonly property var row: ({ address: "sioul:movement/preset%3Aphone%2F2026-11-12", key: "preset:phone/2026-11-12", account: "main", day: "2026-11-12", date: "Thu 12 Nov", who: "Phone", amount: "€19.99", cents: -1999, way: "out", way_words: "Money out", budget: "Home", budget_id: "home", state: "expected on Thursday 12 November", sources: [{ kind: "preset", words: "the recurring payment", address: "", probable: false, key: "preset:phone/2026-11-12" }, { kind: "mail", words: "a mail", address: "mid:bill@example.org", probable: true, key: "mid:bill@example.org" }], from: "From the recurring payment and a mail.", probable: true })
    readonly property var pay: ({ address: "sioul:movement/preset%3Apay%2F2026-11-28", key: "preset:pay/2026-11-28", account: "main", day: "2026-11-28", date: "Sat 28 Nov", who: "Salary", amount: "€2,000", cents: 200000, way: "in", way_words: "Money in", budget: "Home", budget_id: "home", state: "expected on Saturday 28 November", sources: [{ kind: "preset", words: "the recurring payment", address: "", probable: false, key: "preset:pay/2026-11-28" }], from: "From the recurring payment.", probable: false })
    readonly property string view: JSON.stringify({
        title: "Budgets, November 2026",
        accounts: [{ id: "main", title: "Main account", sentences: ["Until Salary comes in on Saturday 28 November, the account holds every payment.", "€460 is left for everyday spending, about €25 a day."], short: false }],
        chart: null,
        left_out: ["Work budgets are kept for your work hours."],
        filter: { work: false, admin: true, leisure: false, label: "Admin", own: false },
        budgets: [],
        coming_title: "What is coming",
        coming: [{ title: "This week", rows: [root.row] }, { title: "Week of Monday 23 November", rows: [root.pay] }],
        unusual: "Unusual spending, about €60 a month over the past year, is counted in the forecast.",
        reserves_title: "Reserves",
        reserves: [],
        mail_title: "Mail about money",
        mail: [],
        choices: [{ id: "home", title: "Home" }, { id: "fun", title: "Leisure" }],
        incomes: []
    })

    QtObject {
        id: mock

        property string budgets: root.view
        property var ticked: []
        property var bankAccounts: []

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textArgs(key, args) {
            return key
        }
        function tickBudgetAreas(areas) {
            mock.ticked = mock.ticked.concat([areas])
        }
        function budgetAreasAsHours() {
        }
        function bank() {
            return JSON.stringify({ store: true, any: false, balance: "", findings: [], coming: [], problem: "", accounts: mock.bankAccounts, budgets: [], reserves: [] })
        }
        function contracts() {
            return JSON.stringify({ store: true, problem: "", kinds: [], contracts: [], suggestions: [], presets: [] })
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

        function openThing(item) {
            fakeWindow.opened = fakeWindow.opened.concat([item])
        }
    }

    Component {
        id: pageComponent

        BudgetsPage {
            anchors.fill: parent
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    Component {
        id: comingComponent

        ComingSection {
            sioul: mock
            theme: testTheme
            window: fakeWindow
            view: JSON.parse(root.view)
        }
    }

    TestCase {
        name: "Coming"
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

        function texts(item) {
            return all(item, i => i.text !== undefined && typeof i.text === "string" && i.visible).map(i => i.text)
        }

        function cleanup() {
            fakeWindow.compact = false
            mock.ticked = []
            mock.bankAccounts = []
            mock.budgets = root.view
        }

        function count(item) {
            let n = 1
            for (let i = 0; i < item.children.length; ++i)
                n += count(item.children[i])
            return n
        }

        // A view fuller than the demo profile's: three accounts, six budgets, twelve rows coming.
        function fullView() {
            const view = JSON.parse(root.view)
            view.accounts = [1, 2, 3].map(n => ({ id: "account" + n, title: "Account " + n, sentences: ["Until Salary comes in, the account holds every payment.", "€460 is left for everyday spending.", "About €25 a day."], fresh: ["Its bank's file was read on Monday."] }))
            view.budgets = [1, 2, 3, 4, 5, 6].map(n => ({ id: "budget" + n, title: "Budget " + n, period: "November", mood: "as-planned", verdict: "It should end as planned.", words: ["€120 is paid.", "€40 is still to come.", "€60 is left."] }))
            view.coming = [1, 2, 3, 4].map(week => ({ title: "Week " + week, rows: [1, 2, 3].map(n => Object.assign({}, root.row, { key: "row:" + week + "/" + n })) }))
            return JSON.stringify(view)
        }

        // On a phone the Budgets tab is made when it first shows, or when
        // something on it is asked for; what the page holds until then stays
        // within Mail's budget of 400 objects. On a computer it comes just after the page.
        function test_a_phone_s_page_keeps_to_its_budget_of_objects() {
            mock.budgets = fullView()
            fakeWindow.compact = true
            const page = createTemporaryObject(pageComponent, root, { width: 412, height: 891 })
            tryVerify(() => named(page, "aheadAccount").length === 3, 5000)
            wait(200)
            compare(page.bank, null, "the Budgets tab waits")
            const opening = count(page)
            verify(opening <= 400, "the whole page: " + opening)
            named(page, "moneyTab1")[0].clicked()
            tryVerify(() => page.bank !== null, 5000, "made at first show")
            verify(page.bank.visible)
            console.warn("sioul-objects: a phone's budgets page " + opening + ", the Budgets tab shown too " + count(page))
            // Asked for from elsewhere, before it showed: there at once.
            const asked = createTemporaryObject(pageComponent, root, { width: 412, height: 891 })
            tryVerify(() => named(asked, "aheadAccount").length === 3, 5000)
            compare(asked.bank, null)
            verify(asked.contractsNow() !== null)
            verify(asked.bank !== null)
            fakeWindow.compact = false
            const computer = createTemporaryObject(pageComponent, root)
            tryVerify(() => computer.bank !== null, 5000, "just after the page")
            compare(computer.tab, 0)
        }

        // A new movement names its bank account though the Budgets tab was never made.
        function test_a_new_movement_has_its_accounts_on_a_phone() {
            fakeWindow.compact = true
            mock.bankAccounts = [{ id: "main", title: "Main account" }]
            const page = createTemporaryObject(pageComponent, root, { width: 412, height: 891 })
            tryVerify(() => named(page, "aheadAccount").length === 1, 5000)
            page.startNew()
            compare(page.bank, null)
            compare(page.newAccounts, [{ id: "main", title: "Main account" }])
        }

        // The page opens with each account's sentences, what the hour leaves
        // out, and Mail's filter, its ticks said to Rust.
        function test_the_page_opens_with_each_account() {
            const page = createTemporaryObject(pageComponent, root)
            tryVerify(() => named(page, "aheadAccount").length === 1, 5000)
            const said = texts(page)
            verify(said.indexOf("Until Salary comes in on Saturday 28 November, the account holds every payment.") >= 0, said)
            verify(said.indexOf("Work budgets are kept for your work hours.") >= 0)
            const filter = named(page, "budgetAreas")[0]
            verify(filter && filter.visible, "the area filter, as Mail's")
            filter.ticked("admin+leisure")
            compare(mock.ticked, ["admin+leisure"])
        }

        // What is coming: its weeks in order, money in as plain as money out,
        // its sign alone telling; the month beside the list on demand.
        function test_what_is_coming_by_week() {
            const coming = createTemporaryObject(comingComponent, root, { width: 1000 })
            tryVerify(() => named(coming, "comingRow").length === 2, 5000)
            const said = texts(coming)
            verify(said.indexOf("This week") >= 0 && said.indexOf("Week of Monday 23 November") >= 0, said)
            verify(said.indexOf("+€2,000") >= 0 && said.indexOf("€19.99") >= 0, said)
            verify(said.indexOf("From the recurring payment and a mail.  coming-probably") >= 0, "what tells of it, together")
            verify(said.indexOf("Unusual spending, about €60 a month over the past year, is counted in the forecast.") >= 0)
            const grid = named(coming, "comingGrid")[0]
            verify(!grid.visible)
            named(coming, "comingMonth")[0].clicked()
            tryVerify(() => grid.visible, 2000, "the month beside the list")
            verify(named(coming, "comingRow")[0].visible, "never instead of it")
        }

        // A row's menu: where it goes, split, the same as, "Not the same"
        // beside the source joined as probably the same, the mail opened.
        function test_a_rows_menu() {
            const coming = createTemporaryObject(comingComponent, root, { width: 1000 })
            tryVerify(() => named(coming, "comingRow").length === 2, 5000)
            coming.menuOf(root.row)
            const menu = () => all(coming.Overlay.overlay, i => i.objectName === "movementSplit")[0]
            tryVerify(() => menu() !== undefined && menu().visible, 3000, "the menu open")
            const items = all(coming.Overlay.overlay, i => i.text !== undefined && typeof i.text === "string" && i.visible).map(i => i.text)
            verify(items.indexOf("movement-not-same · a mail") >= 0, items)
            verify(items.indexOf("ui-open · a mail") >= 0, items)
        }

        // On a phone: the same list, no month beside it, nothing past 412 points.
        function test_a_phone_keeps_the_list() {
            fakeWindow.compact = true
            const coming = createTemporaryObject(comingComponent, root, { width: 412 - 32 })
            tryVerify(() => named(coming, "comingRow").length === 2, 5000)
            verify(!named(coming, "comingMonth")[0].visible, "no month on a phone")
            for (const row of named(coming, "comingRow")) {
                const right = row.mapToItem(root, row.width, 0).x
                verify(right <= 412, "a row within the phone's width: " + right)
            }
            const page = createTemporaryObject(pageComponent, root)
            tryVerify(() => named(page, "budgetAreasPhone").length === 1, 5000)
            verify(named(page, "budgetAreasPhone")[0].visible && named(page, "budgetAreas").length === 0, "the filter under the title")
        }
    }
}
