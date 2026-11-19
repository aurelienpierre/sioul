// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A file a site offers (SiteDownloads.qml and its card, DownloadCard.qml),
// on stand-ins for Sioul and for Qt WebEngine's requests, at a phone's width
// (412): nothing is accepted before Save; the card shows the name in a field,
// where it comes from, a program's caution, the five places with the one
// chosen last first, each control named for a screen reader; each place
// starts the file into the folder Rust holds for it, under its plain name,
// and once it came whole asks Rust to place it, or, failed or cut, to keep
// nothing; "A folder I choose…" asks the system's "Save as" window first;
// Cancel cancels the request and saves nothing; several files are asked one
// after the other; one offered while Sites is not in view waits, and one its
// site gave up meanwhile is passed over. No site is reached, nothing is
// downloaded. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 860

    Theme {
        id: testTheme

        compact: true
    }

    // Sioul's backend, standing in: each call noted; its answers made from the call itself.
    QtObject {
        id: mock

        property var calls: []
        property string startProblem: ""
        property string first: "downloads"

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function isProgram(name) {
            return /\.(exe|sh)$/.test(String(name))
        }
        function note(what, asked) {
            mock.calls = mock.calls.concat([[what, JSON.parse(asked)]])
        }
        function called(what) {
            return mock.calls.filter(c => c[0] === what).map(c => c[1])
        }
        function downloadOffer(asked) {
            mock.note("offer", asked)
            const file = JSON.parse(asked)
            const program = /\.exe$/.test(file.name)
            const place = (id, enabled) => ({ id: id, label: "download-place-" + id, detail: "about " + id, enabled: enabled })
            return JSON.stringify({
                name: file.name,
                host: "files.example.org",
                from: "2 KB, from files.example.org",
                program: program,
                caution: program ? "download-program files.example.org" : "",
                places: [place("downloads", true), place("folder", true), place("notes", true), place("papers", true), place("letters", !program)],
                first: mock.first,
                folder: "/stand-in/Shared"
            })
        }
        function downloadStart(asked) {
            mock.note("start", asked)
            const started = JSON.parse(asked)
            const name = started.place === "folder" ? decodeURIComponent(started.chosen.replace(/^.*\//, "")) : started.name
            return JSON.stringify({ hold: "/stand-in/cache/downloads/" + mock.calls.length, name: name, folder: started.place === "folder" ? "/stand-in/Shared" : "", problem: mock.startProblem })
        }
        function downloadDone(asked) {
            mock.note("done", asked)
        }
        function downloadFailed(asked) {
            mock.note("failed", asked)
        }
        function downloadSaid(what, asked) {
            mock.note(what, asked)
        }
    }

    // Qt WebEngine's request, standing in: its state as Qt gives it
    // (0 asked, 1 coming, 2 come whole, 3 cancelled, 4 cut), accept and cancel.
    Component {
        id: requestKind

        QtObject {
            property int state: 0
            property string downloadFileName: "statement.pdf"
            property string suggestedFileName: "statement.pdf"
            property string mimeType: "application/pdf"
            property real totalBytes: 2048
            property url url: "https://files.example.org/statement.pdf"
            property var view: null
            property string downloadDirectory: ""
            property int interruptReason: 0
            property int accepted: 0
            property int cancelled: 0

            function accept() {
                accepted += 1
                state = 1
            }
            function cancel() {
                cancelled += 1
                state = 3
            }
        }
    }

    SiteDownloads {
        id: downloads

        anchors.fill: parent
        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "Downloads"
        when: windowShown

        readonly property var site: ({ id: "bank", name: "My bank" })

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                walk(kids[i], test, found)
            if (item.contentItem && item.contentItem !== item && kids.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            // A dialog's buttons, in its footer.
            if (item.footer && kids.indexOf(item.footer) < 0)
                walk(item.footer, test, found)
            return found
        }
        // The control shown now: a card asked again makes its places anew, the old ones going later.
        function named(name) {
            return walk(downloads.card, i => i.objectName === name && i.visible, [])[0]
        }
        function request(name) {
            const made = requestKind.createObject(root)
            if (name) {
                made.downloadFileName = name
                made.suggestedFileName = name
            }
            return made
        }
        function offered(name) {
            const made = request(name)
            downloads.offer(made, site)
            tryVerify(() => downloads.card !== null && downloads.card.opened, 2000, "the card asks")
            // Laid out before it is pressed.
            waitForRendering(downloads.card.contentItem, 2000)
            return made
        }

        function init() {
            mock.calls = []
            mock.startProblem = ""
            mock.first = "downloads"
            downloads.shown = true
            downloads.queue = []
            downloads.asking = null
            downloads.coming = []
            if (downloads.card && downloads.card.opened)
                downloads.card.done()
            if (downloads.saveAs.visible)
                downloads.saveAs.close()
            // The "Save as" window of an earlier test may have taken the keys: back to this one.
            root.Window.window.requestActivate()
            tryVerify(() => root.Window.window.active)
        }

        // Nothing is accepted before Save: the card shows the file and where it comes from.
        function test_nothing_is_saved_before_you_say_where() {
            const file = offered("statement.pdf")
            compare(file.accepted, 0)
            compare(file.downloadDirectory, "")
            compare(mock.called("start").length, 0)
            const asked = mock.called("offer")[0]
            compare([asked.site, asked.site_name, asked.name, asked.mime, asked.total, asked.url], ["bank", "My bank", "statement.pdf", "application/pdf", 2048, "https://files.example.org/statement.pdf"])
            compare(named("downloadName").text, "statement.pdf")
            compare(named("downloadName").selectedText, "statement", "its name selected without its extension")
            compare(named("downloadFrom").text, "2 KB, from files.example.org")
            compare(named("downloadFrom").textFormat, Text.PlainText, "words from the site as plain text")
            compare(walk(downloads.card, i => i.objectName === "downloadCaution", [])[0].visible, false, "no caution for a PDF")
            verify(named("downloadPlace:downloads").checked)
            verify(downloads.card.width <= root.width - 16, "at a phone's width: " + downloads.card.width)
            // Save in view whatever scrolls above it: under the card's top, within the window.
            const save = named("downloadSave")
            const at = save.mapToItem(root, 0, 0)
            verify(at.y >= 0 && at.y + save.height <= root.height, "Save in view: " + at.y)
            downloads.card.cancel()
        }

        // Every control named for a screen reader; a program's caution; a place that cannot take it, said.
        function test_a_program_is_said_and_every_control_named() {
            offered("setup.exe")
            verify(named("downloadCaution").visible)
            compare(named("downloadCaution").text, "download-program files.example.org")
            compare(named("downloadName").Accessible.name, "download-name")
            for (const id of ["downloads", "folder", "notes", "papers", "letters"]) {
                const radio = named("downloadPlace:" + id)
                compare(radio.Accessible.name, "download-place-" + id)
                compare(radio.Accessible.description, "about " + id)
            }
            verify(!named("downloadPlace:letters").enabled, "Letters cannot take a program")
            compare(named("downloadSave").text, "ui-save")
            compare(named("downloadCancel").text, "ui-cancel")
            compare(walk(downloads.card, i => i !== downloads.card && i.visible && i.Accessible.role === Accessible.Heading, []).map(h => h.text), ["download-what", "download-where"], "headed sections")
            downloads.card.cancel()
        }

        // Each place: the file started into the folder Rust holds, under its plain name; once whole, placed.
        function test_each_place_starts_the_file_then_places_it() {
            for (const place of ["downloads", "notes", "papers", "letters"]) {
                mock.calls = []
                const file = offered("statement.pdf")
                const radio = named("downloadPlace:" + place)
                mouseClick(radio)
                verify(radio.checked, place)
                named("downloadName").text = "Relevé octobre.pdf"
                mouseClick(named("downloadSave"))
                const started = mock.called("start")
                compare(started.length, 1, place)
                compare([started[0].site, started[0].host, started[0].place, started[0].name], ["bank", "files.example.org", place, "Relevé octobre.pdf"])
                compare(file.accepted, 1, place + ": accepted once")
                verify(file.downloadDirectory.startsWith("/stand-in/cache/downloads/"), "into the folder held for it: " + file.downloadDirectory)
                compare(file.downloadFileName, "Relevé octobre.pdf")
                tryVerify(() => !downloads.card.opened, 2000, "the card closed")
                compare(mock.called("done").length, 0, "not placed while it comes")
                file.state = 2
                const done = mock.called("done")
                compare(done.length, 1)
                compare([done[0].hold, done[0].name, done[0].place], [file.downloadDirectory, "Relevé octobre.pdf", place])
                compare(mock.called("failed").length, 0)
                file.state = 2
                compare(mock.called("done").length, 1, "said once")
            }
        }

        // The place chosen last for the site is the one offered first.
        function test_the_place_chosen_last_is_offered_first() {
            mock.first = "papers"
            offered("statement.pdf")
            verify(named("downloadPlace:papers").checked)
            verify(!named("downloadPlace:downloads").checked)
            keyClick(Qt.Key_Return)
            compare(mock.called("start")[0].place, "papers", "Enter in the name's field is Save")
        }

        // "A folder I choose…": the system's "Save as" window first; the card waits for it.
        function test_a_folder_chosen_asks_the_system_s_window() {
            const file = offered("statement.pdf")
            mouseClick(named("downloadPlace:folder"))
            mouseClick(named("downloadSave"))
            verify(downloads.saveAs.visible, "the Save as window")
            compare(String(downloads.saveAs.selectedFile), "file:///stand-in/Shared/statement.pdf")
            compare(mock.called("start").length, 0, "nothing starts before the folder is chosen")
            verify(downloads.card.opened, "the card waits")
            downloads.saveAs.close()
            downloads.chosen("file:///stand-in/Elsewhere/Relev%C3%A9.pdf")
            const started = mock.called("start")
            compare([started[0].place, started[0].chosen], ["folder", "file:///stand-in/Elsewhere/Relev%C3%A9.pdf"])
            compare(file.accepted, 1)
            compare(file.downloadFileName, "Relevé.pdf")
            file.state = 2
            compare(mock.called("done")[0].folder, "/stand-in/Shared")
        }

        // Cancel: nothing saved, the request cancelled so the site knows; Escape the same.
        function test_cancel_saves_nothing() {
            const file = offered("statement.pdf")
            mouseClick(named("downloadCancel"))
            compare([file.cancelled, file.accepted, file.state], [1, 0, 3])
            compare(mock.called("cancelled").length, 1)
            compare(mock.called("start").length, 0)
            const second = offered("other.pdf")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !downloads.card.opened)
            compare([second.cancelled, second.accepted], [1, 0])
            compare(mock.called("cancelled").length, 2)
        }

        // A name is needed: an empty field saves nothing and says so.
        function test_a_name_is_needed() {
            const file = offered("statement.pdf")
            named("downloadName").text = "  "
            mouseClick(named("downloadSave"))
            compare(mock.called("start").length, 0)
            verify(named("downloadProblem").visible)
            compare(file.accepted, 0)
            downloads.card.cancel()
        }

        // Rust could not hold the file: said in the card, nothing accepted.
        function test_a_start_refused_is_said_in_the_card() {
            mock.startProblem = "download-hold-failed"
            const file = offered("statement.pdf")
            mouseClick(named("downloadSave"))
            compare(file.accepted, 0)
            verify(downloads.card.opened)
            compare(named("downloadProblem").text, "download-hold-failed")
            downloads.card.cancel()
        }

        // Several files at once: one question each, in turn, none saved unasked.
        function test_several_files_are_asked_in_turn() {
            const first = offered("one.pdf")
            const second = request("two.pdf")
            const third = request("three.pdf")
            downloads.offer(second, site)
            downloads.offer(third, site)
            compare(downloads.queue.length, 2)
            compare(named("downloadName").text, "one.pdf")
            mouseClick(named("downloadSave"))
            tryCompare(named("downloadName"), "text", "two.pdf")
            verify(downloads.card.opened)
            compare([first.accepted, second.accepted, third.accepted], [1, 0, 0])
            mouseClick(named("downloadCancel"))
            tryCompare(named("downloadName"), "text", "three.pdf")
            compare([second.cancelled, third.accepted], [1, 0])
            mouseClick(named("downloadSave"))
            compare([first.accepted, second.accepted, third.accepted], [1, 0, 1])
        }

        // Failed or cut: Rust told to keep nothing, with Qt's reason; never placed.
        function test_a_file_cut_is_said_and_nothing_kept() {
            const file = offered("statement.pdf")
            mouseClick(named("downloadSave"))
            file.interruptReason = 22
            file.state = 4
            const failed = mock.called("failed")
            compare(failed.length, 1)
            compare([failed[0].hold, failed[0].name, failed[0].reason], [file.downloadDirectory, "statement.pdf", 22])
            compare(mock.called("done").length, 0)
            compare(downloads.coming.length, 0)
        }

        // Offered while Sites is not in view: it waits, said; asked once it shows.
        // Given up by its site meanwhile: passed over.
        function test_a_file_offered_out_of_view_waits() {
            downloads.shown = false
            const waiting = request("waiting.pdf")
            const gone = request("gone.pdf")
            downloads.offer(gone, site)
            downloads.offer(waiting, site)
            compare(mock.called("waiting").length, 2)
            verify(downloads.card === null || !downloads.card.opened, "no card out of view")
            gone.state = 3
            downloads.shown = true
            tryVerify(() => downloads.card.opened)
            compare(named("downloadName").text, "waiting.pdf", "the one given up passed over")
            // Given up while asked: the card closes.
            waiting.state = 3
            tryVerify(() => !downloads.card.opened)
            compare(downloads.asking, null)
            compare(mock.called("start").length, 0)
        }
    }
}
