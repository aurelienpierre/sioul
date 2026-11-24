// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Rules (« Règles »; docs/rules.md, "The view"): one flat list of
// sentences, "when this happens, do that", grouped under what happens
// ("When a message arrives"), the rules that share the same "when" shown
// together under it, each then by its own bullet. Your own rules, each with
// its switch, Edit, ✕ (Undo in the status line) and "What it did lately":
// each act with Undo and "Not for this one". The rules kept today in their
// own places (a project's routes and the tasks they make, a budget's mail
// rules, Mail's filters, a topic's senders) are in the same list, said by the
// same words, read-only here: each says where it is kept and opens its own
// editor there; nothing of them is copied. One button, "Add a rule…", offers
// three starts (RuleStart.qml): from nothing, from a message, from a preset;
// the editor (RuleEditor.qml) opens on it. A second tab, a conscious choice
// away: Presets, the rules that come with Sioul, what it does by itself
// today, grouped the same way: a line of words each, its switch where a
// setting turns it on or off today, "Details" where it has more to change
// or can be copied (PresetCard.qml). Every sentence comes from the core
// (`rulesView`, made by one function: the editor and the status line say the
// same words). Everything opened on demand is made the first time. Read
// again when the rules' list or their record of acts changes, here or on
// another device.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: tab

    objectName: "rulesTab"

    required property var sioul
    required property var theme

    // What `rulesView` gives: {groups: [{heading, sets: [{when, rules}]}], presets (the same shape), readOnly, note, count}.
    property var view: ({ groups: [], presets: [], readOnly: false, note: "", count: 0 })
    // The tab shown: your rules, or the presets.
    property string part: "own"
    // On a phone: the buttons under each sentence rather than beside it.
    readonly property bool narrow: tab.width < 520
    // The editor, once made.
    readonly property var editor: editorLater.item

    function reload() {
        tab.view = JSON.parse(tab.sioul.rulesView() || "null") || tab.view
        if (tab.view.presets.length === 0)
            tab.part = "own"
        const card = presetLater.item as PresetCard
        if (card && card.visible && card.line)
            card.line = tab.presetLine(card.line.id) || card.line
    }

    // A rule to change, or a new one (null), or a draft made elsewhere.
    function edit(rule) {
        editorLater.now().edit(rule)
    }

    // A start chosen in "Add a rule…": the editor on a blank rule, a message's draft, a preset's copy.
    function startFrom(kind, id) {
        if (kind === "nothing") {
            tab.edit(null)
            return
        }
        const draft = JSON.parse((kind === "message" ? tab.sioul.ruleFromMessage(id) : tab.sioul.ruleFromPreset(id)) || "null")
        if (draft === null || draft.error) {
            tab.sioul.status = draft ? draft.error : ""
            return
        }
        tab.edit(draft)
    }

    // "Add a rule…": its three starts.
    function add() {
        startLater.now().ask()
    }

    // "Add a rule…" closed, if it was ever opened (the window's pictures).
    function closeStart() {
        startLater.close()
    }

    // A preset's card, by its id (the window's pictures).
    function openPreset(id) {
        const line = tab.presetLine(id)
        if (line)
            presetLater.now().show(line)
    }

    // A preset's line, by its id.
    function presetLine(id) {
        for (const group of tab.view.presets) {
            for (const set of group.sets) {
                const found = set.rules.find(r => r.id === id)
                if (found)
                    return found
            }
        }
        return null
    }

    // A rule's line, by its id (tests and the window's pictures).
    function line(id) {
        const find = item => {
            if (item.objectName === "rule:" + id)
                return item
            for (let i = 0; i < item.children.length; i++) {
                const found = find(item.children[i])
                if (found)
                    return found
            }
            return null
        }
        return find(tab)
    }

    Layout.fillWidth: true
    spacing: tab.theme.rowGap
    Component.onCompleted: tab.reload()

    Connections {
        target: tab.sioul

        // The rules or what they did changed, here or on another device.
        function onRulesChanged() {
            tab.reload()
        }
    }

    // Your rules, and the presets, a conscious choice away.
    Tabs {
        objectName: "rulesTabs"
        visible: tab.view.presets.length > 0
        Layout.fillWidth: true
        theme: tab.theme
        name: tab.sioul.text("settings-tab-rules")
        model: tab.view.presets.length > 0 ? [{ id: "own", text: tab.sioul.text("rules-tab-own") }, { id: "presets", text: tab.sioul.text("rules-tab-presets") }] : []
        current: tab.part
        onChosen: id => tab.part = id
    }

    Label {
        Layout.fillWidth: true
        text: tab.sioul.text(tab.part === "own" ? "rules-about" : "rules-presets-about")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.25
        color: tab.theme.muted
    }
    // Written by a newer Sioul, or not read: said once, nothing changed here.
    Label {
        objectName: "rulesNote"
        visible: tab.view.note !== ""
        Layout.fillWidth: true
        text: tab.view.note
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.warm
    }

    Button {
        objectName: "rulesAdd"
        visible: tab.part === "own"
        enabled: !tab.view.readOnly
        Layout.alignment: Qt.AlignLeft
        implicitWidth: implicitContentWidth + leftPadding + rightPadding
        text: "+  " + tab.sioul.text("rules-add")
        onClicked: tab.add()
    }

    // Nothing yet: one line that invites.
    Label {
        objectName: "rulesNone"
        visible: tab.part === "own" && tab.view.groups.length === 0
        Layout.fillWidth: true
        text: tab.sioul.text("rules-none")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.muted
    }

    // What happens, one frame each; inside, the rules that share a "when" together.
    Repeater {
        model: tab.part === "own" ? tab.view.groups : []

        delegate: Section {
            id: group

            required property var modelData
            required property int index

            objectName: "rulesGroup:" + group.index
            Layout.fillWidth: true
            Layout.topMargin: tab.theme.labelGap
            theme: tab.theme
            title: group.modelData.heading
            body.spacing: tab.theme.sectionGap

            Repeater {
                model: group.modelData.sets

                delegate: ColumnLayout {
                    id: set

                    required property var modelData

                    readonly property bool shared: set.modelData.rules.length > 1

                    Layout.fillWidth: true
                    spacing: tab.theme.rowGap

                    // Their one "when", said once.
                    Label {
                        visible: set.shared
                        Layout.fillWidth: true
                        text: set.modelData.when
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.weight: Font.DemiBold
                        color: tab.theme.text
                    }
                    Repeater {
                        model: set.modelData.rules

                        // One rule: its switch, its sentence (or, under a shared "when", what it
                        // does), Edit and ✕; what it did lately, folded. A rule kept elsewhere:
                        // where, and a link that opens it there.
                        delegate: ColumnLayout {
                            id: line

                            required property var modelData
                            readonly property var rule: line.modelData
                            readonly property bool shared: set.shared
                            readonly property bool own: line.rule.own === true
                            readonly property bool on: line.rule.on !== false

                            objectName: "rule:" + line.rule.id
                            Layout.fillWidth: true
                            Layout.leftMargin: line.shared ? tab.theme.indent : 0
                            spacing: 2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: tab.theme.labelGap

                                Switch {
                                    objectName: "ruleSwitch"
                                    visible: line.own
                                    Layout.alignment: Qt.AlignTop
                                    checked: line.on
                                    enabled: !tab.view.readOnly
                                    Accessible.name: tab.sioul.text("rules-switch")
                                    ToolTip.visible: hovered
                                    ToolTip.text: tab.sioul.text("rules-switch")
                                    ToolTip.delay: 400
                                    // Said in the status line by Sioul; the list read again.
                                    onToggled: {
                                        tab.sioul.setRuleOn(line.rule.id, checked)
                                        tab.reload()
                                    }
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    // What the row leaves it: its words wrap, never widening the row.
                                    Layout.preferredWidth: 0
                                    spacing: 2

                                    Label {
                                        objectName: "ruleWords"
                                        Layout.fillWidth: true
                                        text: line.shared ? "•  " + line.rule.then : line.rule.sentence
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        lineHeight: 1.2
                                        color: line.on ? tab.theme.text : tab.theme.muted
                                    }
                                    // Why it cannot run (a project gone, a newer Sioul's), or that it is off where it is kept.
                                    Label {
                                        visible: text !== ""
                                        Layout.fillWidth: true
                                        text: line.rule.problem || ""
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        font.pixelSize: 13
                                        color: tab.theme.warm
                                    }
                                    // Kept elsewhere: where, said plainly.
                                    Label {
                                        visible: !line.own && text !== ""
                                        Layout.fillWidth: true
                                        text: line.rule.kept || ""
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        font.pixelSize: 13
                                        color: tab.theme.muted
                                    }
                                    Link {
                                        objectName: "ruleOpen"
                                        visible: !line.own && !!line.rule.open
                                        Layout.fillWidth: true
                                        theme: tab.theme
                                        text: line.rule.openText || ""
                                        font.pixelSize: 13
                                        onActivated: tab.sioul.reminderOpened(line.rule.open.kind, line.rule.open.uri || "", line.rule.open.key || "")
                                    }
                                    // On a phone, Edit and ✕ under the words.
                                    Flow {
                                        visible: line.own && tab.narrow
                                        Layout.fillWidth: true
                                        spacing: tab.theme.labelGap

                                        Button {
                                            objectName: "ruleEditNarrow"
                                            enabled: !tab.view.readOnly && !line.rule.unknown
                                            text: tab.sioul.text("ui-edit")
                                            onClicked: tab.edit(line.rule.rule)
                                        }
                                        Button {
                                            objectName: "ruleRemoveNarrow"
                                            enabled: !tab.view.readOnly
                                            text: tab.sioul.text("ui-remove")
                                            onClicked: tab.sioul.removeRule(line.rule.id)
                                        }
                                    }
                                }
                                Button {
                                    objectName: "ruleEdit"
                                    visible: line.own && !tab.narrow
                                    Layout.alignment: Qt.AlignTop
                                    enabled: !tab.view.readOnly && !line.rule.unknown
                                    text: tab.sioul.text("ui-edit")
                                    onClicked: tab.edit(line.rule.rule)
                                }
                                ToolButton {
                                    objectName: "ruleRemove"
                                    visible: line.own && !tab.narrow
                                    Layout.alignment: Qt.AlignTop
                                    enabled: !tab.view.readOnly
                                    text: "✕"
                                    Accessible.name: tab.sioul.text("rules-remove")
                                    ToolTip.visible: hovered
                                    ToolTip.text: tab.sioul.text("rules-remove")
                                    ToolTip.delay: 400
                                    onClicked: tab.sioul.removeRule(line.rule.id)
                                }
                            }

                            // What it did lately: made when first unfolded.
                            Fold {
                                objectName: "ruleLately"
                                visible: line.own && (line.rule.lately || []).length > 0
                                Layout.fillWidth: true
                                Layout.leftMargin: tab.narrow ? 0 : 52
                                theme: tab.theme
                                title: tab.sioul.textCounted("rules-lately", (line.rule.lately || []).length)
                                titleSize: 13

                                ColumnLayout {
                                    spacing: tab.theme.rowGap

                                    Repeater {
                                        model: line.rule.lately || []

                                        delegate: ColumnLayout {
                                            id: act

                                            required property var modelData

                                            Layout.fillWidth: true
                                            spacing: 2

                                            Label {
                                                Layout.fillWidth: true
                                                text: act.modelData.line
                                                textFormat: Text.PlainText
                                                wrapMode: Text.Wrap
                                                font.pixelSize: 13
                                                color: act.modelData.undone ? tab.theme.muted : tab.theme.text
                                            }
                                            Flow {
                                                visible: !act.modelData.undone && !tab.view.readOnly
                                                Layout.fillWidth: true
                                                spacing: tab.theme.labelGap

                                                Button {
                                                    objectName: "ruleActUndo"
                                                    font.pixelSize: 13
                                                    text: tab.sioul.text("ui-undo")
                                                    Accessible.name: act.modelData.line + ", " + text
                                                    onClicked: tab.sioul.undoRuleAct(JSON.stringify(act.modelData.act))
                                                }
                                                Button {
                                                    objectName: "ruleActNotThis"
                                                    font.pixelSize: 13
                                                    text: tab.sioul.text("rules-not-this-one")
                                                    Accessible.name: act.modelData.line + ", " + text
                                                    onClicked: tab.sioul.ruleNotForThisOne(line.rule.id, act.modelData.object)
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // The presets, grouped by what happens as your rules are: one line of words
    // each, its switch where a setting turns it on or off today, Details where
    // it has more to change or can be copied.
    Repeater {
        model: tab.part === "presets" ? tab.view.presets : []

        delegate: Section {
            id: presetGroup

            required property var modelData
            required property int index

            objectName: "presetsGroup:" + presetGroup.index
            Layout.fillWidth: true
            Layout.topMargin: tab.theme.labelGap
            theme: tab.theme
            title: presetGroup.modelData.heading
            body.spacing: tab.theme.rowGap

            Repeater {
                model: presetGroup.modelData.sets.reduce((all, set) => all.concat(set.rules), [])

                delegate: RowLayout {
                    id: preset

                    required property var modelData
                    // Its on and off, when a setting of its own says it (a switch).
                    readonly property var onOff: (preset.modelData.settings || []).find(r => r.key === preset.modelData.preset.setting && r.kind === "bool") || null
                    readonly property bool more: (preset.modelData.settings || []).some(r => preset.onOff === null || r.key !== preset.onOff.key) || preset.modelData.preset.offered === true

                    objectName: "preset:" + preset.modelData.id
                    Layout.fillWidth: true
                    spacing: tab.theme.labelGap

                    Switch {
                        visible: preset.onOff !== null
                        Layout.alignment: Qt.AlignTop
                        checked: preset.onOff !== null && preset.onOff.value === true
                        Accessible.name: preset.onOff ? preset.onOff.label : ""
                        onToggled: {
                            const problem = tab.sioul.setSetting(preset.onOff.key, JSON.stringify(checked))
                            if (problem !== "")
                                tab.sioul.status = problem
                            tab.reload()
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        Layout.preferredWidth: 0
                        text: preset.modelData.sentence
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        lineHeight: 1.2
                        color: preset.onOff === null || preset.onOff.value === true ? tab.theme.text : tab.theme.muted
                    }
                    Button {
                        objectName: "presetDetails"
                        visible: preset.more
                        Layout.alignment: Qt.AlignTop
                        text: tab.sioul.text("rules-preset-open")
                        Accessible.name: preset.modelData.sentence + ", " + text
                        onClicked: presetLater.now().show(preset.modelData)
                    }
                }
            }
        }
    }

    Later {
        id: startLater

        sourceComponent: Component {
            RuleStart {
                sioul: tab.sioul
                theme: tab.theme
                onChosen: (kind, id) => tab.startFrom(kind, id)
            }
        }
    }

    Later {
        id: presetLater

        sourceComponent: Component {
            PresetCard {
                sioul: tab.sioul
                theme: tab.theme
                onChanged: tab.reload()
                onCopied: id => tab.startFrom("preset", id)
            }
        }
    }

    Later {
        id: editorLater

        sourceComponent: Component {
            RuleEditor {
                sioul: tab.sioul
                theme: tab.theme
                onSaved: tab.reload()
            }
        }
    }
}
