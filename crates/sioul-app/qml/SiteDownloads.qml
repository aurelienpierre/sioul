// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The files the sites offer, on a computer (docs/sites.md, "Downloads").
// Qt WebEngine fetches a file whether it is answered or not, so each
// request (`WebEngineDownloadRequest`, the sites' profile's
// `downloadRequested`) is accepted at once into a folder Sioul holds for it
// in its cache (`downloadHold`), where it waits for your answer and goes
// nowhere else. What the request says (its address, its page, its site, its
// name) is read once, as it is offered: never its view again, which Qt
// leaves dangling once the page is gone. Each is asked in its card
// (DownloadCard.qml), one after the other: over the pop-up that offered it
// while that shows, else over the Sites page while it is in view; one
// offered while neither shows waits, the status line saying so. Save says
// where (`downloadStart`); once the file came whole, Rust checks and places
// it (`downloadDone`), and a place that refuses it gives its question back
// (`downloadRefused`). Cancel cancels the request and deletes what came.
// Failed or cut, nothing is kept, said (`downloadFailed`). Bounds: three
// files of one site wait at most, more are refused; a file waiting
// unanswered past 1 GiB, or when less than 200 MiB would stay free, is let
// go; a site can be refused as a whole until it is closed or reloaded. As
// Sioul quits, every request under way is cancelled and its folder goes.
// The requests' states are read from the requests themselves (0 asked,
// 1 coming, 2 come whole, 3 cancelled, 4 cut:
// `WebEngineDownloadRequest.DownloadState`): this file does not import Qt
// WebEngine, so that its tests run without it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs

