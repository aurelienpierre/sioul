// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Health and well-being, kept on your computers only (sealed when shared
// between them), the most needed first: doses due while Sioul was closed,
// then today's doses, each marked taken with one click; the medicines and
// when they are taken; the prescriptions, when to go to the pharmacy and when
// to see the doctor (each becomes a task in the list chosen on the page, else
// your usual list, else the first list on a server, so that your phone has
// it: docs/health.md); what the watch says, once one is set up (its files
// and offers behind the page's ⚙); then the pauses: one to move during long
// focus, a daily limit on chats. A dose is reminded once, quietly. Nothing
// counts what was missed.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var shown: ({ today: [], missed: [], shared_note: "", reminded_there: "", medicines: [], prescriptions: [], movement: { enabled: true, minutes: 45 }, chats: { enabled: false, minutes: 60, locked_minutes: 30 } })
    readonly property var locale: Qt.locale(page.sioul.text("qt-locale"))

    // Meals, rest and sleep in view: where the Porch's hours card sends you, and the window's tests.
    function showNeeds() {
        const flick = scroll.contentItem as Flickable
        if (flick)
            flick.contentY = Math.max(0, Math.min(page.needsNow().y - 12, flick.contentHeight - flick.height))
    }

    // Shown as last made at once, made again off the window's thread, shown
    // again when it comes (`healthView`): a view made the same as before
    // announces no change, so the page never waits for one.
    // Meals, rest and sleep: made after the first screen, at once when asked.
    function needsNow() {
        needsLoader.asynchronous = false
        return needsLoader
    }

    function reload() {
        page.takeView()
        page.sioul.refreshHealth()
        if (needsLoader.item)
            needsLoader.item.reload()
    }

    function takeView() {
        if (page.sioul.healthView !== "")
            page.shown = JSON.parse(page.sioul.healthView)
    }

    Connections {
        target: page.sioul

        function onHealthViewChanged() {
            page.takeView()
        }
    }

    function setting(key, value) {
        const problem = page.sioul.setHealth(key, String(value))
        if (problem !== "")
            page.sioul.status = problem
        page.reload()
    }

    onVisibleChanged: if (page.visible) page.reload()
    Component.onCompleted: page.reload()

    // A dose marked, or a medicine changed, on another device: shown at once.
    Connections {
        target: page.sioul

        function onSharedIn(stores) {
            if (page.visible && (stores.indexOf("state/health-state.toml") >= 0 || stores.indexOf("data/health.toml") >= 0))
                page.reload()
        }
    }

    // Doses pass their time while the page is open.
    Timer {
        interval: 60000
        running: page.visible && !page.sioul.away
        repeat: true
        onTriggered: page.reload()
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        anchors.margins: page.theme.gap
        contentWidth: availableWidth

        ColumnLayout {
            width: Math.min(scroll.availableWidth, 760)
            spacing: 10

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("ui-health")
                    font.pixelSize: 22
                    color: page.theme.text
                }
                // The page's settings: the watch's files and its offers.
                ToolButton {
                    id: healthSettings

                    icon.name: "settings-configure"
                    icon.color: page.theme.text
                    display: AbstractButton.IconOnly
                    Accessible.name: page.sioul.text("ui-settings")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("ui-settings")
                    ToolTip.delay: 300
                    onClicked: healthPanel.now().open()
                }
            }
            Label {
                Layout.fillWidth: true
                text: page.sioul.text("health-local")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }


            // Doses due while Sioul ran on none of your computers: a question on the
            // past, answered once; never a reminder to take one now.
            Panel {
                visible: page.shown.missed.length > 0
                Layout.fillWidth: true
                Layout.topMargin: 8
                theme: page.theme

                ColumnLayout {
                    anchors.fill: parent
                    spacing: 6

                    Label {
                        Layout.fillWidth: true
                        text: page.sioul.text("health-missed-question")
                        wrapMode: Text.Wrap
                        font.weight: Font.DemiBold
                        color: page.theme.text
                    }
                    Repeater {
                        model: page.shown.missed

                        delegate: ColumnLayout {
                            id: missed

                            required property var modelData

                            Layout.fillWidth: true
                            spacing: 2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 12

                                Label {
                                    Layout.preferredWidth: 90
                                    text: missed.modelData.time
                                    textFormat: Text.PlainText
                                    font.features: { "tnum": 1 }
                                    color: page.theme.text
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: missed.modelData.name + (missed.modelData.dose !== "" ? "  ·  " + missed.modelData.dose : "")
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    color: page.theme.text
                                }
                                // When it was taken, asked (DoseTaken.qml).
                                Button {
                                    text: page.sioul.text("health-taken-when")
                                    onClicked: page.window.askDose(missed.modelData.key)
                                }
                                Button {
                                    flat: true
                                    text: page.sioul.text("health-not-taken")
                                    onClicked: {
                                        page.sioul.doseNotTaken(missed.modelData.key)
                                        page.reload()
                                    }
                                }
                            }
                            // Whether it was taken on another device is not known here: said, never guessed.
                            Label {
                                visible: missed.modelData.doubt !== ""
                                Layout.fillWidth: true
                                text: missed.modelData.doubt
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: page.theme.warm
                            }
                        }
                    }
                }
            }

            // Today.
            Label {
                Layout.topMargin: 8
                text: page.sioul.text("health-today")
                font.weight: Font.DemiBold
                color: page.theme.text
            }
            // A dose marked on another computer may not be here yet; reminders come where you are.
            Label {
                visible: text !== ""
                Layout.fillWidth: true
                text: [page.shown.shared_note, page.shown.reminded_there].filter(t => t !== "").join(" ")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.shown.shared_note !== "" ? page.theme.warm : page.theme.muted
            }
            Label {
                visible: page.shown.today.length === 0
                Layout.fillWidth: true
                text: page.sioul.text("health-today-none")
                color: page.theme.muted
            }
            Repeater {
                model: page.shown.today

                delegate: ColumnLayout {
                    id: dose

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 2

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 12

                        Label {
                            Layout.preferredWidth: 52
                            text: dose.modelData.time
                            textFormat: Text.PlainText
                            font.features: { "tnum": 1 }
                            color: dose.modelData.past ? page.theme.text : page.theme.muted
                        }
                        Label {
                            Layout.fillWidth: true
                            text: dose.modelData.name + (dose.modelData.dose !== "" ? "  ·  " + dose.modelData.dose : "")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: dose.modelData.past ? page.theme.text : page.theme.muted
                        }
                        // Taken: when, said plainly; one click takes it back. More than
                        // half an hour late, when it was taken is asked.
                        Button {
                            flat: dose.modelData.taken !== ""
                            text: dose.modelData.taken !== "" ? page.sioul.textWith("health-taken-at", "time", dose.modelData.taken) : page.sioul.text(dose.modelData.late ? "health-taken-when" : "health-taken")
                            icon.name: dose.modelData.taken !== "" ? "task-complete" : ""
                            icon.color: page.theme.text
                            onClicked: {
                                if (dose.modelData.taken === "" && dose.modelData.late) {
                                    page.window.askDose(dose.modelData.key)
                                    return
                                }
                                page.sioul.setDoseTaken(dose.modelData.key, dose.modelData.taken === "")
                                page.reload()
                            }
                        }
                    }
                    // Not marked here, and not known whether it was taken on another device: said, never guessed.
                    Label {
                        visible: dose.modelData.doubt !== ""
                        Layout.fillWidth: true
                        text: dose.modelData.doubt
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: page.theme.warm
                    }
                }
            }

            // Medicines.
            RowLayout {
                Layout.topMargin: 12
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("health-medicines")
                    font.weight: Font.DemiBold
                    color: page.theme.text
                }
                Button {
                    flat: true
                    text: page.sioul.text("health-add-medicine")
                    icon.name: "list-add"
                    icon.color: page.theme.text
                    onClicked: medicineDialog.now().edit(null)
                }
            }
            Repeater {
                model: page.shown.medicines

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
                            text: medicine.modelData.name + (medicine.modelData.dose ? "  ·  " + medicine.modelData.dose : "") + (medicine.modelData.paused ? "  ·  " + page.sioul.text("health-paused") : "")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: medicine.modelData.paused ? page.theme.muted : page.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: [medicine.modelData.when, medicine.modelData.prescription_title].filter(t => t !== "").join("  ·  ")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                    }
                    Button {
                        flat: true
                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                        text: page.sioul.text("ui-edit")
                        onClicked: medicineDialog.now().edit(medicine.modelData)
                    }
                }
            }

            // Prescriptions.
            RowLayout {
                Layout.topMargin: 12
                Layout.fillWidth: true

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("health-prescriptions")
                    font.weight: Font.DemiBold
                    color: page.theme.text
                }
                Button {
                    flat: true
                    text: page.sioul.text("health-add-prescription")
                    icon.name: "list-add"
                    icon.color: page.theme.text
                    onClicked: prescriptionDialog.now().edit(null)
                }
            }
            Repeater {
                model: page.shown.prescriptions

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
                            color: page.theme.text
                        }
                        Label {
                            visible: text !== ""
                            Layout.fillWidth: true
                            text: prescription.modelData.next.join("  ·  ")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                    }
                    // Fetched at the pharmacy: the next time is counted from today.
                    // The buttons as wide as their names (the style's are 100
                    // pixels at least): more room for the dates on a phone.
                    Button {
                        visible: prescription.modelData.refill_days !== undefined
                        flat: true
                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                        text: page.sioul.text("health-refilled")
                        onClicked: {
                            const problem = page.sioul.refilled(prescription.modelData.id)
                            if (problem !== "")
                                page.sioul.status = problem
                            page.reload()
                        }
                    }
                    Button {
                        flat: true
                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                        text: page.sioul.text("ui-edit")
                        onClicked: prescriptionDialog.now().edit(prescription.modelData)
                    }
                }
            }
            // Where the pharmacy and the renewals go: a list your phone has.
            RowLayout {
                visible: !!page.shown.lists && page.shown.lists.length > 0
                Layout.fillWidth: true
                spacing: 8

                Label {
                    text: page.sioul.text("health-errands-list")
                    color: page.theme.muted
                }
                ComboBox {
                    readonly property var lists: page.shown.lists || []

                    Layout.fillWidth: true
                    model: lists.map(l => page.theme.plain(l.name))
                    currentIndex: Math.max(0, lists.findIndex(l => l.id === page.shown.errands_list))
                    onActivated: index => page.setting("errands_list", lists[index].id)
                }
            }

            // The watch: what it says of today and the week, once one is set up;
            // made after the first screen, without holding the window.
            Loader {
                visible: !!page.shown.watch && page.shown.watch.any
                Layout.fillWidth: true
                asynchronous: true
                sourceComponent: WatchPanel {
                    sioul: page.sioul
                    theme: page.theme
                    watch: page.shown.watch || null
                    onSetting: (key, value) => page.setting(key, value)
                }
            }

            // Meals, naps and the night: kept free of tasks, the work planned around them.
            Loader {
                id: needsLoader

                Layout.fillWidth: true
                asynchronous: true
                sourceComponent: NeedsSection {
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                }
            }

            // A pause to move, while focusing.
            Label {
                Layout.topMargin: 12
                text: page.sioul.text("health-moving")
                font.weight: Font.DemiBold
                color: page.theme.text
            }
            // The minutes under the sentence when the screen is narrow (a phone).
            Flow {
                Layout.fillWidth: true
                spacing: 8

                CheckBox {
                    text: page.sioul.text("health-moving-every")
                    checked: page.shown.movement.enabled
                    onToggled: page.setting("movement.enabled", checked)
                }
                SpinBox {
                    id: movingMinutes

                    from: 10
                    to: 240
                    stepSize: 5
                    editable: true
                    value: page.shown.movement.minutes
                    enabled: page.shown.movement.enabled
                    Accessible.name: page.sioul.text("health-moving-every")
                    onValueModified: page.setting("movement.minutes", value)
                }
                Label {
                    height: movingMinutes.height
                    verticalAlignment: Text.AlignVCenter
                    text: page.sioul.text("health-minutes")
                    color: page.theme.muted
                }
            }
            Label {
                Layout.fillWidth: true
                text: page.sioul.text("health-moving-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }

            // Chats, a limit a day.
            Label {
                Layout.topMargin: 12
                text: page.sioul.text("health-chats")
                font.weight: Font.DemiBold
                color: page.theme.text
            }
            CheckBox {
                text: page.sioul.text("health-chats-limit")
                checked: page.shown.chats.enabled
                onToggled: page.setting("chats.enabled", checked)
            }
            GridLayout {
                enabled: page.shown.chats.enabled
                columns: 3
                columnSpacing: 8
                rowSpacing: 4

                Label {
                    text: page.sioul.text("health-chats-after")
                    color: page.theme.muted
                }
                SpinBox {
                    from: 5
                    to: 600
                    stepSize: 5
                    editable: true
                    value: page.shown.chats.minutes || 60
                    onValueModified: page.setting("chats.minutes", value)
                }
                Label {
                    text: page.sioul.text("health-minutes-a-day")
                    color: page.theme.muted
                }
                Label {
                    text: page.sioul.text("health-chats-for")
                    color: page.theme.muted
                }
                SpinBox {
                    from: 5
                    to: 600
                    stepSize: 5
                    editable: true
                    value: page.shown.chats.locked_minutes || 30
                    onValueModified: page.setting("chats.locked_minutes", value)
                }
                Label {
                    text: page.sioul.text("health-minutes")
                    color: page.theme.muted
                }
            }
            Label {
                Layout.fillWidth: true
                text: page.sioul.text("health-chats-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            Item {
                Layout.preferredHeight: page.theme.gap
            }
        }
    }

    // The page's settings: where the watch's files come from, whether it may offer a pause.
    Later {
        id: healthPanel

        sourceComponent: Component {
            Popup {
                id: healthPanelForm

                parent: Overlay.overlay
                x: Math.round((parent.width - width) / 2)
                y: Math.round(parent.height / 6)
                width: Math.min(520, parent.width - 2 * page.theme.gap)
                modal: true
                padding: page.theme.gap

                background: Rectangle {
                    color: page.theme.background
                    radius: page.theme.radius
                    border.color: page.theme.line
                }

                contentItem: ColumnLayout {
                    spacing: 8

                    Label {
                        text: page.sioul.text("ui-settings")
                        font.pixelSize: 17
                        font.weight: Font.DemiBold
                        color: page.theme.text
                    }
                    WatchPanel {
                        Layout.fillWidth: true
                        part: "settings"
                        sioul: page.sioul
                        theme: page.theme
                        watch: page.shown.watch || null
                        onSetting: (key, value) => page.setting(key, value)
                    }
                    Button {
                        Layout.alignment: Qt.AlignRight
                        text: page.sioul.text("ui-close")
                        onClicked: healthPanelForm.close()
                    }
                }
            }
        }
    }

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
                    const prescriptions = page.shown.prescriptions
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
                        prescription: link.currentIndex > 0 ? page.shown.prescriptions[link.currentIndex - 1].id : "",
                        paused: paused.checked
                    }
                    const answer = JSON.parse(page.sioul.saveMedicine(medicineDialogForm.medicineId, JSON.stringify(edit)))
                    if (answer.error) {
                        medicineDialogForm.problem = answer.error
                        return
                    }
                    medicineDialogForm.close()
                    page.reload()
                }

                anchors.centerIn: parent
                modal: true
                width: Math.min(520, page.width - 2 * page.theme.gap)
                title: medicineDialogForm.medicineId === "" ? page.sioul.text("health-add-medicine") : page.sioul.text("health-medicine")

                contentItem: GridLayout {
                    columns: 2
                    columnSpacing: 10
                    rowSpacing: 8

                    Label {
                        text: page.sioul.text("health-field-name")
                        color: page.theme.muted
                    }
                    TextField {
                        id: medicineName

                        Layout.fillWidth: true
                    }
                    Label {
                        text: page.sioul.text("health-field-dose")
                        color: page.theme.muted
                    }
                    TextField {
                        id: medicineDose

                        Layout.fillWidth: true
                        placeholderText: page.sioul.text("health-field-dose-hint")
                    }
                    Label {
                        text: page.sioul.text("health-field-when")
                        color: page.theme.muted
                    }
                    ComboBox {
                        id: every

                        Layout.fillWidth: true
                        model: medicineDialogForm.everies.map(e => page.sioul.text("health-every-" + e + "-choice"))
                    }
                    Label {
                        visible: every.currentIndex === 0
                        text: page.sioul.text("health-field-times")
                        color: page.theme.muted
                    }
                    TextField {
                        id: times

                        visible: every.currentIndex === 0
                        Layout.fillWidth: true
                        placeholderText: "12:00, 18:00"
                    }
                    Label {
                        visible: every.currentIndex === 1
                        text: page.sioul.text("health-field-every-days")
                        color: page.theme.muted
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
                            text: page.sioul.text("health-field-at")
                            color: page.theme.muted
                        }
                        TextField {
                            id: dayTime

                            Layout.preferredWidth: 70
                            inputMask: "99:99"
                        }
                    }
                    Label {
                        visible: every.currentIndex === 1
                        text: page.sioul.text("health-field-from")
                        color: page.theme.muted
                    }
                    DateField {
                        id: from

                        visible: every.currentIndex === 1
                        theme: page.theme
                        locale: page.locale
                    }
                    Label {
                        visible: every.currentIndex === 2
                        text: page.sioul.text("health-field-every-hours")
                        color: page.theme.muted
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
                            text: page.sioul.text("health-field-from-time")
                            color: page.theme.muted
                        }
                        TextField {
                            id: hourFrom

                            Layout.preferredWidth: 70
                            inputMask: "99:99"
                        }
                    }

                    Label {
                        text: page.sioul.text("health-field-until")
                        color: page.theme.muted
                    }
                    DateField {
                        id: until

                        theme: page.theme
                        locale: page.locale
                    }
                    Label {
                        text: page.sioul.text("health-field-prescription")
                        color: page.theme.muted
                    }
                    ComboBox {
                        id: link

                        Layout.fillWidth: true
                        model: [page.sioul.text("health-no-prescription")].concat(page.shown.prescriptions.map(p => page.theme.plain(p.title)))
                    }
                    Item {
                        Layout.preferredHeight: 1
                    }
                    CheckBox {
                        id: paused

                        visible: medicineDialogForm.medicineId !== ""
                        text: page.sioul.text("health-pause")
                    }
                    Label {
                        visible: medicineDialogForm.problem !== ""
                        Layout.columnSpan: 2
                        Layout.fillWidth: true
                        text: medicineDialogForm.problem
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: page.theme.warm
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
                            text: page.sioul.text("ui-save")
                            highlighted: true
                            DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                        }
                        Button {
                            text: page.sioul.text("ui-cancel")
                            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                        }
                        Button {
                            visible: medicineDialogForm.medicineId !== ""
                            flat: true
                            text: page.sioul.text("health-remove")
                            DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                            onClicked: removeAsk.now().askFor(medicineName.text, page.sioul.text("health-remove-medicine-ask"), page.sioul.text("health-remove"), medicineDialogForm.medicineId, medicineDialogForm)
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
                    const answer = JSON.parse(page.sioul.savePrescription(prescriptionDialogForm.prescriptionId, JSON.stringify(edit)))
                    if (answer.error) {
                        prescriptionDialogForm.problem = answer.error
                        return
                    }
                    prescriptionDialogForm.close()
                    page.reload()
                }

                anchors.centerIn: parent
                modal: true
                width: Math.min(520, page.width - 2 * page.theme.gap)
                title: prescriptionDialogForm.prescriptionId === "" ? page.sioul.text("health-add-prescription") : page.sioul.text("health-prescription")

                contentItem: GridLayout {
                    columns: 2
                    columnSpacing: 10
                    rowSpacing: 8

                    Label {
                        text: page.sioul.text("health-field-what")
                        color: page.theme.muted
                    }
                    TextField {
                        id: what

                        Layout.fillWidth: true
                        placeholderText: page.sioul.text("health-field-what-hint")
                    }
                    Label {
                        text: page.sioul.text("health-field-prescriber")
                        color: page.theme.muted
                    }
                    TextField {
                        id: prescriber

                        Layout.fillWidth: true
                    }
                    Label {
                        text: page.sioul.text("health-field-valid-until")
                        color: page.theme.muted
                    }
                    DateField {
                        id: validUntil

                        theme: page.theme
                        locale: page.locale
                    }
                    Label {
                        text: page.sioul.text("health-field-refill-days")
                        color: page.theme.muted
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
                            text: page.sioul.text("health-days-at-a-time")
                            color: page.theme.muted
                        }
                    }
                    Label {
                        text: page.sioul.text("health-field-last-refill")
                        color: page.theme.muted
                    }
                    DateField {
                        id: lastRefill

                        theme: page.theme
                        locale: page.locale
                    }
                    Label {
                        text: page.sioul.text("health-field-note")
                        color: page.theme.muted
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
                        color: page.theme.warm
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
                            text: page.sioul.text("ui-save")
                            highlighted: true
                            DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                        }
                        Button {
                            text: page.sioul.text("ui-cancel")
                            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                        }
                        Button {
                            visible: prescriptionDialogForm.prescriptionId !== ""
                            flat: true
                            text: page.sioul.text("health-remove")
                            DialogButtonBox.buttonRole: DialogButtonBox.ActionRole
                            onClicked: removeAsk.now().askFor(what.text, page.sioul.text("health-remove-prescription-ask"), page.sioul.text("health-remove"), prescriptionDialogForm.prescriptionId, prescriptionDialogForm)
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
                    removeAskForm.title = page.theme.plain(heading)
                    removeAskForm.sentence = sentence
                    removeAskForm.action = action
                    removeAskForm.open()
                }

                sioul: page.sioul
                theme: page.theme
                onConfirmed: {
                    const problem = page.sioul.removeHealth(removeAskForm.target)
                    if (problem !== "")
                        page.sioul.status = problem
                    if (removeAskForm.form)
                        removeAskForm.form.close()
                    page.reload()
                }
            }
        }
    }
}
