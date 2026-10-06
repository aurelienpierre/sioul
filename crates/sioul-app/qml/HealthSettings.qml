// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Health page's settings (its ⚙), set once or rarely, made the first
// time they open: the medicines and when they are taken, the prescriptions
// and where their errands go, the usual meals, naps and night, the watch's
// folder and offers, the pause to move, the limit on chats. A day that
// differs is changed on the page itself, that day only. On the right of the
// window, all of a phone's; Escape or a click outside closes it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: panel

    required property var sioul
    required property var theme
    readonly property var dateLocale: Qt.locale(panel.sioul.text("qt-locale"))
    // As the backend gives it (`health::settings_view`).
    property var shown: ({ medicines: [], prescriptions: [], movement: { enabled: true, minutes: 45 }, chats: { enabled: false, minutes: 60, locked_minutes: 30 }, errands_list: "", lists: [], watch: null })

    // Something changed that the page shows (a medicine, the usual meals): it reads its days again.
    signal changed

    function reload() {
        panel.shown = JSON.parse(panel.sioul.healthSettings())
        needs.reload()
    }

    // A form saved or a medicine taken out.
    function saved() {
        panel.reload()
        panel.changed()
    }

    function setting(key, value) {
        const problem = panel.sioul.setHealth(key, String(value))
        if (problem !== "")
            panel.sioul.status = problem
        panel.saved()
    }

    // Opened where the usual meals, naps and night are set (the Porch's "Set
    // my night", Settings ▸ Hours): scrolled there once open, and kept there a
    // moment while the rows above it are laid out, then left to the hand.
    property bool toNeeds: false

    function showNeeds() {
        panel.toNeeds = true
        if (panel.opened)
            panel.scrollToNeeds()
        else
            panel.open()
    }

    function scrollToNeeds() {
        const flick = scroll.contentItem as Flickable
        if (flick)
            flick.contentY = Math.max(0, Math.min(needs.y - 8, column.implicitHeight - flick.height))
        needsSettled.restart()
    }

    onOpened: if (panel.toNeeds) panel.scrollToNeeds()

    Connections {
        target: needs

        function onYChanged() {
            if (panel.toNeeds && panel.opened)
                panel.scrollToNeeds()
        }
    }

    Timer {
        id: needsSettled

        interval: 400
        onTriggered: panel.toNeeds = false
    }

    parent: Overlay.overlay
    x: parent ? parent.width - width : 0
    y: 0
    // Half the window, at most 560 pixels; all of a phone's.
    width: !parent ? 560 : parent.width < 720 ? parent.width : Math.min(560, parent.width * 0.5)
    height: parent ? parent.height : 600
    padding: panel.theme.gap
    modal: true
    dim: false
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    onAboutToShow: panel.reload()

    background: Rectangle {
        color: panel.theme.surface
        border.color: panel.theme.line

        Rectangle {
            width: 1
            height: parent.height
            color: panel.theme.line
        }
    }

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            id: column

            width: scroll.availableWidth
            spacing: 8

            RowLayout {
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: panel.sioul.text("ui-settings")
                    font.pixelSize: 18
                    color: panel.theme.text
                }
                ToolButton {
                    text: "×"
                    Accessible.name: panel.sioul.text("ui-close")
                    onClicked: panel.close()
                }
            }
            Label {
                Layout.fillWidth: true
                text: panel.sioul.text("health-local")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }

            // Medicines.
            RowLayout {
                Layout.topMargin: 10
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: panel.sioul.text("health-medicines")
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    color: panel.theme.accent
                }
                Button {
                    flat: true
                    text: panel.sioul.text("health-add-medicine")
                    icon.name: "list-add"
                    icon.color: panel.theme.text
                    onClicked: medicineDialog.now().edit(null)
                }
            }
            Repeater {
                model: panel.shown.medicines

                delegate: RowLayout {
                    id: medicine

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 10

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: medicine.modelData.name + (medicine.modelData.dose ? "  ·  " + medicine.modelData.dose : "") + (medicine.modelData.paused ? "  ·  " + panel.sioul.text("health-paused") : "")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: medicine.modelData.paused ? panel.theme.muted : panel.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: [medicine.modelData.when, medicine.modelData.prescription_title].filter(t => t !== "").join("  ·  ")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: panel.theme.muted
                        }
                    }
                    Button {
                        flat: true
                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                        text: panel.sioul.text("ui-edit")
                        onClicked: medicineDialog.now().edit(medicine.modelData)
                    }
                }
            }

            // Prescriptions.
            RowLayout {
                Layout.topMargin: 10
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: panel.sioul.text("health-prescriptions")
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                    color: panel.theme.accent
                }
                Button {
                    flat: true
                    text: panel.sioul.text("health-add-prescription")
                    icon.name: "list-add"
                    icon.color: panel.theme.text
                    onClicked: prescriptionDialog.now().edit(null)
                }
            }
            Repeater {
                model: panel.shown.prescriptions

                delegate: RowLayout {
                    id: prescription

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 10

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: prescription.modelData.title + (prescription.modelData.prescriber ? "  ·  " + prescription.modelData.prescriber : "")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: panel.theme.text
                        }
                        Label {
                            visible: text !== ""
                            Layout.fillWidth: true
                            text: prescription.modelData.next.join("  ·  ")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: panel.theme.muted
                        }
                    }
                    // Fetched at the pharmacy: the next time is counted from today.
                    // The buttons as wide as their names: more room for the dates on a phone.
                    Button {
                        visible: prescription.modelData.refill_days !== undefined
                        flat: true
                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                        text: panel.sioul.text("health-refilled")
                        onClicked: {
                            const problem = panel.sioul.refilled(prescription.modelData.id)
                            if (problem !== "")
                                panel.sioul.status = problem
                            panel.saved()
                        }
                    }
                    Button {
                        flat: true
                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                        text: panel.sioul.text("ui-edit")
                        onClicked: prescriptionDialog.now().edit(prescription.modelData)
                    }
                }
            }
            // Where the pharmacy and the renewals go: a list your phone has.
            RowLayout {
                visible: panel.shown.lists.length > 0
                Layout.fillWidth: true
                spacing: 8

                Label {
                    text: panel.sioul.text("health-errands-list")
                    color: panel.theme.muted
                }
                ComboBox {
                    readonly property var lists: panel.shown.lists

                    Layout.fillWidth: true
                    model: lists.map(l => panel.theme.plain(l.name))
                    currentIndex: Math.max(0, lists.findIndex(l => l.id === panel.shown.errands_list))
                    onActivated: index => panel.setting("errands_list", lists[index].id)
                }
            }

            // The usual meals, naps and night.
            NeedsSection {
                id: needs

                Layout.fillWidth: true
                Layout.topMargin: 6
                sioul: panel.sioul
                theme: panel.theme
                onChanged: panel.changed()
            }

            // The watch: where its files come from, whether it may offer a pause.
            WatchPanel {
                Layout.fillWidth: true
                Layout.topMargin: 10
                part: "settings"
                sioul: panel.sioul
                theme: panel.theme
                watch: panel.shown.watch
                onSetting: (key, value) => panel.setting(key, value)
            }

            // A pause to move, while focusing.
            Label {
                Layout.topMargin: 12
                text: panel.sioul.text("health-moving")
                font.pixelSize: 16
                font.weight: Font.DemiBold
                color: panel.theme.accent
            }
            // The minutes under the sentence when the screen is narrow (a phone).
            Flow {
                Layout.fillWidth: true
                spacing: 8

                CheckBox {
                    text: panel.sioul.text("health-moving-every")
                    checked: panel.shown.movement.enabled
                    onToggled: panel.setting("movement.enabled", checked)
                }
                SpinBox {
                    id: movingMinutes

                    from: 10
                    to: 240
                    stepSize: 5
                    editable: true
                    value: panel.shown.movement.minutes
                    enabled: panel.shown.movement.enabled
                    Accessible.name: panel.sioul.text("health-moving-every")
                    onValueModified: panel.setting("movement.minutes", value)
                }
                Label {
                    height: movingMinutes.height
                    verticalAlignment: Text.AlignVCenter
                    text: panel.sioul.text("health-minutes")
                    color: panel.theme.muted
                }
            }
            Label {
                Layout.fillWidth: true
                text: panel.sioul.text("health-moving-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }

            // Chats, a limit a day.
            Label {
                Layout.topMargin: 12
                text: panel.sioul.text("health-chats")
                font.pixelSize: 16
                font.weight: Font.DemiBold
                color: panel.theme.accent
            }
            CheckBox {
                text: panel.sioul.text("health-chats-limit")
                checked: panel.shown.chats.enabled
                onToggled: panel.setting("chats.enabled", checked)
            }
            GridLayout {
                enabled: panel.shown.chats.enabled
                columns: 3
                columnSpacing: 8
                rowSpacing: 4

                Label {
                    text: panel.sioul.text("health-chats-after")
                    color: panel.theme.muted
                }
                SpinBox {
                    from: 5
                    to: 600
                    stepSize: 5
                    editable: true
                    value: panel.shown.chats.minutes || 60
                    onValueModified: panel.setting("chats.minutes", value)
                }
                Label {
                    text: panel.sioul.text("health-minutes-a-day")
                    color: panel.theme.muted
                }
                Label {
                    text: panel.sioul.text("health-chats-for")
                    color: panel.theme.muted
                }
                SpinBox {
                    from: 5
                    to: 600
                    stepSize: 5
                    editable: true
                    value: panel.shown.chats.locked_minutes || 30
                    onValueModified: panel.setting("chats.locked_minutes", value)
                }
                Label {
                    text: panel.sioul.text("health-minutes")
                    color: panel.theme.muted
                }
            }
            Label {
                Layout.fillWidth: true
                text: panel.sioul.text("health-chats-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            Item {
                Layout.preferredHeight: panel.theme.gap
            }

            // The forms, made the first time they open (out of the layout: Later is hidden).
            // A medicine: its name, its dose, when it is taken, until when, and its prescription.
            Later {
                id: medicineDialog

                sourceComponent: Component {
                    Dialog {
                        id: medicineDialogForm

                        property string medicineId: ""
                        property string problem: ""
                        readonly property var everies: ["day", "days", "hours"]

                        function edit(medicine) {
                            medicineDialogForm.medicineId = medicine ? medicine.id : ""
                            medicineDialogForm.problem = ""
                            medicineName.text = medicine ? medicine.name : ""
                            medicineDose.text = medicine ? medicine.dose || "" : ""
                            const schedule = medicine ? medicine.schedule : { every: "day", times: ["08:00"] }
                            every.currentIndex = Math.max(0, medicineDialogForm.everies.indexOf(schedule.every))
                            times.text = schedule.every === "day" ? schedule.times.join(", ") : "08:00"
                            everyDays.value = schedule.every === "days" ? schedule.days : 2
                            everyHours.value = schedule.every === "hours" ? schedule.hours : 6
                            dayTime.text = schedule.every === "days" ? schedule.time : "08:00"
                            // Today on this computer's clock: toISOString would give the day in UTC, yesterday after midnight.
                            from.date = schedule.every === "days" ? schedule.from : Qt.formatDate(new Date(), "yyyy-MM-dd")
                            const start = schedule.every === "hours" ? new Date(schedule.from * 1000) : new Date()
                            hourFrom.text = start.toTimeString().slice(0, 5)
                            until.date = medicine && medicine.until ? medicine.until : ""
                            const prescriptions = panel.shown.prescriptions
                            link.currentIndex = medicine && medicine.prescription ? Math.max(0, prescriptions.findIndex(p => p.id === medicine.prescription) + 1) : 0
                            paused.checked = medicine ? medicine.paused === true : false
                            medicineDialogForm.open()
                            medicineName.forceActiveFocus()
                        }

                        function save() {
                            const kind = medicineDialogForm.everies[every.currentIndex]
                            const today = Qt.formatDate(new Date(), "yyyy-MM-dd")
                            const edit = {
                                name: medicineName.text,
                                dose: medicineDose.text,
                                every: kind,
                                times: times.text.split(",").map(t => t.trim()).filter(t => t !== ""),
                                days: everyDays.value,
                                hours: everyHours.value,
                                time: dayTime.text,
                                from: kind === "hours" ? today + "T" + hourFrom.text : from.date,
                                until: until.date,
                                prescription: link.currentIndex > 0 ? panel.shown.prescriptions[link.currentIndex - 1].id : "",
                                paused: paused.checked
                            }
                            const answer = JSON.parse(panel.sioul.saveMedicine(medicineDialogForm.medicineId, JSON.stringify(edit)))
                            if (answer.error) {
                                medicineDialogForm.problem = answer.error
                                return
                            }
                            medicineDialogForm.close()
                            panel.saved()
                        }

                        parent: Overlay.overlay
                        anchors.centerIn: parent
                        modal: true
                        width: Math.min(520, (parent ? parent.width : 520) - 2 * panel.theme.gap)
                        title: medicineDialogForm.medicineId === "" ? panel.sioul.text("health-add-medicine") : panel.sioul.text("health-medicine")

                        contentItem: GridLayout {
                            columns: 2
                            columnSpacing: 10
                            rowSpacing: 8

                            Label {
                                text: panel.sioul.text("health-field-name")
                                color: panel.theme.muted
                            }
                            TextField {
                                id: medicineName

                                Layout.fillWidth: true
                            }
                            Label {
                                text: panel.sioul.text("health-field-dose")
                                color: panel.theme.muted
                            }
                            TextField {
                                id: medicineDose

                                Layout.fillWidth: true
                                placeholderText: panel.sioul.text("health-field-dose-hint")
                            }
                            Label {
                                text: panel.sioul.text("health-field-when")
                                color: panel.theme.muted
                            }
                            ComboBox {
                                id: every

                                Layout.fillWidth: true
                                model: medicineDialogForm.everies.map(e => panel.sioul.text("health-every-" + e + "-choice"))
                            }
                            Label {
                                visible: every.currentIndex === 0
                                text: panel.sioul.text("health-field-times")
                                color: panel.theme.muted
                            }
                            TextField {
                                id: times

                                visible: every.currentIndex === 0
                                Layout.fillWidth: true
                                placeholderText: "12:00, 18:00"
                            }
                            Label {
                                visible: every.currentIndex === 1
                                text: panel.sioul.text("health-field-every-days")
                                color: panel.theme.muted
                            }
                            RowLayout {
                                visible: every.currentIndex === 1
                                spacing: 6

                                SpinBox {
                                    id: everyDays

                                    from: 1
                                    to: 90
                                    editable: true
                                }
                                Label {
                                    text: panel.sioul.text("health-field-at")
                                    color: panel.theme.muted
                                }
                                TextField {
                                    id: dayTime

                                    Layout.preferredWidth: 70
                                    inputMask: "99:99"
                                }
                            }
                            Label {
                                visible: every.currentIndex === 1
                                text: panel.sioul.text("health-field-from")
                                color: panel.theme.muted
                            }
                            DateField {
                                id: from

                                visible: every.currentIndex === 1
                                theme: panel.theme
                                locale: panel.dateLocale
                            }
                            Label {
                                visible: every.currentIndex === 2
                                text: panel.sioul.text("health-field-every-hours")
                                color: panel.theme.muted
                            }
                            RowLayout {
                                visible: every.currentIndex === 2
                                spacing: 6

                                SpinBox {
                                    id: everyHours

                                    from: 1
                                    to: 48
                                    editable: true
                                }
                                Label {
                                    text: panel.sioul.text("health-field-from-time")
                                    color: panel.theme.muted
                                }
                                TextField {
                                    id: hourFrom

                                    Layout.preferredWidth: 70
                                    inputMask: "99:99"
                                }
                            }

                            Label {
                                text: panel.sioul.text("health-field-until")
                                color: panel.theme.muted
                            }
                            DateField {
                                id: until

                                theme: panel.theme
                                locale: panel.dateLocale
                            }
                            Label {
                                text: panel.sioul.text("health-field-prescription")
                                color: panel.theme.muted
                            }
                            ComboBox {
                                id: link

                                Layout.fillWidth: true
                                model: [panel.sioul.text("health-no-prescription")].concat(panel.shown.prescriptions.map(p => panel.theme.plain(p.title)))
                            }
                            Item {
                                Layout.preferredHeight: 1
                            }
                            CheckBox {
                                id: paused

                                visible: medicineDialogForm.medicineId !== ""
                                text: panel.sioul.text("health-pause")
                            }
                            Label {
                                visible: medicineDialogForm.problem !== ""
                                Layout.columnSpan: 2
                                Layout.fillWidth: true
                                text: medicineDialogForm.problem
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: panel.theme.warm
                            }
                        }

                        // The buttons inside an Item: a DialogButtonBox as the footer itself closes
                        // the dialog on "Save" even when saving fails, and what went wrong is never read.
                        footer: Item {
                            implicitWidth: medicineButtons.implicitWidth
                            implicitHeight: medicineButtons.implicitHeight

                            DialogButtonBox {
                                id: medicineButtons

                                anchors.fill: parent

                                Button {
                                    text: panel.sioul.text("ui-save")
                                    highlighted: true
                                    DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                                }
                                Button {
                                    text: panel.sioul.text("ui-cancel")
                                    DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                                }
                                Button {
                                    visible: medicineDialogForm.medicineId !== ""
                                    flat: true
                                    text: panel.sioul.text("health-remove")
                                    DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                                    onClicked: removeAsk.now().askFor(medicineName.text, panel.sioul.text("health-remove-medicine-ask"), panel.sioul.text("health-remove"), medicineDialogForm.medicineId, medicineDialogForm)
                                }
                                onAccepted: medicineDialogForm.save()
                                onRejected: medicineDialogForm.close()
                            }
                        }
                    }
                }
            }

            // A prescription: what, who, until when, how often at the pharmacy.
            Later {
                id: prescriptionDialog

                sourceComponent: Component {
                    Dialog {
                        id: prescriptionDialogForm

                        property string prescriptionId: ""
                        property string problem: ""

                        function edit(prescription) {
                            prescriptionDialogForm.prescriptionId = prescription ? prescription.id : ""
                            prescriptionDialogForm.problem = ""
                            what.text = prescription ? prescription.title : ""
                            prescriber.text = prescription ? prescription.prescriber || "" : ""
                            validUntil.date = prescription && prescription.until ? prescription.until : ""
                            refillDays.value = prescription && prescription.refill_days ? prescription.refill_days : 0
                            lastRefill.date = prescription && prescription.last_refill ? prescription.last_refill : ""
                            prescriptionNote.text = prescription ? prescription.note || "" : ""
                            prescriptionDialogForm.open()
                            what.forceActiveFocus()
                        }

                        function save() {
                            const edit = { title: what.text, prescriber: prescriber.text, until: validUntil.date, refill_days: refillDays.value, last_refill: lastRefill.date, note: prescriptionNote.text }
                            const answer = JSON.parse(panel.sioul.savePrescription(prescriptionDialogForm.prescriptionId, JSON.stringify(edit)))
                            if (answer.error) {
                                prescriptionDialogForm.problem = answer.error
                                return
                            }
                            prescriptionDialogForm.close()
                            panel.saved()
                        }

                        parent: Overlay.overlay
                        anchors.centerIn: parent
                        modal: true
                        width: Math.min(520, (parent ? parent.width : 520) - 2 * panel.theme.gap)
                        title: prescriptionDialogForm.prescriptionId === "" ? panel.sioul.text("health-add-prescription") : panel.sioul.text("health-prescription")

                        contentItem: GridLayout {
                            columns: 2
                            columnSpacing: 10
                            rowSpacing: 8

                            Label {
                                text: panel.sioul.text("health-field-what")
                                color: panel.theme.muted
                            }
                            TextField {
                                id: what

                                Layout.fillWidth: true
                                placeholderText: panel.sioul.text("health-field-what-hint")
                            }
                            Label {
                                text: panel.sioul.text("health-field-prescriber")
                                color: panel.theme.muted
                            }
                            TextField {
                                id: prescriber

                                Layout.fillWidth: true
                            }
                            Label {
                                text: panel.sioul.text("health-field-valid-until")
                                color: panel.theme.muted
                            }
                            DateField {
                                id: validUntil

                                theme: panel.theme
                                locale: panel.dateLocale
                            }
                            Label {
                                text: panel.sioul.text("health-field-refill-days")
                                color: panel.theme.muted
                            }
                            RowLayout {
                                spacing: 6

                                SpinBox {
                                    id: refillDays

                                    from: 0
                                    to: 365
                                    editable: true
                                }
                                Label {
                                    text: panel.sioul.text("health-days-at-a-time")
                                    color: panel.theme.muted
                                }
                            }
                            Label {
                                text: panel.sioul.text("health-field-last-refill")
                                color: panel.theme.muted
                            }
                            DateField {
                                id: lastRefill

                                theme: panel.theme
                                locale: panel.dateLocale
                            }
                            Label {
                                text: panel.sioul.text("health-field-note")
                                color: panel.theme.muted
                            }
                            TextField {
                                id: prescriptionNote

                                Layout.fillWidth: true
                            }
                            Label {
                                visible: prescriptionDialogForm.problem !== ""
                                Layout.columnSpan: 2
                                Layout.fillWidth: true
                                text: prescriptionDialogForm.problem
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: panel.theme.warm
                            }
                        }

                        // The buttons inside an Item, as the medicine's: saving that fails keeps the form open.
                        footer: Item {
                            implicitWidth: prescriptionButtons.implicitWidth
                            implicitHeight: prescriptionButtons.implicitHeight

                            DialogButtonBox {
                                id: prescriptionButtons

                                anchors.fill: parent

                                Button {
                                    text: panel.sioul.text("ui-save")
                                    highlighted: true
                                    DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                                }
                                Button {
                                    text: panel.sioul.text("ui-cancel")
                                    DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                                }
                                Button {
                                    visible: prescriptionDialogForm.prescriptionId !== ""
                                    flat: true
                                    text: panel.sioul.text("health-remove")
                                    DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                                    onClicked: removeAsk.now().askFor(what.text, panel.sioul.text("health-remove-prescription-ask"), panel.sioul.text("health-remove"), prescriptionDialogForm.prescriptionId, prescriptionDialogForm)
                                }
                                onAccepted: prescriptionDialogForm.save()
                                onRejected: prescriptionDialogForm.close()
                            }
                        }
                    }
                }
            }

            Later {
                id: removeAsk

                sourceComponent: Component {
                    ConfirmDialog {
                        id: removeAskForm

                        property string target: ""
                        property var form: null

                        function askFor(heading, sentence, action, target, form) {
                            removeAskForm.target = target
                            removeAskForm.form = form
                            removeAskForm.title = panel.theme.plain(heading)
                            removeAskForm.sentence = sentence
                            removeAskForm.action = action
                            removeAskForm.open()
                        }

                        sioul: panel.sioul
                        theme: panel.theme
                        onConfirmed: {
                            const problem = panel.sioul.removeHealth(removeAskForm.target)
                            if (problem !== "")
                                panel.sioul.status = problem
                            if (removeAskForm.form)
                                removeAskForm.form.close()
                            panel.saved()
                        }
                    }
                }
            }
        }
    }
}
