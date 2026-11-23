// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Routes that make a task of each message (docs/notes-folder.md, "Routes
// that make tasks"), at a phone's width, 412 points, on a stand-in for
// Sioul. The routes' editor (SettingRow.qml's "routes" kind): "Make a task
// of each message" on a route asks for its form, a route that makes tasks
// says its model in one line with Change…, and says when its number of
// business days is missing; off, the route keeps its words and makes
// nothing. The form (RouteTask.qml): filled from a message (its sender, its
// subject's words between brackets, its project), the date asked filled
// with 3, the title a message gets; saved, what it sends; "Also for the 2
// messages already here" only when asked; a new project named in place;
// nothing past the screen's edge. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 891

    Theme {
        id: testTheme
    }

    // Made up: an invented shop, "[New sale]".
    QtObject {
        id: mock

        property var calls: []
        // What a save answers.
        property int already: 2
        property string saveError: ""

        function routeModels(project) {
            return JSON.stringify([{ id: "send", name: "Send a gift card", summary: "Asked three business days after the mail · About 15 minutes · Uses: 1 Stamps · in Work", missing: false }, { id: "old", name: "By hand", summary: "", missing: true }])
        }
        function routeTaskForm(project, route, key) {
            mock.calls.push({ what: "routeTaskForm", project: project, route: route, key: key })
            const lists = [{ id: "", title: "routetasks-list-usual" }, { id: "local/work", title: "Work" }]
            const supplies = [{ id: "stamps", name: "Stamps", unit: "" }, { id: "cards", name: "Gift cards", unit: "" }]
            const send = { id: "send", name: "Send a gift card", list: "local/work", prefix: "Send:", business_days: 3, estimate: 15, supplies: [{ id: "stamps", quantity: 1 }] }
            if (key !== "")
                return JSON.stringify({ projects: [{ id: "gift-cards", title: "Gift cards" }, { id: "taxes", title: "Taxes" }], project: "gift-cards", route: { from_addresses: ["shop@example.org"], subject_contains: ["New sale"] }, route_line: "", model: { id: "", name: "", list: "", prefix: "", business_days: 3, estimate: 0, supplies: [] }, models: [send], lists: lists, keeps: true, supplies: supplies, subject: "[New sale] Gift card 50 €", title: "Gift card 50 €", error: "" })
            return JSON.stringify({ projects: [{ id: "gift-cards", title: "Gift cards" }], project: project, route: JSON.parse(route), route_line: "set-route-addresses: shop@example.org · set-route-subject: New sale", model: send, models: [send], lists: lists, keeps: true, supplies: supplies, subject: "", title: "", error: "" })
        }
        function routeTaskTitle(subject, words, prefix) {
            return (prefix !== "" ? prefix + " " : "") + subject.replace("[" + JSON.parse(words).join("") + "] ", "")
        }
        function saveRouteTask(form) {
            mock.calls.push({ what: "saveRouteTask", form: JSON.parse(form) })
            return JSON.stringify({ project: "gift-cards", route: "r1", model: "send", already: mock.already, error: mock.saveError })
        }
        function routeTaskAlso(project, route) {
            mock.calls.push({ what: "routeTaskAlso", project: project, route: route })
            return ""
        }
        function text(key) {
            return key === "qt-locale" ? "en_GB" : key
        }
        function textCounted(key, n) {
            return key + " " + n
        }
        function textWith(key, name, value) {
            return key + ": " + value
        }
    }

    // A project's routes, as the projects' page shows them.
    property var routes: [{ id: "r1", from_domains: [], from_addresses: ["shop@example.org"], subject_contains: ["New sale"], text_contains: [], attachment_contains: [], makes: "send" }, { id: "r2", from_domains: ["market.example.org"], from_addresses: [], subject_contains: [], text_contains: [], attachment_contains: [], makes: "" }, { id: "r3", from_domains: [], from_addresses: [], subject_contains: ["Order"], text_contains: [], attachment_contains: [], makes: "old" }]
    property var asked: []
    property var saved: []

    Flickable {
        anchors.fill: parent
        contentHeight: row.implicitHeight

        SettingRow {
            id: row

            width: root.width
            setting: ({ key: "project.gift-cards.routes", kind: "routes", label: "set-routes", help: "set-routes-help", group: "", section: "", choices: [], min: 0, max: 0, step: 1, unit: "", value: root.routes })
            sioul: mock
            theme: testTheme
            onRouteTask: route => root.asked.push(route)
            onSave: (key, value, shown) => root.saved.push({ key: key, value: value })
        }
    }

    RouteTask {
        id: form

        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "RouteTasks"
        when: windowShown

        // An item by its objectName under `item`, its views' content too.
        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = (item.children || []).concat(item.contentItem && item.contentItem !== item ? [item.contentItem] : [])
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found !== null)
                    return found
            }
            return null
        }

        // Every item shown, past the right edge of the screen: its name and its right edge.
        function past(item, found) {
            if (!item || !item.visible)
                return found
            const right = item.mapToItem(root, item.width, 0).x
            if (item.width > 0 && right > root.width + 0.5)
                found.push((item.objectName || item.text || item.toString()) + " ends at " + Math.round(right))
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                past(kids[i], found)
            if (item.contentItem && item.contentItem !== item)
                past(item.contentItem, found)
            return found
        }

        function init() {
            mock.calls = []
            mock.already = 2
            mock.saveError = ""
            root.asked = []
            root.saved = []
            form.close()
            waitForRendering(row)
        }

        // A route that makes tasks: its box ticked, its model in one line, Change… beside.
        function test_a_route_that_makes_tasks_says_its_model() {
            tryVerify(() => find(row, "routeMakes:0") !== null)
            verify(find(row, "routeMakes:0").checked)
            verify(!find(row, "routeMakes:1").checked)
            const line = find(row, "routeMakesLine:0")
            verify(line.visible && line.text.indexOf("Send a gift card · Asked three business days after the mail") === 0, line.text)
            verify(!find(row, "routeMakesMissing:0").visible)
            // Written by hand without its number: said.
            verify(find(row, "routeMakesMissing:2").visible)
            compare(find(row, "routeMakesMissing:2").text, "routetasks-days-missing-line")
            mouseClick(find(row, "routeMakesChange:0"))
            compare(root.asked.length, 1)
            compare(root.asked[0].id, "r1")
        }

        // Ticked on a route that makes none: its form asked for, nothing saved until it is;
        // unticked on one that does: its words kept, `makes` taken off.
        function test_the_box_asks_for_the_form_or_takes_makes_off() {
            tryVerify(() => find(row, "routeMakes:1") !== null)
            const box = find(row, "routeMakes:1")
            mouseClick(box)
            compare(root.asked.length, 1)
            compare(root.asked[0].id, "r2")
            verify(!box.checked, "ticked only once its form is saved")
            compare(root.saved.length, 0)
            mouseClick(find(row, "routeMakes:0"))
            compare(root.saved.length, 1)
            compare(root.saved[0].key, "project.gift-cards.routes")
            compare(root.saved[0].value[0].makes, "")
            compare(root.saved[0].value[0].from_addresses.join(","), "shop@example.org")
            compare(root.saved[0].value[1].id, "r2", "the others as they were")
        }

        // From a message: its sender, its subject's words between brackets, its project; the
        // date asked filled with 3; the title a message like it gets; saved, what it sends, then
        // "Also for the 2 messages already here", asked once.
        function test_a_form_filled_from_a_message() {
            form.forMail("/mail/cur/1")
            tryVerify(() => form.opened)
            const body = form.contentItem
            compare(find(body, "routeTaskFrom").text, "shop@example.org")
            compare(find(body, "routeTaskSubject").text, "New sale")
            compare(find(body, "routeTaskProject").currentText, "Gift cards")
            compare(find(body, "routeTaskDays").value, 3, "the date asked, filled with 3")
            find(body, "routeTaskName").text = "Send a gift card"
            find(body, "routeTaskPrefix").text = "Send:"
            compare(form.titleShown, "Send: Gift card 50 €")
            find(body, "routeTaskDays").value = 2
            find(body, "routeTaskEstimate").value = 15
            form.save()
            const sent = mock.calls.filter(c => c.what === "saveRouteTask")
            compare(sent.length, 1)
            const f = sent[0].form
            compare([f.project, f.new_project, f.route.from_addresses.join(","), f.route.from_domains.length, f.route.subject_contains.join(","), f.model.id, f.model.name, f.model.prefix, f.model.business_days, f.model.estimate].join("|"), "gift-cards||shop@example.org|0|New sale||Send a gift card|Send:|2|15")
            verify(form.opened, "mail already here: asked, never by itself")
            compare(mock.calls.filter(c => c.what === "routeTaskAlso").length, 0)
            const also = find(form.contentItem, "routeTaskAlso")
            verify(also.visible)
            compare(also.text, "routetasks-also 2")
            mouseClick(also)
            compare(mock.calls.filter(c => c.what === "routeTaskAlso").length, 1)
            verify(!form.opened)
        }

        // A domain typed goes to the domains; a new project named in place; nothing already here: closed at once.
        function test_a_new_project_and_a_domain() {
            mock.already = 0
            form.forMail("/mail/cur/1")
            tryVerify(() => form.opened)
            const body = form.contentItem
            find(body, "routeTaskFrom").text = "example.org, shop@example.org"
            const project = find(body, "routeTaskProject")
            project.currentIndex = project.count - 1
            verify(find(body, "routeTaskNewProject").visible)
            verify(!form.canSave, "a new project needs its name")
            find(body, "routeTaskNewProject").text = "Gift cards 2"
            verify(form.canSave)
            form.save()
            const f = mock.calls.filter(c => c.what === "saveRouteTask")[0].form
            compare([f.project, f.new_project, f.route.from_domains.join(","), f.route.from_addresses.join(",")].join("|"), "|Gift cards 2|example.org|shop@example.org")
            verify(!form.opened, "nothing already here: closed")
        }

        // From a project's route: its words in one line, its model's fields; a refusal said in place.
        function test_a_form_for_a_project_s_route() {
            mock.saveError = "A route takes some mail."
            form.forRoute("gift-cards", root.routes[0])
            tryVerify(() => form.opened)
            const body = form.contentItem
            verify(!find(body, "routeTaskFrom").visible && !find(body, "routeTaskProject").visible)
            compare(find(body, "routeTaskName").text, "Send a gift card")
            compare(find(body, "routeTaskList").currentText, "Work")
            compare(find(body, "routeTaskEstimate").value, 15)
            form.save()
            const f = mock.calls.filter(c => c.what === "saveRouteTask")[0].form
            compare([f.project, f.route.id, f.model.id, f.model.supplies.length].join("|"), "gift-cards|r1|send|1")
            verify(form.opened)
            compare(form.problem, "A route takes some mail.")
        }

        function test_the_form_fits_a_phone_s_width() {
            form.forMail("/mail/cur/1")
            tryVerify(() => form.opened)
            waitForRendering(form.contentItem)
            compare(past(form.contentItem, []).join("; "), "", "nothing past 412 points")
            compare(past(row, []).join("; "), "", "the routes either")
        }
    }
}
