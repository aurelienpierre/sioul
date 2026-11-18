// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What belongs to Sioul as a whole rather than to one page: language and
// colours, working hours and days off, what reaches you and when
// (ReachesTab.qml), the words Sioul looks for (WordsTab.qml), reminders,
// the pauses, on a phone what sets it up
// (PhoneSetup.qml), on a computer the multimedia messages sent from it
// (TextsSending.qml), your folder and sharing, your devices (DevicesTab.qml),
// invoices. Each setting says in a sentence what it changes and is saved at once.
//
// On a computer, the sections are tabs across the page (Tabs.qml), and each
// section's rows sit in a centred column of legible width (PageColumn.qml),
// grouped under their headings in frames (Section.qml); What reaches you and
// Devices, tables and grids, take the whole width. On a phone, the page
// opens on the list of its sections, each with a line saying what it holds;
// a tap opens one, and Back (the system's, or the arrow in the page's bar)
// returns to the list. A link to a setting opens its section, there as here.

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
    // The devices' tab, while it is shown.
    readonly property var devices: devicesLoader.item
    property bool shareMade: page.section === "files"
    onSectionChanged: {
        if (page.section === "files")
            page.shareMade = true
        // A section asked for (a link, a step of the pictures) opens, on a phone too.
        page.listShown = false
    }
    // One tab at a time: how it looks, the hours, what reaches you, the words,
    // reminders, the pauses; on Android, what sets it up (its system's); where
    // there is no telephony (a computer, a tablet without a SIM), the texts it
    // sends through the phone; your folder and sharing, your devices,
    // invoices; what AI agents may use (each project closed to them until opened).
    readonly property var sections: ["look", "hours", "attention", "words", "reminders", "pauses"].concat(Qt.platform.os === "android" ? ["phone"] : []).concat(page.sioul.telephony === true ? [] : ["texts"]).concat(["files", "devices", "invoices", "ai"])
    // A tab's name's key: Android's own tab named by what the device is ("This
    // phone", "This tablet", else "This device"), never by its system.
    function tabKey(section) {
        if (section !== "phone")
            return "settings-tab-" + section
        const kind = page.sioul.deviceKind
        return kind === "tablet" ? "settings-tab-tablet" : kind === "computer" ? "settings-tab-device" : "settings-tab-phone"
    }
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
    // A phone: the sections as a list, one open at a time.
    readonly property bool phone: page.theme.compact
    // On a phone, the list of sections shows, rather than the one open.
    property bool listShown: true
    readonly property bool listed: page.phone && page.listShown
    // On a phone, a section open: Back returns to the list (main.qml's `goBack`).
    readonly property bool canGoBack: page.phone && !page.listShown
    function back() {
        page.listShown = true
    }
    // The rows shown, gathered by heading, in their order: one frame each (Section.qml).
    readonly property var groups: {
        const groups = []
        for (const row of page.shown) {
            const last = groups.length > 0 ? groups[groups.length - 1] : null
            if (last !== null && last.group === row.group)
                last.rows.push(row)
            else
                groups.push({ group: row.group, rows: [row] })
        }
        return groups
    }

    // A section shown, from its tab or the phone's list: from its top.
    function open(section) {
        page.section = section
        page.listShown = false
        scroll.ScrollBar.vertical.position = 0
    }

    function reload() {
        page.rows = JSON.parse(page.sioul.settings("parameters"))
    }

    // `shown`: the value the row showed: a list's change is set over the files as they are now.
    function save(key, value, shown) {
        page.problem = shown === undefined ? page.sioul.setSetting(key, JSON.stringify(value)) : page.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
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
        page.listShown = false
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
        // Calls are set up where they are screened (telephony); other apps' notifications on Android.
        const android = Qt.platform.os === "android"
        const older = { "notify": "attention.own", "dnd": "attention.dnd", "dnd.button": "attention.dnd", "dnd.focus": "attention.dnd", "dnd.pauses": "attention.dnd", "dnd.sleep": "attention.dnd", "dnd.people": "attention.exceptions", "calls": page.sioul.telephony === true ? "phone" : "attention.calls", "apps": android ? "phone" : "attention.exceptions" }
        key = older[key] || key
        if (key === "attention" || key.startsWith("attention.")) {
            page.showReaches(key)
            return
        }
        if (key.startsWith("words.")) {
            page.section = "words"
            page.listShown = false
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
        page.listShown = false
        toSetting.key = key
        toSetting.restart()
    }

    Timer {
        id: toSetting

        property string key: ""

        interval: 150
        onTriggered: {
            const item = page.rowItem(column, toSetting.key)
            const flick = scroll.contentItem as Flickable
            if (item && flick)
                flick.contentY = Math.max(0, Math.min(item.mapToItem(column, 0, 0).y - 12, flick.contentHeight - flick.height))
        }
    }

    // A setting's row, by its key, among the frames (`setting:<key>`).
    function rowItem(item, key) {
        if (item.objectName === "setting:" + key)
            return item
        for (let i = 0; i < item.children.length; i++) {
            const found = page.rowItem(item.children[i], key)
            if (found)
                return found
        }
        return null
    }

    // Sharing, at the end of the files' tab.
    function toEnd() {
        page.section = "files"
        page.listShown = false
        Qt.callLater(() => scroll.ScrollBar.vertical.position = 1 - scroll.ScrollBar.vertical.size)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: 6

        // "Settings"; on a phone, the section open, under the bar's "Settings".
        Label {
            Layout.fillWidth: true
            text: page.phone && !page.listShown ? page.sioul.text(page.tabKey(page.section)) : page.sioul.text("ui-parameters")
            font.pixelSize: 22
            wrapMode: Text.Wrap
            color: page.theme.text
        }
        // The sections, across the page (a computer's).
        Tabs {
            objectName: "settingsTabs"
            visible: !page.phone
            Layout.fillWidth: true
            theme: page.theme
            model: page.sections.map(s => ({ id: s, text: page.sioul.text(page.tabKey(s)) }))
            current: page.section
            onChosen: id => page.open(id)
        }

        // A phone's list of the sections: each its name and what it holds.
        ScrollView {
            id: list

            visible: page.listed
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                width: list.availableWidth
                spacing: 0

                Repeater {
                    model: page.listed ? page.sections : []

                    delegate: AbstractButton {
                        id: entry

                        required property string modelData

                        objectName: "settingsSection:" + entry.modelData
                        Layout.fillWidth: true
                        hoverEnabled: true
                        padding: 12
                        leftPadding: 4
                        rightPadding: 4
                        text: page.sioul.text(page.tabKey(entry.modelData))
                        Accessible.role: Accessible.Button
                        Accessible.name: entry.text
                        Accessible.description: page.sioul.text("settings-about-" + entry.modelData)
                        onClicked: page.open(entry.modelData)

                        contentItem: RowLayout {
                            spacing: 10

                            ColumnLayout {
                                Layout.fillWidth: true
                                // What the row leaves it, its words wrapping: it never widens the row.
                                Layout.preferredWidth: 0
                                spacing: 2

                                Label {
                                    Layout.fillWidth: true
                                    text: entry.text
                                    textFormat: Text.PlainText
                                    font.pixelSize: 17
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: page.sioul.text("settings-about-" + entry.modelData)
                                    textFormat: Text.PlainText
                                    font.pixelSize: 13
                                    wrapMode: Text.Wrap
                                    color: page.theme.muted
                                }
                            }
                            Icon {
                                iconName: "go-next"
                                size: 16
                            }
                        }
                        background: Rectangle {
                            color: entry.down ? page.theme.hover : "transparent"

                            Rectangle {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                height: 1
                                color: page.theme.line
                            }
                        }
                    }
                }
            }
        }

        ScrollView {
            id: scroll

            visible: !page.listed
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.topMargin: page.phone ? 0 : 6
            contentWidth: availableWidth
            clip: true

            // The section's rows in a legible column, centred; What reaches you and Devices, the whole width.
            PageColumn {
                id: column

                theme: page.theme
                room: scroll.availableWidth
                wide: page.section === "attention" || page.section === "devices"
                spacing: 6

                Label {
                    visible: page.problem !== ""
                    Layout.fillWidth: true
                    text: page.problem
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.warm
                }

                // Each heading's rows in a frame of their own.
                Repeater {
                    model: page.groups

                    delegate: Section {
                        id: group

                        required property var modelData
                        required property int index

                        Layout.fillWidth: true
                        Layout.topMargin: group.index === 0 ? 6 : 14
                        theme: page.theme
                        title: page.theme.plain(group.modelData.group)
                        body.spacing: 14

                        Repeater {
                            model: group.modelData.rows

                            delegate: SettingRow {
                                required property var modelData

                                objectName: "setting:" + modelData.key
                                Layout.fillWidth: true
                                setting: modelData
                                sioul: page.sioul
                                theme: page.theme
                                onSave: (key, value, shown) => page.save(key, value, shown)
                            }
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
                            onSave: (key, value, shown) => {
                                page.save(key, value, shown)
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
                            onSave: (key, value, shown) => page.save(key, value, shown)
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

                // The multimedia messages sent from this computer (docs/texts.md).
                Loader {
                    active: page.section === "texts"
                    visible: active
                    Layout.fillWidth: true

                    sourceComponent: Component {
                        TextsSending {
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
                            onDevicesAsked: page.showSetting("devices")
                        }
                    }
                }

                // Your devices: what each is, when it last opened and shared, what you
                // say it is for (docs/database.md, "Devices").
                Loader {
                    id: devicesLoader

                    active: page.section === "devices"
                    visible: active
                    Layout.fillWidth: true

                    sourceComponent: Component {
                        DevicesTab {
                            sioul: page.sioul
                            theme: page.theme
                            kindRow: page.rows.find(r => r.key === "device_kind") || null
                            onSave: (key, value, shown) => page.save(key, value, shown)
                            onSharingAsked: page.showSetting("files")
                        }
                    }
                }
            }
        }
    }
}
