// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Your folder and sharing (SharePanel.qml), sharing on, on a
// stand-in for Sioul: this device's build, the line that leads to the
// devices' own tab (tst_devices.qml tests that tab), the switch that sends
// this device's files to the server beside the sync app, and "Send
// everything again" with what it said; notes held after going many at once,
// with Put them back and Take them out now; "Remove its files" for a device
// that no longer counts, what goes said before the press.
// The sending itself is tested in sioul-sync (remote.rs); here the window's
// side, in English and French. Run by tools/qml-test.sh. To see the panel,
// set `pictures` below to a folder (a change of your own, not to commit).

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    // Where the pictures go; none when empty.
    readonly property string pictures: ""

    width: 900
    height: 1400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string language: "en"
        // The notes folder as the backend says it (the sharing's line to the Notes page).
        property string notesFolder: JSON.stringify({ state: "ready", path: "/stand-in/Notes" })
        // Where Sioul's lists are (`storage::view_json`): "" until known.
        property string storageMove: ""
        // What the sharing's line to the notes folder asked the window to open.
        property var opened: []
        property var sends: []
        property int again: 0
        property bool sent: false
        // Notes held after going many at once, and a device that no longer counts (`test_held_notes_and_a_device_s_files`).
        property bool held: false
        // What waits to leave the server, said (`leaving`).
        property bool leaving: false
        property var asked: []
        // Made up: no real device, no real server.
        readonly property var words: ({
            "en": {
                "share-now": "Exchange now",
                "share-stop": "Stop sharing",
                "share-send-help": "Right after each change, Sioul sends this device's own files there itself, beside your sync app.",
                "share-backup-switch": "Also fetch them from the server",
                "share-backup-help": "When your sync app is late, Sioul also reads them on the server.",
                "share-send-again": "Send everything again",
                "share-send-again-help": "Sends each of this device's files the server lacks or holds otherwise.",
                "share-send-again-sending": "Sending this device's files…"
            },
            "fr": {
                "share-now": "Échanger maintenant",
                "share-stop": "Arrêter le partage",
                "share-send-help": "Juste après chaque changement, Sioul envoie lui-même là-bas les fichiers de cet appareil, à côté de votre application de synchronisation.",
                "share-backup-switch": "Aller aussi les chercher sur le serveur",
                "share-backup-help": "Quand votre application de synchronisation tarde, Sioul les lit aussi sur le serveur.",
                "share-send-again": "Tout renvoyer",
                "share-send-again-help": "Envoie chaque fichier de cet appareil que le serveur n’a pas, ou a autrement.",
                "share-send-again-sending": "Envoi des fichiers de cet appareil…"
            }
        })
        readonly property var said: ({
            "en": {
                build: "This device: Sioul 0.0.3 (eff8661abcde).",
                sendSwitch: "Also send this device's changes to cloud.example.org directly",
                sendLine: "Sent directly to cloud.example.org, last at 10:41.",
                again: "Sent 14 files to cloud.example.org at 10:42."
            },
            "fr": {
                build: "Cet appareil : Sioul 0.0.3 (eff8661abcde).",
                sendSwitch: "Envoyer aussi les changements de cet appareil directement à cloud.example.org",
                sendLine: "Envoyé directement à cloud.example.org, la dernière fois à 10:41.",
                again: "14 fichiers envoyés à cloud.example.org à 10:42."
            }
        })

        signal shareListed(string versions)
        signal shareWithheldBack(string problem)
        signal sharePutBackDone(string problem)
        signal shareEstimated(string estimate)
        signal shareStarted(string problem)

        property int takenOut: 0
        function takeOutOldFiles() {
            mock.takenOut += 1
        }
        function reminderOpened(kind, uri, key) {
            mock.opened = mock.opened.concat([kind])
        }
        function text(key) {
            return mock.words[mock.language][key] || key
        }
        function textWith(key, name, value) {
            return mock.text(key) + " " + value
        }
        function textArgs(key, args) {
            return mock.text(key) + " " + args
        }
        function filesAccess() {
            return true
        }
        function viewFlag(flag) {
            return false
        }
        function shareCandidates() {
            return "[]"
        }
        function shareStatus(folder) {
            const said = mock.said[mock.language]
            return JSON.stringify({
                on: true,
                folder: "~/Nextcloud/Documents/Sioul",
                sealed: true,
                lines: [],
                problems: [],
                parts: [],
                vanished: [],
                withheld: mock.held ? [{ store: "files/notes/", text: "23 files went from your notes here at once on Friday 2 October, most of them from ~/Notes/Archives.", leaves: "They leave your other devices by themselves 14 days after they went, by Friday 16 October at the latest, unless they come back here first. Nothing has to be done.", key: "porch-withheld-said:files/notes/:1790000000000", size: mock.leaving ? "Their contents in the folder: 2.3 MB." : "" }] : [],
                leaving: mock.leaving ? [
                    { text: "The contents of 3 notes and papers taken out, 12.4 MB, leave once The phone (FP5) has read that they were. The phone (FP5) has been silent since Sat 3 Oct 09:00: if it is no longer in use, “Forget it and remove its files…” lets them go at the next exchange.", device: "a" },
                    { text: "2 earlier versions and contents used lately, 1.1 MB, leave two weeks after their last use.", device: "" }
                ] : [],
                gone_devices: mock.held ? [{ id: "c", name: "The phone (Model 7)", state: "Forgotten here. Its files are still in the folder: 16.1 MB." }] : [],
                backup: { shown: true, line: "Also fetched directly from cloud.example.org, last at 10:41.", on: true, given: "", found: true, send: !mock.sendsOff(), send_switch: said.sendSwitch, send_line: said.sendLine },
                servers: [],
                mirrored: false,
                build: said.build,
                can_again: true,
                again: mock.sent ? said.again : ""
            })
        }
        function sendsOff() {
            return mock.sends.length > 0 && mock.sends[mock.sends.length - 1] === false
        }
        function setShareSend(on) {
            mock.sends = mock.sends.concat([on])
            return ""
        }
        function shareSendAgain() {
            mock.again += 1
            mock.sent = true
        }
        function sharePutBackWithheld(store) {
            mock.asked = mock.asked.concat(["back " + store])
        }
        function shareTakeOutWithheld(store) {
            mock.asked = mock.asked.concat(["out " + store])
            return ""
        }
        function deviceFiles(id) {
            mock.asked = mock.asked.concat(["files " + id])
            return JSON.stringify({ text: "What goes: its records (17 files) and the small files it kept there, 16.1 MB. What stays: everything your devices already read of it.", live: id !== "c" })
        }
        function deviceRemoveFiles(id) {
            mock.asked = mock.asked.concat(["remove " + id])
            return ""
        }
    }

    Flickable {
        anchors.fill: parent
        contentHeight: panel.implicitHeight

        SharePanel {
            id: panel

            width: root.width - 32
            x: 16
            sioul: mock
            theme: testTheme
        }
    }

    TestCase {
        name: "SharePanel"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const children = item.children || []
            for (let i = 0; i < children.length; ++i)
                walk(children[i], test, found)
            if (item.contentItem && item.contentItem !== item && children.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            return found
        }
        function shown(text) {
            return walk(panel, i => i.visible && i.text !== undefined && i.text === text && i.checked === undefined, [])[0] || null
        }
        function button(text) {
            return walk(panel, i => i.visible && i.text === text && i.clicked !== undefined && i.checked !== undefined && !i.checkable, [])[0] || null
        }
        // The switch beside a label: the one in the same row.
        function switchBeside(text) {
            const label = shown(text)
            let row = label
            while (row && row.parent && walk(row, i => i.checkable === true && i.toggle !== undefined, []).length === 0)
                row = row.parent
            return row ? walk(row, i => i.checkable === true && i.toggle !== undefined, [])[0] : null
        }
        function save(name) {
            if (root.pictures === "")
                return
            wait(100)
            grabImage(root).save(root.pictures + "/" + name + ".png")
        }

        function init() {
            mock.sends = []
            mock.again = 0
            mock.sent = false
            mock.held = false
            mock.leaving = false
            mock.asked = []
        }

        // The notes folder is the Notes page's: one line says which it is (or that there
        // is none), with a button to its row there, by its key.
        // Where Sioul keeps its lists: one line, and the window of the move from it at any time.
        function test_the_lists_line_opens_the_window_of_the_move() {
            mock.opened = []
            const line = walk(panel, i => i.objectName === "shareStorage", [])[0]
            compare(line.text, mock.text("storage-line-not"))
            mock.storageMove = JSON.stringify({ moved: true, papers: "/stand-in/Papers", letters: "/stand-in/Letters" })
            verify(line.text.indexOf("/stand-in/Papers") >= 0 && line.text.indexOf("/stand-in/Letters") >= 0, line.text)
            walk(panel, i => i.objectName === "shareStorageOpen", [])[0].clicked()
            compare(mock.opened, ["storage-move"])
            mock.storageMove = ""
        }

        // The old files: never taken out by themselves; the line, its count, greyed and naming
        // who is still to move until every device moved; then the press asks it, once.
        function test_the_old_files_leave_on_the_press_alone() {
            mock.takenOut = 0
            const line = walk(panel, i => i.objectName === "shareOldFiles", [])[0]
            mock.storageMove = JSON.stringify({ moved: true, old_files: 0 })
            verify(!line.visible, "nothing left: no line")
            mock.storageMove = JSON.stringify({ moved: true, old_files: 12, take_out_open: false, take_out_waiting: "storage-old-files-waiting Laptop" })
            verify(line.visible)
            compare(walk(panel, i => i.objectName === "shareOldFilesLine", [])[0].text, mock.text("storage-old-files") + " " + JSON.stringify({ count: 12 }))
            const waiting = walk(panel, i => i.objectName === "shareOldFilesWaiting", [])[0]
            verify(waiting.visible && waiting.text.indexOf("Laptop") >= 0, waiting.text)
            const button = walk(panel, i => i.objectName === "shareOldFilesTakeOut", [])[0]
            verify(!button.enabled, "greyed until every device moved")
            mock.storageMove = JSON.stringify({ moved: true, old_files: 12, take_out_open: true, take_out_waiting: "" })
            verify(button.enabled && !waiting.visible)
            compare(mock.takenOut, 0, "nothing by itself")
            button.clicked()
            compare(mock.takenOut, 1)
            mock.storageMove = JSON.stringify({ moved: true, old_files: 0, taken: "storage-taken-out" })
            verify(!line.visible)
            verify(walk(panel, i => i.objectName === "shareOldFilesTaken", [])[0].visible)
            mock.storageMove = ""
        }

        // The notes folder said, as where notes travel from; set on the Notes page alone: no door here.
        function test_the_notes_folder_line_says_where_notes_travel_from() {
            const line = walk(panel, i => i.objectName === "shareNotesFolder", [])[0]
            verify(line.visible)
            verify(line.text.indexOf("/stand-in/Notes") >= 0, line.text)
            mock.notesFolder = JSON.stringify({ state: "unset" })
            verify(line.text.indexOf("/stand-in/Notes") < 0 && line.text !== "", line.text)
            compare(walk(panel, i => i.objectName === "shareNotesFolderOpen", []).length, 0, "no door to the Notes page")
            mock.notesFolder = JSON.stringify({ state: "ready", path: "/stand-in/Notes" })
        }

        // Notes held after going many at once, said with what can be done;
        // a device that no longer counts with "Remove its files…", and one
        // that counts with "Forget it and remove its files…": what goes and
        // what stays said first, then the press, or Cancel.
        function test_held_notes_and_a_device_s_files() {
            mock.language = "en"
            mock.held = true
            panel.reload()
            wait(50)
            verify(shown("23 files went from your notes here at once on Friday 2 October, most of them from ~/Notes/Archives.") !== null, "held notes, said")
            // Put back off the window's thread: the buttons wait for its answer.
            button("porch-withheld-put-back").clicked()
            compare(mock.asked, ["back files/notes/"])
            verify(button("porch-withheld-putting-back") !== null && !button("porch-withheld-putting-back").enabled, "said while it runs")
            mock.shareWithheldBack("")
            tryVerify(() => button("porch-withheld-put-back") !== null && button("porch-withheld-put-back").enabled, 2000)
            button("porch-withheld-take-out").clicked()
            compare(mock.asked, ["back files/notes/", "out files/notes/"])
            verify(shown("share-devices-gone") !== null, "the heading of the devices that no longer count")
            verify(shown("Forgotten here. Its files are still in the folder: 16.1 MB.") !== null)
            const what = "What goes: its records (17 files) and the small files it kept there, 16.1 MB. What stays: everything your devices already read of it."
            verify(shown(what) === null, "nothing said before the press")
            mock.asked = []
            button("share-device-remove").clicked()
            tryVerify(() => shown(what) !== null, 2000, "what goes and what stays, said first")
            verify(button("share-device-remove") === null, "its button gives way to the question")
            compare(mock.asked, ["files c"])
            button("ui-cancel").clicked()
            tryVerify(() => shown(what) === null, 2000, "Cancel: nothing removed")
            compare(mock.asked, ["files c"])
            button("share-device-remove").clicked()
            tryVerify(() => button("share-device-remove-confirm") !== null, 2000)
            button("share-device-remove-confirm").clicked()
            compare(mock.asked, ["files c", "files c", "remove c"])
            // The devices that count are listed in their own tab: none here, and no line says so.
            const live = walk(panel, i => i.visible && i.text === "share-device-remove-live" && i.clicked !== undefined, [])
            compare(live.length, 0, "the devices that count have their own tab")
            compare(walk(panel, i => i.objectName === "shareToDevices", []).length, 0, "no line that only leads to the devices' tab")
            save("share-held-and-removal")
        }

        // What waits to leave the server, and why, with sizes and never a
        // file's name (docs/database.md, "Old contents go"): the device it
        // waits for with "Forget it and remove its files…", asked there, the
        // question said once; the held notes' size under their lines.
        function test_what_waits_to_leave_the_server() {
            mock.language = "en"
            mock.held = true
            mock.leaving = true
            panel.reload()
            wait(50)
            verify(shown("share-leaving-title") !== null, "its heading")
            const waiting = "The contents of 3 notes and papers taken out, 12.4 MB, leave once The phone (FP5) has read that they were. The phone (FP5) has been silent since Sat 3 Oct 09:00: if it is no longer in use, “Forget it and remove its files…” lets them go at the next exchange."
            verify(shown(waiting) !== null, "the device it waits for, named")
            verify(shown("2 earlier versions and contents used lately, 1.1 MB, leave two weeks after their last use.") !== null)
            verify(shown("Their contents in the folder: 2.3 MB.") !== null, "the held notes' size")
            const live = walk(panel, i => i.visible && i.text === "share-device-remove-live" && i.clicked !== undefined, [])
            compare(live.length, 1, "where it waits; the devices that count have their own tab")
            live[0].clicked()
            const what = "What goes: its records (17 files) and the small files it kept there, 16.1 MB. What stays: everything your devices already read of it."
            tryVerify(() => shown(what) !== null, 2000, "what goes and what stays, said first")
            compare(walk(panel, i => i.visible && i.text === "share-device-remove-confirm" && i.clicked !== undefined && i.checked !== undefined, []).length, 1, "asked once, where it was pressed")
            compare(mock.asked, ["files a"])
            button("share-device-remove-confirm").clicked()
            compare(mock.asked, ["files a", "remove a"])
            save("share-leaving")
        }

        function test_the_build_the_switch_and_send_everything_again_data() {
            return [{ tag: "en", language: "en" }, { tag: "fr", language: "fr" }]
        }

        function test_the_build_the_switch_and_send_everything_again(data) {
            mock.language = data.language
            panel.reload()
            const said = mock.said[data.language]
            wait(50)
            // This device's build; each other device's is said in the devices' tab.
            verify(shown(said.build) !== null, "this device's build")
            // The switch, on by default where the backup is, and where it stands.
            const sending = switchBeside(said.sendSwitch)
            verify(sending !== null, "the switch to send beside the sync app")
            verify(sending.checked)
            verify(shown(said.sendLine) !== null)
            verify(shown(mock.text("share-send-help")) !== null)
            save("share-" + data.language)
            // Switched off: said to Sioul.
            sending.toggle()
            sending.toggled()
            compare(mock.sends, [false])
            // "Send everything again": asked once, "sending" meanwhile, then what it did.
            const again = button(mock.text("share-send-again"))
            verify(again !== null, "Send everything again")
            again.clicked()
            compare(mock.again, 1)
            verify(shown(mock.text("share-send-again-sending")) !== null)
            tryVerify(() => shown(said.again) !== null, 8000, "what it did, said, as soon as it came")
            verify(shown(mock.text("share-send-again-help")) !== null)
            save("share-again-" + data.language)
        }
    }
}
