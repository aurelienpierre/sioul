// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A row of tabs: the one way a page switches between its views (Settings'
// sections, Tasks' views, Accounts, Money, the Agenda's views, Time's period;
// docs/qt-quick.md, "Tabs"). Drawn as tabs, never as buttons: a thin rule
// under the whole row; the tab shown marked by a thick accent line under it
// and strong words; the others in muted words, darker with a fine line under
// the pointer; no fill. Not Qt's TabButton, whose Basic look fills the tabs
// not chosen with the accent under Sioul's palette.
//
// It spans the width it is given. Each tab takes an equal share while every
// name fits in one; else each takes its name's width and the room left is
// shared among them; when the names are wider than the row, it scrolls
// sideways (a finger, the wheel) and keeps the tab shown in sight. Left and
// Right move to the tab before or after (around the ends), Home and End to
// the first and the last, while the row has the focus.
//
// Each tab is named `<objectName>:<id>` ("moneyTabs:overview"; "tab:<id>"
// without one), for tests; screen readers hear a list of page tabs, the one
// shown checked.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

Control {
    id: tabs

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The tabs, in order: { id, text }. `text` is fixed words (a translation), or made plain (Theme.plain).
    property var model: []
    // The id of the tab shown.
    property string current: ""
    // A tab chosen (a click, a tap, an arrow key): its id. The page shows it, and sets `current`.
    signal chosen(string id)

    // Each tab's room on either side of its name: narrower on a phone.
    readonly property int sidePadding: tabs.theme.compact ? 10 : 14
    readonly property int shownIndex: tabs.model.findIndex(t => t.id === tabs.current)
    // Each name's width at its strongest (the tab shown is bold): the row never moves as you choose.
    readonly property var naturals: tabs.model.map(t => Math.ceil(metrics.advanceWidth(String(t.text))) + 2 * tabs.sidePadding)
    readonly property real naturalTotal: tabs.naturals.reduce((sum, w) => sum + w, 0)
    readonly property real share: tabs.model.length > 0 ? tabs.availableWidth / tabs.model.length : 0
    // "equal": every name fits in an equal share; "spread": each its name's width and an equal
    // part of what is left; "scroll": the names are wider than the row, which scrolls.
    readonly property string fit: tabs.naturals.every(w => w <= tabs.share) ? "equal" : tabs.naturalTotal <= tabs.availableWidth ? "spread" : "scroll"
    readonly property var widths: tabs.fit === "equal" ? tabs.naturals.map(() => tabs.share) : tabs.fit === "spread" ? tabs.naturals.map(w => w + (tabs.availableWidth - tabs.naturalTotal) / tabs.model.length) : tabs.naturals
    // Whether the row scrolls sideways: the names do not fit.
    readonly property bool scrolls: tabs.fit === "scroll"
    // The row, for tests: how far it is scrolled, and its tabs.
    readonly property alias strip: strip
    readonly property alias tabRepeater: tabRepeater

    // The tab before (-1) or after (1) the one shown, around the ends.
    function step(by: int) {
        if (tabs.model.length === 0)
            return
        const from = tabs.shownIndex < 0 ? 0 : tabs.shownIndex
        const to = ((from + by) % tabs.model.length + tabs.model.length) % tabs.model.length
        tabs.chosen(tabs.model[to].id)
    }

    // The tab shown brought into sight, a little of its neighbours with it.
    function reveal() {
        const tab = tabRepeater.itemAt(tabs.shownIndex)
        if (!tab || !tabs.scrolls) {
            strip.contentX = 0
            return
        }
        const margin = Math.min(24, Math.max(0, (strip.width - tab.width) / 2))
        const most = Math.max(0, strip.contentWidth - strip.width)
        if (tab.x - margin < strip.contentX)
            strip.contentX = Math.max(0, tab.x - margin)
        else if (tab.x + tab.width + margin > strip.contentX + strip.width)
            strip.contentX = Math.min(most, tab.x + tab.width + margin - strip.width)
    }

    onShownIndexChanged: Qt.callLater(tabs.reveal)
    onWidthsChanged: Qt.callLater(tabs.reveal)

    implicitWidth: tabs.naturalTotal
    implicitHeight: 40
    padding: 0
    // The row takes the focus as a whole, by Tab or a click; the arrows then move between its tabs.
    focusPolicy: Qt.StrongFocus
    Accessible.role: Accessible.PageTabList

    Keys.onLeftPressed: event => {
        tabs.step(tabs.mirrored ? 1 : -1)
        event.accepted = true
    }
    Keys.onRightPressed: event => {
        tabs.step(tabs.mirrored ? -1 : 1)
        event.accepted = true
    }
    Keys.onPressed: event => {
        if (tabs.model.length === 0)
            return
        if (event.key === Qt.Key_Home) {
            tabs.chosen(tabs.model[0].id)
            event.accepted = true
        } else if (event.key === Qt.Key_End) {
            tabs.chosen(tabs.model[tabs.model.length - 1].id)
            event.accepted = true
        }
    }

    FontMetrics {
        id: metrics

        font.pixelSize: tabs.font.pixelSize
        font.family: tabs.font.family
        font.weight: Font.DemiBold
    }

    // The rule under the whole row.
    background: Item {
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: tabs.theme.pressed
        }
    }

    contentItem: Flickable {
        id: strip

        contentWidth: row.width
        contentHeight: height
        flickableDirection: Flickable.HorizontalFlick
        interactive: tabs.scrolls
        boundsBehavior: Flickable.StopAtBounds
        clip: true
        onWidthChanged: Qt.callLater(tabs.reveal)

        // The wheel, as a mouse turns it, scrolls the row sideways while it scrolls at all.
        WheelHandler {
            enabled: tabs.scrolls
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onWheel: event => {
                const delta = Math.abs(event.angleDelta.x) > Math.abs(event.angleDelta.y) ? event.angleDelta.x : event.angleDelta.y
                strip.contentX = Math.max(0, Math.min(strip.contentWidth - strip.width, strip.contentX - delta / 2))
            }
        }

        Row {
            id: row

            height: strip.height

            Repeater {
                id: tabRepeater

                model: tabs.model

                delegate: AbstractButton {
                    id: tab

                    required property var modelData
                    required property int index
                    readonly property bool shown: tab.index === tabs.shownIndex

                    objectName: (tabs.objectName !== "" ? tabs.objectName : "tab") + ":" + tab.modelData.id
                    width: tabs.widths[tab.index] || 0
                    height: row.height
                    // The row holds the focus, not each tab: the arrows move between them.
                    focusPolicy: Qt.NoFocus
                    hoverEnabled: true
                    text: String(tab.modelData.text)
                    Accessible.role: Accessible.PageTab
                    Accessible.name: tab.text
                    Accessible.checkable: true
                    Accessible.checked: tab.shown
                    onClicked: {
                        // A click gives the row the focus for the arrows, as a tab bar does; a finger, not.
                        if (!tabs.theme.touch)
                            tabs.forceActiveFocus(Qt.MouseFocusReason)
                        tabs.chosen(tab.modelData.id)
                    }

                    contentItem: Text {
                        text: tab.text
                        textFormat: Text.PlainText
                        font.pixelSize: tabs.font.pixelSize
                        font.family: tabs.font.family
                        font.weight: tab.shown ? Font.DemiBold : Font.Normal
                        color: tab.shown || tab.hovered || tab.down ? tabs.theme.text : tabs.theme.muted
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                    }

                    background: Item {
                        // The tab shown: a thick line in the accent; under the pointer, a fine one.
                        Rectangle {
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            height: tab.shown ? 3 : 2
                            visible: tab.shown || tab.hovered || tab.down
                            color: tab.shown ? tabs.theme.accent : tabs.theme.pressed
                        }
                        // The focus, from the keyboard: a ring around the tab shown.
                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: 2
                            visible: tab.shown && tabs.visualFocus
                            color: "transparent"
                            radius: tabs.theme.radius
                            border.width: 2
                            border.color: tabs.theme.focus
                        }
                    }
                }
            }
        }
    }
}
