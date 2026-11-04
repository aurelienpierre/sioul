// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The texts and the other apps' notifications your phone brought, rows of
// the Porch's "From your phone" on a computer (docs/porch.md, "From your
// phone"; docs/android.md, "Messages on your computers"): for the apps you
// chose there, once the phone let them through and while their sender may
// reach you; never counted, no badge, never a notification here. A text
// the Texts page holds is one row, opening it there. Another app's is a row
// per conversation and part of the day: who, the newest words (or "a
// picture", or only who and when; a code said, never shown), when, the
// earlier ones under it. Its main action replies, through the
// notification's own Reply on your phone while the phone says it works
// (replies.rs; a text from a number goes as one of Sioul's own texts where
// this computer reads them), your reply then under their messages with its
// state in words, Take back while it waits; else text back or call back
// (the system's app for such links, KDE Connect handing them to your phone;
// else copy the number). Mark read and Dismiss act at once on every device,
// with ten seconds to undo; neither touches the phone's notification. The
// rest is in the row's menu (⋮, a right click, a long press, the Menu key or
// Shift+F10): Mark as read in the app, with its warning, how they reach you,
// add to your contacts, block.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window
    // The Porch (PorchPage.qml): its words on notices (`say`), the rows still new (`newOnes`).
    required property var porch

    // phonemsgs.rs's `view`: {lines: [{ids (their notices), notice, head, what, when, app,
    // said: [{time, from, text}], note, number, dial, known, texted, code, who, answer}],
    // links: {tel, sms}, texts, notices: [{conversation, notice, ids, targets, who, first,
    // when, count, sent, late}]}; `answer` (replies.rs's `Answer`): {phone, notice, app,
    // label, line, offered, read, texts, placeholder, note, read_item, read_warning,
    // on_phone, limit, mine: [{key, time, text, said, waiting, again, texts}]}.
    property var shown: ({ lines: [], links: { tel: false, sms: false }, texts: false, notices: [] })
    readonly property bool canCall: !!section.shown.links && section.shown.links.tel === true
    readonly property bool canText: !!section.shown.links && section.shown.links.sms === true
    // The line whose reply box is open (by its first notice), and what is
    // typed in each: kept as the lines are read again each minute.
    property string replyingTo: ""
    property var drafts: ({})
    // The rows shown: the Porch's section counts them.
    readonly property var textRows: section.porch.newOnes(section.shown.notices)
    readonly property var lineRows: section.porch.newOnes(section.shown.lines)
    readonly property int count: section.textRows.length + section.lineRows.length

    function act(verb, args) {
        return JSON.parse(section.sioul.phoneMessages(verb, JSON.stringify(args || {})) || "null")
    }

    function reload() {
        const view = section.act("view", {}) || ({})
        section.shown = { lines: view.lines || [], links: view.links || { tel: false, sms: false }, texts: view.texts === true, notices: view.notices || [] }
    }

    function answered(answer) {
        if (answer && answer.said)
            section.sioul.status = answer.said
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
        // A text from the phone it came to, never another's SIM.
        const sent = answer.texts ? JSON.parse(section.sioul.texts("send", JSON.stringify({ to: line.modelData.dial, body: typed, phone: answer.phone })) || "null") || ({})
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

    // Taken back while it waits for the phone, said at once ("Taking it back…", no
    // Take back again; too late, said, when the phone had taken it already); sent
    // again (a new key) where that fits.
    function takeBack(key) {
        section.answered(section.act("take-back", { key: key }))
        section.reload()
    }
    function sendAgain(answer, mine) {
        if (mine.texts)
            section.sioul.texts("again", JSON.stringify({ key: mine.key }))
        else
            // The conversation's notification now, never the refused reply's own, which its app may have replaced:
            // the phone finds its first, on its way, through any notification of the conversation (L-2).
            section.answered(section.act("reply", { phone: answer.phone, verb: "reply", notice: answer.notice, app: answer.app, line: answer.line, words: mine.text, again: mine.key }))
        section.reload()
    }

    // A line's main action: Reply where offered; else text back, call back, or its number copied.
    function mainOf(line, canReply) {
        if (canReply)
            return { id: "reply", text: section.sioul.text("replies-reply"), icon: "mail-reply-sender" }
        if (line.texted && (section.canText || section.shown.texts))
            return { id: "text", text: section.sioul.text("calls-text-back"), icon: "mail-message-new" }
        if (line.texted && section.canCall)
            return { id: "call", text: section.sioul.text("calls-call-back"), icon: "call-start" }
        if (line.dial !== "" && !section.canCall && !section.canText && !section.shown.texts)
            return { id: "copy", text: section.sioul.text("calls-copy"), icon: "edit-copy" }
        return { id: "", text: "", icon: "" }
    }

    // The rest, in its menu.
    function moreOf(line, main) {
        const more = []
        if (main !== "call" && line.texted && section.canCall)
            more.push({ id: "call", text: section.sioul.text("calls-call-back"), icon: "call-start" })
        // The app's own Mark as read, with its warning first (decision 13).
        if (!!line.answer && line.answer.read && !line.answer.texts)
            more.push({ id: "app-read", text: line.answer.read_item, icon: "" })
        // A text: its conversation on the Texts page.
        if (!!line.answer && line.answer.texts)
            more.push({ id: "texts", text: section.sioul.text("replies-open-texts"), icon: "sioul-texts" })
        if (line.dial !== "") {
            more.push({ id: "sheet", text: section.sioul.text("attention-sheet-sender"), icon: "" })
            if (!line.known)
                more.push({ id: "contact", text: section.sioul.text("calls-add-contact"), icon: "contact-new" })
            more.push({ id: "block", text: section.sioul.text("calls-block"), icon: "dialog-cancel" })
        }
        return more
    }

    function chosen(id, row) {
        const line = row.modelData
        if (id === "reply") {
            row.replySaid = ""
            section.replyingTo = row.key
        } else if (id === "text") {
            if (section.shown.texts)
                section.window.openTexts(line.dial)
            else
                Qt.openUrlExternally("sms:" + line.dial)
        } else if (id === "call") {
            Qt.openUrlExternally("tel:" + line.dial)
        } else if (id === "copy") {
            section.window.copy(line.number)
            section.sioul.status = section.sioul.text("calls-copied")
        } else if (id === "app-read") {
            readAsk.answer = line.answer
            readAsk.ask(section.sioul.text("replies-read-title"), line.answer.read_warning, section.sioul.text("replies-read-confirm"))
        } else if (id === "texts") {
            section.window.openTexts(line.dial)
        } else if (id === "sheet") {
            section.window.openPersonSheet("", JSON.stringify(["tel:" + line.dial]))
        } else if (id === "contact") {
            section.window.newContactWith(line.number)
        } else if (id === "block") {
            blockAsk.number = line.dial
            blockAsk.ask(section.sioul.text("phonemsgs-block-title"), section.sioul.textWith("phonemsgs-block-ask", "number", line.number), section.sioul.text("calls-block"))
        }
    }

    spacing: 0
    visible: section.count > 0
    Component.onCompleted: section.reload()

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

    // A text the Texts page holds, not read yet: one row, its sender and first
    // words, opening it there (23 October 2026); the phone's notification of it
    // is folded into it in Rust (`texts::folding`).
    Repeater {
        model: section.textRows

        delegate: NoticeRow {
            id: notice

            required property var modelData

            objectName: "textNotice"
            sioul: section.sioul
            theme: section.theme
            compact: section.porch.narrowRows
            notice: notice.modelData.notice
            iconName: "sioul-texts"
            // A text that came 5 minutes or more after it was sent says when it was sent too.
            note: notice.modelData.sent ? section.sioul.textArgs("texts-notice-late", JSON.stringify({ sent: notice.modelData.sent, late: notice.modelData.late || "" })) : ""
            mainText: section.sioul.text("ui-open")
            mainIcon: "sioul-texts"
            onOpened: section.window.openTexts(notice.modelData.conversation)
            onMainPressed: section.window.openTexts(notice.modelData.conversation)
            onReadPressed: section.porch.say("read", notice.modelData)
            onDismissPressed: section.porch.say("dismiss", notice.modelData)
        }
    }
    Repeater {
        model: section.lineRows

        delegate: NoticeRow {
            id: line

            required property var modelData
            // What it offers a reply from here; null for an older phone's lines (Text back as before).
            readonly property var answer: line.modelData.answer || null
            readonly property string key: line.modelData.ids && line.modelData.ids.length > 0 ? line.modelData.ids[0] : ""
            readonly property bool replying: section.replyingTo !== "" && section.replyingTo === line.key
            readonly property bool canReply: line.answer !== null && (line.answer.offered || line.answer.texts)
            readonly property var main: section.mainOf(line.modelData, line.canReply && !line.replying)
            property string replySaid: ""

            sioul: section.sioul
            theme: section.theme
            compact: section.porch.narrowRows
            notice: line.modelData.notice
            iconName: line.modelData.code ? "dialog-password" : "mail-message"
            chips: line.modelData.app ? [line.modelData.app] : []
            mainText: line.main.text
            mainIcon: line.main.icon
            more: section.moreOf(line.modelData, line.main.id)
            onMainPressed: section.chosen(line.main.id, line)
            onReadPressed: section.porch.say("read", line.modelData)
            onDismissPressed: section.porch.say("dismiss", line.modelData)
            onChosen: id => section.chosen(id, line)

            // Its earlier messages, under the newest: the time, the writer in a group, the words (plain text, never a link opened).
            Repeater {
                model: (line.modelData.said || []).slice(0, -1)

                delegate: RowLayout {
                    id: message

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 10

                    Label {
                        Layout.alignment: Qt.AlignTop
                        text: message.modelData.time
                        textFormat: Text.PlainText
                        font.pixelSize: 13
                        font.features: { "tnum": 1 }
                        color: section.theme.muted
                    }
                    Label {
                        Layout.fillWidth: true
                        visible: text !== ""
                        text: (message.modelData.from !== "" ? message.modelData.from + " · " : "") + message.modelData.text
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
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
                            font.pixelSize: 13
                            font.features: { "tnum": 1 }
                            color: section.theme.muted
                        }
                        Label {
                            Layout.fillWidth: true
                            text: section.sioul.text("replies-you") + " · " + mine.modelData.text
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
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
                font.pixelSize: 13
                color: section.theme.muted
            }
            Label {
                // Someone your address books do not name: their number, under who the app said they are.
                visible: line.modelData.number !== "" && line.modelData.number !== line.notice.who && !line.modelData.known
                Layout.fillWidth: true
                text: line.modelData.number
                textFormat: Text.PlainText
                font.pixelSize: 13
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
