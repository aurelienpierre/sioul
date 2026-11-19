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
// What a device is for is written into the shared settings, and each device
// then brings forward only that, at its hours (docs/areas.md, "What each
// device is for"). Under the devices, the grid says what comes forward on
// each at each of the five times (DeviceGrid.qml): in a wide window one
// matrix, devices by times; in a narrow one each card says its cells in
// words, a line per time. A cell opens three boxes and "As its areas say";
// one you changed is framed. Then one sentence for now on this device, one
// for the areas no device is for, and what a device's areas never keep back.
//
// A phone's card holds its own sections (PhoneCard.qml): its calls, set from
// any device; on the phone itself, what Android must allow, Android's
// restricted settings, and what the tab "This phone" held (design of 18
// November 2026). At a phone's width each section folds.

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

    // What the core says: { on, why, stays, rows, grid, calls } (`share::devices`):
    // `calls`, each phone's section Calls by its id (`calls_line::view`).
    property var view: ({ on: false, why: "", stays: "", rows: [] })
    readonly property var calls: tab.view.calls || ({})
    // What this phone says of itself, on the phone (`phonecard::view`): {phone, allow, restricted}.
    property var own: ({ phone: false })
    readonly property var rows: tab.view.rows || []
    // The devices' ids, in the table's order when a device last came or went:
    // changed only then, never by a rename, so that a reload keeps each card
    // (its folds opened, its sections) instead of making them again.
    property var ids: []
    // The phone card a door lands in, by its part ("calls", "apps"): this
    // device's when it is a phone, else the first phone's (`reveal`).
    readonly property string callsDoor: tab.doorOf(id => tab.calls[id] !== undefined)
    readonly property string appsDoor: tab.doorOf(id => tab.calls[id] !== undefined && (tab.own.phone === true ? tab.rows.some(r => r.id === id && r.here === true) : tab.lists > 0))
    // How many apps, conversations and sites have times of their own (`appNotesSetup`).
    property int lists: 0
    // A door that found no card to land in, in a line (`reveal`); "" otherwise.
    property string doorSaid: ""
    // What is said around the cells: the times' names, now on this device,
    // the areas no device is for, what is never kept back (`device_grid::Grid`).
    readonly property var grid: tab.view.grid || ({ times: [], now: "", unheld: "", never: "" })
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
    // The name being typed there, kept by the tab: anything else pressed in
    // it reads the devices again and makes every card anew (the review of
    // phase 1, R5), and the new card's field takes it back.
    property string nameDraft: ""
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

    // The devices again: every 20 seconds while shown, after each change. Never Android's
    // accesses nor the phone's stars (review of `phones`, L12): those when the tab shows
    // or the window comes back (`reloadAll`), and in answer to what is asked of them.
    function reload() {
        tab.view = JSON.parse(tab.sioul.devices() || "{}")
    }

    // Everything the tab shows: what this device says of itself first (the cards are made
    // knowing it), the devices, the apps with times of their own.
    function reloadAll() {
        if (typeof tab.sioul.phoneCard === "function")
            tab.own = JSON.parse(tab.sioul.phoneCard() || "{}")
        tab.reload()
        if (typeof tab.sioul.appNotesSetup === "function")
            tab.lists = ((JSON.parse(tab.sioul.appNotesSetup() || "{}") || {}).grid || { rows: [] }).rows.length
    }

    // A door's part in view (ParametersPage.qml, "devices.calls", "devices.apps"):
    // the card it lands in, that section unfolded; none without such a card.
    function reveal(part) {
        const id = part === "calls" ? tab.callsDoor : part === "apps" ? tab.appsDoor : ""
        const card = id === "" ? null : tab.cardOf(tab, id)
        const found = card ? card.reveal(part) : null
        // No card to land in: one line says why, at the tab's top (review of `phones`, L17).
        tab.doorSaid = found ? "" : tab.sioul.text(part === "calls" ? "devices-door-no-phone" : "devices-door-no-apps")
        return found
    }

    function cardOf(item, id) {
        const card = item as PhoneCard
        if (card && card.deviceId === id)
            return card
        for (let i = 0; i < item.children.length; i++) {
            const found = tab.cardOf(item.children[i], id)
            if (found)
                return found
        }
        return null
    }

    // The device whose card a door lands in: this one when it fits, else the first that does.
    function doorOf(fits) {
        const here = tab.rows.find(r => r.here === true)
        if (here && fits(here.id))
            return here.id
        const first = tab.rows.find(r => fits(r.id))
        return first ? first.id : ""
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
    onVisibleChanged: {
        if (visible)
            tab.reloadAll()
        else
            tab.doorSaid = ""
    }
    // The cards follow the devices, not each reading of them: a device renamed, which moves in the
    // table's order, keeps every card as it is (review of `phones`, L14); one that comes or goes makes them anew.
    onRowsChanged: {
        const ids = tab.rows.map(r => r.id || "")
        if (JSON.stringify(ids.slice().sort()) !== JSON.stringify(tab.ids.slice().sort()))
            tab.ids = ids
    }
    Component.onCompleted: tab.reloadAll()

    // Back from Android's question or page: what it allows here, read again
    // (and, back from Sioul's page, the accesses asked again: `phonecard::view`).
    Connections {
        target: tab.Window.window

        function onActiveChanged() {
            const window = tab.Window.window
            // Back from Android's page: the accesses still held asked again by themselves (M10).
            if (window && window.active && tab.visible && typeof tab.sioul.phoneCardBack === "function")
                tab.own = JSON.parse(tab.sioul.phoneCardBack() || "{}")
        }
    }

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
    // A door with no card to land in: no phone known, or no app with times of its own yet.
    Label {
        objectName: "devicesDoorSaid"
        visible: tab.doorSaid !== ""
        Layout.fillWidth: true
        text: tab.doorSaid
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.text
    }
    // Settings and accounts do not travel from here: what is said in this tab stays here.
    Label {
        objectName: "devicesStay"
        visible: (tab.view.stays || "") !== ""
        Layout.fillWidth: true
        text: tab.view.stays || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.text
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
        model: tab.ids

        delegate: Rectangle {
            id: device

            required property string modelData
            readonly property var row: tab.rows.find(r => r.id === device.modelData) || ({})
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

                // On a phone: what comes forward on it at each time, in words, a line per time.
                Loader {
                    active: !tab.wide && tab.grid.times.length > 0
                    visible: active
                    Layout.fillWidth: true

                    sourceComponent: Component {
                        ColumnLayout {
                            spacing: 2

                            Label {
                                Layout.fillWidth: true
                                text: tab.sioul.text("devices-grid-title")
                                font.pixelSize: 12
                                elide: Text.ElideRight
                                color: tab.theme.muted
                            }
                            DeviceGrid {
                                objectName: "deviceCells"
                                Layout.fillWidth: true
                                sioul: tab.sioul
                                theme: tab.theme
                                rows: [device.row]
                                times: tab.grid.times
                                wide: false
                                help: tab.sioul.text("devices-cell-help")
                                onPicked: (id, time, areas) => tab.say(id, "at." + time, areas)
                            }
                        }
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
                            onTextEdited: tab.nameDraft = nameField.text
                            onAccepted: saveName.clicked()
                            // A card made anew while its name was being typed: what was typed, again.
                            Component.onCompleted: {
                                if (tab.renaming === device.deviceId)
                                    nameField.text = tab.nameDraft
                            }
                        }
                        Button {
                            id: saveName

                            text: tab.sioul.text("ui-save")
                            onClicked: {
                                const name = nameField.text
                                tab.renaming = ""
                                tab.nameDraft = ""
                                tab.say(device.deviceId, "name", name)
                            }
                        }
                        Button {
                            flat: true
                            text: tab.sioul.text("ui-cancel")
                            onClicked: {
                                tab.renaming = ""
                                tab.nameDraft = ""
                            }
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
                            tab.nameDraft = device.row.given || ""
                            nameField.text = tab.nameDraft
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

                // A phone's own sections: its calls, from any device; on the phone itself what
                // Android must allow and the tab "This phone"'s blocks (PhoneCard.qml). This
                // device's own is where a door to the phone's setup lands (`setting:devices.here`).
                Loader {
                    active: tab.calls[device.deviceId] !== undefined || (device.row.here === true && tab.own.phone === true)
                    visible: active
                    Layout.fillWidth: true
                    Layout.topMargin: 6

                    sourceComponent: Component {
                        PhoneCard {
                            objectName: device.row.here === true ? "setting:devices.here" : "phoneCard-" + device.deviceId
                            sioul: tab.sioul
                            theme: tab.theme
                            deviceId: device.deviceId
                            here: device.row.here === true
                            calls: tab.calls[device.deviceId] || null
                            own: tab.own
                            folds: !tab.wide
                            lists: tab.lists
                            onSay: (field, value) => tab.say(device.deviceId, field, value)
                            onOwnSaid: own => tab.own = own
                        }
                    }
                }
            }
        }
    }

    // Why another device's parts cannot be switched from here.
    Label {
        visible: tab.rows.length > 1
        Layout.fillWidth: true
        text: tab.sioul.text("devices-parts-elsewhere")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: tab.theme.muted
    }

    // What comes forward, and when: the grid, devices by times.
    Label {
        visible: tab.grid.times.length > 0
        Layout.topMargin: 16
        Layout.fillWidth: true
        text: tab.sioul.text("devices-grid-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: tab.theme.accent
    }
    Label {
        visible: tab.grid.times.length > 0
        Layout.fillWidth: true
        text: tab.sioul.text("devices-grid-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.25
        color: tab.theme.muted
    }
    Loader {
        active: tab.wide && tab.grid.times.length > 0
        visible: active
        Layout.fillWidth: true
        Layout.leftMargin: tab.pad
        Layout.rightMargin: tab.pad

        sourceComponent: Component {
            DeviceGrid {
                objectName: "deviceMatrix"
                sioul: tab.sioul
                theme: tab.theme
                rows: tab.rows
                times: tab.grid.times
                wide: true
                help: tab.sioul.text("devices-cell-help")
                onPicked: (id, time, areas) => tab.say(id, "at." + time, areas)
            }
        }
    }
    // Now, on this device; the areas no device is for; what is never kept back.
    Label {
        objectName: "devicesNow"
        visible: (tab.grid.now || "") !== ""
        Layout.fillWidth: true
        text: tab.grid.now || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.text
    }
    Label {
        objectName: "devicesUnheld"
        visible: (tab.grid.unheld || "") !== ""
        Layout.fillWidth: true
        text: tab.grid.unheld || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.text
    }
    // Settings that do not travel from here: what this device tells may come twice.
    Label {
        objectName: "devicesTwice"
        visible: (tab.grid.twice || "") !== ""
        Layout.fillWidth: true
        text: tab.grid.twice || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: tab.theme.text
    }
    Label {
        objectName: "devicesNever"
        visible: tab.grid.times.length > 0
        Layout.fillWidth: true
        Layout.bottomMargin: 24
        text: tab.grid.never || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.25
        color: tab.theme.muted
    }
}
