// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Bitwarden login chooser (LoginChooser.qml), on a stand-in for Sioul.
// The search's own rules are tested in sioul-sync (bitwarden.rs, the fixture
// vault); here the window's side: what the two fields hold when it opens, the
// pause before searching, Tab, Enter, Escape, the arrows, the buttons that
// empty a field, the status line, the warning for another domain, the layout
// at 1000 and 412 px, in English and French. Run by tools/qml-test.sh. To see
// the dialog, set `pictures` below to a folder (a change of your own, not to
// commit): each step saves its picture there.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    // Where the pictures go; none when empty.
    readonly property string pictures: ""

    width: 1000
    height: 700

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string language: "en"
        property int searches: 0
        property var last: []
        property int more: 0
        property string chosenDomain: ""
        // Made up: no real vault, no real person.
        property var logins: [
            { id: "a", name: "Ameli", username: "member-0042", site: "ameli.fr", elsewhere: false },
            { id: "b", name: "Compte assuré", username: "member-0042", site: "assure.ameli.fr", elsewhere: false },
            { id: "g", name: "Google", username: "aurore@example.com", site: "accounts.google.com", elsewhere: true },
            { id: "f", name: "Forum de photo", username: "aurore@example.com", site: "forum.photo.test", elsewhere: true }
        ]
        readonly property var words: ({
            "en": {
                "bitwarden-choose-title": "Which login?",
                "bitwarden-choose-by-site": "Site",
                "bitwarden-choose-by-site-hint": "A domain or a word",
                "bitwarden-choose-by-user": "User name",
                "bitwarden-choose-by-user-hint": "Any part of it",
                "bitwarden-choose-empty": "Empty this field",
                "bitwarden-choose-ask": "Type a site, a user name, or both.",
                "bitwarden-choose-nothing": "Nothing found.",
                "bitwarden-choose-nothing-both": "No login has both: empty one of the fields to see more.",
                "bitwarden-choose-more": "{count} more logins: narrow the search.",
                "bitwarden-choose-other-site": "for {site}",
                "bitwarden-choose-fill": "Fill",
                "ui-cancel": "Cancel"
            },
            "fr": {
                "bitwarden-choose-title": "Quel identifiant ?",
                "bitwarden-choose-by-site": "Site",
                "bitwarden-choose-by-site-hint": "Un domaine ou un mot",
                "bitwarden-choose-by-user": "Nom d’utilisateur",
                "bitwarden-choose-by-user-hint": "Une partie suffit",
                "bitwarden-choose-empty": "Vider ce champ",
                "bitwarden-choose-ask": "Tapez un site, un nom d’utilisateur, ou les deux.",
                "bitwarden-choose-nothing": "Rien trouvé.",
                "bitwarden-choose-nothing-both": "Aucun identifiant n’a les deux : videz l’un des champs pour en voir plus.",
                "bitwarden-choose-more": "Encore {count} identifiants : précisez la recherche.",
                "bitwarden-choose-other-site": "pour {site}",
                "bitwarden-choose-fill": "Remplir",
                "ui-cancel": "Annuler"
            }
        })

        function text(key) {
            return mock.words[mock.language][key] || key
        }
        function textWith(key, name, value) {
            return mock.text(key).replace("{" + name + "}", value)
        }
        function bitwardenSite(url) {
            return JSON.stringify({ domain: "ameli.fr", chosen: mock.chosenDomain, only: "" })
        }
        // A large vault: answered later, by `bitwardenFound`, the newest first
        // (answers may come back out of order).
        property bool large: false
        property int tickets: 0
        property var later: []

        signal bitwardenFound(int ticket, string found)

        // A naive stand-in: the real rules are sioul-sync's.
        function answer(site, user) {
            if (site === "" && user === "")
                return JSON.stringify({ found: [], more: 0 })
            const s = site.toLowerCase()
            const u = user.toLowerCase()
            const found = mock.logins.filter(l => (s === "" || l.site.indexOf(s) >= 0 || l.name.toLowerCase().indexOf(s) >= 0) && (u === "" || l.username.toLowerCase().indexOf(u) >= 0))
            return JSON.stringify({ found: found, more: mock.more })
        }
        function bitwardenSearch(url, site, user) {
            mock.searches += 1
            mock.last = [url, site, user]
            if (!mock.large)
                return mock.answer(site, user)
            mock.tickets += 1
            mock.later = mock.later.concat([[mock.tickets, mock.answer(site, user)]])
            slow.restart()
            return JSON.stringify({ later: mock.tickets })
        }
    }

    Timer {
        id: slow

        interval: 80
        onTriggered: {
            const all = mock.later.slice().reverse()
            mock.later = []
            for (const [ticket, found] of all)
                mock.bitwardenFound(ticket, found)
        }
    }

    LoginChooser {
        id: chooser

        sioul: mock
        theme: testTheme
    }

    SignalSpy {
        id: chosenSpy

        target: chooser
        signalName: "chosen"
    }

    TestCase {
        name: "LoginChooser"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                walk(children[i], test, found)
            if (item.contentItem && item.contentItem !== item && children.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function popupItem() {
            return chooser.contentItem.parent
        }
        function fieldWith(hint) {
            return walk(popupItem(), i => i.placeholderText !== undefined && i.placeholderText === mock.text(hint), [])[0] || null
        }
        function siteField() {
            return fieldWith("bitwarden-choose-by-site-hint")
        }
        function userField() {
            return fieldWith("bitwarden-choose-by-user-hint")
        }
        // The results: in the dialog's content, not the footer's row of buttons (a ListView too).
        function listView() {
            return walk(chooser.contentItem, i => i.currentIndex !== undefined && i.contentHeight !== undefined && i.model !== undefined, [])[0] || null
        }
        function emptyButtonOf(field) {
            return walk(field, i => i !== field && i.icon !== undefined && i.icon.name === "edit-clear", [])[0] || null
        }
        function statusLine() {
            return walk(popupItem(), i => i.wrapMode !== undefined && i.wrapMode === Text.Wrap && i.text !== undefined, [])[0] || null
        }
        function labelsWith(text) {
            return walk(popupItem(), i => i.text !== undefined && typeof i.text === "string" && i.text.indexOf(text) >= 0 && i.font !== undefined && i.placeholderText === undefined, [])
        }
        function typeText(text) {
            for (let i = 0; i < text.length; ++i)
                keyClick(text[i])
        }
        function save(name) {
            if (root.pictures === "")
                return
            wait(100)
            grabImage(root).save(root.pictures + "/" + name + ".png")
        }

        function init() {
            mock.language = "en"
            mock.large = false
            mock.more = 0
            mock.chosenDomain = ""
            chooser.warnElsewhere = true
            chosenSpy.clear()
            root.width = 1000
            wait(30)
        }
        function cleanup() {
            chooser.close()
            tryCompare(chooser, "visible", false, 2000)
        }

        function test_a_site_opens_on_its_domain() {
            const before = mock.searches
            chooser.begin("https://assure.ameli.fr/PortailAS/x")
            tryCompare(chooser, "opened", true)
            compare(chooser.site, "ameli.fr")
            compare(chooser.user, "")
            compare(mock.searches, before + 1, "searched at once")
            compare(mock.last, ["https://assure.ameli.fr/PortailAS/x", "ameli.fr", ""])
            verify(siteField().activeFocus, "Site has the focus")
            compare(siteField().selectedText, "ameli.fr", "selected: typing replaces it")
            compare(listView().count, 2)
            save("site-desktop-en")
        }

        function test_the_login_chosen_there_last_sets_the_site() {
            mock.chosenDomain = "google.com"
            chooser.begin("https://ameli.fr")
            compare(chooser.site, "google.com")
        }

        function test_an_account_opens_on_its_address() {
            chooser.warnElsewhere = false
            chooser.beginForAccount("https://imap.gmail.com", "aurore@example.com")
            tryCompare(chooser, "opened", true)
            compare(chooser.site, "", "a mail server's domain is not put in Site")
            compare(chooser.user, "aurore@example.com")
            compare(mock.last, ["https://imap.gmail.com", "", "aurore@example.com"])
            compare(listView().count, 2)
            compare(labelsWith("for accounts.google.com").length, 0, "no warning for an account's server")
            verify(labelsWith("accounts.google.com").length > 0, "the site shown")
            save("account-desktop-en")
            // Chosen for that server before: its domain in Site.
            chooser.close()
            tryCompare(chooser, "visible", false)
            mock.chosenDomain = "google.com"
            chooser.beginForAccount("https://imap.gmail.com", "aurore@example.com")
            compare(chooser.site, "google.com")
        }

        function test_typing_waits_for_a_pause() {
            chooser.begin("https://ameli.fr")
            const before = mock.searches
            typeText("goo")
            wait(60)
            compare(mock.searches, before, "nothing searched while typing")
            tryCompare(mock, "searches", before + 1, 1000)
            compare(mock.last[1], "goo")
            wait(300)
            compare(mock.searches, before + 1, "searched once")
        }

        function test_enter_takes_the_first() {
            chooser.begin("https://ameli.fr")
            keyClick(Qt.Key_Return)
            compare(chosenSpy.count, 1)
            compare(chosenSpy.signalArguments[0][0], "a")
            tryCompare(chooser, "visible", false)
        }

        function test_enter_right_after_typing_takes_what_was_typed() {
            chooser.begin("https://ameli.fr")
            typeText("google")
            keyClick(Qt.Key_Return)
            compare(chosenSpy.count, 1)
            compare(chosenSpy.signalArguments[0][0], "g", "not the first of the list before the pause")
        }

        function test_arrows_then_enter() {
            chooser.begin("https://ameli.fr")
            keyClick(Qt.Key_Down)
            compare(listView().currentIndex, 1)
            keyClick(Qt.Key_Down)
            compare(listView().currentIndex, 1, "no further than the last")
            keyClick(Qt.Key_Up)
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Return)
            compare(chosenSpy.signalArguments[0][0], "b")
        }

        function test_tab_between_the_fields() {
            chooser.begin("https://ameli.fr")
            verify(siteField().activeFocus)
            keyClick(Qt.Key_Tab)
            verify(userField().activeFocus, "Tab: User name")
            keyClick(Qt.Key_Tab)
            verify(siteField().activeFocus, "Tab again: Site")
            keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
            verify(userField().activeFocus, "Shift+Tab: User name")
            typeText("aurore")
            keyClick(Qt.Key_Return)
            compare(chosenSpy.count, 0, "Site ameli.fr and user aurore: nothing to take")
            verify(chooser.opened)
        }

        function test_escape_closes() {
            chooser.begin("https://ameli.fr")
            verify(chooser.opened)
            keyClick(Qt.Key_Escape)
            tryCompare(chooser, "visible", false)
            compare(chosenSpy.count, 0)
            // From the other field too.
            chooser.begin("https://ameli.fr")
            keyClick(Qt.Key_Tab)
            verify(userField().activeFocus)
            keyClick(Qt.Key_Escape)
            tryCompare(chooser, "visible", false)
        }

        function test_the_buttons_that_empty_a_field() {
            chooser.begin("https://ameli.fr")
            const siteEmpty = emptyButtonOf(siteField())
            const userEmpty = emptyButtonOf(userField())
            verify(siteEmpty !== null && userEmpty !== null)
            verify(siteEmpty.visible)
            verify(!userEmpty.visible, "nothing to empty")
            compare(siteEmpty.focusPolicy, Qt.NoFocus, "not a Tab stop")
            mouseClick(siteEmpty)
            compare(chooser.site, "")
            verify(siteField().activeFocus, "the focus back in the field")
            verify(!siteEmpty.visible)
            tryVerify(() => mock.last[1] === "" && mock.last[2] === "", 1000, "searched again, both fields empty")
            compare(statusLine().text, mock.text("bitwarden-choose-ask"))
            compare(listView().count, 0)
        }

        function test_status_lines() {
            chooser.begin("https://ameli.fr")
            verify(!statusLine().visible, "nothing to say over the site's logins")
            chooser.site = "nothing-here"
            chooser.search()
            compare(statusLine().text, mock.text("bitwarden-choose-nothing"))
            chooser.user = "x"
            chooser.search()
            compare(statusLine().text, mock.text("bitwarden-choose-nothing-both"))
            mock.more = 3
            chooser.site = "ameli"
            chooser.user = ""
            chooser.search()
            compare(statusLine().text, "3 more logins: narrow the search.")
        }

        function test_another_domain_is_said() {
            chooser.begin("https://ameli.fr")
            chooser.site = ""
            chooser.user = "aurore"
            chooser.search()
            compare(listView().count, 2)
            const warned = labelsWith("for accounts.google.com")
            compare(warned.length, 1)
            compare(String(warned[0].color), String(testTheme.warm))
            const own = labelsWith("ameli.fr")
            compare(own.length, 0, "the list holds Gmail's two, no ameli.fr")
            save("user-desktop-en")
            chooser.warnElsewhere = false
            wait(30)
            compare(labelsWith("for accounts.google.com").length, 0)
        }

        function test_a_large_vault_answers_later() {
            mock.large = true
            chooser.begin("https://ameli.fr")
            tryCompare(chooser, "opened", true)
            compare(listView().count, 0, "nothing yet")
            verify(!statusLine().visible, "no 'nothing found' while it is searched")
            tryCompare(listView(), "count", 2, 2000)
            compare(chooser.waiting, 0)
            // Two searches in flight, their answers back in the wrong order: the newest stays.
            chooser.site = "ameli"
            chooser.search()
            chooser.site = "google"
            chooser.search()
            tryCompare(chooser, "waiting", 0, 2000)
            wait(150)
            compare(listView().count, 1)
            keyClick(Qt.Key_Return)
            compare(chosenSpy.signalArguments[0][0], "g")
        }

        function test_enter_before_a_large_vault_answers() {
            mock.large = true
            chooser.begin("https://ameli.fr")
            tryCompare(listView(), "count", 2, 2000)
            typeText("google")
            keyClick(Qt.Key_Return)
            compare(chosenSpy.count, 0, "not yet: still searched")
            verify(chooser.opened)
            tryCompare(chosenSpy, "count", 1, 2000)
            compare(chosenSpy.signalArguments[0][0], "g", "the first of what was typed")
        }

        function test_closed_before_a_large_vault_answers() {
            mock.large = true
            chooser.begin("https://ameli.fr")
            tryCompare(listView(), "count", 2, 2000)
            typeText("google")
            keyClick(Qt.Key_Return)
            keyClick(Qt.Key_Escape)
            tryCompare(chooser, "visible", false)
            wait(300)
            compare(chosenSpy.count, 0, "closed first: nothing filled")
        }

        function test_side_by_side_then_stacked_on_a_phone() {
            for (const language of ["en", "fr"]) {
                mock.language = language
                root.width = 1000
                chooser.begin("https://ameli.fr")
                tryCompare(chooser, "opened", true)
                wait(50)
                let s = siteField().mapToItem(root, 0, 0)
                let u = userField().mapToItem(root, 0, 0)
                compare(Math.round(s.y), Math.round(u.y), "side by side (" + language + ")")
                verify(u.x >= s.x + siteField().width, "User name right of Site")
                compare(Math.round(siteField().width), Math.round(userField().width), "the same width")
                save("site-desktop-" + language)
                chooser.close()
                tryCompare(chooser, "visible", false)
                // A phone held upright.
                root.width = 412
                tryCompare(chooser.parent, "width", 412)
                chooser.begin("https://ameli.fr")
                tryCompare(chooser, "opened", true)
                wait(50)
                s = siteField().mapToItem(root, 0, 0)
                u = userField().mapToItem(root, 0, 0)
                verify(u.y > s.y + siteField().height, "stacked (" + language + ")")
                compare(Math.round(s.x), Math.round(u.x))
                const box = popupItem().mapToItem(root, 0, 0)
                verify(box.x >= 0 && box.x + popupItem().width <= 412, "within 412 px: " + box.x + " + " + popupItem().width)
                verify(s.x + siteField().width <= 412 && u.x + userField().width <= 412)
                chooser.site = ""
                chooser.user = "aurore"
                chooser.search()
                wait(50)
                for (const label of labelsWith("")) {
                    if (!label.visible || label.width <= 0)
                        continue
                    const at = label.mapToItem(root, 0, 0)
                    verify(at.x + label.width <= 412 + 0.5, "a label within 412 px: \"" + label.text + "\" at " + at.x + " + " + label.width)
                }
                save("user-phone-" + language)
                chooser.close()
                tryCompare(chooser, "visible", false)
            }
        }
    }
}
