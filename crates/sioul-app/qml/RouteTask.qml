// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A route that makes a task of each message it takes (docs/notes-folder.md,
// "Routes that make tasks"), modal, its sections headed: Which mail (the
// project, and the route: its words in one line when it is one of the
// project's, its sender and its subject's words when it comes from a
// message), The task (its model: a name, the title's prefix with the title
// a message gets, its list, the date asked in business days, required and
// filled with 3, the first estimate), and the supplies it will use. Opened
// from a route's switch "Make a task of each message" and its Change…, and
// from a message's ⋮ ▸ "Make a task of mail like this…" (main.qml). Saved,
// when mail already here is like it: "Also for the 12 messages already
// here", once, never by itself. The projects', the lists' and the supplies'
// names and the subject are outside words, drawn plain.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: form

    required property var sioul
    required property var theme
    // The form as Rust gives it (`routetasks::form`).
    property var shown: null
    // From a message: its route is new, its words typed here; else one of the project's.
    property bool fromMail: false
    // The model's supplies, as the lines have them: [{id, quantity}].
    property var uses: []
    property string problem: ""
    // Saved: {project, route, model, already}; null before.
    property var saved: null
    // The project chosen among `choices`; the last is "A new project".
    readonly property var choices: form.shown ? form.shown.projects.concat([{ id: "", title: form.sioul.text("routetasks-project-new") }]) : []
    readonly property bool newProject: form.fromMail && projectBox.currentIndex === form.choices.length - 1
    // Saved only with a project: one chosen, or a new one's name.
    readonly property bool canSave: form.shown !== null && (!form.newProject || newProjectField.text.trim() !== "")
    // The models to start from: a new one first, then the project's.
    readonly property var modelChoices: form.shown ? [{ id: "", name: form.sioul.text("routetasks-model-new") }].concat(form.shown.models) : []

    // Something was saved: the projects' page reads its routes again.
    signal kept

    // A project's route: `route` as its row has it.
    function forRoute(project, route) {
        form.fromMail = false
        form.fill(JSON.parse(form.sioul.routeTaskForm(project, JSON.stringify(route), "") || "null"))
    }

    // A message, by its file: its sender and its subject's words between brackets.
    function forMail(key) {
        form.fromMail = true
        form.fill(JSON.parse(form.sioul.routeTaskForm("", "", key) || "null"))
    }

    function fill(got) {
        form.shown = got
        form.saved = null
        form.problem = got && got.error ? got.error : ""
        if (!got) {
            return
        }
        const at = form.choices.findIndex(p => p.id === got.project)
        projectBox.currentIndex = at >= 0 ? at : (got.projects.length > 0 ? 0 : form.choices.length - 1)
        newProjectField.text = ""
        fromField.text = (got.route.from_addresses || []).concat(got.route.from_domains || []).join(", ")
        subjectField.text = (got.route.subject_contains || []).join(", ")
        form.load(got.model)
        modelBox.currentIndex = Math.max(0, form.modelChoices.findIndex(m => m.id !== "" && m.id === got.model.id))
        form.open()
    }

    // A model's fields in the form.
    function load(model) {
        nameField.text = model.name || ""
        prefixField.text = model.prefix || ""
        listBox.currentIndex = Math.max(0, form.shown.lists.findIndex(l => l.id === (model.list || "")))
        daysBox.value = model.business_days
        estimateBox.value = model.estimate || 0
        form.uses = (model.supplies || []).map(u => ({ id: u.id, quantity: u.quantity }))
    }

    // Comma-separated words, as the routes' editor reads them.
    function words(text) {
        return text.split(",").map(t => t.trim()).filter(t => t !== "")
    }

    // The title a message like this gets, as the route would make it.
    readonly property string titleShown: form.shown && form.shown.subject !== "" ? form.sioul.routeTaskTitle(form.shown.subject, JSON.stringify(form.fromMail ? form.words(subjectField.text) : (form.shown.route.subject_contains || [])), prefixField.text) : ""

    function save() {
        const typed = form.words(fromField.text)
        // From a message, its words as typed (the project's route of the same words, by its id, when it has one).
        const route = form.fromMail ? { id: form.shown.route.id || "", from_addresses: typed.filter(w => w.indexOf("@") > 0), from_domains: typed.filter(w => w.indexOf("@") <= 0), subject_contains: form.words(subjectField.text) } : form.shown.route
        const chosen = form.modelChoices[modelBox.currentIndex] || { id: "" }
        const sent = {
            project: form.newProject ? "" : form.choices[projectBox.currentIndex].id,
            new_project: form.newProject ? newProjectField.text : "",
            route: route,
            // The model chosen ("A new model": one of its own), changed as the fields say.
            model: { id: chosen.id || "", name: nameField.text, prefix: prefixField.text, list: form.shown.lists[listBox.currentIndex].id, business_days: daysBox.value, estimate: estimateBox.value, supplies: form.uses }
        }
        const answer = JSON.parse(form.sioul.saveRouteTask(JSON.stringify(sent)) || "{}")
        if (answer.error) {
            form.problem = answer.error
            return
        }
        form.problem = ""
        form.kept()
        if (answer.already > 0)
            form.saved = answer
        else
            form.close()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, (parent ? form.roomWidth : 560) - 2 * form.theme.gap)
    height: Math.min(implicitHeight, (parent ? form.roomHeight : 800) - 2 * form.theme.gap)
    title: form.sioul.text("routetasks-title")

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 14

            // Saved, mail like it already here: made only when asked, once.
            ColumnLayout {
                visible: form.saved !== null
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: form.sioul.text("routetasks-saved")
                    wrapMode: Text.Wrap
                    color: form.theme.text
                }
                Button {
                    objectName: "routeTaskAlso"
                    text: form.saved ? form.sioul.textCounted("routetasks-also", form.saved.already) : ""
                    onClicked: {
                        form.problem = form.sioul.routeTaskAlso(form.saved.project, form.saved.route)
                        if (form.problem === "")
                            form.close()
                    }
                }
            }

            // Which mail: the project, and the route.
            Section {
                visible: form.saved === null
                theme: form.theme
                title: form.sioul.text("routetasks-which")
                Layout.fillWidth: true

                GridLayout {
                    Layout.fillWidth: true
                    columns: form.width < 420 ? 1 : 2
                    columnSpacing: 10
                    rowSpacing: 6

                    Label {
                        text: form.sioul.text("routetasks-project")
                        color: form.theme.muted
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        PlainComboBox {
                            id: projectBox

                            objectName: "routeTaskProject"
                            visible: form.fromMail
                            Layout.fillWidth: true
                            model: form.choices.map(p => form.theme.plain(p.title))
                        }
                        TextField {
                            id: newProjectField

                            objectName: "routeTaskNewProject"
                            visible: form.newProject
                            Layout.fillWidth: true
                            placeholderText: form.sioul.text("routetasks-project-new-hint")
                        }
                        Label {
                            visible: !form.fromMail
                            Layout.fillWidth: true
                            text: form.shown ? form.theme.plain((form.shown.projects.find(p => p.id === form.shown.project) || { title: form.shown.project }).title) : ""
                            wrapMode: Text.Wrap
                            color: form.theme.text
                        }
                    }
                    Label {
                        text: form.sioul.text("set-route")
                        color: form.theme.muted
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        Label {
                            visible: !form.fromMail
                            Layout.fillWidth: true
                            text: form.shown ? form.shown.route_line : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: form.theme.text
                        }
                        Label {
                            visible: form.fromMail
                            text: form.sioul.text("routetasks-from")
                            font.pixelSize: 12
                            color: form.theme.muted
                        }
                        TextField {
                            id: fromField

                            objectName: "routeTaskFrom"
                            visible: form.fromMail
                            Layout.fillWidth: true
                            placeholderText: form.sioul.text("set-route-comma")
                        }
                        Label {
                            visible: form.fromMail
                            text: form.sioul.text("set-route-subject")
                            font.pixelSize: 12
                            color: form.theme.muted
                        }
                        TextField {
                            id: subjectField

                            objectName: "routeTaskSubject"
                            visible: form.fromMail
                            Layout.fillWidth: true
                            placeholderText: form.sioul.text("set-route-comma")
                        }
                    }
                }
                Label {
                    Layout.fillWidth: true
                    text: form.sioul.text("routetasks-gate")
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: form.theme.muted
                }
            }

            // The task: its model.
            Section {
                visible: form.saved === null
                theme: form.theme
                title: form.sioul.text("routetasks-task")
                Layout.fillWidth: true

                GridLayout {
                    Layout.fillWidth: true
                    columns: form.width < 420 ? 1 : 2
                    columnSpacing: 10
                    rowSpacing: 6

                    Label {
                        visible: form.modelChoices.length > 1
                        text: form.sioul.text("routetasks-model")
                        color: form.theme.muted
                    }
                    PlainComboBox {
                        id: modelBox

                        objectName: "routeTaskModel"
                        visible: form.modelChoices.length > 1
                        Layout.fillWidth: true
                        model: form.modelChoices.map(m => form.theme.plain(m.name || m.id))
                        onActivated: index => {
                            if (index > 0)
                                form.load(form.modelChoices[index])
                        }
                    }
                    Label {
                        text: form.sioul.text("routetasks-name")
                        color: form.theme.muted
                    }
                    TextField {
                        id: nameField

                        objectName: "routeTaskName"
                        Layout.fillWidth: true
                        placeholderText: form.sioul.text("routetasks-name-hint")
                    }
                    Label {
                        text: form.sioul.text("routetasks-prefix")
                        color: form.theme.muted
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        TextField {
                            id: prefixField

                            objectName: "routeTaskPrefix"
                            Layout.fillWidth: true
                            placeholderText: form.sioul.text("routetasks-prefix-hint")
                        }
                        Label {
                            visible: form.titleShown !== ""
                            Layout.fillWidth: true
                            text: form.sioul.textWith("routetasks-title-shown", "title", form.titleShown)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: form.theme.muted
                        }
                    }
                    Label {
                        text: form.sioul.text("routetasks-list")
                        color: form.theme.muted
                    }
                    PlainComboBox {
                        id: listBox

                        objectName: "routeTaskList"
                        Layout.fillWidth: true
                        model: form.shown ? form.shown.lists.map(l => form.theme.plain(l.title)) : []
                    }
                    Label {
                        text: form.sioul.text("routetasks-asked")
                        color: form.theme.muted
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2

                        RowLayout {
                            spacing: 8

                            SpinBox {
                                id: daysBox

                                objectName: "routeTaskDays"
                                from: 0
                                to: 60
                                editable: true
                                Accessible.name: form.sioul.text("routetasks-asked")
                            }
                            Label {
                                Layout.fillWidth: true
                                text: form.sioul.textCounted("routetasks-days-after", daysBox.value)
                                wrapMode: Text.Wrap
                                color: form.theme.text
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: form.sioul.text("routetasks-days-help")
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: form.theme.muted
                        }
                    }
                    Label {
                        text: form.sioul.text("routetasks-estimate")
                        color: form.theme.muted
                    }
                    RowLayout {
                        spacing: 8

                        SpinBox {
                            id: estimateBox

                            objectName: "routeTaskEstimate"
                            from: 0
                            to: 600
                            stepSize: 5
                            editable: true
                            Accessible.name: form.sioul.text("routetasks-estimate")
                        }
                        Label {
                            text: form.sioul.text("routetasks-minutes")
                            color: form.theme.text
                        }
                    }
                }
            }

            // What each task will use: the supply lines of a task's form.
            Section {
                visible: form.saved === null && form.shown !== null && (form.shown.keeps || form.uses.length > 0)
                theme: form.theme
                title: form.sioul.text("supplies-section")
                Layout.fillWidth: true

                SupplyLines {
                    Layout.fillWidth: true
                    sioul: form.sioul
                    theme: form.theme
                    choices: form.shown ? form.shown.supplies : []
                    value: form.uses
                    onEdited: value => form.uses = value
                }
            }
            Label {
                visible: form.problem !== ""
                Layout.fillWidth: true
                text: form.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: form.theme.warm
            }
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
                objectName: "routeTaskSave"
                visible: form.saved === null
                text: form.sioul.text("ui-save")
                highlighted: true
                enabled: form.canSave
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: form.saved !== null ? form.sioul.text("routetasks-not-now") : form.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onAccepted: form.save()
            onRejected: form.close()
        }
    }
}
