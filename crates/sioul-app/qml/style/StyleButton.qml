// SPDX-License-Identifier: LGPL-3.0-only OR GPL-2.0-only OR GPL-3.0-only
// Copyright (C) 2017 The Qt Company Ltd.: Qt's Basic Button.qml, which this file changes, under the licences Qt gives it.
// Copyright © 2026 Aurélien Pierre

// Every Button of Sioul's windows: cpp/style.cpp gives this file wherever Qt
// would load the Basic style's own Button.qml (docs/qt-quick.md, "Sioul's
// overrides of the Basic style" and "The look of each kind of thing").
// Basic's button as Qt 6.9 and 6.10 write it, which Qt 6.11 loads too (no
// type, property or signal newer than 6.9: the icon tinted by `icon.color`,
// not 6.11's `IconLabel.defaultIconColor`; the system's high contrast read
// only where Qt has it), but for how it looks. Four looks, whatever the page
// sets `flat` to:
// - the main action (`highlighted`, not `checked`): filled with the accent;
// - selected (`checked`, whatever `highlighted` says): the accent's frame, a
//   light tint of it, bold words, and a ✓ before them when the page says it is
//   one choice among several (`autoExclusive`, or a ButtonGroup); its width is
//   the selected one's at all times, so that nothing moves as the choice changes;
// - any other button with words, or a lone arrow to go back and forth (◂ ▸):
//   outlined in the theme's `edge` on the page's own surface, never filled (a
//   fill is what a field has);
// - an icon alone, a cross or a sign alone: bare, a light ground under the
//   pointer, a ring with the keyboard's focus (BareButton.qml for rows, fold
//   lines and menus).
// Disabled, it shows it: muted words, a fainter outline, no fill. A colour
// icon (Theme.qml's `ownColours`) keeps its own colours; a symbolic one is
// tinted as the words, and in the words' colour whatever the page asks on the
// selected tint and the accent's fill. Every size is Basic's.

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.Button {
    id: control

    // The window's theme (SioulWindow.qml); a window of Qt's own has none: its palette then.
    readonly property var shownIn: control.Window.window
    readonly property var sioulTheme: control.shownIn ? control.shownIn.theme : undefined
    // Words on it, outlined: a letter or a digit (× and ÷ are signs, not letters), or a
    // lone arrow to go back and forth (◂ ▸ ‹ › « »), which is a button too, not a bullet.
    // A cross, a sign or any other symbol alone is an icon, bare.
    readonly property bool hasWords: control.display !== T.AbstractButton.IconOnly
        && (/[0-9A-Za-zÀ-ÖØ-öø-ɏͰ-ϿЀ-ӿ]/.test(control.text) || /^\s*[◂▸◀▶‹›«»]\s*$/.test(control.text))
    // The main action; a page that also checks it means a state: it is drawn selected.
    readonly property bool mainAction: control.highlighted && !control.checked && control.enabled
    readonly property bool selected: control.checked
    // One choice among several, said by the page (`autoExclusive`, or a ButtonGroup): a ✓
    // before its words when selected. A toggle on its own (Filters, Select) has none.
    readonly property bool choice: control.autoExclusive || (control.checkable && control.T.ButtonGroup.group !== null)
    // Its icon in its own colours: a colour icon would become a solid shape under a tint.
    readonly property bool ownColours: control.sioulTheme !== undefined && control.sioulTheme.ownColours(control.icon.name)
    // Its words selected: bold, in the family and size the page gave.
    readonly property font boldFont: Qt.font(control.font.pixelSize > 0
        ? { family: control.font.family, pixelSize: control.font.pixelSize, italic: control.font.italic, capitalization: control.font.capitalization, weight: Font.DemiBold }
        : { family: control.font.family, pointSize: control.font.pointSize, italic: control.font.italic, capitalization: control.font.capitalization, weight: Font.DemiBold })
    // The words' colour, the icon's when it is tinted.
    readonly property color ink: !control.enabled ? (control.sioulTheme ? control.sioulTheme.muted : control.palette.placeholderText)
        : control.mainAction ? control.palette.brightText
        : control.visualFocus && !control.down && !control.selected ? control.palette.highlight : control.palette.windowText

    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)

    padding: 6
    horizontalPadding: padding + 2
    spacing: 6

    icon.width: 24
    icon.height: 24
    icon.color: control.ink

    contentItem: Item {
        // As wide as it is selected, bold and with its ✓, at all times (a checkable button):
        // a choice made does not move the row it sits in.
        implicitWidth: Math.max(label.implicitWidth, control.checkable || control.checked ? Math.ceil(widest.advanceWidth) + (control.icon.name !== "" || control.icon.source.toString() !== "" ? control.icon.width + control.spacing : 0) : 0)
        implicitHeight: label.implicitHeight

        TextMetrics {
            id: widest

            font: control.boldFont
            // Measured only for a button that can be selected.
            text: control.checkable || control.checked ? (control.choice ? "✓  " : "") + control.text : ""
        }

        IconLabel {
            id: label

            anchors.fill: parent
            spacing: control.spacing
            mirrored: control.mirrored
            display: control.display

            icon.name: control.icon.name
            icon.source: control.icon.source
            icon.width: control.icon.width
            icon.height: control.icon.height
            icon.cache: control.icon.cache
            // A colour icon in its own colours, whatever the page asks; a symbolic one as asked,
            // but in the words' colour on the selected tint and the accent's fill, and when disabled.
            icon.color: control.ownColours ? "transparent" : control.enabled && !control.selected && !control.mainAction ? control.icon.color : control.ink
            text: control.selected && control.choice && control.hasWords ? "✓  " + control.text : control.text
            font: control.selected ? control.boldFont : control.font
            color: control.ink
        }
    }

    background: Rectangle {
        readonly property var theme: control.sioulTheme
        readonly property color edge: theme ? theme.edge : control.palette.mid
        readonly property color accent: theme ? theme.accent : control.palette.dark
        // The selected one's tint: a light step of the accent on the page.
        readonly property color tint: theme ? theme.chosenTint : Color.blend(control.palette.window, control.palette.dark, 0.16)
        // High contrast, as the system asks it (Qt 6.10's style hints; never with an older Qt).
        readonly property var hints: Qt.styleHints
        readonly property bool highContrast: hints.accessibility ? hints.accessibility.contrastPreference === Qt.HighContrast : false
        // Nothing drawn at rest: an icon or an arrow alone.
        readonly property bool bare: !control.mainAction && !control.selected && !control.hasWords

        implicitWidth: 100
        implicitHeight: 40
        radius: theme ? theme.radius : 0
        visible: !bare || control.down || control.visualFocus || highContrast || (control.hovered && control.enabled)
        color: control.mainAction ? Color.blend(control.palette.dark, control.palette.mid, control.down ? 0.5 : 0.0)
            : control.down ? control.palette.mid
            : control.selected ? tint
            : control.hovered && control.enabled ? (theme ? theme.hover : control.palette.light)
            : "transparent"
        border.color: control.visualFocus ? control.palette.highlight
            : control.selected ? (control.enabled ? accent : Color.blend(accent, control.palette.window, 0.6))
            : control.mainAction ? "transparent"
            : bare ? control.palette.windowText
            : control.enabled ? edge : Color.blend(edge, control.palette.window, 0.6)
        border.width: control.visualFocus || control.selected ? 2 : control.mainAction ? 0 : !bare || highContrast ? 1 : 0
    }
}
