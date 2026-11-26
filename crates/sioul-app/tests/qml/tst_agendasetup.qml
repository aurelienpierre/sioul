// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Agenda ⚙ ▸ Your agendas (AgendasSetup.qml, AgendaCard.qml; several
// agendas, part 5; at a glance since 26 November 2026) on a stand-in for
// Sioul: one row per agenda, every property in a column on a computer, one
// block per agenda on a phone, no fold; "Counts as busy time" turned on its
// row, never for another person's; who sees each one asked of its server
// once the page is drawn (its last answer shown meanwhile, a quiet mark while
// asked), never for one on this computer; its card: an agenda of yours on a
// server renamed, recoloured, and removed only after the sentence that says
// what goes, shared with a person or a group, stopped; someone else's offers
// "Stop showing…" and nothing to change. "New agenda…" sends its role with
// it, a Google one events only. Fiction only. Run by tools/qml-test.sh.

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
        signal agendasChanged
        signal personsChanged

        property var asked: []
        property var calls: []
        // What Copy put on the clipboard; whether the clipboard takes it.
        property var copied: []
        property bool clipboard: true
        // How often each view was read.
        property int setupReads: 0

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key + ":" + args }
        // The first agenda's person, when a test changes it.
        property string firstPerson: ""
        // The first agenda's sharing as its server last answered on this device, when a test says it.
        property var sharedWith: null
        function agendasSetup() {
            mock.setupReads++
            const view = JSON.parse(JSON.stringify(root.view))
            if (mock.firstPerson !== "")
                view.agendas[0].person = mock.firstPerson
            if (mock.sharedWith !== null)
                view.agendas[0].shared_with = mock.sharedWith
            return JSON.stringify(view)
        }
        function copyText(text) { mock.copied = mock.copied.concat([text]); return mock.clipboard }
        function setAgendaField(id, field, value) { mock.calls = mock.calls.concat([["field", id, field, value]]); return "" }
        function setAgendaBusy(id, counts) { mock.calls = mock.calls.concat([["busy", id, counts]]); return "" }
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

    readonly property var row: ({ colour: "#4c6b5c", events: true, tasks: false, whose: "yours", person: "", area: "admin+leisure", reaches: "all-day", access: "write", server_read_only: false, for_work: false, said: false, kept_hidden: false, parent: "", parent_address: "", parent_answers: "mail", standing: [], titles: false, busy_time: { counts: true, from: "server", server: true, pending: false, changeable: true, line: "" }, synced: "Synced at 09:12", synced_warn: false, shared_with: null })
    readonly property var view: ({
        agendas: [
            Object.assign({}, root.row, { id: "cloud/swimming", name: "Swimming", place: "cloud.example.org", server: { server: "caldav", server_name: "cloud.example.org", own: true } }),
            Object.assign({}, root.row, { id: "cloud/sam", name: "Sam’s week", place: "cloud.example.org", whose: "partner", person: "Sam", access: "read", server_read_only: true, server: { server: "caldav", server_name: "cloud.example.org", own: false }, busy_time: { counts: false, from: "theirs", server: true, pending: false, changeable: false, short: "agenda-busy-short-theirs", line: "agenda-busy-theirs" } }),
            Object.assign({}, root.row, { id: "local/notes", name: "Here", place: "", server: { server: "local", server_name: "this computer", own: true }, busy_time: { counts: true, from: "default", server: false, pending: false, changeable: true, line: "" }, synced: "On this device" }),
            Object.assign({}, root.row, { id: "cloud/busy-sam", name: "Alex, busy times", place: "cloud.example.org", mirror: true, kept_for: "For Sam", server: { server: "caldav", server_name: "cloud.example.org", own: true }, busy_time: { counts: false, from: "server", server: true, pending: false, changeable: false, short: "agenda-busy-short-mirror", line: "agenda-busy-mirror" } })
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

    // What is drawn wider than the page: none, on a computer or a phone.
    // The items of `item`, but inside the agendas' table, which scrolls sideways within its own
    // width rather than cut a value: the table itself counts, never what it scrolls.
    function outside(item, test, out) {
        out = out || []
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            if (child.objectName !== "agendaTable")
                root.outside(child, test, out)
        }
        return out
    }

    function wider(item) {
        return root.outside(item, c => c.visible && c.width > 0 && !(c instanceof Repeater) && c.mapToItem(item, 0, 0).x + c.width > item.width + 1)
    }

    TestCase {
        id: test

        name: "AgendasSetup"
        when: windowShown

        function init() {
            mock.asked = []
            mock.calls = []
            mock.copied = []
            mock.clipboard = true
            mock.firstPerson = ""
            mock.sharedWith = null
            root.width = 412
        }

        function make() {
            const setup = test.createTemporaryObject(setupComponent, root)
            test.waitForRendering(setup)
            return setup
        }

        // Each agenda's row, by its order.
        function line(setup, index) {
            return root.named(setup, "agendaSetup")[index]
        }

        // An agenda's card, opened: what it holds.
        function card(setup, id) {
            setup.openCard(id)
            const dialog = setup.card()
            tryCompare(dialog, "opened", true)
            test.waitForRendering(dialog.contentItem)
            return dialog.contentItem
        }

        // The card closed, and gone from over the window before the next test.
        function closeCard(setup) {
            const dialog = setup.card()
            if (dialog === null)
                return
            dialog.close()
            tryCompare(dialog, "visible", false)
        }

        // The colours offered over the page, once a row's colour is pressed.
        function colours() {
            return root.all(root.Overlay.overlay, c => c.objectName === "agendaColour" && c.visible)
        }

        function shownIn(item, name) {
            return root.named(item, name).filter(c => c.visible)
        }

        function test_every_property_at_a_glance_data() {
            return [{ tag: "computer", width: 1344, wide: true }, { tag: "phone", width: 412, wide: false }]
        }

        // One row per agenda, no fold: its name, account, whose, what for, when it reaches you, what
        // you may do, busy time, who sees it and when it was synced, a table on a computer, a block
        // on a phone; nothing drawn past the page's edge.
        function test_every_property_at_a_glance(data) {
            root.width = data.width
            const setup = make()
            compare(setup.wide, data.wide)
            compare(root.named(setup, "agendaHeadings")[0].visible, data.wide, "the columns' headings over the table alone")
            const swimming = line(setup, 0)
            compare(root.named(swimming, "agendaLineLabel")[0].text, "Swimming")
            compare(root.named(swimming, "agendaAccount")[0].text, "cloud.example.org")
            compare(root.named(swimming, "agendaRowWhose")[0].current, "yours")
            compare(root.named(swimming, "agendaRowWhose")[0].text, "agenda-whose-short-yours", "its few words; the full ones in its menu")
            compare(root.named(swimming, "agendaRowFor")[0].text, "agenda-area-short-admin + agenda-area-short-leisure")
            compare(root.named(swimming, "agendaRowAccess")[0].text, "agenda-access-short-write")
            compare(root.named(line(setup, 1), "agendaRowWhose")[0].text, "agenda-whose-short-person:" + JSON.stringify({ role: "agenda-whose-short-partner", name: "Sam" }))
            verify(root.named(swimming, "agendaRowReaches")[0].visible && root.named(swimming, "agendaRowAccess")[0].visible)
            verify(root.named(swimming, "agendaBusy")[0].checked)
            compare(root.named(swimming, "agendaRowSynced")[0].text, "Synced at 09:12")
            compare(root.named(line(setup, 2), "agendaRowShared")[0].text, "agenda-shared-here", "this computer's: your devices alone")
            compare(shownIn(setup, "agendaOpen").length, data.wide ? 4 : 0)
            compare(shownIn(setup, "agendaOpenNarrow").length, data.wide ? 0 : 4)
            compare(root.wider(setup).length, 0, "nothing wider than the page")
            // Changed in place: whose, what for, when it reaches you, what you may do.
            root.named(swimming, "agendaRowWhose")[0].choose(1)
            compare(JSON.stringify(mock.calls[mock.calls.length - 1]), JSON.stringify(["field", "cloud/swimming", "whose", "child"]))
            root.named(swimming, "agendaRowAccess")[0].choose(1)
            compare(JSON.stringify(mock.calls[mock.calls.length - 1]), JSON.stringify(["field", "cloud/swimming", "access", "read"]))
            root.named(swimming, "agendaRowFor")[0].choose(0)
            compare(JSON.stringify(mock.calls[mock.calls.length - 1]), JSON.stringify(["field", "cloud/swimming", "area", "work+admin+leisure"]))
        }

        function test_no_value_is_cut_data() {
            return [{ tag: "1600", width: 1600 }, { tag: "1280", width: 1280 }, { tag: "1020", width: 1020 }, { tag: "800", width: 800 }]
        }

        // Each value whole at a computer's widths (the coordinator's notes of 26 November 2026): each
        // column as wide as its widest value; narrower than the table, it scrolls sideways, never cuts.
        function test_no_value_is_cut(data) {
            root.width = data.width
            const setup = make()
            mock.agendaShared("cloud/swimming", JSON.stringify({ line: "Sam can read it.", sharer: "nextcloud", people: [{ id: "u/sam", name: "Sam", access: "read", group: false }], link: "https://x.example/l", links: true, note: "", said: "", problem: "" }))
            test.waitForRendering(setup)
            verify(setup.wide)
            const table = root.named(setup, "agendaTable")[0]
            compare(table.contentWidth, Math.max(table.width, setup.needed))
            compare(root.named(setup, "agendaTableScroll")[0].visible, setup.needed > table.width + 1, "a scroll bar only when the table is wider than the page")
            const cut = root.all(table, c => c.visible && c.truncated === true)
            compare(cut.map(c => c.text).join(" | "), "", "no value cut")
        }

        // "Counts as busy time": its one switch, turned on its row and sent; another person's
        // never, not to be turned, saying why; a calendar of busy blocks has none.
        function test_counts_as_busy_time_is_turned_on_its_row() {
            root.width = 1344
            const setup = make()
            const swimming = root.named(line(setup, 0), "agendaBusy")[0]
            verify(swimming.visible && swimming.enabled && swimming.checked)
            swimming.toggle()
            swimming.toggled()
            compare(JSON.stringify(mock.calls[mock.calls.length - 1]), JSON.stringify(["busy", "cloud/swimming", false]))
            verify(swimming.checked, "the row says what Sioul answers, read again")
            verify(!root.named(line(setup, 1), "agendaBusy")[0].visible, "another person's: never yours, no switch")
            compare(root.named(line(setup, 1), "agendaBusyLine")[0].text, "agenda-busy-short-theirs", "why, said in its cell")
            verify(!root.named(line(setup, 3), "agendaBusy")[0].visible, "a calendar of busy blocks: none")
            compare(root.named(line(setup, 3), "agendaBusyLine")[0].text, "agenda-busy-short-mirror", "never an empty cell")
            // Its server not told yet: said under the switch.
            setup.view = Object.assign({}, root.view, { agendas: [Object.assign({}, root.view.agendas[0], { busy_time: { counts: false, from: "word", server: true, pending: true, changeable: true, short: "agenda-busy-short-pending", line: "agenda-busy-pending" } })] })
            test.waitForRendering(setup)
            verify(root.named(line(setup, 0), "agendaBusy")[0].visible, "still turned")
            compare(root.named(line(setup, 0), "agendaBusyLine")[0].text, "agenda-busy-short-pending", "said in its cell in the table")
            root.width = 412
            test.waitForRendering(setup)
            const pending = root.named(line(setup, 0), "agendaBusyLine")[0]
            verify(pending.visible, "and under it in a block")
            compare(pending.text, "agenda-busy-pending")
        }

        // Nothing asked of a server before the page shows; then who sees each agenda on a server,
        // its last answer here shown meanwhile, a quiet mark while it is asked; never this computer's.
        function test_who_sees_it_comes_into_its_row() {
            root.width = 1344
            const answered = { line: "Sam can read it.", sharer: "nextcloud", people: [{ id: "principal:principals/users/sam", name: "Sam", access: "read", group: false }], link: null, links: true, note: "", said: "", problem: "" }
            const setup = test.createTemporaryObject(setupComponent, root)
            compare(mock.asked, [], "nothing asked as it is made")
            tryCompare(mock, "asked", ["cloud/swimming", "cloud/sam"])
            const swimming = line(setup, 0)
            compare(root.named(swimming, "agendaRowShared")[0].text, "", "nothing known yet")
            verify(root.named(swimming, "agendaRowReading")[0].visible, "a quiet mark while asked")
            mock.agendaShared("cloud/swimming", JSON.stringify(answered))
            compare(root.named(swimming, "agendaRowShared")[0].text, "Sam", "its names, the sentence as its tooltip")
            compare(root.named(swimming, "agendaRowShared")[0].ToolTip.text, "Sam can read it.")
            mock.agendaShared("cloud/swimming", JSON.stringify(Object.assign({}, answered, { people: [], link: "https://cloud.example.org/link" })))
            compare(root.named(swimming, "agendaRowShared")[0].text, "agenda-shared-link-alone")
            mock.agendaShared("cloud/swimming", JSON.stringify(Object.assign({}, answered, { people: [] })))
            compare(root.named(swimming, "agendaRowShared")[0].text, "agenda-shared-only-you")
            mock.agendaShared("cloud/swimming", JSON.stringify(answered))
            verify(!root.named(swimming, "agendaRowReading")[0].visible)
            // Opened again: its last answer on this device at once, asked anew all the same.
            mock.asked = []
            mock.sharedWith = answered
            const again = make()
            compare(root.named(line(again, 0), "agendaRowShared")[0].text, "Sam")
            tryCompare(mock, "asked", ["cloud/swimming", "cloud/sam"])
            verify(root.named(line(again, 0), "agendaRowReading")[0].visible)
        }

        // Yours, on its server: recoloured on its row; its card renames, removes after the
        // sentence, shares with a person or a group, and stops.
        function test_an_agenda_of_yours_on_its_server() {
            const setup = make()
            tryCompare(mock, "asked", ["cloud/swimming", "cloud/sam"])
            mouseClick(root.named(line(setup, 0), "agendaRowColour")[0])
            tryVerify(() => colours().length === 8, 1000, "its colour, chosen in place among eight")
            mouseClick(colours()[2])
            compare(JSON.stringify(mock.calls[0]), JSON.stringify(["colour", "cloud/swimming", "#7a5c99"]))
            tryVerify(() => colours().length === 0, 1000, "the colours close once one is chosen")

            const swimming = card(setup, "cloud/swimming")
            compare(shownIn(swimming, "agendaSharingLine")[0].text, "agenda-sharing-reading")
            compare(shownIn(swimming, "agendaColour").length, 0, "the colour is its row's alone")
            const rename = shownIn(swimming, "agendaRename")[0]
            rename.text = "Swimming club"
            rename.editingFinished()
            compare(JSON.stringify(mock.calls[1]), JSON.stringify(["rename", "cloud/swimming", "Swimming club"]))

            // Removed only on the second press, the sentence said first.
            const remove = shownIn(swimming, "agendaRemove")[0]
            compare(remove.text, "agenda-remove")
            mouseClick(remove)
            compare(JSON.stringify(mock.calls[2]), JSON.stringify(["removal", "cloud/swimming"]))
            compare(shownIn(swimming, "agendaRemoveQuestion")[0].text, "Remove “Swimming”, 42 events, from cloud.example.org?")
            verify(!remove.visible, "the question in its place")
            compare(mock.calls.length, 3, "nothing removed yet")
            test.waitForRendering(swimming)
            mouseClick(shownIn(swimming, "agendaRemoveConfirm")[0])
            compare(JSON.stringify(mock.calls[3]), JSON.stringify(["remove", "cloud/swimming"]))

            // Its server answers: who sees it, and what may be done.
            setup.openCard("cloud/swimming")
            mock.agendaShared("cloud/swimming", JSON.stringify({ line: "Sam can read it.", sharer: "nextcloud", people: [{ id: "principal:principals/users/sam", name: "Sam", access: "read", group: false }], link: null, links: true, note: "People and groups of this Nextcloud alone.", said: "", problem: "" }))
            compare(shownIn(swimming, "agendaSharingLine")[0].text, "Sam can read it.")
            const group = shownIn(swimming, "agendaShareGroup")[0]
            verify(group !== undefined, "a group of the same Nextcloud")
            const field = shownIn(swimming, "agendaShareWith")[0]
            field.text = "family"
            group.checked = true
            mouseClick(shownIn(swimming, "agendaShare")[0])
            compare(JSON.stringify(mock.calls[4]), JSON.stringify(["share", "cloud/swimming", "group:family", "read"]))
            compare(field.text, "", "the field emptied for the next one")
            test.waitForRendering(swimming)
            mouseClick(shownIn(swimming, "agendaUnshare")[0])
            compare(JSON.stringify(mock.calls[5]), JSON.stringify(["unshare", "cloud/swimming", "principal:principals/users/sam"]))
            const link = shownIn(swimming, "agendaLink")[0]
            verify(link !== undefined && !link.checked, "the link off until asked")
            closeCard(setup)
        }

        // A list of tasks alone, yours or someone else's: renamed, removed or stopped showing in
        // Tasks ⚙ ▸ Task lists, its one place (24 November 2026, tst_tasklists.qml), so none of it
        // here; a calendar that also takes tasks is.
        function test_a_list_of_tasks_alone_is_renamed_in_tasks() {
            const setup = make()
            const tasks = { counts: false, from: "default", server: false, pending: false, changeable: false, short: "agenda-busy-short-tasks", line: "agenda-busy-tasks" }
            setup.view = Object.assign({}, root.view, { agendas: [
                Object.assign({}, root.row, { id: "cloud/chores", name: "Chores", place: "cloud.example.org", events: false, tasks: true, busy_time: tasks, server: { server: "caldav", server_name: "cloud.example.org", own: true } }),
                Object.assign({}, root.row, { id: "cloud/home", name: "Home", place: "cloud.example.org", events: true, tasks: true, server: { server: "caldav", server_name: "cloud.example.org", own: true } }),
                Object.assign({}, root.row, { id: "cloud/sams-chores", name: "Sam’s chores", place: "cloud.example.org", whose: "shared", events: false, tasks: true, server: { server: "caldav", server_name: "cloud.example.org", own: false } })
            ] })
            test.waitForRendering(setup)
            verify(!root.named(line(setup, 0), "agendaBusy")[0].visible, "a list of tasks alone: no busy time")
            compare(root.named(line(setup, 0), "agendaBusyLine")[0].text, "agenda-busy-tasks", "said why, whole on a phone")
            const chores = card(setup, "cloud/chores")
            verify(shownIn(chores, "agendaRename").length === 0 && shownIn(chores, "agendaRemove").length === 0, "Tasks ⚙'s")
            const home = card(setup, "cloud/home")
            verify(shownIn(home, "agendaRename").length === 1 && shownIn(home, "agendaRemove").length === 1, "a calendar that takes tasks too: here")
            const sams = card(setup, "cloud/sams-chores")
            compare(shownIn(sams, "agendaRemove").length, 0, "someone else's list of tasks: stopped showing in Tasks ⚙")
            closeCard(setup)
        }

        // Someone else's: nothing to change, "Stop showing…" asked the same
        // way; its sharing read but not changed. This computer's: no server asked.
        function test_someone_elses_and_this_computers() {
            const setup = make()
            tryCompare(mock, "asked", ["cloud/swimming", "cloud/sam"])
            mouseClick(root.named(line(setup, 1), "agendaRowColour")[0])
            wait(50)
            compare(colours().length, 0, "not yours to recolour")
            const sams = card(setup, "cloud/sam")
            compare(shownIn(sams, "agendaRename").length, 0, "not yours to rename")
            const leave = shownIn(sams, "agendaRemove")[0]
            compare(leave.text, "agenda-stop-showing")
            mock.agendaShared("cloud/sam", JSON.stringify({ line: "Sam can change it.", sharer: "nextcloud", people: [{ id: "principal:principals/users/sam", name: "Sam", access: "write", group: false }], link: null, links: true, note: "", said: "", problem: "" }))
            compare(shownIn(sams, "agendaSharingLine")[0].text, "Sam can change it.")
            compare(shownIn(sams, "agendaShareWith").length, 0, "its sharing is its owner's")
            compare(shownIn(sams, "agendaUnshare").length, 0)
            mouseClick(leave)
            compare(shownIn(sams, "agendaRemoveConfirm")[0].text, "agenda-leave-confirm")

            const here = card(setup, "local/notes")
            compare(mock.asked, ["cloud/swimming", "cloud/sam"], "an agenda on this computer asks no server")
            compare(shownIn(here, "agendaSharingLine").length, 0)
            closeCard(setup)
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

        // A read-only link turned on: the link itself, selectable, and Copy,
        // which says "Copied." once the clipboard holds it; one the clipboard
        // refused is selected, to copy by hand (5 November 2026).
        function test_an_agenda_s_link_shown_and_copied() {
            const setup = make()
            const swimming = card(setup, "cloud/swimming")
            const link = "https://cloud.example.org/remote.php/dav/public-calendars/AbC123?export"
            mock.agendaShared("cloud/swimming", JSON.stringify({ line: "agenda-sharing-link", sharer: "nextcloud", people: [], link: link, links: true, note: "", said: "", problem: "" }))
            const shown = shownIn(swimming, "agendaLinkLine")[0]
            verify(shown !== undefined, "the link under its switch")
            const field = root.named(shown, "linkField")[0]
            compare(field.text, link)
            verify(field.readOnly && field.selectByMouse, "read only, and it selects")
            const copy = root.named(shown, "linkCopy")[0]
            compare(copy.text, "ui-copy")
            mouseClick(copy)
            compare(mock.copied, [link])
            compare(copy.text, "ui-copied", "said once it is on the clipboard")
            tryCompare(copy, "text", "ui-copy", 3000)
            mock.clipboard = false
            mouseClick(copy)
            compare(copy.text, "ui-copy", "nothing said when the clipboard refused it")
            compare(field.selectedText, link, "selected, to copy by hand")
            closeCard(setup)
        }

        // A calendar of busy blocks Sioul keeps: said kept for its availability, nothing of its
        // own to choose; its card opens Your availability, and it asks no server who sees it.
        function test_a_mirror_says_what_it_is_kept_for_and_opens_your_availability() {
            const setup = make()
            const mirror = line(setup, 3)
            compare(root.named(mirror, "agendaKeptFor")[0].text, "agenda-mirror-kept-for:For Sam")
            verify(!root.named(mirror, "agendaRowWhose")[0].visible, "no role to choose")
            let asked = 0
            setup.availabilityAsked.connect(() => asked++)
            const kept = card(setup, "cloud/busy-sam")
            mouseClick(shownIn(kept, "agendaMirrorOpen")[0])
            compare(asked, 1)
            tryCompare(setup.card(), "visible", false, 1000, "Your availability shown: the card closes")
            compare(root.named(kept, "agendaWhose").filter(c => c.visible).length, 0, "no role to choose in its card either")
            tryCompare(mock, "asked", ["cloud/swimming", "cloud/sam"])
        }

        // An agenda or a person changed elsewhere (its role's person made, a
        // sync): read again at once; the same answer rebuilds nothing.
        function test_a_change_elsewhere_is_read_at_once() {
            const setup = make()
            const before = mock.setupReads
            const first = line(setup, 0)
            mock.personsChanged()
            compare(mock.setupReads, before + 1, "read again")
            compare(line(setup, 0), first, "the same answer: the same rows")
            mock.agendasChanged()
            compare(mock.setupReads, before + 2)
            mock.firstPerson = "Léa"
            mock.personsChanged()
            verify(line(setup, 0) !== first, "a new answer: shown")
        }
    }
}
