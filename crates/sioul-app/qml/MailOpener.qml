// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's one opener for a message (the unified mailbox's §3.3, part
// 2): the one way into a message from anywhere but Mail's own list and the
// Porch's own rows (a notification, a code, the home card, "Tied to it", a
// project's line of time, a task's source, a note's link, a money row, an
// event's invitation, the card table). It never opens a folder: Mail shows,
// its reader saying "Opening the message…" at once, while Rust finds the
// message off the window's thread (`findMessage`: this device first, then
// the servers, read only); then the message opens, or the reader says why
// not (MailPage.qml, Reader.qml). Back and Escape return to the page it was
// opened from, one level, its card shown again (`back`).

pragma ComponentBehavior: Bound

import QtQuick

QtObject {
    id: opener

    // The main window (main.qml): `page`, `cardHost`, `mailPage` (the Mail
    // page, made when first shown).
    required property var window
    // Sioul's backend: `findMessage`, `messageFound`.
    required property var sioul
    // Mail's place among the pages.
    property int mailPageIndex: 2
    // The message last asked for: its answer alone is taken.
    property int ticket: 0
    // Where it was asked from: {page, card}; null once Mail is Mail's own again.
    property var from: null
    // What was last asked of Rust, as JSON: asked again when a server could not be reached.
    property string asked: ""

    // A thing's reference as links, notifications and rows give it ({kind:
    // "mail", uri, key}): its address (`uri`, else a key that is one: a
    // notification's), the file this device last saw a hint.
    function refOf(item) {
        const key = item.key || ""
        const named = /^(mid:|sioul:mail\/)/.test(key)
        return { address: item.uri || (named ? key : ""), file: named ? "" : key }
    }

    // The message of `ref` ({address, file, account, folder, uid}, or
    // {invitation, organizer} for an event's), in Mail's reader.
    function open(ref) {
        const host = opener.window.cardHost
        // The card open, or the one this opening just closed on a phone (`CardHost.leave`).
        const card = host.open ? host.ref : host.leftRef
        const from = opener.window.page !== opener.mailPageIndex ? { page: opener.window.page, card: card } : opener.from
        host.leave()
        opener.ticket++
        opener.window.page = opener.mailPageIndex
        opener.from = from
        opener.window.mailPage.openFound(opener.ticket)
        opener.asked = JSON.stringify({
            address: ref.address || "",
            file: ref.file || "",
            account: ref.account || "",
            folder: ref.folder || "",
            uid: ref.uid || null,
            invitation: ref.invitation || "",
            organizer: ref.organizer || ""
        })
        opener.sioul.findMessage(opener.ticket, opener.asked)
    }

    // "It looks again when it can": a server that could not be reached, or a
    // message on its way to a folder (its copy comes with that folder's sync),
    // asked again once mail comes in (a sync's publish), while the reader still says so.
    function again() {
        const page = opener.window.mailPage
        if (!page || page.askedTicket !== opener.ticket || page.answer === null || ["unreachable", "on-its-way"].indexOf(page.answer.state) < 0)
            return
        page.answer = { state: "asking" }
        opener.sioul.findMessage(opener.ticket, opener.asked)
    }

    // Back from a message it opened: the page it came from, its card again.
    function back() {
        const from = opener.from
        opener.from = null
        if (from === null)
            return
        opener.window.page = from.page
        const host = opener.window.cardHost
        if (from.card && !host.open)
            host.show(from.card)
    }

    // Mail left any other way than this Back (the rail, a key, a
    // notification): where the message came from is forgotten, and Mail's
    // own Back is Mail's again (review of part 2, L14). `back` forgets it
    // itself before it changes the page.
    property Connections leaving: Connections {
        target: opener.window

        function onPageChanged() {
            if (opener.window.page !== opener.mailPageIndex)
                opener.from = null
        }
    }

    // What Rust found, for the message last asked.
    property Connections answers: Connections {
        target: opener.sioul

        function onMessageFound(ticket, answer) {
            if (ticket === opener.ticket && opener.window.mailPage)
                opener.window.mailPage.answered(ticket, JSON.parse(answer || "{}"))
        }

        function onMailFolderChanged() {
            opener.again()
        }
    }
}
