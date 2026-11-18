// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A setting opened by its key, wherever it lives now: from a link
// (`settings:<key>`), a notice (`sioul:settings/<key>`) or a page
// (main.qml's `showParameters`). A setting that one page reads lives behind
// that page's ⚙ (the core's `settings::page_of`, asked as `settingPage`):
// the page is shown, its ⚙ opened at the setting; Bitwarden's are the
// Accounts page's card. Every other one is in Settings, at its tab
// (ParametersPage.qml's `showSetting`, which also knows the older keys).
// The window's pages are made the first time they are shown: each is asked
// for once its page number is set.

import QtQuick

QtObject {
    id: door

    // The window (main.qml): `page`, and the pages by name.
    required property var window
    // Sioul's backend (backend.rs): where a setting lives (`settingPage`).
    required property var sioul

    // The pages whose ⚙ holds settings of their own, as `settings::PAGES`
    // names them: the window's page number and its name there. Bitwarden's
    // are Accounts' card, at the top of Your accounts.
    readonly property var pages: ({
        "sites": [3, "sitesPage"],
        "bitwarden": [11, "accountsPage"]
    })

    function open(key) {
        const view = door.sioul.settingPage(key) || ""
        const place = door.pages[view]
        if (place === undefined) {
            door.window.page = 12
            if (door.window.parametersPage)
                door.window.parametersPage.showSetting(key)
            return
        }
        door.window.page = place[0]
        const page = door.window[place[1]]
        // A page without a ⚙ of its own on this device (Android's Sites page): the page alone.
        if (page && typeof page.openSettings === "function")
            page.openSettings(true, key === view ? "" : key)
    }
}
