// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What belongs to Sioul as a whole rather than to one page: language and
// colours, working hours and days off, what reaches you and when
// (ReachesTab.qml), the words Sioul looks for (WordsTab.qml), reminders,
// the pauses, on a phone what sets it up
// (PhoneSetup.qml), your folder and sharing, invoices. Each setting says in a
// sentence what it changes and is saved at once.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme

    property var rows: []
    property string problem: ""
    // The sharing panel, made the first time its tab is shown, then kept.
    readonly property var share: shareLoader.item
    property bool shareMade: page.section === "files"
    onSectionChanged: {
        if (page.section === "files")
            page.shareMade = true
    }
    // One tab at a time: how it looks, the hours, what reaches you, the words,
    // reminders, the pauses; on a phone, what sets it up; your folder and sharing, invoices;
    // what AI agents may use (each project closed to them until opened).
    readonly property var sections: ["look", "hours", "attention", "words", "reminders", "pauses"].concat(Qt.platform.os === "android" ? ["phone"] : []).concat(["files", "invoices", "ai"])
    // The pause's screen tried from its setup (main.qml shows it, nothing held).
    signal tryPause
    property string section: "look"
    // What reaches you and the words draw their own rows (ReachesTab.qml,
    // WordsTab.qml): none listed here.
    readonly property var shown: page.section === "attention" || page.section === "words" ? [] : page.rows.filter(r => r.section === page.section)
    // A view of What reaches you asked for before its tab was made: shown once it is.
    property string reachesAsked: ""
    // A list of the words asked for ("words.voicemail.operators") before its tab was made: its line opened once it is.
    property string wordsAsked: ""

    function reload() {
        page.rows = JSON.parse(page.sioul.settings("parameters"))
    }

    function save(key, value) {
        page.problem = page.sioul.setSetting(key, JSON.stringify(value))
        if (page.problem === "")
            page.reload()
    }

    onVisibleChanged: if (visible) page.reload()

    // Further down by a part of the page.
    function scrollBy(part) {
        const bar = scroll.ScrollBar.vertical
        bar.position = Math.min(1 - bar.size, bar.position + part)
    }

    // What reaches you at one of its views ("attention.dnd"), times
    // ("attention.pause") or channels ("attention.calls"); the older screens'
    // keys lead there too ("notify", "dnd.people", "calls").
    function showReaches(key) {
        page.section = "attention"
        scroll.ScrollBar.vertical.position = 0
        page.reachesAsked = key.startsWith("attention.") ? key.slice("attention.".length) : "time"
        if (reaches.item)
            page.showAsked()
    }

    // The line holding a list of the words, opened.
    function showWords() {
        const asked = page.wordsAsked
        page.wordsAsked = ""
        const words = wordsLoader.item as WordsTab
        if (words)
            words.show(asked)
    }

    function showAsked() {
        const asked = page.reachesAsked
        page.reachesAsked = ""
        const views = ["time", "person", "own", "exceptions", "dnd"]
        if (views.includes(asked))
            reaches.item.show(asked, "")
        else if (["mail", "calls", "messages"].includes(asked))
            reaches.item.show("person", asked)
        else
            reaches.item.show("time", asked)
    }

    // One setting in view: where the Porch sends you for the hours. Once the
    // page is laid out, which takes a moment after it shows.
    function showSetting(key) {
        const android = Qt.platform.os === "android"
        const older = { "notify": "attention.own", "dnd": "attention.dnd", "dnd.button": "attention.dnd", "dnd.focus": "attention.dnd", "dnd.pauses": "attention.dnd", "dnd.sleep": "attention.dnd", "dnd.people": "attention.exceptions", "calls": android ? "phone" : "attention.calls", "apps": android ? "phone" : "attention.exceptions" }
        key = older[key] || key
        if (key === "attention" || key.startsWith("attention.")) {
            page.showReaches(key)
            return
        }
        if (key.startsWith("words.")) {
            page.section = "words"
            scroll.ScrollBar.vertical.position = 0
            page.wordsAsked = key
            if (wordsLoader.item)
                page.showWords()
            return
        }
        const row = page.rows.find(r => r.key === key)
        if (row)
            page.section = row.section
        // A tab without settings of its own (Calls): the tab itself.
        else if (page.sections.includes(key))
            page.section = key
        toSetting.key = key
        toSetting.restart()
    }

    Timer {
        id: toSetting

        property string key: ""

        interval: 150
        onTriggered: {
            const index = page.shown.findIndex(r => r.key === toSetting.key)
            const item = index >= 0 ? rowsRepeater.itemAt(index) : null
            const flick = scroll.contentItem as Flickable
            if (item && flick)
                flick.contentY = Math.max(0, Math.min(item.y - 12, flick.contentHeight - flick.height))
        }
    }

    // Sharing, at the end of the files' tab.
    function toEnd() {
        page.section = "files"
        Qt.callLater(() => scroll.ScrollBar.vertical.position = 1 - scroll.ScrollBar.vertical.size)
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        anchors.margins: page.theme.gap
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            // What reaches you's cards sit three in a row where they can.
            width: Math.min(scroll.availableWidth, page.section === "attention" ? 1100 : 720)
            spacing: 6

            Label {
                Layout.fillWidth: true
                text: page.sioul.text("ui-parameters")
                font.pixelSize: 22
                color: page.theme.text
            }
            // The tabs, as the task page's views: the one shown filled.
            Flow {
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: page.sections

                    delegate: Button {
                        id: tab

                        required property string modelData

                        text: page.sioul.text("settings-tab-" + modelData)
                        checkable: true
                        checked: page.section === modelData
                        flat: page.section !== modelData
                        onClicked: {
                            page.section = modelData
                            // A click on the tab shown would untick it: it stays the one shown.
                            tab.checked = Qt.binding(() => page.section === tab.modelData)
                        }
                    }
                }
            }
            Label {
                visible: page.problem !== ""
                Layout.fillWidth: true
                text: page.problem
                wrapMode: Text.Wrap
                color: page.theme.warm
            }

            Repeater {
                id: rowsRepeater

                model: page.shown

                delegate: ColumnLayout {
                    id: row

                    required property var modelData
                    required property int index
                    readonly property bool newGroup: row.modelData.group !== "" && (row.index === 0 || page.shown[row.index - 1].group !== row.modelData.group)

                    Layout.fillWidth: true
                    Layout.topMargin: row.newGroup ? 18 : 8
                    spacing: 3

                    Label {
                        visible: row.newGroup
                        Layout.fillWidth: true
                        text: row.modelData.group
                        font.pixelSize: 17
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                        color: page.theme.accent
                    }
                    SettingRow {
                        Layout.fillWidth: true
                        setting: row.modelData
                        sioul: page.sioul
                        theme: page.theme
                        onSave: (key, value) => page.save(key, value)
                    }
                }
            }

            // The pauses' do-not-disturb, a try-out of the screen, the last pause forgotten (docs/pauses.md).
            Loader {
                active: page.section === "pauses"
                visible: active
                Layout.fillWidth: true

                sourceComponent: Component {
                    PauseSetup {
                        sioul: page.sioul
                        theme: page.theme
                        deviceShown: false
                        onTried: page.tryPause()
                    }
                }
            }

            // What reaches you, and when: one place (docs/attention.md).
            Loader {
                id: reaches

                active: page.section === "attention"
                visible: active
                Layout.fillWidth: true
                onLoaded: {
                    if (page.reachesAsked !== "")
                        page.showAsked()
                }

                sourceComponent: Component {
                    ReachesTab {
                        id: reachesTab

                        sioul: page.sioul
                        theme: page.theme
                        rows: page.rows.filter(r => r.section === "attention")
                        onSave: (key, value) => {
                            page.save(key, value)
                            reachesTab.reload()
                        }
                        onToTop: scroll.ScrollBar.vertical.position = 0
                    }
                }
            }

            // The words Sioul looks for (docs/words.md): the languages, then a line per thing recognised.
            Loader {
                id: wordsLoader

                active: page.section === "words"
                visible: active
                Layout.fillWidth: true
                onLoaded: {
                    if (page.wordsAsked !== "")
                        page.showWords()
                }

                sourceComponent: Component {
                    WordsTab {
                        sioul: page.sioul
                        theme: page.theme
                        rows: page.rows.filter(r => r.section === "words")
                        onSave: (key, value) => page.save(key, value)
                        onToTop: scroll.ScrollBar.vertical.position = 0
                    }
                }
            }

            // This phone: what sets it up, nothing that decides when (docs/android.md).
            Loader {
                active: page.section === "phone"
                visible: active
                Layout.fillWidth: true

                sourceComponent: Component {
                    PhoneSetup {
                        sioul: page.sioul
                        theme: page.theme
                    }
                }
            }

            Loader {
                id: shareLoader

                active: page.shareMade
                visible: page.section === "files"
                Layout.fillWidth: true
                Layout.bottomMargin: 24

                sourceComponent: Component {
                    SharePanel {
                        sioul: page.sioul
                        theme: page.theme
                    }
                }
            }
        }
    }
}
