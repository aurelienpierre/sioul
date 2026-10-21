// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The card "What reaches you now" (part 5 of "What reaches you"; the
// proposal's 3.5 and 3.6), opened by the status line's button
// (ReachButton.qml): on a phone from the bottom, across the screen; on a
// computer under the title bar, at its end. In order:
// 1. now in two sentences: what the time is for, and what comes at once
//    (`reaches::now_card`);
// 2. the three states side by side, Reachable, As usual, Do not disturb:
//    choosing Reachable or Do not disturb shows its lengths, in rows that
//    wrap; As usual applies at once;
// 3. what the phone holds, by app, with Let them through now; on a
//    computer, Let what your phone holds through, without saying what;
// 4. Mail every minute (Real time);
// 5. the call screening, under its own heading, while a phone of yours
//    screens calls: its rule in a sentence, Expecting a call (1 h) or Screen
//    calls again, and, on the phone, its settings. Its on/off switch stays in
//    Settings ▸ This phone: one tap in a card opened often never turns spam
//    protection off.
// Every control is named for screen readers; nothing is drawn past the
// screen's edge at 412 points, in either language.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: card

    required property var sioul
    required property var theme
    // The button that opens it, which it is placed by.
    required property Item anchor
    // Under the anchor (a computer's title bar), else above it (a phone's status line at the bottom).
    property bool below: false
    // The status line's moment and its `dnd` part, as the button reads them.
    property var moment: ({})
    property var dnd: ({})
    // "reachable", "usual" or "dnd", and the state with its end in a sentence.
    property string reachState: "usual"
    property string stateLine: ""
    // What `reaches::now_card` said when it opened ({now, held, realtime, screening, calls}).
    property var view: ({ now: [], held: "", realtime: false, screening: "", calls: { shown: false, through: false, line: "" } })
    // The state whose lengths are shown: "reachable", "dnd", or none.
    property string choosing: ""
    readonly property bool android: Qt.platform.os === "android"
    readonly property bool paused: card.dnd.paused === true
    readonly property Item overlay: Overlay.overlay
    readonly property real room: card.overlay ? card.overlay.width : 412

    // Read again: when it opens, and when the moment changes while it is open.
    function read() {
        try {
            card.view = JSON.parse(card.sioul.reachCard())
        } catch (e) {
            card.view = { now: [], held: "", realtime: false, screening: "", calls: { shown: false, through: false, line: "" } }
        }
    }

    function usual() {
        card.choosing = ""
        if (card.dnd.on === true)
            card.sioul.dndToggle(false, 0)
        else if (card.reachState === "reachable")
            card.sioul.reachChoose("usual", "")
    }

    // A length chosen: the state pressed for it.
    function chooseLength(kind, length) {
        if (kind === "reachable")
            card.sioul.reachChoose("reachable", length)
        else
            card.sioul.dndToggle(true, parseInt(length))
        card.choosing = ""
    }

    // The lengths offered for a state ({key, label, name}), as the button's menu offered them.
    function lengths(kind) {
        const out = []
        if (kind === "reachable") {
            const capped = card.dnd.reach_capped || []
            const minutes = ["30", "60", "120"]
            for (let i = 0; i < minutes.length; ++i)
                out.push({ key: minutes[i], label: card.sioul.text("reach-len-" + minutes[i]), name: capped[i] === true ? card.sioul.textWith("reach-for-" + minutes[i] + "-latest", "time", card.dnd.reach_tonight_time || "") : card.sioul.text("reach-for-" + minutes[i]) })
            if ((card.dnd.reach_hours || 0) > 0)
                out.push({ key: "hours", label: card.sioul.textWith("reach-len-until", "time", card.dnd.reach_hours_time || ""), name: card.sioul.textWith("reach-until-time", "time", card.dnd.reach_hours_time || "") })
            if ((card.dnd.reach_tonight || 0) > 0) {
                const night = card.dnd.reach_at_night === true
                out.push({ key: "tonight", label: card.sioul.text(night ? "reach-len-waking" : "reach-len-tonight"), name: card.sioul.textWith(night ? "reach-until-waking" : "reach-until-tonight", "time", card.dnd.reach_tonight_time || "") })
                if (!night)
                    out.push({ key: "off", label: card.sioul.text("reach-len-off"), name: card.sioul.textWith("reach-until-off", "time", card.dnd.reach_tonight_time || "") })
            }
        } else {
            for (const m of ["30", "60", "120"])
                out.push({ key: m, label: card.sioul.text("reach-len-" + m), name: card.sioul.text("dnd-switch") + ", " + card.sioul.text("dnd-for-" + m) })
            if ((card.dnd.end_time || "") !== "")
                out.push({ key: "-1", label: card.sioul.textWith("reach-len-until", "time", card.dnd.end_time), name: card.sioul.text("dnd-switch") + ", " + card.sioul.textWith("dnd-until-time", "time", card.dnd.end_time) })
            out.push({ key: "0", label: card.sioul.text("reach-len-off"), name: card.sioul.text("dnd-switch") + ", " + card.sioul.text("dnd-until-off") })
        }
        return out
    }

    onMomentChanged: {
        if (card.opened)
            card.read()
    }
    onClosed: card.choosing = ""

    parent: card.overlay
    width: Math.min(420, card.room - 2 * card.theme.gap)
    // A phone: across its width; a computer: at the title bar's end.
    x: card.android || card.room < 600 ? Math.round((card.room - card.width) / 2) : card.room - card.width - card.theme.gap
    y: {
        if (!card.overlay)
            return 0
        const at = card.anchor.mapToItem(card.overlay, 0, 0)
        const wanted = card.below ? at.y + card.anchor.height + 6 : at.y - card.height - 6
        return Math.max(card.theme.gap, Math.min(wanted, card.overlay.height - card.height - card.theme.gap))
    }
    height: Math.min(column.implicitHeight + card.topPadding + card.bottomPadding, (card.overlay ? card.overlay.height : 800) - 2 * card.theme.gap - card.anchor.height - 6)
    padding: 12
    modal: false
    focus: true
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    background: Rectangle {
        color: card.theme.surface
        radius: card.theme.radius
        border.color: card.theme.line
    }

    contentItem: Flickable {
        id: flick

        Accessible.role: Accessible.Dialog
        Accessible.name: card.sioul.text("reach-card-title")
        contentWidth: width
        contentHeight: column.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        ColumnLayout {
            id: column

            width: flick.width
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: card.sioul.text("reach-card-title")
                font.weight: Font.DemiBold
                wrapMode: Text.Wrap
                color: card.theme.text
                Accessible.role: Accessible.Heading
            }
            // 1. Now.
            Repeater {
                model: card.view.now || []

                delegate: Label {
                    required property string modelData

                    Layout.fillWidth: true
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
            }

            // 2. The three states, side by side.
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: [
                        { id: "reachable", label: "reach-state-reachable", help: "reach-state-reachable-help" },
                        { id: "usual", label: "reach-usual", help: "reach-state-usual-help" },
                        { id: "dnd", label: "dnd-switch", help: "reach-state-dnd-help" },
                    ]

                    delegate: Button {
                        id: stateButton

                        required property var modelData
                        readonly property bool current: card.reachState === stateButton.modelData.id

                        objectName: "state-" + stateButton.modelData.id
                        Layout.fillWidth: true
                        // Three equal parts, whatever their words.
                        Layout.preferredWidth: 1
                        Layout.minimumWidth: 0
                        // Reachable is never offered during a pause.
                        enabled: !(stateButton.modelData.id === "reachable" && card.paused)
                        checkable: false
                        highlighted: stateButton.current
                        Accessible.name: card.theme.plain(card.sioul.text(stateButton.modelData.label))
                        // The state now said with its end; the others, what they do.
                        Accessible.description: card.theme.plain(stateButton.current ? card.stateLine : card.sioul.text(stateButton.modelData.help))
                        ToolTip.visible: hovered
                        ToolTip.text: card.theme.plain(stateButton.enabled ? card.sioul.text(stateButton.modelData.help) : card.sioul.text("reach-not-in-pause"))
                        ToolTip.delay: 600
                        contentItem: Label {
                            text: card.sioul.text(stateButton.modelData.label)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            horizontalAlignment: Text.AlignHCenter
                            verticalAlignment: Text.AlignVCenter
                            font.weight: stateButton.current ? Font.DemiBold : Font.Normal
                            color: stateButton.current ? card.theme.accent : stateButton.enabled ? card.theme.text : card.theme.muted
                        }
                        background: Rectangle {
                            radius: card.theme.radius
                            color: stateButton.down ? card.theme.pressed : stateButton.current ? Qt.rgba(card.theme.accent.r, card.theme.accent.g, card.theme.accent.b, card.theme.dark ? 0.24 : 0.16) : card.choosing === stateButton.modelData.id ? card.theme.hover : card.theme.button
                            border.width: stateButton.visualFocus ? 2 : card.choosing === stateButton.modelData.id ? 1 : 0
                            border.color: stateButton.visualFocus ? card.theme.focus : card.theme.accent
                        }
                        onClicked: {
                            if (stateButton.modelData.id === "usual")
                                card.usual()
                            else
                                card.choosing = card.choosing === stateButton.modelData.id ? "" : stateButton.modelData.id
                        }
                    }
                }
            }
            // Its lengths, once Reachable or Do not disturb is chosen: rows that wrap.
            Label {
                visible: card.choosing !== ""
                Layout.fillWidth: true
                text: card.sioul.text("reach-lengths")
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: card.theme.muted
            }
            Flow {
                id: lengthsFlow

                visible: card.choosing !== ""
                Layout.fillWidth: true
                spacing: 6

                Repeater {
                    model: card.choosing !== "" ? card.lengths(card.choosing) : []

                    delegate: Button {
                        id: lengthButton

                        required property var modelData

                        objectName: "length-" + card.choosing + "-" + lengthButton.modelData.key
                        leftPadding: 10
                        rightPadding: 10
                        topPadding: 4
                        bottomPadding: 4
                        text: card.theme.plain(lengthButton.modelData.label)
                        Accessible.name: card.theme.plain(lengthButton.modelData.name)
                        ToolTip.visible: hovered
                        ToolTip.text: card.theme.plain(lengthButton.modelData.name)
                        ToolTip.delay: 600
                        // Never wider than the card.
                        width: Math.min(implicitWidth, lengthsFlow.width)
                        onClicked: card.chooseLength(card.choosing, lengthButton.modelData.key)
                    }
                }
            }
            // Do not disturb: why it holds, where, and this device's own system.
            Label {
                visible: card.dnd.on === true && (card.dnd.why_line || "") !== ""
                Layout.fillWidth: true
                text: card.dnd.why_line || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: 12
                color: card.theme.muted
            }
            Button {
                objectName: "dnd-again"
                visible: card.dnd.on === true && card.dnd.here_off === true
                Layout.fillWidth: true
                flat: true
                text: card.theme.plain(card.sioul.text("dnd-again"))
                Accessible.name: card.theme.plain(card.sioul.text("dnd-again"))
                onClicked: card.sioul.dndChange("again", "{}")
            }

            // 3. What the phone holds.
            Label {
                visible: card.android && (card.view.held || "") !== ""
                Layout.fillWidth: true
                Layout.topMargin: 4
                text: card.view.held || ""
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: card.theme.text
            }
            Button {
                objectName: "let-through"
                visible: !card.paused && (card.android ? (card.view.held || "") !== "" : true)
                Layout.fillWidth: true
                flat: true
                text: card.theme.plain(card.sioul.text(card.android ? "reach-card-let-now" : "reach-let-through"))
                Accessible.name: card.theme.plain(card.sioul.text(card.android ? "reach-card-let-now" : "reach-let-through"))
                contentItem: Label {
                    text: card.theme.plain(card.sioul.text(card.android ? "reach-card-let-now" : "reach-let-through"))
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.accent
                }
                onClicked: {
                    card.sioul.letPhoneThrough()
                    card.read()
                }
            }

            // 4. Mail every minute.
            Switch {
                id: everyMinute

                objectName: "mail-every-minute"
                Layout.fillWidth: true
                text: card.sioul.text("reach-card-mail")
                checked: card.sioul.realtime
                Accessible.name: card.theme.plain(card.sioul.text("reach-card-mail"))
                Accessible.description: card.theme.plain(card.sioul.text("ui-realtime-help"))
                ToolTip.visible: hovered
                ToolTip.text: card.theme.plain(card.sioul.text("ui-realtime-help"))
                ToolTip.delay: 600
                contentItem: Label {
                    leftPadding: everyMinute.indicator.width + everyMinute.spacing
                    text: everyMinute.text
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    verticalAlignment: Text.AlignVCenter
                    color: card.theme.text
                }
                onToggled: {
                    card.sioul.setRealtimeMode(everyMinute.checked)
                    everyMinute.checked = Qt.binding(() => card.sioul.realtime)
                }
            }

            // 5. The call screening, apart.
            ColumnLayout {
                visible: card.view.calls !== undefined && card.view.calls.shown === true
                Layout.fillWidth: true
                Layout.topMargin: 4
                spacing: 4

                Label {
                    Layout.fillWidth: true
                    text: card.sioul.text("screening-title")
                    font.weight: Font.DemiBold
                    wrapMode: Text.Wrap
                    color: card.theme.text
                    Accessible.role: Accessible.Heading
                }
                Label {
                    Layout.fillWidth: true
                    text: card.view.screening || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.muted
                }
                Label {
                    visible: card.view.calls !== undefined && card.view.calls.through === true
                    Layout.fillWidth: true
                    text: card.view.calls !== undefined ? (card.view.calls.line || "") : ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: card.theme.accent
                }
                Flow {
                    id: screeningFlow

                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        objectName: "expecting"
                        readonly property bool through: card.view.calls !== undefined && card.view.calls.through === true

                        width: Math.min(implicitWidth, screeningFlow.width)
                        flat: true
                        text: card.theme.plain(card.sioul.text(through ? "calls-note-again" : "calls-note-hour"))
                        Accessible.name: card.theme.plain(card.sioul.text(through ? "calls-note-again" : "calls-note-hour"))
                        Accessible.description: through ? "" : card.theme.plain(card.sioul.text("calls-switch-tip-off"))
                        onClicked: {
                            card.sioul.callsThrough(!through, through ? 0 : 60)
                            card.read()
                        }
                    }
                    Button {
                        objectName: "screening-settings"
                        visible: card.android
                        width: Math.min(implicitWidth, screeningFlow.width)
                        flat: true
                        text: card.theme.plain(card.sioul.text("reach-card-screening-settings"))
                        Accessible.name: card.theme.plain(card.sioul.text("reach-card-screening-settings"))
                        onClicked: {
                            const window = card.anchor.Window.window
                            card.close()
                            if (window && window.showParameters)
                                window.showParameters("calls")
                        }
                    }
                }
            }
        }
    }
}
