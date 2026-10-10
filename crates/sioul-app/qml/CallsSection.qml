// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The calls your phones declined, on the Porch of every device (docs/porch.md,
// "Calls declined"; docs/android.md, "Calls"): each shown at the times its
// caller may reach you, by phone or in writing; never counted, no badge. Who,
// when, on which phone when not this one, a later call that rang, the doubt
// said plainly ("They may have left a message"), and what you may do: text
// back or call back (the phone's own apps, or on a computer the system's app
// for such links, KDE Connect handing them to your phone; else copy the
// number: nothing is sent by Sioul), listen to the voicemail your operator mailed,
// and "Seen", which takes a line away on every device, with ten seconds to
// undo; the rest in the line's menu (⋮, a right click, a long press, the Menu
// key or Shift+F10): add to your contacts (Android's form on a phone, Sioul's
// on a computer), block, see how they reach you, why it went to voicemail.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    required property var window

    // calls.rs's `view`: {lines: [{ids, text, number, dial, known, hidden, doubt,
    // message: {text, path, sound}, why, again}], phone, links: {tel, sms}}.
    property var shown: ({ lines: [], phone: false, links: { tel: false, sms: false } })
    property string said: ""
    // The voicemail being played: its line's first id, and its sound's address.
    property string playing: ""
    property string playingUrl: ""
    // Lines whose "Why?" is open, by their first id.
    property var whyShown: ({})
    // The last Seen pressed here (its time), undone within ten seconds.
    property real undoAt: 0
    readonly property bool onPhone: section.shown.phone === true
    readonly property bool canCall: !!section.shown.links && section.shown.links.tel === true
    readonly property bool canText: !!section.shown.links && section.shown.links.sms === true
    // A phone held upright: the buttons' icons alone, as on every page there (their words on hover and for readers).
    readonly property bool narrow: !!section.window && section.window.compact === true

    function reload() {
        const view = JSON.parse(section.sioul.callsView() || "null") || ({})
        section.shown = { lines: view.lines || [], phone: view.phone === true, links: view.links || { tel: false, sms: false } }
    }

    function act(verb, args) {
        const answer = JSON.parse(section.sioul.callsAction(verb, JSON.stringify(args || {})) || "null")
        section.said = answer && answer.said ? answer.said : ""
        return answer
    }

    spacing: 6
    visible: section.shown.lines.length > 0 || undoTimer.running
    Component.onCompleted: section.reload()

    // Ten seconds to undo a Seen, as the review queue's answers have.
    Timer {
        id: undoTimer

        interval: 10000
        onTriggered: {
            section.undoAt = 0
            section.said = ""
        }
    }

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

    Panel {
        Layout.fillWidth: true
        theme: section.theme

        ColumnLayout {
            anchors.fill: parent
            spacing: 10

            Label {
                Layout.fillWidth: true
                text: section.sioul.text("calls-title")
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
                    readonly property string key: line.modelData.ids.length > 0 ? line.modelData.ids[0] : ""

                    Layout.fillWidth: true
                    spacing: 4

                    // Who called, when: what the network and your contacts say, never more.
                    Label {
                        Layout.fillWidth: true
                        text: line.modelData.text
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.text
                    }
                    Label {
                        visible: line.modelData.number !== ""
                        Layout.fillWidth: true
                        text: line.modelData.number
                        textFormat: Text.PlainText
                        font.features: { "tnum": 1 }
                        color: section.theme.muted
                    }
                    // The same number's later call, which rang: you may have spoken already.
                    Label {
                        visible: (line.modelData.again || "") !== ""
                        Layout.fillWidth: true
                        text: line.modelData.again || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: section.theme.text
                    }
                    // What Sioul cannot know, said; or the message Free mailed.
                    Label {
                        visible: line.modelData.doubt !== ""
                        Layout.fillWidth: true
                        text: line.modelData.doubt
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
                    // The line's own actions, few and calm: the message, a text or a call back,
                    // Seen last; the rest in its menu (⋮, a right click, a long press, the Menu key).
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        Button {
                            visible: !!line.modelData.message && line.modelData.message.sound !== null && line.modelData.message.sound !== undefined && section.playing !== line.key
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-listen")
                            icon.name: "media-playback-start"
                            icon.color: section.theme.text
                            display: section.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            ToolTip.visible: hovered && display === AbstractButton.IconOnly
                            ToolTip.text: section.theme.plain(text)
                            Accessible.name: text
                            onClicked: {
                                const answer = section.act("listen", { path: line.modelData.message.path, sound: line.modelData.message.sound })
                                if (answer && answer.url) {
                                    section.playingUrl = answer.url
                                    section.playing = line.key
                                }
                            }
                        }
                        // Written first: a text before a call. On a computer, through the
                        // system's app for such links (KDE Connect hands them to your phone).
                        Button {
                            visible: line.modelData.dial !== "" && section.canText
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-text-back")
                            icon.name: "mail-message-new"
                            icon.color: section.theme.text
                            display: section.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            ToolTip.visible: hovered && display === AbstractButton.IconOnly
                            ToolTip.text: section.theme.plain(text)
                            Accessible.name: text
                            onClicked: Qt.openUrlExternally("sms:" + line.modelData.dial)
                        }
                        Button {
                            visible: line.modelData.dial !== "" && section.canCall
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
                        // A computer with no app for them: the number, to dial it yourself.
                        Button {
                            visible: line.modelData.dial !== "" && !section.onPhone && !section.canCall && !section.canText
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
                                const answer = section.act("seen", { ids: line.modelData.ids })
                                section.undoAt = answer && answer.at ? answer.at : 0
                                if (section.undoAt > 0)
                                    undoTimer.restart()
                                if (section.playing === line.key)
                                    section.playing = ""
                                section.reload()
                            }
                        }
                        // The rest of what may be done: add to contacts, block, their sheet, why.
                        ToolButton {
                            id: more

                            icon.name: "overflow-menu"
                            icon.color: section.theme.text
                            Accessible.name: section.sioul.text("ui-more")
                            ToolTip.visible: more.hovered
                            ToolTip.text: section.sioul.text("ui-more")
                            ToolTip.delay: 400
                            onClicked: lineMenu.now().show(line.modelData, line.key, more)
                        }
                    }

                    // The keyboard's way to the line's menu, as Mail's rows have it: the
                    // Menu key or Shift+F10, from any of the line's buttons.
                    Keys.onPressed: event => {
                        if (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier))) {
                            lineMenu.now().show(line.modelData, line.key, more)
                            event.accepted = true
                        }
                    }
                    // On a touch screen, the menu comes at a long press, where the finger is,
                    // as the Porch's rows have it; with a mouse, at a right click.
                    TapHandler {
                        id: longPress

                        acceptedDevices: PointerDevice.TouchScreen
                        onLongPressed: {
                            line.Window.window.menuAt = line.mapToItem(null, longPress.point.position.x, longPress.point.position.y)
                            lineMenu.now().show(line.modelData, line.key, null)
                        }
                    }
                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onTapped: lineMenu.now().show(line.modelData, line.key, null)
                    }
                }
            }
            // The menu of a line: one for the section, made the first time it opens.
            Later {
                id: lineMenu

                sourceComponent: Component {
                    SioulMenu {
                        id: lineMenuForm

                        // The line it was opened for (calls.rs's `view`), and its key.
                        property var line: null
                        property string key: ""
                        readonly property bool numbered: lineMenuForm.line !== null && lineMenuForm.line.dial !== ""

                        // At its ⋮ (a click, the keyboard), else where the pointer or the
                        // finger is. Placed, not given the ⋮ for parent: a line is made
                        // again when the list changes, the menu stays the section's.
                        function show(line, key, at) {
                            lineMenuForm.line = line
                            lineMenuForm.key = key
                            if (at) {
                                const below = at.mapToItem(lineMenuForm.parent, 0, at.height)
                                lineMenuForm.popup(below.x, below.y)
                            } else {
                                lineMenuForm.popup()
                            }
                        }

                        MenuItem {
                            visible: lineMenuForm.numbered && !lineMenuForm.line.known
                            height: visible ? implicitHeight : 0
                            text: section.sioul.text("calls-add-contact")
                            icon.name: "contact-new"
                            icon.color: section.theme.text
                            // Android's own form on a phone; Sioul's in Contacts on a computer.
                            onTriggered: {
                                if (section.onPhone)
                                    section.act("add-contact", { number: lineMenuForm.line.dial })
                                else
                                    section.window.newContactWith(lineMenuForm.line.number)
                            }
                        }
                        MenuItem {
                            visible: lineMenuForm.numbered
                            height: visible ? implicitHeight : 0
                            text: section.sioul.text("calls-block")
                            icon.name: "dialog-cancel"
                            icon.color: section.theme.text
                            onTriggered: {
                                blockAsk.number = lineMenuForm.line.dial
                                blockAsk.ask(section.sioul.text("calls-block-title"), section.sioul.textWith(section.onPhone ? "calls-block-ask" : "calls-block-ask-elsewhere", "number", lineMenuForm.line.number), section.sioul.text("calls-block"))
                            }
                        }
                        // Their list, Always through, their calls of the month: their sheet.
                        MenuItem {
                            visible: lineMenuForm.numbered
                            height: visible ? implicitHeight : 0
                            text: section.sioul.text("attention-sheet-sender")
                            onTriggered: section.window.openPersonSheet("", JSON.stringify(["tel:" + lineMenuForm.line.dial]))
                        }
                        MenuItem {
                            text: section.sioul.text("calls-why")
                            onTriggered: {
                                const next = Object.assign({}, section.whyShown)
                                next[lineMenuForm.key] = !next[lineMenuForm.key]
                                section.whyShown = next
                            }
                        }
                    }
                }
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
            // Android's own blocked numbers: calls and texts, for every app; the phone's page.
            Button {
                visible: section.onPhone
                flat: true
                Layout.alignment: Qt.AlignLeft
                text: section.sioul.text("calls-open-blocked")
                onClicked: section.act("open-blocked", {})
            }
        }
    }
}
