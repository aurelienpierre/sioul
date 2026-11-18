// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A window of Sioul: the theme's colours given to Qt's controls, the same in
// every window (the main one, writing, focus, portals). What you type in has
// a fill of its own, the theme's `field` (Qt's `base`: text fields, boxes to
// tick, spin boxes); its border and a flat button's outline, the theme's
// `edge`, are drawn by Sioul's overrides of the Basic style, which read the
// window's `theme` (qml/style/, cpp/style.cpp).

import QtQuick
import QtQuick.Controls.Basic

ApplicationWindow {
    id: root

    // The window's colours, sizes and fonts (Theme.qml). Its colours are given to Qt's
    // controls in the window.
    required property var theme

    color: root.theme.background
    font.pixelSize: 15

    palette.window: root.theme.background
    palette.windowText: root.theme.text
    palette.base: root.theme.field
    palette.alternateBase: root.theme.background
    palette.text: root.theme.text
    palette.button: root.theme.button
    palette.buttonText: root.theme.text
    // Checked and highlighted buttons: light text on the accent.
    palette.brightText: root.theme.accentText
    palette.dark: root.theme.accent
    palette.mid: root.theme.pressed
    // A hovered menu entry draws windowText on it, a hovered list entry highlightedText.
    palette.light: root.theme.hover
    palette.midlight: root.theme.line
    palette.shadow: root.theme.line
    palette.highlight: root.theme.selection
    palette.highlightedText: root.theme.text
    palette.placeholderText: root.theme.muted
    palette.toolTipBase: root.theme.surface
    palette.toolTipText: root.theme.text
    palette.link: root.theme.accent
    palette.linkVisited: root.theme.accent
}
