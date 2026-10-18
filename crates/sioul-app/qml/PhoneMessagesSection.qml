// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "From your phone", on a computer's Porch (docs/porch.md, "From your phone";
// docs/android.md, "Messages on your computers"): the messages your phone's
// notifications brought, for the apps you chose there, once the phone let
// them through and while their sender may reach you; never counted, no
// badge, never a notification here. One line per conversation and part of
// the day: who, when, the words (or "a picture", or only who and when); a
// code said, never shown. What you may do: reply, through the
// notification's own Reply on your phone while the phone says it works
// (replies.rs; a text from a number goes as one of Sioul's own texts where
// this computer reads them), your reply then under their messages with its
// state in words, Take back while it waits; else "Reply on your phone", or
// text back or call back (the system's app for such links, KDE Connect
// handing them to your phone; else copy the number); and "Seen", which
// takes the line away on every device, with ten seconds to undo, and never
// touches the phone's notification; the rest in the line's menu (⋮, a right
// click, a long press, the Menu key or Shift+F10): Mark as read in the app,
// with its warning, how they reach you, add to your contacts, block.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window

    // phonemsgs.rs's `view`: {lines: [{ids, head, said: [{time, from, text}],
    // note, number, dial, known, texted, code, who, answer}], links: {tel, sms}};
    // `answer` (replies.rs's `Answer`): {phone, notice, app, label, line,
    // offered, read, texts, placeholder, note, read_item, read_warning,
    // on_phone, limit, mine: [{key, time, text, said, waiting, again, texts}]}.
    property var shown: ({ lines: [], links: { tel: false, sms: false }, texts: false })
    property string said: ""
    // The last Seen pressed here (its time), undone within ten seconds.
    property real undoAt: 0
    readonly property bool canCall: !!section.shown.links && section.shown.links.tel === true
    readonly property bool canText: !!section.shown.links && section.shown.links.sms === true
    readonly property bool narrow: !!section.window && section.window.compact === true
    // The line whose reply box is open (by its first message's id), and what
    // is typed in each: kept as the lines are read again each minute.
    property string replyingTo: ""
    property var drafts: ({})

    function act(verb, args) {
        return JSON.parse(section.sioul.phoneMessages(verb, JSON.stringify(args || {})) || "null")
    }

    function reload() {
        const view = section.act("view", {}) || ({})
        section.shown = { lines: view.lines || [], links: view.links || { tel: false, sms: false }, texts: view.texts === true }
    }

    function answered(answer) {
        section.said = answer && answer.said ? answer.said : ""
        return answer
    }

    // A reply from the box: through the notification's own Reply (a request
    // the phone decides once), or as one of Sioul's own texts; its state then
    // under their messages. What refused it is said in the box.
    function send(line, words) {
        const answer = line.answer
        const typed = words.trim()
        if (!answer || typed === "")
            return ""
        const sent = answer.texts ? JSON.parse(section.sioul.texts("send", JSON.stringify({ to: line.modelData.dial, body: typed })) || "null") || ({})
                                  : section.act("reply", { phone: answer.phone, verb: "reply", notice: answer.notice, app: answer.app, line: answer.line, words: typed }) || ({})
        if (sent.said)
            return sent.said
        // The first reply in an app that marks the conversation read said so once.
        if (answer.note !== "")
            section.sioul.setViewFlag("replies-note-" + answer.app, true)
        section.drafts[line.key] = ""
        section.replyingTo = ""
        section.reload()
        return ""
    }

    // Taken back while it waits for the phone; sent again (a new key) where that fits.
    function takeBack(key) {
        section.answered(section.act("take-back", { key: key }))
        section.reload()
    }
    function sendAgain(answer, mine) {
        if (mine.texts)
            section.sioul.texts("again", JSON.stringify({ key: mine.key }))
        else
            section.answered(section.act("reply", { phone: answer.phone, verb: "reply", notice: answer.notice, app: answer.app, line: answer.line, words: mine.text, again: mine.key }))
        section.reload()
    }

    spacing: 6
    visible: section.shown.lines.length > 0 || undoTimer.running
    Component.onCompleted: section.reload()

    Timer {
        id: undoTimer

        interval: 10000
        onTriggered: {
            section.undoAt = 0
            section.said = ""
        }
    }

    // At each minute's turn (a sender's time may have come), and when the Porch is sorted again.
    Connections {
        target: section.window

        function onNowChanged() {
            section.reload()
        }
    }
    Connections {
        target: section.sioul

        function onPorchChanged() {
            section.reload()
        }
    }

    Panel {
        Layout.fillWidth: true
        theme: section.theme

        ColumnLayout {
            anchors.fill: parent
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: section.sioul.text("phonemsgs-title")
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                color: section.theme.text
            }
            RowLayout {
                visible: section.said !== ""
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: section.said
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: section.theme.muted
                }
                Button {
                    visible: undoTimer.running && section.undoAt > 0
                    flat: true
                    text: section.sioul.text("ui-undo")
                    onClicked: {
                        section.act("unseen", { at: section.undoAt })
                        section.undoAt = 0
                        section.said = ""
                        undoTimer.stop()
                        section.reload()
                    }
                }
            }

            Repeater {
                model: section.shown.lines

                delegate: ColumnLayout {
                    id: line

                    required property var modelData
                    // What it offers a reply from here; null for an older phone's lines (Text back as before).
                    readonly property var answer: line.modelData.answer || null
                    readonly property string key: line.modelData.ids && line.modelData.ids.length > 0 ? line.modelData.ids[0] : ""
                    readonly property bool replying: section.replyingTo !== "" && section.replyingTo === line.key
                    readonly property bool canReply: line.answer !== null && (line.answer.offered || line.answer.texts)
                    property string replySaid: ""

                    Layout.fillWidth: true
                    spacing: 4

                    // Who wrote, in which part of the day: what the app and your contacts say, never more.
                    Label {
                        Layout.fillWidth: true
                        text: line.modelData.head
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.text
                    }
                    // Each message: its time, its writer in a group, its words (plain text, never a link opened).
                    Repeater {
                        model: line.modelData.said || []

                        delegate: RowLayout {
                            id: message

                            required property var modelData

                            Layout.fillWidth: true
                            spacing: 10

                            Label {
                                Layout.alignment: Qt.AlignTop
                                text: message.modelData.time
                                textFormat: Text.PlainText
                                font.features: { "tnum": 1 }
                                color: section.theme.muted
                            }
                            Label {
                                Layout.fillWidth: true
                                visible: text !== ""
                                text: (message.modelData.from !== "" ? message.modelData.from + " · " : "") + message.modelData.text
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: section.theme.text
                            }
                        }
                    }
                    // Your replies from this computer, under their messages, each with its state in words.
                    Repeater {
                        model: line.answer ? line.answer.mine : []

                        delegate: ColumnLayout {
                            id: mine

                            required property var modelData

                            Layout.fillWidth: true
                            spacing: 2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10

                                Label {
                                    Layout.alignment: Qt.AlignTop
                                    text: mine.modelData.time
                                    textFormat: Text.PlainText
                                    font.features: { "tnum": 1 }
                                    color: section.theme.muted
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: section.sioul.text("replies-you") + " · " + mine.modelData.text
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: section.theme.text
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                text: mine.modelData.said
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: section.theme.muted
                            }
                            Flow {
                                visible: (mine.modelData.waiting && !mine.modelData.texts) || mine.modelData.again
                                Layout.fillWidth: true
                                spacing: 6

                                Button {
                                    visible: mine.modelData.waiting && !mine.modelData.texts
                                    flat: true
                                    text: section.sioul.text("replies-take-back")
                                    onClicked: section.takeBack(mine.modelData.key)
                                }
                                Button {
                                    visible: mine.modelData.again
                                    flat: true
                                    text: section.sioul.text("replies-send-again")
                                    onClicked: section.sendAgain(line.answer, mine.modelData)
                                }
                            }
                        }
                    }
                    // The reply's box, under the line: Send (Ctrl+Enter), Escape closes; what is typed kept.
                    ColumnLayout {
                        visible: line.replying
                        Layout.fillWidth: true
                        spacing: 4

                        Label {
                            visible: line.answer !== null && line.answer.note !== "" && !section.sioul.viewFlag("replies-note-" + line.answer.app)
                            Layout.fillWidth: true
                            text: line.answer ? line.answer.note : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: section.theme.muted
                        }
                        TextArea {
                            id: words

                            objectName: "replyWords"
                            Layout.fillWidth: true
                            placeholderText: line.answer ? section.theme.plain(line.answer.placeholder) : ""
                            textFormat: TextEdit.PlainText
                            wrapMode: TextEdit.Wrap
                            text: section.drafts[line.key] || ""
                            onTextChanged: section.drafts[line.key] = words.text
                            Accessible.name: placeholderText
                            Keys.onPressed: event => {
                                if ((event.key === Qt.Key_Return || event.key === Qt.Key_Enter) && (event.modifiers & Qt.ControlModifier)) {
                                    line.replySaid = section.send(line, words.text)
                                    event.accepted = true
                                } else if (event.key === Qt.Key_Escape) {
                                    section.replyingTo = ""
                                    event.accepted = true
                                }
                            }
                            Component.onCompleted: {
                                if (line.replying)
                                    words.forceActiveFocus()
                            }
                        }
                        // The characters left, near the limit only.
                        Label {
                            visible: line.answer !== null && words.length > line.answer.limit - 200
                            text: line.answer ? section.sioul.textWith("replies-left", "count", String(line.answer.limit - words.length)) : ""
                            textFormat: Text.PlainText
                            font.pixelSize: 13
                            color: section.theme.muted
                        }
                        Label {
                            visible: line.replySaid !== ""
                            Layout.fillWidth: true
                            text: line.replySaid
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: section.theme.muted
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 6

                            Button {
                                enabled: words.text.trim() !== "" && line.answer !== null && words.length <= line.answer.limit
                                text: section.sioul.text("replies-send")
                                onClicked: line.replySaid = section.send(line, words.text)
                            }
                            Button {
                                flat: true
                                text: section.sioul.text("ui-cancel")
                                onClicked: section.replyingTo = ""
                            }
                        }
                    }
                    // Its notification gone, or its Reply closed to Sioul: on the phone (a text: Text back).
                    Label {
                        visible: line.answer !== null && !line.canReply && !line.modelData.texted
                        Layout.fillWidth: true
                        text: line.answer ? line.answer.on_phone : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.muted
                    }
                    Label {
                        visible: line.modelData.number !== ""
                        Layout.fillWidth: true
                        text: line.modelData.number
                        textFormat: Text.PlainText
                        font.features: { "tnum": 1 }
                        color: section.theme.muted
                    }
                    Label {
                        visible: line.modelData.note !== ""
                        Layout.fillWidth: true
                        text: line.modelData.note
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: section.theme.muted
                    }
                    // The line's own actions, few and calm: Reply first, then a call back, Seen last;
                    // the rest in its menu (⋮, a right click, a long press, the Menu key).
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        // Through the notification's own Reply, or as a text of Sioul's own: the box under the line.
                        Button {
                            visible: line.canReply && !line.replying
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("replies-reply")
                            icon.name: "mail-reply-sender"
                            icon.color: section.theme.text
                            display: section.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            ToolTip.visible: hovered && display === AbstractButton.IconOnly
                            ToolTip.text: section.theme.plain(text)
                            Accessible.name: text
                            onClicked: {
                                line.replySaid = ""
                                section.replyingTo = line.key
                            }
                        }
                        // On a computer, through the system's app for such links (KDE Connect hands them to your phone).
                        Button {
                            // texts: in Sioul's Texts page when texts are read here, else the system's app; Reply takes its place when offered.
                            visible: line.modelData.texted && (section.canText || section.shown.texts) && !line.canReply
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-text-back")
                            icon.name: "mail-message-new"
                            icon.color: section.theme.text
                            display: section.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            ToolTip.visible: hovered && display === AbstractButton.IconOnly
                            ToolTip.text: section.theme.plain(text)
                            Accessible.name: text
                            onClicked: {
                                if (section.shown.texts)
                                    section.window.openTexts(line.modelData.dial)
                                else
                                    Qt.openUrlExternally("sms:" + line.modelData.dial)
                            }
                        }
                        Button {
                            visible: line.modelData.texted && section.canCall
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-call-back")
                            icon.name: "call-start"
                            icon.color: section.theme.text
                            display: section.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            ToolTip.visible: hovered && display === AbstractButton.IconOnly
                            ToolTip.text: section.theme.plain(text)
                            Accessible.name: text
                            onClicked: Qt.openUrlExternally("tel:" + line.modelData.dial)
                        }
                        // No app for them here: the number, to answer from the phone yourself.
                        Button {
                            visible: line.modelData.dial !== "" && !section.canCall && !section.canText && !section.shown.texts
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-copy")
                            icon.name: "edit-copy"
                            icon.color: section.theme.text
                            display: section.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            ToolTip.visible: hovered && display === AbstractButton.IconOnly
                            ToolTip.text: section.theme.plain(text)
                            Accessible.name: text
                            onClicked: {
                                section.window.copy(line.modelData.number)
                                section.said = section.sioul.text("calls-copied")
                            }
                        }
                        Button {
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            flat: true
                            text: section.sioul.text("calls-seen")
                            // Seen here, gone from every device's Porch; ten seconds to undo.
                            onClicked: {
                                const answer = section.answered(section.act("seen", { ids: line.modelData.ids }))
                                section.undoAt = answer && answer.at ? answer.at : 0
                                if (section.undoAt > 0)
                                    undoTimer.restart()
                                section.reload()
                            }
                        }
                        ToolButton {
                            id: more

                            visible: line.modelData.dial !== "" || (line.answer !== null && (line.answer.read || line.answer.texts))
                            icon.name: "overflow-menu"
                            icon.color: section.theme.text
                            Accessible.name: section.sioul.text("ui-more")
                            ToolTip.visible: more.hovered
                            ToolTip.text: section.sioul.text("ui-more")
                            ToolTip.delay: 400
                            onClicked: lineMenu.now().show(line.modelData, more)
                        }
                    }

                    Keys.onPressed: event => {
                        if (more.visible && (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier)))) {
                            lineMenu.now().show(line.modelData, more)
                            event.accepted = true
                        }
                    }
                    TapHandler {
                        id: longPress

                        enabled: more.visible
                        acceptedDevices: PointerDevice.TouchScreen
                        onLongPressed: {
                            line.Window.window.menuAt = line.mapToItem(null, longPress.point.position.x, longPress.point.position.y)
                            lineMenu.now().show(line.modelData, null)
                        }
                    }
                    TapHandler {
                        enabled: more.visible
                        acceptedButtons: Qt.RightButton
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onTapped: lineMenu.now().show(line.modelData, null)
                    }
                }
            }
            // The menu of a line: one for the section, made the first time it opens.
            Later {
                id: lineMenu

                sourceComponent: Component {
                    SioulMenu {
                        id: lineMenuForm

                        property var line: null

                        function show(line, at) {
                            lineMenuForm.line = line
                            if (at) {
                                const below = at.mapToItem(lineMenuForm.parent, 0, at.height)
                                lineMenuForm.popup(below.x, below.y)
                            } else {
                                lineMenuForm.popup()
                            }
                        }

                        // The app's own Mark as read, with its warning first (decision 13).
                        MenuItem {
                            visible: lineMenuForm.line !== null && !!lineMenuForm.line.answer && lineMenuForm.line.answer.read && !lineMenuForm.line.answer.texts
                            height: visible ? implicitHeight : 0
                            text: visible ? section.theme.plain(lineMenuForm.line.answer.read_item) : ""
                            onTriggered: {
                                readAsk.answer = lineMenuForm.line.answer
                                readAsk.ask(section.sioul.text("replies-read-title"), lineMenuForm.line.answer.read_warning, section.sioul.text("replies-read-confirm"))
                            }
                        }
                        // A text: its conversation on the Texts page.
                        MenuItem {
                            visible: lineMenuForm.line !== null && !!lineMenuForm.line.answer && lineMenuForm.line.answer.texts
                            height: visible ? implicitHeight : 0
                            text: section.sioul.text("replies-open-texts")
                            onTriggered: section.window.openTexts(lineMenuForm.line.dial)
                        }
                        // Their list, Always through, their calls and messages: their sheet.
                        MenuItem {
                            visible: lineMenuForm.line !== null && lineMenuForm.line.dial !== ""
                            height: visible ? implicitHeight : 0
                            text: section.sioul.text("attention-sheet-sender")
                            onTriggered: section.window.openPersonSheet("", JSON.stringify(["tel:" + lineMenuForm.line.dial]))
                        }
                        MenuItem {
                            visible: lineMenuForm.line !== null && !lineMenuForm.line.known && lineMenuForm.line.dial !== ""
                            height: visible ? implicitHeight : 0
                            text: section.sioul.text("calls-add-contact")
                            icon.name: "contact-new"
                            icon.color: section.theme.text
                            onTriggered: section.window.newContactWith(lineMenuForm.line.number)
                        }
                        MenuItem {
                            visible: lineMenuForm.line !== null && lineMenuForm.line.dial !== ""
                            height: visible ? implicitHeight : 0
                            text: section.sioul.text("calls-block")
                            icon.name: "dialog-cancel"
                            icon.color: section.theme.text
                            onTriggered: {
                                blockAsk.number = lineMenuForm.line.dial
                                blockAsk.ask(section.sioul.text("phonemsgs-block-title"), section.sioul.textWith("phonemsgs-block-ask", "number", lineMenuForm.line.number), section.sioul.text("calls-block"))
                            }
                        }
                    }
                }
            }
            // Mark as read, asked once with its warning: the phone sends the app's own action, nothing else.
            ConfirmDialog {
                id: readAsk

                property var answer: null

                sioul: section.sioul
                theme: section.theme
                onConfirmed: {
                    if (readAsk.answer)
                        section.answered(section.act("reply", { phone: readAsk.answer.phone, verb: "read", notice: readAsk.answer.notice, app: readAsk.answer.app, line: readAsk.answer.line, words: "" }))
                    section.reload()
                }
            }
            // Blocking is asked once: their calls go to voicemail, their messages stay on the phone.
            ConfirmDialog {
                id: blockAsk

                property string number: ""

                sioul: section.sioul
                theme: section.theme
                onConfirmed: {
                    section.answered(section.act("block", { number: blockAsk.number }))
                    section.reload()
                }
            }
        }
    }
}
