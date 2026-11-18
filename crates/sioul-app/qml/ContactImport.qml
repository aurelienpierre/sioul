// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts imported from a file (wave D, contactimport.rs), in one window,
// three steps: what the file holds (for a CSV file, which column is what:
// each column offered a field or "leave out", guessed from its heading and
// shown with its first values, the whole kept under a name if you wish);
// where the cards go (an address book that can be written, or a new one)
// with groups added to every card and the title at the start of a name
// moved into its field; then the count before anything is written ("312
// new, 41 completed, 3 rows that do not read"), and Import, one batch with
// its progress, undone as a whole from the status line. The file is read on
// this device alone; nothing is sent anywhere.

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

    // Shown when a step comes; closed when the import is.
    function follow() {
        const step = dialog.state.step
        if (step === "read") {
            dialog.read = dialog.state
            dialog.fields = (dialog.state.columns || []).map(c => c.field)
            dialog.edited = true
        }
        if (step === "closed") {
            if (dialog.opened)
                dialog.close()
        } else if (!dialog.opened) {
            dialog.open()
        }
    }

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
            titles: titles.checked,
            keep_as: keepAs.text
        })
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
                model: [dialog.read.export || "", dialog.read.kept || "", dialog.read.broken || ""].filter(l => l !== "")

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: dialog.theme.muted
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
                id: titles

                text: dialog.sioul.text("import-titles")
                onToggled: dialog.edited = true
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
            onClicked: {
                dialog.edited = false
                dialog.sioul.planContactsImport(dialog.asked())
            }
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
