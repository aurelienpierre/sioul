// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Contacts page's filters, combined (wave D): the address books shown
// (one, several or all), a group or no group, whether a card has or lacks a
// phone number, an e-mail address, a postal address or a title, and how many
// show ("132 of 840"). The search field is the name's filter, beside it. What
// the core decides (`contactfilter`) is drawn here; each change goes back
// whole (`asked`), kept on this device. On a phone (`compact`) the filters fold
// behind one Filters button that says how many are set, as on the Tasks page:
// the button, "Show all" when one is set, and the count make one row; unfolded,
// the row wraps.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

Flow {
    id: row

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The page's view (`view::contacts_filtered`): {filter, books, categories, counted, shown, total}.
    property var view: ({ filter: {}, books: [], categories: [], counted: "", shown: 0, total: 0 })

    readonly property var filter: row.view && row.view.filter ? row.view.filter : ({})
    readonly property var books: row.view && row.view.books ? row.view.books : []
    readonly property var groups: row.view && row.view.categories ? row.view.categories : []
    // How many filters are set: the books, the group, then each of has or lacks.
    readonly property int setCount: ((row.filter.books || []).length > 0 ? 1 : 0) + ((row.filter.group || "") !== "" || row.filter.ungrouped === true ? 1 : 0)
        + ["phone", "email", "address", "title"].filter(k => (row.filter[k] || "any") !== "any").length
    readonly property bool narrowed: row.setCount > 0
    // A phone's width (the window's `compact`): the filters folded behind their button until it unfolds them.
    property bool compact: false
    property bool unfolded: false
    readonly property bool folded: row.compact && !row.unfolded

    // The filters as they should be now (JSON for `setContactsFilter`), the search kept.
    signal asked(var filter)

    // The filter with one change.
    function edit(change) {
        const next = Object.assign({ books: [], group: "", ungrouped: false, phone: "any", email: "any", address: "any", title: "any", name: "" }, row.filter)
        change(next)
        row.asked(next)
    }

    // A book shown or not; none ticked shows them all.
    function turnBook(place, on) {
        row.edit(f => {
            const kept = (f.books || []).filter(b => b !== place)
            f.books = on ? kept.concat([place]) : kept
        })
    }

    readonly property var wants: ["any", "has", "lacks"]

    objectName: "contactFilters"
    spacing: 6

    // On a phone: the filters below, folded; how many are set, in the Tasks page's words.
    Button {
        id: foldButton

        objectName: "contactFiltersFold"
        visible: row.compact
        checkable: true
        checked: row.unfolded
        flat: true
        text: row.sioul.textCounted("task-filters", row.setCount)
        icon.name: "view-filter"
        onClicked: {
            row.unfolded = !row.unfolded
            // A click on it flips its own tick: it stays the fold's.
            foldButton.checked = Qt.binding(() => row.unfolded)
        }
    }

    // The address books: all, or those ticked.
    Button {
        id: booksButton

        objectName: "contactFiltersBooks"
        visible: row.books.length > 1 && !row.folded
        text: (row.filter.books || []).length === 0 ? row.sioul.text("contacts-filter-books-all") : row.sioul.textArgs("contacts-filter-books-some", JSON.stringify({ n: row.filter.books.length }))
        icon.name: "address-book-new"
        icon.color: row.theme.text
        font.weight: (row.filter.books || []).length > 0 ? Font.DemiBold : Font.Normal
        Accessible.name: row.sioul.text("contacts-filter-books")
        onClicked: booksMenu.now().popup(booksButton, 0, booksButton.height)
    }
    Later {
        id: booksMenu

        sourceComponent: Component {
            SioulMenu {
                id: menu

                MenuItem {
                    text: row.sioul.text("contacts-filter-books-all")
                    checkable: true
                    checked: (row.filter.books || []).length === 0
                    onTriggered: row.edit(f => f.books = [])
                }
                MenuSeparator {}
                Instantiator {
                    model: row.books

                    delegate: MenuItem {
                        id: bookLine

                        required property var modelData

                        objectName: "contactFiltersBook"
                        // "&" marks a shortcut in a menu: "&&" is one.
                        text: row.theme.plain(bookLine.modelData.name.replace(/&/g, "&&"))
                        checkable: true
                        checked: (row.filter.books || []).indexOf(bookLine.modelData.place) >= 0
                        onTriggered: row.turnBook(bookLine.modelData.place, bookLine.checked)
                    }
                    onObjectAdded: (index, object) => menu.insertItem(index + 2, object)
                    onObjectRemoved: (index, object) => menu.removeItem(object)
                }
            }
        }
    }

    // A group, or in none: what decides who reaches you, never the book.
    PlainComboBox {
        id: groupBox

        objectName: "contactFiltersGroup"
        visible: !row.folded
        width: Math.min(220, row.width)
        model: [row.sioul.text("contacts-filter-group-any"), row.sioul.text("contacts-filter-group-none")].concat(row.groups.map(g => row.theme.plain(g)))
        currentIndex: row.filter.ungrouped === true ? 1 : (row.filter.group || "") === "" ? 0 : Math.max(0, row.groups.indexOf(row.filter.group) + 2)
        Accessible.name: row.sioul.text("contacts-filter-group")
        onActivated: index => row.edit(f => {
            f.ungrouped = index === 1
            f.group = index >= 2 ? row.groups[index - 2] : ""
        })
    }

    // Has, or lacks: a phone number, an e-mail address, a postal address, a title.
    Repeater {
        model: ["phone", "email", "address", "title"]

        delegate: PlainComboBox {
            id: want

            required property string modelData

            objectName: "contactFiltersWant"
            visible: !row.folded
            width: Math.min(200, row.width)
            model: row.wants.map(w => row.sioul.text("contacts-filter-" + want.modelData + "-" + w))
            currentIndex: Math.max(0, row.wants.indexOf(row.filter[want.modelData] || "any"))
            font.weight: currentIndex > 0 ? Font.DemiBold : Font.Normal
            Accessible.name: row.sioul.text("contacts-filter-" + want.modelData)
            onActivated: index => row.edit(f => f[want.modelData] = row.wants[index])
        }
    }

    Button {
        objectName: "contactFiltersClear"
        visible: row.narrowed
        flat: true
        text: row.sioul.text("contacts-filter-clear")
        onClicked: row.edit(f => {
            f.books = []
            f.group = ""
            f.ungrouped = false
            f.phone = "any"
            f.email = "any"
            f.address = "any"
            f.title = "any"
        })
    }

    // How many show, of how many: beside the Filters button when they are folded. As high
    // as the controls of the row, centred on them (its own height read only when none shows:
    // read always, it made a binding loop).
    Label {
        objectName: "contactFiltersCount"
        height: Math.max(foldButton.visible ? foldButton.height : 0, booksButton.visible ? booksButton.height : 0, groupBox.visible ? groupBox.height : 0) || implicitHeight
        verticalAlignment: Text.AlignVCenter
        text: row.view ? row.view.counted || "" : ""
        textFormat: Text.PlainText
        color: row.theme.muted
    }
}
