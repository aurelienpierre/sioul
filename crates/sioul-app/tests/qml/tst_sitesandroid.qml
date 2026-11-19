// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Android's Sites page (qml/android/SitesPage.qml), on a stand-in for Sioul,
// at a phone's size (412 × 891): the computer's list, menu and forms
// (SitesList.qml, SiteRow.qml, SiteMenu.qml, SiteEditDialog.qml,
// SiteAddDialog.qml). Each line says where its site opens; the sites for
// other hours fold under one line; the filter and the order by type; a tap
// opens the site through Rust (`siteOpen`, with Sioul's colours), in Sioul's
// own screen or in the browser, its news seen either way, and what went
// wrong said; the menu without what only Qt WebEngine carries (kept open,
// the call switches, the devices, another login), a move, the type; a site
// added from a usual site, opened once pinned; a site changed; a site
// removed; a notice's click handled by the site alone. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"
import "../../qml/android" as Android

Item {
    id: root

    width: 412
    height: 891

    Theme {
        id: testTheme

        touch: true
        compact: true
    }

    QtObject {
        id: mock

        property var asked: []
        property string status: ""
        // What `siteOpen` answers next.
        property var answer: ({ here: true })
        property var given: [
            { id: "bank", name: "My bank", url: "https://bank.example.org", kind: "mailbox", categories: ["Money"], area: "admin", in_view: true, news: true, muted: false, icon: "", opens_line: "Opens in Sioul." },
            { id: "chat", name: "Chat", url: "https://chat.example.org", kind: "chat", categories: [], area: "leisure", in_view: true, news: false, muted: true, icon: "", opens_line: "Opens in Sioul." },
            { id: "work", name: "Work tracker", url: "https://work.example.org", kind: "other", categories: [], area: "work", in_view: false, news: false, muted: false, icon: "", opens_line: "Opens in Sioul." }
        ]

        signal sitesChanged()

        function note(what) {
            mock.asked = mock.asked.concat([what])
        }
        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function viewFlag(key) {
            return false
        }
        function setViewFlag(key, on) {
            mock.note("flag " + key + " " + on)
        }
        function siteList() {
            return JSON.stringify(mock.given)
        }
        function siteOpen(id, look) {
            mock.note("open " + id + " " + look)
            return JSON.stringify(mock.answer)
        }
        function siteSeen(id) {
            mock.note("seen " + id)
        }
        function setSite(id, field, value) {
            mock.note("set " + id + " " + field + " " + value)
            return ""
        }
        function setSiteFor(id, person) {
            mock.note("for " + id + " " + person)
            return ""
        }
        function sitePerson(id) {
            return "null"
        }
        function household() {
            return JSON.stringify({ persons: [{ id: "me", name: "Me" }] })
        }
        function moveSite(id, delta) {
            mock.note("move " + id + " " + delta)
            return ""
        }
        function removeSite(id) {
            mock.note("remove " + id)
            return ""
        }
        function sitePresets(country, region) {
            return JSON.stringify({ country: "", region: "", countries: [], types: ["mailbox"], categories: [], sites: [{ name: "Health insurance", url: "https://health.example.org", host: "health.example.org", kind: "mailbox", categories: ["Health"], area: "admin", about: "", announced_by: ["example.org"], calls: null, kept: false }] })
        }
        function sitePresetsTree() {
            return JSON.stringify({ countries: [] })
        }
        function addSite(edit) {
            mock.note("add " + edit)
            return JSON.stringify({ id: "health" })
        }
        function addSites(rows) {
            return JSON.stringify({ added: 0, error: "" })
        }
    }

    QtObject {
        id: fakeWindow

        property var menuAt: null

        function openChain(uri) {
            mock.note("chain " + uri)
        }
        function linkFrom(thing) {
            mock.note("link " + thing.uri)
        }
    }

    Android.SitesPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
        window: fakeWindow
        outside: address => mock.note("browser " + address)
    }

    TestCase {
        name: "SitesAndroid"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; i++)
                walk(kids[i], test, found)
            if (item.contentItem && item.contentItem !== item && kids.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function showing(item) {
            for (let at = item; at; at = at.parent) {
                if (at.visible === false)
                    return false
            }
            return true
        }
        function rows() {
            return walk(page, i => i.hasOwnProperty("kindIcons") && i.hasOwnProperty("where") && showing(i), [])
        }
        function list() {
            return walk(page, i => i.hasOwnProperty("filterLines"), [])[0]
        }
        function texts(item) {
            return walk(item, i => i.hasOwnProperty("text") && typeof i.text === "string" && showing(i), []).map(i => i.text)
        }

        function init() {
            mock.asked = []
            mock.answer = { here: true }
            page.problem = ""
            list().kindFilter = ""
            list().byCategory = false
            list().laterShown = false
            wait(20)
        }

        // Each line names its site and where it opens; the sites for other hours fold.
        function test_the_lines_say_where_each_site_opens() {
            const shown = rows()
            compare(shown.map(r => r.site.id), ["bank", "chat"], "the sites for these hours")
            compare(shown[0].where, "Opens in Sioul.")
            verify(texts(page).indexOf("site-later:1  ▸") >= 0, "the others folded: " + texts(page))
            verify(texts(page).indexOf("site-android") >= 0, "the page says what Android does and does not")
            list().laterShown = true
            wait(20)
            compare(rows().map(r => r.site.id), ["bank", "chat", "work"])
            verify(rows()[2].later, "quieter")
        }

        // The filter by type, and the order by type with each type named once.
        function test_the_filter_and_the_order() {
            list().kindFilter = "chat"
            wait(20)
            compare(rows().map(r => r.site.id), ["chat"])
            verify(texts(page).indexOf("site-kind-chat") >= 0, "the filter said in words")
            list().kindFilter = ""
            list().byCategory = true
            wait(20)
            const said = texts(page)
            verify(said.indexOf("site-kind-mailbox") >= 0 && said.indexOf("site-kind-chat") >= 0, "each type named: " + said)
        }

        // A tap: Rust decides, in Sioul's own screen or in the browser; its news seen; Sioul's colours given.
        function test_a_tap_opens_the_site_where_rust_says() {
            rows()[0].clicked()
            const opened = mock.asked[0]
            verify(opened.indexOf("open bank ") === 0, opened)
            const look = JSON.parse(opened.slice("open bank ".length))
            compare(look.dark, testTheme.dark)
            compare(look.background, String(testTheme.background))
            verify(look.text !== undefined && look.line !== undefined && look.surface !== undefined)
            compare(mock.asked.slice(1), ["seen bank"], "in Sioul: nothing else")
            mock.asked = []
            mock.answer = { browser: "https://chat.example.org", problem: "site-screen-refused" }
            page.open("chat")
            compare(mock.asked.slice(1), ["browser https://chat.example.org", "seen chat"])
            compare(page.problem, "site-screen-refused", "why it opened in the browser")
            mock.asked = []
            mock.answer = { problem: "site-gone" }
            page.open("gone")
            compare(mock.asked.length, 1, "a site gone: nothing opened, nothing seen")
            compare(page.problem, "site-gone")
            compare(page.clickNotice("bank", "tag"), false, "a notice's click: the site alone")
        }

        // The menu: what only a computer's page carries is left out; a move, a type, a link.
        function test_the_menu_leaves_out_what_only_a_computer_carries() {
            page.menuId = "chat"
            const menu = page.siteMenu
            menu.popup()
            wait(50)
            const labels = []
            for (let i = 0; i < menu.count; i++) {
                const item = menu.itemAt(i)
                if (item && item.visible && item.text)
                    labels.push(item.text)
            }
            for (const gone of ["site-background", "site-microphone", "site-camera", "site-screen", "site-devices…", "bitwarden-choose"])
                verify(labels.indexOf(gone) < 0, gone + " in " + labels)
            for (const kept of ["site-kind-chat", "chain-its-ties", "site-move-up", "site-move-down", "site-link", "site-edit", "site-remove"])
                verify(labels.indexOf(kept) >= 0, kept + " not in " + labels)
            menu.dismiss()
            menu.set("site", "video")
            compare(mock.asked[mock.asked.length - 1], "set chat site video")
            compare(menu.index, 1)
            compare(menu.total, 3)
        }

        // A usual site taken, pinned, then opened; a site changed; a site removed.
        function test_add_change_and_remove() {
            const listed = list()
            listed.addAsked()
            wait(50)
            const adding = walk(root, i => i.hasOwnProperty("shownPresets"), [])[0] || page.children.map(c => c.item).find(i => i && i.shownPresets !== undefined)
            verify(adding, "the form made at first use")
            verify(adding.visible)
            compare(adding.shownPresets.length, 1)
            adding.take(adding.shownPresets[0])
            adding.save()
            const added = mock.asked.find(a => a.indexOf("add ") === 0)
            const edit = JSON.parse(added.slice(4))
            compare([edit.name, edit.url, edit.kind, edit.area], ["Health insurance", "https://health.example.org", "mailbox", "admin"])
            verify(mock.asked.some(a => a.indexOf("open health ") === 0), "opened once pinned")
            // Changed: every field written, whom it is for.
            mock.asked = []
            page.menuId = "bank"
            page.siteMenu.editAsked()
            wait(50)
            const editing = page.children.map(c => c.item).find(i => i && i.hasOwnProperty("persons"))
            verify(editing && editing.visible)
            editing.save()
            for (const field of ["name My bank", "url https://bank.example.org", "site mailbox", "categories [\"Money\"]"])
                verify(mock.asked.indexOf("set bank " + field) >= 0, field + " in " + mock.asked)
            verify(mock.asked.indexOf("for bank ") >= 0)
            verify(!editing.visible, "saved, it closes")
            // Removed, once asked.
            page.siteMenu.removeAsked()
            wait(50)
            const removing = page.children.map(c => c.item).find(i => i && i.hasOwnProperty("commands"))
            verify(removing && removing.visible)
            removing.confirmed()
            verify(mock.asked.indexOf("remove bank") >= 0)
        }
    }
}
