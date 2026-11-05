// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Letters page (LettersPage.qml) and its cards (LettersSection.qml) at a
// phone's width, 412 points, on a stand-in for Sioul: a letter with a long
// sender, kind and lines, a date asked and its project field, a linked one
// moved away since, one filed; nothing past the screen's edge (review of
// 1017fa4, L3); the waiting first, then the filed by year; each one's quiet
// line said once. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 891

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        signal lettersChanged
        signal personsChanged

        // Made up: no real sender, no real person.
        function letters() {
            return JSON.stringify({
                inbox: "/stand-in/letters",
                reader: "",
                missing: "",
                projects: [{ id: "home", title: "Home and its repairs" }],
                letters: [{
                    id: "l1",
                    sender: "Caisse d’allocations familiales de l’Exemple",
                    kind_label: "A request for documents",
                    received: "Wednesday 28 October",
                    lines: ["By Wednesday 18 November: send the requested documents", "Reference 0123456789 for the file of the household"],
                    problem: "",
                    file: "/stand-in/letters/scan.pdf",
                    deadline: "2026-11-18",
                    task: "",
                    appointment: true,
                    project: "",
                    linked: false,
                    gone: false,
                    waiting: false
                }, {
                    id: "l2",
                    sender: "Water board",
                    kind_label: "A bill",
                    received: "Monday 2 November",
                    lines: [],
                    problem: "",
                    file: "/stand-in/Documents/a very long folder name for the scans kept elsewhere/water.pdf",
                    deadline: "",
                    task: "",
                    appointment: false,
                    project: "",
                    linked: true,
                    gone: true,
                    waiting: false
                }],
                filed: [{ year: "2025", letters: [{ id: "l0", sender: "Tax office", kind_label: "A tax notice", received: "Friday 12 September 2025", lines: [], problem: "", file: "/stand-in/letters/2025/notice.pdf", deadline: "", task: "", appointment: false, project: "", linked: false, gone: false, waiting: false }] }]
            })
        }
        function lettersFolder() {
            return "/stand-in/letters"
        }
        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ": " + value
        }
    }

    LettersPage {
        id: page

        anchors.fill: parent
        sioul: mock
        theme: testTheme
        window: root
    }

    TestCase {
        name: "Letters"
        when: windowShown

        // Every item shown, past the right edge of the screen: its name and its right edge.
        function past(item, found) {
            if (!item || !item.visible)
                return found
            const right = item.mapToItem(root, item.width, 0).x
            if (item.width > 0 && right > root.width + 0.5)
                found.push((item.objectName || item.text || item.toString()) + " ends at " + Math.round(right))
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                past(kids[i], found)
            if (item.contentItem && item.contentItem !== item)
                past(item.contentItem, found)
            return found
        }

        // Every label shown under `item`, in the order drawn, each item once
        // (a view's content item is among its children too).
        function labels(item, found, seen) {
            seen = seen || new Set()
            if (!item || !item.visible || seen.has(item))
                return found
            seen.add(item)
            if (item.text !== undefined && item.text !== "" && item instanceof Label)
                found.push(item.text)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                labels(kids[i], found, seen)
            if (item.contentItem && item.contentItem !== item)
                labels(item.contentItem, found, seen)
            return found
        }

        function test_a_letter_fits_a_phone_s_width() {
            page.reload()
            waitForRendering(page)
            tryVerify(() => page.section.shown.letters.length === 2)
            const over = past(page, [])
            compare(over.join("; "), "", "nothing past 412 points")
        }

        // The waiting first, the filed after by year; a linked scan moved away said once, calmly.
        function test_waiting_then_filed_and_a_scan_gone() {
            page.reload()
            waitForRendering(page)
            const shown = labels(page, [])
            const waiting = shown.indexOf("Caisse d’allocations familiales de l’Exemple · A request for documents")
            const filed = shown.indexOf("letters-filed")
            const year = shown.indexOf("2025")
            verify(waiting >= 0 && filed > waiting && year > filed, shown.join(" | "))
            verify(shown.indexOf("Tax office · A tax notice") > year)
            compare(shown.filter(t => t.startsWith("letters-gone")).length, 1, "the scan moved away, said once")
            compare(shown.filter(t => t.startsWith("letters-linked-here")).length, 0, "gone says it, not linked as well")
        }
    }
}
