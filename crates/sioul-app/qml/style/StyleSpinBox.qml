// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic SpinBox.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every SpinBox of Sioul's windows: cpp/style.cpp gives this file wherever Qt
// would load the Basic style's own SpinBox.qml (docs/qt-quick.md, "Sioul's
// overrides of the Basic style"). Basic's as Qt 6.9 to 6.11 write it, with
// nothing newer than 6.9, but for its border and its menu: a number you type
// is a field, the palette's `base` (the theme's `field`) within the theme's
// `edge`, and with the focus 2 points of its `focusEdge`; the menu (cut,
// copy, paste) as StyleTextField.qml's. Every size is Basic's.

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.SpinBox {
    id: control

    // The window's theme (SioulWindow.qml); a window of Qt's own has none: the palette's colours then.
    readonly property var shownIn: control.Window.window
    readonly property var sioulTheme: control.shownIn ? control.shownIn.theme : undefined
    // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
    readonly property var hints: Qt.styleHints
    readonly property bool highContrast: control.hints.accessibility ? control.hints.accessibility.contrastPreference === Qt.HighContrast : false

    // Note: the width of the indicators are calculated into the padding
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            contentItem.implicitWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding,
                             up.implicitIndicatorHeight, down.implicitIndicatorHeight)

    leftPadding: padding + (control.mirrored ? (up.indicator ? up.indicator.width : 0) : (down.indicator ? down.indicator.width : 0))
    rightPadding: padding + (control.mirrored ? (down.indicator ? down.indicator.width : 0) : (up.indicator ? up.indicator.width : 0))

    validator: IntValidator {
        locale: control.locale.name
        bottom: Math.min(control.from, control.to)
        top: Math.max(control.from, control.to)
    }

    contentItem: TextInput {
        id: input

        z: 2
        text: control.displayText
        clip: width < implicitWidth
        padding: 6

        font: control.font
        color: control.palette.text
        selectionColor: control.palette.highlight
        selectedTextColor: control.palette.highlightedText
        horizontalAlignment: Qt.AlignHCenter
        verticalAlignment: Qt.AlignVCenter

        readOnly: !control.editable
        validator: control.validator
        inputMethodHints: control.inputMethodHints

        // Connected only where the menu exists (StyleTextField.qml).
        Connections {
            target: control.sioulTheme && control.sioulTheme.fieldMenu ? input.T.ContextMenu : null

            function onRequested() {
                if (input.T.ContextMenu.menu === null)
                    input.T.ContextMenu.menu = control.sioulTheme.fieldMenuFor(input)
            }
        }

        Rectangle {
            width: parent.width
            height: parent.height
            visible: control.activeFocus
            color: "transparent"
            border.color: control.sioulTheme ? control.sioulTheme.focusEdge : control.palette.highlight
            border.width: 2
        }
    }

    up.indicator: Rectangle {
        x: control.mirrored ? 0 : control.width - width
        height: control.height
        implicitWidth: 40
        implicitHeight: 40
        color: control.up.pressed ? control.palette.mid : control.palette.button
        border.color: enabled ? control.palette.text : control.palette.mid
        border.width: control.highContrast ? 1 : 0

        Rectangle {
            x: (parent.width - width) / 2
            y: (parent.height - height) / 2
            width: parent.width / 3
            height: 2
            color: enabled ? control.palette.buttonText : control.palette.mid
        }
        Rectangle {
            x: (parent.width - width) / 2
            y: (parent.height - height) / 2
            width: 2
            height: parent.width / 3
            color: enabled ? control.palette.buttonText : control.palette.mid
        }
    }

    down.indicator: Rectangle {
        x: control.mirrored ? parent.width - width : 0
        height: control.height
        implicitWidth: 40
        implicitHeight: 40
        color: control.down.pressed ? control.palette.mid : control.palette.button
        border.color: enabled ? control.palette.text : control.palette.mid
        border.width: control.highContrast ? 1 : 0

        Rectangle {
            x: (parent.width - width) / 2
            y: (parent.height - height) / 2
            width: parent.width / 3
            height: 2
            color: enabled ? control.palette.buttonText : control.palette.mid
        }
    }

    background: Rectangle {
        implicitWidth: 140
        color: enabled ? control.palette.base : control.palette.button
        border.color: {
            if (control.highContrast)
                return enabled ? control.palette.text : control.palette.mid
            return control.sioulTheme ? (enabled ? control.sioulTheme.edge : Color.blend(control.sioulTheme.edge, control.palette.base, 0.6)) : control.palette.button
        }
    }
}
