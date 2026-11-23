// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Supplies (docs/supplies.md): the page (SuppliesPage.qml) at a phone's
// width, 412 points, on a stand-in for Sioul: the calm view first, by
// category, each line its name and what is free, "low" as a quiet word; the
// full view one tab away, its stock and prices, the archived folded; nothing
// past the screen's edge; the page's sentence while nothing is kept. A
// supply's card (SupplyCard.qml): a new one made with its first count; on
// one kept, a field saved as it changes, "Received…" and "Count again…" in
// place, its planned tasks and history as links. Run by tools/qml-test.sh.

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

    // Made up: invented supplies, no real shop.
    QtObject {
        id: mock

        signal suppliesChanged
        signal tasksChanged
        property bool keeps: true
        property var calls: []
        // What the movement asked in place answers ("" for done).
        property string movedAnswer: ""

        function row(id, name, category, line, word) {
            return { id: id, name: name, category: category, area: "work", unit: "", buy: "1.39", sell: "", low_at: "10", bought_by: "50", project: "", project_title: "", archived: false, stock: 12, planned: 3, free: 9, stock_text: "12", planned_text: "3 planned", free_text: "9 left", word: word, line: line }
        }
        function suppliesPage() {
            if (!mock.keeps)
                return JSON.stringify({ store: true, problem: "", keeps: false, calm: [], all: [], archived: [], elsewhere: 0, categories: [], projects: [] })
            const stamps = mock.row("stamps", "Stamps with a rather long name that wraps on a phone", "Post", "9 left · 3 planned · low", "low")
            const envelopes = mock.row("envelopes", "Envelopes", "Post", "40 left", "")
            const food = Object.assign(mock.row("food", "Cat food", "Home", "2 left", ""), { area: "leisure" })
            const tape = Object.assign(mock.row("tape", "Tape", "Post", "1 left", ""), { archived: true })
            return JSON.stringify({ store: true, problem: "", keeps: true, calm: [{ category: "Post", supplies: [envelopes, stamps] }], all: [{ category: "Home", supplies: [food] }, { category: "Post", supplies: [envelopes, stamps] }], archived: [tape], elsewhere: 1, categories: ["Home", "Post"], projects: [{ id: "shop", title: "Gift cards" }] })
        }
        function supplyCard(id) {
            const card = mock.row(id, "Stamps", "Post", "9 left · 3 planned · low", "low")
            card.planned_tasks = [{ uid: "sale-1", title: "Gift card 50 €", quantity: "1", day: "" }]
            card.history = [{ id: "u1", kind: "used", when: "Tuesday 17 November", line: "Used: 1", after: "11", task: "sale-0", task_title: "Gift card 20 €" }, { id: "c1", kind: "count", when: "Monday 16 November", line: "Counted: 12", after: "12", task: "", task_title: "" }]
            card.resupplied = null
            card.resupply = null
            card.under_zero = false
            return JSON.stringify(card)
        }
        function saveSupply(id, change, first) {
            mock.calls.push({ what: "saveSupply", id: id, change: JSON.parse(change), first: first })
            return JSON.stringify({ id: id === "" ? "gift-cards" : id, error: "" })
        }
        function supplyMoved(id, kind, quantity) {
            mock.calls.push({ what: "supplyMoved", id: id, kind: kind, quantity: quantity })
            return mock.movedAnswer
        }
        function text(key) {
            return key === "qt-locale" ? "en_GB" : key
        }
        function textCounted(key, n) {
            return key + " " + n
        }
        function textWith(key, name, value) {
            return key + ": " + value
        }
        function textArgs(key, args) {
            return key + " " + args
        }
        function settings(view) {
            return "[]"
        }
    }

    // The window, as the page sees it.
    QtObject {
        id: shell

        property bool compact: true
        property var opened: []
        function openTask(uid) {
            shell.opened.push(uid)
        }
    }

    SuppliesPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
        window: shell
    }

    TestCase {
        name: "Supplies"
        when: windowShown

        // An item by its objectName under `item`, its views' content too.
        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = (item.children || []).concat(item.contentItem && item.contentItem !== item ? [item.contentItem] : [])
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found !== null)
                    return found
            }
            return null
        }

        // Every item shown, past the right edge of the screen: its name and its right edge.
        function past(item, found) {
            if (!item || !item.visible)
                return found
            const right = item.mapToItem(root, item.width, 0).x
            if (item.width > 0 && right > root.width + 0.5)
                found.push((item.objectName || item.text || item.toString()) + " ends at " + Math.round(right))
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                past(kids[i], found)
            if (item.contentItem && item.contentItem !== item)
                past(item.contentItem, found)
            return found
        }

        // Every label shown under `item`, in the order drawn, each item once.
        function labels(item, found, seen) {
            seen = seen || new Set()
            if (!item || !item.visible || seen.has(item))
                return found
            seen.add(item)
            if (item.text !== undefined && item.text !== "" && item instanceof Label)
                found.push(item.text)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                labels(kids[i], found, seen)
            if (item.contentItem && item.contentItem !== item)
                labels(item.contentItem, found, seen)
            return found
        }

        function init() {
            mock.keeps = true
            mock.calls = []
            mock.movedAnswer = ""
            page.tab = "now"
            page.archivedShown = false
            page.reload()
            waitForRendering(page)
        }

        // The calm view first: what now is for, by category, each line its name and what is free;
        // "low" a quiet word in the line's own muted colour, never a colour of its own.
        function test_the_calm_view_first() {
            compare(page.tab, "now")
            const shown = labels(page, [])
            verify(shown.indexOf("Post") >= 0 && shown.indexOf("Home") < 0, shown.join(" | "))
            verify(shown.indexOf("9 left · 3 planned · low") > shown.indexOf("Stamps with a rather long name that wraps on a phone"))
            verify(shown.indexOf("Cat food") < 0, "leisure's supply waits for its time")
            verify(shown.indexOf("supplies-elsewhere 1") >= 0, "those for other times said in one line")
            const line = find(page, "supplyLine:stamps")
            compare(line.color, testTheme.muted)
        }

        function test_the_page_fits_a_phone_s_width() {
            compare(past(page, []).join("; "), "", "nothing past 412 points")
            page.tab = "all"
            page.archivedShown = true
            waitForRendering(page)
            compare(past(page, []).join("; "), "", "the full view either")
        }

        // Every supply one tab away, whatever the hour, its stock and prices; the archived folded.
        function test_the_full_view_one_tab_away() {
            const tabs = find(page, "suppliesTabs")
            verify(tabs !== null && tabs.visible)
            tabs.chosen("all")
            waitForRendering(page)
            let shown = labels(page, [])
            verify(shown.indexOf("Cat food") >= 0 && shown.indexOf("Home") >= 0, shown.join(" | "))
            verify(shown.some(t => t.indexOf("supplies-in-stock: 12") >= 0 && t.indexOf("supplies-buy-at: 1.39") >= 0), shown.join(" | "))
            verify(shown.indexOf("Tape") < 0, "the archived folded")
            verify(find(page, "supplyArchived:tape") === null, "folded")
            page.archivedShown = true
            waitForRendering(page)
            const tape = find(page, "supplyArchived:tape")
            verify(tape !== null && tape.visible, "unfolded, its card one press away")
        }

        // Nothing kept yet: one sentence saying what the page is for, and New supply; no tabs.
        function test_an_empty_page_says_what_it_is_for() {
            mock.keeps = false
            page.reload()
            waitForRendering(page)
            const shown = labels(page, [])
            verify(shown.indexOf("supplies-help") >= 0, shown.join(" | "))
            verify(!find(page, "suppliesTabs").visible)
            verify(find(page, "suppliesNew").enabled)
        }

        // A new supply: its fields, its first count, made at Save, its card then shown.
        function test_a_new_supply_with_its_first_count() {
            page.startNew()
            const card = page.card.item
            tryVerify(() => card.opened)
            verify(card.fresh)
            find(card.contentItem, "supplyName").text = "Gift cards"
            find(card.contentItem, "supplyCategory").text = "Shop"
            find(card.contentItem, "supplyLowAt").text = "5"
            find(card.contentItem, "supplyFirstCount").text = "12"
            card.make()
            const saved = mock.calls.filter(c => c.what === "saveSupply")
            compare(saved.length, 1, JSON.stringify(mock.calls))
            compare([saved[0].id, saved[0].change.name, saved[0].change.category, saved[0].change.low_at, saved[0].first].join("|"), "|Gift cards|Shop|5|12")
            compare(card.supplyId, "gift-cards", "its card shown once made")
            verify(!card.fresh)
            card.close()
        }

        // A supply's card: a field saved as it changes; Received… asked in place; its planned
        // tasks and history open their task.
        function test_a_supply_s_card() {
            page.open("stamps")
            const card = page.card.item
            tryVerify(() => card.opened)
            compare(find(card.contentItem, "supplyStock").text, "12")
            const low = find(card.contentItem, "supplyLowAt")
            low.text = "8"
            low.editingFinished()
            const saved = mock.calls.filter(c => c.what === "saveSupply")
            compare(saved.length, 1, JSON.stringify(mock.calls))
            compare(JSON.stringify(saved[0].change), JSON.stringify({ low_at: "8" }), "the field alone")
            mouseClick(find(card.contentItem, "supplyReceived"))
            compare(card.asking, "received")
            const field = find(card.contentItem, "supplyMovedField")
            verify(field.visible)
            field.text = "50"
            mouseClick(find(card.contentItem, "supplyMovedOk"))
            const moved = mock.calls.filter(c => c.what === "supplyMoved")
            compare(moved.length, 1)
            compare([moved[0].id, moved[0].kind, moved[0].quantity].join("|"), "stamps|received|50")
            compare(card.asking, "", "done: the field goes")
            // Count again, refused: said, the field kept.
            mock.movedAnswer = "supplies-number-unreadable"
            card.ask("count")
            field.text = "many"
            card.moved()
            compare(card.problem, "supplies-number-unreadable")
            compare(card.asking, "count")
            card.close()
        }
    }
}
