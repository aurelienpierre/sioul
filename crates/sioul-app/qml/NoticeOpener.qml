// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's one route for a notice (docs/notifications.md): a click on a
// notice, or its "Open", names its thing by its address, and each scheme
// goes to the route that shows it (main.qml's `open`). `hint` is where this
// device last saw the thing: a message's file, an event's file and the time
// it is for. The pause to move: "Where I stopped"; a meal, a nap or the
// night: its question; the end of the work day or of the whole day: its
// sheet; a site's notice: the site, then the web notice's own click (the
// site opens the conversation), the site alone when it is gone; a message:
// Mail's reader; an event: its card at that time; a dose: its route as
// before; a task, a paper, a contract, a budget's line, a letter: where it lives.

pragma ComponentBehavior: Bound

import QtQuick

QtObject {
    id: opener

    // The main window (main.qml): `openThing`, `openSite`, `showParameters`, `sitesPage`.
    required property var window

    function open(address, hint) {
        const w = opener.window
        const after = prefix => decodeURIComponent(address.slice(prefix.length))
        const key = hint || ""
        if (address === "sioul:move")
            w.openThing({ kind: "stopped", uri: "", key: "" })
        else if (address.startsWith("sioul:need/"))
            w.openThing({ kind: "need", uri: "", key: after("sioul:need/") })
        else if (address.startsWith("sioul:day/"))
            w.openThing({ kind: "review", uri: "", key: after("sioul:day/") })
        else if (address.startsWith("sioul:dose/"))
            w.openThing({ kind: "dose", uri: "", key: after("sioul:dose/") })
        else if (address.startsWith("sioul:site/")) {
            const rest = address.slice("sioul:site/".length)
            const at = rest.indexOf("#")
            const id = decodeURIComponent(at < 0 ? rest : rest.slice(0, at))
            w.openSite(id)
            // Its web notice clicked, while the page keeps it; else the site alone.
            if (at >= 0 && w.sitesPage)
                w.sitesPage.clickNotice(id, decodeURIComponent(rest.slice(at + 1)))
        } else if (address === "sioul:mailbox")
            w.openThing({ kind: "mailbox", uri: "", key: "" })
        else if (address === "sioul:porch")
            w.openThing({ kind: "porch", uri: "", key: "" })
        else if (address.startsWith("sioul:settings/"))
            w.showParameters(address.slice("sioul:settings/".length))
        else if (address.startsWith("mid:") || address.startsWith("sioul:mail/"))
            w.openThing({ kind: "mail", uri: address, key: key })
        // An event's time, by its file and that time (a series' own, not its next); else the agenda.
        else if (address.startsWith("sioul:event/"))
            w.openThing(key !== "" ? { kind: "event", uri: address.split("#")[0], key: key } : { kind: "agenda", uri: "", key: "" })
        // A task, a paper, a contract, a budget's line, a letter…: its kind is its address's first word.
        else if (address.startsWith("sioul:"))
            w.openThing({ kind: address.slice("sioul:".length).split("/")[0], uri: address, key: key })
    }
}
