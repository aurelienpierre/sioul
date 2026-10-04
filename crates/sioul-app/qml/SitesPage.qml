// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sites kept in Sioul: secure mailboxes, chats, a dating site. Pinned on the
// left, one open on the right, logged in once: the profile keeps cookies and
// the files sites download to show themselves. Their notifications are always
// accepted, then wait for the Porch, unless the site is in real time; calls
// and video work (microphone, camera, sharing the screen). A site can be
// silenced; a chat's real time is one tick away, always in view. Logins come
// from Bitwarden when you ask.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Effects
import QtQuick.Layouts
import QtMultimedia
import QtWebEngine

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var sites: []
    property string openId: ""
    // The column of sites folded to their icons, as you left it.
    property bool narrow: page.sioul.viewFlag("sites-narrow")
    // For the window's images: Bitwarden's dialog, the page it asks a key from, the chooser.
    property alias bitwardenUnlock: unlock
    property alias bitwardenKeyView: keyView
    property alias loginChooser: chooser
    property alias addSiteDialog: adding
    property alias addSiteFind: find.text
    // What follows Bitwarden's unlocking: the login filled ("fill") or chosen ("choose").
    property string afterUnlock: "fill"
    // Sites opened this session stay loaded: going back to one does not reload it.
    property var visited: ({})
    property string problem: ""
    // Each site's view, once made, by its id.
    property var siteViews: ({})
    // In every page, before its own scripts: Qt 6.10–6.11 never answers
    // PublicKeyCredential.getClientCapabilities(), and sites asking for a
    // security key wait for it forever (QTBUG-149575); without it they ask
    // the key directly. Injection point 2: DocumentCreation; world 0: the page's.
    readonly property var webFixes: [{
        name: "sioul-webauthn",
        sourceCode: "if (window.PublicKeyCredential && PublicKeyCredential.getClientCapabilities) { delete PublicKeyCredential.getClientCapabilities; }",
        injectionPoint: 2,
        worldId: 0,
        runsOnSubFrames: true
    }, {
        name: "sioul-devices",
        sourceCode: page.devicesScript + "(" + JSON.stringify(page.callDevices) + ");",
        injectionPoint: 2,
        worldId: 0,
        runsOnSubFrames: true
    }]
    // Calls take the camera, microphone and speaker you chose, by their names
    // (the browser's own ids are a site's): asked for when the page does not
    // choose one itself, the speaker given to every sound it plays. The names
    // stay inside the script, out of the page's reach: a page learns them only
    // as its browser tells it, once allowed a microphone or a camera. Changed,
    // they come to the pages open as an event ("sioul-devices").
    readonly property string devicesScript: `(chosen => {
        if (!navigator.mediaDevices || navigator.mediaDevices.__sioul) return;
        navigator.mediaDevices.__sioul = true;
        let names = chosen || {};
        window.addEventListener("sioul-devices", e => { names = (e && e.detail) || {}; });
        const wanted = () => names;
        const fold = s => String(s || "").toLowerCase();
        const find = async (kind, name) => {
            if (!name) return null;
            try {
                const all = (await navigator.mediaDevices.enumerateDevices()).filter(d => d.kind === kind && d.label);
                const want = fold(name);
                const hit = all.find(d => fold(d.label) === want) || all.find(d => fold(d.label).startsWith(want)) || all.find(d => want.startsWith(fold(d.label))) || all.find(d => fold(d.label).includes(want));
                return hit ? hit.deviceId : null;
            } catch (e) { return null; }
        };
        const original = navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);
        navigator.mediaDevices.getUserMedia = async constraints => {
            const asked = constraints ? Object.assign({}, constraints) : {};
            const pick = async (key, kind, name) => {
                if (!asked[key] || !name) return;
                const id = await find(kind, name);
                if (!id) return;
                const given = asked[key] === true ? {} : Object.assign({}, asked[key]);
                if (!given.deviceId) given.deviceId = { ideal: id };
                asked[key] = given;
            };
            await pick("video", "videoinput", wanted().camera);
            await pick("audio", "audioinput", wanted().microphone);
            return original(asked);
        };
        const sink = async element => {
            const name = wanted().speaker;
            if (!name || !element.setSinkId) return;
            const id = await find("audiooutput", name);
            if (id && element.sinkId !== id) { try { await element.setSinkId(id); } catch (e) {} }
        };
        const play = HTMLMediaElement.prototype.play;
        HTMLMediaElement.prototype.play = function () { sink(this); return play.apply(this, arguments); };
        if (window.AudioContext && AudioContext.prototype.setSinkId) {
            const Original = window.AudioContext;
            const Made = function (...args) {
                const context = new Original(...args);
                find("audiooutput", wanted().speaker).then(id => { if (id) context.setSinkId(id).catch(() => {}); });
                return context;
            };
            Made.prototype = Original.prototype;
            window.AudioContext = Made;
        }
    })`
    // Chats covered: their daily time used (Health page), until it comes back.
    property bool chatsCovered: page.sioul.chatsCovered()

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
    // What a site is, as it behaves (sioul_core::sites::TYPES); your categories come on top.
    readonly property var kinds: ["mailbox", "chat", "video", "social", "dating", "other"]
    readonly property var kindIcons: ({ "mailbox": "mail-folder-inbox", "chat": "im-user", "video": "camera-video", "social": "system-users", "dating": "emblem-favorite" })
    // The list filtered, each way at once: what a site is for, its type, one of your categories; "" for any.
    property string areaFilter: ""
    property string kindFilter: ""
    property string categoryFilter: ""
    // The categories your sites have, by name.
    readonly property var categories: {
        const words = []
        page.sites.forEach(s => (s.categories || []).forEach(c => {
            if (words.indexOf(c) < 0)
                words.push(c)
        }))
        return words.sort((a, b) => a.localeCompare(b))
    }
    readonly property string filterWords: [page.areaFilter !== "" ? page.sioul.text("area-" + page.areaFilter) : "", page.kindFilter !== "" ? page.sioul.text("site-kind-" + page.kindFilter) : "", page.categoryFilter].filter(t => t !== "").join(" · ")
    // The usual sites, as a menu: countries, their groups, the sites.
    property var presetTree: []

    // A sign-in window, a call window: each in a window of its own, on the
    // sites' profile, so it stays tied to the page that opened it (a sign-in
    // answers it there). It may use what the site that opened it may, kept
    // as it was then: not the switches of the site in front at the time.
    function openPopup(request, site) {
        const window = popupWindow.createObject(page, { site: site || null }) as SitePopup
        request.openIn(window.view)
        window.show()
        window.raise()
    }

    property alias presetsMenu: presetsMenu
    property alias siteMenu: siteMenu
    property alias devicesDialog: devicesDialog
    property alias filterMenu: filterMenu
    function showPresets() {
        page.presetTree = JSON.parse(page.sioul.sitePresetsTree()).countries
        presetsMenu.popup(presetsButton, 0, -presetsMenu.implicitHeight)
    }
    function showFilters() {
        filterMenu.popup(filterButton, 0, filterButton.height)
    }

    // Usual sites pinned: one, or a whole group; those already here stay as they are.
    function pinAll(rows) {
        const answer = JSON.parse(page.sioul.addSites(JSON.stringify(rows)))
        page.problem = answer.error || ""
        if (answer.added > 0)
            page.sioul.status = page.sioul.textWith("site-pinned", "count", String(answer.added))
        page.reload()
    }
    // The list by category, or in your own order, as you left it.
    property bool byCategory: page.sioul.viewFlag("sites-by-category")
    // The sites for other hours (docs/areas.md), shown when asked.
    property bool laterShown: false
    // The column's rows: the sites for these hours, then one line folding the
    // others; each category named once when sorted by it.
    readonly property var rows: {
        const wanted = s => (page.areaFilter === "" || String(s.area).split("+").indexOf(page.areaFilter) >= 0) && (page.kindFilter === "" || s.kind === page.kindFilter) && (page.categoryFilter === "" || (s.categories || []).indexOf(page.categoryFilter) >= 0)
        const shown = page.sites.filter(wanted)
        const sorted = page.byCategory ? shown.slice().sort((a, b) => page.kinds.indexOf(a.kind) - page.kinds.indexOf(b.kind)) : shown
        const out = []
        const add = (list, later) => list.forEach((s, i) => out.push({ site: s, later: later, header: page.byCategory && (i === 0 || list[i - 1].kind !== s.kind) ? s.kind : "" }))
        add(sorted.filter(s => s.in_view !== false), false)
        const later = sorted.filter(s => s.in_view === false)
        if (later.length > 0) {
            out.push({ fold: true, count: later.length, header: "" })
            if (page.laterShown)
                add(later, true)
        }
        return out
    }
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
                viewIds.remove(i)
        }
        const have = []
        for (let i = 0; i < viewIds.count; i++)
            have.push(viewIds.get(i).siteId)
        for (const id of ids) {
            if (have.indexOf(id) < 0)
                viewIds.append({ siteId: id })
        }
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

    // The camera, microphone and speaker calls use, by their names: given to every
    // page (`devicesScript`), and to those open now.
    property var callDevices: JSON.parse(page.sioul.callDevices() || "{}")
    function setCallDevice(which, name) {
        page.problem = page.sioul.setCallDevice(which, name)
        page.callDevices = JSON.parse(page.sioul.callDevices() || "{}")
        const said = "window.dispatchEvent(new CustomEvent('sioul-devices', { detail: " + JSON.stringify(page.callDevices) + " }));"
        for (const id in page.siteViews) {
            const view = page.siteViews[id] as WebEngineView
            if (view)
                view.runJavaScript(said)
        }
    }

    function open(id) {
        page.openId = id
        page.problem = ""
        const next = Object.assign({}, page.visited)
        next[id] = true
        page.visited = next
        page.sioul.siteSeen(id)
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

    function alive(site) {
        return site.background || page.visited[site.id] === true || site.id === page.openId
    }

    function viewOf(id) {
        return page.siteViews[id] || null
    }

    // The login Bitwarden keeps for the open site, written into its form: the
    // only one at once; several, or none, chosen in a list (the vault searched).
    function fillLogin() {
        const view = page.viewOf(page.openId)
        if (view === null || !page.vaultOpen("fill"))
            return
        const logins = JSON.parse(page.sioul.bitwardenLogins(view.url.toString(), ""))
        if (logins.error) {
            page.problem = logins.error
            return
        }
        page.problem = ""
        if (logins.matches.length === 1)
            page.fillChosen(logins.matches[0].id)
        else
            chooser.begin(view.url.toString())
    }

    // Any login of the vault, chosen in the list.
    function chooseLogin() {
        const view = page.viewOf(page.openId)
        if (view === null || !page.vaultOpen("choose"))
            return
        page.problem = ""
        chooser.begin(view.url.toString())
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

    // One profile for every site: logins kept, files cached on disk.
    WebEngineProfilePrototype {
        id: prototype

        storageName: "sioul-sites"
        httpCacheType: WebEngineProfile.DiskHttpCache
        httpCacheMaximumSize: 512 * 1024 * 1024
        persistentCookiesPolicy: WebEngineProfile.ForcePersistentCookies
        // What a site was allowed is kept: notifications for good (`keepPermissions`),
        // a microphone, a camera, a screen while its switch is on.
        persistentPermissionsPolicy: WebEngineProfile.PersistentPermissionsPolicy.StoreOnDisk

        // The profile is made once the prototype is complete; no view exists before it.
        Component.onCompleted: {
            const made = prototype.instance()
            // Without Qt's name in it: some chats refuse browsers they do not know.
            made.httpUserAgent = made.httpUserAgent.replace(/ QtWebEngine\/[\d.]+/, "")
            // Nor in the brands it tells sites: Google refuses sign-in to browsers it takes for embedded.
            const brands = made.clientHints.fullVersionList
            const kept = {}
            for (const name in brands) {
                if (!/qt/i.test(name))
                    kept[name] = brands[name]
            }
            made.clientHints.fullVersionList = kept
            page.profile = made
            page.keepPermissions()
        }
    }
    property WebEngineProfile profile: null

    Component.onCompleted: page.reload()

    Connections {
        target: page.profile

        // Accepted always; the site in front of you needs no word, the others
        // show at once in real time, else wait for the Porch.
        function onPresentNotification(notification) {
            const site = page.siteOf(notification.origin)
            const looking = site !== null && page.visible && site.id === page.openId
            if (site && !looking)
                page.sioul.siteNotified(site.id, notification.title, notification.message)
            notification.show()
        }

        function onDownloadRequested(download) {
            download.accept()
            page.sioul.status = page.sioul.textWith("site-downloading", "file", download.downloadFileName)
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // The sites, pinned: a narrow column, or their icons only, as you left it.
        ColumnLayout {
            Layout.fillHeight: true
            Layout.preferredWidth: page.narrow ? 40 : 170
            Layout.maximumWidth: page.narrow ? 40 : 170
            spacing: 6

            RowLayout {
                Layout.fillWidth: true
                spacing: 0

                Label {
                    visible: !page.narrow
                    Layout.fillWidth: true
                    text: page.sioul.text("ui-sites")
                    font.pixelSize: 20
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                // Filtered by what a site is for, its type, one of your categories.
                ToolButton {
                    id: filterButton

                    visible: !page.narrow
                    icon.name: "view-filter"
                    icon.color: page.theme.text
                    display: AbstractButton.IconOnly
                    highlighted: page.filterWords !== ""
                    Accessible.name: page.sioul.text("site-filter")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("site-filter")
                    ToolTip.delay: 600
                    onClicked: page.showFilters()
                }
                ToolButton {
                    visible: !page.narrow
                    checkable: true
                    checked: page.byCategory
                    icon.name: "view-sort"
                    icon.color: page.theme.text
                    display: AbstractButton.IconOnly
                    Accessible.name: page.sioul.text(page.byCategory ? "site-sort-category" : "site-sort-own")
                    ToolTip.visible: hovered
                    ToolTip.text: Accessible.name
                    ToolTip.delay: 600
                    onToggled: {
                        page.byCategory = checked
                        page.sioul.setViewFlag("sites-by-category", checked)
                    }
                }
                ToolButton {
                    text: page.narrow ? "»" : "«"
                    Accessible.name: page.narrow ? page.sioul.text("site-column-widen") : page.sioul.text("site-column-narrow")
                    ToolTip.visible: hovered
                    ToolTip.text: Accessible.name
                    ToolTip.delay: 600
                    onClicked: {
                        page.narrow = !page.narrow
                        page.sioul.setViewFlag("sites-narrow", page.narrow)
                    }
                }
            }
            // The filters at work, in words, and a way out of them.
            RowLayout {
                visible: !page.narrow && page.filterWords !== ""
                Layout.fillWidth: true
                spacing: 2

                Label {
                    Layout.fillWidth: true
                    text: page.filterWords
                    font.pixelSize: 12
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                ToolButton {
                    icon.name: "edit-clear"
                    icon.color: page.theme.text
                    display: AbstractButton.IconOnly
                    implicitWidth: 26
                    implicitHeight: 26
                    Accessible.name: page.sioul.text("site-filter-clear")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("site-filter-clear")
                    onClicked: {
                        page.areaFilter = ""
                        page.kindFilter = ""
                        page.categoryFilter = ""
                    }
                }
            }
            Label {
                visible: page.sites.length === 0 && !page.narrow
                Layout.fillWidth: true
                text: page.sioul.text("site-none")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            // Many sites scroll; sorted by category, each category says its name once.
            ScrollView {
                Layout.fillWidth: true
                Layout.fillHeight: true
                contentWidth: availableWidth
                clip: true

                ColumnLayout {
                    width: parent.width
                    spacing: 6

                    Repeater {
                        model: page.rows

                        delegate: ColumnLayout {
                            id: entry

                            required property var modelData
                            required property int index

                            Layout.fillWidth: true
                            spacing: 2

                            Label {
                                visible: entry.modelData.header !== "" && !page.narrow
                                Layout.topMargin: entry.index === 0 ? 0 : 6
                                text: entry.modelData.header !== "" ? page.sioul.text("site-kind-" + entry.modelData.header) : ""
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                            // The sites for other hours: one line, opened on demand.
                            Button {
                                visible: entry.modelData.fold === true
                                Layout.fillWidth: true
                                Layout.topMargin: 6
                                flat: true
                                text: page.narrow ? "⋯" : page.sioul.textWith("site-later", "count", String(entry.modelData.count || 0)) + (page.laterShown ? "  ▾" : "  ▸")
                                font.pixelSize: 13
                                onClicked: page.laterShown = !page.laterShown
                            }

                            ItemDelegate {
                                id: row

                                readonly property var modelData: entry.modelData.site || ({})

                                visible: entry.modelData.fold !== true
                                opacity: entry.modelData.later ? 0.6 : 1
                                Layout.fillWidth: true
                                padding: page.narrow ? 4 : 6
                                highlighted: page.openId === row.modelData.id
                                onClicked: page.open(row.modelData.id)

                                // Its menu, where it is: what it is, what it is for, its name, its place, out of Sioul.
                                TapHandler {
                                    acceptedButtons: Qt.RightButton
                                    onTapped: {
                                        page.menuId = row.modelData.id
                                        siteMenu.popup()
                                    }
                                }
                                ToolTip.visible: page.narrow && hovered
                                ToolTip.text: row.modelData.name || ""
                                ToolTip.delay: 400
                                Accessible.name: row.modelData.name || ""

                                background: Rectangle {
                                    color: row.highlighted || row.hovered ? page.theme.surface : "transparent"
                                    radius: page.theme.radius
                                    border.color: row.visualFocus ? page.theme.focus : row.highlighted ? page.theme.line : "transparent"
                                }

                                contentItem: RowLayout {
                                    spacing: 6

                                    Item {
                                        Layout.preferredWidth: 18
                                        Layout.preferredHeight: 18

                                        // Its own icon, kept on this computer; its type's until then, and when it cannot be drawn.
                                        Image {
                                            id: favicon

                                            visible: favicon.status === Image.Ready
                                            anchors.centerIn: parent
                                            width: 16
                                            height: 16
                                            source: row.modelData.icon || ""
                                            sourceSize: Qt.size(32, 32)
                                            fillMode: Image.PreserveAspectFit
                                            smooth: true
                                            asynchronous: true
                                        }
                                        Icon {
                                            visible: favicon.status !== Image.Ready
                                            anchors.centerIn: parent
                                            iconName: page.kindIcons[row.modelData.kind] || "internet-web-browser"
                                            size: 16
                                        }
                                        // Something notified, not seen yet: a dot, never a number.
                                        Rectangle {
                                            visible: row.modelData.news === true && page.narrow
                                            anchors.right: parent.right
                                            anchors.top: parent.top
                                            width: 7
                                            height: 7
                                            radius: 3.5
                                            color: page.theme.accent
                                        }
                                    }
                                    Label {
                                        visible: !page.narrow
                                        Layout.fillWidth: true
                                        text: row.modelData.name || ""
                                        elide: Text.ElideRight
                                        color: page.theme.text
                                    }
                                    // Quiet, not red as Breeze draws it.
                                    Icon {
                                        visible: row.modelData.muted === true && !page.narrow
                                        iconName: "audio-volume-muted"
                                        color: page.theme.muted
                                        size: 14
                                    }
                                    Rectangle {
                                        visible: row.modelData.news === true && !page.narrow
                                        Layout.preferredWidth: 7
                                        Layout.preferredHeight: 7
                                        radius: 3.5
                                        color: page.theme.accent
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Adding a site, and Bitwarden's account: under the list, small.
            Flow {
                Layout.fillWidth: true
                spacing: 2

                ToolButton {
                    icon.name: "list-add"
                    icon.color: page.theme.text
                    text: page.narrow ? "" : page.sioul.text("site-add")
                    display: page.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                    Accessible.name: page.sioul.text("site-add")
                    ToolTip.visible: page.narrow && hovered
                    ToolTip.text: page.sioul.text("site-add")
                    onClicked: adding.open()
                }
                // The usual sites, by country, group and site: a whole group pinned at once, or one.
                ToolButton {
                    id: presetsButton

                    icon.name: "bookmarks-organize"
                    icon.color: page.theme.text
                    text: page.narrow ? "" : page.sioul.text("site-presets") + "  ▾"
                    display: page.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                    Accessible.name: page.sioul.text("site-presets")
                    ToolTip.visible: page.narrow && hovered
                    ToolTip.text: page.sioul.text("site-presets")
                    onClicked: page.showPresets()
                }
                SettingsButton {
                    sioul: page.sioul
                    theme: page.theme
                    view: "sites"
                }
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
                        if (view)
                            view.reload()
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: page.opened ? page.opened.name : ""
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
                        visible: page.openId === holder.siteId
                        // Covered: blurred, and nothing under it takes a click.
                        layer.enabled: holder.covered
                        layer.effect: MultiEffect {
                            blurEnabled: true
                            blur: 1.0
                            blurMax: 64
                        }
                        Component.onDestruction: delete page.siteViews[holder.siteId]

                        Loader {
                            id: loader

                            anchors.fill: parent
                            active: page.profile !== null && page.alive(holder.modelData)
                            onItemChanged: {
                                if (loader.item)
                                    page.siteViews[holder.siteId] = loader.item
                                else
                                    delete page.siteViews[holder.siteId]
                            }

                            sourceComponent: WebEngineView {
                                profile: page.profile
                                url: holder.startUrl
                                Component.onCompleted: userScripts.collection = page.webFixes
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
    SioulMenu {
        id: siteMenu

        Repeater {
            model: page.kinds

            delegate: MenuItem {
                required property string modelData

                text: page.sioul.text("site-kind-" + modelData)
                checkable: true
                checked: page.menuSite !== null && page.menuSite.kind === modelData
                onTriggered: page.problem = page.sioul.setSite(page.menuId, "site", modelData)
            }
        }
        MenuSeparator {}
        MenuItem {
            text: page.sioul.text("site-background")
            checkable: true
            checked: page.menuSite !== null && page.menuSite.background
            onTriggered: page.problem = page.sioul.setSite(page.menuId, "background", String(checked))
        }
        // What it is for: the hours it comes forward in, any of them together (docs/areas.md).
        // None ticked: as its kind goes.
        SioulMenu {
            title: page.sioul.text("site-area")

            Repeater {
                model: ["work", "admin", "leisure"]

                delegate: MenuItem {
                    required property string modelData

                    readonly property var on: page.menuSite !== null ? String(page.menuSite.area).split("+") : []

                    text: page.sioul.text("area-" + modelData)
                    checkable: true
                    checked: on.indexOf(modelData) >= 0
                    onTriggered: {
                        const was = on
                        const now = ["work", "admin", "leisure"].filter(a => a === modelData ? checked : was.indexOf(a) >= 0)
                        page.problem = page.sioul.setSite(page.menuId, "area", now.join("+"))
                        page.reload()
                    }
                }
            }
        }
        MenuSeparator {}
        // For calls: what the site may use, each asked again at every call.
        MenuItem {
            text: page.sioul.text("site-microphone")
            checkable: true
            checked: page.menuSite !== null && page.menuSite.microphone
            onTriggered: {
                page.problem = page.sioul.setSite(page.menuId, "microphone", String(checked))
                page.reload()
            }
        }
        MenuItem {
            text: page.sioul.text("site-camera")
            checkable: true
            checked: page.menuSite !== null && page.menuSite.camera
            onTriggered: {
                page.problem = page.sioul.setSite(page.menuId, "camera", String(checked))
                page.reload()
            }
        }
        MenuItem {
            text: page.sioul.text("site-screen")
            checkable: true
            checked: page.menuSite !== null && page.menuSite.screen
            onTriggered: {
                page.problem = page.sioul.setSite(page.menuId, "screen", String(checked))
                page.reload()
            }
        }
        // Which camera, microphone and speaker calls use: the same for every site.
        MenuItem {
            text: page.sioul.text("site-devices") + "…"
            onTriggered: devicesDialog.open()
        }
        MenuSeparator {}
        // Your order: the list keeps it (by category, within each).
        MenuItem {
            text: page.sioul.text("site-move-up")
            enabled: page.menuIndex > 0
            onTriggered: {
                page.problem = page.sioul.moveSite(page.menuId, -1)
                page.reload()
            }
        }
        MenuItem {
            text: page.sioul.text("site-move-down")
            enabled: page.menuIndex >= 0 && page.menuIndex < page.sites.length - 1
            onTriggered: {
                page.problem = page.sioul.moveSite(page.menuId, 1)
                page.reload()
            }
        }
        // Tied to a task, a note, a contact: a site is a correspondent too.
        MenuItem {
            text: page.sioul.text("site-link")
            onTriggered: page.window.linkFrom({ uri: "sioul:site/" + encodeURIComponent(page.menuId), kind: "site", key: page.menuId, title: page.menuSite ? page.menuSite.name : "" })
        }
        MenuSeparator {}
        // Another login than the site's own, or one of several: the vault searched (the site open).
        MenuItem {
            visible: page.menuId === page.openId
            height: visible ? implicitHeight : 0
            text: page.sioul.text("bitwarden-choose")
            onTriggered: page.chooseLogin()
        }
        MenuItem {
            text: page.sioul.text("site-edit")
            onTriggered: editing.begin()
        }
        MenuItem {
            text: page.sioul.text("site-remove")
            onTriggered: siteRemove.ask(page.menuSite ? page.menuSite.name : "", page.sioul.text("site-remove-ask"), page.sioul.text("site-remove"))
        }
    }

    // The devices of calls: the system's list of cameras, microphones and
    // speakers, the same on every computer; "The system's own" when unsaid.
    MediaDevices {
        id: mediaDevices
    }
    Dialog {
        id: devicesDialog

        readonly property var lists: [["camera", mediaDevices.videoInputs], ["microphone", mediaDevices.audioInputs], ["speaker", mediaDevices.audioOutputs]]

        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(520, page.width - 2 * page.theme.gap)
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
            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 10
                rowSpacing: 8

                Repeater {
                    model: devicesDialog.lists

                    delegate: RowLayout {
                        id: deviceRow

                        required property var modelData
                        readonly property string which: deviceRow.modelData[0]
                        readonly property var names: [""].concat(deviceRow.modelData[1].map(d => d.description))

                        Layout.columnSpan: 2
                        Layout.fillWidth: true
                        spacing: 10

                        Label {
                            Layout.preferredWidth: 130
                            text: page.sioul.text("site-device-" + deviceRow.which)
                            color: page.theme.muted
                        }
                        ComboBox {
                            Layout.fillWidth: true
                            model: deviceRow.names.map(n => n === "" ? page.sioul.text("site-device-system") : n)
                            currentIndex: Math.max(0, deviceRow.names.indexOf(page.callDevices[deviceRow.which] || ""))
                            onActivated: index => page.setCallDevice(deviceRow.which, deviceRow.names[index])
                        }
                    }
                }
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
            webFixes: page.webFixes
            onWebAuthAsked: request => webAuth.show(request)
            onPopupAsked: request => page.openPopup(request, popupItem.site)
            onScreenAsked: request => page.shareScreen(request, popupItem.site)
        }
    }

    // What the list shows: what a site is for, its type, one of your categories.
    // One list of lines: a Repeater's lines land out of order in a menu once
    // their model changes.
    readonly property var filterLines: {
        const out = [{ head: page.sioul.text("site-filter-for") }, { what: "area", value: "", text: page.sioul.text("site-filter-any-area") }]
        for (const area of ["work", "admin", "leisure"])
            out.push({ what: "area", value: area, text: page.sioul.text("area-" + area) })
        out.push({ head: page.sioul.text("site-filter-type") }, { what: "kind", value: "", text: page.sioul.text("site-filter-any-kind") })
        for (const kind of page.kinds.filter(k => page.sites.some(s => s.kind === k)))
            out.push({ what: "kind", value: kind, text: page.sioul.text("site-kind-" + kind) })
        if (page.categories.length > 0) {
            out.push({ head: page.sioul.text("site-filter-category") }, { what: "category", value: "", text: page.sioul.text("site-filter-any-category") })
            for (const category of page.categories)
                out.push({ what: "category", value: category, text: category })
        }
        return out
    }
    function filterOf(what) {
        return what === "area" ? page.areaFilter : what === "kind" ? page.kindFilter : page.categoryFilter
    }
    function setFilter(what, value) {
        if (what === "area")
            page.areaFilter = value
        else if (what === "kind")
            page.kindFilter = value
        else
            page.categoryFilter = value
    }

    SioulMenu {
        id: filterMenu

        Instantiator {
            model: page.filterLines

            delegate: MenuItem {
                id: filterLine

                required property var modelData
                readonly property bool head: filterLine.modelData.head !== undefined

                // Your categories may hold "&", a shortcut's mark in a menu: "&&" is one.
                text: (filterLine.head ? filterLine.modelData.head : filterLine.modelData.text).replace(/&/g, "&&")
                font.pixelSize: filterLine.head ? 12 : 15
                enabled: !filterLine.head
                checkable: !filterLine.head
                checked: !filterLine.head && page.filterOf(filterLine.modelData.what) === filterLine.modelData.value
                onTriggered: {
                    // Chosen again: no longer filtered by it.
                    const was = page.filterOf(filterLine.modelData.what)
                    page.setFilter(filterLine.modelData.what, was === filterLine.modelData.value ? "" : filterLine.modelData.value)
                    filterLine.checked = Qt.binding(() => !filterLine.head && page.filterOf(filterLine.modelData.what) === filterLine.modelData.value)
                }
            }
            onObjectAdded: (index, object) => filterMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => filterMenu.removeItem(object)
        }
    }

    // The usual sites: country, group, site; a country's states or provinces
    // in a menu of their own, each with its groups. Each place and group
    // opens with "All of them": a whole group pinned at once.
    SioulMenu {
        id: presetsMenu

        Instantiator {
            model: page.presetTree

            delegate: PresetPlaceMenu {
                id: countryMenu

                required property var modelData

                sioul: page.sioul
                place: countryMenu.modelData
                onPin: rows => page.pinAll(rows)

                // Its states or provinces, last.
                property Instantiator regions: Instantiator {
                    model: countryMenu.modelData.regions.length > 0 ? [countryMenu.modelData] : []

                    delegate: SioulMenu {
                        id: regionsMenu

                        required property var modelData

                        title: regionsMenu.modelData.regions_name

                        Instantiator {
                            model: regionsMenu.modelData.regions

                            delegate: PresetPlaceMenu {
                                id: regionMenu

                                required property var modelData

                                sioul: page.sioul
                                place: regionMenu.modelData
                                onPin: rows => page.pinAll(rows)
                            }
                            onObjectAdded: (index, object) => regionsMenu.insertMenu(index, object)
                            onObjectRemoved: (index, object) => regionsMenu.removeMenu(object)
                        }
                    }
                    onObjectAdded: (index, object) => countryMenu.addMenu(object)
                    onObjectRemoved: (index, object) => countryMenu.removeMenu(object)
                }
            }
            onObjectAdded: (index, object) => presetsMenu.insertMenu(index, object)
            onObjectRemoved: (index, object) => presetsMenu.removeMenu(object)
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

    // A call asks to share the screen: a screen, a window, or nothing.
    Dialog {
        id: sharing

        property var request: null
        property bool chosen: false

        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(460, page.width - 2 * page.theme.gap)
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
                    onClicked: {
                        sharing.chosen = true
                        sharing.request.selectScreen(sharing.request.screensModel.index(index, 0))
                        sharing.close()
                    }
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
                    onClicked: {
                        sharing.chosen = true
                        sharing.request.selectWindow(sharing.request.windowsModel.index(index, 0))
                        sharing.close()
                    }
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

    // Bitwarden, opened once for the session, here: by your security key
    // alone, or by the master password (dropped once used), with a second step
    // or a new device's code when asked. A security key is asked from
    // Bitwarden's own page, held unseen in this dialog: its PIN comes in
    // Sioul's own dialog, above this one.
    Dialog {
        id: unlock

        // -1: the password only; else the step asked (0 app, 1 e-mail, 3 YubiKey OTP,
        // 7 security key, 8 recovery, 100 new device).
        property int provider: -1
        property var offered: []
        // The security key's step, when the account has one: {page, script}.
        property var key: null
        // The key being asked: {mode: "factor" or "passkey", page, script}; else null.
        property var keyAsk: null
        // The e-mail code was sent for this try.
        property bool sent: false
        // The security key alone opened the vault last time: proposed first.
        property bool passkeyFirst: false
        property string problem: ""
        property string note: ""
        // The steps Sioul can take, in the order Bitwarden's apps propose them:
        // a security key, a YubiKey's code, an app, e-mail; a recovery code only when chosen.
        readonly property var order: [7, 3, 0, 1, 8]

        function begin() {
            unlock.provider = -1
            unlock.offered = []
            unlock.key = null
            unlock.keyAsk = null
            unlock.sent = false
            unlock.problem = ""
            unlock.note = ""
            secret.text = ""
            code.text = ""
            unlock.passkeyFirst = page.sioul.viewFlag("bitwarden-passkey")
            unlock.open()
            if (unlock.passkeyFirst)
                passkeyButton.forceActiveFocus()
            else
                secret.forceActiveFocus()
        }

        // The security key alone: no master password.
        function usePasskey() {
            unlock.problem = ""
            unlock.note = ""
            const begun = JSON.parse(page.sioul.bitwardenPasskeyBegin())
            if (begun.error)
                unlock.problem = begun.error
            else
                unlock.askKey("passkey", begun)
        }

        // The key asked: Bitwarden's page loaded unseen below, the key asked there
        // at once. A new page each time: each ask is a fresh one.
        function askKey(mode, ask) {
            unlock.problem = ""
            unlock.note = ""
            unlock.keyAsk = null
            Qt.callLater(() => unlock.keyAsk = { mode: mode, page: ask.page, script: ask.script })
        }

        // What the key answered: the vault opened with it, or why not.
        function keyAnswered(mode, result) {
            unlock.keyAsk = null
            const said = JSON.parse(result)
            if (said.error !== undefined)
                unlock.problem = said.error === "not-allowed" ? page.sioul.text("bitwarden-key-not-allowed") : page.sioul.textWith("bitwarden-key-failed", "error", said.error)
            else if (mode === "passkey")
                unlock.answer(JSON.parse(page.sioul.bitwardenPasskey(result)))
            else
                unlock.answer(JSON.parse(page.sioul.bitwardenUnlock(secret.text, 7, said.token)))
        }

        // A step chosen: e-mail sends its code now, once; the security key is asked at once.
        function choose(provider) {
            unlock.provider = provider
            unlock.problem = ""
            unlock.note = ""
            unlock.keyAsk = null
            if (provider === 1 && !unlock.sent)
                unlock.sendCode()
            if (provider === 7)
                unlock.askKey("factor", unlock.key)
            else
                code.forceActiveFocus()
        }

        function sendCode() {
            const problem = page.sioul.bitwardenSendCode(secret.text)
            unlock.sent = problem === ""
            unlock.note = problem === "" ? page.sioul.text("bitwarden-code-sent") : ""
            unlock.problem = problem
        }

        function tryIt() {
            unlock.keyAsk = null
            unlock.answer(JSON.parse(page.sioul.bitwardenUnlock(secret.text, unlock.provider, code.text)))
        }

        // What Bitwarden answered: open, a second step to take, a new device's code, or why not.
        function answer(answer) {
            code.text = ""
            if (answer.ok) {
                secret.text = ""
                unlock.close()
                if (page.afterUnlock === "choose")
                    page.chooseLogin()
                else
                    page.fillLogin()
            } else if (answer.factor) {
                unlock.key = answer.key || null
                const can = p => p === 7 ? unlock.key !== null : [0, 1, 3, 8].indexOf(p) >= 0
                unlock.offered = unlock.order.filter(p => answer.factor.indexOf(p) >= 0 && can(p))
                if (unlock.offered.length === 0) {
                    unlock.provider = -1
                    unlock.problem = page.sioul.text("bitwarden-factor-unsupported")
                } else if (unlock.offered.indexOf(unlock.provider) < 0) {
                    unlock.choose(unlock.offered[0])
                } else {
                    unlock.problem = page.sioul.text("bitwarden-code-refused")
                }
            } else if (answer.new_device) {
                unlock.provider = 100
                unlock.problem = ""
                code.forceActiveFocus()
            } else {
                unlock.problem = answer.error || ""
            }
        }

        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(460, page.width - 2 * page.theme.gap)
        title: page.sioul.text("bitwarden-unlock")
        onClosed: {
            secret.text = ""
            unlock.keyAsk = null
        }

        contentItem: ColumnLayout {
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: page.sioul.text("bitwarden-unlock-help")
                wrapMode: Text.Wrap
                color: page.theme.text
            }
            // The security key alone, when Bitwarden knows it as a passkey: no master password.
            Button {
                id: passkeyButton

                visible: unlock.provider === -1 && unlock.keyAsk === null
                highlighted: unlock.passkeyFirst
                text: page.sioul.text("bitwarden-passkey")
                icon.name: "security-high"
                onClicked: unlock.usePasskey()
            }
            Label {
                visible: unlock.provider === -1 && unlock.keyAsk === null
                Layout.fillWidth: true
                text: page.sioul.text("bitwarden-passkey-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            // The key asked: what to do with it, and a way to stop.
            RowLayout {
                visible: unlock.keyAsk !== null
                Layout.fillWidth: true
                spacing: 8

                Icon {
                    iconName: "security-high"
                }
                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text(unlock.keyAsk !== null && unlock.keyAsk.mode === "passkey" ? "bitwarden-key-waiting-passkey" : "bitwarden-key-waiting")
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Button {
                    flat: true
                    text: page.sioul.text("bitwarden-key-stop")
                    onClicked: unlock.keyAsk = null
                }
            }
            // Bitwarden's page that asks the key, held unseen (one point): the key
            // answers there, its PIN in Sioul's own dialog, above this one. The page
            // must be the vault's: Bitwarden takes keys' signatures for its address only.
            Loader {
                id: keyView

                active: unlock.keyAsk !== null && page.profile !== null
                visible: active
                Layout.preferredWidth: 1
                Layout.preferredHeight: 1
                // A page asks a key only while it has the focus.
                onLoaded: (keyView.item as WebEngineView).forceActiveFocus()

                sourceComponent: WebEngineView {
                    profile: page.profile
                    url: unlock.keyAsk !== null ? unlock.keyAsk.page : ""
                    // Injection point 1: once the page's document is ready.
                    Component.onCompleted: userScripts.collection = page.webFixes.concat([{
                        name: "sioul-key",
                        sourceCode: unlock.keyAsk.script,
                        injectionPoint: 1,
                        worldId: 0,
                        runsOnSubFrames: false
                    }])
                    onWebAuthUxRequested: request => webAuth.show(request)
                }
            }
            // The key's answer, taken from the page once given; the first call asks the key.
            Timer {
                interval: 300
                repeat: true
                running: unlock.keyAsk !== null && keyView.item !== null
                onTriggered: {
                    const mode = unlock.keyAsk.mode
                    const view = keyView.item as WebEngineView
                    view.runJavaScript("window.sioulKey ? window.sioulKey() : ''", result => {
                        if (result && unlock.keyAsk !== null && unlock.keyAsk.mode === mode)
                            unlock.keyAnswered(mode, result)
                    })
                }
            }
            Label {
                visible: unlock.provider === -1
                Layout.fillWidth: true
                Layout.topMargin: 6
                text: page.sioul.text("bitwarden-password-or")
                wrapMode: Text.Wrap
                color: page.theme.text
            }
            PasswordField {
                id: secret

                sioul: page.sioul
                Layout.fillWidth: true
                placeholderText: page.sioul.text("bitwarden-password")
                onAccepted: unlock.tryIt()
            }
            // The second step, or a new device's code.
            ComboBox {
                visible: unlock.offered.length > 1
                Layout.fillWidth: true
                model: unlock.offered.map(p => page.sioul.text("bitwarden-factor-" + p))
                currentIndex: Math.max(0, unlock.offered.indexOf(unlock.provider))
                onActivated: index => unlock.choose(unlock.offered[index])
            }
            Label {
                visible: unlock.provider >= 0
                Layout.fillWidth: true
                text: page.sioul.text(unlock.provider === 100 ? "bitwarden-new-device" : "bitwarden-code-" + unlock.provider)
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            // No key among the steps the account offers: why a YubiKey may be missing.
            Label {
                visible: unlock.offered.length > 0 && unlock.offered.indexOf(7) < 0 && unlock.offered.indexOf(3) < 0 && unlock.provider !== 100
                Layout.fillWidth: true
                text: page.sioul.text("bitwarden-no-key")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            // The security key, asked again (it is asked at once when its step comes).
            Button {
                visible: unlock.provider === 7 && unlock.keyAsk === null
                highlighted: true
                text: page.sioul.text("bitwarden-key-use")
                icon.name: "security-high"
                onClicked: unlock.askKey("factor", unlock.key)
            }
            TextField {
                id: code

                visible: unlock.provider >= 0 && unlock.provider !== 7
                Layout.fillWidth: true
                placeholderText: page.sioul.text("bitwarden-code")
                inputMethodHints: Qt.ImhNoPredictiveText
                onAccepted: unlock.tryIt()
            }
            Button {
                visible: unlock.provider === 1
                flat: true
                text: page.sioul.text("bitwarden-code-again")
                onClicked: unlock.sendCode()
            }
            Label {
                visible: unlock.note !== ""
                Layout.fillWidth: true
                text: unlock.note
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            Label {
                visible: unlock.problem !== ""
                Layout.fillWidth: true
                text: unlock.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.warm
            }
        }

        footer: DialogButtonBox {
            Button {
                visible: unlock.provider !== 7
                text: page.sioul.text("bitwarden-open")
                highlighted: !unlock.passkeyFirst || unlock.provider >= 0
                onClicked: unlock.tryIt()
            }
            Button {
                text: page.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onRejected: unlock.close()
        }
    }

    // Which login to fill: the site's own when it has several or none, or any
    // of the vault's when you ask (⋮ ▸ Choose a login).
    LoginChooser {
        id: chooser

        sioul: page.sioul
        theme: page.theme
        onChosen: item => page.fillChosen(item)
    }

    // A site's name, address, type and categories, changed.
    Dialog {
        id: editing

        // What was refused (an address without https…): said here, the dialog kept open.
        property string problem: ""

        function begin() {
            editName.text = page.menuSite ? page.menuSite.name : ""
            editUrl.text = page.menuSite ? page.menuSite.url : ""
            editKind.currentIndex = page.menuSite ? Math.max(0, page.kinds.indexOf(page.menuSite.kind)) : 0
            editCategories.text = page.menuSite ? (page.menuSite.categories || []).join(", ") : ""
            editing.problem = ""
            editing.open()
        }

        // Saved, it closes; refused, it stays open with why, what you typed kept.
        function save() {
            const categories = editCategories.text.split(",").map(c => c.trim()).filter(c => c !== "")
            const problems = [page.sioul.setSite(page.menuId, "name", editName.text), page.sioul.setSite(page.menuId, "url", editUrl.text), page.sioul.setSite(page.menuId, "site", page.kinds[editKind.currentIndex]), page.sioul.setSite(page.menuId, "categories", JSON.stringify(categories))].filter(p => p !== "")
            page.reload()
            editing.problem = problems.join(" ")
            if (editing.problem === "")
                editing.close()
        }

        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(460, page.width - 2 * page.theme.gap)
        title: page.sioul.text("site-edit")

        contentItem: GridLayout {
            columns: 2
            columnSpacing: 10
            rowSpacing: 8

            Label {
                text: page.sioul.text("site-field-name")
                color: page.theme.muted
            }
            TextField {
                id: editName

                Layout.fillWidth: true
            }
            Label {
                text: page.sioul.text("site-field-url")
                color: page.theme.muted
            }
            TextField {
                id: editUrl

                Layout.fillWidth: true
                onAccepted: editing.save()
            }
            Label {
                text: page.sioul.text("site-field-kind")
                color: page.theme.muted
            }
            ComboBox {
                id: editKind

                Layout.fillWidth: true
                model: page.kinds.map(k => page.sioul.text("site-kind-" + k))
            }
            Label {
                text: page.sioul.text("site-field-categories")
                color: page.theme.muted
            }
            TextField {
                id: editCategories

                Layout.fillWidth: true
                placeholderText: page.sioul.text("site-categories-help")
                onAccepted: editing.save()
            }
            Label {
                visible: editing.problem !== ""
                Layout.columnSpan: 2
                Layout.fillWidth: true
                text: editing.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.warm
            }
        }

        // Out of the footer's own box: a box there closes the dialog on "Save"
        // before saving could be refused. Sioul's own words, translated.
        footer: Item {
            implicitWidth: editButtons.implicitWidth
            implicitHeight: editButtons.implicitHeight

            DialogButtonBox {
                id: editButtons

                anchors.fill: parent

                Button {
                    text: page.sioul.text("ui-save")
                    highlighted: true
                    DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                }
                Button {
                    text: page.sioul.text("ui-cancel")
                    DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                }
                onAccepted: editing.save()
                onRejected: editing.close()
            }
        }
    }

    // A site added: from the sites people usually keep, by country (and state or
    // province) and kind, found by a word; or by hand. Only encrypted addresses.
    Dialog {
        id: adding

        property var offered: ({ country: "", region: "", countries: [], types: [], categories: [], sites: [] })
        // "type:chat", "category:Banque", or "" for all.
        property string category: ""
        // From the list: what it is for and which mail announces it; by hand, as its kind goes.
        property string area: ""
        property var announced: []
        property string chosen: ""
        // What was refused (an address without https…): said here, the dialog kept open.
        property string problem: ""
        readonly property var country: adding.offered.countries.find(c => c.code === adding.offered.country) || null
        readonly property var shownPresets: {
            const words = find.text.toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "").split(/\s+/).filter(w => w !== "")
            const folded = t => String(t).toLowerCase().normalize("NFD").replace(/[\u0300-\u036f]/g, "")
            return adding.offered.sites
                .filter(p => adding.category === "" || (adding.category.startsWith("type:") ? p.kind === adding.category.slice(5) : p.categories.indexOf(adding.category.slice(9)) >= 0))
                .filter(p => words.every(w => folded(p.name + " " + p.about + " " + p.host + " " + p.categories.join(" ")).indexOf(w) >= 0))
                .sort((a, b) => page.kinds.indexOf(a.kind) - page.kinds.indexOf(b.kind) || a.name.localeCompare(b.name))
        }

        function load(country, region) {
            adding.offered = JSON.parse(page.sioul.sitePresets(country, region))
        }

        // Pinned, it closes on the site; refused, it stays open with why, what you typed kept.
        function save() {
            const categories = siteCategories.text.split(",").map(c => c.trim()).filter(c => c !== "")
            const made = JSON.parse(page.sioul.addSite(JSON.stringify({ name: siteName.text.trim(), url: siteUrl.text.trim(), kind: page.kinds[siteKind.currentIndex], area: adding.area, announced_by: adding.announced, categories: categories })))
            if (made.error) {
                adding.problem = made.error
                return
            }
            adding.close()
            page.reload()
            page.open(made.id)
        }

        function take(preset) {
            adding.chosen = preset.url
            siteName.text = preset.name
            siteUrl.text = preset.url
            siteKind.currentIndex = Math.max(0, page.kinds.indexOf(preset.kind))
            siteCategories.text = preset.categories.join(", ")
            adding.area = preset.area
            adding.announced = preset.announced_by
        }

        parent: Overlay.overlay
        anchors.centerIn: parent
        modal: true
        width: Math.min(560, page.width - 2 * page.theme.gap)
        height: Math.min(660, page.height - 2 * page.theme.gap)
        title: page.sioul.text("site-add")
        onAboutToShow: {
            siteName.text = ""
            siteUrl.text = "https://"
            siteKind.currentIndex = 0
            siteCategories.text = ""
            find.text = ""
            adding.category = ""
            adding.area = ""
            adding.announced = []
            adding.chosen = ""
            adding.problem = ""
            adding.load("", "")
        }
        onOpened: find.forceActiveFocus()

        contentItem: ColumnLayout {
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                ComboBox {
                    id: countryChoice

                    readonly property var choices: [{ code: "-", name: page.sioul.text("site-presets-everyone") }].concat(adding.offered.countries)

                    Layout.fillWidth: true
                    model: choices.map(c => c.name)
                    currentIndex: Math.max(0, choices.findIndex(c => c.code === (adding.offered.country || "-")))
                    onActivated: index => adding.load(choices[index].code, "")
                }
                ComboBox {
                    id: regionChoice

                    readonly property var choices: [{ code: "", name: page.sioul.text("site-presets-whole-country") }].concat(adding.country ? adding.country.regions : [])

                    visible: adding.country !== null && adding.country.regions.length > 0
                    Layout.fillWidth: true
                    model: choices.map(r => r.name)
                    currentIndex: Math.max(0, choices.findIndex(r => r.code === adding.offered.region))
                    onActivated: index => adding.load(adding.offered.country, choices[index].code)
                }
                ComboBox {
                    readonly property var choices: [{ id: "", name: page.sioul.text("site-presets-all") }].concat(adding.offered.types.map(t => ({ id: "type:" + t, name: page.sioul.text("site-kind-" + t) }))).concat(adding.offered.categories.map(c => ({ id: "category:" + c, name: c })))

                    Layout.fillWidth: true
                    model: choices.map(c => c.name)
                    currentIndex: Math.max(0, choices.findIndex(c => c.id === adding.category))
                    onActivated: index => adding.category = choices[index].id
                }
            }
            TextField {
                id: find

                Layout.fillWidth: true
                placeholderText: page.sioul.text("site-presets-find")
                inputMethodHints: Qt.ImhNoPredictiveText
            }
            ListView {
                id: presetList

                Layout.fillWidth: true
                Layout.fillHeight: true
                Layout.minimumHeight: 120
                clip: true
                model: adding.shownPresets
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {}

                delegate: ItemDelegate {
                    id: preset

                    required property var modelData

                    width: ListView.view.width
                    highlighted: adding.chosen === preset.modelData.url
                    onClicked: adding.take(preset.modelData)
                    onDoubleClicked: {
                        adding.take(preset.modelData)
                        adding.save()
                    }

                    contentItem: RowLayout {
                        spacing: 8

                        Icon {
                            iconName: page.kindIcons[preset.modelData.kind] || "internet-web-browser"
                            size: 16
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Label {
                                Layout.fillWidth: true
                                text: preset.modelData.name
                                elide: Text.ElideRight
                                font.weight: Font.DemiBold
                                color: page.theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: [preset.modelData.about, preset.modelData.categories.join(", "), preset.modelData.host, preset.modelData.calls === true ? page.sioul.text("site-presets-calls") : "", preset.modelData.kept ? page.sioul.text("site-presets-kept") : ""].filter(t => t !== "").join(" · ")
                                elide: Text.ElideRight
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                        }
                    }
                }
            }
            Label {
                visible: adding.shownPresets.length === 0
                Layout.fillWidth: true
                text: page.sioul.text("site-presets-none")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            Label {
                text: page.sioul.text("site-by-hand")
                color: page.theme.muted
            }
            GridLayout {
                Layout.fillWidth: true
                columns: 2
                columnSpacing: 10
                rowSpacing: 6

                Label {
                    text: page.sioul.text("site-field-name")
                    color: page.theme.muted
                }
                TextField {
                    id: siteName

                    Layout.fillWidth: true
                }
                Label {
                    text: page.sioul.text("site-field-url")
                    color: page.theme.muted
                }
                TextField {
                    id: siteUrl

                    Layout.fillWidth: true
                    inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoPredictiveText
                    onAccepted: adding.save()
                }
                Label {
                    text: page.sioul.text("site-field-kind")
                    color: page.theme.muted
                }
                ComboBox {
                    id: siteKind

                    Layout.fillWidth: true
                    model: page.kinds.map(k => page.sioul.text("site-kind-" + k))
                    // Chosen by hand: what it is for follows its type.
                    onActivated: adding.area = ""
                }
                Label {
                    text: page.sioul.text("site-field-categories")
                    color: page.theme.muted
                }
                TextField {
                    id: siteCategories

                    Layout.fillWidth: true
                    placeholderText: page.sioul.text("site-categories-help")
                }
            }
            Label {
                visible: adding.problem !== ""
                Layout.fillWidth: true
                text: adding.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.warm
            }
        }

        // Out of the footer's own box: a box there closes the dialog on its
        // first button before adding could be refused. Sioul's own words, translated.
        footer: Item {
            implicitWidth: addButtons.implicitWidth
            implicitHeight: addButtons.implicitHeight

            DialogButtonBox {
                id: addButtons

                anchors.fill: parent

                Button {
                    text: page.sioul.text("ui-add")
                    highlighted: true
                    DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                }
                Button {
                    text: page.sioul.text("ui-cancel")
                    DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                }
                onAccepted: adding.save()
                onRejected: adding.close()
            }
        }
    }
}
