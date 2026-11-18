// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts ⚙'s block of the address books (wave D): one line per book, its
// name, where it is kept, how many contacts, whose it is, whether it can be
// written; opened, with whom it is shared (asked of its server then) and
// what may be done with it: renamed, merged into another, removed (asked
// what becomes of its contacts: moved to another book, or deleted with it,
// the count said), shared with people or groups of its Nextcloud, or, when
// someone else's, no longer shown. "New address book…" makes one on an
// account whose server makes them, or on this device alone. A server with
// one fixed book (Google) offers only what it allows, and says so. Books
// say where contacts are kept and with whom they are shared; who may reach
// you is decided by a contact's groups, never by its book.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme
    // The row's sentence, said once above the books.
    property string about: ""
    // `addressbooks::setup`: {books: [line], homes: [{id, name}]}.
    property var view: ({ books: [], homes: [] })
    property string problem: ""
    // The books opened, by place, while the panel lives.
    property var opened: ({})
    // Each book's sharing as its server last answered (`bookShared`), by place.
    property var sharing: ({})
    // The removal asked of a book: its place, and what it would do (`addressBookRemoval`).
    property string removing: ""
    property var removal: ({})
    // "Merge into…" open for a book, by place.
    property string merging: ""
    // "New address book…" open.
    property bool making: false

    function read() {
        try {
            setup.view = JSON.parse(setup.sioul.addressBooksSetup() || "{}")
        } catch (e) {
            setup.view = { books: [], homes: [] }
        }
    }

    function toggle(place) {
        const next = Object.assign({}, setup.opened)
        next[place] = !next[place]
        setup.opened = next
        const book = setup.view.books.find(b => b.place === place)
        if (next[place] && book && book.can_share)
            setup.sioul.readBookSharing(place)
    }

    function askRemoval(place) {
        setup.removal = JSON.parse(setup.sioul.addressBookRemoval(place) || "{}")
        setup.removing = place
    }

    function remove(place, how) {
        setup.problem = setup.sioul.removeAddressBook(place, how)
        setup.removing = ""
        setup.read()
    }

    // The other books a book's contacts can go to: [{place, name}].
    function others(place) {
        return (setup.view.books || []).filter(b => b.place !== place && b.writable)
    }

    Connections {
        target: setup.sioul

        function onContactsChanged() {
            setup.read()
        }

        function onBookShared(place, answer) {
            const next = Object.assign({}, setup.sharing)
            next[place] = JSON.parse(answer || "{}")
            setup.sharing = next
        }
    }

    objectName: "addressBooksSetup"
    spacing: 4
    Component.onCompleted: setup.read()

    Label {
        visible: setup.about !== ""
        Layout.fillWidth: true
        text: setup.about
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.problem !== ""
        Layout.fillWidth: true
        text: setup.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.warm
    }

    Repeater {
        model: setup.view.books || []

        delegate: ColumnLayout {
            id: line

            required property var modelData
            readonly property bool open: setup.opened[line.modelData.place] === true
            readonly property var shared: setup.sharing[line.modelData.place] || null

            objectName: "addressBook"
            Layout.fillWidth: true
            spacing: 4

            // Its name, where it is kept, how many: a click opens the rest.
            ItemDelegate {
                objectName: "addressBookLine"
                Layout.fillWidth: true
                padding: 6
                onClicked: setup.toggle(line.modelData.place)

                contentItem: ColumnLayout {
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        text: (line.open ? "▾  " : "▸  ") + line.modelData.name
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                        color: setup.theme.text
                    }
                    Label {
                        Layout.fillWidth: true
                        Layout.leftMargin: 18
                        text: [line.modelData.kept, line.modelData.counted, line.modelData.whose, line.modelData.access].join(" · ")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: setup.theme.muted
                    }
                }
            }

            ColumnLayout {
                visible: line.open
                Layout.fillWidth: true
                Layout.leftMargin: 18
                spacing: 6

                // A server with one fixed book, made here, waiting: said.
                Label {
                    visible: line.modelData.fixed !== "" || line.modelData.waiting !== ""
                    Layout.fillWidth: true
                    text: line.modelData.fixed !== "" ? line.modelData.fixed : line.modelData.waiting
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: setup.theme.muted
                }

                // Renamed in place.
                RowLayout {
                    visible: line.modelData.can_rename
                    Layout.fillWidth: true
                    spacing: 6

                    TextField {
                        id: newName

                        objectName: "addressBookName"
                        Layout.fillWidth: true
                        text: line.modelData.name
                        Accessible.name: setup.sioul.text("books-rename")
                        onAccepted: renameButton.clicked()
                    }
                    Button {
                        id: renameButton

                        objectName: "addressBookRename"
                        enabled: newName.text.trim() !== "" && newName.text.trim() !== line.modelData.name
                        text: setup.sioul.text("books-rename")
                        onClicked: {
                            setup.problem = setup.sioul.renameAddressBook(line.modelData.place, newName.text)
                            setup.read()
                        }
                    }
                }

                // With whom it is shared, asked of its server when the line opens.
                Label {
                    visible: line.modelData.can_share
                    Layout.fillWidth: true
                    text: line.shared === null ? setup.sioul.text("books-share-asking") : (line.shared.problem || line.shared.line || "")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: setup.theme.text
                }
                Repeater {
                    model: line.shared && line.shared.people ? line.shared.people : []

                    delegate: RowLayout {
                        id: sharee

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: sharee.modelData.name
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: setup.theme.text
                        }
                        Button {
                            objectName: "addressBookUnshare"
                            flat: true
                            text: setup.sioul.text("books-share-stop")
                            onClicked: setup.sioul.unshareBook(line.modelData.place, sharee.modelData.id)
                        }
                    }
                }
                Flow {
                    visible: line.modelData.can_share && line.shared !== null && line.shared.sharer === "nextcloud"
                    Layout.fillWidth: true
                    spacing: 6

                    TextField {
                        id: who

                        objectName: "addressBookShareWho"
                        width: Math.min(220, parent.width)
                        placeholderText: setup.sioul.text("books-share-who")
                    }
                    CheckBox {
                        id: asGroup

                        text: setup.sioul.text("books-share-group")
                    }
                    CheckBox {
                        id: mayWrite

                        text: setup.sioul.text("books-share-write")
                    }
                    Button {
                        objectName: "addressBookShare"
                        enabled: who.text.trim() !== ""
                        text: setup.sioul.text("books-share-go")
                        onClicked: {
                            setup.sioul.shareBook(line.modelData.place, (asGroup.checked ? "group:" : "") + who.text.trim(), mayWrite.checked)
                            who.text = ""
                        }
                    }
                }

                // Merged into another: its contacts moved there, then it removed.
                Flow {
                    visible: line.modelData.can_remove && setup.others(line.modelData.place).length > 0
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        objectName: "addressBookMerge"
                        visible: setup.merging !== line.modelData.place
                        flat: true
                        text: setup.sioul.text("books-merge")
                        onClicked: setup.merging = line.modelData.place
                    }
                    PlainComboBox {
                        id: mergeInto

                        objectName: "addressBookMergeInto"
                        visible: setup.merging === line.modelData.place
                        width: Math.min(260, parent.width)
                        model: setup.others(line.modelData.place).map(b => setup.theme.plain(b.name))
                        Accessible.name: setup.sioul.text("books-merge-into")
                    }
                    Button {
                        objectName: "addressBookMergeGo"
                        visible: setup.merging === line.modelData.place
                        text: setup.sioul.text("books-merge-go")
                        onClicked: {
                            const into = setup.others(line.modelData.place)[Math.max(0, mergeInto.currentIndex)]
                            setup.problem = setup.sioul.removeAddressBook(line.modelData.place, "move:" + into.place)
                            setup.merging = ""
                            setup.read()
                        }
                    }
                    Button {
                        visible: setup.merging === line.modelData.place
                        flat: true
                        text: setup.sioul.text("ui-cancel")
                        onClicked: setup.merging = ""
                    }
                }

                // Removed, asked first; someone else's, no longer shown.
                Button {
                    objectName: "addressBookRemove"
                    visible: (line.modelData.can_remove || line.modelData.can_leave) && setup.removing !== line.modelData.place
                    flat: true
                    text: setup.sioul.text(line.modelData.can_leave ? "books-leave" : "books-remove")
                    onClicked: setup.askRemoval(line.modelData.place)
                }
                ColumnLayout {
                    visible: setup.removing === line.modelData.place
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        objectName: "addressBookRemoval"
                        Layout.fillWidth: true
                        text: setup.removal.sentence || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: setup.theme.text
                    }
                    // Its contacts: moved to another book, or deleted with it.
                    Flow {
                        visible: setup.removal.act === "cards" && (setup.removal.into || []).length > 0
                        Layout.fillWidth: true
                        spacing: 6

                        PlainComboBox {
                            id: removeInto

                            objectName: "addressBookRemoveInto"
                            width: Math.min(260, parent.width)
                            model: (setup.removal.into || []).map(b => setup.theme.plain(b[1]))
                            Accessible.name: setup.sioul.text("books-remove-move")
                        }
                        Button {
                            objectName: "addressBookRemoveMove"
                            text: setup.sioul.text("books-remove-move-go")
                            onClicked: setup.remove(line.modelData.place, "move:" + setup.removal.into[Math.max(0, removeInto.currentIndex)][0])
                        }
                    }
                    Label {
                        visible: setup.removal.act === "cards"
                        Layout.fillWidth: true
                        text: setup.removal.note || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: setup.theme.muted
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            objectName: "addressBookRemoveGo"
                            visible: setup.removal.act !== "none"
                            text: setup.sioul.text(setup.removal.act === "cards" ? "books-remove-delete" : setup.removal.act === "leave" ? "books-leave" : "books-remove-go")
                            onClicked: setup.remove(line.modelData.place, setup.removal.act === "cards" ? "delete" : setup.removal.act)
                        }
                        Button {
                            flat: true
                            text: setup.sioul.text("ui-cancel")
                            onClicked: setup.removing = ""
                        }
                    }
                }
            }
        }
    }

    // A new address book: on an account whose server makes them, or on this device alone.
    Button {
        objectName: "addressBookNew"
        visible: !setup.making && (setup.view.homes || []).length > 0
        text: setup.sioul.text("books-new")
        onClicked: setup.making = true
    }
    ColumnLayout {
        visible: setup.making
        Layout.fillWidth: true
        spacing: 4

        Label {
            text: setup.sioul.text("books-new-where")
            color: setup.theme.text
        }
        PlainComboBox {
            id: homeChoice

            objectName: "addressBookNewHome"
            Layout.fillWidth: true
            model: (setup.view.homes || []).map(h => setup.theme.plain(h.name))
            Accessible.name: setup.sioul.text("books-new-where")
        }
        TextField {
            id: makeName

            objectName: "addressBookNewName"
            Layout.fillWidth: true
            placeholderText: setup.sioul.text("books-new-name")
        }
        Flow {
            Layout.fillWidth: true
            spacing: 6

            Button {
                objectName: "addressBookMake"
                enabled: makeName.text.trim() !== ""
                text: setup.sioul.text("books-make")
                onClicked: {
                    const home = setup.view.homes[Math.max(0, homeChoice.currentIndex)]
                    setup.problem = setup.sioul.makeAddressBook(JSON.stringify({ account: home.id, name: makeName.text }))
                    if (setup.problem === "") {
                        setup.making = false
                        makeName.text = ""
                    }
                    setup.read()
                }
            }
            Button {
                flat: true
                text: setup.sioul.text("ui-cancel")
                onClicked: setup.making = false
            }
        }
    }
}
