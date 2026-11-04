// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The calls your phones declined, rows of the Porch's "From your phone" on
// every device (docs/porch.md, "Calls declined"; docs/android.md, "Calls"):
// each shown at the times its caller may reach you, by phone or in writing;
// never counted, no badge. A row says who, the time of day it was, when;
// under it a later call that rang, the voicemail your operator mailed; that
// one may have been left otherwise is said once, in the section's "?". Its
// main action texts back, writing before calls, else calls back (the
// phone's own apps, or on a computer the system's app for such links, KDE
// Connect handing them to your phone; else the number is copied: nothing is
// sent by Sioul); Dismiss, its "forget", takes it away on every device, with
// ten seconds to undo. The rest is in the row's menu (⋮, a right click, a
// long press, the Menu key or Shift+F10): call back, listen, add to your
// contacts (Android's form on a phone, Sioul's on a computer), block, how
// they reach you, why it went to voicemail.

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

    // calls.rs's `view`: {lines: [{ids (their notices), notice, text, number, dial, known,
    // hidden, doubt, message: {text, path, sound}, why, again}], phone, links: {tel, sms}}.
    property var shown: ({ lines: [], phone: false, links: { tel: false, sms: false } })
    // The voicemail being played: its line's first notice, and its sound's address.
    property string playing: ""
    property string playingUrl: ""
    // Lines whose "Why?" is open, by their first notice.
    property var whyShown: ({})
    readonly property bool onPhone: section.shown.phone === true
    readonly property bool canCall: !!section.shown.links && section.shown.links.tel === true
    readonly property bool canText: !!section.shown.links && section.shown.links.sms === true
    // The rows shown: the Porch's section counts them.
    readonly property var rows: section.porch.newOnes(section.shown.lines)
    readonly property int count: section.rows.length

    function reload() {
        const view = JSON.parse(section.sioul.callsView() || "null") || ({})
        section.shown = { lines: view.lines || [], phone: view.phone === true, links: view.links || { tel: false, sms: false } }
    }

    function act(verb, args) {
        const answer = JSON.parse(section.sioul.callsAction(verb, JSON.stringify(args || {})) || "null")
        if (answer && answer.said)
            section.sioul.status = answer.said
        return answer
    }

    // A row's main action, written first: a text before a call (docs/research.md,
    // finding 29); else a call back; else, a computer with no app for them, its number copied.
    function mainOf(line) {
        if (line.dial === "")
            return { id: "", text: "", icon: "" }
        if (section.canText)
            return { id: "text", text: section.sioul.text("calls-text-back"), icon: "mail-message-new" }
        if (section.canCall)
            return { id: "call", text: section.sioul.text("calls-call-back"), icon: "call-start" }
        return { id: "copy", text: section.sioul.text("calls-copy"), icon: "edit-copy" }
    }

    // The row's other actions, in its menu: a call back after the text.
    function moreOf(line) {
        const more = []
        if (line.dial !== "" && section.canText && section.canCall)
            more.push({ id: "call", text: section.sioul.text("calls-call-back"), icon: "call-start" })
        if (line.message && line.message.sound !== null && line.message.sound !== undefined)
            more.push({ id: "listen", text: section.sioul.text("calls-listen"), icon: "media-playback-start" })
        if (line.dial !== "" && !line.known)
            more.push({ id: "contact", text: section.sioul.text("calls-add-contact"), icon: "contact-new" })
        if (line.dial !== "") {
            more.push({ id: "block", text: section.sioul.text("calls-block"), icon: "dialog-cancel" })
            more.push({ id: "sheet", text: section.sioul.text("attention-sheet-sender"), icon: "" })
        }
        more.push({ id: "why", text: section.sioul.text("calls-why"), icon: "" })
        return more
    }

    function chosen(id, line, key) {
        if (id === "text")
            Qt.openUrlExternally("sms:" + line.dial)
        else if (id === "call")
            Qt.openUrlExternally("tel:" + line.dial)
        else if (id === "copy") {
            section.window.copy(line.number)
            section.sioul.status = section.sioul.text("calls-copied")
        } else if (id === "listen") {
            const answer = section.act("listen", { path: line.message.path, sound: line.message.sound })
            if (answer && answer.url) {
                section.playingUrl = answer.url
                section.playing = key
            }
        } else if (id === "contact") {
            // Android's own form on a phone; Sioul's in Contacts on a computer.
            if (section.onPhone)
                section.act("add-contact", { number: line.dial })
            else
                section.window.newContactWith(line.number)
        } else if (id === "block") {
            blockAsk.number = line.dial
            blockAsk.ask(section.sioul.text("calls-block-title"), section.sioul.textWith(section.onPhone ? "calls-block-ask" : "calls-block-ask-elsewhere", "number", line.number), section.sioul.text("calls-block"))
        } else if (id === "sheet") {
            section.window.openPersonSheet("", JSON.stringify(["tel:" + line.dial]))
        } else if (id === "why") {
            const next = Object.assign({}, section.whyShown)
            next[key] = !next[key]
            section.whyShown = next
        }
    }

    spacing: 0
    visible: section.count > 0
    Component.onCompleted: section.reload()

    // At each minute's turn (a caller's time may have come), and when the Porch is sorted again.
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

    Repeater {
        model: section.rows

        delegate: NoticeRow {
            id: line

            required property var modelData
            readonly property string key: line.modelData.ids.length > 0 ? line.modelData.ids[0] : ""

            sioul: section.sioul
            theme: section.theme
            compact: section.porch.narrowRows
            notice: line.modelData.notice
            readonly property var main: section.mainOf(line.modelData)

            iconName: "call-start"
            mainText: line.main.text
            mainIcon: line.main.icon
            more: section.moreOf(line.modelData)
            onMainPressed: section.chosen(line.main.id, line.modelData, line.key)
            onDismissPressed: {
                if (section.playing === line.key)
                    section.playing = ""
                section.porch.say("dismiss", line.modelData)
            }
            onChosen: id => section.chosen(id, line.modelData, line.key)

            // The same number's later call, which rang: you may have spoken already.
            Label {
                visible: (line.modelData.again || "") !== ""
                Layout.fillWidth: true
                text: line.modelData.again || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: section.theme.muted
            }
            Label {
                visible: !!line.modelData.message
                Layout.fillWidth: true
                text: line.modelData.message ? line.modelData.message.text : ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: section.theme.text
            }
            Label {
                visible: section.whyShown[line.key] === true
                Layout.fillWidth: true
                text: line.modelData.why
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: section.theme.muted
            }
            // The voicemail's sound, played here once asked (AudioPlayer.qml).
            Loader {
                active: section.playing === line.key && section.playingUrl !== ""
                visible: active
                Layout.fillWidth: true

                sourceComponent: Component {
                    AudioPlayer {
                        source: section.playingUrl
                        sioul: section.sioul
                        theme: section.theme
                    }
                }
            }
        }
    }
    // Android's own blocked numbers: calls and texts, for every app; the phone's page.
    Button {
        visible: section.onPhone
        flat: true
        Layout.alignment: Qt.AlignLeft
        text: section.sioul.text("calls-open-blocked")
        onClicked: section.act("open-blocked", {})
    }
    // Blocking is asked once: their calls go to voicemail for good, listed nowhere.
    ConfirmDialog {
        id: blockAsk

        property string number: ""

        sioul: section.sioul
        theme: section.theme
        onConfirmed: {
            section.act("block", { number: blockAsk.number })
            section.reload()
        }
    }
}
