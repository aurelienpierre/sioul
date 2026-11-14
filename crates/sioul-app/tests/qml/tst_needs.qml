// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The usual meals, naps and night in Health's settings (NeedsSection.qml)
// at a phone's 360 points, in English and in French: nothing past the
// screen's edge, a minutes field's words wrapping beside its box (10
// November 2026: the French cards reached 399 points). Their words as the
// Fluent files say them, copied here. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml"

Item {
    id: root

    width: 360
    height: 1600

    Theme {
        id: testTheme

        touch: true
    }

    QtObject {
        id: words

        property string language: "en"
        readonly property var en: {"needs-title": "Meals, rest and sleep", "needs-help": "Times kept free: no task is planned in them. A notice comes before, about the work, then one at the time; nothing else is said, and nothing is recorded.", "needs-meals": "Meals", "needs-naps": "Naps", "needs-sleep": "The night", "needs-add-meal": "Add a meal or a snack", "needs-add-nap": "Add a nap", "needs-at": "At", "needs-eat": "Minutes to eat", "needs-prep": "Minutes to get it ready", "needs-nap-minutes": "Minutes", "needs-after": "Minutes to come back", "needs-bed": "Bedtime", "needs-wake": "Waking", "needs-wind-down": "Minutes to wind down", "needs-notices": "Notices", "needs-nap-alarm": "Alarm at its end (on a phone)", "needs-heads-up": "Notice before, in minutes", "needs-later-by": "“Later” moves it by, in minutes", "needs-name-hint": "Name, as you like", "needs-remove": "Remove", "wake-title": "Wake-up alarm", "wake-help": "Rings on your phone or tablet (Sioul for Android) when the night ends, on the mornings ticked; a night changed on the page rings at its own waking, and a night's or a nap's own switch on the page turns its alarm on or off for that one alone; a nap rings at its end when “Alarm at its end” is ticked with it, above. Android's do-not-disturb lets alarms through.", "weekday-1": "Monday", "weekday-2": "Tuesday", "weekday-3": "Wednesday", "weekday-4": "Thursday", "weekday-5": "Friday", "weekday-6": "Saturday", "weekday-7": "Sunday"}
        readonly property var fr: {"needs-title": "Repas, repos et sommeil", "needs-help": "Des heures gardées libres : aucune tâche n’y est prévue. Un avis vient avant, sur le travail, puis un à l’heure ; rien d’autre n’est dit, et rien n’est noté.", "needs-meals": "Repas", "needs-naps": "Siestes", "needs-sleep": "La nuit", "needs-add-meal": "Ajouter un repas ou un en-cas", "needs-add-nap": "Ajouter une sieste", "needs-at": "À", "needs-eat": "Minutes pour manger", "needs-prep": "Minutes pour le préparer", "needs-nap-minutes": "Minutes", "needs-after": "Minutes pour revenir", "needs-bed": "Coucher", "needs-wake": "Lever", "needs-wind-down": "Minutes pour se préparer", "needs-notices": "Avis", "needs-nap-alarm": "Réveil à la fin (sur un téléphone)", "needs-heads-up": "Avis avant, en minutes", "needs-later-by": "« Plus tard » décale de, en minutes", "needs-name-hint": "Nom, comme vous voulez", "needs-remove": "Retirer", "wake-title": "Réveil au lever", "wake-help": "Sonne sur votre téléphone ou votre tablette (Sioul pour Android) à la fin de la nuit, les matins cochés ; une nuit changée sur la page sonne à son propre lever, et l’interrupteur d’une nuit ou d’une sieste sur la page allume ou éteint son réveil pour elle seule ; une sieste sonne à sa fin quand « Réveil à la fin » est coché avec elle, plus haut. Le mode Ne pas déranger d’Android laisse passer les alarmes.", "weekday-1": "lundi", "weekday-2": "mardi", "weekday-3": "mercredi", "weekday-4": "jeudi", "weekday-5": "vendredi", "weekday-6": "samedi", "weekday-7": "dimanche"}

        function text(key) {
            return key === "qt-locale" ? (words.language === "fr" ? "fr_FR" : "en_GB") : (words[words.language][key] || key)
        }
        function textWith(key, name, value) {
            return words.text(key) + " " + value
        }
        // Made up: a breakfast and a nap, every day, the night with its notices.
        function needs() {
            const block = (name, at) => ({ name: name, at: at, minutes: 20, before: 10, after: 15, days: [true, true, true, true, true, true, true], on: true, notices: true })
            return JSON.stringify({
                needs: { meals_on: true, meals: [block("", "08:00"), block("", "12:30")], naps_on: true, naps: [block("", "14:00")], sleep_on: true, sleep: { bed: "23:00", wake: "07:00", wind_down: 60, notices: true, alarm: [false, false, false, false, false, false, false] }, heads_up: 15, later: 15 },
                usual: { meals: ["Breakfast", "Lunch"], nap: "Nap", sleep: "" },
                gaps: [],
                wake: { phone: false, names: [], short: [], next: "", exact: true, screen: true, notifications: true }
            })
        }
    }

    Component {
        id: sectionComponent

        NeedsSection {
            x: 16
            width: root.width - 32
            sioul: words
            theme: testTheme
        }
    }

    TestCase {
        name: "Needs"
        when: windowShown

        function past(item, out) {
            if (!item.visible)
                return out
            const right = item.mapToItem(root, item.width, 0).x
            if (item.width > 0 && right > root.width - 16 + 0.5)
                out.push(String(item).split("(")[0] + " " + Math.round(right) + (item.text !== undefined ? " \"" + String(item.text).slice(0, 30) + "\"" : ""))
            for (let i = 0; i < item.children.length; ++i)
                past(item.children[i], out)
            return out
        }

        function test_within_a_small_phone_data() {
            return [{ tag: "English", language: "en" }, { tag: "French", language: "fr" }]
        }

        function test_within_a_small_phone(data) {
            words.language = data.language
            const section = createTemporaryObject(sectionComponent, root)
            waitForRendering(section)
            compare(past(section, []), [], "nothing past 360 points")
            // A nap's alarm at its end, every day it is kept: on the nap only, never a meal.
            const alarms = all(section, c => c.objectName === "napAlarm", [])
            compare(alarms.length, 3, "one per meal and nap editor")
            compare(alarms.filter(c => c.visible).length, 1, "the nap's alone")
        }

        function all(item, test, out) {
            if (test(item))
                out.push(item)
            for (let i = 0; i < item.children.length; ++i)
                all(item.children[i], test, out)
            return out
        }
    }
}
