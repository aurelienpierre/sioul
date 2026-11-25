// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Rules (docs/rules.md, "The view"), at a phone's width, 412
// points, on a stand-in for Sioul. The list (RulesTab.qml): rules grouped
// under what happens, those of the same "when" together under it; a rule of
// yours switched off and removed (what it asks of Sioul); what it did lately
// folded, made when first opened, each act with Undo and Not for this one;
// a rule kept elsewhere saying where, its link opening it there, without a
// switch or Edit. The presets' tab: a line each, a switch only where a
// setting turns it on or off, Details only where there is more. The editor
// (RuleEditor.qml) built from declarations alone: a type and a field added
// here, never written into the editor, show up with their value's control;
// what is not available yet is greyed with "later" and never chosen; a
// parameter fixed or taken from the message; the preview's count and "Also
// for these messages" unticked; Save sends the rule as edited. "Add a
// rule…"'s three starts (RuleStart.qml). Nothing past the screen's edge. Run
// by tools/qml-test.sh.

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

    // Made up: an invented tax office and shop.
    QtObject {
        id: mock

        signal rulesChanged
        signal rulesPreviewed(string found)
        signal rulesStartsFound(string found)

        property var calls: []
        property string status: ""
        // The declarations, as the core's catalogue gives them; a test adds to them.
        property var catalogue: ({
            format: 1, phase: 1, levels: [],
            types: [
                { id: "message", name: "A message", available: true, phase: 1,
                  events: [{ id: "arrives", name: "arrives", available: true, phase: 1 }, { id: "is-sent", name: "is sent", available: false, phase: 2 }],
                  fields: [
                      { id: "sender-domain", name: "its sender's domain", tests: [{ id: "is", name: "is", value: "domains" }], open: false, proof: true, available: true, phase: 1 },
                      { id: "subject", name: "its subject", tests: [{ id: "has-words", name: "has the words", value: "words" }, { id: "contains", name: "contains", value: "text" }], open: false, proof: false, available: true, phase: 1 },
                      { id: "kind", name: "what Sioul recognises in it", tests: [{ id: "is", name: "is", value: "choice" }], choices: [{ id: "bill", name: "a bill", available: true, phase: 1 }, { id: "payment-refused", name: "a payment refused", available: false, phase: 2 }], open: true, proof: false, available: true, phase: 1 },
                      { id: "attachment", name: "its attachments", tests: [{ id: "exists", name: "there is one", value: "none" }], open: false, proof: false, available: true, phase: 1 },
                      { id: "amount", name: "the amount it states", tests: [{ id: "above", name: "above", value: "number" }], open: false, proof: false, available: false, phase: 3 }
                  ],
                  actions: [
                      { id: "tie-to-project", name: "tie it to a project", available: true, phase: 1, params: [{ id: "project", name: "the project", value: "project", required: true, fixed: true, from: [] }] },
                      { id: "trash", name: "move it to the trash", available: false, phase: 4, params: [] }
                  ] },
                { id: "none", name: "Sioul", available: true, phase: 1, events: [], fields: [], actions: [{ id: "tell", name: "tell you", available: true, phase: 1, params: [] }] },
                { id: "task", name: "A task", available: true, phase: 2, events: [{ id: "is-done", name: "is done", available: false, phase: 2 }], fields: [],
                  actions: [{ id: "make", name: "make a task from a model", available: true, phase: 1, params: [
                      { id: "model", name: "the model", value: "model", required: true, fixed: true, from: [] },
                      { id: "title", name: "its title", value: "text", required: false, fixed: true, from: [{ id: "subject", name: "its subject", available: true, phase: 1 }] },
                      { id: "days", name: "business days after it came", value: "number", required: false, fixed: true, from: [] }] }] }
            ]
        })
        property var choices: ({ project: [{ id: "taxes", name: "Taxes 2025", available: true }], model: [{ id: "pay", name: "Pay a bill", available: true }], topic: [], contact: [] })
        // The list, as `rulesView` gives it.
        property var view: ({
            readOnly: false, note: "", count: 3,
            groups: [{ heading: "When a message arrives", sets: [
                { when: "When a message arrives from anyone at impots.example:", rules: [
                    { id: "r1", own: true, on: true, sentence: "When a message arrives from anyone at impots.example: tie it to the project Taxes 2025.", then: "tie it to the project Taxes 2025", problem: "", unknown: false, off: "",
                      rule: { id: "r1", on: true, when: { type: "message", event: "arrives", if: [{ field: "sender-domain", test: "is", value: ["impots.example"] }] }, then: { type: "message", action: "tie-to-project", with: { project: "taxes" } } },
                      lately: [
                          { line: "Tue 24 Nov 09:20, “Rent”: The moment does not let a notice come now.", act: { at: 3, rule: "r1", object: "mid:c@impots.example" }, object: "mid:c@impots.example", undo: false, notThis: true },
                          { line: "Tue 24 Nov 09:12: “Notice” tied to the project Taxes 2025.", act: { at: 1, rule: "r1", object: "mid:a@impots.example", act: "tie" }, object: "mid:a@impots.example", undo: true, notThis: false },
                          { line: "Mon 23 Nov 08:00: “Old” tied to the project Taxes 2025. Taken back since.", act: { at: 0, rule: "r1", object: "mid:b@impots.example", act: "tie" }, object: "mid:b@impots.example", undo: false, notThis: false }] },
                    { id: "r2", own: true, on: true, sentence: "When a message arrives from anyone at impots.example: make a task from the model “Pay a bill”.", then: "make a task from the model “Pay a bill”", problem: "", unknown: false, off: "", rule: { id: "r2" }, lately: [] }] },
                { when: "When a message arrives from anyone at new.example:", rules: [
                    { id: "r3", own: true, on: true, sentence: "When a newer Sioul's rule arrives.", then: "", problem: "Written by a newer Sioul.", unknown: true, off: "", rule: { id: "r3" }, lately: [] }] }],
                places: [{ id: "project:shop", title: "The project Shop: one rule", kept: "Kept with the project Shop, among its routes; changed on its page.", open: { kind: "project", uri: "sioul:project/shop", key: "shop" }, openText: "Open the project",
                           rules: [{ id: "read:route:1", own: false, on: true, sentence: "When a message arrives from anyone at shop.example: tie it to the project Shop.", off: "" }] }] }],
            presets: [{ heading: "Mail", presets: [
                { id: "codes-told", sentence: "When a message holds a code, it is shown at once and told.", off: false, open: null, openText: "", offered: false },
                { id: "new-mail-told", sentence: "When new mail arrives, Sioul tells you of it.", off: true, open: { kind: "settings:reminders.mail" }, openText: "Change it where it is set", offered: false }] },
                { heading: "Tasks", presets: [{ id: "first-estimate", sentence: "When a task is made, Sioul writes its first estimate.", off: false, open: null, openText: "", offered: true }] }]
        })
        function rulesView() {
            return JSON.stringify(mock.view)
        }
        function rulesCatalogue() {
            return JSON.stringify(mock.catalogue)
        }
        function rulesChoices() {
            return JSON.stringify(mock.choices)
        }
        function ruleSentence(rule) {
            const r = JSON.parse(rule)
            return JSON.stringify({ sentence: "said: " + r.when.if.map(c => c.field).join(",") + " → " + r.then.action, problem: r.when.if.length === 0 ? "Add a condition." : r.then.action === "" ? "Choose what it does." : "" })
        }
        function rulePreview(rule) {
            mock.calls.push({ what: "rulePreview", rule: JSON.parse(rule) })
            mock.rulesPreviewed(JSON.stringify({ asked: rule, count: 2, past: true, text: "2 messages in the last 30 days:", lines: ["12 Nov, Tax office: “Notice” — tie it", "20 Nov, Tax office: “Reminder” — tie it"] }))
        }
        function saveRule(rule, also) {
            mock.calls.push({ what: "saveRule", rule: JSON.parse(rule), also: also })
            return JSON.stringify({ id: "r9", error: "" })
        }
        function saveRuleModel(model) {
            mock.calls.push({ what: "saveRuleModel", model: JSON.parse(model) })
            mock.choices = Object.assign({}, mock.choices, { model: mock.choices.model.concat([{ id: "check", name: JSON.parse(model).name, available: true }]) })
            return JSON.stringify({ id: "check", error: "" })
        }
        function setRuleOn(id, on) {
            mock.calls.push({ what: "setRuleOn", id: id, on: on })
        }
        function removeRule(id) {
            mock.calls.push({ what: "removeRule", id: id })
        }
        function undoRuleAct(act) {
            mock.calls.push({ what: "undoRuleAct", act: JSON.parse(act) })
        }
        function ruleNotForThisOne(rule, object) {
            mock.calls.push({ what: "ruleNotForThisOne", rule: rule, object: object })
        }
        function ruleFromMessage(key) {
            return JSON.stringify({ id: "", on: true, when: { type: "message", event: "arrives", if: [{ field: "sender-domain", test: "is", value: ["shop.example"] }, { field: "kind", test: "is", value: ["bill"] }] }, then: { type: "", action: "", with: {} } })
        }
        function ruleFromPreset(id) {
            return JSON.stringify({ id: "", on: true, when: { type: "message", event: "arrives", if: [{ field: "kind", test: "is", value: ["bill"] }] }, then: { type: "none", action: "tell", with: {} } })
        }
        function rulesStartsAsk() {
            mock.calls.push({ what: "rulesStartsAsk" })
            mock.rulesStartsFound(JSON.stringify({ messages: [{ key: "/mail/cur/1", subject: "Your bill", from: "Shop", day: "20 November" }, { key: "/mail/cur/2", subject: "Notice", from: "Tax office", day: "12 November" }], presets: [] }))
        }
        function reminderOpened(kind, uri, key) {
            mock.calls.push({ what: "reminderOpened", kind: kind, uri: uri, key: key })
        }
        function setSetting(key, value) {
            mock.calls.push({ what: "setSetting", key: key, value: JSON.parse(value) })
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

    Flickable {
        id: flick

        anchors.fill: parent
        contentHeight: tab.implicitHeight

        RulesTab {
            id: tab

            width: root.width
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "Rules"
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

        function calls(what) {
            return mock.calls.filter(c => c.what === what)
        }

        function editor() {
            return tab.editor
        }

        function init() {
            mock.calls = []
            tab.part = "own"
            tab.unfolded = ({})
            tab.removing = ""
            tab.reload()
            // Every card of the tab closed (the editor, the starts, a question).
            for (const kid of tab.children) {
                if (kid.item && typeof kid.item.close === "function")
                    kid.item.close()
            }
            waitForRendering(tab)
        }

        // Grouped by what happens; the two rules of one "when" together under it, each by
        // what it does; a rule kept elsewhere folded with its place.
        function test_rules_are_grouped_by_what_happens() {
            const group = find(tab, "rulesGroup:0")
            verify(group !== null)
            compare(group.title, "When a message arrives")
            compare(find(find(tab, "rule:r1"), "ruleWords").text, "•  tie it to the project Taxes 2025")
            const place = find(tab, "rulesPlace:project:shop")
            verify(place !== null)
            compare(place.title, "The project Shop: one rule")
            verify(!place.made, "folded: its sentences made when first opened")
        }

        // Switched off and removed: what the list asks of Sioul; a newer Sioul's rule asked first.
        function test_a_rule_switched_off_and_removed() {
            const line = find(tab, "rule:r1")
            const toggle = find(line, "ruleSwitch")
            verify(toggle.visible && toggle.checked)
            mouseClick(toggle)
            compare(calls("setRuleOn").length, 1)
            compare([calls("setRuleOn")[0].id, calls("setRuleOn")[0].on].join("|"), "r1|false")
            // On a phone, Edit and Remove under the sentence.
            verify(!find(line, "ruleRemove").visible)
            mouseClick(find(line, "ruleRemoveNarrow"))
            compare(calls("removeRule").length, 1)
            compare(calls("removeRule")[0].id, "r1")
            // A rule a newer Sioul wrote cannot be put back: asked, then removed.
            const newer = find(tab, "rule:r3")
            verify(!find(newer, "ruleEditNarrow").enabled, "never edited here")
            mouseClick(find(newer, "ruleRemoveNarrow"))
            compare(calls("removeRule").length, 1, "not yet: asked first")
            compare(tab.removing, "r3")
        }

        // What it did lately: folded, made when first opened, kept open when the list is read
        // again; Undo only where the rule wrote it, Not for this one only where it waits.
        function test_what_it_did_lately() {
            let fold = find(find(tab, "rule:r1"), "ruleLately")
            verify(fold.visible)
            compare(fold.title, "rules-lately 3")
            verify(!fold.made, "made when first unfolded")
            verify(!find(find(tab, "rule:r2"), "ruleLately").visible, "nothing done: no fold")
            fold.want(true)
            tryVerify(() => fold.made)
            const undos = []
            const notThese = []
            const walk = item => {
                if (!item)
                    return
                if (item.objectName === "ruleActUndo")
                    undos.push(item)
                if (item.objectName === "ruleActNotThis")
                    notThese.push(item)
                for (const kid of item.children || [])
                    walk(kid)
            }
            walk(fold)
            compare(undos.map(b => b.visible).join(","), "false,true,false", "Undo on the done line alone")
            compare(notThese.map(b => b.visible).join(","), "true,false,false", "Not for this one on the waiting line alone")
            undos[1].clicked()
            compare(calls("undoRuleAct")[0].act.object, "mid:a@impots.example")
            notThese[0].clicked()
            compare([calls("ruleNotForThisOne")[0].rule, calls("ruleNotForThisOne")[0].object].join("|"), "r1|mid:c@impots.example")
            // Read again (another device wrote): the fold stays open.
            tab.reload()
            waitForRendering(tab)
            fold = find(find(tab, "rule:r1"), "ruleLately")
            verify(fold.open, "kept open")
        }

        // Kept elsewhere: where, and its link, once for the place; no switch, no Edit, no ✕.
        function test_a_rule_kept_elsewhere_opens_where_it_is_kept() {
            const place = find(tab, "rulesPlace:project:shop")
            place.want(true)
            tryVerify(() => place.made)
            const line = find(place, "rule:read:route:1")
            verify(line !== null)
            verify(find(line, "ruleSwitch") === null && find(line, "ruleEdit") === null)
            const link = find(place, "ruleOpen")
            verify(link.visible)
            compare(link.text, "Open the project")
            link.activated()
            compare([calls("reminderOpened")[0].kind, calls("reminderOpened")[0].key].join("|"), "project|shop")
        }

        // Presets: grouped by what they watch, each its written sentence; off said in words; a
        // link to where its setting lives, never a switch of its own; a copy where Sioul runs it.
        function test_presets_are_written_lines_with_a_link_where_set() {
            verify(find(tab, "rulesTabs").visible)
            tab.part = "presets"
            waitForRendering(tab)
            compare(find(tab, "presetsGroup:0").title, "Mail")
            const codes = find(tab, "preset:codes-told")
            const mail = find(tab, "preset:new-mail-told")
            verify(!find(codes, "presetWhere").visible && !find(codes, "presetOff").visible, "nothing set elsewhere: one line of words")
            verify(find(mail, "presetOff").visible, "off where it is set, said")
            const where = find(mail, "presetWhere")
            verify(where.visible)
            where.activated()
            compare(calls("reminderOpened")[0].kind, "settings:reminders.mail")
            let switches = 0
            const walk = item => {
                if (!item)
                    return
                if (item instanceof Switch && item.visible)
                    switches++
                for (const kid of item.children || [])
                    walk(kid)
            }
            walk(tab)
            compare(switches, 0, "no switch drawn twice: it lives where it is set")
            verify(!find(codes, "presetCopy").visible)
            verify(find(find(tab, "preset:first-estimate"), "presetCopy").visible, "copied where Sioul runs it")
            compare(past(tab, []).join("; "), "", "nothing past 412 points")
        }

        // The editor follows the declarations: a type and a field added here appear, their value's
        // control by its kind; what comes later is greyed and never chosen.
        function test_the_editor_follows_the_declarations() {
            const added = JSON.parse(JSON.stringify(mock.catalogue))
            added.types[0].fields.push({ id: "colour", name: "its colour", tests: [{ id: "is", name: "is", value: "choice" }], choices: [{ id: "blue", name: "blue", available: true, phase: 1 }], open: false, proof: false, available: true, phase: 1 })
            mock.catalogue = added
            tab.edit(null)
            const form = editor()
            tryVerify(() => form.opened)
            compare(form.draft.when.type, "message", "what is available first")
            const types = find(form.contentItem, "ruleType")
            compare(types.model.map(m => m.text).join(","), "A message,rules-later: A task", "Sioul, without events, is never a When; a task, with none available yet, later")
            compare(form.draft.when.event, "arrives")
            const events = find(form.contentItem, "ruleEvent")
            compare(events.count, 2)
            compare(events.model[1].text, "rules-later: is sent")
            events.activated(1)
            compare(form.draft.when.event, "arrives", "a greyed event is never chosen")
            mouseClick(find(form.contentItem, "ruleAddCondition"))
            compare(form.draft.when.if.length, 1)
            const field = find(form.contentItem, "ruleField")
            compare(field.model.map(m => m.text).join(","), "its sender's domain,its subject,what Sioul recognises in it,its attachments,rules-later: the amount it states,its colour")
            field.activated(5)
            compare(form.draft.when.if[0].field, "colour")
            const value = find(form.contentItem, "ruleValueChoice")
            tryVerify(() => value.visible)
            compare(value.model[0].text, "blue")
            value.activated(0)
            compare(form.draft.when.if[0].value.join(","), "blue")
            // A test that takes no value: no control (the condition's line made anew at each change).
            find(form.contentItem, "ruleField").activated(3)
            compare(form.draft.when.if[0].test, "exists")
            verify(!find(form.contentItem, "ruleValue").visible)
            mock.catalogue = JSON.parse(JSON.stringify(added))
            mock.catalogue.types[0].fields.pop()
        }

        // Then: an action across types, a parameter fixed or taken from the message; the
        // preview's count; Save sends the rule as edited, the past unticked.
        function test_an_action_its_parameters_the_preview_and_save() {
            tab.edit(JSON.parse(mock.ruleFromMessage("/mail/cur/1")))
            const form = editor()
            tryVerify(() => form.opened)
            compare(form.draft.when.if.length, 2, "filled from the message")
            const action = find(form.contentItem, "ruleAction")
            compare(action.displayText, "rules-choose-action")
            const later = action.model.findIndex(m => m.text === "rules-later: move it to the trash")
            verify(later >= 0, "deleting is shown as later, never offered")
            action.activated(later)
            compare(form.draft.then.action, "")
            action.activated(action.model.findIndex(m => m.text === "make a task from a model"))
            compare([form.draft.then.type, form.draft.then.action].join("/"), "task/make")
            const title = find(form.contentItem, "ruleParam:title")
            tryVerify(() => title !== null && title.visible)
            mouseClick(find(title, "ruleParamFrom"))
            compare(JSON.stringify(form.draft.then.with.title), JSON.stringify({ from: "subject" }))
            form.setParam("model", "pay")
            compare(form.problem, "")
            tryVerify(() => calls("rulePreview").length > 0, 2000)
            tryVerify(() => form.preview !== null)
            compare(form.preview.count, 2)
            const also = find(form.contentItem, "ruleAlsoPast")
            verify(also.visible && !also.checked, "the past, unticked")
            compare(also.text, "rules-also-past 2")
            form.save()
            const sent = calls("saveRule")
            compare(sent.length, 1)
            compare(sent[0].also, false)
            compare([sent[0].rule.then.with.model, sent[0].rule.when.if[1].value[0]].join("|"), "pay|bill")
            verify(!form.opened)
            compare(past(form.contentItem, []).join("; "), "", "nothing past 412 points")
        }

        // No model fits: one named in place, kept with the rules, chosen at once.
        function test_a_new_model_named_in_place() {
            tab.edit(JSON.parse(mock.ruleFromMessage("/mail/cur/1")))
            const form = editor()
            tryVerify(() => form.opened)
            form.setAction("task", "make")
            const fold = find(form.contentItem, "ruleNewModel")
            tryVerify(() => fold !== null && fold.visible)
            fold.want(true)
            tryVerify(() => find(fold, "ruleModelName") !== null)
            const keep = find(fold, "ruleModelKeep")
            verify(!keep.enabled, "a model needs its name")
            find(fold, "ruleModelName").text = "Check the statement"
            verify(keep.enabled)
            // Below the card's fold of the window: pressed as a screen reader would.
            keep.clicked()
            compare(calls("saveRuleModel").length, 1)
            compare([calls("saveRuleModel")[0].model.name, calls("saveRuleModel")[0].model.business_days].join("|"), "Check the statement|3")
            compare(form.draft.then.with.model, "check")
            form.close()
        }

        // The field being typed in is brought into view (a phone's keyboard hides half the form).
        function test_the_field_typed_in_is_kept_in_view() {
            tab.edit(JSON.parse(mock.ruleFromMessage("/mail/cur/1")))
            const form = editor()
            tryVerify(() => form.opened)
            form.setAction("task", "make")
            const flick = form.contentItem.contentItem
            tryVerify(() => flick.contentHeight > flick.height, 2000, "the form is longer than its card")
            flick.contentY = 0
            const days = find(find(form.contentItem, "ruleParam:days"), "ruleValueNumber")
            verify(days !== null)
            days.forceActiveFocus()
            tryVerify(() => {
                const top = days.mapToItem(flick.contentItem, 0, 0).y
                return top >= flick.contentY && top + days.height <= flick.contentY + flick.height
            }, 2000, "the focused box is within the card's view")
            form.close()
        }

        // "Add a rule…": three starts; a message found by its words opens the editor on its draft.
        function test_three_starts() {
            mouseClick(find(tab, "rulesAdd"))
            tryVerify(() => tab.children.some(c => c.item && c.item.objectName === "ruleStart" && c.item.opened))
            const start = tab.children.find(c => c.item && c.item.objectName === "ruleStart").item
            compare(calls("rulesStartsAsk").length, 1, "asked off the window's thread")
            verify(!start.looking, "found")
            verify(find(start.contentItem, "ruleStartBlank").visible)
            verify(!find(start.contentItem, "ruleStartPresets").visible, "no preset to start from yet")
            verify(find(start.contentItem, "ruleStartPresetsLater").visible, "said once, quietly")
            find(start.contentItem, "ruleStartFind").text = "tax"
            start.find = "tax"
            compare(start.found.length, 1)
            compare(start.found[0].subject, "Notice")
            start.choose("message", start.found[0].key)
            tryVerify(() => editor() !== null && editor().opened)
            compare(editor().draft.when.if[1].value[0], "bill")
            editor().close()
        }
    }
}
