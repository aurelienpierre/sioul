// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What a screen reader hears of a file a site offers (DownloadCard.qml, by
// way of SiteDownloads.qml), with an assistive tool listening
// (qml-test: accessibility; tools/qml-test.sh starts it so): the card's sections
// headed, each control named, each place described by its line, why a place
// cannot take a program, and a missing name said as an alert. On the
// stand-ins of DownloadsStandIn.qml, at a phone's width.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 860

    Theme {
        id: testTheme

        compact: true
    }

    DownloadsStandIn {
        id: mock
    }

    // A probe of the assistive tool: Qt writes a button's own state over what QML said of it
    // once a tool listens. Not checkable, said checkable: told so only while no tool listens.
    AbstractButton {
        id: probe

        y: 840
        text: "probe"
        Accessible.checkable: true
    }

    SiteDownloads {
        id: downloads

        anchors.fill: parent
        sioul: mock
        theme: testTheme
    }

    TestCase {
        name: "DownloadsAccess"
        when: windowShown

        function walk(item, test, found) {
            if (!item)
                return found
            if (test(item))
                found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                walk(kids[i], test, found)
            if (item.contentItem && item.contentItem !== item && kids.indexOf(item.contentItem) < 0)
                walk(item.contentItem, test, found)
            if (item.footer && kids.indexOf(item.footer) < 0)
                walk(item.footer, test, found)
            return found
        }
        function named(name) {
            return walk(downloads.card, i => i.objectName === name && i.visible, [])[0]
        }
        function offered(name, mime) {
            downloads.offer(mock.request(name, mime), { id: "bank", name: "My bank" }, "https://files.example.org/statements", null)
            tryVerify(() => downloads.card !== null && downloads.card.opened, 2000, "the card asks")
            waitForRendering(downloads.card.contentItem, 2000)
        }

        function test_a_screen_reader_hears_the_card() {
            // Run with an assistive tool listening (tools/qml-test.sh), or this test proves nothing.
            verify(!probe.Accessible.checkable, "an assistive tool listens: Qt said the probe as it is")
            offered("setup.exe", "application/x-msdownload")
            verify(named("downloadCaution").visible)
            compare(named("downloadCaution").text, "download-program files.example.org")
            compare(named("downloadName").Accessible.name, "download-name")
            for (const id of ["downloads", "folder", "notes", "papers", "letters"]) {
                const radio = named("downloadPlace:" + id)
                compare(radio.Accessible.name, "download-place-" + id)
                compare(radio.Accessible.description, (["notes", "papers", "letters"].indexOf(id) >= 0 ? "not " : "about ") + id)
            }
            verify(!named("downloadPlace:notes").enabled && !named("downloadPlace:papers").enabled && !named("downloadPlace:letters").enabled, "a program goes to no place in Sioul")
            compare(named("downloadSave").text, "ui-save")
            compare(named("downloadCancel").text, "ui-cancel")
            compare(walk(downloads.card, i => i !== downloads.card && i.visible && i.Accessible.role === Accessible.Heading, []).map(h => h.text), ["download-what", "download-where"], "headed sections")
            // A name missing: said, as an alert.
            named("downloadName").text = " "
            mouseClick(named("downloadSave"))
            compare(named("downloadProblem").Accessible.role, Accessible.AlertMessage)
            compare(named("downloadProblem").Accessible.name, "download-no-name")
            downloads.card.cancel()
        }

        function test_a_site_s_link_is_named() {
            offered("one.pdf")
            downloads.card.cancel()
            offered("two.pdf")
            compare(named("downloadRefuseSite").Accessible.name, "download-refuse-site")
            compare(named("downloadWaits").text, "download-waits")
            downloads.card.cancel()
            compare(mock.viewReads, 0)
        }
    }
}
