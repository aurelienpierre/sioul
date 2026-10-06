// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's own title bar, on a computer (main.qml): the status line
// (StatusLine.qml) across the whole width, at the top, where the system's
// title bar only took room. At its left end, over the places, the button that
// shows their names or keeps their icons only (F9); the window's buttons,
// minimize, maximize or restore, and close, on the side and in the order the
// system puts them (desktop.rs). Its empty space, the sentence included, moves
// the window when dragged and maximizes or restores it at a double click; the
// window's edges resize it (WindowEdges.qml). While a pause covers the
// window, only the window's buttons stay, and the bar to move it.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import com.aurelienpierre.sioul

Rectangle {
    id: bar

    required property var window
    required property var sioul
    required property var theme
    // The window's buttons on each side, in order: {left, right}, each
    // "minimize", "maximize" or "close", where the system puts them
    // (desktop.rs); in the documentation's pictures, minimize, maximize and
    // close on the right, wherever they are taken.
    readonly property var buttons: bar.sioul.grabFolder() !== "" ? ({ left: [], right: ["minimize", "maximize", "close"] }) : JSON.parse(desktop.windowButtons())
    // A pause covers the window (PauseCover.qml).
    property bool covered: false
    // The places' width beside the pages, which the names' button spans when
    // no window button is on the left: the status line starts where the pages do.
    property real placesWidth: 56
    // The places' buttons' margin on each side (Places.qml's `inset`).
    readonly property int inset: bar.window.placesNamed ? 8 : 6
    readonly property alias statusLine: line
    readonly property alias namesButton: namesToggle

    implicitHeight: 36
    color: bar.theme.surface
    Accessible.role: Accessible.TitleBar
    Accessible.name: "Sioul"

    // Dragged where nothing else takes the pointer: the window moves, as the
    // system's title bar moves it (it may restore a maximized window); not in full screen.
    DragHandler {
        id: mover

        target: null
        onActiveChanged: {
            if (mover.active && bar.window.visibility !== Window.FullScreen)
                bar.window.startSystemMove()
        }
    }
    // A double click: maximized, or back to its size; full screen left.
    TapHandler {
        acceptedButtons: Qt.LeftButton
        onDoubleTapped: bar.window.toggleMaximized()
    }

    // Where the system puts the window's buttons, asked once.
    Desktop {
        id: desktop
    }

    Rectangle {
        anchors.bottom: parent.bottom
        width: parent.width
        height: 1
        color: bar.theme.line
    }

    // One of the window's buttons: its icon, its name said on hover and to
    // screen readers; reached by Tab, never taking the focus from what you
    // write in at a click. As tall as the bar: maximized, the last one reaches
    // the screen's corner.
    component WindowButton: ToolButton {
        id: windowButton

        // "minimize", "maximize" or "close".
        required property string modelData
        readonly property bool full: bar.window.visibility === Window.FullScreen
        readonly property bool maximized: bar.window.visibility === Window.Maximized

        Layout.fillHeight: true
        Layout.bottomMargin: 1
        Layout.preferredWidth: 40
        focusPolicy: Qt.TabFocus
        padding: 0
        display: AbstractButton.IconOnly
        icon.width: 16
        icon.height: 16
        icon.color: bar.theme.text
        icon.name: windowButton.modelData === "minimize" ? "window-minimize" : windowButton.modelData === "close" ? "window-close" : windowButton.maximized || windowButton.full ? "window-restore" : "window-maximize"
        text: {
            if (windowButton.modelData === "minimize")
                return bar.sioul.text("titlebar-minimize")
            if (windowButton.modelData === "close")
                return bar.sioul.text(bar.window.trayHolds ? "titlebar-close-tray" : "titlebar-close")
            if (windowButton.full)
                return bar.sioul.text("titlebar-full-screen-leave")
            return bar.sioul.text(windowButton.maximized ? "titlebar-restore" : "titlebar-maximize")
        }
        Accessible.name: windowButton.text
        ToolTip.visible: windowButton.hovered
        ToolTip.text: windowButton.text
        ToolTip.delay: 600
        onClicked: {
            if (windowButton.modelData === "minimize")
                bar.window.showMinimized()
            else if (windowButton.modelData === "close")
                bar.window.close()
            else
                bar.window.toggleMaximized()
        }
        Keys.onReturnPressed: windowButton.clicked()
        Keys.onEnterPressed: windowButton.clicked()

        background: Rectangle {
            color: windowButton.down ? bar.theme.pressed : windowButton.hovered ? bar.theme.hover : "transparent"
            border.width: windowButton.visualFocus ? 2 : 0
            border.color: bar.theme.focus
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        Repeater {
            model: bar.buttons.left

            delegate: WindowButton {}
        }
        // The places' names beside their icons, or their icons only (F9), over
        // the places; a narrow window pulls them over the pages from ☰ instead.
        RailButton {
            id: namesToggle

            readonly property bool spans: bar.buttons.left.length === 0

            visible: !bar.window.compact && !bar.covered
            Layout.leftMargin: bar.inset
            Layout.rightMargin: bar.inset
            Layout.preferredWidth: namesToggle.spans ? Math.max(44, bar.placesWidth - 2 * bar.inset) : 44
            Layout.preferredHeight: 28
            Layout.alignment: Qt.AlignVCenter
            theme: bar.theme
            iconName: bar.window.placesNamed ? "sidebar-collapse-left" : "sidebar-expand-left"
            name: bar.sioul.text(bar.window.placesNamed ? "ui-places-names-hide" : "ui-places-names-show")
            keys: "F9"
            named: bar.window.placesNamed && namesToggle.spans
            onChosen: bar.window.togglePlaces(namesToggle.visualFocus)
        }
        // The status line; hidden under a pause, its place kept.
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true

            StatusLine {
                id: line

                anchors.fill: parent
                visible: !bar.covered
                window: bar.window
                sioul: bar.sioul
                theme: bar.theme
                atTop: true
            }
        }
        Repeater {
            model: bar.buttons.right

            delegate: WindowButton {}
        }
    }
}
