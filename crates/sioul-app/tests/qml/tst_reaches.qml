// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ What reaches you as one page (ReachesTab.qml; docs/attention.md,
// "The interface"), on a stand-in for Sioul, at a computer's width and at a
// phone's 412 points: Your rules (the people's table, a circle's behaviours,
// Sioul's own groups and care switches, the moments, the line of what is set
// elsewhere, the calls' line) and What comes when (the grid in words, now's
// columns marked; on a phone, a row's values). Each choice reaches Sioul as
// the rows it writes (`reachesChoose`), a cell as its row (`reachesSet`).
// The view is made up, in the shape `reaches::view` gives. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1344
    height: 900

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var chosen: []
        property var set: []
        property var opened: []
        property string status: ""
        // What `reaches::view` gives, made up.
        property var view: ({
            now: ["Now: work, until 17:00."],
            state: "usual",
            column: "work",
            now_columns: ["work"],
            presets: [{ id: "usual", label: "As Sioul does now", current: true, changes: 0 }, { id: "quieter", label: "Quieter", current: false, changes: 12 }, { id: "reachable", label: "More reachable", current: false, changes: 6 }],
            presets_note: "A preset leaves calls as they are.",
            changed: "",
            people: {
                channels: [{ id: "mail", label: "Mail", note: "" }, { id: "messages", label: "Messages", note: "" }],
                rows: [
                    { id: "always", label: "Always through", help: "The people on your Always through list.", cells: [{ channel: "mail", value: "always", label: "Always", choices: [{ id: "always", label: "Always", help: "" }, { id: "awake", label: "When awake", help: "" }], rows: ["mail.always"] }, { channel: "messages", value: "always", label: "Always", choices: [{ id: "always", label: "Always", help: "" }], rows: ["messages.always"] }] },
                    { id: "safe", label: "Safe", help: "Only you put someone here.", cells: [{ channel: "mail", value: "awake", label: "When awake", choices: [{ id: "always", label: "Always", help: "" }, { id: "awake", label: "When awake", help: "" }, { id: "never", label: "Never told", help: "Mail stays on the Porch without a word; a message is held." }], rows: ["mail.safe"] }, { channel: "messages", value: "awake", label: "When awake", choices: [{ id: "awake", label: "When awake", help: "" }, { id: "work", label: "In work hours", help: "" }], rows: ["messages.safe"] }] },
                    { id: "contacts", label: "Your contacts", help: "Everyone in your address books.", cells: [{ channel: "mail", value: "own", label: "Your own", choices: [{ id: "work-admin", label: "In work and admin hours", help: "" }, { id: "awake", label: "When awake", help: "" }], rows: ["mail.neutral"] }, { channel: "messages", value: "awake", label: "When awake", choices: [{ id: "awake", label: "When awake", help: "" }, { id: "work-admin", label: "In work and admin hours", help: "" }], rows: ["messages.neutral", "messages.groups"] }] }
                ],
                blocked: { label: "Blocked", help: "Never, on any channel.", word: "Never" },
                notes: ["A message is told only while what it was written to, an address or an app, is for now."],
                own_help: "Set cell by cell, in What comes when."
            },
            own: [
                { id: "asked", label: "What you set or asked for", holds: "Codes, doses, the alarm at waking and an event's alarms always come at once.", value: "sleep", value_label: "Doses while you sleep", choices: [{ id: "sleep", label: "Doses while you sleep" }, { id: "waking", label: "Doses at waking" }], switches: [], gathered: "" },
                { id: "reminders", label: "Reminders", holds: "Before an event, the working day before, dates, waits, payments and papers.", value: "own", value_label: "Your own", choices: [{ id: "awake", label: "When awake" }, { id: "off", label: "Off" }], switches: [], gathered: "" },
                { id: "care", label: "Care", holds: "Notices that look after your day.", value: "on", value_label: "", choices: [], switches: [{ kind: "needs", label: "Meals, naps and the night", on: true }, { kind: "move", label: "The pause to move", on: true }], gathered: "" },
                { id: "sites", label: "Sites and apps", holds: "Sites' news on a computer, apps' automatic notifications on a phone.", value: "gathered", value_label: "At the gathered times", choices: [{ id: "now", label: "At once" }, { id: "gathered", label: "At the gathered times" }], switches: [], gathered: "Gathered at 09:00, 13:00 and 18:00." }
            ],
            moments: {
                columns: ["Who passes", "Turned on by", "On your phone"],
                rows: [{ id: "usual", label: "As usual", who: "Your rules above", by: "Nothing", phone: "As Android is set" }, { id: "dnd", label: "Do not disturb", who: "Always through, and what you set", by: "The switch, and the boxes below", phone: "Sioul's own mode" }],
                holds_title: "Who holds what on a phone",
                holds_columns: ["Sioul", "Android"],
                holds: [{ id: "call", label: "A call", sioul: "Rings, or goes to voicemail", android: "Whether a ringing call sounds" }],
                principle: "Sioul decides what gets through, and when; Android's do-not-disturb only decides whether it makes a sound."
            },
            elsewhere: { line: "Set elsewhere: one site in real time.", links: [{ id: "sites", label: "Sites" }] },
            calls: "Calls are set for each phone in Settings ▸ Devices.",
            rows: ["mail.safe", "calls.neutral", "doses"],
            calls_grid: { text: "reach-calls-grid", door_label: "reach-calls-grid-open", door: "settings:devices.calls" },
            grid: {
                columns: [{ id: "work", label: "Work" }, { id: "sleep", label: "Sleep" }],
                marks: [],
                rows: [
                    { id: "mail.safe", label: "Safe", group: "Mail", group_id: "mail", help: "Only you put someone on this list.", changed: false, choices: [{ id: "now", label: "At once", word: "Now" }, { id: "quiet", label: "Shown, not told", word: "Shown" }], cells: [{ column: "work", value: "now", word: "Now", choices: ["now", "quiet"], said: "Mail, Safe, at Work: At once." }, { column: "sleep", value: "quiet", word: "Shown", choices: ["now", "quiet"], said: "Mail, Safe, at Sleep: Shown, not told." }] },
                    { id: "calls.neutral", label: "Your contacts", group: "Calls", group_id: "calls", help: "Everyone in your address books.", changed: false, choices: [{ id: "now", label: "At once", word: "Now" }, { id: "later", label: "Voicemail", word: "Voicemail" }], cells: [{ column: "work", value: "now", word: "Now", choices: ["now", "later"], said: "Calls, Your contacts, at Work: At once." }, { column: "sleep", value: "later", word: "Voicemail", choices: ["now", "later"], said: "Calls, Your contacts, at Sleep: Voicemail." }] },
                    { id: "doses", label: "Doses", group: "What you set or asked for", group_id: "asked", help: "A dose's reminder at its time.", changed: false, choices: [{ id: "now", label: "At once", word: "Now" }, { id: "later", label: "Later", word: "Later" }], cells: [{ column: "work", value: "now", word: "Now", choices: ["now"], locked: "Fixed: a dose comes at its time.", said: "Doses, at Work: At once." }, { column: "sleep", value: "now", word: "Now", choices: ["now", "later"], said: "Doses, at Sleep: At once." }] }
                ]
            },
            phone: false,
            exact: true
        })

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function textArgs(key, args) {
            return key
        }
        function reachesView() {
            return JSON.stringify(mock.view)
        }
        // Who is in which circle (`settings::for_view("senders")`): a list, the line of
        // what was named as your contacts (review C1), a group on that list.
        function settings(view) {
            if (view !== "senders")
                return "[]"
            const row = (key, kind, label, value, choices, group) => ({ key: key, kind: kind, label: label, help: "", value: value, choices: choices, min: 0, max: 0, step: 1, unit: "", group: group })
            const lists = [{ value: "", label: "No list" }, { value: "safe", label: "Safe" }, { value: "neutral", label: "Your contacts" }]
            return JSON.stringify([
                row("safe", "senders", "Safe", ["camille@example.org"], [], ""),
                row("neutral", "senders", "2 addresses, numbers and domains you named as contacts", ["*@school.example.org", "+33 1 99 00 12 34"], [], ""),
                row("category:Clients", "choice", "Clients", "neutral", lists, "Groups")
            ])
        }
        function reachesChoose(what, value) {
            mock.chosen = mock.chosen.concat([[what, value]])
            return ""
        }
        function reachesSet(row, words, shown) {
            mock.set = mock.set.concat([[row, JSON.parse(words), JSON.parse(shown)]])
            return ""
        }
        function reachesPreset(id) {
            return ""
        }
        function reminderOpened(kind, uri, key) {
            mock.opened = mock.opened.concat([kind])
        }
        function dndSetup() {
            return ""
        }
    }

    // A computer's page and a phone's, side by side.
    ReachesTab {
        id: wide

        width: 1000
        sioul: mock
        theme: testTheme
        rows: []
    }
    ReachesTab {
        id: narrow

        x: 1010
        width: 412
        sioul: mock
        theme: testTheme
        rows: []
    }
    SignalSpy {
        id: revealed

        target: wide
        signalName: "reveal"
    }

    TestCase {
        name: "Reaches"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name && item.visible)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            if (item.contentItem && item.contentItem !== item) {
                const inside = find(item.contentItem, name)
                if (inside)
                    return inside
            }
            return null
        }

        // An item of an open popup, by its objectName (a menu's lines live in the overlay).
        function inOverlay(name) {
            return find(Overlay.overlay, name)
        }

        function init() {
            mock.chosen = []
            mock.set = []
            mock.opened = []
            for (const page of [wide, narrow]) {
                page.closeChoices()
                page.view = "rules"
                page.rowOpen = ""
            }
        }

        // The calls' rows in What comes when, and under them the line that a phone's own table
        // replaces them but in a pause and during do-not-disturb, with its door (review of `phones`).
        function test_the_calls_rows_in_the_full_ledger() {
            find(wide, "reachesTabs:table").clicked()
            verify(find(wide, "cell-calls.neutral-work") !== null, "a calls' row, every column")
            verify(find(wide, "groupNote-calls") !== null, "its line under it")
            mock.opened = []
            find(wide, "groupDoor-calls").clicked()
            compare(mock.opened, ["settings:devices.calls"])
            find(narrow, "reachesTabs:table").clicked()
            verify(find(narrow, "row-calls.neutral") !== null && find(narrow, "callsGridNote") !== null)
            find(wide, "reachesTabs:rules").clicked()
            find(narrow, "reachesTabs:rules").clicked()
        }

        // A phone's top in three rows before the tabs (the pictures' review, point 1): the
        // sentence, the states (pressed, the card), Start from as one button opening the presets
        // with the calls' line; a computer keeps Change… and the presets side by side.
        function test_a_phone_s_top_in_three_rows() {
            verify(find(narrow, "reachesStates") === null, "no Change… on a phone: the row opens the card")
            verify(find(wide, "reachesStates") !== null)
            const start = find(narrow, "reachesStartFrom")
            verify(start !== null && start.text.indexOf("As Sioul does now") >= 0, start ? start.text : "none")
            verify(find(narrow, "preset-quieter") === null && find(wide, "preset-quieter") !== null)
            verify(find(narrow, "reachesPresetsNote") === null, "the calls' line is in the phone's menu")
        }

        // Nothing at all under its own heading; the grid's words said under it (points 4 and 5).
        function test_free_time_and_the_grid_s_key() {
            verify(find(wide, "reachesGridKey") === null, "the key with the grid only")
            find(wide, "reachesTabs:table").clicked()
            verify(find(wide, "reachesGridKey") !== null)
            find(wide, "reachesTabs:rules").clicked()
        }

        // The state on is said by a mark and to a screen reader, not by colour alone (review W1).
        function test_the_state_on_is_said() {
            const usual = find(wide, "state-usual")
            verify(usual.text.indexOf("✓") === 0, usual.text)
            compare(usual.Accessible.name, "reach-moment-usual, reach-state-now")
            const dnd = find(wide, "state-dnd")
            compare(dnd.Accessible.name, "reach-moment-dnd")
        }

        // What was named as your contacts (review C1): one line, opened to read and remove, never
        // to add to; a group on that list says so, not "No list".
        function test_what_was_named_as_contacts() {
            wide.readLists()
            const line = find(wide, "namedContacts")
            verify(line, "the line")
            verify(line.text.indexOf("2 addresses, numbers and domains you named as contacts") >= 0, line.text)
            verify(!find(wide, "namedContactsList"), "folded at first")
            line.clicked()
            const list = find(wide, "namedContactsList")
            verify(list, "opened")
            const named = wide.lists.find(l => l.key === "neutral")
            compare(named.value.length, 2)
            const group = wide.lists.find(l => l.key === "category:Clients")
            compare(group.value, "neutral", "the group shows its list")
            line.clicked()
            verify(!find(wide, "namedContactsList"), "folded again")
        }

        // A computer: a table, the circles down, mail and messages across, each cell its behaviour in words.
        function test_the_people_s_table_on_a_computer() {
            verify(!wide.narrow && narrow.narrow)
            const cell = find(wide, "choice-safe-mail")
            verify(cell !== null, "a cell per circle and channel")
            verify(cell.text.indexOf("When awake") === 0, cell.text)
            const own = find(wide, "choice-contacts-mail")
            verify(own.text.indexOf("Your own") === 0 && own.font.italic, "a row set cell by cell says so")
            compare(find(wide, "circle-safe"), null, "no phone's lines on a computer")
            // A press: its choices; one chosen, its rows written whole.
            cell.clicked()
            const choice = inOverlay("behaviour-mail-never")
            verify(choice !== null, "Never told offered")
            choice.triggered()
            compare(mock.chosen, [["people.mail.safe", "never"]])
        }

        // A phone: a line per circle, its two choices under it, a press opening both.
        function test_the_people_as_lines_at_412() {
            compare(find(narrow, "choice-safe-mail"), null, "no grid of cells on a phone")
            const line = find(narrow, "circle-contacts")
            verify(line !== null && line.width <= 412)
            line.clicked()
            verify(inOverlay("behaviour-mail-awake") !== null && inOverlay("behaviour-messages-work-admin") !== null, "both channels' choices")
            inOverlay("behaviour-messages-work-admin").triggered()
            compare(mock.chosen, [["people.messages.contacts", "work-admin"]])
        }

        // Sioul's own: a radio per choice, none ticked when the rows are your own; care, a switch each.
        function test_sioul_s_own_groups_and_care() {
            for (const page of [wide, narrow]) {
                mock.chosen = []
                const sleep = find(page, "group-asked-sleep")
                verify(sleep.checked, "the choice the rows read")
                const off = find(page, "group-reminders-off")
                verify(!off.checked && !find(page, "group-reminders-awake").checked, "your own: none ticked")
                off.clicked()
                const move = find(page, "care-move")
                verify(move.checked)
                move.toggle()
                move.toggled()
                compare(mock.chosen, [["group.reminders", "off"], ["care.move", "off"]])
            }
        }

        // Moments, elsewhere and calls: said, each with its door.
        function test_moments_elsewhere_and_calls() {
            verify(find(wide, "part-moments") !== null)
            verify(find(wide, "reachesHolds").text === "Who holds what on a phone")
            verify(find(wide, "reachesElsewhere").text.indexOf("one site") > 0)
            find(wide, "reachesDevices").clicked()
            compare(mock.opened, ["settings:devices"])
            compare(find(wide, "state-usual").font.weight, Font.DemiBold, "the state now marked")
        }

        // A door from elsewhere: the part of Your rules, or What comes when for a time.
        function test_doors_lead_to_their_part() {
            revealed.clear()
            wide.show("dnd", "")
            compare(wide.view, "rules")
            tryCompare(revealed, "count", 1)
            compare(revealed.signalArguments[0][0].objectName, "part-moments")
            wide.show("time", "pause")
            compare(wide.view, "table")
            wide.show("rules", "mail")
            compare(wide.view, "rules")
            // A door to a row lands on the row (review W4): in sight on a computer, opened on a phone.
            revealed.clear()
            wide.show("time", "doses")
            compare(wide.view, "table")
            tryCompare(revealed, "count", 1)
            compare(revealed.signalArguments[0][0].objectName, "cell-doses-work")
            narrow.show("time", "doses")
            compare(narrow.rowOpen, "doses")
            narrow.show("time", "pause")
            compare(narrow.rowOpen, "", "a time is no row")
        }

        // What comes when, on a computer: the grid in words, now's column marked; a cell changed by its row.
        function test_what_comes_when_in_words() {
            find(wide, "reachesTabs:table").clicked()
            compare(wide.view, "table")
            const grid = find(wide, "attentionGrid")
            verify(grid !== null && grid.words, "the grid, in words")
            compare(grid.now, ["work"])
            const cell = find(grid, "cell-mail.safe-sleep")
            verify(cell !== null)
            wide.openCell("mail.safe", "sleep")
            compare(grid.openCell.column, "sleep", "its choices open")
            grid.choose("now")
            compare(mock.set.length, 1)
            compare(mock.set[0][0], "mail.safe")
            compare(mock.set[0][1], ["work", "sleep"])
        }

        // What comes when, at 412: a line per row, now's word beside it; a row's values, a value's choices.
        function test_what_comes_when_at_412() {
            narrow.view = "table"
            compare(find(narrow, "attentionGrid"), null, "no grid of marks on a phone")
            const row = find(narrow, "row-mail.safe")
            verify(row !== null)
            row.clicked()
            compare(narrow.rowOpen, "mail.safe")
            const line = find(narrow, "line-mail.safe-sleep")
            verify(line !== null)
            line.clicked()
            narrow.chooseCell(narrow.openRowData, "sleep", "now")
            compare(mock.set[0][0], "mail.safe")
            compare(mock.set[0][1], ["work", "sleep"])
            find(narrow, "rowBack").clicked()
            compare(narrow.rowOpen, "")
        }
    }
}
