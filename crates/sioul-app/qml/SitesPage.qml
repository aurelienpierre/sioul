// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sites kept in Sioul: secure mailboxes, chats, a dating site. Pinned on the
// left, one open on the right, logged in once: the profile keeps cookies and
// the files sites download to show themselves. Their notifications are always
// accepted, then wait for the Porch, unless the site is in real time; calls
// and video work (microphone, camera, sharing the screen). A site can be
// silenced; a chat's real time is one tick away, always in view. Logins come
// from Bitwarden when you ask, its account set in Accounts (VaultCard.qml).
// The profile every site shares, and Bitwarden's page that asks a security
// key, are the window's (SitesWeb.qml), so that Accounts asks a key too. A
// file a site offers waits in Sioul's own folder until you say where it
// goes (SiteDownloads.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
import QtQuick.Layouts
import QtMultimedia
import QtWebEngine
// colour: ScreenColours, a type of Sioul's own module (cpp/screencolours.h).
import com.aurelienpierre.sioul

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var sites: []
    property string openId: ""
    // The column of sites folded to their icons, as you left it (SitesList.qml).
    property alias narrow: list.narrow
    // For the window's images: Bitwarden's dialog, the page it asks a key from, the chooser.
    property alias bitwardenUnlock: unlock
    property alias bitwardenKeyView: unlock.keyView
    property alias loginChooser: chooser
    property alias addSiteDialog: adding
    property alias addSiteFind: adding.findText
    // The files the sites offer, and their card (the window's images).
    property alias downloads: siteFiles
    // What follows Bitwarden's unlocking: the login filled ("fill") or chosen ("choose").
    property string afterUnlock: "fill"
    // Sites opened this session stay loaded: going back to one does not reload it.
    property var visited: ({})
    property string problem: ""
    // Each site's view, once made, by its id.
    property var siteViews: ({})
    // The web notices Sioul told (`siteNotified` true), kept by "<site
    // id>#<key>" ({ notice, at }; the key their tag, "" without one, so that
    // the next takes its place): Qt gives them to JavaScript, which would
    // collect them; kept, a click on Sioul's notice calls their own click()
    // (`clickNotice`), and a call's `closed` takes Sioul's away
    // (docs/notifications.md). Each goes once its site is opened, or after
    // the longest a notice of Sioul's lasts (`webNoticeKept`).
    property var webNotices: ({})
    readonly property int webNoticeKept: 60 * 60 * 1000
    // Sioul quitting: every page closed (`closeAll`), for the program to end (cpp/webengine.cpp).
    signal sitesClosed()
    // How long a page may take to close: Chromium itself gives one that does
    // not answer half a second for its beforeunload, as much for its unload.
    readonly property int closeWait: 2000
    // The sites taken out of Sioul whose pages are closing, by id: their views go once closed.
    property var leaving: ({})
    // The pop-ups open (`openPopup`): their pages are closed with the sites'.
    property var popups: []
    // Calls take the camera, microphone and speaker you chose, by their names
    // (the browser's own ids are each site's), and change them during a call
    // (src/calls.js, docs/sites.md). The names go only to sites allowed a
    // microphone or a camera, which can read them from their browser anyway;
    // inside the script they stay out of the page's reach. A change comes to
    // the pages open as an event ("sioul-devices"), in every frame.
    readonly property string callsScript: page.sioul.callsScript()
    // A secret of this run, which marks what the calls script says in the
    // console: a page cannot write in its name. Qt's JavaScript draws
    // Math.random from QRandomGenerator::global(), seeded by the system.
    readonly property string callToken: [0, 1, 2, 3].map(() => Math.floor(Math.random() * 4294967296).toString(36)).join("")
    function devicesFor(site) {
        return site && (site.microphone || site.camera) ? page.callDevices : ({})
    }
    // A page's scripts, for a site (null: a page of no site). Bindings read
    // it: the sites' web holder is made before them (`keepViews`).
    function webFixesFor(site) {
        // In every page, before its own scripts, the security keys' fix (SitesWeb.qml).
        return [page.window.sitesWeb().webauthnFix, {
            name: "sioul-calls",
            sourceCode: "(" + page.callsScript + ")(" + JSON.stringify(page.devicesFor(site)) + ", " + JSON.stringify(page.callToken) + ");",
            injectionPoint: 2,
            worldId: 0,
            runsOnSubFrames: true
        }]
    }
    // This page's ⚙, open at one of its settings ("calmer_colours", from a link by key:
    // SettingDoor.qml), or closed.
    function openSettings(open, key) {
        list.settingsButton.show(open)
        if (open && key)
            Qt.callLater(() => list.settingsButton.scrollTo(key))
    }

    // Chats covered: their daily time used (Health page), until it comes back.
    property bool chatsCovered: page.sioul.chatsCovered()

    // colour: the two lines of this page's ⚙, for every site and pop-up: the
    // screen's own colours, and calmer colours (cpp/screencolours.cpp, docs/colour.md).
    readonly property var colourSettings: page.sioul.reading ? JSON.parse(page.sioul.reading) : ({})
    Binding {
        target: ScreenColours
        property: "screenConversion"
        value: page.colourSettings.screen_colours !== false
    }
    Binding {
        target: ScreenColours
        property: "calmer"
        value: ({ "little": 1, "more": 2 })[page.colourSettings.calmer_colours] || 0
    }

    // A minute in a chat, counted while you look at it; covered, then back after their time.
    Timer {
        interval: 60000
        running: true
        repeat: true
        onTriggered: {
            const looking = page.visible && page.window.active && page.opened !== null && page.opened.kind === "chat"
            page.chatsCovered = looking ? page.sioul.chatMinute() : page.sioul.chatsCovered()
        }
    }
    readonly property var opened: page.sites.find(s => s.id === page.openId) || null
    // The site the site menu acts on: the open one (⋮), or one right-clicked in the list.
    property string menuId: ""
    readonly property var menuSite: page.sites.find(s => s.id === page.menuId) || null
    readonly property int menuIndex: page.sites.findIndex(s => s.id === page.menuId)
    // The list's filters, its order and its folded line (SitesList.qml), for the window's images.
    property alias areaFilter: list.areaFilter
    property alias kindFilter: list.kindFilter
    property alias categoryFilter: list.categoryFilter

    // A sign-in window, a call window: each in a window of its own, on the
    // sites' profile, so it stays tied to the page that opened it (a sign-in
    // answers it there). It may use what the site that opened it may, kept
    // as it was then: not the switches of the site in front at the time.
    function openPopup(request, site) {
        const window = popupWindow.createObject(page, { site: site || null }) as SitePopup
        page.popups = page.popups.filter(p => page.popupOpen(p)).concat([window])
        request.openIn(window.view)
        window.show()
        window.raise()
    }

    // Sioul quits: every site's page closed as a browser closes its tabs,
    // before Qt WebEngine shuts down. Its beforeunload, pagehide and unload
    // run: some sites keep their login only in the open page and write it
    // back to their storage then (Discord takes its token out of localStorage
    // once loaded, and puts it back as the page goes); a page destroyed
    // without them loses it, and the site asks to log in again. The pop-ups'
    // pages too. Returns how many are still closing; `sitesClosed` says when
    // none is (cpp/webengine.cpp waits for it, two seconds at most).
    function closeAll() {
        // The files under way let go, their held folders gone (SiteDownloads.qml).
        siteFiles.closeAll()
        // How long it takes, with SIOUL_TIMING.
        page.sioul.mark("closing the sites' pages")
        const closing = []
        for (let i = 0; i < views.count; i++) {
            const holder = views.itemAt(i)
            if (holder && holder.made)
                closing.push(holder)
        }
        for (const popup of page.popups) {
            if (page.popupOpen(popup) && !popup.pageClosed)
                closing.push(popup)
        }
        let waiting = closing.length
        const one = () => {
            waiting -= 1
            if (waiting === 0) {
                page.sioul.mark("the sites' pages closed")
                page.sitesClosed()
            }
        }
        for (const item of closing) {
            // One that cannot close is not waited for: the others still are.
            try {
                item.closePage(one)
            } catch (e) {
                console.warn("Sioul: a site's page could not be closed: " + e)
                one()
            }
        }
        return waiting
    }
    // A pop-up not gone yet (one gone reads as null, or as nothing).
    function popupOpen(popup) {
        return popup !== null && popup !== undefined && typeof popup.closePage === "function"
    }

    property alias presetsMenu: list.presetsMenu
    property alias siteMenu: siteMenu
    property alias devicesDialog: devicesDialog
    property alias callPanel: callPanel
    property alias filterMenu: list.filterMenu
    function showPresets() {
        list.showPresets()
    }
    function showFilters() {
        list.showFilters()
    }
    // The list by type, or in your own order, as you left it.
    property alias byCategory: list.byCategory
    // The sites for other hours (docs/areas.md), shown when asked.
    property alias laterShown: list.laterShown
    readonly property int openedIndex: page.sites.findIndex(s => s.id === page.openId)

    function reload() {
        page.sites = JSON.parse(page.sioul.siteList() || "[]")
        page.keepViews()
        page.keepPermissions()
    }

    // The views follow the sites: one added or taken out, the others untouched.
    ListModel {
        id: viewIds
    }
    function keepViews() {
        const ids = page.sites.map(s => s.id)
        for (let i = viewIds.count - 1; i >= 0; i--) {
            if (ids.indexOf(viewIds.get(i).siteId) < 0)
                page.dropView(viewIds.get(i).siteId)
        }
        const have = []
        for (let i = 0; i < viewIds.count; i++)
            have.push(viewIds.get(i).siteId)
        for (const id of ids) {
            if (have.indexOf(id) < 0) {
                // The sites' web holder (SitesWeb.qml) made first, here: a view's
                // `fixes` reads it at once (`webFixesFor`), and made from inside
                // that binding it changed the loader the binding had just read
                // (a binding loop on "fixes").
                page.window.sitesWeb()
                viewIds.append({ siteId: id })
            }
        }
    }

    // A site taken out of Sioul: its page closed first, as a browser closes a
    // tab (its login stays in the profile, whole), then its view goes.
    function dropView(id) {
        if (page.leaving[id] === true)
            return
        // Closed: its files, if they were refused, are asked again should it come back.
        siteFiles.forgive(id)
        let holder = null
        for (let i = 0; i < views.count; i++) {
            const item = views.itemAt(i)
            if (item && item.siteId === id)
                holder = item
        }
        const gone = () => {
            delete page.leaving[id]
            if (page.calls[id] !== undefined) {
                const calls = Object.assign({}, page.calls)
                delete calls[id]
                page.calls = calls
            }
            for (let i = 0; i < viewIds.count; i++) {
                if (viewIds.get(i).siteId === id) {
                    viewIds.remove(i)
                    break
                }
            }
            // Put back while its page closed: a new view.
            page.keepViews()
        }
        if (holder === null) {
            gone()
            return
        }
        page.leaving[id] = true
        holder.closePage(gone)
    }

    // Notifications granted to every site before it asks, and kept: Sioul
    // takes them and gathers them itself, so a site never asks again. A
    // microphone, camera or screen once allowed is forgotten when its switch
    // is off.
    function keepPermissions() {
        if (!page.profile)
            return
        const kinds = WebEnginePermission.PermissionType
        for (const site of page.sites) {
            const origin = (String(site.url).match(/^https?:\/\/[^\/?#]+/) || [""])[0]
            if (origin === "")
                continue
            page.profile.queryPermission(origin, kinds.Notifications).grant()
            const kept = [[site.microphone, kinds.MediaAudioCapture], [site.camera, kinds.MediaVideoCapture], [site.microphone && site.camera, kinds.MediaAudioVideoCapture]]
            for (const [on, type] of kept) {
                if (!on)
                    page.profile.queryPermission(origin, type).reset()
            }
        }
    }

    // The camera, microphone and speaker calls use, by their names: given to the
    // pages of sites allowed a device (`webFixesFor`, for the pages to come),
    // and to every frame of those open now, pop-ups too: calls in progress
    // change at once.
    property var callDevices: JSON.parse(page.sioul.callDevices() || "{}")
    function setCallDevice(which, name) {
        page.problem = page.sioul.setCallDevice(which, name)
        page.callDevices = JSON.parse(page.sioul.callDevices() || "{}")
        const said = "window.dispatchEvent(new CustomEvent('sioul-devices', { detail: " + JSON.stringify(page.callDevices) + " }));"
        const allowed = site => site !== null && site !== undefined && (site.microphone || site.camera)
        for (const site of page.sites) {
            if (allowed(site))
                page.everyFrame(page.viewOf(site.id), said)
        }
        for (const popup of page.popups) {
            if (page.popupOpen(popup) && allowed(popup.site))
                page.everyFrame(popup.view, said)
        }
    }
    // A script run in each frame of a view, in the page's world: a call may be
    // in a frame of another site (an embedded room), which the view's own
    // runJavaScript never reaches.
    function everyFrame(view, script) {
        if (!view)
            return
        const visit = frame => {
            if (!frame.isValid)
                return
            frame.runJavaScript(script, 0, () => {})
            for (const child of frame.children)
                visit(child)
        }
        visit((view as WebEngineView).mainFrame)
    }
    // The sites with a call now, by id: how many of their pages and frames
    // have one (src/calls.js says "call" and "end"); a pop-up counts its own
    // (`callCount`), gone with it. The call button shows then.
    property var calls: ({})
    function calling(id) {
        return (page.calls[id] || 0) > 0 || page.popups.some(p => page.popupOpen(p) && p.site !== null && p.site.id === id && p.callCount > 0)
    }
    // What a page's console says: the calls script's lines carry this run's
    // token; a device it could not open is said in the line above the site,
    // with the name Sioul has, never words from the page. `popup`: the pop-up
    // it comes from, or none.
    function heard(site, message, popup) {
        const mark = "sioul:" + page.callToken + ":"
        if (!site || !String(message).startsWith(mark))
            return
        let said = null
        try {
            said = JSON.parse(String(message).slice(mark.length))
        } catch (e) {
            return
        }
        const next = Object.assign({}, page.calls)
        if (said.what === "call" || said.what === "end") {
            const step = said.what === "call" ? 1 : -1
            // The call's panel lists the devices at once.
            if (said.what === "call")
                Qt.callLater(page.knowDevices)
            if (popup)
                popup.callCount = Math.max(0, popup.callCount + step)
            else
                next[site.id] = Math.max(0, (next[site.id] || 0) + step)
        } else if (said.what === "kept") {
            const which = ({ "video": "camera", "audio": "microphone", "speaker": "speaker" })[said.kind]
            if (which)
                page.problem = page.sioul.textArgs("site-device-kept-" + which, JSON.stringify({ site: site.name, device: page.callDevices[which] || page.sioul.text("site-device-system") }))
        }
        page.calls = next
    }

    function open(id) {
        page.openId = id
        page.problem = ""
        const next = Object.assign({}, page.visited)
        next[id] = true
        page.visited = next
        page.sioul.siteSeen(id)
        page.forgetNotices(id)
    }

    // "https://app.element.io/#/room" → "app.element.io".
    function hostOf(url) {
        const m = String(url).match(/^[a-z]+:\/\/(?:[^@\/]*@)?([^:\/?#]+)/i)
        return m ? m[1].toLowerCase() : ""
    }

    // The site a notification comes from: the same host, or the same domain.
    function siteOf(origin) {
        const host = page.hostOf(origin)
        const tail = h => h.split(".").slice(-2).join(".")
        return page.sites.find(s => page.hostOf(s.url) === host) || page.sites.find(s => tail(page.hostOf(s.url)) === tail(host)) || null
    }

    // A site's notice clicked (main.qml's `open`, after the site is open):
    // its web notice's own click, which the site answers (a chat opens the
    // conversation); false when it is gone (the view unloaded, withdrawn).
    function clickNotice(id, key) {
        const kept = page.webNotices[id + "#" + key]
        if (!kept)
            return false
        kept.notice.click()
        return true
    }

    // The web notices kept no longer: a site's own once it is opened (`site`),
    // and every one older than `webNoticeKept`.
    function forgetNotices(site) {
        const old = Date.now() - page.webNoticeKept
        for (const kept of Object.keys(page.webNotices)) {
            if ((site !== "" && kept.startsWith(site + "#")) || page.webNotices[kept].at < old)
                delete page.webNotices[kept]
        }
    }

    function alive(site) {
        return site.background || page.visited[site.id] === true || site.id === page.openId
    }

    function viewOf(id) {
        return page.siteViews[id] || null
    }

    // The login Bitwarden keeps for the open site, written into its form: the
    // only one at once, unless another was chosen there last; else chosen in
    // the chooser, found by site and by user name.
    function fillLogin() {
        const view = page.viewOf(page.openId)
        if (view === null || !page.vaultOpen("fill"))
            return
        const site = JSON.parse(page.sioul.bitwardenSite(view.url.toString()))
        if (site.error) {
            page.problem = site.error
            return
        }
        page.problem = ""
        if (site.only !== "")
            page.fillChosen(site.only)
        else
            chooser.now().begin(view.url.toString())
    }

    // Any login of the vault, chosen in the list.
    function chooseLogin() {
        const view = page.viewOf(page.openId)
        if (view === null || !page.vaultOpen("choose"))
            return
        page.problem = ""
        chooser.now().begin(view.url.toString())
    }

    // Whether the vault is open; else its dialog, then `then` ("fill" or "choose").
    function vaultOpen(then) {
        const state = page.sioul.bitwardenState()
        if (state === "missing") {
            page.problem = page.sioul.text("bitwarden-missing")
            return false
        }
        if (state === "locked") {
            page.afterUnlock = then
            unlock.begin()
            return false
        }
        return true
    }

    // One login of the vault, by its id, written into the open site's form.
    function fillChosen(item) {
        const view = page.viewOf(page.openId)
        if (view === null)
            return
        const login = JSON.parse(page.sioul.bitwardenLogin(view.url.toString(), item))
        if (login.error) {
            page.problem = login.error
            return
        }
        page.problem = ""
        page.fillWith(view, login.username, login.password, login.code)
    }

    // Values through JSON, so nothing in them can change the script.
    function fillWith(view, user, password, code) {
        view.runJavaScript("(" + page.fillScript + ")(" + JSON.stringify(user) + ", " + JSON.stringify(password) + ", " + JSON.stringify(code || "") + ")")
    }

    // Sets the password field and the name field before it, as typing would.
    readonly property string fillScript: `function (user, password, code) {
        const set = (field, value) => {
            const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(field), 'value').set;
            setter.call(field, value);
            field.dispatchEvent(new Event('input', { bubbles: true }));
            field.dispatchEvent(new Event('change', { bubbles: true }));
        };
        const secret = document.querySelector('input[type=password]');
        const names = Array.from(document.querySelectorAll('input[type=email], input[type=text], input:not([type])'));
        const before = secret ? names.filter(f => f.compareDocumentPosition(secret) & Node.DOCUMENT_POSITION_FOLLOWING) : names;
        const name = before.length ? before[before.length - 1] : null;
        if (name && user) set(name, user);
        if (secret && password) set(secret, password);
        // A page asking for the one-time code: the code the vault computes now.
        const otp = document.querySelector('input[autocomplete="one-time-code"]') || Array.from(document.querySelectorAll('input:not([type=password]):not([type=hidden])')).find(f => /otp|totp|2fa|mfa|code|token/i.test((f.name || '') + ' ' + (f.id || '') + ' ' + (f.getAttribute('aria-label') || '')));
        if (otp && code && !secret) set(otp, code);
        return !!(name || secret);
    }`

    onVisibleChanged: if (page.visible) page.reload()

    Connections {
        target: page.sioul

        function onSitesChanged() {
            page.reload()
        }
    }

    // One profile for every site: logins kept, files cached on disk. It is
    // the window's (SitesWeb.qml), shared with Bitwarden's page that asks a
    // security key, and starts Chromium once made (some 25 MB in Sioul's
    // process and 45 MB in its zygotes, with no site): made with the first
    // site's view, a site kept open or one opened, never for the page shown
    // with no site open.
    property WebEngineProfile profile: null
    readonly property bool profileWanted: page.sites.some(s => page.alive(s))
    onProfileWantedChanged: page.makeProfile()

    // The profile, complete once made; no view exists before it.
    function makeProfile() {
        if (page.profile !== null || !page.profileWanted)
            return
        page.profile = page.window.sitesWeb().profile()
        page.keepPermissions()
    }

    Component.onCompleted: {
        page.reload()
        page.makeProfile()
    }

    Connections {
        target: page.profile

        // Accepted always; the site in front of you needs no word, the others
        // show at once in real time, else wait for the Porch. Each kept by its
        // site and its key (its tag: one per conversation on a chat site), the
        // last of a key in the place of the one before; withdrawn by the site,
        // Sioul's notice of it goes too.
        function onPresentNotification(notification) {
            const site = page.siteOf(notification.origin)
            const looking = site !== null && page.visible && site.id === page.openId
            if (site && !looking) {
                const key = notification.tag
                const kept = site.id + "#" + key
                page.forgetNotices("")
                // Kept only when Sioul told it: what waits for the Porch alone keeps nothing here.
                if (page.sioul.siteNotified(site.id, key, notification.title, notification.message)) {
                    page.webNotices[kept] = { notice: notification, at: Date.now() }
                    notification.closed.connect(() => {
                        const held = page.webNotices[kept]
                        if (!held || held.notice !== notification)
                            return
                        delete page.webNotices[kept]
                        page.sioul.siteNoticeClosed(site.id, key)
                    })
                }
            }
            notification.show()
        }

        // A file a site offers: accepted at once into Sioul's own folder, where it
        // waits for you to say where (SiteDownloads.qml). Its view read here, once:
        // Qt leaves it dangling once the page is gone.
        function onDownloadRequested(download) {
            const view = download.view
            const from = page.siteOfView(view)
            siteFiles.offer(download, from.site, view && view.url ? String(view.url) : "", from.popup)
        }
    }

    // The files the sites offer: each asked in its card, one after the other,
    // over the pop-up that offered it or over this page while it is in view
    // (docs/sites.md, "Downloads").
    SiteDownloads {
        id: siteFiles

        sioul: page.sioul
        theme: page.theme
        window: page.window
        shown: page.visible && page.window.visible
    }

    // The site a page belongs to, and the pop-up when it is one: a site's own
    // view, or a pop-up it opened; {site: null, popup: null} for another page.
    function siteOfView(view) {
        if (!view)
            return { site: null, popup: null }
        for (const id of Object.keys(page.siteViews)) {
            if (page.siteViews[id] === view)
                return { site: page.sites.find(s => s.id === id) || null, popup: null }
        }
        for (const popup of page.popups) {
            if (page.popupOpen(popup) && popup.view === view)
                return { site: popup.site, popup: popup }
        }
        return { site: null, popup: null }
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // The sites, pinned: a narrow column, or their icons only, as you left it.
        SitesList {
            id: list

            Layout.fillHeight: true
            Layout.preferredWidth: list.narrow ? 40 : 170
            Layout.maximumWidth: list.narrow ? 40 : 170
            sioul: page.sioul
            theme: page.theme
            sites: page.sites
            openId: page.openId
            canNarrow: true
            settings: true
            onChosen: id => page.open(id)
            onMenuAsked: id => {
                page.menuId = id
                siteMenu.popup()
            }
            onAddAsked: adding.open()
            onPinned: problem => {
                page.problem = problem
                page.reload()
            }
        }

        // The site open.
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 6

            RowLayout {
                visible: page.opened !== null
                Layout.fillWidth: true
                spacing: 4

                ToolButton {
                    icon.name: "go-previous"
                    icon.color: page.theme.text
                    Accessible.name: page.sioul.text("site-back")
                    onClicked: {
                        const view = page.viewOf(page.openId)
                        if (view)
                            view.goBack()
                    }
                }
                ToolButton {
                    icon.name: "view-refresh"
                    icon.color: page.theme.text
                    Accessible.name: page.sioul.text("site-reload")
                    onClicked: {
                        const view = page.viewOf(page.openId)
                        // Reloaded by you: a site whose files were refused is asked again.
                        siteFiles.forgive(page.openId)
                        if (view)
                            view.reload()
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: page.opened ? page.opened.name : ""
                    textFormat: Text.PlainText
                    font.pixelSize: 18
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                // A chat's real time: its notifications at once, until unticked.
                CheckBox {
                    visible: page.opened !== null && page.opened.kind === "chat"
                    text: page.sioul.text("ui-realtime")
                    checked: page.opened !== null && page.opened.realtime
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("site-realtime-help")
                    ToolTip.delay: 400
                    onToggled: page.problem = page.sioul.setSite(page.openId, "realtime", String(checked))
                }
                // A call in this site, or in a pop-up it opened: its camera, microphone
                // and speaker, changed at once (`callPanel`).
                ToolButton {
                    id: callButton

                    visible: page.opened !== null && page.calling(page.openId)
                    icon.name: "camera-video"
                    icon.color: page.theme.text
                    Accessible.name: page.sioul.text("site-call-devices")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("site-call-devices")
                    ToolTip.delay: 400
                    onClicked: callPanel.open()
                }
                ToolButton {
                    checkable: true
                    checked: page.opened !== null && page.opened.muted
                    icon.name: checked ? "audio-volume-muted" : "audio-volume-high"
                    icon.color: page.theme.text
                    Accessible.name: page.sioul.text("site-mute")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("site-mute")
                    onToggled: page.problem = page.sioul.setSite(page.openId, "muted", String(checked))
                }
                Button {
                    flat: true
                    text: page.sioul.text("site-fill-login")
                    icon.name: "password-copy"
                    icon.color: page.theme.text
                    onClicked: page.fillLogin()
                }
                ToolButton {
                    id: more

                    icon.name: "overflow-menu"
                    icon.color: page.theme.text
                    Accessible.name: page.sioul.text("ui-more")
                    onClicked: {
                        page.menuId = page.openId
                        siteMenu.popup(more, 0, more.height)
                    }
                }
            }
            Label {
                visible: page.problem !== ""
                Layout.fillWidth: true
                text: page.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.warm
            }
            Label {
                visible: page.opened === null && page.sites.length > 0
                Layout.fillWidth: true
                Layout.topMargin: 40
                text: page.sioul.text("site-choose")
                horizontalAlignment: Text.AlignHCenter
                color: page.theme.muted
            }

            // Each site's view, made when it is first opened (a chat: at once, to hear it).
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                // Over a covered chat: one calm line, and no click goes through.
                Rectangle {
                    z: 2
                    visible: page.chatsCovered && page.opened !== null && page.opened.kind === "chat"
                    anchors.fill: parent
                    color: Qt.rgba(page.theme.background.r, page.theme.background.g, page.theme.background.b, 0.6)

                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.AllButtons
                        onWheel: wheel => wheel.accepted = true
                    }
                    Label {
                        anchors.centerIn: parent
                        width: Math.min(parent.width - 2 * page.theme.gap, 420)
                        text: page.sioul.text("site-chats-covered")
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                        font.pixelSize: 17
                        color: page.theme.text
                    }
                }

                // One view per site, by its id: a site's change (silenced, news,
                // its type) leaves every view as it is. A list of the sites as the
                // model would make each view again, reloading every page, a call too.
                Repeater {
                    id: views

                    model: viewIds

                    delegate: Item {
                        id: holder

                        required property string siteId
                        readonly property var modelData: page.sites.find(s => s.id === holder.siteId) || ({ id: holder.siteId })
                        // Where the page starts: it goes there again only when the address changes.
                        readonly property string startUrl: holder.modelData.url || ""
                        readonly property bool covered: page.chatsCovered && holder.modelData.kind === "chat"

                        anchors.fill: parent
                        // The site in view, while the window shows or a call goes on there:
                        // hidden in the tray, its page is hidden as the others are (Chromium
                        // stops drawing it, sixty frames a second for nobody), its sockets
                        // and its notifications going on.
                        visible: page.openId === holder.siteId && (page.window.visible || page.calling(holder.siteId))
                        // Covered: blurred, and nothing under it takes a click.
                        layer.enabled: holder.covered
                        layer.effect: MultiEffect {
                            blurEnabled: true
                            blur: 1.0
                            blurMax: 64
                        }
                        Component.onDestruction: delete page.siteViews[holder.siteId]
                        // Its pages' scripts: given again when the site or the devices change,
                        // for the pages it loads next (a reload starts with the names of now).
                        readonly property var fixes: page.webFixesFor(holder.modelData)
                        onFixesChanged: {
                            const view = loader.item as WebEngineView
                            if (view)
                                view.userScripts.collection = holder.fixes
                        }

                        // Its view, made when first wanted (`page.alive`), stays for the
                        // session: it goes only closed (`closePage`), as Sioul quits or
                        // the site is taken out.
                        readonly property bool wanted: page.profile !== null && page.alive(holder.modelData)
                        property bool made: false
                        onWantedChanged: {
                            if (holder.wanted)
                                holder.made = true
                        }
                        Component.onCompleted: {
                            if (holder.wanted)
                                holder.made = true
                        }
                        // Who waits for its page to close.
                        property var whenClosed: []

                        // Its page closed as a browser closes a tab: its beforeunload,
                        // then its pagehide and unload (where Discord puts its login
                        // back in its storage); then `done`, at most `page.closeWait`
                        // later. A view not made has nothing to run.
                        function closePage(done) {
                            const view = loader.item as WebEngineView
                            if (view === null) {
                                done()
                                return
                            }
                            holder.whenClosed = holder.whenClosed.concat([done])
                            if (closeDeadline.running)
                                return
                            closeDeadline.start()
                            view.triggerWebAction(WebEngineView.RequestClose)
                        }
                        function pageClosed() {
                            closeDeadline.stop()
                            const waiting = holder.whenClosed
                            holder.whenClosed = []
                            for (const done of waiting)
                                done()
                        }

                        Timer {
                            id: closeDeadline

                            interval: page.closeWait
                            onTriggered: holder.pageClosed()
                        }

                        Loader {
                            id: loader

                            anchors.fill: parent
                            active: holder.made
                            onItemChanged: {
                                if (loader.item)
                                    page.siteViews[holder.siteId] = loader.item
                                else
                                    delete page.siteViews[holder.siteId]
                            }

                            sourceComponent: WebEngineView {
                                profile: page.profile
                                url: holder.startUrl
                                // colour: the page in the screen's own colours, calmer when asked; drawn
                                // as it is when that changes nothing (ColourEffect.qml, docs/colour.md).
                                layer.enabled: visible && ScreenColours.revision >= 0 && ScreenColours.active(Screen.name)
                                layer.effect: ColourEffect {}
                                // Sharing the screen in a call: Qt WebEngine refuses every
                                // capture without it, after the choice even; the site's
                                // switch and the choice still decide (`shareScreen`).
                                settings.screenCaptureEnabled: true
                                Component.onCompleted: userScripts.collection = holder.fixes
                                // What the calls script says (`heard`).
                                onJavaScriptConsoleMessage: (level, message, lineNumber, sourceID) => page.heard(holder.modelData, message, null)
                                // Its page closed (`closePage`); a page's own window.close() changes nothing.
                                onWindowCloseRequested: holder.pageClosed()
                                // Called by Qt WebEngine when a page refused to close: no longer waited for.
                                function windowCloseRejected() {
                                    holder.pageClosed()
                                }
                                // While it closes, its "Leave the site?" is answered yes: you closed it.
                                onJavaScriptDialogRequested: request => {
                                    if (closeDeadline.running && request.type === JavaScriptDialogRequest.DialogTypeBeforeUnload) {
                                        request.accepted = true
                                        request.dialogAccept()
                                    }
                                }
                                // A security key asked for: its account, its PIN, why it failed.
                                onWebAuthUxRequested: request => webAuth.show(request)
                                audioMuted: holder.modelData.muted || holder.covered
                                // Notifications always (Sioul keeps them); the microphone, the camera
                                // and the screen as the site's switches say (⋮); never where you are,
                                // nor the fonts of this computer (they tell it apart) or the pointer.
                                // The clipboard: Qt asks the same for a page's "Copy" as for reading.
                                onPermissionRequested: permission => {
                                    const type = permission.permissionType
                                    const kinds = WebEnginePermission.PermissionType
                                    const site = holder.modelData
                                    const asked = type === kinds.MediaAudioCapture ? (site.microphone ? "" : "microphone")
                                        : type === kinds.MediaVideoCapture ? (site.camera ? "" : "camera")
                                        : type === kinds.MediaAudioVideoCapture ? (!site.microphone ? "microphone" : !site.camera ? "camera" : "")
                                        : (type === kinds.DesktopVideoCapture || type === kinds.DesktopAudioVideoCapture) ? (site.screen ? "" : "screen")
                                        : ""
                                    const allowed = [kinds.Notifications, kinds.ClipboardReadWrite, kinds.MediaAudioCapture, kinds.MediaVideoCapture,
                                                     kinds.MediaAudioVideoCapture, kinds.DesktopVideoCapture, kinds.DesktopAudioVideoCapture]
                                    if (allowed.indexOf(type) < 0) {
                                        permission.deny()
                                    } else if (asked !== "") {
                                        permission.deny()
                                        page.problem = page.sioul.textWith("site-permission-off-" + asked, "site", site.name)
                                    } else {
                                        permission.grant()
                                    }
                                }
                                // Sharing a screen in a call: you choose which, or none; only where it is allowed.
                                onDesktopMediaRequested: request => page.shareScreen(request, holder.modelData)
                                // A sign-in window, or a page meant for another tab: its own window,
                                // allowed what this site is, whichever site is in front.
                                onNewWindowRequested: request => page.openPopup(request, holder.modelData)
                            }
                        }
                    }
                }
            }
        }
    }

    // What a site is, and whether it stays open: the open site's (⋮), or any
    // site's of the list (a right click on it).
    SiteMenu {
        id: siteMenu

        sioul: page.sioul
        window: page.window
        site: page.menuSite
        index: page.menuIndex
        total: page.sites.length
        canChoose: page.menuId === page.openId
        onSaid: problem => page.problem = problem
        onChanged: page.reload()
        onDevicesAsked: devicesDialog.open()
        onChooseAsked: page.chooseLogin()
        onEditAsked: editing.begin(page.menuSite)
        onRemoveAsked: siteRemove.ask(page.menuSite ? page.menuSite.name : "", page.sioul.text("site-remove-ask"), page.sioul.text("site-remove"))
    }

    // The devices of calls: the system's list of cameras, microphones and
    // speakers, the same on every computer; "The system's own" when unsaid. A
    // device chosen but not plugged in now stays named, at the end of its list.
    // Known from a site's first call, or from the first look at the list: Qt
    // Multimedia, started for it, loads FFmpeg, makes a graphics context of its
    // own and reaches the sound server (some 60 MB in all, in the sandbox
    // without a graphics card where it was measured).
    property MediaDevices mediaDevices: null
    function knowDevices() {
        if (page.mediaDevices === null)
            page.mediaDevices = mediaDevicesKind.createObject(page)
    }
    Component {
        id: mediaDevicesKind

        MediaDevices {}
    }
    Component {
        id: deviceRows

        GridLayout {
            id: rowsGrid

            readonly property var lists: page.mediaDevices === null ? [] : [["camera", page.mediaDevices.videoInputs], ["microphone", page.mediaDevices.audioInputs], ["speaker", page.mediaDevices.audioOutputs]]

            columns: 2
            columnSpacing: 10
            rowSpacing: 8
            Component.onCompleted: page.knowDevices()

            Repeater {
                model: rowsGrid.lists

                delegate: RowLayout {
                    id: deviceRow

                    required property var modelData
                    readonly property string which: deviceRow.modelData[0]
                    readonly property string chosen: page.callDevices[deviceRow.which] || ""
                    readonly property var listed: deviceRow.modelData[1].map(d => d.description)
                    readonly property var names: [""].concat(deviceRow.listed, deviceRow.chosen !== "" && deviceRow.listed.indexOf(deviceRow.chosen) < 0 ? [deviceRow.chosen] : [])

                    Layout.columnSpan: 2
                    Layout.fillWidth: true
                    spacing: 10

                    Label {
                        Layout.preferredWidth: 130
                        text: page.sioul.text("site-device-" + deviceRow.which)
                        color: page.theme.muted
                    }
                    PlainComboBox {
                        Layout.fillWidth: true
                        model: deviceRow.names.map(n => n === "" ? page.sioul.text("site-device-system") : page.theme.plain(n))
                        currentIndex: Math.max(0, deviceRow.names.indexOf(deviceRow.chosen))
                        onActivated: index => page.setCallDevice(deviceRow.which, deviceRow.names[index])
                    }
                }
            }
        }
    }
    SafeDialog {
        id: devicesDialog

        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(520, Math.min(page.width, devicesDialog.roomWidth) - 2 * page.theme.gap)
        title: page.sioul.text("site-devices")

        // Sioul's own button, in your language: Qt's standard ones are not translated.
        footer: DialogButtonBox {
            Button {
                text: page.sioul.text("ui-close")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }

        contentItem: ColumnLayout {
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: page.sioul.text("site-devices-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            Loader {
                Layout.fillWidth: true
                active: devicesDialog.visible
                sourceComponent: deviceRows
            }
        }
    }
    // The call button's panel: the same lists, under the button, during a call.
    Popup {
        id: callPanel

        parent: callButton
        x: Math.min(0, callButton.width - callPanel.width)
        y: callButton.height + 4
        width: Math.min(480, page.width - 2 * page.theme.gap)
        padding: 14
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        background: Rectangle {
            color: page.theme.surface
            border.color: page.theme.line
            radius: page.theme.radius
        }
        contentItem: ColumnLayout {
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: page.sioul.text("site-call-devices")
                font.bold: true
                color: page.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: page.sioul.text("site-devices-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            Loader {
                Layout.fillWidth: true
                active: callPanel.visible
                sourceComponent: deviceRows
            }
        }
    }

    // Out of Sioul: its sign-in stays in the site's own data until it is cleared.
    ConfirmDialog {
        id: siteRemove

        sioul: page.sioul
        theme: page.theme
        onConfirmed: {
            page.problem = page.sioul.removeSite(page.menuId)
            if (page.menuId === page.openId)
                page.openId = ""
            page.reload()
        }
    }

    // A pop-up of a site, made for each request (see `openPopup`).
    Component {
        id: popupWindow

        SitePopup {
            id: popupItem

            theme: page.theme
            profile: page.profile
            webFixes: page.webFixesFor(popupItem.site)
            closeWait: page.closeWait
            onWebAuthAsked: request => webAuth.show(request)
            onPopupAsked: request => page.openPopup(request, popupItem.site)
            onScreenAsked: request => page.shareScreen(request, popupItem.site)
            onConsoleSaid: message => page.heard(popupItem.site, message, popupItem)
        }
    }


    WebAuthDialog {
        id: webAuth

        sioul: page.sioul
        theme: page.theme
    }

    // Sharing a screen in a call, from a site's page or its call window: you
    // choose which, or none; only where the site's switch allows it.
    function shareScreen(request, site) {
        if (!site || !site.screen) {
            request.cancel()
            page.problem = page.sioul.textWith("site-permission-off-screen", "site", site ? site.name : "")
            return
        }
        sharing.request = request
        sharing.chosen = false
        sharing.open()
    }

    // What a call shares, chosen: a screen (`screen`) or a window, by its place in the list.
    function shareChosen(screen, index) {
        const request = sharing.request
        const list = request ? (screen ? request.screensModel : request.windowsModel) : null
        if (!list || index < 0 || index >= list.rowCount())
            return
        sharing.chosen = true
        if (screen)
            request.selectScreen(request.screensModel.index(index, 0))
        else
            request.selectWindow(request.windowsModel.index(index, 0))
        sharing.close()
    }

    // A call asks to share the screen: a screen, a window, or nothing.
    SafeDialog {
        id: sharing

        property var request: null
        property bool chosen: false

        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(460, Math.min(page.width, sharing.roomWidth) - 2 * page.theme.gap)
        title: page.sioul.text("site-share")
        onClosed: {
            if (!sharing.chosen && sharing.request)
                sharing.request.cancel()
            sharing.request = null
        }

        contentItem: ColumnLayout {
            spacing: 6

            Label {
                text: page.sioul.text("site-share-screens")
                font.weight: Font.DemiBold
                color: page.theme.text
            }
            Repeater {
                model: sharing.request ? sharing.request.screensModel : null

                delegate: Button {
                    required property int index
                    // The model's own row: its name is its display role.
                    required property var model

                    Layout.fillWidth: true
                    text: page.theme.plain(model.display)
                    onClicked: page.shareChosen(true, index)
                }
            }
            Label {
                text: page.sioul.text("site-share-windows")
                font.weight: Font.DemiBold
                color: page.theme.text
            }
            Repeater {
                model: sharing.request ? sharing.request.windowsModel : null

                delegate: Button {
                    required property int index
                    // The model's own row: its name is its display role.
                    required property var model

                    Layout.fillWidth: true
                    // Other programs' window titles, a web page's among them: plain text.
                    text: page.theme.plain(model.display)
                    onClicked: page.shareChosen(false, index)
                }
            }
        }
        footer: DialogButtonBox {
            Button {
                text: page.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }
    }

    // Bitwarden, opened once for the session (VaultUnlock.qml), the security
    // key asked from Bitwarden's own page, held unseen in the dialog: the
    // window's (SitesWeb.qml), as from Accounts.
    VaultUnlock {
        id: unlock

        sioul: page.sioul
        theme: page.theme
        keys: page.window.vaultKeys
        onUnlocked: {
            if (page.afterUnlock === "choose")
                page.chooseLogin()
            else
                page.fillLogin()
        }
    }

    // Which login to fill: the site's own when it has several or none, or any
    // of the vault's when you ask (⋮ ▸ Choose a login). Made the first time.
    Later {
        id: chooser

        sourceComponent: Component {
            LoginChooser {
                sioul: page.sioul
                theme: page.theme
                onChosen: item => page.fillChosen(item)
            }
        }
    }

    // A site's name, address, type and categories, changed.
    SiteEditDialog {
        id: editing

        sioul: page.sioul
        theme: page.theme
        room: page.width
        onSaved: page.reload()
    }

    // A site added: from the sites people usually keep, or by hand; it opens once pinned.
    SiteAddDialog {
        id: adding

        sioul: page.sioul
        theme: page.theme
        room: page.width
        roomTall: page.height
        onAdded: id => {
            page.reload()
            page.open(id)
        }
    }
}
