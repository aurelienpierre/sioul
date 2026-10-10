// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The status line (main.qml): one sentence about what happened last, with
// "Undo" for ten seconds after anything is moved, deleted or sent; what now
// is (quiet time, work kept late) and, behind a click, the way back, never
// suggested; the work day or the day offered to close at its time; the keys;
// do-not-disturb, the sounds and the weather; at its end, Free time and
// Pause, always in the same place. On a computer, it is the window's title
// bar, at the top (TitleBar.qml); on a phone, it is at the bottom. Narrow (a
// phone, a window as narrow), every button is its icon alone (LineButton.qml),
// its name said to screen readers and at a long press, and the sentence takes
// the rest of the line; cut short, a tap or a long press shows it whole, as
// the pointer resting on it does.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: line

    required property var window
    required property var sioul
    required property var theme
    // At the window's top: the pop-ups of its buttons open under it.
    property bool atTop: false
    // Icons alone: a phone, a window as narrow.
    readonly property bool compact: line.window.compact
    // Work time or quiet time, the day to close, the two pauses (main.qml).
    readonly property var moment: line.window.moment
    readonly property var offer: line.window.offer
    readonly property var pauses: line.window.pauses
    // What it says: what just happened (Undo's line while it waits). Narrow,
    // when nothing did, the day to close at its time, else what now is, whose
    // button is then its icon.
    readonly property string sentence: line.sioul.undoLine !== "" ? line.sioul.undoLine : line.sioul.status !== "" ? line.sioul.status : !line.compact ? "" : line.offer.kind !== "" ? line.offer.line : line.moment.line
    // For the documentation's pictures (main.qml's grabber).
    readonly property alias weatherApplet: weatherApplet
    readonly property alias modeMenu: modeMenu

    implicitHeight: 36

    // The moment's menu: from its line, or Free time's right click or long press.
    function openModeMenu() {
        modeMenu.active = true
        const menu = modeMenu.item as SioulMenu
        menu.popup()
    }

    // Free time's menu: back, the usual end kept, nothing at all; nothing
    // opens when none applies.
    function freeMenu() {
        if (line.moment.reason !== "free-time" && !line.pauses.can_keep)
            return
        line.openModeMenu()
    }

    // The sentence whole, for three seconds: a tap on it cut short (and the pictures').
    function sayWhole() {
        sentenceLabel.saying = true
        sentenceSaid.restart()
    }

    RowLayout {
        id: row

        anchors.fill: parent
        anchors.leftMargin: line.theme.gap
        // The pauses' place at the end is kept; what does not fit is cut, never over it.
        anchors.rightMargin: pausesRow.width + 2 * line.theme.gap
        spacing: line.compact ? 2 : line.theme.gap
        clip: true

        Label {
            id: sentenceLabel

            // Said whole after a tap on it, cut short.
            property bool saying: false

            Layout.fillWidth: true
            Layout.minimumWidth: 40
            // Narrow, it takes what the buttons leave, never asking for more.
            Layout.preferredWidth: line.compact ? 0 : -1
            text: line.sentence
            // Server answers and file names: never read as rich text.
            textFormat: Text.PlainText
            color: line.sioul.undoLine !== "" ? line.theme.text : line.theme.muted
            elide: Text.ElideRight
            // Never over the line beside it, however little room is left.
            clip: true
            ToolTip.visible: sentenceLabel.truncated && (sentenceHover.hovered || sentenceLabel.saying)
            ToolTip.text: line.theme.plain(sentenceLabel.text)
            ToolTip.delay: sentenceLabel.saying ? 0 : 600

            HoverHandler {
                id: sentenceHover
            }
            // A finger has no pointer to rest: a tap or a long press says it whole.
            TapHandler {
                acceptedDevices: PointerDevice.TouchScreen
                onTapped: line.sayWhole()
                onLongPressed: line.sayWhole()
            }
            Timer {
                id: sentenceSaid

                interval: 3000
                onTriggered: sentenceLabel.saying = false
            }

            // Wide, when nothing else is said, what can be closed now
            // (DayReview.qml): whole or not at all, and out of the layout,
            // which it never widens.
            Label {
                anchors.fill: parent
                visible: !line.compact && parent.text === "" && line.offer.line !== "" && parent.width >= implicitWidth
                text: line.offer.line
                textFormat: Text.PlainText
                verticalAlignment: Text.AlignVCenter
                color: line.theme.muted
            }
        }
        // Quiet time, or work kept late: the line, and behind it the way back,
        // never suggested. Narrow, its icon alone (its line is the sentence
        // when nothing else is said).
        LineButton {
            id: modeButton

            visible: line.moment.line !== "" && line.sioul.undoLine === ""
            // Room shared with what just happened, when something did; narrower
            // when the row is full (the pauses' buttons stay whole at its end).
            Layout.fillWidth: !line.compact
            // Wide, a sentence: it gives way, cut short, when the line is full,
            // down to its icon, which its menu stays behind.
            Layout.minimumWidth: line.compact ? implicitWidth : 16 + leftPadding + rightPadding
            Layout.maximumWidth: line.compact ? -1 : Math.round(line.window.width * (line.sioul.status !== "" ? 0.3 : 0.55))
            theme: line.theme
            compact: line.compact
            icon.name: "chronometer"
            text: line.moment.line
            textColor: line.theme.muted
            menuOpen: modeMenu.item !== null && (modeMenu.item as SioulMenu).opened
            onChosen: {
                // A meal or sleep offers no way back to work: they come first.
                if ((line.moment.time === "meals" || line.moment.time === "sleep") && line.moment.work_now !== true)
                    return
                line.openModeMenu()
            }
        }
        // A task just done: how it was, if you want to say; never asked again.
        LineButton {
            id: howButton

            visible: line.window.lastDone !== "" && line.sioul.undoLine === "" && line.sioul.status !== "" && line.sioul.status === line.window.lastDoneSaid
            theme: line.theme
            compact: line.compact
            icon.name: "smiley-add"
            text: line.sioul.text("felt-ask")
            onChosen: line.window.howWasIt(line.window.lastDone)
        }
        // Back from free time, the end of work said once: one key keeps the usual end, no reason asked.
        LineButton {
            visible: line.window.freeSaid !== "" && line.sioul.undoLine === "" && line.sioul.status === line.window.freeSaid && line.pauses.can_keep
            theme: line.theme
            compact: line.compact
            icon.name: "chronometer-reset"
            text: line.sioul.text("free-keep-end")
            onChosen: line.sioul.keepUsualEnd()
        }
        // The work day, or the day, can be closed (DayReview.qml): offered,
        // never pressed for you; nothing at work or while you sleep. Wide, it
        // waits while Undo does; narrow, its icon stays beside Undo's.
        LineButton {
            visible: line.offer.kind !== "" && (line.sioul.undoLine === "" || line.compact)
            theme: line.theme
            compact: line.compact
            // The same icons as the places' buttons that close them at any hour (Places.qml).
            icon.name: line.offer.kind === "night" ? "system-suspend" : "task-complete"
            text: line.offer.button
            // Narrow, its name first: its icon alone says little.
            tip: line.compact ? line.offer.button + "\n" + line.offer.line : line.offer.line
            onChosen: line.window.reviewDay(line.offer.kind)
        }
        // Ten seconds to change your mind.
        LineButton {
            visible: line.sioul.undoLine !== ""
            theme: line.theme
            compact: line.compact
            raised: true
            icon.name: "edit-undo"
            icon.color: line.theme.text
            text: line.sioul.text("ui-undo")
            tip: line.sioul.text("ui-undo") + " (Ctrl+Z)"
            onChosen: line.sioul.undo()
        }
        // The keys, when nothing else needs the room; cut short before the
        // buttons beside them are.
        Label {
            visible: line.moment.line === "" && !line.compact && !howButton.visible
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.maximumWidth: implicitWidth
            text: line.sioul.text("ui-keys")
            elide: Text.ElideRight
            color: line.theme.muted
            font.pixelSize: 12
        }
        // Do-not-disturb on every device (DndApplet.qml): its switch, apart
        // from the pauses' buttons at the line's end.
        DndApplet {
            sioul: line.sioul
            theme: line.theme
            moment: line.moment
            compact: line.compact
        }
        // Every call let through on your phone, while one screens them (CallsApplet.qml).
        CallsApplet {
            sioul: line.sioul
            theme: line.theme
            moment: line.moment
            compact: line.compact
        }
        // Sounds to focus or to rest by, never on by themselves.
        SoundsApplet {
            sioul: line.sioul
            theme: line.theme
            compact: line.compact
            below: line.atTop
        }
        // The weather, in one colour, at the place you chose.
        WeatherApplet {
            id: weatherApplet

            sioul: line.sioul
            theme: line.theme
            compact: line.compact
            below: line.atTop
        }
    }

    // The two pauses (docs/pauses.md): a place of their own at the line's end,
    // the same every time, never pushed out by what the line holds.
    RowLayout {
        id: pausesRow

        anchors.right: parent.right
        anchors.rightMargin: line.theme.gap
        anchors.verticalCenter: parent.verticalCenter
        spacing: line.compact ? 2 : 4

        // Free time (docs/pauses.md): leisure whatever the hour, a switch
        // showing its state; its menu at a right click or a long press.
        LineButton {
            id: freeButton

            readonly property bool isOn: line.moment.reason === "free-time"

            // Never squeezed by a long status line: always reachable.
            Layout.minimumWidth: implicitWidth
            theme: line.theme
            compact: line.compact
            checkable: true
            checked: freeButton.isOn
            switchedOn: freeButton.isOn
            tipOnHold: false
            icon.name: "flower-shape"
            text: line.sioul.text("free-time")
            name: freeButton.isOn ? line.sioul.text("free-time-back") : line.sioul.text("free-time")
            tip: freeButton.isOn ? line.sioul.text("free-time-back") : line.sioul.text("free-time-tip")
            menuOpen: modeMenu.item !== null && (modeMenu.item as SioulMenu).opened
            onChosen: {
                line.window.setFreeTime(!freeButton.isOn)
                freeButton.checked = Qt.binding(() => freeButton.isOn)
            }

            TapHandler {
                acceptedButtons: Qt.RightButton
                // A touch has no buttons: on a touch screen, the long press below.
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                onTapped: line.freeMenu()
            }
            TapHandler {
                id: freeHold

                acceptedDevices: PointerDevice.TouchScreen
                onLongPressed: {
                    freeButton.held = true
                    line.window.menuAt = freeHold.point.scenePosition
                    line.freeMenu()
                }
            }
        }
        // Pause (docs/pauses.md): apart from the rest, at the row's end, the
        // same place every time; it asks nothing.
        LineButton {
            Layout.leftMargin: line.compact ? 2 : line.theme.gap
            Layout.minimumWidth: implicitWidth
            theme: line.theme
            compact: line.compact
            icon.name: "media-playback-pause"
            text: line.sioul.text("pause-button")
            tip: line.sioul.text("pause-tip")
            onChosen: line.sioul.pauseNow()
        }
    }

    // The moment's menu, made the first time it is opened.
    Loader {
        id: modeMenu

        active: false
        sourceComponent: SioulMenu {
            // Free time (docs/pauses.md): back from it, the usual end kept, nothing at all.
            MenuItem {
                visible: line.moment.reason === "free-time"
                height: visible ? implicitHeight : 0
                text: line.sioul.text("free-time-back")
                onTriggered: line.window.setFreeTime(false)
            }
            MenuItem {
                visible: line.pauses.can_keep
                height: visible ? implicitHeight : 0
                text: line.theme.plain(line.sioul.textWith("free-menu-keep", "time", line.pauses.usual_end))
                onTriggered: line.sioul.keepUsualEnd()
            }
            MenuItem {
                id: nothingItem

                visible: line.moment.reason === "free-time"
                height: visible ? implicitHeight : 0
                text: line.sioul.text("free-menu-nothing")
                checkable: true
                checked: line.pauses.nothing
                onTriggered: {
                    line.sioul.setFreeNothing(!line.pauses.nothing)
                    nothingItem.checked = Qt.binding(() => line.pauses.nothing)
                }
            }
            // The day closed today can be taken back, that day.
            MenuItem {
                visible: line.moment.reason === "done-for-the-day" && line.moment.today
                height: visible ? implicitHeight : 0
                text: line.sioul.text("mode-back-to-plan")
                onTriggered: line.sioul.usualHours()
            }
            MenuItem {
                visible: line.moment.reason === "working-late"
                height: visible ? implicitHeight : 0
                text: line.sioul.text("mode-usual-hours")
                onTriggered: line.sioul.usualHours()
            }
            // Work shown whatever the hours, as on the Porch; not during a
            // meal or sleep, which come first.
            MenuItem {
                id: workNowItem

                visible: (line.moment.quiet && line.moment.time !== "meals" && line.moment.time !== "sleep" && line.moment.reason !== "free-time") || line.moment.work_now === true
                height: visible ? implicitHeight : 0
                text: line.sioul.text("mode-work-now")
                checkable: true
                checked: line.moment.work_now === true
                onTriggered: {
                    line.sioul.setWorkNow(!(line.moment.work_now === true))
                    workNowItem.checked = Qt.binding(() => line.moment.work_now === true)
                }
            }
            Repeater {
                model: line.moment.quiet && line.moment.time !== "meals" && line.moment.time !== "sleep" && line.moment.reason !== "free-time" && !(line.moment.reason === "done-for-the-day" && line.moment.today) ? [30, 60, 120, 240] : []

                delegate: MenuItem {
                    required property int modelData

                    text: line.sioul.text("mode-work-" + modelData)
                    onTriggered: line.sioul.workAWhile(modelData)
                }
            }
        }
    }
}
