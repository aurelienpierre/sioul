// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Duplicates' merge across an address book's switch (ContactDuplicates.qml,
// review L2) on a stand-in for Sioul, at a computer's width and at a
// phone's: when the card kept is in a book whose groups decide who may
// reach the person and the other card, in a book whose groups are held
// back, carries groups the kept one lacks, one box offers to take them,
// unticked, with what taking them does to who may reach you; Merge sends
// the box as it is; another name kept, or another pair, unticks it again;
// a merge that crosses no switch offers no box. Which groups cross and what
// they do is the core's (`addressbooks::merge_groups`, tested there); this
// is the window's side. Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1200
    height: 1400

    Theme {
        id: testTheme
    }

    // Lou's card on the co-workers' book ("Team", its groups held back) and Lou's own
    // card; then two cards of one office, in books whose groups both decide. Invented.
    function side(key, name, book, groups) {
        return { key: key, name: name, book: book, photo: "", work: "", phones: [], emails: ["lou@example.org"], addresses: [], birthday: "", categories: groups, notes: "" }
    }
    readonly property var found: ({
        region: "Numbers read as in France.",
        within: "",
        cleanings: [],
        between: "2 pairs may be one person each.",
        pairs: [
            { first: root.side("/team/lou.vcf", "Lou Exemple", "Team · example.org", ["Clients"]), second: root.side("/contacts/lou.vcf", "L. Exemple", "Contacts · example.org", []), lead: "second", share: "Same e-mail address." },
            { first: root.side("/contacts/office.vcf", "Exemple office", "Contacts · example.org", ["Suppliers"]), second: root.side("/pro/office.vcf", "Office Exemple", "Professional · example.org", []), lead: "first", share: "Same e-mail address." }
        ],
        done: [],
        kept: ""
    })

    QtObject {
        id: mock

        property string duplicates: ""
        property var merged: []
        property var asked: []

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + name + "=" + value }
        function findDuplicates() { mock.duplicates = JSON.stringify(root.found) }
        // The groups only the other card carries, when it comes from the held-back book into one whose groups decide.
        function duplicatesMergeAsk(lead, other) {
            // Noted in place: a property assigned here, inside the view's binding, would loop.
            mock.asked.push([lead, other])
            return JSON.stringify(other === "/team/lou.vcf" && lead !== other
                ? { groups: ["Clients"], offer: "Also take its groups: Clients", reach: "Taken into the card kept, “Clients” is on your Safe list: its addresses and numbers would be let through as that list says." }
                : { groups: [], offer: "", reach: "" })
        }
        function mergeContacts(lead, other, groups) {
            mock.merged = mock.merged.concat([[lead, other, groups]])
            return JSON.stringify({ key: lead })
        }
        function cleanContacts(keys) { return "" }
        function notSameContacts(first, second) { return "" }
        function undoContacts(id) { return "" }
    }

    Component {
        id: viewComponent

        ContactDuplicates {
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

    function named(item, name) {
        return root.all(item, c => c.objectName === name)
    }

    TestCase {
        id: test

        name: "ContactDuplicates"
        when: windowShown

        function init() {
            mock.duplicates = ""
            mock.merged = []
            mock.asked = []
        }

        function make(width) {
            const view = test.createTemporaryObject(viewComponent, root, { width: width, height: 1300, compact: width < 560 })
            tryVerify(() => view.pair !== null, 1000, "the pairs read")
            test.waitForRendering(view)
            return view
        }

        function mergeButton(view) {
            return root.all(view, c => c.text === "dup-merge" && c.visible)[0]
        }

        // The card kept in a book whose groups decide, the other's groups held back: one box,
        // unticked, and what taking them does; Merge sends it as it is.
        function test_the_other_card_s_groups_only_with_the_box() {
            for (const width of [1000, 392]) {
                init()
                const view = make(width)
                compare(view.kept, "second", "Lou's own card's name kept, as the core leads")
                compare(JSON.stringify(mock.asked[mock.asked.length - 1]), JSON.stringify(["/contacts/lou.vcf", "/team/lou.vcf"]), "asked of the core: the kept card, then the other")
                const box = root.named(view, "duplicatesTakeGroups")[0]
                const reach = root.named(view, "duplicatesTakeGroupsReach")[0]
                verify(box.visible && !box.checked, "offered, unticked")
                compare(box.text, testTheme.plain("Also take its groups: Clients"))
                verify(reach.visible && reach.text.indexOf("Safe list") >= 0, "what taking them does, said")
                for (const item of [box, reach]) {
                    const x = item.mapToItem(view, 0, 0).x
                    verify(x >= 0 && x + item.width <= width + 1, item.objectName + " within " + width + ": " + x + " + " + item.width)
                }
                // Left unticked: merged without them.
                mouseClick(mergeButton(view))
                compare(JSON.stringify(mock.merged[0]), JSON.stringify(["/contacts/lou.vcf", "/team/lou.vcf", false]))
                // Ticked: with them.
                mouseClick(box)
                verify(box.checked && view.takeGroups)
                mouseClick(mergeButton(view))
                compare(JSON.stringify(mock.merged[1]), JSON.stringify(["/contacts/lou.vcf", "/team/lou.vcf", true]))
            }
        }

        // Another name kept, the other way round: nothing crosses, no box; back, unticked again.
        // Another pair: unticked; one that crosses no switch offers no box.
        function test_the_box_unticked_again_and_only_where_it_crosses() {
            const view = make(1000)
            const box = root.named(view, "duplicatesTakeGroups")[0]
            mouseClick(box)
            verify(view.takeGroups)
            const names = root.all(view, c => c.text === testTheme.plain("Lou Exemple") && c.visible && c.checkable === true)
            compare(names.length, 1, "whose name is kept: the team card's")
            mouseClick(names[0])
            compare(view.kept, "first")
            verify(!view.takeGroups, "another name kept: unticked")
            verify(!box.visible && !root.named(view, "duplicatesTakeGroupsReach")[0].visible, "the own card's groups into the team's book: nothing crosses")
            mouseClick(mergeButton(view))
            compare(JSON.stringify(mock.merged[0]), JSON.stringify(["/team/lou.vcf", "/contacts/lou.vcf", false]))
            view.kept = "second"
            verify(box.visible && !box.checked, "back: offered again, unticked")
            mouseClick(box)
            verify(view.takeGroups)
            // "Later": the next pair, unticked, and no box (both books' groups decide).
            mouseClick(root.all(view, c => c.text === "dup-later" && c.visible)[0])
            compare(view.pair.first.key, "/contacts/office.vcf")
            verify(!view.takeGroups, "another pair: unticked")
            verify(!box.visible, "no switch crossed: no box")
            mouseClick(mergeButton(view))
            compare(JSON.stringify(mock.merged[1]), JSON.stringify(["/contacts/office.vcf", "/pro/office.vcf", false]))
        }
    }
}
