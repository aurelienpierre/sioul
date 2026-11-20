// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A list keeps its place (docs/qt-quick.md, "A list keeps its place"): when
// its rows are read again, an item added, changed or deleted, the row at the
// top of its view stays where it was, to the pixel; when that row is gone,
// the one beside it. Qt alone does not: a ListView given a new array goes
// back to its top, and the rows of a Repeater move by whatever changed above
// them.
//
// One beside each scrolling view: `view` is the ListView, or the ScrollView
// or Flickable its rows scroll in; `source` the rows as the page has them;
// the view reads `rows`, never `source`. A change of `source` reaches the
// view through `apply`, which notes the place first, gives the view its rows
// and puts the place back. A ListView's rows are known by `key`; the rows in
// a Flickable, and the rows inside a ListView's own rows (a group's tasks, a
// day's events), say their key with a `placeKey` property on their delegate.
// A ListModel the page changes in place is given as `rows` itself, and
// changed inside `keep(() => …)`. When `scope` (what it lists: a filter, a
// search, a day) changes, the list goes to its top instead.
//
// When: the view is put back at once, then once more after the window has
// laid its rows out (its `afterAnimating`, after Qt's polish and before the
// frame is drawn): a row whose words wrap only learns its height there, and
// a place put back before it is off by every such row above. Never while the
// person drags or flicks the view: new rows wait until the movement ends.

import QtQuick

