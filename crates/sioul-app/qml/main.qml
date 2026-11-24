// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's window: on the left, a narrow column of icons (RailButton.qml):
// "New" for anything, the places where your things live, then, apart at the
// bottom, the accounts, the settings and one button that refreshes
// everything; their names beside them when Settings ▸ Display says so or the
// button at the title bar's left end (F9), else said when the pointer rests
// on one. One quiet status line (StatusLine.qml), where "Undo" waits ten
// seconds after anything is moved, deleted or sent: on a computer, it is the
// window's own title bar, at the top, with the window's buttons where the
// system puts them (TitleBar.qml); on a phone, it is at the bottom. No
// badges, no counts in the title, no red; the system's dark mode is
// followed. Everything works from the keyboard: Tab, Enter, Escape, Ctrl+1 to
// Ctrl+9 and Ctrl+0 (the places by the list), Ctrl+N (new), Ctrl+Z, F5
// (refresh), F9 (the places' names or their icons only). Whatever links lead
// to (a task, a message, a note, an event, a contact) opens where it lives.
// On a phone, or in a window as narrow, the places are pulled over the pages
// from the left (☰) with their names, the pages take the whole width, and
// Android's Back puts the places away, then goes back to the Porch.

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
    readonly property bool demoGrab: sioul.grabFolder() !== "" && (sioul.grabSteps() === "demo" || sioul.grabSteps() === "invoices" || sioul.grabSteps() === "notes-find") && !window.phoneGrab
    // A tablet's pictures (SIOUL_GRAB_STEPS=tablet): each page at 1280 by 800, the device a
    // tablet, without the window's own title bar and with touch, as on Android; a
    // computer's layout, since the room decides it, never the system.
    readonly property bool tabletGrab: sioul.grabFolder() !== "" && sioul.grabSteps() === "tablet"

    width: window.phoneGrab ? 412 : window.demoGrab || window.tabletGrab ? 1280 : 1100
    // Taller when saving images of the pages, so long pages show whole.
    height: window.phoneGrab ? 891 : window.demoGrab ? 860 : window.tabletGrab ? 800 : sioul.grabFolder() ? 1500 : 760
    // Android's screen, or its window in split screen or free-form, is the system's
    // to size (a phone's, a tablet's): no minimum of Sioul's there. The layout follows
    // whatever size it is given (`compact`).
    minimumWidth: Qt.platform.os === "android" || window.phoneGrab ? 0 : 680
    minimumHeight: Qt.platform.os === "android" || window.phoneGrab ? 0 : 480
    // Shown as Sioul starts; a desktop's then maximized (Component.onCompleted). A
    // `visibility` set here instead left a phone's window unshown: a black screen.
    visible: true
    // The taskbar's name; no title bar shows it (TitleBar.qml).
    title: "Sioul"
    theme: theme
    // On a computer, Sioul draws its own title bar (TitleBar.qml): the status
    // line at the top, the window's buttons where the system puts them; the
    // system draws none. Android's window has none (a phone's, a tablet's: the
    // system's own), nor have a phone's or a tablet's pictures.
    readonly property bool ownTitleBar: Qt.platform.os !== "android" && !window.phoneGrab && !window.tabletGrab
    flags: window.ownTitleBar ? Qt.Window | Qt.FramelessWindowHint : Qt.Window

    // How the window was shown before it went to the tray: shown again so.
    property int shownAs: Window.Maximized
    // What full screen (F11) goes back to.
    property int beforeFullScreen: Window.Maximized
    // Quitting for good (the tray's menu, Ctrl+Q): the close goes through.
    property bool quitting: false
    onVisibilityChanged: {
        if (window.visibility === Window.Maximized || window.visibility === Window.Windowed || window.visibility === Window.FullScreen)
            window.shownAs = window.visibility
    }
    // Hidden in the tray with no other window of Sioul's open: the Porch, the
    // budgets and the accounts, shown here alone, wait until it shows (`set_hidden`).
    readonly property bool viewsHeld: Qt.platform.os !== "android" && !window.visible && window.drafts.length === 0 && (window.focusWindow === null || !window.focusWindow.visible)
    onViewsHeldChanged: sioul.setHidden(window.viewsHeld)
    // The system tray holds Sioul (qml-desktop/Tray.qml): closing the window hides it there.
    readonly property bool trayHolds: trayLoader.item !== null && trayLoader.item.available === true

    // Hidden in the tray, Sioul going on behind; said once, the first time.
    function hideToTray() {
        window.hide()
        if (!sioul.viewFlag("tray-said")) {
            sioul.setViewFlag("tray-said", true)
            trayLoader.item.showMessage(sioul.text("tray-said-title"), sioul.text("tray-said"))
        }
    }

    // Shown again as it was (from the tray), and brought forward. `visible`
    // first: declared above, it went false with hide(), and `visibility` set
    // against it warned "Conflicting properties 'visible' and 'visibility'".
    function bringBack() {
        if (!window.visible) {
            window.visible = true
            window.visibility = window.shownAs
        }
        window.raise()
        window.requestActivate()
    }

    // For the window's pictures and tests: Android's system bars stood in for
    // (points on each side), as Qt's safe area gives them to every item, the
    // overlay's popups included (SafeDialog.qml), and drawn as a dim band.
    function simulateSystemBars(top, right, bottom, left) {
        // The window's root item, which holds its content and its overlay: both get them.
        const root = frame.Overlay.overlay.parent
        root.SafeArea.additionalMargins.top = top
        root.SafeArea.additionalMargins.right = right
        root.SafeArea.additionalMargins.bottom = bottom
        root.SafeArea.additionalMargins.left = left
        simulatedBars.margins = { top: top, right: right, bottom: bottom, left: left }
    }

    // The tray's click: hidden when the window is in front, else brought back.
    function toggleShown() {
        if (window.visible && window.active)
            window.hideToTray()
        else
            window.bringBack()
    }

    // Quit for good: the tray's menu, Ctrl+Q.
    function quitSioul() {
        window.quitting = true
        window.close()
    }

    // F11: full screen, or back to how it was.
    function toggleFullScreen() {
        if (window.visibility === Window.FullScreen) {
            window.visibility = window.beforeFullScreen
            return
        }
        window.beforeFullScreen = window.visibility === Window.Windowed ? Window.Windowed : Window.Maximized
        window.visibility = Window.FullScreen
    }

    // The title bar's double click and its button (TitleBar.qml): maximized,
    // or back to its size; from full screen, back to how it was.
    function toggleMaximized() {
        if (window.visibility === Window.FullScreen)
            window.toggleFullScreen()
        else if (window.visibility === Window.Maximized)
            window.showNormal()
        else
            window.showMaximized()
    }

    // The status line (StatusLine.qml): in the title bar on a computer, at the bottom on a phone.
    readonly property var statusLine: window.ownTitleBar ? (titleBar.item ? (titleBar.item as TitleBar).statusLine : null) : bottomLine.item
    // The button that shows the places' names or keeps their icons only: at the
    // title bar's left end on a computer, else at the places' foot.
    readonly property var namesButton: window.ownTitleBar ? (titleBar.item ? (titleBar.item as TitleBar).namesButton : null) : (places.shown ? places.shown.namesButton : null)

    // The writing windows open, one per draft.
    property var drafts: []
    // Month and day names in Sioul's language, not the system's.
    readonly property var sioulLocale: Qt.locale(sioul.text("qt-locale"))
    // A phone, or a window as narrow: the places pulled over the pages from the left.
    readonly property bool compact: window.width < 720
    property bool placesOpen: false
    // The places made the first time they show, then kept (Places.qml): a
    // phone's drawer at its first ☰; the column beside the pages at once.
    property bool placesMade: false
    onPlacesOpenChanged: {
        if (window.placesOpen)
            window.placesMade = true
    }
    onRailShownChanged: {
        if (window.railShown)
            window.placesMade = true
    }
    // Their names beside their icons, or their icons only: the button at the
    // title bar's left end (at their foot where the status line is at the
    // bottom), F9, or Settings ▸ Display, one setting; wherever they are beside the pages.
    readonly property bool placesNamed: sioul.reading ? JSON.parse(sioul.reading).places_named === true : false
    // The places beside the pages: a window wide enough; never hidden.
    readonly property bool railShown: !window.compact
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
    readonly property var pageNames: ["ui-porch", "ui-tasks", "ui-mail", "ui-sites", "ui-agenda", "ui-contacts", "ui-notes", "ui-projects", "ui-time", "ui-budgets", "ui-health", "ui-accounts", "ui-parameters", "ui-papers", "texts-page-title", "ui-letters", "ui-ties", "ui-supplies"]
    // Each page is made when it is first shown, then kept: the window opens
    // with the Porch alone, and a page asked for (a link, "New") is made
    // before it is used, as the page is set first.
    property var made: sioul.grabFolder() !== "" && !["memory", "rest", "rest-shared"].includes(sioul.grabSteps()) ? ({ 0: true, 1: true, 2: true, 3: true, 4: true, 5: true, 6: true, 7: true, 8: true, 9: true, 10: true, 11: true, 12: true, 13: true, 14: true, 15: true, 16: true, 17: true }) : ({ 0: true })
    onPageChanged: {
        window.placesOpen = false
        // On a phone the card covers the page: the page asked for shows (review of 590eb18, M3).
        cardHost.leave()
        if (window.made[window.page] !== true) {
            const made = Object.assign({}, window.made)
            made[window.page] = true
            window.made = made
        }
    }
    // Each page's file, read the first time it is made (setSource, for their
    // required properties): what it imports (maps, PDFs, sites, sounds) is
    // loaded then, not as the window opens. Android has its own Sites page.
    readonly property var pageFiles: ["PorchPage.qml", "TasksPage.qml", "MailPage.qml", Qt.platform.os === "android" ? "android/SitesPage.qml" : "SitesPage.qml", "AgendaPage.qml", "ContactsPage.qml", "NotesPage.qml", "ProjectsPage.qml", "TimePage.qml", "BudgetsPage.qml", "HealthPage.qml", "AccountsPage.qml", "ParametersPage.qml", "PapersPage.qml", "TextsPage.qml", "LettersPage.qml", "ChainPage.qml", "SuppliesPage.qml"]
    onMadeChanged: window.makePages()
    function makePages() {
        const loaders = [porchPageLoader, tasksPageLoader, mailPageLoader, sitesPageLoader, agendaPageLoader, contactsPageLoader, notesPageLoader, projectsPageLoader, timePageLoader, budgetsPageLoader, healthPageLoader, accountsPageLoader, parametersPageLoader, papersPageLoader, textsPageLoader, lettersPageLoader, chainPageLoader, suppliesPageLoader]
        for (let i = 0; i < loaders.length; i++) {
            if (window.made[i] !== true || loaders[i].status !== Loader.Null)
                continue
            // Settings takes no window; Accounts takes it for Bitwarden's key page (`vaultKeys`).
            const given = i === 12 ? { sioul: sioul, theme: theme } : { sioul: sioul, theme: theme, window: window }
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
    // texts: the Texts page, a computer's only (a phone has its own SMS app).
    readonly property var textsPage: textsPageLoader.item
    // Paper letters: their own page, waiting then filed (LettersPage.qml).
    readonly property var lettersPage: lettersPageLoader.item
    // The chain of ties from one thing (ChainPage.qml), opened from any thing's card or menu.
    readonly property var chainPage: chainPageLoader.item
    // Supplies: what is used up or sold, and what is left of it (SuppliesPage.qml).
    readonly property var suppliesPage: suppliesPageLoader.item
    // The page it was opened from, which Back gives again.
    property int chainFrom: 0
    // The thing last shown with its card's buttons (ThingActions.qml), whose ties Ctrl+G opens.
    property var thingShown: null

    // The chain of ties from a thing, by its address (docs/tasks.md, "The chain of ties").
    function openChain(uri) {
        if ((uri || "") === "")
            return
        cardHost.leave()
        if (window.page !== 16)
            window.chainFrom = window.page
        window.page = 16
        Qt.callLater(() => {
            if (window.chainPage)
                window.chainPage.show(uri)
        })
    }

    // Back from the chain: the page it was opened from.
    function closeChain() {
        cardHost.leave()
        window.page = window.chainFrom
    }
    // The page in view; on a phone, a page showing one thing it opened
    // (a message, a task) says so (canGoBack) and closes it (back()).
    readonly property var shownPage: pages.children[window.page] ? pages.children[window.page].item : null
    readonly property bool canGoBack: window.compact && (cardHost.open || window.shownPage !== null && window.shownPage.canGoBack === true)
    // Every object's card, where you are (CardHost.qml), and what it shows: the agenda, Tasks
    // and Health outline the open thing.
    readonly property alias cardHost: cardHost
    readonly property var cardRef: cardHost.ref

    // Back, on a phone (its button and Android's) and Escape: the card first, then what the page opened.
    function goBack() {
        if (cardHost.open)
            cardHost.back()
        else if (window.shownPage !== null && window.shownPage.canGoBack === true)
            window.shownPage.back()
    }
    // Work time or quiet time: {quiet, reason, until, line, hours}.
    readonly property var moment: sioul.mode ? JSON.parse(sioul.mode) : ({ quiet: false, sleep: false, reason: "", until: "", line: "", hours: false, night: true })

    // The start timed (on Android, and with SIOUL_TIMING): the first frame drawn.
    property bool drawn: false
    onFrameSwapped: {
        if (!window.drawn) {
            window.drawn = true
            sioul.firstFrame()
            sitesWarm.start()
            window.loadTray()
        }
    }
    // Sites kept open (their notifications) live in the Sites page: made a
    // moment after the window shows, when there are such sites. Not on
    // Android, whose screen of a site keeps nothing open (docs/android.md, "Sites").
    Timer {
        id: sitesWarm

        interval: 1500
        onTriggered: {
            if (Qt.platform.os !== "android" && window.made[3] !== true && JSON.parse(sioul.siteList() || "[]").some(s => s.background)) {
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

        offset: ms => sioul.zoneOffset(ms / 1000)

        paused: sioul.away
        onTurned: {
            if (Date.now() - clock.started >= 30000)
                sioul.refreshMode()
        }
    }
    property alias now: clock.now
    property alias today: clock.today
    // When Sioul last came back on the screen (Unix seconds): a dose's mark is
    // being read until the Health job reads again after it (WeekPlanning.qml).
    property real backAt: 0

    Connections {
        target: sioul

        function onAwayChanged() {
            if (!sioul.away)
                window.backAt = Date.now() / 1000
        }
    }

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
        cardHost.leave()
        window.page = 5
        contactsPage.open(key)
    }

    // A person's page, in Contacts (docs/persons.md).
    function openPerson(id) {
        cardHost.leave()
        window.page = 5
        contactsPage.openPerson(id)
    }

    // A new contact with a number filled in: a call's line on a computer's Porch (CallsSection.qml).
    function newContactWith(number) {
        window.page = 5
        contactsPage.startNew(number)
    }

    // A task's card, where you are (the window's one TaskPanel, in the card host).
    function openTask(uid) {
        cardHost.show({ kind: "task", owner: uid })
    }

    // Today in the agenda's Day: what Tasks ▸ The day opens (the unified agenda, part 2).
    function showAgendaDay() {
        window.page = 4
        agendaPage.showDay(window.today)
    }

    // This week in the agenda's Week: what Health's Week opens (part 5, for one release).
    function showAgendaWeek() {
        window.page = 4
        agendaPage.anchor = new Date(window.today + "T12:00:00")
        if (agendaPage.mode === "week")
            agendaPage.load()
        else
            agendaPage.mode = "week"
    }

    // A note, at one of its headings when `anchor` names one ("Money").
    function openNote(path, anchor) {
        cardHost.leave()
        window.page = 6
        notesPage.openAt(path, anchor || "")
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
            cardHost.startNewTask(source)
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
        } else if (kind === "letter") {
            // Paper letters have their page: a scan chosen there, copied where scans are read.
            window.page = 15
            Qt.callLater(() => {
                if (window.lettersPage)
                    window.lettersPage.addScan()
            })
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
                // What was typed in an invoice's editor, written before Android may end Sioul.
                if (window.budgetsPage)
                    window.budgetsPage.flushInvoices()
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

    // A notice's click or "Open" (its thing by its address, `kind` "": `open`),
    // a link or a setting's (its kind), or Sioul started again while it runs:
    // what it is about, shown, the window brought forward, back from the tray
    // if hidden there.
    Connections {
        target: sioul

        function onReminderOpened(kind, uri, key) {
            if (kind === "" && uri !== "")
                window.open(uri, key)
            else
                window.openThing({ kind: kind, uri: uri, key: key })
            window.bringBack()
        }


        // An attachment kept in the wallet: what it is, asked.
        function onPaperKept(file, title, kind) {
            window.page = 13
            papersPage.startFrom(file, title, kind)
        }

        // A task done: "How was it?" beside the line that says so, gone with it; what it used
        // of your supplies, "Change…" beside it; a task to resupply asks how many came in.
        function onTaskDone(uid) {
            window.lastDone = uid
            window.lastDoneSaid = sioul.status
            const supplies = sioul.taskSupplies(uid)
            window.lastDoneSupplies = supplies === "null" ? null : JSON.parse(supplies)
            if (window.lastDoneSupplies !== null && window.lastDoneSupplies.done && window.lastDoneSupplies.resupply)
                window.suppliesAsk(uid, "arrived")
        }

        // On Windows, a notice with something to open, through Sioul's icon
        // in the tray, whose message has no button: a click on it opens its
        // thing (tell.rs). Without a tray that shows messages, the system's
        // own notice, its words alone.
        function onTrayNotice(title, body, address, hint) {
            // A click on a message says nothing of which one it was: with
            // another told in the last ten minutes, a click opens neither,
            // never the wrong thing (the window still comes forward).
            const earlier = window.noticed !== null && Date.now() < window.noticed.at + 10 * 60 * 1000 && window.noticed.address !== address
            window.noticed = { address: earlier ? "" : address, hint: earlier ? "" : hint, at: Date.now(), until: Date.now() + window.noticeKept }
            const tray = trayLoader.item
            if (tray !== null && tray.available && tray.supportsMessages)
                tray.showMessage(title, body)
            else
                sioul.plainNotice(title, body)
        }
    }

    // The task just done, and the status line that said so.
    property string lastDone: ""
    property string lastDoneSaid: ""
    // Its supplies (`supplies::task_supplies`): what it used, "Change…" in the status line.
    property var lastDoneSupplies: null

    // What a task done used, said again ("used"), or how many came in for a task to
    // resupply ("arrived") (SuppliesUsed.qml, made the first time).
    function suppliesAsk(uid, mode) {
        if (suppliesUsedForm.item === null)
            suppliesUsedForm.setSource("SuppliesUsed.qml", { sioul: sioul, theme: theme })
        if (mode === "arrived")
            suppliesUsedForm.item.arrive(uid)
        else
            suppliesUsedForm.item.ask(uid)
    }

    // What the tray's last notice is about ({address, hint, until}, `onTrayNotice`):
    // a click on its message opens it, once, until then.
    property var noticed: null
    // Clicked later than this, a notice opens nothing: its sheet would close another day.
    readonly property int noticeKept: 6 * 3600 * 1000

    // The tray's message clicked (Tray.qml): what its notice offered while it
    // holds; the window brought back in any case.
    function trayMessageClicked() {
        const noticed = window.noticed
        window.noticed = null
        if (noticed !== null && noticed.address !== "" && Date.now() < noticed.until)
            window.open(noticed.address, noticed.hint)
        window.bringBack()
    }

    // A notice's thing, by its address: the one route (NoticeOpener.qml,
    // docs/notifications.md), `hint` where this device last saw it.
    NoticeOpener {
        id: noticeOpener

        window: window
    }
    function open(address, hint) {
        noticeOpener.open(address, hint)
    }

    // A file a notice names (a file a site offered, in Downloads or a folder you chose):
    // a folder in the file manager; a file with the desktop's own application, a
    // program never, its folder instead (docs/sites.md, "Downloads").
    function openFile(url) {
        if (!url.endsWith("/") && sioul.isProgram(url)) {
            Qt.openUrlExternally(url.slice(0, url.lastIndexOf("/") + 1))
            return
        }
        Qt.openUrlExternally(url)
    }

    // What a download's notice opens (`sioul:download/<id>`), as this session kept it (downloads.rs).
    function downloadOpened(address) {
        return sioul.downloadOpened(address)
    }


    // The tray's menu (Tray.qml): the work day ("work") or the whole day
    // ("night") closed at any hour, on its sheet in the window.
    function reviewFromTray(kind) {
        window.reviewDay(kind)
        window.bringBack()
    }

    // How a task just done was: its ratings as felt, if you want to say (HowWasIt.qml, made the first time).
    function howWasIt(uid) {
        if (howWasItForm.item === null)
            howWasItForm.setSource("HowWasIt.qml", { sioul: sioul, theme: theme })
        window.howWasItPopup.ask(uid)
    }
    readonly property var howWasItPopup: howWasItForm.item

    // "A payment…" from a message, by its file (`kind` "mail", `trust` its sender's
    // level), or a paper letter, by its id (`kind` "letter"): a short form filled from
    // what it says, any budget (PaymentForm.qml, made the first time).
    function askPayment(kind, key, trust) {
        if (paymentForm.item === null)
            paymentForm.setSource("PaymentForm.qml", { sioul: sioul, theme: theme })
        paymentForm.item.ask(JSON.parse(sioul.paymentFrom(kind, key, trust || "")))
    }

    // A mail kept as a contract: the Budgets page, the form started from it.
    function contractFromMail(subject, from) {
        window.page = 9
        budgetsPage.startContract(JSON.parse(sioul.contractFrom(subject, from)))
    }

    // A project's route that makes a task of each message: its form
    // (RouteTask.qml; docs/notes-folder.md, "Routes that make tasks"), `route`
    // as its row in the routes' editor has it.
    function routeTaskFor(project, route) {
        routeTask.now().forRoute(project, route)
    }

    // A message's ⋮ ▸ "Make a task of mail like this…": the same form, filled
    // with its sender and the words between brackets in its subject, in its project.
    function routeTaskFromMail(key) {
        routeTask.now().forMail(key)
    }

    // A message's ⋮ ▸ "Make a rule from this…" (the reader's menu, the Porch's
    // row's): the rule's editor filled from the message (its sender's proven
    // domain, what Sioul recognises in it, its project; docs/rules.md), over
    // the page where it was asked; saved, the status line says the rule.
    function ruleFromMail(key) {
        const draft = JSON.parse(sioul.ruleFromMessage(key) || "null")
        if (draft === null || draft.error) {
            sioul.status = draft ? draft.error : ""
            return
        }
        ruleEditor.now().edit(draft)
    }

    // Mail ⚙ at one of its settings ("mail.filter", "topics.list"): where a
    // rule kept in Mail (a filter, a topic's senders) is changed.
    function openMailSetting(key) {
        cardHost.leave()
        window.page = 2
        if (!mailPage)
            return
        mailPage.openSettings(true)
        Qt.callLater(() => mailPage.scrollSettings(key))
    }

    // The search's "Make it a filter…" (MailSearch.qml): a new mail filter with
    // its conditions ({conditions, match}, as the filters write them), last in
    // the list, its editor open in Mail ▸ ⚙ to choose what it does.
    function newFilterFrom(conditions) {
        const problem = sioul.newMailFilter(conditions)
        if (problem !== "") {
            sioul.status = problem
            return
        }
        window.page = 2
        if (mailPage)
            mailPage.openSettings(true)
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
        // Under New where it shows; the drawer put away, at the page's top left.
        const under = (window.railShown || window.placesOpen) && places.shown !== null
        newMenu.item.popup(under ? places.shown.addButton : pages, under ? 0 : theme.gap, under ? places.shown.addButton.height : theme.gap)
    }

    // The places' names shown beside their icons, or their icons only (the
    // button at the title bar's left end, or at their foot where the status
    // line is at the bottom; F9; the setting of Settings ▸ Display). On a
    // phone, or in a window as narrow, the drawer pulled out or put away
    // instead. The keyboard's focus on the places or on the button stays on
    // the button, with a ring when it came by the keyboard (`byKeys`).
    function togglePlaces(byKeys) {
        if (window.compact) {
            window.placesOpen = !window.placesOpen
            return
        }
        const button = window.namesButton
        let onThem = false
        for (let item = window.activeFocusItem; item !== null; item = item.parent)
            onThem = onThem || item === places || (button !== null && item === button)
        sioul.setSetting("places_named", window.placesNamed ? "false" : "true")
        if (onThem && button !== null)
            button.forceActiveFocus(byKeys ? Qt.TabFocusReason : Qt.MouseFocusReason)
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
        tip.text = button.theme.plain(button.tip)
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

    // The window of the move of Sioul's lists out of the notes folder
    // (StorageMove.qml, made the first time): by itself once, when every
    // device knows the new places; from Settings ▸ Sharing at any time.
    function showStorageMove() {
        if (storageMove.item === null) {
            storageMove.setSource("StorageMove.qml", { sioul: sioul, theme: theme })
        }
        const shown = storageMove.item as StorageMove
        if (shown !== null) {
            shown.show()
        }
    }

    // A meal's, a nap's or the night's question (later, at another time, not
    // today), or "" to note where you stopped alone (Interruption.qml, made
    // the first time); the pages showing them read again.
    function askNeed(key) {
        if (interruption.item === null)
            interruption.setSource("Interruption.qml", { sioul: sioul, theme: theme })
        interruption.item.ask(key)
    }

    // A thing's card, where you are (the unified agenda, part 3), by the
    // reference every row carries (its kind, its source's key, start or date,
    // its task): an appointment's, a task's (a step, a time block, a task
    // done), a meal's, a nap's or the night's, in the card host; a dose its day
    // in Health, its row lit, where its medicine is named; days off where they
    // are set. From a card (`fromCard`): the card it came from kept, one level deep.
    // A dated thing's card for a link or a reminder: its kind and key from its
    // address; false when it has none here (a past payment, a paid invoice).
    function openDated(item, fromCard) {
        const uri = item.uri || ""
        const tail = prefix => decodeURIComponent(uri.slice(prefix.length))
        let kind = "", key = ""
        if (item.kind === "paper") {
            kind = "paper"
            key = item.key || tail("sioul:paper/")
        } else if (item.kind === "contract") {
            kind = "notice"
            key = item.key || tail("sioul:contract/")
        } else if (item.kind === "letter") {
            kind = "letter"
            key = item.key || tail("sioul:letter/")
        } else if (item.kind === "invoice") {
            kind = "invoice"
            key = item.key || tail("sioul:invoice/")
        } else if (item.kind === "budget" && /^sioul:budget\/[^/]+\/\d+$/.test(uri)) {
            kind = "payment"
            key = uri
        }
        if (kind === "" || key === "" || JSON.parse(sioul.datedCard(kind, key) || "{}").there !== true)
            return false
        window.openCard({ kind: "dated", datedKind: kind, srcKey: key, title: item.title || "" }, fromCard)
        return true
    }

    // A paper letter on its page (LettersPage.qml), its card in view: from
    // the Porch's row, a link, a reminder. Made the first time if need be.
    function openLetter(id) {
        cardHost.leave()
        window.page = 15
        Qt.callLater(() => {
            if (window.lettersPage)
                window.lettersPage.openLetter(id || "")
        })
    }

    // A contract's page, or a budget's (a dated card's "Its page").
    function openPage(kind, key) {
        cardHost.leave()
        window.page = 9
        if (kind === "contract")
            budgetsPage.openContract(key)
        // A budget (a person's page's Money: docs/persons.md), opened on the Budgets page.
        else if (kind === "budget")
            budgetsPage.openId = key
    }

    // The agenda shown now: Tasks ▸ The day's while it is in sight (the same page,
    // AgendaPage.qml, given the Day alone), else the Agenda's.
    function agendaShown() {
        const day = window.tasksPage ? window.tasksPage.agendaDay : null
        return day && day.visible ? day : window.agendaPage
    }

    // The agenda's days read again (a date asked changed in its card: its mark follows).
    function agendaAgain() {
        if (window.agendaShown())
            window.agendaShown().load()
    }

    function openCard(ref, fromCard) {
        // A message: through the one opener, in Mail's reader (decision 5), never a card.
        if (ref.kind === "mail") {
            window.openMail(ref)
            return
        }
        if (ref.kind === "dose") {
            window.page = 10
            healthPage.openRow(ref.date, ref.srcKey)
        } else if (ref.kind === "day-off") {
            window.showParameters("time_off")
        } else if (fromCard === true) {
            cardHost.follow(ref)
        } else {
            cardHost.show(ref)
        }
    }

    // Whatever a link leads to: a task's or an event's card where you are
    // (from a card, `fromCard`, the card it came from kept), the rest opened where it lives.
    function openThing(item, fromCard) {
        const id = uri => decodeURIComponent(item.uri.slice(uri.length))
        // A place (a message, a note, a contact, a project, a page): on a phone
        // the card over the page closes first, so that the place shows, even
        // the page already shown. A task and an appointment open their card; a
        // dose and a meal answer a notification as before.
        if (["task", "event", "dose", "need", "stopped"].indexOf(item.kind) < 0)
            cardHost.leave()
        if (item.kind === "task")
            window.openCard({ kind: "task", owner: (item.uri || "").startsWith("sioul:task/") ? id("sioul:task/") : item.key, title: item.title || "" }, fromCard)
        else if (item.kind === "note") {
            // Its path and its heading, each decoded apart: a "#" in a name is written "%23".
            const [path, anchor] = (item.uri || "").slice("sioul:note/".length).split("#")
            const decoded = words => {
                try {
                    return decodeURIComponent(words)
                } catch (malformed) {
                    return words
                }
            }
            window.openNote(decoded(path), decoded(anchor || ""))
        }
        else if (item.kind === "event") {
            // Its time as the row gave it, or as a reminder named it with its file.
            const at = cardHost.eventTime(item.key)
            window.openCard({ kind: "event", srcKey: at.key, srcStart: item.start || at.start, title: item.title || "" }, fromCard)
        } else if (item.kind === "mail") {
            // Its address (a tie's, a link's, a notification's) and the file this device last saw as a hint.
            window.openMail(mailOpener.refOf(item))
        } else if (item.kind === "draft")
            window.openDraft(item.key)
        else if (item.kind === "contact")
            window.openContact(item.key)
        // A person of the household: their page, in Contacts (docs/persons.md).
        else if (item.kind === "person")
            window.openPerson(item.key || decodeURIComponent((item.uri || "").slice("sioul:person/".length)))
        // A money movement: the Budgets page's Movements tab, opened on it.
        else if ((item.uri || "").startsWith("sioul:movement/")) {
            window.page = 9
            budgetsPage.openMovement(item.uri)
        }
        // A paper, a contract, a letter, an invoice, a payment's line: its dated card where
        // you are, when it reads (the unified agenda, part 4); else its page.
        else if (window.openDated(item, fromCard))
            return
        else if (item.kind === "budget")
            window.page = 9
        else if (item.kind === "paper") {
            window.page = 13
            papersPage.open(item.key || decodeURIComponent(item.uri.slice("sioul:paper/".length)))
        } else if (item.kind === "contract") {
            window.openPage("contract", item.key || decodeURIComponent(item.uri.slice("sioul:contract/".length)))
        } else if (item.kind === "letter")
            window.openLetter(item.key || decodeURIComponent((item.uri || "").slice("sioul:letter/".length)))
        // An invoice without a card here (paid, a credit note): its pane in Money ▸ Invoices.
        else if (item.kind === "invoice")
            window.openInvoice(item.key || decodeURIComponent((item.uri || "").slice("sioul:invoice/".length)))
        // A project's address, or as it was written before the one name ("sioul:case/").
        else if (item.kind === "project")
            window.openProject(item.key || decodeURIComponent(item.uri.replace(/^sioul:(project|case)\//, "")))
        else if (item.kind === "health")
            window.page = 10
        // The usual meals, naps and the night, in Settings ▸ Hours at their heading (the
        // Porch's "Set my night", a meal's card, What reaches you ▸ Exceptions, Health's ⚙).
        else if (item.kind === "needs")
            window.showParameters("needs")
        // Who may reach you on one channel: Settings ▸ What reaches you ▸ By person, at that channel.
        else if (item.kind === "reach-mail" || item.kind === "reach-calls" || item.kind === "reach-messages")
            window.showParameters("attention." + item.kind.slice("reach-".length))
        // "Subscribe to a calendar…" from Settings ▸ Hours ▸ Vacations and holidays: the Agenda's ⚙, its form open.
        else if (item.kind === "settings:agendas.subscribe") {
            window.page = 4
            Qt.callLater(() => agendaPage.askSubscribe())
        }
        // A rule kept in Mail ⚙ (Settings ▸ Rules): "mail-settings:mail.filter", "mail-settings:topics.list".
        else if (item.kind.startsWith("mail-settings:"))
            window.openMailSetting(item.kind.slice("mail-settings:".length))
        // A setting, from another tab's link: "settings:attention.pause", What reaches you at the pause's card.
        else if (item.kind.startsWith("settings:"))
            window.showParameters(item.kind.slice("settings:".length))
        // Where Sioul keeps its lists (Settings ▸ Sharing): the window of the move.
        else if (item.kind === "storage-move")
            window.showStorageMove()
        else if (item.kind === "agenda")
            window.page = 4
        else if (item.kind === "sites")
            window.page = 3
        // What reaches you's line of what is set elsewhere: a person's own times are on their sheet, in Contacts.
        else if (item.kind === "contacts")
            window.page = 5
        else if (item.kind === "porch")
            window.page = 0
        // New mail's notification of several messages: the mailbox at its top (decision 3).
        else if (item.kind === "mailbox") {
            window.page = 2
            mailPage.openBoxTop()
        }
        // A message of the phone's home screen card (homecard.rs): on the
        // Porch, in its Reader, while the Porch shows it; else the one opener.
        // Its address (a card of this version), else its file (an older card's).
        else if (item.kind === "card-mail") {
            window.page = 0
            if (window.porchPage)
                window.porchPage.openCardMessage(item.uri || "", item.key || "")
        }
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
        // Anything else, outside Sioul; a link to a paper's or a scan's old place in the notes
        // folder (a task's or an event's, never rewritten) opens the moved file (`storage::moved_to`).
        else if (item.key !== "") {
            const moved = item.key.startsWith("file:") ? sioul.movedFile(item.key) : ""
            Qt.openUrlExternally(moved !== "" ? moved : item.key)
        }
    }

    // A message, from anywhere but Mail's own list and the Porch's own rows (the
    // unified mailbox's §3.3): the one opener (MailOpener.qml), never a folder;
    // Back and Escape return where it was opened from (`mailBack`).
    MailOpener {
        id: mailOpener

        window: window
        sioul: sioul
    }
    property alias mailFrom: mailOpener.from
    function openMail(ref) {
        mailOpener.open(ref)
    }
    function mailBack() {
        mailOpener.back()
    }

    // A site of the Sites page, opened.
    function openSite(id) {
        window.page = 3
        sitesPage.open(id)
    }

    // A project's page.
    function openProject(id) {
        cardHost.leave()
        window.page = 7
        projectsPage.open(id)
    }

    // Money ▸ Invoices, a draft in its editor: a project's Make the invoice, a contact's Bill this client.
    function openInvoiceDraft(id) {
        cardHost.leave()
        window.page = 9
        budgetsPage.openInvoiceDraft(id)
    }

    // Money ▸ Invoices, an issued invoice in its pane.
    function openInvoice(number) {
        cardHost.leave()
        window.page = 9
        budgetsPage.openInvoice(number)
    }

    // The declaration view at a day of its period ("" today's): the Porch's declaration and DES rows.
    function openDeclaration(anchor) {
        cardHost.leave()
        window.page = 9
        budgetsPage.openDeclaration(anchor, -1)
    }

    // A project's tasks, as a board, a list or a calendar.
    function showTasksOf(id, mode) {
        window.page = 1
        tasksPage.showProject(id, mode)
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
        const opened = window.composer.createObject(null, { draftId: id, sioul: sioul, theme: theme, phoneSize: window.phoneGrab })
        // Asked to the front: on Wayland with the token a second Sioul handed on
        // with its link (outside.rs), so that KWin does not keep it behind.
        opened.requestActivate()
        window.drafts = window.drafts.concat([opened])
        opened.finished.connect(() => {
            window.drafts = window.drafts.filter(w => w !== opened)
            Qt.callLater(() => opened.destroy())
        })
    }

    // Copies text onto the system's clipboard (`copyText`); whether it holds it then.
    function copy(text) {
        return sioul.copyText(text)
    }

    // The desktop's colours: with high contrast, or where the system names no
    // colour scheme, they say whether it is dark.
    SystemPalette {
        id: system
    }

    Theme {
        id: theme

        compact: window.compact
        touch: Qt.platform.os === "android" || window.phoneGrab || window.tabletGrab
        // SIOUL_THEME=dark or light forces one, for the window's images.
        // SIOUL_THEME for the window's images, else the setting, else the system's
        // colour scheme, followed as it changes (Windows 10's dark mode included),
        // else the system's colours (Theme.darkFrom).
        readonly property string chosen: sioul.forcedTheme() || (sioul.reading ? JSON.parse(sioul.reading).theme : "")
        readonly property bool highContrast: Qt.styleHints.accessibility.contrastPreference === Qt.ContrastPreference.HighContrast
        dark: theme.darkFrom(chosen, Qt.styleHints.colorScheme, system.window.hslLightness, highContrast)
        readingFamily: sioul.reading ? JSON.parse(sioul.reading).family : ""
        readingSize: sioul.reading ? JSON.parse(sioul.reading).size : 16
        readingSpacing: sioul.reading ? JSON.parse(sioul.reading).spacing : 1.5

        // The icons drawn for these colours, light or dark, whenever they
        // change: the system's turning dark at night, a choice in Settings
        // (desktop.rs, `follow`; Icon.qml reads its name again).
        onDarkChanged: sioul.followIcons(dark, chosen !== "")
        onChosenChanged: sioul.followIcons(dark, chosen !== "")
    }

    Sioul {
        id: sioul
    }

    // Settings ▸ What reaches you, once made (ReachesTab.qml): for the pictures' steps.
    function reachesTab(): var {
        const find = item => {
            if (!item)
                return null
            if (item.objectName === "reachesTab")
                return item
            for (let i = 0; i < item.children.length; i++) {
                const found = find(item.children[i])
                if (found)
                    return found
            }
            return null
        }
        return find(parametersPage)
    }

    // Settings ▸ Words (WordsTab.qml), once its tab was shown: the demo steps open its lines.
    function wordsTab(): var {
        const find = item => {
            if (!item)
                return null
            if (item.objectName === "wordsTab")
                return item
            for (let i = 0; i < item.children.length; i++) {
                const found = find(item.children[i])
                if (found)
                    return found
            }
            return null
        }
        return find(parametersPage)
    }

    // How someone reaches you (PersonSheet.qml, made the first time): a
    // contact's card by its file or UID, or a sender's addresses (a JSON array).
    function openPersonSheet(key, addresses) {
        if (personSheet.item === null)
            personSheet.setSource("PersonSheet.qml", { sioul: sioul, theme: theme })
        personSheet.item.ask(key, addresses)
    }

    // texts: the Texts page (TextsPage.qml), at a conversation by its id when
    // one is given; a device with telephony (a phone, a tablet with a SIM) has
    // its own SMS app, where the conversation opens.
    function openTexts(id) {
        if (sioul.telephony) {
            if (id !== "")
                Qt.openUrlExternally("sms:" + id)
            return
        }
        window.page = 14
        if (id !== "" && window.textsPage)
            window.textsPage.select(id)
    }

    // A setting, by its key, where it lives (SettingDoor.qml): Settings at its
    // tab (the hours, from the Porch), or the ⚙ of the page that reads it.
    function showParameters(key) {
        settingDoor.open(key)
    }
    SettingDoor {
        id: settingDoor

        window: window
        sioul: sioul
    }

    // The sites' one web profile and Bitwarden's page that asks a security key
    // (SitesWeb.qml, held by `sitesWebLoader`, at the end of this file): on
    // computers only, made the first time a site's view or a vault's dialog
    // wants it, from Sites or from Accounts.
    function sitesWeb() {
        if (Qt.platform.os === "android")
            return null
        if (sitesWebLoader.status === Loader.Null)
            sitesWebLoader.setSource("SitesWeb.qml", { sioul: sioul, theme: theme })
        return sitesWebLoader.item
    }
    // What asks a security key for a vault's dialog (VaultUnlock.qml's `keys`):
    // Bitwarden's page, held unseen; none on a phone, which has no Qt WebEngine.
    readonly property var vaultKeys: Qt.platform.os === "android" ? null : (() => window.sitesWeb().keyPage())

    // The Porch's card "Join your other devices": Settings ▸ Your folder and
    // sharing, where the join begins (SharePanel.qml, JoinPanel.qml): "murena",
    // "nextcloud" (its address asked), "folder" (a sync app's).
    function joinDevices(how) {
        window.showParameters("files")
        // Once the page and its panel are made (the panel is, the first time its tab shows).
        Qt.callLater(() => {
            const share = window.parametersPage ? window.parametersPage.share as SharePanel : null
            if (share)
                share.startJoin(how)
        })
    }

    Component.onCompleted: {
        sioul.mark("the window's own objects made")
        window.makePages()
        sioul.mark("the Porch made")
        sioul.start()
        // The places beside the pages, made before the first frame.
        window.placesMade = window.placesMade || window.railShown
        // A desktop's window maximized; a phone's is its screen; the pictures' keep their size.
        if (Qt.platform.os !== "android" && sioul.grabFolder() === "")
            window.showMaximized()
        sioul.mark("Sioul started")
    }

    // Sioul in the system tray (qml-desktop/Tray.qml): a desktop's only.
    Loader {
        id: trayLoader
    }

    // The system tray, on a computer, once the window shows (onFrameSwapped);
    // never while taking the documentation's pictures. Plasma's tray menu is
    // made of widgets: Sioul runs as a widgets application there (cpp/application.cpp).
    function loadTray() {
        if (Qt.platform.os !== "android" && sioul.grabFolder() === "" && trayLoader.status === Loader.Null)
            trayLoader.setSource(Qt.resolvedUrl("../qml-desktop/Tray.qml"), { window: window, sioul: sioul })
    }

    // Closed while the tray holds Sioul: the window hides there, and Sioul goes
    // on behind it (reminders, medicines, mail, the sharing); quitting is the
    // tray's menu or Ctrl+Q. Else, what waits for "Undo" is done before the
    // window goes, not lost, and Sioul quits: on a computer, the other windows
    // closing (a draft, the focus window, all of them while this one is in the
    // tray) never quit it (cpp/application.cpp).
    onClosing: close => {
        if (window.trayHolds && !window.quitting) {
            close.accepted = false
            window.hideToTray()
            return
        }
        for (const open of window.drafts)
            open.close()
        if (window.focusWindow) {
            window.focusWindow.quitting = true
            window.focusWindow.close()
        }
        // What was typed in an invoice's editor, written before Sioul quits (the review of phase 3, F11).
        if (window.budgetsPage)
            window.budgetsPage.flushInvoices()
        sioul.flush()
        if (Qt.platform.os !== "android")
            Qt.quit()
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
                window.bringBack()
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
                window.bringBack()
            }
        }
    }
    readonly property var focusWindow: focusWindowLoader.item

    // The system bars stood in for (`simulateSystemBars`), over everything, for the pictures.
    Item {
        id: simulatedBars

        property var margins: ({ top: 0, right: 0, bottom: 0, left: 0 })

        // Over the window's content and its overlay alike (the overlay shows only while a popup is open).
        parent: frame.Overlay.overlay.parent
        anchors.fill: parent
        z: 1000000
        visible: simulatedBars.margins.top + simulatedBars.margins.right + simulatedBars.margins.bottom + simulatedBars.margins.left > 0
        enabled: false

        Repeater {
            model: [
                { x: 0, y: 0, w: simulatedBars.width, h: simulatedBars.margins.top },
                { x: simulatedBars.width - simulatedBars.margins.right, y: 0, w: simulatedBars.margins.right, h: simulatedBars.height },
                { x: 0, y: simulatedBars.height - simulatedBars.margins.bottom, w: simulatedBars.width, h: simulatedBars.margins.bottom },
                { x: 0, y: 0, w: simulatedBars.margins.left, h: simulatedBars.height }
            ]

            delegate: Rectangle {
                required property var modelData

                x: modelData.x
                y: modelData.y
                width: modelData.w
                height: modelData.h
                color: "#b0202020"
            }
        }
    }

    // A phone's site screen asks for a login from Bitwarden (SiteScreen.java,
    // sitefill.rs): the vault's unlock card and the chooser show here, over
    // the page in view, and the site comes back once answered (SiteFill.qml).
    // Made at the start, on Android only, so that a request that waited for
    // the window while Android had ended Sioul is read as the window comes up.
    Loader {
        active: Qt.platform.os === "android"

        sourceComponent: Component {
            SiteFill {
                sioul: sioul
                theme: theme
            }
        }
    }

    // A calendar file (.ics) opened in Sioul (calendarfile.rs): its events,
    // the agenda and the categories chosen, then recorded. Made the first time.
    Later {
        id: calendarFile

        sourceComponent: Component {
            CalendarFileDialog {
                sioul: sioul
                theme: theme
            }
        }
    }
    Connections {
        target: sioul

        function onCalendarFileOpened(view) {
            calendarFile.now().show(view)
            window.bringBack()
        }

        // A file of contacts read (contactimport.rs: "Import…", or opened with Sioul): its window.
        function onContactsImportChanged() {
            const step = sioul.contactsImport ? JSON.parse(sioul.contactsImport).step : "closed"
            if (step !== "closed" && !contactImport.active) {
                contactImport.now().follow()
                window.bringBack()
            }
        }
    }

    // Contacts imported from a vCard or a CSV file (ContactImport.qml): made the first time one is read.
    Later {
        id: contactImport

        sourceComponent: Component {
            ContactImport {
                sioul: sioul
                theme: theme
            }
        }
    }

    // A route that makes a task of each message (RouteTask.qml): made the
    // first time it is asked for; saved, the open project shows its routes again.
    Later {
        id: routeTask

        sourceComponent: Component {
            RouteTask {
                sioul: sioul
                theme: theme
                onKept: {
                    if (window.projectsPage)
                        window.projectsPage.reload()
                }
            }
        }
    }

    // A rule made from a message (RuleEditor.qml): made the first time it is asked for.
    Later {
        id: ruleEditor

        sourceComponent: Component {
            RuleEditor {
                sioul: sioul
                theme: theme
            }
        }
    }

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
    // A move on the agenda's timeline, as the plan answered it: what the status line
    // says after its Undo's line ({line, more}: "2 tasks moved around it."), and for a
    // meal, a nap or the night, "Every day" beside it ({line, date, key}); null without.
    property var undoMore: null
    property var everyDay: null

    // A meal or a nap had already ("Had it"): watched on the agenda's timeline, as a move is.
    function watchMoved(ref) {
        if (window.agendaShown())
            window.agendaShown().watchMove(ref)
    }

    // Free time on or off: on, the line on where you stopped when a session
    // stopped (GP3); off, the end of work said once, with its key (GP11, GP16).
    function setFreeTime(on) {
        const said = sioul.setFreeTime(on)
        window.freeSaid = on ? "" : said
        if (on && said === "true")
            window.askNeed("")
    }

    // Free time's menu, a right click or a long press on its button: back,
    // the usual end kept, nothing at all; nothing opens when none applies (StatusLine.qml).
    function freeMenu() {
        if (window.statusLine)
            window.statusLine.freeMenu()
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
            readonly property var steps: ({ "actions": grabber.actions, "pim": grabber.pim, "pgp": grabber.pgp, "tasks": grabber.tasks, "move": grabber.move, "links": grabber.links, "projects": grabber.projects, "map": grabber.map, "duplicates": grabber.duplicates, "sites": grabber.sites, "quiet": grabber.quiet, "folders": grabber.folders, "notes": grabber.notes, "collections": grabber.collections, "health": grabber.health, "movetask": grabber.movetask, "google": grabber.google, "github": grabber.github, "batch-a": grabber.batchA, "export": grabber.exportCsv, "noantivirus": grabber.noAntivirus, "share": grabber.share, "share-join": grabber.shareJoin, "share-two": grabber.shareTwo, "parameters": grabber.parameters, "closed-on": grabber.closedOn, "closed-off": grabber.closedOff, "papers": grabber.papers, "budget": grabber.budget, "energy": grabber.energy, "day": grabber.day, "bitwarden-key": grabber.bitwardenKey, "bitwarden-passkey": grabber.bitwardenPasskey, "bitwarden-choose": grabber.bitwardenChoose, "bitwarden-wait": grabber.bitwardenWait, "tray": grabber.traySteps, "areas": grabber.areas, "presets": grabber.presets, "ownership": grabber.ownership, "zoom": grabber.zoom, "accounts": grabber.accounts, "account-tabs": grabber.accountTabs, "batch-11": grabber.batch11, "contracts": grabber.contracts, "bank": grabber.bankSteps, "porch-money": grabber.porchMoney, "porch-notices": grabber.porchNotices, "letters": grabber.lettersSteps, "letters-act": grabber.lettersAct, "demo": grabber.demo, "phone": grabber.phone, "drag": grabber.dragSteps, "taskform": grabber.taskForm, "tiles": grabber.tilesSteps, "review": grabber.review, "site-open": grabber.siteOpen, "site-quit": grabber.siteQuit, "site-during": grabber.siteDuring, "site-share": grabber.siteShare, "site-devices": grabber.siteDevices, "site-colour": grabber.siteColour, "rail": grabber.railSteps, "pauses": grabber.pauseSteps, "blocks": grabber.blockSteps, "unsubscribe": grabber.unsubscribeSteps, "leave-spam": grabber.leaveSpamSteps, "attention": grabber.attentionSteps, "line": grabber.lineSteps, "compose": grabber.composeSteps, "share-panel": grabber.sharePanel, "join": grabber.joinSteps, "share-send": grabber.shareSend, "spam": grabber.spamSteps, "mail-search": grabber.mailSearchSteps, "mail-filters": grabber.mailFilterSteps, "rules": grabber.rulesSteps, "security-key": grabber.securityKeySteps, "health-gpg": grabber.healthGpgSteps, "calls": grabber.callsSteps, "words": grabber.wordsSteps, "texts": grabber.textsSteps, "ai": grabber.aiSteps, "projects-file": grabber.projectsFileSteps, "attachments": grabber.attachmentSteps, "memory": grabber.memorySteps, "rest": grabber.restSteps, "rest-shared": grabber.restSharedSteps, "notes-shared": grabber.notesSharedSteps, "microsoft": grabber.microsoftSteps, "reach": grabber.reachSteps, "steps": grabber.stepSteps, "checklist": grabber.checklistSteps, "composed": grabber.composedSteps, "pair": grabber.pairSteps, "agenda": grabber.agendaSteps, "cards": grabber.cardSteps, "taskviews": grabber.taskViewSteps, "mail-open": grabber.mailOpenSteps, "mailbox": grabber.mailboxSteps, "mailbox-measure": grabber.mailboxMeasureSteps, "safe-area": grabber.safeAreaSteps, "budgets-page": grabber.budgetsPageSteps, "money-tabs": grabber.moneyTabSteps, "shell": grabber.shellSteps, "views": grabber.viewsSteps, "chain": grabber.chainSteps, "letters-page": grabber.lettersPageSteps, "persons": grabber.personsSteps, "crud": grabber.crudSteps, "availability": grabber.availabilitySteps, "topics": grabber.topicSteps, "timeline": grabber.timelineSteps, "eventcard": grabber.eventCardSteps, "invite": grabber.inviteSteps, "contact-address": grabber.contactAddressSteps, "hours": grabber.hoursSteps, "ties": grabber.tiesSteps, "invoices": grabber.invoicesSteps, "address-books": grabber.addressBookSteps, "contact-card": grabber.contactCardSteps, "phones": grabber.phonesSteps, "calendars": grabber.calendarSteps, "storage": grabber.storageSteps, "downloads": grabber.downloadSteps, "household": grabber.householdSteps, "notes-find": grabber.notesFindSteps, "search-tabs": grabber.searchTabSteps })[sioul.grabSteps()] || grabber.pages
            // The documentation's pictures, on the demo profile (tools/demo/screenshots.sh):
            // each place as it is used, a weekday afternoon. Run again on the profile
            // without hours (make-demo.py --no-hours), where everything comes at once:
            // the Porch asking for them, and the budgets of every area together.
            // In French (make-demo.py --language fr), the same places, under its names.
            readonly property bool demoFrench: sioul.text("qt-locale") === "fr_FR"
            // Each page in a phone's layout (SIOUL_GRAB_STEPS=phone), Settings scrolled too.
            // Health's day as its list, then the week it opens: the agenda's, which a
            // finger reads and moves as Health's timeline did (SIOUL_GRAB_STEPS=drag;
            // SIOUL_GRAB_PHONE for a phone's).
            readonly property var dragSteps: [
                () => window.page = 10,
                () => {},
                () => grabber.save("health-day"),
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-week"),
                () => window.close()
            ]
            // The agenda's Day and Week, one timeline of everything with a time
            // (SIOUL_GRAB_STEPS=agenda; SIOUL_GRAB_PHONE for a phone's, then in
            // landscape): today's Day, its Show menu, the agenda filter's list, Agenda ⚙'s
            // agendas and conflicts, the Week (a phone's: a list of days), the next week,
            // What comes and the Month; the items alive in each, said in the log.
            // Pictures alone: nothing is changed.
            // The agenda's links and its page (SIOUL_GRAB_STEPS=crud; SIOUL_GRAB_PHONE for a
            // phone's): your availability shared by a link in the demo profile (no server:
            // shared at once, its link stood in), Agenda ⚙ at its line with the link and
            // Copy; the Busy times agenda's line, its link under its switch; then today's
            // Day, its header without New event where the places show New.
            // The agenda's timeline (SIOUL_GRAB_STEPS=timeline; SIOUL_GRAB_PHONE for a
            // phone's): today's Day, a meal to come chosen, its times over it and its grips;
            // the "+" of an empty quarter; the menu of a meal to come, with "Had it"; the
            // Week, its meals and nights named. Pictures alone: nothing is changed.
            readonly property var timelineSteps: [
                () => window.page = 4,
                () => agendaPage.showDay(window.today),
                () => {},
                () => {},
                () => agendaPage.timelineChoose("meal"),
                () => {},
                () => grabber.saveCounted("timeline-times", agendaPage),
                () => agendaPage.timelinePlus(13 * 60 + 15),
                () => {},
                () => grabber.save("timeline-plus"),
                () => agendaPage.timelineMenu("meal"),
                () => {},
                () => {},
                () => {
                    const menu = cardHost.needMenuNow()
                    if (menu)
                        grabber.savePopup(menu, "timeline-had-menu")
                },
                () => {
                    const menu = cardHost.needMenuNow()
                    if (menu)
                        menu.close()
                },
                () => agendaPage.mode = "week",
                () => {},
                () => {},
                () => grabber.saveCounted("timeline-week", agendaPage),
                () => window.close()
            ]
            // The event card (SIOUL_GRAB_STEPS=eventcard; SIOUL_GRAB_PHONE for a phone's): an
            // appointment's card with its place's buttons and its status, then its map (its own
            // GEO: no geocoder asked; the sandbox has no network, so its tiles stay blank);
            // a tentative one's card, and the week with its outline dashed; the event form's
            // place typed, the places used before suggested (no network: no service asked).
            readonly property var eventCardSteps: [
                () => window.page = 4,
                () => agendaPage.mode = "week",
                () => {},
                () => {},
                () => agendaPage.openTitled("Dr Varga"),
                () => {},
                () => {},
                () => grabber.saveCounted("event-card", cardHost),
                () => cardHost.eventCard.showMap(),
                () => {},
                () => {},
                () => {},
                () => grabber.saveCounted("event-card-map", cardHost),
                () => agendaPage.openTitled(sioul.text("qt-locale").indexOf("fr") === 0 ? "Escalade" : "Climbing"),
                () => {},
                () => {},
                () => grabber.saveCounted("event-card-tentative", cardHost),
                // A meeting with others: its status said, your answer, its people and their answers.
                () => agendaPage.openTitled(sioul.text("qt-locale").indexOf("fr") === 0 ? "Fougères" : "Fernhill"),
                () => {},
                () => {},
                () => grabber.saveCounted("event-card-people", cardHost),
                () => cardHost.close(),
                () => {},
                () => grabber.saveCounted("timeline-tentative-week", agendaPage),
                () => agendaPage.typePlace(sioul.text("qt-locale").indexOf("fr") === 0 ? "Berges" : "River"),
                () => {},
                () => {},
                () => {},
                () => grabber.savePopup(agendaPage.eventForm(), "event-form-place"),
                () => agendaPage.eventForm().close(),
                () => window.close()
            ]
            // A meeting you organize (SIOUL_GRAB_STEPS=invite; SIOUL_GRAB_PHONE for a phone's): its
            // card with its people and Delete, its form with its guests, a guest suggested from
            // the address book, and what Save says first (its calendar's server says nothing of
            // scheduling: the invitation is drafted). Nothing is saved.
            readonly property var inviteSteps: [
                () => window.page = 4,
                () => agendaPage.mode = "week",
                () => {},
                () => {},
                () => agendaPage.openTitled(sioul.text("qt-locale").indexOf("fr") === 0 ? "Chantier" : "Garden"),
                () => {},
                () => {},
                () => grabber.saveCounted("invite-card", cardHost),
                () => agendaPage.eventForm().edit(cardHost.eventCard.event.key, ""),
                () => {},
                () => {},
                () => grabber.savePopup(agendaPage.eventForm(), "invite-form"),
                () => agendaPage.eventForm().typeGuest("Iri"),
                () => {},
                () => {},
                () => grabber.savePopup(agendaPage.eventForm(), "invite-suggestions"),
                () => agendaPage.eventForm().addGuest("lou@example.net"),
                () => agendaPage.eventForm().save(),
                () => {},
                () => {},
                () => grabber.savePopup(agendaPage.eventForm().inviteQuestion, "invite-ask"),
                () => agendaPage.eventForm().inviteQuestion.close(),
                () => agendaPage.eventForm().close(),
                () => window.close()
            ]
            // A contact's card with its postal address and Directions beside it
            // (SIOUL_GRAB_STEPS=contact-address; AddressLine.qml), its details unfolded.
            readonly property var contactAddressSteps: [
                () => window.page = 5,
                () => {},
                () => contactsPage.open((contactsPage.shown.contacts.find(c => c.name.indexOf("Iris") === 0) || contactsPage.shown.contacts[0]).key),
                () => {},
                () => contactsPage.revealDetails(),
                () => {},
                // Scrolled again once the details are laid out.
                () => contactsPage.revealed += 1,
                () => {},
                () => grabber.save("contact-address"),
                () => window.close()
            ]
            // An office's opening hours (docs/tasks.md, "Offices' hours"): the health cover's card
            // with its hours, their form; the town hall's card, which says neither that it is an
            // organisation's nor holds hours, its section under More with the link that says it is
            // an office's, pressed; its lookup asked and its list (the demo's examples, no
            // network); the call that follows the card, its panel, then unticked with "Follow
            // them"; Contacts ⚙'s map's search service.
            readonly property var hoursSteps: [
                () => {
                    // A computer's at 1400 points, a phone's as it comes (412).
                    if (!window.phoneGrab)
                        window.width = 1400
                    window.page = 5
                },
                () => {},
                () => contactsPage.open((contactsPage.shown.contacts.find(c => c.key.indexOf("demo-contact-healthcover") >= 0) || contactsPage.shown.contacts[0]).key),
                () => {},
                () => {},
                () => grabber.save("hours-card"),
                () => contactsPage.changeHours(),
                () => {},
                () => {},
                () => grabber.saveWindow("hours-form"),
                () => contactsPage.closeHours(),
                () => contactsPage.open((contactsPage.shown.contacts.find(c => c.key.indexOf("demo-contact-townhall") >= 0) || contactsPage.shown.contacts[0]).key),
                () => {},
                () => contactsPage.revealDetails(),
                () => {},
                () => {},
                // Scrolled again to the end once the details are laid out: the section under More in sight on a phone.
                () => contactsPage.revealDetails(),
                () => {},
                () => {},
                () => grabber.save("hours-card-none"),
                () => sioul.markOffice(contactsPage.openKey),
                () => {},
                () => {},
                // The card back at its top: its section beside its numbers now, the mark said with Undo.
                () => contactsPage.raised += 1,
                () => {},
                () => grabber.save("hours-card-office"),
                () => contactsPage.lookUpHours(),
                () => {},
                () => {},
                () => grabber.saveWindow("hours-lookup-ask"),
                () => contactsPage.lookUpAsk(),
                () => {},
                () => {},
                () => {},
                () => grabber.saveWindow("hours-lookup-list"),
                () => contactsPage.closeHours(),
                () => window.openCard({ kind: "task", owner: "demo-task-health-call", title: "" }),
                () => {},
                () => {},
                () => {},
                () => grabber.saveWindow("hours-task"),
                // Unticked: the panel says it does not follow the card's hours, with "Follow them".
                () => {
                    const read = JSON.parse(sioul.task("demo-task-health-call") || "{}")
                    if (read.edit)
                        sioul.saveTask("demo-task-health-call", JSON.stringify(Object.assign({}, read.edit, { office_hours: false, read: read.edit })), "")
                },
                () => cardHost.close(),
                () => {},
                () => window.openCard({ kind: "task", owner: "demo-task-health-call", title: "" }),
                () => {},
                () => {},
                () => {},
                () => grabber.saveWindow("hours-task-follow"),
                () => cardHost.close(),
                () => window.page = 5,
                () => contactsPage.openSettings(true, "map.search"),
                () => {},
                () => {},
                () => {},
                () => grabber.saveWindow("hours-settings"),
                () => contactsPage.openSettings(false, ""),
                () => window.close()
            ]
            readonly property var crudSteps: [
                () => window.page = 4,
                () => agendaPage.showDay(window.today),
                () => sioul.shareAvailability(JSON.stringify({ name: "Public", agendas: ["cloud/personal"], audience: "link", who: "", way: "mirror-link", account: "cloud" })),
                () => agendaPage.openSettings(true, "agendas"),
                () => {},
                () => {},
                () => agendaPage.revealSetting("availabilityLink"),
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/agenda-availability-link.png"),
                // The clipboard takes a link and holds it (`copyText`, read back), said in the log.
                () => console.warn("sioul-copied: " + window.copy("https://cloud.example.org/remote.php/dav/public-calendars/busy-times?export")),
                () => agendaPage.openAgendaLine(JSON.parse(sioul.availability()).items.slice(-1)[0].mirrors[0]),
                () => {},
                () => {},
                () => agendaPage.revealSetting("agendaLink"),
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/agenda-link.png"),
                () => agendaPage.openSettings(false),
                () => {},
                () => grabber.saveCounted("agenda-day", agendaPage),
                () => window.close()
            ]
            // Agenda ⚙ ▸ Your availability (SIOUL_GRAB_STEPS=availability; SIOUL_GRAB_PHONE for a
            // phone's): the demo's availability for Sam, then one more shared by a link (the demo
            // profile asks no server: made at once, its link stood in); the section, then
            // "Share with…" opened on the first card, the way it names for a person.
            // Mail's topics (docs/topics.md): day one's card on the Porch, the
            // mailbox with its rows' topics, one topic's mail, a row's "Topic…",
            // and Mail ▸ ⚙ ▸ Topics (pictures alone).
            // The chain of ties from the demo's website project: the drawing, the outline,
            // "+" on the project, a card beside it and Back to the same picture; what passes
            // the window's right edge said in the log, at a computer's size and a phone's.
            readonly property var chainSteps: [
                () => window.openChain("sioul:project/fernhill"),
                () => {},
                () => {},
                () => {},
                () => grabber.saveChecked("chain-drawing"),
                () => window.chainPage.outlineShown = true,
                () => {},
                () => grabber.saveChecked("chain-outline"),
                () => window.chainPage.plus(0),
                () => {},
                () => {},
                () => {},
                () => grabber.saveChecked("chain-plus"),
                () => window.chainPage.outlineShown = false,
                () => {},
                () => grabber.saveChecked("chain-plus-drawing"),
                () => window.chainPage.openAt(1),
                () => {},
                () => {},
                () => grabber.saveChecked("chain-card"),
                () => window.goBack(),
                () => {},
                () => grabber.saveChecked("chain-back"),
                () => window.close()
            ]
            readonly property var topicSteps: [
                () => window.page = 0,
                () => {},
                () => {},
                () => grabber.saveWindow("topics-porch"),
                () => window.page = 2,
                () => {},
                () => {},
                () => grabber.saveWindow("topics-mail"),
                () => mailPage.chooseTopic("bank"),
                () => {},
                () => {},
                () => grabber.saveWindow("topics-mail-bank"),
                () => mailPage.chooseTopic(""),
                () => {},
                () => mailPage.askTopicOfFirst(),
                () => {},
                () => grabber.savePopup(mailPage.topicMenu, "topics-menu"),
                () => mailPage.topicMenu.close(),
                () => mailPage.openSettings(true),
                () => mailPage.scrollSettings("topics.list"),
                () => {},
                () => {},
                () => grabber.saveWindow("topics-settings"),
                () => mailPage.openSettings(false),
                () => window.close()
            ]
            readonly property var availabilitySteps: [
                () => window.page = 4,
                () => agendaPage.showDay(window.today),
                () => sioul.shareAvailability(JSON.stringify({ name: "Public", agendas: ["cloud/personal"], audience: "link", who: "", way: "mirror-link", account: "cloud" })),
                () => agendaPage.openSettings(true, "availability"),
                () => {},
                () => {},
                // A phone's panel scrolled to the section's own top (its row's title above it on a computer).
                () => { if (agendaPage.narrow) agendaPage.revealSetting("availabilitySetup") },
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/agenda-availability.png"),
                () => agendaPage.openAvailabilityChooser(),
                () => {},
                () => agendaPage.askAvailability("sam"),
                () => {},
                () => agendaPage.revealSetting("availabilityChooser"),
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/agenda-availability-chooser.png"),
                () => window.close()
            ]
            readonly property var agendaSteps: [
                () => window.page = 4,
                () => agendaPage.showDay(window.today),
                () => {},
                () => {},
                () => grabber.saveCounted("agenda-day", agendaPage),
                () => agendaPage.openShow(),
                () => {},
                () => grabber.savePopup(agendaPage.openShow(), "agenda-show"),
                () => agendaPage.closeShow(),
                // The agenda filter's list (several agendas), then Agenda ⚙: the agendas, the conflicts.
                () => agendaPage.openFilter(),
                () => {},
                () => grabber.savePopup(agendaPage.openFilter(), "agenda-filter"),
                () => agendaPage.closeFilter(),
                // Who does what (part 4): Léa's week; then the swimming lesson's card from it,
                // who takes it (part 3).
                () => agendaPage.whoDoesWhat("cloud/lea"),
                () => {},
                () => {},
                () => grabber.savePopup(agendaPage.whoDoesWhat("cloud/lea"), "agenda-who-does-what"),
                () => {
                    const week = agendaPage.whoDoesWhat("cloud/lea")
                    let row = null
                    for (const day of week.shown.days || []) {
                        for (const item of day.items)
                            row = row || item
                    }
                    if (row)
                        week.openRow(row)
                },
                () => {},
                () => {},
                () => grabber.saveCounted("agenda-child-card", cardHost),
                () => window.goBack(),
                () => agendaPage.openSettings(true, "agendas"),
                () => {},
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/agenda-settings-agendas.png"),
                // One agenda's line open (part 5): its name, colour, removal and who sees it.
                () => agendaPage.openAgendaLine("cloud/personal"),
                () => {},
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/agenda-settings-agenda-line.png"),
                () => agendaPage.openAgendaLine("cloud/personal"),
                () => agendaPage.openSettings(true, "conflicts"),
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/agenda-settings-conflicts.png"),
                () => agendaPage.openSettings(false),
                () => agendaPage.mode = "week",
                () => {},
                () => {},
                () => grabber.saveCounted("agenda-week", agendaPage),
                () => agendaPage.move(1),
                () => {},
                () => {},
                () => grabber.saveCounted("agenda-next-week", agendaPage),
                // What comes and the Month (the unified agenda, part 5), from today.
                () => {
                    agendaPage.anchor = new Date(window.today + "T12:00:00")
                    agendaPage.mode = "agenda"
                },
                () => {},
                () => {},
                () => grabber.saveCounted("agenda-comes", agendaPage),
                () => agendaPage.mode = "month",
                () => {},
                () => {},
                () => grabber.saveCounted("agenda-month", agendaPage),
                // Back to next week, which the next step leaves for this week.
                () => {
                    agendaPage.anchor = new Date(window.today + "T12:00:00")
                    agendaPage.mode = "week"
                    agendaPage.move(1)
                },
                () => {},
                () => {
                    agendaPage.move(-1)
                    if (window.phoneGrab) {
                        window.width = 891
                        window.height = 412
                    }
                },
                () => {},
                () => grabber.saveCounted("agenda-week-wide", agendaPage),
                () => agendaPage.showDay(window.today),
                () => {},
                () => {},
                () => grabber.saveCounted("agenda-day-wide", agendaPage),
                () => window.close()
            ]
            // Every object's card where you are (SIOUL_GRAB_STEPS=cards; SIOUL_GRAB_PHONE
            // for a phone's): from today's Day, an appointment's, a step's (its
            // task's), a meal's and the night's cards, then Back; a task's from Tasks
            // beside its list; then its first tie (a note), which on a phone shows
            // in place of the card. The items alive under the host, said in the log.
            // Pictures alone: nothing is changed.
            // The task views (composed tasks §7.2): Now, the List with its tasks folded and one
            // opened, the Board, the Gantt, a composed task's details; each counted.
            readonly property var taskViewSteps: [
                () => window.page = 1,
                () => tasksPage.mode = "now",
                () => {},
                () => {},
                () => grabber.saveCounted("tasks-now", tasksPage),
                () => tasksPage.mode = "list",
                () => {},
                () => {},
                () => grabber.saveCounted("tasks-list", tasksPage),
                () => sioul.foldTask("demo-task-mockup", true),
                () => {},
                () => {},
                () => grabber.saveCounted("tasks-list-open", tasksPage),
                () => sioul.foldTask("demo-task-mockup", false),
                () => tasksPage.mode = "board",
                () => {},
                () => {},
                () => grabber.saveCounted("tasks-board", tasksPage),
                () => tasksPage.mode = "timeline",
                () => {},
                () => {},
                () => grabber.saveCounted("tasks-gantt", tasksPage),
                () => tasksPage.open("demo-task-doctor-prep"),
                () => {},
                () => {},
                () => grabber.saveCounted("tasks-details", cardHost),
                // A phone held sideways: the List and Now (the leftovers round).
                () => {
                    cardHost.leave()
                    if (window.phoneGrab) {
                        window.width = 891
                        window.height = 412
                    }
                    tasksPage.mode = "list"
                },
                () => {},
                () => {},
                () => {
                    if (window.phoneGrab)
                        grabber.saveCounted("tasks-list-wide", tasksPage)
                },
                () => tasksPage.mode = "now",
                () => {},
                () => {},
                () => {
                    if (window.phoneGrab)
                        grabber.saveCounted("tasks-now-wide", tasksPage)
                },
                () => window.close()
            ]
            readonly property var cardSteps: [
                () => window.page = 4,
                () => agendaPage.showDay(window.today),
                () => {},
                () => {},
                () => grabber.openFirstOf("event"),
                () => {},
                () => grabber.saveCounted("card-event", cardHost),
                () => grabber.openFirstOf("step"),
                () => {},
                () => grabber.saveCounted("card-step", cardHost),
                () => grabber.openFirstOf("meal"),
                () => {},
                () => grabber.saveCounted("card-meal", cardHost),
                () => grabber.openFirstOf("night"),
                () => {},
                () => grabber.saveCounted("card-night", cardHost),
                () => window.goBack(),
                () => {},
                () => grabber.save("card-closed"),
                // A date asked's card, from the Week's heads (the unified agenda, part 4).
                () => agendaPage.mode = "week",
                () => {},
                () => {},
                () => grabber.openFirstOf("dated"),
                () => {},
                () => grabber.saveCounted("card-dated", cardHost),
                () => window.goBack(),
                () => window.page = 1,
                () => tasksPage.mode = "list",
                () => {},
                () => tasksPage.openFirst(),
                () => {},
                () => grabber.saveCounted("card-task-in-tasks", cardHost),
                () => grabber.openFirstTie(),
                () => {},
                () => grabber.save("card-tie-opened"),
                () => window.close()
            ]
            // Links that open a message (the unified mailbox's part 0): a message
            // opened as a tie opens it, Mail beside; one that is not on this
            // device, said in the reader's place. Counted.
            // Android's system bars stood in for (review of 4c77d0c, L9): the event
            // form unfolded, inside the window's safe area, its buttons in view,
            // with the button bar below, then beside (a phone held sideways).
            readonly property var safeAreaSteps: [
                () => window.simulateSystemBars(0, 0, 48, 0),
                () => window.page = 4,
                () => agendaPage.newEvent("2026-10-06"),
                () => agendaPage.fillEvent("Piano lesson", "17:00", "18:00", "weekly"),
                () => {},
                () => grabber.saveWindow("safe-event-form-bottom"),
                // Held sideways: the buttons beside.
                () => {
                    if (window.phoneGrab) {
                        window.width = 891
                        window.height = 412
                    }
                    window.simulateSystemBars(0, 48, 0, 0)
                },
                () => {},
                () => {},
                () => grabber.saveWindow("safe-event-form-side"),
                () => window.simulateSystemBars(0, 0, 0, 0),
                () => window.close()
            ]
            // The mailbox (the unified mailbox's parts 4 and 5): "All addresses"
            // as Mail opens, its filter unfolded (on a phone in the folders'
            // sheet), every area ticked, a selection across two addresses, the
            // Spam chip with Spam and Not spam, a row's menu, a message opened
            // and Back; on a phone (SIOUL_GRAB_PHONE), then again turned sideways.
            // The objects under Mail counted at each (sioul-objects), what passes
            // the window's edge said.
            readonly property var mailboxSteps: [
                () => window.page = 2,
                () => {},
                () => {},
                () => grabber.saveCounted("mailbox", mailPage),
                // A phone: the folders' sheet first, its filter and each address's box.
                () => mailPage.foldersShown = window.phoneGrab,
                () => {},
                () => {
                    if (window.phoneGrab)
                        grabber.saveCounted("mailbox-sheet", mailPage)
                },
                () => mailPage.unfoldAreas(),
                () => {},
                () => {},
                () => grabber.saveWindow("mailbox-filter"),
                // Every area ticked (the resting address's mail too), then the first message of each address chosen.
                () => mailPage.foldAreas(),
                () => mailPage.tickAreas("work+admin+leisure"),
                () => {},
                () => {
                    mailPage.chooseAcross()
                    mailPage.choosing = window.phoneGrab
                },
                () => grabber.saveCounted("mailbox-selection", mailPage),
                () => {
                    mailPage.clearSelection()
                    mailPage.chooseChip("spam")
                },
                () => {},
                () => grabber.saveCounted("mailbox-spam", mailPage),
                () => mailPage.openRowMenu(0),
                () => {},
                () => grabber.saveWindow("mailbox-spam-menu"),
                () => mailPage.closeRowMenu(),
                () => mailPage.chooseChip("all"),
                () => {},
                () => mailPage.clickRow(0, mailPage.listItems[0].key, 0),
                () => {},
                () => {},
                () => grabber.saveCounted("mailbox-opened", mailPage),
                () => mailPage.back(),
                () => {},
                () => grabber.saveCounted("mailbox-back", mailPage),
                () => {
                    if (window.phoneGrab) {
                        window.width = 891
                        window.height = 412
                    }
                },
                () => {},
                () => {},
                () => grabber.saveCounted("mailbox-wide", mailPage),
                () => {
                    mailPage.chooseAcross()
                    mailPage.choosing = window.phoneGrab
                },
                () => grabber.saveCounted("mailbox-wide-selection", mailPage),
                () => {
                    mailPage.clearSelection()
                    mailPage.chooseChip("spam")
                },
                () => {},
                () => grabber.saveCounted("mailbox-wide-spam", mailPage),
                () => mailPage.chooseChip("all"),
                () => {},
                () => mailPage.clickRow(0, mailPage.listItems[0].key, 0),
                () => {},
                () => {},
                () => grabber.saveCounted("mailbox-wide-opened", mailPage),
                () => mailPage.back(),
                () => {},
                () => grabber.saveCounted("mailbox-wide-back", mailPage),
                () => window.close()
            ]
            // The mailbox and a folder's list measured on a large profile
            // (make-demo.py's --bulk): from asking to the rows drawn, the objects alive.
            readonly property var mailboxMeasureSteps: [
                () => {
                    grabber.timeRows("the mailbox", mailPage === null ? null : mailPage)
                    window.page = 2
                    if (mailPage !== null && !mailPage.boxShown)
                        mailPage.openBox(false)
                },
                () => {},
                () => {},
                () => grabber.saveCounted("mailbox-big", mailPage),
                // What a sync's report runs (`backend::show`), its index kept: SIOUL_TIMING's lines.
                () => sioul.refresh(),
                () => {},
                () => {},
                () => {
                    const first = mailPage.accounts.find(a => !a.resting) || mailPage.accounts[0]
                    grabber.timeRows("the first inbox", mailPage)
                    mailPage.openFolder(first.id, "INBOX", false)
                },
                () => {},
                () => {},
                () => grabber.saveCounted("folder-big", mailPage),
                () => window.close()
            ]
            // From now to the first frame after the page's rows next change (the
            // mailbox's or a folder's text published, then drawn), in the log.
            function timeRows(label, page) {
                const began = Date.now()
                const target = () => page !== null ? page : mailPage
                let published = 0
                const drawn = () => {
                    window.frameSwapped.disconnect(drawn)
                    console.info("sioul-perf: " + label + ": rows published in " + published + " ms, drawn in " + (Date.now() - began) + " ms")
                }
                const changed = () => {
                    sioul.mailboxPage.disconnect(changed)
                    target().mailFolderTextChanged.disconnect(changed)
                    published = Date.now() - began
                    window.frameSwapped.connect(drawn)
                }
                Qt.callLater(() => {
                    sioul.mailboxPage.connect(changed)
                    target().mailFolderTextChanged.connect(changed)
                })
            }
            readonly property var mailOpenSteps: [
                () => window.openThing(JSON.parse(sioul.searchThings("", "mail", "") || "[]")[0] || { kind: "mail", uri: "", key: "" }),
                () => {},
                () => {},
                () => grabber.saveCounted("mail-linked", mailPage),
                () => window.openThing({ kind: "mail", uri: "", key: "/stand-in/not-here/cur/1777000000.U1-1.sioul" }),
                () => {},
                () => {},
                () => grabber.saveCounted("mail-not-here", mailPage),
                // The one opener (part 2): an address no copy here holds, whose servers the demo never asks.
                () => window.openMail({ address: "mid:not-on-this-device@example.org" }),
                () => {},
                () => {},
                () => grabber.saveCounted("mail-unreachable", mailPage),
                // A message moved without its server saying where (part 3): on its way to the archive (make-demo.py's note).
                () => window.openMail({ address: "mid:on-its-way@example.org" }),
                () => {},
                () => {},
                () => grabber.saveCounted("mail-on-its-way", mailPage),
                // From Tasks, then Back: Tasks again.
                () => {
                    window.page = 1
                    window.openThing(JSON.parse(sioul.searchThings("", "mail", "") || "[]")[0] || { kind: "mail", uri: "", key: "" })
                },
                () => {},
                () => {},
                () => {
                    mailPage.back()
                    console.warn("mail-open: Back led to the page " + window.page)
                },
                () => {},
                () => grabber.saveCounted("mail-back", window.shownPage),
                () => window.close()
            ]
            // The Search tab of Tasks and of the Agenda, one view (20 November 2026;
            // SIOUL_GRAB_STEPS=search-tabs, SIOUL_GRAB_PHONE for a phone's): Tasks ▸ List on a
            // project that holds open tasks, with its own three filters; Tasks ▸ Search starting
            // on that project, a word typed; the List again, its filters as they were; the
            // Agenda's Search with "Without guests" and a word; then the calendar of what it
            // found unticked in Agendas, the line under the results. Pictures alone, on a demo
            // profile (its agenda filter changed).
            readonly property var searchTabSteps: [
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1280
                        window.height = 860
                    }
                    window.page = 1
                },
                () => {
                    const found = JSON.parse(sioul.findThings(JSON.stringify({ which: "tasks", done: false })) || "{}").found || []
                    const held = found.find(f => f.project_ids.length > 0)
                    tasksPage.showProject(held ? held.project_ids[0] : "", "list")
                },
                () => {},
                () => {},
                () => grabber.save("tasks-list-filters"),
                () => tasksPage.mode = "search",
                () => {},
                () => grabber.searchFor(tasksPage.searchItem),
                () => {},
                () => {},
                () => grabber.save("tasks-search"),
                () => tasksPage.mode = "list",
                () => {},
                () => {},
                () => grabber.save("tasks-list-back"),
                () => window.page = 4,
                () => agendaPage.mode = "search",
                () => {},
                () => {
                    agendaPage.searchItem.guests = "no"
                    agendaPage.searchItem.run()
                },
                () => grabber.searchFor(agendaPage.searchItem),
                () => {},
                () => {},
                () => grabber.save("agenda-search"),
                () => grabber.untickCalendarOf(agendaPage.searchItem),
                () => {},
                () => {},
                () => {},
                () => grabber.save("agenda-search-left-out"),
                () => window.close()
            ]
            // A word of the first thing a search finds, typed in it (four letters at least).
            function searchFor(search) {
                const word = thing => thing.title.split(" ").find(w => w.length >= 4)
                const first = (search ? search.found : []).find(word)
                if (!first) {
                    console.warn("search-tabs: nothing to search for")
                    return
                }
                search.words = word(first)
                search.run()
            }
            // The calendar that holds the first thing the Agenda's search found, unticked in Agendas.
            function untickCalendarOf(search) {
                const first = search ? search.found[0] : null
                const parts = first ? first.key.split("/") : []
                const place = parts.length > 2 ? parts[parts.length - 3] + "/" + parts[parts.length - 2] : ""
                const listed = search ? search.agendas.groups.some(g => g.agendas.some(a => a.id === place)) : false
                if (!listed) {
                    console.warn("search-tabs: no calendar to untick")
                    return
                }
                const problem = sioul.turnAgendasByHand(JSON.stringify([place]), false)
                console.warn("search-tabs: unticked " + place + (problem ? ": " + problem : ""))
            }
            // The task card's first tie to a note (else its first), opened as a tap opens it.
            function openFirstTie() {
                const lists = []
                const walk = item => {
                    if (item.leaveOut !== undefined && item.items !== undefined && item.reload !== undefined)
                        lists.push(item)
                    for (let i = 0; i < item.children.length; ++i)
                        walk(item.children[i])
                }
                if (cardHost.taskPanel)
                    walk(cardHost.taskPanel)
                for (const list of lists) {
                    const tie = list.items.find(t => t.kind === "note") || list.items[0]
                    if (tie) {
                        list.openThing(tie)
                        return
                    }
                }
                console.warn("cards: no tie in the task's card")
            }
            // The first thing of `kind` of the agenda's days (today first), its card opened as a tap opens it.
            function openFirstOf(kind) {
                const shown = JSON.parse(sioul.agenda || "{}")
                for (const day of shown.days || []) {
                    for (const row of (day.timed || []).concat(day.edges || [], day.head || [])) {
                        const item = row.item
                        if (item.kind !== kind)
                            continue
                        const source = item.source || {}
                        const time = item.time || {}
                        window.openCard({ kind: kind, srcKey: source.key || "", srcStart: source.start || 0, srcDate: source.date || "", owner: item.owner || "", date: day.date, itemId: item.id, title: item.title, when: row.when, at: time.at || 0, certainty: typeof item.certainty === "string" ? item.certainty : "", datedKind: source.type === "dated" ? source.kind || "" : "" })
                        return
                    }
                }
                console.warn("cards: no " + kind + " in the days shown")
            }
            // Leaving a list from a message (SIOUL_GRAB_STEPS=unsubscribe; SIOUL_GRAB_PHONE
            // for a phone's): a newsletter open in Mail, Unsubscribe in its row; then a
            // client's message, without it.
            readonly property var unsubscribeSteps: [
                () => window.page = 2,
                () => {},
                () => mailPage.openSubject(grabber.demoFrench ? "Lettres & Pixels" : "Type & Pixels"),
                () => {},
                () => grabber.save("unsubscribe-reader"),
                () => mailPage.openSubject(grabber.demoFrench ? "deux petites modifications" : "two small changes"),
                () => {},
                () => grabber.save("unsubscribe-none"),
                () => window.close()
            ]
            // Leaving the lists of the mail you called spam (SIOUL_GRAB_STEPS=leave-spam, on a
            // profile made with SIOUL_DEMO_LEAVE_SPAM=1; SIOUL_GRAB_PHONE for a phone's): the row
            // in Mail's ⚙, its sheet, then its button, which a demo answers without sending.
            function leaveSpam() {
                return grabber.named(frame.Overlay.overlay, "leaveSpam")
            }
            readonly property var leaveSpamSteps: [
                () => window.page = 2,
                () => {},
                () => mailPage.openSettings(true),
                () => {},
                () => {},
                () => mailPage.scrollSettings("mail.leave_spam"),
                () => {},
                () => {
                    console.warn("leave-spam: " + JSON.stringify(grabber.leaveSpam().found))
                    grabber.saveWindow("leave-spam-row")
                },
                () => grabber.leaveSpam().sheet.open(),
                () => {},
                () => grabber.saveWindow("leave-spam-sheet"),
                () => grabber.leaveSpam().sheet.accept(),
                () => {},
                () => {
                    console.warn("leave-spam: " + sioul.status)
                    grabber.saveWindow("leave-spam-said")
                },
                () => window.close()
            ]
            // The status line (SIOUL_GRAB_STEPS=line; SIOUL_GRAB_PHONE for a phone's), on
            // a demo profile after its working hours: the work day offered to close;
            // then Undo beside it, with the pauses; Free time on; do-not-disturb on; a
            // sentence too long for the line, said whole; on a computer, the title bar
            // of a narrow window. It deletes a new empty draft, then takes it back.
            readonly property var lineSteps: [
                () => window.page = 1,
                () => {},
                () => grabber.save("line-offer"),
                () => sioul.discard(sioul.compose("new", "", "")),
                () => {},
                () => grabber.save("line-undo"),
                () => sioul.undo(),
                () => window.setFreeTime(true),
                () => {},
                () => grabber.save("line-free"),
                () => window.setFreeTime(false),
                () => sioul.dndToggle(true, 0),
                () => {},
                () => grabber.save("line-dnd"),
                () => sioul.dndToggle(false, 0),
                () => sioul.status = sioul.text("tray-said"),
                () => window.statusLine.sayWhole(),
                () => grabber.saveWindow("line-whole"),
                () => {
                    if (!window.phoneGrab)
                        window.width = 700
                },
                () => {},
                () => grabber.save("line-narrow"),
                () => window.close()
            ]
            // The button of what reaches you and its card (SIOUL_GRAB_STEPS=reach; the
            // runner's `phone` at 412 points, what passes the right edge said in the log):
            // As usual, Reachable's lengths, Reachable pressed for an hour, Do not disturb
            // for an hour, then As usual again. On a demo profile: it presses the states.
            readonly property var reachSteps: [
                () => window.page = 0,
                () => {},
                () => grabber.saveChecked("reach-line"),
                () => window.statusLine.reachButton.openCard(),
                () => {},
                () => grabber.savePopup(window.statusLine.reachButton.cardLoader.item, "reach-card"),
                () => window.statusLine.reachButton.cardLoader.item.choosing = "reachable",
                () => {},
                () => grabber.savePopup(window.statusLine.reachButton.cardLoader.item, "reach-card-lengths"),
                () => window.statusLine.reachButton.cardLoader.item.chooseLength("reachable", "60"),
                () => {},
                () => grabber.savePopup(window.statusLine.reachButton.cardLoader.item, "reach-card-reachable"),
                () => window.statusLine.reachButton.cardLoader.item.close(),
                () => {},
                () => grabber.saveChecked("reach-line-reachable"),
                () => sioul.dndToggle(true, 60),
                () => {},
                () => grabber.saveChecked("reach-line-dnd"),
                () => window.statusLine.reachButton.openCard(),
                () => window.statusLine.reachButton.cardLoader.item.choosing = "dnd",
                () => {},
                () => grabber.savePopup(window.statusLine.reachButton.cardLoader.item, "reach-card-dnd"),
                () => window.statusLine.reachButton.cardLoader.item.usual(),
                () => window.statusLine.reachButton.cardLoader.item.close(),
                () => window.statusLine.reachButton.openMenu(),
                () => {},
                () => grabber.saveWindow("reach-menu"),
                () => window.statusLine.reachButton.menuLoader.item.close(),
                () => window.close()
            ]
            // The writing window (SIOUL_GRAB_STEPS=compose; the runner's `phone` at a
            // phone's width, what passes its right edge said in the log): an answer to
            // all of a message with a long subject, two addresses to answer and a copy
            // (make-demo.py --compose; else the first message), then in preview; a new
            // message, then written and deleted. On a demo profile: it makes drafts.
            readonly property var composeSteps: [
                () => window.page = 2,
                () => {
                    mailPage.openSubject(grabber.demoFrench ? "date de mise en ligne" : "the launch date")
                    if (mailPage.openKey === "")
                        mailPage.openFirst()
                },
                () => window.compose("reply-all", mailPage.openKey),
                () => {},
                () => grabber.saveDraft("compose-reply"),
                () => {
                    if (window.drafts.length > 0)
                        window.drafts[0].previewing = true
                },
                () => grabber.saveDraft("compose-reply-preview"),
                () => grabber.closeDrafts(),
                () => window.compose("new", ""),
                () => {},
                () => grabber.saveDraft("compose-new"),
                () => {
                    if (window.drafts.length > 0)
                        window.drafts[0].fill("Ana Lima <ana.lima@example.org>, Bruno Costa <bruno.costa@example.org>", grabber.demoFrench ? "Les horaires d’hiver de la médiathèque, le coin des enfants et la lettre" : "The library's winter hours, the children's corner and the newsletter", "")
                },
                () => grabber.saveDraft("compose-new-saved"),
                () => {
                    if (window.drafts.length > 0)
                        window.drafts[0].discard()
                },
                () => {},
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
                // The agenda's place (Texts comes before it where there is no telephony).
                () => places.shown.repeater.itemAt(sioul.telephony ? 4 : 5).forceActiveFocus(Qt.TabFocusReason),
                () => {},
                () => grabber.saveWindow("rail-focus"),
                () => {
                    window.page = 0
                    sioul.setSetting("places_named", "true")
                },
                () => {},
                () => grabber.save("rail-named"),
                // The button at the title bar's left end: the icons only again.
                () => window.togglePlaces(false),
                () => {},
                () => grabber.save("rail-icons"),
                () => window.showNewMenu(),
                () => {},
                () => grabber.saveWindow("rail-new"),
                () => newMenu.item.close(),
                () => window.height = 540,
                () => {},
                () => grabber.save("rail-short"),
                // The last place.
                () => places.shown.repeater.itemAt(places.shown.repeater.count - 1).forceActiveFocus(Qt.TabFocusReason),
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
                () => {
                    window.page = 7
                    projectsPage.showAll()
                },
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
                // The Porch as it is lived with: the cards of the first days answered "Leave as is",
                // as the profile says (make-demo.py writes their flags, unless --first-day). The
                // Porch reads those flags as it is made, before `start` loads them: pressed here.
                () => {
                    for (const [flag, card] of [["porch-join-left", "joinCard"], ["topics-day-one-left", "topicsDayOne"]]) {
                        const panel = grabber.named(window.porchPage, card)
                        if (sioul.viewFlag(flag) && panel !== null && panel.visible)
                            grabber.leaveAsIs(panel)
                    }
                },
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
                // The second account's last movements unfolded, each saying where it went and why.
                () => {
                    if (!window.moment.hours) {
                        budgetsPage.bankNow().unfold("paypal")
                        budgetsPage.showBank()
                    }
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
                // Whether the card shows its number and circle rows, said in the log: some runs
                // drew it without them, with the same steps and the same data.
                () => {
                    const sheet = grabber.named(contactsPage, "personSheetButton")
                    console.warn("demo: contact card, its sheet's button " + (sheet !== null && sheet.visible ? "shown at " + Math.round(sheet.mapToItem(contactsPage, 0, 0).y) : "not shown"))
                },
                () => grabber.save("contacts"),
                // Notes as folders, the project's open.
                () => {
                    window.page = 6
                    sioul.setNotesView("folder", "all")
                },
                () => notesPage.unfold(grabber.demoFrench ? "Projets" : "Projects"),
                () => notesPage.open(grabber.demoFrench ? "Projets/Médiathèque des Fougères.md" : "Projects/Fernhill Library.md"),
                () => {},
                () => grabber.save("notes"),
                () => {
                    window.page = 7
                    projectsPage.showAll()
                    projectsPage.open("fernhill")
                },
                () => {},
                () => grabber.save("projects"),
                // Time: last week, a whole one, whatever the day (and the hour:
                // every area ticked in its filter).
                () => {
                    window.page = 8
                    timePage.showAll()
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
                // Its week, and its settings (the errands, the pause to move, the limit on chats).
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-week"),
                () => healthPage.showDay(),
                () => healthPage.settingsPanel().open(),
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
                // Settings, its tabs across the window, on the first.
                () => {
                    window.page = 12
                    parametersPage.open("look")
                },
                () => {},
                () => grabber.save("settings-tabs"),
                // Settings ▸ Devices, this computer's card.
                () => window.showParameters("devices"),
                () => {},
                () => {},
                () => grabber.save("settings-devices"),
                () => window.showParameters("attention"),
                () => {},
                () => {},
                () => grabber.save("settings-attention"),
                () => {
                    window.page = 12
                    parametersPage.section = "hours"
                },
                () => {},
                () => grabber.save("settings-hours"),
                // The usual meals, naps and night, under their heading in the same tab.
                () => window.showParameters("needs"),
                () => {},
                () => {},
                () => grabber.save("settings-needs"),
                // Reachable for an hour (part 3: "Work now" went): the Porch open, every lane and address.
                () => {
                    window.page = 0
                    sioul.reachChoose("reachable", "60")
                },
                () => {},
                () => grabber.save("reachable"),
                () => sioul.reachChoose("usual", ""),
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
            // Memory (SIOUL_GRAB_STEPS=memory, tools/measure-memory.py, on a demo profile):
            // a minute at rest on the Porch, every page and form as "pages" opens them
            // (each page made as it is first shown, as in use, no picture taken), the
            // same once the scripts' garbage is collected, then the profile's sites
            // shown one by one. "sioul-memory: <moment>" in the log at each moment
            // measured, then half a minute held for the measure.
            readonly property bool pictures: !["memory", "rest", "rest-shared"].includes(sioul.grabSteps())
            function held(steps) {
                return Array(steps).fill(() => {})
            }
            // Rest (SIOUL_GRAB_STEPS=rest, tools/measure-idle.py, on a demo profile):
            // the window left alone, as through a day beside other apps: the Porch
            // shown, the window hidden as to the tray, then the profile's sites
            // opened and kept behind the Porch, then hidden too, then hidden
            // with a site in view, three minutes each. "sioul-rest: <moment>"
            // in the log as each begins; meanwhile nothing is done, the steps'
            // own timer stopped until the moment's end. Three minutes of the
            // Porch first ("warm", not measured), then the window hidden and
            // brought back once: the start's last work done before the moments
            // (the first minutes' writes, and the views made again after them,
            // every message triaged again when they changed what decides it).
            function rested(moment) {
                return [() => {
                    console.info("sioul-rest: " + moment)
                    grabber.interval = 180000
                }, () => grabber.interval = 1500]
            }
            readonly property var restSteps: grabber.held(20).concat(
                grabber.rested("warm"), [() => window.hide()], grabber.held(4), [() => window.bringBack()], grabber.held(10), grabber.rested("porch"),
                [() => window.hide()], grabber.rested("hidden"),
                [() => window.bringBack(), () => window.page = 3], grabber.held(2),
                [0, 1, 2, 3, 4].reduce((all, i) => all.concat([() => {
                    if (sitesPage.sites.length > i)
                        sitesPage.open(sitesPage.sites[i].id)
                }], grabber.held(4)), []),
                [() => window.page = 0], grabber.held(2), grabber.rested("sites"),
                [() => window.hide()], grabber.rested("sites-hidden"),
                [() => window.bringBack(), () => window.page = 3, () => {
                    if (sitesPage.sites.length > 0)
                        sitesPage.open(sitesPage.sites[0].id)
                }, () => window.hide()], grabber.rested("site-hidden"),
                [() => console.info("sioul-rest: end"), () => window.close()])
            // "rest-shared": the same, sharing started first in a folder of the
            // run's own (as "share" starts it), so that its watch and its minute's
            // exchange run too.
            readonly property var restSharedSteps: [() => sioul.status = sioul.startSharing(grabber.folder + "/shared", "quiet harbour at dawn", "quiet harbour at dawn"), () => console.info("sioul-sharing: " + sioul.status)].concat(grabber.restSteps)
            readonly property var memorySteps: grabber.held(40).concat(
                [() => console.info("sioul-memory: rest")], grabber.held(20),
                grabber.pages.slice(0, -1), grabber.held(4),
                [() => console.info("sioul-memory: pages")], grabber.held(20),
                [() => gc(), () => console.info("sioul-memory: pages-gc")], grabber.held(20),
                [() => window.page = 3],
                [0, 1, 2, 3, 4].reduce((all, i) => all.concat([() => {
                    if (sitesPage.sites.length > i)
                        sitesPage.open(sitesPage.sites[i].id)
                }], grabber.held(6)), []),
                [() => console.info("sioul-memory: sites")], grabber.held(20),
                [() => window.close()])
            // colour: the test site in the screen's colours, then calmer too, then
            // neither (tools/check-colour.py reads the patches back from each picture).
            readonly property var siteColour: grabber.siteOpen.concat(Array(5).fill(() => {}), [
                () => grabber.save("colour-screen"),
                () => sioul.setSetting("calmer_colours", JSON.stringify("little")),
                () => {},
                () => grabber.save("colour-calmer"),
                () => sioul.setSetting("screen_colours", "false"),
                () => sioul.setSetting("calmer_colours", JSON.stringify("")),
                () => {},
                () => grabber.save("colour-plain"),
                () => window.close()
            ])
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
            // The devices of calls (tools/check-sites.py devices), on Chromium's fake
            // devices: a camera and a microphone chosen before Sioul started, four
            // test sites opened (a mailbox allowed no device, a call in a pop-up, in a
            // frame of another site, in the page), then the camera, the speaker and the
            // microphone changed during the calls, as from the call button; the page's
            // own camera button; a camera not plugged in; every call hung up; the
            // page reloaded after one more change. Each page is told the phase
            // first, so that it says what it sees under it.
            readonly property var siteDevices: {
                const calls = () => ["test-chat", "test-frame"].map(id => sitesPage.viewOf(id)).filter(v => v !== null)
                    .concat(sitesPage.popups.filter(p => sitesPage.popupOpen(p)).map(p => p.view))
                const phase = name => calls().forEach(view => sitesPage.everyFrame(view, "window.setPhase && setPhase('" + name + "')"))
                const calling = () => JSON.stringify({ chat: sitesPage.calling("test-chat"), frame: sitesPage.calling("test-frame"),
                                                       popup: sitesPage.calling("test-popup"), bank: sitesPage.calling("test-bank") })
                const wait = n => Array(n).fill(() => {})
                return [
                    () => window.page = 3,
                    () => sitesPage.open("test-bank"),
                    () => sitesPage.open("test-popup"),
                    () => sitesPage.open("test-frame"),
                    () => sitesPage.open("test-chat")
                ].concat(wait(4), [
                    () => {
                        phase("camera")
                        sitesPage.setCallDevice("camera", "fake_device_1")
                    }
                ], wait(2), [
                    () => sitesPage.callPanel.open(),
                    () => sitesPage.callPanel.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/call-panel.png")),
                    () => sitesPage.callPanel.close(),
                    () => {
                        phase("speaker")
                        sitesPage.setCallDevice("speaker", "Fake Audio Output 2")
                    }
                ], wait(2), [
                    () => {
                        phase("microphone")
                        sitesPage.setCallDevice("microphone", "Fake Audio Input 2")
                    }
                ], wait(2), [
                    () => {
                        phase("mute")
                        sitesPage.everyFrame(sitesPage.viewOf("test-chat"), "mute(true)")
                    }
                ], wait(2), [
                    () => {
                        phase("unmute")
                        sitesPage.everyFrame(sitesPage.viewOf("test-chat"), "mute(false)")
                    }
                ], wait(2), [
                    () => {
                        phase("missing")
                        sitesPage.setCallDevice("camera", "Unplugged camera")
                    },
                    () => {},
                    () => console.warn("sioul-devices: the line above the site says: " + sitesPage.problem),
                    () => console.warn("sioul-devices: calling " + calling()),
                    () => {
                        phase("hangup")
                        calls().forEach(view => sitesPage.everyFrame(view, "window.hangup && hangup()"))
                    }
                ], wait(2), [
                    () => console.warn("sioul-devices: after hanging up, calling " + calling()),
                    // A page loaded after a change starts its call with the devices of now.
                    () => sitesPage.setCallDevice("camera", "fake_device_0"),
                    () => sitesPage.viewOf("test-chat").reload()
                ], wait(4), [
                    () => window.close()
                ])
            }
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
            // Sites, on a local test page: opened, its notification kept, the Porch telling it;
            // then a made-up login filled by the page's own path, Rust's guarded call (with
            // SIOUL_TEST_VAULT in a test build: a login of id "test"; else the vault is locked).
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
                () => sioul.bitwardenState(),
                () => sitesPage.fillChosen("test"),
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
                () => {
                    window.page = 7
                    projectsPage.showAll()
                },
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
            // tiles in its panel, then in an event's form. With SIOUL_GRAB_PHONE,
            // a phone's. On a scratch profile: it makes tasks and saves ratings.
            // The costs as tiles (CostTiles.qml) at the window's least width, 1100
            // and 1600 (a phone's with SIOUL_GRAB_PHONE): a new task's tiles, unsaid
            // then rated; the same task as one felt before, its faint values; an
            // event's form; "How was it?" with the forecast's marks. Each width in
            // the log, with the panel's width and the tiles' columns. On a demo
            // profile: it makes tasks and events, rates them and keeps how one felt.
            readonly property var tilesSteps: {
                const french = grabber.demoFrench
                const felt = french ? "Appeler la banque" : "Call the bank"
                const fresh = french ? "Écrire à l’école" : "Write to the school"
                const widths = window.phoneGrab ? [412] : [window.minimumWidth, 1100, 1600]
                // The event's form, over the window: the overlay's one child narrower than it.
                const saveEvent = name => {
                    const over = frame.Overlay.overlay
                    for (const item of over.children)
                        if (item.visible && item.width < over.width) {
                            item.grabToImage(result => result.saveToFile(grabber.folder + "/" + name + ".png"))
                            console.warn("tiles: " + name + ": the form " + Math.round(item.width) + " wide")
                            if (window.phoneGrab)
                                grabber.overflow(name, item)
                        }
                }
                const tiles = name => {
                    const t = tasksPage.panel.costTiles
                    console.warn("tiles: " + name + ": window " + window.width + ", panel " + Math.round(tasksPage.panel.width) + ", tiles " + Math.round(t.width) + " in " + t.columns + " column(s), words " + (t.beside ? "beside the gauges" : "above them"))
                }
                const steps = [
                    () => window.page = 1,
                    () => {},
                    // A task felt before: made, its worry foreseen at 8, done, how it went kept.
                    () => window.newThing("task"),
                    () => {},
                    () => tasksPage.panel.confirmTitle(felt),
                    () => {},
                    () => {},
                    () => {
                        const found = JSON.parse(sioul.searchTasks(felt, "") || "[]")
                        grabber.doneUid = found.length > 0 ? found[0].uid : ""
                        tasksPage.panel.rate("anxiety", 8)
                    },
                    () => {},
                    () => sioul.setTaskStatus(grabber.doneUid, "completed"),
                    () => {},
                    () => console.warn("tiles: how it felt kept: " + (sioul.setFelt(grabber.doneUid, JSON.stringify({ cognitive: 2, emotional: null, anxiety: 3, body: null, gain: 6 })) || "yes")),
                    () => tasksPage.closePanel(),
                    () => {}
                ]
                for (const w of widths) {
                    const name = "tiles-" + w
                    steps.push(
                        () => {
                            window.width = w
                            window.height = window.phoneGrab ? 891 : 1000
                            window.page = 1
                        },
                        () => {},
                        // A new task: its tiles unsaid, then two given.
                        () => window.newThing("task"),
                        () => {},
                        () => tasksPage.panel.confirmTitle(fresh),
                        () => {},
                        () => {},
                        () => tasksPage.panel.showRatings(),
                        () => {},
                        () => {
                            grabber.save(name + "-task")
                            tiles(name + "-task")
                        },
                        () => {
                            tasksPage.panel.rate("anxiety", 7)
                            tasksPage.panel.rate("gain", 0)
                        },
                        () => {},
                        () => {},
                        () => tasksPage.panel.showRatings(),
                        () => grabber.save(name + "-task-rated"),
                        () => tasksPage.closePanel(),
                        // The task felt before, made again: its faint values.
                        () => window.newThing("task"),
                        () => {},
                        () => tasksPage.panel.confirmTitle(felt),
                        () => {},
                        () => {},
                        () => tasksPage.panel.showRatings(),
                        () => {},
                        () => {
                            grabber.save(name + "-proposed")
                            console.warn("tiles: " + name + "-proposed: faint " + JSON.stringify(tasksPage.panel.costTiles.proposed) + " from " + tasksPage.panel.costTiles.proposedFrom)
                        },
                        () => tasksPage.closePanel(),
                        // An event's form, its costs in view.
                        () => {
                            window.page = 4
                            agendaPage.newEvent(agendaPage.today())
                        },
                        () => agendaPage.fillEvent(french ? "Rendez-vous à la mairie" : "Town hall appointment", "10:00", "11:00", ""),
                        () => agendaPage.showEventRatings(),
                        () => {},
                        () => {},
                        () => saveEvent(name + "-event"),
                        () => agendaPage.saveEvent(),
                        // "How was it?" on the task felt before: its forecast as marks.
                        () => window.page = 1,
                        () => window.howWasIt(grabber.doneUid),
                        () => {},
                        () => {},
                        () => grabber.savePopup(howWasItForm.item, name + "-felt"),
                        () => howWasItForm.item.close(),
                        () => {}
                    )
                }
                steps.push(() => window.close())
                return steps
            }
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
                // Done: "How was it?" on the status line; its five tiles; a felt rating kept.
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
            // A task of the demo profile (make-demo.py) moved to its other list, then
            // sent: nothing is sent off the network (SIOUL_DEMO=1); from the test
            // build, against a test server holding them, the move goes there.
            // A task's steps in their order (docs/tasks.md, "Steps in order"): the
            // demo's mock-up, a step moved up by its place and then by the keys'
            // way (`shiftStep`), its menu, its form; the order read back each time.
            function stepsSaid(what) {
                const panel = tasksPage.panel
                console.warn("steps: " + what + ": " + (panel && panel.detail ? panel.detail.steps.map(s => s.title).join(" | ") : "none"))
            }
            readonly property var stepSteps: [
                () => window.page = 1,
                () => {},
                () => tasksPage.open("demo-task-mockup"),
                () => {},
                () => grabber.stepsSaid("before"),
                () => grabber.save("steps-before"),
                () => tasksPage.panel.shiftStep(2, -1),
                () => {},
                () => {},
                () => grabber.stepsSaid("last moved up"),
                () => grabber.save("steps-moved"),
                () => tasksPage.panel.shiftStep(0, 1),
                () => {},
                () => {},
                () => grabber.stepsSaid("first moved down"),
                () => tasksPage.panel.showStepMenu(1),
                () => {},
                () => grabber.savePopup(tasksPage.panel.stepMenuItem(), "steps-menu"),
                () => tasksPage.panel.stepMenuItem().close(),
                () => tasksPage.panel.editing = true,
                () => {},
                () => grabber.save("steps-form"),
                () => window.close()
            ]
            // A list to tick (docs/tasks.md, "Checklists"): the demo's sketch, its
            // second item ticked as a tap does; the doctor's appointment, its first;
            // the notes read back each time.
            function eventNamed(words) {
                const shown = JSON.parse(sioul.agenda || "{}")
                for (const day of shown.days || [])
                    for (const row of (day.head || []).concat(day.timed || []))
                        if (row.detail && row.item.source.key.indexOf(words) >= 0)
                            return Object.assign({ key: row.item.source.key, start: row.item.source.start }, row.detail, JSON.parse(sioul.eventDetails(row.item.source.key, row.item.source.start) || "{}"))
                return null
            }
            readonly property var checklistSteps: [
                () => window.page = 1,
                () => {},
                () => tasksPage.open("demo-task-sketch"),
                () => {},
                () => grabber.save("checklist-before"),
                () => console.warn("checklist: task: " + (sioul.checkItem("task", "demo-task-sketch", 3, "toggle") || "ticked")),
                () => {},
                () => {},
                () => console.warn("checklist: task notes: " + JSON.parse(sioul.task("demo-task-sketch")).edit.notes.replace(/\n/g, " | ")),
                () => grabber.save("checklist-after"),
                () => {
                    const event = grabber.eventNamed("demo-event-doctor")
                    console.warn("checklist: event: " + (event ? sioul.checkItem("event", event.key, 1, "done") || "ticked" : "not shown"))
                },
                () => {},
                () => {},
                () => {
                    const event = grabber.eventNamed("demo-event-doctor")
                    console.warn("checklist: event notes: " + (event ? event.notes.replace(/\n/g, " | ") : "not shown"))
                },
                () => window.close()
            ]
            // Composed tasks changed (docs/tasks.md, "Composed tasks"): the demo's quote made a
            // step of the mock-up and a task of its own again, a step folded into its list
            // (asked first, then done), the mock-up done with its last step; each answer read back.
            function stepsOf(uid) {
                const read = JSON.parse(sioul.task(uid) || "{}")
                return read.steps ? read.steps.map(s => s.title + " (" + s.status + ")").join(" | ") + " · notes: " + read.edit.notes.replace(/\n/g, " | ") : "none"
            }
            readonly property var composedSteps: [
                () => window.page = 1,
                () => {},
                () => tasksPage.open("demo-task-mockup"),
                () => console.warn("composed: step of: " + sioul.makeStepOf("demo-task-quote", "demo-task-mockup", false)),
                () => {},
                () => console.warn("composed: steps: " + grabber.stepsOf("demo-task-mockup")),
                () => grabber.save("composed-step-of"),
                () => console.warn("composed: own again: " + sioul.makeStepOf("demo-task-quote", "", false)),
                () => console.warn("composed: fold asked: " + sioul.foldStep("demo-task-photos", false)),
                () => console.warn("composed: fold: " + sioul.foldStep("demo-task-photos", true)),
                () => {},
                () => console.warn("composed: steps: " + grabber.stepsOf("demo-task-mockup")),
                () => console.warn("composed: sketch done: " + sioul.setTaskStatus("demo-task-sketch", "completed")),
                () => console.warn("composed: send done: " + sioul.setTaskStatus("demo-task-send-mockup", "completed")),
                () => {},
                () => console.warn("composed: mock-up: " + JSON.parse(sioul.task("demo-task-mockup") || "{}").card.status),
                () => grabber.save("composed-done"),
                () => window.close()
            ]
            // An appointment and its steps (docs/tasks.md, "An appointment and its steps"): the
            // doctor's appointment, a task made to prepare it, untied and tied again; each answer read back.
            property string pairTask: ""
            function pairSaid(what) {
                const event = grabber.eventNamed("demo-event-doctor")
                const read = JSON.parse(sioul.task(grabber.pairTask) || "{}")
                console.warn("pair: " + what + ": event " + (event ? sioul.eventPairing(event.key) : "not shown") + " · task " + JSON.stringify(read.appointment || null) + " due " + (read.edit ? read.edit.due : ""))
            }
            // A task before one time of a repeating event, and a date asked's hour (docs/tasks.md, "One
            // time of a repeating event", "A date asked and its hour"): a series begun a week ago made,
            // a task's form, its field's rows, a time tied, the task's line and the card of that time.
            property string tiesTask: ""
            function tiesFind(item, name) {
                if (!item)
                    return null
                if (item.objectName === name)
                    return item
                for (const kid of item.children || []) {
                    const found = grabber.tiesFind(kid, name)
                    if (found)
                        return found
                }
                return item.contentItem && item.contentItem !== item ? grabber.tiesFind(item.contentItem, name) : null
            }
            readonly property var tiesSteps: [
                () => {
                    const two = n => (n < 10 ? "0" : "") + n
                    const first = new Date(Date.now() - 7 * 86400000)
                    const day = first.getFullYear() + "-" + two(first.getMonth() + 1) + "-" + two(first.getDate())
                    console.warn("ties: event: " + sioul.saveEvent("", JSON.stringify({ title: "Morning routine", start: day + "T11:00", end: day + "T11:30", repeat: "daily", every: 2 }), ""))
                    const blank = JSON.parse(sioul.newTaskForm("", "", 0, ""))
                    const made = JSON.parse(sioul.saveTask("", JSON.stringify(Object.assign({}, blank.edit, { title: "Bring the keys" })), ""))
                    grabber.tiesTask = made.uid || ""
                    console.warn("ties: task: " + JSON.stringify(made))
                },
                () => {},
                () => {},
                () => tasksPage.open(grabber.tiesTask),
                () => {},
                () => {
                    tasksPage.panel.editing = true
                    tasksPage.panel.moreShown = true
                },
                () => {},
                () => {
                    const field = grabber.tiesFind(tasksPage.panel, "taskAttach")
                    console.warn("ties: field " + (field ? field.visible : "none"))
                    field.text = "Morning"
                    field.textEdited()
                    console.warn("ties: rows: " + JSON.stringify(field.matches.map(r => [r.title, r.when, r.detail, r.rid])))
                },
                () => {},
                () => frame.Overlay.overlay.parent.grabToImage(result => result.saveToFile(grabber.folder + "/ties-rows.png")),
                () => {},
                () => {
                    const field = grabber.tiesFind(tasksPage.panel, "taskAttach")
                    const row = field.matches[1]
                    console.warn("ties: attach: " + (sioul.attachToEvent(grabber.tiesTask, row.key, row.rid) || "done"))
                    field.clear()
                },
                () => {},
                () => {},
                () => tasksPage.panel.reload(),
                () => {
                    const read = JSON.parse(sioul.task(grabber.tiesTask) || "{}")
                    console.warn("ties: read: " + JSON.stringify(read.appointment ? read.appointment.turn : null) + " due " + read.edit.due)
                },
                () => grabber.save("ties-task"),
                () => {
                    const read = JSON.parse(sioul.task(grabber.tiesTask))
                    const shown = new Date(read.edit.due + ":00")
                    console.warn("ties: pairing: " + sioul.eventPairing(read.appointment.key))
                    console.warn("ties: that time's card: " + JSON.stringify(JSON.parse(sioul.appointmentView(read.appointment.key, shown.getTime() / 1000)).to_do_before))
                    // Another task, before a time passed: its line, and the one button.
                    const blank = JSON.parse(sioul.newTaskForm("", "", 0, ""))
                    const made = JSON.parse(sioul.saveTask("", JSON.stringify(Object.assign({}, blank.edit, { title: "Bring the chairs" })), ""))
                    const two = n => (n < 10 ? "0" : "") + n
                    const rid = read.appointment.rid
                    const back = new Date(Date.UTC(+rid.slice(0, 4), +rid.slice(4, 6) - 1, +rid.slice(6, 8) - 6, +rid.slice(9, 11), +rid.slice(11, 13)))
                    const passed = back.getUTCFullYear() + two(back.getUTCMonth() + 1) + two(back.getUTCDate()) + "T" + two(back.getUTCHours()) + two(back.getUTCMinutes()) + "00Z"
                    console.warn("ties: a time passed " + passed + ": " + (sioul.attachToEvent(made.uid, read.appointment.key, passed) || "done"))
                    grabber.tiesTask = made.uid
                },
                () => {},
                () => {},
                () => tasksPage.open(grabber.tiesTask),
                () => {},
                () => tasksPage.panel.editing = false,
                () => tasksPage.panel.reload(),
                () => console.warn("ties: read: " + JSON.stringify(JSON.parse(sioul.task(grabber.tiesTask)).appointment.turn)),
                () => grabber.save("ties-passed"),
                () => {
                    const next = grabber.tiesFind(tasksPage.panel, "taskNextOne")
                    console.warn("ties: next one: " + (next ? next.visible + " " + next.text : "none"))
                    next.clicked()
                },
                () => {},
                () => {},
                () => console.warn("ties: read: " + JSON.stringify(JSON.parse(sioul.task(grabber.tiesTask)).appointment.turn) + " due " + JSON.parse(sioul.task(grabber.tiesTask)).edit.due),
                () => grabber.save("ties-next"),
                () => window.close()
            ]
            readonly property var pairSteps: [
                () => window.page = 1,
                () => {},
                () => {
                    const event = grabber.eventNamed("demo-event-doctor")
                    const made = event ? JSON.parse(sioul.stepsForEvent(event.key, "") || "{}") : { error: "not shown" }
                    grabber.pairTask = made.uid || ""
                    console.warn("pair: made: " + JSON.stringify(made))
                },
                () => {},
                () => grabber.pairSaid("made"),
                () => tasksPage.open(grabber.pairTask),
                () => {},
                () => grabber.save("pair-task"),
                () => console.warn("pair: detach: " + (sioul.detachFromEvent(grabber.pairTask) || "done")),
                () => {},
                () => grabber.pairSaid("untied"),
                () => {
                    const event = grabber.eventNamed("demo-event-doctor")
                    console.warn("pair: attach: " + (event ? sioul.attachToEvent(grabber.pairTask, event.key, "") || "done" : "not shown"))
                },
                () => {},
                () => grabber.pairSaid("tied again"),
                () => window.close()
            ]
            readonly property var movetask: [
                () => {
                    sioul.status = sioul.moveTask("demo-task-bike", "cloud/work-tasks", false)
                    console.warn("movetask: " + sioul.status)
                },
                () => sioul.syncNow(),
                () => {},
                () => {},
                () => {},
                () => window.close()
            ]
            // Google: the account form with its steps, and a task of a Google Tasks list.
            // Microsoft's mail form: a personal address as discovery finds it
            // (Outlook.com's domains, known without the network), then a work
            // one as discovery would find it for a Microsoft 365 domain, with
            // what its administrator does unfolded. Nothing is signed in.
            readonly property var microsoftSteps: [
                () => window.page = 11,
                () => accountsPage.addMailFor("someone@hotmail.fr"),
                () => {},
                () => {},
                () => grabber.save("microsoft-personal"),
                () => {
                    const address = "someone@contoso.example"
                    accountsPage.addMailFor(address)
                },
                // Offline, discovery finds nothing for it: its DNS question waits its five seconds.
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => grabber.save("microsoft-nothing-found"),
                () => {
                    const address = "someone@contoso.example"
                    sioul.formError = ""
                    sioul.found = JSON.stringify({ host: "outlook.office365.com", port: 993, security: "tls", login: address, by: "", hint: "", help_url: null, google: false, google_key: false, microsoft: true, microsoft_kind: "work", microsoft_key: true, microsoft_admin: sioul.textWith("microsoft-admin-steps", "link", "https://login.microsoftonline.com/contoso.example/v2.0/adminconsent?client_id=00000000-0000-0000-0000-000000000000") })
                    accountsPage.microsoftAdminUnfolded = true
                },
                () => {},
                () => grabber.save("microsoft-work"),
                // Only the mailbox of the login Microsoft's page signed in opened: the question, both addresses named.
                () => accountsPage.microsoftAsked = sioul.textArgs("microsoft-other-login-question", JSON.stringify({ address: "someone@contoso.example", login: "someone.else@contoso.example" })),
                () => {},
                () => grabber.save("microsoft-other-login"),
                () => window.close()
            ]
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
                    sioul.setNotesView("folder", "all")
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
            // The Letters page: its letters waiting, then filed, its bar, then its ⚙
            // (pictures alone, on a demo profile; docs/porch.md, "Paper letters").
            readonly property var lettersPageSteps: [
                () => window.page = 15,
                () => {},
                () => {},
                () => grabber.save("letters-page"),
                () => window.lettersPage.openSettings(),
                () => {},
                () => {},
                () => window.lettersPage.grabSettings(grabber.folder + "/letters-settings.png"),
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
            // The Porch open: its sections and rows, further down, a section's help, a row
            // dismissed with its ten seconds to undo (docs/porch.md, "Sections"; pictures alone).
            readonly property var porchNotices: [
                () => sioul.openAnyway(),
                () => window.page = 1,
                () => window.page = 0,
                () => {},
                () => grabber.save("porch-notices"),
                () => porchPage.scrollToLane("filed"),
                () => {},
                () => grabber.save("porch-notices-down"),
                () => {
                    const lane = porchPage.view.lanes.find(l => l.key === "people") || porchPage.view.lanes[0]
                    if (lane)
                        porchPage.showHelp(lane)
                },
                () => {},
                () => {
                    if (porchPage.helpItem() !== null)
                        grabber.savePopup(porchPage.helpItem(), "porch-notices-help")
                },
                () => porchPage.closeHelp(),
                () => porchPage.scrollToLane("people"),
                () => porchPage.dismissFirst(),
                () => {},
                () => grabber.save("porch-notices-undo"),
                // A phone held sideways: the same sections and rows.
                () => {
                    if (window.phoneGrab) {
                        window.width = 891
                        window.height = 412
                    }
                },
                () => {},
                () => {},
                () => {
                    if (window.phoneGrab)
                        grabber.saveCounted("porch-notices-wide", porchPage)
                },
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
            // The budgets page from one income to the next (docs/accounting.md, "The budgets
            // page"): its first screen, What is coming (the month beside it on a computer), the
            // bank accounts (pictures alone).
            readonly property var budgetsPageSteps: [
                () => window.page = 9,
                () => {},
                () => {},
                () => grabber.save("budgets-1"),
                () => budgetsPage.showComing(),
                () => {},
                () => grabber.save("budgets-2"),
                () => budgetsPage.showMonth(),
                () => {},
                () => grabber.save("budgets-3"),
                () => budgetsPage.showBank(),
                () => {},
                () => grabber.save("budgets-4"),
                () => window.close()
            ]
            // The shell (the settings rework's A1): tabs across the pages, Settings'
            // sections in their centred column and, on a phone, its list of sections;
            // fields and buttons that stand out. Settings ▸ Display and ▸ Hours, Tasks ▸
            // List, the Agenda's Week, Money's Overview, Accounts and the new movement's
            // form; on a computer at 1400 by 900, then Settings at the window's first 1100, its
            // tabs scrolling (pictures alone). First, in the log, whether Sioul's overrides of the
            // Basic style are in place (cpp/style.cpp): a flat button with words drawn with its
            // outline, a text area with its ground; the release's Flatpak job reads that line.
            // The second wave of the full views: the Week's steps by area, at a computer's width and a
            // phone's (SIOUL_GRAB_PHONE), the filter unfolded, then admin and leisure alone; the Time
            // page's month with its comparison by area (pictures alone; docs/areas.md, docs/capacity.md).
            readonly property var viewsSteps: [
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1400
                        window.height = 900
                    }
                    sioul.stepAreasEverything()
                    window.page = 4
                    agendaPage.mode = "week"
                },
                () => {},
                () => {},
                () => grabber.save("agenda-week-steps-filter"),
                () => sioul.tickStepAreas("admin+leisure"),
                () => {},
                () => {},
                () => grabber.save("agenda-week-admin-leisure"),
                () => agendaPage.openStepAreas(),
                () => {},
                // A popup lies above the page: its own picture, two areas ticked, "Everything" offered.
                () => grabber.savePopup(agendaPage.openStepAreas(), "agenda-week-steps-filter-open"),
                () => {
                    agendaPage.closeStepAreas()
                    sioul.stepAreasEverything()
                },
                () => {
                    window.page = 8
                    timePage.period = "month"
                },
                () => {},
                // Every area ticked, so that the table has a line for each area with time in it.
                () => timePage.showAll(),
                () => {},
                () => {},
                () => grabber.save("time-by-area"),
                () => window.close()
            ]
            // Address books and contacts in bulk (wave D; SIOUL_GRAB_STEPS=address-books, SIOUL_GRAB_PHONE
            // for a phone's): the filter row (on a phone, folded, then unfolded), a selection and its bar, the title's preview on the
            // co-workers' cards, Contacts ⚙ with a book a co-worker can write and its switch, and the
            // import's three steps on the demo's small file of invented people.
            // A contact's card with two address books, its book in its main part and Delete
            // beside Edit, then a new card's form asking its book (SIOUL_GRAB_STEPS=contact-card;
            // SIOUL_GRAB_PHONE for a phone's): a computer's at 1400 points.
            readonly property var contactCardSteps: [
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1400
                        window.height = 900
                    }
                    window.page = 5
                },
                () => {},
                () => contactsPage.open((contactsPage.shown.contacts.find(c => c.name.indexOf("Iris") === 0) || contactsPage.shown.contacts[0]).key),
                () => {},
                () => {},
                () => grabber.save("contact-card-books"),
                () => contactsPage.startNew(""),
                () => {},
                () => {},
                () => grabber.save("contact-new-form"),
                () => window.close()
            ]
            readonly property var addressBookSteps: [
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1400
                        window.height = 900
                    }
                    window.page = 5
                },
                () => {},
                () => {},
                () => grabber.save("contacts-filters"),
                // On a phone, the filters unfolded by their button, then folded again.
                () => {
                    if (window.phoneGrab)
                        grabber.named(contactsPage, "contactFilters").unfolded = true
                },
                () => {},
                () => {
                    if (window.phoneGrab)
                        grabber.save("contacts-filters-open")
                },
                () => {
                    grabber.named(contactsPage, "contactFilters").unfolded = false
                },
                () => {
                    contactsPage.selecting = true
                    contactsPage.selectShown()
                },
                () => {},
                () => grabber.save("contacts-selection"),
                // The title's move on the co-workers' cards: each before and after, with its box.
                () => {
                    const next = {}
                    for (const c of contactsPage.shown.contacts.filter(c => (c.place || "").endsWith("/team")))
                        next[c.key] = true
                    contactsPage.selected = next
                },
                () => grabber.named(contactsPage, "contactSelection").act("title"),
                () => {},
                () => {},
                () => grabber.saveWindow("title-preview"),
                () => {
                    // The dialog is a pop-up, out of the overlay's children: closed through the bar that made it.
                    grabber.named(contactsPage, "contactSelection").act("title").close()
                    contactsPage.selecting = false
                    contactsPage.selected = {}
                },
                // Contacts ⚙ at its address books, the co-workers' book opened: its switch, off, and why.
                () => contactsPage.openSettings(true, "addressbooks"),
                () => {},
                () => {
                    const setup = contactsPage.settingsPart("addressBooksSetup")
                    const team = setup ? (setup.view.books || []).find(b => b.place.endsWith("/team")) : null
                    if (team)
                        setup.toggle(team.place)
                },
                () => {},
                () => {},
                () => grabber.saveWindow("address-books"),
                () => contactsPage.openSettings(false, ""),
                // The import's three steps: what the file holds, where the cards go, the count.
                () => sioul.importContacts("/home/demo/.cache/demo-import/clients.csv"),
                () => {},
                () => {},
                () => grabber.saveWindow("import-1-file"),
                () => contactImport.item.reveal("importBook"),
                () => {},
                () => grabber.saveWindow("import-2-where"),
                () => contactImport.item.count(),
                () => {},
                () => {},
                () => contactImport.item.reveal("importSummary"),
                () => {},
                () => grabber.saveWindow("import-3-count"),
                () => window.close()
            ]
            // Calls per phone, starred people and the phone's permissions (SIOUL_GRAB_STEPS=phones;
            // the demo with SIOUL_DEMO_STARS=1): on a computer, a phone stood in beside this device,
            // its section Calls first while its line says nothing (today's table, Varies), then set
            // Personal with a group's row, then the Always through list saying
            // where each is starred; on a phone, this device stood in as the phone, its card's
            // sections unfolded one by one: Calls, what Android must allow, the restricted settings'
            // three steps (`phonecard::demo`). First, the tab as it opens, each card closed, this
            // device beside a stand-in of the other kind, then this device's card with its Details
            // opened (design of 20 November 2026: a short card, its details folded).
            property string phonesId: ""
            function phonesFold(name, open) {
                // A phone's sections are in its card's Details: opened first.
                if (grabber.phonesId !== "")
                    parametersPage.devices.detailsOf(grabber.phonesId)
                const fold = grabber.named(parametersPage, name)
                if (fold)
                    fold.open = open
                return fold
            }
            // The phone's card: on a computer, the demo's stand-in phone ("phone-id", `share::devices`);
            // on a phone's layout, this device. `say`: a Personal table with the group School said first.
            function phonesShow(say) {
                const devices = parametersPage.devices
                const own = (devices.view.rows || []).filter(row => row.here === true)
                grabber.phonesId = window.phoneGrab && own.length > 0 ? own[0].id : "phone-id"
                if (say) {
                    sioul.deviceSay(grabber.phonesId, "calls.profile", "personal")
                    sioul.deviceSay(grabber.phonesId, "calls.group.add", grabber.demoFrench ? "École" : "School")
                }
                devices.reloadAll()
                const when = grabber.demoFrench ? "sam. 10 oct. 21:40" : "Sat 10 Oct 21:40"
                const mine = (devices.view.rows || []).filter(row => row.here === true)
                if (window.phoneGrab) {
                    // This device, a phone.
                    const rows = (devices.view.rows || []).map(row => row.here === true ? Object.assign({}, row, { form: "phone", form_words: sioul.text("set-device-kind-phone") }) : row)
                    devices.view = Object.assign({}, devices.view, { rows: rows })
                } else {
                    const phone = { id: "phone-id", here: false, name: "Moss", given: "Moss", system: sioul.text("device-the-phone") + " (Moss)", form: "phone", form_words: sioul.text("set-device-kind-phone"), code: "closed", state: sioul.textWith("devices-now-closed", "when", when), notes: [], opened: when, shared: when, version: "", older: false, build: "", area: "admin+leisure", area_words: "", parts: [], parts_words: sioul.text("devices-parts-unsaid"), off: false, forget: false, forget_now: false }
                    devices.view = Object.assign({}, devices.view, { on: true, rows: mine.concat([phone]) })
                }
            }
            // The tab with this device and one of the other kind: on a computer, the stand-in phone
            // ("phone-id"); on a phone's layout, this device as the phone and a computer stood in.
            function devicesShow() {
                const devices = parametersPage.devices
                devices.reloadAll()
                const when = grabber.demoFrench ? "sam. 10 oct. 21:40" : "Sat 10 Oct 21:40"
                const mine = (devices.view.rows || []).filter(row => row.here === true)
                if (!window.phoneGrab) {
                    grabber.phonesShow(false)
                    return
                }
                const phone = mine.map(row => Object.assign({}, row, { form: "phone", form_words: sioul.text("set-device-kind-phone") }))
                const desk = { id: "desk-id", here: false, name: grabber.demoFrench ? "Bureau" : "Study desk", given: grabber.demoFrench ? "Bureau" : "Study desk", system: "noa-desk", form: "computer", form_words: sioul.text("set-device-kind-computer"), code: "closed", state: sioul.textWith("devices-now-closed", "when", when), notes: [], opened: when, shared: when, version: "", older: false, build: "", area: "work", area_words: "", parts: [], parts_words: sioul.text("devices-parts-unsaid"), off: false, forget: false, forget_now: false }
                devices.view = Object.assign({}, devices.view, { on: true, rows: phone.concat([desk]) })
            }
            // This device's card, its Details opened or closed again.
            function devicesHere(open) {
                const devices = parametersPage.devices
                const here = (devices.rows || []).find(row => row.here === true)
                const card = here ? devices.detailsOf(here.id) : null
                const fold = card ? grabber.named(card, "deviceDetails") : null
                if (fold)
                    fold.open = open
            }
            readonly property var phonesSteps: [
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1400
                        window.height = 900
                    }
                    window.showParameters("devices")
                },
                () => {},
                // The tab as it opens: every card closed, short.
                () => grabber.devicesShow(),
                () => {},
                () => grabber.save("devices-closed"),
                // One card's Details: this device's (on a phone, with its sections, each folded).
                () => grabber.devicesHere(true),
                () => {},
                () => grabber.save("devices-details"),
                () => grabber.devicesHere(false),
                // The tab made anew for the steps after, its reload's clock with it: every 20 seconds
                // it reads the devices again, which puts the stand-ins away.
                () => parametersPage.open("look"),
                () => window.showParameters("devices"),
                () => {},
                // On a computer, the phone's card while its line says nothing: what it does today, Varies.
                () => {
                    if (!window.phoneGrab)
                        grabber.phonesShow(false)
                },
                () => {},
                // The phone's Calls as a door opens them: its Details, the section, scrolled to once laid out.
                () => {
                    if (!window.phoneGrab)
                        parametersPage.devices.reveal("calls")
                },
                () => {},
                () => {
                    if (!window.phoneGrab)
                        grabber.save("devices-calls-today")
                },
                () => grabber.phonesShow(true),
                () => {},
                () => parametersPage.devices.reveal("calls"),
                () => {},
                () => grabber.save("devices-calls"),
                () => parametersPage.scrollBy(0.25),
                () => grabber.save("devices-calls-down"),
                // On a phone: what Android must allow, then the restricted settings' three steps.
                () => {
                    if (!window.phoneGrab)
                        return
                    grabber.phonesFold("foldCalls", false)
                    // The restricted settings' card open under it: room for the list's first line at the top.
                    grabber.phonesFold("foldRestricted", true)
                    grabber.phonesFold("foldAllow", true)
                },
                () => {},
                // Laid out: the fold brought to the top.
                () => {
                    if (window.phoneGrab)
                        parametersPage.bringIntoView(grabber.named(parametersPage, "foldAllow"))
                },
                () => {},
                () => {
                    if (window.phoneGrab)
                        grabber.save("devices-allow")
                },
                () => {
                    if (!window.phoneGrab)
                        return
                    grabber.phonesFold("foldAllow", false)
                    grabber.phonesFold("foldRestricted", true)
                },
                () => {
                    if (window.phoneGrab)
                        parametersPage.bringIntoView(grabber.named(parametersPage, "foldRestricted"))
                },
                () => {},
                () => {
                    if (!window.phoneGrab)
                        return
                    grabber.phonesFold("foldAllow", false)
                    grabber.phonesFold("foldRestricted", true)
                },
                () => {
                    if (window.phoneGrab)
                        parametersPage.bringIntoView(grabber.named(parametersPage, "foldRestricted"))
                },
                () => {
                    if (window.phoneGrab)
                        grabber.save("restricted-0")
                },
                () => {
                    if (window.phoneGrab)
                        parametersPage.devices.own = JSON.parse(sioul.restrictedStep("notice") || "{}")
                },
                () => {},
                () => {
                    if (!window.phoneGrab)
                        return
                    grabber.phonesFold("foldAllow", false)
                    grabber.phonesFold("foldRestricted", true)
                },
                () => {
                    if (window.phoneGrab)
                        parametersPage.bringIntoView(grabber.named(parametersPage, "foldRestricted"))
                },
                () => {
                    if (window.phoneGrab)
                        grabber.save("restricted-1")
                },
                () => {
                    if (window.phoneGrab)
                        parametersPage.devices.own = JSON.parse(sioul.restrictedStep("info") || "{}")
                },
                () => {},
                () => {
                    if (!window.phoneGrab)
                        return
                    grabber.phonesFold("foldAllow", false)
                    grabber.phonesFold("foldRestricted", true)
                },
                () => {
                    if (window.phoneGrab)
                        parametersPage.bringIntoView(grabber.named(parametersPage, "foldRestricted"))
                },
                () => {
                    if (window.phoneGrab)
                        grabber.save("restricted-2")
                },
                () => {
                    if (window.phoneGrab)
                        parametersPage.devices.own = JSON.parse(sioul.restrictedStep("back") || "{}")
                },
                () => {},
                () => {
                    if (!window.phoneGrab)
                        return
                    grabber.phonesFold("foldAllow", false)
                    grabber.phonesFold("foldRestricted", true)
                },
                () => {
                    if (window.phoneGrab)
                        parametersPage.bringIntoView(grabber.named(parametersPage, "foldRestricted"))
                },
                () => {
                    if (window.phoneGrab)
                        grabber.save("restricted-3")
                },
                // The Always through list, with where each is starred (What reaches you ▸ Exceptions).
                () => window.showParameters("attention.exceptions"),
                () => {},
                () => {},
                () => parametersPage.bringIntoView(grabber.named(parametersPage, "starsSwitch")),
                () => {},
                () => grabber.save("always-through"),
                () => window.close()
            ]
            readonly property var shellSteps: [
                () => {
                    const button = Qt.createQmlObject("import QtQuick.Controls.Basic; Button { opacity: 0; flat: true; text: \"Probe\" }", window.contentItem)
                    const area = Qt.createQmlObject("import QtQuick.Controls.Basic; TextArea { opacity: 0 }", window.contentItem)
                    // Out of sight by their opacity: an item hidden would say its background hidden too.
                    const ours = button.background.visible && area.background !== null && area.background.visible
                    console.warn("style: " + (ours ? "Sioul's overrides in place" : "Basic's own controls, the overrides not loaded"))
                    button.destroy()
                    area.destroy()
                },
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1400
                        window.height = 900
                    }
                    window.page = 12
                },
                () => parametersPage.back(),
                () => {},
                () => grabber.save("settings-list"),
                () => parametersPage.open("look"),
                () => {},
                () => grabber.save("settings-display"),
                () => parametersPage.open("hours"),
                () => {},
                () => grabber.save("settings-hours"),
                () => {
                    window.page = 1
                    tasksPage.mode = "list"
                },
                () => {},
                () => {},
                () => grabber.save("tasks-list"),
                () => {
                    window.page = 4
                    agendaPage.mode = "week"
                },
                () => {},
                () => {},
                () => grabber.save("agenda-week"),
                // The full views (A3): the Day draws every planned step, whatever the hour.
                () => agendaPage.mode = "day",
                () => {},
                () => {},
                () => grabber.save("agenda-day"),
                // Now, with its cover when the run is at night (RUN_HOUR), the tabs in reach.
                () => {
                    window.page = 1
                    tasksPage.mode = "now"
                },
                () => {},
                () => grabber.save("tasks-now"),
                // Projects and Time by their filter of areas, as the hours say: what folds, said.
                () => window.page = 7,
                () => {},
                () => {},
                () => grabber.save("projects"),
                () => {
                    window.page = 8
                    timePage.move(-1)
                },
                () => {},
                () => {},
                () => grabber.save("time"),
                () => {
                    window.page = 9
                    budgetsPage.showTab(0)
                },
                () => {},
                () => {},
                () => grabber.save("money-overview"),
                () => window.page = 11,
                () => {},
                () => grabber.save("accounts"),
                () => {
                    window.page = 9
                    budgetsPage.showTab(2)
                },
                () => {},
                () => budgetsPage.movements.startNew(),
                () => budgetsPage.movements.moreNew(),
                () => {},
                () => grabber.saveWindow("movement-form"),
                () => budgetsPage.movements.closeForms(),
                () => {
                    if (!window.phoneGrab)
                        window.width = 1100
                    window.page = 12
                    parametersPage.open("look")
                },
                () => {},
                () => grabber.save("settings-1100"),
                // The settings moved (A2) on the shell's components: Settings ▸ Hours at the usual
                // meals and night; the password manager's card in each of its states; Sites ▸ ⚙ at its colours.
                () => {
                    if (!window.phoneGrab)
                        window.width = 1400
                    window.showParameters("needs")
                },
                () => {},
                () => {},
                () => grabber.save("settings-needs"),
                () => window.showParameters("bitwarden"),
                () => {},
                () => grabber.save("vault-card-none"),
                // An account set (the demo's own fictional address): locked, as with no server reached.
                () => {
                    console.warn("vault: account set: " + (sioul.setSetting("bitwarden.email", JSON.stringify("noa@example.com")) || "yes"))
                    accountsPage.vaultCard.reload()
                },
                () => {},
                () => grabber.save("vault-card-locked"),
                // Open: stood in for (no Bitwarden server in the demo), told to the card alone:
                // its row follows the card's reading (SettingRow.qml's `vaultState`).
                () => accountsPage.vaultCard.vault = "unlocked",
                () => {},
                () => grabber.save("vault-card-unlocked"),
                () => {
                    sioul.setSetting("bitwarden.email", JSON.stringify(""))
                    accountsPage.vaultCard.reload()
                },
                () => window.showParameters("calmer_colours"),
                () => {},
                () => {},
                () => grabber.saveWindow("sites-settings"),
                () => sitesPage.openSettings(false, ""),
                () => window.close()
            ]
            // The Budgets page's four tabs (the money ledger's design §6): Overview,
            // Budgets, Movements with a payout opened and its menu, New movement
            // with More, the declaration, From mail and a row's menu (pictures alone).
            readonly property var moneyTabSteps: [
                () => window.page = 9,
                () => {},
                () => {},
                () => grabber.saveCounted("money-overview", budgetsPage),
                () => budgetsPage.showTab(1),
                () => {},
                () => {},
                () => grabber.saveCounted("money-budgets", budgetsPage.budgetsShown),
                () => grabber.saveCounted("money-page-with-budgets", budgetsPage),
                () => budgetsPage.showBank(),
                () => {},
                () => grabber.save("money-bank"),
                () => budgetsPage.showTab(2),
                () => {},
                () => {},
                () => grabber.saveCounted("money-movements", budgetsPage.movements),
                () => budgetsPage.movements.menuOf(budgetsPage.movements.firstTold()),
                () => {},
                () => grabber.saveWindow("money-menu"),
                () => budgetsPage.movements.closeForms(),
                () => budgetsPage.movements.openMovement(budgetsPage.movements.firstTold()),
                () => {},
                () => grabber.save("money-movement-open"),
                () => budgetsPage.movements.detail = null,
                () => budgetsPage.movements.startNew(),
                () => budgetsPage.movements.moreNew(),
                () => {},
                () => grabber.saveWindow("money-new"),
                () => budgetsPage.movements.closeForms(),
                () => budgetsPage.movements.showDeclaration("", ""),
                () => {},
                () => grabber.save("money-declaration"),
                () => budgetsPage.movements.declaration = null,
                () => budgetsPage.showTab(3),
                () => {},
                () => {},
                () => {},
                () => grabber.saveCounted("money-mail", budgetsPage.moneyMail),
                () => budgetsPage.showTab(0),
                () => window.close()
            ]
            // Money ▸ Invoices on the demo's invented business (docs/projects.md, the guide's
            // pictures): a project's page with its client's profile, its Make the invoice in
            // the editor, its time unfolded and its preview; issued, its pane; Paid…; the
            // list; Settings ▸ Invoices. A phone's layout with "phone" (run.sh).
            // Calendars by their address, vacations, a series that pauses (SIOUL_GRAB_STEPS=calendars;
            // SIOUL_GRAB_PHONE for a phone's): Agenda ⚙ ▸ Your agendas with the subscribed calendars
            // and their lines, one opened; the subscription form; Settings ▸ Hours ▸ Vacations and
            // holidays; the choir's card with its next pause; its form at its repeat's end and its
            // pauses. Nothing is saved, nothing fetched (SIOUL_DEMO).
            readonly property var calendarSteps: [
                // The minute's rounds at once (the series' pauses kept), not half a minute or more after the start.
                () => sioul.refreshMode(),
                () => window.page = 4,
                () => {},
                () => {},
                () => agendaPage.openSettings(true, "agendas"),
                () => {},
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/calendars-agendas.png"),
                () => agendaPage.openAgendaLine("subscribed/fr-school-zone-a"),
                () => {},
                () => {},
                () => agendaPage.revealSetting("subscriptionLine"),
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/calendars-agenda-line.png"),
                () => agendaPage.askSubscribe(),
                () => {},
                () => {},
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/calendars-subscribe.png"),
                () => agendaPage.openSettings(false),
                () => window.showParameters("time_off"),
                () => {},
                () => {},
                () => grabber.save("calendars-vacations"),
                () => window.page = 4,
                () => agendaPage.mode = "week",
                () => {},
                () => {},
                () => agendaPage.openTitled(sioul.text("qt-locale").indexOf("fr") === 0 ? "Répétition" : "Choir"),
                () => {},
                () => {},
                () => grabber.saveCounted("calendars-card", cardHost),
                () => agendaPage.eventForm().edit(cardHost.eventCard.event.key, ""),
                () => {},
                () => {},
                () => agendaPage.eventForm().showRepeat(),
                () => {},
                () => {},
                () => grabber.savePopup(agendaPage.eventForm(), "calendars-event-form"),
                () => agendaPage.eventForm().close(),
                () => window.close()
            ]
            readonly property var invoicesSteps: [
                () => {
                    window.page = 7
                    projectsPage.showAll()
                },
                () => projectsPage.openFirst(),
                () => {},
                () => grabber.save("project-client"),
                () => projectsPage.makeInvoice(),
                () => {},
                () => {},
                () => grabber.save("invoice-editor"),
                () => budgetsPage.invoices.editor.toPreview(),
                // Once the page is laid out, at its very end.
                () => budgetsPage.invoices.editor.toPreview(),
                () => grabber.save("invoice-preview"),
                () => budgetsPage.invoices.editor.issueNow(),
                () => {},
                () => {},
                () => grabber.save("invoice-issued"),
                () => budgetsPage.invoices.askPaid(budgetsPage.invoices.invoice),
                () => {},
                () => grabber.saveWindow("invoice-paid"),
                () => budgetsPage.invoices.closeForms(),
                () => budgetsPage.invoices.back(),
                () => {},
                () => grabber.save("invoices"),
                () => window.showParameters("invoice"),
                () => {},
                () => {},
                () => grabber.save("settings-invoices"),
                () => window.close()
            ]
            readonly property var bankSteps: [
                () => window.page = 9,
                () => sioul.status = JSON.parse(sioul.importBank("file://" + grabber.folder + "/../releve-banque.csv")).said,
                () => budgetsPage.contractsNow().reload(),
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
                () => sitesPage.bitwardenUnlock.ask(sioul.bitwardenPasskeyBegin(), said => {
                    const begun = JSON.parse(said)
                    if (begun.error)
                        sitesPage.bitwardenUnlock.problem = begun.error
                    else
                        sitesPage.bitwardenUnlock.answer({ factor: [1, 7], key: { page: begun.page, script: begun.script.replace('})("passkey", ', '})("factor", ') } })
                }),
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
            // Bitwarden asked off the window's thread (sites.rs, `bitwarden_later`): the dialog
            // waiting, Open greyed; then an unlock asked and its answer come with
            // `bitwardenAnswered` (a demo profile sets no account: the answer says so, and
            // no server is asked).
            readonly property var bitwardenWait: [
                () => window.page = 3,
                () => sitesPage.bitwardenUnlock.begin(),
                () => sitesPage.bitwardenUnlock.ask(-1, null),
                () => {},
                () => sitesPage.bitwardenUnlock.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/bitwarden-waiting.png")),
                () => {
                    sitesPage.bitwardenUnlock.waiting = 0
                    sitesPage.bitwardenUnlock.tryIt()
                    console.warn("sioul-bitwarden: asked, the dialog waits: " + sitesPage.bitwardenUnlock.busy)
                },
                () => {},
                () => console.warn("sioul-bitwarden: answered, the dialog waits: " + sitesPage.bitwardenUnlock.busy + "; it says: " + sitesPage.bitwardenUnlock.problem),
                () => sitesPage.bitwardenUnlock.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/bitwarden-answered.png")),
                () => sitesPage.bitwardenUnlock.close(),
                // Where Bitwarden is set: the password manager's card at the top of Accounts
                // (none set on a demo profile), then its form in Add an account.
                () => window.showParameters("bitwarden"),
                () => {},
                () => grabber.save("vault-card-none"),
                () => accountsPage.addVault(),
                () => {},
                () => {},
                () => grabber.save("vault-form"),
                () => window.close()
            ]
            // The window hidden as to the tray, then brought back (`bringBack`): no
            // "Conflicting properties" in the log.
            readonly property var traySteps: [
                () => window.hide(),
                () => console.warn("sioul-tray: hidden: " + !window.visible),
                () => window.bringBack(),
                () => console.warn("sioul-tray: brought back: " + window.visible + ", visibility " + window.visibility),
                () => grabber.save("tray-back"),
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
            // Reachable; an address's own settings on its card; the hours in Settings.
            readonly property var ownership: [
                () => window.page = 0,
                () => {},
                () => {},
                () => grabber.save("own-porch"),
                () => porchPage.openSettings(true),
                () => {},
                () => porchPage.grabSettings(grabber.folder + "/own-porch-settings.png"),
                () => porchPage.openSettings(false),
                () => sioul.reachChoose("reachable", "60"),
                () => {},
                () => grabber.save("own-reachable"),
                () => sioul.reachChoose("usual", ""),
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
            // one, the keys.
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
                () => grabber.save("account-tabs-keys"),
                () => window.close()
            ]
            // What reaches you (SIOUL_GRAB_STEPS=attention; SIOUL_GRAB_PHONE for a
            // phone's; a computer's at 1400 wide): Your rules top to bottom, Sioul's own,
            // Moments, a circle's choices, Who is in which circle; What comes when, a
            // fixed cell's why, a row's times on a phone; a contact's card and how they
            // reach you; then Busy on an event's card and on a task's panel.
            readonly property var attentionSteps: [
                // Settings ▸ What reaches you: Your rules, top to bottom; a circle's choices.
                () => {
                    if (!window.phoneGrab)
                        window.width = 1400
                    window.showParameters("attention")
                },
                () => {},
                () => {},
                () => grabber.save("attention-rules"),
                () => parametersPage.scrollBy(0.45),
                () => {},
                () => grabber.save("attention-rules-2"),
                () => parametersPage.scrollBy(0.45),
                () => {},
                () => grabber.save("attention-rules-3"),
                () => window.showParameters("attention.own"),
                () => {},
                () => {},
                () => grabber.save("attention-own"),
                () => window.showParameters("attention.dnd"),
                () => {},
                () => {},
                () => grabber.save("attention-moments"),
                () => window.showParameters("attention.person"),
                () => {},
                () => {},
                () => window.reachesTab().openChoice("contacts", "mail"),
                () => {},
                () => grabber.saveWindow("attention-choices"),
                () => window.reachesTab().closeChoices(),
                // Who is in which circle: your safe senders put on Always through first.
                () => sioul.dndChange("add-safe", "{}"),
                () => window.showParameters("attention.exceptions"),
                () => {},
                () => {},
                () => grabber.save("attention-circles"),
                // What comes when: the grid in words; a fixed cell's why; on a phone, a row's values.
                () => window.showParameters("attention.table"),
                () => {},
                () => {},
                () => grabber.save("attention-table"),
                () => window.reachesTab().openCell("doses", "dnd"),
                () => {},
                () => grabber.saveWindow("attention-fixed"),
                () => window.reachesTab().closeChoices(),
                () => window.reachesTab().rowOpen = "mail.neutral",
                () => {},
                () => grabber.save("attention-row"),
                // A contact's card, and their sheet.
                () => window.page = 5,
                () => {},
                () => contactsPage.open((contactsPage.shown.contacts.find(c => c.name.indexOf("Varga") >= 0) || contactsPage.shown.contacts[0]).key),
                () => {},
                () => grabber.save("attention-card"),
                () => window.openPersonSheet(contactsPage.openKey, ""),
                () => {},
                () => {},
                () => grabber.savePopup(personSheet.item, "attention-sheet"),
                () => personSheet.item.close(),
                // Busy: an event's card, a task's panel.
                () => window.page = 4,
                () => agendaPage.mode = "week",
                () => {},
                () => {},
                () => agendaPage.openTitled("Dr Varga"),
                () => {},
                () => {},
                () => grabber.saveCounted("busy-event-card", cardHost),
                () => cardHost.close(),
                () => window.page = 1,
                () => {},
                () => tasksPage.openFirst(),
                () => {},
                () => {},
                () => grabber.save("busy-task"),
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
            // Your people in What reaches you (SIOUL_GRAB_STEPS=household; SIOUL_GRAB_PHONE for a
            // phone's; the demo's Léa, no card linked, and Maud, linked): People with their line,
            // Who is in which circle, Maud's sheet with its reason, Maud's page, and a phone's calls
            // with their row.
            readonly property var householdSteps: [
                () => {
                    if (!window.phoneGrab)
                        window.width = 1400
                    window.showParameters("attention.people")
                },
                () => {},
                () => {},
                () => grabber.save("household-people"),
                () => window.showParameters("attention.circles"),
                () => {},
                () => {},
                () => grabber.save("household-circles"),
                () => window.page = 5,
                () => {},
                () => contactsPage.open((contactsPage.shown.contacts.find(c => c.name.indexOf("Maud") >= 0) || contactsPage.shown.contacts[0]).key),
                () => {},
                () => window.openPersonSheet(contactsPage.openKey, ""),
                () => {},
                () => {},
                () => grabber.savePopup(personSheet.item, "household-sheet"),
                () => personSheet.item.close(),
                () => window.openPerson("demo-person-maud"),
                () => {},
                () => {},
                () => grabber.save("household-person"),
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1400
                        window.height = 900
                    }
                    window.showParameters("devices")
                },
                () => {},
                () => grabber.phonesShow(true),
                () => {},
                () => {
                    grabber.phonesFold("foldCalls", true)
                    parametersPage.bringIntoView(grabber.named(parametersPage, window.phoneGrab ? "foldCalls" : "phoneCard-phone-id"))
                },
                () => {},
                () => grabber.save("household-calls"),
                () => parametersPage.scrollBy(0.25),
                () => {},
                () => grabber.save("household-calls-down"),
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
                () => grabber.savePopup(window.statusLine.modeMenu.item, "pauses-free-menu"),
                () => window.statusLine.modeMenu.item.dismiss(),
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
                // Settings ▸ Pauses, by its section's key.
                () => window.showParameters("pauses"),
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
                // As the tray's menu opens it (Tray.qml): the sheet, the window brought back.
                () => window.reviewFromTray("night"),
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
            // The household (docs/persons.md): Contacts with its row of persons, a
            // child's page, the Papers page's chips, a paper's "For" (pictures alone).
            readonly property var personsSteps: [
                () => window.page = 5,
                () => {},
                () => grabber.save("household"),
                () => contactsPage.openPerson((JSON.parse(sioul.household()).persons.find(p => p.relation === "child") || { id: "me" }).id),
                () => {},
                () => grabber.save("person-page"),
                // The parent you help: a procuration among their papers, Ends… at the foot.
                () => contactsPage.openPerson((JSON.parse(sioul.household()).persons.find(p => p.relation === "parent") || { id: "me" }).id),
                () => {},
                () => grabber.save("person-parent"),
                () => {
                    contactsPage.openPerson("")
                    window.page = 13
                },
                () => {},
                () => grabber.save("papers-for"),
                () => papersPage.forId = (JSON.parse(sioul.household()).persons.find(p => p.relation === "child") || { id: "" }).id,
                () => {},
                () => grabber.save("papers-child"),
                () => papersPage.forId = "",
                () => {
                    const child = JSON.parse(sioul.household()).persons.find(p => p.relation === "child")
                    const paper = child ? [].concat(...papersPage.shown.families.map(f => f.papers)).find(p => (p.persons || []).indexOf(child.id) >= 0) : null
                    if (paper)
                        papersPage.open(paper.id)
                },
                () => {},
                () => papersPage.dialog.item && papersPage.dialog.item.opened ? papersPage.dialog.item.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/paper-for-dialog.png")) : console.warn("no paper for a child"),
                () => {},
                () => papersPage.dialog.close(),
                // Agenda ⚙ ▸ Your agendas, the child's line: whose it is and the other parent as persons.
                () => window.page = 4,
                () => agendaPage.openSettings(true, "agendas"),
                () => {},
                () => agendaPage.openAgendaLine("cloud/lea"),
                () => {},
                () => {},
                () => agendaPage.grabSettings(grabber.folder + "/agenda-lea-line.png"),
                () => agendaPage.openSettings(false),
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
            // Where Sioul keeps its lists, on a demo profile of the old layout
            // (SIOUL_DEMO_OLD_LAYOUT=1 SIOUL_GRAB_STEPS=storage, a phone's with
            // SIOUL_GRAB_PHONE): the window of the move before the press, top and
            // end; the move made on the demo profile; then Settings ▸ Sharing's line
            // of the old files with "Take them out", and what the press said.
            readonly property var storageSteps: [
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1400
                        window.height = 900
                    }
                    window.showStorageMove()
                },
                () => {},
                () => {},
                () => grabber.saveWindow("storage-move"),
                () => {
                    const shown = storageMove.item as StorageMove
                    if (shown !== null)
                        shown.toEnd()
                },
                () => {},
                () => grabber.saveWindow("storage-move-end"),
                () => {
                    const shown = storageMove.item as StorageMove
                    if (shown !== null)
                        shown.moveNow()
                },
                () => {},
                () => {},
                () => {},
                () => {},
                () => grabber.saveWindow("storage-moved"),
                () => {
                    const shown = storageMove.item as StorageMove
                    if (shown !== null)
                        shown.close()
                },
                () => {
                    window.page = 12
                    parametersPage.section = "files"
                },
                () => {},
                () => parametersPage.revealNamed("shareStorage"),
                () => {},
                () => grabber.save("storage-old-files"),
                () => sioul.takeOutOldFiles(),
                () => {},
                () => {},
                () => {},
                () => parametersPage.revealNamed("shareStorage"),
                () => {},
                () => grabber.save("storage-taken"),
                () => window.close()
            ]
            // A file a site offers (SIOUL_GRAB_STEPS=downloads, on a demo profile; a
            // phone's with SIOUL_GRAB_PHONE): a page of the bank's own address, made
            // here, offers a statement (no site reached), its card over the site;
            // kept in Downloads, the status line saying so; then a program, its card
            // with its caution, cancelled; then the bank's next file, whose card can
            // refuse the site as a whole; then the status line of a file that did not
            // come whole (docs/sites.md, "Downloads").
            readonly property var downloadSteps: [
                () => {
                    if (!window.phoneGrab) {
                        window.width = 1400
                        window.height = 900
                    }
                    window.page = 3
                },
                () => sitesPage.open("bank"),
                () => {},
                () => {},
                () => grabber.offerFile(sioul.text("download-place-downloads") === "Téléchargements" ? "Relevé octobre 2026.pdf" : "Statement October 2026.pdf", "application/pdf", 182000),
                () => {},
                () => {},
                () => grabber.saveWindow("download-card"),
                () => {
                    const card = sitesPage.downloads.card
                    if (card)
                        card.save()
                },
                () => {},
                () => {},
                () => {},
                () => grabber.saveWindow("download-saved"),
                () => grabber.offerFile("setup.exe", "application/x-msdownload", 5300000),
                () => {},
                () => {},
                () => grabber.saveWindow("download-card-program"),
                () => {
                    const card = sitesPage.downloads.card
                    if (card)
                        card.cancel()
                },
                () => {},
                () => grabber.saveWindow("download-cancelled"),
                () => grabber.offerFile(sioul.text("download-place-downloads") === "Téléchargements" ? "Relevé novembre 2026.pdf" : "Statement November 2026.pdf", "application/pdf", 175000),
                () => {},
                () => {},
                () => grabber.saveWindow("download-card-refuse"),
                () => {
                    const card = sitesPage.downloads.card
                    if (card)
                        card.cancel()
                },
                () => sioul.downloadFailed(JSON.stringify({ hold: "", held: "", name: sioul.text("download-place-downloads") === "Téléchargements" ? "Relevé décembre 2026.pdf" : "Statement December 2026.pdf", reason: 22 })),
                () => {},
                () => grabber.saveWindow("download-failed"),
                () => window.close()
            ]
            // A file offered by a page made in the open site's view, at the site's own
            // address: `size` bytes of nothing named `name`, clicked as the page loads.
            function offerFile(name, type, size) {
                const view = sitesPage.viewOf(sitesPage.openId)
                if (!view) {
                    console.warn("sioul-grab: downloads: no site's view")
                    return
                }
                const script = "const a = document.getElementById('a'); a.href = URL.createObjectURL(new Blob([new Uint8Array(" + size + ")], { type: " + JSON.stringify(type) + " })); a.click();"
                view.loadHtml("<!doctype html><meta charset=utf-8><body><a id=a download=" + JSON.stringify(name) + ">file</a><script>" + script + "</script></body>", view.url)
            }
            // Settings ▸ Your folder and sharing before sharing, pictures only
            // (SIOUL_GRAB_STEPS=share-panel, a phone's with SIOUL_GRAB_PHONE): how
            // a device reaches the folder, then the form for a Nextcloud Sioul
            // keeps in step itself (showing it asks nothing of the server).
            readonly property var sharePanel: [
                () => {
                    window.page = 12
                    parametersPage.section = "files"
                },
                () => {},
                () => {},
                () => grabber.save("share-panel"),
                () => {
                    if (parametersPage.share)
                        parametersPage.share.byServer = true
                },
                () => {},
                () => grabber.save("share-panel-server"),
                () => parametersPage.scrollBy(0.25),
                () => {},
                () => grabber.save("share-panel-server-down"),
                () => window.close()
            ]
            // Joining your other devices through your Nextcloud's own sign-in
            // (SIOUL_GRAB_STEPS=join, docs/database.md), on a demo profile, which
            // reaches no server: the Porch's card on a device that does not share
            // yet; Settings ▸ Your folder and sharing, its sign-in offered; then the
            // panel's steps stood in for (`JoinPanel.stand`, as join.rs says them):
            // signing in in the browser, the folder found, joined. Then a sharing
            // device's sights, the panel shown as sharing: "Add a device" on a
            // computer, and on a phone whether it replaces another phone; then
            // Settings ▸ Devices, a phone silent a day stood in there with
            // "Forget it now". Nothing signed in, sent nor asked.
            readonly property var joinSteps: [
                () => window.page = 0,
                () => {},
                () => grabber.save("join-card"),
                () => {
                    window.page = 12
                    parametersPage.section = "files"
                },
                () => {},
                () => grabber.save("join-panel"),
                () => parametersPage.share.join.stand = { step: "signing", host: "murena.io" },
                () => {},
                () => grabber.save("join-signing"),
                () => parametersPage.share.join.stand = { step: "found", host: "murena.io", signed: sioul.textArgs("join-signed", JSON.stringify({ host: "murena.io", login: "jane@murena.io" })), places: ["Documents/Sioul"] },
                () => {},
                () => grabber.save("join-found"),
                () => parametersPage.share.join.stand = {
                    step: "joined",
                    host: "murena.io",
                    lines: [
                        sioul.textWith("join-with", "names", sioul.textArgs("join-and", JSON.stringify({ first: "desk", last: "laptop" }))),
                        sioul.text("join-settings"),
                        sioul.textWith("join-accounts", "accounts", sioul.textArgs("join-and", JSON.stringify({ first: "jane@example.org", last: sioul.textWith("join-agenda", "host", "murena.io") }))),
                        sioul.textWith("join-hours", "which", sioul.textArgs("join-and", JSON.stringify({ first: sioul.text("join-hours-work"), last: sioul.text("join-hours-admin") }))),
                        sioul.textWith("join-agenda-kept", "host", "murena.io")
                    ],
                    needs: [sioul.textWith("join-needs-password", "account", "jane@example.org")]
                },
                () => {},
                () => grabber.save("join-joined"),
                () => parametersPage.share.join.stand = { step: "" },
                () => {},
                () => {
                    const share = parametersPage.share
                    const when = grabber.demoFrench ? "sam. 10 oct. 21:40" : "Sat 10 Oct 21:40"
                    const adding = [sioul.text("share-add-intro"), sioul.text("share-add-murena"), sioul.textWith("share-add-place", "place", "Documents/Sioul"), sioul.text("share-add-passphrase")]
                    const asking = window.phoneGrab ? { id: "phone-id", ask: sioul.textArgs("join-replace-ask", JSON.stringify({ name: "Moss", when: when })) } : null
                    share.status = Object.assign({}, share.status, { on: true, add_device: window.phoneGrab ? [] : adding, replace: asking })
                    share.adding = true
                },
                () => grabber.save(window.phoneGrab ? "share-replace" : "share-add-device"),
                () => parametersPage.scrollBy(0.3),
                () => grabber.save(window.phoneGrab ? "share-replace-down" : "share-add-device-down"),
                // Settings ▸ Devices, a phone silent a day stood in beside this device's own row: Forget it now.
                () => parametersPage.section = "devices",
                () => {},
                () => {
                    const devices = parametersPage.devices
                    if (!devices || window.phoneGrab)
                        return
                    const when = grabber.demoFrench ? "sam. 10 oct. 21:40" : "Sat 10 Oct 21:40"
                    const own = (devices.view.rows || []).filter(row => row.here === true)
                    const phone = { id: "phone-id", here: false, name: sioul.text("device-the-phone") + " (Moss)", given: "", system: sioul.text("device-the-phone") + " (Moss)", form: "phone", form_words: sioul.text("set-device-kind-phone"), code: "closed", state: sioul.textWith("devices-now-closed", "when", when), notes: [], opened: when, shared: when, version: "", older: false, build: "", area: "", area_words: sioul.text("devices-for-all"), parts: [], parts_words: sioul.text("devices-parts-unsaid"), off: false, forget: false, forget_now: true }
                    devices.view = { on: true, rows: own.concat([phone]) }
                },
                // Forget it now is in the phone's Details, last: opened.
                () => {
                    const devices = parametersPage.devices
                    if (devices && !window.phoneGrab)
                        devices.detailsOf("phone-id")
                },
                () => {},
                () => grabber.save(window.phoneGrab ? "devices-phone" : "devices-forget-now"),
                () => window.close()
            ]
            // This device's files sent to the server beside the sync app
            // (SIOUL_GRAB_STEPS=share-send), on a demo profile, which reaches no
            // server: sharing started in a folder of the profile; the build,
            // the switch and "Send everything again", then what it said.
            readonly property var shareSend: [
                () => {
                    window.page = 12
                    parametersPage.section = "files"
                },
                () => sioul.status = sioul.startSharing("~/Documents/Sioul", "quiet harbour at dawn", "quiet harbour at dawn"),
                () => {},
                () => {},
                () => parametersPage.share.reload(),
                () => parametersPage.scrollBy(0.2),
                () => {},
                () => grabber.save("share-send"),
                // "Stop sharing" asks first, saying what stops and what stays; cancelled, it goes on.
                // A sandbox whose keyring keeps no key shares nothing: the panel is
                // shown as sharing for these pictures, nothing stopped or asked of it.
                () => {
                    if (!parametersPage.share.status.on)
                        parametersPage.share.status = Object.assign({}, parametersPage.share.status, { on: true })
                    parametersPage.share.stopping = true
                },
                () => grabber.save("share-stop-ask"),
                () => parametersPage.share.stopping = false,
                () => grabber.save("share-stop-cancelled"),
                () => parametersPage.share.reload(),
                () => parametersPage.share.sendAgain(),
                () => {},
                () => {},
                () => {},
                () => {},
                () => {},
                () => grabber.save("share-send-again"),
                () => window.close()
            ]
            // A note changed on another device while the Notes page shows it
            // (SIOUL_GRAB_STEPS=notes-shared), on a demo profile: the other
            // device's change written into the note (saved without its stamp,
            // as the sharing writes it), then the sharing's word stood in for
            // (`sharedIn`). Only read: the page shows the change. Being typed
            // in: what was typed stays, a line says a newer version came, and
            // saving puts it beside that version.
            readonly property string sharedNote: grabber.demoFrench ? "Maison/Liste de lecture.md" : "Home/Reading list.md"
            readonly property var notesSharedSteps: [
                () => window.page = 6,
                () => notesPage.open(grabber.sharedNote),
                () => {},
                () => grabber.save("notes-shared-before"),
                () => sioul.saveNote(grabber.sharedNote, notesPage.note.text + "\nFrom the phone: a second book.\n", ""),
                () => sioul.sharedIn("files/notes/"),
                () => {},
                () => grabber.save("notes-shared-read"),
                () => notesPage.editing = true,
                () => notesPage.typeAtEnd("\nTyped here, not saved yet.\n"),
                () => sioul.saveNote(grabber.sharedNote, notesPage.note.text + "From the phone again.\n", ""),
                () => sioul.sharedIn("files/notes/"),
                () => {},
                () => grabber.save("notes-shared-typing"),
                () => notesPage.save(),
                () => {},
                () => grabber.save("notes-shared-saved"),
                () => window.close()
            ]
            // The search by conditions (SIOUL_GRAB_STEPS=mail-search), on a demo
            // profile: automatic mail of both addresses arrived in the last three
            // weeks, its column in the folders' place, the results with where each
            // is; a result read; all of them chosen; held over a folder, the
            // folders back in the search's place (never let go); "Move to…". With
            // SIOUL_GRAB_PHONE, a phone's: the fields alone, then the results, a
            // selection begun from a row's menu, "Move to…". Nothing is moved.
            readonly property string searchSince: new Date(Date.now() - 21 * 86400000).toISOString().slice(0, 10)
            readonly property var mailSearchSteps: window.phoneGrab ? [
                () => window.page = 2,
                () => {},
                () => mailPage.searchWith([{ field: "from", test: "contains", value: "no-reply" }, { field: "date", test: "after", value: grabber.searchSince }], false),
                () => {},
                () => grabber.save("mail-search-form"),
                () => mailPage.searchFormShown = false,
                () => {},
                () => grabber.save("mail-search-results"),
                () => mailPage.chooseFirst(2),
                () => {},
                () => grabber.save("mail-search-choosing"),
                () => mailPage.askMove(mailPage.selectedKeys),
                () => {},
                () => grabber.saveWindow("mail-search-move"),
                () => mailPage.closeMove(),
                () => window.close()
            ] : [
                // The size of the guide's other pictures of the Mail page.
                () => {
                    window.width = 1280
                    window.height = 860
                    window.page = 2
                },
                () => {},
                () => mailPage.searchWith([{ field: "from", test: "contains", value: "no-reply" }, { field: "date", test: "after", value: grabber.searchSince }], false),
                () => {},
                () => grabber.save("mail-search"),
                () => mailPage.openFirst(),
                () => {},
                () => grabber.save("mail-search-reader"),
                () => mailPage.openKey = "",
                () => mailPage.selectAll(),
                () => {},
                () => grabber.save("mail-search-selected"),
                () => mailPage.holdOver(mailPage.selectedKeys, "archive"),
                () => mailPage.holdOver(mailPage.selectedKeys, "archive"),
                () => {},
                () => grabber.save("mail-search-drag"),
                () => mailPage.letGo(),
                () => mailPage.askMove(mailPage.selectedKeys),
                () => {},
                () => grabber.saveWindow("mail-search-move"),
                () => mailPage.closeMove(),
                () => window.close()
            ]
            // The mail filters (SIOUL_GRAB_STEPS=mail-filters; SIOUL_GRAB_PHONE for a
            // phone's), on a demo profile: four filters as Mail ▸ ⚙ lists them, in
            // the wider panel; the first one's editor, tried on the inboxes; what
            // running them on the inboxes would change, said first; a search made a
            // filter, its editor open at the list's end. Nothing is run.
            function demoFilters() {
                const fr = sioul.text("qt-locale").indexOf("fr") === 0
                const condition = (field, test, value) => ({ field: field, test: test, value: value, until: "" })
                const act = (id, name) => ({ do: id, name: name || "" })
                return [
                    { name: fr ? "Banque" : "Bank", enabled: true, accounts: [], any: false, stop: true,
                      conditions: [condition("from", "contains", fr ? "@banquedesberges.example.org" : "@riversidebank.example.org")],
                      actions: [act("move", "Archive"), act("read")] },
                    { name: "", enabled: true, accounts: [], any: false, stop: false,
                      conditions: [condition("list", "exists", ""), condition("weekday", "is", "sat,sun")],
                      actions: [act("read")] },
                    { name: "", enabled: true, accounts: [fr ? "perso" : "personal"], any: false, stop: false,
                      conditions: [condition("from", "contains", fr ? "@bonnes-affaires.example.com" : "@deals-today.example.com")],
                      actions: [act("junk")] },
                    { name: fr ? "Factures" : "Invoices", enabled: false, accounts: [], any: true, stop: false,
                      conditions: [condition("subject", "contains", fr ? "facture" : "invoice"), condition("attachment-type", "is", "pdf")],
                      actions: [act("flag"), act("keyword", fr ? "Factures" : "Invoices")] }
                ]
            }
            // An item by its objectName under `root`: what a pop-up holds (the filters in Mail's ⚙).
            function named(root, name) {
                if (!root)
                    return null
                if (root.objectName === name)
                    return root
                for (let i = 0; i < root.children.length; ++i) {
                    const found = grabber.named(root.children[i], name)
                    if (found)
                        return found
                }
                return null
            }
            // A Porch card's "Leave as is" under `root`, pressed.
            function leaveAsIs(root) {
                const words = sioul.text("porch-hours-leave")
                const walk = item => {
                    if (item.text === words && typeof item.clicked === "function") {
                        item.clicked()
                        return true
                    }
                    for (let i = 0; i < item.children.length; ++i) {
                        if (walk(item.children[i]))
                            return true
                    }
                    return false
                }
                return walk(root)
            }
            function mailFilters() {
                return grabber.named(frame.Overlay.overlay, "mailFilters")
            }
            readonly property var mailFilterSteps: [
                () => window.page = 2,
                () => sioul.status = sioul.setMailFilters(JSON.stringify(grabber.demoFilters()), ""),
                () => mailPage.openSettings(true),
                () => mailPage.scrollSettings("mail.filter"),
                () => {},
                () => grabber.saveWindow("mail-filters"),
                // The first one opened and tried on the inboxes.
                () => grabber.mailFilters().open(0),
                () => grabber.mailFilters().tryIt(),
                () => {},
                () => grabber.saveWindow("mail-filter-editor"),
                () => grabber.mailFilters().close(),
                // Running them on the inboxes, read mail too: said first, nothing done.
                () => grabber.mailFilters().preview(),
                () => {},
                () => grabber.saveWindow("mail-filters-run"),
                () => mailPage.openSettings(false),
                // A search made a filter: its folder condition left out, said; what it does to choose.
                () => window.newFilterFrom(JSON.stringify({ conditions: [{ field: "anywhere", test: "contains", value: sioul.text("qt-locale").indexOf("fr") === 0 ? "facture" : "invoice", until: "" }, { field: "folder", test: "is", value: "archive", until: "" }], match: "all" })),
                () => {},
                () => {},
                () => grabber.saveWindow("mail-filter-from-search"),
                () => window.close()
            ]
            // Sioul's own spam filter (SIOUL_GRAB_STEPS=spam; SIOUL_GRAB_PHONE for a
            // phone's), on a demo profile made with its table and two strangers'
            // messages it has a word for (make-demo.py --spam, or SIOUL_DEMO_SPAM=1):
            // its settings in Mail's ⚙, then its training's block; on the Porch the
            // quiet words beside the two, and one opened with why it is here.
            readonly property var spamSteps: [
                () => window.page = 2,
                () => {},
                () => mailPage.openSettings(true),
                () => mailPage.scrollSettings("spam.actions"),
                () => {},
                () => grabber.saveWindow("spam-settings"),
                // The panel alone, its background included (the guide's picture), in a window
                // tall enough for the whole filter, its training's switch included (French
                // takes more lines): the pop-up's own item, the overlay's child that holds the setting.
                () => {
                    if (!window.phoneGrab)
                        window.height = grabber.demoFrench ? 2200 : 1840
                },
                () => mailPage.scrollSettings("spam.actions"),
                () => {},
                () => {
                    let item = grabber.named(frame.Overlay.overlay, "setting:spam.actions")
                    while (item !== null && item.parent !== null && item.parent !== frame.Overlay.overlay)
                        item = item.parent
                    if (item !== null)
                        item.grabToImage(result => result.saveToFile(grabber.folder + "/spam-settings-panel.png"))
                },
                () => {
                    if (!window.phoneGrab)
                        window.height = 1500
                },
                () => mailPage.scrollSettings("spam.filter"),
                () => {},
                () => grabber.saveWindow("spam-training"),
                () => mailPage.openSettings(false),
                () => window.page = 0,
                () => {},
                // The review queue, opened: what the filter flagged or moved, Spam and Not spam on each.
                () => porchPage.openLane("review"),
                () => {},
                () => porchPage.scrollToLane("review"),
                () => {},
                () => grabber.save("spam-porch"),
                () => porchPage.openSpam(""),
                () => {},
                () => porchPage.showReasons(),
                // The Porch's column, narrower beside the message, scrolled to the queue again.
                () => porchPage.scrollToLane("review"),
                () => {},
                () => grabber.save("spam-reader"),
                () => window.close()
            ]
            // The calls your phones declined, on a computer's Porch (SIOUL_GRAB_STEPS=calls),
            // on a demo profile made with a phone's log (make-demo.py --calls, or
            // SIOUL_DEMO_CALLS=1); then the doctor's sheet, opened by the doctor's number as
            // from a call's line, with the doctor's calls of the month.
            readonly property var callsSteps: [
                () => window.page = 0,
                () => {},
                () => {},
                () => grabber.save("calls-porch"),
                () => window.openPersonSheet("", JSON.stringify(["tel:+33465714770"])),
                () => {},
                () => {},
                () => grabber.savePopup(personSheet.item, "calls-sheet"),
                () => personSheet.item.close(),
                () => window.close()
            ]
            // texts: the Texts page (docs/texts.md) at the window's widths (1100,
            // the narrowest, 1600): the list, a conversation, the sending states
            // and a picture, a long thread and further back, an agent's draft, a
            // search and one that finds nothing, the page before texts are read here.
            readonly property var textsSteps: [
                () => window.openTexts(""),
                () => {},
                () => {},
                () => grabber.saveChecked("texts-list"),
                () => window.textsPage.select("+33199001234"),
                () => {},
                () => {},
                () => grabber.saveChecked("texts-conversation"),
                () => window.textsPage.select("+33465710042"),
                () => {},
                () => {},
                () => grabber.saveChecked("texts-states"),
                () => window.textsPage.select("+262639989046"),
                () => {},
                () => {},
                () => grabber.saveChecked("texts-long"),
                () => window.textsPage.scrollBack(),
                () => {},
                () => grabber.saveChecked("texts-long-back"),
                () => window.textsPage.select("+33465714770"),
                () => {},
                () => {},
                () => grabber.saveChecked("texts-draft"),
                () => window.textsPage.search(sioul.text("texts-page-title") === "SMS" ? "colis" : "parcel"),
                () => {},
                () => grabber.saveChecked("texts-search"),
                () => window.textsPage.search("xyzzy"),
                () => {},
                () => grabber.saveChecked("texts-search-none"),
                () => window.textsPage.search(""),
                () => window.width = 680,
                () => window.textsPage.close(),
                () => {},
                () => grabber.saveChecked("texts-narrow-list"),
                () => window.textsPage.select("+33465710042"),
                () => {},
                () => {},
                () => grabber.saveChecked("texts-narrow-thread"),
                () => window.width = 1600,
                () => {},
                () => {},
                () => grabber.saveChecked("texts-wide"),
                () => window.textsPage.select("+262639989046"),
                () => {},
                () => {},
                () => grabber.saveChecked("texts-wide-long"),
                () => {
                    window.width = 1100
                    window.textsPage.showNotSetUp()
                },
                () => {},
                () => grabber.saveChecked("texts-off"),
                () => window.close()
            ]
            // The projects' file under its first name (make-demo.py --old-projects,
            // docs/notes-folder.md): Settings ▸ Your folder and sharing offers to
            // rename it; renamed there, the first project's page and its form read as before.
            readonly property var projectsFileSteps: [
                () => window.showParameters("case_store"),
                () => {},
                () => grabber.save("projects-file"),
                () => window.parametersPage.save("projects.rename", true),
                () => {},
                () => grabber.save("projects-file-renamed"),
                () => {
                    window.page = 7
                    projectsPage.showAll()
                },
                () => projectsPage.openFirst(),
                () => {},
                () => grabber.save("projects-file-page"),
                () => projectsPage.editOpen(),
                () => {},
                () => grabber.savePopup(projectsPage.editOpen(), "projects-file-form"),
                () => window.close()
            ]
            // What AI agents may use (docs/mcp.md): Settings ▸ AI agents, the first
            // project closed then opened on its page, the tab again; the Texts page
            // with an agent's draft (make-demo.py --texts), used: its words in the box, nothing sent.
            readonly property var aiSteps: [
                () => window.showParameters("ai"),
                () => {},
                () => grabber.save("ai-settings"),
                () => {
                    window.page = 7
                    projectsPage.showAll()
                },
                () => projectsPage.openFirst(),
                () => {},
                () => grabber.save("ai-project-closed"),
                () => projectsPage.saveRoutes("project." + projectsPage.openId + ".ai", true),
                () => {},
                () => grabber.save("ai-project-open"),
                () => window.showParameters("ai"),
                () => {},
                () => grabber.save("ai-settings-open"),
                () => window.openTexts(""),
                () => {},
                () => {},
                () => grabber.save("ai-texts-list"),
                () => window.textsPage.select("+33465714770"),
                () => {},
                () => {},
                () => grabber.save("ai-texts-draft"),
                () => window.textsPage.useDraft(0),
                () => {},
                () => {},
                () => grabber.save("ai-texts-used"),
                () => window.textsPage.close(),
                () => window.close()
            ]
            // Settings ▸ Words (docs/words.md): the lines, a line's page, a word
            // added and one taken away, a brand of your own, back to the defaults,
            // German read too, and the way there from the Filed lane's ⚙.
            // Settings ▸ Rules (docs/rules.md): the list with your rules and those kept elsewhere
            // (the demo's routes, a route that makes tasks, its budgets' mail rules, its filters, its
            // topics' senders), what a rule did lately, the editor at each step and its preview, a
            // rule from a message, "Add a rule…", the presets and a preset's card.
            function rulesTab() {
                return parametersPage ? parametersPage.rules : null
            }
            function rulesEditor() {
                const tab = grabber.rulesTab()
                return tab ? tab.editor : null
            }
            // On the demo profile (its project "taxes-2026"), not on a copy of a real one, whose own
            // filters and rules the pictures show as they are.
            function rulesOnDemo() {
                return JSON.parse(sioul.rulesChoices()).project.some(p => p.id === "taxes-2026")
            }
            // Two rules of the demo's own, the first with the id its record of acts names (make-demo.py).
            function demoRules() {
                if (!grabber.rulesOnDemo())
                    return
                const fr = sioul.text("qt-locale").indexOf("fr") === 0
                const tax = fr ? "impots.example.org" : "tax-office.example.org"
                const when = conditions => ({ type: "message", event: "arrives", if: conditions })
                sioul.saveRule(JSON.stringify({ id: "demo-taxes", on: true, when: when([{ field: "sender-domain", test: "is", value: [tax] }, { field: "subject", test: "has-words", value: [fr ? "avis" : "notice"] }]), then: { type: "message", action: "tie-to-project", with: { project: "taxes-2026" } } }), false)
                sioul.saveRule(JSON.stringify({ id: "demo-taxes-tell", on: false, when: when([{ field: "sender-domain", test: "is", value: [tax] }, { field: "subject", test: "has-words", value: [fr ? "avis" : "notice"] }]), then: { type: "none", action: "tell", with: { level: "now" } } }), false)
            }
            readonly property var rulesSteps: [
                () => window.page = 2,
                () => {
                    if (grabber.rulesOnDemo())
                        sioul.status = sioul.setMailFilters(JSON.stringify(grabber.demoFilters()), "")
                },
                () => grabber.demoRules(),
                () => window.showParameters("rules"),
                () => {},
                () => {},
                () => grabber.save("rules-list"),
                // What the first rule did lately, unfolded.
                () => {
                    const line = grabber.rulesTab().line("demo-taxes")
                    const fold = line ? grabber.named(line, "ruleLately") : null
                    if (fold)
                        fold.want(true)
                },
                () => {},
                () => grabber.save("rules-lately"),
                () => parametersPage.scrollBy(0.5),
                () => {},
                () => grabber.save("rules-list-more"),
                () => parametersPage.scrollBy(1),
                () => {},
                () => grabber.save("rules-list-end"),
                // The editor, new, then at each step.
                () => grabber.rulesTab().edit(null),
                () => {},
                () => grabber.saveWindow("rules-editor-new"),
                () => {
                    const fr = sioul.text("qt-locale").indexOf("fr") === 0
                    const form = grabber.rulesEditor()
                    form.addCondition()
                    form.setValue(0, [fr ? "ondea.example.org" : "wavecell.example.org"])
                    form.addCondition()
                    form.setField(1, "kind")
                    form.setValue(1, ["bill"])
                },
                () => {},
                () => grabber.saveWindow("rules-editor-when"),
                () => grabber.rulesEditor().setAction("task", "make"),
                () => {},
                () => grabber.saveWindow("rules-editor-then"),
                // The first model there is: the demo's "Check the statement", or one of the profile's own.
                () => {
                    const form = grabber.rulesEditor()
                    const models = JSON.parse(sioul.rulesChoices()).model
                    if (models.length > 0)
                        form.setParam("model", models[0].id)
                    form.setParam("title", { from: "subject" })
                    form.setParam("days", 3)
                },
                () => {},
                () => {},
                () => {},
                () => grabber.saveWindow("rules-editor-preview"),
                () => grabber.rulesEditor().close(),
                // From a message: its first one of the last 30 days.
                () => {
                    const offered = JSON.parse(sioul.rulesStarts())
                    window.ruleFromMail(offered.messages.length > 0 ? offered.messages[0].key : "")
                },
                () => {},
                () => {},
                () => grabber.saveWindow("rules-from-message"),
                () => ruleEditor.close(),
                // "Add a rule…": three starts.
                () => grabber.rulesTab().add(),
                () => {},
                () => grabber.saveWindow("rules-start"),
                () => {
                    const start = grabber.named(frame.Overlay.overlay, "ruleStart")
                    if (start)
                        start.close()
                },
                // The presets, and one's card.
                () => {
                    parametersPage.scrollBy(-2)
                    grabber.rulesTab().part = "presets"
                },
                () => {},
                () => grabber.save("rules-presets"),
                () => parametersPage.scrollBy(0.6),
                () => {},
                () => grabber.save("rules-presets-more"),
                () => grabber.rulesTab().openPreset("new-mail-told"),
                () => {},
                () => grabber.saveWindow("rules-preset-card"),
                () => window.close()
            ]
            readonly property var wordsSteps: [
                () => window.showParameters("words"),
                () => {},
                () => {},
                () => grabber.save("words-lines"),
                () => parametersPage.scrollBy(0.5),
                () => {},
                () => grabber.save("words-lines-more"),
                () => window.showParameters("words.folders.trash"),
                () => {},
                () => {},
                () => grabber.save("words-folders"),
                () => grabber.changeWords("words.folders.trash", ["Papierkorb"], ["bin"]),
                () => {},
                () => {},
                () => grabber.save("words-folders-changed"),
                () => window.showParameters("words.brands.brands"),
                () => {},
                () => {},
                () => grabber.save("words-brands"),
                () => grabber.changeWords("words.brands.brands", ["Ma Banque: mabanque.example"], []),
                () => {},
                () => {},
                () => grabber.save("words-brands-changed"),
                () => parametersPage.save("words.reset.brands", true),
                () => {},
                () => {},
                () => grabber.save("words-brands-reset"),
                () => {
                    parametersPage.save("words.languages", ["fr", "en", "de"])
                    window.wordsTab().show("")
                },
                () => {},
                () => {},
                () => grabber.save("words-german"),
                () => sioul.reminderOpened("settings:words.senders.automatic", "", ""),
                () => {},
                () => {},
                () => grabber.save("words-senders"),
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
            // A message's attachments, unfolded in the Reader: the words and buttons of a
            // computer, and of a phone with "phone" (no antivirus to call, "Save…"); nothing opened.
            readonly property var attachmentSteps: [
                () => window.page = 2,
                () => {},
                () => mailPage.searchWith([{ field: "attachment", test: "exists", value: "" }], false),
                () => {},
                () => mailPage.searchFormShown = false,
                () => mailPage.openFirst(),
                () => {},
                () => mailPage.showAttachments(),
                () => mailPage.showAttachments(),
                () => grabber.save("attachments"),
                () => window.close()
            ]
            // A project's billable time written as a spreadsheet, then the Time page.
            readonly property var exportCsv: [
                () => sioul.status = sioul.exportTimeCsv("studio", "2026-10-01", "2026-10-31", "file://" + grabber.folder + "/studio%20octobre.csv"),
                () => {
                    window.page = 8
                    timePage.showAll()
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
                // The items alive under the page, said in the log (part 5: its day a list alone).
                () => grabber.saveCounted("health", healthPage),
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-week"),
                () => healthPage.showDay(),
                () => healthPage.settingsPanel().open(),
                () => {},
                () => grabber.savePopup(healthPage.settingsPanel(), "health-settings"),
                () => healthPage.settingsPanel().showEnd(),
                () => {},
                () => grabber.savePopup(healthPage.settingsPanel(), "health-settings-end"),
                () => healthPage.closeSettings(),
                // The usual meals, naps and night, in Settings ▸ Hours at their heading.
                () => window.showParameters("needs"),
                () => {},
                () => {},
                () => grabber.save("settings-needs"),
                () => window.page = 10,
                () => {},
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
                () => window.statusLine.weatherApplet.openForecast(),
                () => {},
                () => window.statusLine.weatherApplet.grab(grabber.folder + "/weather.png"),
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
            // Notes found by their text, a note's outline, the dated lists and one type (pictures
            // alone; docs/tasks.md, "Notes"): on a computer the outline beside the note, on a
            // phone its fold opened, the list shown again for the lists.
            readonly property var notesFindSteps: [
                () => {
                    window.page = 6
                    sioul.setNotesView("folder", "all")
                },
                () => notesPage.grabSearch(grabber.demoFrench ? "ronceverte" : "brambleworth"),
                () => {},
                () => {},
                () => grabber.save("notes-search"),
                () => notesPage.grabSearch(""),
                () => notesPage.open(grabber.demoFrench ? "Idées/Plan du jardin.md" : "Ideas/Garden plan.md"),
                () => notesPage.grabOutline(),
                () => {},
                () => grabber.save("notes-outline"),
                () => {
                    if (window.compact)
                        notesPage.back()
                    sioul.setNotesView("changed", "all")
                },
                () => {},
                () => grabber.save("notes-changed"),
                () => sioul.setNotesView("created", "all"),
                () => {},
                () => grabber.save("notes-created"),
                () => sioul.setNotesView("changed", "sound"),
                () => {},
                () => grabber.save("notes-sound"),
                () => sioul.setNotesView("folder", "all"),
                () => window.close()
            ]
            // Notes by last change and as folders; a note with a picture, then a picture, a PDF, a memo.
            readonly property var notes: [
                () => {
                    sioul.setNotesView("changed", "all")
                    window.page = 6
                },
                () => notesPage.open("admin/lease.md"),
                () => {},
                () => grabber.save("notes-list"),
                () => sioul.setNotesView("folder", "all"),
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
                () => sioul.setNotesView("folder", "all"),
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
            // Your security key (SIOUL_GRAB_STEPS=security-key), on a demo profile,
            // whose key is software (its PIN 123456): its setup in Accounts ▸
            // Encryption, then a message signed at Send: the PIN asked, a wrong
            // one, the touch, then the ten seconds of "Undo".
            readonly property var securityKeySteps: [
                () => window.page = 11,
                () => accountsPage.showTab(2),
                () => grabber.save("seckey-accounts"),
                () => sioul.readSecurityKey(),
                () => grabber.save("seckey-found"),
                () => sioul.findSecurityKeyCertificate(""),
                () => {},
                () => grabber.save("seckey-kept"),
                () => window.compose("new", ""),
                () => window.drafts[0].fill("Bob <bob@example.org>", "Signed at Send", "Hello Bob,\nthis one is signed with my security key."),
                () => window.drafts[0].setSigned(true),
                () => window.drafts[0].send(),
                () => {},
                () => grabber.saveDraft("seckey-pin"),
                () => sioul.signAndSend(window.drafts[0].draftId, "000000"),
                () => {},
                () => grabber.saveDraft("seckey-wrong-pin"),
                () => sioul.signAndSend(window.drafts[0].draftId, "123456"),
                () => grabber.saveDraft("seckey-touch"),
                () => {},
                () => {},
                () => grabber.save("seckey-sending"),
                () => window.close()
            ]
            // A prescription's medicines as rows, each with its takes, a row added; a
            // medicine's own form with its takes; the page saying each take's amount.
            // Then the security key's GnuPG steps as buttons, their commands by hand,
            // and what goes public before keys.openpgp.org (docs/health.md, docs/client.md).
            readonly property var healthGpgSteps: [
                () => window.page = 10,
                () => {},
                () => healthPage.editPrescription(healthPage.prescriptions[0]),
                () => {},
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-prescription-form"),
                () => healthPage.openPopup().addMedicine(),
                () => {},
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-prescription-added"),
                () => healthPage.closePopups(),
                () => healthPage.editMedicine(healthPage.medicines.find(m => m.takes && m.takes.some(t => t.amount !== "")) || healthPage.medicines[0]),
                () => {},
                () => {},
                () => grabber.savePopup(healthPage.openPopup(), "health-medicine-form"),
                () => healthPage.closePopups(),
                () => healthPage.showMedicines(),
                () => {},
                () => grabber.save("health-takes"),
                // What a doctor or a pharmacist is shown: all taken now, then the prescription's.
                () => healthPage.showProfessional(""),
                () => {},
                () => {},
                () => grabber.savePopup(healthPage.professionalView(), "health-professional"),
                () => healthPage.closeProfessional(),
                () => healthPage.showProfessional(healthPage.prescriptions[0].id),
                () => {},
                () => {},
                () => grabber.savePopup(healthPage.professionalView(), "health-professional-prescription"),
                () => healthPage.closeProfessional(),
                () => window.page = 11,
                () => accountsPage.showTab(2),
                () => sioul.readSecurityKey(),
                () => {},
                () => grabber.save("seckey-gnupg-found"),
                () => sioul.importSecurityKeyFromGnupg(""),
                () => {},
                () => grabber.save("seckey-gnupg-kept"),
                () => accountsPage.askToSend(accountsPage.securityKeys.cards[0].ident),
                () => grabber.save("seckey-send-ask"),
                () => sioul.sendSecurityKeyToKeysOpenpgp(accountsPage.securityKeys.cards[0].ident),
                () => {},
                () => grabber.save("seckey-sent"),
                () => window.close()
            ]
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
                // A section's "?": its help and what changes it, in its popup.
                () => {
                    const lane = porchPage.view.lanes.find(l => l.key === "people") || porchPage.view.lanes[0]
                    if (lane)
                        porchPage.showHelp(lane)
                },
                () => {
                    if (porchPage.helpItem() !== null)
                        grabber.savePopup(porchPage.helpItem(), "porch-help")
                },
                () => porchPage.closeHelp(),
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
                () => {
                    window.page = 7
                    projectsPage.showAll()
                },
                () => projectsPage.openFirst(),
                () => grabber.save("projects"),
                () => {
                    window.page = 8
                    timePage.showAll()
                },
                () => grabber.save("time"),
                // Last week's, which has some: the first stretch changed, once the week's view has come
                // (it is made off the window's thread: `timeView`).
                () => timePage.move(-1),
                () => {},
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
                // The week, then the settings (the errands, the pause to move, the limit on chats).
                () => healthPage.showWeek(),
                () => {},
                () => grabber.save("health-week"),
                () => healthPage.showDay(),
                () => healthPage.settingsPanel().open(),
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
                    window.width = window.phoneGrab ? 412 : window.demoGrab || window.tabletGrab ? 1280 : 1100
                    window.height = window.phoneGrab ? 891 : window.demoGrab ? 860 : window.tabletGrab ? 800 : 1500
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

            // A list of Settings ▸ Words changed as a person would: words added, others taken away.
            function changeWords(key, added, removed) {
                const row = parametersPage.rows.find(r => r.key === key)
                if (row)
                    parametersPage.save(key, row.value.filter(w => removed.indexOf(w) < 0).concat(added))
            }

            function save(name) {
                if (!grabber.pictures)
                    return
                frame.grabToImage(result => result.saveToFile(grabber.folder + "/" + name + ".png"))
                if (window.phoneGrab)
                    grabber.overflow(name)
            }

            // A picture, and how many items live under `root` (visible or not), said
            // in the log: what a view costs as it stands (the agenda's budget).
            function saveCounted(name, root) {
                grabber.save(name)
                const count = item => {
                    let n = 1
                    for (let i = 0; i < item.children.length; ++i)
                        n += count(item.children[i])
                    return n
                }
                console.warn("sioul-objects: " + name + " " + count(root))
            }

            // texts: a picture and, at any width, what passes the window's edge, said in the log.
            function saveChecked(name) {
                grabber.save(name)
                if (!window.phoneGrab)
                    grabber.overflow(name)
            }

            // A phone's pictures: what passes the window's right edge, said in the log,
            // the deepest items only (their containers pass it with them). `root`: a
            // pop-up's own item, which the window's content leaves out, or the content
            // of the writing window (as wide as this one at a phone's size).
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
                    // A row scrolled sideways (the Money tabs on a phone) is wider on purpose.
                    if (depth > 60 || !item.visible || sideways(item))
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
                if (!grabber.pictures)
                    return
                frame.Overlay.overlay.parent.grabToImage(result => result.saveToFile(grabber.folder + "/" + name + ".png"))
            }

            // An open menu or pop-up, which the frame leaves out.
            function savePopup(popup, name) {
                if (!grabber.pictures)
                    return
                popup.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/" + name + ".png"))
                if (window.phoneGrab)
                    grabber.overflow(name, popup.contentItem.parent)
            }

            // The writing window's picture; at a phone's size, what passes its right edge too.
            function saveDraft(name) {
                if (window.drafts.length === 0 || !grabber.pictures)
                    return
                window.drafts[0].grab(grabber.folder + "/" + name + ".png")
                if (window.phoneGrab)
                    grabber.overflow(name, window.drafts[0].contentItem)
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
        id: storageMove
    }
    // Every device knows the new places, nothing moved here yet: the window of the move, once.
    Connections {
        target: sioul

        function onStorageMoveChanged() {
            const view = sioul.storageMove ? JSON.parse(sioul.storageMove) : ({})
            if (view.offer === true && !sioul.viewFlag("storage-move-said"))
                window.showStorageMove()
        }
    }
    // Recents' picture of Sioul kept out while a health card is open (Android
    // 13 and later; RecentsGuard.qml): the Health page, a dose's question, a
    // prescription's date in the card host, an errand's task card there.
    RecentsGuard {
        sioul: sioul
        health: window.page === 10 || (doseTaken.item !== null && doseTaken.item.opened)
                || (cardHost.open && cardHost.ref.card === "dated" && ["prescription-ends", "runs-out"].indexOf(cardHost.ref.datedKind) >= 0)
                || (cardHost.open && cardHost.ref.card === "task" && sioul.isErrand(cardHost.ref.owner || ""))
                // The chain of ties showing an errand, its appointment or a prescription's date.
                || (window.page === 16 && window.chainPage !== null && window.chainPage.health)
    }
    Loader {
        id: interruption
    }
    Loader {
        id: personSheet
    }
    // A person's list or Always through changed on their sheet: their card says it again.
    Connections {
        target: personSheet.item
        ignoreUnknownSignals: true

        // texts: a person's conversation on the Texts page.
        function onTextsAsked(id) {
            window.openTexts(id)
        }

        function onChanged() {
            if (contactsPage.openKey !== "")
                contactsPage.readMail()
            // What reaches you's lists and words, when they are what is shown.
            const tab = window.reachesTab()
            if (tab) {
                tab.readLists()
                tab.reload()
            }
        }
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
        id: suppliesUsedForm
    }
    Loader {
        id: paymentForm
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

    Connections {
        target: sioul
        function onCopyRequested(text) {
            window.copy(text)
        }
        function onOpenUrl(url) {
            Qt.openUrlExternally(url)
        }
        function onScanUnavailable(key, index, what, name, question, action) {
            unchecked.key = key
            unchecked.index = index
            unchecked.what = what
            unchecked.active = true
            // Not checked: the question as Rust says it (attachments::decide),
            // the hint's command to copy; the file's name, from a message, never one.
            unchecked.item.ask(name, question, action, true)
        }
        function onComposeRequested(id) {
            window.openDraft(id)
        }
    }

    // Android's Back: the places put away, else what the page opened closed,
    // else back to the Porch; on the Porch, Android's own (Sioul goes to the background).
    Shortcut {
        sequence: "Back"
        enabled: window.placesOpen || window.canGoBack || cardHost.open || window.page !== 0
        onActivated: {
            if (window.placesOpen)
                window.placesOpen = false
            else if (window.canGoBack || cardHost.open)
                window.goBack()
            else
                window.page = 0
        }
    }
    // Escape: the card open first (a computer's; a phone's Back above).
    Shortcut {
        sequence: "Escape"
        enabled: cardHost.open
        onActivated: cardHost.back()
    }
    // Then the chain of ties, back to where it was opened.
    Shortcut {
        sequence: "Escape"
        enabled: !cardHost.open && window.page === 16
        onActivated: window.closeChain()
    }
    // The chain of ties of the thing open (Ctrl+L forwards a message in the Reader).
    Shortcut {
        sequence: "Ctrl+G"
        enabled: window.thingShown !== null && (window.thingShown.uri || "") !== ""
        onActivated: window.openChain(window.thingShown.uri)
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
    // The places' names shown or their icons only; on a phone, the drawer pulled over the pages or put away.
    Shortcut {
        sequence: "F9"
        onActivated: window.togglePlaces(true)
    }
    // A desktop's window: full screen, or back; and quitting for good, closing
    // the window hiding it in the tray while one holds Sioul.
    Shortcut {
        sequence: "F11"
        enabled: Qt.platform.os !== "android"
        onActivated: window.toggleFullScreen()
    }
    Shortcut {
        sequence: "Ctrl+Q"
        enabled: Qt.platform.os !== "android"
        onActivated: window.quitSioul()
    }

    Rectangle {
        id: frame

        anchors.fill: parent
        color: theme.background

        // The places and the pages; on a computer, under the title bar.
        Item {
            anchors.fill: parent
            anchors.topMargin: titleBar.height

            // The places: a narrow column of icons beside the pages, their names
            // beside them or not by the button at the title bar's left end or F9
            // (Places.qml; the same setting as Settings ▸ Display); on a phone, or
            // in a window as narrow, pulled over the pages from the left, with their names.
            Rectangle {
                id: places

                // Names beside the icons: in a phone's drawer, else when asked for.
                readonly property bool named: window.compact || window.placesNamed
                // What it holds, once made.
                readonly property Places shown: placesContent.item as Places

                z: 2
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
                            onClicked: window.goBack()
                        }
                        Label {
                            Layout.fillWidth: true
                            text: sioul.text(window.pageNames[window.page] || "ui-porch")
                            textFormat: Text.PlainText
                            font.pixelSize: 18
                            color: theme.text
                            elide: Text.ElideRight
                        }
                        // The page's ⚙, where the page gives its rows' whole width to its tabs (Money: `barSettings`).
                        ToolButton {
                            objectName: "barSettings"
                            visible: !window.canGoBack && window.shownPage !== null && window.shownPage.barSettings === true
                            Layout.preferredWidth: 48
                            Layout.fillHeight: true
                            icon.name: "settings-configure"
                            icon.color: theme.text
                            display: AbstractButton.IconOnly
                            Accessible.name: sioul.text("ui-settings")
                            onClicked: window.shownPage.openSettings(true, "")
                        }
                    }
                }

                // The pages, and every object's card where you are (CardHost.qml): beside
                // the page while the page keeps its room (`cardRoom`: the Week's grid,
                // a list's 480 pixels), else over its right part; a phone's whole page.
                Item {
                    id: pageArea

                    readonly property real cardWidth: window.compact ? pageArea.width : Math.round(Math.min(cardHost.naturalWidth, pageArea.width / 2))
                    readonly property real cardRoom: window.shownPage && window.shownPage.cardRoom !== undefined ? window.shownPage.cardRoom : 480
                    readonly property bool beside: !window.compact && cardHost.open && pageArea.width - pageArea.cardWidth >= pageArea.cardRoom

                    Layout.fillWidth: true
                    Layout.fillHeight: true

                StackLayout {
                    id: pages

                    x: 0
                    y: 0
                    width: pageArea.beside ? pageArea.width - pageArea.cardWidth : pageArea.width
                    height: pageArea.height
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
                    Loader {
                        id: textsPageLoader
                    }
                    Loader {
                        id: lettersPageLoader
                    }
                    Loader {
                        id: chainPageLoader
                    }
                    Loader {
                        id: suppliesPageLoader
                    }
                }

                CardHost {
                    id: cardHost

                    objectName: "cardHost"
                    visible: cardHost.open
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.right: parent.right
                    width: pageArea.cardWidth
                    z: 2
                    sioul: sioul
                    theme: theme
                    window: window
                }
                }

                // On a phone, the status line at the bottom (StatusLine.qml); a
                // computer's is its title bar, at the top.
                Rectangle {
                    visible: !window.ownTitleBar
                    Layout.fillWidth: true
                    Layout.preferredHeight: 36
                    color: theme.surface

                    Rectangle {
                        width: parent.width
                        height: 1
                        color: theme.line
                    }

                    Loader {
                        id: bottomLine

                        anchors.fill: parent
                        active: !window.ownTitleBar
                        sourceComponent: StatusLine {
                            window: window
                            sioul: sioul
                            theme: theme
                        }
                    }
                }
            }
        }

        // On a computer, the window's own title bar (TitleBar.qml): the status
        // line across the top, the window's buttons; after the pages, so that
        // Tab reaches it after them, as it reached the status line at the bottom.
        Loader {
            id: titleBar

            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            height: window.ownTitleBar ? 36 : 0
            active: window.ownTitleBar
            sourceComponent: TitleBar {
                window: window
                sioul: sioul
                theme: theme
                covered: pauseCover.active
                placesWidth: places.width
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
            // On a computer, under the title bar: the window's buttons, and the bar to move it.
            anchors.topMargin: titleBar.height
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

        // On a computer, the window's edges resize it (WindowEdges.qml), and a
        // line in the theme's colour shows them: the system draws no border
        // around a window with its own title bar. Neither while maximized or
        // full screen, nor in the documentation's pictures.
        Loader {
            anchors.fill: parent
            z: 20
            active: window.ownTitleBar && sioul.grabFolder() === "" && window.visibility === Window.Windowed
            sourceComponent: WindowEdges {
                window: window
            }
        }
        Rectangle {
            anchors.fill: parent
            z: 20
            visible: window.ownTitleBar && sioul.grabFolder() === "" && window.visibility === Window.Windowed
            color: "transparent"
            border.width: 1
            border.color: theme.line
        }
    }

    // An extension's sign-in page (SignInWindow.qml, docs/extensions.md), on
    // computers only: made when the backend asks for one (`signInPage`), gone
    // when it is answered or given up. A phone has its own (SignIn.java).
    Connections {
        target: sioul

        function onSignInPageChanged() {
            if (Qt.platform.os === "android")
                return
            if (sioul.signInPage === "")
                signInLoader.source = ""
            else
                signInLoader.setSource("SignInWindow.qml", { sioul: sioul, theme: theme, page: JSON.parse(sioul.signInPage) })
        }
    }
    Loader {
        id: signInLoader
    }

    // The holder of the sites' web profile (SitesWeb.qml), declared last of the
    // window's items on purpose: Qt makes them in this order and destroys them in
    // the same order, so it goes after every page and every view that uses its
    // profile (the Sites page's sites and pop-ups, the vault's key page in the
    // dialogs of Sites and Accounts), as Qt asks of a WebEngineProfile. Keep it
    // last (tests/web_profile.rs).
    Loader {
        id: sitesWebLoader
    }
}
