// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Stand-ins for the files the sites offer (tst_downloads.qml,
// tst_downloads_access.qml), no kinder than the program: Sioul's backend,
// answering calls (each noted), cleaning names as Rust does and deriving the
// folder from what the "Save as" window gave; and Qt WebEngine's request
// (`request`), which fetches once accepted, ignores a folder or a name set
// once accepted, cannot be cancelled once whole, and counts every read of
// its view (`viewReads`), which Qt crashes on once the page is gone.

import QtQuick

QtObject {
    id: mock

    // Reads of a request's view: none is allowed (review of the downloads, 3.2).
    property int viewReads: 0

    property var calls: []
    property string status: ""
    property string holdProblem: ""
    property string startProblem: ""
    property string first: "downloads"
    property real room: 1e12
    property int holds: 0

    signal downloadFiled(string hold, string place, string id)
    signal downloadRefused(string hold, string problem)

    function text(key) {
        return key
    }
    function textWith(key, name, value) {
        return key + " " + value
    }
    function isProgram(name) {
        return /\.(exe|sh)$/.test(String(name))
    }
    function papers() {
        return JSON.stringify({ families: [], kinds: [] })
    }
    function note(what, asked) {
        mock.calls = mock.calls.concat([[what, JSON.parse(asked)]])
    }
    function called(what) {
        return mock.calls.filter(c => c[0] === what).map(c => c[1])
    }
    // As Rust does: one plain name, a PDF's extension for a name without one.
    function clean(name, mime) {
        const plain = String(name).replace(/[\/\\]/g, "_")
        return /\.[A-Za-z0-9]+$/.test(plain) || mime !== "application/pdf" ? plain : plain + ".pdf"
    }
    function downloadHold(asked) {
        mock.note("hold", asked)
        const file = JSON.parse(asked)
        mock.holds += 1
        return JSON.stringify({ hold: "/stand-in/cache/downloads/" + mock.holds, held: mock.clean(file.name, file.mime), problem: mock.holdProblem })
    }
    function downloadOffer(asked) {
        mock.note("offer", asked)
        const file = JSON.parse(asked)
        const name = mock.clean(file.name, file.mime)
        const program = /\.exe$/.test(name) || file.mime === "application/x-msdownload"
        const scan = /\.(pdf|png|jpg)$/.test(name)
        const note = /\.(pdf|png|md|ods)$/.test(name)
        const place = (id, enabled) => ({ id: id, label: "download-place-" + id, detail: enabled ? "about " + id : "not " + id, enabled: enabled })
        return JSON.stringify({
            name: name,
            host: "files.example.org",
            from: "2 KB, from files.example.org",
            waits: "download-waits",
            program: program,
            caution: program ? "download-program files.example.org" : "",
            places: [place("downloads", true), place("folder", true), place("notes", note && !program), place("papers", !program), place("letters", scan && !program)],
            first: mock.first,
            folder: "/stand-in/Shared"
        })
    }
    function downloadStart(asked) {
        mock.note("start", asked)
        const started = JSON.parse(asked)
        if (started.place === "folder") {
            const path = decodeURIComponent(String(started.chosen).replace(/^file:\/\//, ""))
            const folder = path.slice(0, path.lastIndexOf("/"))
            if (folder.startsWith("/stand-in/state"))
                return JSON.stringify({ name: "", folder: "", problem: "download-folder-own" })
            return JSON.stringify({ name: mock.clean(path.slice(path.lastIndexOf("/") + 1), ""), folder: folder, problem: mock.startProblem })
        }
        return JSON.stringify({ name: mock.clean(started.name, ""), folder: "", problem: mock.startProblem })
    }
    function downloadRoom(hold) {
        return mock.room
    }
    function downloadDone(asked) {
        mock.note("done", asked)
    }
    function downloadFailed(asked) {
        mock.note("failed", asked)
    }
    function downloadDrop(asked) {
        mock.note("drop", asked)
    }
    function downloadSaid(what, asked) {
        mock.note(what, asked)
    }

    // Qt WebEngine's request, standing in, no kinder than Qt: its state
    // (0 asked, 1 coming, 2 come whole, 3 cancelled, 4 cut) told to whoever
    // listens; accepted, it fetches; a folder or a name set once accepted is
    // ignored; `cancel` does nothing once it came whole; its view is never to be read.
    function request(name, mime) {
        const listeners = []
        const made = {
            stateNow: 0,
            folderNow: "",
            nameNow: name || "statement.pdf",
            suggestedFileName: name || "statement.pdf",
            mimeType: mime || "application/pdf",
            totalBytes: 2048,
            receivedBytes: 0,
            url: "https://files.example.org/" + (name || "statement.pdf"),
            interruptReason: 0,
            accepted: 0,
            cancelled: 0,
            ignored: 0,
            get view() {
                mock.viewReads += 1
                throw new Error("a request's view read after it was offered: Qt crashes on this once the page is gone")
            },
            get state() {
                return this.stateNow
            },
            set state(value) {
                this.stateNow = value
                for (const listener of listeners.slice())
                    listener()
            },
            get downloadDirectory() {
                return this.folderNow
            },
            set downloadDirectory(value) {
                if (this.stateNow === 0)
                    this.folderNow = value
                else
                    this.ignored += 1
            },
            get downloadFileName() {
                return this.nameNow
            },
            set downloadFileName(value) {
                if (this.stateNow === 0)
                    this.nameNow = value
                else
                    this.ignored += 1
            },
            stateChanged: {
                connect: f => listeners.push(f),
                disconnect: f => {
                    const at = listeners.indexOf(f)
                    if (at >= 0)
                        listeners.splice(at, 1)
                }
            },
            listening: () => listeners.length,
            accept() {
                this.accepted += 1
                this.state = 1
            },
            cancel() {
                if (this.stateNow === 2 || this.stateNow === 3)
                    return
                this.cancelled += 1
                this.state = 3
            }
        }
        return made
    }
}
