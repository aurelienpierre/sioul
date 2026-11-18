// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic TextField.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every TextField of Sioul's windows: cpp/style.cpp gives this file wherever
// Qt would load the Basic style's own TextField.qml (docs/qt-quick.md, "Sioul's
// overrides of the Basic style"). Basic's field as Qt 6.9 to 6.11 write it,
// with nothing newer than 6.9, but for its border and its menu. The border is
// the theme's firmer `edge` at rest; with the focus, 2 points of its
// `focusEdge`, the accent's hue at 3:1 or more against the border at rest and
// the fill. The fill is the palette's `base`, which SioulWindow.qml gives the
// theme's `field`. Cut, copy and paste come on a right click or a long press
// where Qt has the menu for them (TextEditingContextMenu, Qt 6.11): made at the
// first request, then kept; with an older Qt, as with Basic's own there, none,
// and the click goes on to what is under the field. Every size is Basic's.

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.TextField {
    id: control

    // The window's theme (SioulWindow.qml); a window of Qt's own has none: the palette's colours then.
    readonly property var shownIn: control.Window.window
    readonly property var sioulTheme: control.shownIn ? control.shownIn.theme : undefined

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

    // Connected only where the menu exists: a request heard and left without a menu
    // would take the click from what is under the field (Qt 6.10).
    Connections {
        target: control.sioulTheme && control.sioulTheme.fieldMenu ? control.T.ContextMenu : null

        function onRequested() {
            if (control.T.ContextMenu.menu === null)
                control.T.ContextMenu.menu = control.sioulTheme.fieldMenuFor(control)
        }
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
        readonly property color edge: control.sioulTheme ? control.sioulTheme.edge : control.palette.mid
        readonly property color focusEdge: control.sioulTheme ? control.sioulTheme.focusEdge : control.palette.highlight
        // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
        readonly property var hints: Qt.styleHints
        readonly property bool highContrast: hints.accessibility ? hints.accessibility.contrastPreference === Qt.HighContrast : false

        implicitWidth: 200
        implicitHeight: 40
        border.width: control.activeFocus ? 2 : 1
        color: control.palette.base
        border.color: {
            if (control.activeFocus)
                return focusEdge
            else if (!highContrast)
                return control.enabled ? edge : Color.blend(edge, control.palette.base, 0.6)
            else
                return Color.blend(control.palette.text, control.palette.base,
                                   control.enabled ? 0.0 : 0.5)
        }
    }
}
