// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The one head of a page (docs/qt-quick.md, "The look of each kind of
// thing", rule 14): its tabs (`tabs`, Tabs.qml), then one row with the
// navigation at the left (`navigation`: Today ◂ ▸ and the title, a page's
// own few actions), the filters after it (written inside it), and the page's
// ⚙ last at the right (`settings`, SettingsButton.qml), a page having one.
// The row wraps when the window is too narrow: each item that does not fit
// goes on the line under it (the navigation's buttons one by one too), the
// lines under the first using the head's whole width, each item at its own
// width (one wider than its line narrowed to it); the ⚙ stays at the right
// of the first line, never alone on a line of its own, never over an item (a
// Flow cannot keep its room on one line only: the row lays its items out
// itself, `lay()`). On a
// phone (`foldsFilters`), the filters fold behind one button, "Filters",
// with how many are set; the page hides its filters while `filtersFolded`.
//
//     PageHead {
//         Layout.fillWidth: true
//         sioul: page.sioul
//         theme: page.theme
//         filtersSet: page.filtersSet
//         tabs: Tabs { … }
//         navigation: Button { … }
//         settings: SettingsButton { … }
//
//         PlainComboBox { visible: !head.filtersFolded; … }
//     }

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: head

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // Its tabs, across the page, above the row.
    property alias tabs: tabsSlot.data
    // At the left of the row: the navigation, a page's own few actions.
    property alias navigation: navigationSlot.data
    // After them, on the same row while it has room: the filters.
    default property alias filters: row.data
    // At the right end of the first line: the page's ⚙.
    property alias settings: settingsSlot.data
    // The filters fold behind one button (a phone).
    property bool foldsFilters: false
    // That button shown at all (a page's view with no filter: none).
    property bool offersFilters: true
    // How many filters are set, said on that button.
    property int filtersSet: 0
    // Unfolded by that button.
    property bool filtersShown: false
    // The filters hidden behind it: the page's filters are not shown.
    readonly property bool filtersFolded: head.foldsFilters && !head.filtersShown
    // The button's name, for a page's tests and pictures.
    property string filtersName: "pageFilters"
    // The row the navigation and filters are laid in, for a page's widths (`row.width`, `row.spacing`).
    readonly property alias row: row
    // The row's items followed, whether a laying out is asked already, and the items
    // narrowed to their line, with their own width (`lay()`).
    property var watched: []
    property bool laying: false
    property var narrowed: []

    spacing: head.theme.labelGap + 4
    Component.onCompleted: head.watch()

    ColumnLayout {
        id: tabsSlot

        visible: tabsSlot.children.length > 0
        Layout.fillWidth: true
        spacing: 0
    }
    // The row: its items laid out by `lay()`, left to right, on as many lines as they need;
    // the ⚙ at the right end of the first, whose room it keeps.
    Item {
        id: line

        Layout.fillWidth: true
        implicitHeight: Math.max(row.laidHeight, settingsSlot.visible ? settingsSlot.height : 0)

        Item {
            id: row

            // Between two items, and between two lines.
            property int spacing: head.theme.labelGap
            // The height its lines take.
            property real laidHeight: 0

            objectName: "pageHeadRow"
            width: line.width
            height: row.laidHeight
            onWidthChanged: head.layLater()
            onChildrenChanged: head.watch()

            // The navigation's items, laid one by one in the row's own lines (`lay()`), so that
            // they wrap one at a time, as the filters do.
            Item {
                id: navigationSlot

                objectName: "pageHeadNavigation"
                width: row.width
                height: row.laidHeight
                onChildrenChanged: head.watch()
            }
            // On a phone: the filters, folded, and how many are set.
            Button {
                id: filtersButton

                objectName: head.filtersName
                visible: head.foldsFilters && head.offersFilters
                checkable: true
                checked: head.filtersShown
                text: head.foldsFilters && head.offersFilters ? head.sioul.textCounted("ui-filters", head.filtersSet) : ""
                icon.name: "view-filter"
                onClicked: {
                    head.filtersShown = !head.filtersShown
                    // A click on it flips its own tick: it stays the fold's.
                    filtersButton.checked = Qt.binding(() => head.filtersShown)
                }
            }
        }
        Row {
            id: settingsSlot

            objectName: "pageHeadSettings"
            visible: settingsSlot.children.length > 0
            anchors.right: parent.right
            anchors.top: parent.top
            spacing: head.theme.labelGap
            onWidthChanged: head.layLater()
            onVisibleChanged: head.layLater()
        }
    }

    // The row's items in their order: the navigation's, then the Filters button and the filters.
    function items() {
        const all = []
        for (let i = 0; i < navigationSlot.children.length; i++)
            all.push(navigationSlot.children[i])
        for (let i = 0; i < row.children.length; i++)
            if (row.children[i] !== navigationSlot)
                all.push(row.children[i])
        return all
    }

    // Each item of the row followed: its size or its showing changed, the row is laid out again.
    function watch() {
        for (const item of head.items()) {
            if (head.watched.indexOf(item) >= 0)
                continue
            head.watched.push(item)
            item.widthChanged.connect(head.layLater)
            item.heightChanged.connect(head.layLater)
            item.visibleChanged.connect(head.layLater)
            item.implicitWidthChanged.connect(head.layLater)
        }
        head.layLater()
    }

    function layLater() {
        if (!head.laying) {
            head.laying = true
            Qt.callLater(head.lay)
        }
    }

    // Left to right, a line after another; the first line keeps the ⚙'s room at its
    // right and is as tall as the ⚙ at least. An item wider than the line it starts is
    // narrowed to it, and given its own width back once it fits (`narrowed`).
    function lay() {
        head.laying = false
        const width = row.width
        const keep = settingsSlot.visible ? settingsSlot.width + row.spacing : 0
        let x = 0
        let y = 0
        let tallest = keep > 0 ? settingsSlot.height : 0
        let first = true
        for (const item of head.items()) {
            if (!item.visible)
                continue
            const narrowed = head.narrowed.find(n => n.item === item)
            // Its own width: as the page gave it, or as it was before it was narrowed.
            const own = narrowed ? narrowed.width : item.width
            if (x > 0 && x + own > (first ? width - keep : width)) {
                y += tallest + row.spacing
                x = 0
                tallest = 0
                first = false
            }
            const room = Math.max(0, (first ? width - keep : width) - x)
            if (own > room) {
                if (!narrowed)
                    head.narrowed.push({ item: item, width: item.width, implicit: item.width === item.implicitWidth })
                item.width = room
            } else if (narrowed) {
                head.narrowed = head.narrowed.filter(n => n.item !== item)
                if (narrowed.implicit)
                    item.width = Qt.binding(() => item.implicitWidth)
                else
                    item.width = narrowed.width
            }
            item.x = x
            item.y = y
            x += item.width + row.spacing
            tallest = Math.max(tallest, item.height)
        }
        row.laidHeight = y + tallest
    }
}
