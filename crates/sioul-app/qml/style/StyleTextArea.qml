// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic TextArea.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every TextArea of Sioul's windows: cpp/style.cpp gives this file wherever
// Qt would load the Basic style's own TextArea.qml (docs/qt-quick.md, "Sioul's
// overrides of the Basic style"). Basic draws a text area as bare words, with
// no ground and no border but in high contrast; here one you can type in is a
// field, as a TextField: the palette's `base` (the theme's `field`) and a
// border in the theme's `edge`. A read-only one (a text to select and copy)
// stays bare. A text area that brings its own background keeps it; every size
// is Basic's (the background has no size of its own).

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.TextArea {
    id: control

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

    // Cut, copy and paste on a right click or a long press, as Basic's own has
    // them since Qt 6.11 (its TextEditingContextMenu): made at the first
    // request, then kept; none with an older Qt, as none in Basic's own there.
    T.ContextMenu.onRequested: {
        if (T.ContextMenu.menu !== null)
            return
        const made = Qt.createComponent("QtQuick.Controls.Basic.impl", "TextEditingContextMenu")
        if (made.status === Component.Ready)
            T.ContextMenu.menu = made.createObject(control, { editor: control })
    }

    background: Rectangle {
        // The window's theme (SioulWindow.qml); a window of Qt's own has none: its palette's border then.
        readonly property var shownIn: control.Window.window
        readonly property var theme: shownIn ? shownIn.theme : undefined
        // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
        readonly property var hints: Qt.styleHints
        readonly property bool highContrast: hints.accessibility ? hints.accessibility.contrastPreference === Qt.HighContrast : false
        readonly property color edge: theme ? theme.edge : control.palette.mid

        visible: !control.readOnly || highContrast
        color: control.readOnly ? "transparent" : control.palette.base
        border.width: control.activeFocus ? 2 : 1
        border.color: control.activeFocus ? control.palette.highlight
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
