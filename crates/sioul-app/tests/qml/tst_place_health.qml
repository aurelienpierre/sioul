// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Health page's day keeps its place (HealthPage.qml, ListPlace.qml;
// docs/qt-quick.md, "A list keeps its place"), on a stand-in for Sioul that
// hands the whole view again after each change, as Rust does: a long day of
// meals and doses scrolled to its middle; a meal's "15 min later" pressed on
// its row moves it under the next; a dose taken on the phone above the view
// leaves the list, a meal added above comes in: the row at the top stays at
// its pixel. Another day starts at its top. Made-up days far ahead. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 520
    height: 900

    Theme {
        id: testTheme
    }

    // The night going on (begun the evening before, its own day the 1st), lunch and this evening's night.
    function night(date, note) {
        return { key: "sleep", date: date, kind: "sleep", name: "Night", dose: "", from: "23:00", to: "07:30", at: "00:00", detail: "winding down, bed at 00:00", note: note, changed: false, added: false, quiet: false, off: false, past: false, minutes: 0, before: 60, after: 0, taken: "", late: false, doubt: "", doubt_off: [], choose: false, alarm: "07:30", alarm_skipped: false, alarm_open: true, waking: "07:30", rings: true }
    }

    // The day's items: meals and doses every twenty minutes, from 06:00.
    property var items: []
    property string date: "2096-10-02"

    function item(n, kind) {
        return { key: kind + ":" + n, date: root.date, kind: kind, name: (kind === "dose" ? "Dose " : "Meal ") + n + (n % 3 === 0 ? ", with words long enough to take two lines on a phone's narrow screen" : ""),
                 dose: "", from: "", to: "", at: "", detail: "", note: "", changed: false, added: false, quiet: false, off: false, past: false, minutes: 360 + n * 20, before: 20, after: 0,
                 taken: "", late: false, doubt: "", doubt_off: [], choose: false, alarm: "", alarm_skipped: false, alarm_open: false, waking: "", rings: false }
    }

    function tell() {
        const time = m => { const h = Math.floor(m / 60) % 24; const mm = m % 60; return (h < 10 ? "0" : "") + h + ":" + (mm < 10 ? "0" : "") + mm }
        const shown = root.items.slice().sort((a, b) => a.minutes - b.minutes).map(i => Object.assign({}, i, { from: time(i.minutes), to: time(i.minutes + 30), at: time(i.minutes) }))
        const day = { date: root.date, title: "Tuesday " + root.date, weekday: "Tue", number: 2, today: true, past: false, items: shown, lines: [] }
        mock.healthView = JSON.stringify({ any: true, unreadable: "", week: { monday: "2096-10-01", today: root.date, days: [day] }, missed: [], shared_note: "", reminded_there: "", later: 15, medicines: [], prescriptions: [] })
    }

    QtObject {
        id: mock

        property var calls: []
        property string status: ""
        property string healthView: ""
        signal sharedIn(var stores)
        signal modeChanged

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function showHealthWeek(day) {}
        function dayReviewLine(date) { return "null" }
        // "15 min later": that item a quarter of an hour later, the day sorted and given again whole.
        function changeNeed(edit) {
            const asked = JSON.parse(edit)
            mock.calls.push(asked)
            if (asked.action === "later")
                root.items = root.items.map(i => i.key === asked.key ? Object.assign({}, i, { minutes: i.minutes + 15 }) : i)
            root.tell()
            return ""
        }
    }

    QtObject {
        id: shell

        property string today: "2096-10-02"
        property point menuAt: Qt.point(10, 10)
        property var opened: []
        property string page: "health"

        function openCard(ref) {
            shell.opened.push(ref)
        }
        function askDose(key) {}
        function showAgendaWeek(date) {}
    }

    Component {
        id: pageComponent

        HealthPage {
            width: root.width
            height: root.height
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    // Every object under `item` that `test` takes, once each, its popups' and loaders' own searched.
    function found(item, test) {
        const out = root.all(item, test, [])
        return out.filter((c, i) => out.indexOf(c) === i)
    }

    function all(item, test, out) {
        out = out || []
        if (!item)
            return out
        const kids = (item.children || []).concat(item.data || [])
        const seen = []
        for (let i = 0; i < kids.length; ++i) {
            const kid = kids[i]
            if (!kid || seen.indexOf(kid) >= 0)
                continue
            seen.push(kid)
            if (test(kid))
                out.push(kid)
            if (kid.children !== undefined || kid.data !== undefined)
                root.all(kid, test, out)
            if (kid.contentItem !== undefined && kid.contentItem !== null && kid.contentItem !== kid && seen.indexOf(kid.contentItem) < 0)
                root.all(kid.contentItem, test, out)
            if (kid.item !== undefined && kid.item !== null && typeof kid.item === "object" && kid.status !== undefined) {
                if (test(kid.item))
                    out.push(kid.item)
                root.all(kid.item, test, out)
            }
        }
        return out
    }

    TestCase {
        name: "PlaceHealth"
        when: windowShown

        property var page: null
        property var flick: null

        function frame() {
            root.Window.window.update()
            waitForRendering(root)
        }

        // The day's rows shown, with their tops in its view, top first.
        function rows() {
            return root.found(flick.contentItem, c => c.ident !== undefined && c.visible)
                .map(r => ({ key: r.modelData.key, y: r.mapToItem(flick, 0, 0).y, height: r.height, row: r }))
                .filter(r => r.y + r.height > 0 && r.y < flick.height)
                .sort((a, b) => a.y - b.y)
        }

        function toMiddle() {
            flick.contentY = (flick.contentHeight - flick.height) / 2
            frame()
            flick.contentY += rows()[0].y + 7
            frame()
            return rows()[0]
        }

        function init() {
            root.date = "2096-10-02"
            root.items = Array.from({ length: 40 }, (_, n) => root.item(n, n % 4 === 1 ? "dose" : "meal"))
            root.tell()
            page = createTemporaryObject(pageComponent, root)
            page.wanted = root.date
            page.takeView()
            tryVerify(() => root.found(page, c => c.objectName === "needRowLater" && c.visible).length >= 20, 3000, "the list made")
            wait(0)
            frame()
            let item = root.found(page, c => c.ident !== undefined && c.visible)[0]
            while (item && item.flicking === undefined)
                item = item.parent
            flick = item
            verify(flick !== null && flick.contentHeight > flick.height, "a day longer than its room")
        }

        function test_a_meal_later_from_its_row_keeps_the_place() {
            const top = toMiddle()
            const shown = rows()
            const meal = shown.slice(2).find(r => r.key.startsWith("meal:"))
            const later = root.found(meal.row, c => c.objectName === "needRowLater" && c.visible)[0]
            mouseClick(later)
            wait(0)
            frame()
            compare(mock.calls[mock.calls.length - 1].action, "later")
            compare(rows()[0].key, top.key, "the same row at the top")
            compare(rows()[0].y, top.y, "at the same pixel")
        }

        function test_taken_on_the_phone_and_added_above_keep_the_place() {
            const top = toMiddle()
            root.items = root.items.filter(i => i.key !== "dose:5")
            root.tell()
            wait(0)
            frame()
            compare(rows()[0].key, top.key, "a dose gone above")
            compare(rows()[0].y, top.y)
            root.items = root.items.concat([Object.assign(root.item(90, "meal"), { minutes: 400 })])
            root.tell()
            wait(0)
            frame()
            compare(rows()[0].key, top.key, "a meal added above")
            compare(rows()[0].y, top.y)
        }

        // Another day: another list, from its top.
        function test_another_day_starts_at_its_top() {
            toMiddle()
            root.date = "2096-10-03"
            root.tell()
            page.goTo("2096-10-03")
            wait(0)
            frame()
            compare(flick.contentY, flick.originY, "at its top")
        }
    }
}
