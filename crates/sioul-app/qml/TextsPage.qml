// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Texts (« SMS »), on a computer (docs/texts.md): a page of the window, laid
// out as Mail is. Your phone's texts, read there and carried sealed by the
// sharing's part "Texts", and those you write here for your phone to send.
// On the left, the conversations, newest first, people by your address books'
// names, their last words, when, and "a draft waits" where an AI agent left
// one; over them, the search through every text's words. On the right, the
// conversation chosen, as a thread: the whole history in time order, each day
// said once, yours on the right and theirs on the left, quietly; a multimedia
// message's pictures shown and its other media opened or saved (each kept
// sealed here; the copies opened for the page deleted when it goes out of
// sight); a text deleted on your phone said so; what became of each text you
// sent, in words, never in red: waiting for your phone, sent at, delivered at
// (only when your carrier says so), not sent and why, expired, may not have
// been sent; and "Send again", which makes a new text. Under it, an AI
// agent's drafts, then writing, with how many texts it takes. Never sent from
// here: short numbers, several people at once, pictures. In a narrow window,
// one at a time, as Mail: the list, then the conversation, and Back. A phone
// has its own SMS app: no page there (main.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // texts.rs's `view`, as its text: compared before it is taken, so a list
    // that did not change keeps its place. {can, phone: {id, name, shared,
    // said}, conversations, said, never, empty}.
    property string listedText: ""
    readonly property var listed: page.listedText ? JSON.parse(page.listedText) : ({ can: false, conversations: [], said: "", never: "", empty: "" })
    readonly property var rows: page.listed.conversations || []
    // The conversation open, by its id ("+33199001234"), and texts.rs's
    // `conversation` for it: {id, title, messages, drafts, can_write, group}.
    property string openId: ""
    property string talkText: ""
    readonly property var talk: page.talkText ? JSON.parse(page.talkText) : null
    readonly property var messages: page.talk !== null ? page.talk.messages : []
    // The search through the texts' words, and what was said of the last action.
    property string query: ""
    property string said: ""
    property var counted: ({ parts: 0, said: "" })
    // The thread held at its end while its rows take their heights, until you scroll back.
    property bool atEnd: true
    // The list moved through with the keyboard: its ring shows; a click hides it.
    property bool keyed: false
    // In a narrow window, one at a time: the list, the conversation; Back goes the other way (main.qml).
    readonly property bool canGoBack: page.openId !== ""

    function back() {
        page.close()
    }

    function act(verb, args) {
        // Before the window hands the page its backend (its first visibility change): nothing to do.
        if (!page.sioul)
            return ({})
        return JSON.parse(page.sioul.texts(verb, JSON.stringify(args || {})) || "null") || ({})
    }

    // Read while shown, as Mail. What did not change keeps its place; a thread
    // read to its end stays at its end as a new text comes.
    function reload() {
        if (!page.sioul)
            return
        const listed = page.sioul.texts("view", JSON.stringify({ query: page.query }))
        if (listed !== page.listedText)
            page.listedText = listed
        if (page.openId === "") {
            page.talkText = ""
            return
        }
        const talk = page.sioul.texts("conversation", JSON.stringify({ id: page.openId }))
        if (talk === page.talkText)
            return
        page.atEnd = page.atEnd || page.talkText === "" || thread.atYEnd
        page.talkText = talk
        if (page.atEnd)
            Qt.callLater(() => thread.positionViewAtEnd())
    }

    // A conversation opened: from the list, the Porch's Text back, a person's
    // sheet (window.openTexts). Its newest text in view.
    function select(id) {
        if (id !== page.openId) {
            page.said = ""
            page.talkText = ""
            page.atEnd = true
            draft.text = ""
        }
        page.openId = id
        page.reload()
        list.currentIndex = page.rows.findIndex(r => r.id === id)
    }

    // Back to the list alone.
    function close() {
        page.openId = ""
        page.talkText = ""
        page.said = ""
        list.forceActiveFocus()
    }

    // The search through the texts' words (also the window's pictures).
    function search(words) {
        page.query = words
        page.reload()
    }

    // An AI agent's draft (the conversation's `drafts`, by its place): its words in the
    // box, the draft taken away; sending stays the Send button's.
    function useDraft(index) {
        const waiting = page.talk !== null ? (page.talk.drafts || []) : []
        if (index < 0 || index >= waiting.length)
            return
        draft.text = page.act("draft-use", { id: waiting[index].id }).body || draft.text
        page.reload()
    }

    // A media file opened for the page, saved where you choose.
    function save(part) {
        saving.part = part
        saving.open()
    }

    // For the window's pictures: the page as it is before texts are read on this computer.
    function showNotSetUp() {
        page.close()
        page.listedText = JSON.stringify({ can: false, said: page.sioul.text("texts-page-off"), conversations: [] })
    }

    // The thread a screen further back (the window's pictures, PageUp).
    function scrollBack() {
        page.atEnd = false
        thread.contentY = Math.max(thread.originY, thread.contentY - thread.height * 0.9)
    }

    // Out of sight: the plain copies of the media opened for the page deleted (each stays sealed here).
    onVisibleChanged: {
        if (page.visible)
            page.reload()
        else
            page.act("close", {})
    }
    Component.onCompleted: page.reload()

    // A new minute, news from your phone: read again, the draft kept.
    Connections {
        target: page.sioul

        function onPorchChanged() {
            if (page.visible)
                page.reload()
        }
    }

    // Searching waits for a pause in typing.
    Timer {
        id: searching

        interval: 300
        onTriggered: page.reload()
    }

    Shortcut {
        sequence: "Escape"
        enabled: page.visible && page.openId !== ""
        onActivated: page.close()
    }

    FileDialog {
        id: saving

        property var part: null

        fileMode: FileDialog.SaveFile
        onAccepted: {
            const answer = page.act("save-media", { hash: saving.part.hash, ct: saving.part.ct, path: saving.selectedFile.toString() })
            page.said = page.sioul.text(answer.saved ? "texts-saved" : "texts-not-saved")
        }
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // ---------------------------------------------------------------- the conversations
        ColumnLayout {
            id: listColumn

            visible: !(page.window.compact && page.openId !== "")
            Layout.fillHeight: true
            Layout.fillWidth: page.openId === ""
            Layout.minimumWidth: 0
            Layout.preferredWidth: page.openId === "" ? columns.width : Math.round((columns.width - columns.spacing) * 0.38)
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                ActionButton {
                    visible: page.openId !== ""
                    theme: page.theme
                    compact: true
                    iconName: "go-previous"
                    label: page.sioul.text("texts-page-back")
                    onClicked: page.close()
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 0

                    // In a narrow window, the window's own title says it.
                    Label {
                        visible: !page.window.compact
                        Layout.fillWidth: true
                        text: page.sioul.text("texts-page-title")
                        font.pixelSize: 19
                        elide: Text.ElideRight
                        color: page.theme.text
                    }
                    Label {
                        visible: page.listed.can && !!page.listed.phone && page.listed.phone.said !== ""
                        Layout.fillWidth: true
                        text: page.listed.can && page.listed.phone ? page.listed.phone.said : ""
                        textFormat: Text.PlainText
                        font.pixelSize: 12
                        elide: Text.ElideRight
                        color: page.theme.muted
                    }
                }
                SearchField {
                    visible: page.listed.can && !page.window.compact
                    // Narrower when the thread takes the room, as Mail's.
                    Layout.fillWidth: true
                    Layout.preferredWidth: 200
                    Layout.maximumWidth: 200
                    Layout.minimumWidth: 90
                }
            }

            // In a narrow window, the search on a line of its own: the title keeps the first.
            SearchField {
                visible: page.listed.can && page.window.compact
                Layout.fillWidth: true
            }

            // Not read on this computer yet: why, and where to turn it on.
            Label {
                visible: !page.listed.can && page.listedText !== ""
                Layout.fillWidth: true
                Layout.topMargin: page.theme.gap
                text: page.listed.said || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            // No conversation, or none holds the words searched.
            Label {
                visible: page.listed.can && page.rows.length === 0
                Layout.fillWidth: true
                Layout.topMargin: page.theme.gap
                text: page.listed.empty || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            ListView {
                id: list

                visible: page.listed.can && page.rows.length > 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 0
                activeFocusOnTab: true
                keyNavigationEnabled: true
                currentIndex: -1
                model: page.rows
                ScrollBar.vertical: ScrollBar {}
                Accessible.name: page.sioul.text("texts-page-title")
                Keys.onReturnPressed: list.openCurrent()
                Keys.onEnterPressed: list.openCurrent()
                Keys.onPressed: event => {
                    page.keyed = true
                    event.accepted = false
                }

                function openCurrent() {
                    if (list.currentIndex >= 0 && list.currentIndex < page.rows.length)
                        page.select(page.rows[list.currentIndex].id)
                }

                delegate: ItemDelegate {
                    id: row

                    required property var modelData
                    required property int index
                    readonly property bool focused: page.keyed && list.activeFocus && list.currentIndex === row.index

                    width: list.width - 12
                    highlighted: row.modelData.id === page.openId
                    // Its words for screen readers: the row has no text of its own.
                    Accessible.name: [row.modelData.title, row.modelData.last, row.modelData.when].filter(t => !!t).join(", ")
                    padding: 10
                    topInset: 1
                    bottomInset: 1
                    focusPolicy: Qt.NoFocus
                    onClicked: {
                        page.keyed = false
                        list.currentIndex = row.index
                        page.select(row.modelData.id)
                    }

                    background: Rectangle {
                        color: row.highlighted || row.hovered ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                        border.color: row.focused ? page.theme.focus : row.highlighted ? page.theme.line : "transparent"
                        border.width: row.focused ? 2 : 1
                    }

                    contentItem: ColumnLayout {
                        spacing: 3

                        RowLayout {
                            spacing: 8

                            // A person's own words are plain text: a "<" opens no tag, loads nothing.
                            Label {
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: row.modelData.title
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            Label {
                                text: row.modelData.when
                                textFormat: Text.PlainText
                                font.pixelSize: 13
                                color: page.theme.muted
                            }
                        }
                        // Their last words, on one line.
                        Label {
                            visible: row.modelData.last !== ""
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            text: String(row.modelData.last).replace(/\s+/g, " ")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            maximumLineCount: 1
                            color: page.theme.text
                        }
                        // A draft an AI agent wrote waits there, said once, quietly.
                        Label {
                            visible: (row.modelData.drafts || 0) > 0
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            text: page.sioul.text("texts-agent-draft-waits")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                    }
                }

                // What Android keeps to your messages app, said once, at the end of the list.
                footer: Label {
                    width: list.width - 12
                    topPadding: 12
                    leftPadding: 10
                    rightPadding: 10
                    text: page.listed.never || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: page.theme.muted
                }
            }
            // With no list, the rest of the column: what is said stays at its top.
            Item {
                visible: !list.visible
                Layout.fillHeight: true
            }
        }

        // ---------------------------------------------------------------- a conversation
        ColumnLayout {
            id: threadColumn

            visible: page.openId !== "" && page.talk !== null
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.62)
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: page.talk !== null ? page.talk.title : ""
                textFormat: Text.PlainText
                font.pixelSize: 17
                elide: Text.ElideRight
                color: page.theme.text
            }
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 1
                color: page.theme.line
            }

            ListView {
                id: thread

                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 6
                activeFocusOnTab: true
                model: page.messages
                ScrollBar.vertical: ScrollBar {}
                Accessible.name: page.talk !== null ? page.talk.title : ""
                // Held at its end as its rows take their heights, or the window its size; let go when you scroll back.
                onContentHeightChanged: {
                    if (page.atEnd)
                        Qt.callLater(thread.toEnd)
                }
                onHeightChanged: {
                    if (page.atEnd)
                        Qt.callLater(thread.toEnd)
                }

                // Once per turn of the event loop, however many rows changed meanwhile.
                function toEnd() {
                    if (page.atEnd)
                        thread.positionViewAtEnd()
                }
                onMovementEnded: page.atEnd = thread.atYEnd
                onFlickEnded: page.atEnd = thread.atYEnd
                // The keyboard reads back and forth: a line, a screen, the ends.
                Keys.onPressed: event => {
                    const last = thread.contentHeight + thread.originY - thread.height
                    Qt.callLater(() => page.atEnd = thread.atYEnd)
                    if (event.key === Qt.Key_Up) {
                        thread.contentY = Math.max(thread.originY, thread.contentY - 48)
                        event.accepted = true
                    } else if (event.key === Qt.Key_Down) {
                        thread.contentY = Math.max(thread.originY, Math.min(last, thread.contentY + 48))
                        event.accepted = true
                    } else if (event.key === Qt.Key_PageUp) {
                        page.scrollBack()
                        event.accepted = true
                    } else if (event.key === Qt.Key_PageDown) {
                        thread.contentY = Math.max(thread.originY, Math.min(last, thread.contentY + thread.height * 0.9))
                        event.accepted = true
                    } else if (event.key === Qt.Key_End) {
                        thread.positionViewAtEnd()
                        event.accepted = true
                    } else if (event.key === Qt.Key_Home) {
                        thread.positionViewAtBeginning()
                        event.accepted = true
                    }
                }

                delegate: ColumnLayout {
                    id: message

                    required property var modelData
                    required property int index
                    readonly property bool mine: message.modelData.direction === "out"
                    // Each day said once, over its first message.
                    readonly property bool newDay: message.index === 0 || page.messages[message.index - 1].day !== message.modelData.day

                    width: thread.width - 14
                    spacing: 6

                    RowLayout {
                        visible: message.newDay
                        Layout.fillWidth: true
                        Layout.topMargin: message.index > 0 ? 10 : 2
                        spacing: 10

                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 1
                            color: page.theme.line
                        }
                        Label {
                            text: message.modelData.day
                            textFormat: Text.PlainText
                            font.pixelSize: 12
                            color: page.theme.muted
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            Layout.preferredHeight: 1
                            color: page.theme.line
                        }
                    }

                    // The message, on its side: yours on the right, theirs on the left.
                    Item {
                        Layout.fillWidth: true
                        implicitHeight: block.implicitHeight

                        Rectangle {
                            id: block

                            x: message.mine ? parent.width - block.width : 0
                            // Rounded up, and a little more: a text as wide as its line never wraps its last word.
                            width: Math.min(parent.width * 0.78, Math.ceil(Math.max(from.implicitWidth, wordsWidth.implicitWidth, metaWidth.implicitWidth, media.implicitWidth, again.visible ? again.implicitWidth : 0)) + 26)
                            implicitHeight: column.implicitHeight + 16
                            radius: page.theme.radius
                            color: message.mine ? page.theme.button : page.theme.surface

                            // The words' and the line's widths unwrapped: a short text keeps to one line.
                            Label {
                                id: wordsWidth

                                visible: false
                                text: words.text
                                textFormat: Text.PlainText
                                font: words.font
                            }
                            Label {
                                id: metaWidth

                                visible: false
                                text: meta.text
                                textFormat: Text.PlainText
                                font: meta.font
                            }

                            ColumnLayout {
                                id: column

                                x: 12
                                y: 8
                                width: block.width - 24
                                spacing: 4

                                Label {
                                    id: from

                                    visible: message.modelData.from !== ""
                                    Layout.fillWidth: true
                                    text: message.modelData.from
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: 13
                                    font.weight: Font.DemiBold
                                    color: page.theme.muted
                                }
                                // A multimedia message's media: a picture here shown; the others opened or saved; one not here said why.
                                ColumnLayout {
                                    id: media

                                    visible: message.modelData.parts.length > 0
                                    Layout.fillWidth: true
                                    spacing: 4

                                    Repeater {
                                        model: message.modelData.parts

                                        delegate: ColumnLayout {
                                            id: shown

                                            required property var modelData
                                            readonly property bool here: shown.modelData.hash !== ""
                                            // A picture's copy for the page; the other media copied only when opened.
                                            readonly property string picture: shown.here && shown.modelData.kind === "picture" ? (page.act("media", { hash: shown.modelData.hash, ct: shown.modelData.ct }).url || "") : ""

                                            Layout.fillWidth: true
                                            spacing: 2

                                            // Read at the width it is drawn at, at most: a phone's photo
                                            // read whole held some 48 MB, drawn 320 pixels wide.
                                            // Its box set before it loads: a row whose height changed as its
                                            // picture came moved the thread, which made rows again, each
                                            // opening its picture again (the window froze on long threads).
                                            Image {
                                                id: photo

                                                visible: shown.here && shown.modelData.kind === "picture"
                                                Layout.preferredWidth: Math.min(320, column.width)
                                                Layout.preferredHeight: visible ? 240 : 0
                                                horizontalAlignment: Image.AlignLeft
                                                sourceSize.width: Math.ceil(320 * Screen.devicePixelRatio)
                                                source: shown.picture
                                                fillMode: Image.PreserveAspectFit
                                                asynchronous: true
                                                Accessible.name: shown.modelData.said
                                            }
                                            Label {
                                                Layout.fillWidth: true
                                                text: shown.modelData.said + (shown.modelData.name !== "" ? "  ·  " + shown.modelData.name : "")
                                                textFormat: Text.PlainText
                                                wrapMode: Text.Wrap
                                                font.pixelSize: 13
                                                color: page.theme.muted
                                            }
                                            RowLayout {
                                                visible: shown.here
                                                spacing: 4

                                                Button {
                                                    flat: true
                                                    text: page.sioul.text("texts-open")
                                                    onClicked: Qt.openUrlExternally(page.act("media", { hash: shown.modelData.hash, ct: shown.modelData.ct }).url || "")
                                                }
                                                Button {
                                                    flat: true
                                                    text: page.sioul.text("texts-save")
                                                    onClicked: page.save(shown.modelData)
                                                }
                                            }
                                        }
                                    }
                                }
                                Label {
                                    id: words

                                    visible: words.text !== ""
                                    Layout.fillWidth: true
                                    text: message.modelData.body !== "" ? message.modelData.body : (message.modelData.picture && message.modelData.parts.length === 0 ? page.sioul.text("phonemsgs-picture") : "")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: page.theme.readingSize
                                    color: page.theme.text
                                }
                                // Its time; for a text sent from here, what became of it, in words.
                                Label {
                                    id: meta

                                    Layout.fillWidth: true
                                    text: message.modelData.time + (message.modelData.said ? "  ·  " + message.modelData.said.text : "") + (message.modelData.deleted ? "  ·  " + page.sioul.text("texts-deleted-on-phone") : "")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 12
                                    horizontalAlignment: message.mine ? Text.AlignRight : Text.AlignLeft
                                    color: page.theme.muted
                                }
                                Button {
                                    id: again

                                    visible: !!message.modelData.said && message.modelData.said.again
                                    Layout.alignment: message.mine ? Qt.AlignRight : Qt.AlignLeft
                                    flat: true
                                    text: page.sioul.text("texts-again")
                                    onClicked: {
                                        const answer = page.act("again", { key: message.modelData.key })
                                        page.said = answer.said || ""
                                        page.reload()
                                        Qt.callLater(() => thread.positionViewAtEnd())
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Label {
                visible: page.said !== ""
                Layout.fillWidth: true
                text: page.said
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            Label {
                visible: page.talk !== null && !page.talk.can_write
                Layout.fillWidth: true
                text: page.sioul.text("texts-group-read-only")
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            Rectangle {
                visible: page.talk !== null && page.talk.can_write
                Layout.fillWidth: true
                Layout.preferredHeight: 1
                color: page.theme.line
            }
            // What an AI agent drafted here (docs/mcp.md, "Texts"): its words go in the
            // box with Use it, and sending stays yours; Discard takes it away. Never sent by itself.
            Repeater {
                model: page.talk !== null && page.talk.can_write ? (page.talk.drafts || []) : []

                delegate: ColumnLayout {
                    id: agentDraft

                    required property var modelData
                    required property int index

                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        text: page.sioul.text("texts-agent-draft") + "  ·  " + agentDraft.modelData.when
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: 13
                        color: page.theme.muted
                    }
                    Label {
                        Layout.fillWidth: true
                        text: agentDraft.modelData.body
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: page.theme.text
                    }
                    RowLayout {
                        spacing: 4

                        Button {
                            flat: true
                            text: page.sioul.text("texts-agent-draft-use")
                            onClicked: page.useDraft(agentDraft.index)
                        }
                        Button {
                            flat: true
                            text: page.sioul.text("texts-agent-draft-discard")
                            onClicked: {
                                page.act("draft-discard", { id: agentDraft.modelData.id })
                                page.reload()
                            }
                        }
                    }
                }
            }

            // Writing: the phone sends it, filed by Android in its own messages.
            RowLayout {
                visible: page.talk !== null && page.talk.can_write
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 8

                ScrollView {
                    Layout.fillWidth: true
                    Layout.minimumWidth: 0
                    Layout.maximumHeight: 140

                    TextArea {
                        id: draft

                        placeholderText: page.sioul.text("texts-write")
                        wrapMode: TextEdit.Wrap
                        Accessible.name: page.sioul.text("texts-write")
                        onTextChanged: page.counted = page.act("parts", { body: draft.text })

                        background: Rectangle {
                            color: page.theme.surface
                            radius: page.theme.radius
                            border.color: draft.activeFocus ? page.theme.focus : page.theme.line
                        }
                    }
                }
                ColumnLayout {
                    spacing: 2

                    Button {
                        enabled: draft.text.trim() !== ""
                        text: page.sioul.text("texts-send")
                        icon.name: "document-send"
                        icon.color: page.theme.text
                        onClicked: {
                            const answer = page.act("send", { to: page.talk.id, body: draft.text })
                            page.said = answer.said || ""
                            if (answer.shared)
                                draft.text = ""
                            page.reload()
                            Qt.callLater(() => thread.positionViewAtEnd())
                        }
                    }
                    Label {
                        Layout.maximumWidth: 160
                        text: page.counted.said || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 12
                        color: page.theme.muted
                    }
                }
            }
        }
    }

    // The search through the texts' words: in the title's row, or under it in a narrow window.
    component SearchField: TextField {
        id: searchField

        placeholderText: page.sioul.text("ui-search")
        text: page.query
        Accessible.name: page.sioul.text("ui-search")
        onTextEdited: {
            page.query = searchField.text
            searching.restart()
        }
    }
}
