// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Your folder and sharing (SharePanel.qml), sharing on, on a
// stand-in for Sioul: this device's build, each other device's build (an
// older one said calmly), the switch that sends this device's files to the
// server beside the sync app, and "Send everything again" with what it said.
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
