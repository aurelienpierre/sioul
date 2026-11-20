// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An office's opening hours in the window (OpeningHours.qml,
// OpeningHoursForm.qml, OpeningHoursLookup.qml) and the map's search
// service's setting (SettingRow.qml), on a stand-in for Sioul that answers
// through calls, as the backend does, and as late, empty or odd as it can,
// never kinder than the program: it reads hours as the core does for what
// the tests type (a rule it cannot count leaves the whole value uncounted;
// ";" in a day is a new rule), and speaks French words when asked to. The
// section shows nothing before the card is read, then what is true now and
// the hours; a card that is not an office's offers no lookup but the link
// that says it is one; two quick presses of "Call back…" make one task; the
// lookup sends only the words shown, once per press, ignores an answer for
// another card or an earlier press, says who is asked and when nobody can
// be; the form fills its week from the short form and makes the short form
// from its days, keeps a day it cannot count in its field, and Save reads
// the day typed last even when its press does not take the focus; the
// public holidays' tick follows 24/7. Everything within the edge at 412
// points, and in a 372-point column in French with a long name. The rules
// themselves are the core's (opening.rs, hourslookup.rs). Fiction only. Run
// by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1200
    height: 1600

    Theme {
        id: testTheme
    }

    // An invented town hall's section, as `contactHours` writes it.
    readonly property var hall: ({
        name: "Example town hall", card: "org", read_only: false, mark: false,
        open: { value: "Mo-Fr 09:00-12:00,14:00-17:00; PH off", short: "mo-fr 09:00-12:00, 14:00-17:00", lines: ["Monday to Friday: 9:00–12:00, 14:00–17:00"], holidays_off: true, unread: false, doubt: "", rest: [], source: "Typed by you on 20 November 2026", stale: false, now: "Open until 12:00" },
        phone: null, note: "Closed in August", numbers: ["01 23 45 67 89"],
        asked: { name: "Example town hall", place: "99999 Exempleville", france: true },
        sources: "the directory and OpenStreetMap", map: "", map_name: "OpenStreetMap", holidays_known: true, offline: false
    })

    // The French words of the section, as the program's are, for the narrow column (fr/sioul.ftl).
    readonly property var french: ({
        "hours-title": "Horaires d’ouverture",
        "hours-none": "Pas encore d’horaires d’ouverture sur cette fiche.",
        "hours-add": "Ajouter des horaires d’ouverture",
        "hours-change": "Modifier",
        "hours-look-up": "Les chercher…",
        "hours-call-back": "Rappeler…",
        "hours-mark-office": "C’est un bureau, une boutique ou un cabinet",
        "hours-mark-why": "Sioul ne cherche des horaires que pour un bureau, une boutique ou un cabinet : le nom d’une personne n’est jamais envoyé nulle part.",
        "hours-holidays": "Fermé les jours fériés.",
        "hours-holidays-unknown": "Fermé les jours fériés, mais Sioul n’en connaît encore aucun : ajoutez leur agenda dans Réglages ▸ Heures.",
        "hours-phone-title": "Au téléphone",
        "hours-read-only": "Le carnet de cette fiche ne peut qu’être lu : ses horaires ne peuvent pas y être écrits ici."
    })

    QtObject {
        id: mock

        signal contactsChanged
        signal hoursFound(string key, string found)

        // What the next call of `contactHours` answers: "" while the card is not read yet.
        property string section: ""
        property bool frenchWords: false
        property var asked: []
        property var picked: []
        property var added: []
        property var saved: []
        property int calls: 0
        property int marks: 0

        function text(key) { return mock.frenchWords && root.french[key] !== undefined ? root.french[key] : key }
        function textWith(key, name, value) { return mock.text(key) + ":" + name + "=" + value }
        function textArgs(key, args) { return key + ":" + args }
        function contactHours(key) { return mock.section }
        function callBack(key) { mock.calls++; return JSON.stringify({ uid: "call-" + mock.calls }) }
        function markOffice(key) { mock.marks++; return JSON.stringify({ ok: true }) }
        function lookUpHours(key, asked) { mock.asked = mock.asked.concat([asked]) }
        function pickHours(key, found) { mock.picked = mock.picked.concat([found]); return JSON.stringify({ ok: true, changed: true, ticked: 0, offers: { phone: "+33 1 98 76 54 32", address: "" } }) }
        function addToCard(key, what, value) { mock.added = mock.added.concat([what + "=" + value]); return JSON.stringify({ ok: true }) }
        function saveContactHours(key, typed) { mock.saved = mock.saved.concat([typed]); return JSON.stringify({ ok: true, changed: true, ticked: 0 }) }

        // As the core reads them, for what these tests type: days (English or French
        // two letters, a range), then hours ("9-12", "09:00-12:00", "9h"), going
        // forward in a day; "PH off"; "24/7". A rule it cannot count leaves the whole
        // value uncounted, kept whole in `rest`.
        function readHours(text) {
            const keys = { mo: 0, tu: 1, we: 2, th: 3, fr: 4, sa: 5, su: 6, lu: 0, ma: 1, me: 2, je: 3, ve: 4, di: 6 }
            const time = t => {
                const m = /^(\d{1,2})(?:[:h](\d{2}))?h?$/.exec(t.trim())
                if (!m || Number(m[1]) > 24 || (m[2] && Number(m[2]) > 59))
                    return null
                return (m[1].length === 1 ? "0" : "") + m[1] + ":" + (m[2] || "00")
            }
            const week = [[], [], [], [], [], [], []]
            const none = { written: "", short: "", lines: [], days: ["", "", "", "", "", "", ""], rest: text.trim() === "" ? [] : [text.trim()], holidays_off: true, holidays_said: false, doubt: "" }
            let said = false
            let open = false
            for (const raw of text.split(";")) {
                const rule = raw.trim()
                if (rule === "")
                    continue
                if (/^ph (off|closed)$/i.test(rule)) {
                    said = true
                    continue
                }
                if (rule === "24/7") {
                    for (let d = 0; d < 7; d++)
                        week[d] = ["00:00-24:00"]
                    said = true
                    open = true
                    continue
                }
                const space = rule.indexOf(" ")
                if (space < 0)
                    return JSON.stringify(none)
                const daysWord = rule.slice(0, space).toLowerCase()
                const span = daysWord.split("-")
                const from = keys[span[0]]
                const to = keys[span[span.length - 1]]
                if (from === undefined || to === undefined || span.length > 2)
                    return JSON.stringify(none)
                const ranges = []
                let last = ""
                for (const part of rule.slice(space + 1).split(",")) {
                    const ends = part.trim().split("-")
                    const start = ends.length === 2 ? time(ends[0]) : null
                    const end = ends.length === 2 ? time(ends[1]) : null
                    if (start === null || end === null || start >= end || start < last)
                        return JSON.stringify(none)
                    ranges.push(start + "-" + end)
                    last = end
                }
                for (let d = from; ; d = (d + 1) % 7) {
                    week[d] = ranges.slice()
                    if (d === to)
                        break
                }
            }
            if (week.every(d => d.length === 0))
                return JSON.stringify(none)
            const days = week.map(d => d.join(", "))
            const names = ["mo", "tu", "we", "th", "fr", "sa", "su"]
            const parts = []
            for (let d = 0; d < 7; d++) {
                if (days[d] === "")
                    continue
                let e = d
                while (e + 1 < 7 && days[e + 1] === days[d])
                    e++
                parts.push((e === d ? names[d] : names[d] + "-" + names[e]) + " " + days[d])
                d = e
            }
            return JSON.stringify({ written: parts.join("; "), short: parts.join("; "), lines: parts, days: days, rest: [], holidays_off: !open, holidays_said: said, doubt: "" })
        }
    }

    Component {
        id: sectionComponent

        OpeningHours {
            sioul: mock
            theme: testTheme
            window: root
            key: "/books/hall.vcf"
        }
    }

    Component {
        id: lookupComponent

        OpeningHoursLookup {
            sioul: mock
            theme: testTheme
        }
    }

    Component {
        id: formComponent

        OpeningHoursForm {
            sioul: mock
            theme: testTheme
        }
    }

    // A narrow column, as a phone's card is: the section inside it.
    Component {
        id: columnComponent

        Column {
            width: 372

            OpeningHours {
                width: parent.width
                sioul: mock
                theme: testTheme
                window: root
                key: "/books/hall.vcf"
            }
        }
    }

    // Every item under `item` that passes `test`, a popup's content and a scroll view's included.
    function all(item, test, out) {
        out = out || []
        if (!item)
            return out
        for (let i = 0; i < (item.children || []).length; i++) {
            const child = item.children[i]
            if (test(child))
                out.push(child)
            root.all(child, test, out)
        }
        if (item.contentItem && item.contentItem !== item && (item.children || []).indexOf(item.contentItem) < 0) {
            if (test(item.contentItem))
                out.push(item.contentItem)
            root.all(item.contentItem, test, out)
        }
        return out
    }

    function named(item, name) {
        return root.all(item, c => c.objectName === name)[0]
    }

    // Every visible item within `width` of `within`.
    function inside(within, width) {
        for (const item of root.all(within, c => c.visible && c.width > 0 && c.text !== undefined)) {
            const x = item.mapToItem(within, 0, 0).x
            if (x < -1 || x + item.width > width + 1)
                return item.text + " at " + x + " + " + item.width
        }
        return ""
    }

    TestCase {
        id: test

        name: "OpeningHours"
        when: windowShown

        function init() {
            mock.section = ""
            mock.frenchWords = false
            mock.asked = []
            mock.picked = []
            mock.added = []
            mock.saved = []
            mock.calls = 0
            mock.marks = 0
        }

        // Typed as a person types: the field given the focus, then each key.
        function type(field, text) {
            mouseClick(field)
            field.selectAll()
            keyClick(Qt.Key_Delete)
            for (const c of text)
                keyClick(c)
        }

        // Before the card is read, nothing; once the contacts change, its hours and what is true now.
        function test_the_section_reads_the_card_when_it_changes() {
            for (const width of [1000, 412]) {
                init()
                const section = createTemporaryObject(sectionComponent, root, { width: width })
                waitForRendering(section)
                compare(section.shown, null, "the backend has not answered")
                verify(!root.named(section, "openingHoursChange").visible)
                mock.section = JSON.stringify(root.hall)
                mock.contactsChanged()
                waitForRendering(section)
                compare(root.named(section, "openingHoursNow").text, "Open until 12:00")
                compare(root.named(section, "openingHoursNote").text, "Closed in August")
                verify(root.named(section, "openingHoursLookUp").visible && root.named(section, "openingHoursChange").visible && root.named(section, "openingHoursCallBack").visible)
                verify(!root.named(section, "openingHoursMark").visible, "an office's card says it is one")
                compare(root.inside(section, width), "", "within " + width)
                // Call back: one press, the task made by the backend; a second press at once makes nothing.
                let answer = null
                section.calledBack.connect(a => answer = a)
                mouseClick(root.named(section, "openingHoursCallBack"))
                mouseClick(root.named(section, "openingHoursCallBack"))
                compare(mock.calls, 1, "two quick presses: one task")
                compare(answer.uid, "call-1")
            }
        }

        // A card that is not an office's: no lookup, the one link that says it is one. A card only
        // read: neither changed nor looked up. With no calendar of public holidays, said where to add one.
        function test_what_a_card_does_not_offer() {
            const section = createTemporaryObject(sectionComponent, root, { width: 600 })
            mock.section = JSON.stringify(Object.assign({}, root.hall, { card: "", open: null, note: "", asked: null, mark: true }))
            section.reload()
            waitForRendering(section)
            verify(!root.named(section, "openingHoursLookUp").visible, "a person's name is never sent")
            verify(root.named(section, "openingHoursChange").visible && root.named(section, "openingHoursMarkWhy").visible)
            const mark = root.named(section, "openingHoursMark")
            verify(mark.visible)
            let answer = null
            section.marked.connect(a => answer = a)
            mouseClick(mark)
            compare(mock.marks, 1)
            compare(answer.ok, true)
            mock.section = JSON.stringify(Object.assign({}, root.hall, { read_only: true }))
            section.reload()
            verify(!root.named(section, "openingHoursLookUp").visible && !root.named(section, "openingHoursChange").visible && !root.named(section, "openingHoursMark").visible)
            // A card with no hours yet says so, and offers to add them.
            mock.section = JSON.stringify(Object.assign({}, root.hall, { open: null, note: "" }))
            section.reload()
            verify(root.named(section, "openingHoursNone").visible)
            compare(root.named(section, "openingHoursChange").text, "hours-add")
            // No calendar of public holidays: said, and where to add one.
            mock.section = JSON.stringify(Object.assign({}, root.hall, { holidays_known: false }))
            section.reload()
            verify(root.all(section, c => c.visible && c.text === "hours-holidays-unknown").length === 1)
            // The card gone: nothing.
            mock.section = ""
            section.reload()
            compare(section.shown, null)
        }

        // On a phone, in French, with a long name: the section within a 372-point column (review 9).
        function test_the_section_fits_a_phone_in_french() {
            mock.frenchWords = true
            const column = createTemporaryObject(columnComponent, root, {})
            const section = column.children[0]
            const long = "Mairie annexe d’Exempleville-sur-Exemple, accueil des démarches"
            mock.section = JSON.stringify(Object.assign({}, root.hall, { name: long, card: "", open: null, note: "", asked: null, mark: true, holidays_known: false }))
            section.reload()
            waitForRendering(column)
            verify(section.width <= 372)
            compare(root.inside(section, 372), "", "nothing past the column's edge")
            mock.section = JSON.stringify(Object.assign({}, root.hall, { name: long, holidays_known: false }))
            section.reload()
            waitForRendering(column)
            compare(root.inside(section, 372), "")
        }

        // The words shown are what is sent, once per press; an answer for another card or an earlier
        // press says nothing; nothing found is said.
        function test_a_lookup_sends_what_it_shows_and_waits() {
            for (const width of [1000, 412]) {
                init()
                const holder = createTemporaryObject(lookupComponent, root, {})
                holder.begin("/books/hall.vcf", root.hall)
                holder.width = width
                tryVerify(() => holder.opened && holder.contentItem.width <= width, 2000)
                waitForRendering(holder.contentItem)
                const words = root.named(holder.contentItem, "openingHoursLookupWords")
                compare(words.text, "Example town hall")
                compare(mock.asked.length, 0, "nothing sent before the press")
                type(words, "Mairie de Exempleville")
                mouseClick(root.named(holder.footer, "openingHoursLookupGo"))
                compare(mock.asked.length, 1)
                const sent = JSON.parse(mock.asked[0])
                compare([sent.name, sent.place, sent.france], ["Mairie de Exempleville", "99999 Exempleville", true])
                // Another card's answer, late: ignored; an earlier press's too.
                mock.hoursFound("/books/other.vcf", JSON.stringify({ press: sent.press, found: [{ name: "Elsewhere" }], failed: [] }))
                mock.hoursFound("/books/hall.vcf", JSON.stringify({ press: sent.press - 1, found: [{ name: "Earlier" }], failed: [] }))
                compare(holder.step, "asking")
                // Nothing found, one source away: both said, nothing to pick.
                mock.hoursFound("/books/hall.vcf", JSON.stringify({ press: sent.press, found: [], failed: ["OpenStreetMap did not answer"] }))
                compare(holder.step, "answered")
                verify(root.named(holder.contentItem, "openingHoursLookupNone").visible)
                compare(root.named(holder.contentItem, "openingHoursLookupPick0"), undefined)
                // A second answer after it: ignored, the list stays.
                mock.hoursFound("/books/hall.vcf", JSON.stringify({ press: sent.press, found: [{ name: "Late" }], failed: [] }))
                compare(holder.found.length, 0)
                holder.close()
            }
        }

        // Who is asked, as it is: the map set to None, the directory alone; nobody, Look up off.
        function test_the_lookup_says_who_is_asked() {
            const holder = createTemporaryObject(lookupComponent, root, {})
            holder.begin("/books/hall.vcf", Object.assign({}, root.hall, { sources: "the directory", map: "off" }))
            tryVerify(() => holder.opened, 2000)
            compare(root.named(holder.contentItem, "openingHoursLookupSends").text, "hours-lookup-sends-directory:sources=the directory")
            verify(root.named(holder.contentItem, "openingHoursLookupMapStopped").visible)
            holder.close()
            holder.begin("/books/hall.vcf", Object.assign({}, root.hall, { sources: "", map: "waiting" }))
            tryVerify(() => holder.opened, 2000)
            compare(root.named(holder.contentItem, "openingHoursLookupSends").text, "hours-lookup-nobody")
            verify(!root.named(holder.footer, "openingHoursLookupGo").enabled)
            holder.ask()
            compare(mock.asked.length, 0, "nobody to ask: nothing sent")
            holder.close()
            // The map asked: named, with what goes along.
            holder.begin("/books/hall.vcf", root.hall)
            tryVerify(() => holder.opened, 2000)
            compare(root.named(holder.contentItem, "openingHoursLookupSends").text, "hours-lookup-sends:" + JSON.stringify({ sources: "the directory and OpenStreetMap", map: "OpenStreetMap" }))
            holder.close()
        }

        // Picked: the backend writes the hours; the number the card lacks is offered, written only on its press.
        function test_a_pick_offers_what_the_card_lacks() {
            const holder = createTemporaryObject(lookupComponent, root, {})
            holder.begin("/books/hall.vcf", root.hall)
            holder.width = 412
            tryVerify(() => holder.opened, 2000)
            mouseClick(root.named(holder.footer, "openingHoursLookupGo"))
            const press = JSON.parse(mock.asked[0]).press
            const office = { source: "service-public.gouv.fr", name: "Mairie - Exempleville", address: "Place de l'Exemple\n99999 Exempleville", phone: "+33 1 98 76 54 32", hours: "Mo-Fr 09:00-17:00; PH off", note: "", lines: ["Monday to Friday: 9:00–17:00"], source_name: "the directory" }
            const bare = { source: "openstreetmap.org", name: "Example post office", address: "", phone: "", hours: "", note: "", lines: [], source_name: "OpenStreetMap" }
            mock.hoursFound("/books/hall.vcf", JSON.stringify({ press: press, found: [office, bare], failed: [] }))
            waitForRendering(holder.contentItem)
            verify(root.named(holder.contentItem, "openingHoursLookupPick0").visible)
            verify(!root.named(holder.contentItem, "openingHoursLookupPick1").visible, "no hours there: nothing to take")
            compare(root.inside(holder.contentItem, holder.contentItem.width), "")
            compare(mock.added.length, 0)
            mouseClick(root.named(holder.contentItem, "openingHoursLookupPick0"))
            compare(mock.picked.length, 1)
            compare(holder.step, "picked")
            const add = root.named(holder.contentItem, "openingHoursLookupAddPhone")
            verify(add.visible)
            compare(mock.added.length, 0, "never written by itself")
            mouseClick(add)
            compare(mock.added, ["phone=+33 1 98 76 54 32"])
            verify(!add.visible)
            holder.close()
        }

        // The week fills from the short form; a day typed makes the short form again, ";" in it
        // read as ","; a day Sioul cannot count keeps its text, said under it, and goes to the
        // free line at Save; Save reads the day typed last, its press taking no focus (review 8).
        function test_the_form_fills_its_week_and_back() {
            const form = createTemporaryObject(formComponent, root, {})
            form.edit("/books/hall.vcf", root.hall)
            form.width = 412
            tryVerify(() => form.opened && form.contentItem.width <= 412, 2000)
            waitForRendering(form.contentItem)
            const day = d => root.named(form.contentItem, "openingHoursFormDay" + d)
            compare(day(0).text, "09:00-12:00, 14:00-17:00")
            compare(day(5).text, "", "Saturday closed")
            // Saturday typed, then the next field: the short form made again.
            type(day(5), "9-12")
            keyClick(Qt.Key_Tab)
            compare(root.named(form.contentItem, "openingHoursFormShort").text, "mo-fr 09:00-12:00, 14:00-17:00; sa 09:00-12:00")
            compare(root.inside(form.contentItem, form.contentItem.width), "")
            // Monday typed with ";": read as ",", never as a rule for every day.
            type(day(0), "9-12; 14-18")
            keyClick(Qt.Key_Tab)
            compare(day(0).text, "09:00-12:00, 14:00-18:00")
            compare(day(1).text, "09:00-12:00, 14:00-17:00", "Tuesday as it was")
            // A day it cannot count: kept in its field, said under it.
            type(day(1), "9-12 and 2-5")
            keyClick(Qt.Key_Tab)
            compare(day(1).text, "9-12 and 2-5")
            verify(root.named(form.contentItem, "openingHoursFormDayDoubt1").visible)
            // Thursday typed last, then Save pressed with a press that takes no focus.
            const save = root.named(form.footer, "openingHoursFormSave")
            save.focusPolicy = Qt.NoFocus
            type(day(3), "10-12")
            mouseClick(save)
            compare(mock.saved.length, 1)
            const sent = JSON.parse(mock.saved[0])
            verify(sent.open.indexOf("th 10:00-12:00") >= 0, sent.open)
            verify(sent.open.indexOf("mo 09:00-12:00, 14:00-18:00") >= 0, sent.open)
            verify(sent.note.indexOf("9-12 and 2-5") >= 0 && sent.note.indexOf("Closed in August") === 0, sent.note)
            compare(sent.holidays_open, false)
        }

        // "Open on public holidays": as the card says it; 24/7 ticks it; kept in what is saved (review 4).
        function test_the_public_holidays_tick() {
            const form = createTemporaryObject(formComponent, root, {})
            form.edit("/books/hall.vcf", Object.assign({}, root.hall, { open: Object.assign({}, root.hall.open, { holidays_off: false }) }))
            tryVerify(() => form.opened, 2000)
            waitForRendering(form.contentItem)
            const tick = root.named(form.contentItem, "openingHoursFormHolidays")
            verify(tick.checked)
            mouseClick(tick)
            verify(!tick.checked)
            type(root.named(form.contentItem, "openingHoursFormShort"), "24/7")
            keyClick(Qt.Key_Tab)
            verify(tick.checked, "24/7: open on public holidays")
            const save = root.named(form.footer, "openingHoursFormSave")
            mouseClick(save)
            const sent = JSON.parse(mock.saved[0])
            compare([sent.open, sent.holidays_open], ["24/7", true])
        }
    }
}
