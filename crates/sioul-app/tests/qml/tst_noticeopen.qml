// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's one route for a notice (NoticeOpener.qml, main.qml's `open`,
// docs/notifications.md): each address of a notice goes to the route that
// shows its thing, on a stand-in for the window that records what it is
// asked. Made-up things only. Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

TestCase {
    id: root

    name: "NoticeOpen"

    QtObject {
        id: fakeWindow

        property var asked: []
        // The Sites page: whether it still keeps the web notice clicked.
        property bool kept: true
        property var sitesPage: QtObject {
            function clickNotice(id, key) {
                fakeWindow.asked = fakeWindow.asked.concat([["click", id, key]])
                return fakeWindow.kept
            }
        }

        function openThing(item) {
            fakeWindow.asked = fakeWindow.asked.concat([["thing", item.kind, item.uri, item.key]])
        }
        function openSite(id) {
            fakeWindow.asked = fakeWindow.asked.concat([["site", id]])
        }
        function showParameters(key) {
            fakeWindow.asked = fakeWindow.asked.concat([["settings", key]])
        }
    }

    NoticeOpener {
        id: opener

        window: fakeWindow
    }

    function opened(address, hint) {
        fakeWindow.asked = []
        opener.open(address, hint)
        return fakeWindow.asked
    }

    // Sioul's own things: the pause to move, a need, the end of the day, a dose.
    function test_sioul_s_own_things_open_their_questions() {
        compare(opened("sioul:move", ""), [["thing", "stopped", "", ""]], "Where I stopped")
        compare(opened("sioul:need/meal%3Alunch%3A2026-11-13", ""), [["thing", "need", "", "meal:lunch:2026-11-13"]])
        compare(opened("sioul:day/work:2026-11-13", ""), [["thing", "review", "", "work:2026-11-13"]])
        compare(opened("sioul:day/night:2026-11-13", ""), [["thing", "review", "", "night:2026-11-13"]])
        compare(opened("sioul:dose/levothyrox%2F08%3A00", ""), [["thing", "dose", "", "levothyrox/08:00"]], "the doses' route, as before")
    }

    // A site's notice: the site, then its web notice's own click, by its key decoded.
    function test_a_site_s_notice_opens_the_site_then_its_conversation() {
        compare(opened("sioul:site/chat#room%20%234", ""), [["site", "chat"], ["click", "chat", "room #4"]])
        fakeWindow.kept = false
        compare(opened("sioul:site/proton%2Fmail#n3", ""), [["site", "proton/mail"], ["click", "proton/mail", "n3"]], "gone: the site alone")
        fakeWindow.kept = true
        compare(opened("sioul:site/chat", ""), [["site", "chat"]], "no key: the site")
    }

    // A message and its file; several, the mailbox; the Porch; a setting.
    function test_mail_the_porch_and_settings() {
        compare(opened("mid:code-1@example.org", "/stand-in/mail/cur/1.U1-1.sioul"), [["thing", "mail", "mid:code-1@example.org", "/stand-in/mail/cur/1.U1-1.sioul"]])
        compare(opened("sioul:mail/alice/0a1b2c", ""), [["thing", "mail", "sioul:mail/alice/0a1b2c", ""]])
        compare(opened("sioul:mailbox", ""), [["thing", "mailbox", "", ""]])
        compare(opened("sioul:porch", ""), [["thing", "porch", "", ""]])
        compare(opened("sioul:settings/words.languages", ""), [["settings", "words.languages"]])
    }

    // An event at its time, by its file and time; without them, the agenda;
    // a task, a paper, a budget's line where they live.
    function test_dated_things_open_where_they_live() {
        compare(opened("sioul:event/yoga#1793520000", "/stand-in/cal/yoga.ics#1793520000"), [["thing", "event", "sioul:event/yoga", "/stand-in/cal/yoga.ics#1793520000"]])
        compare(opened("sioul:event/yoga#1793520000", ""), [["thing", "agenda", "", ""]])
        compare(opened("sioul:task/t1", ""), [["thing", "task", "sioul:task/t1", ""]])
        compare(opened("sioul:paper/passport", ""), [["thing", "paper", "sioul:paper/passport", ""]])
        compare(opened("sioul:budget/home/3", ""), [["thing", "budget", "sioul:budget/home/3", ""]])
        compare(opened("https://example.org", ""), [], "not a thing of Sioul's: nothing")
    }
}
