// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A phone's card in Settings ▸ Devices (DevicesTab.qml, PhoneCard.qml), on a
// stand-in for Sioul. Another phone's card, at a computer's width: its
// section Calls, open, a grid of Rings and Voicemail by row and moment, the
// profiles (Professional's own line saying that a second call does not ring
// outside work hours), the switches, a group given its row and taken off,
// the SIMs' sentence, that this phone does not screen yet; each change said
// as a field of its line (`deviceSay(id, "calls.…", value)`); no fold line;
// Android's accesses said to be set on that phone. This phone's own card, at
// a phone's width (412 points): every section folded to one line, Calls
// opened as a list of rows, a row opening its four choices, no grid, nothing
// past the edge; what Android must allow in the design's order, each line
// its state and one button (`phoneAllow`); the restricted settings' card,
// its three steps each a button ticked when done (`restrictedStep`), the
// drawing of ⋮ under the second; the blocks of the tab "This phone", made
// only once unfolded. A phone whose line says nothing: the table of what it
// does today, a cell that varies saying so (a press makes it ring) and when
// it rings under the table, no profile's line, that calls are set here.
// The doors (`reveal`): Calls and each app by time, unfolded, in this
// phone's card, else the first phone's; on a computer, each app by time
// listed in a phone's card once some have times. A reload keeps the cards
// and what is unfolded. Run by tools/qml-test.sh. To see the cards, set
// `pictures` below to a folder (a change of your own, not to commit).

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    // Where the pictures go; none when empty.
    readonly property string pictures: ""

    width: 1400
    height: 2400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var asked: []
        // This device: the phone itself (the narrow tab), or a computer (the wide one).
        property bool herePhone: false
        property bool telephony: false
        property var steps: ({ notice: false, info: false, back: false })
        // The phone's line says nothing: the table of what it does today.
        property bool today: false
        // Apps, conversations and sites with times of their own (`appNotesSetup`'s grid's rows).
        property int lists: 0

        readonly property var row: ({ here: false, name: "Moss", system: "The phone (Moss)", given: "Moss", form: "phone", form_words: "A phone", code: "closed", state: "Closed since 21:40.", notes: [], opened: "21:00", shared: "21:40", version: "0.0.11", older: false, build: "", area: "admin+leisure", area_words: "Admin, Leisure", parts: [], parts_words: "Settings", off: false, forget: false, forget_now: false, follows: false, cells: [] })

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function cells(words) {
            const whens = ["work", "day", "night", "busy"]
            const cell = { R: "rings", V: "voicemail", U: "usual" }
            const word = { R: "Rings", V: "Voicemail", U: "As usual" }
            return words.map((w, i) => ({ when: whens[i], cell: cell[w], rings: w === "R", word: word[w] }))
        }
        // `calls_line::view` for a Personal phone with a group School, as the core words it.
        function calls() {
            return mock.today ? mock.callsToday() : mock.callsSaid()
        }
        function callsSaid() {
            return {
                set: true, profile: "personal", offered: "personal",
                profiles: [
                    { id: "professional", label: "Professional", said: "Numbers nobody knows ring in your work hours; commercial callers never. Outside work hours everything but Always through goes to voicemail: a second call within 15 minutes does not ring outside work hours.", chosen: false },
                    { id: "personal", label: "Personal", said: "Your contacts ring in the day; numbers nobody knows go to voicemail unless they call twice within 15 minutes.", chosen: true },
                    { id: "both", label: "Both", said: "Professional in your work hours, Personal outside them.", chosen: false },
                    { id: "as-android", label: "As Android does", said: "Sioul screens nothing on this phone: Android decides.", chosen: false }
                ],
                said: "",
                columns: [{ id: "work", label: "Work hours" }, { id: "day", label: "The rest of the day" }, { id: "night", label: "The night" }, { id: "busy", label: "While busy" }],
                rows: [
                    { id: "always", label: "Always through", fixed: true, group: false, cells: mock.cells(["R", "R", "R", "R"]) },
                    { id: "safe", label: "Safe", fixed: false, group: false, cells: mock.cells(["R", "R", "V", "R"]) },
                    { id: "contacts", label: "Your contacts", fixed: false, group: false, cells: mock.cells(["R", "R", "V", "U"]) },
                    { id: "group:School", label: "School", fixed: false, group: true, cells: mock.cells(["R", "R", "V", "R"]) },
                    { id: "restricted", label: "Restricted", fixed: false, group: false, cells: mock.cells(["R", "V", "V", "V"]) },
                    { id: "unknown", label: "Numbers nobody knows", fixed: false, group: false, cells: mock.cells(["V", "V", "V", "V"]) },
                    { id: "hidden", label: "Hidden numbers", fixed: false, group: false, cells: mock.cells(["R", "V", "V", "V"]) }
                ],
                switches: [{ key: "commercial", label: "Commercial and automated callers never ring", on: true }, { key: "twice", label: "A second call within 15 minutes rings", on: true }, { key: "called", label: "Numbers you called in the last 14 days ring as your contacts do", on: true }, { key: "impossible", label: "Numbers that cannot be a caller's never ring", on: true, more: true }, { key: "unverified", label: "Numbers your network could not verify never ring", on: false, more: true }],
                busy: "While busy, As usual rings as the moment's column says.",
                held: "Under do-not-disturb and in a pause, Always through alone rings, as on every phone.",
                groups: ["Clients"],
                screens: false,
                not_screening: "This phone does not screen calls yet.",
                sims: "Android does not tell a screening app which SIM a call comes on.",
                always: "Whatever this table says, emergency numbers ring."
            }
        }
        // `calls_line::view` for a line that says nothing: what the phone does today, two cells varying.
        function callsToday() {
            const shown = mock.callsSaid()
            const rows = shown.rows.filter(r => r.group !== true).map(r => Object.assign({}, r))
            const contacts = rows.find(r => r.id === "contacts")
            contacts.cells = mock.cells(["R", "R", "V", "R"])
            contacts.cells[1] = Object.assign({}, contacts.cells[1], { varies: true, word: "Varies" })
            contacts.cells[3] = Object.assign({}, contacts.cells[3], { varies: true, word: "As usual" })
            return Object.assign({}, shown, {
                set: false, profile: "", today: true,
                profiles: shown.profiles.map(p => Object.assign({}, p, { chosen: false })),
                said: "Until you change this table, this phone screens calls as every phone does.",
                rows: rows,
                details: ["Your contacts, The rest of the day: rings in Admin, Leisure, Meals; voicemail in Free time.", "While busy, calls ring as the time of day says."],
                here: "Calls are set here, phone by phone."
            })
        }
        function devices() {
            const phone = Object.assign({}, mock.row, { id: "phone", here: mock.herePhone })
            const desk = Object.assign({}, mock.row, { id: "desk", here: !mock.herePhone, name: "Desk", system: "desk", form: "computer", form_words: "A computer" })
            const rows = mock.herePhone ? [phone, desk] : [desk, phone]
            return JSON.stringify({ on: true, why: "", stays: "", rows: rows, grid: { times: [], now: "", unheld: "", twice: "", never: "" }, calls: { phone: mock.calls() } })
        }
        function phoneCard() {
            if (!mock.herePhone)
                return JSON.stringify({ phone: false })
            const line = (what, state) => ({ what: what, label: "label-" + what, for: "for-" + what, state: state, said: state, button: state === "given" ? "Android's page" : "Allow in Android…" })
            return JSON.stringify({
                phone: true,
                allow: [line("role", "given"), line("contacts", "given"), line("history", "not-given"), line("notifications", "not-given"), line("texts", "not-given"), line("dnd", "given"), line("alarms", "given"), line("battery", "unknown")],
                restricted: { intro: "Sioul was installed from a file.", steps: [{ step: "notice", said: "Let Android show its notice.", button: "Show Android's notice", done: mock.steps.notice }, { step: "info", said: "Open Sioul's page in Android.", button: "Open Sioul's page", done: mock.steps.info }, { step: "back", said: "Come back to Sioul.", button: "Ask again now", done: mock.steps.back }], works: mock.steps.back, cannot: "Sioul cannot see the setting itself." }
            })
        }
        function phoneAllow(what) {
            mock.asked = mock.asked.concat(["allow " + what])
            return mock.phoneCard()
        }
        function restrictedStep(step) {
            mock.asked = mock.asked.concat(["step " + step])
            const steps = Object.assign({}, mock.steps)
            steps[step] = true
            mock.steps = steps
            return mock.phoneCard()
        }
        function deviceSay(id, field, value) {
            mock.asked = mock.asked.concat([["say", id, field, value].join(" ").trim()])
            return ""
        }
        function deviceOff(id, off) {
            return ""
        }
        function deviceForget(id) {
            return ""
        }
        function deviceFiles(id) {
            return "{}"
        }
        // What the tab "This phone"'s blocks read, stood in for: nothing to show.
        function appNotesSetup() {
            if (mock.lists === 0)
                return ""
            const cell = column => ({ column: column, value: "now", word: "Now", choices: ["now", "later"], said: "Chat, at " + column + ": At once." })
            const row = i => ({ id: "app." + i, label: "Chat " + i, group: "Apps", group_id: "apps", help: "", changed: true, choices: [{ id: "now", label: "At once", word: "Now" }, { id: "later", label: "Later", word: "Later" }], cells: [cell("work"), cell("sleep")] })
            const rows = []
            for (let i = 0; i < mock.lists; ++i)
                rows.push(row(i))
            return JSON.stringify({ android: mock.herePhone, access: true, restricted: false, contacts: true, hold: null, times: "", heard: "", rang: [], link: { available: false, linked: false, on: false, giving_back: false, given_back: false }, silenced: [], apps: [], conversations: [], sites: [], grid: { columns: [{ id: "work", label: "Work" }, { id: "sleep", label: "Sleep" }], marks: [], rows: rows }, more: false })
        }
        function appNotesChange(verb, json) {
            return ""
        }
        function phoneMessages() {
            return ""
        }
        function texts() {
            return ""
        }
        function callsSetup() {
            return ""
        }
        function callsSetupChange(verb, json) {
            return ""
        }
        function dndSetup() {
            return ""
        }
        function dndChange(verb, json) {
            return ""
        }
        function reminderOpened(kind, a, b) {
        }
        function wakeSettings(which) {
            mock.asked = mock.asked.concat(["wake " + which])
        }
    }

    Flickable {
        id: flick

        anchors.fill: parent
        contentHeight: tab.implicitHeight

        DevicesTab {
            id: tab

            width: flick.width - 32
            x: 16
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "PhoneCard"
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
        function showing(item) {
            for (let i = item; i; i = i.parent) {
                if (i.visible === false)
                    return false
                if (i === tab)
                    return true
            }
            return true
        }
        function named(name) {
            return walk(tab, i => i.objectName === name, [])[0] || null
        }
        function all(name) {
            return walk(tab, i => i.objectName === name && showing(i), [])
        }
        function within(item) {
            const at = item.mapToItem(root, 0, 0)
            return at.x >= -0.5 && at.x + item.width <= root.width + 0.5
        }
        function save(name) {
            if (root.pictures === "")
                return
            wait(100)
            grabImage(root).save(root.pictures + "/" + name + ".png")
        }
        function reload(width, herePhone) {
            // Each test from cards made anew, at its width (a reload alone keeps them).
            tab.view = ({ on: false, why: "", stays: "", rows: [] })
            tab.ids = []
            wait(10)
            root.width = width
            mock.herePhone = herePhone
            // A phone without telephony's own block of the call screening: none (a tablet's case).
            mock.telephony = false
            mock.asked = []
            mock.steps = { notice: false, info: false, back: false }
            mock.today = false
            mock.lists = 0
            tab.reload()
            wait(50)
        }

        // Another phone, at a computer's width: its calls set from here, open, a grid.
        function test_another_phone_s_calls_at_a_computer_s_width() {
            reload(1400, false)
            const card = named("phoneCard-phone")
            verify(card !== null, "the phone's card has its sections")
            verify(named("setting:devices.here") === null, "the computer's own row has no phone card")
            compare(all("foldLine").length, 0, "nothing folds at a computer's width")
            verify(showing(named("callsGrid")), "the table is a grid")
            // Professional's own line says that a second call does not ring outside work hours.
            const professional = named("callsProfile-professional")
            verify(showing(professional))
            compare(named("callsProfile-personal").checked, true, "the profile chosen")
            const said = named("callsProfileSaid")
            verify(said.text.indexOf("Personal") < 0 && said.text.indexOf("unless they call twice") > 0, said.text)
            // A cell pressed: the other of its two states, written as a field of the phone's line.
            const night = named("callsCell-contacts-night")
            compare(night.text, "Voicemail")
            night.clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.cell.contacts.night rings")
            named("callsCell-safe-work").clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.cell.safe.work voicemail")
            // While busy, three values in turn: As usual, Rings, Voicemail (H2).
            const usual = named("callsCell-contacts-busy")
            compare(usual.text, "As usual")
            usual.clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.cell.contacts.busy rings")
            named("callsCell-safe-busy").clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.cell.safe.busy voicemail")
            verify(showing(named("callsBusy")) && showing(named("callsHeld")), "what as usual means while busy, what a pause and do-not-disturb do")
            // Each switch one setting; the two others under More (M3).
            verify(named("callsSwitch-unverified") === null || !showing(named("callsSwitch-unverified")), "folded under More")
            named("callsMore").clicked()
            wait(20)
            verify(showing(named("callsSwitch-unverified")))
            named("callsSwitch-unverified").toggle()
            named("callsSwitch-unverified").toggled()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.switch.unverified on")
            verify(all("callsCell-always-work").length === 0, "Always through shows no button: it always rings")
            // A profile, a switch, a group taken off, a group given its row.
            professional.clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.profile professional")
            named("callsSwitch-twice").toggle()
            named("callsSwitch-twice").toggled()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.switch.twice off")
            named("callsRemove-group:School").clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.group.remove School")
            verify(showing(named("callsAddGroup")), "a group with no row yet is offered")
            // What this phone does not do yet, the SIMs, what always rings; Android's accesses are set on it.
            verify(showing(named("callsNotScreening")))
            verify(showing(named("callsSims")) && !showing(named("callsSimsOpen")), "the SIM page is opened from the phone itself")
            verify(showing(named("callsAlways")))
            named("callsBack").clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.clear")
            verify(named("foldAllow") === null || !showing(named("foldAllow")), "another phone's checklist is not shown here")
            for (const cell of walk(card, i => i.objectName.indexOf("callsCell-") === 0 && showing(i), []))
                verify(within(cell), cell.objectName + ": within the page")
            save("phone-card-computer")
        }

        // This phone's own card at a phone's width: everything folds; Calls a list of rows.
        function test_this_phone_s_card_at_a_phone_s_width() {
            reload(412, true)
            const card = named("setting:devices.here")
            verify(card !== null, "this phone's card, where a door to its setup lands")
            const folds = all("foldLine")
            verify(folds.length >= 7, "Calls, what Android must allow, restricted settings and the tab's blocks: " + folds.length)
            for (const line of folds)
                verify(within(line), line.text + ": within the page")
            verify(!showing(named("callsGrid")), "no grid on a phone")
            verify(!showing(named("callsProfile-personal")), "Calls folded")
            verify(["apps", "messages", "texts", "dnd"].every(b => named("block-" + b).item === null), "the tab's blocks made only once unfolded")
            // Calls unfolded: the rows, each opening its four choices.
            named("foldCalls").open = true
            wait(20)
            const contacts = named("callsRow-contacts")
            verify(showing(contacts))
            verify(named("callsCell-contacts-night") === null, "a row's choices made once opened")
            contacts.clicked()
            wait(20)
            const night = named("callsCell-contacts-night")
            verify(showing(night) && within(night))
            night.clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.cell.contacts.night rings")
            named("callsSimsOpen").clicked()
            compare(mock.asked[mock.asked.length - 1], "allow sims")
            save("phone-card-phone-calls")
            named("foldCalls").open = false
            // What Android must allow, in the design's order, each line its state and one button.
            named("foldAllow").open = true
            wait(20)
            const lines = walk(card, i => i.objectName.indexOf("allow-") === 0 && showing(i), [])
            compare(lines.map(l => l.objectName.slice("allow-".length)).join(" "), "role contacts history notifications texts dnd alarms battery")
            for (const line of lines)
                verify(within(line), line.objectName + ": within the page")
            const notifications = lines[3]
            walk(notifications, i => i.objectName === "allowButton", [])[0].clicked()
            compare(mock.asked[mock.asked.length - 1], "allow notifications")
            save("phone-card-allow")
            named("foldAllow").open = false
            // The restricted settings' card: three steps, each a button, ticked once done.
            named("foldRestricted").open = true
            wait(20)
            const steps = ["notice", "info", "back"].map(id => named("restrictedStep-" + id))
            verify(steps.every(s => showing(s)), "three steps")
            for (const id of ["notice", "info", "back"]) {
                walk(named("restrictedStep-" + id), i => i.objectName === "restrictedButton", [])[0].clicked()
                wait(20)
                compare(mock.asked[mock.asked.length - 1], "step " + id)
                save("phone-card-restricted-" + id)
            }
            verify(within(named("restrictedStep-info")), "the drawing within the page")
            // A block of the tab "This phone", unfolded: made then.
            named("fold-dnd").open = true
            wait(20)
            verify(named("block-dnd").item !== null, "made once unfolded")
            verify(named("fold-screening") === null || !showing(named("fold-screening")), "no call screening's block without telephony")
        }

        // A phone whose line says nothing: what it does today, nothing hidden (review of `reach`, R3).
        function test_a_phone_that_says_nothing_shows_today() {
            reload(1400, false)
            mock.today = true
            tab.reload()
            wait(50)
            verify(!showing(named("callsProfileSaid")), "no profile's line: the table is today's, not a profile's")
            compare(named("callsProfile-personal").checked, false, "nothing chosen")
            const varies = named("callsCell-contacts-day")
            compare(varies.text, "Varies")
            verify(!varies.highlighted, "a cell that varies is neither Rings nor Voicemail")
            varies.clicked()
            compare(mock.asked[mock.asked.length - 1], "say phone calls.cell.contacts.day rings")
            compare(named("callsCell-contacts-busy").text, "As usual")
            compare(all("callsDetail").length, 2, "when each varying cell rings, under the table")
            verify(showing(named("callsHere")), "calls are set here; What reaches you's presets leave them")
            verify(!showing(named("callsBack")), "nothing to take back")
            save("phone-card-today")
        }

        // The doors: Calls and each app by time, unfolded, in the card they land in.
        function test_doors_land_in_a_phone_s_card() {
            // A computer, narrow: the first phone's Calls; no app by time listed, no door for them.
            reload(412, false)
            const calls = tab.reveal("calls")
            verify(calls !== null && calls.objectName === "foldCalls" && calls.open, "Calls unfolded in the phone's card")
            verify(named("phoneCard-phone").reveal("calls") === calls)
            compare(tab.reveal("apps"), null, "no app has times of its own")
            verify(named("fold-lists") === null || !showing(named("fold-lists")))
            // Apps with times of their own: listed in the phone's card on a computer, read and changed there.
            mock.lists = 2
            tab.reload()
            wait(50)
            const lists = tab.reveal("apps")
            verify(lists !== null && lists.objectName === "fold-lists" && lists.open, "each app by time, unfolded")
            wait(50)
            verify(named("block-lists").item !== null, "made once unfolded")
            // This phone: its own card, both doors.
            reload(412, true)
            const here = named("setting:devices.here")
            const own = tab.reveal("apps")
            verify(own !== null && own.parent === here, "the lists of this phone's own card")
            verify(tab.reveal("calls").parent === here)
        }

        // A reload (every 20 s while shown, after each change) keeps the cards and what is unfolded.
        function test_a_reload_keeps_the_card() {
            reload(412, true)
            const card = named("setting:devices.here")
            named("foldCalls").open = true
            named("fold-dnd").open = true
            named("callsRow-contacts").clicked()
            wait(20)
            const dnd = named("fold-dnd")
            tab.reload()
            wait(50)
            verify(named("setting:devices.here") === card, "the same card")
            verify(named("foldCalls").open, "Calls still unfolded")
            verify(showing(named("callsCell-contacts-night")), "the row still open")
            verify(named("fold-dnd") === dnd && dnd.open, "the tab's blocks kept, unfolded")
        }
    }
}
