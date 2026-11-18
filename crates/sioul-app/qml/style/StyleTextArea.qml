// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic TextArea.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every TextArea of Sioul's windows: cpp/style.cpp gives this file wherever
// Qt would load the Basic style's own TextArea.qml (docs/qt-quick.md, "Sioul's
// overrides of the Basic style"). Basic's text area as Qt 6.9 to 6.11 write it,
// with nothing newer than 6.9, but for its background and its menu. Basic
// draws a text area as bare words, with no ground and no border but in high
// contrast; here one you can type in is a field, as a TextField: the
// palette's `base` (the theme's `field`), a border in the theme's `edge`, and
// with the focus 2 points of its `focusEdge`. A read-only one (a text to
// select and copy) stays bare. A text area that brings its own background
// keeps it; every size is Basic's (the background has no size of its own).
// The menu (cut, copy, paste) as StyleTextField.qml's.

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.TextArea {
    id: control

    // The window's theme (SioulWindow.qml); a window of Qt's own has none: the palette's colours then.
    readonly property var shownIn: control.Window.window
    readonly property var sioulTheme: control.shownIn ? control.shownIn.theme : undefined

    implicitWidth: Math.max(contentWidth + leftPadding + rightPadding,
                            implicitBackgroundWidth + leftInset + rightInset,
                            placeholder.implicitWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(contentHeight + topPadding + bottomPadding,
                             implicitBackgroundHeight + topInset + bottomInset,
                             placeholder.implicitHeight + topPadding + bottomPadding)

    padding: 6
    leftPadding: padding + 4

    color: control.palette.text
    placeholderTextColor: control.palette.placeholderText
    selectionColor: control.palette.highlight
    selectedTextColor: control.palette.highlightedText

    // Connected only where the menu exists (StyleTextField.qml).
    Connections {
        target: control.sioulTheme && control.sioulTheme.fieldMenu ? control.T.ContextMenu : null

        function onRequested() {
            if (control.T.ContextMenu.menu === null)
                control.T.ContextMenu.menu = control.sioulTheme.fieldMenuFor(control)
        }
    }

    background: Rectangle {
        readonly property color edge: control.sioulTheme ? control.sioulTheme.edge : control.palette.mid
        readonly property color focusEdge: control.sioulTheme ? control.sioulTheme.focusEdge : control.palette.highlight
        // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
        readonly property var hints: Qt.styleHints
        readonly property bool highContrast: hints.accessibility ? hints.accessibility.contrastPreference === Qt.HighContrast : false

        visible: !control.readOnly || highContrast
        color: control.readOnly ? "transparent" : control.palette.base
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? focusEdge
            : highContrast ? (control.enabled ? control.palette.mid : control.palette.midlight)
            : control.enabled ? edge : Color.blend(edge, control.palette.base, 0.6)
    }

    PlaceholderText {
        id: placeholder

        x: control.leftPadding
        y: control.topPadding
        width: control.width - (control.leftPadding + control.rightPadding)
        height: control.height - (control.topPadding + control.bottomPadding)

        text: control.placeholderText
        font: control.font
        color: control.placeholderTextColor
        verticalAlignment: control.verticalAlignment
        visible: !control.length && !control.preeditText && (!control.activeFocus || control.horizontalAlignment !== Qt.AlignHCenter)
        elide: Text.ElideRight
        renderType: control.renderType
    }
}
