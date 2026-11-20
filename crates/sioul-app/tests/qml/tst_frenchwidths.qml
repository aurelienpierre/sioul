// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Two tables drawn with their French words, at the widths a window 1280 and
// 1400 points wide gives them (the guide's pictures): Settings ▸ What reaches
// you, Your rules (ReachesTab.qml: the column of 720 points that Settings
// keeps for reading), its table of People first; and Settings ▸ Devices
// (DevicesTab.qml), the whole width, its table's titles over two computers.
// No word shown is cut: a label wraps rather than ends in "…", a button's
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
            "reach-people": "Personnes",
            "reach-own-notices": "Les avis de Sioul",
            "reach-moments": "Moments",
            "reach-circles": "Qui est dans quel cercle",
            "reach-circles-help": "Toute personne de vos carnets d’adresses est parmi vos contacts, sans rien régler."
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
        function cell(channel, value, label) {
            const all = [["always", "Toujours"], ["awake", "Quand vous ne dormez pas"], ["work-admin", "Aux heures de travail et de démarches"], ["work", "Aux heures de travail"], ["gathered", "Aux heures de regroupement"], ["never", "Jamais notifié"]]
            return { channel: channel, value: value, label: label, choices: all.map(c => ({ id: c[0], label: c[1], help: "" })), rows: [channel + ".x"] }
        }
        function reachesView() {
            const c = reachesMock.cell
            return JSON.stringify({
                now: ["Maintenant : travail, jusqu’à 17:00."],
                state: "usual",
                column: "work",
                now_columns: ["work"],
                presets: [{ id: "usual", label: "Comme Sioul le fait maintenant", current: true, changes: 0 }, { id: "quieter", label: "Plus calme", current: false, changes: 12 }, { id: "reachable", label: "Plus joignable", current: false, changes: 6 }],
                presets_note: "Un préréglage laisse les appels tels quels : ils se règlent pour chaque téléphone, dans Paramètres ▸ Appareils.",
                changed: "",
                people: {
                    channels: [{ id: "mail", label: "Courriels", note: "" }, { id: "messages", label: "Messages", note: "" }],
                    rows: [
                        { id: "always", label: "Passent toujours", help: "Les personnes de votre liste Passent toujours.", cells: [c("mail", "always", "Toujours"), c("messages", "always", "Toujours")] },
                        { id: "safe", label: "Sûrs", help: "Il n’y entre que qui vous y mettez.", cells: [c("mail", "awake", "Quand vous ne dormez pas"), c("messages", "awake", "Quand vous ne dormez pas")] },
                        { id: "contacts", label: "Vos contacts", help: "Toute personne de vos carnets d’adresses.", cells: [c("mail", "work-admin", "Aux heures de travail et de démarches"), c("messages", "own", "À votre façon")] },
                        { id: "restricted", label: "Restreints", help: "Connus, entendus seulement aux moments que vous choisissez.", cells: [c("mail", "gathered", "Aux heures de regroupement"), c("messages", "work", "Aux heures de travail")] },
                        { id: "others", label: "Tous les autres", help: "Dans aucun carnet d’adresses et sur aucune liste.", cells: [c("mail", "never", "Jamais notifié"), c("messages", "never", "Jamais notifié")] }
                    ],
                    blocked: { label: "Bloqués", help: "Jamais, sur aucun canal.", word: "Jamais" },
                    notes: ["Un message n’est notifié que lorsque l’adresse ou l’application à laquelle il a été écrit est pour maintenant."],
                    own_help: "Réglé case par case, dans Ce qui vient quand."
                },
                own: [
                    { id: "asked", label: "Ce que vous avez réglé ou demandé", holds: "Les codes et liens que vous demandez, les prises, le réveil et les alarmes d’un événement.", value: "sleep", value_label: "Prises pendant le sommeil", choices: [{ id: "sleep", label: "Prises pendant le sommeil" }, { id: "waking", label: "Prises au réveil" }], switches: [], gathered: "" },
                    { id: "reminders", label: "Rappels", holds: "Avant un événement, le jour travaillé d’avant, les dates, les attentes, les paiements et les papiers.", value: "awake", value_label: "Quand vous ne dormez pas", choices: [{ id: "awake", label: "Quand vous ne dormez pas" }, { id: "off", label: "Non" }], switches: [], gathered: "" },
                    { id: "sites", label: "Sites et applications", holds: "Les nouvelles des sites sur un ordinateur, les notifications automatiques des applications sur un téléphone.", value: "gathered", value_label: "Aux heures de regroupement", choices: [{ id: "now", label: "Tout de suite" }, { id: "gathered", label: "Aux heures de regroupement" }], switches: [], gathered: "" }
                ],
                moments: {
                    columns: ["Qui passe", "Mis par", "Sur votre téléphone"],
                    rows: [{ id: "usual", label: "Comme d’habitude", who: "Vos règles ci-dessus", by: "Rien", phone: "Comme Android est réglé" }, { id: "dnd", label: "Ne pas déranger", who: "Passent toujours, et ce que vous avez réglé", by: "L’interrupteur, et les cases ci-dessous", phone: "Le mode de Sioul, qui laisse passer qui Sioul laisse passer" }],
                    holds_title: "Qui retient quoi sur un téléphone",
                    holds_columns: ["Sioul", "Android"],
                    holds: [],
                    principle: "Sioul décide de ce qui passe, et quand ; le mode Ne pas déranger d’Android décide seulement s’il fait un bruit."
                },
                elsewhere: { line: "Rien n’est réglé ailleurs.", links: [] },
                calls: "Les appels se règlent pour chaque téléphone dans Paramètres ▸ Appareils.",
                rows: [],
                grid: { columns: [], marks: [], rows: [] },
                phone: false,
                exact: true
            })
        }
        function settings(view) {
            return "[]"
        }
        function reachesChoose(what, value) {
            return ""
        }
        function reachesSet(row, words, shown) {
            return ""
        }
        function reachesPreset(id) {
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
            return key === "devices-system-name" ? "Son système l’appelle " + value + "." : devicesMock.text(key) + " " + value
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

        // Settings ▸ What reaches you, Your rules: the People table first, then Sioul's own and the moments.
        function test_what_reaches_you_cuts_no_french_word() {
            reaches.visible = true
            devices.visible = false
            reaches.reload()
            waitForRendering(reaches)
            const people = named(reaches, "part-people")
            verify(people !== null, "the People table shown")
            const cutPeople = cut(people)
            compare(cutPeople.length, 0, "cut in People: " + cutPeople.join("; "))
            const cutAll = cut(reaches)
            compare(cutAll.length, 0, "cut in Your rules: " + cutAll.join("; "))
        }

        // Settings ▸ Devices, the whole width of a window 1400 and 1280 wide: the table's titles first.
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
            const titles = walk(devices, []).filter(i => i.truncated !== undefined && devicesMock.words["devices-col-opened"] === i.text)
            verify(titles.length > 0, "the title Dernière ouverture shown")
            for (const title of titles)
                verify(!title.truncated, "« Dernière ouverture » whole in " + Math.round(title.width))
            const cutAll = cut(devices)
            compare(cutAll.length, 0, "cut in Devices: " + cutAll.join("; "))
        }
    }
}
