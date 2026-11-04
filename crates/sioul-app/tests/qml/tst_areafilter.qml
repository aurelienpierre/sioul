// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A page's filter by area (AreaFilter.qml; docs/areas.md, "A page's filter
// by area"), on a stand-in for Sioul: its button says what shows and unfolds
// three boxes ticked as Rust says; a box ticked or unticked says the three as
// they then stand, and shows Rust's word until Rust answers; "As the hours
// say" only while your ticks hold; the "?" says how it works, once; at a
// phone's 412 points nothing passes the window's edge, in the longest words
// of both languages. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 732

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // The longest of each language's words.
        readonly property var words: ({ "area-filter-work": "Travail", "area-filter-admin": "Démarches", "area-filter-leisure": "Loisirs", "area-filter-as-hours": "Selon vos heures", "area-filter-how": "Comment ce filtre fonctionne" })

        function text(key) {
            return mock.words[key] || key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
    }

    Component {
        id: filterComponent

        AreaFilter {
            sioul: mock
            theme: testTheme
            help: "Montre les courriels des adresses activées, pour ce qui est coché. Les cases suivent vos heures, et rien n’est coché pendant votre sommeil ; ce que vous cochez vous-même tient jusqu’au changement d’heures. Une adresse dont l’usage n’est pas dit compte comme travail."
        }
    }

    Component {
        id: spyComponent

        SignalSpy {}
    }

    TestCase {
        name: "AreaFilter"
        when: windowShown

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

        // An item of the filter by its name: its button, or in its list over the window.
        function named(filter, name) {
            return all(filter, i => i.objectName === name)[0] || all(filter.Overlay.overlay, i => i.objectName === name)[0] || null
        }

        function made(shown, x) {
            return createTemporaryObject(filterComponent, root, { shown: shown, x: x || 0, y: 40 })
        }

        function opened(filter) {
            filter.unfold()
            tryCompare(filter, "unfolded", true)
        }

        // The button says what shows, for the eye and for screen readers, and
        // unfolds the three boxes ticked as Rust says; a click again folds them.
        function test_the_button_says_what_shows() {
            const filter = made({ work: false, admin: true, leisure: true, label: "Démarches · Loisirs", own: false })
            const button = named(filter, "areaFilter")
            compare(button.text, "Démarches · Loisirs  ▾")
            compare(button.Accessible.name, "area-filter-name:Démarches · Loisirs")
            mouseClick(button)
            tryCompare(filter, "unfolded", true)
            compare([named(filter, "area:work").checked, named(filter, "area:admin").checked, named(filter, "area:leisure").checked], [false, true, true])
            compare([named(filter, "area:work").text, named(filter, "area:admin").text, named(filter, "area:leisure").text], ["Travail", "Démarches", "Loisirs"])
            verify(!named(filter, "areaFilterAsHours").visible, "as the hours say: nothing to set back")
            mouseClick(button)
            tryCompare(filter, "unfolded", false)
        }

        // A box says the three as they stand ("" for none), and shows Rust's
        // word until Rust answers with the filter's new state.
        function test_a_box_says_the_three_as_they_stand() {
            const filter = made({ work: false, admin: true, leisure: true, label: "Démarches · Loisirs", own: false })
            const spy = createTemporaryObject(spyComponent, root, { target: filter, signalName: "ticked" })
            opened(filter)
            mouseClick(named(filter, "area:work"))
            compare(spy.count, 1)
            compare(spy.signalArguments[0][0], "work+admin+leisure")
            compare(named(filter, "area:work").checked, false, "Rust's word until it answers")
            filter.shown = { work: true, admin: true, leisure: true, label: "Travail · Démarches · Loisirs", own: true }
            compare(named(filter, "area:work").checked, true)
            mouseClick(named(filter, "area:admin"))
            compare(spy.signalArguments[1][0], "work+leisure")
            filter.shown = { work: false, admin: false, leisure: true, label: "Loisirs", own: true }
            mouseClick(named(filter, "area:leisure"))
            compare(spy.signalArguments[2][0], "", "nothing ticked")
        }

        // "As the hours say": only while your ticks hold, and it says so.
        function test_as_the_hours_say_while_your_ticks_hold() {
            const filter = made({ work: true, admin: false, leisure: true, label: "Travail · Loisirs", own: true })
            const spy = createTemporaryObject(spyComponent, root, { target: filter, signalName: "asHours" })
            opened(filter)
            const back = named(filter, "areaFilterAsHours")
            verify(back.visible)
            compare(back.text, "Selon vos heures")
            mouseClick(back)
            compare(spy.count, 1)
            filter.shown = { work: true, admin: false, leisure: false, label: "Travail", own: false }
            verify(!back.visible)
        }

        // One "?": how it works, under the boxes, while pressed; folded, gone.
        function test_the_question_mark_says_how_it_works() {
            const filter = made({ work: true, admin: false, leisure: false, label: "Travail", own: false })
            opened(filter)
            const help = named(filter, "areaFilterHelpText")
            verify(!help.visible)
            compare(named(filter, "areaFilterHelp").Accessible.name, "Comment ce filtre fonctionne")
            mouseClick(named(filter, "areaFilterHelp"))
            verify(help.visible)
            compare(help.text, filter.help)
            mouseClick(named(filter, "areaFilterHelp"))
            verify(!help.visible)
            mouseClick(named(filter, "areaFilterHelp"))
            filter.fold()
            tryCompare(filter, "unfolded", false)
            opened(filter)
            verify(!help.visible, "unfolded again: the boxes alone")
        }

        // At a phone's 412 points, its button near the edge: the list, its
        // words the longest, its help shown, all within the window.
        function test_nothing_passes_a_phone_s_edge() {
            const filter = made({ work: true, admin: true, leisure: true, label: "Travail · Démarches · Loisirs", own: true }, 300)
            opened(filter)
            mouseClick(named(filter, "areaFilterHelp"))
            const help = named(filter, "areaFilterHelpText")
            // Laid out: within the list's width, on several lines.
            tryVerify(() => help.visible && help.lineCount > 1)
            const drawn = all(filter.Overlay.overlay, i => i.visible && i.width > 0 && (i.objectName.indexOf("area") === 0))
            verify(drawn.length >= 6, drawn.length + " items")
            for (const item of drawn) {
                const at = item.mapToItem(null, 0, 0)
                verify(at.x >= 0 && at.x + item.width <= root.width + 0.5, item.objectName + ": " + at.x + " + " + item.width)
            }
        }
    }
}
