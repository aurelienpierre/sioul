// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Projects page's filter by area (ProjectsPage.qml; docs/areas.md, "A
// page's filter by area"), on a stand-in for Sioul: the list holds the
// projects Rust says the ticks show, the filter's button says what shows; a
// tick, or "As the hours say", goes to Rust and the list is read again; the
// time changing reads it again; asleep, no cover and no "Show anyway": the
// filter says what shows, and one sentence when it shows none. The project's form says what it is for by the
// three switches (AreaSwitches.qml), nothing ticked saying it counts as work.
// The Time page has the same filter, its ticks the same, in place of its
// cover; one project chosen shows whatever the ticks, the filter put away.
// Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
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

        signal linksChanged
        signal tasksChanged
        signal projectsChanged
        signal invoicesChanged
        signal sharedIn(string stores)

        // What Rust's filter shows now ("leisure" as the hours say in the evening).
        property string ticked: "leisure"
        property bool own: false
        property var ticks: []
        property int asHours: 0
        property int reads: 0
        property var saved: null
        readonly property string budgets: JSON.stringify({ choices: [] })

        function text(key) { return key }
        function textWith(key, name, value) { return key + ": " + value }
        function textArgs(key, args) { return key }
        function grabFolder() { return "" }
        function settings(key) { return "[]" }
        function projectPage(id) { return "null" }
        function clientProfile(client) { return "null" }

        function projectsPage() {
            mock.reads++
            const on = mock.ticked === "" ? [] : mock.ticked.split("+")
            const row = (id, title, area) => ({ id: id, title: title, for_client: area === "work", status: "", client: "", open_tasks: 0, unbilled: "", area: area, ai: false, shown: area.split("+").some(a => on.indexOf(a) >= 0) })
            return JSON.stringify({
                rows: [row("lumen", "Studio Lumen site", "work"), row("garden", "Community garden", "admin+leisure"), row("taxes", "Tax return", "admin")],
                filter: { work: on.indexOf("work") >= 0, admin: on.indexOf("admin") >= 0, leisure: on.indexOf("leisure") >= 0, label: mock.ticked === "" ? "area-filter-none" : on.join(" · "), own: mock.own }
            })
        }
        function tickProjectAreas(areas) {
            mock.ticks = mock.ticks.concat([areas])
            mock.ticked = areas
            mock.own = true
        }
        function projectAreasAsHours() {
            mock.asHours++
            mock.ticked = "leisure"
            mock.own = false
        }
        function zoneOffset(seconds) { return 0 }
        function projectRows() { return "[]" }
        // The Time page: what its filter folds is Rust's; the filter's button, as the Projects page's.
        property var asked: []
        property string timeView: ""
        function showTimePage(period, anchor, project) {
            mock.asked = mock.asked.concat([project])
            const shown = JSON.parse(mock.projectsPage())
            mock.timeView = JSON.stringify({ title: "", sentence: "", total: "1 h", total_line: "1 h in all", most: 0, bars: [], entries: [], projects: [], filter: shown.filter, by_area: [{ area: "work", spent: "1 h 30", estimated: "1 h", spent_minutes: 90, estimated_minutes: 60 }, { area: "leisure", spent: "40 min", estimated: "45 min", spent_minutes: 40, estimated_minutes: 45 }] })
        }
        function saveProject(id, edit) {
            mock.saved = JSON.parse(edit)
            return JSON.stringify({ id: id || "new" })
        }
    }

    QtObject {
        id: shell

        property var moment: ({ quiet: true, sleep: false, line: "Leisure until 22:00: what you enjoy." })
        property bool compact: true
        property string sioulLocale: "en_GB"
        property string today: "2099-01-05"
        function openInvoiceDraft(draft) {}
    }

    Component {
        id: pageComponent

        ProjectsPage {
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

        name: "Projects"
        when: windowShown

        function init() {
            mock.ticked = "leisure"
            mock.own = false
            mock.ticks = []
            mock.asHours = 0
            mock.reads = 0
            mock.saved = null
            shell.moment = { quiet: true, sleep: false, line: "Leisure until 22:00: what you enjoy." }
        }

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item))
                found.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, found)
            return found
        }

        function named(item, name) {
            return all(item, i => i.objectName === name)[0] || all(item.Overlay.overlay, i => i.objectName === name)[0] || null
        }

        // The titles the list shows, in its order.
        function listed(page) {
            return page.listed.map(r => r.title)
        }

        function test_the_list_shows_what_the_filter_ticks() {
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            compare(listed(page), ["Community garden"], "in the evening, leisure's")
            const filter = named(page, "projectAreas")
            verify(filter !== null && filter.visible, "the filter above the list")
            compare(filter.shown.label, "leisure")
            verify(!filter.shown.own)
        }

        function test_a_tick_reads_the_list_again_and_the_hours_bring_it_back() {
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            const filter = named(page, "projectAreas")
            // Work ticked too, to manage a client's project in the evening.
            filter.tick("work", true)
            compare(mock.ticks, ["work+leisure"], "the three as they then stand, to Rust")
            compare(listed(page), ["Studio Lumen site", "Community garden"], "read again at once")
            verify(filter.shown.own, "your ticks hold")
            filter.unfold()
            const back = named(page, "areaFilterAsHours")
            verify(back !== null && back.visible, "As the hours say, while your ticks hold")
            back.clicked()
            compare(mock.asHours, 1)
            compare(listed(page), ["Community garden"], "the hours' again")
            filter.fold()
        }

        function test_the_time_changing_reads_it_again() {
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            const before = mock.reads
            mock.ticked = "work"
            shell.moment = { quiet: false, sleep: false, line: "" }
            compare(mock.reads, before + 1, "read as the time changes")
            compare(listed(page), ["Studio Lumen site"], "work's in working hours")
        }

        function test_asleep_no_cover_the_filter_says_what_shows() {
            mock.ticked = ""
            shell.moment = { quiet: true, sleep: true, line: "Sleep: nothing disturbs until 07:00." }
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            compare(listed(page), [], "nothing ticked")
            verify(named(page, "projectsNoneTicked").visible, "said: no project for what is ticked")
            compare(all(page, i => i.objectName === "" && i.text === "ui-show-anyway").length, 0, "no Show anyway")
            verify(all(page, i => i.text === "project-resting").length === 0, "no resting line")
            verify(named(page, "projectAreas").visible, "the filter, everything one tick away")
            named(page, "projectAreas").tick("admin", true)
            compare(listed(page), ["Community garden", "Tax return"])
            verify(!named(page, "projectsNoneTicked").visible)
        }

        function test_the_time_page_has_the_same_filter_and_no_cover() {
            shell.moment = { quiet: true, sleep: true, line: "Sleep: nothing disturbs until 07:00." }
            const page = createTemporaryObject(timeComponent, root)
            waitForRendering(page)
            const filter = named(page, "timeAreas")
            verify(filter !== null && filter.visible, "the filter over the time")
            compare(page.shown.total_line, "1 h in all", "the view asked, taken when it came")
            // Where estimates are off, by area: a line an area, time spent beside the first estimate.
            const table = named(page, "timeByArea")
            verify(table !== null && table.visible, "the comparison by area")
            const words = all(table, i => i.text !== undefined && i.visible).map(i => i.text)
            verify(words.indexOf("area-filter-work") >= 0 && words.indexOf("1 h 30") >= 0 && words.indexOf("1 h") >= 0 && words.indexOf("area-filter-leisure") >= 0 && words.indexOf("45 min") >= 0, words.join(" | "))
            verify(words.indexOf("area-filter-admin") < 0, "no line for an area with nothing done")
            compare(all(page, i => i.text === "ui-show-anyway").length, 0, "no cover, no Show anyway")
            const reads = mock.asked.length
            filter.tick("work", true)
            compare(mock.ticks, ["work+leisure"], "the projects' ticks")
            compare(mock.asked.length, reads + 1, "read again")
            compare(filter.shown.label, "work · leisure")
            // One project chosen: shown whatever the ticks, the filter put away.
            page.project = "lumen"
            page.reload()
            compare(mock.asked[mock.asked.length - 1], "lumen")
            verify(!filter.visible)
            // The pictures' way: every area ticked.
            page.project = ""
            page.showAll()
            compare(mock.ticks[mock.ticks.length - 1], "work+admin+leisure")
        }

        function test_the_form_says_what_a_project_is_for() {
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            // A new project's form (`editOpen` with none open).
            const form = page.editOpen()
            tryVerify(() => form.opened)
            const switches = named(page, "projectArea")
            verify(switches !== null, "the three switches in the form")
            compare(switches.value, "", "a new project: nothing said")
            compare(all(switches.parent, i => i.text === "project-area-unsaid").length, 1, "nothing ticked: counted as work, said so")
            switches.picked("admin+leisure")
            compare(switches.value, "admin+leisure")
            compare(all(switches.parent, i => i.text === "project-area-unsaid").length, 0)
            form.save()
            verify(mock.saved !== null, "saved")
            compare(mock.saved.area, "admin+leisure", "in the files' words")
            // An older record's word, as Rust gives it in the files' words.
            form.edit("garden", { title: "Community garden", for_client: false, client: "", rate: 0, budget: "", status: "", area: "admin+leisure", ai: false })
            compare(switches.value, "admin+leisure")
            form.close()
        }
    }
}
