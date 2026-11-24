// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A rule, new or changed (docs/rules.md, "The view"): one card with two
// headed parts, When and Then, built entirely from what the core declares
// (`sioul_core::wiring::declare::catalogue`, asked once as
// `rulesCatalogue`): never a control written by hand for a pair of types.
// When: a type and what happens to it, then conditions, each a field, a test
// and a value whose control follows the value's kind (words, addresses,
// domains, a choice, a project, a topic, days, a time, a date, a size…).
// Then: one action of one type, its parameters each fixed or taken from what
// triggered the rule ("its subject", "its project"). What the core declares
// as not yet available is shown greyed with "later", and never chosen. Above
// the parts, the rule in words, made by the one function that words every
// rule (`ruleSentence`: the list and the status line say the same); under
// them, what the rule would have done in the last 30 days (`rulePreview`,
// off the window's thread), with "Also for these messages" unticked. Save
// writes it through the core's store; the dialog stays open, saying why,
// when the store refuses it. Made the first time it opens (RulesTab.qml,
// and main.qml for "Make a rule from this…").

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: form

    objectName: "ruleEditor"

    required property var sioul
    required property var theme

    // What the core declares ({format, types}), and what the values choose among
    // ({project, topic, account, list, model, level, contact}: [{id, name}]).
    property var catalogue: ({ types: [] })
    property var choices: ({})
    // The rule as it is being changed (the core's JSON); null when closed.
    property var draft: null
    // The rule in words, and what keeps it from being saved ("" none).
    property string sentence: ""
    property string problem: ""
    // What the store said when saving failed.
    property string refused: ""
    // The preview: {count, text, lines: [{subject, from, day, does}]}; looking meanwhile.
    property var preview: null
    property bool looking: false
    // The rule as the preview was asked for it: an answer for an older draft is left aside.
    property string previewAsked: ""
    // "Also for these messages" ticked.
    property bool alsoPast: false
    // A phone's width: each label above its control.
    readonly property bool narrow: form.width < 520

    // Saved: the list reads its rules again (the store's change says it too).
    signal saved(string id)

    // A rule to change (its JSON as the list gives it), or a draft to fill (a
    // message's example, `rulesFromMessage`); none for a new one.
    function edit(rule) {
        form.catalogue = JSON.parse(form.sioul.rulesCatalogue() || "null") || { types: [] }
        form.choices = JSON.parse(form.sioul.rulesChoices() || "null") || {}
        form.draft = rule ? form.copy(rule) : form.blank()
        if (!form.draft.when)
            form.draft.when = form.blank().when
        if (!form.draft.when.if)
            form.draft.when.if = []
        if (!form.draft.then)
            form.draft.then = { type: "", action: "", with: {} }
        if (!form.draft.then.with)
            form.draft.then.with = {}
        form.refused = ""
        form.alsoPast = false
        form.changed()
        form.open()
    }

    // A new rule: what the core declares first, available now; nothing done yet.
    function blank() {
        const type = form.catalogue.types.find(t => t.available && (t.events || []).some(e => e.available)) || null
        const event = type ? type.events.find(e => e.available) : null
        return { id: "", on: true, when: { type: type ? type.id : "", event: event ? event.id : "", match: "all", if: [] }, then: { type: "", action: "", with: {} } }
    }

    function copy(value) {
        return JSON.parse(JSON.stringify(value))
    }

    // ------------------------------------------------------------ the declarations

    function typeOf(id) {
        return form.catalogue.types.find(t => t.id === id) || null
    }
    function whenType() {
        return form.draft ? form.typeOf(form.draft.when.type) : null
    }
    function fieldOf(id) {
        const type = form.whenType()
        return type ? ((type.fields || []).find(f => f.id === id) || null) : null
    }
    function testOf(field, id) {
        return field ? ((field.tests || []).find(t => t.id === id) || null) : null
    }
    // What a condition's value is: its test's own kind ("has the words" reads words), else its field's.
    function valueKind(field, test) {
        return test && test.value ? test.value : field ? field.value : "none"
    }
    // Every action of every type, as one list: {type, action, name, available, phase}.
    readonly property var actions: {
        const out = []
        for (const type of form.catalogue.types || []) {
            for (const action of type.actions || [])
                out.push({ type: type.id, action: action.id, name: action.name, available: action.available === true && type.available !== false, phase: action.phase || type.phase || 0 })
        }
        return out
    }
    function actionOf() {
        if (!form.draft)
            return null
        const type = form.typeOf(form.draft.then.type)
        return type ? ((type.actions || []).find(a => a.id === form.draft.then.action) || null) : null
    }

    // ------------------------------------------------------------ changes

    // Something changed: the rule said again, its preview looked for again a moment later.
    function changed() {
        const said = JSON.parse(form.sioul.ruleSentence(JSON.stringify(form.draft)) || "{}")
        form.sentence = said.sentence || ""
        form.problem = said.problem || ""
        form.refused = ""
        form.preview = null
        form.alsoPast = false
        if (form.problem === "")
            previewTimer.restart()
        else
            previewTimer.stop()
        // The parts drawn again from the draft.
        form.draft = form.copy(form.draft)
    }

    function setType(id) {
        const type = form.typeOf(id)
        if (!type || !type.available || form.draft.when.type === id)
            return
        const event = (type.events || []).find(e => e.available)
        form.draft.when = { type: id, event: event ? event.id : "", match: form.draft.when.match, if: [] }
        form.changed()
    }
    function setEvent(id) {
        form.draft.when.event = id
        form.changed()
    }
    function setMatch(any) {
        form.draft.when.match = any ? "any" : "all"
        form.changed()
    }
    function addCondition() {
        const type = form.whenType()
        const field = type ? (type.fields || []).find(f => f.available) : null
        if (!field)
            return
        form.draft.when.if.push({ field: field.id, test: field.tests[0].id, value: [] })
        form.changed()
    }
    function removeCondition(index) {
        form.draft.when.if.splice(index, 1)
        form.changed()
    }
    function setField(index, id) {
        const field = form.fieldOf(id)
        if (!field || !field.available)
            return
        form.draft.when.if[index] = { field: id, test: field.tests[0].id, value: [] }
        form.changed()
    }
    function setTest(index, id) {
        const one = form.draft.when.if[index]
        const field = form.fieldOf(one.field)
        const before = form.valueKind(field, form.testOf(field, one.test))
        one.test = id
        // A value of another kind is no value for this test.
        if (form.valueKind(field, form.testOf(field, id)) !== before)
            one.value = []
        if (id !== "between")
            delete one.until
        form.changed()
    }
    function setValue(index, value, until) {
        const one = form.draft.when.if[index]
        one.value = value
        if (until !== undefined && until !== "")
            one.until = until
        else
            delete one.until
        form.changed()
    }
    function setAction(type, action) {
        if (form.draft.then.type === type && form.draft.then.action === action)
            return
        form.draft.then = { type: type, action: action, with: {} }
        form.changed()
    }
    // A parameter fixed (a value) or taken from what triggered the rule ({from: field}); undefined: unset.
    function setParam(id, value) {
        if (value === undefined || value === "" || (Array.isArray(value) && value.length === 0))
            delete form.draft.then.with[id]
        else
            form.draft.then.with[id] = value
        form.changed()
    }

    // What the rule would have done in the last 30 days, off the window's thread.
    function lookBack() {
        if (!form.draft || form.problem !== "")
            return
        form.looking = true
        form.previewAsked = JSON.stringify(form.draft)
        form.sioul.rulePreview(form.previewAsked)
    }

    function save() {
        const answer = JSON.parse(form.sioul.saveRule(JSON.stringify(form.draft), form.alsoPast) || "{}")
        if (answer.error) {
            form.refused = answer.error
            return
        }
        form.close()
        form.saved(answer.id || "")
    }

    onClosed: {
        previewTimer.stop()
        form.looking = false
    }

    Timer {
        id: previewTimer

        interval: 500
        onTriggered: form.lookBack()
    }

    Connections {
        target: form.sioul

        // The preview found: shown if it is the one asked last (`asked`, the rule as it was sent).
        function onRulesPreviewed(found) {
            if (!form.visible)
                return
            const shown = JSON.parse(found || "null")
            if (shown === null || shown.asked !== form.previewAsked)
                return
            form.looking = false
            form.preview = shown
        }
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(640, (parent ? form.roomWidth : 640) - 2 * form.theme.gap)
    title: form.draft && form.draft.id ? form.sioul.text("rules-editor-edit") : form.sioul.text("rules-editor-new")

    // A long rule in a small window: the form scrolls, its foot stays.
    contentItem: ScrollView {
        id: scroll

        implicitHeight: Math.min(column.implicitHeight, (form.parent ? form.roomHeight : 700) - 170)
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: form.theme.sectionGap

            // The rule in words: what the list and the status line will say.
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2

                Label {
                    objectName: "ruleSentence"
                    Layout.fillWidth: true
                    text: form.sentence
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 16
                    lineHeight: 1.2
                    color: form.theme.text
                }
                Label {
                    objectName: "ruleProblem"
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: form.refused !== "" ? form.refused : form.problem
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: form.theme.warm
                }
            }

            // ---------------------------------------------------- When
            Section {
                objectName: "ruleWhen"
                Layout.fillWidth: true
                theme: form.theme
                title: form.sioul.text("rules-when")
                body.spacing: form.theme.rowGap

                // A type, and what happens to it.
                GridLayout {
                    Layout.fillWidth: true
                    columns: form.narrow ? 1 : 2
                    columnSpacing: form.theme.labelGap
                    rowSpacing: form.theme.labelGap

                    RuleCombo {
                        sioul: form.sioul
                        theme: form.theme
                        objectName: "ruleType"
                        Layout.fillWidth: true
                        things: form.catalogue.types || []
                        chosen: form.draft ? form.draft.when.type : ""
                        Accessible.name: form.sioul.text("rules-when-type")
                        onPicked: id => form.setType(id)
                    }
                    RuleCombo {
                        sioul: form.sioul
                        theme: form.theme
                        objectName: "ruleEvent"
                        Layout.fillWidth: true
                        things: form.whenType() ? form.whenType().events || [] : []
                        chosen: form.draft ? form.draft.when.event : ""
                        Accessible.name: form.sioul.text("rules-when-event")
                        onPicked: id => form.setEvent(id)
                    }
                }

                // Every condition, or one of them, asked once there are two.
                RowLayout {
                    visible: form.draft !== null && form.draft.when.if.length > 1
                    Layout.fillWidth: true
                    spacing: form.theme.labelGap

                    PlainComboBox {
                        objectName: "ruleMatch"
                        Layout.fillWidth: true
                        model: [form.sioul.text("rules-match-all"), form.sioul.text("rules-match-any")]
                        currentIndex: form.draft && form.draft.when.match === "any" ? 1 : 0
                        Accessible.name: form.sioul.text("rules-match")
                        onActivated: index => form.setMatch(index === 1)
                    }
                }

                Repeater {
                    model: form.draft ? form.draft.when.if : []

                    // A condition: what it reads and how it compares, ✕; under them its value.
                    delegate: ColumnLayout {
                        id: condition

                        required property var modelData
                        required property int index
                        readonly property var field: form.fieldOf(condition.modelData.field)
                        readonly property var test: form.testOf(condition.field, condition.modelData.test)

                        objectName: "ruleCondition:" + condition.index
                        Layout.fillWidth: true
                        spacing: form.theme.labelGap

                        GridLayout {
                            Layout.fillWidth: true
                            columns: form.narrow ? 2 : 3
                            columnSpacing: form.theme.labelGap
                            rowSpacing: form.theme.labelGap

                            RuleCombo {
                                sioul: form.sioul
                                theme: form.theme
                                objectName: "ruleField"
                                Layout.row: 0
                                Layout.column: 0
                                Layout.fillWidth: true
                                things: form.whenType() ? form.whenType().fields || [] : []
                                chosen: condition.modelData.field
                                Accessible.name: form.sioul.text("rules-field")
                                onPicked: id => form.setField(condition.index, id)
                            }
                            RuleCombo {
                                sioul: form.sioul
                                theme: form.theme
                                objectName: "ruleTest"
                                Layout.row: form.narrow ? 1 : 0
                                Layout.column: form.narrow ? 0 : 1
                                Layout.columnSpan: form.narrow ? 2 : 1
                                Layout.fillWidth: true
                                things: condition.field ? condition.field.tests || [] : []
                                chosen: condition.modelData.test
                                Accessible.name: form.sioul.text("rules-test")
                                onPicked: id => form.setTest(condition.index, id)
                            }
                            ToolButton {
                                objectName: "ruleConditionRemove"
                                Layout.row: 0
                                Layout.column: form.narrow ? 1 : 2
                                text: "✕"
                                Accessible.name: form.sioul.text("rules-condition-remove")
                                ToolTip.visible: hovered
                                ToolTip.text: form.theme.plain(Accessible.name)
                                ToolTip.delay: 400
                                onClicked: form.removeCondition(condition.index)
                            }
                        }
                        RuleValue {
                            objectName: "ruleValue"
                            Layout.fillWidth: true
                            sioul: form.sioul
                            theme: form.theme
                            kind: form.valueKind(condition.field, condition.test)
                            between: condition.modelData.test === "between"
                            things: condition.field && condition.field.choices ? condition.field.choices : form.choices[form.valueKind(condition.field, condition.test)] || []
                            value: condition.modelData.value || []
                            until: condition.modelData.until || ""
                            name: condition.field ? condition.field.name : ""
                            onEdited: (value, until) => form.setValue(condition.index, value, until)
                        }
                    }
                }

                Button {
                    objectName: "ruleAddCondition"
                    Layout.alignment: Qt.AlignLeft
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    enabled: form.whenType() !== null
                    text: "+  " + form.sioul.text("rules-add-condition")
                    onClicked: form.addCondition()
                }
            }

            // ---------------------------------------------------- Then
            Section {
                objectName: "ruleThen"
                Layout.fillWidth: true
                theme: form.theme
                title: form.sioul.text("rules-then")
                body.spacing: form.theme.rowGap

                RuleCombo {
                    sioul: form.sioul
                    theme: form.theme
                    objectName: "ruleAction"
                    Layout.fillWidth: true
                    things: form.actions.map(a => ({ id: a.type + "/" + a.action, name: a.name, available: a.available }))
                    chosen: form.draft && form.draft.then.action !== "" ? form.draft.then.type + "/" + form.draft.then.action : ""
                    placeholder: form.sioul.text("rules-choose-action")
                    Accessible.name: form.sioul.text("rules-then-action")
                    onPicked: id => form.setAction(id.split("/")[0], id.split("/").slice(1).join("/"))
                }

                Repeater {
                    model: form.actionOf() ? form.actionOf().params || [] : []

                    // A parameter: its name; fixed here or from what triggered the rule, when both
                    // are possible; then its control, or the field it is taken from.
                    delegate: ColumnLayout {
                        id: param

                        required property var modelData
                        readonly property var given: form.draft ? form.draft.then.with[param.modelData.id] : undefined
                        readonly property bool fromIt: param.given !== undefined && param.given !== null && typeof param.given === "object" && !Array.isArray(param.given) && param.given.from !== undefined
                        readonly property bool both: param.modelData.fixed !== false && (param.modelData.from || []).length > 0
                        // The trigger's fields it may be taken from, by their names.
                        readonly property var sources: (param.modelData.from || []).map(f => ({ id: f.id, name: f.name, available: f.available !== false }))

                        objectName: "ruleParam:" + param.modelData.id
                        Layout.fillWidth: true
                        spacing: form.theme.labelGap

                        Label {
                            Layout.fillWidth: true
                            text: param.modelData.required ? param.modelData.name : form.sioul.textWith("rules-optional", "name", param.modelData.name)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: form.theme.muted
                        }
                        // Chosen here, or from what triggered it: one choice of two.
                        Flow {
                            visible: param.both
                            Layout.fillWidth: true
                            spacing: form.theme.labelGap

                            Button {
                                objectName: "ruleParamFixed"
                                checkable: true
                                autoExclusive: true
                                checked: !param.fromIt
                                text: form.sioul.text("rules-chosen-here")
                                onClicked: {
                                    if (param.fromIt)
                                        form.setParam(param.modelData.id, undefined)
                                }
                            }
                            Button {
                                objectName: "ruleParamFrom"
                                checkable: true
                                autoExclusive: true
                                checked: param.fromIt
                                text: form.sioul.text("rules-taken-from")
                                onClicked: {
                                    if (!param.fromIt && param.sources.length > 0)
                                        form.setParam(param.modelData.id, { from: param.sources[0].id })
                                }
                            }
                        }
                        RuleValue {
                            visible: param.modelData.fixed !== false && !param.fromIt
                            Layout.fillWidth: true
                            sioul: form.sioul
                            theme: form.theme
                            kind: param.modelData.value
                            single: true
                            things: param.modelData.choices ? param.modelData.choices : form.choices[param.modelData.value] || []
                            value: param.given === undefined || param.fromIt ? [] : Array.isArray(param.given) ? param.given : [param.given]
                            name: param.modelData.name
                            onEdited: (value, until) => form.setParam(param.modelData.id, param.modelData.value === "number" ? (value.length > 0 ? Number(value[0]) : undefined) : value.length > 0 ? value[0] : undefined)
                        }
                        RuleCombo {
                            sioul: form.sioul
                            theme: form.theme
                            visible: param.fromIt || (param.modelData.fixed === false && param.sources.length > 0)
                            Layout.fillWidth: true
                            things: param.sources
                            chosen: param.fromIt ? param.given.from : ""
                            Accessible.name: param.modelData.name
                            onPicked: id => form.setParam(param.modelData.id, { from: id })
                        }
                        // A model of task of your own, named here when none fits: kept with
                        // your rules (a general model), chosen at once.
                        Fold {
                            objectName: "ruleNewModel"
                            visible: param.modelData.value === "model"
                            Layout.fillWidth: true
                            theme: form.theme
                            title: form.sioul.text("rules-model-new")
                            titleSize: 13

                            GridLayout {
                                id: model

                                columns: form.narrow ? 1 : 2
                                columnSpacing: form.theme.labelGap
                                rowSpacing: form.theme.labelGap

                                Label {
                                    text: form.sioul.text("rules-model-name")
                                    color: form.theme.muted
                                }
                                TextField {
                                    id: modelName

                                    objectName: "ruleModelName"
                                    Layout.fillWidth: true
                                    placeholderText: form.sioul.text("rules-model-name-hint")
                                    Accessible.name: form.sioul.text("rules-model-name")
                                }
                                Label {
                                    text: form.sioul.text("rules-model-prefix")
                                    color: form.theme.muted
                                }
                                TextField {
                                    id: modelPrefix

                                    Layout.fillWidth: true
                                    placeholderText: form.sioul.text("rules-model-prefix-hint")
                                    Accessible.name: form.sioul.text("rules-model-prefix")
                                }
                                Label {
                                    text: form.sioul.text("rules-model-list")
                                    color: form.theme.muted
                                }
                                RuleCombo {
                                    id: modelList

                                    sioul: form.sioul
                                    theme: form.theme
                                    Layout.fillWidth: true
                                    things: [{ id: "", name: form.sioul.text("rules-model-list-usual"), available: true }].concat(form.choices.list || [])
                                    Accessible.name: form.sioul.text("rules-model-list")
                                    onPicked: id => modelList.chosen = id
                                }
                                Label {
                                    text: form.sioul.text("rules-model-days")
                                    color: form.theme.muted
                                }
                                SpinBox {
                                    id: modelDays

                                    from: 0
                                    to: 60
                                    value: 3
                                    editable: true
                                    Accessible.name: form.sioul.text("rules-model-days")
                                }
                                Button {
                                    objectName: "ruleModelKeep"
                                    Layout.columnSpan: form.narrow ? 1 : 2
                                    enabled: modelName.text.trim() !== ""
                                    text: form.sioul.text("rules-model-keep")
                                    onClicked: {
                                        const answer = JSON.parse(form.sioul.saveRuleModel(JSON.stringify({ id: "", name: modelName.text.trim(), prefix: modelPrefix.text.trim(), list: modelList.chosen, business_days: modelDays.value })) || "{}")
                                        if (answer.error) {
                                            form.refused = answer.error
                                            return
                                        }
                                        form.choices = JSON.parse(form.sioul.rulesChoices() || "null") || form.choices
                                        form.setParam("model", answer.id)
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---------------------------------------------------- What it would have done
            Section {
                objectName: "rulePreview"
                visible: form.problem === "" && form.draft !== null
                Layout.fillWidth: true
                theme: form.theme
                title: form.sioul.text("rules-preview-title")
                body.spacing: 4

                Label {
                    Layout.fillWidth: true
                    text: form.looking || form.preview === null ? form.sioul.text("rules-preview-looking") : form.preview.text
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: form.looking ? form.theme.muted : form.theme.text
                }
                Repeater {
                    model: !form.looking && form.preview ? form.preview.lines : []

                    delegate: Label {
                        required property var modelData

                        Layout.fillWidth: true
                        text: "•  " + modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        lineHeight: 1.2
                        color: form.theme.text
                    }
                }
                // Those messages too, once each: a choice made at saving, unticked.
                CheckBox {
                    objectName: "ruleAlsoPast"
                    visible: !form.looking && form.preview !== null && form.preview.count > 0
                    Layout.fillWidth: true
                    checked: form.alsoPast
                    text: form.preview ? form.sioul.textCounted("rules-also-past", form.preview.count) : ""
                    onToggled: form.alsoPast = checked
                }
            }
        }
    }

    footer: DialogFoot {
        sioul: form.sioul
        theme: form.theme
        mainText: form.sioul.text("ui-save")
        mainEnabled: form.draft !== null && form.problem === ""
        onAccepted: form.save()
        onRejected: form.close()
    }
}
