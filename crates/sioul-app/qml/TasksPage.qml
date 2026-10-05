// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Tasks. "Now" first: the one next step, picked by the plan, with why, and
// the one after it; nothing else unless asked. The list, the board and the
// timeline are one switch away. A line at the top takes a new task in one
// sentence. Nothing is overdue: a date asked is time left; a day that passed
// moves the task forward instead (docs/tasks.md).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string tasksText: ""
    readonly property var shown: page.tasksText ? JSON.parse(page.tasksText) : null

    function takeShown() {
        if (page.visible)
            page.tasksText = page.sioul.tasks
    }

    Connections {
        target: page.sioul

        function onTasksChanged() {
            page.takeShown()
        }
    }
    Component.onCompleted: {
        page.takeShown()
        if (!page.rest)
            page.nowMade = true
    }
    // For the window's images: the task open.
    readonly property var panel: panelLoader.item
    // The task's panel, made the first time a task opens (a form of many fields), then kept.
    property bool panelMade: false
    onOpenedChanged: if (page.opened !== "") page.panelMade = true
    property alias routines: routinesDialog
    // "now", "day", "list", "board", "timeline".
    property string mode: "now"
    // The views made so far: each the first time it is shown; "now" not
    // while the rest cover hides it.
    property var made: ({ now: true })
    property bool nowMade: false
    onRestChanged: {
        if (!page.rest)
            page.nowMade = true
    }
    onModeChanged: {
        const made = Object.assign({}, page.made)
        made[page.mode] = true
        page.made = made
    }
    property string opened: ""
    // On a phone, the task open takes the page; Back closes it (main.qml).
    readonly property bool canGoBack: page.opened !== ""
    function back() {
        page.opened = ""
    }
    property bool othersShown: false
    property bool startedShown: false
    property bool doneShown: false
    property bool joyShown: false
    property string by: "case"
    property bool showDone: false
    property string query: ""
    property string caseFilter: ""
    // A narrow screen (a phone): the board's columns one under the other.
    readonly property bool narrow: page.width < 640
    readonly property int halfFlow: Math.floor((viewsFlow.width - viewsFlow.spacing) / 2)
    // One kind, one category; "" for all. Kept between sessions, as the choices above.
    readonly property string kindFilter: page.shown && page.shown.view ? page.shown.view.filter.kind : ""
    readonly property string categoryFilter: page.shown && page.shown.view ? page.shown.view.filter.category : ""
    property bool restored: false
    // Rest (outside every hours set): no tasks, unless you ask; the next visit asks again.
    property bool anyway: false
    readonly property bool rest: page.window.moment.rest === true && !page.anyway

    onVisibleChanged: {
        page.takeShown()
        if (!page.visible && page.anyway) {
            page.anyway = false
            page.sioul.showTasksAnyway(false)
        }
    }

    // The choices as you left them, once, when the page first shows.
    onShownChanged: {
        if (page.restored || page.shown === null || !page.shown.view)
            return
        page.restored = true
        page.by = page.shown.view.by || "case"
        page.showDone = page.shown.view.done
        page.caseFilter = page.shown.view.case || ""
    }
    // A card dragged on the board, and where.
    property var dragging: null
    property point dragPoint: Qt.point(0, 0)

    function open(uid) {
        page.opened = uid
    }

    // One project's tasks, from its page.
    function showCase(id, mode) {
        page.caseFilter = id
        page.mode = mode
        page.apply()
    }

    function apply() {
        page.sioul.showTasks(page.by, page.showDone, page.query, page.caseFilter)
    }

    function tick(task) {
        const done = task.status === "completed" || task.status === "cancelled"
        page.sioul.setTaskStatus(task.uid, done ? "needs-action" : "completed")
    }

    function focusOn(uid, minutes) {
        page.sioul.focusStart(uid, minutes)
    }


    // The day closed at once; the screen after it says where everything went.
    function stopForToday() {
        const closing = page.sioul.doneForTheDay()
        if (closing !== "")
            doneDialog.now().show(JSON.parse(closing))
    }

    // For the window's tests.
    function grabStop(path) {
        doneDialog.now().grab(path)
    }

    // For the window's tests: the settings, open or closed, and as an image.
    function openSettings(open) {
        tasksSettings.show(open)
    }

    function grabSettings(path) {
        tasksSettings.grab(path)
    }

    // For the window's tests: the screen closed.
    function closeStop() {
        doneDialog.now().accept()
    }

    // For the window's tests.
    // A new task: the line to type it in.
    function startNew() {
        (page.rest ? restCapture : capture).focusLine()
    }

    // A task with its folded details shown (for captures).
    function openDetails(uid) {
        page.opened = uid
        panelLoader.item.moreShown = true
    }

    function openFirst() {
        if (page.shown && page.shown.now.now)
            page.opened = page.shown.now.now.uid
    }

    // For the window's pictures: the open task's folded details shown.
    function showPanelDetails() {
        panelLoader.item.moreShown = true
    }

    Shortcut {
        sequence: "Escape"
        enabled: page.visible && page.opened !== ""
        onActivated: page.opened = ""
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        ColumnLayout {
            visible: !(page.window.compact && page.opened !== "")
            Layout.fillHeight: true
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: page.opened === "" ? columns.width : Math.round((columns.width - columns.spacing) * 0.58)
            spacing: 10

            // The ways to see tasks, then the filters: on a second line when the
            // window is too narrow for one, never past its edge.
            Flow {
                id: viewsFlow

                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: ["now", "day", "list", "board", "timeline"]

                    delegate: Button {
                        id: modeButton

                        required property string modelData

                        text: page.sioul.text("task-mode-" + modeButton.modelData)
                        checkable: true
                        checked: page.mode === modeButton.modelData
                        flat: page.mode !== modeButton.modelData
                        // A click on the way shown unticks it: its binding ticks it again.
                        onClicked: {
                            page.mode = modeButton.modelData
                            modeButton.checked = Qt.binding(() => page.mode === modeButton.modelData)
                        }
                    }
                }
                // Routines: steps played one at a time.
                Button {
                    flat: true
                    text: page.sioul.text("routines")
                    icon.name: "media-playback-start"
                    onClicked: routinesDialog.now().show()
                }
                // The board and the timeline: one case, or all.
                ComboBox {
                    readonly property var choices: [{ id: "", title: page.sioul.text("task-all-cases") }].concat(page.shown ? page.shown.cases : [])

                    // On a phone, two choices a line.
                    width: page.narrow ? page.halfFlow : 200
                    model: choices.map(c => page.theme.plain(c.title))
                    currentIndex: Math.max(0, choices.findIndex(c => c.id === page.caseFilter))
                    onActivated: index => {
                        page.caseFilter = choices[index].id
                        page.apply()
                    }
                }
                // One kind of task at a time: the calls together, the forms together.
                ComboBox {
                    id: kindChoice

                    readonly property var choices: [{ id: "", label: page.sioul.text("task-filter-all") }].concat(page.shown ? page.shown.kinds : [])

                    width: page.narrow ? page.halfFlow : 190
                    model: choices.map(c => page.theme.plain(c.label))
                    currentIndex: Math.max(0, choices.findIndex(c => c.id === page.kindFilter))
                    Accessible.name: page.sioul.text("task-kind")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("task-kind-help")
                    ToolTip.delay: 600
                    onActivated: index => page.sioul.filterTasks(choices[index].id, page.categoryFilter)
                }
                ComboBox {
                    readonly property var choices: [""].concat(page.shown ? page.shown.categories : [])

                    visible: choices.length > 1
                    width: page.narrow ? page.halfFlow : 170
                    model: choices.map(c => c === "" ? page.sioul.text("task-filter-any-category") : page.theme.plain(c))
                    currentIndex: Math.max(0, choices.findIndex(c => c.toLowerCase() === page.categoryFilter.toLowerCase()))
                    onActivated: index => page.sioul.filterTasks(page.kindFilter, choices[index])
                }
                SettingsButton {
                    id: tasksSettings

                    sioul: page.sioul
                    theme: page.theme
                    view: "tasks"
                }
            }

            // Where you stopped, when something interrupted you, until it is done.
            StoppedCard {
                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                window: page.window
            }

            // A task in one line, wherever you are on the page. In quiet time, a
            // thought noted waits for work to come back, out of sight.
            CaptureField {
                id: capture

                Layout.fillWidth: true
                visible: page.shown !== null && !page.shown.no_list
                sioul: page.sioul
                theme: page.theme
                placeholder: page.shown !== null && page.shown.quiet ? page.sioul.text("task-note-for-later") : page.sioul.text("task-capture-hint")
                onAdded: uid => {
                    if (!page.shown || !page.shown.quiet)
                        page.opened = uid
                }
            }

            // No list takes tasks yet: one is made here.
            Panel {
                visible: page.shown !== null && page.shown.no_list
                Layout.fillWidth: true
                theme: page.theme

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 8

                    Label {
                        Layout.fillWidth: true
                        text: page.sioul.text("task-first-list")
                        wrapMode: Text.Wrap
                        color: page.theme.text
                    }
                    RowLayout {
                        spacing: 8

                        ComboBox {
                            id: listAccount

                            model: JSON.parse(page.sioul.listAccounts())
                        }
                        TextField {
                            id: listName

                            Layout.fillWidth: true
                            text: page.sioul.text("task-default-list")
                        }
                        Button {
                            text: page.sioul.text("task-make-list")
                            highlighted: true
                            onClicked: page.sioul.newList(listAccount.currentText, listName.text)
                        }
                    }
                }
            }

            StackLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                currentIndex: ["now", "day", "list", "board", "timeline"].indexOf(page.mode)

                // Now: the next step, why, and the one after it. Made when first shown
                // outside rest: behind the rest cover it waits.
                Loader {
                    active: page.nowMade

                    sourceComponent: Component {
                        ScrollView {
                            id: nowScroll

                            contentWidth: availableWidth
                            clip: true

                            ColumnLayout {
                                width: Math.min(nowScroll.availableWidth, 680)
                                x: Math.max(0, (nowScroll.availableWidth - width) / 2)
                                spacing: 12

                                // Quiet time: the rest of the day, and when work comes back.
                                ColumnLayout {
                                    visible: page.shown !== null && page.shown.quiet
                                    Layout.fillWidth: true
                                    spacing: 2

                                    Label {
                                        text: page.sioul.text(page.window.moment.reason === "time-off" ? "quiet-time-off-title" : "quiet-title")
                                        textFormat: Text.PlainText
                                        font.pixelSize: 20
                                        color: page.theme.text
                                    }
                                    Label {
                                        visible: page.window.moment.line !== ""
                                        Layout.fillWidth: true
                                        text: page.window.moment.line
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                    }
                                }
                                // The day starts with the step the plan had when the day before closed.
                                Panel {
                                    visible: page.shown !== null && page.shown.first_step !== ""
                                    Layout.fillWidth: true
                                    theme: page.theme

                                    RowLayout {
                                        anchors.fill: parent
                                        spacing: 10

                                        Label {
                                            Layout.fillWidth: true
                                            text: page.shown ? page.shown.first_step : ""
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            font.pixelSize: 17
                                            color: page.theme.text
                                        }
                                        Button {
                                            visible: page.shown !== null && page.shown.first_step_task !== ""
                                            text: page.sioul.text("focus-two")
                                            icon.name: "chronometer-start"
                                            icon.color: page.theme.text
                                            onClicked: {
                                                page.focusOn(page.shown.first_step_task, 2)
                                                page.sioul.clearFirstStep()
                                            }
                                        }
                                        Button {
                                            flat: true
                                            text: page.sioul.text("first-step-clear")
                                            onClicked: page.sioul.clearFirstStep()
                                        }
                                    }
                                }

                                // How today is: chosen, never guessed. Work's question, so not in quiet time.
                                RowLayout {
                                    visible: page.shown !== null && !page.shown.quiet
                                    Layout.fillWidth: true
                                    spacing: 6

                                    Label {
                                        Layout.fillWidth: true
                                        text: page.sioul.text("task-weather")
                                        color: page.theme.muted
                                        horizontalAlignment: Text.AlignRight
                                    }
                                    Repeater {
                                        model: ["clear", "haze", "fog"]

                                        delegate: Button {
                                            id: weatherButton

                                            required property string modelData

                                            text: page.sioul.text("task-weather-" + weatherButton.modelData)
                                            flat: !checked
                                            checkable: true
                                            checked: page.shown !== null && page.shown.weather === weatherButton.modelData
                                            onClicked: {
                                                const already = page.shown !== null && page.shown.weather === weatherButton.modelData
                                                page.sioul.setWeather(weatherButton.modelData)
                                                // A click on the weather chosen unticks it, and nothing comes
                                                // back to tick it again: its binding does.
                                                if (already)
                                                    weatherButton.checked = Qt.binding(() => page.shown !== null && page.shown.weather === weatherButton.modelData)
                                            }
                                        }
                                    }
                                }
                                // What the watch says of this morning: an offer, never an alarm.
                                RowLayout {
                                    visible: page.shown !== null && !page.shown.quiet && page.shown.morning !== "" && page.shown.weather === "clear"
                                    Layout.fillWidth: true
                                    spacing: 8

                                    Label {
                                        Layout.fillWidth: true
                                        text: page.shown && page.shown.morning !== "" ? page.sioul.text("watch-morning-" + page.shown.morning) : ""
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                    }
                                    Button {
                                        flat: true
                                        text: page.sioul.text("watch-morning-lighter")
                                        onClicked: page.sioul.setWeather(page.shown.morning === "strain" ? "fog" : "haze")
                                    }
                                }
                                Label {
                                    visible: page.shown !== null && page.shown.now.weather_note !== ""
                                    Layout.fillWidth: true
                                    text: page.shown ? page.shown.now.weather_note : ""
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.muted
                                }
                                // The next date asked within a week: the time it needs, the room there is.
                                Label {
                                    visible: page.shown !== null && page.shown.budget !== ""
                                    Layout.fillWidth: true
                                    text: page.shown ? page.shown.budget : ""
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.muted
                                }

                                Panel {
                                    Layout.fillWidth: true
                                    theme: page.theme
                                    accent: page.shown !== null && page.shown.now.now !== null

                                    ColumnLayout {
                                        anchors.fill: parent
                                        spacing: 8

                                        Label {
                                            visible: page.shown !== null && page.shown.now.now === null
                                            Layout.fillWidth: true
                                            text: page.shown ? page.shown.now.empty : ""
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            font.pixelSize: 17
                                            color: page.theme.text
                                        }
                                        Label {
                                            visible: page.shown !== null && page.shown.now.now !== null
                                            text: page.sioul.text("task-now-title")
                                            font.pixelSize: 13
                                            color: page.theme.muted
                                        }
                                        Label {
                                            visible: page.shown !== null && page.shown.now.now !== null
                                            Layout.fillWidth: true
                                            text: page.shown && page.shown.now.now ? page.shown.now.now.title : ""
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            font.pixelSize: 22
                                            color: page.theme.text

                                            TapHandler {
                                                onTapped: page.open(page.shown.now.now.uid)
                                            }
                                        }
                                        // The bigger task it is a step of, and its case.
                                        Label {
                                            readonly property var task: page.shown && page.shown.now.now ? page.shown.now.now : null

                                            visible: task !== null && text !== ""
                                            Layout.fillWidth: true
                                            text: task ? [task.parent].concat(task.cases).filter(t => t !== "").join("  ·  ") : ""
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: page.theme.muted
                                        }
                                        Repeater {
                                            model: page.shown && page.shown.now.now ? page.shown.now.why.concat(page.shown.now.picked !== "" ? [page.shown.now.picked] : []) : []

                                            delegate: Label {
                                                required property string modelData

                                                Layout.fillWidth: true
                                                text: modelData
                                                textFormat: Text.PlainText
                                                wrapMode: Text.Wrap
                                                color: page.theme.muted
                                            }
                                        }
                                        Label {
                                            readonly property var task: page.shown && page.shown.now.now ? page.shown.now.now : null

                                            visible: task !== null && text !== ""
                                            Layout.fillWidth: true
                                            text: task ? [task.estimate, task.stopped, task.steps].filter(t => t !== "").join("  ·  ") : ""
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: page.theme.text
                                        }
                                        Flow {
                                            visible: page.shown !== null && page.shown.now.now !== null
                                            Layout.fillWidth: true
                                            spacing: 6

                                            Button {
                                                text: page.sioul.text("task-start")
                                                icon.name: "chronometer-start"
                                                icon.color: page.theme.accentText
                                                highlighted: true
                                                onClicked: nowStart.now().popup()

                                                Later {
                                                    id: nowStart

                                                    sourceComponent: Component {
                                                        SioulMenu {
                                                            id: nowStartForm

                                                            Repeater {
                                                                model: [2, 15, 25, 45, 0]

                                                                delegate: MenuItem {
                                                                    required property int modelData

                                                                    text: modelData === 2 ? page.sioul.text("focus-two") : modelData === 0 ? page.sioul.text("focus-open-ended") : page.sioul.textWith("focus-for", "minutes", String(modelData))
                                                                    onTriggered: page.focusOn(page.shown.now.now.uid, modelData)
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Button {
                                                text: page.sioul.text("task-done")
                                                icon.name: "task-complete"
                                                icon.color: page.theme.text
                                                onClicked: page.sioul.setTaskStatus(page.shown.now.now.uid, "completed")
                                            }
                                            Button {
                                                flat: true
                                                text: page.sioul.text("task-not-now")
                                                onClicked: page.sioul.notNow(page.shown.now.now.uid)
                                            }
                                            Button {
                                                flat: true
                                                text: page.sioul.text("task-hard")
                                                onClicked: hardMenu.now().popup()

                                                // What makes it hard: one matching help each.
                                                Later {
                                                    id: hardMenu

                                                    sourceComponent: Component {
                                                        SioulMenu {
                                                            id: hardMenuForm

                                                            MenuItem {
                                                                text: page.sioul.text("task-hard-how")
                                                                onTriggered: page.open(page.shown.now.now.uid)
                                                            }
                                                            MenuItem {
                                                                text: page.sioul.text("task-hard-big")
                                                                onTriggered: {
                                                                    page.open(page.shown.now.now.uid)
                                                                    Qt.callLater(() => panelLoader.item.addStep())
                                                                }
                                                            }
                                                            MenuItem {
                                                                text: page.sioul.text("task-hard-dread")
                                                                onTriggered: page.focusOn(page.shown.now.now.uid, 2)
                                                            }
                                                            MenuItem {
                                                                text: page.sioul.text("task-hard-boring")
                                                                onTriggered: page.focusOn(page.shown.now.now.uid, 2)
                                                            }
                                                            MenuItem {
                                                                text: page.sioul.text("task-hard-energy")
                                                                onTriggered: page.sioul.setWeather("fog")
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // After a heavy step: something that gives back, offered.
                                Label {
                                    visible: page.shown !== null && page.shown.now.rest !== null
                                    text: page.sioul.text("task-rest-offer")
                                    font.pixelSize: 13
                                    color: page.theme.muted
                                    Layout.topMargin: 4
                                }
                                TaskRow {
                                    visible: page.shown !== null && page.shown.now.rest !== null
                                    Layout.fillWidth: true
                                    task: page.shown && page.shown.now.rest ? page.shown.now.rest : ({ uid: "", title: "", status: "", due: "", estimate: "", steps: "", waits: "", stopped: "", list: "", done_on: "", read_only: true, has_steps: false })
                                    theme: page.theme
                                    sioul: page.sioul
                                    compact: true
                                    onOpen: uid => page.open(uid)
                                    onMenu: task => taskMenu.now().show(task)
                                    onTick: page.tick(page.shown.now.rest)
                                }

                                // The one after it, in a line.
                                Label {
                                    visible: page.shown !== null && page.shown.now.then !== null
                                    text: page.sioul.text("task-then")
                                    font.pixelSize: 13
                                    color: page.theme.muted
                                    Layout.topMargin: 4
                                }
                                TaskRow {
                                    visible: page.shown !== null && page.shown.now.then !== null
                                    Layout.fillWidth: true
                                    task: page.shown && page.shown.now.then ? page.shown.now.then : ({ uid: "", title: "", status: "", due: "", estimate: "", steps: "", waits: "", stopped: "", list: "", done_on: "", read_only: true, has_steps: false })
                                    theme: page.theme
                                    sioul: page.sioul
                                    compact: true
                                    onOpen: uid => page.open(uid)
                                    onMenu: task => taskMenu.now().show(task)
                                    onTick: page.tick(page.shown.now.then)
                                }

                                // Behind a key: two other choices, what is started, done this week.
                                Repeater {
                                    model: [
                                        { key: "others", label: "task-other-choices", items: page.shown ? page.shown.now.others : [] },
                                        { key: "started", label: "task-started", items: page.shown ? page.shown.now.started : [] },
                                        { key: "joy", label: "task-joy", items: page.shown ? page.shown.now.joy : [] },
                                        { key: "done", label: "task-done-week", items: page.shown ? page.shown.now.done_week : [] }
                                    ]

                                    delegate: ColumnLayout {
                                        id: fold

                                        required property var modelData
                                        readonly property bool open: fold.modelData.key === "others" ? page.othersShown : fold.modelData.key === "started" ? page.startedShown : fold.modelData.key === "joy" ? page.joyShown : page.doneShown

                                        visible: fold.modelData.items.length > 0
                                        Layout.fillWidth: true
                                        spacing: 2

                                        Button {
                                            flat: true
                                            text: (fold.open ? "▾  " : "▸  ") + page.sioul.text(fold.modelData.label)
                                            onClicked: {
                                                if (fold.modelData.key === "others")
                                                    page.othersShown = !page.othersShown
                                                else if (fold.modelData.key === "started")
                                                    page.startedShown = !page.startedShown
                                                else if (fold.modelData.key === "joy")
                                                    page.joyShown = !page.joyShown
                                                else
                                                    page.doneShown = !page.doneShown
                                            }
                                        }
                                        Repeater {
                                            model: fold.open ? fold.modelData.items : []

                                            delegate: TaskRow {
                                                required property var modelData

                                                Layout.fillWidth: true
                                                task: modelData
                                                theme: page.theme
                                                sioul: page.sioul
                                                compact: true
                                                selected: page.opened === modelData.uid
                                                onOpen: uid => page.open(uid)
                                                onMenu: task => taskMenu.now().show(task)
                                                onTick: page.tick(modelData)
                                            }
                                        }
                                    }
                                }
                                Repeater {
                                    model: page.shown ? page.shown.now.loops.concat(page.shown.now.wip !== "" ? [page.shown.now.wip] : []) : []

                                    delegate: Label {
                                        required property string modelData

                                        Layout.fillWidth: true
                                        text: modelData
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                    }
                                }
                                // Out of energy before the list is done: today's tasks
                                // move to their next day, and work rests until it is back.
                                Button {
                                    visible: page.shown !== null && !page.shown.quiet
                                    Layout.topMargin: 12
                                    Layout.alignment: Qt.AlignHCenter
                                    flat: true
                                    text: page.sioul.text("done-button")
                                    icon.name: "weather-clear-night"
                                    icon.color: page.theme.text
                                    ToolTip.visible: hovered
                                    ToolTip.text: page.sioul.text("done-button-help")
                                    ToolTip.delay: 500
                                    onClicked: page.stopForToday()
                                }
                            }
                        }
                    }
                }

                // The day: events at their times, today's steps in the gaps, now.
                // Made the first time it is shown.
                Loader {
                    active: page.made["day"] === true
                    sourceComponent: Component {
                        DayView {
                            sioul: page.sioul
                            theme: page.theme
                            day: page.shown ? page.shown.day : null
                            onRelay: page.sioul.refreshWork()
                            onOpenTask: uid => page.open(uid)
                            onOpenEvent: key => page.window.openThing({ kind: "event", uri: "", key: key })
                        }
                    }
                }

                // The list: every open task, a bigger one followed by its steps.
                // Made the first time it is shown.
                Loader {
                    active: page.made["list"] === true
                    sourceComponent: Component {
                        ColumnLayout {
                            spacing: 8

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                TextField {
                                    Layout.fillWidth: true
                                    placeholderText: page.sioul.text("task-search")
                                    onTextEdited: {
                                        page.query = text
                                        page.apply()
                                    }
                                }
                                ComboBox {
                                    model: [page.sioul.text("task-by-case"), page.sioul.text("task-by-list")]
                                    currentIndex: page.by === "list" ? 1 : 0
                                    onActivated: index => {
                                        page.by = index === 1 ? "list" : "case"
                                        page.apply()
                                    }
                                }
                                CheckBox {
                                    text: page.sioul.text("task-show-done")
                                    checked: page.showDone
                                    onToggled: {
                                        page.showDone = checked
                                        page.apply()
                                    }
                                }
                            }
                            ListView {
                                id: groups

                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                spacing: 14
                                model: page.shown ? page.shown.list.groups : []
                                ScrollBar.vertical: ScrollBar {}

                                delegate: ColumnLayout {
                                    id: group

                                    required property var modelData

                                    width: groups.width - 12
                                    spacing: 2

                                    Label {
                                        text: group.modelData.title
                                        textFormat: Text.PlainText
                                        font.weight: Font.DemiBold
                                        font.pixelSize: 16
                                        color: page.theme.text
                                    }
                                    Repeater {
                                        model: group.modelData.rows

                                        delegate: TaskRow {
                                            required property var modelData

                                            Layout.fillWidth: true
                                            task: modelData
                                            theme: page.theme
                                            sioul: page.sioul
                                            depth: modelData.depth
                                            selected: page.opened === modelData.uid
                                            onOpen: uid => page.open(uid)
                                            onMenu: task => taskMenu.now().show(task)
                                            onTick: page.tick(modelData)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // The board: free to start, started, waiting, done. Cards move by dragging.
                // Made the first time it is shown.
                Loader {
                    active: page.made["board"] === true
                    sourceComponent: Component {
                        ColumnLayout {
                            id: boardView

                            // A card dropped on a column: started, done, or free again.
                            function dropAt(point, task) {
                                for (let i = 0; i < boardColumns.count; ++i) {
                                    const column = boardColumns.itemAt(i)
                                    const local = column.mapFromItem(null, point.x, point.y)
                                    if (local.x < 0 || local.y < 0 || local.x > column.width || local.y > column.height)
                                        continue
                                    const status = ({ "ready": "needs-action", "doing": "in-process", "done": "completed" })[page.shown.board.columns[i].id]
                                    if (status && status !== task.status)
                                        page.sioul.setTaskStatus(task.uid, status)
                                }
                            }

                            spacing: 6

                            Label {
                                visible: page.shown !== null && page.shown.board.wip !== ""
                                Layout.fillWidth: true
                                text: page.shown ? page.shown.board.wip : ""
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: page.theme.muted
                            }
                            // Side by side; on a phone, one under the other, the board scrolled whole.
                            Flickable {
                                id: board

                                Layout.fillWidth: true
                                Layout.fillHeight: true
                                clip: true
                                contentWidth: width
                                contentHeight: page.narrow ? boardGrid.implicitHeight : height
                                interactive: page.narrow && page.dragging === null
                                boundsBehavior: Flickable.StopAtBounds

                                GridLayout {
                                    id: boardGrid

                                    width: board.width
                                    height: page.narrow ? implicitHeight : board.height
                                    columns: page.narrow ? 1 : Math.max(1, boardColumns.count)
                                    columnSpacing: 10
                                    rowSpacing: 10

                                    Repeater {
                                        id: boardColumns

                                        model: page.shown ? page.shown.board.columns : []

                                        delegate: Rectangle {
                                            id: column

                                            required property var modelData
                                            readonly property string columnId: column.modelData.id
                                            readonly property bool target: {
                                                if (page.dragging === null || column.columnId === "waiting")
                                                    return false
                                                const local = column.mapFromItem(null, page.dragPoint.x, page.dragPoint.y)
                                                return local.x >= 0 && local.y >= 0 && local.x <= column.width && local.y <= column.height
                                            }

                                            Layout.fillWidth: true
                                            Layout.fillHeight: !page.narrow
                                            Layout.preferredWidth: 1
                                            Layout.preferredHeight: page.narrow ? columnContent.implicitHeight + 16 : -1
                                            radius: page.theme.radius
                                            color: column.target ? page.theme.surface : "transparent"
                                            border.color: column.target ? page.theme.accent : page.theme.line

                                            ColumnLayout {
                                                id: columnContent

                                                anchors.fill: parent
                                                anchors.margins: 8
                                                spacing: 6

                                                Label {
                                                    text: column.modelData.title
                                                    textFormat: Text.PlainText
                                                    font.weight: Font.DemiBold
                                                    color: page.theme.text
                                                }
                                                ListView {
                                                    id: cards

                                                    Layout.fillWidth: true
                                                    Layout.fillHeight: !page.narrow
                                                    // On a phone, every card: the board scrolls, not the column.
                                                    Layout.preferredHeight: page.narrow ? cards.contentHeight : -1
                                                    clip: true
                                                    spacing: 4
                                                    model: column.modelData.cards
                                                    interactive: !page.narrow && page.dragging === null

                                                    delegate: Item {
                                                        id: holder

                                                        required property var modelData

                                                        width: cards.width
                                                        height: card.implicitHeight

                                                        TaskRow {
                                                            id: card

                                                            width: parent.width
                                                            task: holder.modelData
                                                            theme: page.theme
                                                            sioul: page.sioul
                                                            compact: true
                                                            selected: page.opened === holder.modelData.uid
                                                            opacity: page.dragging !== null && page.dragging.uid === holder.modelData.uid ? 0.4 : 1
                                                            onOpen: uid => page.open(uid)
                                                            onMenu: task => taskMenu.now().show(task)
                                                            onTick: page.tick(holder.modelData)
                                                        }
                                                        // With a mouse only: on a touch screen a drag scrolls the board.
                                                        DragHandler {
                                                            id: drag

                                                            target: null
                                                            enabled: !holder.modelData.read_only
                                                            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                                            onActiveChanged: {
                                                                if (drag.active) {
                                                                    page.dragging = holder.modelData
                                                                } else {
                                                                    boardView.dropAt(page.dragPoint, holder.modelData)
                                                                    page.dragging = null
                                                                }
                                                            }
                                                            onCentroidChanged: page.dragPoint = drag.centroid.scenePosition
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

                // The timeline: each open task on the days the plan gives it.
                // Made the first time it is shown.
                Loader {
                    active: page.made["timeline"] === true
                    sourceComponent: Component {
                        TaskTimeline {
                            sioul: page.sioul
                            theme: page.theme
                            timeline: page.shown ? page.shown.timeline : null
                            opened: page.opened
                            onOpen: uid => page.open(uid)
                        }
                    }
                }
            }
        }

        Loader {
            id: panelLoader

            active: page.panelMade
            visible: page.opened !== ""
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.42)

            sourceComponent: TaskPanel {
                theme: page.theme
                sioul: page.sioul
                window: page.window
                uid: page.opened
                cases: page.shown ? page.shown.cases : []
                kinds: page.shown ? page.shown.kinds : []
                categories: page.shown ? page.shown.categories : []
                onClosed: page.opened = ""
                onFocusRequested: (uid, minutes) => page.focusOn(uid, minutes)
            }
        }
    }

    Later {
        id: doneDialog

        sourceComponent: Component {
            DoneDialog {
                id: doneDialogForm

                sioul: page.sioul
                theme: page.theme
                window: page.window
            }
        }
    }

    // Right click on a task: open it, or tie it to something new or something that exists.
    Later {
        id: taskMenu

        sourceComponent: Component {
            SioulMenu {
                id: taskMenuForm

                property var target: null
                readonly property var source: taskMenuForm.target ? { uri: "sioul:task/" + encodeURIComponent(taskMenuForm.target.uid), kind: "task", key: taskMenuForm.target.uid, title: taskMenuForm.target.title } : null

                function show(task) {
                    taskMenuForm.target = task
                    taskMenuForm.popup()
                }

                MenuItem {
                    text: page.sioul.text("ui-open")
                    onTriggered: page.open(taskMenuForm.target.uid)
                }
                MenuSeparator {}
                AddMenu {
                    sioul: page.sioul
                    window: page.window
                    source: taskMenuForm.source
                }
                MenuItem {
                    text: page.sioul.text("ui-link-existing")
                    onTriggered: page.window.linkFrom(taskMenuForm.source)
                }
            }
        }
    }

    // The card being dragged, under the pointer.
    Rectangle {
        visible: page.dragging !== null
        x: page.dragPoint.x - page.mapToItem(null, 0, 0).x + 8
        y: page.dragPoint.y - page.mapToItem(null, 0, 0).y + 8
        width: ghost.implicitWidth + 20
        height: ghost.implicitHeight + 12
        radius: page.theme.radius
        color: page.theme.surface
        border.color: page.theme.accent

        Label {
            id: ghost

            anchors.centerIn: parent
            text: page.dragging ? page.dragging.title : ""
            textFormat: Text.PlainText
            color: page.theme.text
        }
    }

    Later {
        id: routinesDialog

        sourceComponent: Component {
            RoutinesDialog {
                id: routinesDialogForm

                sioul: page.sioul
                theme: page.theme
                onPlay: routine => page.window.playRoutine(routine)
            }
        }
    }

    // Rest: one line, a thought noted for later, and the tasks if you ask.
    RestCover {
        visible: page.rest
        sioul: page.sioul
        theme: page.theme
        line: page.window.moment.line
        onShown: {
            page.anyway = true
            page.sioul.showTasksAnyway(true)
        }

        CaptureField {
            id: restCapture

            Layout.fillWidth: true
            sioul: page.sioul
            theme: page.theme
            placeholder: page.sioul.text("task-note-for-later")
        }
    }
}
