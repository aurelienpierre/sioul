// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic CheckBox.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every CheckBox of Sioul's windows (WrapCheckBox.qml's too): cpp/style.cpp
// gives this file wherever Qt would load the Basic style's own CheckBox.qml
// (docs/qt-quick.md, "Sioul's overrides of the Basic style"). Basic's box as
// Qt 6.9 to 6.11 write it, with nothing newer than 6.9, but for its box's
// border: the theme's `edge`, as every field's, and with the keyboard's focus 2
// points of its `focusEdge`. Its fill is the palette's `base`, the theme's
// `field`. Every size is Basic's.

import QtQuick
import QtQuick.Templates as T
import QtQuick.Controls.impl

T.CheckBox {
    id: control

    // The window's theme (SioulWindow.qml); a window of Qt's own has none: the palette's colours then.
    readonly property var shownIn: control.Window.window
    readonly property var sioulTheme: control.shownIn ? control.shownIn.theme : undefined
    // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
    readonly property var hints: Qt.styleHints
    readonly property bool highContrast: control.hints.accessibility ? control.hints.accessibility.contrastPreference === Qt.HighContrast : false

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding,
                             implicitIndicatorHeight + topPadding + bottomPadding)

    padding: 6
    spacing: 6

    indicator: Rectangle {
        implicitWidth: 28
        implicitHeight: 28

        x: control.text ? (control.mirrored ? control.width - width - control.rightPadding : control.leftPadding) : control.leftPadding + (control.availableWidth - width) / 2
        y: control.topPadding + (control.availableHeight - height) / 2

        color: control.down ? control.palette.light : control.palette.base
        border.width: control.visualFocus ? 2 : 1
        border.color: {
            if (control.visualFocus)
                return control.sioulTheme ? control.sioulTheme.focusEdge : control.palette.highlight
            else if (!control.highContrast)
                return control.sioulTheme ? (control.enabled ? control.sioulTheme.edge : Color.blend(control.sioulTheme.edge, control.palette.base, 0.6)) : control.palette.mid
            else
                return Color.blend(control.palette.dark, control.palette.base,
                                   control.enabled ? 0.0 : 0.5)
        }

        ColorImage {
            x: (parent.width - width) / 2
            y: (parent.height - height) / 2
            defaultColor: "#353637"
            color: control.palette.text
            source: "qrc:/qt-project.org/imports/QtQuick/Controls/Basic/images/check.png"
            visible: control.checkState === Qt.Checked
        }

        Rectangle {
            x: (parent.width - width) / 2
            y: (parent.height - height) / 2
            width: 16
            height: 3
            color: control.palette.text
            visible: control.checkState === Qt.PartiallyChecked
        }
    }

    contentItem: CheckLabel {
        leftPadding: control.indicator && !control.mirrored ? control.indicator.width + control.spacing : 0
        rightPadding: control.indicator && control.mirrored ? control.indicator.width + control.spacing : 0

        text: control.text
        font: control.font
        color: control.palette.windowText
    }
}
