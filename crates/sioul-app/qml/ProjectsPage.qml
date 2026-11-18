// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Projects, the umbrella over everything else. On the left, each
// with its open tasks and the time left to bill, by Mail's filter by area
// (AreaFilter.qml): work's, admin's or leisure's as the hours say on this
// device, any of them one tick away (docs/areas.md); on the right, the one open:
// what is coming, then what happened, on one line of time; its tasks one
// click away as a board, a list or a calendar; its time and its invoices;
// its client's profile (ClientProfileDialog.qml). "Make the invoice" opens
// the draft of its unbilled time in Money ▸ Invoices' editor.

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

    // Every project, in the list's order, each with whether the filter shows it (`shown`).
    property var rows: []
    readonly property var listed: page.rows.filter(r => r.shown === true)
    // The filter by area as Rust says it (`areafilter::Shown`); kept on this device
    // with Time's, in projects-view.toml.
    property var filter: null
    // The mail that belongs to the one open: its routes, folded until asked.
    property bool routesShown: false
    property var routes: []
    property string openId: ""
    // On a phone, the project open takes the page; Back closes it (main.qml).
    readonly property bool canGoBack: page.openId !== ""
    function back() {
        page.openId = ""
    }
    property var shown: null
    property string problem: ""
    // The client's profile of the project open, for work for a client: {exists, summary, …}; null for none.
    property var profile: null

    function reload() {
        const got = JSON.parse(page.sioul.projectsPage() || "{}")
        page.rows = got.rows || []
        page.filter = got.filter || null
        page.shown = page.openId !== "" ? JSON.parse(page.sioul.projectPage(page.openId) || "null") : null
        page.routes = page.openId !== "" ? JSON.parse(page.sioul.settings("project:" + page.openId) || "[]") : []
        page.profile = page.shown !== null && page.shown.for_client && page.shown.client !== "" ? JSON.parse(page.sioul.clientProfile(page.shown.client) || "null") : null
    }

    // A setting of the project's own (its routes, whether AI agents may read it), saved at once.
    // `shown`: the routes the row showed: the change is set over the file as it is now.
    function saveRoutes(key, value, shown) {
        page.problem = page.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
        page.reload()
    }

    function open(id) {
        page.openId = id
        page.problem = ""
        page.reload()
    }

    // A new project: its form.
    function startNew() {
        projectDialog.now().edit("", null)
    }

    // The open project's form (its Edit button); returns the form.
    function editOpen() {
        const form = projectDialog.now()
        form.edit(page.openId, page.shown)
        return form
    }

    // The first the list shows, else the first of all (the window's pictures).
    function openFirst() {
        const first = page.listed.length > 0 ? page.listed[0] : page.rows[0]
        if (first)
            page.open(first.id)
    }

    // An amount as the core says amounts: digits in your language's way, the
    // currency where it puts it ("€62.50", "62,50 €"), cents only when there are some.
    function money(value) {
        const digits = Number(value).toLocaleString(page.window.sioulLocale, "f", Number.isInteger(value) ? 0 : 2)
        return page.sioul.textWith("money-positive", "value", digits)
    }

    // The draft of the hours not billed yet (the project's draft, the time
    // since added, never time a draft holds or you took out), in the editor;
    // what was added said in the status line.
    function makeInvoice() {
        const made = JSON.parse(page.sioul.makeInvoice(page.openId) || "{}")
        if (made.error) {
            page.problem = made.error
            return
        }
        page.window.openInvoiceDraft(made.draft)
        if (made.said)
            page.sioul.status = made.said
    }

    // An issued invoice's file comes as it was kept, copied by the core: never
    // printed again. Only an invoice from before November 2026, which kept
    // none, is printed from its page, as it always was.
    function printInvoice(made) {
        const failed = made.html ? pdf.write(made.html, made.pdf) : ""
        if (failed !== "") {
            page.problem = page.sioul.textWith("invoice-not-written", "path", failed)
            return
        }
        // Where it is, and what the issue found to say (the checks' notes, a
        // copy that could not be written): the status line.
        const said = []
        if (made.pdf)
            said.push(page.sioul.textWith("invoice-written", "path", made.pdf))
        if (made.said)
            said.push(made.said)
        page.sioul.status = said.join(" ")
        // Shown in your PDF reader, except while the window takes its own pictures.
        if (made.pdf && page.sioul.grabFolder() === "")
            Qt.openUrlExternally(page.theme.fileUrl(made.pdf))
    }

    onVisibleChanged: {
        if (page.visible)
            page.reload()
    }
    Component.onCompleted: page.reload()

    // The time changed (work's hours over, a meal, the night): the filter's ticks follow.
    Connections {
        target: page.window

        function onMomentChanged() {
            if (page.visible)
                page.reload()
        }
    }

    Connections {
        target: page.sioul

        function onLinksChanged() {
            page.reload()
        }

        function onTasksChanged() {
            if (page.visible)
                page.reload()
        }

        // A project made, changed or gone, here or elsewhere.
        function onProjectsChanged() {
            if (page.visible)
                page.reload()
        }

        // Its drafts and invoices, here or elsewhere.
        function onInvoicesChanged() {
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

        // Every project; on a phone, the whole page until one is open.
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
                    onClicked: projectDialog.now().edit("", null)
                }
            }
            Label {
                visible: page.rows.length === 0
                Layout.fillWidth: true
                text: page.sioul.text("project-none")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            // What shows: work's, admin's, leisure's projects, as the hours say until you tick
            // others; nothing else explains it (docs/areas.md, "A page's filter by area").
            AreaFilter {
                objectName: "projectAreas"
                visible: page.rows.length > 0 && page.filter !== null
                sioul: page.sioul
                theme: page.theme
                shown: page.filter
                help: page.sioul.text("projects-filter-help")
                onTicked: picked => {
                    page.sioul.tickProjectAreas(picked)
                    page.reload()
                }
                onAsHours: {
                    page.sioul.projectAreasAsHours()
                    page.reload()
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
                            iconName: row.modelData.for_client ? "view-financial-account" : "folder-documents"
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
                            text: page.shown ? [page.shown.for_client ? page.sioul.text("project-kind-project") : page.sioul.text("project-kind-own"), page.shown.client, page.shown.for_client && page.shown.rate > 0 ? page.sioul.textWith("project-rate", "rate", page.money(page.shown.rate)) : "", page.shown.status !== "" ? page.sioul.text("project-status-" + page.shown.status) : ""].filter(t => t !== "").join("  ·  ") : ""
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
                        onClicked: page.editOpen()
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
                        onClicked: timeDialog.now().begin(page.openId)
                    }
                    Button {
                        visible: page.shown !== null && page.shown.for_client && page.shown.unbilled !== ""
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
                // Closed to AI agents until opened (docs/ai.md): the switch, and what it means in a sentence.
                Switch {
                    id: openToAi

                    visible: page.shown !== null
                    text: page.sioul.text("project-ai")
                    checked: page.shown !== null && page.shown.ai === true
                    onToggled: {
                        page.saveRoutes("project." + page.openId + ".ai", openToAi.checked)
                        // As the file says now, for this project and the next one opened.
                        openToAi.checked = Qt.binding(() => page.shown !== null && page.shown.ai === true)
                    }
                }
                Label {
                    visible: page.shown !== null
                    Layout.fillWidth: true
                    text: page.sioul.text("project-ai-help")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: page.theme.muted
                }
                Label {
                    visible: page.problem !== ""
                    Layout.fillWidth: true
                    text: page.problem
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.warm
                }
                // Its client's profile: what its invoices print, made or changed here.
                RowLayout {
                    visible: page.profile !== null
                    Layout.fillWidth: true
                    spacing: 8

                    Label {
                        objectName: "projectClientProfile"
                        Layout.preferredWidth: 80
                        Layout.fillWidth: true
                        text: page.profile ? (page.profile.exists ? page.sioul.textWith("clients-profile-line", "profile", page.profile.summary) : page.sioul.text("clients-profile-none")) : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: page.theme.muted
                    }
                    Button {
                        flat: true
                        text: page.sioul.text(page.profile && page.profile.exists ? "clients-profile-change" : "clients-profile-make")
                        onClicked: profileForm.now().edit(page.shown.client)
                    }
                }

                ThingActions {
                    visible: page.shown !== null
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    source: page.shown ? { uri: "sioul:project/" + encodeURIComponent(page.openId), kind: "project", key: page.openId, title: page.shown.title } : null
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
                        onSave: (key, value, shown) => page.saveRoutes(key, value, shown)
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

                        // Opened in Money ▸ Invoices: its payments, Paid…, Credit note…, Write the mail.
                        ItemDelegate {
                            Layout.preferredWidth: 80
                            Layout.fillWidth: true
                            padding: 2
                            onClicked: page.window.openInvoice(bill.modelData.number)

                            contentItem: Label {
                                // A credit note says what it credits, and what it has to pay back.
                                text: [bill.modelData.number, bill.modelData.date, bill.modelData.total, bill.modelData.note || ""].filter(t => t !== "").join("  ·  ")
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: page.theme.text
                            }
                        }
                        CheckBox {
                            // A credit note's box says "Paid back", while it has money to pay back.
                            visible: bill.modelData.kind !== "credit-note" || bill.modelData.pay_back
                            text: page.sioul.text(bill.modelData.kind === "credit-note" ? "invoice-paid-back" : "invoice-paid")
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

    Later {
        id: projectDialog

        sourceComponent: Component {
            ProjectDialog {
                id: projectDialogForm

                sioul: page.sioul
                theme: page.theme
                onSaved: id => page.open(id)
                onRemoved: {
                    page.openId = ""
                    page.reload()
                }
            }
        }
    }

    Later {
        id: profileForm

        sourceComponent: Component {
            ClientProfileDialog {
                sioul: page.sioul
                theme: page.theme
                onSaved: page.reload()
            }
        }
    }

    Later {
        id: timeDialog

        sourceComponent: Component {
            TimeDialog {
                id: timeDialogForm

                sioul: page.sioul
                theme: page.theme
                onSaved: page.reload()
            }
        }
    }
}
