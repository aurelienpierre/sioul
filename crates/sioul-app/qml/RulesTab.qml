// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Rules (« Règles »; docs/rules.md, "The view"): one flat list of
// sentences, "when this happens, do that", grouped under what happens
// ("When a message arrives"), the rules that share the same "when" shown
// together under it, each then by its own bullet. Your own rules, each with
// its switch, Edit, ✕ (Undo in the status line; a rule a newer Sioul wrote,
// which cannot be put back, asked first) and "What it did lately": each
// result as it stands now (done, taken back, left as it was, taken out by
// the antivirus, waiting), Undo only where the rule wrote it, "Not for this
// one" only where it waits. The rules kept today in their own places (a
// project's routes and the tasks they make, a budget's mail rules, Mail's
// filters, a topic's senders) are in the same groups, said by the same
// words, read-only: one fold per place, which says where they are kept and
// opens that place; nothing of them is copied. One button, "Add a rule…",
// offers its starts (RuleStart.qml); the editor (RuleEditor.qml) opens on
// one. A second tab, a conscious choice away: Presets, the rules that come
// with Sioul, grouped by what they watch, each its own written sentence,
// "Off where it is set." when its setting is off, and a link to where that
// setting lives (a switch is drawn once, there); a preset the engine runs
// (the calls' tasks) has its switch here, its one place, and its choices
// under its sentence (which calls make a task), and "Copy to make my own". Every sentence of a rule
// comes from the core (`rulesView`, one function: the editor and the status
// line say the same words). Everything opened on demand is made the first
// time; a fold stays as it was when the list is read again.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: tab

    objectName: "rulesTab"

    required property var sioul
    required property var theme

    // What `rulesView` gives: {groups: [{heading, sets: [{when, rules}], places: [{id, title, kept, open, openText, rules}]}],
    // presets: [{heading, presets: [{id, sentence, off, open, openText, offered, engine, choices: [{id, text, on}]}]}], readOnly, note, count}.
    property var view: ({ groups: [], presets: [], readOnly: false, note: "", count: 0 })
    // The tab shown: your rules, or the presets.
    property string part: "own"
    // The folds open, by their ids ("lately:<rule>", "place:<id>"): kept as the list is read again.
    property var unfolded: ({})
    // A rule a newer Sioul wrote, waiting for its removal to be confirmed.
    property string removing: ""
    // On a phone: the buttons under each sentence rather than beside it.
    readonly property bool narrow: tab.width < 520
    // The editor, once made.
    readonly property var editor: editorLater.item
    // "Add a rule…"'s card, once made (the window's pictures).
    readonly property var starts: startLater.item

    function reload() {
        tab.view = JSON.parse(tab.sioul.rulesView() || "null") || tab.view
        if (tab.view.presets.length === 0)
            tab.part = "own"
    }

    // A fold opened or closed: kept by its id.
    function fold(id, open) {
        const kept = Object.assign({}, tab.unfolded)
        kept[id] = open
        tab.unfolded = kept
    }

    // A rule to change, or a new one (null), or a draft made elsewhere.
    function edit(rule) {
        editorLater.now().edit(rule)
    }

    // ✕: removed at once with Undo; one a newer Sioul wrote, which nothing here can put back, asked first.
    function remove(line) {
        if (line.unknown) {
            tab.removing = line.id
            confirmLater.now().ask(tab.sioul.text("rules-remove-newer-title"), tab.sioul.text("rules-remove-newer"), tab.sioul.text("rules-remove-for-good"))
            return
        }
        tab.sioul.removeRule(line.id)
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

    // "Add a rule…": its starts.
    function add() {
        startLater.now().ask()
    }

    // "Add a rule…" closed, if it was ever opened (the window's pictures).
    function closeStart() {
        startLater.close()
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

    // What happens, one frame each: your rules by their "when", then one fold per place that keeps rules of its own.
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

                        // One rule of yours: its switch, its sentence (under a shared "when", what
                        // it does), Edit and ✕; what it did lately, folded.
                        delegate: ColumnLayout {
                            id: line

                            required property var modelData
                            readonly property var rule: line.modelData
                            readonly property bool on: line.rule.on !== false

                            objectName: "rule:" + line.rule.id
                            Layout.fillWidth: true
                            Layout.leftMargin: set.shared ? tab.theme.indent : 0
                            spacing: 2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: tab.theme.labelGap

                                Switch {
                                    objectName: "ruleSwitch"
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
                                        text: set.shared ? "•  " + line.rule.then : line.rule.sentence
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        lineHeight: 1.2
                                        color: line.on ? tab.theme.text : tab.theme.muted
                                    }
                                    // Why it cannot run (a project gone, a newer Sioul's).
                                    Label {
                                        visible: text !== ""
                                        Layout.fillWidth: true
                                        text: line.rule.problem || ""
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        font.pixelSize: 13
                                        color: tab.theme.warm
                                    }
                                    // On a phone, Edit and Remove under the words.
                                    Flow {
                                        visible: tab.narrow
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
                                            onClicked: tab.remove(line.rule)
                                        }
                                    }
                                }
                                Button {
                                    objectName: "ruleEdit"
                                    visible: !tab.narrow
                                    Layout.alignment: Qt.AlignTop
                                    enabled: !tab.view.readOnly && !line.rule.unknown
                                    text: tab.sioul.text("ui-edit")
                                    onClicked: tab.edit(line.rule.rule)
                                }
                                ToolButton {
                                    objectName: "ruleRemove"
                                    visible: !tab.narrow
                                    Layout.alignment: Qt.AlignTop
                                    enabled: !tab.view.readOnly
                                    text: "✕"
                                    Accessible.name: tab.sioul.text("rules-remove")
                                    ToolTip.visible: hovered
                                    ToolTip.text: tab.sioul.text("rules-remove")
                                    ToolTip.delay: 400
                                    onClicked: tab.remove(line.rule)
                                }
                            }

                            // What it did lately: made when first unfolded, kept open as the list is read again.
                            Fold {
                                objectName: "ruleLately"
                                visible: (line.rule.lately || []).length > 0
                                Layout.fillWidth: true
                                Layout.leftMargin: tab.narrow ? 0 : 52
                                theme: tab.theme
                                title: tab.sioul.textCounted("rules-lately", (line.rule.lately || []).length)
                                titleSize: 13
                                controlled: true
                                open: tab.unfolded["lately:" + line.rule.id] === true
                                onToggled: open => tab.fold("lately:" + line.rule.id, open)

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
                                                color: act.modelData.undo || act.modelData.notThis ? tab.theme.text : tab.theme.muted
                                            }
                                            Flow {
                                                visible: (act.modelData.undo || act.modelData.notThis) && !tab.view.readOnly
                                                Layout.fillWidth: true
                                                spacing: tab.theme.labelGap

                                                Button {
                                                    objectName: "ruleActUndo"
                                                    visible: act.modelData.undo
                                                    font.pixelSize: 13
                                                    text: tab.sioul.text("ui-undo")
                                                    Accessible.name: act.modelData.line + ", " + text
                                                    onClicked: tab.sioul.undoRuleAct(JSON.stringify(act.modelData.act))
                                                }
                                                Button {
                                                    objectName: "ruleActNotThis"
                                                    visible: act.modelData.notThis
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

            // The rules a place keeps (a project, a budget, Mail's filters, a topic): one fold,
            // where they are kept and the link there said once, their sentences inside.
            Repeater {
                model: group.modelData.places

                delegate: Fold {
                    id: place

                    required property var modelData

                    objectName: "rulesPlace:" + place.modelData.id
                    Layout.fillWidth: true
                    theme: tab.theme
                    title: place.modelData.title
                    controlled: true
                    open: tab.unfolded["place:" + place.modelData.id] === true
                    onToggled: open => tab.fold("place:" + place.modelData.id, open)

                    ColumnLayout {
                        spacing: tab.theme.rowGap

                        Label {
                            Layout.fillWidth: true
                            text: place.modelData.kept
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: tab.theme.muted
                        }
                        Link {
                            objectName: "ruleOpen"
                            visible: !!place.modelData.open
                            Layout.fillWidth: true
                            theme: tab.theme
                            text: place.modelData.openText || ""
                            onActivated: tab.sioul.reminderOpened(place.modelData.open.kind, place.modelData.open.uri || "", place.modelData.open.key || "")
                        }
                        Repeater {
                            model: place.modelData.rules

                            delegate: ColumnLayout {
                                id: kept

                                required property var modelData

                                objectName: "rule:" + kept.modelData.id
                                Layout.fillWidth: true
                                spacing: 2

                                Label {
                                    objectName: "ruleWords"
                                    Layout.fillWidth: true
                                    text: kept.modelData.sentence
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    lineHeight: 1.2
                                    color: kept.modelData.on ? tab.theme.text : tab.theme.muted
                                }
                                // Off where it is kept, or a filter after which no other is asked.
                                Label {
                                    visible: text !== ""
                                    Layout.fillWidth: true
                                    text: kept.modelData.off || ""
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 13
                                    color: tab.theme.muted
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // The presets, grouped by what they watch: each its written sentence, off said
    // in words, a link to where its setting lives, and a copy where Sioul can run it.
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
                model: presetGroup.modelData.presets

                delegate: ColumnLayout {
                    id: preset

                    required property var modelData

                    objectName: "preset:" + preset.modelData.id
                    Layout.fillWidth: true
                    spacing: 2

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: tab.theme.labelGap

                        // Its switch, here alone, for one the engine runs.
                        Switch {
                            objectName: "presetSwitch"
                            visible: preset.modelData.engine === true
                            Layout.alignment: Qt.AlignTop
                            checked: !preset.modelData.off
                            Accessible.name: tab.sioul.text("rules-preset-switch")
                            ToolTip.visible: hovered
                            ToolTip.text: tab.sioul.text("rules-preset-switch")
                            ToolTip.delay: 400
                            onToggled: {
                                tab.sioul.setPresetOn(preset.modelData.id, checked)
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
                            color: preset.modelData.off ? tab.theme.muted : tab.theme.text
                        }
                    }
                    // Its choices: which calls make a task.
                    Repeater {
                        model: preset.modelData.choices || []

                        delegate: WrapCheckBox {
                            id: choice

                            required property var modelData

                            objectName: "presetChoice:" + choice.modelData.id
                            Layout.fillWidth: true
                            Layout.leftMargin: tab.narrow ? 0 : 52
                            enabled: !preset.modelData.off
                            checked: choice.modelData.on
                            text: choice.modelData.text
                            onToggled: {
                                tab.sioul.setPresetChoice(preset.modelData.id, choice.modelData.id, checked)
                                tab.reload()
                            }
                        }
                    }
                    Label {
                        objectName: "presetOff"
                        visible: preset.modelData.off && preset.modelData.engine !== true
                        Layout.fillWidth: true
                        text: tab.sioul.text("rules-preset-off")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: tab.theme.muted
                    }
                    Link {
                        objectName: "presetWhere"
                        visible: !!preset.modelData.open
                        Layout.fillWidth: true
                        theme: tab.theme
                        font.pixelSize: 13
                        text: preset.modelData.openText || ""
                        onActivated: tab.sioul.reminderOpened(preset.modelData.open.kind, "", "")
                    }
                    Button {
                        objectName: "presetCopy"
                        visible: preset.modelData.offered === true
                        Layout.alignment: Qt.AlignLeft
                        text: tab.sioul.text("rules-preset-copy")
                        onClicked: tab.startFrom("preset", preset.modelData.id)
                    }
                }
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

    // Removing a rule a newer Sioul wrote: asked once, since it cannot be put back.
    Later {
        id: confirmLater

        sourceComponent: Component {
            ConfirmDialog {
                sioul: tab.sioul
                theme: tab.theme
                onConfirmed: {
                    tab.sioul.removeRule(tab.removing)
                    tab.removing = ""
                }
            }
        }
    }
}
