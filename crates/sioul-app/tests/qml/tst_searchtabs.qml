// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Search tab of Tasks and of the Agenda as their pages show it (20
// November 2026), on a stand-in for Sioul that answers as the window's
// search does (`thingsearch.rs`: the page's agenda filter applied, what it
// leaves out counted, every agenda on asking) and changes as Sioul does
// (a turn of the filter reaches the page later, the Agenda by its days, the
// Tasks page by its tasks only when a list of tasks turned). In Tasks, the
// page's own project, type and category, their fold on a phone and the line
// that adds a task are not shown in Search and take no room, and come back
// in the other tabs as they were; the search starts on them. In both pages
// the agenda filter is the page's, at its place, and narrows what is found;
// both Search tabs hold the same controls in the same order; nothing is wider
// than a phone, in French. Fiction only. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1000
    height: 860

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal linksChanged
        signal porchChanged
        // The page's tasks and the agenda's days, as Sioul says them.
        property string tasks: ""
        property string agenda: ""
        property string overlaps: "[]"
        property string doseMarks: ""
        property real doseReadAt: 0
        property bool replanDue: false
        property string status: ""
        // French words when a test asks (as the program's fr/sioul.ftl says them).
        property var words: ({})
        // The agendas turned off by hand, by place.
        property var off: []
        property var asked: []
        // The Tasks page's kind and category, kept by Sioul (`filterTasks`).
        property string kind: "call"
        property string category: "Papers"

        // Lists and calendars (CalDAV collections): home holds tasks and events, work tasks, club events.
        readonly property var agendas: [{ id: "dav/home", name: "Home", tasks: true, events: true }, { id: "dav/work", name: "Work", tasks: true, events: false }, { id: "dav/club", name: "Club", tasks: false, events: true }]
        readonly property var things: [
            { thing: "task", key: "/t/form.ics", uid: "form", title: "Fill the form", place: "", at: 200, when: "coming", date: "Thu 12 Nov", projects: ["Home"], project_ids: ["home"], kind: "call", categories: ["Papers"], status: "needs-action", done: false, list_id: "dav/home" },
            { thing: "task", key: "/t/report.ics", uid: "report", title: "Form for the report", place: "", at: 300, when: "coming", date: "Fri 13 Nov", projects: ["Home"], project_ids: ["home"], kind: "call", categories: ["Papers"], status: "needs-action", done: false, list_id: "dav/work" },
            { thing: "event", key: "/c/home/dentist.ics", uid: "dentist", title: "Dentist form", place: "", at: 100, when: "coming", date: "Tue 10 Nov, 14:00", projects: [], project_ids: [], kind: "", categories: [], status: "confirmed", done: false, calendar: "dav/home" },
            { thing: "event", key: "/c/club/choir.ics", uid: "choir", title: "Choir form", place: "", at: 400, when: "coming", date: "Sat 14 Nov, 18:00", projects: [], project_ids: [], kind: "", categories: [], status: "confirmed", done: false, calendar: "dav/club" }
        ]

        function text(key) { return mock.words[key] || key }
        function textWith(key, name, value) { return mock.text(key) + " " + value }
        function textArgs(key, args) { return mock.text(key) }
        function textCounted(key, n) {
            if (key === "task-filters")
                return n === 0 ? mock.text("task-filters-none") : mock.text("task-filters-none") + " (" + n + ")"
            if (key === "things-left-out")
                return (mock.words["things-left-out"] || "things-left-out") + ":" + n
            return key + ":" + n
        }

        // The filter as a full view lists it: the agendas holding tasks, or events.
        function filterShown(tasks) {
            const listed = mock.agendas.filter(a => tasks ? a.tasks : a.events)
            return { chosen: mock.off.length > 0, groups: [{ key: "yours:", title: "Yours", agendas: listed.map(a => ({ id: a.id, name: a.name, on: mock.off.indexOf(a.id) < 0, usual: true })) }] }
        }

        // As `thingsearch.rs` answers: what is searched, the page's; the words; the page's
        // filter applied (a task by its list, an event by its calendar), unless every agenda.
        function findThings(text) {
            const query = JSON.parse(text)
            mock.asked = mock.asked.concat([query])
            const tasks = query.which === "tasks"
            const words = (query.words || "").toLowerCase().split(" ").filter(w => w !== "")
            const matching = mock.things.filter(t => (tasks ? t.thing === "task" : t.thing === "event") && words.every(w => t.title.toLowerCase().indexOf(w) >= 0) && (query.project === "" || t.project_ids.indexOf(query.project) >= 0) && (query.kind === "" || t.kind === query.kind) && (query.category === "" || t.categories.indexOf(query.category) >= 0))
            const place = t => t.thing === "task" ? t.list_id : t.calendar
            const off = query.every_agenda ? [] : mock.off
            const found = matching.filter(t => off.indexOf(place(t)) < 0)
            return JSON.stringify({ projects: [{ id: "home", label: "Home" }], kinds: [{ id: "call", label: "Call" }], categories: ["Papers"], lists: [], found: found, left_out: matching.length - found.length, agendas: mock.filterShown(tasks) })
        }

        // A turn of the filter from a full view (`turn_agendas_by_hand`): written, then the
        // agenda job and the tasks' replan say it later. The tasks' answer changes only when
        // a list of tasks turned (`FilterNow::shown_as(true, …)` lists those alone).
        function turnAgendasByHand(ids, on) {
            const turned = JSON.parse(ids)
            mock.off = on ? mock.off.filter(id => turned.indexOf(id) < 0) : mock.off.concat(turned.filter(id => mock.off.indexOf(id) < 0))
            const tasksTurned = turned.some(id => mock.agendas.some(a => a.id === id && a.tasks))
            Qt.callLater(() => {
                mock.agenda = mock.agendaText()
                if (tasksTurned)
                    mock.tasks = mock.tasksText()
            })
            return ""
        }
        function turnAgendas(ids, on) { return mock.turnAgendasByHand(ids, on) }
        function agendasAsHours() { mock.off = []; return "" }
        function filterTasks(kind, category) {
            mock.kind = kind
            mock.category = category
            mock.tasks = mock.tasksText()
        }

        function tasksText() {
            return JSON.stringify({
                view: { by: "project", done: false, project: "home", filter: { kind: mock.kind, category: mock.category } },
                kinds: [{ id: "call", label: "Call" }],
                categories: ["Papers"],
                weather: "clear",
                now: { weather: "clear", office_note: "", tied_note: "", weather_note: "", now: null, why: [], picked: "", then: null, others: [], started: [], wip: "", empty: "", loops: [], done_week: [], joy: [], rest: null },
                list: { groups: [] },
                board: { columns: [] },
                timeline: { rows: [], days: [] },
                lists: [{ id: "dav/home", name: "Home", color: null, pending: false, tasks: true }],
                projects: [{ id: "home", title: "Home" }],
                no_list: false,
                quiet: false,
                first_step: "",
                first_step_task: "",
                budget: "",
                free: [],
                agendas: mock.filterShown(true),
                agendas_full: mock.filterShown(true)
            })
        }

        function agendaText() {
            return JSON.stringify({ version: 3, from: "2099-01-05", sentence: "", days: [], comes: true, filter: mock.filterShown(false), hidden: mock.off })
        }

        // The rest of what the pages ask, answered with nothing.
        function stopped() { return "null" }
        function forgetNewTasks() {}
        function showTasksAnyway(on) {}
        function showTasks(by, done, query, project) {}
        function listAccounts() { return "[]" }
        function foldTask(uid, open) {}
        function setTaskStatus(uid, status) { return "" }
        function focusStart(uid, minutes) {}
        function agendaView() { return "{}" }
        function setAgendaView(view) { return "" }
        function showDays(from, count, upcoming) {}
        function refreshHealth() {}
        function relayToday() {}
        function holderQuestions() { return "[]" }
    }

    QtObject {
        id: shell

        property var moment: ({ quiet: false, sleep: false, reason: "", line: "" })
        property var cardHost: null
        property var cardRef: null
        property string today: "2099-01-05"
        property real now: new Date(2099, 0, 5, 10).getTime() / 1000
        property real backAt: 0
        property bool compact: false
        property bool railShown: true
        function openCard(ref) {}
        function openTask(uid) {}
    }

    Component {
        id: tasksComponent

        TasksPage {
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    Component {
        id: agendaComponent

        AgendaPage {
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    function all(item, test, out) {
        out = out || []
        if (!item)
            return out
        for (let i = 0; i < item.children.length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            root.all(child, test, out)
        }
        return out
    }

    function named(item, name) {
        return root.all(item, c => c.objectName === name)[0]
    }

    // Shown: itself and every parent up to the page, with a width.
    function seen(item) {
        return item !== undefined && item !== null && item.visible && item.width > 0
    }

    // Within an item named one of `names`: a row of tabs scrolls sideways by design (Tabs.qml).
    function inside(item, names) {
        for (let at = item; at; at = at.parent) {
            if (names.indexOf(at.objectName) >= 0)
                return true
        }
        return false
    }

    // The words as fr/sioul.ftl says them (20 November 2026), for the widths.
    readonly property var french: ({
        "things-words": "Des mots du titre, des notes ou du lieu",
        "things-any-project": "Tous les projets",
        "things-no-project": "Sans projet",
        "things-any-kind": "Tous les types",
        "things-no-kind": "Sans type",
        "things-any-category": "Toutes les catégories",
        "things-no-category": "Sans catégorie",
        "things-anytime": "Toutes les dates",
        "things-range": "Entre deux jours",
        "things-from": "Du",
        "things-to": "Au",
        "things-guests-any": "Avec ou sans invités",
        "things-guests-yes": "Avec des invités",
        "things-guests-no": "Sans invités",
        "things-status-confirmed": "Confirmé",
        "things-status-tentative": "À confirmer",
        "things-status-cancelled": "Annulé",
        "things-done-any": "Faites ou non",
        "things-done-no": "Pas faites",
        "things-done-yes": "Faites",
        "things-due-any": "Avec ou sans date demandée",
        "things-due-yes": "Avec une date demandée",
        "things-due-no": "Sans date demandée",
        "things-coming": "À venir",
        "things-none-found": "Rien ne correspond.",
        "things-none-found-filtered": "Rien ne correspond dans les agendas cochés.",
        "things-left-out": "Deux résultats dans des agendas non cochés.",
        "things-show-all": "Les afficher tous",
        "things-every-agenda": "Tous les agendas, pour cette recherche.",
        "things-agendas-shown": "Revenir aux agendas cochés",
        "batch-select": "Sélectionner",
        "task-filters-none": "Filtres",
        "routines": "Routines",
        "agendas-filter": "Agendas",
        "ui-new-event": "Nouvel événement",
        "ui-tasks": "Tâches",
        "ui-agenda": "Agenda",
        "task-mode-now": "Maintenant",
        "task-mode-day": "La journée",
        "task-mode-list": "Liste",
        "task-mode-board": "Tableau",
        "task-mode-timeline": "Gantt",
        "task-mode-search": "Chercher",
        "agenda-mode-agenda": "Ce qui vient",
        "agenda-mode-day": "Jour",
        "agenda-mode-week": "Semaine",
        "agenda-mode-month": "Mois",
        "agenda-mode-search": "Chercher",
        "agenda-today": "Aujourd’hui",
        "agenda-show": "Afficher",
        "ui-new-task": "Nouvelle tâche"
    })

    TestCase {
        id: test

        name: "SearchTabs"
        when: windowShown

        function init() {
            mock.words = {}
            mock.off = []
            mock.asked = []
            mock.kind = "call"
            mock.category = "Papers"
            mock.tasks = mock.tasksText()
            mock.agenda = mock.agendaText()
            shell.compact = false
            shell.railShown = true
        }

        function tasksPage(width) {
            const page = test.createTemporaryObject(tasksComponent, root, { width: width || 1000, height: 860 })
            test.waitForRendering(page)
            return page
        }

        function agendaPage(width) {
            const page = test.createTemporaryObject(agendaComponent, root, { width: width || 1000, height: 860 })
            test.waitForRendering(page)
            return page
        }

        // The search of a page, once its tab shows.
        function searchOf(page) {
            const loader = root.named(page, page.modes !== undefined ? "agendaSearch" : "tasksSearch")
            tryVerify(() => loader.item !== null && loader.item.visible && loader.item.width > 0, 2000, "the search made and shown")
            test.waitForRendering(page)
            return loader.item
        }

        function rows(search) {
            return root.all(root.named(search, "thingsFound").contentItem, c => c.objectName === "thingRow").map(r => r.modelData.uid).sort()
        }

        // In Tasks ▸ Search the page's project, type and category, and the line that adds a
        // task, are not shown and take no room: the words' field comes right under the row of
        // Routines, Agendas and ⚙. Back on List, they show again as they were, untouched by
        // what was changed in Search; the search started on them.
        function test_tasks_search_has_one_set_of_filters() {
            const page = tasksPage()
            page.mode = "list"
            tryVerify(() => page.projectFilter === "home", 2000, "the page's project")
            const own = ["taskProjectFilter", "taskKindFilter", "taskCategoryFilter", "taskCaptureLine"]
            for (const name of own)
                verify(root.seen(root.named(page, name)), name + " shown in List")
            compare(root.named(page, "taskKindFilter").currentText, "Call")
            page.mode = "search"
            const search = searchOf(page)
            for (const name of own.concat(["taskFilters"]))
                verify(!root.named(page, name).visible, name + " not shown in Search")
            // No room: the words' line right under the page's row.
            const agendas = root.named(page, "agendaFilter")
            verify(root.seen(agendas), "the page's agenda filter, at its place")
            const words = root.named(search, "thingsWords")
            const rowBottom = agendas.mapToItem(page, 0, agendas.height).y
            const wordsTop = words.mapToItem(page, 0, 0).y
            const between = root.all(page, c => c.visible && c.height > 0 && c.mapToItem(page, 0, 0).y >= rowBottom - 1 && c.mapToItem(page, 0, c.height).y <= wordsTop + 1).map(c => (c.objectName || String(c).split("(")[0]) + "@" + c.mapToItem(page, 0, 0).y + "+" + c.height)
            verify(wordsTop - rowBottom <= 12, "the words' line under the page's row: " + rowBottom + " → " + wordsTop + " " + between.join(", "))
            // The search starts on the page's choices, shown as set.
            const first = mock.asked.filter(q => q.which === "tasks")[0]
            compare([first.project, first.kind, first.category], ["home", "call", "Papers"])
            compare(root.named(search, "thingsProject").currentText, "Home")
            compare(root.named(search, "thingsKind").currentText, "Call")
            compare(root.named(search, "thingsCategory").currentText, "Papers")
            // Changed in Search: the page's own untouched.
            root.named(search, "thingsProject").activated(0)
            root.named(search, "thingsKind").activated(1)
            compare(mock.asked[mock.asked.length - 1].project, "")
            page.mode = "list"
            for (const name of own)
                verify(root.seen(root.named(page, name)), name + " back in List")
            compare(page.projectFilter, "home", "the page's project untouched")
            compare([mock.kind, mock.category], ["call", "Papers"], "the page's type and category untouched")
            compare(root.named(page, "taskProjectFilter").currentText, "Home")
            // Back in Search, nothing changed upstream: the search as it was left.
            page.mode = "search"
            const again = mock.asked[mock.asked.length - 1]
            compare([again.project, again.kind], ["", ":none"], "what was changed in Search stays in Search")
        }

        // The agenda filter narrows what each page's search finds: Tasks a task by its list,
        // the Agenda an event by its calendar, the filter's button at its usual place, a full
        // view's (by hand); what it leaves out is counted under the results, and shown all
        // on asking. A turn reaches the search as Sioul says it, later.
        function test_the_agenda_filter_narrows_both_searches() {
            for (const which of ["tasks", "events"]) {
                mock.off = []
                mock.asked = []
                const page = which === "tasks" ? tasksPage() : agendaPage()
                if (which === "events")
                    page.mode = "week"
                page.mode = "search"
                const search = searchOf(page)
                compare(search.which, which)
                search.words = "form"
                search.run()
                const both = which === "tasks" ? ["form", "report"] : ["choir", "dentist"]
                tryVerify(() => JSON.stringify(rows(search)) === JSON.stringify(both), 2000, which + ": every agenda ticked: " + rows(search))
                const filter = root.all(page, c => c.objectName === "agendaFilter" && c.visible)[0]
                verify(filter !== undefined, which + ": the page's filter shown in Search")
                verify(filter.byHand, which + ": a full view's filter")
                const turned = which === "tasks" ? "dav/work" : "dav/club"
                verify(filter.groups[0].agendas.some(a => a.id === turned && a.on), which + ": listed, on")
                filter.turn([turned], false)
                tryCompare(search, "leftOut", 1, 2000, which + ": the turn reached the search")
                const narrowed = [which === "tasks" ? "form" : "dentist"]
                tryVerify(() => JSON.stringify(rows(search)) === JSON.stringify(narrowed), 2000, which + ": narrowed: " + rows(search))
                verify(filter.groups[0].agendas.some(a => a.id === turned && !a.on), which + ": its button says so")
                const list = root.named(search, "thingsFound")
                const line = root.all(list, c => c.objectName === "thingsLeftOut")[0]
                verify(line.visible, which + ": the quiet line")
                compare(root.named(line, "thingsLeftOutLine").text, "things-left-out:1")
                mouseClick(root.named(line, "thingsShowAll"))
                tryVerify(() => JSON.stringify(rows(search)) === JSON.stringify(both), 2000, which + ": shown all, for this search: " + rows(search))
                compare(root.named(line, "thingsLeftOutLine").text, "things-every-agenda")
            }
        }

        // The two Search tabs hold the same controls in the same order; each kind's own come
        // last, after the dates. The Agenda's Show, which draws nothing of a search, is not shown.
        function test_both_search_tabs_are_one_view() {
            const tasks = tasksPage()
            tasks.mode = "search"
            const agenda = agendaPage()
            agenda.mode = "search"
            const own = ["thingsGuests", "thingsStatus-confirmed", "thingsStatus-tentative", "thingsStatus-cancelled", "thingsDone", "thingsDeadline"]
            const names = (search, test) => root.all(search, c => c.objectName.indexOf("things") === 0 && c.objectName !== "thingsFound" && c.objectName.indexOf("thingsLeftOut") !== 0 && c.objectName !== "thingsShowAll" && test(c)).map(c => c.objectName)
            const tasksSearch = searchOf(tasks)
            const agendaSearch = searchOf(agenda)
            const any = c => true
            compare(names(tasksSearch, any).filter(n => own.indexOf(n) < 0), names(agendaSearch, any).filter(n => own.indexOf(n) < 0), "the same controls, in the same order")
            const shown = c => c.visible
            const common = ["thingsWords", "thingsSelect", "thingsProject", "thingsKind", "thingsCategory", "thingsDates"]
            compare(names(tasksSearch, shown), common.concat(["thingsDone", "thingsDeadline"]), "a task's own after the dates")
            compare(names(agendaSearch, shown), common.concat(["thingsGuests", "thingsStatus-confirmed", "thingsStatus-tentative", "thingsStatus-cancelled"]), "an event's own after the dates")
            // On the words' line, Select: the same place in both.
            for (const search of [tasksSearch, agendaSearch]) {
                const words = root.named(search, "thingsWords")
                const select = root.named(search, "thingsSelect")
                verify(Math.abs(words.mapToItem(search, 0, 0).y - select.mapToItem(search, 0, 0).y) < 1)
                verify(root.named(search, "thingsProject").mapToItem(search, 0, 0).y > words.mapToItem(search, 0, 0).y, "the filters under the words")
            }
            verify(!root.named(agenda, "agendaShow").visible, "Show: nothing of it in Search")
        }

        // On a phone, the search's filters fold under one button on the words' line, which
        // says how many are set: in Tasks, the page's three taken, open at once; in the Agenda,
        // closed. The page's own fold is not shown in Search. The Agenda's Search keeps one line
        // above it: its title, Agendas, New event and ⚙.
        function test_the_fold_on_a_phone() {
            shell.compact = true
            shell.railShown = false
            const page = tasksPage(412)
            page.mode = "list"
            tryVerify(() => page.projectFilter === "home", 2000)
            verify(root.named(page, "taskFilters").visible, "the page's own fold, in List")
            page.mode = "search"
            const search = searchOf(page)
            verify(!root.named(page, "taskFilters").visible, "not in Search")
            const fold = root.named(search, "thingsFilters")
            verify(fold.visible && fold.checked, "three set: open")
            compare(fold.text, "task-filters-none (3)")
            const agenda = agendaPage(412)
            agenda.mode = "search"
            const events = searchOf(agenda)
            const agendaFold = root.named(events, "thingsFilters")
            verify(agendaFold.visible && !agendaFold.checked && !root.named(events, "thingsProject").visible, "nothing set: closed")
            compare(agendaFold.text, "task-filters-none")
            verify(root.named(agenda, "agendaNewEventNarrow").visible, "New event on the title's line")
            const words = root.named(events, "thingsWords")
            const filter = root.all(agenda, c => c.objectName === "agendaFilter" && c.visible)[0]
            verify(filter !== undefined)
            const tabs = root.named(agenda, "agendaModes")
            const between = words.mapToItem(agenda, 0, 0).y - tabs.mapToItem(agenda, 0, tabs.height).y
            verify(between < filter.height + 30, "one line between the tabs and the words: " + between)
        }

        // At 412 points, in French, with every filter shown and a range of days: nothing wider
        // than the page, in either Search tab, the page's rows above it included.
        function test_nothing_wider_than_a_phone_in_french() {
            mock.words = root.french
            shell.compact = true
            shell.railShown = false
            for (const make of [() => tasksPage(412), () => agendaPage(412)]) {
                const page = make()
                page.mode = "search"
                const search = searchOf(page)
                const fold = root.named(search, "thingsFilters")
                if (!fold.checked)
                    mouseClick(fold)
                root.named(search, "thingsDates").activated(1)
                mock.off = ["dav/home", "dav/work", "dav/club"]
                search.run()
                test.waitForRendering(page)
                const shown = root.all(page, c => c.visible && c.width > 0 && (c.text !== undefined || c.objectName.indexOf("things") === 0) && c.objectName !== "thingsFound" && !root.inside(c, ["taskModes", "agendaModes"]))
                verify(shown.length > 10)
                for (const item of shown) {
                    const at = item.mapToItem(page, 0, 0)
                    verify(at.x >= -0.5 && at.x + item.width <= page.width + 0.5, (item.objectName || item.text) + ": " + at.x + " + " + item.width + " within " + page.width)
                }
            }
        }
    }
}
