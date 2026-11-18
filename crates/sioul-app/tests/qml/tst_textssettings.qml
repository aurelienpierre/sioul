// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Texts page's ⚙ (SettingsPanel.qml with `settings::for_view("texts")`):
// its one row draws the multimedia messages sent from here
// (TextsSending.qml, made by its file in SettingRow.qml), with its limit
// and what the phone said of its carrier, on a stand-in for Sioul. Made-up
// carrier. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 600
    height: 700

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        property var asked: []

        signal sharedIn(var stores)

        function text(key) {
            return key
        }
        function settings(view) {
            return JSON.stringify(view !== "texts" ? [] : [{ key: "texts.sending", kind: "textssending", label: "texts-sending-title", help: "", value: "", choices: [], group: "", section: "", min: 0, max: 0, step: 1, unit: "" }])
        }
        function texts(verb, args) {
            mock.asked = mock.asked.concat([verb])
            return JSON.stringify({ limit: { key: "limit", kind: "choice", label: "texts-sending-limit", help: "", value: "carrier", choices: [{ value: "carrier", label: "Up to your carrier's limit (600 kB)" }, { value: "300", label: "300 kB" }] }, phone: ["SIM 1: 600 kB (Example Mobile)"], said: "" })
        }
    }

    SettingsPanel {
        id: panel

        sioul: mock
        theme: testTheme
        view: "texts"
    }

    TestCase {
        name: "TextsSettings"
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

        function test_the_texts_settings_hold_the_multimedia_messages() {
            panel.open()
            tryCompare(panel, "opened", true)
            tryVerify(() => walk(panel.contentItem, i => i instanceof TextsSending, []).length === 1, 2000, "the block, made by its file")
            const block = walk(panel.contentItem, i => i instanceof TextsSending, [])[0]
            verify(block.visible)
            verify(mock.asked.indexOf("sending") >= 0, "what it holds asked of the texts")
            tryVerify(() => walk(block, i => i.objectName === "textsSendingLimit" && i.visible, []).length === 1, 2000, "its limit")
            verify(walk(block, i => i.visible && i.text === "SIM 1: 600 kB (Example Mobile)", []).length === 1, "the phone's word on its carrier")
            compare(walk(panel.contentItem, i => i.visible && i.text === "texts-sending-title", []).length, 1, "its title once")
            panel.close()
        }
    }
}
