// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A button that opens a list to choose a value from, drawn as the window's
// one drop-down (docs/qt-quick.md, "The look of each kind of thing", rule
// 2): a PlainComboBox's closed box, the field's fill and its firmer `edge`
// (2 points of the `focusEdge` with the focus), the value at the left and
// the ⇕ at the right, so that a choice of a value looks the same whether Qt's
// ComboBox or a list of the page's own opens under it (the filter by area,
// by topic). "▾" after words is for a button that opens a menu of actions
// ("New ▾"), never for a value. Its words are plain text, cut with "…" when
// the box is narrower than they are; a screen reader hears a combo box, by
// `Accessible.name`, or by its value.
//
//     DropDown {
//         theme: page.theme
//         text: page.theme.plain(chosen.name)
//         Accessible.name: page.sioul.textWith("topic-filter-name", "topic", chosen.name)
//         onClicked: list.open()
//     }

import QtQuick
import QtQuick.Controls.impl
import QtQuick.Templates as T

T.Button {
    id: control

    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme

    // Basic's ComboBox's sizes: its box 140 by 40, its words 12 points in, the ⇕ 6 from the edge.
    implicitWidth: Math.max(implicitBackgroundWidth + leftInset + rightInset,
                            implicitContentWidth + leftPadding + rightPadding)
    implicitHeight: Math.max(implicitBackgroundHeight + topInset + bottomInset,
                             implicitContentHeight + topPadding + bottomPadding)

    padding: 6
    leftPadding: 12
    rightPadding: padding + arrows.width + spacing
    spacing: 6
    Accessible.role: Accessible.ComboBox
    Accessible.name: control.text

    contentItem: T.Label {
        text: control.text
        textFormat: Text.PlainText
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
        font: control.font
        color: control.enabled ? control.palette.text : control.theme.muted
    }

    // Basic's ComboBox's own ⇕, in its place and colour.
    ColorImage {
        id: arrows

        x: control.mirrored ? control.padding : control.width - width - control.padding
        y: control.topPadding + (control.availableHeight - height) / 2
        color: control.enabled ? control.palette.dark : control.theme.muted
        defaultColor: "#353637"
        source: "qrc:/qt-project.org/imports/QtQuick/Controls/Basic/images/double-arrow.png"
        opacity: control.enabled ? 1 : 0.6
    }

    background: Rectangle {
        implicitWidth: 140
        implicitHeight: 40
        color: control.down ? control.palette.mid : control.theme.field
        border.color: control.activeFocus ? control.theme.focusEdge : control.enabled ? control.theme.edge : Color.blend(control.theme.edge, control.theme.background, 0.6)
        border.width: control.activeFocus ? 2 : 1
    }
}
