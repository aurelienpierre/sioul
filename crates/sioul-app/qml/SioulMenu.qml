// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Every menu of Sioul: as wide as its widest line, so that no label is cut;
// never narrower than a menu usually is, never wider than the window. Kept
// inside the window, never taller than it: a longer list scrolls (the wheel,
// a drag, the keys), its bar showing there is more.

import QtQuick
import QtQuick.Controls.Basic

Menu {
    id: menu

    // A menu drawn as a window of its own can run past the screen, where it
    // cannot scroll: drawn in the window, it is fitted to it.
    popupType: Popup.Item
    margins: 8
    width: {
        let widest = 0
        for (let i = 0; i < menu.count; ++i) {
            const item = menu.itemAt(i)
            if (item)
                widest = Math.max(widest, item.implicitWidth)
        }
        const window = menu.parent ? menu.parent.Window.window : null
        const room = window ? window.width - 24 : Number.MAX_VALUE
        // Room for the scroll bar when the list is longer than the window.
        return Math.min(Math.max(200, widest + menu.leftPadding + menu.rightPadding + (list.interactive ? 12 : 0)), room)
    }

    contentItem: ListView {
        id: list

        implicitHeight: list.contentHeight
        model: menu.contentModel
        interactive: list.contentHeight + menu.topPadding + menu.bottomPadding > menu.height + 1
        clip: true
        currentIndex: menu.currentIndex
        boundsBehavior: Flickable.StopAtBounds

        ScrollBar.vertical: ScrollBar {
            policy: list.interactive ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
        }
    }
}
