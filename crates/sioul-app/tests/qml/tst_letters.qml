// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Porch's paper letters (LettersSection.qml) at a phone's width, 412
// points, on a stand-in for Sioul: a letter with a long sender, kind and
// lines, a date asked and its project field, nothing past the screen's edge
// (review of 1017fa4, L3). Run by tools/qml-test.sh.

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
                    project: ""
                }]
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

    ColumnLayout {
        width: root.width

        LettersSection {
            id: section

            Layout.fillWidth: true
            sioul: mock
            theme: testTheme
            window: root
        }
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

        function test_a_letter_fits_a_phone_s_width() {
            section.reload()
            waitForRendering(section)
            tryVerify(() => section.shown.letters.length === 1)
            const over = past(section, [])
            compare(over.join("; "), "", "nothing past 412 points")
        }
    }
}
