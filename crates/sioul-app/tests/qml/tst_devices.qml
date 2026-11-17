// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Devices (DevicesTab.qml), on a stand-in for Sioul: this device
// first, each device's fields as the core words them; what a device is for
// as three switches, nothing ticked being everything; a name given, and
// taken back; Count it again, Forget, Forget it now, and "Forget it and
// remove its files…" with what goes said first; this device's own kind and
// its parts' link; sharing off, this device alone. In a wide window the
// fields line up as a table under their titles; at a phone's width (412
// points) each device is a card and nothing runs past the edge. The rows
// themselves are tested in sioul-sync (device_table.rs). Run by
// tools/qml-test.sh. To see the tab, set `pictures` below to a folder (a
// change of your own, not to commit).

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    // Where the pictures go; none when empty.
    readonly property string pictures: ""

    width: 1100
    height: 1500

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // Sharing on, why not when a folder is set all the same, and what was asked of Sioul, in order.
        property bool on: true
        property string why: ""
        // The sentence said while the part "settings" does not travel from here.
        property string stays: ""
        property var asked: []
        // What you said of each device, as the settings hold it: id → { name, area }.
        property var said: ({ "desk": { name: "Work desk", area: "work" } })
        // Made up: no real device.
        readonly property var known: [
            { id: "desk", here: true, system: "desk-host", form: "computer", form_words: "A computer", code: "in-use", state: "In use now.", notes: [], opened: "09:12", shared: "10:41", version: "0.0.3 (eff8661abcde)", older: false, build: "Sioul 0.0.3 (eff8661abcde).", parts: ["settings", "health"], parts_words: "Settings and accounts, Health", off: false, forget: false, forget_now: false },
            { id: "laptop", here: false, system: "laptop-host", form: "computer", form_words: "A computer", code: "off", state: "Counted as off, as you said, until it shares again.", notes: ["It does not share its doses."], opened: "Fri 2 Oct 08:00", shared: "Fri 2 Oct 08:30", version: "0.0.3 (eff8661abcde)", older: false, build: "Sioul 0.0.3 (eff8661abcde).", parts: [], parts_words: "Its Sioul does not say what it shares.", off: true, forget: true, forget_now: false },
            { id: "phone", here: false, system: "The phone (Moss)", form: "phone", form_words: "A phone", code: "closed", state: "Closed since Sat 10 Oct 21:40.", notes: [], opened: "Sat 10 Oct 21:00", shared: "Read the others Sat 10 Oct 21:39; wrote its own Sat 10 Oct 21:40.", version: "0.0.2 (0123456789ab)", older: true, build: "Sioul 0.0.2 (0123456789ab), older than this device's (0.0.3 (eff8661abcde)): updating it there, when you can, keeps both alike.", parts: ["settings"], parts_words: "Settings and accounts", off: false, forget: false, forget_now: true }
        ]

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
        function devices() {
            const rows = (mock.on ? mock.known : mock.known.slice(0, 1)).map(row => {
                const word = mock.said[row.id] || {}
                return Object.assign({}, row, { given: word.name || "", name: word.name || row.system, area: word.area || "", area_words: word.area || "Everything comes here" })
            })
            return JSON.stringify({ on: mock.on, why: mock.why, stays: mock.stays, rows: rows })
        }
        function deviceSay(id, field, value) {
            mock.asked = mock.asked.concat([["say", id, field, value].join(" ").trim()])
            const said = Object.assign({}, mock.said)
            said[id] = Object.assign({}, said[id] || {})
            said[id][field] = value
            mock.said = said
            return ""
        }
        function deviceOff(id, off) {
            mock.asked = mock.asked.concat(["off " + id + " " + off])
            return ""
        }
        function deviceForget(id) {
            mock.asked = mock.asked.concat(["forget " + id])
            return ""
        }
        function deviceFiles(id) {
            mock.asked = mock.asked.concat(["files " + id])
            return JSON.stringify({ text: "What goes: its records (17 files), 16.1 MB. What stays: everything your devices already read of it.", live: true })
        }
        function deviceRemoveFiles(id) {
            mock.asked = mock.asked.concat(["remove " + id])
            return ""
        }
    }

    property var saved: []
    // The setting of what this device is, as the settings' rows give it.
    readonly property var kind: ({ key: "device_kind", kind: "choice", label: "What this device is", help: "A phone is carried.", value: "", choices: [{ value: "", label: "Found by itself (a computer)" }, { value: "phone", label: "A phone" }, { value: "tablet", label: "A tablet" }, { value: "computer", label: "A computer" }] })

    Flickable {
        id: flick

        anchors.fill: parent
        contentHeight: tab.implicitHeight

        DevicesTab {
            id: tab

            width: flick.width - 32
            x: 16
            sioul: mock
            theme: testTheme
            kindRow: root.kind
            onSave: (key, value, shown) => root.saved = root.saved.concat([[key, value]])
        }
    }

    SignalSpy {
        id: sharingSpy

        target: tab
        signalName: "sharingAsked"
    }

    TestCase {
        name: "DevicesTab"
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
        // Whether an item shows: itself and everything it is in.
        function showing(item) {
            for (let i = item; i; i = i.parent) {
                if (i.visible === false)
                    return false
                if (i === tab)
                    return true
            }
            return true
        }
        function card(id) {
            return walk(tab, i => i.objectName === "device-" + id, [])[0] || null
        }
        function shown(within, text) {
            return walk(within, i => showing(i) && i.text !== undefined && i.text === text && i.clicked === undefined, [])[0] || null
        }
        function button(within, text) {
            return walk(within, i => showing(i) && i.text === text && i.clicked !== undefined, [])[0] || null
        }
        function named(within, name) {
            return walk(within, i => i.objectName === name, [])[0] || null
        }
        function boxes(within) {
            return walk(named(within, "deviceFor"), i => i.checkable === true && i.toggled !== undefined, [])
        }
        function save(name) {
            if (root.pictures === "")
                return
            wait(100)
            grabImage(root).save(root.pictures + "/" + name + ".png")
        }

        function init() {
            root.width = 1100
            mock.on = true
            mock.why = ""
            mock.stays = ""
            mock.asked = []
            mock.said = { "desk": { name: "Work desk", area: "work" } }
            root.saved = []
            tab.renaming = ""
            tab.nameDraft = ""
            tab.removing = ""
            tab.kindOpen = false
            tab.problem = ""
            tab.reload()
            sharingSpy.clear()
            wait(50)
        }

        // This device first, then the others; each field as the core words it;
        // in a wide window, as a table under its titles.
        function test_each_device_with_its_fields_this_one_first() {
            verify(tab.wide, "a wide window: the table")
            compare(tab.rows.map(r => r.id), ["desk", "laptop", "phone"])
            const cards = walk(tab, i => (i.objectName || "").startsWith("device-"), [])
            compare(cards.map(c => c.objectName), ["device-desk", "device-laptop", "device-phone"])
            verify(cards[0].y < cards[1].y && cards[1].y < cards[2].y, "this device first")
            // The titles once, above the devices; none repeated in the rows.
            for (const title of tab.titles)
                compare(walk(tab, i => showing(i) && i.text === title, []).length, 1, title)
            const desk = card("desk")
            compare(named(desk, "deviceName").text, "Work desk")
            verify(shown(desk, "devices-this") !== null, "marked as this device")
            verify(shown(desk, "devices-system-name:desk-host") !== null, "what its system calls it, under the name given")
            verify(shown(desk, "A computer") !== null && shown(desk, "In use now.") !== null && shown(desk, "09:12") !== null && shown(desk, "10:41") !== null)
            verify(shown(desk, "0.0.3 (eff8661abcde)") !== null && shown(desk, "Settings and accounts, Health") !== null)
            const phone = card("phone")
            compare(named(phone, "deviceName").text, "The phone (Moss)")
            verify(shown(phone, "devices-this") === null && shown(phone, "devices-system-name:The phone (Moss)") === null, "no name given: its system's alone")
            verify(shown(phone, "Closed since Sat 10 Oct 21:40.") !== null)
            verify(shown(phone, "Read the others Sat 10 Oct 21:39; wrote its own Sat 10 Oct 21:40.") !== null)
            verify(shown(phone, "Sioul 0.0.2 (0123456789ab), older than this device's (0.0.3 (eff8661abcde)): updating it there, when you can, keeps both alike.") !== null, "an older Sioul, said calmly")
            const laptop = card("laptop")
            verify(shown(laptop, "Counted as off, as you said, until it shares again. It does not share its doses.") !== null, "its notes with how it is")
            verify(shown(laptop, "Its Sioul does not say what it shares.") !== null)
            // The fields line up: each device's cell starts where its title does.
            const title = walk(tab, i => showing(i) && i.text === "devices-col-now", [])[0]
            for (const id of ["desk", "phone"]) {
                const cell = shown(card(id), id === "desk" ? "In use now." : "Closed since Sat 10 Oct 21:40.")
                compare(Math.round(cell.mapToItem(tab, 0, 0).x), Math.round(title.mapToItem(tab, 0, 0).x), "under its title: " + id)
            }
            verify(shown(tab, "devices-parts-elsewhere") !== null, "why another device's parts are read only")
            save("devices-table")
        }

        // What a device is for: three switches, written at once for any device;
        // nothing ticked says that everything comes there.
        function test_what_a_device_is_for_is_three_switches() {
            const desk = boxes(card("desk"))
            compare(desk.map(b => b.text), ["area-work", "area-admin", "area-leisure"])
            compare(desk.map(b => b.checked), [true, false, false])
            verify(!showing(named(card("desk"), "deviceForAll")), "something ticked: not everything")
            const phone = card("phone")
            compare(boxes(phone).map(b => b.checked), [false, false, false])
            verify(showing(named(phone, "deviceForAll")), "nothing ticked: everything comes here")
            compare(named(phone, "deviceForAll").text, "devices-for-all")
            // Another device's line, set from this one.
            boxes(phone)[2].toggle()
            boxes(phone)[2].toggled()
            compare(mock.asked, ["say phone area leisure"])
            tryVerify(() => boxes(card("phone")).map(b => b.checked).join() === "false,false,true", 2000)
            verify(!showing(named(card("phone"), "deviceForAll")))
            boxes(card("phone"))[1].toggle()
            boxes(card("phone"))[1].toggled()
            compare(mock.asked[1], "say phone area admin+leisure")
            // This device's own, the last one unticked: unsaid again.
            boxes(card("desk"))[0].toggle()
            boxes(card("desk"))[0].toggled()
            compare(mock.asked[2], "say desk area")
            tryVerify(() => showing(named(card("desk"), "deviceForAll")), 2000, "everything comes here again")
        }

        // "Rename…": the name you give, shown at once; empty, its system's name comes back.
        function test_a_device_is_renamed_and_takes_its_system_s_name_back() {
            const phone = card("phone")
            verify(!showing(named(phone, "deviceNameField")), "no field before the press")
            named(phone, "deviceRename").clicked()
            const field = named(phone, "deviceNameField")
            verify(showing(field))
            compare(field.text, "", "no name given yet")
            compare(field.placeholderText, "The phone (Moss)")
            verify(shown(phone, "devices-rename-help") !== null)
            field.text = "Pocket"
            button(phone, "ui-save").clicked()
            compare(mock.asked, ["say phone name Pocket"])
            tryVerify(() => named(card("phone"), "deviceName").text === "Pocket", 2000)
            verify(shown(card("phone"), "devices-system-name:The phone (Moss)") !== null, "its system's name under yours")
            verify(!showing(named(card("phone"), "deviceNameField")), "the field gone once saved")
            // Cancel writes nothing; the field opens on the name given.
            named(card("phone"), "deviceRename").clicked()
            compare(named(card("phone"), "deviceNameField").text, "Pocket")
            button(card("phone"), "ui-cancel").clicked()
            compare(mock.asked.length, 1)
            // Emptied: its system's name again.
            named(card("phone"), "deviceRename").clicked()
            named(card("phone"), "deviceNameField").text = ""
            button(card("phone"), "ui-save").clicked()
            compare(mock.asked[1], "say phone name")
            tryVerify(() => named(card("phone"), "deviceName").text === "The phone (Moss)", 2000)
        }

        // A name being typed survives anything else pressed in the tab, which
        // reads the devices again and makes every card anew; saving then
        // writes what was typed, never an emptied field (the review of phase 1, R5).
        function test_a_name_being_typed_survives_the_cards_made_anew() {
            named(card("laptop"), "deviceRename").clicked()
            const field = named(card("laptop"), "deviceNameField")
            compare(field.text, "", "no name given yet")
            field.text = "Attic laptop"
            field.textEdited()
            // Something else pressed meanwhile: the phone's Leisure, then this device's kind.
            const before = card("laptop")
            boxes(card("phone"))[2].toggle()
            boxes(card("phone"))[2].toggled()
            tryVerify(() => card("laptop") !== before, 2000, "every card made anew")
            const again = named(card("laptop"), "deviceNameField")
            verify(showing(again), "still renaming")
            compare(again.text, "Attic laptop", "what was typed, kept")
            named(card("desk"), "deviceKind").clicked()
            tab.reload()
            wait(50)
            compare(named(card("laptop"), "deviceNameField").text, "Attic laptop")
            button(card("laptop"), "ui-save").clicked()
            compare(mock.asked, ["say phone area leisure", "say laptop name Attic laptop"])
            tryVerify(() => named(card("laptop"), "deviceName").text === "Attic laptop", 2000)
            // Cancel forgets the draft: the next Rename… starts from the name given.
            named(card("laptop"), "deviceRename").clicked()
            named(card("laptop"), "deviceNameField").text = "Nope"
            named(card("laptop"), "deviceNameField").textEdited()
            button(card("laptop"), "ui-cancel").clicked()
            named(card("laptop"), "deviceRename").clicked()
            compare(named(card("laptop"), "deviceNameField").text, "Attic laptop")
            compare(mock.asked.length, 2)
        }

        // While the part "settings" does not travel from here, the tab says
        // that what is said in it stays here (the review of phase 1, R7).
        function test_what_is_said_here_stays_here_while_settings_do_not_travel() {
            verify(!showing(named(tab, "devicesStay")), "said only while settings do not travel")
            mock.stays = "Settings and accounts do not travel from this device: a name or what a device is for, said here, stays here."
            tab.reload()
            wait(50)
            verify(showing(named(tab, "devicesStay")))
            compare(named(tab, "devicesStay").text, mock.stays)
        }

        // What can be done about another device: count it again, forget it,
        // forget a silent phone now (said first what it changes), and remove its
        // files, what goes and what stays said before the press. None of it
        // for this device.
        function test_another_device_is_counted_forgotten_and_its_files_removed() {
            const laptop = card("laptop")
            button(laptop, "share-device-count-again").clicked()
            compare(mock.asked, ["off laptop false"])
            button(card("laptop"), "share-device-forget").clicked()
            compare(mock.asked[1], "forget laptop")
            const phone = card("phone")
            verify(shown(phone, "share-device-forget-now-help") !== null, "said first what forgetting it now changes")
            verify(button(phone, "share-device-forget") === null, "not silent a week")
            button(phone, "share-device-forget-now").clicked()
            compare(mock.asked[2], "forget phone")
            // Its files: asked first.
            mock.asked = []
            const what = "What goes: its records (17 files), 16.1 MB. What stays: everything your devices already read of it."
            verify(shown(tab, what) === null, "nothing said before the press")
            button(card("phone"), "share-device-remove-live").clicked()
            tryVerify(() => shown(card("phone"), what) !== null, 2000, "what goes and what stays, said first")
            verify(shown(card("laptop"), what) === null, "asked on its own card alone")
            compare(mock.asked, ["files phone"])
            button(card("phone"), "ui-cancel").clicked()
            tryVerify(() => shown(tab, what) === null, 2000, "Cancel: nothing removed")
            compare(mock.asked, ["files phone"])
            button(card("phone"), "share-device-remove-live").clicked()
            tryVerify(() => button(card("phone"), "share-device-remove-confirm") !== null, 2000)
            button(card("phone"), "share-device-remove-confirm").clicked()
            compare(mock.asked, ["files phone", "files phone", "remove phone"])
            // This device: nothing to forget, nothing to remove.
            const desk = card("desk")
            for (const text of ["share-device-count-again", "share-device-forget", "share-device-forget-now", "share-device-remove-live"])
                verify(button(desk, text) === null, text)
        }

        // This device's own: what it is, changed here as in Settings ▸ This
        // device, and its parts, switched in Your folder and sharing.
        function test_this_device_s_kind_and_parts() {
            const desk = card("desk")
            verify(named(card("phone"), "deviceKind") === null || !showing(named(card("phone"), "deviceKind")), "another device's kind is its own to say")
            verify(walk(desk, i => showing(i) && i.text === "A phone", []).length === 0, "no choice before the press")
            named(desk, "deviceKind").clicked()
            tryVerify(() => shown(desk, "A phone is carried.") !== null, 2000, "the setting, with its sentence")
            const choices = walk(desk, i => showing(i) && i.model !== undefined && i.activated !== undefined, [])
            compare(choices.length, 1, "the choice of what this device is")
            choices[0].currentIndex = 2
            choices[0].activated(2)
            compare(root.saved, [["device_kind", "tablet"]])
            named(card("desk"), "deviceKind").clicked()
            tryVerify(() => shown(card("desk"), "A phone is carried.") === null, 2000, "closed again")
            // Its parts are switched where the sharing is set up.
            const change = named(card("desk"), "devicePartsChange")
            verify(showing(change))
            verify(!showing(named(card("phone"), "devicePartsChange")), "another device's parts are read only")
            change.clicked()
            compare(sharingSpy.count, 1)
        }

        // At a phone's width: cards, each field under its name, nothing past the edge.
        function test_cards_on_a_narrow_window() {
            root.width = 412
            tryVerify(() => !tab.wide, 2000)
            wait(100)
            compare(tab.width, 380)
            const phone = card("phone")
            // No row of titles; each field carries its name.
            for (const title of tab.titles.slice(1))
                compare(walk(tab, i => showing(i) && i.text === title, []).length, 3, title + ": once a card")
            verify(shown(phone, "Closed since Sat 10 Oct 21:40.") !== null)
            const now = shown(phone, "devices-col-now")
            const state = shown(phone, "Closed since Sat 10 Oct 21:40.")
            verify(now.mapToItem(tab, 0, 0).y < state.mapToItem(tab, 0, 0).y, "its name above it")
            // Nothing runs past the card: every item that shows ends within the tab.
            const past = walk(tab, i => showing(i) && i.width > 0 && i.mapToItem(tab, 0, 0).x + i.width > tab.width + 0.5, [])
            compare(past.map(i => i.objectName || i.text || String(i)), [], "nothing past the edge")
            const cards = walk(tab, i => (i.objectName || "").startsWith("device-"), [])
            verify(cards[0].y + cards[0].height <= cards[1].y && cards[1].y + cards[1].height <= cards[2].y, "one card under the other")
            // The switches still work there.
            boxes(card("laptop"))[0].toggle()
            boxes(card("laptop"))[0].toggled()
            compare(mock.asked, ["say laptop area work"])
            save("devices-cards")
        }

        // Sharing off: this device alone, why, and where sharing is set up;
        // its name and what it is for can still be said.
        function test_sharing_off_lists_this_device_alone() {
            mock.on = false
            tab.reload()
            wait(50)
            compare(tab.rows.map(r => r.id), ["desk"])
            verify(shown(tab, "devices-alone") !== null, "why it is alone")
            verify(shown(tab, "devices-parts-elsewhere") === null)
            verify(!showing(named(card("desk"), "devicePartsChange")), "no part to change while it does not share")
            named(tab, "devicesToSharing").clicked()
            compare(sharingSpy.count, 1)
            boxes(card("desk"))[1].toggle()
            boxes(card("desk"))[1].toggled()
            compare(mock.asked, ["say desk area work+admin"])
            verify(showing(named(card("desk"), "deviceRename")))
            // A folder set whose key is no longer held: said in its own words.
            mock.why = "Type the passphrase again on this device: the keyring no longer holds it."
            tab.reload()
            wait(50)
            verify(shown(tab, mock.why) !== null && shown(tab, "devices-alone") === null, "why it does not share now")
        }
    }
}
