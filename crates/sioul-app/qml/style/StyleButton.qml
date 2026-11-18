// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic Button.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every Button of Sioul's windows: cpp/style.cpp gives this file wherever Qt
// would load the Basic style's own Button.qml (docs/qt-quick.md, "Sioul's
// overrides of the Basic style"). Basic's button as Qt 6.9 and 6.10 write it,
// which Qt 6.11 loads too (no type, property or signal newer than 6.9: the
// icon tinted by `icon.color`, not 6.11's `IconLabel.defaultIconColor`; the
// system's high contrast read only where Qt has it), but for its background.
// A flat button at rest that holds words keeps an outline in the theme's
// `edge`, so that no action is drawn as plain words; a flat button of an icon
// alone stays bare, as do the rows, fold lines and menus that are buttons
// (BareButton.qml). Filled, checked, highlighted and pressed buttons are drawn
// as Basic draws them, and every size is Basic's.

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
    icon.color: control.checked || control.highlighted ? control.palette.brightText :
                control.flat && !control.down ? (control.visualFocus ? control.palette.highlight : control.palette.windowText) : control.palette.buttonText

    contentItem: IconLabel {
        spacing: control.spacing
        mirrored: control.mirrored
        display: control.display

        icon: control.icon
        text: control.text
        font: control.font
        color: control.checked || control.highlighted ? control.palette.brightText :
               control.flat && !control.down ? (control.visualFocus ? control.palette.highlight : control.palette.windowText) : control.palette.buttonText
    }

    background: Rectangle {
        // A flat button with nothing on it: no fill; an outline when it holds words (a
        // letter or a digit: an arrow or a cross alone is an icon, as a button showing its icon only).
        readonly property bool bare: control.flat && !control.down && !control.checked && !control.highlighted
        readonly property bool outlined: bare && control.display !== T.AbstractButton.IconOnly && /[0-9A-Za-z\u00C0-\u024F\u0370-\u03FF\u0400-\u04FF]/.test(control.text)
        // The window's theme (SioulWindow.qml); a window of Qt's own has none: its palette's border then.
        readonly property var shownIn: control.Window.window
        readonly property var theme: shownIn ? shownIn.theme : undefined
        readonly property color edge: theme ? theme.edge : control.palette.mid
        // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
        readonly property var hints: Qt.styleHints
        readonly property bool highContrast: hints.accessibility ? hints.accessibility.contrastPreference === Qt.HighContrast : false

        implicitWidth: 100
        implicitHeight: 40
        visible: !bare || outlined || control.visualFocus || highContrast
        color: bare ? "transparent" : Color.blend(control.checked || control.highlighted ? control.palette.dark : control.palette.button,
                                                  control.palette.mid, control.down ? 0.5 : 0.0)
        border.color: control.visualFocus ? control.palette.highlight
            : outlined ? (control.enabled ? edge : Color.blend(edge, control.palette.window, 0.6)) : control.palette.windowText
        border.width: control.visualFocus ? 2 : outlined || highContrast ? 1 : 0
    }
}
