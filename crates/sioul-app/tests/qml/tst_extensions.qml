// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Extensions (ExtensionsTab.qml, docs/extensions.md) on a
// stand-in for Sioul: what an extension brings, the sites it talks to and
// whether it keeps a sign-in are said before anything is enabled; each
// button shows only where it does something; each agenda it brought says
// where its lessons go, its name as plain text, and a calendar chosen goes
// back to Sioul; each set of contacts says where its contacts go, and an
// address book chosen goes back to Sioul; Remove asks whether to leave its
// lessons in your calendars (left unless you say otherwise) and says its
// contacts stay; a choice the extension asks before its sign-in is
// searchable, and the one chosen goes back to it.
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
        property string extensionSignInForm: ""
        property int pages: 0
        property var asked: []

        signal extensionChoices(string id, string choices)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function viewFlag(key) {
            return false
        }
        function extensionCredentials(id, values) {
            mock.asked = mock.asked.concat(["creds " + id + " " + values])
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
        // What Agenda ⚙ knows to make a calendar (`agenda::setup`), and its making (`servers::make`).
        signal personsChanged
        function agendasSetup() {
            return JSON.stringify({ accounts: [{ id: "cloud", name: "you@cloud.example.org", google: false }, { id: "local", name: "this computer", google: false }], persons: { partner: [{ id: "p-sam", name: "Sam" }], child: [], parent: [] } })
        }
        function newAgenda(text) {
            const asked = JSON.parse(text)
            mock.asked = mock.asked.concat(["new " + asked.account + " " + asked.name + " " + asked.whose + " " + asked.events + " " + asked.tasks])
            return JSON.stringify({ place: asked.account + "/new", problem: "" })
        }
        function setAgendaPerson(place, field, person) {
            mock.asked = mock.asked.concat(["person " + place + " " + field + " " + person])
            return ""
        }
        function household() { return "{}" }
        function extensionCalendar(id, agenda, choice) {
            mock.asked = mock.asked.concat(["calendar " + id + " " + agenda + " " + choice])
        }
        function extensionBook(id, contactSet, choice) {
            mock.asked = mock.asked.concat(["book " + id + " " + contactSet + " " + choice])
        }
        // An extension's log, the newest first (`extensions::log_json`); what was copied.
        property var logLines: []
        property string copied: ""
        function extensionLog(id) {
            mock.asked = mock.asked.concat(["log " + id])
            return JSON.stringify({ title: "The log of " + id, lines: mock.logLines })
        }
        function copyText(text) {
            mock.copied = text
            return true
        }
        // The versions and the list (Part C): an update taken, all at once, the list read again.
        function extensionUpdate(id) {
            mock.asked = mock.asked.concat(["update " + id])
        }
        function extensionsUpdateAll() {
            mock.asked = mock.asked.concat(["update all"])
        }
        function extensionsReadList() {
            mock.asked = mock.asked.concat(["read list"])
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

    function view(offers, more) {
        const extra = more || {}
        return JSON.stringify({ reading: false, where: "From example.org.", problem: "", offers: offers,
            list: extra.list || "", updates: extra.updates || 0, updateAll: "Update all (" + (extra.updates || 0) + ")", asks: extra.asks || [],
            calendars: [{ choice: "new:cloud", label: "A new calendar on cloud.example.org" }, { choice: "new:local", label: "A new calendar on this device" }, { choice: "place:local/kids", label: "Kids · this computer" }],
            books: [{ choice: "new:cloud", label: "A new address book on cloud.example.org" }, { choice: "place:cloud/clients", label: "Clients · example.org" }, { choice: "new:local", label: "A new address book on this device" }] })
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
            mock.extensionSignInForm = ""
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
                { id: "p-2", name: "Timetable: Sam", line: "In Kids: 140 lessons.", warn: false, place: "local/kids" },
                { id: "p-3", name: "Lessons", line: "In Lessons: 12 lessons.", warn: false, place: "local/lessons", theirs: true, whose: "Lessons is Kim's agenda. Its events are theirs." }
            ] })])
            const shown = texts(tab, []).join("\n")
            verify(shown.indexOf("Timetable: <b>Alex</b>") >= 0, "a name the extension gave, as plain text: " + shown)
            verify(shown.indexOf("In Kids: 140 lessons.") >= 0 && shown.indexOf("Choose the calendar.") >= 0, shown)
            const none = find(tab, "extensionCalendar:p-1")
            const kids = find(tab, "extensionCalendar:p-2")
            compare(none.currentIndex, 0, "Choose…, while none is chosen")
            compare(kids.currentIndex, 3, "the calendar chosen")
            kids.activated(3)
            kids.activated(0)
            compare(mock.asked, [], "the one already chosen, and Choose…, ask nothing")
            // A new calendar: Agenda ⚙'s form, its account and name proposed, whose it is asked there;
            // once made, written as the agenda's line writes it, then chosen.
            none.activated(1)
            const form = find(tab, "extensionNewCalendar:p-1")
            verify(form !== null && form.visible, "the form that makes a calendar")
            compare(find(form, "agendaNewName").text, "Timetable: <b>Alex</b>")
            const whose = find(form, "agendaWhose")
            whose.activated(3)
            compare(form.whose, "partner")
            verify(find(form, "agendaWhoseTheirs").visible, "what another person's agenda does, said where it is chosen")
            find(form, "agendaPerson").activated(1)
            compare(form.personId, "p-sam")
            form.make()
            compare(mock.asked, ["new cloud Timetable: <b>Alex</b> partner true false", "person cloud/new person p-sam", "calendar timetable p-1 place:cloud/new"])
            verify(find(tab, "extensionNewCalendar:p-1") === null, "closed once made")
            mock.asked = []
            // Whether its lessons count as busy time is its calendar's own switch, in Your agendas
            // (26 November 2026): none here, for any agenda.
            verify(find(tab, "extensionBusy:p-1") === null && find(tab, "extensionBusy:p-2") === null)
            // Another person's agenda: whose said in words.
            compare(find(tab, "extensionAgendaWhose:p-3").text, "Lessons is Kim's agenda. Its events are theirs.")
            verify(find(tab, "extensionAgendaWhose:p-3").visible)
        }

        function test_each_set_says_where_its_contacts_go_and_a_book_is_chosen() {
            const tab = make([offer({ installed: true, enabled: true, signedIn: true, brings: "It brings contacts.", sets: [
                { id: "clients", name: "Clients: <b>the salon</b>", line: "Choose the address book.", warn: true, place: "" },
                { id: "staff", name: "Staff", line: "In Clients: 128 contacts.", warn: false, place: "cloud/clients" }
            ] })])
            const shown = texts(tab, []).join("\n")
            verify(shown.indexOf("Clients: <b>the salon</b>") >= 0, "a name the extension gave, as plain text: " + shown)
            verify(shown.indexOf("In Clients: 128 contacts.") >= 0 && shown.indexOf("Choose the address book.") >= 0, shown)
            const none = find(tab, "extensionBook:clients")
            const chosen = find(tab, "extensionBook:staff")
            compare(none.currentIndex, 0, "Choose…, while none is chosen")
            compare(chosen.currentIndex, 2, "the address book chosen")
            compare(none.model.length, 4, "Choose…, a new one on the account, the person's own, a new one here")
            none.activated(1)
            chosen.activated(2)
            chosen.activated(0)
            none.activated(3)
            compare(mock.asked, ["book timetable clients new:cloud", "book timetable clients new:local"], "the one already chosen, and Choose…, ask nothing")
        }

        function test_remove_says_its_contacts_stay() {
            const tab = make([offer({ installed: true, enabled: true, sets: [{ id: "clients", name: "Clients", line: "", warn: false, place: "cloud/clients" }] })])
            find(tab, "extensionRemove:timetable").clicked()
            const dialog = tab.removeDialog
            tryCompare(dialog, "opened", true)
            verify(find(dialog, "extensionContactsStay").visible, "its contacts stay, said")
            dialog.reject()
            tryCompare(dialog, "opened", false)
            // One that brought no contacts says nothing of them.
            mock.extensions = view([offer({ installed: true, enabled: true })])
            waitForRendering(tab)
            find(tab, "extensionRemove:timetable").clicked()
            tryCompare(dialog, "opened", true)
            verify(!find(dialog, "extensionContactsStay").visible)
            dialog.accept()
            compare(mock.asked, ["remove timetable true"])
        }

        function test_remove_asks_of_lessons_only_for_one_that_brings_agendas() {
            const tab = make([offer({ installed: true, enabled: true, bringsAgenda: false, sets: [{ id: "clients", name: "Clients", line: "", warn: false, place: "cloud/clients" }] })])
            find(tab, "extensionRemove:timetable").clicked()
            const dialog = tab.removeDialog
            tryCompare(dialog, "opened", true)
            verify(!find(dialog, "extensionKeepAgendas").visible, "no lessons to ask about")
            verify(find(dialog, "extensionContactsStay").visible)
            verify(texts(dialog.contentItem, []).indexOf("extension-remove-says-alone") >= 0, "its own sentence, without lessons")
            dialog.accept()
            compare(mock.asked, ["remove timetable true"], "nothing taken out")
            // One that brings agendas asks, as before.
            mock.extensions = view([offer({ installed: true, enabled: true, bringsAgenda: true })])
            waitForRendering(tab)
            find(tab, "extensionRemove:timetable").clicked()
            tryCompare(dialog, "opened", true)
            verify(find(dialog, "extensionKeepAgendas").visible)
            verify(texts(dialog.contentItem, []).indexOf("extension-remove-says") >= 0)
            dialog.reject()
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

        function test_a_credentials_form_shows_sioul_s_labels_masks_the_password_and_hands_the_values_back() {
            const tab = make([offer({ installed: true, enabled: true })])
            // The backend asks for a form: the extension's name, Sioul's own labels per kind.
            mock.extensionSignInForm = JSON.stringify({ id: "planity", name: "Planity", french: false, fields: [
                { kind: "email", label: "Email", masked: false },
                { kind: "password", label: "Password", masked: true }] })
            const dialog = tab.signInFormDialog
            tryCompare(dialog, "opened", true)
            verify(dialog.title.indexOf("Planity") >= 0, "the extension's name in the title: " + dialog.title)
            const shown = texts(dialog.contentItem, []).join("\n")
            verify(shown.indexOf("Email") >= 0 && shown.indexOf("Password") >= 0, "Sioul's labels: " + shown)
            // The password is masked; the email is not.
            const email = find(dialog.contentItem, "extensionCred:email").item
            const password = find(dialog.contentItem, "extensionCred:password").item
            compare(password.echoMode, TextInput.Password, "the password masked")
            compare(email.echoMode, TextInput.Normal, "the email plain")
            email.text = "alex@example.org"
            password.text = "hunter2"
            // Sign in hands the values back once, keyed by kind.
            find(dialog.footer, "extensionSignInDo").clicked()
            compare(mock.asked, ['creds planity {"email":"alex@example.org","password":"hunter2"}'])
            tryCompare(dialog, "opened", false)
        }

        function test_a_web_address_field_is_labelled_by_sioul_shown_plain_and_handed_back() {
            const tab = make([offer({ installed: true, enabled: true })])
            mock.extensionSignInForm = JSON.stringify({ id: "feed", name: "Example Bookings", french: false, fields: [
                { kind: "email", label: "Email", masked: false },
                { kind: "password", label: "Password", masked: true },
                { kind: "web-address", label: "Web address", masked: false }] })
            const dialog = tab.signInFormDialog
            tryCompare(dialog, "opened", true)
            const shown = texts(dialog.contentItem, []).join("\n")
            verify(shown.indexOf("Web address") >= 0, "Sioul's label for a link: " + shown)
            // A link is shown as it is pasted, with a keyboard for addresses.
            const link = find(dialog.contentItem, "extensionCred:web-address").item
            compare(link.echoMode, TextInput.Normal, "the link plain")
            verify((link.inputMethodHints & Qt.ImhUrlCharactersOnly) !== 0, "a keyboard for addresses")
            find(dialog.contentItem, "extensionCred:email").item.text = "alex@example.org"
            find(dialog.contentItem, "extensionCred:password").item.text = "hunter2"
            link.text = "https://calendar.example.org/feed?key=k"
            find(dialog.footer, "extensionSignInDo").clicked()
            compare(mock.asked, ['creds feed {"email":"alex@example.org","password":"hunter2","web-address":"https://calendar.example.org/feed?key=k"}'])
            tryCompare(dialog, "opened", false)
        }

        function test_a_problem_s_detail_shows_under_it_and_details_show_the_log() {
            const tab = make([offer({ installed: true, enabled: true, problem: "Bookings needs you to sign in again.", detail: "In its own words: the password sign-in answered HTTP 400 with INVALID_LOGIN_CREDENTIALS" }), offer({ id: "plain", installed: true, enabled: true })])
            const detail = find(tab, "extensionDetail:timetable")
            verify(detail.visible && detail.text.indexOf("INVALID_LOGIN_CREDENTIALS") >= 0, detail.text)
            verify(!find(tab, "extensionDetail:plain").visible, "no problem: no detail")
            verify(find(tab, "extensionDetails:timetable").visible, "Details… on an installed extension")
            // Details…: its log, the newest first, selectable, read only, copied whole.
            mock.logLines = ["2026-11-26 10:04:00 sign-in: refused", "2026-11-26 10:02:00 sioul: the sign-in form shown (email, password, web-address)"]
            find(tab, "extensionDetails:timetable").clicked()
            const card = tab.logDialog
            tryCompare(card, "opened", true)
            compare(mock.asked[mock.asked.length - 1], "log timetable")
            compare(card.title, "The log of timetable")
            const lines = find(card.contentItem, "extensionLogLines")
            verify(lines.visible && lines.readOnly && lines.selectByMouse)
            compare(lines.text, mock.logLines.join("\n"))
            verify(!find(card.contentItem, "extensionLogEmpty").visible)
            find(card.footer, "extensionLogCopy").clicked()
            compare(mock.copied, mock.logLines.join("\n"))
            compare(find(card.footer, "extensionLogCopy").text, "ui-copied")
            find(card.footer, "extensionLogClose").clicked()
            tryCompare(card, "opened", false)
            // An empty log says so, and nothing to copy.
            mock.logLines = []
            find(tab, "extensionDetails:plain").clicked()
            tryCompare(card, "opened", true)
            verify(find(card.contentItem, "extensionLogEmpty").visible)
            verify(!find(card.footer, "extensionLogCopy").enabled)
            card.close()
            tryCompare(card, "opened", false)
        }

        function test_versions_an_update_and_the_list_read_again() {
            mock.extensions = view([
                offer({ installed: true, enabled: true, versionLine: "Version 0.2.0 runs here; 0.2.1 is offered.", update: "0.2.1", updateLabel: "Update to version 0.2.1" }),
                offer({ id: "asks", installed: true, enabled: true, versionLine: "Version 1.0 runs here; 1.1 is offered.", waiting: "Version 1.1 asks for more than the one in use: these sites: new.example.org." }),
                offer({ id: "fresh", versionLine: "Version 0.1.0 is offered." })
            ], { list: "The list in use is number 2, read today at 10:15; its signature was checked.", updates: 2, asks: ["Asks: Version 1.1 asks for more than the one in use: these sites: new.example.org."] })
            mock.extensionSignInForm = ""
            mock.asked = []
            const tab = createTemporaryObject(tabComponent, root)
            waitForRendering(tab)
            compare(find(tab, "extensionVersion:timetable").text, "Version 0.2.0 runs here; 0.2.1 is offered.")
            compare(find(tab, "extensionVersion:fresh").text, "Version 0.1.0 is offered.", "one not installed: the version offered")
            // An update that asks for no more: its own button; one that asks for more: Take the update, under what it asks.
            const update = find(tab, "extensionUpdate:timetable")
            verify(update.visible && update.text === "Update to version 0.2.1")
            verify(!find(tab, "extensionUpdate:asks").visible && find(tab, "extensionAccept:asks").visible)
            update.clicked()
            compare(mock.asked[mock.asked.length - 1], "update timetable")
            // The list in use, said at the foot, and read again on demand.
            compare(find(tab, "extensionsList").text, "The list in use is number 2, read today at 10:15; its signature was checked.")
            find(tab, "extensionsReadList").clicked()
            compare(mock.asked[mock.asked.length - 1], "read list")
            // Update all: one question first, since one of them asks for more; nothing until yes.
            const all = find(tab, "extensionsUpdateAll")
            verify(all.visible && all.text === "Update all (2)")
            all.clicked()
            const dialog = tab.updateAllDialog
            tryCompare(dialog, "opened", true)
            verify(texts(dialog.contentItem, []).indexOf("Asks: Version 1.1 asks for more than the one in use: these sites: new.example.org.") >= 0)
            compare(mock.asked[mock.asked.length - 1], "read list", "nothing taken before the yes")
            dialog.accept()
            compare(mock.asked[mock.asked.length - 1], "update all")
            tryCompare(dialog, "opened", false)
            // Several that ask for no more: at once. One alone: no Update all.
            mock.extensions = view([offer({ update: "0.2.1", installed: true }), offer({ id: "b", update: "2", installed: true })], { updates: 2 })
            waitForRendering(tab)
            find(tab, "extensionsUpdateAll").clicked()
            compare(mock.asked[mock.asked.length - 1], "update all")
            verify(!dialog.opened)
            mock.extensions = view([offer({ update: "0.2.1", installed: true })], { updates: 1 })
            waitForRendering(tab)
            verify(!find(tab, "extensionsUpdateAll").visible)
        }

        function test_sign_in_reads_each_field_as_it_is_now() {
            const tab = make([offer({ installed: true, enabled: true })])
            mock.extensionSignInForm = JSON.stringify({ id: "planity", name: "Planity", french: false, fields: [
                { kind: "email", label: "Email", masked: false },
                { kind: "password", label: "Password", masked: true },
                { kind: "web-address", label: "Web address", masked: false }] })
            const dialog = tab.signInFormDialog
            tryCompare(dialog, "opened", true)
            find(dialog.contentItem, "extensionCred:email").item.text = "alex@example.org"
            find(dialog.contentItem, "extensionCred:password").item.text = "hunter2"
            find(dialog.contentItem, "extensionCred:web-address").item.text = "https://calendar.example.org/feed?key=k"
            // What a field's last change said may lag behind (a phone's keyboard composing): the fields are read at Sign in.
            dialog.typed = {}
            find(dialog.footer, "extensionSignInDo").clicked()
            compare(mock.asked, ['creds planity {"email":"alex@example.org","password":"hunter2","web-address":"https://calendar.example.org/feed?key=k"}'])
            tryCompare(dialog, "opened", false)
        }

        function test_cancelling_the_credentials_form_runs_nothing() {
            const tab = make([offer({ installed: true, enabled: true })])
            mock.extensionSignInForm = JSON.stringify({ id: "planity", name: "Planity", french: false, fields: [
                { kind: "password", label: "Password", masked: true }] })
            const dialog = tab.signInFormDialog
            tryCompare(dialog, "opened", true)
            find(dialog.footer, "extensionSignInCancel").clicked()
            compare(mock.asked, ["creds planity "])
            tryCompare(dialog, "opened", false)
        }
    }
}
