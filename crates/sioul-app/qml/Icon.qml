// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A desktop icon, by its freedesktop name, in its own colours. Decorative,
// unless it has a tip: then hovering it says what it stands for, and screen
// readers read the tip.
//
// Its colours are those of the set of Sioul's bundled icons drawn for the
// window's (light or dark, desktop.rs `follow`), where the desktop's theme
// does not draw it. When the window turns dark or light while the icon
// shows, its name is read again, from the set now in use: the set drawn for
// light colours shows dark grey, which a dark window would hide.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Controls.impl
import QtQuick.Layouts

IconImage {
    id: icon

    // The icon, by its freedesktop name ("mail-reply-sender").
    required property string iconName
    // Its side, in pixels.
    property int size: 18
    // Said on hover, when set: what the icon stands for, in words.
    property string tip: ""

    // The window it shows in, and whether that window's colours are dark
    // (each of Sioul's windows has a theme: SioulWindow.qml).
    readonly property var shownIn: Window.window
    readonly property bool dark: icon.shownIn !== null && icon.shownIn.theme !== undefined && icon.shownIn.theme.dark
    // Whether its name was read for dark colours (1) or light (0): known
    // once it shows in a window (`know`).
    property int readFor: -1
    // False a moment, for the name to be read again.
    property bool named: true

    ToolTip.visible: hover.hovered && icon.tip !== ""
    // A tip may hold what a message says of itself (the shield's checks): a
    // word joiner after each "<" keeps it plain text (Theme.plain).
    ToolTip.text: icon.tip.replace(/</g, "<⁠")
    ToolTip.delay: 300

    name: icon.named ? icon.iconName : ""
    sourceSize: Qt.size(size, size)
    // Read on a thread: a page with many icons waited for each (half a second on a phone).
    asynchronous: true
    Layout.preferredWidth: size
    Layout.preferredHeight: size
    Accessible.ignored: icon.tip === ""
    Accessible.name: icon.tip

    // The colours its name was read for: the window's when it first shows there.
    function know() {
        if (icon.readFor < 0 && icon.shownIn !== null)
            icon.readFor = icon.shownIn.theme !== undefined && icon.shownIn.theme.dark ? 1 : 0
    }

    Component.onCompleted: icon.know()
    onShownInChanged: icon.know()
    onDarkChanged: {
        if (icon.readFor < 0 || icon.shownIn === null) {
            icon.know()
            return
        }
        const now = icon.dark ? 1 : 0
        if (now === icon.readFor)
            return
        icon.readFor = now
        // After the window's own handler has switched the set (main.qml, its Theme).
        Qt.callLater(() => {
            if (!icon)
                return
            icon.named = false
            icon.named = true
        })
    }

    HoverHandler {
        id: hover

        enabled: icon.tip !== ""
    }
}
