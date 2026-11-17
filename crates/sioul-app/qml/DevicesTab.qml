// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Settings ▸ Devices (« Appareils »; docs/database.md, "Devices"): every
// device Sioul shares with, this one first, as the core words them
// (sioul-sync's device_table, the same rows `sioul devices` prints). For
// each: its name, yours when you gave one ("Rename…"), what it is (this
// device's own can be changed here, as in Settings ▸ This device), how it is
// now, when it last opened and last shared, its Sioul, what it is for (three
// switches, nothing ticked being everything), the parts it shares (read
// only for another device: a part is switched on where its data lands), and
// for another device Count it again, Forget, Forget it now, and Forget it
// and remove its files (RemovalAsked.qml). In a wide window the fields line
// up under their names, as a table; in a narrow one each device is a card,
// each field under its name. With sharing off, this device alone.
//
// What a device is for is written into the shared settings and shown;
// nothing is folded or held by it yet.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: tab

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The setting of what this device is (`device_kind`), as the settings'
    // rows give it; none while they are not read.
    property var kindRow: null

    // What the core says: { on, why, rows } (`share::devices`).
    property var view: ({ on: false, why: "", rows: [] })
    readonly property var rows: tab.view.rows || []
    property string problem: ""
    // Wide enough for eight columns: the fields line up as a table.
    readonly property bool wide: tab.width >= 900
    // Each column's share of the width: the device, what it is, now, last
    // opened, last shared, its Sioul, what it is for, what it shares.
    readonly property var shares: [0.15, 0.09, 0.17, 0.09, 0.11, 0.12, 0.14, 0.13]
    readonly property var titles: ["devices-col-name", "devices-col-form", "devices-col-now", "devices-col-opened", "devices-col-shared", "devices-col-sioul", "devices-col-for", "devices-col-parts"]
    readonly property int pad: 12
    readonly property int between: 10
    // The device being renamed (its id), and the one whose kind is being changed (this one).
    property string renaming: ""
    property bool kindOpen: false
    // "Remove its files" pressed for a device (its id): what goes and what stays, said first ({text, live}).
    property string removing: ""
    property var removal: ({ text: "", live: false })

    // This device's kind changed: saved as any setting (ParametersPage.qml).
    signal save(string key, var value, var shown)
    // Your folder and sharing asked for: where this device's parts are switched.
    signal sharingAsked

    // A column's width in the table.
    function col(index) {
        return Math.floor((tab.width - 2 * tab.pad - 7 * tab.between) * tab.shares[index])
    }

    function reload() {
        tab.view = JSON.parse(tab.sioul.devices() || "{}")
    }

    function say(id, field, value) {
        tab.problem = tab.sioul.deviceSay(id, field, value)
        tab.reload()
    }

    function askRemoval(id) {
        tab.removal = JSON.parse(tab.sioul.deviceFiles(id) || "{}")
        tab.removing = id
    }

    spacing: 8
    onVisibleChanged: if (visible) tab.reload()
    Component.onCompleted: tab.reload()

    // While shown: news of the other devices as it comes; never under a name
    // being typed, a choice being made or a question being read.
    Timer {
        interval: 20000
        running: tab.visible && tab.renaming === "" && !tab.kindOpen && tab.removing === ""
        repeat: true
        onTriggered: tab.reload()
    }

    // A field of a device: in the table, a cell under its column's title;
    // in a card, its name above it. What is declared inside comes under its words.
    component Field: ColumnLayout {
        id: field

        // The tab: an inline component is given what it reads.
        required property var host
        required property int column
        property string value: ""

        Layout.alignment: Qt.AlignTop
        Layout.fillWidth: !field.host.wide
        Layout.preferredWidth: field.host.wide ? field.host.col(field.column) : -1
        Layout.maximumWidth: field.host.wide ? field.host.col(field.column) : Number.POSITIVE_INFINITY
        spacing: 2

        Label {
            visible: !field.host.wide
            Layout.fillWidth: true
            text: field.host.sioul.text(field.host.titles[field.column])
            font.pixelSize: 12
            elide: Text.ElideRight
            color: field.host.theme.muted
        }
        Label {
            visible: field.value !== ""
            Layout.fillWidth: true
            text: field.value
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 14
            color: field.host.theme.text
        }
    }

    Label {
        Layout.topMargin: 12
        Layout.fillWidth: true
        text: tab.sioul.text("devices-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: tab.theme.accent
    }
    Label {
        Layout.fillWidth: true
        text: tab.sioul.text("devices-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.25
        color: tab.theme.muted
    }
    Label {
        visible: tab.problem !== ""
        Layout.fillWidth: true
        text: tab.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.warm
    }
    // Sharing off: why this device is alone (it does not share, or its
    // sharing's key is no longer held), and where sharing is set up.
    Label {
        visible: tab.view.on !== true
        Layout.fillWidth: true
        text: tab.view.why || tab.sioul.text("devices-alone")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.text
    }
    Button {
        objectName: "devicesToSharing"
        visible: tab.view.on !== true
        flat: true
        text: tab.sioul.text("settings-tab-files")
        icon.name: "go-next"
        icon.color: tab.theme.text
        onClicked: tab.sharingAsked()
    }

    // The table's titles, above the devices.
    RowLayout {
        visible: tab.wide
        Layout.fillWidth: true
        Layout.leftMargin: tab.pad
        Layout.rightMargin: tab.pad
        Layout.topMargin: 6
        spacing: tab.between

        Repeater {
            model: tab.wide ? tab.titles : []

            delegate: Label {
                required property string modelData
                required property int index

                Layout.preferredWidth: tab.col(index)
                Layout.maximumWidth: tab.col(index)
                text: tab.sioul.text(modelData)
                font.pixelSize: 12
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                color: tab.theme.muted
            }
        }
    }

    Repeater {
        model: tab.rows

        delegate: Rectangle {
            id: device

            required property var modelData
            readonly property var row: device.modelData
            readonly property string deviceId: device.row.id || ""
            readonly property bool renamed: (device.row.given || "") !== "" && device.row.given !== device.row.system

            objectName: "device-" + device.deviceId
            Layout.fillWidth: true
            implicitHeight: body.implicitHeight + 2 * tab.pad
            radius: tab.theme.radius
            color: tab.theme.surface
            border.width: 1
            border.color: tab.theme.line

            ColumnLayout {
                id: body

                x: tab.pad
                y: tab.pad
                width: device.width - 2 * tab.pad
                spacing: 6

                GridLayout {
                    Layout.fillWidth: true
                    columns: tab.wide ? 8 : 1
                    columnSpacing: tab.between
                    rowSpacing: 8

                    // Its name: yours when given, with what its system calls it under it.
                    ColumnLayout {
                        Layout.alignment: Qt.AlignTop
                        Layout.fillWidth: !tab.wide
                        Layout.preferredWidth: tab.wide ? tab.col(0) : -1
                        Layout.maximumWidth: tab.wide ? tab.col(0) : Number.POSITIVE_INFINITY
                        spacing: 2

                        Label {
                            objectName: "deviceName"
                            Layout.fillWidth: true
                            text: device.row.name || ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 15
                            font.weight: Font.DemiBold
                            color: tab.theme.text
                        }
                        Label {
                            visible: device.row.here === true
                            Layout.fillWidth: true
                            text: tab.sioul.text("devices-this")
                            font.pixelSize: 12
                            elide: Text.ElideRight
                            color: tab.theme.accent
                        }
                        Label {
                            visible: device.renamed
                            Layout.fillWidth: true
                            text: device.renamed ? tab.sioul.textWith("devices-system-name", "system", device.row.system || "") : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: tab.theme.muted
                        }
                    }
                    Field {
                        host: tab
                        column: 1
                        value: device.row.form_words || ""
                    }
                    Field {
                        host: tab
                        column: 2
                        value: [device.row.state || ""].concat(device.row.notes || []).join(" ")
                    }
                    Field {
                        host: tab
                        column: 3
                        value: device.row.opened || ""
                    }
                    Field {
                        host: tab
                        column: 4
                        value: device.row.shared || ""
                    }
                    // Its Sioul: an older one said calmly, in its own sentence.
                    Field {
                        host: tab
                        column: 5
                        value: device.row.older === true ? (device.row.build || "") : (device.row.version || "")
                    }
                    // What it is for: three switches; nothing ticked, everything comes here.
                    Field {
                        host: tab
                        column: 6

                        AreaSwitches {
                            objectName: "deviceFor"
                            Layout.fillWidth: true
                            sioul: tab.sioul
                            value: device.row.area || ""
                            stacked: tab.wide
                            onPicked: value => tab.say(device.deviceId, "area", value)
                        }
                        Label {
                            objectName: "deviceForAll"
                            visible: (device.row.area || "") === ""
                            Layout.fillWidth: true
                            text: tab.sioul.text("devices-for-all")
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: tab.theme.muted
                        }
                    }
                    // What it shares: read only; this device's own are switched in Your folder and sharing (below).
                    Field {
                        host: tab
                        column: 7
                        value: device.row.parts_words || ""
                    }
                }

                // The name you give it: empty, its system's name comes back.
                ColumnLayout {
                    visible: tab.renaming === device.deviceId
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        Layout.fillWidth: true
                        text: tab.sioul.text("devices-rename-help")
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: tab.theme.muted
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        TextField {
                            id: nameField

                            objectName: "deviceNameField"
                            Layout.fillWidth: true
                            Layout.maximumWidth: 360
                            maximumLength: 60
                            placeholderText: tab.theme.plain(device.row.system || tab.sioul.text("devices-rename-field"))
                            Accessible.name: tab.sioul.text("devices-rename-field")
                            onAccepted: saveName.clicked()
                        }
                        Button {
                            id: saveName

                            text: tab.sioul.text("ui-save")
                            onClicked: {
                                tab.renaming = ""
                                tab.say(device.deviceId, "name", nameField.text)
                            }
                        }
                        Button {
                            flat: true
                            text: tab.sioul.text("ui-cancel")
                            onClicked: tab.renaming = ""
                        }
                    }
                }

                // What this device is, changed here as in Settings ▸ This device.
                Loader {
                    active: device.row.here === true && tab.kindOpen && tab.kindRow !== null
                    visible: active
                    Layout.fillWidth: true
                    Layout.maximumWidth: 640

                    sourceComponent: Component {
                        SettingRow {
                            setting: tab.kindRow
                            sioul: tab.sioul
                            theme: tab.theme
                            onSave: (key, value, shown) => {
                                tab.save(key, value, shown)
                                tab.reload()
                            }
                        }
                    }
                }

                // A phone silent a day, on a computer: forgotten now, said first what it changes.
                Label {
                    visible: device.row.forget_now === true
                    Layout.fillWidth: true
                    text: tab.sioul.text("share-device-forget-now-help")
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: tab.theme.muted
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        objectName: "deviceRename"
                        visible: tab.renaming !== device.deviceId
                        flat: true
                        text: tab.sioul.text("devices-rename")
                        onClicked: {
                            nameField.text = device.row.given || ""
                            tab.renaming = device.deviceId
                            nameField.forceActiveFocus()
                        }
                    }
                    Button {
                        objectName: "deviceKind"
                        visible: device.row.here === true && tab.kindRow !== null
                        flat: true
                        text: tab.sioul.text("set-device-kind")
                        onClicked: tab.kindOpen = !tab.kindOpen
                    }
                    // This device's parts are switched where its sharing is set up.
                    Button {
                        objectName: "devicePartsChange"
                        visible: device.row.here === true && tab.view.on === true
                        flat: true
                        text: tab.sioul.text("devices-parts-change")
                        onClicked: tab.sharingAsked()
                    }
                    Button {
                        visible: device.row.off === true
                        flat: true
                        text: tab.sioul.text("share-device-count-again")
                        onClicked: {
                            tab.problem = tab.sioul.deviceOff(device.deviceId, false)
                            tab.reload()
                        }
                    }
                    Button {
                        visible: device.row.forget === true
                        flat: true
                        text: tab.sioul.text("share-device-forget")
                        onClicked: {
                            tab.problem = tab.sioul.deviceForget(device.deviceId)
                            tab.reload()
                        }
                    }
                    Button {
                        visible: device.row.forget_now === true
                        flat: true
                        text: tab.sioul.text("share-device-forget-now")
                        onClicked: {
                            tab.problem = tab.sioul.deviceForget(device.deviceId)
                            tab.reload()
                        }
                    }
                    // Its files taken out of the folder, forgotten first (docs/database.md, "Removing a device's files").
                    Button {
                        visible: device.row.here !== true && tab.removing !== device.deviceId
                        flat: true
                        text: tab.sioul.text("share-device-remove-live")
                        onClicked: tab.askRemoval(device.deviceId)
                    }
                }
                RemovalAsked {
                    visible: device.row.here !== true && tab.removing === device.deviceId
                    Layout.fillWidth: true
                    host: tab
                    deviceId: device.deviceId
                }
            }
        }
    }

    // Why another device's parts cannot be switched from here.
    Label {
        visible: tab.rows.length > 1
        Layout.fillWidth: true
        Layout.bottomMargin: 24
        text: tab.sioul.text("devices-parts-elsewhere")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: tab.theme.muted
    }
}
