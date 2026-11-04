// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The household at the head of the Contacts page (Household.qml) and a
// person's page (PersonPage.qml) at a phone's width, 412 points, on a
// stand-in for Sioul: you first, a child, a parent helped, the child's other
// parent after a gap, "Add a person"; nothing past the screen's edge, the
// chips wrapping; a child's page with its line, papers and card, the switch
// for agents off and pressed; your own page without the switch or cards.
// Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 891

    property var cards: []
    function openCard(ref) {
        root.cards.push(ref)
    }

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var agents: []
        property var opened: []

        // Made up: no real person.
        function household() {
            return JSON.stringify({
                persons: [
                    { id: "me", name: "persons-you", relation: "me", line: "" },
                    { id: "p-lea", name: "Léa", relation: "child", line: "Child · 12 · lives with you" },
                    { id: "p-odile", name: "Odile Grandmother-of-the-household", relation: "parent", line: "Parent" },
                    { id: "p-robin", name: "Robin", relation: "partner", line: "Partner" }
                ],
                others: [{ id: "p-sam", name: "Sam", relation: "other-parent", line: "Léa’s other parent" }],
                relations: [{ id: "child", label: "Child" }, { id: "", label: "In your household" }]
            })
        }
        function personPage(id) {
            if (id === "me")
                return JSON.stringify({ id: "me", me: true, name: "persons-you", full_name: "", relation: "me", line: "", born: "", lives_with: false, agents: true, holds_papers: true, of: [], papers: [], cards: [] })
            return JSON.stringify({
                id: "p-lea", me: false, name: "Léa", full_name: "Léa Martin", relation: "child",
                line: "Child · 12 · lives with you", born: "2014-05-12", lives_with: true, agents: false, holds_papers: true, of: [],
                papers: [{ id: "lea-id", title: "Identity card with a long name to wrap on a phone", kind_label: "Identity card", line: "valid until Monday 3 May 2027", standing: "valid", persons: ["p-lea"] },
                         { id: "lea-school", title: "School insurance", kind_label: "Insurance", line: "valid until Wednesday 30 June 2027: time to renew it", standing: "renew", persons: ["p-lea"] }],
                cards: [{ uid: "c-lea", key: "/stand-in/lea.vcf", name: "Léa Martin", detail: "lea@example.org" }],
                agendas: [{ id: "home/lea-school", name: "Léa, school and activities", child: true, whose: "" }]
            })
        }
        function whoDoesWhat(from, child, mine) {
            mock.week = child
            return JSON.stringify({ days: [], children: [], week: "", from: "", previous: "", next: "" })
        }
        property string week: ""
        function papers() {
            return JSON.stringify({ store: true, problem: "", kinds: [], families: [], choices: [], chips: [] })
        }
        function setPersonAgents(id, on) {
            mock.agents.push(id + "=" + on)
            return ""
        }
        function linkPersonCard(id, uid, linked) {
            return ""
        }
        function personCardChoices(query) {
            return "[]"
        }
        function savePerson(id, edit) {
            return JSON.stringify({ id: id })
        }
        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ": " + value
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        Household {
            id: household

            Layout.fillWidth: true
            sioul: mock
            theme: testTheme
            onOpenPerson: id => mock.opened.push(id)
        }
        PersonChooser {
            id: chooser

            property string picked: "unset"
            property int asked: 0

            Layout.fillWidth: true
            sioul: mock
            theme: testTheme
            persons: [{ id: "p-lea", name: "Léa" }, { id: "p-tom", name: "Tom" }]
            onChosen: id => chooser.picked = id
            onNewAsked: chooser.asked += 1
        }
        PersonPage {
            id: person

            Layout.fillWidth: true
            Layout.fillHeight: true
            sioul: mock
            theme: testTheme
            window: root
        }
    }

    TestCase {
        name: "Household"
        when: windowShown

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

        // The item of this objectName under `item`, shown or not.
        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            return item.contentItem && item.contentItem !== item ? find(item.contentItem, name) : null
        }

        // Every label shown under `item`, each once.
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

        function test_you_first_then_the_household_then_other_parents() {
            household.reload()
            waitForRendering(household)
            const chips = find(household, "householdChips")
            verify(chips !== null)
            const you = find(household, "householdChip:persons-you")
            const lea = find(household, "householdChip:Léa")
            const sam = find(household, "householdChip:Sam")
            const add = find(household, "householdAdd")
            verify(you && lea && sam && add, "every chip and the add button")
            // In reading order: you, Léa, …, Sam, then "Add a person".
            const order = item => item.mapToItem(root, 0, 0).y * 10000 + item.mapToItem(root, 0, 0).x
            verify(order(you) < order(lea) && order(lea) < order(sam) && order(sam) < order(add), "you, the household, the other parents, add")
            compare(past(household, []).join("; "), "", "the chips wrap within 412 points")
            mouseClick(lea)
            compare(mock.opened[mock.opened.length - 1], "p-lea", "a chip opens the person's page")
        }

        function test_a_child_s_page_and_its_switch_for_agents() {
            person.personId = ""
            person.personId = "p-lea"
            waitForRendering(person)
            const shown = labels(person, [])
            verify(shown.indexOf("Léa") >= 0 && shown.indexOf("Child · 12 · lives with you") >= 0, shown.join(" | "))
            verify(shown.indexOf("person-papers") >= 0 && shown.indexOf("person-contacts") >= 0, "both sections, each said once: " + shown.join(" | "))
            compare(shown.filter(t => t === "person-papers").length, 1)
            verify(shown.indexOf("lea@example.org") >= 0, "the linked card")
            const agents = find(person, "personAgents")
            verify(agents.visible && !agents.checked, "closed to agents until opened")
            mouseClick(agents)
            compare(mock.agents[mock.agents.length - 1], "p-lea=true")
            compare(past(person, []).join("; "), "", "nothing past 412 points")
        }

        function test_a_child_s_agendas_with_who_does_what() {
            person.personId = ""
            person.personId = "p-lea"
            waitForRendering(person)
            const shown = labels(person, [])
            verify(shown.indexOf("person-agendas") >= 0 && shown.indexOf("Léa, school and activities") >= 0, shown.join(" | "))
            mouseClick(find(person, "personWhoDoesWhat"))
            compare(mock.week, "home/lea-school", "who does what, on that agenda")
            // Its sheet closed again, so that it covers nothing for the next test.
            keyClick(Qt.Key_Escape)
            tryVerify(() => !Overlay.overlay || Overlay.overlay.children.every(c => !c.visible || c.objectName !== "whoDoesWhat"))
        }

        // Nobody said first, then the persons, then "New person…", which asks the caller.
        function test_a_person_chosen_or_a_new_one_asked() {
            compare(chooser.model.length, 4)
            compare(chooser.model[0], "agenda-person-none")
            compare(chooser.currentIndex, 0)
            chooser.personId = "p-tom"
            compare(chooser.currentIndex, 2)
            chooser.activated(1)
            compare(chooser.picked, "p-lea")
            chooser.activated(3)
            compare(chooser.asked, 1)
            compare(chooser.currentIndex, 2, "still the person chosen")
            chooser.personId = ""
            chooser.fallback = "Lea (an older text)"
            compare(chooser.model[0], "Lea (an older text)", "a name naming nobody, as written")
        }

        function test_your_own_page_has_no_switch_and_no_cards() {
            person.personId = "me"
            waitForRendering(person)
            verify(!find(person, "personAgents").visible, "you are always open, as before")
            verify(!find(person, "personLinkCard").visible, "no card to link to you")
            verify(find(person, "personAddPaper").visible)
        }
    }
}
