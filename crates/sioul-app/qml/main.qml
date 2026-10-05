// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's window: on the left, "New" for anything, the places where your
// things live, then, apart at the bottom, the accounts, the settings and one
// button that refreshes everything; one quiet status line at the bottom,
// where "Undo" waits ten seconds after anything is moved, deleted or sent. No
// badges, no counts in the title, no red; the system's dark mode is
// followed. Everything works from the keyboard: Tab, Enter, Escape, Ctrl+1 to
// Ctrl+9 and Ctrl+0 (the places by the list), Ctrl+N (new), Ctrl+Z, F5
// (refresh). Whatever links lead to (a task, a message, a note, an event, a
// contact) opens where it lives. On a phone, or in a window as narrow, the
// places are pulled over the pages from the left (☰), the pages take the
// whole width, and Android's Back puts the places away, then goes back to
// the Porch.

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
            loaders[i].setSource(window.pageFiles[i], given)
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
    readonly property var moment: sioul.mode ? JSON.parse(sioul.mode) : ({ quiet: false, reason: "", until: "", line: "", hours: false })

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

    // Work comes and goes with the hours: looked at each minute.
    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: sioul.refreshMode()
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
    // address}): made, then opened where it lives.
    function addLinked(kind, source) {
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
    }

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
        newMenu.item.popup(newButton, 0, newButton.height)
    }

    // You are at this window: what follows you (medicines' reminders) comes here.
    onActiveChanged: if (window.active) sioul.touch()

    Timer {
        interval: 30000
        repeat: true
        running: window.active
        onTriggered: sioul.touch()
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
        else if (item.kind === "porch")
            window.page = 0
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
            // A page is shown at one tick and saved at the next, since an image is
            // taken at the next frame.
            readonly property var steps: ({ "actions": grabber.actions, "pim": grabber.pim, "pgp": grabber.pgp, "tasks": grabber.tasks, "move": grabber.move, "links": grabber.links, "projects": grabber.projects, "map": grabber.map, "sites": grabber.sites, "quiet": grabber.quiet, "folders": grabber.folders, "notes": grabber.notes, "collections": grabber.collections, "health": grabber.health, "movetask": grabber.movetask, "google": grabber.google, "github": grabber.github, "batch-a": grabber.batchA, "export": grabber.exportCsv, "noantivirus": grabber.noAntivirus, "watch": grabber.watch, "share": grabber.share, "share-join": grabber.shareJoin, "share-two": grabber.shareTwo, "parameters": grabber.parameters, "closed-on": grabber.closedOn, "closed-off": grabber.closedOff, "papers": grabber.papers, "budget": grabber.budget, "energy": grabber.energy, "day": grabber.day, "bitwarden-key": grabber.bitwardenKey, "bitwarden-passkey": grabber.bitwardenPasskey, "bitwarden-choose": grabber.bitwardenChoose, "areas": grabber.areas, "presets": grabber.presets, "ownership": grabber.ownership, "zoom": grabber.zoom, "accounts": grabber.accounts, "account-tabs": grabber.accountTabs, "batch-11": grabber.batch11, "contracts": grabber.contracts, "bank": grabber.bankSteps, "porch-money": grabber.porchMoney, "letters": grabber.lettersSteps, "letters-act": grabber.lettersAct, "demo": grabber.demo, "phone": grabber.phone })[sioul.grabSteps()] || grabber.pages
            // The documentation's pictures, on the demo profile (tools/demo/screenshots.sh):
            // each place as it is used, a weekday afternoon. Run again on the profile
            // without hours (make-demo.py --no-hours), where everything comes at once:
            // the Porch asking for them, and the budgets of every area together.
            // In French (make-demo.py --language fr), the same places, under its names.
            readonly property bool demoFrench: sioul.text("qt-locale") === "fr_FR"
            // Each page in a phone's layout (SIOUL_GRAB_STEPS=phone), Settings scrolled too.
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
            // The login chooser, on a made-up vault (SIOUL_TEST_VAULT): a site with two
            // accounts, then a search over the vault, then the one chosen last proposed first.
            readonly property var bitwardenChoose: [
                () => window.page = 3,
                () => sioul.bitwardenState(),
                () => sitesPage.loginChooser.begin("https://www.example.org/login"),
                () => {},
                () => sitesPage.loginChooser.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/choose-site.png")),
                () => sitesPage.loginChooser.query = "bank",
                () => {},
                () => sitesPage.loginChooser.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/choose-search.png")),
                () => sitesPage.loginChooser.close(),
                () => sioul.bitwardenLogin("https://www.example.org/login", "b"),
                () => sitesPage.loginChooser.begin("https://www.example.org/login"),
                () => {},
                () => sitesPage.loginChooser.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/choose-last-first.png")),
                () => window.close()
            ]
            // Areas and hours (docs/areas.md), on a test week whose Sunday is free time: the
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
                () => tasksPage.panel.moreShown = true,
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
                () => budgetsPage.bank.rulesDialog.show(budgetsPage.bank.shown.accounts[0]),
                () => {},
                () => grabber.savePopup(budgetsPage.bank.rulesDialog, "accounts-rules"),
                () => budgetsPage.bank.rulesDialog.close(),
                () => budgetsPage.bank.accountDialog.edit(budgetsPage.bank.shown.accounts[0]),
                () => {},
                () => grabber.savePopup(budgetsPage.bank.accountDialog, "accounts-form"),
                () => budgetsPage.bank.accountDialog.close(),
                () => budgetsPage.bank.reserveDialog.edit(budgetsPage.bank.shown.reserves[1]),
                () => {},
                () => grabber.savePopup(budgetsPage.bank.reserveDialog, "accounts-reserve"),
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
                () => budgetsPage.contracts.dialog.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/contract-dialog.png")),
                () => window.close()
            ]
            // The day seen, the routines, one played.
            readonly property var day: [
                () => window.page = 1,
                () => tasksPage.mode = "day",
                () => {},
                () => grabber.save("day"),
                () => tasksPage.routines.show(),
                () => {},
                () => tasksPage.routines.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/routines.png")),
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
                () => papersPage.dialog.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/paper-dialog.png")),
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
                () => grabber.save("tasks-now"),
                () => tasksPage.mode = "list",
                () => grabber.save("tasks-list"),
                () => tasksPage.mode = "board",
                () => grabber.save("tasks-board"),
                () => tasksPage.mode = "timeline",
                () => grabber.save("tasks-timeline"),
                () => tasksPage.openFirst(),
                () => grabber.save("tasks-panel"),
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
                () => window.page = 9,
                () => grabber.save("budgets"),
                () => budgetsPage.openFirst(),
                () => grabber.save("budget"),
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
            // the deepest items only (their containers pass it with them).
            function overflow(name) {
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
                walk(window.contentItem, 0)
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
                wide(window.contentItem, 0)
            }

            // An open menu or pop-up, which the frame leaves out.
            function savePopup(popup, name) {
                popup.contentItem.parent.grabToImage(result => result.saveToFile(grabber.folder + "/" + name + ".png"))
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
        id: newMenu
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

    Rectangle {
        id: frame

        anchors.fill: parent
        color: theme.background

        Item {
            anchors.fill: parent

            // The places: beside the pages, or on a phone pulled over them from the left.
            Rectangle {
                id: places

                z: 2
                width: 200
                height: parent.height
                x: !window.compact || window.placesOpen ? 0 : -width - 1
                color: theme.surface

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

                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: theme.gap
                    spacing: 6

                    // Something new, of any kind: always here, whatever the page.
                    Button {
                        id: newButton

                        Layout.fillWidth: true
                        Layout.bottomMargin: theme.gap
                        text: sioul.text("ui-new") + "  ▾"
                        icon.name: "list-add"
                        icon.color: theme.accentText
                        highlighted: true
                        onClicked: window.showNewMenu()
                    }

                    // Where your things live.
                    Repeater {
                        // [page, name]: the papers' page came after the others, its place is by the budgets.
                        model: [[0, "ui-porch"], [1, "ui-tasks"], [2, "ui-mail"], [3, "ui-sites"], [4, "ui-agenda"], [5, "ui-contacts"], [6, "ui-notes"], [7, "ui-projects"], [8, "ui-time"], [9, "ui-budgets"], [13, "ui-papers"], [10, "ui-health"]]

                        delegate: Button {
                            required property var modelData

                            Layout.fillWidth: true
                            text: sioul.text(modelData[1])
                            checkable: true
                            checked: window.page === modelData[0]
                            flat: true
                            font.pixelSize: 16
                            onClicked: {
                                window.page = modelData[0]
                                window.placesOpen = false
                            }
                        }
                    }

                    Item {
                        Layout.fillHeight: true
                    }

                    // Sioul itself, apart: the accounts and the settings.
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: theme.line
                    }
                    // The accounts, the settings, and everything fetched again: one row of icons.
                    RowLayout {
                        Layout.alignment: Qt.AlignHCenter
                        spacing: 4

                        Repeater {
                            model: [[11, "ui-accounts", "user-identity"], [12, "ui-parameters", "settings-configure"]]

                            delegate: ToolButton {
                                required property var modelData

                                icon.name: modelData[2]
                                icon.color: theme.text
                                display: AbstractButton.IconOnly
                                checkable: true
                                checked: window.page === modelData[0]
                                Accessible.name: sioul.text(modelData[1])
                                ToolTip.visible: hovered
                                ToolTip.text: sioul.text(modelData[1])
                                ToolTip.delay: 600
                                onClicked: {
                                    window.page = modelData[0]
                                    window.placesOpen = false
                                }
                            }
                        }
                        // Mail, agenda, tasks, contacts and the rest, fetched again at once.
                        ToolButton {
                            id: refreshButton

                            icon.name: "view-refresh"
                            icon.color: theme.text
                            enabled: !sioul.busy
                            Accessible.name: sioul.text("ui-refresh-all")
                            ToolTip.visible: hovered
                            ToolTip.text: sioul.busy ? sioul.text("ui-refreshing") : sioul.text("ui-refresh-all")
                            ToolTip.delay: 600
                            onClicked: sioul.syncNow()
                        }
                    }
                    // Turning slowly while it fetches.
                    RotationAnimator {
                        target: refreshButton.contentItem
                        running: sioul.busy
                        from: 0
                        to: 360
                        duration: 1600
                        loops: Animation.Infinite
                        onRunningChanged: if (!running) refreshButton.contentItem.rotation = 0
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
                anchors.leftMargin: window.compact ? 0 : places.width
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
                        anchors.leftMargin: theme.gap
                        anchors.rightMargin: theme.gap
                        spacing: theme.gap

                        Label {
                            Layout.fillWidth: true
                            text: sioul.undoLine !== "" ? sioul.undoLine : sioul.status
                            // Server answers and file names: never read as rich text.
                            textFormat: Text.PlainText
                            color: sioul.undoLine !== "" ? theme.text : theme.muted
                            elide: Text.ElideRight
                        }
                        // Quiet time, or work kept late: the line, and behind it the
                        // way back, never suggested.
                        Button {
                            id: modeButton

                            visible: window.moment.line !== "" && sioul.undoLine === ""
                            Layout.maximumWidth: Math.round(window.width * 0.55)
                            Layout.preferredHeight: 28
                            flat: true
                            text: window.moment.line
                            ToolTip.visible: hovered
                            ToolTip.text: window.moment.line
                            ToolTip.delay: 800
                            onClicked: {
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
                                    // Work shown whatever the hours, as on the Porch.
                                    MenuItem {
                                        id: workNowItem

                                        visible: window.moment.quiet || window.moment.work_now === true
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
                                        model: window.moment.quiet && !(window.moment.reason === "done-for-the-day" && window.moment.today) ? [30, 60, 120, 240] : []

                                        delegate: MenuItem {
                                            required property int modelData

                                            text: sioul.text("mode-work-" + modelData)
                                            onTriggered: sioul.workAWhile(modelData)
                                        }
                                    }
                                }
                            }
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
                        Label {
                            visible: window.moment.line === "" && !window.compact
                            text: sioul.text("ui-keys")
                            color: theme.muted
                            font.pixelSize: 12
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
                }
            }
        }
    }
}
