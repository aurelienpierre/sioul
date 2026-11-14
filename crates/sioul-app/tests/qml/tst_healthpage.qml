// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Health page's list after midnight (HealthPage.qml; docs/health.md,
// "Tonight"): today's list holds the night going on, kept under the evening
// before, and this evening's own; each row's changes are sent under its own
// day, never the day shown: its "15 min later", its menu (the alarm's switch)
// and its form. On a stand-in for Sioul that keeps what it is asked. Made-up
// days far ahead. Run by tools/qml-test.sh.

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
        function changeNeed(edit) {
            mock.calls.push(JSON.parse(edit))
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
        name: "HealthPage"
        when: windowShown

        function makePage() {
            mock.calls = []
            const page = createTemporaryObject(pageComponent, root)
            page.wanted = "2096-10-02"
            const lunch = { key: "meal:1", date: "2096-10-02", kind: "meal", name: "Lunch", dose: "", from: "12:10", to: "13:00", at: "12:30", detail: "", note: "", changed: false, added: false, quiet: false, off: false, past: false, minutes: 30, before: 20, after: 0, taken: "", late: false, doubt: "", doubt_off: [], choose: false, alarm: "", alarm_skipped: false, alarm_open: false, waking: "", rings: false }
            const today = { date: "2096-10-02", title: "Today, Tuesday 2 October", weekday: "Tue", number: 2, today: true, past: false, items: [root.night("2096-10-01", "going on"), lunch, root.night("2096-10-02", "this evening")], lines: [] }
            mock.healthView = JSON.stringify({ any: true, unreadable: "", week: { monday: "2096-10-01", today: "2096-10-02", days: [today] }, missed: [], shared_note: "", reminded_there: "", later: 15, medicines: [], prescriptions: [] })
            page.takeView()
            tryVerify(() => root.found(page, c => c.objectName === "needRowLater" && c.visible).length === 3, 3000, "the list made")
            return page
        }

        // Each row's "15 min later" is sent under its own day: the night going on under the evening before.
        function test_a_row_s_change_is_kept_under_its_own_day() {
            const page = makePage()
            const later = root.found(page, c => c.objectName === "needRowLater" && c.visible)
            mouseClick(later[0])
            mouseClick(later[2])
            compare(mock.calls.map(c => [c.date, c.key, c.action].join(" ")).join(" | "), "2096-10-01 sleep later | 2096-10-02 sleep later")
        }

        // Its menu: the alarm's switch of the night going on, sent under the evening before.
        function test_a_row_s_menu_is_kept_under_its_own_day() {
            const page = makePage()
            const more = root.found(page, c => c.objectName === "needRowMore" && c.visible)
            compare(more.length, 3)
            mouseClick(more[0])
            tryVerify(() => page.menuOpen, 2000, "the night's menu open")
            const menu = root.found(page, c => c.acted !== undefined && c.formAsked !== undefined)[0]
            const switches = () => root.found(menu.contentItem, c => c.objectName === "needAlarm" && c.visible)
            compare(switches().length, 1, "the switch in the night's menu")
            const alarm = switches()
            verify(alarm[0].checked, "it rings")
            alarm[0].triggered()
            compare(mock.calls.map(c => [c.date, c.key, c.action].join(" ")).join(" | "), "2096-10-01 sleep no-alarm")
        }

        // Its form ("Change its times…"): saved under the night's own day too.
        function test_a_row_s_form_is_kept_under_its_own_day() {
            const page = makePage()
            page.showChange("sleep", "times")
            const form = root.found(page, c => c.keptKey !== undefined && c.ownDate !== undefined)
            compare(form.length, 1)
            compare(form[0].keptKey, "need:2096-10-01|sleep|times")
            form[0].save()
            compare(mock.calls.map(c => [c.date, c.key, c.action].join(" ")).join(" | "), "2096-10-01 sleep times")
        }
    }
}
