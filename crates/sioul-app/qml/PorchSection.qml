// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One section of the Porch (docs/porch.md, "Sections"), every one alike: a
// one-line header, its title, a "?" that says the section's help once (on
// a computer as a tooltip, pressed or on a phone in a popup with what
// changes it), a chevron that folds it; its folding remembered on this
// device. Its rows under it (NoticeRow.qml). A section with nothing to show
// is not there at all: no "nothing here".

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: section

    required property var sioul
    required property var theme
    // What its folding is remembered by: "people", "project:<id>", "phone".
    required property string key
    required property string title
    // What it holds and how things land there, said once.
    property string help: ""
    // Folded until unfolded by hand (Filed, less important, set aside).
    property bool foldedByDefault: false
    // Its rows shown: none, and it is not there.
    property int count: 0
    // A phone: the "?" opens its popup, a tooltip never shows there.
    property bool compact: false
    // Its rows.
    default property alias rows: body.data

    // Unfolded or folded against its usual, on this device (`viewFlag`).
    readonly property string flag: (section.foldedByDefault ? "porch-unfolded." : "porch-folded.") + section.key
    property bool turned: section.sioul.viewFlag(section.flag)
    readonly property bool folded: section.foldedByDefault !== section.turned

    // The "?" pressed: the page shows the help and what changes it.
    signal helpAsked

    function toggle() {
        section.turned = !section.turned
        section.sioul.setViewFlag(section.flag, section.turned)
    }

    // Unfolded, for the window's pictures.
    function unfold() {
        if (section.folded)
            section.toggle()
    }

    visible: section.count > 0
    Layout.fillWidth: true
    spacing: 0

    RowLayout {
        Layout.fillWidth: true
        spacing: 2

        // The title folds and unfolds it; the chevron says which.
        ItemDelegate {
            id: header

            objectName: "sectionHeader"
            Layout.fillWidth: true
            leftPadding: 8
            rightPadding: 8
            topPadding: 6
            bottomPadding: 6
            onClicked: section.toggle()
            Accessible.name: section.title
            Accessible.description: section.folded ? section.sioul.text("ui-section-folded") : section.sioul.text("ui-section-unfolded")

            background: Rectangle {
                color: header.hovered ? section.theme.surface : "transparent"
                radius: section.theme.radius
                border.color: header.visualFocus ? section.theme.focus : "transparent"
                border.width: 2
            }
            contentItem: RowLayout {
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    // A long title elided, never wider than its header.
                    Layout.preferredWidth: 0
                    text: section.title
                    textFormat: Text.PlainText
                    font.pixelSize: 15
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    color: section.theme.text
                }
                Label {
                    text: section.folded ? "▸" : "▾"
                    color: section.theme.muted
                }
            }
        }
        ToolButton {
            objectName: "sectionHelp"
            visible: section.help !== ""
            text: "?"
            Accessible.name: section.sioul.text("ui-section-help")
            Accessible.description: section.help
            ToolTip.visible: hovered && !section.compact
            ToolTip.text: section.theme.plain(section.help)
            ToolTip.delay: 300
            onClicked: section.helpAsked()
        }
    }
    ColumnLayout {
        id: body

        visible: !section.folded
        Layout.fillWidth: true
        spacing: 0
    }
}
