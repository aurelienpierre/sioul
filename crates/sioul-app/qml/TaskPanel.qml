// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One task, open on the right: its details first (its title, what to do
// with it now: start, done, not now; its steps, what it waits for and frees,
// its fields in words, its notes, everything tied to it), then "Edit" turns
// them into its form: every field, its dates, length, costs and gain, project
// and notes. Each change is saved as it is made: only the lines that changed
// are written. A task done asks, quietly, "How was it?".
//
// A new task opens here too, every field unfolded, its title first
// (`startNew`): the fields are kept in the form until the title is given
// (Enter, or leaving the field: another field, Close, Escape, Back); the task
// is made then, with all of them, and the panel goes on as any task's. Left
// with no title, nothing is made.

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
    // The projects a task can belong to: [{id, title}].
    property var projects: []
    // The kinds of task, as you named them: {id, label}.
    property var kinds: []
    // Every tag in use, to choose one again.
    property var categories: []
    // A new task's form, not made yet: its title to give first.
    property bool making: false
    // A task's form, its fields open to change ("Edit" on its details).
    property bool editing: false
    // "How was it?" unfolded, on a task done.
    property bool feltShown: false
    // The task just made from the form: its form stays until it is read back.
    property string madeUid: ""
    // Fields changed after the making, before the reading: saved once it is read.
    property bool unsaved: false
    readonly property var card: panel.detail ? panel.detail.card : null
    // The form of a task not read yet: new, or just made (`newTaskForm`, no card).
    readonly property bool fresh: panel.detail !== null && !panel.detail.card
    readonly property bool canEdit: panel.fresh || (panel.card !== null && !panel.card.read_only)
    // The form shows (a new task, one just made, or "Edit"); else its details.
    readonly property bool form: panel.making || panel.fresh || panel.editing
    readonly property bool done: panel.card !== null && (panel.card.status === "completed" || panel.card.status === "cancelled")
    // The four costs; the gain apart.
    readonly property var costs: ["cognitive", "emotional", "anxiety", "body"]
    // What its list does not keep (Google Tasks): greyed, never hidden.
    readonly property var limited: panel.detail && panel.detail.limited ? panel.detail.limited : []
    // The gap the next wait added takes, in minutes (the form's "Then wait"): 0, none.
    readonly property int nextGap: gapCount.value * (gapUnit.currentIndex === 1 ? 7 : 1) * 1440
    // Its natural width, which the page gives it beside the list (TasksPage.qml):
    // its fields at about 70 characters of the body font, a readable line, or
    // its tiles side by side, whichever is wider; no cap of its own
    // (docs/qt-quick.md, "An editing card's width").
    readonly property real naturalWidth: Math.ceil(Math.max(70 * body.averageCharacterWidth, ratingsTop.naturalWidth)) + panel.leftPadding + panel.rightPadding + 12
    // The costs' tiles (for the window's tests and pictures).
    readonly property alias costTiles: ratingsTop

    function keeps(field) {
        return panel.limited.indexOf(field) < 0
    }

    signal closed
    signal focusRequested(string uid, int minutes)
    // A new task made from the form, once its title was given.
    signal made(string uid)

    function reload() {
        if (panel.uid === "") {
            if (!panel.making)
                panel.detail = null
            return
        }
        const text = panel.sioul.task(panel.uid)
        if (text === "")
            return
        const read = JSON.parse(text)
        // Changed while it was being read: saved over what was read, and shown as changed.
        if (panel.unsaved && panel.fresh) {
            panel.unsaved = false
            const answer = JSON.parse(panel.sioul.saveTask(panel.uid, JSON.stringify(panel.detail.edit), ""))
            if (answer.error)
                error.text = answer.error
            else
                read.edit = panel.detail.edit
        }
        panel.detail = read
    }

    // A new task's form: blank, or what Add ▾ makes from `source` (a card's
    // {uri, key, start}: its title, its tie). Returns what went wrong, else "".
    function startNew(source) {
        const blank = JSON.parse(panel.sioul.newTaskForm(source ? source.uri || "" : "", source ? source.key || "" : "", source ? source.start || 0 : 0, ""))
        if (blank.error)
            return blank.error
        if (panel.uid !== "")
            panel.uid = ""
        panel.madeUid = ""
        panel.unsaved = false
        panel.making = true
        panel.detail = blank
        panel.moreShown = true
        panel.rebind()
        error.text = ""
        scroll.contentItem.contentY = 0
        title.forceActiveFocus()
        title.selectAll()
        return ""
    }

    // The title given: the task made from the form, in the list chosen. The
    // panel then goes on as any task's (the page opens it: `made`).
    function make() {
        const given = title.text.trim()
        if (!panel.making || given === "" || !panel.detail)
            return
        const edit = Object.assign({}, panel.detail.edit, { title: given })
        const answer = JSON.parse(panel.sioul.saveTask("", JSON.stringify(edit), panel.detail.list || ""))
        if (answer.error) {
            error.text = answer.error
            return
        }
        error.text = ""
        panel.making = false
        panel.editing = true
        panel.madeUid = answer.uid
        panel.detail = Object.assign({}, panel.detail, { edit: edit })
        panel.made(answer.uid)
    }

    // The form left (closed, another task opened): a title typed makes the
    // task, as leaving its field does; the panel does not go on with it.
    function leave() {
        if (panel.making && title.text.trim() !== "") {
            panel.make()
            panel.madeUid = ""
        }
        panel.making = false
    }

    // For the window's tests: the title typed (or the one given kept), then confirmed.
    function confirmTitle(text) {
        if (text) {
            title.clear()
            title.insert(0, text)
        }
        panel.make()
    }

    // For the window's pictures: the costs and the gain in view.
    function showRatings() {
        panel.moreShown = true
        const top = ratingsTop.mapToItem(column, 0, 0).y
        scroll.contentItem.contentY = Math.max(0, Math.min(top - 12, column.height - scroll.height))
    }

    // For the window's tests: a rating given as by hand.
    function rate(name, value) {
        panel.changeRating(name, value)
    }

    // For the window's tests: "Do at…" opened; then a time given there, as by hand
    // ("2026-10-06T16:00", minutes): it says what went wrong, else "".
    function askDoAt() {
        doAt.now().ask()
    }

    function pinAt(at, minutes) {
        doAt.close()
        return panel.sioul.pinTask(panel.uid, at, minutes)
    }

    // The fields whose own editing undoes their binding, bound again to the task shown.
    function rebind() {
        // A tag being chosen belongs to the task it was chosen for.
        newTag.currentIndex = -1
        newTag.editText = ""
        title.text = Qt.binding(() => panel.detail ? panel.detail.edit.title : "")
        startField.date = Qt.binding(() => panel.detail ? panel.detail.edit.start.slice(0, 10) : "")
        dueField.date = Qt.binding(() => panel.detail ? panel.detail.edit.due.slice(0, 10) : "")
    }

    // The form's own copy changed: a task not made yet, or not read yet. What
    // is typed in the title stays typed.
    function hold(edit, more) {
        if (panel.making)
            edit.title = title.text
        panel.detail = Object.assign({}, panel.detail, { edit: edit }, more || {})
        if (panel.uid !== "")
            panel.unsaved = true
    }

    // A new task's list, chosen before it is made: what that list keeps, greyed or not.
    function chooseList(id) {
        const blank = JSON.parse(panel.sioul.newTaskForm("", "", 0, id))
        if (blank.error)
            return
        panel.hold(Object.assign({}, panel.detail.edit), { list: blank.list, limited: blank.limited })
    }

    // What it waits for, in the form of a task not read yet; with a gap in
    // minutes, written in the task waited for once this one is (work.rs, `write_gaps`).
    function waitFor(other, otherTitle, wait, gap) {
        const edit = Object.assign({}, panel.detail.edit)
        edit.waits_for = edit.waits_for.filter(u => u !== other).concat(wait ? [other] : [])
        const gaps = Object.assign({}, edit.wait_gaps || {})
        delete gaps[other]
        if (wait && gap > 0)
            gaps[other] = gap
        edit.wait_gaps = gaps
        const shown = panel.detail.waits_for.filter(w => w[0] !== other).concat(wait ? [[other, otherTitle, panel.gapWords(wait ? gap : 0)]] : [])
        panel.hold(edit, { waits_for: shown })
    }

    // A wait's gap in words, as the details say it (taskview.rs, `gap_words`): "two weeks after it".
    function gapWords(minutes) {
        if (!(minutes > 0))
            return ""
        if (minutes % (7 * 1440) === 0)
            return panel.sioul.textCounted("task-waits-after-weeks", minutes / (7 * 1440))
        if (minutes % 1440 === 0)
            return panel.sioul.textCounted("task-waits-after-days", minutes / 1440)
        return panel.sioul.textCounted("task-waits-after-hours", Math.ceil(minutes / 60))
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

    // Saves the form with one field changed; a task not made or not read yet keeps it in its form.
    function change(field, value) {
        if (!panel.detail || !panel.canEdit)
            return
        const edit = Object.assign({}, panel.detail.edit)
        edit[field] = value
        if (panel.fresh) {
            panel.hold(edit)
            return
        }
        const answer = JSON.parse(panel.sioul.saveTask(panel.uid, JSON.stringify(edit), ""))
        if (answer.error) {
            error.text = answer.error
            return
        }
        error.text = ""
        // Kept as saved until it is read again: a second change at once (two
        // ratings given in a breath) goes on from this one, not from before it.
        panel.detail = Object.assign({}, panel.detail, { edit: edit })
    }

    readonly property var marginMinutes: [0, 5, 10, 15, 20, 30, 45, 60, 90, 120]

    // The time kept before or after it, saved.
    function changeMargin(side, minutes) {
        const margins = Object.assign({ before: 0, after: 0 }, panel.detail.edit.margins)
        margins[side] = minutes
        panel.change("margins", margins)
    }

    // A cost or the gain, 0 to 10; null when unsaid.
    function rating(name) {
        const value = panel.detail && panel.detail.edit.demands ? panel.detail.edit.demands[name] : null
        return value === undefined ? null : value
    }

    function changeRating(name, value) {
        const demands = Object.assign({ cognitive: null, emotional: null, anxiety: null, body: null, gain: null }, panel.detail.edit.demands)
        demands[name] = value
        panel.change("demands", demands)
    }

    // Several at once ("Looks right": what was felt before, for what was unsaid), in one save.
    function changeRatings(values) {
        const demands = Object.assign({ cognitive: null, emotional: null, anxiety: null, body: null, gain: null }, panel.detail.edit.demands, values)
        panel.change("demands", demands)
    }

    // What it takes, from its ratings once a cost is said (the plan's rule,
    // docs/tasks.md): light up to 3, the usual from 4 to 6, heavy from 7; it
    // gives back with a gain of 5 or more and no cost above 3. "" when no cost
    // is said: then it is chosen.
    function level() {
        const said = panel.costs.map(n => panel.rating(n)).filter(v => v !== null)
        if (said.length === 0)
            return ""
        const top = Math.max(...said)
        const gain = panel.rating("gain")
        if (gain !== null && gain >= 5 && top <= 3)
            return "rest"
        return top <= 3 ? "light" : top <= 6 ? "usual" : "heavy"
    }

    // A day as words: "Thursday 8 October".
    function dayText(text) {
        const day = new Date(text.slice(0, 10) + "T12:00:00")
        return isNaN(day.getTime()) ? text : day.toLocaleDateString(panel.window.sioulLocale, "dddd d MMMM")
    }

    // Its fields in words, for its details: only what is said; the date asked,
    // the length and what it waits for are said above, as the plan words them.
    function facts() {
        if (!panel.detail || !panel.detail.edit)
            return []
        const e = panel.detail.edit
        const rows = []
        const add = (key, value) => {
            if (value !== "" && value !== null && value !== undefined)
                rows.push({ label: panel.sioul.text(key), value: String(value) })
        }
        add("task-field-start", e.start ? panel.dayText(e.start) : "")
        if (e.margins && e.margins.before > 0)
            add("task-field-before", panel.minutesText(e.margins.before))
        if (e.margins && e.margins.after > 0)
            add("task-field-after", panel.minutesText(e.margins.after))
        // Each cost said, as its tile says it: "Worry: 7, very hard".
        for (const name of panel.costs.concat(["gain"])) {
            const value = panel.rating(name)
            if (value !== null)
                rows.push({ label: ratingsTop.title(name), value: value + ", " + ratingsTop.word(name, value) })
        }
        const level = panel.level()
        if (level !== "")
            add("task-field-energy", panel.sioul.textWith("task-energy-computed", "level", panel.sioul.text("task-energy-" + level)))
        else if (e.energy)
            add("task-field-energy", panel.sioul.text("task-energy-" + e.energy))
        const project = e.projects && e.projects.length > 0 ? panel.projects.find(c => c.id === e.projects[0]) : null
        add("task-field-project", project ? project.title : (e.projects && e.projects.length > 0 ? e.projects[0] : ""))
        add("task-field-billable", e.billable === true ? panel.sioul.text("task-billable-yes") : e.billable === false ? panel.sioul.text("task-billable-no") : "")
        const kind = e.kind ? panel.kinds.find(k => k.id === e.kind) : null
        add("task-kind", kind ? kind.label : (e.kind || ""))
        add("task-field-area", e.area ? e.area.split("+").map(a => panel.sioul.text("area-" + a)).join(", ") : "")
        if (e.office_hours)
            add("task-office-hours", e.office_times || panel.sioul.text("task-office-usual-short"))
        add("task-field-repeat", e.repeat ? panel.sioul.text("repeat-" + e.repeat) : "")
        const list = panel.detail.lists ? panel.detail.lists.find(l => l.id === panel.detail.list) : null
        add("task-field-list", list ? list.name : "")
        return rows
    }

    // Today, as the backend dates what is felt: "2026-10-06".
    function today() {
        const d = new Date()
        const pad = n => n < 10 ? "0" + n : String(n)
        return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate())
    }

    // Today's "How was it?" as kept so far; null when none today (an older turn's stays out).
    function feltToday() {
        if (!panel.detail || !panel.detail.edit || !panel.detail.edit.felt)
            return null
        return panel.detail.felt_on === panel.today() ? panel.detail.edit.felt : null
    }

    // How it was, kept as felt (only that, read from the file itself: what
    // else the task says is left as it is on disk).
    function keepFelt(values) {
        const said = panel.sioul.setFelt(panel.uid, JSON.stringify(values))
        if (said !== "") {
            error.text = said
            return
        }
        error.text = ""
        panel.detail = Object.assign({}, panel.detail, { edit: Object.assign({}, panel.detail.edit, { felt: values }), felt_on: panel.today() })
    }

    function minutesText(m) {
        if (m === 0)
            return "—"
        if (m < 60)
            return m + " min"
        return Math.floor(m / 60) + " h" + (m % 60 ? " " + (m % 60 < 10 ? "0" : "") + m % 60 : "")
    }

    function addStep() {
        panel.editing = true
        panel.moreShown = false
        steps.focusLine()
    }

    onUidChanged: {
        // The task just made from the form: the form stays as it is until the task is read.
        if (panel.uid !== "" && panel.uid === panel.madeUid) {
            panel.madeUid = ""
            panel.reload()
            return
        }
        panel.madeUid = ""
        panel.making = false
        panel.editing = false
        panel.feltShown = false
        panel.unsaved = false
        panel.detail = null
        panel.moreShown = false
        panel.reload()
        panel.rebind()
    }

    Connections {
        target: panel.sioul
        function onTasksChanged() {
            panel.reload()
        }
    }

    FontMetrics {
        id: body

        font: panel.font
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                TextField {
                    id: title

                    visible: panel.form
                    // Its own width, not its wrapped text's: the column gives it the rest
                    // (the text's width would follow the width it sets, a binding loop).
                    implicitWidth: 120
                    Layout.fillWidth: true
                    // Its own title as it was given (or kept, just saved); "Untitled" when it has none.
                    text: panel.detail ? panel.detail.edit.title : ""
                    readOnly: !panel.canEdit
                    placeholderText: panel.theme.plain(panel.making ? panel.sioul.text("task-new-title") : panel.card ? panel.card.title : "")
                    font.pixelSize: 20
                    wrapMode: TextInput.Wrap
                    // Like every field, a border: light, darker when it has the focus.
                    background: Rectangle {
                        color: "transparent"
                        radius: panel.theme.radius
                        border.color: title.activeFocus ? panel.theme.focus : panel.theme.line
                    }
                    Accessible.name: panel.making ? panel.sioul.text("task-new-title") : panel.sioul.text("task-title")
                    // A new task is made as its title is given; a task's title changed is saved.
                    onEditingFinished: {
                        if (panel.making)
                            panel.make()
                        else if (panel.detail && title.text.trim() !== "" && title.text !== panel.detail.edit.title)
                            panel.change("title", title.text.trim())
                    }
                }
                // Its details: the title, read; its own width, not its text's, as the field's.
                Label {
                    visible: !panel.form
                    Layout.fillWidth: true
                    Layout.preferredWidth: 120
                    text: panel.card ? panel.card.title : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 20
                    color: panel.theme.text
                }
                // Its details or its form, one click apart. A new task is a form.
                Button {
                    visible: panel.canEdit && !panel.fresh
                    Layout.alignment: Qt.AlignTop
                    flat: true
                    text: panel.form ? panel.sioul.text("ui-details") : panel.sioul.text("ui-edit")
                    icon.name: panel.form ? "view-list-details" : "document-edit"
                    icon.color: panel.theme.text
                    onClicked: {
                        panel.editing = !panel.editing
                        if (panel.editing)
                            panel.moreShown = true
                    }
                }
            }
            // Pinned to a time, near its title: a pin, and its block's day and
            // time in words. A click opens the block in the agenda; its menu
            // (right click, a long press, or ⋯) moves it or leaves it to the
            // plan (docs/tasks.md, "Pinned to a time"). Quiet: the accent only.
            RowLayout {
                id: pinned

                readonly property var pin: panel.detail && panel.detail.pin && !panel.done && !panel.making ? panel.detail.pin : null

                visible: pinned.pin !== null
                Layout.fillWidth: true
                spacing: 8

                Icon {
                    Layout.alignment: Qt.AlignVCenter
                    iconName: "pin"
                    size: 16
                    color: panel.theme.accent
                }
                Label {
                    id: pinnedText

                    Layout.fillWidth: true
                    // Its own width, not its text's: it wraps on a phone.
                    Layout.preferredWidth: 120
                    Layout.alignment: Qt.AlignVCenter
                    text: pinned.pin ? panel.sioul.textWith("task-pinned", "when", pinned.pin.when) : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: panel.theme.accent
                    font.underline: pinnedHover.hovered
                    Accessible.role: Accessible.Link
                    Accessible.name: pinnedText.text

                    HoverHandler {
                        id: pinnedHover

                        cursorShape: Qt.PointingHandCursor
                    }
                    // A click, a tap: the block, in the agenda.
                    TapHandler {
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onTapped: panel.window.openThing({ kind: "event", uri: "", key: pinned.pin.key })
                    }
                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        // A touch has no buttons: on a touch screen, the long press below.
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onTapped: pinMenu.now().popup()
                    }
                    TapHandler {
                        id: pinnedHold

                        acceptedDevices: PointerDevice.TouchScreen
                        onTapped: panel.window.openThing({ kind: "event", uri: "", key: pinned.pin.key })
                        onLongPressed: {
                            panel.window.menuAt = pinnedHold.point.scenePosition
                            pinMenu.now().popup()
                        }
                    }
                }
                ToolButton {
                    Layout.alignment: Qt.AlignVCenter
                    text: "⋯"
                    Accessible.name: panel.sioul.text("task-pinned-menu")
                    ToolTip.visible: hovered
                    ToolTip.text: panel.sioul.text("task-pinned-menu")
                    ToolTip.delay: 600
                    onClicked: pinMenu.now().popup()
                }
            }
            Label {
                visible: pinned.pin !== null && pinned.pin.read_only
                Layout.fillWidth: true
                text: panel.sioul.text("task-pinned-read-only")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            // A new task: when it is made.
            Label {
                visible: panel.making
                Layout.fillWidth: true
                text: panel.sioul.text("task-new-made-when")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
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
                    onClicked: startMenu.now().popup()

                    Later {
                        id: startMenu

                        sourceComponent: Component {
                            SioulMenu {
                                id: startMenuForm

                                Repeater {
                                    model: [2, 15, 25, 45, 0]

                                    delegate: MenuItem {
                                        required property int modelData

                                        text: panel.theme.plain(modelData === 2 ? panel.sioul.text("focus-two") : modelData === 0 ? panel.sioul.text("focus-open-ended") : panel.sioul.textWith("focus-for", "minutes", String(modelData)))
                                        onTriggered: panel.focusRequested(panel.uid, modelData)
                                    }
                                }
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
                // Pinned to a time: a block in your calendar, where the plan lays it (docs/tasks.md, "Pinned to a time").
                Button {
                    visible: panel.detail !== null && !panel.detail.pin
                    flat: true
                    text: panel.sioul.text("task-do-at")
                    enabled: panel.canEdit
                    onClicked: doAt.now().ask()
                }
                // Not to be done after all: kept, struck out, out of the plan; "Open again" brings it back.
                Button {
                    flat: true
                    text: panel.sioul.text("task-drop")
                    enabled: panel.canEdit
                    ToolTip.visible: hovered
                    ToolTip.text: panel.sioul.text("task-drop-help")
                    ToolTip.delay: 600
                    onClicked: panel.sioul.setTaskStatus(panel.uid, "cancelled")
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
            // How long things like it take you, against the first guess: on request only.
            Button {
                id: ratioAsk

                property bool shown: false

                visible: !panel.form && panel.detail !== null && (panel.detail.ratio_line || "") !== ""
                flat: true
                text: (ratioAsk.shown ? "▾  " : "▸  ") + panel.sioul.text("task-ratio-ask")
                onClicked: ratioAsk.shown = !ratioAsk.shown
            }
            Label {
                visible: ratioAsk.visible && ratioAsk.shown
                Layout.fillWidth: true
                text: panel.detail ? (panel.detail.ratio_line || "") : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.theme.muted
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
                visible: panel.detail !== null && ((panel.form && panel.canEdit) || panel.detail.edit.categories.length > 0)
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
                                visible: panel.form && panel.canEdit && panel.keeps("categories")
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

                    visible: panel.form && panel.canEdit && panel.keeps("categories")
                    width: 170
                    editable: true
                    model: newTag.others.map(c => panel.theme.plain(c))
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
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.theme.warm
            }

            // Its steps, and one more.
            Label {
                visible: panel.form || (panel.detail !== null && panel.detail.steps.length > 0)
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
            // A step needs its task: on a new one, once its title is typed (going
            // there gives the title, and makes the task).
            CaptureField {
                id: steps

                Layout.fillWidth: true
                visible: panel.form && panel.canEdit
                enabled: panel.keeps("steps") && (panel.uid !== "" || (panel.making && title.text.trim() !== ""))
                opacity: panel.keeps("steps") ? 1 : 0.45
                sioul: panel.sioul
                theme: panel.theme
                parentUid: panel.uid
                step: true
                placeholder: panel.keeps("steps") ? panel.sioul.text("task-add-step") : panel.sioul.text("google-tasks-greyed")
            }

            // What it waits for: removable; and one more, found by its title.
            Label {
                visible: panel.form || (panel.detail !== null && panel.detail.waits_for.length > 0)
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
                        // Its gap beside it: "Send the letter · two weeks after it".
                        text: wait.modelData[2] ? wait.modelData[1] + " · " + wait.modelData[2] : wait.modelData[1]
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: panel.theme.text

                        TapHandler {
                            onTapped: panel.uid = wait.modelData[0]
                        }
                    }
                    ToolButton {
                        visible: panel.form && panel.canEdit
                        text: "×"
                        Accessible.name: panel.sioul.text("task-waits-remove")
                        ToolTip.visible: hovered
                        ToolTip.text: panel.sioul.text("task-waits-remove")
                        onClicked: {
                            if (panel.fresh)
                                panel.waitFor(wait.modelData[0], wait.modelData[1], false, 0)
                            else
                                panel.sioul.setWaits(panel.uid, wait.modelData[0], false, 0)
                        }
                    }
                }
            }
            TextField {
                id: waitsFor

                // The tasks found by their title, offered below.
                property var matches: []

                visible: panel.form && panel.canEdit
                enabled: panel.keeps("waits")
                opacity: panel.keeps("waits") ? 1 : 0.45
                Layout.fillWidth: true
                placeholderText: panel.keeps("waits") ? panel.sioul.text("task-waits-add") : panel.sioul.text("google-tasks-greyed")
                onTextEdited: {
                    waitsFor.matches = waitsFor.text.trim() === "" ? [] : JSON.parse(panel.sioul.searchTasks(waitsFor.text, panel.uid))
                    if (waitsFor.matches.length > 0)
                        foundPopup.now().open()
                }
                Keys.onEscapePressed: {
                    waitsFor.clear()
                    foundPopup.close()
                }

                Later {
                    id: foundPopup

                    sourceComponent: Component {
                        Popup {
                            id: foundPopupForm

                            y: waitsFor.height
                            width: waitsFor.width
                            padding: 4

                            ListView {
                                id: found

                                implicitHeight: Math.min(contentHeight, 240)
                                width: parent.width
                                clip: true
                                model: waitsFor.matches

                                delegate: ItemDelegate {
                                    id: option

                                    required property var modelData

                                    width: found.width
                                    // "&" doubled: a button reads one as a key to underline.
                                    text: panel.theme.plain(option.modelData.title).replace(/&/g, "&&")
                                    onClicked: {
                                        if (panel.fresh)
                                            panel.waitFor(option.modelData.uid, option.modelData.title, true, panel.nextGap)
                                        else
                                            error.text = panel.sioul.setWaits(panel.uid, option.modelData.uid, true, panel.nextGap)
                                        // A gap is for the wait it was given to.
                                        gapCount.value = 0
                                        waitsFor.clear()
                                        foundPopupForm.close()
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // The gap the next wait added takes ("the answer comes within two
            // weeks"), as `sioul tasks import` writes it: in days or weeks; 0, none.
            RowLayout {
                visible: panel.form && panel.canEdit
                enabled: panel.keeps("waits")
                opacity: panel.keeps("waits") ? 1 : 0.45
                Layout.fillWidth: true
                spacing: 6

                Label {
                    text: panel.sioul.text("task-waits-gap")
                    color: panel.theme.text
                }
                SpinBox {
                    id: gapCount

                    from: 0
                    to: 99
                    editable: true
                    Accessible.name: panel.sioul.text("task-waits-gap")
                }
                ComboBox {
                    id: gapUnit

                    Layout.fillWidth: true
                    model: [panel.sioul.textCounted("task-waits-gap-days", gapCount.value), panel.sioul.textCounted("task-waits-gap-weeks", gapCount.value)]
                    Accessible.name: panel.sioul.text("task-waits-gap")
                }
            }
            Label {
                visible: panel.form && panel.canEdit && panel.keeps("waits")
                Layout.fillWidth: true
                text: panel.sioul.text("task-waits-gap-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
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

            // Its details: its fields in words, what is said only; its notes.
            GridLayout {
                visible: !panel.form && facts.count > 0
                Layout.fillWidth: true
                Layout.topMargin: 6
                columns: 2
                columnSpacing: 10
                rowSpacing: 4

                Repeater {
                    id: facts

                    model: panel.form ? [] : panel.facts()

                    delegate: Label {
                        id: fact

                        required property var modelData
                        required property int index

                        // A label, then its value: two cells a row.
                        Layout.row: fact.index
                        Layout.column: 0
                        Layout.maximumWidth: 150
                        Layout.alignment: Qt.AlignTop
                        text: fact.modelData.label
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: panel.theme.muted
                    }
                }
                Repeater {
                    model: panel.form ? [] : panel.facts()

                    delegate: Label {
                        id: value

                        required property var modelData
                        required property int index

                        Layout.row: value.index
                        Layout.column: 1
                        Layout.fillWidth: true
                        // Its own width, not its text's: it wraps on a phone.
                        Layout.preferredWidth: 120
                        text: value.modelData.value
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: panel.theme.text
                    }
                }
            }
            Label {
                visible: !panel.form && panel.detail !== null && panel.detail.edit.notes !== ""
                Layout.fillWidth: true
                Layout.topMargin: 6
                // As written: a list shared with other applications may hold anything.
                text: panel.detail ? panel.detail.edit.notes : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.family: panel.theme.readingFamily || font.family
                font.pixelSize: panel.theme.readingSize
                color: panel.theme.text
            }

            // Done: how it was, if you want to say. Filled in with what was
            // foreseen; nothing is kept until one changes, or "As expected".
            Button {
                visible: panel.done && !panel.form && panel.canEdit
                flat: true
                text: (panel.feltShown ? "▾  " : "▸  ") + panel.sioul.text("felt-ask")
                onClicked: panel.feltShown = !panel.feltShown
            }
            FeltRatings {
                visible: panel.feltShown && panel.done && !panel.form
                Layout.fillWidth: true
                sioul: panel.sioul
                theme: panel.theme
                forecast: panel.detail && panel.detail.edit.demands ? panel.detail.edit.demands : ({})
                kept: panel.feltToday()
                onGiven: values => panel.keepFelt(values)
            }

            // The rest, folded, in the form.
            Button {
                visible: panel.form
                flat: true
                text: (panel.moreShown ? "▾  " : "▸  ") + panel.sioul.text("ui-more-details")
                onClicked: panel.moreShown = !panel.moreShown
            }
            Label {
                visible: panel.form && panel.moreShown && panel.limited.length > 0
                Layout.fillWidth: true
                text: panel.sioul.text("task-limited")
                wrapMode: Text.Wrap
                color: panel.theme.muted
            }
            GridLayout {
                visible: panel.form && panel.moreShown && panel.detail !== null
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
                    id: startField

                    theme: panel.theme
                    sioul: panel.sioul
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
                    id: dueField

                    theme: panel.theme
                    sioul: panel.sioul
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
                    model: minutes.map(m => panel.theme.plain(panel.minutesText(m)))
                    currentIndex: panel.detail ? Math.max(0, minutes.indexOf(panel.detail.edit.estimate)) : 0
                    onActivated: index => panel.change("estimate", minutes[index])
                }
                // Getting there and back, getting ready: kept free around it, never a pause.
                Label {
                    text: panel.sioul.text("task-field-before")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("margins") ? 1 : 0.45
                }
                ComboBox {
                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("margins")
                    opacity: panel.keeps("margins") ? 1 : 0.45
                    model: panel.marginMinutes.map(m => m === 0 ? panel.sioul.text("task-rating-unsaid") : panel.theme.plain(panel.minutesText(m)))
                    currentIndex: panel.detail && panel.detail.edit.margins ? Math.max(0, panel.marginMinutes.indexOf(panel.detail.edit.margins.before)) : 0
                    onActivated: index => panel.changeMargin("before", panel.marginMinutes[index])
                }
                Label {
                    text: panel.sioul.text("task-field-after")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("margins") ? 1 : 0.45
                }
                ComboBox {
                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("margins")
                    opacity: panel.keeps("margins") ? 1 : 0.45
                    model: panel.marginMinutes.map(m => m === 0 ? panel.sioul.text("task-rating-unsaid") : panel.theme.plain(panel.minutesText(m)))
                    currentIndex: panel.detail && panel.detail.edit.margins ? Math.max(0, panel.marginMinutes.indexOf(panel.detail.edit.margins.after)) : 0
                    onActivated: index => panel.changeMargin("after", panel.marginMinutes[index])
                }
                // What it costs, and what it gives back: four tiles and a row, 0 to 10
                // each, unsaid until said (CostTiles.qml); each value saved as it is
                // given, "Looks right" saving the faint ones at once.
                CostTiles {
                    id: ratingsTop

                    Layout.columnSpan: 2
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    Layout.bottomMargin: 4
                    sioul: panel.sioul
                    theme: panel.theme
                    enabled: panel.canEdit && panel.keeps("costs")
                    opacity: panel.keeps("costs") ? 1 : 0.45
                    values: panel.detail && panel.detail.edit.demands ? panel.detail.edit.demands : ({})
                    proposed: panel.detail && panel.detail.proposed ? panel.detail.proposed.values : ({})
                    proposedFrom: panel.detail && panel.detail.proposed ? panel.detail.proposed.from : ""
                    onEdited: (name, value) => panel.changeRating(name, value)
                    onTaken: values => panel.changeRatings(values)
                }
                Label {
                    text: panel.sioul.text("task-field-project")
                    color: panel.theme.muted
                    Layout.maximumWidth: 120
                    wrapMode: Text.Wrap
                    opacity: panel.keeps("project") ? 1 : 0.45
                }
                ComboBox {
                    readonly property var choices: [{ id: "", title: "—" }].concat(panel.projects)

                    Layout.fillWidth: true
                    enabled: panel.canEdit && panel.keeps("project")
                    opacity: panel.keeps("project") ? 1 : 0.45
                    model: choices.map(c => panel.theme.plain(c.title))
                    currentIndex: panel.detail ? Math.max(0, choices.findIndex(c => c.id === (panel.detail.edit.projects[0] || ""))) : 0
                    onActivated: index => panel.change("projects", choices[index].id === "" ? [] : [choices[index].id])
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
                // Once a cost is said, it follows the ratings, shown, not chosen.
                Label {
                    visible: panel.level() !== ""
                    Layout.fillWidth: true
                    text: panel.level() !== "" ? panel.sioul.textWith("task-energy-computed", "level", panel.sioul.text("task-energy-" + panel.level())) : ""
                    wrapMode: Text.Wrap
                    color: panel.theme.text
                    opacity: panel.keeps("energy") ? 1 : 0.45
                }
                ComboBox {
                    readonly property var choices: ["", "light", "heavy", "rest"]

                    visible: panel.level() === ""
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
                            model: [1, 2, 3, 4, 5, 6, 7].map(d => panel.sioul.text("weekday-" + d))
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
                    // A task just made moves once it is read (a moment).
                    enabled: panel.canEdit && (panel.making || !panel.fresh)
                    model: choices.map(l => panel.theme.plain(l.tasks ? l.name : l.name + "  ·  " + panel.sioul.text("task-list-greyed")))
                    currentIndex: panel.detail ? Math.max(0, choices.findIndex(l => l.id === panel.detail.list)) : 0
                    onActivated: index => {
                        const list = listChoice.choices[index]
                        if (!list.tasks || list.id === panel.detail.list) {
                            listChoice.currentIndex = Math.max(0, listChoice.choices.findIndex(l => l.id === panel.detail.list))
                            return
                        }
                        // A new task is made there, once it has its title.
                        if (panel.making)
                            panel.chooseList(list.id)
                        else
                            panel.moveTo(list, false)
                    }

                    delegate: ItemDelegate {
                        required property var modelData
                        required property int index

                        width: listChoice.width
                        text: panel.theme.plain(modelData.replace(/&/g, "&&"))
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
                visible: panel.form && panel.moreShown && panel.detail !== null
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

            // A new task from a card (Add ▾): what it will be tied to, until it is read.
            Label {
                visible: panel.fresh && (panel.detail.source || "") !== ""
                Layout.fillWidth: true
                Layout.topMargin: 6
                text: visible ? panel.sioul.textWith("task-new-tied", "title", panel.detail.source) : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.theme.muted
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
                model: (!panel.form || panel.moreShown) && panel.detail ? panel.detail.sessions : []

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
                    visible: !panel.making
                    sioul: panel.sioul
                    theme: panel.theme
                    window: panel.window
                    source: panel.uid !== "" && panel.card ? { uri: "sioul:task/" + encodeURIComponent(panel.uid), kind: "task", key: panel.uid, title: panel.card.title } : null
                }
                // Nothing to delete before it is made, and read.
                Button {
                    flat: true
                    visible: panel.canEdit && !panel.fresh
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

    // A pinned task's menu, made the first time: its block in the agenda,
    // moved, or left to the plan.
    Later {
        id: pinMenu

        sourceComponent: Component {
            SioulMenu {
                MenuItem {
                    text: panel.sioul.text("task-pinned-open")
                    onTriggered: panel.window.openThing({ kind: "event", uri: "", key: pinned.pin ? pinned.pin.key : "" })
                }
                MenuItem {
                    enabled: pinned.pin !== null && !pinned.pin.read_only && panel.canEdit
                    text: panel.sioul.text("task-pinned-move")
                    onTriggered: doAt.now().ask()
                }
                MenuItem {
                    enabled: pinned.pin !== null && !pinned.pin.read_only && panel.canEdit
                    text: panel.sioul.text("day-let-plan")
                    onTriggered: error.text = panel.sioul.setTaskAt(panel.uid, "")
                }
            }
        }
    }

    // "Do at…": its day, its time and how long, made the first time it is
    // asked for. The task's block, an event, is made there or moved there;
    // "Undo" waits in the status line (docs/tasks.md, "Pinned to a time").
    Later {
        id: doAt

        sourceComponent: Component {
            Dialog {
                id: doAtForm

                readonly property var lengths: [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240]
                // The lengths offered: the usual ones, and the block's own.
                property var shown: doAtForm.lengths
                property string problem: ""
                readonly property bool ready: /^([01][0-9]|2[0-3]):[0-5][0-9]$/.test(timeField.text) && dayField.date.length === 10

                function pad(n) {
                    return n < 10 ? "0" + n : String(n)
                }

                // On its block when it has one; else the next quarter of an hour
                // (tomorrow at 9:00 late in the evening), as long as the plan lays it.
                function ask() {
                    const pin = panel.detail ? panel.detail.pin : null
                    let day = ""
                    let time = ""
                    let length = 30
                    if (pin) {
                        day = pin.at.slice(0, 10)
                        time = pin.at.slice(11, 16)
                        length = pin.minutes
                    } else {
                        const d = new Date()
                        d.setMinutes(Math.ceil((d.getMinutes() + 1) / 15) * 15, 0, 0)
                        if (d.getHours() >= 22) {
                            d.setDate(d.getDate() + 1)
                            d.setHours(9, 0, 0, 0)
                        }
                        day = d.getFullYear() + "-" + doAtForm.pad(d.getMonth() + 1) + "-" + doAtForm.pad(d.getDate())
                        time = doAtForm.pad(d.getHours()) + ":" + doAtForm.pad(d.getMinutes())
                        length = panel.detail && panel.detail.pin_minutes > 0 ? panel.detail.pin_minutes : 30
                    }
                    doAtForm.shown = doAtForm.lengths.indexOf(length) < 0 ? doAtForm.lengths.concat([length]).sort((a, b) => a - b) : doAtForm.lengths
                    dayField.date = day
                    timeField.text = time
                    lengthField.currentIndex = doAtForm.shown.indexOf(length)
                    doAtForm.problem = ""
                    doAtForm.open()
                }

                function save() {
                    if (!doAtForm.ready)
                        return
                    const problem = panel.sioul.pinTask(panel.uid, dayField.date + "T" + timeField.text, doAtForm.shown[Math.max(0, lengthField.currentIndex)])
                    if (problem !== "") {
                        doAtForm.problem = problem
                        return
                    }
                    doAtForm.close()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? parent.width : 440) - 2 * panel.theme.gap)
                title: panel.sioul.text("task-do-at")

                ColumnLayout {
                    width: parent.width
                    spacing: 8

                    Label {
                        Layout.fillWidth: true
                        text: panel.sioul.text("task-do-at-help")
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: panel.theme.muted
                    }
                    GridLayout {
                        Layout.fillWidth: true
                        columns: 2
                        columnSpacing: 10
                        rowSpacing: 6

                        Label {
                            text: panel.sioul.text("task-do-at-day")
                            color: panel.theme.muted
                        }
                        DateField {
                            id: dayField

                            theme: panel.theme
                            sioul: panel.sioul
                            locale: panel.window.sioulLocale
                            pickLabel: panel.sioul.text("event-pick-day")
                        }
                        Label {
                            text: panel.sioul.text("task-do-at-time")
                            color: panel.theme.muted
                        }
                        TextField {
                            id: timeField

                            Layout.preferredWidth: 80
                            inputMask: "99:99"
                            Accessible.name: panel.sioul.text("task-do-at-time")
                            onAccepted: doAtForm.save()
                        }
                        Label {
                            text: panel.sioul.text("task-do-at-length")
                            color: panel.theme.muted
                        }
                        ComboBox {
                            id: lengthField

                            Layout.fillWidth: true
                            model: doAtForm.shown.map(m => panel.theme.plain(panel.minutesText(m)))
                            Accessible.name: panel.sioul.text("task-do-at-length")
                        }
                    }
                    Label {
                        visible: doAtForm.problem !== ""
                        Layout.fillWidth: true
                        text: doAtForm.problem
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: panel.theme.warm
                    }
                    RowLayout {
                        Layout.fillWidth: true

                        Item {
                            Layout.fillWidth: true
                        }
                        Button {
                            text: panel.sioul.text("ui-cancel")
                            onClicked: doAtForm.close()
                        }
                        Button {
                            text: panel.sioul.text("task-do-at-save")
                            highlighted: true
                            enabled: doAtForm.ready
                            onClicked: doAtForm.save()
                        }
                    }
                }
            }
        }
    }
}
