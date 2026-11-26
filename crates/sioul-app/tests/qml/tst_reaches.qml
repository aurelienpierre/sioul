// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ What reaches you as one page (ReachesTab.qml; docs/attention.md,
// "The page"; the redesign of 24 November 2026), on a stand-in for Sioul, at
// a computer's width and at a phone's 412 points: the usual rule in
// sentences with its one drop-down; your exceptions, each with Edit and ✕;
// the card that adds one (four drop-downs, a person found by name, your own
// times as nine moments); a start that replaces exceptions asked first;
// where a rule does nothing, said; Who is in which circle and What comes
// when, worked out, folded; the grid read-only, a cell saying which line
// decides it, with "Add an exception for this…". What the page asks reaches
// the stand-in through `reachesAct`. The view is made up, in the shape
// `reaches::view` gives. Run by tools/qml-test.sh.

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

        property var asked: []
        property var opened: []
        property string status: ""
        property bool startAsks: true
        // The moment, which the page follows (read again when it changes).
        property string mode: ""
        readonly property var mailContacts: ({ kind: "channel", channel: "mail" })
        // What `reaches::view` gives, made up.
        property var view: ({
            now: ["Now: work, until 17:00."],
            state: "usual",
            phone: false,
            start: { id: "usual", choices: [{ id: "usual", label: "As Sioul does now" }, { id: "quieter", label: "Quieter" }, { id: "reachable", label: "More reachable" }] },
            rule: [
                { circle: "always", text: "Always through: always, even through do-not-disturb.", count: "3 people" },
                { circle: "safe", text: "Safe: when you are awake.", count: "2 groups and 5 people" },
                { circle: "neutral", text: "Your contacts: when you are awake; their mail in your work and admin hours.", count: "312 cards" },
                { circle: "stranger", text: "Everyone else: in your work and admin hours, gathered outside them.", count: "" },
                { circle: "blocked", text: "Blocked: never.", count: "" }
            ],
            idle: ["None of your phones gives Sioul notification access, as each last said: the rules for messages do nothing yet."],
            exceptions: [
                { text: "Mail from your contacts: when you are awake.", note: "", value: { what: { kind: "channel", channel: "mail" }, whom: { kind: "circle", person: "neutral" }, where: { kind: "everywhere" }, when: { kind: "pattern", pattern: "awake" }, store: { kind: "rows", rows: ["mail.neutral"] } } },
                { text: "Messages from your people: always.", note: "None of your phones gives Sioul notification access, as each last said: the rules for messages do nothing yet.", value: { what: { kind: "channel", channel: "messages" }, whom: { kind: "circle", person: "people" }, where: { kind: "everywhere" }, when: { kind: "pattern", pattern: "always" }, store: { kind: "rows", rows: ["messages.people"] } } },
                { text: "Calls, on The phone: by the table set on that phone.", note: "", ask: "Taking this out removes the whole table set on that phone, its groups' rows and its switches: the phone then screens calls by the usual rule.", value: { what: { kind: "channel", channel: "calls" }, whom: { kind: "anyone" }, where: { kind: "device", id: "p1", name: "The phone" }, when: { kind: "calls", profile: "none" }, store: { kind: "phone", id: "p1" } } }
            ],
            card: {
                what: [{ value: { kind: "anything" }, label: "Mail, calls and messages" }, { value: { kind: "channel", channel: "mail" }, label: "Mail" }, { value: { kind: "channel", channel: "messages" }, label: "Messages" }],
                whom: [{ value: { kind: "anyone" }, label: "Anyone" }, { value: { kind: "circle", person: "neutral" }, label: "Your contacts" }, { value: { kind: "person", id: "", name: "" }, label: "A person…", search: true }],
                where: [{ value: { kind: "everywhere" }, label: "Every device" }, { value: { kind: "device", id: "p1", name: "The phone" }, label: "The phone" }]
            },
            now_columns: ["work"],
            grid: {
                columns: [{ id: "work", label: "Work" }, { id: "sleep", label: "Sleep" }],
                marks: [],
                rows: [
                    { id: "mail.safe", label: "Safe", group: "Mail", group_id: "mail", help: "Only you put someone on this list.", changed: false, choices: [], cells: [{ column: "work", value: "now", word: "Now", choices: ["now", "quiet"], said: "Mail, Safe, at Work: At once.", by: "Decided by the usual rule for Safe.", changed: false, except: { what: { kind: "channel", channel: "mail" }, whom: { kind: "circle", person: "safe" }, where: { kind: "everywhere" }, when: { kind: "pattern", pattern: "awake" } } }, { column: "sleep", value: "quiet", word: "Shown", choices: ["now", "quiet"], said: "Mail, Safe, at Sleep: Shown.", by: "Decided by the usual rule for Safe.", changed: false, except: null }] },
                    { id: "mail.neutral", label: "Your contacts", group: "Mail", group_id: "mail", help: "Everyone in your address books.", changed: true, choices: [], cells: [{ column: "work", value: "now", word: "Now", choices: ["now"], said: "Mail, Your contacts, at Work: At once.", by: "Decided by your exception: Mail from your contacts: when you are awake.", changed: true, except: null }, { column: "sleep", value: "later", word: "Later", choices: ["now"], said: "Mail, Your contacts, at Sleep: Later.", by: "Decided by your exception: Mail from your contacts: when you are awake.", changed: true, except: null }] }
                ]
            },
            circle: { title: "Your people", help: "Read from Contacts ▸ Your people.", people: [{ id: "lea", name: "Léa", notes: [] }] },
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
        // What the page asks (`reaches::act`), recorded; its answers made up.
        function reachesAct(verb, json) {
            const args = JSON.parse(json)
            mock.asked = mock.asked.concat([[verb, args]])
            if (verb === "offered" && args.where.kind === "device" && args.what.kind === "channel" && args.what.channel === "mail")
                return JSON.stringify({ when: [], own: false, moments: [], why: "reach-where-every" })
            if (verb === "offered" && args.what.kind === "channel" && args.what.channel === "calls" && args.where.kind === "device") {
                // A table changed cell by cell: offered only as the exception's own, while it is changed.
                const own = args.old && args.old.when.kind === "calls" ? [{ value: args.old.when, label: "By the table set on that phone", cells: null }] : []
                return JSON.stringify({ when: own.concat([{ value: { kind: "calls", profile: "professional" }, label: "As a work phone", cells: null }]), own: false, moments: [], why: "" })
            }
            if (verb === "offered")
                return JSON.stringify({ when: [{ value: { kind: "pattern", pattern: "always" }, label: "Always", cells: ["now", "now", "now", "now", "quiet", "quiet", "now", "now", "now"] }, { value: { kind: "pattern", pattern: "awake" }, label: "When you are awake", cells: null }], own: true, moments: ["work", "admin", "leisure", "meals", "sleep", "pause", "free", "slot", "dnd"].map(c => ({ column: c, label: c, levels: [{ value: "now", label: "At once" }, { value: "quiet", label: "Shown without a word" }, { value: "later", label: "Later" }] })), why: "" })
            if (verb === "find")
                return JSON.stringify(args.text.length >= 2 ? [{ value: { kind: "person", id: "contact:lea", name: "Léa Martin" }, label: "Léa Martin" }] : [])
            if (verb === "start" && mock.startAsks && !args.sure)
                return JSON.stringify({ problem: "", ask: "Starting from Quieter replaces one of your exceptions: Mail from your contacts: when you are awake." })
            return JSON.stringify({ problem: "" })
        }
        function settings(view) {
            if (view !== "senders")
                return "[]"
            const row = (key, kind, label, value, choices, group) => ({ key: key, kind: kind, label: label, help: "", value: value, choices: choices, min: 0, max: 0, step: 1, unit: "", group: group })
            return JSON.stringify([
                row("safe", "senders", "Safe", ["camille@example.org"], [], ""),
                row("contact:c-1", "choice", "Amandine", "safe", [], "People on a list"),
                row("category:Clients", "choice", "Clients", "", [], "Groups")
            ])
        }
        function reminderOpened(kind, uri, key) {
            mock.opened = mock.opened.concat([kind === "person" ? "person:" + key : kind])
        }
        function dndSetup() {
            return ""
        }
        function appNotesSetup() {
            return ""
        }
    }

    // The same, as a phone gives it: the page says this phone's own switches for other apps.
    QtObject {
        id: phoneMock

        property var asked: []
        property bool hold: true
        property bool people: false
        property bool shade: true

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
            return JSON.stringify(Object.assign({}, mock.view, { phone: true }))
        }
        function reachesAct(verb, json) {
            return mock.reachesAct(verb, json)
        }
        function settings(view) {
            return mock.settings(view)
        }
        function reminderOpened(kind, uri, key) {
        }
        function dndSetup() {
            return ""
        }
        function setup() {
            const row = (key, label, value) => ({ key: key, kind: "bool", label: label, help: "", value: value, choices: [] })
            return JSON.stringify({ android: true, access: true, restricted: false, contacts: true, hold: row("hold", "Hold other apps' notifications until their time", phoneMock.hold), people: row("people", "Hold people's messages too, by who writes and when", phoneMock.people), shade: row("shade", "Say in the phone's notifications what Sioul holds", phoneMock.shade), times: "", heard: "", rang: [] })
        }
        function appNotesSetup() {
            return phoneMock.setup()
        }
        function appNotesChange(verb, json) {
            const args = JSON.parse(json)
            phoneMock.asked = phoneMock.asked.concat([verb + " " + args.key + "=" + args.value])
            if (verb === "set" && args.key === "hold")
                phoneMock.hold = args.value
            if (verb === "set" && args.key === "people")
                phoneMock.people = args.value
            if (verb === "set" && args.key === "shade")
                phoneMock.shade = args.value
            return phoneMock.setup()
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
    ReachesTab {
        id: phonePage

        y: 1000
        width: 412
        sioul: phoneMock
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

        // Every item under `item`, shown.
        function walk(item, out) {
            if (!item || !item.visible)
                return out
            out.push(item)
            for (const kid of item.children || [])
                walk(kid, out)
            return out
        }

        function asked(verb) {
            return mock.asked.filter(a => a[0] === verb)
        }

        function init() {
            mock.asked = []
            mock.opened = []
            mock.startAsks = true
            phoneMock.asked = []
            for (const page of [wide, narrow, phonePage]) {
                page.closeChoices()
                page.show("rules", "")
                page.rowOpen = ""
                page.reload()
            }
        }

        // The rule in sentences with what each circle holds, the one drop-down, what does nothing said;
        // the exceptions, each with Edit and ✕; nothing of the older page.
        function test_the_rule_and_the_exceptions() {
            for (const page of [wide, narrow]) {
                verify(find(page, "rule-safe") !== null, "a circle's sentence")
                compare(find(page, "reachesStartFrom").currentText, "As Sioul does now")
                verify(find(page, "reachesIdle") !== null, "where a rule does nothing, said")
                verify(find(page, "exception-0") !== null && find(page, "exceptionEdit-1") !== null && find(page, "exceptionRemove-1") !== null)
                for (const gone of ["part-people", "part-own", "part-moments", "reachesPresetsNote", "reachesElsewhere", "reachesCalls", "reachesDevices", "reachesTabs"])
                    verify(find(page, gone) === null, gone + " left the page")
            }
            // ✕ takes it out, through the backend (Undo is the status line's).
            find(wide, "exceptionRemove-1").clicked()
            compare(asked("remove").length, 1)
            compare(asked("remove")[0][1].value.whom.person, "people")
        }

        // Add an exception…: the card, its four drop-downs; a person found by name; your own
        // times as nine moments, starting from the pattern chosen; saved through the backend.
        function test_the_card_adds_an_exception() {
            find(wide, "reachesAdd").clicked()
            const card = wide.card
            verify(card !== null && card.opened, "the card")
            compare(card.what.kind, "anything")
            compare(card.whom.kind, "anyone")
            compare(card.where.kind, "everywhere")
            verify(asked("offered").length > 0, "When asked for what is chosen")
            // From whom: a person…, found by name.
            card.whom = card.choices.whom[2].value
            card.searching = true
            card.find("Lé")
            compare(card.found.length, 1)
            card.whom = card.found[0].value
            card.searching = false
            card.refresh()
            compare(card.whom.id, "contact:lea")
            // Your own times: from Always's cells.
            card.when = card.offered.when[0].value
            card.chooseOwn()
            verify(card.ownChosen)
            compare(card.cells[4], "quiet", "sleep as Always says it")
            verify(find(card.contentItem, "exceptionMoments") !== null, "the nine moments shown")
            card.save()
            const saved = asked("save")
            compare(saved.length, 1)
            compare(saved[0][1].new.when.kind, "own")
            compare(saved[0][1].new.whom.id, "contact:lea")
            compare(saved[0][1].old, null)
            verify(!card.opened, "closed once saved")
        }

        // Edit: the card filled with the exception, saved with it as the one it replaces.
        function test_edit_changes_the_one_it_was() {
            find(wide, "exceptionEdit-0").clicked()
            const card = wide.card
            verify(card !== null && card.opened)
            compare(card.when.pattern, "awake")
            card.when = card.offered.when[0].value
            card.save()
            const saved = asked("save")
            compare(saved[0][1].old.store.rows[0], "mail.neutral")
            compare(saved[0][1].new.when.pattern, "always")
        }

        // A start that replaces exceptions: asked first, then taken.
        function test_a_start_is_asked_first() {
            wide.chooseStart("quieter", false)
            verify(find(wide, "reachesStartAsk") !== null, "the question")
            compare(asked("start")[0][1].sure, false)
            wide.chooseStart(wide.startAsked, true)
            compare(asked("start")[1][1].sure, true)
            verify(find(wide, "reachesStartAsk") === null)
        }

        // What comes when, worked out: folded; open, read-only, a cell says which line decides it
        // and offers an exception for it, the card starting from that cell's row.
        function test_the_worked_out_grid_explains() {
            verify(find(wide, "attentionGrid") === null, "folded")
            wide.show("table", "")
            tryVerify(() => find(wide, "attentionGrid") !== null)
            compare(wide.view, "table", "Settings wider")
            const grid = find(wide, "attentionGrid")
            grid.openAt("mail.safe", "work")
            compare(grid.lines[1].note, "Decided by the usual rule for Safe.")
            compare(grid.lines[2].id, "except")
            grid.except(grid.openCell.except)
            const card = wide.card
            verify(card !== null && card.opened)
            compare(card.whom.person, "safe")
            card.close()
            // A door to another part folds it again.
            wide.show("dnd", "")
            compare(wide.view, "rules")
        }

        // On a phone: the grid as rows that open, a moment saying which line decides it; nothing wider than the screen.
        function test_a_phone_s_rows_open() {
            narrow.show("table", "")
            tryVerify(() => find(narrow, "row-mail.safe") !== null)
            verify(find(narrow, "attentionGrid") === null, "no table on a phone")
            find(narrow, "row-mail.safe").clicked()
            tryVerify(() => find(narrow, "line-mail.safe-work") !== null)
            for (const item of walk(narrow, [])) {
                if (item.width > 0 && item.mapToItem && item.objectName !== "")
                    verify(item.mapToItem(narrow, 0, 0).x + item.width <= narrow.width + 1, item.objectName + " passes the right edge")
            }
        }

        // Who is in which circle, folded: Always through, your people, the lists of addresses and
        // numbers; a person's list and a group's left it (their sheet; an exception).
        function test_who_is_in_which_circle() {
            wide.show("circles", "")
            tryVerify(() => find(wide, "close-lea") !== null)
            compare(wide.lists.map(r => r.key), ["safe"])
        }

        // The doors lead to their part: the rule, the exceptions, do-not-disturb, other apps.
        function test_doors_lead_to_their_part() {
            for (const [view, detail, part] of [["people", "", "part-rule"], ["exceptions", "", "part-exceptions"], ["dnd", "", "part-dnd"], ["rules", "mail", "part-rule"], ["circles", "", "part-circles"]]) {
                revealed.clear()
                wide.show(view, detail)
                tryCompare(revealed, "count", 1, 1000, view)
                compare(revealed.signalArguments[0][0].objectName, part)
            }
        }

        // The three states: the one on drawn as selected; the other two words read, never framed as buttons.
        function test_the_states_are_read_not_pressed() {
            compare(find(wide, "state-usual").background.border.width, 2)
            compare(find(wide, "state-dnd").background.border.width, 0)
            compare(find(wide, "state-reachable").background.border.width, 0)
        }

        // ✕ on a phone's table of calls asks first, under its line; Keep it, then Take it out.
        function test_a_phone_s_table_asks_before_it_is_taken_out() {
            find(wide, "exceptionRemove-2").clicked()
            compare(asked("remove").length, 0, "not taken out yet")
            verify(find(wide, "exceptionAsk-2") !== null, "the question under its line")
            find(wide, "exceptionAskKeep-2").clicked()
            verify(find(wide, "exceptionAsk-2") === null)
            compare(asked("remove").length, 0)
            find(wide, "exceptionRemove-2").clicked()
            find(wide, "exceptionAskTake-2").clicked()
            compare(asked("remove").length, 1)
            compare(asked("remove")[0][1].value.store.kind, "phone")
            // Another line is taken out at once.
            find(wide, "exceptionRemove-0").clicked()
            compare(asked("remove").length, 2)
        }

        // Edit never rewrites what the card cannot say: the table set cell by cell stays chosen,
        // asked with the exception it changes, and saved as it was.
        function test_edit_keeps_what_the_card_cannot_say() {
            find(wide, "exceptionEdit-2").clicked()
            const card = wide.card
            verify(card !== null && card.opened)
            const offered = asked("offered")
            compare(offered[offered.length - 1][1].old.store.kind, "phone", "asked with the exception it changes")
            compare(card.when.profile, "none", "its own When, not the first offered")
            card.save()
            compare(asked("save")[0][1].new.when.profile, "none")
        }

        // Nothing to choose for a device that cannot keep it: the card says why.
        function test_the_card_says_why_nothing_is_offered() {
            find(wide, "reachesAdd").clicked()
            const card = wide.card
            card.what = card.choices.what[1].value
            card.where = card.choices.where[1].value
            card.refresh()
            const why = find(card.contentItem, "exceptionWhy")
            verify(why !== null)
            compare(why.text, "reach-where-every")
            card.close()
        }

        // Your own times kept moment by moment where still offered when What changes.
        function test_own_times_keep_what_is_offered() {
            find(wide, "reachesAdd").clicked()
            const card = wide.card
            card.whom = card.choices.whom[1].value
            card.refresh()
            card.when = card.offered.when[0].value
            card.chooseOwn()
            const cells = card.cells.slice()
            card.cells = cells.map((c, i) => i === 0 ? "nowhere" : c)
            card.what = card.choices.what[2].value
            card.refresh()
            compare(card.cells[0], "now", "a level not offered there: the moment's first")
            compare(card.cells[4], cells[4], "the others kept")
            card.close()
        }

        // Enter takes the card's main action.
        function test_enter_takes_the_main_action() {
            find(wide, "exceptionEdit-0").clicked()
            const card = wide.card
            verify(card.opened)
            card.forceActiveFocus()
            keyClick(Qt.Key_Return)
            tryVerify(() => asked("save").length === 1, 1000, "saved by Enter")
            verify(!card.opened)
        }

        // On a phone: this phone's own switches for other apps under "Other apps on this phone"
        // (moved from its card in Devices, whose test held these): people's messages held too, off as
        // worked out where nothing was chosen, under holding; the line in the shade under it, on unless
        // turned off; each set through the phone's own choices; holding off takes the other two away.
        function test_a_phone_s_own_section() {
            phoneMock.hold = true
            phoneMock.people = false
            phoneMock.shade = true
            phonePage.reload()
            phonePage.show("apps", "")
            tryVerify(() => find(phonePage, "apps-hold") !== null)
            const hold = find(phonePage, "apps-hold")
            const people = find(phonePage, "apps-people")
            const shade = find(phonePage, "apps-shade")
            verify(people !== null && shade !== null, "shown while holding is on")
            verify(hold.setting.value === true && people.setting.value === false && shade.setting.value === true)
            verify(hold.mapToItem(phonePage, 0, 0).y < people.mapToItem(phonePage, 0, 0).y, "people's under the master switch")
            verify(people.mapToItem(phonePage, 0, 0).y < shade.mapToItem(phonePage, 0, 0).y, "the line in the shade under people's")
            verify(find(phonePage, "setting:notices.informative_minutes") === null, "a computer's own")
            compare(find(phonePage, "part-apps").title, "reach-apps")
            // People's messages turned on, the line turned off: said to the phone's choices, shown as they came back.
            people.save("people", true, false)
            compare(phoneMock.asked[0], "set people=true")
            tryVerify(() => find(phonePage, "apps-people").setting.value === true)
            find(phonePage, "apps-shade").save("shade", false, true)
            compare(phoneMock.asked[1], "set shade=false")
            tryVerify(() => find(phonePage, "apps-shade").setting.value === false)
            // Holding off: the other two leave.
            find(phonePage, "apps-hold").save("hold", false, true)
            compare(phoneMock.asked[2], "set hold=false")
            tryVerify(() => find(phonePage, "apps-people") === null && find(phonePage, "apps-shade") === null)
            find(phonePage, "apps-hold").save("hold", true, false)
            tryVerify(() => find(phonePage, "apps-people") !== null)
            for (const item of walk(phonePage, [])) {
                if (item.width > 0 && item.mapToItem && item.objectName !== "")
                    verify(item.mapToItem(phonePage, 0, 0).x + item.width <= phonePage.width + 1, item.objectName + " passes the right edge")
            }
        }
    }
}
