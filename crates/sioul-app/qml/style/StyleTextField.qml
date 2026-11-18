// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic TextField.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every TextField of Sioul's windows: cpp/style.cpp gives this file wherever
// Qt would load the Basic style's own TextField.qml (docs/qt-quick.md, "Sioul's
// overrides of the Basic style"). Basic's field, line for line, but for its
// border: the theme's firmer `edge` rather than the palette's mid tone. Its
// fill is the palette's `base`, which SioulWindow.qml gives the theme's
// `field`; every size is Basic's.

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.TextField {
    id: control

    implicitWidth: implicitBackgroundWidth + leftInset + rightInset
                   || Math.max(contentWidth, placeholder.implicitWidth) + leftPadding + rightPadding
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             contentHeight + topPadding + bottomPadding,
                             placeholder.implicitHeight + topPadding + bottomPadding)

    padding: 6
    leftPadding: padding + 4

    color: control.palette.text
    selectionColor: control.palette.highlight
    selectedTextColor: control.palette.highlightedText
    placeholderTextColor: control.palette.placeholderText
    verticalAlignment: TextInput.AlignVCenter

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

    background: Rectangle {
        // The window's theme (SioulWindow.qml); a window of Qt's own has none: its palette's border then.
        readonly property var shownIn: control.Window.window
        readonly property var theme: shownIn ? shownIn.theme : undefined
        // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
        readonly property var hints: Qt.styleHints
        readonly property bool highContrast: hints.accessibility ? hints.accessibility.contrastPreference === Qt.HighContrast : false
        readonly property color edge: theme ? theme.edge : control.palette.mid

        implicitWidth: 200
        implicitHeight: 40
        border.width: control.activeFocus ? 2 : 1
        color: control.palette.base
        border.color: {
            if (control.activeFocus)
                return control.palette.highlight
            else if (!highContrast)
                return control.enabled ? edge : Color.blend(edge, control.palette.base, 0.6)
            else
                return Color.blend(control.palette.text, control.palette.base,
                                   control.enabled ? 0.0 : 0.5)
        }
    }
}
