// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic Button.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// A button drawn as its words or its icon alone, for what is not an action of
// the page: a row of a list or a panel (Mail's addresses among their
// folders), a line of a menu made of buttons, a button inside a field (a
// password's eye), a state that answers a press (a dose taken). A fold line
// is Fold.qml, made of one. Sioul's buttons that hold words keep an outline
// (qml/style/StyleButton.qml); these do not. Basic's flat button, as Qt 6.9
// to 6.11 write it: no ground at rest, a light one under the pointer, Basic's
// mid tone while pressed, and a ring in the highlight while the keyboard's
// focus is on it. Chosen (`checked` or `highlighted`: the topic shown, a
// line of a menu), it takes the window's one look for what is selected: the
// accent's frame, a light tint of it, bold words, and a ✓ before them among
// other lines that can be chosen. A colour icon keeps its own colours. The
// same properties and sizes as a Button.

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.Button {
    id: control

    // The window's theme (SioulWindow.qml); a window of Qt's own has none: its palette then.
    readonly property var sioulTheme: control.Window.window ? control.Window.window.theme : undefined
    readonly property bool selected: control.checked || control.highlighted
    // One line that can be chosen among several: a ✓ before its words when chosen.
    readonly property bool choice: control.autoExclusive
        || (control.checkable && control.parent !== null && Array.prototype.some.call(control.parent.children, c => c !== control && c.checkable === true && c.flat !== undefined))
    readonly property bool ownColours: control.sioulTheme !== undefined && control.sioulTheme.ownColours(control.icon.name)
    readonly property font boldFont: Qt.font(control.font.pixelSize > 0
        ? { family: control.font.family, pixelSize: control.font.pixelSize, italic: control.font.italic, capitalization: control.font.capitalization, weight: Font.DemiBold }
        : { family: control.font.family, pointSize: control.font.pointSize, italic: control.font.italic, capitalization: control.font.capitalization, weight: Font.DemiBold })
    readonly property color ink: !control.enabled ? (control.sioulTheme ? control.sioulTheme.muted : control.palette.placeholderText)
        : control.visualFocus && !control.down && !control.selected ? control.palette.highlight : control.palette.windowText

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
    icon.color: control.ink

    contentItem: IconLabel {
        spacing: control.spacing
        mirrored: control.mirrored
        display: control.display

        icon.name: control.icon.name
        icon.source: control.icon.source
        icon.width: control.icon.width
        icon.height: control.icon.height
        icon.cache: control.icon.cache
        icon.color: control.ownColours ? "transparent" : control.enabled ? control.icon.color : control.ink
        text: control.selected && control.choice ? "✓  " + control.text : control.text
        font: control.selected ? control.boldFont : control.font
        color: control.ink
    }

    background: Rectangle {
        readonly property var theme: control.sioulTheme

        implicitWidth: 100
        implicitHeight: 40
        radius: theme ? theme.radius : 0
        visible: control.selected || control.down || control.visualFocus || (control.hovered && control.enabled)
        color: control.down ? control.palette.mid
            : control.selected ? (theme ? theme.chosen : Color.blend(control.palette.window, control.palette.dark, 0.16))
            : control.hovered ? (theme ? theme.hover : control.palette.light) : "transparent"
        border.color: control.visualFocus ? control.palette.highlight : theme ? theme.accent : control.palette.dark
        border.width: control.visualFocus || control.selected ? 2 : 0
    }
}
