// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Projects and cases, the umbrella over everything else. On the left, each
// with its open tasks and the time left to bill; on the right, the one open:
// what is coming, then what happened, on one line of time; its tasks one
// click away as a board, a list or a calendar; its time and its invoices.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import com.aurelienpierre.sioul

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var rows: []
    // The mail that belongs to the one open: its routes, folded until asked.
    property bool routesShown: false
    property var routes: []
    // Quiet time: only your own projects, unless you ask for the others.
    property bool anyway: false
    readonly property bool resting: page.window.moment.quiet && !page.anyway
    readonly property var listed: page.resting ? page.rows.filter(r => r.personal) : page.rows
    property string openId: ""
    // On a phone, the project open takes the page; Back closes it (main.qml).
    readonly property bool canGoBack: page.openId !== ""
    function back() {
        page.openId = ""
    }
    property var shown: null
    property string problem: ""
    // Invoices are numbered on another computer: offered here, on purpose.
    property bool invoicesElsewhere: false

    function reload() {
        page.rows = JSON.parse(page.sioul.projectRows() || "[]")
        page.shown = page.openId !== "" ? JSON.parse(page.sioul.projectPage(page.openId) || "null") : null
        page.routes = page.openId !== "" ? JSON.parse(page.sioul.settings("project:" + page.openId) || "[]") : []
    }

    function saveRoutes(key, value) {
        page.problem = page.sioul.setSetting(key, JSON.stringify(value))
        if (page.problem === "")
            page.reload()
    }

    function open(id) {
        page.openId = id
        page.problem = ""
        page.reload()
    }

    // A new project: its form.
    function startNew() {
        projectDialog.edit("", null)
    }

    function openFirst() {
        if (page.rows.length > 0)
            page.open(page.rows[0].id)
    }

    // An amount as the core says amounts: digits in your language's way, the
    // currency where it puts it ("€62.50", "62,50 €"), cents only when there are some.
    function money(value) {
        const digits = Number(value).toLocaleString(page.window.sioulLocale, "f", Number.isInteger(value) ? 0 : 2)
        return page.sioul.textWith("money-positive", "value", digits)
    }

    // The invoice of the hours not billed yet, printed where invoices go, then shown.
    function makeInvoice() {
        const made = JSON.parse(page.sioul.makeInvoice(page.openId))
        page.invoicesElsewhere = made.elsewhere === true
        if (made.error) {
            page.problem = made.error
            return
        }
        page.printInvoice(made)
    }

    function printInvoice(made) {
        const failed = pdf.write(made.html, made.pdf)
        if (failed !== "") {
            page.problem = page.sioul.textWith("invoice-not-written", "path", failed)
            return
        }
        page.sioul.status = page.sioul.textWith("invoice-written", "path", made.pdf)
        // Shown in your PDF reader, except while the window takes its own pictures.
        if (page.sioul.grabFolder() === "")
            Qt.openUrlExternally(page.theme.fileUrl(made.pdf))
    }

    // Shown anyway once: the next visit in quiet time asks again.
    onVisibleChanged: {
        if (page.visible)
            page.reload()
        else
            page.anyway = false
    }
    Component.onCompleted: page.reload()

    Connections {
        target: page.sioul

        function onLinksChanged() {
            page.reload()
        }

        function onTasksChanged() {
            if (page.visible)
                page.reload()
        }
    }

    PdfWriter {
        id: pdf
    }

    RowLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // Every project and case; on a phone, the whole page until one is open.
        ColumnLayout {
            visible: !(page.window.compact && page.openId !== "")
            Layout.fillHeight: true
            Layout.fillWidth: page.window.compact
            Layout.preferredWidth: 300
            spacing: 8

            RowLayout {
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("ui-projects")
                    font.pixelSize: 22
                    color: page.theme.text
                }
                Button {
                    text: page.sioul.text("project-new")
                    icon.name: "list-add"
                    icon.color: page.theme.text
                    onClicked: projectDialog.edit("", null)
                }
            }
            Label {
                visible: page.rows.length === 0
                Layout.fillWidth: true
                text: page.sioul.text("project-none")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            RowLayout {
                visible: page.resting && page.listed.length < page.rows.length
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("project-resting")
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                Button {
                    flat: true
                    text: page.sioul.text("ui-show-anyway")
                    onClicked: page.anyway = true
                }
            }
            ListView {
                id: list

                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                model: page.listed
                ScrollBar.vertical: ScrollBar {}

                delegate: ItemDelegate {
                    id: row

                    required property var modelData

                    width: list.width - 12
                    padding: 8
                    highlighted: page.openId === row.modelData.id
                    onClicked: page.open(row.modelData.id)

                    background: Rectangle {
                        color: row.highlighted || row.hovered ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                        border.color: row.visualFocus ? page.theme.focus : row.highlighted ? page.theme.line : "transparent"
                    }

                    contentItem: RowLayout {
                        spacing: 8

                        Icon {
                            iconName: row.modelData.is_project ? "view-financial-account" : "folder-documents"
                            size: 16
                            opacity: row.modelData.status === "closed" ? 0.5 : 1
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Label {
                                Layout.fillWidth: true
                                text: row.modelData.title
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: row.modelData.status === "closed" ? page.theme.muted : page.theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: [row.modelData.client, row.modelData.open_tasks > 0 ? page.sioul.textArgs("project-open-tasks", JSON.stringify({ n: row.modelData.open_tasks })) : "", row.modelData.unbilled !== "" ? page.sioul.textWith("project-to-bill", "time", row.modelData.unbilled) : ""].filter(t => t !== "").join("  ·  ")
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                        }
                    }
                }
            }
        }

        // The one open.
        ScrollView {
            id: detail

            visible: !page.window.compact || page.openId !== ""
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth
            clip: true

            ColumnLayout {
                width: detail.availableWidth
                spacing: 10

                Label {
                    visible: page.shown === null
                    Layout.fillWidth: true
                    Layout.topMargin: 40
                    text: page.sioul.text("project-choose")
                    horizontalAlignment: Text.AlignHCenter
                    color: page.theme.muted
                }

                RowLayout {
                    visible: page.shown !== null
                    Layout.fillWidth: true

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        Label {
                            Layout.fillWidth: true
                            text: page.shown ? page.shown.title : ""
                            textFormat: Text.PlainText
                            font.pixelSize: 22
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: page.shown ? [page.shown.is_project ? page.sioul.text("project-kind-project") : page.sioul.text("project-kind-case"), page.shown.client, page.shown.is_project && page.shown.rate > 0 ? page.sioul.textWith("project-rate", "rate", page.money(page.shown.rate)) : "", page.shown.status !== "" ? page.sioul.text("project-status-" + page.shown.status) : ""].filter(t => t !== "").join("  ·  ") : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.muted
                        }
                    }
                    Button {
                        flat: true
                        text: page.sioul.text("ui-edit")
                        icon.name: "document-edit"
                        icon.color: page.theme.text
                        onClicked: projectDialog.edit(page.openId, page.shown)
                    }
                }

                // Its tasks, its time, its invoice: one click each.
                Flow {
                    visible: page.shown !== null
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        text: page.sioul.text("project-board")
                        icon.name: "view-list-details"
                        icon.color: page.theme.text
                        onClicked: page.window.showTasksOf(page.openId, "board")
                    }
                    Button {
                        text: page.sioul.text("project-list")
                        icon.name: "view-list-text"
                        icon.color: page.theme.text
                        onClicked: page.window.showTasksOf(page.openId, "list")
                    }
                    Button {
                        text: page.sioul.text("project-calendar")
                        icon.name: "view-calendar-timeline"
                        icon.color: page.theme.text
                        onClicked: page.window.showTasksOf(page.openId, "timeline")
                    }
                    Button {
                        text: page.sioul.text("time-note")
                        icon.name: "chronometer-start"
                        icon.color: page.theme.text
                        onClicked: timeDialog.begin(page.openId)
                    }
                    Button {
                        visible: page.shown !== null && page.shown.is_project && page.shown.unbilled !== ""
                        text: page.sioul.text("invoice-make")
                        icon.name: "document-new"
                        icon.color: page.theme.accentText
                        highlighted: true
                        onClicked: page.makeInvoice()
                    }
                }

                Label {
                    visible: page.shown !== null
                    Layout.fillWidth: true
                    text: page.shown ? [page.sioul.textArgs("project-tasks-count", JSON.stringify({ open: page.shown.open_tasks })) + (page.shown.done_tasks > 0 ? ", " + page.sioul.textArgs("project-tasks-done", JSON.stringify({ done: page.shown.done_tasks })) : ""), page.shown.time !== "" ? page.sioul.textWith("project-time-all", "time", page.shown.time) : "", page.shown.unbilled !== "" ? page.sioul.textWith("project-to-bill-amount", "time", page.shown.unbilled) + (page.shown.unbilled_amount !== "" ? " (" + page.shown.unbilled_amount + ")" : "") : ""].filter(t => t !== "").join("  ·  ") : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                Label {
                    visible: page.problem !== ""
                    Layout.fillWidth: true
                    text: page.problem
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.warm
                }
                Button {
                    visible: page.invoicesElsewhere
                    text: page.sioul.text("invoice-take")
                    onClicked: {
                        page.problem = page.sioul.takeInvoices()
                        page.invoicesElsewhere = false
                    }
                }

                ThingActions {
                    visible: page.shown !== null
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    source: page.shown ? { uri: "sioul:case/" + encodeURIComponent(page.openId), kind: "case", key: page.openId, title: page.shown.title } : null
                }

                // The mail that comes here by itself: by sender, words, attachments; replies follow.
                Button {
                    visible: page.shown !== null && page.routes.length > 0
                    flat: true
                    text: (page.routesShown ? "▾  " : "▸  ") + page.sioul.text("project-routes")
                    onClicked: page.routesShown = !page.routesShown
                }
                Repeater {
                    model: page.routesShown && page.shown !== null ? page.routes : []

                    delegate: SettingRow {
                        required property var modelData

                        Layout.fillWidth: true
                        setting: modelData
                        sioul: page.sioul
                        theme: page.theme
                        onSave: (key, value) => page.saveRoutes(key, value)
                    }
                }

                // Its invoices: printed again, paid.
                Label {
                    visible: page.shown !== null && page.shown.invoices.length > 0
                    Layout.topMargin: 6
                    text: page.sioul.text("project-invoices")
                    font.weight: Font.DemiBold
                    color: page.theme.text
                }
                Repeater {
                    model: page.shown ? page.shown.invoices : []

                    delegate: RowLayout {
                        id: bill

                        required property var modelData

                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            Layout.fillWidth: true
                            text: [bill.modelData.number, bill.modelData.date, bill.modelData.total].join("  ·  ")
                            textFormat: Text.PlainText
                            color: page.theme.text
                        }
                        CheckBox {
                            text: page.sioul.text("invoice-paid")
                            checked: bill.modelData.paid
                            onToggled: page.problem = page.sioul.setInvoicePaid(bill.modelData.number, checked)
                        }
                        Button {
                            flat: true
                            text: page.sioul.text("invoice-print")
                            onClicked: {
                                const again = JSON.parse(page.sioul.invoiceAgain(bill.modelData.number))
                                if (again.error)
                                    page.problem = again.error
                                else
                                    page.printInvoice(again)
                            }
                        }
                    }
                }

                // One line of time: what is coming, then what happened.
                Label {
                    visible: page.shown !== null && page.shown.moments.length > 0
                    Layout.topMargin: 6
                    text: page.sioul.text("project-timeline")
                    font.weight: Font.DemiBold
                    color: page.theme.text
                }
                Repeater {
                    model: page.shown ? page.shown.moments : []

                    delegate: ItemDelegate {
                        id: moment

                        required property var modelData
                        required property int index
                        // "Coming" over the first of what is to come, "Before" over the first of what was.
                        readonly property bool turns: moment.index === 0 || page.shown.moments[moment.index - 1].coming !== moment.modelData.coming

                        Layout.fillWidth: true
                        padding: 4
                        enabled: moment.modelData.uri !== ""
                        onClicked: page.window.openThing({ uri: moment.modelData.uri, kind: moment.modelData.kind, key: moment.modelData.key })

                        background: Rectangle {
                            color: moment.hovered && moment.enabled ? page.theme.surface : "transparent"
                            radius: page.theme.radius
                        }

                        contentItem: ColumnLayout {
                            spacing: 2

                            Label {
                                visible: moment.turns
                                Layout.topMargin: moment.index === 0 ? 0 : 8
                                text: page.sioul.text(moment.modelData.coming ? "project-coming" : "project-before")
                                textFormat: Text.PlainText
                                font.pixelSize: 12
                                font.weight: Font.DemiBold
                                color: page.theme.accent
                            }
                            RowLayout {
                                spacing: 8

                                Icon {
                                    iconName: moment.modelData.kind === "time" ? "chronometer-start" : moment.modelData.kind === "invoice" ? "document-new" : page.theme.kindIcons[moment.modelData.kind] || "insert-link"
                                    size: 16
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 0

                                    Label {
                                        Layout.fillWidth: true
                                        text: moment.modelData.title
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: page.theme.text
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: [moment.modelData.date, moment.modelData.detail].filter(t => t !== "").join("  ·  ")
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        font.pixelSize: 12
                                        color: page.theme.muted
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ProjectDialog {
        id: projectDialog

        sioul: page.sioul
        theme: page.theme
        onSaved: id => page.open(id)
        onRemoved: {
            page.openId = ""
            page.reload()
        }
    }

    TimeDialog {
        id: timeDialog

        sioul: page.sioul
        theme: page.theme
        onSaved: page.reload()
    }
}
