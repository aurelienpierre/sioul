// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// When a meal or a nap was had, picked in two taps ("Had it at…", from its
// menu or its card): its hour among the hours of today up to now, then its
// minute, by five, never after now. The second tap gives the time
// (`picked`, "11:40") and closes; "Just now" gives "" at once. Nothing is
// asked beyond the time.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafePopup {
    id: popup

    required property var sioul
    required property var theme
    // Its name, said at its head ("Lunch").
    property string title: ""
    // Now on the clock, in minutes from midnight: no time after it is offered.
    property int nowMinute: 0
    // The hour tapped first; -1 until then.
    property int hour: -1

    signal picked(string time)

    function pad(n) {
        return n < 10 ? "0" + n : String(n)
    }

    // Asked for `title`, now at `minute` of the day.
    function ask(title, minute) {
        popup.title = title
        popup.nowMinute = minute
        popup.hour = -1
        popup.open()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    focus: true
    width: Math.min(popup.roomWidth - 2 * popup.safeGap, 360)
    padding: 14

    background: Rectangle {
        radius: popup.theme.radius
        color: popup.theme.background
        border.color: popup.theme.line
    }

    contentItem: ColumnLayout {
        spacing: 10

        Label {
            Layout.fillWidth: true
            text: popup.title !== "" ? popup.title + " · " + popup.sioul.text("need-had-when") : popup.sioul.text("need-had-when")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.weight: Font.DemiBold
            color: popup.theme.text
        }
        Button {
            objectName: "hadJustNow"
            visible: popup.hour < 0
            text: popup.sioul.text("need-had-just-now")
            onClicked: {
                popup.close()
                popup.picked("")
            }
        }
        // The hours of today up to now, the latest nine: the first tap.
        Flow {
            visible: popup.hour < 0
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: {
                    const last = Math.floor(popup.nowMinute / 60)
                    const out = []
                    for (let h = Math.max(0, last - 8); h <= last; h++)
                        out.push(h)
                    return out
                }

                delegate: Button {
                    required property int modelData

                    objectName: "hadHour"
                    text: popup.theme.plain(popup.pad(modelData) + ":…")
                    font.features: { "tnum": 1 }
                    onClicked: popup.hour = modelData
                }
            }
        }
        // Its minutes by five, never after now: the second tap gives the time.
        Flow {
            visible: popup.hour >= 0
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: {
                    const out = []
                    for (let m = 0; m < 60; m += 5)
                        if (popup.hour * 60 + m <= popup.nowMinute)
                            out.push(m)
                    return popup.hour >= 0 ? out : []
                }

                delegate: Button {
                    required property int modelData

                    objectName: "hadMinute"
                    text: popup.theme.plain(popup.pad(popup.hour) + ":" + popup.pad(modelData))
                    font.features: { "tnum": 1 }
                    onClicked: {
                        const time = popup.pad(popup.hour) + ":" + popup.pad(modelData)
                        popup.close()
                        popup.picked(time)
                    }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true

            Button {
                visible: popup.hour >= 0
                flat: true
                text: "◂"
                Accessible.name: popup.sioul.text("ui-back")
                onClicked: popup.hour = -1
            }
            Item {
                Layout.fillWidth: true
            }
            Button {
                flat: true
                text: popup.sioul.text("ui-cancel")
                onClicked: popup.close()
            }
        }
    }
}
