// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Extensions (ExtensionsTab.qml, docs/extensions.md) on a
// stand-in for Sioul: what an extension brings, the sites it talks to and
// whether it keeps a sign-in are said before anything is enabled; each
// button shows only where it does something; each agenda it brought says
// where its lessons go, its name as plain text, and a calendar chosen goes
// back to Sioul; Remove asks whether to leave its lessons in your calendars
// (left unless you say otherwise); a choice the extension asks before its
// sign-in is searchable, and the one chosen goes back to it.
// Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 800

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string extensions: ""
        property int pages: 0
        property var asked: []

        signal extensionChoices(string id, string choices)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function extensionsPage() {
            mock.pages += 1
        }
        function extensionEnable(id) {
            mock.asked = mock.asked.concat(["enable " + id])
        }
        function extensionSignIn(id, choice) {
            mock.asked = mock.asked.concat(["sign-in " + id + " " + choice])
        }
        function extensionDisable(id) {
            mock.asked = mock.asked.concat(["disable " + id])
        }
        function extensionRemove(id, keep) {
            mock.asked = mock.asked.concat(["remove " + id + " " + keep])
        }
        function extensionAccept(id) {
            mock.asked = mock.asked.concat(["accept " + id])
        }
        function extensionCalendar(id, agenda, choice) {
            mock.asked = mock.asked.concat(["calendar " + id + " " + agenda + " " + choice])
        }
    }

    function offer(changes) {
        const base = {
            id: "timetable", name: "A timetable", about: "Lessons.", version: "0.1.0",
            brings: "It brings a read-only agenda.", sites: "It talks to 2 sites: api.example.org, sso.example.org.",
            signIn: "It keeps a sign-in on this device.", needsSignIn: true, usable: true, unusable: "",
            installed: false, enabled: false, signedIn: false, running: false, line: "", problem: "", waiting: "", agendas: []
        }
        for (const key in changes)
            base[key] = changes[key]
        return base
    }

    function view(offers) {
        return JSON.stringify({ reading: false, where: "From example.org.", problem: "", offers: offers, calendars: [{ choice: "new:cloud", label: "A new calendar on cloud.example.org" }, { choice: "new:local", label: "A new calendar on this device" }, { choice: "place:local/kids", label: "Kids · this computer" }] })
    }

    Component {
        id: tabComponent

        ExtensionsTab {
            width: 860
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "Extensions"
        when: windowShown

        function find(item, name) {
            if (item.objectName === name)
                return item
            const children = item.children || []
            for (let i = 0; i < children.length; ++i) {
                const found = find(children[i], name)
                if (found)
                    return found
            }
            if (item.contentItem && item.contentItem !== item) {
                const found = find(item.contentItem, name)
                if (found)
                    return found
            }
            return null
        }

        function texts(item, out) {
            if (item.text !== undefined && item.visible !== false && typeof item.text === "string")
                out.push(item.text)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                texts(children[i], out)
            return out
        }

        function make(offers) {
            mock.extensions = view(offers)
            mock.asked = []
            const tab = createTemporaryObject(tabComponent, root)
            verify(tab !== null)
            waitForRendering(tab)
            return tab
        }

        function test_what_it_asks_is_said_before_it_is_enabled() {
            const tab = make([offer({})])
            compare(mock.pages > 0, true, "opening the page reads the list")
            const shown = texts(tab, []).join("\n")
            verify(shown.indexOf("It brings a read-only agenda.") >= 0, shown)
            verify(shown.indexOf("api.example.org, sso.example.org") >= 0, shown)
            verify(shown.indexOf("It keeps a sign-in on this device.") >= 0, shown)
            verify(find(tab, "extensionEnable:timetable").visible, "Enable, before anything")
            verify(!find(tab, "extensionSignIn:timetable").visible, "no sign-in before it is enabled")
            verify(!find(tab, "extensionRemove:timetable").visible, "nothing to remove")
            find(tab, "extensionEnable:timetable").clicked()
            compare(mock.asked, ["enable timetable"])
        }

        function test_an_enabled_one_signs_in_disables_and_takes_an_update() {
            const tab = make([offer({ installed: true, enabled: true, waiting: "Version 0.2.0 asks for more." })])
            verify(!find(tab, "extensionEnable:timetable").visible)
            const signIn = find(tab, "extensionSignIn:timetable")
            verify(signIn.visible && signIn.text === "extension-sign-in")
            signIn.clicked()
            find(tab, "extensionAccept:timetable").clicked()
            find(tab, "extensionDisable:timetable").clicked()
            compare(mock.asked, ["sign-in timetable ", "accept timetable", "disable timetable"])
            // Signed in: Sign in again; one that keeps no sign-in has no such button; one this Sioul cannot run, no Enable.
            mock.extensions = view([offer({ installed: true, enabled: true, signedIn: true }), offer({ id: "plain", needsSignIn: false, installed: true, enabled: true }), offer({ id: "newer", usable: false, unusable: "It needs a newer Sioul." })])
            waitForRendering(tab)
            compare(find(tab, "extensionSignIn:timetable").text, "extension-sign-in-again")
            verify(!find(tab, "extensionSignIn:plain").visible)
            verify(!find(tab, "extensionEnable:newer").visible)
            verify(texts(tab, []).indexOf("It needs a newer Sioul.") >= 0)
        }

        function test_remove_leaves_the_lessons_unless_told() {
            const tab = make([offer({ installed: true, enabled: true, agendas: [{ id: "p-1", name: "Timetable: Alex", line: "In Kids.", warn: false, place: "local/kids" }] })])
            find(tab, "extensionRemove:timetable").clicked()
            const dialog = tab.removeDialog
            tryCompare(dialog, "opened", true)
            verify(find(dialog, "extensionKeepAgendas").checked, "left in your calendars by default")
            dialog.accept()
            compare(mock.asked, ["remove timetable true"])
            find(tab, "extensionRemove:timetable").clicked()
            tryCompare(dialog, "opened", true)
            find(dialog, "extensionKeepAgendas").checked = false
            dialog.accept()
            compare(mock.asked, ["remove timetable true", "remove timetable false"])
        }

        function test_each_agenda_says_where_its_lessons_go_and_a_calendar_is_chosen() {
            const tab = make([offer({ installed: true, enabled: true, signedIn: true, agendas: [
                { id: "p-1", name: "Timetable: <b>Alex</b>", line: "Choose the calendar.", warn: true, place: "" },
                { id: "p-2", name: "Timetable: Sam", line: "In Kids: 140 lessons.", warn: false, place: "local/kids" }
            ] })])
            const shown = texts(tab, []).join("\n")
            verify(shown.indexOf("Timetable: <b>Alex</b>") >= 0, "a name the extension gave, as plain text: " + shown)
            verify(shown.indexOf("In Kids: 140 lessons.") >= 0 && shown.indexOf("Choose the calendar.") >= 0, shown)
            const none = find(tab, "extensionCalendar:p-1")
            const kids = find(tab, "extensionCalendar:p-2")
            compare(none.currentIndex, 0, "Choose…, while none is chosen")
            compare(kids.currentIndex, 3, "the calendar chosen")
            none.activated(1)
            kids.activated(3)
            kids.activated(0)
            compare(mock.asked, ["calendar timetable p-1 new:cloud"], "the one already chosen, and Choose…, ask nothing")
        }

        function test_a_choice_before_the_sign_in_goes_back_to_the_extension() {
            const tab = make([offer({ installed: true, enabled: true })])
            mock.extensionChoices("timetable", JSON.stringify([{ id: "north", name: "North Platform" }, { id: "south", name: "South Platform" }]))
            const dialog = tab.chooseDialog
            tryCompare(dialog, "opened", true)
            compare(dialog.shown.length, 2)
            const filter = find(dialog, "extensionChooseFilter")
            filter.text = "south"
            compare(dialog.shown.length, 1)
            compare(dialog.shown[0].id, "south")
            dialog.close()
            tryCompare(dialog, "opened", false)
            // Chosen: the extension's sign-in again, with the choice.
            mock.extensionChoices("timetable", JSON.stringify([{ id: "north", name: "North Platform" }]))
            tryCompare(dialog, "opened", true)
            const choice = find(dialog, "extensionChoice:north")
            verify(choice !== null)
            choice.clicked()
            compare(mock.asked, ["sign-in timetable north"])
            tryCompare(dialog, "opened", false)
        }
    }
}
