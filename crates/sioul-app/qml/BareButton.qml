// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic Button.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// A button drawn as its words or its icon alone, for what is not an action of
// the page: a row of a list or a panel (Mail's addresses among their
// folders), a fold line ("▸ Server and folders"), a line of a menu made of
// buttons, a button inside a field (a password's eye), a state that answers
// a press (a dose taken). Sioul's flat buttons that hold words keep an outline
// (qml/style/StyleButton.qml); these do not. Basic's flat button, as Qt 6.9 to
// 6.11 write it: no ground at rest; filled when pressed, checked or
// highlighted; and, unlike Basic's flat one, a ring in the highlight while
// the keyboard's focus is on it. The same properties and sizes as a Button.

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
    flat: true

    icon.width: 24
    icon.height: 24
    icon.color: control.checked || control.highlighted ? control.palette.brightText :
                !control.down ? (control.visualFocus ? control.palette.highlight : control.palette.windowText) : control.palette.buttonText

    contentItem: IconLabel {
        spacing: control.spacing
        mirrored: control.mirrored
        display: control.display

        icon: control.icon
        text: control.text
        font: control.font
        color: control.checked || control.highlighted ? control.palette.brightText :
               !control.down ? (control.visualFocus ? control.palette.highlight : control.palette.windowText) : control.palette.buttonText
    }

    background: Rectangle {
        readonly property bool filled: control.down || control.checked || control.highlighted

        implicitWidth: 100
        implicitHeight: 40
        visible: filled || control.visualFocus
        color: filled ? Color.blend(control.checked || control.highlighted ? control.palette.dark : control.palette.button,
                                    control.palette.mid, control.down ? 0.5 : 0.0) : "transparent"
        border.color: control.palette.highlight
        border.width: control.visualFocus ? 2 : 0
    }
}
