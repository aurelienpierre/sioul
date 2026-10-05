// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The mail client, for when you choose to look: the accounts and their
// folders on the left, the main ones in view and the others folded; a folder's
// last two weeks in the middle, without previews, a small dot for the unread;
// a message on the right, the folders folding away while you read. No counts,
// no badges. Right click on a message for what is not in view.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string mailAccountsText: ""
    readonly property var accounts: page.mailAccountsText ? JSON.parse(page.mailAccountsText) : []

    function takeShown() {
        if (page.visible)
            page.draftsText = page.sioul.drafts
        if (page.visible)
            page.mailFolderText = page.sioul.mailFolder
        if (page.visible)
            page.mailAccountsText = page.sioul.mailAccounts
    }

    Connections {
        target: page.sioul

        function onDraftsChanged() {
            page.takeShown()
        }
        function onMailFolderChanged() {
            page.takeShown()
        }
        function onMailAccountsChanged() {
            page.takeShown()
        }
    }
    onVisibleChanged: page.takeShown()
    Component.onCompleted: page.takeShown()
    property string mailFolderText: ""
    readonly property var shown: page.mailFolderText ? JSON.parse(page.mailFolderText) : null
    property string draftsText: ""
    readonly property var drafts: page.draftsText ? JSON.parse(page.draftsText) : []
    property string account: ""
    property string folder: ""
    // Sioul's own drafts, instead of a folder.
    property bool draftsShown: false
    property bool all: false
    property string query: ""
    // The message being read, by its file, and its row.
    property string openKey: ""
    // Whether a message was opened since the page was made: its reader is made then.
    property bool readerMade: false
    onOpenKeyChanged: {
        if (page.openKey !== "")
            page.readerMade = true
    }
    // On a phone, one at a time: the folders (asked for), the list, the
    // message; Back goes the other way (main.qml).
    property bool foldersShown: false
    // Every account resting (quiet time) and none chosen: no list, the line saying so.
    readonly property bool allResting: page.account === "" && !page.draftsShown && page.accounts.length > 0 && page.accounts.every(a => a.resting)
    readonly property bool canGoBack: page.openKey !== "" || page.foldersShown
    function back() {
        if (page.openKey !== "")
            page.openKey = ""
        else
            page.foldersShown = false
    }
    readonly property var opened: findItem(page.openKey)
    // Accounts unfolded by hand; the first one is unfolded at first.
    property var unfolded: ({})
    property var moreShown: ({})
    // Messages chosen with Ctrl or Shift: acted on together, or dragged onto a folder.
    property var selected: ({})
    property int anchorIndex: -1
    readonly property var selectedKeys: Object.keys(page.selected)
    // Conversations unfolded, by their first message.
    property var threadsOpen: ({})
    // Messages being dragged onto a folder.
    property bool dragging: false
    property var dragKeys: []

    function rowShown(item) {
        return !item.member || page.threadsOpen[item.thread] === true
    }

    // A click: alone, it opens the message; with Ctrl, it adds it to the
    // selection or takes it out; with Shift, everything from the last one clicked.
    function clickRow(index, key, modifiers) {
        if (modifiers & Qt.ControlModifier) {
            const next = Object.assign({}, page.selected)
            if (next[key])
                delete next[key]
            else
                next[key] = true
            page.selected = next
            page.anchorIndex = index
            return
        }
        if ((modifiers & Qt.ShiftModifier) && page.anchorIndex >= 0 && page.shown) {
            const next = {}
            const items = page.shown.items
            for (let i = Math.min(page.anchorIndex, index); i <= Math.max(page.anchorIndex, index) && i < items.length; i++)
                if (page.rowShown(items[i]))
                    next[items[i].key] = true
            page.selected = next
            return
        }
        page.selected = ({})
        page.anchorIndex = index
        page.fromLink = false
        page.openKey = key
    }

    function selectAll() {
        const next = {}
        for (const item of page.shown ? page.shown.items : [])
            if (page.rowShown(item))
                next[item.key] = true
        page.selected = next
    }

    // What a drag carries: the selection when the row is in it, else the row alone.
    function startDrag(key) {
        page.dragKeys = page.selected[key] ? page.selectedKeys : [key]
        page.dragging = true
    }

    function endDrag() {
        if (!page.dragging)
            return
        ghost.Drag.drop()
        page.dragging = false
    }

    function dropOn(account, folder) {
        page.sioul.moveMessages(JSON.stringify(page.dragKeys), account, folder)
        page.selected = ({})
    }

    // For the window's tests: messages chosen by words of their subjects, as Ctrl+click would.
    function selectBySubjects(words) {
        const next = {}
        for (const item of page.shown ? page.shown.items : [])
            if (words.some(w => item.subject.indexOf(w) >= 0))
                next[item.key] = true
        page.selected = next
    }

    function moveSelection(account, folder) {
        page.dragKeys = page.selectedKeys
        page.dropOn(account, folder)
    }

    function unfoldConversations() {
        const next = {}
        for (const item of page.shown ? page.shown.items : [])
            if (item.size > 1)
                next[item.thread] = true
        page.threadsOpen = next
    }

    function actOnSelection(action) {
        page.sioul.actMany(JSON.stringify(page.selectedKeys), action)
        page.selected = ({})
    }

    // A message keeps the start of its file name when reading or flagging it renames it.
    function sameMessage(a, b) {
        return a !== "" && b !== "" && a.split(/[\/\\]/).pop().split(/[:!]/)[0] === b.split(/[\/\\]/).pop().split(/[:!]/)[0]
    }

    function findItem(key) {
        if (!key || !page.shown)
            return null
        for (const item of page.shown.items)
            if (page.sameMessage(item.key, key))
                return item
        return null
    }

    function openFolder(account, folder) {
        page.selected = ({})
        page.anchorIndex = -1
        page.fromLink = false
        page.account = account
        page.folder = folder
        page.draftsShown = false
        page.all = false
        page.query = ""
        page.openKey = ""
        page.foldersShown = false
        page.sioul.openFolder(account, folder, false, "")
    }

    // The first account open, unless you chose; in quiet time, the first of yours.
    function isUnfolded(index, id) {
        return page.unfolded[id] === undefined ? index === page.accounts.findIndex(a => !a.resting) : page.unfolded[id]
    }

    // What went wrong, in the status line; nothing when all went well.
    function say(line) {
        if (line !== "")
            page.sioul.status = line
    }

    function toggle(map, id, value) {
        const next = Object.assign({}, map)
        next[id] = value
        return next
    }

    // A message opened from a link: it stays open whatever folder is shown.
    property bool fromLink: false

    function openMessage(key) {
        page.fromLink = true
        page.openKey = key
    }

    // A message by its subject, for the window's tests.
    function openSubject(text) {
        for (const item of page.shown ? page.shown.items : [])
            if (item.subject.indexOf(text) >= 0) {
                page.openKey = item.key
                return
            }
    }

    // The first message of the folder, for the window's images.
    function openFirst() {
        if (page.shown && page.shown.items.length > 0)
            page.openKey = page.shown.items[0].key
    }

    // Once the bindings reading the accounts are done: the folder opened
    // changes what they read (a binding loop when done at once).
    onAccountsChanged: Qt.callLater(() => {
        const first = page.accounts.find(a => !a.resting)
        if (page.account === "" && first)
            page.openFolder(first.id, "INBOX")
    })

    // A message filed away leaves the folder: the reader closes with it.
    onShownChanged: {
        if (page.openKey !== "" && !page.fromLink && page.shown !== null && page.findItem(page.openKey) === null)
            page.openKey = ""
    }

    // Searching waits for a pause in typing.
    Timer {
        id: searching

        interval: 300
        onTriggered: page.sioul.openFolder(page.account, page.folder, page.all, page.query)
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // The accounts and their folders; folded away while a message is open.
        ScrollView {
            id: folderColumn

            visible: page.window.compact ? (page.foldersShown || page.allResting) && page.openKey === "" : page.openKey === "" || page.dragging
            Layout.fillHeight: true
            Layout.fillWidth: page.window.compact
            Layout.preferredWidth: 230
            contentWidth: availableWidth

            ColumnLayout {
                width: folderColumn.availableWidth
                spacing: 2

                // Drafts written here, kept until sent: only when there are some.
                ItemDelegate {
                    id: draftsRow

                    visible: page.drafts.length > 0
                    Layout.fillWidth: true
                    Layout.bottomMargin: 8
                    highlighted: page.draftsShown
                    onClicked: {
                        page.draftsShown = true
                        page.openKey = ""
                        page.foldersShown = false
                    }

                    contentItem: RowLayout {
                        spacing: 8

                        Icon {
                            iconName: "document-edit"
                        }
                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("ui-drafts-here")
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                    }
                }

                Repeater {
                    model: page.accounts

                    delegate: ColumnLayout {
                        id: accountBlock

                        required property var modelData
                        required property int index
                        readonly property bool open: page.isUnfolded(accountBlock.index, accountBlock.modelData.id)

                        Layout.fillWidth: true
                        spacing: 0

                        Button {
                            id: accountTitle

                            Layout.fillWidth: true
                            flat: true
                            onClicked: page.unfolded = page.toggle(page.unfolded, accountBlock.modelData.id, !accountBlock.open)

                            // Dragging over a folded account unfolds it.
                            DropArea {
                                anchors.fill: parent
                                keys: ["sioul-mail"]
                                onEntered: {
                                    if (!accountBlock.open)
                                        page.unfolded = page.toggle(page.unfolded, accountBlock.modelData.id, true)
                                }
                            }

                            contentItem: RowLayout {
                                spacing: 6

                                Label {
                                    text: accountBlock.open ? "▾" : "▸"
                                    color: page.theme.muted
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: accountBlock.modelData.title
                                    textFormat: Text.PlainText
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                    // Work mail in quiet time: there if you look for it.
                                    color: accountBlock.modelData.resting ? page.theme.muted : page.theme.text
                                }
                                // A new folder on its server: its name asked in a small dialog.
                                ToolButton {
                                    visible: accountBlock.open
                                    implicitWidth: 28
                                    implicitHeight: 28
                                    icon.name: "folder-new"
                                    icon.color: page.theme.text
                                    display: AbstractButton.IconOnly
                                    Accessible.name: page.sioul.text("folder-new-button")
                                    ToolTip.visible: hovered
                                    ToolTip.text: page.sioul.text("folder-new-button")
                                    ToolTip.delay: 600
                                    onClicked: newFolderDialog.now().ask(accountBlock.modelData.id, accountBlock.modelData.title)
                                }
                                // Something unread in its inbox, said without a number.
                                Rectangle {
                                    visible: !accountBlock.open && accountBlock.modelData.unread
                                    Layout.preferredWidth: 7
                                    Layout.preferredHeight: 7
                                    radius: 3.5
                                    color: page.theme.accent
                                }
                            }
                        }

                        Repeater {
                            model: accountBlock.open ? accountBlock.modelData.folders.concat(page.moreShown[accountBlock.modelData.id] ? accountBlock.modelData.more : []) : []

                            delegate: ItemDelegate {
                                id: folderRow

                                required property var modelData

                                Layout.fillWidth: true
                                leftPadding: 22
                                highlighted: !page.draftsShown && page.account === accountBlock.modelData.id && page.folder === folderRow.modelData.name
                                Accessible.name: folderRow.modelData.title
                                ToolTip.visible: hovered && !folderRow.modelData.kept
                                ToolTip.text: page.sioul.text("folder-server-only")
                                ToolTip.delay: 600
                                onClicked: page.openFolder(accountBlock.modelData.id, folderRow.modelData.name)

                                // Right click: kept here or on the server only; an empty folder of yours deleted.
                                TapHandler {
                                    acceptedButtons: Qt.RightButton
                                    onTapped: folderMenu.now().show(accountBlock.modelData.id, folderRow.modelData)
                                }

                                background: Rectangle {
                                    color: dropHere.containsDrag ? page.theme.hover : folderRow.highlighted ? page.theme.surface : folderRow.hovered ? page.theme.button : "transparent"
                                    radius: page.theme.radius
                                    border.color: dropHere.containsDrag ? page.theme.accent : folderRow.visualFocus ? page.theme.focus : "transparent"
                                    border.width: dropHere.containsDrag ? 2 : 1
                                }

                                // Messages dropped here move here, from this account or another.
                                DropArea {
                                    id: dropHere

                                    anchors.fill: parent
                                    keys: ["sioul-mail"]
                                    onDropped: drop => {
                                        drop.accept()
                                        page.dropOn(accountBlock.modelData.id, folderRow.modelData.name)
                                    }
                                }

                                contentItem: RowLayout {
                                    spacing: 8

                                    Icon {
                                        iconName: folderRow.modelData.icon
                                        size: 16
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: folderRow.modelData.title
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: folderRow.modelData.kept ? page.theme.text : page.theme.muted
                                    }
                                    Rectangle {
                                        visible: folderRow.modelData.unread
                                        Layout.preferredWidth: 7
                                        Layout.preferredHeight: 7
                                        radius: 3.5
                                        color: page.theme.accent
                                    }
                                }
                            }
                        }

                        Button {
                            visible: accountBlock.open && accountBlock.modelData.more.length > 0
                            Layout.fillWidth: true
                            leftPadding: 22
                            flat: true
                            font.pixelSize: 13
                            text: (page.moreShown[accountBlock.modelData.id] ? "▾  " : "▸  ") + page.sioul.text("ui-more-folders")
                            onClicked: page.moreShown = page.toggle(page.moreShown, accountBlock.modelData.id, !page.moreShown[accountBlock.modelData.id])
                        }
                    }
                }

                Label {
                    visible: page.accounts.length === 0
                    Layout.fillWidth: true
                    text: page.sioul.text("ui-no-mail-accounts")
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
            }
        }

        // Quiet time, and only work addresses: nothing opens by itself.
        Label {
            // On a phone, the folders take its place: an account can still be opened.
            visible: page.allResting && !page.window.compact
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
            Layout.topMargin: page.theme.gap
            text: page.sioul.text(page.window.moment.rest === true ? "mail-rest" : "mail-resting")
            wrapMode: Text.Wrap
            color: page.theme.muted
        }

        // The folder's messages, or the drafts.
        ColumnLayout {
            id: listColumn

            visible: !page.allResting && !(page.window.compact && (page.openKey !== "" || page.foldersShown))
            Layout.fillHeight: true
            Layout.fillWidth: page.openKey === ""
            Layout.minimumWidth: 0
            Layout.preferredWidth: page.openKey === "" ? columns.width : Math.round((columns.width - columns.spacing) * 0.38)
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                ActionButton {
                    visible: page.openKey !== ""
                    theme: page.theme
                    compact: true
                    iconName: "go-previous"
                    label: page.sioul.text("ui-folders")
                    onClicked: page.openKey = ""
                }
                // A phone shows the folders on their own, when asked.
                ActionButton {
                    visible: page.window.compact && page.openKey === ""
                    theme: page.theme
                    compact: true
                    iconName: "folder-mail"
                    label: page.sioul.text("ui-folders")
                    onClicked: page.foldersShown = true
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    Label {
                        Layout.fillWidth: true
                        text: page.draftsShown ? page.sioul.text("ui-drafts-here") : page.shown ? page.shown.title : ""
                        textFormat: Text.PlainText
                        font.pixelSize: 19
                        elide: Text.ElideRight
                        color: page.theme.text
                    }
                    Label {
                        visible: !page.draftsShown
                        Layout.fillWidth: true
                        text: page.account
                        font.pixelSize: 12
                        elide: Text.ElideRight
                        color: page.theme.muted
                    }
                }
                SearchField {
                    visible: !page.draftsShown && !page.window.compact
                    // Narrower when the reader takes the room: a width of its own
                    // would push the row past the column, under the reader.
                    Layout.fillWidth: true
                    Layout.preferredWidth: 200
                    Layout.maximumWidth: 200
                    Layout.minimumWidth: 90
                }
                CheckBox {
                    id: realtime

                    text: page.sioul.text("ui-realtime")
                    checked: page.sioul.realtime
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("ui-realtime-help")
                    ToolTip.delay: 400
                    onToggled: {
                        page.sioul.setRealtimeMode(checked)
                        realtime.checked = Qt.binding(() => page.sioul.realtime)
                    }
                }
                SettingsButton {
                    sioul: page.sioul
                    theme: page.theme
                    view: "mail"
                }
            }

            // On a phone, the search on a line of its own: the folder's name keeps the first.
            SearchField {
                visible: !page.draftsShown && page.window.compact
                Layout.fillWidth: true
            }
            Label {
                visible: !page.draftsShown && page.shown !== null && page.shown.sentence !== ""
                Layout.fillWidth: true
                text: page.shown ? page.shown.sentence : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            // What is chosen, and what can be done to all of it at once.
            RowLayout {
                visible: !page.draftsShown && page.selectedKeys.length > 0
                Layout.fillWidth: true
                spacing: 6

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.textWith("ui-selected", "n", String(page.selectedKeys.length))
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                Button {
                    text: page.sioul.text("ui-mark-read")
                    onClicked: page.actOnSelection("read")
                }
                Button {
                    visible: page.shown !== null && page.shown.role !== "archive"
                    text: page.sioul.text("ui-archive")
                    onClicked: page.actOnSelection("archive")
                }
                Button {
                    text: page.shown && page.shown.role === "trash" ? page.sioul.text("ui-delete-for-good") : page.sioul.text("ui-trash")
                    onClicked: page.actOnSelection("trash")
                }
                Button {
                    text: page.sioul.text("ui-move-to")
                    onClicked: {
                        page.dragKeys = page.selectedKeys
                        moveDialog.now().open()
                    }
                }
                ToolButton {
                    text: "×"
                    Accessible.name: page.sioul.text("ui-clear-selection")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("ui-clear-selection")
                    onClicked: page.selected = ({})
                }
            }

            ListView {
                id: list

                visible: !page.draftsShown
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 0
                model: page.shown ? page.shown.items : []
                ScrollBar.vertical: ScrollBar {}
                Keys.onPressed: event => {
                    if (event.matches(StandardKey.SelectAll)) {
                        page.selectAll()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Escape && page.selectedKeys.length > 0) {
                        page.selected = ({})
                        event.accepted = true
                    }
                }

                delegate: ItemDelegate {
                    id: row

                    required property var modelData
                    required property int index
                    readonly property bool chosen: page.selected[row.modelData.key] === true
                    readonly property bool threadOpen: page.threadsOpen[row.modelData.thread] === true

                    // Folded inside its conversation: no room taken.
                    visible: page.rowShown(row.modelData)
                    height: visible ? implicitHeight : 0
                    width: list.width - 12
                    highlighted: page.sameMessage(page.openKey, row.modelData.key) || row.chosen
                    // Its words for screen readers: the row has no text of its own.
                    Accessible.name: [row.modelData.who, row.modelData.subject, row.modelData.date].filter(t => !!t).join(", ")
                    padding: 10
                    leftPadding: row.modelData.member ? 40 : 10
                    topInset: 1
                    bottomInset: 1
                    Keys.onReturnPressed: page.clickRow(row.index, row.modelData.key, 0)
                    Keys.onEnterPressed: page.clickRow(row.index, row.modelData.key, 0)

                    background: Rectangle {
                        color: row.chosen ? page.theme.hover : row.highlighted || row.hovered ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                        border.color: row.visualFocus ? page.theme.focus : row.chosen ? page.theme.accent : row.highlighted ? page.theme.line : "transparent"
                        border.width: row.visualFocus ? 2 : 1
                    }

                    TapHandler {
                        acceptedButtons: Qt.LeftButton
                        onTapped: page.clickRow(row.index, row.modelData.key, point.modifiers)
                    }
                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        onTapped: rowMenu.now().show(row.modelData)
                    }
                    // Dragged sideways, towards the folders: the row, or the selection it is in.
                    // With a mouse only: on a touch screen a drag scrolls the list.
                    DragHandler {
                        id: dragger

                        target: null
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        grabPermissions: PointerHandler.CanTakeOverFromAnything
                        onActiveChanged: {
                            if (dragger.active)
                                page.startDrag(row.modelData.key)
                            else
                                page.endDrag()
                        }
                        onCentroidChanged: {
                            if (dragger.active) {
                                const at = row.mapToItem(page, dragger.centroid.position.x, dragger.centroid.position.y)
                                ghost.x = at.x + 12
                                ghost.y = at.y + 12
                            }
                        }
                    }

                    contentItem: ColumnLayout {
                        spacing: 3

                        RowLayout {
                            spacing: 8

                            // A conversation: its size, and the arrow that unfolds it.
                            ToolButton {
                                visible: row.modelData.size > 1
                                Layout.preferredHeight: 22
                                Layout.preferredWidth: implicitContentWidth + 10
                                padding: 2
                                text: (row.threadOpen ? "▾ " : "▸ ") + row.modelData.size
                                font.pixelSize: 12
                                Accessible.name: page.sioul.textWith("ui-conversation", "n", String(row.modelData.size))
                                ToolTip.visible: hovered
                                ToolTip.text: page.sioul.textWith("ui-conversation", "n", String(row.modelData.size))
                                ToolTip.delay: 400
                                onClicked: page.threadsOpen = page.toggle(page.threadsOpen, row.modelData.thread, !row.threadOpen)
                            }
                            // Unread: a small dot, not bold.
                            Rectangle {
                                Layout.preferredWidth: 7
                                Layout.preferredHeight: 7
                                radius: 3.5
                                color: row.modelData.unread ? page.theme.accent : "transparent"
                            }
                            Icon {
                                visible: row.modelData.trust_level !== "own"
                                iconName: row.modelData.trust_level === "verified" ? "security-high" : row.modelData.trust_level === "forged" ? "security-low" : "security-medium"
                                size: 16
                                tip: row.modelData.checks
                            }
                            // A message's own words are plain text: a "<" opens no tag, loads nothing.
                            Label {
                                Layout.fillWidth: true
                                text: row.modelData.who
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: row.modelData.trust_level === "own" && row.modelData.member ? page.theme.muted : page.theme.text
                            }
                            Icon {
                                visible: row.modelData.flagged
                                iconName: "mail-flag"
                                size: 16
                            }
                            Icon {
                                visible: row.modelData.answered
                                iconName: "mail-replied"
                                size: 16
                            }
                            Icon {
                                visible: row.modelData.attachments
                                iconName: "mail-attachment"
                                size: 16
                            }
                            Label {
                                text: row.modelData.date
                                color: page.theme.muted
                                font.pixelSize: 13
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            Layout.leftMargin: 15
                            text: row.modelData.subject
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                    }
                }

                footer: Button {
                    visible: page.shown !== null && page.shown.earlier
                    height: visible ? implicitHeight + 16 : 0
                    flat: true
                    text: page.sioul.text("ui-earlier")
                    onClicked: {
                        page.all = true
                        page.sioul.openFolder(page.account, page.folder, true, page.query)
                    }
                }
            }

            // Drafts written here: a click opens one again.
            ListView {
                id: draftList

                visible: page.draftsShown
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 2
                model: page.drafts

                delegate: ItemDelegate {
                    id: draftRow

                    required property var modelData

                    width: draftList.width - 12
                    padding: 10
                    Accessible.name: [draftRow.modelData.to, draftRow.modelData.subject].filter(t => !!t).join(", ")
                    onClicked: page.window.openDraft(draftRow.modelData.id)

                    background: Rectangle {
                        color: draftRow.hovered ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                        border.color: draftRow.visualFocus ? page.theme.focus : "transparent"
                    }

                    contentItem: ColumnLayout {
                        spacing: 3

                        RowLayout {
                            spacing: 8

                            Label {
                                Layout.fillWidth: true
                                text: draftRow.modelData.to
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            Label {
                                text: draftRow.modelData.date
                                color: page.theme.muted
                                font.pixelSize: 13
                            }
                            Button {
                                flat: true
                                text: page.sioul.text("ui-discard")
                                onClicked: page.sioul.discard(draftRow.modelData.id)
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: draftRow.modelData.subject
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: page.theme.text
                        }
                    }
                }
            }
        }

        // The reader is made with the first message opened, then kept.
        Loader {
            id: readerLoader

            active: page.readerMade
            visible: page.openKey !== "" && readerLoader.item !== null && readerLoader.item.reading !== null
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.62)

            sourceComponent: Component {
                Reader {
                    id: reader

                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    key: page.openKey
                    item: page.opened
                    onCloseRequested: page.openKey = ""
                }
            }
        }
    }

    // What is being dragged, under the pointer.
    Rectangle {
        id: ghost

        visible: page.dragging
        z: 10
        width: ghostLabel.implicitWidth + 24
        height: ghostLabel.implicitHeight + 12
        radius: page.theme.radius
        color: page.theme.accent
        Drag.active: page.dragging
        Drag.keys: ["sioul-mail"]
        Drag.hotSpot.x: 0
        Drag.hotSpot.y: 0

        Label {
            id: ghostLabel

            anchors.centerIn: parent
            text: page.sioul.textWith("ui-drag-count", "n", String(page.dragKeys.length))
            color: page.theme.accentText
        }
    }

    Later {
        id: moveDialog

        sourceComponent: Component {
            MoveDialog {
                id: moveDialogForm

                sioul: page.sioul
                theme: page.theme
                fromAccount: page.account
                fromFolder: page.folder
                onChosen: (account, folder) => page.dropOn(account, folder)
            }
        }
    }

    // Right click on a message: what is not in view.
    // A folder: kept here or on the server only; an empty folder of yours, deleted.
    Later {
        id: folderMenu

        sourceComponent: Component {
            SioulMenu {
                id: folderMenuForm

                property string account: ""
                property var folder: null

                function show(account, folder) {
                    folderMenuForm.account = account
                    folderMenuForm.folder = folder
                    folderMenuForm.popup()
                }

                MenuItem {
                    id: keepItem

                    enabled: folderMenuForm.folder !== null && folderMenuForm.folder.role !== "inbox"
                    text: page.sioul.text("folder-keep")
                    checkable: true
                    checked: folderMenuForm.folder !== null && folderMenuForm.folder.kept
                    onTriggered: {
                        // What the folder is now decides, not the tick the click left; the tick
                        // then says it again (forgetting it may still be cancelled).
                        if (!folderMenuForm.folder.kept)
                            page.say(page.sioul.keepFolder(folderMenuForm.account, folderMenuForm.folder.name, true))
                        else
                            forgetDialog.now().ask(folderMenuForm.account, folderMenuForm.folder)
                        keepItem.checked = Qt.binding(() => folderMenuForm.folder !== null && folderMenuForm.folder.kept)
                    }
                }
                MenuItem {
                    enabled: folderMenuForm.folder !== null && folderMenuForm.folder.role === "other"
                    text: page.sioul.text("folder-delete")
                    onTriggered: deleteDialog.now().ask(folderMenuForm.account, folderMenuForm.folder)
                }
            }
        }
    }

    // Not kept here: the copy goes, the server keeps everything.
    Later {
        id: forgetDialog

        sourceComponent: Component {
            Dialog {
                id: forgetDialogForm

                property string account: ""
                property var folder: null

                function ask(account, folder) {
                    forgetDialogForm.account = account
                    forgetDialogForm.folder = folder
                    forgetDialogForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? parent.width : 440) - 2 * page.theme.gap)
                title: forgetDialogForm.folder ? page.theme.plain(forgetDialogForm.folder.title) : ""

                contentItem: Label {
                    text: page.sioul.text("folder-forget-ask")
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }

                footer: DialogButtonBox {
                    Button {
                        text: page.sioul.text("folder-forget")
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        highlighted: true
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }

                onAccepted: page.say(page.sioul.keepFolder(forgetDialogForm.account, forgetDialogForm.folder.name, false))
            }
        }
    }

    // A new folder on an account's server: its name.
    Later {
        id: newFolderDialog

        sourceComponent: Component {
            Dialog {
                id: newFolderDialogForm

                property string account: ""
                property string accountTitle: ""

                function ask(account, title) {
                    newFolderDialogForm.account = account
                    newFolderDialogForm.accountTitle = title
                    newFolderName.text = ""
                    newFolderDialogForm.open()
                    newFolderName.forceActiveFocus()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? parent.width : 440) - 2 * page.theme.gap)
                title: page.sioul.textWith("folder-new-title", "account", newFolderDialogForm.accountTitle)

                contentItem: TextField {
                    id: newFolderName

                    placeholderText: page.sioul.text("folder-new")
                    onAccepted: newFolderDialogForm.accept()
                }

                footer: DialogButtonBox {
                    Button {
                        text: page.sioul.text("folder-new-make")
                        highlighted: true
                        enabled: newFolderName.text.trim() !== ""
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }

                onAccepted: {
                    if (newFolderName.text.trim() !== "")
                        page.sioul.createFolder(newFolderDialogForm.account, newFolderName.text.trim())
                }
            }
        }
    }

    // An empty folder of yours, taken off the server.
    Later {
        id: deleteDialog

        sourceComponent: Component {
            Dialog {
                id: deleteDialogForm

                property string account: ""
                property var folder: null

                function ask(account, folder) {
                    deleteDialogForm.account = account
                    deleteDialogForm.folder = folder
                    deleteDialogForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, (parent ? parent.width : 440) - 2 * page.theme.gap)
                title: deleteDialogForm.folder ? page.theme.plain(deleteDialogForm.folder.title) : ""

                contentItem: Label {
                    text: page.sioul.text("folder-delete-ask")
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }

                footer: DialogButtonBox {
                    Button {
                        text: page.sioul.text("folder-delete")
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        highlighted: true
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }

                onAccepted: page.sioul.deleteFolder(deleteDialogForm.account, deleteDialogForm.folder.name)
            }
        }
    }

    Later {
        id: rowMenu

        sourceComponent: Component {
            SioulMenu {
                id: rowMenuForm

                property var target: null
                property var source: null

                function show(item) {
                    rowMenuForm.target = item
                    rowMenuForm.source = { uri: page.sioul.uriOf("mail", item.key), kind: "mail", key: item.key, title: item.subject, name: item.who, address: item.address, known: false }
                    rowMenuForm.popup()
                }

                MenuItem {
                    text: page.sioul.text("ui-reply")
                    onTriggered: page.window.compose("reply", rowMenuForm.target.key)
                }
                MenuItem {
                    text: page.sioul.text("ui-forward")
                    onTriggered: page.window.compose("forward", rowMenuForm.target.key)
                }
                MenuSeparator {}
                MenuItem {
                    text: rowMenuForm.target && rowMenuForm.target.unread ? page.sioul.text("ui-mark-read") : page.sioul.text("ui-mark-unread")
                    onTriggered: page.sioul.act(rowMenuForm.target.key, rowMenuForm.target.unread ? "read" : "unread", "")
                }
                MenuItem {
                    text: rowMenuForm.target && rowMenuForm.target.flagged ? page.sioul.text("ui-unflag") : page.sioul.text("ui-flag")
                    onTriggered: page.sioul.act(rowMenuForm.target.key, rowMenuForm.target.flagged ? "unflag" : "flag", "")
                }
                MenuSeparator {}
                MenuItem {
                    visible: page.shown !== null && page.shown.role !== "archive"
                    height: visible ? implicitHeight : 0
                    text: page.sioul.text("ui-archive")
                    onTriggered: page.sioul.act(rowMenuForm.target.key, "archive", "")
                }
                MenuItem {
                    text: page.shown && page.shown.role === "trash" ? page.sioul.text("ui-delete-for-good") : page.sioul.text("ui-trash")
                    onTriggered: page.sioul.act(rowMenuForm.target.key, "trash", "")
                }
                MenuItem {
                    text: page.shown && page.shown.role === "junk" ? page.sioul.text("ui-not-junk") : page.sioul.text("ui-junk")
                    onTriggered: page.sioul.act(rowMenuForm.target.key, page.shown && page.shown.role === "junk" ? "not-junk" : "junk", "")
                }
                MenuItem {
                    text: page.sioul.text("ui-move-to")
                    onTriggered: {
                        page.dragKeys = page.selected[rowMenuForm.target.key] ? page.selectedKeys : [rowMenuForm.target.key]
                        moveDialog.now().open()
                    }
                }
                MenuSeparator {}
                AddMenu {
                    sioul: page.sioul
                    window: page.window
                    source: rowMenuForm.source
                }
                MenuItem {
                    enabled: rowMenuForm.source !== null && rowMenuForm.source.uri !== ""
                    text: page.sioul.text("ui-link-existing")
                    onTriggered: page.window.linkFrom(rowMenuForm.source)
                }
            }
        }
    }

    // The folder's search: in the title's row, or under it on a phone.
    component SearchField: TextField {
        id: searchField

        placeholderText: page.sioul.text("ui-search")
        text: page.query
        onTextEdited: {
            page.query = searchField.text
            searching.restart()
        }
    }
}
