// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Budgets page's Movements tab (MovementsTab.qml, MovementActions.qml,
// MovementDialog.qml; docs/accounting.md, "Movements tab"), on a stand-in
// for Sioul: the rows by day, fees inside their row, no sum until the
// filters ask a question that has one; the filters in use as words, each
// ×'d; a row opened in its pane with Back; its menu by what it is made from
// (Change…, Tie to…, Leave out at once, Delete asked once); the form
// changing a bank line shows neither its amount nor its day and sends only
// what changed; the declaration's sentences and register; the page's four
// tabs, Overview first; at a phone's 412 points nothing passes the edge, nor
// past 360 a movement's pane.
// Run by tools/qml-test.sh.

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

    // Made up: fiction names and amounts.
    readonly property var rows: [
        { address: "sioul:movement/bank%3Amain%2FP1", key: "bank:main/P1", section: "Thursday 12 November", who: "PayPal → My bank", amount: "+€1,184.10", cents: 118410, way: "in", budget: "Work", standing: "has come in", second: "paid €1,220 · fees €35.90 · carries 3 payments", note: "" },
        { address: "sioul:movement/line%3Ah1", key: "line:h1", section: "Tuesday 10 November", who: "Bakery", amount: "−€12.50", cents: -1250, way: "out", budget: "Home", standing: "has left the account", second: "", note: "Bread for Sunday" }
    ]
    readonly property var bankRow: ({ address: "sioul:movement/bank%3Amain%2FX1", key: "bank:main/X1", gone: false, who: "Shop", amount: "−€45", cents: -4500, budget_id: "home", lines: ["Tue 10 Nov · Shop · −€45 · Home · has left the account", "Told on 10 November."], sources: [{ kind: "bank", words: "the bank's line", address: "", probable: false, key: "bank:main/X1" }], probable: false, carried: [], carried_by: "", allowed: { made: "bank", fields: ["label", "gross", "fees", "paid_on", "note", "budget"], removal: "leave-out" }, says: "Its amount and its day are the bank's.", data: { who: "Shop", amount: -45, gross: null, fees: [], day: "2026-11-10", paid_on: null, note: "", account: "main", state: "done", budgets: [{ id: "home" }] } })
    readonly property var lineRow: ({ address: "sioul:movement/line%3Ah1", key: "line:h1", gone: false, who: "Bakery", amount: "−€12.50", cents: -1250, budget_id: "home", lines: ["Tue 10 Nov · Bakery · −€12.50 · Home"], sources: [], probable: false, carried: [], carried_by: "", allowed: { made: "line", fields: ["label", "gross", "fees", "net", "day", "paid_on", "note", "budget", "planned", "account"], removal: "delete" }, says: "", data: { who: "Bakery", amount: -12.5, gross: null, fees: [], day: "2026-11-10", paid_on: null, note: "Bread for Sunday", account: null, state: "done", budgets: [{ id: "home" }] } })

    QtObject {
        id: mock

        signal moneyChanged

        property var asked: []
        property var changes: []
        property var removed: []
        property string budgets: JSON.stringify({ title: "Budgets, November 2026", accounts: [], charts: [], left_out: [], budgets: [], groups: [], coming: [], reserves: [], mail: [], choices: [{ id: "home", title: "Home" }], incomes: [] })
        property string moneyMail: ""

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textArgs(key, args) {
            return key
        }
        function movements(asked) {
            const q = JSON.parse(asked)
            mock.asked = mock.asked.concat([q])
            const chips = q.way === "in" ? [{ id: "way", words: "Money in" }] : []
            return JSON.stringify({ rows: root.rows, page: 0, more: false, total: q.way === "in" && q.from !== "" ? "Money in: €1,220 paid, €1,184.10 arrived." : "", chips: chips, filter: null, accounts: [{ id: "main", title: "My bank" }], budgets: [{ id: "home", title: "Home" }, { id: "work", title: "Work" }], declares: [{ id: "work", title: "Work" }] })
        }
        function movementDetail(address) {
            return JSON.stringify(address === root.lineRow.address ? root.lineRow : root.bankRow)
        }
        function changeMovement(address, edit) {
            mock.changes = mock.changes.concat([{ address: address, edit: JSON.parse(edit) }])
            return JSON.stringify({ said: "Work holds €45 more for November 2026." })
        }
        function removeMovement(address) {
            mock.removed = mock.removed.concat([address])
            return JSON.stringify({ said: "Home holds €45 more for November 2026." })
        }
        function makeMovement(edit) {
            return JSON.stringify({ said: "" })
        }
        function putBackMovement(address) {
            return JSON.stringify({ said: "" })
        }
        function declaration(budget, anchor) {
            return JSON.stringify({ budget: "work", title: "Work", anchor: "2026-11-05", previous: "2026-09-30", next: "", sentences: ["Turnover received from 1 October 2026 to 31 December 2026: €6,940, paid by clients.", "To declare by 31 January 2027."], rows: [{ day: "Fri 2 Oct", client: "Client A", invoice: "2026-014", means: "PayPal", gross: "€520", known: true, address: "sioul:movement/line%3Aw1" }], declares: [{ id: "work", title: "Work" }] })
        }
        function linkTarget(source, href) {
            return "null"
        }
        function searchThings(query, kind, from) {
            return "[]"
        }
        function sameCandidates(key) {
            return "[]"
        }
        function bank() {
            return JSON.stringify({ store: true, any: false, balance: "", findings: [], coming: [], problem: "", accounts: [], budgets: [], reserves: [] })
        }
        function contracts() {
            return JSON.stringify({ store: true, problem: "", kinds: [], contracts: [], suggestions: [], presets: [] })
        }
        function tickBudgetAreas(areas) {
        }
        function budgetAreasAsHours() {
        }
    }

    QtObject {
        id: fakeWindow

        property bool compact: false
        property var menuAt: null

        function openThing(item) {
        }
    }

    Component {
        id: tabComponent

        MovementsTab {
            sioul: mock
            theme: testTheme
            window: fakeWindow
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

    TestCase {
        name: "Movements"
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

        function overlayTexts(item) {
            return all(item.Overlay.overlay, i => i.text !== undefined && typeof i.text === "string" && i.visible && i.height > 0).map(i => i.text)
        }

        function cleanup() {
            fakeWindow.compact = false
            mock.asked = []
            mock.changes = []
            mock.removed = []
        }

        // The rows by day, newest first, fees inside their row; no sum until
        // a question has one; a filter in use said in words, its × forgets it.
        function test_the_ledger_by_day_with_its_filters_as_words() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "movementRow").length === 2, 5000)
            const said = texts(tab)
            verify(said.indexOf("Thursday 12 November") >= 0 && said.indexOf("Tuesday 10 November") >= 0, said)
            verify(said.indexOf("paid €1,220 · fees €35.90 · carries 3 payments") >= 0, said)
            verify(said.indexOf("+€1,184.10") >= 0, said)
            verify(!named(tab, "movementsTotal")[0].visible, "no running total")
            tab.ask({ way: "in", from: "2026-10-01" })
            tryVerify(() => named(tab, "movementsTotal")[0].visible, 2000)
            compare(named(tab, "movementsTotal")[0].text, "Money in: €1,220 paid, €1,184.10 arrived.")
            const chip = named(tab, "movementsChip")[0]
            compare(chip.text, "Money in  ×")
            chip.clicked()
            compare(mock.asked[mock.asked.length - 1].way, "", "the × forgets its filter")
            verify(named(tab, "movementsDeclaration")[0].visible, "a work budget declares")
        }

        // A row opens its pane, every field said; Back gives the list again.
        function test_a_row_opens_in_its_pane() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "movementRow").length === 2, 5000)
            named(tab, "movementRow").find(r => r.who === "Bakery").clicked()
            tryVerify(() => named(tab, "movementDetail")[0].visible, 2000)
            verify(texts(tab).indexOf("Tue 10 Nov · Bakery · −€12.50 · Home") >= 0, texts(tab))
            named(tab, "movementBack")[0].clicked()
            verify(!named(tab, "movementDetail")[0].visible && named(tab, "movementsList")[0].visible)
        }

        // Its menu by what it is made from: a bank line Changes, Ties, and is
        // Left out at once; your own line is Deleted, asked once.
        function test_the_menu_by_what_it_is_made_from() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "movementRow").length === 2, 5000)
            tab.menuOf(root.bankRow.address)
            tryVerify(() => all(tab.Overlay.overlay, i => i.objectName === "movementRemove" && i.visible).length === 1, 3000)
            const items = overlayTexts(tab)
            verify(items.indexOf("movement-change") >= 0 && items.indexOf("movement-tie") >= 0 && items.indexOf("movement-removal-leave-out") >= 0, items)
            all(tab.Overlay.overlay, i => i.objectName === "movementRemove")[0].triggered()
            compare(mock.removed, [root.bankRow.address], "Leave out at once, with Undo after")
            tab.menuOf(root.lineRow.address)
            tryVerify(() => overlayTexts(tab).indexOf("movement-removal-delete") >= 0, 3000)
            all(tab.Overlay.overlay, i => i.objectName === "movementRemove" && i.text === "movement-removal-delete")[0].triggered()
            tryVerify(() => overlayTexts(tab).indexOf("movement-delete-ask") >= 0, 3000, "asked once")
            compare(mock.removed.length, 1, "nothing deleted before the answer")
        }

        // Changing a bank line: neither its amount nor its day in the form,
        // what its kind keeps said once; only what changed is sent.
        function test_a_bank_line_changes_in_what_it_allows() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "movementRow").length === 2, 5000)
            tab.openMovement(root.bankRow.address)
            actionsOf(tab).changeNow(root.bankRow)
            const amount = () => all(tab.Overlay.overlay, i => i.objectName === "movementAmount")[0]
            tryVerify(() => amount() !== undefined, 3000)
            verify(!amount().visible, "the bank's amount is not offered")
            verify(all(tab.Overlay.overlay, i => i.objectName === "movementFormSays" && i.visible).length === 1)
            const dialog = actionsOf(tab).changing
            dialog.more = true
            const gross = all(tab.Overlay.overlay, i => i.objectName === "movementGross")[0]
            gross.text = "47"
            dialog.save()
            compare(mock.changes.length, 1)
            compare(JSON.stringify(Object.keys(mock.changes[0].edit)), JSON.stringify(["gross"]), JSON.stringify(mock.changes[0].edit))
            compare(mock.changes[0].edit.gross, -47, "what was paid goes the way the amount goes")
        }

        // For the declaration: its sentences, its register, Save.
        function test_the_declaration_and_its_register() {
            const tab = createTemporaryObject(tabComponent, root, { width: 900, height: 700 })
            tryVerify(() => named(tab, "movementRow").length === 2, 5000)
            named(tab, "movementsDeclaration")[0].clicked()
            tryVerify(() => named(tab, "declarationView")[0].visible, 2000)
            compare(named(tab, "declarationSentence").length, 2)
            compare(named(tab, "receiptRow").length, 1)
            verify(named(tab, "receiptsSave")[0].visible)
        }

        // The page: four tabs, Overview first, and Overview again when it comes back.
        function test_the_page_opens_on_overview() {
            const page = createTemporaryObject(pageComponent, root)
            tryVerify(() => named(page, "moneyTab3").length === 1, 5000)
            compare(page.tab, 0)
            named(page, "moneyTab2")[0].clicked()
            compare(page.tab, 2)
            tryVerify(() => named(page, "movementRow").length === 2, 5000, "made at first show")
            page.visible = false
            compare(page.tab, 0, "hidden, it opens on Overview next time")
            page.visible = true
            page.openMovement(root.bankRow.address)
            compare(page.tab, 2, "a link opens the tab it names")
        }

        // At a phone's 412 points: nothing past the edge.
        function test_a_phone_keeps_every_row_within_its_width() {
            fakeWindow.compact = true
            const tab = createTemporaryObject(tabComponent, root, { width: 412 - 32, height: 700 })
            tryVerify(() => named(tab, "movementRow").length === 2, 5000)
            for (const row of named(tab, "movementRow")) {
                const right = row.mapToItem(root, row.width, 0).x
                verify(right <= 412, "a row within the phone's width: " + right)
            }
        }

        // A bank line's pane at 360 points (10 November 2026: its buttons were past the
        // edge, its words cut with them): Back, Change…, Ties and ⋯ on one line within it.
        function test_a_movement_s_pane_fits_a_small_phone() {
            fakeWindow.compact = true
            const tab = createTemporaryObject(tabComponent, root, { width: 360 - 16, height: 700 })
            tryVerify(() => named(tab, "movementRow").length === 2, 5000)
            tab.openMovement(root.bankRow.address)
            tryVerify(() => named(tab, "movementDetail")[0].visible, 2000)
            waitForRendering(tab)
            const row = named(tab, "movementBack")[0].parent
            verify(row.implicitWidth <= 360 - 16, "its buttons' row: " + row.implicitWidth)
            const buttons = Array.from(row.children).filter(c => c.visible && c.text !== undefined)
            compare(buttons.length, 4, "Back, Change…, Ties and ⋯")
            for (const button of buttons)
                verify(button.mapToItem(tab, button.width, 0).x <= 360 - 16 + 0.5, button.text + " within the screen: " + button.mapToItem(tab, button.width, 0).x)
        }

        function actionsOf(tab) {
            return all(tab, i => i.changeNow !== undefined)[0]
        }
    }
}
