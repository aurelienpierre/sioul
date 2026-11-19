// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The files the sites offer, on a computer (docs/sites.md, "Downloads"):
// nothing is saved before you say where. Each request Qt WebEngine gives
// (`WebEngineDownloadRequest`, the sites' profile's `downloadRequested`) is
// held unanswered, and asked in its card over the site (DownloadCard.qml),
// one after the other, while the Sites page is in view; one offered while it
// is not waits there, the status line saying so. Save starts it into a
// folder Sioul holds for it (`downloadStart`), so that nothing half-written
// shows where it goes; "A folder I choose…" asks the system's "Save as"
// window first, and its Cancel comes back to the card. Once it came whole,
// Rust places it (`downloadDone`); failed or cut, nothing is kept
// (`downloadFailed`). Cancel cancels the request: the site is told, nothing
// is saved. A request its site gave up while it waited (its page closed) is
// passed over. The requests' states are read from the requests themselves,
// as they are when asked (0 asked, 1 coming, 2 come whole, 3 cancelled,
// 4 cut: `WebEngineDownloadRequest.DownloadState`), never from a copy: this
// file does not import Qt WebEngine, so that its tests run without it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Dialogs

Item {
    id: downloads

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // Whether a question may show now: the Sites page in view, the window shown.
    property bool shown: true
    // Files offered and not asked yet, in order: {download, site}.
    property var queue: []
    // The one asked now, {download, site, offer}; null when none.
    property var asking: null
    // Files coming, each {download, hold, name, place, folder, mime}, until they come whole or not.
    property var coming: []
    // The card, made the first time a file is offered (for the window's pictures too).
    readonly property var card: cardLater.item
    // The system's "Save as" window, for "A folder I choose…".
    property alias saveAs: saveAsWindow

    // The request's states (WebEngineDownloadRequest.DownloadState).
    readonly property int asked: 0
    readonly property int comingState: 1
    readonly property int whole: 2
    readonly property int cancelledState: 3
    readonly property int cut: 4

    onShownChanged: downloads.next()

    // A file a site offers (`site`: the site in Sioul, or null), held until asked.
    function offer(download, site) {
        const entry = { download: download, site: site || null, offer: null, started: false }
        downloads.queue = downloads.queue.concat([entry])
        // Given up by its site while it waits (its page closed): its question goes.
        download.stateChanged.connect(() => downloads.givenUp(entry))
        if (!downloads.shown)
            downloads.sioul.downloadSaid("waiting", JSON.stringify(downloads.described(entry)))
        downloads.next()
    }

    // What Rust is told of a request (downloads::Offered), read from it now.
    function described(entry) {
        const download = entry.download
        const view = download.view || null
        return {
            site: entry.site ? entry.site.id : "",
            site_name: entry.site ? entry.site.name : "",
            url: String(download.url || ""),
            page: view && view.url ? String(view.url) : "",
            // The site's own name: Qt's download name may already carry a number for the system's folder.
            name: download.suggestedFileName || download.downloadFileName || "",
            mime: download.mimeType || "",
            total: download.totalBytes
        }
    }

    // The next question, when none is asked and the page is in view.
    function next() {
        if (downloads.asking !== null || !downloads.shown)
            return
        while (downloads.queue.length > 0) {
            const entry = downloads.queue[0]
            downloads.queue = downloads.queue.slice(1)
            if (entry.download.state !== downloads.asked)
                continue
            entry.offer = JSON.parse(downloads.sioul.downloadOffer(JSON.stringify(downloads.described(entry))))
            downloads.asking = entry
            cardLater.now().ask(entry.offer)
            return
        }
    }

    // Save pressed: "A folder I choose…" asks the system's window first.
    function saved(place, name) {
        if (place === "folder") {
            downloads.askFolder(name)
            return
        }
        downloads.start(place, name, "")
    }

    // The system's "Save as" window, in the folder chosen last for this site (else Downloads), the name given.
    function askFolder(name) {
        if (downloads.asking === null)
            return
        const folder = downloads.theme.fileUrl(downloads.asking.offer.folder || "")
        saveAsWindow.currentFolder = folder
        saveAsWindow.selectedFile = folder.replace(/\/$/, "") + "/" + encodeURIComponent(name)
        saveAsWindow.open()
    }

    // The file the system's window gave (a file:// address): its folder and its name.
    function chosen(url) {
        downloads.start("folder", "", String(url))
    }

    // The file started where it waits while it comes; the card closed, the next asked.
    function start(place, name, chosen) {
        const entry = downloads.asking
        if (entry === null)
            return
        const download = entry.download
        if (download.state !== downloads.asked) {
            downloads.giveUp(entry)
            return
        }
        const answer = JSON.parse(downloads.sioul.downloadStart(JSON.stringify({
            site: entry.site ? entry.site.id : "",
            site_name: entry.site ? entry.site.name : "",
            host: entry.offer.host || "",
            place: place,
            name: name,
            chosen: chosen
        })))
        if (answer.problem !== "") {
            if (downloads.card)
                downloads.card.problem = answer.problem
            return
        }
        const moving = { download: download, hold: answer.hold, name: answer.name, place: place, folder: answer.folder, mime: download.mimeType || "", over: false }
        downloads.coming = downloads.coming.concat([moving])
        entry.started = true
        download.stateChanged.connect(() => downloads.moved(moving))
        // Into Sioul's own folder for it, under its plain name: never where the site would say.
        download.downloadDirectory = answer.hold
        download.downloadFileName = answer.name
        download.accept()
        downloads.asking = null
        if (downloads.card)
            downloads.card.done()
        downloads.moved(moving)
        downloads.next()
    }

    // A file coming, as its request is now: come whole, placed; failed or cut, nothing kept.
    function moved(moving) {
        if (moving.over)
            return
        const state = moving.download.state
        if (state !== downloads.whole && state !== downloads.cancelledState && state !== downloads.cut)
            return
        moving.over = true
        downloads.coming = downloads.coming.filter(c => c !== moving)
        const said = JSON.stringify({ hold: moving.hold, name: moving.name, place: moving.place, folder: moving.folder, mime: moving.mime, reason: moving.download.interruptReason || 0 })
        if (state === downloads.whole)
            downloads.sioul.downloadDone(said)
        else
            downloads.sioul.downloadFailed(said)
    }

    // Cancel: the request cancelled, so the site knows; nothing saved, said.
    function cancel() {
        const entry = downloads.asking
        if (entry === null)
            return
        downloads.asking = null
        if (entry.download.state === downloads.asked)
            entry.download.cancel()
        downloads.sioul.downloadSaid("cancelled", JSON.stringify(downloads.described(entry)))
        downloads.next()
    }

    // A request its site gave up before it was answered: passed over, its card closed if it was asked.
    function givenUp(entry) {
        if (entry.started || entry.download.state === downloads.asked)
            return
        if (downloads.asking === entry)
            downloads.giveUp(entry)
    }

    function giveUp(entry) {
        if (downloads.asking !== entry)
            return
        downloads.asking = null
        if (downloads.card)
            downloads.card.done()
        downloads.next()
    }

    Later {
        id: cardLater

        sourceComponent: Component {
            DownloadCard {
                sioul: downloads.sioul
                theme: downloads.theme
                onSaved: (place, name) => downloads.saved(place, name)
                onCancelled: downloads.cancel()
            }
        }
    }

    // "A folder I choose…": the folder and the name, in the system's own window.
    // Its own question of a name already there is left out: such a name gets a number instead.
    FileDialog {
        id: saveAsWindow

        fileMode: FileDialog.SaveFile
        options: FileDialog.DontConfirmOverwrite
        title: downloads.sioul.text("download-save-as")
        onAccepted: downloads.chosen(saveAsWindow.selectedFile)
    }
}
