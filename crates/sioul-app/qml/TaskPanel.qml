// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One task, open on the right: its title, what to do with it now (start,
// done, not now), its steps, what it waits for and frees, then, folded, its
// dates, length, case and notes; at the bottom, everything tied to it. Each
// change is saved as it is made: only the lines that changed are written.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import com.aurelienpierre.sioul

Panel {
    id: panel

    required property var sioul
    required property var window
    property string uid: ""
    property var detail: null
    property bool moreShown: false
    // The cases a task can belong to: [{id, title}].
    property var cases: []
    // The kinds of task, as you named them: {id, label}.
    property var kinds: []
    // Every tag in use, to choose one again.
    property var categories: []
    readonly property var card: panel.detail ? panel.detail.card : null
    readonly property bool canEdit: panel.card !== null && !panel.card.read_only
    // What its list does not keep (Google Tasks): greyed, never hidden.
    readonly property var limited: panel.detail && panel.detail.limited ? panel.detail.limited : []

    function keeps(field) {
        return panel.limited.indexOf(field) < 0
    }

    signal closed
    signal focusRequested(string uid, int minutes)

    function reload() {
        if (panel.uid === "") {
            panel.detail = null
            return
        }
        const text = panel.sioul.task(panel.uid)
        if (text !== "")
            panel.detail = JSON.parse(text)
    }

    // Into another list: asked first when it would not keep everything.
    function moveTo(list, confirmed) {
        const answer = JSON.parse(panel.sioul.moveTask(panel.uid, list.id, confirmed))
        if (answer.losses) {
            moveAsk.list = list
            moveAsk.ask(list.name, panel.sioul.textArgs("task-move-ask", JSON.stringify({ place: list.name, list: answer.losses.join(", ") })), panel.sioul.text("move-anyway"))
            return
        }
        if (answer.error)
            error.text = answer.error
        else
            panel.reload()
    }

    // Saves the form with one field changed.
    function change(field, value) {
        if (!panel.detail || !panel.canEdit)
            return
        const edit = Object.assign({}, panel.detail.edit)
        edit[field] = value
        const answer = JSON.parse(panel.sioul.saveTask(panel.uid, JSON.stringify(edit), ""))
        if (answer.error)
            error.text = answer.error
        else
            error.text = ""
    }

    function minutesText(m) {
        if (m === 0)
            return "—"
        if (m < 60)
            return m + " min"
        return Math.floor(m / 60) + " h" + (m % 60 ? " " + (m % 60 < 10 ? "0" : "") + m % 60 : "")
    }

    function addStep() {
        panel.moreShown = false
        steps.focusLine()
    }

    onUidChanged: {
        panel.detail = null
        panel.moreShown = false
        panel.reload()
    }

    Connections {
        target: panel.sioul
        function onTasksChanged() {
            panel.reload()
        }
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 10

            TextField {
                id: title

                Layout.fillWidth: true
                text: panel.card ? panel.card.title : ""
                readOnly: !panel.canEdit
                font.pixelSize: 20
                wrapMode: TextInput.Wrap
                // Like every field, a border: light, darker when it has the focus.
                background: Rectangle {
                    color: "transparent"
                    radius: panel.theme.radius
                    border.color: title.activeFocus ? panel.theme.focus : panel.theme.line
                }
                Accessible.name: panel.sioul.text("task-title")
                onEditingFinished: {
                    if (panel.detail && title.text.trim() !== "" && title.text !== panel.detail.edit.title)
                        panel.change("title", title.text.trim())
                }
            }

            // What to do with it now.
            Flow {
                visible: panel.card !== null && panel.card.status !== "completed" && panel.card.status !== "cancelled" && !panel.card.has_steps
                Layout.fillWidth: true
                spacing: 6

                Button {
                    text: panel.sioul.text("task-start")
                    icon.name: "chronometer-start"
                    icon.color: panel.theme.accentText
                    highlighted: true
                    enabled: panel.canEdit
                    onClicked: startMenu.popup()

                    SioulMenu {
                        id: startMenu

                        Repeater {
                            model: [2, 15, 25, 45, 0]

                            delegate: MenuItem {
                                required property int modelData

                                text: modelData === 2 ? panel.sioul.text("focus-two") : modelData === 0 ? panel.sioul.text("focus-open-ended") : panel.sioul.textWith("focus-for", "minutes", String(modelData))
                                onTriggered: panel.focusRequested(panel.uid, modelData)
                            }
                        }
                    }
                }
                Button {
                    text: panel.sioul.text("task-done")
                    icon.name: "task-complete"
                    icon.color: panel.theme.text
                    enabled: panel.canEdit
                    onClicked: panel.sioul.setTaskStatus(panel.uid, "completed")
                }
                Button {
                    flat: true
                    text: panel.sioul.text("task-not-now")
                    onClicked: panel.sioul.notNow(panel.uid)
                }
            }
            Button {
                visible: panel.card !== null && (panel.card.status === "completed" || panel.card.status === "cancelled")
                flat: true
                text: panel.sioul.text("task-open-again")
                enabled: panel.canEdit
                onClicked: panel.sioul.setTaskStatus(panel.uid, "needs-action")
            }

            // What matters about it, in words.
            Repeater {
                model: panel.card ? [panel.card.due, panel.detail.budget || "", panel.card.estimate, panel.card.waits, panel.card.unblocks, panel.card.stopped, panel.card.spent, panel.card.done_on, panel.detail.steps_total].filter(t => t !== "") : []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: panel.theme.muted
                }
            }
            Label {
                visible: panel.card !== null && panel.card.tight !== ""
                Layout.fillWidth: true
                text: panel.card ? panel.card.tight : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.theme.warm
            }
            // Its tags: each taken off with ×, one more typed or chosen among those in use.
            Flow {
                visible: panel.detail !== null && (panel.canEdit || panel.detail.edit.categories.length > 0)
                Layout.fillWidth: true
                spacing: 6
                opacity: panel.keeps("categories") ? 1 : 0.45

                Repeater {
                    model: panel.detail ? panel.detail.edit.categories : []

                    delegate: Rectangle {
                        id: tag

                        required property string modelData

                        implicitWidth: tagRow.implicitWidth + 12
                        implicitHeight: tagRow.implicitHeight + 4
                        radius: height / 2
                        color: "transparent"
                        border.color: panel.theme.line

                        Row {
                            id: tagRow

                            anchors.centerIn: parent
                            spacing: 2

                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                text: "#" + tag.modelData
                                textFormat: Text.PlainText
                                font.pixelSize: 13
                                color: panel.theme.text
                            }
                            ToolButton {
                                visible: panel.canEdit && panel.keeps("categories")
                                implicitWidth: 22
                                implicitHeight: 22
                                text: "×"
                                Accessible.name: panel.sioul.textWith("task-tag-remove", "tag", tag.modelData)
                                onClicked: panel.change("categories", panel.detail.edit.categories.filter(c => c !== tag.modelData))
                            }
                        }
                    }
                }
                ComboBox {
                    id: newTag

                    readonly property var others: panel.categories.filter(c => !panel.detail || panel.detail.edit.categories.indexOf(c) < 0)

                    visible: panel.canEdit && panel.keeps("categories")
                    width: 170
                    editable: true
                    model: newTag.others
                    currentIndex: -1
                    displayText: ""
                    Accessible.name: panel.sioul.text("task-tag-add")

                    function add(text) {
                        const tag = text.trim().replace(/^#/, "")
                        if (tag !== "" && panel.detail.edit.categories.indexOf(tag) < 0)
                            panel.change("categories", panel.detail.edit.categories.concat([tag]))
                        newTag.editText = ""
                        newTag.currentIndex = -1
                    }

                    onAccepted: newTag.add(newTag.editText)
                    onActivated: index => newTag.add(newTag.others[index])

                    contentItem: TextField {
                        text: newTag.editText
                        placeholderText: "+ " + panel.sioul.text("task-tag-add")
                        font.pixelSize: 13
                        background: Item {}
                        onTextEdited: newTag.editText = text
                        onAccepted: newTag.add(text)
                    }
                }
            }
            Label {
                id: error

                visible: text !== ""
                Layout.fillWidth: true
                wrapMode: Text.Wrap
                color: panel.theme.warm
            }

            // Its steps, and one more.
            Label {
                text: panel.sioul.text("task-steps")
                font.weight: Font.DemiBold
                color: panel.theme.text
                Layout.topMargin: 6
            }
            Repeater {
                model: panel.detail ? panel.detail.steps : []

                delegate: TaskRow {
                    required property var modelData

                    Layout.fillWidth: true
                    task: modelData
                    theme: panel.theme
                    sioul: panel.sioul
                    compact: true
                    onOpen: uid => panel.uid = uid
                    onTick: uid => panel.sioul.setTaskStatus(uid, modelData.status === "completed" ? "needs-action" : "completed")
                }
            }
            CaptureField {
                id: steps

                Layout.fillWidth: true
                visible: panel.canEdit
                enabled: panel.keeps("steps")
                opacity: panel.keeps("steps") ? 1 : 0.45
                sioul: panel.sioul
                theme: panel.theme
                parentUid: panel.uid
                placeholder: panel.keeps("steps") ? panel.sioul.text("task-add-step") : panel.sioul.text("google-tasks-greyed")
            }

            // What it waits for: removable; and one more, found by its title.
            Label {
                text: panel.sioul.text("task-waits-title")
                font.weight: Font.DemiBold
                color: panel.theme.text
                Layout.topMargin: 6
            }
            Repeater {
                model: panel.detail ? panel.detail.waits_for : []

                delegate: RowLayout {
                    id: wait

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        Layout.fillWidth: true
                        text: wait.modelData[1]
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: panel.theme.text

                        TapHandler {
                            onTapped: panel.uid = wait.modelData[0]
                        }
                    }
                    ToolButton {
                        visible: panel.canEdit
                        text: "×"
                        Accessible.name: panel.sioul.text("task-waits-remove")
                        ToolTip.visible: hovered
                        ToolTip.text: panel.sioul.text("task-waits-remove")
                        onClicked: panel.sioul.setWaits(panel.uid, wait.modelData[0], false)
                    }
                }
            }
            TextField {
                id: waitsFor

                visible: panel.canEdit
                enabled: panel.keeps("waits")
                opacity: panel.keeps("waits") ? 1 : 0.45
                Layout.fillWidth: true
                placeholderText: panel.keeps("waits") ? panel.sioul.text("task-waits-add") : panel.sioul.text("google-tasks-greyed")
                onTextEdited: {
                    found.model = waitsFor.text.trim() === "" ? [] : JSON.parse(panel.sioul.searchTasks(waitsFor.text, panel.uid))
                    if (found.model.length > 0)
                        foundPopup.open()
                }
                Keys.onEscapePressed: {
                    waitsFor.clear()
                    foundPopup.close()
                }

                Popup {
                    id: foundPopup

                    y: waitsFor.height
                    width: waitsFor.width
                    padding: 4

                    ListView {
                        id: found

                        implicitHeight: Math.min(contentHeight, 240)
                        width: parent.width
                        clip: true
                        model: []

                        delegate: ItemDelegate {
                            id: option

                            required property var modelData

                            width: found.width
                            // "&" doubled: a button reads one as a key to underline.
                            text: panel.theme.plain(option.modelData.title).replace(/&/g, "&&")
                            onClicked: {
                                panel.sioul.setWaits(panel.uid, option.modelData.uid, true)
                                waitsFor.clear()
                                foundPopup.close()
                            }
                        }
                    }
                }
            }
            Repeater {
                model: panel.detail ? panel.detail.frees : []

                delegate: Label {
                    id: freed

                    required property var modelData

                    Layout.fillWidth: true
                    text: panel.sioul.textWith("task-frees-one", "title", freed.modelData[1])
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: panel.theme.muted

                    TapHandler {
                        onTapped: panel.uid = freed.modelData[0]
                    }
                }
            }

            // The rest, folded.
            Button {
                flat: true
                text: (panel.moreShown ? "▾  " : "▸  ") + panel.sioul.text("ui-more-details")
                onClicked: panel.moreShown = !panel.moreShown
            }
            Label {
                visible: panel.moreShown && panel.limited.length > 0
                Layout.fillWidth: true
                text: panel.sioul.text("task-limited")
                wrapMode: Text.Wrap
                color: panel.theme.muted
            }
            GridLayout {
                visible: panel.moreShown && panel.detail !== null
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 10
                rowSpacing: 6

                Label {
                    text: panel.sioul.text("task-field-start")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("start") ? 1 : 0.45
                }
                DateField {
                    theme: panel.theme
                    locale: panel.window.sioulLocale
                    enabled: panel.canEdit && panel.keeps("start")
                    opacity: panel.keeps("start") ? 1 : 0.45
                    date: panel.detail ? panel.detail.edit.start.slice(0, 10) : ""
                    pickLabel: panel.sioul.text("event-pick-day")
                    onEdited: {
                        if (date.length === 10 || date.replace(/[-_ ]/g, "") === "")
                            panel.change("start", date.replace(/[-_ ]/g, "") === "" ? "" : date)
                    }
                }
                Label {
                    text: panel.sioul.text("task-field-due")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                }
                DateField {
                    theme: panel.theme
                    locale: panel.window.sioulLocale
                    enabled: panel.canEdit
                    date: panel.detail ? panel.detail.edit.due.slice(0, 10) : ""
                    pickLabel: panel.sioul.text("event-pick-day")
                    onEdited: {
                        if (date.length === 10 || date.replace(/[-_ ]/g, "") === "")
                            panel.change("due", date.replace(/[-_ ]/g, "") === "" ? "" : date)
                    }
                }
                Label {
                    text: panel.sioul.text("task-field-estimate")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("estimate") ? 1 : 0.45
                }
                ComboBox {
                    readonly property var minutes: [0, 2, 5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240]

                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("estimate")
                    opacity: panel.keeps("estimate") ? 1 : 0.45
                    model: minutes.map(m => panel.minutesText(m))
                    currentIndex: panel.detail ? Math.max(0, minutes.indexOf(panel.detail.edit.estimate)) : 0
                    onActivated: index => panel.change("estimate", minutes[index])
                }
                Label {
                    text: panel.sioul.text("task-field-case")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("case") ? 1 : 0.45
                }
                ComboBox {
                    readonly property var choices: [{ id: "", title: "—" }].concat(panel.cases)

                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("case")
                    opacity: panel.keeps("case") ? 1 : 0.45
                    model: choices.map(c => panel.theme.plain(c.title))
                    currentIndex: panel.detail ? Math.max(0, choices.findIndex(c => c.id === (panel.detail.edit.cases[0] || ""))) : 0
                    onActivated: index => panel.change("cases", choices[index].id === "" ? [] : [choices[index].id])
                }
                Label {
                    text: panel.sioul.text("task-field-billable")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("billable") ? 1 : 0.45
                }
                // Its time billed or not; unsaid, as its project says (work for a client is billed).
                ComboBox {
                    readonly property var choices: [null, true, false]

                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("billable")
                    opacity: panel.keeps("billable") ? 1 : 0.45
                    model: [panel.sioul.text("task-billable-project"), panel.sioul.text("task-billable-yes"), panel.sioul.text("task-billable-no")]
                    currentIndex: panel.detail ? Math.max(0, choices.indexOf(panel.detail.edit.billable === undefined ? null : panel.detail.edit.billable)) : 0
                    onActivated: index => panel.change("billable", choices[index])
                }
                Label {
                    text: panel.sioul.text("task-field-energy")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("energy") ? 1 : 0.45
                }
                // What it takes: the day's weather says how many heavy ones a day holds;
                // what gives back is offered after a heavy one, never planned in.
                ComboBox {
                    readonly property var choices: ["", "light", "heavy", "rest"]

                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("energy")
                    opacity: panel.keeps("energy") ? 1 : 0.45
                    model: choices.map(c => panel.sioul.text("task-energy-" + (c === "" ? "usual" : c)))
                    currentIndex: panel.detail ? Math.max(0, choices.indexOf(panel.detail.edit.energy || "")) : 0
                    onActivated: index => panel.change("energy", choices[index])
                }
                Label {
                    text: panel.sioul.text("task-kind")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("kind") ? 1 : 0.45
                }
                ComboBox {
                    readonly property string current: panel.detail ? panel.detail.edit.kind : ""
                    // A kind taken away from the choices stays shown on the tasks that have it.
                    readonly property var choices: [{ id: "", label: panel.sioul.text("task-kind-none") }].concat(panel.kinds).concat(current !== "" && !panel.kinds.some(k => k.id === current) ? [{ id: current, label: current }] : [])

                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("kind")
                    opacity: panel.keeps("kind") ? 1 : 0.45
                    model: choices.map(k => panel.theme.plain(k.label))
                    currentIndex: Math.max(0, choices.findIndex(k => k.id === current))
                    ToolTip.visible: hovered
                    ToolTip.text: panel.sioul.text("task-kind-help")
                    ToolTip.delay: 600
                    onActivated: index => panel.change("kind", choices[index].id)
                }
                // What it is for: the hours it comes in (docs/areas.md); none ticked of
                // its own, as its tags say, shown.
                Label {
                    text: panel.sioul.text("task-field-area")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                }
                Flow {
                    id: taskArea

                    readonly property string own: panel.detail !== null ? (panel.detail.edit.area || "") : ""
                    readonly property var on: (taskArea.own !== "" ? taskArea.own : (panel.detail !== null ? panel.detail.area_tags : "")).split("+")

                    Layout.fillWidth: true
                    enabled: panel.canEdit
                    spacing: 4

                    Repeater {
                        model: ["work", "admin", "leisure"]

                        delegate: CheckBox {
                            required property string modelData

                            text: panel.sioul.text("area-" + modelData)
                            checked: taskArea.on.indexOf(modelData) >= 0
                            opacity: taskArea.own === "" ? 0.6 : 1
                            onToggled: {
                                const was = taskArea.on
                                panel.change("area", ["work", "admin", "leisure"].filter(a => a === modelData ? checked : was.indexOf(a) >= 0).join("+"))
                            }
                        }
                    }
                    Label {
                        visible: taskArea.own === ""
                        text: panel.sioul.text("task-area-tags")
                        font.pixelSize: 12
                        color: panel.theme.muted
                    }
                }
                // A call to an office, a form at a counter: only when offices are open.
                CheckBox {
                    Layout.columnSpan: 2
                    opacity: panel.keeps("office") ? 1 : 0.45
                    enabled: panel.canEdit && panel.keeps("office")
                    text: panel.sioul.text("task-office-hours")
                    checked: panel.detail !== null && panel.detail.edit.office_hours
                    ToolTip.visible: hovered
                    ToolTip.text: panel.sioul.text("task-office-hours-help")
                    ToolTip.delay: 600
                    onToggled: panel.change("office_hours", checked)
                }
                // Its office's own hours, when they are not offices' usual ones: one range
                // of days, one or two of hours (a lunch break). Kept short on the task:
                // "mo-fr 09:00-12:00, 14:00-17:00".
                ColumnLayout {
                    id: officeTimes

                    readonly property string stored: panel.detail !== null ? (panel.detail.edit.office_times || "") : ""
                    readonly property var parts: /^([a-z]{2})(?:-([a-z]{2}))? (\d\d:\d\d)-(\d\d:\d\d)(?:, (\d\d:\d\d)-(\d\d:\d\d))?$/.exec(officeTimes.stored)
                    readonly property var days: ["mo", "tu", "we", "th", "fr", "sa", "su"]

                    function save() {
                        const from = officeTimes.days[fromDay.currentIndex]
                        const to = officeTimes.days[toDay.currentIndex]
                        let hours = opens.text + "-" + closes.text
                        if (lunch.checked && opensAgain.text !== "" && closesAgain.text !== "")
                            hours += ", " + opensAgain.text + "-" + closesAgain.text
                        panel.change("office_times", (from === to ? from : from + "-" + to) + " " + hours)
                    }

                    visible: panel.detail !== null && panel.detail.edit.office_hours
                    enabled: panel.canEdit && panel.keeps("office")
                    Layout.columnSpan: 2
                    Layout.leftMargin: 28
                    spacing: 4

                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            height: fromDay.height
                            verticalAlignment: Text.AlignVCenter
                            text: panel.sioul.text("task-office-open")
                            color: panel.theme.muted
                        }
                        ComboBox {
                            id: fromDay

                            implicitWidth: 120
                            model: [1, 2, 3, 4, 5, 6, 7].map(d => panel.sioul.text("weekday-" + d))
                            currentIndex: officeTimes.parts ? Math.max(0, officeTimes.days.indexOf(officeTimes.parts[1])) : 0
                            onActivated: officeTimes.save()
                        }
                        Label {
                            height: fromDay.height
                            verticalAlignment: Text.AlignVCenter
                            text: panel.sioul.text("task-office-to")
                            color: panel.theme.muted
                        }
                        ComboBox {
                            id: toDay

                            implicitWidth: 120
                            model: fromDay.model
                            currentIndex: officeTimes.parts ? Math.max(0, officeTimes.days.indexOf(officeTimes.parts[2] || officeTimes.parts[1])) : 4
                            onActivated: officeTimes.save()
                        }
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        TextField {
                            id: opens

                            implicitWidth: 64
                            text: officeTimes.parts ? officeTimes.parts[3] : "09:00"
                            validator: RegularExpressionValidator { regularExpression: /^\d{0,2}([:h]\d{0,2})?$/ }
                            onEditingFinished: officeTimes.save()
                        }
                        Label {
                            height: opens.height
                            verticalAlignment: Text.AlignVCenter
                            text: "–"
                            color: panel.theme.muted
                        }
                        TextField {
                            id: closes

                            implicitWidth: 64
                            text: officeTimes.parts ? officeTimes.parts[4] : "17:00"
                            validator: opens.validator
                            onEditingFinished: officeTimes.save()
                        }
                        CheckBox {
                            id: lunch

                            text: panel.sioul.text("task-office-and")
                            checked: officeTimes.parts !== null && officeTimes.parts[5] !== undefined
                            onToggled: officeTimes.save()
                        }
                        TextField {
                            id: opensAgain

                            visible: lunch.checked
                            implicitWidth: 64
                            text: officeTimes.parts && officeTimes.parts[5] ? officeTimes.parts[5] : "14:00"
                            validator: opens.validator
                            onEditingFinished: officeTimes.save()
                        }
                        Label {
                            visible: lunch.checked
                            height: opens.height
                            verticalAlignment: Text.AlignVCenter
                            text: "–"
                            color: panel.theme.muted
                        }
                        TextField {
                            id: closesAgain

                            visible: lunch.checked
                            implicitWidth: 64
                            text: officeTimes.parts && officeTimes.parts[6] ? officeTimes.parts[6] : "17:00"
                            validator: opens.validator
                            onEditingFinished: officeTimes.save()
                        }
                    }
                    // Hours set otherwise (several ranges of days): shown as they are, kept until changed here.
                    Label {
                        Layout.fillWidth: true
                        text: officeTimes.stored === "" ? panel.sioul.text("task-office-usual") : officeTimes.parts ? "" : officeTimes.stored
                        textFormat: Text.PlainText
                        visible: text !== ""
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: panel.theme.muted
                    }
                }
                Label {
                    text: panel.sioul.text("task-field-repeat")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("repeat") ? 1 : 0.45
                }
                ComboBox {
                    readonly property var choices: ["", "daily", "weekly", "monthly", "yearly"]

                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("repeat")
                    opacity: panel.keeps("repeat") ? 1 : 0.45
                    model: choices.map(c => panel.sioul.text("repeat-" + (c === "" ? "none" : c)))
                    currentIndex: panel.detail ? Math.max(0, choices.indexOf(panel.detail.edit.repeat)) : 0
                    onActivated: index => panel.change("repeat", choices[index])
                }
                Label {
                    text: panel.sioul.text("task-field-list")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                }
                // Another list moves it there, its steps with it; a calendar that keeps
                // no task is shown greyed, with why.
                ComboBox {
                    id: listChoice

                    readonly property var choices: panel.detail ? panel.detail.lists : []

                    Layout.fillWidth: true
                    enabled: panel.canEdit
                    model: choices.map(l => panel.theme.plain(l.tasks ? l.name : l.name + "  ·  " + panel.sioul.text("task-list-greyed")))
                    currentIndex: panel.detail ? Math.max(0, choices.findIndex(l => l.id === panel.detail.list)) : 0
                    onActivated: index => {
                        const list = listChoice.choices[index]
                        if (!list.tasks || list.id === panel.detail.list) {
                            listChoice.currentIndex = Math.max(0, listChoice.choices.findIndex(l => l.id === panel.detail.list))
                            return
                        }
                        panel.moveTo(list, false)
                    }

                    delegate: ItemDelegate {
                        required property var modelData
                        required property int index

                        width: listChoice.width
                        text: modelData.replace(/&/g, "&&")
                        enabled: listChoice.choices[index] ? listChoice.choices[index].tasks : true
                        highlighted: listChoice.highlightedIndex === index
                    }
                }
            }
            TextArea {
                id: taskNotes

                // A field shows where it is: a border, darker when it has the focus.
                background: Rectangle {
                    color: panel.theme.surface
                    radius: panel.theme.radius
                    border.color: taskNotes.activeFocus ? panel.theme.focus : panel.theme.line
                }
                visible: panel.moreShown && panel.detail !== null
                Layout.fillWidth: true
                Layout.preferredHeight: Math.max(90, implicitHeight)
                enabled: panel.canEdit
                text: panel.detail ? panel.detail.edit.notes : ""
                placeholderText: panel.sioul.text("task-field-notes")
                wrapMode: TextArea.Wrap
                font.family: panel.theme.readingFamily || font.family
                font.pixelSize: panel.theme.readingSize

                TextSpacing {
                    document: taskNotes.textDocument
                    spacing: panel.theme.readingSpacing
                }
                onEditingFinished: {
                    if (panel.detail && text !== panel.detail.edit.notes)
                        panel.change("notes", text)
                }
            }

            RelatedList {
                Layout.fillWidth: true
                Layout.topMargin: 6
                sioul: panel.sioul
                theme: panel.theme
                title: panel.sioul.text("related-title")
                uri: panel.uid !== "" ? "sioul:task/" + encodeURIComponent(panel.uid) : ""
                // Steps and waits show above, in their own place.
                leaveOut: [panel.sioul.text("how-step"), panel.sioul.text("how-part-of"), panel.sioul.text("how-waits-for"), panel.sioul.text("how-unblocks")]
                onOpenThing: item => panel.window.openThing(item)
            }

            Repeater {
                model: panel.moreShown && panel.detail ? panel.detail.sessions : []

                delegate: Label {
                    required property string modelData

                    text: modelData
                    textFormat: Text.PlainText
                    font.pixelSize: 12
                    color: panel.theme.muted
                }
            }

            // Something new tied to it, or a tie to something that exists.
            Flow {
                Layout.fillWidth: true
                Layout.topMargin: 6
                spacing: 6

                ThingActions {
                    sioul: panel.sioul
                    theme: panel.theme
                    window: panel.window
                    source: panel.uid !== "" && panel.card ? { uri: "sioul:task/" + encodeURIComponent(panel.uid), kind: "task", key: panel.uid, title: panel.card.title } : null
                }
                Button {
                    flat: true
                    visible: panel.canEdit
                    text: panel.sioul.text("ui-delete")
                    onClicked: {
                        panel.sioul.deleteTask(panel.uid)
                        panel.closed()
                    }
                }
                Button {
                    text: panel.sioul.text("ui-close")
                    onClicked: panel.closed()
                }
            }
        }
    }

    ConfirmDialog {
        id: moveAsk

        property var list: null

        sioul: panel.sioul
        theme: panel.theme
        onConfirmed: panel.moveTo(moveAsk.list, true)
    }
}
