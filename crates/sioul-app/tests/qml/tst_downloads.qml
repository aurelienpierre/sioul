// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A file a site offers (SiteDownloads.qml and its card, DownloadCard.qml),
// on stand-ins for Sioul and for Qt WebEngine's requests, at a phone's width
// (412); what a screen reader hears is tst_downloads_access.qml's. Each
// request is accepted at once into the
// folder Rust holds for it, and nothing is placed before Save; the card
// shows the name in a field, where it comes from, where it waits, a
// program's caution, the five places with the one chosen last first, each
// control named; the places follow the name typed; Save places the file
// once it came whole, under the name Rust answered; a place that refuses it
// asks again; "A folder I choose…" asks the system's "Save as" window, whose
// answer goes to the file it was opened for; Cancel deletes what came;
// several files are asked one after the other; three of one site wait at
// most; a site can be refused as a whole until it is closed or reloaded; a
// file past its bounds is let go; a pop-up's file is asked over the pop-up;
// one offered while Sites is not in view waits; as Sioul quits, everything
// is let go. The keyboard: arrows move the choice, Enter on a place saves
// there, Enter on Cancel cancels.
//
// The stand-ins are no kinder than the program (docs/sites.md, "Downloads"):
// the request fetches once accepted, whatever is answered; it ignores a
// folder or a name set after it is accepted, as Qt does; its `cancel` does
// nothing once it came whole; and reading its view fails the test, as Qt
// crashes on that read once the page is gone. The backend's stand-in answers
// calls, cleans names and derives the folder from what the "Save as" window
// gave. No site is reached, nothing is downloaded. Run by tools/qml-test.sh.

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

    // Sioul's backend and Qt WebEngine's requests, standing in (DownloadsStandIn.qml).
    DownloadsStandIn {
        id: mock
    }

    // Sioul's window, standing in: the Letters page asked for a letter.
    QtObject {
        id: fakeWindow

        property var letters: []

        function openLetter(id) {
            fakeWindow.letters = fakeWindow.letters.concat([id])
        }
        function show() {
        }
        function raise() {
        }
        function requestActivate() {
        }
    }

    // A pop-up's window, standing in: a window of its own.
    Window {
        id: popupWindow

        width: 412
        height: 700
        visible: false
    }

    SiteDownloads {
        id: downloads

        anchors.fill: parent
        sioul: mock
        theme: testTheme
        window: fakeWindow
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
        // Typed, key by key, as a person does (the field hears each edit).
        function typed(text) {
            for (const c of text)
                keyClick(c)
        }
        function offered(name, mime, from, popup) {
            const made = mock.request(name, mime)
            downloads.offer(made, from === undefined ? site : from, "https://files.example.org/statements", popup || null)
            tryVerify(() => downloads.card !== null && downloads.card.opened, 2000, "the card asks")
            // Laid out before it is pressed.
            waitForRendering(downloads.card.contentItem, 2000)
            return made
        }

        function init() {
            mock.calls = []
            mock.status = ""
            mock.holdProblem = ""
            mock.startProblem = ""
            mock.first = "downloads"
            mock.room = 1e12
            fakeWindow.letters = []
            popupWindow.visible = false
            downloads.shown = true
            downloads.closeAll()
            downloads.refused = {}
            downloads.hadRefused = {}
            downloads.tooManySaid = {}
            if (downloads.card && downloads.card.opened)
                downloads.card.done()
            downloads.closeSaveAs()
            mock.viewReads = 0
            mock.calls = []
            // The "Save as" window of an earlier test may have taken the keys: back to this one.
            root.Window.window.requestActivate()
            tryVerify(() => root.Window.window.active)
        }

        function cleanup() {
            compare(mock.viewReads, 0, "a request's view is never read after it is offered")
        }

        // Accepted at once, into the folder Rust holds for it, under its plain
        // name; nothing placed before Save, even once it came whole.
        function test_a_file_waits_in_its_held_folder_until_you_say_where() {
            const file = offered("statement.pdf")
            compare([file.accepted, file.downloadDirectory, file.downloadFileName], [1, "/stand-in/cache/downloads/" + mock.holds, "statement.pdf"])
            const asked = mock.called("hold")[0]
            compare([asked.site, asked.site_name, asked.name, asked.mime, asked.total, asked.url, asked.page], ["bank", "My bank", "statement.pdf", "application/pdf", 2048, "https://files.example.org/statement.pdf", "https://files.example.org/statements"])
            file.state = 2
            compare(mock.called("done").length, 0, "come whole, still not placed")
            compare(named("downloadName").text, "statement.pdf")
            compare(named("downloadName").selectedText, "statement", "its name selected without its extension")
            compare(named("downloadFrom").text, "2 KB, from files.example.org")
            compare(named("downloadFrom").textFormat, Text.PlainText, "words from the site as plain text")
            compare(named("downloadWaits").text, "download-waits", "where it waits, what Cancel does")
            compare(walk(downloads.card, i => i.objectName === "downloadCaution", [])[0].visible, false, "no caution for a PDF")
            verify(walk(downloads.card, i => i.objectName === "downloadRefuseSite", [])[0].visible === false, "no link to refuse a site's first file")
            verify(named("downloadPlace:downloads").checked)
            verify(downloads.card.width <= root.width - 16, "at a phone's width: " + downloads.card.width)
            // Save in view whatever scrolls above it: under the card's top, within the window.
            const save = named("downloadSave")
            const at = save.mapToItem(root, 0, 0)
            verify(at.y >= 0 && at.y + save.height <= root.height, "Save in view: " + at.y)
            mouseClick(save)
            const done = mock.called("done")
            compare(done.length, 1)
            compare([done[0].hold, done[0].held, done[0].name, done[0].place], [file.downloadDirectory, "statement.pdf", "statement.pdf", "downloads"])
            // Over once Rust says so.
            mock.downloadFiled(file.downloadDirectory, "downloads", "")
            compare(downloads.entries.length, 0)
        }

        // Each place: said at Save, the name Rust answered kept; placed once it came whole; said once.
        function test_each_place_places_the_file_once_it_came_whole() {
            for (const place of ["downloads", "notes", "papers", "letters"]) {
                mock.calls = []
                const file = offered("statement.pdf")
                const radio = named("downloadPlace:" + place)
                mouseClick(radio)
                verify(radio.checked, place)
                named("downloadName").text = "Relevé/octobre.pdf"
                mouseClick(named("downloadSave"))
                const started = mock.called("start")
                compare(started.length, 1, place)
                compare([started[0].site, started[0].url, started[0].place, started[0].name, started[0].coming], ["bank", "https://files.example.org/statement.pdf", place, "Relevé/octobre.pdf", true])
                tryVerify(() => !downloads.card.opened, 2000, "the card closed")
                compare(mock.called("done").length, 0, "not placed while it comes")
                compare(file.downloadDirectory, "/stand-in/cache/downloads/" + mock.holds, "the held folder, set before it was accepted")
                file.state = 2
                const done = mock.called("done")
                compare(done.length, 1)
                compare([done[0].held, done[0].name, done[0].place], ["statement.pdf", "Relevé_octobre.pdf", place], "the name Rust answered, not the field's")
                file.state = 2
                compare(mock.called("done").length, 1, "said once")
                mock.downloadFiled(file.downloadDirectory, place, "")
            }
        }

        // The place chosen last for the site is the one offered first; Enter in the name is Save.
        function test_the_place_chosen_last_is_offered_first() {
            mock.first = "papers"
            offered("statement.pdf")
            verify(named("downloadPlace:papers").checked)
            verify(!named("downloadPlace:downloads").checked)
            keyClick(Qt.Key_Return)
            compare(mock.called("start")[0].place, "papers", "Enter in the name's field is Save")
        }

        // The keyboard: the arrows move the choice among the places that can
        // take the file; Enter on a place chooses it and saves there; Enter on Cancel cancels.
        function test_the_keyboard_chooses_and_saves_where_it_stands() {
            offered("setup.exe", "application/x-msdownload")
            named("downloadPlace:downloads").forceActiveFocus()
            keyClick(Qt.Key_Down)
            verify(named("downloadPlace:folder").checked && named("downloadPlace:folder").activeFocus, "down: the next place")
            keyClick(Qt.Key_Down)
            verify(named("downloadPlace:downloads").checked, "the places a program cannot go are passed over, around the end")
            keyClick(Qt.Key_Up)
            verify(named("downloadPlace:folder").checked, "up: the one before")
            mouseClick(named("downloadPlace:downloads"))
            named("downloadPlace:folder").forceActiveFocus()
            keyClick(Qt.Key_Return)
            verify(downloads.saveAs.visible, "Enter on a place saves there, not where the choice was")
            // The system's window closed without a folder: the keys come back to the card.
            downloads.saveAs.reject()
            root.Window.window.requestActivate()
            tryVerify(() => root.Window.window.active)
            tryCompare(root.Window.window, "activeFocusItem", named("downloadName"), 2000, "the keys back in the card")
            named("downloadCancel").forceActiveFocus()
            keyClick(Qt.Key_Return)
            tryVerify(() => !downloads.card.opened)
            compare(mock.called("cancelled").length, 1, "Enter on Cancel cancels")
            compare(mock.called("start").length, 0)
        }

        // The places follow the name typed: a scan's type opens Letters; a program's shuts Sioul's places.
        function test_the_card_follows_the_name_typed() {
            offered("scan", "application/octet-stream")
            verify(!named("downloadPlace:letters").enabled, "no scan's type")
            const field = named("downloadName")
            field.forceActiveFocus()
            field.selectAll()
            keyClick(Qt.Key_Delete)
            typed("scan.pdf")
            tryVerify(() => named("downloadPlace:letters").enabled, 2000, "a PDF: Letters can take it")
            mouseClick(named("downloadPlace:letters"))
            field.forceActiveFocus()
            field.selectAll()
            typed("scan.exe")
            tryVerify(() => named("downloadCaution") !== undefined && named("downloadCaution").visible, 2000, "a program's name: its caution")
            verify(!named("downloadPlace:letters").enabled)
            verify(named("downloadPlace:downloads").checked, "the place chosen could no longer take it: Downloads")
            downloads.card.cancel()
        }

        // "A folder I choose…": the system's "Save as" window first; its answer
        // goes to the file it was opened for, and goes when that file does.
        function test_a_folder_chosen_asks_the_system_s_window() {
            const file = offered("statement.pdf")
            mouseClick(named("downloadPlace:folder"))
            mouseClick(named("downloadSave"))
            verify(downloads.saveAs.visible, "the Save as window")
            compare(String(downloads.saveAs.selectedFile), "file:///stand-in/Shared/statement.pdf")
            compare(mock.called("start").length, 0, "nothing said before the folder is chosen")
            verify(downloads.card.opened, "the card waits")
            downloads.saveAs.selectedFile = "file:///stand-in/Elsewhere/Relev%C3%A9.pdf"
            downloads.saveAs.accepted()
            const started = mock.called("start")
            compare([started[0].place, decodeURIComponent(started[0].chosen)], ["folder", "file:///stand-in/Elsewhere/Relevé.pdf"])
            file.state = 2
            compare([mock.called("done")[0].folder, mock.called("done")[0].name], ["/stand-in/Elsewhere", "Relevé.pdf"])
            mock.downloadFiled(file.downloadDirectory, "folder", "")
            // A folder of Sioul's own: refused in the card.
            offered("other.pdf")
            mouseClick(named("downloadPlace:folder"))
            mouseClick(named("downloadSave"))
            downloads.saveAs.selectedFile = "file:///stand-in/state/sioul/other.pdf"
            downloads.saveAs.accepted()
            compare(named("downloadProblem").text, "download-folder-own")
            verify(downloads.card.opened)
            // The window open for one file, which goes: the window goes with it, and its answer saves nothing.
            const first = downloads.asking.download
            const second = mock.request("second.pdf")
            downloads.offer(second, { id: "other", name: "Other" }, "", null)
            mouseClick(named("downloadSave"))
            verify(downloads.saveAs.visible)
            first.interruptReason = 22
            first.state = 4
            tryVerify(() => !downloads.saveAs.visible, 2000, "the window closed with its file")
            tryCompare(named("downloadName"), "text", "second.pdf")
            downloads.saveAs.selectedFile = "file:///stand-in/Elsewhere/first.pdf"
            downloads.chosen("file:///stand-in/Elsewhere/first.pdf")
            compare(mock.called("start").filter(s => s.place === "folder" && s.chosen.indexOf("first.pdf") >= 0).length, 0, "not given to another file")
            downloads.card.cancel()
        }

        // Cancel: what came deleted; the request cancelled while it still comes,
        // nothing to cancel once it came whole; Escape the same.
        function test_cancel_deletes_what_came() {
            const file = offered("statement.pdf")
            file.state = 2
            mouseClick(named("downloadCancel"))
            compare([file.cancelled, mock.called("drop").length, mock.called("drop")[0].hold], [0, 1, file.downloadDirectory])
            compare(mock.called("cancelled").length, 1)
            compare(mock.called("done").length, 0)
            const second = offered("other.pdf")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !downloads.card.opened)
            compare([second.cancelled, mock.called("drop").length], [1, 2])
            compare(second.listening(), 0, "no longer followed")
        }

        // A name is needed: an empty field saves nothing and says so.
        function test_a_name_is_needed() {
            offered("statement.pdf")
            named("downloadName").text = "  "
            mouseClick(named("downloadSave"))
            compare(mock.called("start").length, 0)
            verify(named("downloadProblem").visible)
            downloads.card.cancel()
        }

        // Rust refused the place: said in the card, the card stays.
        function test_a_start_refused_is_said_in_the_card() {
            mock.startProblem = "download-papers-program"
            offered("statement.pdf")
            mouseClick(named("downloadSave"))
            verify(downloads.card.opened)
            compare(named("downloadProblem").text, "download-papers-program")
            downloads.card.cancel()
        }

        // The cache refused: the request cancelled, said; no card.
        function test_a_file_the_cache_refuses_is_said() {
            mock.holdProblem = "download-hold-failed"
            const file = mock.request("statement.pdf")
            downloads.offer(file, site, "", null)
            compare([file.accepted, file.cancelled, mock.status], [0, 1, "download-hold-failed"])
            compare(downloads.entries.length, 0)
        }

        // The place refused the file once it came (full, closed): it stays held, asked again, saying why.
        function test_a_place_that_refuses_asks_again() {
            const file = offered("statement.pdf")
            mouseClick(named("downloadPlace:notes"))
            mouseClick(named("downloadSave"))
            file.state = 2
            compare(mock.called("done").length, 1)
            mock.downloadRefused(file.downloadDirectory, "statement.pdf could not be saved there (no room)")
            tryVerify(() => downloads.card.opened, 2000, "asked again")
            compare(named("downloadProblem").text, "statement.pdf could not be saved there (no room)")
            mouseClick(named("downloadPlace:downloads"))
            mouseClick(named("downloadSave"))
            compare(mock.called("done").length, 2, "placed again at once: it came whole")
            compare(mock.called("done")[1].place, "downloads")
        }

        // Several files at once: one question each, in turn, each fetched meanwhile.
        function test_several_files_are_asked_in_turn() {
            const first = offered("one.pdf")
            const second = mock.request("two.pdf")
            const third = mock.request("three.pdf")
            downloads.offer(second, site, "", null)
            downloads.offer(third, site, "", null)
            compare([first.accepted, second.accepted, third.accepted], [1, 1, 1], "each comes into its own held folder")
            verify(named("downloadRefuseSite").visible, "others wait from this site: it can be refused")
            compare(named("downloadName").text, "one.pdf")
            mouseClick(named("downloadSave"))
            tryCompare(named("downloadName"), "text", "two.pdf")
            mouseClick(named("downloadCancel"))
            tryCompare(named("downloadName"), "text", "three.pdf")
            compare(second.cancelled, 1)
            mouseClick(named("downloadSave"))
            compare(mock.called("start").map(s => s.name), ["one.pdf", "three.pdf"])
        }

        // Three files of one site wait at most: the fourth and the next refused, said once; another site still asked.
        function test_three_files_of_a_site_wait_at_most() {
            offered("auto-0.txt", "text/plain")
            const made = [mock.request("auto-1.txt")]
            for (let i = 2; i < 30; ++i)
                made.push(mock.request("auto-" + i + ".txt"))
            for (const file of made)
                downloads.offer(file, site, "", null)
            compare(downloads.entries.length, 3)
            compare(made.filter(f => f.accepted === 1).length, 2)
            compare(made.filter(f => f.cancelled === 1 && f.accepted === 0).length, 27, "of thirty, three held; the 27 others refused, never fetched")
            compare(mock.called("too-many").length, 1, "said once")
            const other = mock.request("other.pdf")
            downloads.offer(other, { id: "other", name: "Other" }, "", null)
            compare(other.accepted, 1)
        }

        // A site refused as a whole: offered after one of its files was
        // refused; those waiting let go, the next refused at once until it is closed or reloaded.
        function test_a_site_can_be_refused_as_a_whole() {
            offered("one.pdf")
            mouseClick(named("downloadCancel"))
            const second = offered("two.pdf")
            verify(named("downloadRefuseSite").visible, "after a file refused")
            compare(named("downloadRefuseSite").Accessible.name, "download-refuse-site")
            const third = mock.request("three.pdf")
            downloads.offer(third, site, "", null)
            mouseClick(named("downloadRefuseSite"))
            tryVerify(() => !downloads.card.opened)
            compare([second.cancelled, third.cancelled, mock.called("refused").length], [1, 1, 1])
            compare(mock.called("drop").length, 3, "what came of each deleted")
            const fourth = mock.request("four.pdf")
            downloads.offer(fourth, site, "", null)
            compare([fourth.accepted, fourth.cancelled], [0, 1], "refused at once")
            verify(!downloads.card.opened)
            downloads.forgive("bank")
            const fifth = offered("five.pdf")
            compare(fifth.accepted, 1, "asked again once the site is reloaded")
            verify(!named("downloadRefuseSite"), "forgiven: no link before a file is refused again")
            downloads.card.cancel()
        }

        // A file waiting unanswered past 1 GiB, or with the disk nearly full, is let go, said.
        function test_a_file_past_its_bounds_is_let_go() {
            const big = offered("big.pdf")
            big.receivedBytes = 1.1 * 1024 * 1024 * 1024
            tryVerify(() => mock.called("too-big").length === 1, 3000, "past 1 GiB")
            compare([big.cancelled, mock.called("drop").length], [1, 1])
            tryVerify(() => !downloads.card.opened)
            const crowded = offered("crowded.pdf")
            crowded.totalBytes = 300 * 1024 * 1024
            mock.room = 400 * 1024 * 1024
            tryVerify(() => mock.called("too-big").length === 2, 3000, "less than 200 MiB would stay free")
            compare(crowded.cancelled, 1)
            // Answered, it is no longer bounded: you said where.
            mock.room = 1e12
            const answered = offered("answered.pdf")
            mouseClick(named("downloadSave"))
            answered.receivedBytes = 2 * 1024 * 1024 * 1024
            wait(1500)
            compare(answered.cancelled, 0)
        }

        // Failed or cut: Rust told to keep nothing, with Qt's reason; let go in
        // Chromium too; answered or not, the next is asked.
        function test_a_file_cut_is_said_and_nothing_kept() {
            const file = offered("statement.pdf")
            mouseClick(named("downloadSave"))
            file.interruptReason = 22
            file.state = 4
            const failed = mock.called("failed")
            compare(failed.length, 1)
            compare([failed[0].hold, failed[0].name, failed[0].reason, failed[0].place], [file.downloadDirectory, "statement.pdf", 22, "downloads"])
            compare(file.cancelled, 1, "let go in Chromium, where it would stay resumable")
            compare(mock.called("done").length, 0)
            const waiting = offered("waiting.pdf")
            const next = mock.request("next.pdf")
            downloads.offer(next, site, "", null)
            waiting.state = 4
            compare(mock.called("failed").length, 2)
            tryCompare(named("downloadName"), "text", "next.pdf", 2000, "the next asked")
            downloads.card.cancel()
        }

        // Offered while Sites is not in view: fetched, said, asked once it shows;
        // left out of view while asked: the card goes, the question waits.
        function test_a_file_offered_out_of_view_waits() {
            downloads.shown = false
            const waiting = mock.request("waiting.pdf")
            downloads.offer(waiting, site, "", null)
            compare(mock.called("waiting").length, 1)
            compare(waiting.accepted, 1)
            verify(downloads.card === null || !downloads.card.opened, "no card out of view")
            downloads.shown = true
            tryVerify(() => downloads.card.opened)
            compare(named("downloadName").text, "waiting.pdf")
            downloads.shown = false
            tryVerify(() => !downloads.card.opened, 2000, "the card goes with the page")
            compare([mock.called("waiting").length, waiting.cancelled, mock.called("cancelled").length], [2, 0, 0], "it waits, not cancelled")
            downloads.shown = true
            tryVerify(() => downloads.card.opened, 2000, "asked again")
            compare(named("downloadName").text, "waiting.pdf")
            downloads.card.cancel()
        }

        // A pop-up's file: its card over the pop-up; the pop-up gone, the card
        // comes to Sioul's window, which is shown for it.
        function test_a_pop_up_s_file_is_asked_over_it() {
            popupWindow.visible = true
            downloads.shown = false
            const file = offered("statement.pdf", "application/pdf", site, popupWindow)
            compare(downloads.card.parent, popupWindow.Overlay.overlay, "over the pop-up, whatever is in view in Sioul")
            popupWindow.visible = false
            tryVerify(() => !downloads.card.opened, 3000, "the pop-up gone: the card leaves it")
            compare(mock.called("waiting").length, 1, "Sites not in view: its question waits there, said")
            downloads.shown = true
            tryVerify(() => downloads.card.opened, 2000)
            compare(downloads.card.parent, downloads.Overlay.overlay, "now over Sioul's window")
            downloads.card.cancel()
            compare(file.cancelled, 1)
        }

        // A letter filed: the Letters page on it only while Sites is still in view.
        function test_a_letter_filed_is_shown_only_from_sites() {
            const file = offered("courrier.pdf")
            mouseClick(named("downloadPlace:letters"))
            mouseClick(named("downloadSave"))
            file.state = 2
            mock.downloadFiled(file.downloadDirectory, "letters", "2026-11-20-0a1b2c3d")
            compare(fakeWindow.letters, ["2026-11-20-0a1b2c3d"])
            downloads.shown = false
            mock.downloadFiled("/stand-in/cache/downloads/gone", "letters", "2026-11-20-ffffffff")
            compare(fakeWindow.letters, ["2026-11-20-0a1b2c3d"], "never taken to another page from elsewhere")
            verify(downloads.paperForm === null, "a paper's form neither")
            mock.downloadFiled("/stand-in/cache/downloads/gone", "papers", "p-1")
            verify(downloads.paperForm === null)
        }

        // Sioul quits: every request under way cancelled, every held folder deleted.
        function test_at_quit_every_file_is_let_go() {
            const asked = offered("one.pdf")
            const waiting = mock.request("two.pdf")
            downloads.offer(waiting, { id: "other", name: "Other" }, "", null)
            const whole = mock.request("three.pdf")
            downloads.offer(whole, { id: "third", name: "Third" }, "", null)
            whole.state = 2
            downloads.closeAll()
            compare(downloads.entries.length, 0)
            compare([asked.cancelled, waiting.cancelled, whole.cancelled, mock.called("drop").length], [1, 1, 0, 3], "come whole: nothing to cancel, its folder deleted all the same")
            tryVerify(() => !downloads.card.opened)
        }
    }
}
