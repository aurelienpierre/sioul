// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The notes of a task or an event, read (Description.qml), on a stand-in for
// Sioul that draws them as Rust does (description::description_html, whose
// own tests show that no HTML and no picture come through): a link opens
// outside Sioul, and only once where it leads is said; nothing but the web,
// an address or a number opens. Run by tools/qml-test.sh.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml"

Item {
    id: root

    width: 600
    height: 400

    Theme {
        id: testTheme
    }

    // A stand-in for Sioul: its sentences are their keys; the notes drawn as
    // Rust draws "Join [the call](https://meet.example.org/abc) or write".
    QtObject {
        id: mock

        property var drawn: []

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function descriptionHtml(markdown) {
            mock.drawn.push(markdown)
            return "<p>Join <a href=\"https://meet.example.org/abc\">the call</a> or <a href=\"mailto:desk@example.org?subject=Hi\">write</a></p>\n"
        }
    }

    // The window: a new message to an address.
    QtObject {
        id: stand

        property var written: []

        function writeTo(address) {
            stand.written.push(address)
        }
    }

    ColumnLayout {
        width: root.width

        Description {
            id: notes

            Layout.fillWidth: true
            sioul: mock
            theme: testTheme
            window: stand
            markdown: "Join [the call](https://meet.example.org/abc) or [write](mailto:desk@example.org?subject=Hi)"
        }
    }

    TestCase {
        name: "Description"
        when: windowShown

        function init() {
            notes.pointed = ""
            stand.written = []
        }

        function test_drawn_from_rust_read_only() {
            const words = notes.drawn
            verify(words.indexOf("meet.example.org/abc") >= 0, words)
            verify(words.indexOf("color:" + testTheme.accent) >= 0, "links in the theme's colour")
            verify(mock.drawn.indexOf(notes.markdown) >= 0, "the Markdown went to Rust")
            const asked = mock.drawn.length
            notes.markdown = "  "
            verify(notes.drawn.indexOf("meet.example.org") < 0, "empty notes draw nothing")
            compare(mock.drawn.length, asked, "nor ask Rust")
            notes.markdown = "Join [the call](https://meet.example.org/abc) or [write](mailto:desk@example.org?subject=Hi)"
        }

        function test_a_link_says_where_it_leads_then_opens() {
            const opened = []
            notes.opener = url => opened.push(url)
            const line = findChild(notes, "descriptionLink")
            verify(line !== null && !line.visible)
            // A first tap says where it leads; it opens at the second.
            notes.follow("https://meet.example.org/abc")
            compare(opened, [])
            verify(line.visible)
            compare(line.text, "ui-link https://meet.example.org/abc")
            notes.follow("https://meet.example.org/abc")
            compare(opened, ["https://meet.example.org/abc"])
            // Under the pointer it is said already: one click opens it.
            notes.pointed = "https://example.org/other"
            notes.follow("https://example.org/other")
            compare(opened.length, 2)
            notes.opener = url => Qt.openUrlExternally(url)
        }

        function test_nothing_else_opens() {
            const opened = []
            notes.opener = url => opened.push(url)
            for (const url of ["javascript:alert(1)", "file:///etc/passwd", "data:text/html,x", "smb://host/share", "other.md", "sioul:task/x"]) {
                notes.follow(url)
                notes.follow(url)
                compare(notes.pointed, "", url + " is not even said")
            }
            compare(opened, [])
            compare(stand.written, [])
            notes.opener = url => Qt.openUrlExternally(url)
        }

        function test_an_address_opens_a_new_message() {
            notes.follow("mailto:desk%40example.org?subject=Hi")
            notes.follow("mailto:desk%40example.org?subject=Hi")
            compare(stand.written, ["desk@example.org"])
        }

        function test_new_notes_forget_the_link_said() {
            notes.follow("https://meet.example.org/abc")
            verify(notes.pointed !== "")
            notes.markdown = "Other notes"
            compare(notes.pointed, "")
            notes.markdown = "Join [the call](https://meet.example.org/abc) or [write](mailto:desk@example.org?subject=Hi)"
        }
    }
}
