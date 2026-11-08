// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The agenda filter (AgendaFilter.qml, several agendas) on a stand-in for
// Sioul: its list grouped by whose the agendas are, a box per agenda and one
// per group, each group folded or open; what a box asks of the backend; "As
// the hours say" only while your word holds; on a phone, a sheet from the
// bottom within the screen's width. Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 860

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string status: ""
        property var turned: []
        property int hours: 0

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function turnAgendas(ids, on) {
            mock.turned = mock.turned.concat([{ ids: JSON.parse(ids), on: on }])
            return ""
        }
        // From a full list of tasks: the hours aside (`turn_agendas_by_hand`).
        property var byHand: []
        function turnAgendasByHand(ids, on) {
            mock.byHand = mock.byHand.concat([{ ids: JSON.parse(ids), on: on }])
            return ""
        }
        function agendasAsHours() {
            mock.hours++
            return ""
        }
        // Sets (part 4): the set chosen, kept or forgotten, last.
        property string used: ""
        property var kept: null
        function useAgendaSet(name) {
            mock.used = name
            return ""
        }
        function keepAgendaSet(name, agendas, times) {
            mock.kept = { name: name, agendas: JSON.parse(agendas), times: JSON.parse(times) }
            return ""
        }
        // Who does what (part 4): the week asked, last.
        property var week: null
        function whoDoesWhat(from, child, mine) {
            mock.week = { from: from, child: child, mine: mine }
            return JSON.stringify({ from: "2026-11-02", previous: "2026-10-26", next: "2026-11-09", week: "holder-week-of", children: [{ id: "a/lea", name: "Léa" }], days: [{ label: "Tue 3 Nov", items: [{ kind: "event", key: "/c/lea/dentist.ics", start: 1, title: "Dentist", time: "10:00", child: "Léa", who: "Sam", yours: false, nobody: false }] }] })
        }
    }

    // Yours (home shown, work left out by the hours) and Léa's (school shown).
    readonly property var shown: ({
        chosen: false,
        until: "",
        groups: [
            { key: "yours:", title: "agendas-group-yours", agendas: [{ id: "a/home", name: "Home", colour: "#4c6b5c", on: true, usual: true }, { id: "a/work", name: "Work", on: false, usual: false }] },
            { key: "child:léa", title: "Léa", agendas: [{ id: "a/lea", name: "Léa's school", on: true, usual: true }] }
        ]
    })

    Component {
        id: filterComponent

        AgendaFilter {
            sioul: mock
            theme: testTheme
        }
    }

    function all(item, test, out) {
        out = out || []
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            root.all(child, test, out)
        }
        return out
    }

    TestCase {
        id: test

        name: "AgendaFilter"
        when: windowShown

        function init() {
            mock.turned = []
            mock.hours = 0
            mock.used = ""
            mock.kept = null
            mock.week = null
        }

        function make(shown, narrow) {
            const filter = test.createTemporaryObject(filterComponent, root, { shown: shown, narrow: narrow === true })
            test.waitForRendering(filter)
            return filter
        }

        // The groups by whose, their boxes as their agendas stand; a box asks the
        // backend to turn its agenda, a group's box its whole group: all on unless
        // all are on. "As the hours say" only while your word holds.
        function test_the_boxes_turn_agendas_and_their_groups() {
            const filter = make(root.shown, false)
            const list = filter.open()
            wait(50)
            const groups = root.all(list.contentItem, c => c.objectName === "agendaGroup" && c.visible)
            compare(groups.map(g => g.text).join(" | "), "agendas-group-yours | Léa")
            compare(groups[0].checkState, Qt.PartiallyChecked, "yours: some shown")
            compare(groups[1].checkState, Qt.Checked, "Léa's: all shown")
            const lines = root.all(list.contentItem, c => c.objectName === "agendaLine" && c.visible)
            compare(lines.map(l => l.text + (l.checked ? "+" : "-")).join(" | "), "Home+ | Work- | Léa's school+")
            mouseClick(lines[1])
            compare(JSON.stringify(mock.turned[0]), JSON.stringify({ ids: ["a/work"], on: true }), "one agenda turned on")
            mouseClick(groups[0])
            compare(JSON.stringify(mock.turned[1]), JSON.stringify({ ids: ["a/home", "a/work"], on: true }), "a group half shown: all on")
            mouseClick(groups[1])
            compare(JSON.stringify(mock.turned[2]), JSON.stringify({ ids: ["a/lea"], on: false }), "a group all shown: all off")
            const hours = root.all(list.contentItem, c => c.objectName === "agendasAsHours")[0]
            verify(!hours.visible, "nothing chosen: the hours decide")
            filter.close()
            const chosen = make(Object.assign({}, root.shown, { chosen: true, until: "17:00" }), false)
            const again = chosen.open()
            wait(50)
            const button = root.all(again.contentItem, c => c.objectName === "agendasAsHours")[0]
            verify(button.visible, "your word holds: the hours offered back")
            mouseClick(button)
            compare(mock.hours, 1)
        }

        // A full list's filter (the task list, 8 November 2026): its boxes ask
        // for your word against "shown unless kept hidden", never the hours'.
        function test_a_full_lists_filter_turns_by_hand() {
            const filter = make(root.shown, false)
            filter.byHand = true
            const list = filter.open()
            wait(50)
            const lines = root.all(list.contentItem, c => c.objectName === "agendaLine" && c.visible)
            const before = mock.turned.length
            mouseClick(lines[0])
            compare(JSON.stringify(mock.byHand[mock.byHand.length - 1]), JSON.stringify({ ids: ["a/home"], on: false }), "turned by hand")
            compare(mock.turned.length, before, "never the hours' way")
            filter.close()
        }

        // A group folds to its line, its box kept.
        function test_a_group_folds() {
            const filter = make(root.shown, false)
            const list = filter.open()
            wait(50)
            const lines = () => root.all(list.contentItem, c => c.objectName === "agendaLine" && c.visible).length
            compare(lines(), 3)
            filter.fold("yours:")
            wait(30)
            compare(lines(), 1, "yours folded, Léa's open")
            compare(root.all(list.contentItem, c => c.objectName === "agendaGroup" && c.visible).length, 2, "both groups' boxes stay")
        }

        // On a phone: the button its icon alone, the list a sheet from the bottom,
        // within the screen's 412 points.
        function test_on_a_phone_a_sheet_within_the_screen() {
            const filter = make(root.shown, true)
            compare(filter.display, AbstractButton.IconOnly)
            const list = filter.open()
            wait(80)
            verify(list.x >= 0 && list.x + list.width <= root.width, "within the width: " + list.x + " + " + list.width)
            verify(list.y + list.height <= root.height && list.y > root.height / 4, "from the bottom: " + list.y + " + " + list.height)
            const lines = root.all(list.contentItem, c => c.objectName === "agendaLine" && c.visible)
            verify(lines.every(l => l.mapToItem(root, 0, 0).x + l.width <= root.width + 1), "nothing past the right edge")
        }

        // Sets (part 4): each set a button, the one showing marked; a tap chooses it;
        // "Keep these as a set…" names the agendas shown, with the times ticked; × forgets.
        // Beside a child's agenda alone, "Who does what" opens its week.
        function test_sets_and_who_does_what() {
            const sets = [{ name: "Tom and Léa", agendas: ["a/lea"], times: ["leisure"], by_time: true, showing: true }, { name: "My week", agendas: ["a/work"], times: [], by_time: false, showing: false }]
            const filter = make(Object.assign({}, root.shown, { sets: sets }), false)
            const list = filter.open()
            wait(50)
            const buttons = root.all(list.contentItem, c => c.objectName === "agendaSet" && c.visible)
            compare(buttons.map(b => b.text + (b.highlighted ? "*" : "")).join(" | "), "Tom and Léa · agendas-set-now* | My week")
            mouseClick(buttons[1])
            compare(mock.used, "My week")
            const keep = root.all(list.contentItem, c => c.objectName === "agendasKeepSet")[0]
            mouseClick(keep)
            wait(30)
            const name = root.all(list.contentItem, c => c.objectName === "agendasSetName")[0]
            verify(name.visible)
            name.text = "Evenings"
            name.accepted()
            compare(JSON.stringify(mock.kept), JSON.stringify({ name: "Evenings", agendas: ["a/home", "a/lea"], times: [] }), "the agendas shown, no time ticked")
            const who = root.all(list.contentItem, c => c.objectName === "agendaWhoDoesWhat" && c.visible)
            compare(who.length, 1, "beside Léa's agenda alone")
            mouseClick(who[0])
            wait(80)
            compare(JSON.stringify(mock.week), JSON.stringify({ from: "", child: "a/lea", mine: false }))
        }

        // No agenda at all: no button.
        function test_no_agenda_no_button() {
            const filter = make({ chosen: false, groups: [] }, false)
            verify(!filter.visible)
        }
    }
}
