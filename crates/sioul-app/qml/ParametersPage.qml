// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What belongs to Sioul as a whole rather than to one page: language and
// colours, working hours and days off, who may write to you when, sharing
// between your computers, the antivirus, connections asked for. Each setting
// says in a sentence what it changes and is saved at once.

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
    // One tab at a time: how it looks, the hours, reminders and notifications,
    // your folder and sharing, invoices.
    readonly property var sections: ["look", "hours", "reminders", "pauses", "dnd", "files", "invoices"]
    // The pause's screen tried from its setup (main.qml shows it, nothing held).
    signal tryPause
    property string section: "look"
    readonly property var shown: page.rows.filter(r => r.section === page.section)

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

    // One setting in view: where the Porch sends you for the hours. Once the
    // page is laid out, which takes a moment after it shows.
    function showSetting(key) {
        const row = page.rows.find(r => r.key === key)
        if (row)
            page.section = row.section
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
            width: Math.min(scroll.availableWidth, 720)
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
                        onTried: page.tryPause()
                    }
                }
            }

            // Do-not-disturb on every device: this device's line, the list of people, the phone's own (docs/do-not-disturb.md).
            Loader {
                active: page.section === "dnd"
                visible: active
                Layout.fillWidth: true

                sourceComponent: Component {
                    DndSetup {
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