Item {
    id: downloads

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // Sioul's window (main.qml): its Letters page, shown and raised for a pop-up's file. null in tests.
    property var window: null
    // Whether a question may show over the Sites page now: the page in view, the window shown.
    property bool shown: true
    // Every file offered and not over yet, in the order offered: {download,
    // site, popup, key, said, hold, held, answer, whole, over, problem}.
    property var entries: []
    // The one asked now; null when none.
    property var asking: null
    // The card, made the first time a file is offered (for the window's pictures too).
    readonly property var card: cardLater.item
    // The system's "Save as" window, for "A folder I choose…", and the file it was opened for.
    property alias saveAs: saveAsWindow
    property var saveAsFor: null
    // The paper's form, opened once a file filed in Papers is a paper, to correct it.
    readonly property var paperForm: paperFormLater.item
    // Sites refused as a whole until closed or reloaded, those that had a file
    // refused this session, those told their fourth file was refused: by key.
    property var refused: ({})
    property var hadRefused: ({})
    property var tooManySaid: ({})

    // At most this many files of one site wait for an answer.
    readonly property int perSite: 3
    // A file waiting unanswered is let go past this size, or when less than `keptFree` would stay free.
    readonly property real largest: 1024 * 1024 * 1024
    readonly property real keptFree: 200 * 1024 * 1024

    // The request's states (WebEngineDownloadRequest.DownloadState).
    readonly property int askedState: 0
    readonly property int comingState: 1
    readonly property int whole: 2
    readonly property int cancelledState: 3
    readonly property int cut: 4

    onShownChanged: {
        // Left out of view: its question waits, said, and comes back with the page.
        const entry = downloads.asking
        if (!downloads.shown && entry !== null && !downloads.overPopup(entry)) {
            downloads.letCardGo()
            downloads.sioul.downloadSaid("waiting", JSON.stringify(entry.said))
        }
        downloads.next()
    }

    // A file a site offers (`site`: the site in Sioul, or null; `page`: the
    // address of the page that offers it; `popup`: the pop-up window it came
    // from, or null), read once now and accepted at once into its held folder.
    function offer(download, site, page, popup) {
        const said = {
            site: site ? site.id : "",
            site_name: site ? site.name : "",
            url: String(download.url || ""),
            page: String(page || ""),
            // The site's own name: Qt's download name may already carry a number for the system's folder.
            name: download.suggestedFileName || download.downloadFileName || "",
            mime: download.mimeType || "",
            total: download.totalBytes
        }
        const key = site ? site.id : "host:" + downloads.hostOf(said.url || said.page)
        if (downloads.refused[key] === true) {
            download.cancel()
            return
        }
        const waiting = downloads.entries.filter(e => e.key === key && e.answer === null).length
        if (waiting >= downloads.perSite) {
            download.cancel()
            if (downloads.tooManySaid[key] !== true) {
                downloads.tooManySaid[key] = true
                downloads.sioul.downloadSaid("too-many", JSON.stringify(said))
            }
            return
        }
        const held = JSON.parse(downloads.sioul.downloadHold(JSON.stringify(said)))
        if (held.problem !== "") {
            download.cancel()
            downloads.sioul.status = held.problem
            return
        }
        const entry = { download: download, site: site || null, popup: popup || null, key: key, said: said, hold: held.hold, held: held.held, answer: null, whole: false, over: false, problem: "", placing: false, offer: null, onState: null }
        // Into Sioul's own folder for it, under its plain name: never where the site would say.
        download.downloadDirectory = held.hold
        download.downloadFileName = held.held
        download.accept()
        entry.onState = () => downloads.moved(entry)
        download.stateChanged.connect(entry.onState)
        downloads.entries = downloads.entries.concat([entry])
        // Another file of the site asked now: its card may now refuse them all.
        if (downloads.asking !== null && downloads.asking.key === key && downloads.card)
            downloads.card.canRefuseSite = true
        if (!downloads.canAsk(entry))
            downloads.sioul.downloadSaid("waiting", JSON.stringify(said))
        downloads.moved(entry)
        downloads.next()
    }

    // The host of an address, for the key of a page that is no site of Sioul's.
    function hostOf(url) {
        const match = /^[a-z]+:\/\/([^\/?#]*)/i.exec(String(url).replace(/^blob:/, ""))
        return match ? match[1].toLowerCase() : ""
    }

    // A pop-up still there and shown (one gone reads as null).
    function popupShown(popup) {
        return popup !== null && popup !== undefined && popup.visible === true
    }

    // Whether the entry's question shows over its pop-up.
    function overPopup(entry) {
        return downloads.popupShown(entry.popup)
    }

    function canAsk(entry) {
        return downloads.overPopup(entry) || downloads.shown
    }

    // The next question, when none is asked: the oldest unanswered that can show.
    function next() {
        if (downloads.asking !== null)
            return
        for (const entry of downloads.entries) {
            if (entry.answer === null && !entry.over && downloads.canAsk(entry)) {
                downloads.ask(entry)
                return
            }
        }
    }

    // A site that had a file refused this session, or has others waiting:
    // its card offers to refuse every file from it.
    function mayRefuseSite(entry) {
        return downloads.hadRefused[entry.key] === true || downloads.entries.some(e => e !== entry && e.key === entry.key && e.answer === null)
    }

    function ask(entry) {
        entry.offer = JSON.parse(downloads.sioul.downloadOffer(JSON.stringify(entry.said)))
        downloads.asking = entry
        const card = cardLater.now()
        // Over the pop-up that offered it; else over Sioul's window, shown and raised for a pop-up's file.
        if (downloads.overPopup(entry)) {
            card.parent = entry.popup.Overlay.overlay
            entry.popup.raise()
            entry.popup.requestActivate()
        } else {
            card.parent = downloads.Overlay.overlay
            if (entry.popup !== null && downloads.window !== null) {
                downloads.window.show()
                downloads.window.raise()
                downloads.window.requestActivate()
            }
        }
        card.ask(entry.offer, entry.problem, downloads.mayRefuseSite(entry))
        entry.problem = ""
    }

    // The card closed without an answer: its file still waits.
    function letCardGo() {
        downloads.asking = null
        downloads.closeSaveAs()
        if (downloads.card)
            downloads.card.done()
    }

    function closeSaveAs() {
        downloads.saveAsFor = null
        if (saveAsWindow.visible)
            saveAsWindow.close()
    }

    // The name in the card's field changed: what the places can take, asked again.
    function renamed(name) {
        const entry = downloads.asking
        if (entry === null || !downloads.card)
            return
        downloads.card.update(JSON.parse(downloads.sioul.downloadOffer(JSON.stringify(Object.assign({}, entry.said, { name: name })))))
    }

    // Save pressed: "A folder I choose…" asks the system's window first.
    function saved(place, name) {
        const entry = downloads.asking
        if (entry === null)
            return
        if (place === "folder") {
            downloads.askFolder(entry, name)
            return
        }
        downloads.start(entry, place, name, "")
    }

    // The system's "Save as" window, in the folder chosen last for this site (else Downloads), the name given.
    function askFolder(entry, name) {
        const folder = downloads.theme.fileUrl(entry.offer.folder || "")
        downloads.saveAsFor = entry
        saveAsWindow.currentFolder = folder
        saveAsWindow.selectedFile = folder.replace(/\/$/, "") + "/" + encodeURIComponent(name)
        saveAsWindow.open()
    }

    // The file the system's window gave (a file:// address): its folder and its name, for the file it was opened for.
    function chosen(url) {
        const entry = downloads.saveAsFor
        downloads.saveAsFor = null
        if (entry === null || entry !== downloads.asking || entry.over)
            return
        downloads.start(entry, "folder", "", String(url))
    }

    // Where it goes, said: kept with the file; placed now if it came whole, else once it does.
    function start(entry, place, name, chosen) {
        const answer = JSON.parse(downloads.sioul.downloadStart(JSON.stringify(Object.assign({}, entry.said, { place: place, name: name, chosen: chosen, coming: !entry.whole }))))
        if (answer.problem !== "") {
            if (downloads.card)
                downloads.card.say(answer.problem)
            return
        }
        entry.answer = { place: place, name: answer.name, folder: answer.folder }
        downloads.asking = null
        if (downloads.card)
            downloads.card.done()
        if (entry.whole)
            downloads.place(entry)
        downloads.next()
    }

    // Rust checks and places it (off the window's thread); `onDownloadFiled` or `onDownloadRefused` follows.
    function place(entry) {
        entry.placing = true
        downloads.sioul.downloadDone(JSON.stringify({ hold: entry.hold, held: entry.held, name: entry.answer.name, place: entry.answer.place, folder: entry.answer.folder, mime: entry.said.mime }))
    }

    // A file as its request is now: come whole, placed once answered; failed or cut, nothing kept.
    function moved(entry) {
        if (entry.over)
            return
        const state = entry.download.state
        if (state === downloads.whole) {
            if (entry.whole)
                return
            entry.whole = true
            if (entry.answer !== null && !entry.placing)
                downloads.place(entry)
            return
        }
        if (state !== downloads.cancelledState && state !== downloads.cut)
            return
        downloads.finish(entry)
        // Cut: let go in Chromium too, where it would stay to be taken up again.
        if (state === downloads.cut)
            entry.download.cancel()
        downloads.sioul.downloadFailed(JSON.stringify({ hold: entry.hold, held: entry.held, name: entry.answer ? entry.answer.name : entry.held, place: entry.answer ? entry.answer.place : "", reason: entry.download.interruptReason || 0 }))
        if (downloads.asking === entry)
            downloads.letCardGo()
        downloads.next()
    }

    // Over: no longer followed, its request let go by this file.
    function finish(entry) {
        entry.over = true
        if (entry.onState !== null) {
            entry.download.stateChanged.disconnect(entry.onState)
            entry.onState = null
        }
        downloads.entries = downloads.entries.filter(e => e !== entry)
    }

    // Let go before it is placed: the request cancelled, what came deleted, said (`word`; "" nothing).
    function letGo(entry, word) {
        if (entry.over)
            return
        downloads.finish(entry)
        const state = entry.download.state
        if (state === downloads.askedState || state === downloads.comingState)
            entry.download.cancel()
        downloads.sioul.downloadDrop(JSON.stringify({ hold: entry.hold }))
        if (word !== "")
            downloads.sioul.downloadSaid(word, JSON.stringify(entry.said))
        if (downloads.asking === entry)
            downloads.letCardGo()
    }

    // Cancel: what came deleted, the site told when it still sends; its next file's card offers to refuse them all.
    function cancel() {
        const entry = downloads.asking
        if (entry === null)
            return
        downloads.hadRefused[entry.key] = true
        downloads.letGo(entry, "cancelled")
        downloads.next()
    }

    // "Refuse every file from this site": those waiting let go, the next refused until it is closed or reloaded.
    function refuseSite() {
        const entry = downloads.asking
        if (entry === null)
            return
        const key = entry.key
        downloads.refused[key] = true
        downloads.sioul.downloadSaid("refused", JSON.stringify(entry.said))
        for (const other of downloads.entries.filter(e => e.key === key && e.answer === null))
            downloads.letGo(other, "")
        downloads.next()
    }

    // A site closed or reloaded by you: its files are asked again.
    function forgive(siteId) {
        delete downloads.refused[siteId]
        delete downloads.hadRefused[siteId]
        delete downloads.tooManySaid[siteId]
    }

    // Sioul quits: every request under way cancelled, every held folder gone.
    function closeAll() {
        for (const entry of downloads.entries.slice())
            downloads.letGo(entry, "")
    }

    function byHold(hold) {
        return downloads.entries.find(e => e.hold === hold) || null
    }

    // No card nor form open over Sioul's window now.
    function nothingOpen() {
        const overlay = downloads.Overlay.overlay
        return overlay !== null && !overlay.children.some(child => child.visible)
    }

    // A paper made of a file, its form opened to correct it, only while the
    // Sites page is in view and nothing else is open; else its notice opens it.
    function correctPaper(id) {
        if (!downloads.shown || !downloads.nothingOpen())
            return
        const shown = JSON.parse(downloads.sioul.papers() || "{}")
        for (const family of shown.families || [])
            for (const paper of family.papers || [])
                if (paper.id === id) {
                    const form = paperFormLater.now()
                    form.kinds = shown.kinds || []
                    form.edit(paper)
                    return
                }
    }

    Connections {
        target: downloads.sioul

        // Placed: over. A paper's form to correct it, or the Letters page on
        // the letter, only while Sites is still in view and nothing else is open.
        function onDownloadFiled(hold, place, id) {
            const entry = downloads.byHold(hold)
            if (entry !== null)
                downloads.finish(entry)
            if (place === "papers" && id !== "")
                downloads.correctPaper(id)
            else if (place === "letters" && id !== "" && downloads.shown && downloads.nothingOpen() && downloads.window !== null)
                downloads.window.openLetter(id)
        }

        // The place refused it: still held, its question asked again, saying why.
        function onDownloadRefused(hold, problem) {
            const entry = downloads.byHold(hold)
            if (entry === null)
                return
            entry.answer = null
            entry.placing = false
            entry.problem = problem
            if (!downloads.canAsk(entry))
                downloads.sioul.downloadSaid("waiting", JSON.stringify(entry.said))
            downloads.next()
        }
    }

    // Each second while files wait: one waiting unanswered past 1 GiB, or with
    // the disk nearly full, let go; a question over a pop-up that went comes back to the window.
    Timer {
        interval: 1000
        repeat: true
        running: downloads.entries.length > 0
        onTriggered: {
            const entry = downloads.asking
            if (entry !== null && entry.popup !== null && !downloads.overPopup(entry) && downloads.card && downloads.card.parent !== downloads.Overlay.overlay) {
                downloads.letCardGo()
                if (!downloads.canAsk(entry))
                    downloads.sioul.downloadSaid("waiting", JSON.stringify(entry.said))
                downloads.next()
            }
            for (const waiting of downloads.entries.slice()) {
                if (waiting.answer !== null || waiting.whole || waiting.over)
                    continue
                const got = waiting.download.receivedBytes
                const left = Math.max(0, waiting.download.totalBytes - got)
                const free = downloads.sioul.downloadRoom(waiting.hold)
                if (got > downloads.largest || (free >= 0 && free - left < downloads.keptFree)) {
                    downloads.letGo(waiting, "too-big")
                    downloads.next()
                }
            }
        }
    }

    Later {
        id: cardLater

        sourceComponent: Component {
            DownloadCard {
                sioul: downloads.sioul
                theme: downloads.theme
                onSaved: (place, name) => downloads.saved(place, name)
                onCancelled: downloads.cancel()
                onSiteRefused: downloads.refuseSite()
                onRenamed: name => downloads.renamed(name)
            }
        }
    }

    Later {
        id: paperFormLater

        sourceComponent: Component {
            PaperDialog {
                sioul: downloads.sioul
                theme: downloads.theme
                window: downloads.window
            }
        }
    }

    // "A folder I choose…": the folder and the name, in the system's own window.
    // Its own question of a name already there is left out where it can be (Qt's
    // window): such a name gets a number instead, and the notice says it.
    FileDialog {
        id: saveAsWindow

        fileMode: FileDialog.SaveFile
        options: FileDialog.DontConfirmOverwrite
        title: downloads.sioul.text("download-save-as")
        onAccepted: downloads.chosen(saveAsWindow.selectedFile)
        // Closed without a folder: back to the card, the keys with it.
        onRejected: {
            downloads.saveAsFor = null
            if (downloads.card && downloads.card.opened)
                downloads.card.refocus()
        }
    }
}
