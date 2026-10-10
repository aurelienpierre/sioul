// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A project: its name, and for work done for someone, who it is for, what
// an hour costs and which budget its invoices are expected in; whether AI
// agents may read it and add to it (closed until opened, docs/ai.md).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    property string projectId: ""
    property string problem: ""
    readonly property var budgets: dialog.sioul.budgets ? JSON.parse(dialog.sioul.budgets).choices || [] : []
    readonly property var statuses: ["open", "waiting", "closed"]

    signal saved(string id)
    signal removed

    // `page` is the project's page, or null for a new one.
    function edit(id, page) {
        dialog.projectId = id
        dialog.problem = ""
        title.text = page ? page.title : ""
        isProject.checked = page ? page.for_client : true
        client.text = page ? page.client : ""
        rate.text = page && page.rate > 0 ? String(page.rate).replace(".", Qt.locale(dialog.sioul.text("qt-locale")).decimalPoint) : ""
        budget.currentIndex = page ? Math.max(0, dialog.budgets.findIndex(b => b.id === page.budget) + 1) : 0
        status.currentIndex = page ? Math.max(0, dialog.statuses.indexOf(page.status)) : 0
        personal.checked = page ? page.area === "personal" : false
        openToAi.checked = page ? page.ai === true : false
        dialog.open()
        title.forceActiveFocus()
    }

    // An amount as typed, "62,50 €": its number, 0 when nothing is typed (the
    // invoices' rate then), NaN when it is no number (never a silent 0).
    function amount(text) {
        const typed = text.replace(/[\s€]/g, "").replace("−", "-").replace(",", ".")
        return typed === "" ? 0 : Number(typed)
    }

    function save() {
        const hourly = isProject.checked ? dialog.amount(rate.text) : 0
        if (Number.isNaN(hourly)) {
            dialog.problem = dialog.sioul.textWith("amount-unreadable", "text", rate.text.trim())
            return
        }
        const edit = {
            title: title.text,
            kind: isProject.checked ? "project" : "",
            status: dialog.statuses[status.currentIndex] === "open" ? "" : dialog.statuses[status.currentIndex],
            client: isProject.checked ? client.text : "",
            rate: hourly,
            budget: isProject.checked && budget.currentIndex > 0 ? dialog.budgets[budget.currentIndex - 1].id : "",
            area: personal.checked ? "personal" : "",
            ai: openToAi.checked
        }
        const answer = JSON.parse(dialog.sioul.saveProject(dialog.projectId, JSON.stringify(edit)))
        if (answer.error) {
            dialog.problem = answer.error
            return
        }
        dialog.close()
        dialog.saved(answer.id)
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(480, (parent ? parent.width : 480) - 2 * dialog.theme.gap)
    title: dialog.projectId === "" ? dialog.sioul.text("project-new") : dialog.sioul.text("project-edit")

    contentItem: GridLayout {
        columns: 2
        columnSpacing: 10
        rowSpacing: 8

        Label {
            text: dialog.sioul.text("project-field-title")
            color: dialog.theme.muted
        }
        TextField {
            id: title

            Layout.fillWidth: true
            onAccepted: dialog.save()
        }
        Item {
            Layout.preferredHeight: 1
        }
        CheckBox {
            id: isProject

            text: dialog.sioul.text("project-field-billable")
            ToolTip.visible: hovered
            ToolTip.text: dialog.sioul.text("project-field-billable-help")
            ToolTip.delay: 500
        }
        Label {
            visible: isProject.checked
            text: dialog.sioul.text("project-field-client")
            color: dialog.theme.muted
        }
        TextField {
            id: client

            visible: isProject.checked
            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("project-field-client-hint")
        }
        Label {
            visible: isProject.checked
            text: dialog.sioul.text("project-field-rate")
            color: dialog.theme.muted
        }
        TextField {
            id: rate

            visible: isProject.checked
            Layout.fillWidth: true
            placeholderText: dialog.sioul.text("project-field-rate-hint")
            inputMethodHints: Qt.ImhFormattedNumbersOnly
        }
        Label {
            visible: isProject.checked
            text: dialog.sioul.text("project-field-budget")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: budget

            visible: isProject.checked
            Layout.fillWidth: true
            model: [dialog.sioul.text("project-no-budget")].concat(dialog.budgets.map(b => dialog.theme.plain(b.title)))
        }
        Label {
            text: dialog.sioul.text("project-field-status")
            color: dialog.theme.muted
        }
        PlainComboBox {
            id: status

            Layout.fillWidth: true
            model: dialog.statuses.map(s => dialog.sioul.text("project-status-" + s))
        }
        Item {
            Layout.preferredHeight: 1
        }
        // Yours outside work: its tasks stay in view in quiet time.
        CheckBox {
            id: personal

            text: dialog.sioul.text("project-field-personal")
            ToolTip.visible: hovered
            ToolTip.text: dialog.sioul.text("project-field-personal-help")
            ToolTip.delay: 500
        }
        // Closed to AI agents until opened; what opening means, in one sentence.
        Switch {
            id: openToAi

            Layout.columnSpan: 2
            text: dialog.sioul.text("project-ai")
        }
        Label {
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.sioul.text("project-ai-help")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 12
            color: dialog.theme.muted
        }
        Label {
            visible: dialog.problem !== ""
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: dialog.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: dialog.theme.warm
        }
    }

    // The buttons inside an Item: a DialogButtonBox as the footer itself closes
    // the dialog on "Save" even when saving fails, and what went wrong is never read.
    footer: Item {
        implicitWidth: buttons.implicitWidth
        implicitHeight: buttons.implicitHeight

        DialogButtonBox {
            id: buttons

            anchors.fill: parent

            Button {
                text: dialog.sioul.text("ui-save")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: dialog.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            // Out of the list; everything gathered under it stays where it is.
            Button {
                visible: dialog.projectId !== ""
                flat: true
                text: dialog.sioul.text("project-remove")
                DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                onClicked: removeAsk.ask(title.text, dialog.sioul.text("project-remove-ask"), dialog.sioul.text("project-remove"))
            }
            onAccepted: dialog.save()
            onRejected: dialog.close()
        }
    }

    ConfirmDialog {
        id: removeAsk

        sioul: dialog.sioul
        theme: dialog.theme
        onConfirmed: {
            const problem = dialog.sioul.removeProject(dialog.projectId)
            if (problem !== "") {
                dialog.problem = problem
                return
            }
            dialog.close()
            dialog.removed()
        }
    }
}
