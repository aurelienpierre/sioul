// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic Button.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every Button of Sioul's windows: cpp/style.cpp gives this file wherever Qt
// would load the Basic style's own Button.qml (docs/qt-quick.md, "Sioul's
// overrides of the Basic style"). Basic's button, line for line, but for one
// thing: a flat button at rest keeps an outline in the theme's `edge`, so that
// no button is drawn as plain words. Filled, checked, highlighted and pressed
// buttons are drawn as Basic draws them, and every size is Basic's.

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.Button {
    id: control

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)

    padding: 6
    horizontalPadding: padding + 2
    spacing: 6

    icon.width: 24
    icon.height: 24

    contentItem: IconLabel {
        spacing: control.spacing
        mirrored: control.mirrored
        display: control.display

        icon: control.icon
        defaultIconColor: control.checked || control.highlighted ? control.palette.brightText
            : control.flat && !control.down ? (control.visualFocus ? control.palette.highlight
            : control.palette.windowText) : control.palette.buttonText
        text: control.text
        font: control.font
        color: defaultIconColor
    }

    background: Rectangle {
        // A flat button with nothing on it: no fill, an outline.
        readonly property bool bare: control.flat && !control.down && !control.checked && !control.highlighted
        // The window's theme (SioulWindow.qml); a window of Qt's own has none: its palette's border then.
        readonly property var shownIn: control.Window.window
        readonly property var theme: shownIn ? shownIn.theme : undefined
        // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
        readonly property var hints: Qt.styleHints
        readonly property bool highContrast: hints.accessibility ? hints.accessibility.contrastPreference === Qt.HighContrast : false
        readonly property color edge: theme ? theme.edge : control.palette.mid

        implicitWidth: 100
        implicitHeight: 40
        color: bare ? "transparent" : Color.blend(control.checked || control.highlighted ? control.palette.dark : control.palette.button,
                                                  control.palette.mid, control.down ? 0.5 : 0.0)
        border.color: control.visualFocus ? control.palette.highlight
            : bare ? (control.enabled ? edge : Color.blend(edge, control.palette.window, 0.6)) : control.palette.windowText
        border.width: control.visualFocus ? 2 : bare || highContrast ? 1 : 0
    }
}
