// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts ⚙'s block of the address books (wave D): one line per book, its
// name, where it is kept, how many contacts, whose it is, whether it can be
// written; opened, with whom it is shared (asked of its server then) and
// what may be done with it: renamed, merged into another, removed (asked
// what becomes of its contacts: moved to another book, or deleted with it,
// the count said), shared with people or groups of its Nextcloud, or, when
// someone else's, no longer shown; exported as one vCard file, the copy to
// keep before a large batch (on a computer: a phone's picker gives no file
// to write). "New address book…" makes one on an account whose server makes
// them, or on this device alone. A server with one fixed book (Google)
// offers only what it allows, and says so. A merge, or a removal that moves
// the contacts, asks first: how many go where, what the target would not
// keep, who sees it. What is chosen or typed in a line is kept by its book
// while the lines are drawn again at each sync (review F4). Below the
// books, the changes to many contacts of the last thirty days, each undone
// as a whole (review F2). Books say where contacts are kept and with whom
// they are shared; who may reach you is decided by a contact's groups,
// never by its book, with one switch about trust: "Its groups decide who
// may reach me", on for a book only you can write, off until you turn it on
// for one someone else can write (the line says so when its writers
// change), since they could otherwise add a card in a group a list lets
// through.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
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
    // The book chosen in each line's "Merge into" and "Move them to", by the line's place.
    property var chosen: ({})
    // The name typed in each line's rename field, by place, while it differs.
    property var typed: ({})
    // The question before a merge (`mergeQuestion`), with its two places; {} when none.
    property var question: ({})
    // The batches of the last thirty days (`contactsBatches`), and the one whose Undo is asked.
    readonly property var batches: setup.sioul.contactsBatches ? JSON.parse(setup.sioul.contactsBatches) : []
    property string undoing: ""
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
        return (setup.view.books || []).filter(b => b.place !== place && b.writable && b.waiting !== setup.sioul.text("book-refused"))
    }

    // The place chosen in a line's chooser, kept by the line's book across redraws.
    function choice(place, places) {
        const kept = setup.chosen[place]
        return places.indexOf(kept) >= 0 ? kept : (places.length > 0 ? places[0] : "")
    }

    function choose(place, into) {
        const next = Object.assign({}, setup.chosen)
        next[place] = into
        setup.chosen = next
    }

    // Asked before a merge: the count and the target, what it would not keep, who sees it.
    function ask(from, into) {
        if (into === "")
            return
        const asked = JSON.parse(setup.sioul.mergeQuestion(from, into) || "{}")
        asked.from = from
        asked.into = into
        setup.question = asked
        if (setup.sharing[into] === undefined)
            setup.sioul.readBookSharing(into)
    }

    function merge() {
        setup.problem = setup.sioul.removeAddressBook(setup.question.from, "move:" + setup.question.into)
        setup.question = {}
        setup.merging = ""
        setup.removing = ""
        setup.read()
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
            // Who sees the target, now known: the question asked again with it.
            if (setup.question.into === place)
                setup.ask(setup.question.from, setup.question.into)
        }
    }

    objectName: "addressBooksSetup"
    spacing: 4
    Component.onCompleted: {
        setup.read()
        setup.sioul.readContactsBatches()
    }

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
                    // Made here and waiting, refused by its server, or kept because its server
                    // holds more; its groups no longer counting: said on the line itself, open or not.
                    Repeater {
                        model: [line.modelData.waiting || "", line.modelData.groups_note || ""].filter(l => l !== "")

                        delegate: Label {
                            required property string modelData

                            objectName: "addressBookWaiting"
                            Layout.fillWidth: true
                            Layout.leftMargin: 18
                            text: modelData
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: setup.theme.text
                        }
                    }
                }
            }

            ColumnLayout {
                visible: line.open
                Layout.fillWidth: true
                Layout.leftMargin: 18
                spacing: 6

                // Whether its groups decide who may reach you: a matter of trust in who can write it.
                Switch {
                    objectName: "addressBookGroups"
                    Layout.fillWidth: true
                    checked: line.modelData.groups === true
                    text: setup.sioul.text("book-groups-count")
                    onToggled: {
                        setup.problem = setup.sioul.setBookGroups(line.modelData.place, checked)
                        setup.read()
                    }
                }

                // A server with one fixed book: what it allows, said.
                Label {
                    visible: line.modelData.fixed !== ""
                    Layout.fillWidth: true
                    text: line.modelData.fixed
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
                        text: setup.typed[line.modelData.place] !== undefined ? setup.typed[line.modelData.place] : line.modelData.name
                        Accessible.name: setup.sioul.text("books-rename")
                        onTextEdited: {
                            const next = Object.assign({}, setup.typed)
                            next[line.modelData.place] = newName.text
                            setup.typed = next
                        }
                        onAccepted: renameButton.clicked()
                    }
                    Button {
                        id: renameButton

                        objectName: "addressBookRename"
                        enabled: newName.text.trim() !== "" && newName.text.trim() !== line.modelData.name
                        text: setup.sioul.text("books-rename")
                        onClicked: {
                            setup.problem = setup.sioul.renameAddressBook(line.modelData.place, newName.text)
                            if (setup.problem === "") {
                                const next = Object.assign({}, setup.typed)
                                delete next[line.modelData.place]
                                setup.typed = next
                            }
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

                        readonly property var places: setup.others(line.modelData.place).map(b => b.place)

                        objectName: "addressBookMergeInto"
                        visible: setup.merging === line.modelData.place && setup.question.from !== line.modelData.place
                        width: Math.min(260, parent.width)
                        model: setup.others(line.modelData.place).map(b => setup.theme.plain(b.name))
                        currentIndex: Math.max(0, mergeInto.places.indexOf(setup.choice(line.modelData.place, mergeInto.places)))
                        Accessible.name: setup.sioul.text("books-merge-into")
                        onActivated: index => setup.choose(line.modelData.place, mergeInto.places[index])
                    }
                    Button {
                        objectName: "addressBookMergeGo"
                        visible: setup.merging === line.modelData.place && setup.question.from !== line.modelData.place
                        text: setup.sioul.text("books-merge-go")
                        onClicked: setup.ask(line.modelData.place, setup.choice(line.modelData.place, mergeInto.places))
                    }
                    Button {
                        visible: setup.merging === line.modelData.place && setup.question.from !== line.modelData.place
                        flat: true
                        text: setup.sioul.text("ui-cancel")
                        onClicked: setup.merging = ""
                    }
                }

                // The question before a merge, or a removal that moves the contacts: said, then confirmed.
                ColumnLayout {
                    visible: setup.question.from === line.modelData.place
                    Layout.fillWidth: true
                    spacing: 4

                    Repeater {
                        model: setup.question.from === line.modelData.place ? [setup.question.sentence || "", setup.question.losses || "", setup.question.shared || ""].filter(l => l !== "") : []

                        delegate: Label {
                            required property string modelData
                            required property int index

                            objectName: "addressBookMergeQuestion"
                            Layout.fillWidth: true
                            text: modelData
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.weight: index === 0 ? Font.DemiBold : Font.Normal
                            color: setup.theme.text
                        }
                    }
                    Repeater {
                        model: setup.question.from === line.modelData.place ? setup.question.cannot || [] : []

                        delegate: Label {
                            required property var modelData

                            Layout.fillWidth: true
                            text: setup.sioul.textArgs("batch-left-line", JSON.stringify({ title: modelData.name, why: modelData.why }))
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: setup.theme.warm
                        }
                    }
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            objectName: "addressBookMergeConfirm"
                            enabled: (setup.question.cannot || []).length === 0
                            highlighted: true
                            text: setup.sioul.text("books-merge-go")
                            onClicked: setup.merge()
                        }
                        Button {
                            flat: true
                            text: setup.sioul.text("ui-cancel")
                            onClicked: {
                                setup.question = {}
                                setup.merging = ""
                            }
                        }
                    }
                }

                // Its cards as one vCard file, where you choose.
                Button {
                    objectName: "addressBookExport"
                    visible: line.modelData.count > 0 && Qt.platform.os !== "android"
                    flat: true
                    text: setup.sioul.text("books-export")
                    onClicked: {
                        exportFile.place = line.modelData.place
                        exportFile.open()
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
                        visible: setup.removal.act === "cards" && (setup.removal.into || []).length > 0 && setup.question.from !== line.modelData.place
                        Layout.fillWidth: true
                        spacing: 6

                        PlainComboBox {
                            id: removeInto

                            readonly property var places: (setup.removal.into || []).map(b => b[0])

                            objectName: "addressBookRemoveInto"
                            width: Math.min(260, parent.width)
                            model: (setup.removal.into || []).map(b => setup.theme.plain(b[1]))
                            currentIndex: Math.max(0, removeInto.places.indexOf(setup.choice(line.modelData.place, removeInto.places)))
                            Accessible.name: setup.sioul.text("books-remove-move")
                            onActivated: index => setup.choose(line.modelData.place, removeInto.places[index])
                        }
                        Button {
                            objectName: "addressBookRemoveMove"
                            text: setup.sioul.text("books-remove-move-go")
                            onClicked: setup.ask(line.modelData.place, setup.choice(line.modelData.place, removeInto.places))
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
                            visible: setup.removal.act !== "none" && setup.question.from !== line.modelData.place
                            text: setup.sioul.text(setup.removal.act === "cards" ? "books-remove-delete" : setup.removal.act === "leave" ? "books-leave" : "books-remove-go")
                            onClicked: setup.remove(line.modelData.place, setup.removal.act === "cards" ? "delete" : setup.removal.act)
                        }
                        Button {
                            visible: setup.question.from !== line.modelData.place
                            flat: true
                            text: setup.sioul.text("ui-cancel")
                            onClicked: setup.removing = ""
                        }
                    }
                }
            }
        }
    }

    // Where an export goes.
    FileDialog {
        id: exportFile

        property string place: ""

        fileMode: FileDialog.SaveFile
        defaultSuffix: "vcf"
        nameFilters: [setup.sioul.text("books-export-kind") + " (*.vcf)"]
        onAccepted: setup.problem = setup.sioul.exportContacts(JSON.stringify({ book: exportFile.place }), exportFile.selectedFile.toString())
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

    // The changes to many contacts of the last thirty days, newest first, each undone as a whole.
    Label {
        Layout.fillWidth: true
        Layout.topMargin: 12
        text: setup.sioul.text("books-batches")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.weight: Font.DemiBold
        color: setup.theme.text
    }
    Label {
        Layout.fillWidth: true
        text: setup.sioul.text(setup.batches.length > 0 ? "books-batches-help" : "books-batches-none")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Repeater {
        model: setup.batches

        delegate: ColumnLayout {
            id: batch

            required property var modelData

            objectName: "contactsBatch"
            Layout.fillWidth: true
            spacing: 2

            Label {
                Layout.fillWidth: true
                text: [batch.modelData.line, batch.modelData.partly].filter(l => l !== "").join(" ")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: setup.theme.text
            }
            Label {
                visible: batch.modelData.why_not !== ""
                Layout.fillWidth: true
                text: batch.modelData.why_not
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: setup.theme.muted
            }
            Button {
                objectName: "contactsBatchUndo"
                visible: batch.modelData.why_not === "" && setup.undoing !== batch.modelData.id
                flat: true
                text: setup.sioul.text("batch-undo")
                onClicked: setup.undoing = batch.modelData.id
            }
            Label {
                visible: setup.undoing === batch.modelData.id
                Layout.fillWidth: true
                text: batch.modelData.ask
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: setup.theme.text
            }
            Flow {
                visible: setup.undoing === batch.modelData.id
                Layout.fillWidth: true
                spacing: 6

                Button {
                    objectName: "contactsBatchUndoGo"
                    text: setup.sioul.text("batch-undo-go")
                    onClicked: {
                        setup.problem = setup.sioul.undoContactsBatch(batch.modelData.id)
                        setup.undoing = ""
                    }
                }
                Button {
                    flat: true
                    text: setup.sioul.text("ui-cancel")
                    onClicked: setup.undoing = ""
                }
            }
        }
    }
}
