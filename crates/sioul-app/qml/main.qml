// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's window: on the left, a narrow column of icons (RailButton.qml):
// "New" for anything, the places where your things live, then, apart at the
// bottom, the accounts, the settings and one button that refreshes
// everything; their names beside them when Settings ▸ Display says so, else
// said when the pointer rests on one; the whole column hidden by the button
// at its foot or F9, on this device. One quiet status line at the bottom,
// where "Undo" waits ten seconds after anything is moved, deleted or sent. No
// badges, no counts in the title, no red; the system's dark mode is
// followed. Everything works from the keyboard: Tab, Enter, Escape, Ctrl+1 to
// Ctrl+9 and Ctrl+0 (the places by the list), Ctrl+N (new), Ctrl+Z, F5
// (refresh), F9 (the places hidden or shown). Whatever links lead to (a task,
// a message, a note, an event, a contact) opens where it lives. On a phone,
// or in a window as narrow, the places are pulled over the pages from the
// left (☰) with their names, the pages take the whole width, and Android's
// Back puts the places away, then goes back to the Porch.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import com.aurelienpierre.sioul

SioulWindow {
    id: window

    property int page: 0

    // The documentation's pictures (SIOUL_GRAB_STEPS=demo) are a landscape screen;
    // with SIOUL_GRAB_PHONE (or SIOUL_GRAB_STEPS=phone), a phone's: 412 by 891, its layout.
    readonly property bool phoneGrab: sioul.grabFolder() !== "" && (sioul.grabSteps() === "phone" || sioul.grabPhone())
    readonly property bool demoGrab: sioul.grabFolder() !== "" && sioul.grabSteps() === "demo" && !window.phoneGrab

    width: window.phoneGrab ? 412 : window.demoGrab ? 1280 : 1100
    // Taller when saving images of the pages, so long pages show whole.
    height: window.phoneGrab ? 891 : window.demoGrab ? 860 : sioul.grabFolder() ? 1500 : 760
    // A phone's screen is the window, whatever its size.
    minimumWidth: Qt.platform.os === "android" || window.phoneGrab ? 0 : 680
    minimumHeight: Qt.platform.os === "android" || window.phoneGrab ? 0 : 480
    visible: true
    title: "Sioul"
    theme: theme

    // The writing windows open, one per draft.
    property var drafts: []
    // Month and day names in Sioul's language, not the system's.
    readonly property var sioulLocale: Qt.locale(sioul.text("qt-locale"))
    // A phone, or a window as narrow: the places pulled over the pages from the left.
    readonly property bool compact: window.width < 720
    property bool placesOpen: false
    // The places hidden beside the pages (F9, or the button at their foot): this
    // device's choice, read as Sioul starts; a phone's ☰ is not concerned.
    property bool placesHidden: false
    // The places made the first time they show, then kept (Places.qml): a
    // phone's drawer at its first ☰, a column hidden as Sioul starts when it is
    // shown again; never before this device's choice is read.
    property bool placesMade: false
    onPlacesOpenChanged: {
        if (window.placesOpen)
            window.placesMade = true
    }
    onRailShownChanged: {
        if (window.railShown)
            window.placesMade = true
    }
    // Their names beside their icons (Settings ▸ Display), wherever they are beside the pages.
    readonly property bool placesNamed: sioul.reading ? JSON.parse(sioul.reading).places_named === true : false
    // The places beside the pages: a window wide enough, and not hidden.
    readonly property bool railShown: !window.compact && !window.placesHidden
    // Where a long press opened a menu, for the menu to come there (SioulMenu.qml):
    // a touch screen has no cursor to place it at. Forgotten after a moment.
    property var menuAt: null
    onMenuAtChanged: {
        if (window.menuAt !== null)
            menuAtForgotten.restart()
    }

    Timer {
        id: menuAtForgotten

        interval: 1000
        onTriggered: window.menuAt = null
    }

    // Each page's name, by its place in the pages' stack.
    readonly property var pageNames: ["ui-porch", "ui-tasks", "ui-mail", "ui-sites", "ui-agenda", "ui-contacts", "ui-notes", "ui-projects", "ui-time", "ui-budgets", "ui-health", "ui-accounts", "ui-parameters", "ui-papers"]
    // Each page is made when it is first shown, then kept: the window opens
    // with the Porch alone, and a page asked for (a link, "New") is made
    // before it is used, as the page is set first.
    property var made: sioul.grabFolder() !== "" ? ({ 0: true, 1: true, 2: true, 3: true, 4: true, 5: true, 6: true, 7: true, 8: true, 9: true, 10: true, 11: true, 12: true, 13: true }) : ({ 0: true })
    onPageChanged: {
        window.placesOpen = false
        if (window.made[window.page] !== true) {
            const made = Object.assign({}, window.made)
            made[window.page] = true
            window.made = made
        }
    }
    // Each page's file, read the first time it is made (setSource, for their
    // required properties): what it imports (maps, PDFs, sites, sounds) is
    // loaded then, not as the window opens. Android has its own Sites page.
    readonly property var pageFiles: ["PorchPage.qml", "TasksPage.qml", "MailPage.qml", Qt.platform.os === "android" ? "android/SitesPage.qml" : "SitesPage.qml", "AgendaPage.qml", "ContactsPage.qml", "NotesPage.qml", "ProjectsPage.qml", "TimePage.qml", "BudgetsPage.qml", "HealthPage.qml", "AccountsPage.qml", "ParametersPage.qml", "PapersPage.qml"]
    onMadeChanged: window.makePages()
    function makePages() {
        const loaders = [porchPageLoader, tasksPageLoader, mailPageLoader, sitesPageLoader, agendaPageLoader, contactsPageLoader, notesPageLoader, projectsPageLoader, timePageLoader, budgetsPageLoader, healthPageLoader, accountsPageLoader, parametersPageLoader, papersPageLoader]
        for (let i = 0; i < loaders.length; i++) {
            if (window.made[i] !== true || loaders[i].status !== Loader.Null)
                continue
            // Accounts and Settings take no window.
            const given = i === 11 || i === 12 ? { sioul: sioul, theme: theme } : { sioul: sioul, theme: theme, window: window }
            const began = Date.now()
            loaders[i].setSource(window.pageFiles[i], given)
            // How long each page takes to make, in the log (a phone's: adb logcat).
            console.info("sioul-perf: " + window.pageFiles[i] + " made in " + (Date.now() - began) + " ms")
        }
    }
    readonly property var porchPage: porchPageLoader.item
    readonly property var tasksPage: tasksPageLoader.item
    readonly property var mailPage: mailPageLoader.item
    readonly property var sitesPage: sitesPageLoader.item
    readonly property var agendaPage: agendaPageLoader.item
    readonly property var contactsPage: contactsPageLoader.item
    readonly property var notesPage: notesPageLoader.item
    readonly property var projectsPage: projectsPageLoader.item
    readonly property var timePage: timePageLoader.item
    readonly property var budgetsPage: budgetsPageLoader.item
    readonly property var healthPage: healthPageLoader.item
    readonly property var accountsPage: accountsPageLoader.item
    readonly property var parametersPage: parametersPageLoader.item
    readonly property var papersPage: papersPageLoader.item
    // The page in view; on a phone, a page showing one thing it opened
    // (a message, a task) says so (canGoBack) and closes it (back()).
    readonly property var shownPage: pages.children[window.page] ? pages.children[window.page].item : null
    readonly property bool canGoBack: window.compact && window.shownPage !== null && window.shownPage.canGoBack === true
    // Work time or quiet time: {quiet, reason, until, line, hours}.
    readonly property var moment: sioul.mode ? JSON.parse(sioul.mode) : ({ quiet: false, sleep: false, reason: "", until: "", line: "", hours: false, night: true })

    // The start timed (on Android, and with SIOUL_TIMING): the first frame drawn.
    property bool drawn: false
    onFrameSwapped: {
        if (!window.drawn) {
            window.drawn = true
            sioul.firstFrame()
            sitesWarm.start()
        }
    }
    // Sites kept open (their notifications) live in the Sites page: made a
    // moment after the window shows, when there are such sites.
    Timer {
        id: sitesWarm

        interval: 1500
        onTriggered: {
            if (window.made[3] !== true && JSON.parse(sioul.siteList() || "[]").some(s => s.background)) {
                const made = Object.assign({}, window.made)
                made[3] = true
                window.made = made
            }
        }
    }

    // Now: the window's clock (Clock.qml), one for every page. The pages give
    // it to what shows the time of day (the line at now in the day views,
    // today's marks) and follow it to read again. At each minute's turn, the
    // backend's minute too: doses, work coming and going with the hours, the
    // exchange; on a phone put away as well; half a minute after the start at
    // least, the window first.
    Clock {
        id: clock

        readonly property real started: Date.now()

        paused: sioul.away
        onTurned: {
            if (Date.now() - clock.started >= 30000)
                sioul.refreshMode()
        }
    }
    property alias now: clock.now
    property alias today: clock.today

    // Starts a draft ("new", "reply", "reply-all", "forward") and opens its window.
    function compose(kind, key) {
        const account = window.page === 2 ? mailPage.account : ""
        const id = sioul.compose(kind, key || "", account)
        if (id)
            window.openDraft(id)
    }

    // "Write" from a contact: a new message to them.
    function writeTo(recipients) {
        const id = sioul.compose("new", "", "")
        if (!id)
            return
        window.openDraft(id)
        window.drafts[window.drafts.length - 1].prefill(recipients)
    }

    // A sender's contact, from a message.
    function openContact(key) {
        window.page = 5
        contactsPage.open(key)
    }

    function openTask(uid) {
        window.page = 1
        tasksPage.open(uid)
    }

    function openNote(path) {
        window.page = 6
        notesPage.open(path)
    }

    // A new event made from a message or a task, the link kept.
    function makeEvent(title, note, link) {
        window.page = 4
        agendaPage.makeFrom(title, note, link)
    }

    // Something new tied to `source` ({uri, kind, key, title, start, name,
    // address}): made, then opened where it lives. A task opens as a new
    // task's full form, its title and its tie given: made once its title is.
    function addLinked(kind, source) {
        if (kind === "task") {
            window.page = 1
            tasksPage.startNew(source)
            return
        }
        if (kind === "event") {
            window.makeEvent(source.kind === "contact" ? "" : source.title, "", source.uri)
            return
        }
        if (kind === "mail" && source.kind === "mail") {
            window.compose("reply", source.key)
            return
        }
        if (kind === "mail" && source.kind === "contact" && source.address) {
            window.writeTo(source.name ? source.name + " <" + source.address + ">" : source.address)
            return
        }
        if (kind === "contact") {
            sioul.addSender(source.name || "", source.address)
            return
        }
        const made = JSON.parse(sioul.makeLinked(kind, source.uri, source.key || "", source.start || 0))
        if (made.uid)
            window.openTask(made.uid)
        else if (made.path)
            window.openNote(made.path)
        else if (made.draft)
            window.openDraft(made.draft)
        else if (made.error)
            sioul.status = made.error
    }

    // Something new from the "New" menu: opened where that kind lives.
    function newThing(kind) {
        if (kind === "mail") {
            window.compose("new", "")
        } else if (kind === "task") {
            window.page = 1
            tasksPage.startNew()
        } else if (kind === "event") {
            window.page = 4
            agendaPage.newEvent(agendaPage.today())
        } else if (kind === "contact") {
            window.page = 5
            contactsPage.startNew()
        } else if (kind === "note") {
            window.page = 6
            notesPage.startNew()
        } else if (kind === "project") {
            window.page = 7
            projectsPage.startNew()
        } else if (kind === "time") {
            window.page = 8
            timePage.startNew()
        } else if (kind === "movement") {
            window.page = 9
            budgetsPage.startNew()
        } else if (kind === "paper") {
            window.page = 13
            papersPage.startNew()
        } else if (kind === "stopped") {
            window.askNeed("")
        }
    }

    // Put away on a phone: what was marked goes out, and your other devices
    // learn this one marks nothing until it is back. Its next minute says it is.
    Connections {
        target: Qt.application
        enabled: Qt.platform.os === "android"

        function onStateChanged() {
            if (Qt.application.state === Qt.ApplicationSuspended || Qt.application.state === Qt.ApplicationHidden) {
                // No text field keeps the focus in the back (the keyboard goes
                // with it): Qt's text handles would keep asking Android for the
                // clipboard, refused.
                window.contentItem.forceActiveFocus()
                sioul.goingAway()
                // What it draws with, given back while away (android/main.cpp).
                window.releaseResources()
            }
            else if (Qt.application.state === Qt.ApplicationActive) {
                sioul.backHere()
                // What waited while it was away: the plan, the mode, the sites' notices.
                sioul.refreshMode()
            }
        }
    }

    // A reminder's "Open": what it is about, shown, the window brought forward.
    Connections {
        target: sioul

        function onReminderOpened(kind, uri, key) {
            window.openThing({ kind: kind, uri: uri, key: key })
            window.raise()
            window.requestActivate()
        }

        // An attachment kept in the wallet: what it is, asked.
        function onPaperKept(file, title, kind) {
            window.page = 13
            papersPage.startFrom(file, title, kind)
        }

        // A task done: "How was it?" beside the line that says so, gone with it.
        function onTaskDone(uid) {
            window.lastDone = uid
            window.lastDoneSaid = sioul.status
        }
    }

    // The task just done, and the status line that said so.
    property string lastDone: ""
    property string lastDoneSaid: ""

    // How a task just done was: its ratings as felt, if you want to say (HowWasIt.qml, made the first time).
    function howWasIt(uid) {
        if (howWasItForm.item === null)
            howWasItForm.setSource("HowWasIt.qml", { sioul: sioul, theme: theme })
        window.howWasItPopup.ask(uid)
    }
    readonly property var howWasItPopup: howWasItForm.item

    // A mail kept as a contract: the Budgets page, the form started from it.
    function contractFromMail(subject, from) {
        window.page = 9
        budgetsPage.startContract(JSON.parse(sioul.contractFrom(subject, from)))
    }

    // "Link to…": the picker, for `source`.
    function linkFrom(source, kind) {
        if (!source || !source.uri)
            return
        if (linkPicker.item === null)
            linkPicker.setSource("LinkPicker.qml", { sioul: sioul, theme: theme })
        linkPicker.item.show(source, kind || "")
    }

    // Something new, of any kind, under the New button.
    function showNewMenu() {
        if (newMenu.item === null)
            newMenu.setSource("NewMenu.qml", { sioul: sioul, window: window })
        // Under New where it shows; the places hidden or put away, at the page's top left.
        const under = (window.railShown || window.placesOpen) && places.shown !== null
        newMenu.item.popup(under ? places.shown.addButton : pages, under ? 0 : theme.gap, under ? places.shown.addButton.height : theme.gap)
    }

    // The places beside the pages hidden, or shown again; kept on this device.
    // The keyboard's focus on them goes to the button that brings them back,
    // and back; with a ring when it came by the keyboard (`byKeys`).
    function togglePlaces(byKeys) {
        let inPlaces = false
        for (let item = window.activeFocusItem; item !== null; item = item.parent)
            inPlaces = inPlaces || item === places || item === showPlaces
        window.placesHidden = !window.placesHidden
        sioul.setViewFlag("places-hidden", window.placesHidden)
        if (inPlaces && (window.placesHidden || places.shown !== null))
            (window.placesHidden ? showPlaces : places.shown.hideButton).forceActiveFocus(byKeys ? Qt.TabFocusReason : Qt.MouseFocusReason)
    }

    // The places' names and keys, at the right of the button under the pointer,
    // reached by the keyboard or held by a finger (RailButton.qml): one tip for
    // them all, made the first time one is asked for. Half a second on hover;
    // at once, for `ms` milliseconds, after a long press.
    function sayRailTip(button, shown, ms) {
        if (!shown) {
            const said = railTip.item as ToolTip
            if (said !== null && said.parent === button)
                said.close()
            return
        }
        railTip.active = true
        const tip = railTip.item as ToolTip
        tip.parent = button
        tip.text = button.tip
        tip.delay = ms > 0 ? 0 : 500
        tip.timeout = ms
        tip.open()
    }

    // You are at this window: what follows you (medicines' reminders) comes
    // here, and now is read again at once (the window back after a while, a
    // computer woken from sleep); a phone's Sioul back reads it as it unpauses.
    onActiveChanged: {
        if (!window.active)
            return
        sioul.touch()
        clock.read()
    }

    Timer {
        interval: 30000
        repeat: true
        running: window.active
        onTriggered: sioul.touch()
    }

    // A dose marked late, or due while Sioul was closed: when it was taken
    // (DoseTaken.qml, made the first time); the pages showing doses read them again.
    function askDose(key) {
        if (doseTaken.item === null)
            doseTaken.setSource("DoseTaken.qml", { sioul: sioul, theme: theme })
        doseTaken.item.ask(key)
    }

    // A meal's, a nap's or the night's question (later, at another time, not
    // today), or "" to note where you stopped alone (Interruption.qml, made
    // the first time); the pages showing them read again.
    function askNeed(key) {
        if (interruption.item === null)
            interruption.setSource("Interruption.qml", { sioul: sioul, theme: theme })
        interruption.item.ask(key)
    }

    // Whatever a link leads to, opened where it lives.
    function openThing(item) {
        const id = uri => decodeURIComponent(item.uri.slice(uri.length))
        if (item.kind === "task")
            window.openTask(id("sioul:task/"))
        else if (item.kind === "note")
            window.openNote(id("sioul:note/").split("#")[0])
        else if (item.kind === "event") {
            window.page = 4
            agendaPage.openEvent(item.key)
        } else if (item.kind === "mail") {
            window.page = 2
            mailPage.openMessage(item.key)
        } else if (item.kind === "draft")
            window.openDraft(item.key)
        else if (item.kind === "contact")
            window.openContact(item.key)
        else if (item.kind === "budget")
            window.page = 9
        else if (item.kind === "paper") {
            window.page = 13
            papersPage.open(item.key || decodeURIComponent(item.uri.slice("sioul:paper/".length)))
        } else if (item.kind === "contract") {
            window.page = 9
            budgetsPage.openContract(item.key || decodeURIComponent(item.uri.slice("sioul:contract/".length)))
        }
        else if (item.kind === "case")
            window.openProject(decodeURIComponent(item.uri.slice("sioul:case/".length)))
        else if (item.kind === "health")
            window.page = 10
        // Meals, naps and the night, where Health sets them (the Porch's card, Settings ▸ Hours).
        else if (item.kind === "needs") {
            window.page = 10
            if (window.healthPage)
                window.healthPage.showNeeds()
        }
        else if (item.kind === "porch")
            window.page = 0
        // The phone's home screen card (homecard.rs): Now; its step's details
        // when Now shows another step (the card was older than the plan).
        else if (item.kind === "now") {
            window.page = 1
            tasksPage.mode = "now"
            const step = item.uri.startsWith("sioul:task/") ? id("sioul:task/") : ""
            const shown = tasksPage.shown
            if (step !== "" && !(shown && shown.now.now && shown.now.now.uid === step))
                tasksPage.open(step)
        }
        else if (item.kind === "dose")
            window.askDose(item.key)
        else if (item.kind === "need")
            window.askNeed(item.key)
        else if (item.kind === "stopped")
            window.askNeed("")
        else if (item.kind === "review")
            window.reviewDay(item.key || "work")
        else if (item.kind === "site") {
            window.page = 3
            sitesPage.open(item.key || decodeURIComponent(item.uri.slice("sioul:site/".length)))
        }
        else if (item.key !== "")
            Qt.openUrlExternally(item.key)
    }

    // A site of the Sites page, opened.
    function openSite(id) {
        window.page = 3
        sitesPage.open(id)
    }

    // A project's page.
    function openProject(id) {
        window.page = 7
        projectsPage.open(id)
    }

    // A project's tasks, as a board, a list or a calendar.
    function showTasksOf(id, mode) {
        window.page = 1
        tasksPage.showCase(id, mode)
    }

    // A draft's window; brought forward when it is already open.
    function openDraft(id) {
        for (const open of window.drafts) {
            if (open.draftId === id) {
                open.raise()
                open.requestActivate()
                return
            }
        }
        if (window.composer === null)
            window.composer = Qt.createComponent("ComposeWindow.qml")
        const opened = window.composer.createObject(null, { draftId: id, sioul: sioul, theme: theme })
        window.drafts = window.drafts.concat([opened])
        opened.finished.connect(() => {
            window.drafts = window.drafts.filter(w => w !== opened)
            Qt.callLater(() => opened.destroy())
        })
    }

    // Copies text the way Qt Quick allows it: through a hidden text field.
    function copy(text) {
        clipboard.text = text
        clipboard.selectAll()
        clipboard.copy()
        clipboard.text = ""
    }

    // The desktop's colours say whether it is dark.
    SystemPalette {
        id: system
    }

    Theme {
        id: theme

        compact: window.compact
        // SIOUL_THEME=dark or light forces one, for the window's images.
        // SIOUL_THEME for the window's images, else the setting, else the system's colours.
        readonly property string chosen: sioul.forcedTheme() || (sioul.reading ? JSON.parse(sioul.reading).theme : "")
        dark: chosen !== "" ? chosen === "dark" : system.window.hslLightness < 0.5
        readingFamily: sioul.reading ? JSON.parse(sioul.reading).family : ""
        readingSize: sioul.reading ? JSON.parse(sioul.reading).size : 16
        readingSpacing: sioul.reading ? JSON.parse(sioul.reading).spacing : 1.5
    }

    Sioul {
        id: sioul
    }

    // Settings, at one of them: the hours, from the Porch.
    function showParameters(key) {
        window.page = 12
        parametersPage.showSetting(key)
    }

    Component.onCompleted: {
        sioul.mark("the window's own objects made")
        window.makePages()
        sioul.mark("the Porch made")
        sioul.start()
        // The places as this device left them, before the first frame.
        window.placesHidden = sioul.viewFlag("places-hidden")
        window.placesMade = window.placesMade || window.railShown
        sioul.mark("Sioul started")
    }
    // What waits for "Undo" is done before the window goes, not lost.
    onClosing: {
        for (const open of window.drafts)
            open.close()
        if (window.focusWindow) {
            window.focusWindow.quitting = true
            window.focusWindow.close()
        }
        sioul.flush()
    }

    // The window to write in (ComposeWindow.qml): read the first time a draft opens.
    property Component composer: null

    // A routine played, in its own small window on top of the others: made
    // the first time one is played.
    Loader {
        id: routinePlayerLoader

        active: false
        sourceComponent: RoutinePlayer {
            transientParent: null
            sioul: sioul
            theme: theme
            onOpenThing: what => {
                if (what === "porch")
                    window.page = 0
                else if (what.startsWith("sioul:task/"))
                    window.openTask(decodeURIComponent(what.slice("sioul:task/".length)))
                window.raise()
                window.requestActivate()
            }
        }
    }
    readonly property var routinePlayer: routinePlayerLoader.item

    function playRoutine(routine) {
        routinePlayerLoader.active = true
        window.routinePlayer.play(routine)
    }

    // The focus session, in its own small window on top of the others: made
    // while a session runs.
    Loader {
        id: focusWindowLoader

        active: sioul.focusSession !== ""
        sourceComponent: FocusWindow {
            transientParent: null
            sioul: sioul
            theme: theme
            session: sioul.focusSession ? JSON.parse(sioul.focusSession) : null
            onCloseDay: {
                window.page = 1
                window.tasksPage.stopForToday()
                window.raise()
                window.requestActivate()
            }
        }
    }
    readonly property var focusWindow: focusWindowLoader.item

    // The end of the work day ("Done for today") or of the day before sleep
    // (DayReview.qml, docs/reviews.md): made the first time it is asked for.
    Later {
        id: dayReview

        sourceComponent: Component {
            DayReview {
                sioul: sioul
                theme: theme
                window: window
            }
        }
    }
    // What the status line offers now: {kind, date, line, button}, kind "" for nothing.
    readonly property var offer: window.moment.review || ({ kind: "", date: "", line: "", button: "" })
    // The two pauses (docs/pauses.md): Free time's choices and today's end of work.
    readonly property var pauses: window.moment.pauses || ({ nothing: false, usual_end: "", moved_end: "", can_keep: false })
    // The end of work said once, back from free time: "Keep my usual end" shows beside it.
    property string freeSaid: ""

    // Free time on or off: on, the line on where you stopped when a session
    // stopped (GP3); off, the end of work said once, with its key (GP11, GP16).
    function setFreeTime(on) {
        const said = sioul.setFreeTime(on)
        window.freeSaid = on ? "" : said
        if (on && said === "true")
            window.askNeed("")
    }

    // Free time's menu, a right click or a long press on its button: back,
    // the usual end kept, nothing at all; nothing opens when none applies.
    function freeMenu() {
        if (window.moment.reason !== "free-time" && !window.pauses.can_keep)
            return
        modeMenu.active = true
        const menu = modeMenu.item as SioulMenu
        menu.popup()
    }

    // The pause's screen tried from its setup (PauseSetup.qml): nothing held.
    Connections {
        target: window.parametersPage
        ignoreUnknownSignals: true

        function onTryPause() {
            pauseCover.trial = true
        }
    }

    // "work" or "night": the review's sheet.
    function reviewDay(kind) {
        dayReview.now().show(kind)
    }

    // The work day closed from the sheet: the Tasks page's screen says where
    // everything went, over the page shown (the Tasks page made if it was not).
    function showClosing(closing) {
        if (window.made[1] !== true) {
            const made = Object.assign({}, window.made)
            made[1] = true
            window.made = made
        }
        if (window.tasksPage)
            window.tasksPage.showClosing(closing)
    }

    // SIOUL_GRAB=<folder>: each page saved as an image, a message opened on the
    // Porch included, then the window quits. For development and documentation.
    // Made only when asked for (SIOUL_GRAB): a thousand lines of steps, never read otherwise.
    Loader {
        active: sioul.grabFolder() !== ""
        sourceComponent: Component {
        Timer {
            id: grabber

            readonly property string folder: sioul.grabFolder()
            property int step: 0
            // A task the steps marked done (taskform).
            property string doneUid: ""
            // A page is shown at one tick and saved at the next, since an image is
            // taken at the next frame.
            readonly property var steps: ({ "actions": grabber.actions, "pim": grabber.pim, "pgp": grabber.pgp, "tasks": grabber.tasks, "move": grabber.move, "links": grabber.links, "projects": grabber.projects, "map": grabber.map, "duplicates": grabber.duplicates, "sites": grabber.sites, "quiet": grabber.quiet, "folders": grabber.folders, "notes": grabber.notes, "collections": grabber.collections, "health": grabber.health, "movetask": grabber.movetask, "google": grabber.google, "github": grabber.github, "batch-a": grabber.batchA, "export": grabber.exportCsv, "noantivirus": grabber.noAntivirus, "watch": grabber.watch, "share": grabber.share, "share-join": grabber.shareJoin, "share-two": grabber.shareTwo, "parameters": grabber.parameters, "closed-on": grabber.closedOn, "closed-off": grabber.closedOff, "papers": grabber.papers, "budget": grabber.budget, "energy": grabber.energy, "day": grabber.day, "bitwarden-key": grabber.bitwardenKey, "bitwarden-passkey": grabber.bitwardenPasskey, "bitwarden-choose": grabber.bitwardenChoose, "areas": grabber.areas, "presets": grabber.presets, "ownership": grabber.ownership, "zoom": grabber.zoom, "accounts": grabber.accounts, "account-tabs": grabber.accountTabs, "batch-11": grabber.batch11, "contracts": grabber.contracts, "bank": grabber.bankSteps, "porch-money": grabber.porchMoney, "letters": grabber.lettersSteps, "letters-act": grabber.lettersAct, "demo": grabber.demo, "phone": grabber.phone, "drag": grabber.dragSteps, "taskform": grabber.taskForm, "review": grabber.review, "site-open": grabber.siteOpen, "site-quit": grabber.siteQuit, "site-during": grabber.siteDuring, "site-share": grabber.siteShare, "rail": grabber.railSteps, "pauses": grabber.pauseSteps, "blocks": grabber.blockSteps })[sioul.grabSteps()] || grabber.pages
            // The documentation's pictures, on the demo profile (tools/demo/screenshots.sh):
            // each place as it is used, a weekday afternoon. Run again on the profile
            // without hours (make-demo.py --no-hours), where everything comes at once:
            // the Porch asking for them, and the budgets of every area together.
            // In French (make-demo.py --language fr), the same places, under its names.
            readonly property bool demoFrench: sioul.text("qt-locale") === "fr_FR"
            // Each page in a phone's layout (SIOUL_GRAB_STEPS=phone), Settings scrolled too.
            // The Health page's timeline opened readable, as a finger's long press
            // opens it on a thin day (SIOUL_GRAB_STEPS=drag; SIOUL_GRAB_PHONE for a phone's).
            readonly property var dragSteps: [
                () => window.page = 10,
                () => {},
                () => grabber.save("health-day"),
                () => healthPage.showReadable(true),
                () => {},
                () => grabber.save("health-readable"),
                () => healthPage.showReadable(false),
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-week"),
                () => healthPage.showReadable(true),
                () => {},
                () => grabber.save("health-week-readable"),
                () => window.close()
            ]
            // The places (SIOUL_GRAB_STEPS=rail): the column of icons, a tip, the
            // keyboard's focus, the names asked for, hidden (New's menu then at the
            // page's corner), a short window scrolled. On a demo profile: it changes
            // its settings.
            readonly property var railSteps: [
                () => window.page = 1,
                () => {},
                () => grabber.save("rail"),
                // Held longer than a finger's three seconds: the picture comes two steps later.
                () => window.sayRailTip(places.shown.repeater.itemAt(2), true, 8000),
                () => {},
                () => grabber.saveWindow("rail-tip"),
                () => window.sayRailTip(places.shown.repeater.itemAt(2), false, 0),
                () => places.shown.repeater.itemAt(4).forceActiveFocus(Qt.TabFocusReason),
                () => {},
                () => grabber.saveWindow("rail-focus"),
                () => {
                    window.page = 0
                    sioul.setSetting("places_named", "true")
                },
                () => {},
                () => grabber.save("rail-named"),
                () => window.togglePlaces(false),
                () => {},
                () => grabber.save("rail-named-hidden"),
                () => sioul.setSetting("places_named", "false"),
                () => {},
                () => grabber.save("rail-hidden"),
                () => window.showNewMenu(),
                () => {},
                () => grabber.saveWindow("rail-hidden-new"),
                () => newMenu.item.close(),
                () => {
                    window.togglePlaces(false)
                    window.height = 540
                },
                () => {},
                () => grabber.save("rail-short"),
                () => places.shown.repeater.itemAt(11).forceActiveFocus(Qt.TabFocusReason),
                () => {},
                () => grabber.saveWindow("rail-short-end"),
                // The setting, in Settings ▸ Display.
                () => {
                    window.height = window.demoGrab ? 860 : 1500
                    window.page = 12
                    parametersPage.section = "look"
                },
                () => {},
                () => grabber.save("rail-settings"),
                () => window.close()
            ]
            readonly property var phone: [
                () => window.page = 0,
                () => grabber.save("phone-porch"),
                () => window.page = 1,
                () => grabber.save("phone-tasks"),
                () => window.page = 2,
                () => grabber.save("phone-mail"),
                () => window.page = 4,
                () => grabber.save("phone-agenda"),
                () => window.page = 5,
                () => grabber.save("phone-contacts"),
                () => window.page = 7,
                () => grabber.save("phone-projects"),
                () => window.page = 9,
                () => grabber.save("phone-budgets"),
                () => window.page = 11,
                () => grabber.save("phone-accounts"),
                () => window.page = 12,
                () => grabber.save("phone-settings"),
                () => window.parametersPage.scrollBy(0.5),
                () => grabber.save("phone-settings-down"),
                () => window.placesOpen = true,
                () => grabber.save("phone-places"),
                () => window.close()
            ]
            readonly property var demo: [
                () => window.page = 0,
                () => {},
                () => {},
                () => grabber.save(window.moment.hours ? "porch" : "porch-hours"),
                () => {
                    if (!window.moment.hours)
                        window.page = 9
                },
                () => {},
                () => {
                    if (!window.moment.hours)
                        grabber.save("budgets")
                },
                () => {
                    if (!window.moment.hours)
                        budgetsPage.showBank()
                },
                () => {},
                () => {
                    if (!window.moment.hours)
                        grabber.save("bank-accounts")
                },
                () => {
                    if (!window.moment.hours)
                        window.close()
                },
                () => window.page = 2,
                () => {},
                () => grabber.save("mail"),
                // A message open: the client's, this morning's.
                () => mailPage.openSubject(grabber.demoFrench ? "deux petites modifications" : "two small changes"),
                () => {},
                () => grabber.save("mail-reader"),
                () => {
                    window.page = 1
                    tasksPage.mode = "now"
                },
                () => {},
                () => grabber.save("tasks-now"),
                () => tasksPage.mode = "day",
                () => {},
                () => grabber.save("tasks-day"),
                () => tasksPage.mode = "list",
                () => {},
                () => grabber.save("tasks-list"),
                () => {
                    tasksPage.mode = "now"
                    window.page = 4
                    agendaPage.mode = "week"
                },
                () => {},
                () => grabber.save("agenda-week"),
                () => {
                    agendaPage.mode = "agenda"
                    window.page = 5
                },
                () => contactsPage.open((contactsPage.shown.contacts.find(c => c.name.startsWith("Iris")) || contactsPage.shown.contacts[0]).key),
                () => {},
                () => grabber.save("contacts"),
                // Notes as folders, the project's open.
                () => {
                    window.page = 6
                    sioul.setNotesTree(true)
                },
                () => notesPage.unfold(grabber.demoFrench ? "Projets" : "Projects"),
                () => notesPage.open(grabber.demoFrench ? "Projets/Médiathèque des Fougères.md" : "Projects/Fernhill Library.md"),
                () => {},
                () => grabber.save("notes"),
                () => {
                    window.page = 7
                    projectsPage.open("fernhill")
                },
                () => {},
                () => grabber.save("projects"),
                // Time: last week, a whole one, whatever the day (and the hour:
                // shown anyway in quiet time).
                () => {
                    window.page = 8
                    timePage.anyway = true
                    timePage.move(-1)
                },
                () => {},
                () => grabber.save("time"),
                () => window.page = 13,
                () => {},
                () => grabber.save("papers"),
                () => window.page = 10,
                () => {},
                () => grabber.save("health"),
                // Its medicines and prescriptions, after the day (on a phone, below its first screen).
                () => healthPage.showMedicines(),
                () => {},
                () => grabber.save("health-medicines"),
                // Its week, and its settings (the usual meals and night).
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-week"),
                () => healthPage.showDay(),
                () => healthPage.showNeeds(),
                () => {},
                () => grabber.savePopup(healthPage.settingsPanel(), "health-settings"),
                () => healthPage.closeSettings(),
                // Sites: those for these hours, then the others unfolded.
                () => {
                    window.page = 3
                    sitesPage.laterShown = true
                },
                () => {},
                () => grabber.save("sites"),
                // A site's menu, as a right click opens it.
                () => {
                    sitesPage.menuId = (sitesPage.sites.find(s => s.id === "chat") || sitesPage.sites[0]).id
                    sitesPage.siteMenu.popup()
                },
                () => {},
                () => grabber.savePopup(sitesPage.siteMenu, "sites-menu"),
                () => {
                    sitesPage.siteMenu.dismiss()
                    window.page = 11
                },
                () => {},
                () => grabber.save("accounts"),
                () => accountsPage.showTab(2),
                () => {},
                () => grabber.save("accounts-senders"),
                () => {
                    accountsPage.showTab(0)
                    window.page = 12
                    parametersPage.section = "hours"
                },
                () => {},
                () => grabber.save("settings-hours"),
                // Work shown whatever the hours, from the Porch.
                () => {
                    window.page = 0
                    sioul.setWorkNow(true)
                },
                () => {},
                () => grabber.save("work-now"),
                () => sioul.setWorkNow(false),
                () => window.close()
            ]
            // A site's page closed as Sioul quits (tools/check-sites.py quit, on a demo
            // profile): the test site open, then nothing (a signal ends Sioul), or the
            // window closed some fifteen seconds later.
            readonly property var siteOpen: [
                () => window.page = 3,
                () => sitesPage.open("test-chat")
            ]
            readonly property var siteQuit: grabber.siteOpen.concat(Array(8).fill(() => {}), [() => window.close()])
            // Sharing the screen from the test site (tools/check-sites.py share): Qt
            // WebEngine's screen capture off (as Sioul had it), then on, then the site's
            // switch off; each time the page asks, the first screen chosen as you would.
            readonly property var siteShare: [
                () => window.page = 3,
                () => sitesPage.open("test-chat"),
                () => sitesPage.viewOf("test-chat").settings.screenCaptureEnabled = false,
                () => sitesPage.viewOf("test-chat").runJavaScript("share('capture-off')"),
                () => {},
                () => sitesPage.shareChosen(true, 0),
                () => {},
                () => sitesPage.viewOf("test-chat").settings.screenCaptureEnabled = true,
                () => sitesPage.viewOf("test-chat").runJavaScript("share('capture-on')"),
                () => {},
                () => sitesPage.shareChosen(true, 0),
                () => {},
                () => {
                    sitesPage.problem = sioul.setSite("test-chat", "screen", "false")
                    sitesPage.reload()
                },
                () => sitesPage.viewOf("test-chat").runJavaScript("share('switch-off')"),
                () => {},
                () => console.warn("sioul-share: the line above the site says: " + sitesPage.problem),
                () => window.close()
            ]
            // A site's pages closed while Sioul runs: its pop-up closed as you close a
            // window, then the site taken out of Sioul, then the window closed.
            readonly property var siteDuring: grabber.siteOpen.concat(Array(4).fill(() => {}), [
                () => sitesPage.popups.forEach(p => {
                    if (sitesPage.popupOpen(p))
                        p.close()
                }),
                () => {},
                () => {},
                () => {
                    sioul.removeSite("test-chat")
                    sitesPage.reload()
                },
                () => {},
                () => {},
                () => {},
                () => window.close()
            ])
            // Sites, on a local test page: opened, its notification kept, the Porch telling it.
            readonly property var sites: [
                () => window.page = 0,
                () => {},
                () => {},
                () => {},
                () => grabber.save("sites-porch"),
                () => window.page = 3,
                () => sitesPage.open("test-chat"),
                () => {},
                () => sitesPage.fillLogin(),
                () => grabber.save("sites"),
                () => sitesPage.fillWith(sitesPage.viewOf("test-chat"), "jane@example.org", "not-a-real-password"),
                () => sitesPage.viewOf("test-chat").runJavaScript("document.title = document.querySelector('input[type=email]').value + ' / ' + document.querySelector('input[type=password]').value.length"),
                () => console.warn("filled: " + sitesPage.viewOf("test-chat").title),
                () => window.close()
            ]
            // The contacts' map, on invented contacts: their addresses placed, the map, a card.
            readonly property var map: [
                () => window.page = 5,
                () => contactsPage.mapShown = true,
                () => grabber.save("map-ask"),
                () => sioul.locateAddresses(true),
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => grabber.save("map"),
                () => contactsPage.openFirst(),
                () => {},
                () => {},
                () => grabber.save("map-contact"),
                () => window.close()
            ]
            // The contacts' duplicates and categories (SIOUL_GRAB_STEPS=duplicates), on
            // invented cards in a test profile: the view, cards cleaned, a pair merged,
            // both undone; a card with its categories, its form, the list of one category.
            readonly property var duplicates: [
                () => window.page = 5,
                () => grabber.save("contacts-list"),
                () => contactsPage.showDuplicates(),
                () => {},
                () => {},
                () => grabber.save("duplicates"),
                () => contactsPage.duplicatesNow().clean(),
                () => {},
                () => {},
                () => grabber.save("duplicates-cleaned"),
                () => contactsPage.duplicatesNow().merge(),
                () => {},
                () => {},
                () => grabber.save("duplicates-merged"),
                () => contactsPage.duplicatesNow().undoLast(),
                () => {},
                () => contactsPage.duplicatesNow().undoLast(),
                () => {},
                () => {},
                () => grabber.save("duplicates-undone"),
                () => contactsPage.open((contactsPage.shown.contacts.find(c => c.name.startsWith("Jean")) || contactsPage.shown.contacts[0]).key),
                () => grabber.save("contact-categories"),
                () => {
                    contactsPage.formNow().load(contactsPage.person)
                    contactsPage.editing = true
                },
                () => grabber.save("contact-form"),
                () => contactsPage.back(),
                () => contactsPage.back(),
                () => sioul.showContactsCategory("amis"),
                () => {},
                () => grabber.save("contacts-category"),
                () => window.close()
            ]
            // Projects, time and invoices, on invented data: a project's page, its invoice made, the Time page.
            readonly property var projects: [
                () => window.page = 7,
                () => projectsPage.openFirst(),
                () => grabber.save("projects"),
                () => projectsPage.routesShown = true,
                () => grabber.save("projects-routes"),
                () => projectsPage.routesShown = false,
                () => projectsPage.makeInvoice(),
                () => {},
                () => grabber.save("projects-invoiced"),
                () => window.page = 8,
                () => {},
                () => grabber.save("time"),
                () => timePage.period = "month",
                () => grabber.save("time-month"),
                () => window.page = 9,
                () => budgetsPage.openFirst(),
                () => {},
                () => grabber.save("budget-work"),
                () => window.close()
            ]
            // Ties by hand, on invented tasks and notes: one made through the picker,
            // seen from both ends, undone; then a task made from a note.
            readonly property var links: [
                () => window.page = 1,
                () => tasksPage.mode = "list",
                () => tasksPage.openFirst(),
                () => grabber.save("links-task"),
                () => window.linkFrom({ uri: "sioul:task/" + encodeURIComponent(tasksPage.opened), kind: "task", key: tasksPage.opened, title: "" }),
                () => linkPicker.item.type("lease"),
                () => linkPicker.item.grab(grabber.folder + "/links-picker.png"),
                () => linkPicker.item.chooseFirst(),
                () => {},
                () => grabber.save("links-task-linked"),
                () => window.openNote("admin/lease.md"),
                () => {},
                () => grabber.save("links-note"),
                () => sioul.unlinkThings("sioul:note/admin/lease.md", "sioul:task/" + encodeURIComponent(tasksPage.opened)),
                () => {},
                () => grabber.save("links-note-unlinked"),
                () => window.addLinked("task", { uri: "sioul:note/admin/lease.md", kind: "note", key: "admin/lease.md", title: "Lease" }),
                () => grabber.save("links-form"),
                () => tasksPage.panel.confirmTitle(""),
                () => {},
                () => grabber.save("links-made"),
                () => window.close()
            ]
            // Moving mail, for tests against a test server with two accounts: conversations,
            // a selection, a move to the other account.
            readonly property var move: [
                () => window.page = 2,
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => mailPage.openFolder("alice", "INBOX"),
                () => {},
                () => grabber.save("move-threads"),
                () => mailPage.unfoldConversations(),
                () => grabber.save("move-threads-open"),
                () => mailPage.selectBySubjects(["Lunch", "order"]),
                () => grabber.save("move-selected"),
                () => mailPage.moveSelection("bob", "INBOX"),
                () => grabber.save("move-waiting"),
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => mailPage.openFolder("bob", "INBOX"),
                () => {},
                () => {},
                () => {},
                () => grabber.save("move-bob"),
                () => window.close()
            ]
            // A new task's full form (New ▸ A task, then Add ▾ ▸ A task from a
            // note), made once its title is given; the costs and the gain as
            // sliders in its panel, then in an event's form. With SIOUL_GRAB_PHONE,
            // a phone's. On a scratch profile: it makes tasks and saves ratings.
            readonly property var taskForm: [
                () => window.page = 1,
                () => {},
                () => window.newThing("task"),
                () => {},
                () => grabber.save("taskform-new"),
                () => tasksPage.panel.confirmTitle(grabber.demoFrench ? "Appeler la banque" : "Call the bank"),
                () => {},
                () => {},
                () => grabber.save("taskform-made"),
                () => tasksPage.panel.showRatings(),
                () => {},
                () => grabber.save("taskform-unrated"),
                () => {
                    tasksPage.panel.rate("anxiety", 7)
                    tasksPage.panel.rate("gain", 0)
                },
                () => {},
                () => {},
                () => tasksPage.panel.showRatings(),
                () => grabber.save("taskform-rated"),
                () => tasksPage.closePanel(),
                // From the client's message of this morning (its subject, sender and link), else a note.
                () => {
                    const found = JSON.parse(sioul.searchThings(grabber.demoFrench ? "deux petites modifications" : "two small changes", "mail", "") || "[]").concat(JSON.parse(sioul.searchThings("", "note", "") || "[]"))
                    console.warn("taskform: from " + (found.length > 0 ? found[0].kind + " " + found[0].title : "nothing"))
                    if (found.length > 0)
                        window.addLinked("task", { uri: found[0].uri, kind: found[0].kind, key: found[0].key || "", title: found[0].title || "" })
                    else
                        window.newThing("task")
                },
                () => {},
                () => grabber.save("taskform-linked"),
                () => tasksPage.panel.showRatings(),
                () => {},
                () => grabber.save("taskform-linked-down"),
                // Closed with its title as given: made. Then a form closed empty: nothing made.
                () => tasksPage.closePanel(),
                () => window.newThing("task"),
                () => tasksPage.closePanel(),
                () => tasksPage.mode = "list",
                () => {},
                () => {},
                () => grabber.save("taskform-list"),
                () => {
                    tasksPage.mode = "now"
                    window.page = 4
                    agendaPage.newEvent(agendaPage.today())
                },
                () => agendaPage.fillEvent(grabber.demoFrench ? "Rendez-vous à la mairie" : "Town hall appointment", "10:00", "11:00", ""),
                () => {},
                // The form over the window: the overlay's one child narrower than it (not its dimmer).
                () => {
                    const over = frame.Overlay.overlay
                    for (const item of over.children)
                        if (item.visible && item.width < over.width)
                            item.grabToImage(result => result.saveToFile(grabber.folder + "/taskform-event.png"))
                },
                () => agendaPage.saveEvent(),
                // A task's details first, then "Edit": its form.
                () => {
                    window.page = 1
                    tasksPage.mode = "now"
                },
                () => tasksPage.openFirst(),
                () => {},
                () => grabber.save("taskdetails"),
                () => {
                    tasksPage.panel.editing = true
                    tasksPage.panel.moreShown = true
                },
                () => {},
                () => grabber.save("taskdetails-edit"),
                // "I dread it": two minutes offered on Now's card, nothing started.
                () => {
                    tasksPage.closePanel()
                    tasksPage.twoOffered = tasksPage.shown.now.now ? tasksPage.shown.now.now.uid : ""
                },
                () => {},
                () => grabber.save("taskdetails-hard"),
                () => console.warn("taskform: a focus session after the answer: " + (sioul.focusSession !== "" ? "yes" : "no")),
                // Done: "How was it?" on the status line; its five sliders; a felt rating kept.
                () => {
                    const found = JSON.parse(sioul.searchTasks(grabber.demoFrench ? "Appeler la banque" : "Call the bank", "") || "[]")
                    grabber.doneUid = found.length > 0 ? found[0].uid : ""
                    console.warn("taskform: marking done " + grabber.doneUid)
                    if (grabber.doneUid !== "")
                        sioul.setTaskStatus(grabber.doneUid, "completed")
                },
                () => {},
                () => grabber.save("taskdetails-done"),
                () => window.howWasIt(window.lastDone),
                () => {},
                () => grabber.savePopup(howWasItForm.item, "taskdetails-felt"),
                () => howWasItForm.item.keep({ cognitive: null, emotional: null, anxiety: 3, body: null, gain: 6 }),
                () => {},
                () => grabber.savePopup(howWasItForm.item, "taskdetails-felt-kept"),
                () => howWasItForm.item.close(),
                () => tasksPage.open(grabber.doneUid),
                () => tasksPage.panel.feltShown = true,
                () => {},
                () => grabber.save("taskdetails-done-panel"),
                () => window.close()
            ]
            // The task pages only: nothing else is opened, no message marked read.
            readonly property var tasks: [
                () => window.page = 1,
                () => grabber.save("tasks-now"),
                () => tasksPage.mode = "list",
                () => grabber.save("tasks-list"),
                () => tasksPage.mode = "board",
                () => grabber.save("tasks-board"),
                () => tasksPage.mode = "timeline",
                () => grabber.save("tasks-timeline"),
                () => tasksPage.openFirst(),
                () => grabber.save("tasks-panel"),
                () => window.close()
            ]
            // A task moved to another list on a test server, then sent.
            readonly property var movetask: [
                () => sioul.status = sioul.moveTask("life-ansel-done", "agenda/errands", false),
                () => sioul.syncNow(),
                () => {},
                () => {},
                () => {},
                () => window.close()
            ]
            // Google: the account form with its steps, and a task of a Google Tasks list.
            readonly property var google: [
                () => window.page = 11,
                () => accountsPage.showGoogle(),
                () => {},
                () => grabber.save("google-account"),
                () => {
                    window.page = 1
                    tasksPage.openDetails("t2")
                },
                () => {},
                () => grabber.save("google-task"),
                () => tasksPage.openDetails("t9"),
                () => {},
                () => grabber.save("local-task"),
                () => window.close()
            ]
            // The New menu, a contact's picture, a task's fields, the notes' tree
            // unfolded below the fold, the sites' column wide and narrow.
            readonly property var batchA: [
                () => window.showNewMenu(),
                () => newMenu.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/new-menu.png")),
                () => newMenu.item.close(),
                () => window.page = 5,
                () => contactsPage.openFirst(),
                () => grabber.save("contact"),
                () => {
                    window.page = 1
                    tasksPage.mode = "list"
                },
                () => tasksPage.openDetails("t1"),
                () => grabber.save("task-panel"),
                () => {
                    window.page = 6
                    sioul.setNotesTree(true)
                },
                () => notesPage.unfold("notes"),
                () => notesPage.unfold("notes/zz-archive"),
                () => notesPage.scrollList(900),
                () => grabber.save("notes-before"),
                () => notesPage.unfold("notes/admin"),
                () => grabber.save("notes-after"),
                () => window.page = 3,
                () => grabber.save("sites-wide"),
                () => sitesPage.narrow = true,
                () => grabber.save("sites-narrow"),
                () => window.close()
            ]
            // A watch's day read from its folder: the Health page, then the Tasks page's morning.
            readonly property var watch: [
                () => sioul.refreshMode(),
                () => {},
                () => {},
                () => window.page = 10,
                () => {},
                () => grabber.save("health-watch"),
                () => window.page = 1,
                () => grabber.save("tasks-morning"),
                () => window.close()
            ]
            // Reminders with the window closed: the session's entry written and the watcher started; then stopped.
            readonly property var closedOn: [
                () => sioul.status = sioul.setSetting("reminders_closed", "true"),
                () => {},
                () => window.close()
            ]
            readonly property var closedOff: [
                () => sioul.status = sioul.setSetting("reminders_closed", "false"),
                () => {},
                () => window.close()
            ]
            // Each letter's date as a task, then each done and filed.
            readonly property var lettersAct: [
                () => JSON.parse(sioul.letters()).letters.forEach(l => sioul.status = sioul.letterTask(l.id)),
                () => {},
                () => JSON.parse(sioul.letters()).letters.forEach(l => sioul.status = sioul.letterDone(l.id, "")),
                () => {},
                () => window.close()
            ]
            // Paper letters read from the scans in the inbox, then their cards in the open Porch.
            readonly property var lettersSteps: [
                () => sioul.refreshMode(),
                () => sioul.openAnyway(),
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => window.page = 1,
                () => window.page = 0,
                () => {},
                () => grabber.save("letters"),
                () => window.close()
            ]
            // The Porch open, its money line.
            readonly property var porchMoney: [
                () => sioul.openAnyway(),
                () => window.page = 1,
                () => window.page = 0,
                () => {},
                () => grabber.save("porch-money"),
                () => window.close()
            ]
            // A bank export taken in (the grab's folder holds it), then the Budgets page.
            readonly property var bankSteps: [
                () => window.page = 9,
                () => sioul.status = JSON.parse(sioul.importBank("file://" + grabber.folder + "/../releve-banque.csv")).said,
                () => budgetsPage.contracts.reload(),
                () => window.page = 0,
                () => window.page = 9,
                () => {},
                () => grabber.save("bank"),
                () => window.close()
            ]
            // Bitwarden's second step with a security key: offered first and asked at once,
            // from Bitwarden's page held unseen in the dialog (a passkey login's options from
            // Bitwarden's cloud stand in for a second step's: same shape, no account); then
            // the key not used in time, said, with the button to ask it again.
            readonly property var bitwardenKey: [
                () => window.page = 3,
                () => sitesPage.bitwardenUnlock.begin(),
                () => {
                    const begun = JSON.parse(sioul.bitwardenPasskeyBegin())
                    sitesPage.bitwardenUnlock.answer({ factor: [1, 7], key: { page: begun.page, script: begun.script.replace('})("passkey", ', '})("factor", ') } })
                },
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => sitesPage.bitwardenKeyView.item.runJavaScript("location.href + ' | ' + document.readyState + ' | focused: ' + document.hasFocus() + ' | asked: ' + (typeof window.sioulKey)", said => sitesPage.bitwardenUnlock.note = "Page: " + said),
                () => {},
                () => sitesPage.bitwardenUnlock.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/key-asking.png")),
                () => sitesPage.bitwardenKeyView.item.runJavaScript("window.sioulKey = () => JSON.stringify({ error: 'not-allowed' })"),
                () => {},
                () => {},
                () => {},
                () => sitesPage.bitwardenUnlock.note = "Page held: " + (sitesPage.bitwardenKeyView.item !== null),
                () => sitesPage.bitwardenUnlock.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/key-refused.png")),
                () => window.close()
            ]
            // Bitwarden opened by the security key alone: the dialog, then the key asked at
            // once from Bitwarden's page held unseen (real options from Bitwarden's cloud,
            // asked of no account); then an answer without the key's secret, as a key without
            // PRF gives: refused here, before Bitwarden is asked.
            readonly property var bitwardenPasskey: [
                () => window.page = 3,
                () => sitesPage.bitwardenUnlock.begin(),
                () => {},
                () => sitesPage.bitwardenUnlock.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/passkey-dialog.png")),
                () => sitesPage.bitwardenUnlock.usePasskey(),
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => sitesPage.bitwardenKeyView.item.runJavaScript("location.href + ' | ' + document.readyState + ' | focused: ' + document.hasFocus() + ' | asked: ' + (typeof window.sioulKey)", said => sitesPage.bitwardenUnlock.note = "Page: " + said),
                () => {},
                () => sitesPage.bitwardenUnlock.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/passkey-asking.png")),
                () => sitesPage.bitwardenKeyView.item.runJavaScript("window.sioulKey = () => JSON.stringify({ response: { id: 'test' }, secret: '' })"),
                () => {},
                () => {},
                () => {},
                () => sitesPage.bitwardenUnlock.note = "Page held: " + (sitesPage.bitwardenKeyView.item !== null),
                () => sitesPage.bitwardenUnlock.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/passkey-no-secret.png")),
                () => window.close()
            ]
            // The login chooser, on a made-up vault (SIOUL_TEST_VAULT): a site's accounts
            // by its domain, then by a user name alone, then by both fields; then the one
            // chosen last proposed first.
            readonly property var bitwardenChoose: [
                () => window.page = 3,
                () => sioul.bitwardenState(),
                () => sitesPage.loginChooser.now().begin("https://www.example.org/login"),
                () => {},
                () => sitesPage.loginChooser.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/choose-site.png")),
                () => {
                    sitesPage.loginChooser.item.site = ""
                    sitesPage.loginChooser.item.user = "me@"
                    sitesPage.loginChooser.item.search()
                },
                () => {},
                () => sitesPage.loginChooser.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/choose-user.png")),
                () => {
                    sitesPage.loginChooser.item.site = "bank"
                    sitesPage.loginChooser.item.search()
                },
                () => {},
                () => sitesPage.loginChooser.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/choose-both.png")),
                () => sitesPage.loginChooser.close(),
                () => sioul.bitwardenLogin("https://www.example.org/login", "b"),
                () => sitesPage.loginChooser.now().begin("https://www.example.org/login"),
                () => {},
                () => sitesPage.loginChooser.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/choose-last-first.png")),
                () => window.close()
            ]
            // Areas and hours (docs/areas.md), on a test week whose Sunday is leisure: the
            // sites of these hours and the others folded, by kind; the presets to add one;
            // doses due while Sioul was closed; the week's three kinds of hours; a call
            // to an office with its own opening hours.
            readonly property var areas: [
                () => window.page = 3,
                () => {},
                () => {},
                () => grabber.save("areas-sites"),
                () => sitesPage.laterShown = true,
                () => {},
                () => grabber.save("areas-sites-later"),
                () => sitesPage.byCategory = true,
                () => {},
                () => grabber.save("areas-sites-sorted"),
                () => sitesPage.addSiteDialog.open(),
                () => {},
                () => {},
                () => sitesPage.addSiteDialog.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/areas-add.png")),
                () => sitesPage.addSiteFind = "banque",
                () => {},
                () => sitesPage.addSiteDialog.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/areas-add-bank.png")),
                () => sitesPage.addSiteDialog.close(),
                () => window.page = 10,
                () => {},
                () => {},
                () => grabber.save("areas-health"),
                () => window.page = 12,
                () => {},
                () => parametersPage.scrollBy(0.33),
                () => {},
                () => grabber.save("areas-hours"),
                () => parametersPage.scrollBy(0.33),
                () => {},
                () => grabber.save("areas-hours-2"),
                () => window.openTask("office-call"),
                () => {},
                () => tasksPage.showPanelDetails(),
                () => {},
                () => grabber.save("areas-task"),
                () => window.close()
            ]
            // The usual sites' menu: countries, a country's groups, a group's sites;
            // a group pinned at once; the list's filters, and the list filtered.
            readonly property var presets: [
                () => window.page = 3,
                () => {},
                () => sitesPage.showPresets(),
                () => {},
                () => grabber.savePopup(sitesPage.presetsMenu, "presets-menu"),
                () => sitesPage.presetsMenu.itemAt(1).triggered(),
                () => {},
                () => grabber.savePopup(sitesPage.presetsMenu.menuAt(1), "presets-country"),
                () => sitesPage.presetsMenu.menuAt(1).itemAt(sitesPage.presetsMenu.menuAt(1).count - 1).triggered(),
                () => {},
                () => grabber.savePopup(sitesPage.presetsMenu.menuAt(1).menuAt(sitesPage.presetsMenu.menuAt(1).count - 1), "presets-states"),
                () => sitesPage.presetsMenu.menuAt(1).menuAt(sitesPage.presetsMenu.menuAt(1).count - 1).itemAt(2).triggered(),
                () => {},
                () => grabber.savePopup(sitesPage.presetsMenu.menuAt(1).menuAt(sitesPage.presetsMenu.menuAt(1).count - 1).menuAt(2), "presets-state"),
                () => sitesPage.presetsMenu.dismiss(),
                () => sitesPage.showPresets(),
                () => sitesPage.presetsMenu.itemAt(2).triggered(),
                () => sitesPage.presetsMenu.menuAt(2).itemAt(2).triggered(),
                () => {},
                () => grabber.savePopup(sitesPage.presetsMenu.menuAt(2).menuAt(2), "presets-group"),
                () => sitesPage.presetsMenu.menuAt(2).menuAt(2).itemAt(0).triggered(),
                () => {},
                () => grabber.save("presets-pinned"),
                () => sitesPage.showPresets(),
                () => sitesPage.presetsMenu.itemAt(2).triggered(),
                () => sitesPage.presetsMenu.menuAt(2).itemAt(2).triggered(),
                () => {},
                () => grabber.savePopup(sitesPage.presetsMenu.menuAt(2).menuAt(2), "presets-group-kept"),
                () => sitesPage.presetsMenu.dismiss(),
                () => sitesPage.showFilters(),
                () => {},
                () => grabber.savePopup(sitesPage.filterMenu, "presets-filters"),
                () => sitesPage.filterMenu.dismiss(),
                () => sitesPage.kindFilter = "mailbox",
                () => {},
                () => grabber.save("presets-filtered"),
                () => window.close()
            ]
            // Who owns which setting: the Porch asking for the hours, its own settings,
            // "Work now"; an address's own settings on its card; the hours in Settings.
            readonly property var ownership: [
                () => window.page = 0,
                () => {},
                () => {},
                () => grabber.save("own-porch"),
                () => porchPage.openSettings(true),
                () => {},
                () => porchPage.grabSettings(grabber.folder + "/own-porch-settings.png"),
                () => porchPage.openSettings(false),
                () => sioul.setWorkNow(true),
                () => {},
                () => grabber.save("own-work-now"),
                () => sioul.setWorkNow(false),
                () => window.page = 11,
                () => {},
                () => accountsPage.openFirstSettings(),
                () => {},
                () => grabber.save("own-accounts"),
                () => window.showParameters("window"),
                () => {},
                () => grabber.save("own-hours"),
                () => window.close()
            ]
            // The day's steps and events, nothing over anything; the agenda's day and
            // week fitted to the hours given to something, and the events outside them.
            readonly property var zoom: [
                () => window.page = 1,
                () => tasksPage.mode = "day",
                () => {},
                () => {},
                () => grabber.save("zoom-day"),
                () => window.page = 4,
                () => agendaPage.mode = "week",
                () => {},
                () => {},
                () => grabber.save("zoom-week"),
                () => agendaPage.move(1),
                () => {},
                () => {},
                () => grabber.save("zoom-next-week"),
                () => agendaPage.mode = "day",
                () => {},
                () => {},
                () => grabber.save("zoom-agenda-day"),
                () => window.close()
            ]
            // Bank accounts on the Budgets page: the cards, one's movements placed,
            // its rules, its form; a reserve's form; the budgets counting them.
            readonly property var accounts: [
                () => window.page = 9,
                () => {},
                () => grabber.save("accounts-budgets"),
                () => budgetsPage.showBank(),
                () => {},
                () => grabber.save("accounts-cards"),
                () => budgetsPage.bank.unfold("bank"),
                () => {},
                () => budgetsPage.showBank(),
                () => {},
                () => grabber.save("accounts-movements"),
                () => budgetsPage.bank.rulesDialog.now().show(budgetsPage.bank.shown.accounts[0]),
                () => {},
                () => grabber.savePopup(budgetsPage.bank.rulesDialog.item, "accounts-rules"),
                () => budgetsPage.bank.rulesDialog.close(),
                () => budgetsPage.bank.accountDialog.now().edit(budgetsPage.bank.shown.accounts[0]),
                () => {},
                () => grabber.savePopup(budgetsPage.bank.accountDialog.item, "accounts-form"),
                () => budgetsPage.bank.accountDialog.close(),
                () => budgetsPage.bank.reserveDialog.now().edit(budgetsPage.bank.shown.reserves[1]),
                () => {},
                () => grabber.savePopup(budgetsPage.bank.reserveDialog.item, "accounts-reserve"),
                () => budgetsPage.bank.reserveDialog.close(),
                () => window.close()
            ]
            // Accounts in tabs: an address with its services (one switched off), adding
            // one, the senders' lists, the keys.
            readonly property var accountTabs: [
                () => window.page = 11,
                () => {},
                () => {},
                () => grabber.save("account-tabs-yours"),
                () => sioul.setAccountEnabled("example-agenda", false),
                () => {},
                () => {},
                () => grabber.save("account-tabs-off"),
                () => accountsPage.showTab(1),
                () => {},
                () => grabber.save("account-tabs-add"),
                () => accountsPage.showTab(2),
                () => {},
                () => grabber.save("account-tabs-senders"),
                () => accountsPage.showTab(3),
                () => {},
                () => grabber.save("account-tabs-keys"),
                () => window.close()
            ]
            // Settings in tabs; the Health page in its order; a site's menu by a right
            // click, and the devices of calls.
            readonly property var batch11: [
                () => window.page = 12,
                () => {},
                () => grabber.save("b11-settings"),
                () => parametersPage.section = "hours",
                () => {},
                () => grabber.save("b11-settings-hours"),
                () => window.page = 10,
                () => {},
                () => grabber.save("b11-health"),
                () => window.page = 3,
                () => {},
                () => {
                    sitesPage.menuId = sitesPage.sites.length > 1 ? sitesPage.sites[1].id : ""
                    sitesPage.siteMenu.popup()
                },
                () => {},
                () => grabber.savePopup(sitesPage.siteMenu, "b11-site-menu"),
                () => sitesPage.siteMenu.dismiss(),
                () => sitesPage.devicesDialog.open(),
                () => {},
                () => grabber.savePopup(sitesPage.devicesDialog, "b11-devices"),
                () => sitesPage.devicesDialog.close(),
                () => window.page = 11,
                () => {},
                () => grabber.save("b11-accounts"),
                () => window.close()
            ]
            // Contracts on the Budgets page, one open, the letter drafted.
            readonly property var contracts: [
                () => window.page = 9,
                () => {},
                () => budgetsPage.openContract("__none__"),
                () => grabber.save("budgets-contracts"),
                () => budgetsPage.openContract("assurance-habitation"),
                () => {},
                () => budgetsPage.contracts.dialog.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/contract-dialog.png")),
                () => window.close()
            ]
            // The day seen, the routines, one played.
            // The end of the work day and of the day (docs/reviews.md): the sheets
            // as they open and with answers, the screen after closing, the status
            // line, the night's sheet with the notes of the last days, the Health page.
            // The two pauses (docs/pauses.md): free time on, its menu, back; the pause's
            // screen, its list and numbers open, coming back; the setup and its try-out.
            // A task pinned to a time (SIOUL_GRAB_STEPS=blocks; on a demo profile: it
            // writes a block): its details, "Do at…", pinned two hours from now for 45
            // minutes, the day with it in its block, the agenda's week, then left to the plan.
            property string blockUid: ""
            readonly property var blockSteps: [
                () => window.page = 1,
                () => {},
                () => {
                    const next = tasksPage.shown && tasksPage.shown.now && tasksPage.shown.now.now ? tasksPage.shown.now.now.uid : ""
                    grabber.blockUid = next
                    console.warn("blocks: " + (next === "" ? "no next step" : next))
                    if (next !== "")
                        tasksPage.open(next)
                },
                () => {},
                () => grabber.save("blocks-panel"),
                () => tasksPage.panel.askDoAt(),
                () => {},
                // The form over the window: the overlay's one child narrower than it (not its dimmer).
                () => {
                    const over = frame.Overlay.overlay
                    for (const item of over.children) {
                        if (item.visible && item.width < over.width) {
                            item.grabToImage(result => result.saveToFile(grabber.folder + "/blocks-do-at.png"))
                            if (window.phoneGrab)
                                grabber.overflow("blocks-do-at", item)
                        }
                    }
                },
                () => {
                    const d = new Date()
                    d.setHours(d.getHours() + 2, 0, 0, 0)
                    const pad = n => n < 10 ? "0" + n : String(n)
                    console.warn("blocks: pinned " + tasksPage.panel.pinAt(d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()) + "T" + pad(d.getHours()) + ":00", 45))
                },
                () => {},
                () => {},
                () => grabber.save("blocks-pinned"),
                () => {
                    tasksPage.closePanel()
                    tasksPage.mode = "list"
                },
                () => {},
                () => grabber.save("blocks-list"),
                () => tasksPage.mode = "day",
                () => {},
                () => {},
                () => grabber.save("blocks-day"),
                () => {
                    window.page = 4
                    agendaPage.mode = "week"
                },
                () => {},
                () => {},
                () => grabber.save("blocks-agenda"),
                () => {
                    agendaPage.mode = "agenda"
                    window.page = 1
                    tasksPage.mode = "now"
                    tasksPage.open(grabber.blockUid)
                },
                () => console.warn("blocks: left " + sioul.setTaskAt(grabber.blockUid, "")),
                () => {},
                () => {},
                () => grabber.save("blocks-left"),
                () => window.close()
            ]
            readonly property var pauseSteps: [
                () => window.page = 1,
                () => window.setFreeTime(true),
                () => {},
                () => grabber.save("pauses-free"),
                () => window.freeMenu(),
                () => {},
                () => grabber.savePopup(modeMenu.item, "pauses-free-menu"),
                () => modeMenu.item.dismiss(),
                () => window.setFreeTime(false),
                () => {},
                () => grabber.save("pauses-free-back"),
                // The pause set up on the demo profile: a list, a line, the breathing guide.
                () => sioul.setSetting("pause.helps", JSON.stringify(grabber.demoFrench ? ["Le casque, et la playlist bleue https://music.example/bleue", "Un thé dans la cuisine"] : ["Headphones on, the blue playlist https://music.example/blue", "Tea in the kitchen"])),
                () => sioul.setSetting("pause.grounding", JSON.stringify(grabber.demoFrench ? "Les pieds sur le sol." : "Feet on the floor.")),
                () => sioul.setSetting("pause.breathing", JSON.stringify(true)),
                () => sioul.pauseNow(),
                () => {},
                () => {},
                () => grabber.save("pauses-paused"),
                () => {
                    pauseCover.item.listOpen = true
                    pauseCover.item.numbersOpen = true
                    pauseCover.item.guideRunning = true
                },
                () => {},
                () => grabber.save("pauses-paused-open"),
                () => pauseCover.item.comeBack(),
                () => {},
                () => {},
                () => grabber.save("pauses-back"),
                () => pauseCover.item.done(),
                () => window.showParameters("pause.doses"),
                () => {},
                () => {},
                () => grabber.save("pauses-setup"),
                () => window.parametersPage.scrollBy(0.5),
                () => {},
                () => grabber.save("pauses-setup-more"),
                () => window.parametersPage.scrollBy(1),
                () => {},
                () => grabber.save("pauses-setup-end"),
                () => pauseCover.trial = true,
                () => {},
                () => grabber.save("pauses-try"),
                () => pauseCover.item.done(),
                () => window.close()
            ]
            readonly property var review: [
                () => {
                    sioul.setWeather("haze")
                    window.page = 1
                },
                () => {},
                () => grabber.save("review-offer"),
                () => tasksPage.stopForToday(),
                () => {},
                () => grabber.savePopup(dayReview.item, "review-work"),
                () => {
                    dayReview.item.felt = "heavy"
                    dayReview.item.mix = "about_right"
                    dayReview.item.write("The bank called back: **done**.\n\n- the form sent\n- the rest on Thursday")
                },
                () => {},
                () => grabber.savePopup(dayReview.item, "review-work-said"),
                () => dayReview.item.previewing = true,
                () => {},
                () => grabber.savePopup(dayReview.item, "review-work-preview"),
                () => dayReview.item.closeIt(),
                () => {},
                () => {},
                () => tasksPage.grabStop(grabber.folder + "/review-closing.png"),
                () => tasksPage.closeStop(),
                () => {},
                () => grabber.save("review-status"),
                () => window.reviewDay("night"),
                () => {},
                () => grabber.savePopup(dayReview.item, "review-night"),
                () => dayReview.item.backShown = true,
                () => {},
                () => grabber.savePopup(dayReview.item, "review-night-back"),
                () => dayReview.item.close(),
                () => window.page = 10,
                () => {},
                () => {},
                () => grabber.save("review-health"),
                () => sioul.usualHours(),
                () => window.close()
            ]
            readonly property var day: [
                () => window.page = 1,
                () => tasksPage.mode = "day",
                () => {},
                () => grabber.save("day"),
                () => tasksPage.routines.now().show(),
                () => {},
                () => tasksPage.routines.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/routines.png")),
                () => tasksPage.routines.close(),
                () => window.playRoutine(JSON.parse(sioul.routines())[0]),
                () => {},
                () => {},
                () => routinePlayer.face.grabToImage(result => result.saveToFile(grabber.folder + "/routine-player.png")),
                () => routinePlayer.stop(),
                () => window.close()
            ]
            // What tasks take: the offer after a heavy one, and the choice in a task's panel.
            readonly property var energy: [
                () => window.page = 1,
                () => {},
                () => grabber.save("energy-now"),
                () => window.openTask("heavy1"),
                () => {},
                () => grabber.save("energy-task"),
                () => window.close()
            ]
            // The time budget: on the Now view, and in a task's panel.
            readonly property var budget: [
                () => window.page = 1,
                () => {},
                () => grabber.save("budget-now"),
                () => window.openTask("t1"),
                () => {},
                () => grabber.save("budget-task"),
                () => window.close()
            ]
            // The papers wallet: by family, a paper's form, a renewal planned.
            readonly property var papers: [
                () => window.page = 13,
                () => {},
                () => grabber.save("papers"),
                () => papersPage.open("passeport"),
                () => {},
                () => papersPage.dialog.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/paper-dialog.png")),
                () => papersPage.dialog.close(),
                () => sioul.status = sioul.planRenewal("passeport"),
                () => papersPage.reload(),
                () => {},
                () => grabber.save("papers-renewal"),
                () => window.close()
            ]
            // The Parameters page, from its top to its end.
            readonly property var parameters: [
                () => window.page = 12,
                () => {},
                () => grabber.save("parameters-1"),
                () => parametersPage.scrollBy(0.33),
                () => grabber.save("parameters-2"),
                () => parametersPage.scrollBy(0.33),
                () => grabber.save("parameters-3"),
                () => parametersPage.toEnd(),
                () => grabber.save("parameters-4"),
                () => window.close()
            ]
            // Sharing between computers, in a folder of the grab's: before, then shared alone.
            readonly property var share: [
                () => window.page = 12,
                () => {},
                () => parametersPage.toEnd(),
                () => {},
                () => grabber.save("share-off"),
                () => sioul.status = sioul.startSharing(grabber.folder + "/shared", "quiet harbour at dawn", "quiet harbour at dawn"),
                () => {},
                () => {},
                () => parametersPage.share.reload(),
                () => parametersPage.toEnd(),
                () => {},
                () => grabber.save("share-on"),
                () => window.close()
            ]
            // A second computer: the folder already sealed (one passphrase), then joined.
            readonly property var shareJoin: [
                () => window.page = 12,
                () => {},
                () => parametersPage.share.choose(grabber.folder + "/shared"),
                () => parametersPage.toEnd(),
                () => {},
                () => grabber.save("share-join"),
                () => sioul.status = sioul.startSharing(grabber.folder + "/shared", "quiet harbour at dawn", ""),
                () => {},
                () => {},
                () => parametersPage.share.reload(),
                () => parametersPage.toEnd(),
                () => {},
                () => grabber.save("share-joined"),
                () => window.close()
            ]
            // The first computer again, its keyring empty (tests keep none): the
            // passphrase asked once more, then it hears from the second.
            readonly property var shareTwo: [
                () => window.page = 12,
                () => {},
                () => parametersPage.toEnd(),
                () => {},
                () => grabber.save("share-again"),
                () => sioul.status = sioul.startSharing(grabber.folder + "/shared", "quiet harbour at dawn", ""),
                () => {},
                () => {},
                () => parametersPage.share.reload(),
                () => parametersPage.toEnd(),
                () => {},
                () => grabber.save("share-two"),
                () => window.close()
            ]
            // An attachment opened where no antivirus answers: the question, with the command.
            readonly property var noAntivirus: [
                () => porchPage.openFirst(),
                () => sioul.openAttachment(porchPage.openKey, 0),
                () => {},
                () => unchecked.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/no-antivirus.png")),
                () => window.close()
            ]
            // A project's billable time written as a spreadsheet, then the Time page.
            readonly property var exportCsv: [
                () => sioul.status = sioul.exportTimeCsv("studio", "2026-10-01", "2026-10-31", "file://" + grabber.folder + "/studio%20octobre.csv"),
                () => {
                    window.page = 8
                    timePage.anyway = true
                },
                () => grabber.save("time"),
                () => window.close()
            ]
            // GitHub's issues brought from a stand-in, as tasks.
            readonly property var github: [
                () => {},
                () => {},
                () => {},
                () => {
                    window.page = 1
                    tasksPage.mode = "list"
                },
                () => {},
                () => grabber.save("github-tasks"),
                () => window.close()
            ]
            // Health, on invented medicines: today's doses, one marked taken.
            readonly property var health: [
                () => window.page = 10,
                () => {},
                () => grabber.save("health"),
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-week"),
                () => healthPage.showDay(),
                () => healthPage.showNeeds(),
                () => {},
                () => grabber.savePopup(healthPage.settingsPanel(), "health-settings"),
                () => healthPage.closeSettings(),
                // Dinner's menu, then its times for today only.
                () => healthPage.showMenu("meal:2"),
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-menu"),
                () => healthPage.closePopups(),
                () => healthPage.showChange("meal:2", "times"),
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-change"),
                () => healthPage.closePopups(),
                () => {},
                () => {},
                () => weatherApplet.openForecast(),
                () => {},
                () => weatherApplet.grab(grabber.folder + "/weather.png"),
                () => grabber.save("weather-bar"),
                () => window.close()
            ]
            // A task list renamed, then deleted, against a test server.
            readonly property var collections: [
                () => sioul.syncNow(),
                () => {},
                () => {},
                () => {},
                () => sioul.setSetting("collections", JSON.stringify({ from: "calendars/agenda/emptylist", to: "Renamed list" })),
                () => {},
                () => {},
                () => {},
                () => sioul.setSetting("collections", JSON.stringify({ from: "calendars/agenda/emptylist", to: "" })),
                () => {},
                () => {},
                () => {},
                () => window.close()
            ]
            // Notes as a list and as folders; a note with a picture, then a picture, a PDF, a memo.
            readonly property var notes: [
                () => {
                    sioul.setNotesTree(false)
                    window.page = 6
                },
                () => notesPage.open("admin/lease.md"),
                () => {},
                () => grabber.save("notes-list"),
                () => sioul.setNotesTree(true),
                () => notesPage.unfold("admin"),
                () => notesPage.unfold("admin/scans"),
                () => grabber.save("notes-tree"),
                () => notesPage.open("admin/scans/lease-signed.png"),
                () => {},
                () => grabber.save("notes-image"),
                () => notesPage.open("admin/scans/manual.pdf"),
                () => {},
                () => {},
                () => grabber.save("notes-pdf"),
                () => notesPage.open("memos/2026-10-03 call with the CAF.wav"),
                () => {},
                () => grabber.save("notes-audio"),
                () => sioul.setNotesTree(false),
                () => window.close()
            ]
            // Folders, against a test server: one made, left on the server, kept again, deleted.
            readonly property var folders: [
                () => window.page = 2,
                () => sioul.createFolder("alice", "Projects"),
                () => {},
                () => {},
                () => grabber.save("folders-made"),
                () => sioul.keepFolder("alice", "Projects", false),
                () => {},
                () => grabber.save("folders-server-only"),
                () => sioul.keepFolder("alice", "Projects", true),
                () => sioul.deleteFolder("alice", "Projects"),
                () => {},
                () => {},
                () => grabber.save("folders-deleted"),
                () => window.close()
            ]
            // Quiet time: working late, done for the day with a first step, quiet, then work back.
            readonly property var quiet: [
                () => {
                    sioul.workAWhile(60)
                    window.page = 1
                },
                () => {},
                () => grabber.save("quiet-late"),
                () => tasksPage.stopForToday(),
                () => {},
                () => grabber.savePopup(dayReview.item, "quiet-review"),
                () => dayReview.item.closeIt(),
                () => {},
                () => tasksPage.grabStop(grabber.folder + "/quiet-done-dialog.png"),
                () => tasksPage.closeStop(),
                () => {},
                () => {},
                () => grabber.save("quiet-done"),
                () => window.page = 0,
                () => {},
                () => grabber.save("quiet-porch"),
                () => {
                    sioul.usualHours()
                    window.page = 1
                },
                () => {},
                () => {},
                () => grabber.save("quiet-day-off"),
                () => sioul.workAWhile(30),
                () => {},
                () => {},
                () => grabber.save("quiet-work-again"),
                () => tasksPage.openSettings(true),
                () => {},
                () => tasksPage.grabSettings(grabber.folder + "/quiet-task-settings.png"),
                () => tasksPage.openSettings(false),
                () => sioul.usualHours(),
                () => window.close()
            ]
            // OpenPGP, for tests against a test server: what the reader says of protected messages.
            readonly property var pgp: [
                () => window.page = 2,
                () => mailPage.openSubject("GnuPG, encrypted"),
                () => grabber.save("pgp-encrypted"),
                () => mailPage.openSubject("GnuPG, signed"),
                () => grabber.save("pgp-signed"),
                () => mailPage.openSubject("GnuPG, tampered"),
                () => grabber.save("pgp-tampered"),
                () => window.close()
            ]
            // Contacts and calendars, for tests against test servers: an invitation
            // accepted, an event written, a contact changed.
            readonly property var pim: [
                () => window.page = 2,
                () => mailPage.openSubject("Invitation"),
                () => grabber.save("pim-invite"),
                () => sioul.answerInvitation(mailPage.openKey, "accepted"),
                () => {},
                () => {},
                () => grabber.save("pim-answered"),
                () => window.page = 4,
                () => agendaPage.newEvent("2026-10-06"),
                () => agendaPage.fillEvent("Piano lesson", "17:00", "18:00", "weekly"),
                () => {},
                () => agendaPage.saveEvent(),
                () => {},
                () => {},
                () => grabber.save("pim-agenda"),
                () => window.page = 5,
                () => contactsPage.openFirst(),
                () => contactsPage.addNumber("+33 4 65 71 22 33"),
                () => {},
                () => grabber.save("pim-contact"),
                () => window.close()
            ]
            // Acting on mail, for tests against a test server: archive and undo, delete, write and send.
            readonly property var actions: [
                () => window.page = 2,
                () => mailPage.openFirst(),
                () => grabber.save("act-open"),
                () => sioul.act(mailPage.openKey, "archive", ""),
                () => grabber.save("act-archived"),
                () => sioul.undo(),
                () => grabber.save("act-undone"),
                () => mailPage.openFirst(),
                () => sioul.act(mailPage.openKey, "trash", ""),
                () => grabber.save("act-trash-waiting"),
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => grabber.save("act-trashed"),
                () => window.compose("new", ""),
                () => window.drafts[0].fill("Bob <bob@example.org>", "Hello Bob", "Hi **Bob**,\nhow are you?\n\n- one\n- two"),
                () => grabber.saveDraft("act-compose"),
                () => window.drafts[0].send(),
                () => grabber.save("act-sending"),
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => grabber.save("act-sent"),
                () => window.close()
            ]
            readonly property var pages: [
                () => window.page = 0,
                () => grabber.save("porch"),
                // A dose due while Sioul was closed, when there is one: when it was taken, asked.
                () => {
                    if (porchPage.missedDoses.length > 0)
                        window.askDose(porchPage.missedDoses[0].key)
                },
                () => {
                    if (doseTaken.item !== null && doseTaken.item.opened)
                        grabber.savePopup(doseTaken.item, "dose-taken")
                },
                () => {
                    if (doseTaken.item !== null)
                        doseTaken.item.close()
                },
                () => {
                    for (const lane of porchPage.view.lanes)
                        if (lane.key === "people" || lane.key === "screener")
                            porchPage.toggleHelp(lane.key)
                },
                () => grabber.save("porch-help"),
                () => porchPage.helpShown = ({}),
                () => porchPage.openSettings(true),
                () => porchPage.grabSettings(grabber.folder + "/porch-settings.png"),
                () => porchPage.openSettings(false),
                () => porchPage.openFirst(),
                () => grabber.save("message"),
                () => window.page = 1,
                // Asleep the tasks are hidden: shown anyway, as the page's button does.
                () => {
                    tasksPage.anyway = true
                    sioul.showTasksAnyway(true)
                },
                () => grabber.save("tasks-now"),
                () => tasksPage.mode = "list",
                () => grabber.save("tasks-list"),
                () => tasksPage.mode = "board",
                () => grabber.save("tasks-board"),
                () => tasksPage.mode = "timeline",
                () => grabber.save("tasks-timeline"),
                () => tasksPage.mode = "day",
                () => grabber.save("tasks-day"),
                () => tasksPage.openFirst(),
                () => grabber.save("tasks-panel"),
                () => tasksPage.showPanelDetails(),
                () => grabber.save("tasks-panel-more"),
                () => tasksPage.mode = "now",
                () => window.page = 2,
                () => grabber.save("mail"),
                () => mailPage.openFirst(),
                () => grabber.save("mail-message"),
                () => window.compose("reply", mailPage.openKey),
                () => grabber.saveDraft("compose"),
                () => grabber.closeDrafts(),
                () => window.page = 4,
                () => grabber.save("agenda"),
                () => agendaPage.mode = "day",
                () => grabber.save("agenda-day"),
                () => agendaPage.mode = "week",
                () => grabber.save("agenda-week"),
                () => agendaPage.mode = "month",
                () => grabber.save("agenda-month"),
                () => agendaPage.mode = "agenda",
                () => window.page = 5,
                () => grabber.save("contacts"),
                () => contactsPage.openFirst(),
                () => grabber.save("contact"),
                () => window.page = 6,
                () => grabber.save("notes"),
                () => notesPage.openFirst(),
                () => grabber.save("note"),
                () => window.page = 7,
                () => projectsPage.openFirst(),
                () => grabber.save("projects"),
                () => {
                    window.page = 8
                    timePage.anyway = true
                },
                () => grabber.save("time"),
                // Last week's, which has some: the first stretch changed.
                () => timePage.move(-1),
                () => timePage.changeFirst(),
                () => {
                    if (timePage.changing.item && timePage.changing.item.opened)
                        grabber.savePopup(timePage.changing.item, "time-change")
                },
                () => timePage.changing.close(),
                () => window.page = 9,
                () => grabber.save("budgets"),
                () => budgetsPage.openFirst(),
                () => grabber.save("budget"),
                () => window.page = 10,
                () => {},
                () => grabber.save("health"),
                // The week, then the settings (the usual meals and night).
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-week"),
                () => healthPage.showDay(),
                () => healthPage.showNeeds(),
                () => {},
                () => grabber.savePopup(healthPage.settingsPanel(), "health-settings"),
                () => healthPage.closeSettings(),
                // Dinner's menu, then its times for today only.
                () => healthPage.showMenu("meal:2"),
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-menu"),
                () => healthPage.closePopups(),
                () => healthPage.showChange("meal:2", "times"),
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-change"),
                () => healthPage.closePopups(),
                // A medicine's form and a prescription's, opened from the page.
                () => healthPage.editMedicine(healthPage.medicines[1] || null),
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-medicine"),
                () => healthPage.closePopups(),
                () => healthPage.editPrescription(healthPage.prescriptions[0] || null),
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-prescription"),
                () => healthPage.closePopups(),
                // A wide window: the medicines and prescriptions in a column of their
                // own, the day's and the week's; then the pictures' size again.
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1920
                        window.height = 1000
                    }
                },
                () => {},
                () => grabber.save("health-wide"),
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-wide-week"),
                () => {
                    healthPage.showDay()
                    window.width = window.phoneGrab ? 412 : window.demoGrab ? 1280 : 1100
                    window.height = window.phoneGrab ? 891 : window.demoGrab ? 860 : 1500
                },
                () => {},
                // Lunch's question: later, at another time, not today, where you stopped.
                () => window.askNeed("meal:1"),
                () => {
                    if (interruption.item !== null && interruption.item.opened)
                        grabber.savePopup(interruption.item, "need-question")
                },
                () => {
                    if (interruption.item !== null)
                        interruption.item.close()
                },
                () => budgetsPage.openId = "",
                () => window.page = 11,
                () => grabber.save("accounts"),
                () => window.page = 12,
                () => grabber.save("parameters"),
                () => window.showNewMenu(),
                () => newMenu.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/new-menu.png")),
                () => newMenu.item.close(),
                // The focus window, when a session runs.
                () => {
                    if (focusWindow && focusWindow.visible)
                        focusWindow.face.grabToImage(result => result.saveToFile(grabber.folder + "/focus.png"))
                },
                () => window.close()
            ]

            function save(name) {
                frame.grabToImage(result => result.saveToFile(grabber.folder + "/" + name + ".png"))
                if (window.phoneGrab)
                    grabber.overflow(name)
            }

            // A phone's pictures: what passes the window's right edge, said in the log,
            // the deepest items only (their containers pass it with them). `root`: a
            // pop-up's own item, which the window's content leaves out.
            function overflow(name, root) {
                const from = root || window.contentItem
                const limit = window.width + 1
                const passes = item => item.visible && item.width > 0 && item.mapToItem(null, item.width, 0).x > limit
                // A chart scrolled sideways (the tasks' timeline) is wider on purpose.
                const sideways = item => item.contentWidth !== undefined && item.contentX !== undefined && item.clip && item.contentWidth > item.width + 1
                const walk = (item, depth) => {
                    if (depth > 60)
                        return
                    let deeper = false
                    for (let i = 0; !sideways(item) && i < item.children.length; ++i) {
                        const child = item.children[i]
                        if (child.visible && walk(child, depth + 1))
                            deeper = true
                    }
                    if (!passes(item))
                        return deeper
                    if (!deeper) {
                        const words = item.text !== undefined ? " \"" + String(item.text).slice(0, 48) + "\"" : ""
                        console.warn("OVERFLOW " + name + ": " + String(item).split("(")[0] + words + " right " + Math.round(item.mapToItem(null, item.width, 0).x))
                    }
                    return true
                }
                walk(from, 0)
                // The rows that widen their column: a row or grid wider, at its own
                // width, than the screen (a column takes its widest row's width).
                const words = item => {
                    if (item.text !== undefined && String(item.text) !== "")
                        return String(item.text).slice(0, 40)
                    for (let i = 0; i < item.children.length; ++i) {
                        const found = words(item.children[i])
                        if (found !== "")
                            return found
                    }
                    return ""
                }
                const wide = (item, depth) => {
                    if (depth > 60 || !item.visible)
                        return
                    const kind = String(item).split("(")[0]
                    if ((kind.indexOf("RowLayout") >= 0 || kind.indexOf("GridLayout") >= 0) && item.implicitWidth > window.width - 16)
                        console.warn("WIDE " + name + ": " + kind + " " + Math.round(item.implicitWidth) + " \"" + words(item) + "\"")
                    for (let i = 0; i < item.children.length; ++i)
                        wide(item.children[i], depth + 1)
                }
                wide(from, 0)
            }

            // The window with what floats over it (a tip, a menu), which the frame leaves out.
            function saveWindow(name) {
                frame.Overlay.overlay.parent.grabToImage(result => result.saveToFile(grabber.folder + "/" + name + ".png"))
            }

            // An open menu or pop-up, which the frame leaves out.
            function savePopup(popup, name) {
                popup.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/" + name + ".png"))
                if (window.phoneGrab)
                    grabber.overflow(name, popup.contentItem.parent)
            }

            function saveDraft(name) {
                if (window.drafts.length > 0)
                    window.drafts[0].grab(grabber.folder + "/" + name + ".png")
            }

            function closeDrafts() {
                for (const open of window.drafts)
                    open.close()
            }

            interval: 1500
            running: folder !== ""
            repeat: true
            onTriggered: {
                if (grabber.step >= grabber.steps.length) {
                    grabber.stop()
                    return
                }
                grabber.steps[grabber.step]()
                grabber.step += 1
            }
        }
        }
    }

    // No antivirus here: the file opens only once you said so, knowing it.
    // Made the first time it is asked, as the other dialogs and menus below:
    // they would take a third of the window's start.
    Loader {
        id: unchecked

        property string key: ""
        property int index: -1
        // 0 open, 1 save, 2 keep as a paper.
        property int what: 0

        active: false
        sourceComponent: ConfirmDialog {
            sioul: sioul
            theme: theme
            onConfirmed: sioul.attachmentUnchecked(unchecked.key, unchecked.index, unchecked.what)
        }
    }

    // "Link to…" (LinkPicker.qml, linkFrom) and something new (NewMenu.qml, showNewMenu).
    Loader {
        id: linkPicker
    }
    Loader {
        id: doseTaken
    }
    Loader {
        id: interruption
    }
    Connections {
        target: interruption.item

        function onChanged() {
            if (window.healthPage)
                window.healthPage.reload()
        }
    }
    Connections {
        target: doseTaken.item

        function onAnswered() {
            if (window.healthPage)
                window.healthPage.reload()
            if (window.porchPage)
                window.porchPage.reloadDoses()
        }
    }
    Loader {
        id: newMenu
    }
    Loader {
        id: howWasItForm
    }
    Loader {
        id: railTip

        active: false
        // At the right of the button it names (its parent, given as it is said).
        sourceComponent: ToolTip {
            x: parent ? parent.width + 8 : 0
            y: parent ? Math.round((parent.height - implicitHeight) / 2) : 0
        }
    }

    TextEdit {
        id: clipboard
        visible: false
    }

    Connections {
        target: sioul
        function onCopyRequested(text) {
            window.copy(text)
        }
        function onOpenUrl(url) {
            Qt.openUrlExternally(url)
        }
        function onScanUnavailable(key, index, what, name, hint) {
            unchecked.key = key
            unchecked.index = index
            unchecked.what = what
            unchecked.active = true
            unchecked.item.ask(name, sioul.textArgs("scan-ask", JSON.stringify({ name: name, hint: hint })), what === 0 ? sioul.text("scan-open-anyway") : sioul.text("scan-save-anyway"))
        }
        function onComposeRequested(id) {
            window.openDraft(id)
        }
    }

    // Android's Back: the places put away, else what the page opened closed,
    // else back to the Porch; on the Porch, Android's own (Sioul goes to the background).
    Shortcut {
        sequence: "Back"
        enabled: window.placesOpen || window.canGoBack || window.page !== 0
        onActivated: {
            if (window.placesOpen)
                window.placesOpen = false
            else if (window.canGoBack)
                window.shownPage.back()
            else
                window.page = 0
        }
    }
    Shortcut {
        sequence: "Ctrl+1"
        onActivated: window.page = 0
    }
    Shortcut {
        sequence: "Ctrl+2"
        onActivated: window.page = 1
    }
    Shortcut {
        sequence: "Ctrl+3"
        onActivated: window.page = 2
    }
    Shortcut {
        sequence: "Ctrl+4"
        onActivated: window.page = 3
    }
    Shortcut {
        sequence: "Ctrl+5"
        onActivated: window.page = 4
    }
    Shortcut {
        sequence: "Ctrl+6"
        onActivated: window.page = 5
    }
    Shortcut {
        sequence: "Ctrl+7"
        onActivated: window.page = 6
    }
    Shortcut {
        sequence: "Ctrl+8"
        onActivated: window.page = 7
    }
    Shortcut {
        sequence: "Ctrl+9"
        onActivated: window.page = 8
    }
    Shortcut {
        sequence: "Ctrl+0"
        onActivated: window.page = 9
    }
    Shortcut {
        sequence: "Ctrl+N"
        onActivated: window.showNewMenu()
    }
    Shortcut {
        sequence: "Ctrl+Z"
        enabled: sioul.undoLine !== ""
        onActivated: sioul.undo()
    }
    Shortcut {
        sequence: "F5"
        onActivated: sioul.syncNow()
    }
    // The places hidden or shown again beside the pages; on a phone, pulled over them or put away.
    Shortcut {
        sequence: "F9"
        onActivated: {
            if (window.compact)
                window.placesOpen = !window.placesOpen
            else
                window.togglePlaces(true)
        }
    }

    Rectangle {
        id: frame

        anchors.fill: parent
        color: theme.background

        Item {
            anchors.fill: parent

            // The places: a narrow column of icons beside the pages, their names
            // beside them when Settings ▸ Display says so, hidden altogether by the
            // button at its foot or F9 (Places.qml); on a phone, or in a window as
            // narrow, pulled over the pages from the left, with their names.
            Rectangle {
                id: places

                // Names beside the icons: in a phone's drawer, else when asked for.
                readonly property bool named: window.compact || window.placesNamed
                // What it holds, once made.
                readonly property Places shown: placesContent.item as Places

                z: 2
                visible: window.compact || !window.placesHidden
                // With names, as wide as "Refresh everything" needs at its size.
                width: places.named ? 216 : 56
                height: parent.height
                x: !window.compact || window.placesOpen ? 0 : -width - 1
                color: theme.surface
                Accessible.role: Accessible.Pane
                Accessible.name: sioul.text("ui-places")

                Behavior on x {
                    enabled: window.compact
                    NumberAnimation {
                        duration: 160
                        easing.type: Easing.OutCubic
                    }
                }

                Rectangle {
                    anchors.right: parent.right
                    width: 1
                    height: parent.height
                    color: theme.line
                }

                // Not made before they show (placesMade).
                Loader {
                    id: placesContent

                    anchors.fill: parent
                    active: window.placesMade
                    sourceComponent: Component {
                        Places {
                            window: window
                            theme: theme
                            sioul: sioul
                            named: places.named
                        }
                    }
                }
            }

            // A tap beside the places pulled over the pages puts them away.
            Rectangle {
                z: 1
                anchors.fill: parent
                visible: window.compact && window.placesOpen
                color: Qt.rgba(0, 0, 0, 0.35)

                MouseArea {
                    anchors.fill: parent
                    onClicked: window.placesOpen = false
                }
            }

            ColumnLayout {
                anchors.fill: parent
                anchors.leftMargin: window.railShown ? places.width : 0
                spacing: 0

                // On a phone: ☰ for the places, and the page you are on.
                Rectangle {
                    visible: window.compact
                    Layout.fillWidth: true
                    Layout.preferredHeight: 48
                    color: theme.surface

                    Rectangle {
                        anchors.bottom: parent.bottom
                        width: parent.width
                        height: 1
                        color: theme.line
                    }

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 4
                        anchors.rightMargin: theme.gap
                        spacing: 4

                        ToolButton {
                            visible: !window.canGoBack
                            Layout.preferredWidth: 48
                            Layout.fillHeight: true
                            icon.name: "application-menu"
                            icon.color: theme.text
                            display: AbstractButton.IconOnly
                            Accessible.name: sioul.text("ui-menu")
                            onClicked: window.placesOpen = true
                        }
                        // Back to the page's list, from what it opened.
                        ToolButton {
                            visible: window.canGoBack
                            Layout.preferredWidth: 48
                            Layout.fillHeight: true
                            icon.name: "go-previous"
                            icon.color: theme.text
                            display: AbstractButton.IconOnly
                            Accessible.name: sioul.text("ui-back")
                            onClicked: window.shownPage.back()
                        }
                        Label {
                            Layout.fillWidth: true
                            text: sioul.text(window.pageNames[window.page] || "ui-porch")
                            textFormat: Text.PlainText
                            font.pixelSize: 18
                            color: theme.text
                            elide: Text.ElideRight
                        }
                    }
                }

                StackLayout {
                    id: pages

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    currentIndex: window.page

                    Loader {
                        id: porchPageLoader
                    }
                    Loader {
                        id: tasksPageLoader
                    }
                    Loader {
                        id: mailPageLoader
                    }
                    Loader {
                        id: sitesPageLoader
                    }
                    Loader {
                        id: agendaPageLoader
                    }
                    Loader {
                        id: contactsPageLoader
                    }
                    Loader {
                        id: notesPageLoader
                    }
                    Loader {
                        id: projectsPageLoader
                    }
                    Loader {
                        id: timePageLoader
                    }
                    Loader {
                        id: budgetsPageLoader
                    }
                    Loader {
                        id: healthPageLoader
                    }
                    Loader {
                        id: accountsPageLoader
                    }
                    Loader {
                        id: parametersPageLoader
                    }
                    Loader {
                        id: papersPageLoader
                    }
                }

                // One sentence about what happened last, and the keys.
                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 36
                    color: theme.surface

                    Rectangle {
                        width: parent.width
                        height: 1
                        color: theme.line
                    }

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: showPlaces.visible ? 10 : theme.gap
                        // The pauses' place at the end is kept; what does not fit is cut, never over it.
                        anchors.rightMargin: pausesRow.width + 2 * theme.gap
                        spacing: theme.gap
                        clip: true

                        // The places hidden (F9): the button that shows them again,
                        // where the one that hid them was; its icon alone, its words in
                        // its tip, so that the line keeps its room.
                        RailButton {
                            id: showPlaces

                            visible: window.placesHidden && !window.compact
                            Layout.preferredWidth: 36
                            Layout.preferredHeight: 28
                            theme: theme
                            iconName: "sidebar-expand-left"
                            name: sioul.text("ui-places-show")
                            keys: "F9"
                            onChosen: window.togglePlaces(showPlaces.visualFocus)
                        }
                        Label {
                            Layout.fillWidth: true
                            Layout.minimumWidth: 40
                            text: sioul.undoLine !== "" ? sioul.undoLine : sioul.status

                            // When nothing else is said, what can be closed now (DayReview.qml):
                            // whole or not at all, and out of the layout, which it never widens.
                            Label {
                                anchors.fill: parent
                                visible: parent.text === "" && window.offer.line !== "" && parent.width >= implicitWidth
                                text: window.offer.line
                                textFormat: Text.PlainText
                                verticalAlignment: Text.AlignVCenter
                                color: theme.muted
                            }
                            // Server answers and file names: never read as rich text.
                            textFormat: Text.PlainText
                            color: sioul.undoLine !== "" ? theme.text : theme.muted
                            elide: Text.ElideRight
                            // Never over the line beside it, however little room is left.
                            clip: true
                        }
                        // Quiet time, or work kept late: the line, and behind it the
                        // way back, never suggested.
                        Button {
                            id: modeButton

                            // On a phone, the end of the day offered takes its place (DayReview.qml).
                            visible: window.moment.line !== "" && sioul.undoLine === "" && !(window.compact && window.offer.kind !== "")
                            // Room shared with what just happened, when something did; narrower
                            // when the row is full (the pauses' buttons stay whole at its end).
                            Layout.fillWidth: true
                            Layout.maximumWidth: Math.round(window.width * (sioul.status !== "" ? 0.3 : 0.55))
                            Layout.preferredHeight: 28
                            flat: true
                            text: window.moment.line
                            ToolTip.visible: hovered
                            ToolTip.text: window.moment.line
                            ToolTip.delay: 800
                            onClicked: {
                                // A meal or sleep offers no way back to work: they come first.
                                if ((window.moment.time === "meals" || window.moment.time === "sleep") && window.moment.work_now !== true)
                                    return
                                modeMenu.active = true
                                modeMenu.item.popup()
                            }

                            contentItem: Label {
                                text: modeButton.text
                                color: theme.muted
                                elide: Text.ElideRight
                            }

                            // Made the first time it is opened.
                            Loader {
                                id: modeMenu

                                active: false
                                sourceComponent: SioulMenu {
                                    // Free time (docs/pauses.md): back from it, the usual end kept, nothing at all.
                                    MenuItem {
                                        visible: window.moment.reason === "free-time"
                                        height: visible ? implicitHeight : 0
                                        text: sioul.text("free-time-back")
                                        onTriggered: window.setFreeTime(false)
                                    }
                                    MenuItem {
                                        visible: window.pauses.can_keep
                                        height: visible ? implicitHeight : 0
                                        text: sioul.textWith("free-menu-keep", "time", window.pauses.usual_end)
                                        onTriggered: sioul.keepUsualEnd()
                                    }
                                    MenuItem {
                                        id: nothingItem

                                        visible: window.moment.reason === "free-time"
                                        height: visible ? implicitHeight : 0
                                        text: sioul.text("free-menu-nothing")
                                        checkable: true
                                        checked: window.pauses.nothing
                                        onTriggered: {
                                            sioul.setFreeNothing(!window.pauses.nothing)
                                            nothingItem.checked = Qt.binding(() => window.pauses.nothing)
                                        }
                                    }
                                    // The day closed today can be taken back, that day.
                                    MenuItem {
                                        visible: window.moment.reason === "done-for-the-day" && window.moment.today
                                        height: visible ? implicitHeight : 0
                                        text: sioul.text("mode-back-to-plan")
                                        onTriggered: sioul.usualHours()
                                    }
                                    MenuItem {
                                        visible: window.moment.reason === "working-late"
                                        height: visible ? implicitHeight : 0
                                        text: sioul.text("mode-usual-hours")
                                        onTriggered: sioul.usualHours()
                                    }
                                    // Work shown whatever the hours, as on the Porch; not
                                    // during a meal or sleep, which come first.
                                    MenuItem {
                                        id: workNowItem

                                        visible: (window.moment.quiet && window.moment.time !== "meals" && window.moment.time !== "sleep" && window.moment.reason !== "free-time") || window.moment.work_now === true
                                        height: visible ? implicitHeight : 0
                                        text: sioul.text("mode-work-now")
                                        checkable: true
                                        checked: window.moment.work_now === true
                                        onTriggered: {
                                            sioul.setWorkNow(!(window.moment.work_now === true))
                                            workNowItem.checked = Qt.binding(() => window.moment.work_now === true)
                                        }
                                    }
                                    Repeater {
                                        model: window.moment.quiet && window.moment.time !== "meals" && window.moment.time !== "sleep" && window.moment.reason !== "free-time" && !(window.moment.reason === "done-for-the-day" && window.moment.today) ? [30, 60, 120, 240] : []

                                        delegate: MenuItem {
                                            required property int modelData

                                            text: sioul.text("mode-work-" + modelData)
                                            onTriggered: sioul.workAWhile(modelData)
                                        }
                                    }
                                }
                            }
                        }
                        // A task just done: how it was, if you want to say; never asked again.
                        Button {
                            id: howButton

                            visible: window.lastDone !== "" && sioul.undoLine === "" && sioul.status !== "" && sioul.status === window.lastDoneSaid
                            Layout.preferredHeight: 28
                            flat: true
                            text: sioul.text("felt-ask")
                            onClicked: window.howWasIt(window.lastDone)
                        }
                        // Back from free time, the end of work said once: one key keeps the usual end, no reason asked.
                        Button {
                            visible: window.freeSaid !== "" && sioul.undoLine === "" && sioul.status === window.freeSaid && window.pauses.can_keep
                            Layout.preferredHeight: 28
                            flat: true
                            text: sioul.text("free-keep-end")
                            onClicked: sioul.keepUsualEnd()
                        }
                        // The work day, or the day, can be closed (DayReview.qml): offered,
                        // never pressed for you; nothing at work or while you sleep.
                        Button {
                            visible: window.offer.kind !== "" && sioul.undoLine === ""
                            Layout.preferredHeight: 28
                            flat: true
                            text: window.offer.button
                            ToolTip.visible: hovered
                            ToolTip.text: window.offer.line
                            ToolTip.delay: 800
                            Accessible.description: window.offer.line
                            onClicked: window.reviewDay(window.offer.kind)
                        }
                        // Ten seconds to change your mind.
                        Button {
                            visible: sioul.undoLine !== ""
                            Layout.preferredHeight: 28
                            text: sioul.text("ui-undo")
                            icon.name: "edit-undo"
                            icon.color: theme.text
                            onClicked: sioul.undo()
                        }
                        // The keys, when nothing else needs the room.
                        Label {
                            visible: window.moment.line === "" && !window.compact && !howButton.visible
                            text: sioul.text("ui-keys")
                            color: theme.muted
                            font.pixelSize: 12
                        }
                        // Do-not-disturb on every device (DndApplet.qml): its switch, apart
                        // from the pauses' buttons at the line's end.
                        DndApplet {
                            sioul: sioul
                            theme: theme
                            moment: window.moment
                            compact: window.compact
                        }
                        // Sounds to focus or to rest by, never on by themselves.
                        SoundsApplet {
                            sioul: sioul
                            theme: theme
                        }
                        // The weather, in one colour, at the place you chose.
                        WeatherApplet {
                            id: weatherApplet

                            sioul: sioul
                            theme: theme
                        }
                    }
                    // The two pauses (docs/pauses.md): a place of their own at the line's end,
                    // the same every time, never pushed out by what the line holds.
                    RowLayout {
                        id: pausesRow

                        anchors.right: parent.right
                        anchors.rightMargin: theme.gap
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4

                        // Free time (docs/pauses.md): leisure whatever the hour, a switch
                        // showing its state; its menu at a right click or a long press.
                        ToolButton {
                            id: freeButton

                            readonly property bool on: window.moment.reason === "free-time"

                            // Never squeezed by a long status line: always reachable.
                            Layout.minimumWidth: implicitWidth
                            Layout.preferredHeight: 28
                            checkable: true
                            checked: freeButton.on
                            icon.name: "flower-shape"
                            icon.color: freeButton.on ? theme.accent : theme.text
                            text: sioul.text("free-time")
                            display: window.compact ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            Accessible.name: freeButton.on ? sioul.text("free-time-back") : sioul.text("free-time")
                            ToolTip.visible: hovered
                            ToolTip.text: freeButton.on ? sioul.text("free-time-back") : sioul.text("free-time-tip")
                            ToolTip.delay: 600
                            onClicked: {
                                window.setFreeTime(!freeButton.on)
                                freeButton.checked = Qt.binding(() => freeButton.on)
                            }

                            TapHandler {
                                acceptedButtons: Qt.RightButton
                                // A touch has no buttons: on a touch screen, the long press below.
                                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                onTapped: window.freeMenu()
                            }
                            TapHandler {
                                id: freeHold

                                acceptedDevices: PointerDevice.TouchScreen
                                onLongPressed: {
                                    window.menuAt = freeHold.point.scenePosition
                                    window.freeMenu()
                                }
                            }
                        }
                        // Pause (docs/pauses.md): apart from the rest, at the row's end, the
                        // same place every time; it asks nothing.
                        ToolButton {
                            id: pauseButton

                            Layout.leftMargin: window.compact ? 2 : theme.gap
                            Layout.minimumWidth: implicitWidth
                            Layout.preferredHeight: 28
                            icon.name: "media-playback-pause"
                            icon.color: theme.text
                            text: sioul.text("pause-button")
                            display: window.compact ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            Accessible.name: sioul.text("pause-button")
                            Accessible.description: sioul.text("pause-tip")
                            ToolTip.visible: hovered
                            ToolTip.text: sioul.text("pause-tip")
                            ToolTip.delay: 600
                            onClicked: sioul.pauseNow()
                        }
                    }
                }
            }
        }

        // The pause's screen over the whole window (PauseCover.qml): while paused,
        // on every device; for a try-out from Settings; for the few words on
        // coming back, which stay until read. Made when first needed.
        Loader {
            id: pauseCover

            property bool trial: false
            property bool returning: false

            anchors.fill: parent
            z: 10
            active: window.moment.reason === "paused" || pauseCover.trial || pauseCover.returning
            visible: active

            sourceComponent: Component {
                PauseCover {
                    sioul: sioul
                    theme: theme
                    trial: pauseCover.trial
                    onComing: pauseCover.returning = true
                    onDone: {
                        pauseCover.returning = false
                        pauseCover.trial = false
                    }
                }
            }
        }
    }
}
