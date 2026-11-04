// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The agenda's line in Agenda ⚙ ▸ Your agendas (AgendasSetup.qml, several
// agendas, part 5) on a stand-in for Sioul: an agenda of yours on a server is
// renamed, recoloured, and removed only after the sentence that says what
// goes; its sharing is asked of its server when the line opens, says who sees
// it, shares with a person or a group, and stops; someone else's offers
// "Stop showing…" and nothing to change; one on this computer asks no server.
// "New agenda…" sends its role with it, a Google one events only. Fiction
// only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 2400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal agendaShared(string id, string answer)
        signal copyRequested(string text)

        property var asked: []
        property var calls: []

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function agendasSetup() { return JSON.stringify(root.view) }
        function availability() { return JSON.stringify({ current: null, line: "", agendas: [], accounts: [] }) }
        function setAgendaField(id, field, value) { return "" }
        function readAgendaSharing(id) { mock.asked = mock.asked.concat([id]) }
        function renameAgenda(id, name) { mock.calls = mock.calls.concat([["rename", id, name]]); return "" }
        function recolourAgenda(id, colour) { mock.calls = mock.calls.concat([["colour", id, colour]]); return "" }
        function agendaRemoval(id) {
            mock.calls = mock.calls.concat([["removal", id]])
            return JSON.stringify(id === "cloud/swimming"
                ? { act: "remove", sentence: "Remove “Swimming”, 42 events, from cloud.example.org?", note: "Nextcloud keeps it in its trash bin." }
                : { act: "leave", sentence: "Stop showing “Sam’s week”?", note: "" })
        }
        function removeAgenda(id) { mock.calls = mock.calls.concat([["remove", id]]); return "" }
        function shareAgenda(id, who, access) { mock.calls = mock.calls.concat([["share", id, who, access]]) }
        function unshareAgenda(id, who) { mock.calls = mock.calls.concat([["unshare", id, who]]) }
        function agendaLink(id, on) { mock.calls = mock.calls.concat([["link", id, on]]) }
        function newAgenda(text) { mock.calls = mock.calls.concat([["new", JSON.parse(text)]]); return JSON.stringify({ place: "google/abc", problem: "" }) }
    }

    readonly property var row: ({ colour: "#4c6b5c", events: true, tasks: false, whose: "yours", person: "", area: "admin+leisure", reaches: "all-day", access: "write", server_read_only: false, for_work: false, said: false, kept_hidden: false, parent: "", parent_address: "", parent_answers: "mail", standing: [], titles: false })
    readonly property var view: ({
        agendas: [
            Object.assign({}, root.row, { id: "cloud/swimming", name: "Swimming", place: "cloud.example.org", server: { server: "caldav", server_name: "cloud.example.org", own: true } }),
            Object.assign({}, root.row, { id: "cloud/sam", name: "Sam’s week", place: "cloud.example.org", whose: "shared", person: "Sam", access: "read", server_read_only: true, server: { server: "caldav", server_name: "cloud.example.org", own: false } }),
            Object.assign({}, root.row, { id: "local/notes", name: "Here", place: "", server: { server: "local", server_name: "this computer", own: true } })
        ],
        pairs: [],
        children: { kinds: [], yours: [], follow_ups: true, any: false },
        accounts: [{ id: "cloud", name: "you@cloud.example.org", google: false }, { id: "google", name: "you@gmail.example", google: true }, { id: "local", name: "this computer", google: false }]
    })

    Component {
        id: setupComponent

        AgendasSetup {
            width: root.width
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

        name: "AgendasSetup"
        when: windowShown

        function init() {
            mock.asked = []
            mock.calls = []
        }

        function make() {
            const setup = test.createTemporaryObject(setupComponent, root)
            test.waitForRendering(setup)
            return setup
        }

        // Each line within its agenda: the agenda's own controls, by its place.
        function line(setup, index) {
            return root.named(setup, "agendaSetup")[index]
        }

        // Yours, on its server: renamed, recoloured, removed after the
        // sentence; its sharing asked when it opens, said, changed.
        function test_an_agenda_of_yours_on_its_server() {
            const setup = make()
            const swimming = line(setup, 0)
            setup.toggle("cloud/swimming")
            wait(30)
            compare(mock.asked, ["cloud/swimming"], "who sees it, asked of its server when the line opens")
            compare(root.named(swimming, "agendaSharingLine")[0].text, "agenda-sharing-reading")

            const rename = root.named(swimming, "agendaRename")[0]
            verify(rename.visible, "yours: its name to change")
            rename.text = "Swimming club"
            rename.editingFinished()
            compare(JSON.stringify(mock.calls[0]), JSON.stringify(["rename", "cloud/swimming", "Swimming club"]))

            const dots = root.named(swimming, "agendaColour")
            compare(dots.length, 8)
            mouseClick(dots[2])
            compare(JSON.stringify(mock.calls[1]), JSON.stringify(["colour", "cloud/swimming", "#7a5c99"]))

            // Removed only on the second press, the sentence said first.
            const remove = root.named(swimming, "agendaRemove")[0]
            compare(remove.text, "agenda-remove")
            mouseClick(remove)
            compare(JSON.stringify(mock.calls[2]), JSON.stringify(["removal", "cloud/swimming"]))
            wait(30)
            compare(root.named(swimming, "agendaRemoveQuestion")[0].text, "Remove “Swimming”, 42 events, from cloud.example.org?")
            verify(!remove.visible, "the question in its place")
            compare(mock.calls.length, 3, "nothing removed yet")
            mouseClick(root.named(swimming, "agendaRemoveConfirm")[0])
            compare(JSON.stringify(mock.calls[3]), JSON.stringify(["remove", "cloud/swimming"]))

            // Its server answers: who sees it, and what may be done.
            mock.agendaShared("cloud/swimming", JSON.stringify({ line: "Sam can read it.", sharer: "nextcloud", people: [{ id: "principal:principals/users/sam", name: "Sam", access: "read", group: false }], link: null, links: true, note: "People and groups of this Nextcloud alone.", said: "", problem: "" }))
            wait(30)
            compare(root.named(swimming, "agendaSharingLine")[0].text, "Sam can read it.")
            const group = root.named(swimming, "agendaShareGroup")[0]
            verify(group.visible, "a group of the same Nextcloud")
            const field = root.named(swimming, "agendaShareWith")[0]
            field.text = "family"
            group.checked = true
            mouseClick(root.named(swimming, "agendaShare")[0])
            compare(JSON.stringify(mock.calls[4]), JSON.stringify(["share", "cloud/swimming", "group:family", "read"]))
            compare(field.text, "", "the field emptied for the next one")
            mouseClick(root.named(swimming, "agendaUnshare")[0])
            compare(JSON.stringify(mock.calls[5]), JSON.stringify(["unshare", "cloud/swimming", "principal:principals/users/sam"]))
            const link = root.named(swimming, "agendaLink")[0]
            verify(link.visible && !link.checked, "the link off until asked")
        }

        // Someone else's: nothing to change, "Stop showing…" asked the same
        // way; its sharing read but not changed. This computer's: no server asked.
        function test_someone_elses_and_this_computers() {
            const setup = make()
            const sams = line(setup, 1)
            setup.toggle("cloud/sam")
            wait(30)
            verify(!root.named(sams, "agendaRename")[0].visible, "not yours to rename")
            compare(root.all(sams, c => c.objectName === "agendaColour" && c.visible).length, 0, "nor to recolour")
            const leave = root.named(sams, "agendaRemove")[0]
            compare(leave.text, "agenda-stop-showing")
            mock.agendaShared("cloud/sam", JSON.stringify({ line: "Sam can change it.", sharer: "nextcloud", people: [{ id: "principal:principals/users/sam", name: "Sam", access: "write", group: false }], link: null, links: true, note: "", said: "", problem: "" }))
            wait(30)
            compare(root.named(sams, "agendaSharingLine")[0].text, "Sam can change it.")
            verify(!root.named(sams, "agendaShareWith")[0].visible, "its sharing is its owner's")
            compare(root.all(sams, c => c.objectName === "agendaUnshare" && c.visible).length, 0)
            mouseClick(leave)
            wait(30)
            compare(root.named(sams, "agendaRemoveConfirm")[0].text, "agenda-leave-confirm")

            setup.toggle("local/notes")
            wait(30)
            compare(mock.asked, ["cloud/sam"], "an agenda on this computer asks no server")
            verify(!root.named(line(setup, 2), "agendaSharingLine")[0].visible)
        }

        // "New agenda…": its role sent with it; on Google, events only.
        function test_a_new_agenda_arrives_with_its_role() {
            const setup = make()
            mouseClick(root.named(setup, "agendaNew")[0])
            wait(30)
            const account = root.named(setup, "agendaNewAccount")[0]
            account.currentIndex = 1
            root.named(setup, "agendaNewName")[0].text = "Swimming"
            const tasks = root.all(setup, c => c.text === "agenda-new-tasks")[0]
            verify(!tasks.enabled, "a Google calendar holds events")
            const reaches = root.named(setup, "agendaNewReaches")[0]
            reaches.currentIndex = 1
            mouseClick(root.named(setup, "agendaNewMake")[0])
            const made = mock.calls[0]
            compare(made[0], "new")
            compare(JSON.stringify([made[1].account, made[1].name, made[1].events, made[1].tasks, made[1].whose, made[1].area, made[1].reaches]), JSON.stringify(["google", "Swimming", true, false, "yours", "admin+leisure", "outside-work"]))
            verify(!setup.making, "the form closed once made")
        }
    }
}