QtObject {
    id: place

    // The view whose place is kept: a ListView, or a ScrollView or a Flickable whose rows are Repeaters'.
    property Item view: null
    // The rows as the page has them now.
    property var source: undefined
    // The rows the view shows: its model, or what its Repeaters read; a ListModel changed in place.
    property var rows: []
    // A ListView's row known by this field of `rows` (or a ListModel's role), or by a function of the row.
    property var key: "key"
    // What the list lists (a filter, a search, a folder, a day, a tab): when it changes, the next rows
    // go to the top, as a view of other things.
    property var scope: ""
    // At its very top, the list stays at its top: what comes first shows there (a browser's rule).
    property bool topStays: true

    // The rows of another scope shown, the list at its top: a page that opens such a list
    // elsewhere (the Agenda's What comes on today) places it here.
    signal fresh()

    // The Flickable that scrolls, and whether it is a ListView.
    readonly property Item flick: place.view === null ? null : place.view.contentY !== undefined ? place.view : place.view.contentItem
    readonly property bool isList: place.view !== null && typeof place.view.positionViewAtIndex === "function"

    // New rows held while the person moves the view: `source` not given yet, changes not run yet.
    property bool waiting: false
    property var held: []
    // The scope the rows shown were read under.
    property string shownScope: ""
    // The place noted, put back once more after the window's layout.
    property var settling: null
    // A row the person just made from this list, brought into view when it comes.
    property string wanted: ""

    // The rows changed: given to the view once every binding of the change has settled (the
    // scope may change with them, from the same answer), the view still showing the old ones.
    onSourceChanged: Qt.callLater(place.apply)
    Component.onCompleted: {
        place.shownScope = JSON.stringify(place.scope === undefined ? "" : place.scope)
        if (place.source !== undefined)
            place.rows = place.source
    }

    // `source` given to the view, and the changes held meanwhile run, its place kept.
    function apply() {
        const swap = place.source !== undefined && place.rows !== place.source
        if (!swap && place.held.length === 0)
            return
        if (place.flick !== null && place.flick.moving) {
            place.waiting = true
            return
        }
        place.waiting = false
        const held = place.held
        place.held = []
        place.keep(() => {
            for (const change of held)
                change()
            if (swap)
                place.rows = place.source
        })
    }

    // `change` run (it changes what the view shows), the view's place kept around it. While the
    // person moves the view, it waits for the movement's end, after the changes held before it.
    function keep(change) {
        const flick = place.flick
        if (flick !== null && flick.moving) {
            place.held = place.held.concat([change])
            place.waiting = true
            return
        }
        const scope = JSON.stringify(place.scope === undefined ? "" : place.scope)
        const scoped = scope !== place.shownScope
        place.shownScope = scope
        if (flick === null || scoped) {
            change()
            if (scoped && flick !== null) {
                place.wanted = ""
                place.settling = null
                place.toTop()
                place.fresh()
            }
            return
        }
        const noted = place.note()
        change()
        if (noted !== null) {
            place.putBack(noted)
            place.settling = noted
            place.revealWanted(false)
        } else {
            place.revealWanted(true)
        }
    }

    // The list at its top, as when it shows other things; the person's own ask too.
    function toTop() {
        const flick = place.flick
        if (flick === null)
            return
        if (place.isList)
            place.view.positionViewAtBeginning()
        else
            flick.contentY = flick.originY - flick.topMargin
    }

    // The person just made the row known by `key` from this list: brought into view when it is
    // there, by the smallest move, and only if it would be out of sight.
    function reveal(key) {
        place.wanted = key
        place.revealWanted(place.settling === null)
    }

    // How a row of `rows` is known.
    function keyOf(row) {
        if (row === undefined || row === null)
            return ""
        const known = typeof place.key === "function" ? place.key(row) : row[place.key]
        return known === undefined || known === null ? "" : String(known)
    }

    // The ListView's row `index`, from `rows`: an array, or a ListModel.
    function rowAt(index) {
        return Array.isArray(place.rows) ? place.rows[index] : place.rows.get(index)
    }

    // Where a row is, in the Flickable's content.
    function topOf(item) {
        return item.mapToItem(place.flick.contentItem, 0, 0).y
    }

    // The rows of `item`'s tree that say their key (`placeKey`), as [path, item]: a row's own
    // rows are walked too; hidden ones are not there.
    function keyed(item, path, out) {
        const kids = item.children
        for (let i = 0; i < kids.length; ++i) {
            const kid = kids[i]
            if (!kid.visible)
                continue
            const own = kid.placeKey
            if (own !== undefined && own !== null && String(own) !== "") {
                const here = path + "\u001f" + own
                const before = out.length
                place.keyed(kid, here, out)
                if (out.length === before)
                    out.push([here, kid])
            } else {
                place.keyed(kid, path, out)
            }
        }
        return out
    }

    // A ListView's rows made now (those in sight and around), as [path, item, index, its row].
    function made() {
        const list = place.view
        const flick = place.flick
        let at = -1
        for (let dy = 0; at < 0 && dy <= flick.height; dy += 2) {
            at = list.indexAt(1, flick.contentY + dy)
            if (at < 0)
                at = list.indexAt(list.width / 2, flick.contentY + dy)
        }
        if (at < 0)
            return []
        let first = at
        while (first > 0 && list.itemAtIndex(first - 1) !== null)
            first -= 1
        const out = []
        for (let i = first; i < list.count && list.itemAtIndex(i) !== null; ++i) {
            const item = list.itemAtIndex(i)
            const outer = "\u001e" + place.keyOf(place.rowAt(i))
            const inner = item.visible ? place.keyed(item, outer, []) : []
            if (inner.length === 0)
                out.push([outer, item, i, item])
            for (const one of inner)
                out.push([one[0], one[1], i, item])
        }
        return out
    }

    // A ListView's rows made now laid out at once, and the view with them: a row the view
    // has just made (put in view by this) learns its height only in the window's layout, and
    // the rows under it would move then.
    function settleList() {
        const list = place.view
        const polish = item => {
            item.ensurePolished()
            const kids = item.children
            for (let i = 0; i < kids.length; ++i)
                polish(kids[i])
            item.ensurePolished()
        }
        for (const one of place.made())
            polish(one[3])
        list.ensurePolished()
    }

    // The place noted: the row at the view's top and how far the view's top is into it; the rows
    // under it, in order, to take its place when it is gone; the rows over it, nearest first, each
    // with its own distance, when none under it is left.
    function note() {
        const flick = place.flick
        if (flick.height <= 0)
            return null
        const found = place.isList ? place.made() : place.keyed(flick.contentItem, "", [])
        const rows = found.map(f => ({ path: f[0], top: place.topOf(f[1]), height: f[1].height, index: f.length > 2 ? f[2] : -1 }))
        rows.sort((a, b) => a.top - b.top)
        let first = rows.findIndex(r => r.top + r.height > flick.contentY)
        if (first < 0)
            first = rows.length - 1
        return {
            atTop: place.topStays && flick.contentY <= flick.originY - flick.topMargin + 0.5,
            offset: first >= 0 ? flick.contentY - rows[first].top : 0,
            under: rows.slice(Math.max(0, first)).map(r => r.path),
            over: rows.slice(0, Math.max(0, first)).reverse().map(r => ({ path: r.path, offset: flick.contentY - r.top })),
            // A ListView's rows in order, by key: the nearest that is left when none of those made is.
            keys: place.isList ? Array.from({ length: place.view.count }, (_, i) => place.keyOf(place.rowAt(i))) : [],
            last: rows.length > 0 ? Math.max.apply(null, rows.map(r => r.index)) : -1
        }
    }

    // The view at `y`, within its bounds.
    function scrollTo(y) {
        const flick = place.flick
        const least = flick.originY - flick.topMargin
        const most = Math.max(least, flick.originY + flick.contentHeight + flick.bottomMargin - flick.height)
        const kept = Math.max(least, Math.min(most, y))
        if (Math.abs(flick.contentY - kept) > 0.01)
            flick.contentY = kept
    }

    // The row noted as `path`, now: in a ListView, its row made first (with those ahead and
    // behind) where it is now; in a Flickable, from the rows found once (`keyedNow`).
    function itemOf(path, where, keyedNow) {
        if (!place.isList)
            return keyedNow[path] || null
        const outer = path.split("\u001f")[0]
        const index = where[outer]
        if (index === undefined)
            return null
        if (place.view.itemAtIndex(index) === null)
            place.view.positionViewAtIndex(index, ListView.Beginning)
        const item = place.view.itemAtIndex(index)
        if (item === null)
            return null
        const inner = place.keyed(item, outer, [])
        if (path === outer)
            return inner.length === 0 ? item : null
        const found = inner.find(f => f[0] === path)
        return found ? found[1] : null
    }

    // A ListView's rows by key: where each is now.
    function indexes() {
        const where = {}
        if (!place.isList)
            return where
        for (let i = 0; i < place.view.count; ++i) {
            const known = "\u001e" + place.keyOf(place.rowAt(i))
            if (where[known] === undefined)
                where[known] = i
        }
        return where
    }

    // The place noted, put back: the row that was at the top where it was; gone, the first row
    // under it still there in its place; none, the nearest over it where it was. In a ListView,
    // when none of the rows it had made is left, the next row still there in the top one's place.
    function putBack(noted) {
        const flick = place.flick
        if (flick === null || flick.moving)
            return
        if (noted.atTop) {
            place.scrollTo(flick.originY - flick.topMargin)
            return
        }
        const where = place.indexes()
        const keyedNow = {}
        if (!place.isList)
            for (const f of place.keyed(flick.contentItem, "", []))
                keyedNow[f[0]] = f[1]
        const put = (path, offset) => {
            const item = place.itemOf(path, where, keyedNow)
            if (item === null)
                return false
            place.scrollTo(place.topOf(item) + offset)
            if (place.isList) {
                place.settleList()
                const again = place.itemOf(path, where, keyedNow)
                if (again !== null)
                    place.scrollTo(place.topOf(again) + offset)
            }
            return true
        }
        for (const path of noted.under)
            if (put(path, noted.offset))
                return
        for (const row of noted.over)
            if (put(row.path, row.offset))
                return
        if (place.isList && noted.keys.length > 0) {
            const from = Math.max(0, noted.last)
            const order = noted.keys.slice(from + 1).concat(noted.keys.slice(0, from + 1).reverse())
            for (const known of order) {
                const index = where["\u001e" + known]
                if (index !== undefined) {
                    place.view.positionViewAtIndex(index, ListView.Beginning)
                    place.settleList()
                    place.view.positionViewAtIndex(index, ListView.Beginning)
                    return
                }
            }
        }
    }

    // The row the person just made, in view by the smallest move once it is there; `last`: no
    // correction comes after (else it is looked for again then).
    function revealWanted(last) {
        if (place.wanted === "" || place.flick === null || place.flick.moving)
            return
        const flick = place.flick
        let item = null
        if (place.isList) {
            const index = place.indexes()["\u001e" + place.wanted]
            if (index !== undefined) {
                if (last)
                    place.wanted = ""
                place.view.positionViewAtIndex(index, ListView.Contain)
                place.settleList()
                place.view.positionViewAtIndex(index, ListView.Contain)
                return
            }
            const made = place.made().find(f => f[0].endsWith("\u001f" + place.wanted))
            item = made ? made[1] : null
        } else {
            const found = place.keyed(flick.contentItem, "", []).find(f => f[0].endsWith("\u001f" + place.wanted))
            item = found ? found[1] : null
        }
        if (item === null)
            return
        if (last)
            place.wanted = ""
        const top = place.topOf(item)
        if (top < flick.contentY)
            place.scrollTo(top)
        else if (top + item.height > flick.contentY + flick.height)
            place.scrollTo(Math.min(top, top + item.height - flick.height))
    }

    // Once the window has laid the new rows out, the place put back again: a row's height known
    // only then moved it.
    readonly property Connections laidOut: Connections {
        target: place.view !== null && place.settling !== null ? place.view.Window.window : null
        ignoreUnknownSignals: true

        function onAfterAnimating() {
            const noted = place.settling
            place.settling = null
            if (noted !== null)
                place.putBack(noted)
            place.revealWanted(true)
        }
    }

    // The movement over: the rows held meanwhile given. The person's own scrolling ends a
    // correction still waiting.
    readonly property Connections moves: Connections {
        target: place.flick
        ignoreUnknownSignals: true

        function onMovementStarted() {
            place.settling = null
        }

        // Once every handler of the movement's end has run (a page's own fixing up included).
        function onMovementEnded() {
            if (place.waiting)
                Qt.callLater(place.apply)
        }
    }
}
