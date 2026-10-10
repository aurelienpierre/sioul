// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The search by conditions, in the folders' place (MailPage): plain fields,
// one condition at first, more when asked. The conditions are the mail
// filters' own (a field, a test, a value; all of them or any), so a search
// can become a filter as it is. The list beside shows what they find and the
// reader beside it, so one goes back and forth; the sentence over the list
// says what is searched. "Clear" returns to the folder.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ScrollView {
    id: column

    required property var sioul
    required property var theme
    // On a phone the column is a page of its own: its results are one tap away.
    property bool compact: false
    // What the fields offer, in your language (`mailSearchFields`): read when the search opens.
    property var words: null
    property bool any: false
    // The window can take a search to the filters' editor: "Make it a filter…" shows.
    property bool filterOffered: false

    // A condition changed: the page searches (`now`: Enter was pressed).
    signal changed(bool now)
    signal clearRequested
    signal showRequested
    // "Make it a filter…": the conditions, as the filters write them.
    signal filterRequested(string conditions)

    // The conditions as JSON, as the filters write them: {conditions, match}.
    function json() {
        const conditions = []
        for (let i = 0; i < conditionsModel.count; i++) {
            const c = conditionsModel.get(i)
            conditions.push({ field: c.field, test: c.test, value: c.value, until: c.until })
        }
        return JSON.stringify({ conditions: conditions, match: column.any ? "any" : "all" })
    }

    // Opened: one condition, anywhere, with what the quick search held.
    function start(text) {
        column.words = JSON.parse(column.sioul.mailSearchFields())
        column.any = false
        conditionsModel.clear()
        conditionsModel.append({ field: "anywhere", test: "contains", value: text, until: "" })
        column.changed(true)
    }

    // For the window's pictures: conditions given whole, [{field, test, value, until}].
    function startWith(conditions, any) {
        column.words = JSON.parse(column.sioul.mailSearchFields())
        column.any = any
        conditionsModel.clear()
        for (const c of conditions)
            conditionsModel.append({ field: c.field, test: c.test, value: c.value || "", until: c.until || "" })
        column.changed(true)
    }

    function fieldOf(id) {
        return column.words ? column.words.fields.find(f => f.id === id) || null : null
    }

    function testOf(field, id) {
        const f = column.fieldOf(field)
        return f ? f.tests.find(t => t.id === id) || null : null
    }

    // What a value is, under a field and a test: "nothing", "text", "date",
    // "dates", "size", "kind", "who", and the search's own "account", "folder", "mark".
    function kindOf(field, test) {
        if (field === "account" || field === "folder" || field === "mark")
            return field
        const t = column.testOf(field, test)
        return t ? t.value : "text"
    }

    // A value to start with, for a kind of value.
    function firstValue(kind) {
        if (!column.words)
            return ""
        switch (kind) {
        case "kind":
            return column.words.kinds.length > 0 ? column.words.kinds[0].id : ""
        case "who":
            return column.words.who.length > 0 ? column.words.who[0].id : ""
        case "mark":
            return "read"
        case "folder":
            return "inbox"
        case "account":
            return column.words.accounts.length > 0 ? column.words.accounts[0].id : ""
        default:
            return ""
        }
    }

    function setField(index, field) {
        const f = column.fieldOf(field)
        if (!f || conditionsModel.get(index).field === field)
            return
        const test = f.tests[0].id
        conditionsModel.set(index, { field: field, test: test, value: column.firstValue(column.kindOf(field, test)), until: "" })
        column.changed(false)
    }

    function setTest(index, test) {
        const c = conditionsModel.get(index)
        if (c.test === test)
            return
        const before = column.kindOf(c.field, c.test)
        const after = column.kindOf(c.field, test)
        conditionsModel.setProperty(index, "test", test)
        if (before !== after) {
            conditionsModel.setProperty(index, "value", column.firstValue(after))
            conditionsModel.setProperty(index, "until", "")
        }
        column.changed(false)
    }

    function setValue(index, key, value, now) {
        conditionsModel.setProperty(index, key, value)
        column.changed(now)
    }

    function add() {
        conditionsModel.append({ field: "from", test: "contains", value: "", until: "" })
    }

    function remove(index) {
        conditionsModel.remove(index)
        column.changed(false)
    }

    contentWidth: availableWidth
    clip: true

    ListModel {
        id: conditionsModel
    }

    ColumnLayout {
        width: column.availableWidth
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: column.sioul.text("search-title")
            font.pixelSize: 19
            color: column.theme.text
        }
        Label {
            Layout.fillWidth: true
            text: column.words ? column.words.help : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: column.theme.muted
        }

        // All of them, or any: asked once there are two.
        PlainComboBox {
            id: joinBox

            visible: conditionsModel.count > 1
            Layout.fillWidth: true
            model: [column.sioul.text("search-match-all"), column.sioul.text("search-match-any")]
            currentIndex: column.any ? 1 : 0
            Accessible.name: column.sioul.text("search-match-all")
            onActivated: index => {
                column.any = index === 1
                column.changed(false)
            }
        }

        Repeater {
            model: conditionsModel

            delegate: ColumnLayout {
                id: row

                required property int index
                required property string field
                required property string test
                required property string value
                required property string until
                readonly property var fieldInfo: column.fieldOf(row.field)
                readonly property string kind: column.kindOf(row.field, row.test)

                Layout.fillWidth: true
                Layout.topMargin: row.index > 0 ? 6 : 0
                spacing: 4

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    PlainComboBox {
                        id: fieldBox

                        Layout.fillWidth: true
                        model: column.words ? column.words.fields.map(f => Object.assign({}, f, { label: column.theme.plain(f.label) })) : []
                        textRole: "label"
                        valueRole: "id"
                        Accessible.name: row.fieldInfo ? row.fieldInfo.label : ""
                        Component.onCompleted: fieldBox.currentIndex = fieldBox.indexOfValue(row.field)
                        onActivated: column.setField(row.index, fieldBox.currentValue)

                        Connections {
                            target: row
                            function onFieldChanged() {
                                fieldBox.currentIndex = fieldBox.indexOfValue(row.field)
                            }
                        }
                    }
                    ToolButton {
                        visible: conditionsModel.count > 1
                        text: "×"
                        Accessible.name: column.sioul.text("search-remove")
                        ToolTip.visible: hovered
                        ToolTip.text: column.sioul.text("search-remove")
                        ToolTip.delay: 600
                        onClicked: column.remove(row.index)
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    PlainComboBox {
                        id: testBox

                        // Alone on its line when nothing follows it ("there is one");
                        // narrower before a day, whose field needs the room.
                        Layout.fillWidth: row.kind === "nothing"
                        Layout.preferredWidth: row.kind === "nothing" ? -1 : row.kind === "date" || row.kind === "dates" ? 96 : 128
                        model: row.fieldInfo ? row.fieldInfo.tests.map(t => Object.assign({}, t, { label: column.theme.plain(t.label) })) : []
                        textRole: "label"
                        valueRole: "id"
                        Accessible.name: row.fieldInfo ? row.fieldInfo.label : ""
                        Component.onCompleted: testBox.currentIndex = testBox.indexOfValue(row.test)
                        onActivated: column.setTest(row.index, testBox.currentValue)
                        onModelChanged: testBox.currentIndex = testBox.indexOfValue(row.test)

                        Connections {
                            target: row
                            function onTestChanged() {
                                testBox.currentIndex = testBox.indexOfValue(row.test)
                            }
                        }
                    }

                    // Words: a name, an address, a subject, a size.
                    TextField {
                        id: textValue

                        visible: row.kind === "text" || row.kind === "size" || row.kind === "time" || row.kind === "times" || row.kind === "days"
                        Layout.fillWidth: true
                        text: row.value
                        placeholderText: column.sioul.text(row.kind === "size" ? "search-placeholder-size" : (row.field === "from" || row.field === "to" || row.field === "cc") ? "search-placeholder-who" : "search-placeholder")
                        Accessible.name: row.fieldInfo ? row.fieldInfo.label : ""
                        onTextEdited: column.setValue(row.index, "value", textValue.text, false)
                        onAccepted: column.changed(true)
                    }

                    // A day; "between": the first, the last below.
                    DateField {
                        visible: row.kind === "date" || row.kind === "dates"
                        Layout.fillWidth: true
                        theme: column.theme
                        sioul: column.sioul
                        locale: Qt.locale(column.sioul.text("qt-locale"))
                        date: row.value
                        pickLabel: row.fieldInfo ? row.fieldInfo.label : ""
                        onEdited: column.setValue(row.index, "value", date, false)
                    }

                    // A choice: a kind of file, who the sender is, a mark, an address, a folder.
                    PlainComboBox {
                        id: choiceBox

                        readonly property var choices: !column.words ? [] : row.kind === "kind" ? column.words.kinds : row.kind === "who" ? column.words.who : row.kind === "mark" ? column.words.marks : row.kind === "account" ? column.words.accounts : row.kind === "folder" ? column.words.folders : []

                        visible: choiceBox.choices.length > 0
                        Layout.fillWidth: true
                        model: choiceBox.choices.map(c => Object.assign({}, c, { label: column.theme.plain(c.label) }))
                        textRole: "label"
                        valueRole: "id"
                        Accessible.name: row.fieldInfo ? row.fieldInfo.label : ""
                        Component.onCompleted: choiceBox.currentIndex = choiceBox.indexOfValue(row.value)
                        onModelChanged: choiceBox.currentIndex = choiceBox.indexOfValue(row.value)
                        onActivated: column.setValue(row.index, "value", choiceBox.currentValue, false)

                        Connections {
                            target: row
                            function onValueChanged() {
                                if (choiceBox.visible)
                                    choiceBox.currentIndex = choiceBox.indexOfValue(row.value)
                            }
                        }
                    }
                }

                RowLayout {
                    visible: row.kind === "dates"
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        Layout.preferredWidth: 96
                        horizontalAlignment: Text.AlignRight
                        rightPadding: 8
                        text: column.sioul.text("search-and")
                        color: column.theme.muted
                    }
                    DateField {
                        Layout.fillWidth: true
                        theme: column.theme
                        sioul: column.sioul
                        locale: Qt.locale(column.sioul.text("qt-locale"))
                        date: row.until
                        pickLabel: row.fieldInfo ? row.fieldInfo.label : ""
                        onEdited: column.setValue(row.index, "until", date, false)
                    }
                }
            }
        }

        Button {
            flat: true
            text: "+ " + column.sioul.text("search-add")
            onClicked: column.add()
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 6
            spacing: 8

            Button {
                text: column.sioul.text("search-clear")
                onClicked: column.clearRequested()
            }
            // Given to the filters' editor, when the window has one to open.
            Button {
                visible: column.filterOffered
                flat: true
                text: column.sioul.text("search-make-filter")
                onClicked: column.filterRequested(column.json())
            }
            Item {
                Layout.fillWidth: true
            }
        }
        // On a phone, the results are a page of their own: the way there on
        // a line of its own (three buttons side by side are wider than a phone).
        Button {
            visible: column.compact
            Layout.fillWidth: true
            Layout.topMargin: 4
            highlighted: true
            text: column.sioul.text("search-show")
            onClicked: column.showRequested()
        }
    }
}
