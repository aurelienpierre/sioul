// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The site named above an extension's sign-in page (SignInWindow.qml,
// docs/extensions.md, "A sign-in"): the host of the last page drawn, so that
// you see where a password is typed. Never the host of a navigation still
// under way: Chromium shows the address of a navigation the window itself
// starts (a pop-up turned into the window's own page) before its page comes,
// and the page drawn meanwhile is still the old one. So the name follows a
// load of the main frame that succeeded, and while a navigation is under way
// the page is covered and its name is "…". A load that failed stays covered
// (the page drawn may be the old one or an error page); one stopped leaves
// the old page, and its name, as they were.

pragma ComponentBehavior: Bound

import QtQml

QtObject {
    id: site

    // The host of the last page that came in the main frame; "" before the first.
    property string host: ""
    // A navigation under way: the page is covered, its name not said.
    property bool pending: true
    // The last navigation failed: the page stays covered.
    property bool failed: false
    // The page is covered, and nothing can be typed into it.
    readonly property bool covered: site.pending || site.failed || site.host === ""
    // What is said above the page.
    readonly property string shown: site.covered ? "…" : site.host

    // The host of an address, with its port when it names one: no scheme, no user, lower case.
    function hostOf(address) {
        const rest = String(address).replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
        return rest.split(/[\/?#]/)[0].replace(/^.*@/, "").toLowerCase()
    }

    // A load of the main frame: "started", "succeeded", "failed" or "stopped", at `address`.
    function loading(status, address) {
        switch (status) {
        case "started":
            site.pending = true
            site.failed = false
            break
        case "succeeded":
            site.host = site.hostOf(address)
            site.pending = false
            site.failed = false
            break
        case "failed":
            site.pending = false
            site.failed = true
            break
        case "stopped":
            // Stopped before its page came: the old page is still the one drawn.
            site.pending = false
            break
        }
    }
}
