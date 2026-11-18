// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts imported from a file (wave D, contactimport.rs), in one window,
// three steps: what the file holds and how it was read (its encoding, the
// separator of a CSV file, which can be changed, each limit reached said in
// a sentence; for a CSV file, which column is what: each column offered a
// field or "leave out", guessed from its heading and shown with its first
// values, the whole kept under a name if you wish, at Import); where the
// cards go (an address book that can be written, or a new one) with groups
// added to every new card, the title moved into its field where it cannot
// be anything but a title, numbers written in their international form;
// then the count before anything is written: new cards, those already here
// (the same identifier, or the same name with an address or a number) left
// as they are, each listed with what the file would add, added only when
// "Complete them with what the file adds" is ticked (review F14). Import is
// one batch with its progress, undone as a whole. The file is read on this
// device alone; nothing is sent anywhere.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: dialog

    required property var sioul
    required property var theme
    // The import's step (`contactsImport`): {step, line, kind, name, columns, fields, books, homes, broken, kept, export, more, count}.
    readonly property var state: dialog.sioul.contactsImport ? JSON.parse(dialog.sioul.contactsImport) : ({ step: "closed" })
    // What was read (the first step), kept while the plan is asked.
    property var read: ({ columns: [], fields: [], books: [], homes: [] })
    // Each column's field, as chosen.
    property var fields: []
    // Something changed since the count was asked: it is asked again before Import.
    property bool edited: true
    // The cards here under another name to complete all the same, by file, ticked one by one (review V5).
    property var completeAlso: []
    // The file's own groups that a list reads, kept on its new cards (off at first, review V6).
    property bool keepListed: false

    // Shown when a step comes; closed when the import is.
    function follow() {
        const step = dialog.state.step
        if (step === "read") {
            const another = dialog.read.name !== dialog.state.name
            dialog.read = dialog.state
            dialog.fields = (dialog.state.columns || []).map(c => c.field)
            dialog.edited = true
            if (another) {
                complete.checked = false
                groupsHere.checked = false
                dialog.completeAlso = []
                dialog.keepListed = false
                international.checked = dialog.state.region === true && dialog.state.kind === "csv"
            }
        }
        if (step === "closed") {
            if (dialog.opened)
                dialog.close()
        } else if (!dialog.opened) {
            dialog.open()
        }
    }

    // The separators a CSV file can be read with.
    readonly property var separators: [";", ",", "\t"]

    // What the import asks, for its plan.
    function asked() {
        const books = dialog.read.books || []
        const newBook = bookChoice.currentIndex >= books.length
        const home = (dialog.read.homes || [])[Math.max(0, homeChoice.currentIndex)]
        return JSON.stringify({
            fields: dialog.fields,
            book: newBook ? "" : books[Math.max(0, bookChoice.currentIndex)].place,
            new_account: newBook && home ? home.id : "",
            new_name: newBook ? newName.text : "",
            groups: groups.text,
            groups_here: groupsHere.checked,
            complete: complete.checked,
            titles: titles.checked,
            international: international.checked,
            keep_as: keepAs.text,
            complete_also: dialog.completeAlso,
            keep_listed: dialog.keepListed
        })
    }

    function count() {
        dialog.edited = false
        dialog.sioul.planContactsImport(dialog.asked())
    }

    Connections {
        target: dialog.sioul

        function onContactsImportChanged() {
            dialog.follow()
        }
    }

    objectName: "contactImport"
    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(640, dialog.roomWidth - 2 * dialog.theme.gap)
    title: dialog.sioul.text("import-title")
    closePolicy: Popup.CloseOnEscape
    onRejected: dialog.sioul.closeContactsImport()

    contentItem: ScrollView {
        implicitHeight: Math.min(steps.implicitHeight, dialog.roomHeight * 0.7)
        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: steps

            width: parent ? parent.width : 500
            spacing: 8

            // What the file holds, or why it does not read.
            Label {
                objectName: "importLine"
                Layout.fillWidth: true
                text: (dialog.read.name ? dialog.read.name + " — " : "") + (dialog.state.step === "planned" ? dialog.read.line || "" : dialog.state.line || "")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.weight: Font.DemiBold
                color: dialog.state.step === "problem" ? dialog.theme.warm : dialog.theme.text
            }
            Repeater {
                model: [dialog.read.export || "", dialog.read.kept || "", dialog.read.broken || ""].concat(dialog.read.reading || []).filter(l => l !== "")

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
            }

            // For a CSV file: its separator, read again when another is chosen.
            RowLayout {
                visible: dialog.read.kind === "csv"
                Layout.fillWidth: true
                spacing: 6

                Label {
                    text: dialog.sioul.text("import-separator")
                    color: dialog.theme.text
                }
                PlainComboBox {
                    objectName: "importSeparator"
                    Layout.preferredWidth: 220
                    model: [dialog.sioul.text("import-separator-semicolon"), dialog.sioul.text("import-separator-comma"), dialog.sioul.text("import-separator-tab")]
                    currentIndex: Math.max(0, dialog.separators.indexOf(dialog.read.separator || ";"))
                    Accessible.name: dialog.sioul.text("import-separator")
                    onActivated: index => {
                        dialog.edited = true
                        dialog.sioul.separateContactsImport(dialog.separators[index])
                    }
                }
            }

            // For a CSV file: which column is what.
            Label {
                visible: (dialog.read.columns || []).length > 0
                text: dialog.sioul.text("import-columns")
                font.weight: Font.DemiBold
                color: dialog.theme.text
            }
            Repeater {
                model: dialog.read.columns || []

                delegate: GridLayout {
                    id: column

                    required property var modelData
                    required property int index

                    objectName: "importColumn"
                    Layout.fillWidth: true
                    columns: dialog.width < 520 ? 1 : 2
                    columnSpacing: 8
                    rowSpacing: 2

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: column.modelData.heading
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: dialog.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: column.modelData.samples
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            color: dialog.theme.muted
                        }
                    }
                    PlainComboBox {
                        objectName: "importField"
                        Layout.preferredWidth: 220
                        Layout.fillWidth: dialog.width < 520
                        model: (dialog.read.fields || []).map(f => dialog.theme.plain(f.label))
                        currentIndex: Math.max(0, (dialog.read.fields || []).findIndex(f => f.field === dialog.fields[column.index]))
                        Accessible.name: dialog.sioul.text("import-columns")
                        onActivated: index => {
                            const next = dialog.fields.slice()
                            next[column.index] = dialog.read.fields[index].field
                            dialog.fields = next
                            dialog.edited = true
                        }
                    }
                }
            }
            TextField {
                id: keepAs

                objectName: "importKeepAs"
                visible: (dialog.read.columns || []).length > 0
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("import-keep-as")
                onTextEdited: dialog.edited = true
            }

            // Where they go.
            Label {
                text: dialog.sioul.text("import-into")
                font.weight: Font.DemiBold
                color: dialog.theme.text
            }
            PlainComboBox {
                id: bookChoice

                objectName: "importBook"
                Layout.fillWidth: true
                model: (dialog.read.books || []).map(b => dialog.theme.plain(b.name)).concat([dialog.sioul.text("import-into-new")])
                Accessible.name: dialog.sioul.text("import-into")
                onActivated: dialog.edited = true
            }
            RowLayout {
                visible: bookChoice.currentIndex >= (dialog.read.books || []).length
                Layout.fillWidth: true
                spacing: 6

                PlainComboBox {
                    id: homeChoice

                    Layout.preferredWidth: 220
                    model: (dialog.read.homes || []).map(h => dialog.theme.plain(h.name))
                    Accessible.name: dialog.sioul.text("books-new-where")
                    onActivated: dialog.edited = true
                }
                TextField {
                    id: newName

                    Layout.fillWidth: true
                    placeholderText: dialog.sioul.text("books-new-name")
                    onTextEdited: dialog.edited = true
                }
            }
            TextField {
                id: groups

                objectName: "importGroups"
                Layout.fillWidth: true
                placeholderText: dialog.sioul.text("import-groups")
                onTextEdited: dialog.edited = true
            }
            CheckBox {
                id: groupsHere

                objectName: "importGroupsHere"
                visible: groups.text.trim() !== ""
                text: dialog.sioul.text("import-groups-here")
                onToggled: dialog.edited = true
            }
            CheckBox {
                id: titles

                text: dialog.sioul.text("import-titles")
                onToggled: dialog.edited = true
            }
            CheckBox {
                id: international

                objectName: "importInternational"
                visible: dialog.read.region === true
                text: dialog.theme.plain(dialog.read.international || "")
                onToggled: dialog.edited = true
            }
            Label {
                visible: dialog.read.region === false
                Layout.fillWidth: true
                text: dialog.read.international || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.muted
            }

            // The count, before anything is written.
            Label {
                objectName: "importSummary"
                visible: dialog.state.step === "planned" || dialog.state.step === "problem-plan"
                Layout.fillWidth: true
                text: dialog.state.step === "planned" || dialog.state.step === "problem-plan" ? dialog.state.line || "" : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.weight: Font.DemiBold
                color: dialog.state.step === "problem-plan" ? dialog.theme.warm : dialog.theme.text
            }
            Repeater {
                model: dialog.state.step === "planned" ? dialog.state.more || [] : []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
                }
            }

            // Those already here, card by card, with what the file would add: added only when ticked.
            CheckBox {
                id: complete

                objectName: "importComplete"
                visible: dialog.state.step === "planned" && (dialog.state.gaining || 0) > 0
                text: dialog.sioul.text("import-complete")
                onToggled: dialog.count()
            }
            Label {
                visible: complete.visible
                Layout.fillWidth: true
                text: dialog.sioul.text("import-complete-help")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: dialog.theme.muted
            }
            Label {
                visible: complete.visible
                Layout.fillWidth: true
                text: dialog.sioul.text("import-gains")
                font.weight: Font.DemiBold
                color: dialog.theme.text
            }
            // Every card that would gain, in a list that draws only what shows (review V9); a card
            // whose identifier is here under another name has its own box, unticked (review V5).
            ListView {
                id: gains

                objectName: "importGains"
                visible: dialog.state.step === "planned" && count > 0
                Layout.fillWidth: true
                Layout.leftMargin: 12
                Layout.preferredHeight: Math.min(contentHeight, 320)
                clip: true
                model: dialog.state.step === "planned" ? dialog.state.matches || [] : []

                delegate: Item {
                    id: gain

                    required property var modelData

                    width: gains.width
                    height: Math.max(gainLine.implicitHeight, gainBox.visible ? gainBox.implicitHeight : 0) + 4

                    CheckBox {
                        id: gainBox

                        objectName: "importGainBox"
                        visible: gain.modelData.other !== ""
                        checked: gain.modelData.ticked === true
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        Accessible.name: gain.modelData.name
                        onToggled: {
                            const next = dialog.completeAlso.filter(k => k !== gain.modelData.key)
                            if (gainBox.checked)
                                next.push(gain.modelData.key)
                            dialog.completeAlso = next
                            dialog.count()
                        }
                    }
                    Label {
                        id: gainLine

                        objectName: "importGain"
                        anchors.left: gainBox.visible ? gainBox.right : parent.left
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        text: (gainBox.visible ? "" : "•  ") + gain.modelData.name + (gain.modelData.in_file ? " " + dialog.sioul.text("import-gain-in-file") : "") + (gain.modelData.other !== "" ? " (" + gain.modelData.other + ")" : "") + " — " + gain.modelData.adds
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: dialog.theme.text
                    }
                }
            }
            // The file's own groups that one of your lists reads: kept on its new cards only when ticked (review V6).
            CheckBox {
                objectName: "importKeepListed"
                visible: dialog.state.step === "planned" && (dialog.state.keep_listed || "") !== ""
                Layout.fillWidth: true
                checked: dialog.keepListed
                text: dialog.theme.plain(dialog.state.keep_listed || "")
                onToggled: {
                    dialog.keepListed = checked
                    dialog.count()
                }
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            text: dialog.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        Button {
            objectName: "importCheck"
            enabled: dialog.state.step === "read" || dialog.state.step === "planned" || dialog.state.step === "problem-plan"
            text: dialog.sioul.text("import-check")
            onClicked: dialog.count()
        }
        Button {
            objectName: "importGo"
            enabled: dialog.state.step === "planned" && !dialog.edited && (dialog.state.count || 0) > 0
            highlighted: true
            text: dialog.sioul.text("import-go")
            onClicked: {
                const problem = dialog.sioul.runContactsImport()
                if (problem !== "")
                    dialog.sioul.status = problem
            }
        }
    }
}
