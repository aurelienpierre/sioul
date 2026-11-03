// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The PDF view (PdfView.qml) on a stand-in document of one page and three
// links (data/links.pdf): a link to the web shows where it goes and opens
// only with Open; an address goes to the writing window with Open; a file:
// link is not followed, and the bar says so (decided 3 November 2026).
// Nothing opens by a tap alone. Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

Item {
    id: root

    width: 420
    height: 700

    QtObject {
        id: mock

        property var opened: []
        property var written: []

        function text(id) {
            return id
        }
        function textArgs(id, args) {
            const a = JSON.parse(args)
            return id + "|" + (a.host || "") + "|" + (a.url || "")
        }
        function writeTo(address) {
            mock.written = mock.written.concat([address])
        }
    }

    PdfView {
        id: view

        anchors.fill: parent
        source: Qt.resolvedUrl("data/links.pdf")
        sioul: mock
        window: mock
        openExternally: url => mock.opened = mock.opened.concat([url])
    }

    TestCase {
        name: "Pdf"
        when: windowShown

        function links() {
            const out = []
            const walk = item => {
                if (item.objectName === "pdfLink")
                    out.push(item)
                for (let i = 0; i < item.children.length; ++i)
                    walk(item.children[i])
            }
            walk(view)
            return out
        }

        function link(prefix) {
            return links().find(l => String(l.url).startsWith(prefix))
        }

        function find(name) {
            let found = null
            const walk = item => {
                if (item.objectName === name)
                    found = item
                for (let i = 0; i < item.children.length && found === null; ++i)
                    walk(item.children[i])
            }
            walk(view)
            return found
        }

        function init() {
            view.asked = null
            mock.opened = []
            mock.written = []
            tryVerify(() => links().length === 3, 5000, "the stand-in's three links")
        }

        function test_a_web_link_shows_where_it_goes_and_opens_only_with_open() {
            const web = link("https:")
            mouseClick(web)
            tryVerify(() => view.asked !== null)
            compare(mock.opened, [], "nothing opens by a tap")
            const words = find("pdfLinkWords")
            compare(words.text, "pdf-link-ask|example.org|https://example.org/page?x=1")
            verify(find("pdfLinkOpen").visible)
            mouseClick(find("pdfLinkOpen"))
            compare(mock.opened, ["https://example.org/page?x=1"])
            compare(view.asked, null, "asked once")
        }

        function test_cancel_opens_nothing() {
            mouseClick(link("https:"))
            tryVerify(() => view.asked !== null)
            mouseClick(find("pdfLinkCancel"))
            compare(view.asked, null)
            compare(mock.opened, [])
        }

        function test_an_address_goes_to_the_writing_window() {
            mouseClick(link("mailto:"))
            tryVerify(() => view.asked !== null)
            compare(find("pdfLinkWords").text, "pdf-link-ask|ana@example.org|mailto:ana@example.org")
            mouseClick(find("pdfLinkOpen"))
            compare(mock.written, ["ana@example.org"])
            compare(mock.opened, [], "never the browser")
        }

        function test_a_file_link_is_not_followed() {
            mouseClick(link("file:"))
            tryVerify(() => view.asked !== null)
            compare(find("pdfLinkWords").text, "pdf-link-refused||file:///etc/passwd")
            verify(!find("pdfLinkOpen").visible, "no Open")
            view.open()
            compare(mock.opened, [])
            compare(mock.written, [])
        }
    }
}
