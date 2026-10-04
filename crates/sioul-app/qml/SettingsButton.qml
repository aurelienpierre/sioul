// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The small button that opens a page's settings, at the end of its first
// row; "Aa" opens how long text reads.

import QtQuick
import QtQuick.Controls.Basic

ToolButton {
    id: button

    required property var sioul
    required property var theme
    property string view: ""
    // "Aa": the reading settings, wherever long text is.
    property bool reading: false

    icon.name: button.reading ? "" : "settings-configure"
    icon.color: button.theme.text
    text: button.reading ? "Aa" : ""
    font.weight: Font.DemiBold
    display: button.reading ? AbstractButton.TextOnly : AbstractButton.IconOnly
    ToolTip.visible: button.hovered
    ToolTip.text: button.reading ? button.sioul.text("ui-reading") : button.sioul.text("ui-settings")
    ToolTip.delay: 300
    Accessible.name: ToolTip.text
    onClicked: button.show(true)

    function show(open) {
        if (open) {
            if (panel.item === null)
                panel.setSource("SettingsPanel.qml", { sioul: button.sioul, theme: button.theme, view: button.reading ? "reading" : button.view })
            panel.item.open()
        } else if (panel.item !== null) {
            panel.item.close()
        }
    }

    // For the window's images: the panel floats over the page, out of its picture.
    function grab(path) {
        if (panel.item !== null)
            panel.item.contentItem.grabToImage(result => result.saveToFile(path))
    }

    // The panel (SettingsPanel.qml), made the first time it opens: each page
    // has this button, and the panel is long to make.
    Loader {
        id: panel

        Binding {
            target: panel.item
            when: panel.item !== null
            property: "view"
            value: button.reading ? "reading" : button.view
        }
    }
}
