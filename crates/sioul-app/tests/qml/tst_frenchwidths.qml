// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Two tables drawn with their French words, at the widths a window 1280 and
// 1400 points wide gives them (the guide's pictures): Settings ▸ What reaches
// you (ReachesTab.qml: the column of 720 points that Settings
// keeps for reading), its rule first; and Settings ▸ Devices
// (DevicesTab.qml), the whole width, its table's four titles over two
// computers, each whole on one line, then each card's Details opened. No
// word shown is cut: a label wraps rather than ends in "…", a button's
// words fit, or wrap. On stand-ins for Sioul that answer in French. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1400
    height: 1500

    Theme {
        id: testTheme
    }

    // Settings ▸ What reaches you, as `reaches::view` words it in French (made up, in its shape).
    QtObject {
        id: reachesMock

        readonly property var words: ({
            "reach-rule": "La règle habituelle",
            "reach-exceptions": "Vos exceptions",
            "reach-add": "Ajouter une exception…",
            "reach-edit": "Modifier",
            "reach-dnd": "« Ne pas déranger » se met tout seul",
            "reach-apps-computer": "Les notifications sur cet ordinateur",
            "reach-start": "Partir de",
            "reach-circles": "Qui est dans quel cercle",
            "reach-worked-out": "Ce qui vient quand, en détail"
        })
        function text(key) {
            return reachesMock.words[key] || key
        }
        function textWith(key, name, value) {
            return reachesMock.text(key) + " " + value
        }
        function textArgs(key, args) {
            return reachesMock.text(key)
        }
        function reachesView() {
            const line = (text, pattern) => ({ text: text, note: "", value: { what: { kind: "channel", channel: "mail" }, whom: { kind: "circle", person: "neutral" }, where: { kind: "everywhere" }, when: { kind: "pattern", pattern: pattern } } })
            return JSON.stringify({
                now: ["Maintenant : travail, jusqu’à 17:00."],
                state: "usual",
                phone: false,
                start: { id: "usual", choices: [{ id: "usual", label: "Comme Sioul le fait maintenant" }, { id: "quieter", label: "Plus calme" }, { id: "reachable", label: "Plus joignable" }] },
                rule: [
                    { circle: "always", text: "Passent toujours : toujours, même pendant « Ne pas déranger » ; leurs courriels montrés sans un mot la nuit et en pause.", count: "3 personnes" },
                    { circle: "neutral", text: "Vos contacts : quand vous ne dormez pas ; leurs courriels aux heures de travail et de démarches.", count: "312 fiches" },
                    { circle: "stranger", text: "Tous les autres : aux heures de travail et de démarches, regroupés en dehors ; leurs courriels aux heures de travail et de démarches.", count: "" }
                ],
                idle: [],
                exceptions: [line("Les courriels de vos contacts : quand vous ne dormez pas.", "awake"), line("Les courriels de tous les autres : tout de suite pendant le travail et les démarches ; montrés sans un mot pendant les loisirs ; plus tard le reste du temps.", "work")],
                card: { what: [], whom: [], where: [] },
                now_columns: ["work"],
                grid: { columns: [], marks: [], rows: [] },
                circle: { title: "Vos proches", help: "", people: [] },
                exact: true
            })
        }
        function settings(view) {
            return "[]"
        }
        function reachesAct(verb, json) {
            return JSON.stringify({ problem: "" })
        }
        function appNotesSetup() {
            return ""
        }
        function dndSetup() {
            return ""
        }
    }

    // Settings ▸ Devices, as `share::devices` words it in French (made up: no real device).
    QtObject {
        id: devicesMock

        readonly property var words: ({
            "devices-col-name": "Appareil",
            "devices-col-form": "Ce qu’il est",
            "devices-col-now": "Maintenant",
            "devices-col-opened": "Dernière ouverture",
            "devices-col-shared": "Dernier partage",
            "devices-col-sioul": "Sioul",
            "devices-col-for": "Pour",
            "devices-col-parts": "Partage",
            "devices-title": "Vos appareils",
            "devices-this": "Cet appareil",
            "devices-for-all": "Tout arrive ici",
            "devices-rename": "Renommer…",
            "devices-grid-title": "Ce qui arrive, et quand",
            "devices-grid-device": "Appareil",
            "devices-parts-change": "Changer ce qui voyage depuis cet appareil…",
            "devices-parts-elsewhere": "Une partie s’active ou se coupe sur l’appareil lui-même, là où ses données arrivent.",
            "devices-details": "Détails",
            "devices-rename-help": "Un nom à vous pour cet appareil, qui voyage avec vos réglages. Laissé vide, le nom que lui donne son système revient.",
            "devices-rename-field": "Le nom que vous lui donnez",
            "ui-save": "Enregistrer",
            "ui-cancel": "Annuler",
            "share-device-remove-live": "L’oublier et retirer ses fichiers…",
            "share-device-count-again": "Le compter de nouveau",
            "share-device-forget": "Oublier cet appareil",
            "share-device-forget-now": "L’oublier maintenant",
            "area-work": "Travail",
            "area-admin": "Vos démarches",
            "area-leisure": "Loisirs"
        })
        function text(key) {
            return devicesMock.words[key] || key
        }
        function textWith(key, name, value) {
            return key === "devices-system-name" ? "Son système l’appelle " + value + "." : key === "devices-details-of" ? "Détails\u202f: " + value : devicesMock.text(key) + " " + value
        }
        function textArgs(key, args) {
            const given = JSON.parse(args)
            return key === "devices-plain-line" ? given.label + "\u202f: " + given.value : devicesMock.text(key)
        }
        // How the window left its pages (`viewFlag`): unsaid, false.
        property var flags: ({})
        function viewFlag(name) {
            return devicesMock.flags[name] === true
        }
        function setViewFlag(name, on) {
            const flags = Object.assign({}, devicesMock.flags)
            flags[name] = on
            devicesMock.flags = flags
        }
        function devices() {
            const times = ["Travail", "Démarches", "Loisirs", "Repas", "Sommeil"]
            const cells = areas => ["work", "admin", "leisure", "meals", "sleep"].map((time, i) => ({ time: time, title: times[i], work: areas.indexOf("work") >= 0, admin: areas.indexOf("admin") >= 0, leisure: areas.indexOf("leisure") >= 0, areas: areas, label: areas === "" ? "Rien" : "Travail", own: false, name: times[i], line: times[i] }))
            const row = (id, here, name, system, area) => ({
                id: id, here: here, given: name === system ? "" : name, name: name, system: system, form: "computer", form_words: "Un ordinateur", code: "in-use",
                state: here ? "Utilisé en ce moment." : "Fermé depuis ven. 20 nov. 08:40.", notes: [], opened: here ? "09:12" : "ven. 20 nov. 08:00", shared: here ? "10:41" : "ven. 20 nov. 08:39",
                version: "0.0.12 (f3dd1877dce7)", older: false, build: "", parts: ["settings"], parts_words: "Réglages et comptes, Santé", off: false, forget: !here, forget_now: false,
                area: area, area_words: area === "" ? "Tout arrive ici" : "Travail", follows: true, cells: cells(area), cells_older: ""
            })
            return JSON.stringify({
                on: true, why: "", stays: "",
                rows: [row("desk", true, "Portable du bureau", "noa-laptop", "work"), row("tower", false, "tour-du-salon", "tour-du-salon", "")],
                grid: { times: ["Travail", "Démarches", "Loisirs", "Repas", "Sommeil"], now: "Maintenant, travail jusqu’à 17:00 : le travail arrive ici.", unheld: "", twice: "", never: "Quoi qu’un appareil fasse, ceci arrive toujours : les prises et tout ce qui est de Santé." }
            })
        }
        function deviceSay(id, field, value) {
            return ""
        }
    }

    ReachesTab {
        id: reaches

        width: 720
        sioul: reachesMock
        theme: testTheme
        rows: []
    }
    DevicesTab {
        id: devices

        y: 0
        x: 0
        visible: false
        width: 1152
        sioul: devicesMock
        theme: testTheme
    }

    TestCase {
        name: "FrenchWidths"
        when: windowShown

        function walk(item, found) {
            if (!item || item.visible === false)
                return found
            found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                walk(kids[i], found)
            if (item.contentItem && item.contentItem !== item && kids.indexOf(item.contentItem) < 0)
                walk(item.contentItem, found)
            return found
        }

        // What is cut in `within`, in words: a label ended with "…", a button whose words
        // are wider than its room and do not wrap.
        function cut(within) {
            const out = []
            for (const item of walk(within, [])) {
                if (item.width <= 0)
                    continue
                if (item.truncated === true)
                    out.push("label « " + item.text + " » (" + Math.round(item.width) + " wide)")
                const button = item.availableWidth !== undefined && item.implicitContentWidth !== undefined && typeof item.text === "string" && item.text !== "" && item.contentItem
                if (!button)
                    continue
                const wraps = item.contentItem.wrapMode !== undefined && item.contentItem.wrapMode !== Text.NoWrap
                if (!wraps && item.implicitContentWidth > item.availableWidth + 0.5)
                    out.push("button « " + item.text + " » (" + Math.round(item.implicitContentWidth) + " in " + Math.round(item.availableWidth) + ")")
            }
            return out
        }

        function named(item, name) {
            return walk(item, []).find(i => i.objectName === name) || null
        }

        // Settings ▸ What reaches you: the rule's sentences first, then the exceptions and the rest.
        function test_what_reaches_you_cuts_no_french_word() {
            reaches.visible = true
            devices.visible = false
            reaches.reload()
            waitForRendering(reaches)
            const rule = named(reaches, "part-rule")
            verify(rule !== null, "the usual rule shown")
            const cutRule = cut(rule)
            compare(cutRule.length, 0, "cut in the rule: " + cutRule.join("; "))
            const cutAll = cut(reaches)
            compare(cutAll.length, 0, "cut in the page: " + cutAll.join("; "))
        }

        // Settings ▸ Devices, the whole width of a window 1400 and 1280 wide: the
        // table's four titles first, each whole on one line; then the cards, closed and with their Details.
        function test_devices_cut_no_french_word_data() {
            return [{ tag: "1400", width: 1400 - 216 - 32 }, { tag: "1280", width: 1280 - 216 - 32 }]
        }
        function test_devices_cut_no_french_word(data) {
            reaches.visible = false
            devices.visible = true
            devices.width = data.width
            devices.reloadAll()
            tryVerify(() => named(devices, "device-tower") !== null)
            waitForRendering(devices)
            verify(devices.wide, "a table")
            const titles = walk(named(devices, "devicesTitles"), []).filter(i => i.truncated !== undefined)
            compare(titles.map(t => t.text), ["Appareil", "Ce qu’il est", "Maintenant", "Pour"])
            for (const title of titles)
                verify(!title.truncated && title.lineCount === 1 && title.implicitWidth <= title.width + 0.5, "« " + title.text + " » whole on one line in " + Math.round(title.width))
            const cutAll = cut(devices)
            compare(cutAll.length, 0, "cut in Devices: " + cutAll.join("; "))
            // Each card's Details opened: their fields and buttons, whole too.
            for (const id of ["desk", "tower"])
                devices.detailsOf(id)
            waitForRendering(devices)
            verify(walk(devices, []).some(i => i.text === devicesMock.words["share-device-remove-live"]), "the tower's Details shown")
            const cutDetails = cut(devices)
            compare(cutDetails.length, 0, "cut in the Details: " + cutDetails.join("; "))
            for (const fold of walk(devices, []).filter(i => i.objectName === "deviceDetails"))
                fold.open = false
        }
    }
}
