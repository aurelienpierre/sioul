// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "From your phone", on a computer's Porch (docs/porch.md, "From your phone";
// docs/android.md, "Messages on your computers"): the messages your phone's
// notifications brought, for the apps you chose there, once the phone let
// them through and while their sender may reach you; never counted, no
// badge, never a notification here. One line per conversation and part of
// the day: who, when, the words (or "a picture", or only who and when); a
// code said, never shown. What you may do: text back or call back (the
// system's app for such links, KDE Connect handing them to your phone; else
// copy the number: nothing is sent by Sioul), and "Seen", which takes the
// line away on every device, with ten seconds to undo, and never touches
// the phone's notification; the rest in the line's menu (⋮, a right click,
// a long press, the Menu key or Shift+F10): how they reach you, add to your
// contacts, block.

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
    // note, number, dial, known, texted, code}], links: {tel, sms}}.
    property var shown: ({ lines: [], links: { tel: false, sms: false }, texts: false })
    property string said: ""
    // The last Seen pressed here (its time), undone within ten seconds.
    property real undoAt: 0
    readonly property bool canCall: !!section.shown.links && section.shown.links.tel === true
    readonly property bool canText: !!section.shown.links && section.shown.links.sms === true
    readonly property bool narrow: !!section.window && section.window.compact === true

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
                    // The line's own actions, few and calm: a text or a call back, Seen last;
                    // the rest in its menu (⋮, a right click, a long press, the Menu key).
                    Flow {
                        Layout.fillWidth: true
                        spacing: 6

                        // On a computer, through the system's app for such links (KDE Connect hands them to your phone).
                        Button {
                            // texts: in Sioul's Texts page when texts are read here, else the system's app.
                            visible: line.modelData.texted && (section.canText || section.shown.texts)
                            width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                            text: section.sioul.text("calls-text-back")
                            icon.name: "mail-message-new"
                            icon.color: section.theme.text
                            display: section.narrow ? AbstractButton.IconOnly : AbstractButton.TextBesideIcon
                            ToolTip.visible: hovered && display === AbstractButton.IconOnly
                            ToolTip.text: text
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
                            ToolTip.text: text
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
                            ToolTip.text: text
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

                            visible: line.modelData.dial !== ""
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
                        if (line.modelData.dial !== "" && (event.key === Qt.Key_Menu || (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier)))) {
                            lineMenu.now().show(line.modelData, more)
                            event.accepted = true
                        }
                    }
                    TapHandler {
                        id: longPress

                        enabled: line.modelData.dial !== ""
                        acceptedDevices: PointerDevice.TouchScreen
                        onLongPressed: {
                            line.Window.window.menuAt = line.mapToItem(null, longPress.point.position.x, longPress.point.position.y)
                            lineMenu.now().show(line.modelData, null)
                        }
                    }
                    TapHandler {
                        enabled: line.modelData.dial !== ""
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

                        // Their list, Always through, their calls and messages: their sheet.
                        MenuItem {
                            text: section.sioul.text("attention-sheet-sender")
                            onTriggered: section.window.openPersonSheet("", JSON.stringify(["tel:" + lineMenuForm.line.dial]))
                        }
                        MenuItem {
                            visible: lineMenuForm.line !== null && !lineMenuForm.line.known
                            height: visible ? implicitHeight : 0
                            text: section.sioul.text("calls-add-contact")
                            icon.name: "contact-new"
                            icon.color: section.theme.text
                            onTriggered: section.window.newContactWith(lineMenuForm.line.number)
                        }
                        MenuItem {
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
