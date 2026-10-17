// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sharing with your other computers, on the Parameters page: what travels and
// how, the folder a sync carries, a passphrase typed once on each computer
// (twice on the first), then where things stand; what travels from this
// device, part by part; the versions kept before other devices' changes, to put back.
// Not shared yet: joining your other devices through your Nextcloud's own
// sign-in (JoinPanel.qml), beside the two ways. A phone sharing beside
// another phone: whether it replaces it, asked once. A computer that shares:
// what a new device needs ("Add a device"), and a phone silent a day forgotten now.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: panel

    required property var sioul
    required property var theme

    property var status: ({ on: false, folder: "", sealed: false, lines: [], problems: [], parts: [], vanished: [], withheld: [], devices: [], gone_devices: [], backup: {} })
    // The other devices' files fetched from the server too: {shown, line, on, given, found}.
    readonly property var backup: panel.status.backup || ({})
    property string problem: ""
    // Notes or papers about to be switched on: what would travel, said first.
    property string asking: ""
    property string estimate: ""
    // The versions kept, shown on demand: part → file → versions; one being put back, said first.
    property bool showHistory: false
    property var history: []
    property string openFile: ""
    property var confirming: null
    property bool puttingBack: false
    // How this device reaches the folder: through a folder a sync app carries,
    // or Sioul keeping it in step with a Nextcloud itself, no sync app
    // (docs/database.md, "Kept in step by Sioul itself"); started off the window's thread.
    property bool byServer: false
    property bool starting: false
    // "Send everything again" pressed, its answer not read yet, and what it said before.
    property bool sending: false
    property string againBefore: ""
    // "Stop sharing" pressed: what stops and what stays, said first.
    property bool stopping: false
    // A sign-in through the server under way, or its result to read: the two ways wait meanwhile.
    readonly property bool joining: joinPanel.busy
    // Not shared yet, nothing joining: the two ways and their form.
    readonly property bool setting: !panel.status.on && !panel.joining
    // "Add a device" pressed: what a new device needs, said.
    property bool adding: false
    // "Remove its files" pressed for a device (its id): what goes and what stays, said first ({text, live}).
    property string removing: ""
    property var removal: ({ text: "", live: false })

    function askRemoval(id) {
        panel.removal = JSON.parse(panel.sioul.deviceFiles(id) || "{}")
        panel.removing = id
    }
    // The sign-in's panel, for the window's pictures (main.qml's grabber).
    property alias join: joinPanel

    // The Porch's card ("Join your other devices"): Murena's sign-in, another
    // Nextcloud's address, or the folder of a sync app.
    function startJoin(how) {
        if (how === "folder") {
            panel.byServer = false
            folderField.forceActiveFocus()
            return
        }
        joinPanel.begin(how)
    }

    // "Send everything again", off the window's thread: what it did is read a few seconds later.
    function sendAgain() {
        panel.sending = true
        panel.againBefore = panel.status.again || ""
        sentAgain.tries = 0
        panel.sioul.shareSendAgain()
        sentAgain.restart()
    }

    function listHistory() {
        panel.sioul.shareHistory(historyFilter.text)
    }

    Connections {
        target: panel.sioul

        function onShareListed(versions) {
            panel.history = JSON.parse(versions || "[]")
        }
        function onSharePutBackDone(problem) {
            panel.puttingBack = false
            panel.confirming = null
            panel.problem = problem
            panel.listHistory()
            panel.reload()
        }
        function onShareEstimated(estimate) {
            const counted = JSON.parse(estimate || "{}")
            if (counted.part === panel.asking)
                panel.estimate = counted.text || ""
        }
        function onShareStarted(problem) {
            panel.starting = false
            panel.problem = problem
            if (problem === "") {
                passphrase.clear()
                again.clear()
            }
            panel.reload()
        }
    }
    // Android: no folder dialog (it hands out content:// addresses, not paths);
    // the path typed, or one of the folders your other devices share through.
    readonly property bool android: Qt.platform.os === "android"
    property bool filesAccess: true
    property var candidates: []

    function reload() {
        panel.filesAccess = panel.sioul.filesAccess()
        panel.status = JSON.parse(panel.sioul.shareStatus(folderField.text))
        if (!folderField.activeFocus)
            folderField.text = panel.status.folder
        if (!placeField.activeFocus)
            placeField.text = (panel.status.backup || {}).given || ""
        panel.candidates = panel.status.on || !panel.filesAccess ? [] : JSON.parse(panel.sioul.shareCandidates() || "[]").filter(c => c !== folderField.text)
    }

    // A folder typed or picked: what it holds is said at once.
    function choose(folder) {
        folderField.text = folder
        panel.reload()
    }

    spacing: 6
    onVisibleChanged: if (visible) panel.reload()
    Component.onCompleted: panel.reload()

    // While shown: news of the other computers as it comes.
    Timer {
        interval: 20000
        running: panel.visible
        repeat: true
        onTriggered: panel.reload()
    }

    Label {
        Layout.topMargin: 18
        Layout.fillWidth: true
        text: panel.sioul.text("share-title")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: panel.theme.accent
    }
    Label {
        Layout.fillWidth: true
        text: panel.sioul.text("share-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.muted
    }
    Repeater {
        model: panel.status.lines

        // Where things stand, your other computers' names among it: plain text.
        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: panel.theme.text
        }
    }
    Repeater {
        model: panel.status.problems

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: panel.theme.warm
        }
    }
    // On a phone sharing while another phone shares: whether this one replaces it, asked once.
    Label {
        visible: !!panel.status.replace
        Layout.topMargin: 6
        Layout.fillWidth: true
        text: panel.status.replace ? panel.status.replace.ask : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.weight: Font.DemiBold
        color: panel.theme.text
    }
    Label {
        visible: !!panel.status.replace
        Layout.fillWidth: true
        text: panel.sioul.text("join-replace-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.muted
    }
    Flow {
        visible: !!panel.status.replace
        Layout.fillWidth: true
        spacing: 8

        Button {
            text: panel.sioul.text("join-replace-yes")
            onClicked: {
                panel.problem = panel.sioul.joinReplace(panel.status.replace.id, true)
                panel.reload()
            }
        }
        Button {
            flat: true
            text: panel.sioul.text("join-replace-no")
            onClicked: {
                panel.problem = panel.sioul.joinReplace(panel.status.replace.id, false)
                panel.reload()
            }
        }
    }
    // Not shared yet: joining through your Nextcloud's own sign-in (JoinPanel.qml);
    // its result stays to read once sharing is on.
    JoinPanel {
        id: joinPanel

        visible: !panel.status.on || joinPanel.busy
        Layout.topMargin: 6
        Layout.fillWidth: true
        sioul: panel.sioul
        theme: panel.theme
        onStepChanged: panel.reload()
    }
    // Android: the folder your sync app carries is read by its path, once Android allows it.
    Label {
        visible: panel.android && !panel.filesAccess && !panel.joining
        Layout.fillWidth: true
        text: panel.sioul.text("share-files-access")
        wrapMode: Text.Wrap
        color: panel.theme.text
    }
    Button {
        visible: panel.android && !panel.filesAccess && !panel.joining
        text: panel.sioul.text("share-files-allow")
        onClicked: panel.sioul.askFilesAccess()
    }
    // The folders your other devices already share through: one tap.
    Label {
        visible: panel.candidates.length > 0 && !panel.byServer && !panel.joining
        Layout.fillWidth: true
        text: panel.sioul.text("share-found")
        wrapMode: Text.Wrap
        color: panel.theme.muted
    }
    Repeater {
        model: panel.byServer || panel.joining ? [] : panel.candidates

        delegate: Button {
            required property string modelData

            Layout.fillWidth: true
            flat: true
            text: panel.theme.plain(modelData)
            onClicked: panel.choose(modelData)
        }
    }
    // Not shared yet: how, the folder, and the passphrase.
    ColumnLayout {
        visible: panel.setting
        Layout.fillWidth: true
        spacing: 0

        RadioButton {
            Layout.fillWidth: true
            checked: !panel.byServer
            text: panel.sioul.text("share-mode-folder")
            onToggled: panel.byServer = !checked
        }
        RadioButton {
            Layout.fillWidth: true
            checked: panel.byServer
            text: panel.sioul.text("share-mode-server")
            onToggled: panel.byServer = checked
        }
        Label {
            visible: panel.byServer
            Layout.fillWidth: true
            text: panel.sioul.text((panel.status.servers || []).length > 0 ? "share-server-help" : "share-server-none")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: panel.theme.muted
        }
    }
    GridLayout {
        visible: panel.setting
        Layout.fillWidth: true
        columns: 3
        columnSpacing: 8
        rowSpacing: 6

        Label {
            visible: !panel.byServer
            text: panel.sioul.text("share-folder")
            color: panel.theme.muted
        }
        TextField {
            id: folderField

            visible: !panel.byServer
            Layout.fillWidth: true
            placeholderText: panel.android ? "/storage/emulated/0/Documents/Sioul" : "~/Nextcloud/Sioul"
            onEditingFinished: panel.reload()
        }
        Button {
            visible: !panel.byServer
            text: panel.sioul.text("share-choose")
            onClicked: panel.android ? panel.browse() : folderPicker.open()
        }
        // Sioul keeping it itself: which account's server, and where among its files.
        Label {
            visible: panel.byServer
            text: panel.sioul.text("share-server-account")
            color: panel.theme.muted
        }
        PlainComboBox {
            id: accountBox

            visible: panel.byServer
            Layout.columnSpan: 2
            Layout.fillWidth: true
            model: (panel.status.servers || []).map(s => Object.assign({}, s, { label: panel.theme.plain(s.label) }))
            textRole: "label"
            valueRole: "id"
            Accessible.name: panel.sioul.text("share-server-account")
        }
        Label {
            visible: panel.byServer
            text: panel.sioul.text("share-server-place")
            color: panel.theme.muted
        }
        TextField {
            id: serverPlace

            visible: panel.byServer
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: "Documents/Sioul"
            Accessible.name: panel.sioul.text("share-server-place")
        }
        Label {
            text: panel.sioul.text("share-passphrase")
            color: panel.theme.muted
        }
        PasswordField {
            id: passphrase

            Layout.columnSpan: 2
            Layout.fillWidth: true
            sioul: panel.sioul
        }
        Label {
            visible: panel.byServer || !panel.status.sealed
            text: panel.sioul.text("share-again")
            color: panel.theme.muted
        }
        PasswordField {
            id: again

            visible: panel.byServer || !panel.status.sealed
            Layout.columnSpan: 2
            Layout.fillWidth: true
            sioul: panel.sioul
        }
        Label {
            Layout.columnSpan: 3
            Layout.fillWidth: true
            text: panel.sioul.text(panel.byServer ? "share-server-passphrase-hint" : panel.status.sealed ? "share-passphrase-known" : "share-passphrase-hint")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: panel.theme.muted
        }
    }
    Label {
        visible: panel.starting
        Layout.fillWidth: true
        text: panel.sioul.text("share-server-starting")
        wrapMode: Text.Wrap
        color: panel.theme.muted
    }
    Label {
        visible: panel.problem !== ""
        Layout.fillWidth: true
        text: panel.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: panel.theme.warm
    }
    RowLayout {
        spacing: 8

        Button {
            visible: panel.setting
            enabled: !panel.starting && (!panel.byServer || (panel.status.servers || []).length > 0)
            highlighted: true
            text: panel.sioul.text("share-start")
            onClicked: {
                // Sioul keeping it itself: the server asked off the window's thread, `shareStarted` brings the end.
                if (panel.byServer) {
                    panel.starting = true
                    panel.problem = ""
                    panel.sioul.startSharingOnServer(accountBox.currentValue || "", serverPlace.text, passphrase.text, again.text)
                    return
                }
                panel.problem = panel.sioul.startSharing(folderField.text, passphrase.text, again.text)
                if (panel.problem === "") {
                    passphrase.clear()
                    again.clear()
                }
                panel.reload()
            }
        }
        Button {
            visible: panel.status.on
            text: panel.sioul.text("share-now")
            onClicked: {
                panel.sioul.shareNow()
                panel.reload()
            }
        }
        Button {
            visible: panel.status.on && !panel.stopping
            flat: true
            text: panel.sioul.text("share-stop")
            onClicked: panel.stopping = true
        }
    }
    // What stopping does, then a word: nothing goes until it is said.
    Label {
        visible: panel.status.on && panel.stopping
        Layout.fillWidth: true
        text: panel.sioul.text("share-stop-ask")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: panel.theme.text
    }
    Flow {
        visible: panel.status.on && panel.stopping
        Layout.fillWidth: true
        spacing: 8

        Button {
            text: panel.sioul.text("share-stop-confirm")
            onClicked: {
                panel.stopping = false
                panel.problem = panel.sioul.stopSharing()
                panel.reload()
            }
        }
        Button {
            flat: true
            text: panel.sioul.text("ui-cancel")
            onClicked: panel.stopping = false
        }
    }
    // Your other devices, as this one knows them (docs/database.md, "Devices"):
    // in use, closed, silent, off as you said; when each last shared, in
    // words. Nothing red, no counts.
    Label {
        visible: panel.status.on && (panel.status.devices || []).length > 0
        Layout.topMargin: 12
        Layout.fillWidth: true
        text: panel.sioul.text("share-devices")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: panel.theme.text
    }
    Repeater {
        model: panel.status.on ? (panel.status.devices || []) : []

        delegate: ColumnLayout {
            id: device

            required property var modelData

            Layout.fillWidth: true
            spacing: 2

            Label {
                Layout.fillWidth: true
                text: device.modelData.name
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: panel.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: [device.modelData.state].concat(device.modelData.notes || []).join(" ")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            // The Sioul it runs, as its entry says: an older one said calmly.
            Label {
                visible: text !== ""
                Layout.fillWidth: true
                text: device.modelData.build || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            // A phone silent a day, on a computer: forgotten now, said first what it changes.
            Label {
                visible: device.modelData.forget_now === true
                Layout.fillWidth: true
                text: panel.sioul.text("share-device-forget-now-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            RowLayout {
                visible: device.modelData.off || device.modelData.forget || device.modelData.forget_now === true
                spacing: 6

                Button {
                    visible: device.modelData.off
                    flat: true
                    text: panel.sioul.text("share-device-count-again")
                    onClicked: {
                        panel.problem = panel.sioul.deviceOff(device.modelData.id, false)
                        panel.reload()
                    }
                }
                Button {
                    visible: device.modelData.forget
                    flat: true
                    text: panel.sioul.text("share-device-forget")
                    onClicked: {
                        panel.problem = panel.sioul.deviceForget(device.modelData.id)
                        panel.reload()
                    }
                }
                Button {
                    visible: device.modelData.forget_now === true
                    flat: true
                    text: panel.sioul.text("share-device-forget-now")
                    onClicked: {
                        panel.problem = panel.sioul.deviceForget(device.modelData.id)
                        panel.reload()
                    }
                }
            }
            // Its files taken out of the folder, forgotten first (docs/database.md, "Removing a device's files").
            Button {
                visible: panel.removing !== device.modelData.id
                flat: true
                text: panel.sioul.text("share-device-remove-live")
                onClicked: panel.askRemoval(device.modelData.id)
            }
            RemovalAsked {
                visible: panel.removing === device.modelData.id
                Layout.fillWidth: true
                host: panel
                deviceId: device.modelData.id
            }
        }
    }
    // The devices that no longer count here (they left, you forgot them,
    // another replaces them) whose files are still in the folder: each with
    // "Remove its files".
    Label {
        visible: panel.status.on && (panel.status.gone_devices || []).length > 0
        Layout.topMargin: 12
        Layout.fillWidth: true
        text: panel.sioul.text("share-devices-gone")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: panel.theme.text
    }
    Repeater {
        model: panel.status.on ? (panel.status.gone_devices || []) : []

        delegate: ColumnLayout {
            id: gone

            required property var modelData

            Layout.fillWidth: true
            spacing: 2

            Label {
                Layout.fillWidth: true
                text: gone.modelData.name
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: panel.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: gone.modelData.state
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            Button {
                visible: panel.removing !== gone.modelData.id
                flat: true
                text: panel.sioul.text("share-device-remove")
                onClicked: panel.askRemoval(gone.modelData.id)
            }
            RemovalAsked {
                visible: panel.removing === gone.modelData.id
                Layout.fillWidth: true
                host: panel
                deviceId: gone.modelData.id
            }
        }
    }
    // On a computer that shares: what a new device needs, in a few lines, on a press.
    Button {
        visible: (panel.status.add_device || []).length > 0
        Layout.topMargin: 6
        flat: true
        checkable: true
        checked: panel.adding
        text: panel.sioul.text("share-add-title")
        onToggled: panel.adding = checked
    }
    Repeater {
        model: panel.adding ? (panel.status.add_device || []) : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: panel.theme.text
        }
    }
    // This device's own build (docs/building.md, "Which build").
    Label {
        visible: (panel.status.build || "") !== ""
        Layout.topMargin: 6
        Layout.fillWidth: true
        text: panel.status.build || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.muted
    }
    // The other devices' files fetched from the server too (docs/database.md,
    // "Fetched from the server too"): where it stands, this device's switch,
    // and the folder's place there, given by hand when it is not found.
    RowLayout {
        visible: panel.backup.shown === true
        Layout.topMargin: 12
        Layout.fillWidth: true
        spacing: 8

        Switch {
            Layout.alignment: Qt.AlignTop
            checked: panel.backup.on === true
            Accessible.name: panel.sioul.text("share-backup-switch")
            onToggled: {
                panel.problem = panel.sioul.setShareBackup(checked)
                panel.reload()
                lookAgain.restart()
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            Label {
                Layout.fillWidth: true
                text: panel.sioul.text("share-backup-switch")
                wrapMode: Text.Wrap
                color: panel.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: panel.sioul.text("share-backup-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            // From where, and since when; or why not, in words.
            Label {
                visible: text !== ""
                Layout.fillWidth: true
                text: panel.backup.line || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.text
            }
        }
    }
    Label {
        visible: placeRow.visible
        Layout.fillWidth: true
        text: panel.sioul.text("share-backup-place")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.muted
    }
    RowLayout {
        id: placeRow

        visible: panel.backup.shown === true && panel.backup.on === true && (panel.backup.found !== true || (panel.backup.given || "") !== "")
        Layout.fillWidth: true
        spacing: 8

        TextField {
            id: placeField

            Layout.fillWidth: true
            placeholderText: "Documents/Sioul"
            Accessible.name: panel.sioul.text("share-backup-place")
            onAccepted: lookThere.clicked()
        }
        Button {
            id: lookThere

            text: panel.sioul.text("share-backup-look")
            onClicked: {
                panel.problem = panel.sioul.shareBackupPlace(placeField.text)
                panel.reload()
                lookAgain.restart()
            }
        }
    }
    // Looked for off the window's thread: said a few seconds later.
    Timer {
        id: lookAgain

        interval: 4000
        onTriggered: panel.reload()
    }
    // This device's own files sent to that server too, beside the sync app
    // (docs/database.md, "Sent to the server too"): the switch, and where it stands.
    RowLayout {
        visible: panel.backup.shown === true && panel.backup.on === true
        Layout.fillWidth: true
        spacing: 8

        Switch {
            Layout.alignment: Qt.AlignTop
            checked: panel.backup.send === true
            Accessible.name: panel.backup.send_switch || ""
            onToggled: {
                panel.problem = panel.sioul.setShareSend(checked)
                panel.reload()
                lookAgain.restart()
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            Label {
                Layout.fillWidth: true
                text: panel.backup.send_switch || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: panel.sioul.text("share-send-help")
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            Label {
                visible: text !== ""
                Layout.fillWidth: true
                text: panel.backup.send_line || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.text
            }
        }
    }
    // "Send everything again": for a sync app late, or a server that lost
    // files; what it did, said under it once done.
    RowLayout {
        visible: panel.status.can_again === true
        Layout.fillWidth: true
        spacing: 8

        Button {
            id: sendAgain

            text: panel.sioul.text("share-send-again")
            onClicked: panel.sendAgain()
        }
        Label {
            Layout.fillWidth: true
            text: panel.sending ? panel.sioul.text("share-send-again-sending") : panel.sioul.text("share-send-again-help")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: panel.theme.muted
        }
    }
    Label {
        visible: panel.status.can_again === true && text !== "" && !panel.sending
        Layout.fillWidth: true
        text: panel.status.again || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.text
    }
    // Sent off the window's thread, after any send running: read again every
    // three seconds until its answer comes, a minute at most.
    Timer {
        id: sentAgain

        property int tries: 0

        interval: 3000
        repeat: true
        onTriggered: {
            sentAgain.tries += 1
            panel.reload()
            if ((panel.status.again || "") !== panel.againBefore || sentAgain.tries >= 20) {
                panel.sending = false
                sentAgain.stop()
            }
        }
    }
    // What travels from this device: each part's switch, what it carries, when it last exchanged.
    Label {
        Layout.topMargin: 12
        Layout.fillWidth: true
        text: panel.sioul.text("share-parts")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: panel.theme.text
    }
    Label {
        Layout.fillWidth: true
        text: panel.sioul.text("share-parts-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.muted
    }
    Repeater {
        model: panel.status.parts || []

        delegate: RowLayout {
            id: part

            required property var modelData

            Layout.fillWidth: true
            spacing: 8

            Switch {
                Layout.alignment: Qt.AlignTop
                checked: part.modelData.on
                // Refused here (a sync app carries the notes folder): it can only be switched off.
                enabled: part.modelData.refused === "" || part.modelData.on
                Accessible.name: part.modelData.name
                onToggled: {
                    // Notes and papers: what would travel is said before they are switched on.
                    if (checked && (part.modelData.id === "notes" || part.modelData.id === "papers")) {
                        panel.asking = part.modelData.id
                        panel.estimate = ""
                        panel.sioul.shareEstimate(part.modelData.id)
                    } else {
                        panel.problem = panel.sioul.setSharePart(part.modelData.id, checked)
                    }
                    panel.reload()
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 1

                Label {
                    Layout.fillWidth: true
                    text: part.modelData.name
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: panel.theme.text
                }
                Label {
                    Layout.fillWidth: true
                    text: part.modelData.carries
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: panel.theme.muted
                }
                Label {
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: part.modelData.last
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: panel.theme.muted
                }
                Label {
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: part.modelData.refused
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: panel.theme.warm
                }
                // On a phone the refusal is a guess (Sioul cannot read the sync app's settings there): you say.
                Button {
                    visible: part.modelData.guessed === true
                    flat: true
                    text: panel.sioul.text("share-carried-wrong")
                    onClicked: {
                        panel.problem = panel.sioul.setNotesCarried(false)
                        panel.reload()
                    }
                }
                // Switching notes or papers on: how much would travel, then a word.
                Label {
                    visible: panel.asking === part.modelData.id
                    Layout.fillWidth: true
                    text: panel.estimate !== "" ? panel.estimate : panel.sioul.text("share-estimating")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: panel.theme.text
                }
                // On a phone, asked first: does a sync app there carry the notes folder?
                Label {
                    visible: panel.asking === part.modelData.id && panel.estimate !== "" && (panel.status.carried_ask || "") !== ""
                    Layout.fillWidth: true
                    text: panel.status.carried_ask || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: panel.theme.text
                }
                Flow {
                    visible: panel.asking === part.modelData.id && panel.estimate !== ""
                    Layout.fillWidth: true
                    spacing: 8

                    Button {
                        highlighted: true
                        text: panel.sioul.text((panel.status.carried_ask || "") !== "" ? "share-carried-not" : "share-switch-on")
                        onClicked: {
                            let problem = ""
                            if ((panel.status.carried_ask || "") !== "")
                                problem = panel.sioul.setNotesCarried(false)
                            if (problem === "")
                                problem = panel.sioul.setSharePart(part.modelData.id, true)
                            panel.problem = problem
                            panel.asking = ""
                            panel.reload()
                        }
                    }
                    Button {
                        visible: (panel.status.carried_ask || "") !== ""
                        text: panel.sioul.text("share-carried-yes")
                        onClicked: {
                            panel.problem = panel.sioul.setNotesCarried(true)
                            panel.asking = ""
                            panel.reload()
                        }
                    }
                    Button {
                        flat: true
                        text: panel.sioul.text("ui-cancel")
                        onClicked: panel.asking = ""
                    }
                }
            }
        }
    }
    // Notes or papers gone at once here: held, said, taken out everywhere on a word.
    Repeater {
        model: panel.status.vanished || []

        delegate: ColumnLayout {
            id: vanished

            required property var modelData

            Layout.fillWidth: true
            spacing: 4

            Label {
                Layout.fillWidth: true
                text: vanished.modelData.text
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.theme.warm
            }
            Button {
                text: panel.sioul.text("share-vanished-confirm")
                onClicked: {
                    panel.problem = panel.sioul.shareConfirmGone(vanished.modelData.store)
                    panel.reload()
                }
            }
        }
    }
    // Notes or papers gone many at once here, held until they leave your
    // other devices by themselves: said as long as it lasts, with what you
    // may do; nothing has to be pressed.
    Repeater {
        model: panel.status.withheld || []

        delegate: ColumnLayout {
            id: held

            required property var modelData

            Layout.fillWidth: true
            spacing: 4

            Label {
                Layout.fillWidth: true
                text: held.modelData.text
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: panel.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: held.modelData.leaves
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: panel.theme.muted
            }
            Flow {
                Layout.fillWidth: true
                spacing: 8

                Button {
                    text: panel.sioul.text("porch-withheld-put-back")
                    onClicked: {
                        panel.problem = panel.sioul.sharePutBackWithheld(held.modelData.store)
                        panel.reload()
                    }
                }
                Button {
                    text: panel.sioul.text("porch-withheld-take-out")
                    onClicked: {
                        panel.problem = panel.sioul.shareConfirmGone(held.modelData.store)
                        panel.reload()
                    }
                }
            }
        }
    }
    Label {
        Layout.fillWidth: true
        text: panel.sioul.text("share-not-shared")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: panel.theme.muted
    }

    // Earlier versions: what a file held before another device's change was
    // written into it, kept on this device; part, then file, then its versions.
    Button {
        Layout.topMargin: 6
        flat: true
        text: panel.sioul.text(panel.showHistory ? "share-history-hide" : "share-history-show")
        onClicked: {
            panel.showHistory = !panel.showHistory
            if (panel.showHistory)
                panel.listHistory()
        }
    }
    ColumnLayout {
        visible: panel.showHistory
        Layout.fillWidth: true
        spacing: 4

        Label {
            Layout.fillWidth: true
            text: panel.sioul.text("share-history-help")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: panel.theme.muted
        }
        // A part of a file's name: those that hold it.
        TextField {
            id: historyFilter

            Layout.fillWidth: true
            placeholderText: panel.sioul.text("share-history-filter")
            onTextEdited: filterTimer.restart()
        }
        Timer {
            id: filterTimer

            interval: 300
            onTriggered: panel.listHistory()
        }
        Label {
            visible: panel.history.length === 0
            Layout.fillWidth: true
            text: panel.sioul.text("share-history-empty")
            wrapMode: Text.Wrap
            color: panel.theme.muted
        }
        Repeater {
            model: panel.history

            delegate: ColumnLayout {
                id: keptPart

                required property var modelData

                Layout.fillWidth: true
                spacing: 2

                Label {
                    Layout.topMargin: 6
                    Layout.fillWidth: true
                    text: keptPart.modelData.name
                    textFormat: Text.PlainText
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    color: panel.theme.text
                }
                Label {
                    visible: text !== ""
                    Layout.fillWidth: true
                    text: keptPart.modelData.more
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 13
                    color: panel.theme.muted
                }
                Repeater {
                    model: keptPart.modelData.files

                    delegate: ColumnLayout {
                        id: keptFile

                        required property var modelData
                        readonly property string key: keptPart.modelData.part + "\u001f" + modelData.file

                        Layout.fillWidth: true
                        spacing: 2

                        // The file: its versions open beneath it.
                        ItemDelegate {
                            id: fileRow

                            Layout.fillWidth: true
                            text: keptFile.modelData.shown + "  ·  " + keptFile.modelData.count
                            Accessible.name: text
                            onClicked: panel.openFile = panel.openFile === keptFile.key ? "" : keptFile.key

                            contentItem: Label {
                                text: fileRow.text
                                textFormat: Text.PlainText
                                wrapMode: Text.WrapAnywhere
                                color: panel.theme.text
                            }
                        }
                        Repeater {
                            model: panel.openFile === keptFile.key ? keptFile.modelData.versions : []

                            delegate: ColumnLayout {
                                id: version

                                required property var modelData
                                readonly property bool asked: panel.confirming !== null && panel.confirming.stamp === modelData.stamp && panel.confirming.file === keptFile.modelData.file

                                Layout.fillWidth: true
                                Layout.leftMargin: 16
                                spacing: 4

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 8

                                    Label {
                                        Layout.fillWidth: true
                                        text: version.modelData.when + "  ·  " + version.modelData.size
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: panel.theme.text
                                    }
                                    Button {
                                        visible: !version.asked
                                        enabled: !panel.puttingBack
                                        text: panel.sioul.text("share-put-back")
                                        onClicked: panel.confirming = { part: keptPart.modelData.part, file: keptFile.modelData.file, stamp: version.modelData.stamp, text: panel.sioul.sharePutBackPreview(keptPart.modelData.part, keptFile.modelData.file, version.modelData.stamp) }
                                    }
                                }
                                // What putting it back changes, then a word.
                                Label {
                                    visible: version.asked
                                    Layout.fillWidth: true
                                    text: version.asked ? panel.confirming.text : ""
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: panel.theme.text
                                }
                                Flow {
                                    visible: version.asked
                                    Layout.fillWidth: true
                                    spacing: 8

                                    Button {
                                        highlighted: true
                                        enabled: !panel.puttingBack
                                        text: panel.sioul.text(panel.puttingBack ? "share-putting-back" : "share-put-back")
                                        onClicked: {
                                            panel.puttingBack = true
                                            panel.sioul.sharePutBack(panel.confirming.part, panel.confirming.file, panel.confirming.stamp)
                                        }
                                    }
                                    Button {
                                        flat: true
                                        enabled: !panel.puttingBack
                                        text: panel.sioul.text("ui-cancel")
                                        onClicked: panel.confirming = null
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // On Android, Sioul's own browser (FolderBrowser.qml): the system's picker
    // refuses the phone's storage. It opens in Documents, where Murena's eDrive
    // keeps what it syncs.
    function browse() {
        if (browser.item === null)
            browser.setSource("FolderBrowser.qml", { sioul: panel.sioul, theme: panel.theme })
        browser.item.begin(folderField.text !== "" ? folderField.text : "/storage/emulated/0/Documents")
    }

    Loader {
        id: browser
    }
    Connections {
        target: browser.item

        function onChosen(path) {
            panel.choose(path)
        }
    }

    FolderDialog {
        id: folderPicker

        // On Windows the address is file:///C:/…: its path is C:/…, not /C:/…;
        // on Android, a folder of the phone's storage (Theme.localPath).
        onAccepted: {
            const path = panel.theme.localPath(folderPicker.selectedFolder)
            if (path === "")
                panel.problem = panel.sioul.text("folder-not-on-device")
            else
                panel.choose(path)
        }
    }

    // Before "Remove its files": what goes, with its size, and what stays,
    // in plain words; then the press, or Cancel.
    component RemovalAsked: ColumnLayout {
        id: asked

        // The panel it is said in: an inline component sees no id of the file's.
        required property var host
        // The device's id.
        required property string deviceId

        spacing: 4

        Label {
            Layout.fillWidth: true
            text: asked.host.removal.text || ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: asked.host.theme.text
        }
        RowLayout {
            spacing: 6

            Button {
                text: asked.host.sioul.text("share-device-remove-confirm")
                onClicked: {
                    asked.host.problem = asked.host.sioul.deviceRemoveFiles(asked.deviceId)
                    asked.host.removing = ""
                    asked.host.reload()
                }
            }
            Button {
                flat: true
                text: asked.host.sioul.text("ui-cancel")
                onClicked: asked.host.removing = ""
            }
        }
    }
}
