// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Joining your other devices through your Nextcloud's own sign-in, in
// Settings ▸ Your folder and sharing (SharePanel.qml, JoinPanel.qml), on a
// stand-in for Sioul: nothing under way, Murena's sign-in and another
// Nextcloud's address offered; signing in, the wait with Cancel and the page
// again, the two ways hidden meanwhile; the folder found, joined with the
// passphrase typed once; none found, a sharing started there with it typed
// twice; joined, what came and what still needs a sign-in, then Done. A
// sharing device's sights: on a phone, whether it replaces another phone,
// asked; on a computer, what a new device needs, and a phone silent a day
// forgotten now. The sign-in itself is tested in sioul-sync (loginflow.rs,
// remote.rs) and join.rs. In English and French. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 900
    height: 1400

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property string language: "en"
        property var stand: ({ step: "" })
        property var calls: []
        property var shareState: ({ on: false })
        // Made up: no real server, no real device.
        readonly property var words: ({
            "en": {
                "join-title": "Join your other devices",
                "join-murena": "Sign in to Murena",
                "join-other": "Sign in to another Nextcloud",
                "join-sign-in": "Sign in",
                "join-signing": "Sign in to { $host } in your browser, then come back here. Sioul waits twenty minutes at most.",
                "join-open-again": "Open the page again",
                "join-found": "Your other devices share through { $place } there. Type the passphrase you chose on them.",
                "join-none": "No folder shared by Sioul was found there.",
                "join-join": "Join",
                "join-look-again": "Look again",
                "join-start": "Start sharing there",
                "join-done": "Done",
                "join-needs-title": "Still to sign in on this device:",
                "join-replace-yes": "Yes, it replaces it",
                "join-replace-no": "No, it is a second phone",
                "share-mode-folder": "Through a folder your sync app carries",
                "share-add-title": "Add a device",
                "share-device-forget-now": "Forget it now",
                "share-device-forget-now-help": "Silent for a day or more. Forgotten, it no longer holds your doses’ reminders back here; it counts again if it ever shares again, and nothing of it is deleted.",
                "appnotes-open-info": "Sioul's app info",
                "texts-setup-ask": "Allow in Android…",
                "ui-cancel": "Cancel"
            },
            "fr": {
                "join-title": "Rejoindre vos autres appareils",
                "join-murena": "Se connecter à Murena",
                "join-other": "Se connecter à un autre Nextcloud",
                "join-sign-in": "Se connecter",
                "join-signing": "Connectez-vous à { $host } dans votre navigateur, puis revenez ici. Sioul attend vingt minutes au plus.",
                "join-open-again": "Rouvrir la page",
                "join-found": "Vos autres appareils partagent par { $place } là-bas. Tapez la phrase de passe choisie sur eux.",
                "join-none": "Aucun dossier partagé par Sioul n’a été trouvé là-bas.",
                "join-join": "Rejoindre",
                "join-look-again": "Chercher de nouveau",
                "join-start": "Commencer à partager là-bas",
                "join-done": "Terminé",
                "join-needs-title": "Reste à vous connecter sur cet appareil :",
                "join-replace-yes": "Oui, il le remplace",
                "join-replace-no": "Non, c’est un second téléphone",
                "share-mode-folder": "Par votre application de synchronisation",
                "share-add-title": "Ajouter un appareil",
                "share-device-forget-now": "L’oublier maintenant",
                "share-device-forget-now-help": "Silencieux depuis un jour ou plus.",
                "appnotes-open-info": "Infos de l’application Sioul",
                "texts-setup-ask": "Autoriser dans Android…",
                "ui-cancel": "Annuler"
            }
        })

        signal joinChanged
        signal shareListed(string versions)
        signal sharePutBackDone(string problem)
        signal shareEstimated(string estimate)
        signal shareStarted(string problem)

        function text(key) {
            return mock.words[mock.language][key] || key
        }
        function textWith(key, name, value) {
            return mock.text(key).replace("{ $" + name + " }", value)
        }
        function viewFlag(flag) {
            return false
        }
        function filesAccess() {
            return true
        }
        function shareCandidates() {
            return "[]"
        }
        function shareStatus(folder) {
            return JSON.stringify(Object.assign({ folder: "", sealed: false, lines: [], problems: [], parts: [], vanished: [], devices: [], backup: {}, servers: [], build: "", add_device: [], replace: null }, mock.shareState))
        }
        function joinStatus() {
            return JSON.stringify(mock.stand)
        }
        function said(call) {
            mock.calls = mock.calls.concat([call])
        }
        function joinSignIn(server) {
            mock.said("sign-in " + server)
        }
        function joinCancel() {
            mock.said("cancel")
        }
        function joinOpenAgain() {
            mock.said("open again")
        }
        function joinLookAgain() {
            mock.said("look again")
        }
        function joinFolder(place, passphrase, again) {
            mock.said(["folder", place, passphrase, again].join("|"))
        }
        function joinDone() {
            mock.said("done")
        }
        function joinReplace(device, yes) {
            mock.said("replace " + device + " " + yes)
            return ""
        }
        function deviceForget(device) {
            mock.said("forget " + device)
            return ""
        }
        // What Android withholds on this phone (access.rs's lines), and the presses for it.
        property var withheld: []
        function texts(verb, json) {
            const args = JSON.parse(json)
            if (verb !== "access")
                mock.said(verb + (args.what ? " " + args.what : ""))
            return JSON.stringify({ lines: mock.withheld })
        }
        // join.rs moved on: the panel reads it again.
        function move(stand) {
            mock.stand = stand
            mock.joinChanged()
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
        name: "JoinPanel"
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
        // Shown, as a person sees it: itself and every item holding it visible.
        function seen(item) {
            for (let at = item; at && at !== panel; at = at.parent) {
                if (!at.visible)
                    return false
            }
            return true
        }
        function shown(text) {
            return walk(panel, i => i.text !== undefined && i.text === text && i.checked === undefined && seen(i), [])[0] || null
        }
        function button(text) {
            return walk(panel, i => i.text === text && i.clicked !== undefined && i.checked !== undefined && !i.checkable && seen(i), [])[0] || null
        }
        function checkable(text) {
            return walk(panel, i => i.text === text && i.checkable === true && seen(i), [])[0] || null
        }
        function fields() {
            return walk(panel, i => i.echoMode !== undefined && i.selectAll !== undefined && seen(i), [])
        }
        function press(text) {
            const found = button(text)
            verify(found !== null, text)
            found.clicked()
        }

        function init() {
            mock.language = "en"
            panel.join.other = false
            panel.join.otherPlace = false
            mock.calls = []
            mock.shareState = { on: false }
            mock.move({ step: "" })
            panel.reload()
        }

        function test_nothing_under_way_offers_murena_and_another_nextcloud_data() {
            return [{ tag: "en", language: "en" }, { tag: "fr", language: "fr" }]
        }

        function test_nothing_under_way_offers_murena_and_another_nextcloud(data) {
            mock.language = data.language
            panel.reload()
            verify(shown(mock.text("join-title")) !== null)
            // Beside the two ways.
            verify(checkable(mock.text("share-mode-folder")) !== null, "the two ways still offered")
            press(mock.text("join-murena"))
            compare(mock.calls, ["sign-in murena"])
            // Another Nextcloud: its address asked, then signed in to.
            press(mock.text("join-other"))
            const address = walk(panel, i => i.placeholderText === "cloud.example.org" && seen(i), [])[0]
            verify(address !== undefined, "its address asked")
            address.text = "cloud.example.org"
            press(mock.text("join-sign-in"))
            compare(mock.calls, ["sign-in murena", "sign-in cloud.example.org"])
        }

        function test_signing_in_waits_with_cancel_the_two_ways_hidden() {
            mock.move({ step: "signing", host: "murena.io" })
            verify(shown("Sign in to murena.io in your browser, then come back here. Sioul waits twenty minutes at most.") !== null)
            verify(checkable(mock.text("share-mode-folder")) === null, "the two ways wait")
            press("Open the page again")
            press("Cancel")
            compare(mock.calls, ["open again", "cancel"])
        }

        function test_the_folder_found_is_joined_with_the_passphrase_typed_once() {
            mock.move({ step: "found", host: "murena.io", signed: "Signed in to murena.io as jane.", places: ["Documents/Sioul"] })
            verify(shown("Signed in to murena.io as jane.") !== null)
            verify(shown("Your other devices share through Documents/Sioul there. Type the passphrase you chose on them.") !== null)
            const typed = fields()
            compare(typed.length, 1, "the passphrase once")
            verify(button("Join") !== null && !button("Join").enabled, "nothing typed yet")
            typed[0].text = "quiet harbour at dawn"
            press("Join")
            compare(mock.calls, ["folder|Documents/Sioul|quiet harbour at dawn|"])
        }

        // The look could not go through (M3): never "none found", never a new sharing;
        // look again, or its place typed and the passphrase once.
        function test_a_look_that_could_not_go_through_offers_no_new_sharing() {
            mock.move({ step: "unchecked", host: "murena.io", signed: "Signed in to murena.io as jane.", places: [], problem: "Sioul could not look through all your files on murena.io." })
            verify(shown("Sioul could not look through all your files on murena.io.") !== null)
            verify(button("Start sharing there") === null, "no new sharing offered")
            const typed = fields()
            compare(typed.length, 2, "its place, the passphrase once")
            typed[1].text = "quiet harbour at dawn"
            press("Join")
            press("Look again")
            compare(mock.calls, ["folder|Documents/Sioul|quiet harbour at dawn|", "look again"])
        }

        function test_none_found_starts_sharing_there_with_the_passphrase_twice() {
            mock.move({ step: "none", host: "cloud.example.org", signed: "Signed in to cloud.example.org as jane.", places: [] })
            verify(shown("No folder shared by Sioul was found there.") !== null)
            const typed = fields()
            compare(typed.length, 3, "its place, the passphrase, once more")
            compare(typed[0].text, "Documents/Sioul")
            typed[1].text = "quiet harbour at dawn"
            typed[2].text = "quiet harbour at dawn"
            press("Start sharing there")
            compare(mock.calls, ["folder|Documents/Sioul|quiet harbour at dawn|quiet harbour at dawn"])
        }

        function test_joined_says_what_came_and_what_still_needs_a_sign_in() {
            mock.move({ step: "joined", lines: ["This device now shares with desk.", "Your settings came with them."], needs: ["jane@example.org: its password, in Accounts."] })
            for (const line of ["This device now shares with desk.", "Your settings came with them.", "Still to sign in on this device:", "jane@example.org: its password, in Accounts."])
                verify(shown(line) !== null, line)
            press("Done")
            compare(mock.calls, ["done"])
        }

        // A phone joined while Android withholds what Sioul needs here: each said, with what Android wants.
        function test_joined_says_what_android_has_not_allowed_yet() {
            mock.withheld = [{ what: "texts", said: "Texts: Android keeps them from Sioul until you allow its restricted settings.", restricted: true }]
            mock.move({ step: "joined", lines: ["This device now shares with desk."], needs: [] })
            verify(shown("Texts: Android keeps them from Sioul until you allow its restricted settings.") !== null)
            press("Sioul's app info")
            press("Allow in Android…")
            compare(mock.calls, ["access-info", "access-ask texts"])
            mock.withheld = []
        }

        function test_a_phone_is_asked_whether_it_replaces_another() {
            mock.shareState = { on: true, replace: { id: "old-phone", ask: "Does this phone replace Moss, silent since Sat 10 Oct 21:40?" } }
            panel.reload()
            verify(shown("Does this phone replace Moss, silent since Sat 10 Oct 21:40?") !== null)
            press("Yes, it replaces it")
            compare(mock.calls, ["replace old-phone true"])
            mock.calls = []
            press("No, it is a second phone")
            compare(mock.calls, ["replace old-phone false"])
        }

        function test_a_computer_says_what_a_new_device_needs_and_forgets_a_silent_phone_now() {
            const lines = ["On the new device, open Sioul.", "Choose “Sign in to Murena”.", "The folder there, Documents/Sioul, is found by itself.", "Then type the passphrase: the one you chose for this sharing. Sioul never shows it."]
            mock.shareState = { on: true, add_device: lines, devices: [{ id: "phone-id", name: "The phone (Moss)", state: "Silent since Sat 10 Oct 21:40.", notes: [], off: false, forget: false, forget_now: true, build: "" }] }
            panel.reload()
            // What a new device needs, on a press: never the passphrase.
            verify(shown(lines[0]) === null, "said on a press")
            const adding = checkable("Add a device")
            verify(adding !== null)
            adding.toggle()
            adding.toggled()
            for (const line of lines)
                verify(shown(line) !== null, line)
            // A phone silent a day: what forgetting it changes, then the button.
            verify(shown(mock.text("share-device-forget-now-help")) !== null)
            press("Forget it now")
            compare(mock.calls, ["forget phone-id"])
        }
    }
}
