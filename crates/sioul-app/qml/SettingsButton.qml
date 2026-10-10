// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The small button that opens a page's settings, at the end of its first
// row; "Aa" opens how long text reads.

import QtQuick
import QtQuick.Controls.Basic

ToolButton {
    id: button

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The page whose settings it opens: "tasks", "mail", "agenda"… (the core's
    // `settings::for_view`).
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

    // Its settings panel opened (made the first time), or closed.
    function show(open) {
        if (open) {
            if (panel.item === null)
                panel.setSource("SettingsPanel.qml", { sioul: button.sioul, theme: button.theme, view: button.reading ? "reading" : button.view })
            panel.item.open()
        } else if (panel.item !== null) {
            panel.item.close()
        }
    }

    // For the window's images: the panel scrolled to one of its settings.
    function scrollTo(key) {
        const shown = panel.item as SettingsPanel
        if (shown !== null)
            shown.scrollTo(key)
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
