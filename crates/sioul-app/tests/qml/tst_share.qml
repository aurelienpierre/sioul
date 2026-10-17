// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Your folder and sharing (SharePanel.qml), sharing on, on a
// stand-in for Sioul: this device's build, each other device's build (an
// older one said calmly), the switch that sends this device's files to the
// server beside the sync app, and "Send everything again" with what it said;
// notes held after going many at once, with Put them back and Take them out
// now; "Remove its files" for a device, what goes said before the press.
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
        property var sends: []
        property int again: 0
        property bool sent: false
        // Notes held after going many at once, and a device that no longer counts (`test_held_notes_and_a_device_s_files`).
        property bool held: false
        property var asked: []
        // Made up: no real device, no real server.
        readonly property var words: ({
            "en": {
                "share-now": "Exchange now",
                "share-stop": "Stop sharing",
                "share-devices": "Your other devices",
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
                "share-devices": "Vos autres appareils",
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
                phone: "Sioul 0.0.2 (0123456789ab), older than this device's (0.0.3 (eff8661abcde)): updating it there, when you can, keeps both alike.",
                laptop: "Sioul 0.0.3 (eff8661abcde).",
                sendSwitch: "Also send this device's changes to cloud.example.org directly",
                sendLine: "Sent directly to cloud.example.org, last at 10:41.",
                again: "Sent 14 files to cloud.example.org at 10:42."
            },
            "fr": {
                build: "Cet appareil : Sioul 0.0.3 (eff8661abcde).",
                phone: "Sioul 0.0.2 (0123456789ab), plus ancien que celui de cet appareil (0.0.3 (eff8661abcde)) : le mettre à jour là-bas, quand vous pouvez, garde les deux pareils.",
                laptop: "Sioul 0.0.3 (eff8661abcde).",
                sendSwitch: "Envoyer aussi les changements de cet appareil directement à cloud.example.org",
                sendLine: "Envoyé directement à cloud.example.org, la dernière fois à 10:41.",
                again: "14 fichiers envoyés à cloud.example.org à 10:42."
            }
        })

        signal shareListed(string versions)
        signal sharePutBackDone(string problem)
        signal shareEstimated(string estimate)
        signal shareStarted(string problem)

        function text(key) {
            return mock.words[mock.language][key] || key
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
                withheld: mock.held ? [{ store: "files/notes/", text: "23 files went from your notes here at once on Friday 2 October, most of them from ~/Notes/Archives.", leaves: "They leave your other devices by themselves 14 days after they went, by Friday 16 October at the latest, unless they come back here first. Nothing has to be done.", key: "porch-withheld-said:files/notes/:1790000000000" }] : [],
                gone_devices: mock.held ? [{ id: "c", name: "The phone (Model 7)", state: "Forgotten here. Its files are still in the folder: 16.1 MB." }] : [],
                devices: [
                    { id: "a", name: "The phone (FP5)", state: "In use now; last shared 10:41.", notes: [], off: false, forget: false, build: said.phone },
                    { id: "b", name: "laptop", state: "Closed 09:12; last shared 09:12.", notes: [], off: false, forget: false, build: said.laptop }
                ],
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
            return ""
        }
        function shareConfirmGone(store) {
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
            mock.asked = []
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
            button("porch-withheld-put-back").clicked()
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
            // A device that counts: on its own line, the same question.
            mock.asked = []
            const live = walk(panel, i => i.visible && i.text === "share-device-remove-live" && i.clicked !== undefined, [])
            compare(live.length, 2, "one for each device that counts")
            live[0].clicked()
            tryVerify(() => shown(what) !== null, 2000)
            compare(mock.asked, ["files a"])
            button("share-device-remove-confirm").clicked()
            compare(mock.asked, ["files a", "remove a"])
            save("share-held-and-removal")
        }

        function test_the_build_the_switch_and_send_everything_again_data() {
            return [{ tag: "en", language: "en" }, { tag: "fr", language: "fr" }]
        }

        function test_the_build_the_switch_and_send_everything_again(data) {
            mock.language = data.language
            panel.reload()
            const said = mock.said[data.language]
            wait(50)
            // This device's build, each other device's, an older one said.
            verify(shown(said.build) !== null, "this device's build")
            verify(shown(said.phone) !== null, "the phone's older build, said")
            verify(shown(said.laptop) !== null, "the laptop's build")
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
