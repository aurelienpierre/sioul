// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Sites page's list, on a computer and on Android alike (SitesPage.qml,
// android/SitesPage.qml): the sites for these hours first, the others
// folded under one line (docs/areas.md); filtered by what a site is for, its
// type, one of your categories; in your own order or grouped by type, as
// you left it; each with its icon and its news (SiteRow.qml). Under the
// list, Pin a site and the usual sites, as a menu (a whole group pinned at
// once). On a computer the column can fold to the sites' icons, and the
// page's ⚙ sits there too.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: list

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The sites, as `sioul.siteList()` gives them.
    property var sites: []
    // The site open, highlighted.
    property string openId: ""
    // The column may fold to the sites' icons (a computer's).
    property bool canNarrow: false
    // Folded to their icons, as you left it.
    property bool narrow: list.canNarrow && list.sioul.viewFlag("sites-narrow")
    // The page's ⚙ under the list (a computer's: its colours, Bitwarden).
    property bool settings: false
    // Said under the title (Android: where sites open, and what they cannot do there).
    property string note: ""
    // The ⚙, made on a computer alone (Android's page has none: review C2).
    readonly property var settingsButton: sitesSettings.item
    property alias presetsMenu: presetsMenu
    property alias filterMenu: filterMenu

    // What a site is, as it behaves (sioul_core::sites::TYPES); your categories come on top.
    readonly property var kinds: ["mailbox", "chat", "video", "social", "dating", "other"]
    // The list filtered, each way at once: what a site is for, its type, one of your categories; "" for any.
    property string areaFilter: ""
    property string kindFilter: ""
    property string categoryFilter: ""
    // The list by type, or in your own order, as you left it.
    property bool byCategory: list.sioul.viewFlag("sites-by-category")
    // The sites for other hours (docs/areas.md), shown when asked.
    property bool laterShown: false
    // The usual sites, as a menu: countries, their groups, the sites.
    property var presetTree: []

    // A site chosen in the list.
    signal chosen(string id)
    // A site's menu asked for, where it is.
    signal menuAsked(string id)
    // "Pin a site".
    signal addAsked()
    // Usual sites pinned: what went wrong, else "".
    signal pinned(string problem)

    // The categories your sites have, by name.
    readonly property var categories: {
        const words = []
        list.sites.forEach(s => (s.categories || []).forEach(c => {
            if (words.indexOf(c) < 0)
                words.push(c)
        }))
        return words.sort((a, b) => a.localeCompare(b))
    }
    readonly property string filterWords: [list.areaFilter !== "" ? list.sioul.text("area-" + list.areaFilter) : "", list.kindFilter !== "" ? list.sioul.text("site-kind-" + list.kindFilter) : "", list.categoryFilter].filter(t => t !== "").join(" · ")
    // The rows: the sites for these hours, then one line folding the others;
    // each type named once when sorted by it.
    readonly property var rows: {
        const wanted = s => (list.areaFilter === "" || String(s.area).split("+").indexOf(list.areaFilter) >= 0) && (list.kindFilter === "" || s.kind === list.kindFilter) && (list.categoryFilter === "" || (s.categories || []).indexOf(list.categoryFilter) >= 0)
        const shown = list.sites.filter(wanted)
        const sorted = list.byCategory ? shown.slice().sort((a, b) => list.kinds.indexOf(a.kind) - list.kinds.indexOf(b.kind)) : shown
        const out = []
        const add = (some, later) => some.forEach((s, i) => out.push({ site: s, later: later, header: list.byCategory && (i === 0 || some[i - 1].kind !== s.kind) ? s.kind : "" }))
        add(sorted.filter(s => s.in_view !== false), false)
        const later = sorted.filter(s => s.in_view === false)
        if (later.length > 0) {
            out.push({ fold: true, count: later.length, header: "" })
            if (list.laterShown)
                add(later, true)
        }
        return out
    }

    function showPresets() {
        list.presetTree = JSON.parse(list.sioul.sitePresetsTree()).countries
        presetsMenu.popup(presetsButton, 0, -presetsMenu.implicitHeight)
    }
    function showFilters() {
        filterMenu.popup(filterButton, 0, filterButton.height)
    }

    // Usual sites pinned: one, or a whole group; those already here stay as they are.
    function pinAll(rows) {
        const answer = JSON.parse(list.sioul.addSites(JSON.stringify(rows)))
        if (answer.added > 0)
            list.sioul.status = list.sioul.textWith("site-pinned", "count", String(answer.added))
        list.pinned(answer.error || "")
    }

    spacing: 6

    RowLayout {
        Layout.fillWidth: true
        spacing: 0

        Label {
            visible: !list.narrow
            Layout.fillWidth: true
            text: list.sioul.text("ui-sites")
            font.pixelSize: 20
            elide: Text.ElideRight
            color: list.theme.text
        }
        // Filtered by what a site is for, its type, one of your categories.
        ToolButton {
            id: filterButton

            visible: !list.narrow
            icon.name: "view-filter"
            icon.color: list.theme.text
            display: AbstractButton.IconOnly
            highlighted: list.filterWords !== ""
            Accessible.name: list.sioul.text("site-filter")
            ToolTip.visible: hovered
            ToolTip.text: list.sioul.text("site-filter")
            ToolTip.delay: 600
            onClicked: list.showFilters()
        }
        ToolButton {
            visible: !list.narrow
            checkable: true
            checked: list.byCategory
            icon.name: "view-sort"
            icon.color: list.theme.text
            display: AbstractButton.IconOnly
            Accessible.name: list.sioul.text(list.byCategory ? "site-sort-category" : "site-sort-own")
            ToolTip.visible: hovered
            ToolTip.text: list.theme.plain(Accessible.name)
            ToolTip.delay: 600
            onToggled: {
                list.byCategory = checked
                list.sioul.setViewFlag("sites-by-category", checked)
            }
        }
        ToolButton {
            visible: list.canNarrow
            text: list.narrow ? "»" : "«"
            Accessible.name: list.narrow ? list.sioul.text("site-column-widen") : list.sioul.text("site-column-narrow")
            ToolTip.visible: hovered
            ToolTip.text: list.theme.plain(Accessible.name)
            ToolTip.delay: 600
            onClicked: {
                list.narrow = !list.narrow
                list.sioul.setViewFlag("sites-narrow", list.narrow)
            }
        }
    }
    Label {
        visible: list.note !== "" && !list.narrow
        Layout.fillWidth: true
        text: list.note
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: list.theme.muted
    }
    // The filters at work, in words, and a way out of them.
    RowLayout {
        visible: !list.narrow && list.filterWords !== ""
        Layout.fillWidth: true
        spacing: 2

        Label {
            Layout.fillWidth: true
            text: list.filterWords
            textFormat: Text.PlainText
            font.pixelSize: 12
            wrapMode: Text.Wrap
            color: list.theme.muted
        }
        ToolButton {
            icon.name: "edit-clear"
            icon.color: list.theme.text
            display: AbstractButton.IconOnly
            implicitWidth: 26
            implicitHeight: 26
            Accessible.name: list.sioul.text("site-filter-clear")
            ToolTip.visible: hovered
            ToolTip.text: list.sioul.text("site-filter-clear")
            onClicked: {
                list.areaFilter = ""
                list.kindFilter = ""
                list.categoryFilter = ""
            }
        }
    }
    Label {
        visible: list.sites.length === 0 && !list.narrow
        Layout.fillWidth: true
        text: list.sioul.text("site-none")
        wrapMode: Text.Wrap
        color: list.theme.muted
    }
    // Many sites scroll; sorted by type, each type says its name once.
    ScrollView {
        id: sitesScroll

        Layout.fillWidth: true
        Layout.fillHeight: true
        contentWidth: availableWidth
        clip: true

        // A site added, changed or taken out: the list keeps its place (ListPlace.qml); other
        // filters, from its top.
        ListPlace {
            id: sitesPlace

            view: sitesScroll
            source: list.rows
            scope: [list.areaFilter, list.kindFilter, list.categoryFilter, list.byCategory]
        }

        ColumnLayout {
            width: parent.width
            spacing: 6

            Repeater {
                model: sitesPlace.rows

                delegate: ColumnLayout {
                    id: entry

                    required property var modelData
                    required property int index
                    readonly property string placeKey: entry.modelData.fold ? "later" : (entry.modelData.site || {}).id || ""

                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        visible: entry.modelData.header !== "" && !list.narrow
                        Layout.topMargin: entry.index === 0 ? 0 : 6
                        text: entry.modelData.header !== "" ? list.sioul.text("site-kind-" + entry.modelData.header) : ""
                        font.pixelSize: 12
                        color: list.theme.muted
                    }
                    // The sites for other hours: one line, opened on demand.
                    Button {
                        visible: entry.modelData.fold === true
                        Layout.fillWidth: true
                        Layout.topMargin: 6
                        flat: true
                        text: list.theme.plain(list.narrow ? "⋯" : list.sioul.textWith("site-later", "count", String(entry.modelData.count || 0)) + (list.laterShown ? "  ▾" : "  ▸"))
                        font.pixelSize: 13
                        onClicked: list.laterShown = !list.laterShown
                    }
                    SiteRow {
                        visible: entry.modelData.fold !== true
                        Layout.fillWidth: true
                        sioul: list.sioul
                        theme: list.theme
                        site: entry.modelData.site || ({})
                        narrow: list.narrow
                        later: entry.modelData.later === true
                        highlighted: list.openId !== "" && list.openId === (entry.modelData.site || {}).id
                        onChosen: list.chosen((entry.modelData.site || {}).id || "")
                        onMenuAsked: list.menuAsked((entry.modelData.site || {}).id || "")
                    }
                }
            }
        }
    }
    // Adding a site, and the page's settings (its colours, Bitwarden): under the list, small.
    Flow {
        Layout.fillWidth: true
        spacing: 2

        ToolButton {
            icon.name: "list-add"
            icon.color: list.theme.text
            text: list.narrow ? "" : list.sioul.text("site-add")
            display: list.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
            Accessible.name: list.sioul.text("site-add")
            ToolTip.visible: list.narrow && hovered
            ToolTip.text: list.sioul.text("site-add")
            onClicked: list.addAsked()
        }
        // The usual sites, by country, group and site: a whole group pinned at once, or one.
        ToolButton {
            id: presetsButton

            icon.name: "bookmarks-organize"
            icon.color: list.theme.text
            text: list.narrow ? "" : list.sioul.text("site-presets") + "  ▾"
            display: list.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
            Accessible.name: list.sioul.text("site-presets")
            ToolTip.visible: list.narrow && hovered
            ToolTip.text: list.sioul.text("site-presets")
            onClicked: list.showPresets()
        }
        Loader {
            id: sitesSettings

            active: list.settings
            visible: active

            sourceComponent: SettingsButton {
                sioul: list.sioul
                theme: list.theme
                view: "sites"
            }
        }
    }

    // What the list shows: what a site is for, its type, one of your categories.
    // One list of lines: a Repeater's lines land out of order in a menu once
    // their model changes.
    readonly property var filterLines: {
        const out = [{ head: list.sioul.text("site-filter-for") }, { what: "area", value: "", text: list.sioul.text("site-filter-any-area") }]
        for (const area of ["work", "admin", "leisure"])
            out.push({ what: "area", value: area, text: list.sioul.text("area-" + area) })
        out.push({ head: list.sioul.text("site-filter-type") }, { what: "kind", value: "", text: list.sioul.text("site-filter-any-kind") })
        for (const kind of list.kinds.filter(k => list.sites.some(s => s.kind === k)))
            out.push({ what: "kind", value: kind, text: list.sioul.text("site-kind-" + kind) })
        if (list.categories.length > 0) {
            out.push({ head: list.sioul.text("site-filter-category") }, { what: "category", value: "", text: list.sioul.text("site-filter-any-category") })
            for (const category of list.categories)
                out.push({ what: "category", value: category, text: category })
        }
        return out
    }
    function filterOf(what) {
        return what === "area" ? list.areaFilter : what === "kind" ? list.kindFilter : list.categoryFilter
    }
    function setFilter(what, value) {
        if (what === "area")
            list.areaFilter = value
        else if (what === "kind")
            list.kindFilter = value
        else
            list.categoryFilter = value
    }

    SioulMenu {
        id: filterMenu

        Instantiator {
            model: list.filterLines

            delegate: MenuItem {
                id: filterLine

                required property var modelData
                readonly property bool head: filterLine.modelData.head !== undefined

                // Your categories may hold "&", a shortcut's mark in a menu: "&&" is one.
                text: list.theme.plain((filterLine.head ? filterLine.modelData.head : filterLine.modelData.text).replace(/&/g, "&&"))
                font.pixelSize: filterLine.head ? 12 : 15
                enabled: !filterLine.head
                checkable: !filterLine.head
                checked: !filterLine.head && list.filterOf(filterLine.modelData.what) === filterLine.modelData.value
                onTriggered: {
                    // Chosen again: no longer filtered by it.
                    const was = list.filterOf(filterLine.modelData.what)
                    list.setFilter(filterLine.modelData.what, was === filterLine.modelData.value ? "" : filterLine.modelData.value)
                    filterLine.checked = Qt.binding(() => !filterLine.head && list.filterOf(filterLine.modelData.what) === filterLine.modelData.value)
                }
            }
            onObjectAdded: (index, object) => filterMenu.insertItem(index, object)
            onObjectRemoved: (index, object) => filterMenu.removeItem(object)
        }
    }

    // The usual sites: country, group, site; a country's states or provinces
    // in a menu of their own, each with its groups. Each place and group
    // opens with "All of them": a whole group pinned at once.
    SioulMenu {
        id: presetsMenu

        Instantiator {
            model: list.presetTree

            delegate: PresetPlaceMenu {
                id: countryMenu

                required property var modelData

                sioul: list.sioul
                place: countryMenu.modelData
                onPin: rows => list.pinAll(rows)

                // Its states or provinces, last.
                property Instantiator regions: Instantiator {
                    model: countryMenu.modelData.regions.length > 0 ? [countryMenu.modelData] : []

                    delegate: SioulMenu {
                        id: regionsMenu

                        required property var modelData

                        title: list.theme.plain(regionsMenu.modelData.regions_name)

                        Instantiator {
                            model: regionsMenu.modelData.regions

                            delegate: PresetPlaceMenu {
                                id: regionMenu

                                required property var modelData

                                sioul: list.sioul
                                place: regionMenu.modelData
                                onPin: rows => list.pinAll(rows)
                            }
                            onObjectAdded: (index, object) => regionsMenu.insertMenu(index, object)
                            onObjectRemoved: (index, object) => regionsMenu.removeMenu(object)
                        }
                    }
                    onObjectAdded: (index, object) => countryMenu.addMenu(object)
                    onObjectRemoved: (index, object) => countryMenu.removeMenu(object)
                }
            }
            onObjectAdded: (index, object) => presetsMenu.insertMenu(index, object)
            onObjectRemoved: (index, object) => presetsMenu.removeMenu(object)
        }
    }
}
