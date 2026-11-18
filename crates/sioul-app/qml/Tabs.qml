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
// It spans the width it is given, less what a page keeps at its end
// (`rightPadding`, for a ⚙). Each tab takes an equal share while every name
// fits in one; else each takes its name's width and the room left is shared
// among them, their padding narrowed before the row scrolls. When the names
// are wider than the row even so, it scrolls sideways (a finger, the wheel),
// and says so: the side that has more fades out, the tab at that edge shows
// cut, never whole, and on a computer a chevron there brings the next tab
// into view. The tab shown is kept in sight. The names are measured in the font the row has now: a font changed
// later is measured again.
//
// The keyboard's focus is on the tab shown: Tab reaches it, Left and Right
// choose the tab before or after (around the ends), Home and End the first and
// the last, and the focus follows. Screen readers hear a list of page tabs
// named `name`, each tab by its words, the one shown checked and selected.
// Each tab is named `<objectName>:<id>` ("moneyTabs:overview"; "tab:<id>"
// without one), for tests.

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
    // What the row switches, for screen readers: the page's name.
    property string name: ""
    // What lies behind the row, which its cut edges fade into: the page's ground.
    property color ground: tabs.theme.background
    // A tab chosen (a click, a tap, an arrow key): its id. The page shows it, and sets `current`.
    signal chosen(string id)

    // Each tab's room on either side of its name: narrower on a phone; at least
    // `leastPadding` before the row scrolls (as many names in sight as a row of buttons had).
    readonly property int sidePadding: tabs.theme.compact ? 10 : 14
    readonly property int leastPadding: 6
    readonly property int shownIndex: tabs.model.findIndex(t => t.id === tabs.current)
    // Each name's width at its strongest (the tab shown is bold): the row never moves as you
    // choose. `metrics.font` is read so that the names are measured again when the font
    // changes: a call to advanceWidth() alone is read once, in the font of then (62a86f5f).
    readonly property var wordWidths: {
        const font = metrics.font
        return font ? tabs.model.map(t => Math.ceil(metrics.advanceWidth(String(t.text)))) : []
    }
    readonly property var naturals: tabs.wordWidths.map(w => w + 2 * tabs.sidePadding)
    readonly property real naturalTotal: tabs.naturals.reduce((sum, w) => sum + w, 0)
    // The names at their least padding: what a row too narrow for their room shows.
    readonly property var leasts: tabs.wordWidths.map(w => w + 2 * tabs.leastPadding)
    readonly property real leastTotal: tabs.leasts.reduce((sum, w) => sum + w, 0)
    readonly property real share: tabs.model.length > 0 ? tabs.availableWidth / tabs.model.length : 0
    // "equal": every name fits in an equal share; "spread": each its name's width and an equal
    // part of what is left; "tight": the same at the least padding; "scroll": the names are
    // wider than the row even so, which scrolls, at the least padding.
    readonly property string fit: tabs.naturals.every(w => w <= tabs.share) ? "equal" : tabs.naturalTotal <= tabs.availableWidth ? "spread" : tabs.leastTotal <= tabs.availableWidth ? "tight" : "scroll"
    readonly property var widths: tabs.fit === "equal" ? tabs.naturals.map(() => tabs.share)
        : tabs.fit === "spread" ? tabs.naturals.map(w => w + (tabs.availableWidth - tabs.naturalTotal) / tabs.model.length)
        : tabs.fit === "tight" ? tabs.leasts.map(w => w + (tabs.availableWidth - tabs.leastTotal) / tabs.model.length)
        : tabs.cutWidths(tabs.leasts, tabs.availableWidth - (tabs.chevrons ? tabs.chevronWidth : 0))
    // Whether the row scrolls sideways: the names do not fit.
    readonly property bool scrolls: tabs.fit === "scroll"
    // More tabs out of sight before the first one shown whole, or after the last.
    readonly property bool moreBefore: tabs.scrolls && strip.contentX > 1
    readonly property bool moreAfter: tabs.scrolls && strip.contentX < strip.contentWidth - strip.width - 1
    // A chevron at each end that has more: on a computer (a finger swipes).
    readonly property bool chevrons: tabs.fit === "scroll" && !tabs.theme.touch
    readonly property int chevronWidth: 28
    // How much of a neighbour shows, cut, beside a tab brought into view: past a chevron, short of whole.
    function peek(w: real): real {
        return Math.max(0, Math.min(w - 12, Math.max(tabs.chevrons ? tabs.chevronWidth + 6 : 24, w * 0.45)))
    }
    // The row, for tests: how far it is scrolled, and its tabs.
    readonly property alias strip: strip
    readonly property alias tabRepeater: tabRepeater

    // The names' widths when they scroll, each widened alike so that the row's edge, at its
    // start, falls in the middle of a tab: a tab whole at the edge would say the row ends there.
    // `room`: the row's width up to its chevron.
    function cutWidths(naturals: var, room: real): var {
        let before = 0
        let k = 0
        while (k < naturals.length && before + naturals[k] <= room) {
            before += naturals[k]
            k += 1
        }
        if (k >= naturals.length)
            return naturals
        const seen = (room - before) / naturals[k]
        if (seen >= 0.3 && seen <= 0.7)
            return naturals
        // The cut moved to the middle of tab k, or of the one before it when k's start would show too little.
        const at = seen > 0.7 || k === 0 ? k : k - 1
        const start = naturals.slice(0, at).reduce((sum, w) => sum + w, 0)
        const extra = (room - start - naturals[at] / 2) / (at + 0.5)
        return extra > 0 ? naturals.map(w => w + extra) : naturals
    }

    // The tab before (-1) or after (1) the one shown, around the ends.
    function step(by: int) {
        if (tabs.model.length === 0)
            return
        const from = tabs.shownIndex < 0 ? 0 : tabs.shownIndex
        const to = ((from + by) % tabs.model.length + tabs.model.length) % tabs.model.length
        tabs.chosen(tabs.model[to].id)
    }

    // Where tab `index` starts in the row: from the widths, not from the row's items, which
    // are laid out only before the next frame (a width just changed has not moved them yet).
    function startOf(index: int): real {
        return tabs.widths.slice(0, index).reduce((sum, w) => sum + w, 0)
    }

    // Tab `index` brought whole into view, its neighbour on the side it came from shown cut.
    function bring(index: int) {
        if (index < 0 || index >= tabs.widths.length || !tabs.scrolls) {
            strip.contentX = 0
            return
        }
        const x = tabs.startOf(index)
        const w = tabs.widths[index]
        const most = Math.max(0, tabs.startOf(tabs.widths.length) - strip.width)
        const before = index > 0 ? tabs.peek(tabs.widths[index - 1]) : 0
        const after = index < tabs.widths.length - 1 ? tabs.peek(tabs.widths[index + 1]) : 0
        if (x - before < strip.contentX)
            strip.contentX = Math.max(0, x - before)
        else if (x + w + after > strip.contentX + strip.width)
            strip.contentX = Math.min(most, x + w + after - strip.width)
    }

    // A chevron: the first tab out of view on that side (-1 before, 1 after) brought into it.
    function scrollOn(side: int) {
        const left = strip.contentX + (tabs.chevrons ? tabs.chevronWidth : 0)
        const right = strip.contentX + strip.width - (tabs.chevrons ? tabs.chevronWidth : 0)
        if (side > 0) {
            for (let i = 0; i < tabs.widths.length; ++i) {
                if (tabs.startOf(i) + tabs.widths[i] > right + 0.5) {
                    tabs.bring(i)
                    return
                }
            }
        } else {
            for (let i = tabs.widths.length - 1; i >= 0; --i) {
                if (tabs.startOf(i) < left - 0.5) {
                    tabs.bring(i)
                    return
                }
            }
        }
    }

    // Whether the keyboard's focus is on one of the tabs (it follows the tab shown).
    function focusInside(): bool {
        for (let item = tabs.Window.activeFocusItem; item; item = item.parent)
            if (item === tabs)
                return true
        return false
    }

    // The tab shown brought into view.
    function reveal() {
        tabs.bring(tabs.shownIndex)
    }

    onShownIndexChanged: Qt.callLater(tabs.reveal)
    onWidthsChanged: Qt.callLater(tabs.reveal)

    implicitWidth: tabs.naturalTotal
    implicitHeight: 40
    padding: 0
    // The tab shown takes the focus, not the row (a focus scope): Tab reaches it, then leaves.
    focusPolicy: Qt.NoFocus
    Accessible.role: Accessible.PageTabList
    Accessible.name: tabs.name

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
                    // The tab shown alone takes the focus (Tab reaches it, then leaves the row); the
                    // arrows choose another, and the focus follows it (the tab left keeps its policy
                    // until the focus has left it: Qt refuses it otherwise).
                    focusPolicy: tab.shown || tab.activeFocus ? Qt.StrongFocus : Qt.NoFocus
                    hoverEnabled: true
                    text: String(tab.modelData.text)
                    // Checkable, checked while shown: what screen readers are told, which Qt takes from the
                    // button itself once a tool listens. A click on the tab shown keeps it so.
                    checkable: true
                    checked: tab.shown
                    Accessible.role: Accessible.PageTab
                    Accessible.name: tab.text
                    Accessible.selectable: true
                    Accessible.selected: tab.shown
                    onShownChanged: {
                        if (tab.shown && tabs.focusInside())
                            tab.forceActiveFocus(Qt.TabFocusReason)
                    }
                    onClicked: {
                        tabs.chosen(tab.modelData.id)
                        tab.checked = Qt.binding(() => tab.shown)
                        // A click gives the tab shown the focus, for the arrows, as a tab bar does; a finger, not.
                        if (!tabs.theme.touch && tab.shown)
                            tab.forceActiveFocus(Qt.MouseFocusReason)
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
                        // The focus, from the keyboard: a ring around the tab.
                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: 2
                            visible: tab.visualFocus
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

    // The cut edges: one ground over the row, clear in its middle, fading in at each side that has
    // more (solid under its chevron), the rule under the row kept. One item, as a phone counts them.
    Rectangle {
        id: fade

        readonly property real reach: (tabs.chevrons ? tabs.chevronWidth + 20 : 40) / Math.max(1, fade.width)
        readonly property real solid: (tabs.chevrons ? tabs.chevronWidth : 0) / Math.max(1, fade.width)
        readonly property color clear: Qt.rgba(tabs.ground.r, tabs.ground.g, tabs.ground.b, 0)

        objectName: "fade"
        z: 1
        visible: tabs.moreBefore || tabs.moreAfter
        x: tabs.leftPadding
        width: tabs.availableWidth
        height: tabs.height - 1
        gradient: Gradient {
            orientation: Gradient.Horizontal

            GradientStop {
                position: 0
                color: tabs.moreBefore ? tabs.ground : fade.clear
            }
            GradientStop {
                position: fade.solid
                color: tabs.moreBefore ? tabs.ground : fade.clear
            }
            GradientStop {
                position: fade.reach
                color: fade.clear
            }
            GradientStop {
                position: 1 - fade.reach
                color: fade.clear
            }
            GradientStop {
                position: 1 - fade.solid
                color: tabs.moreAfter ? tabs.ground : fade.clear
            }
            GradientStop {
                position: 1
                color: tabs.moreAfter ? tabs.ground : fade.clear
            }
        }
    }

    // On a computer, a chevron at each end that has more: the next tab out of view brought into
    // it. Made only where they may show (a row that scrolls, not under a finger).
    component Chevron: AbstractButton {
        id: chevron

        required property int side

        width: tabs.chevronWidth
        height: tabs.height - 1
        focusPolicy: Qt.NoFocus
        hoverEnabled: true
        // The arrows and the tab list say it to screen readers already.
        Accessible.ignored: true
        onClicked: tabs.scrollOn(chevron.side)

        contentItem: Icon {
            iconName: chevron.side < 0 ? "go-previous" : "go-next"
            size: 16
        }
        background: Rectangle {
            radius: tabs.theme.radius
            color: chevron.hovered || chevron.down ? tabs.theme.hover : "transparent"
        }
    }
    Loader {
        z: 2
        x: tabs.leftPadding
        width: tabs.availableWidth
        height: tabs.height
        active: tabs.chevrons

        sourceComponent: Component {
            Item {
                Chevron {
                    objectName: "chevronBefore"
                    visible: tabs.moreBefore
                    x: 0
                    side: -1
                }
                Chevron {
                    objectName: "chevronAfter"
                    visible: tabs.moreAfter
                    x: tabs.availableWidth - width
                    side: 1
                }
            }
        }
    }
}
