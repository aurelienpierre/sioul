// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A row of actions with their words where there is room, the rest in its ⋮
// with their words too (rule 10 of the interface: an icon alone only for ⚙,
// ✕, ⋮, +, ◂ ▸; nothing named only on hover). Each action is an
// ActionButton whose `folds` is this: it shows in the row while its words
// fit, in its rank, and else waits in the row's ⋮ menu, which lists
// `folded` first (an Instantiator of MenuItems, each pressing its action).
// The row's ⋮ itself is the page's: its width is left out of `room`.

import QtQml

QtObject {
    id: folds

    // The row's actions, in the order they stand in it (ActionButton.qml).
    property var actions: []
    // The width the row gives its actions, its ⋮ and the rest of it left out.
    property real room: 0
    // The row's spacing, between two actions.
    property real spacing: 4

    // The actions offered that have no room, in the order they stand in the row.
    // The ranks fill the row from the lowest; once one does not fit, every one
    // after it in rank waits in ⋮ too, so that a smaller one never jumps ahead.
    readonly property var folded: {
        const offered = folds.actions.filter(a => a !== null && a !== undefined && a.offered)
        const ranked = offered.map((a, i) => ({ action: a, at: i })).sort((x, y) => x.action.rank - y.action.rank || x.at - y.at)
        let used = 0
        let full = false
        const out = []
        for (const r of ranked) {
            const width = r.action.wideWidth + (used > 0 ? folds.spacing : 0)
            if (!full && used + width <= folds.room) {
                used += width
                continue
            }
            full = true
            out.push(r)
        }
        return out.sort((x, y) => x.at - y.at).map(r => r.action)
    }
}
