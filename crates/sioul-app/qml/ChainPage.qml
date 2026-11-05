// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The chain of ties from one thing ("Ties of …", docs/tasks.md): what it is
// tied to, what those are tied to, up to four steps, both ways, the
// strongest chains first, collections shown with what they hold. Made in
// Rust (`chain::walk`, `sioul.askChain`, brought by `chainReady`); drawn
// here as a layered tree read from the left, one column per step, the tie's
// words on each edge, ties between things already shown as thin curves; or
// as an outline, the same things in the same order. A computer opens on the
// drawing, a phone on the outline; a switch moves between them, the keys on
// the same thing.
//
// Keys, as the W3C tree view pattern has them, in both views: ↓ ↑ move,
// → presses "+" or steps in, ← folds or steps out, Enter opens, Home goes to
// the entry point, the menu key opens the thing's menu, typing finds a title.
// A card opens beside the page on a computer and over it on a phone; Back
// comes back to the same picture. Each opening starts afresh from the entry
// point; "+" moves nothing already shown.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Shapes

FocusScope {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // The entry point, by its address, and the things whose "+" was pressed (once per press).
    property string entry: ""
    property var opened: []
    // The chain as last brought (`chain::ChainView`).
    property var view: null
    // The outline rather than the drawing: a phone's first view.
    property bool outlineShown: page.theme.touch
    // The thing the keys are on, by its place in the outline's order.
    property int current: 0
    // Zoom of the drawing.
    property real zoom: 1
    // What typing found by, and when it stops counting.
    property string typed: ""
    readonly property bool health: page.view !== null && page.view.health === true
    // On a phone, Back closes the page (main.qml's `goBack`); a computer's Escape too.
    readonly property bool canGoBack: true
    readonly property real cardRoom: 420
    // "Ties of …", by the entry point's title.
    readonly property string title: page.view !== null && page.view.things.length > 0 ? page.sioul.textWith("chain-title", "title", page.view.things[0].title) : page.sioul.text("ui-ties")
    // On a phone, two columns across the width, swiped a column at a time; the tie's words then on each thing's second line.
    readonly property int columnWidth: page.theme.touch ? Math.max(180, Math.floor((drawing.width - 16) / 2)) : 300
    readonly property int boxWidth: page.theme.touch ? page.columnWidth - 28 : 220
    readonly property int rowHeight: 60

    // The chain from `uri`, afresh.
    function show(uri) {
        page.entry = uri
        page.opened = []
        page.current = 0
        page.view = null
        things.clear()
        page.ask()
        page.forceActiveFocus()
    }

    function ask() {
        if (page.entry !== "")
            page.sioul.askChain(page.entry, JSON.stringify(page.opened), page.theme.touch)
    }

    function back() {
        page.window.closeChain()
    }

    // The thing at `place`, as a row of the list.
    function at(place) {
        return place >= 0 && place < things.count ? things.get(place) : null
    }

    // One row of the list from the chain's: flat, as a ListModel keeps it.
    function rowOf(t) {
        return {
            uri: t.uri, kind: t.kind, family: t.family, kindWords: t.kind_words, title: t.title, detail: t.detail, date: t.date,
            key: t.key, found: t.found, program: t.program, column: t.column, row: t.row, parentPlace: t.parent === null ? -1 : t.parent,
            how: t.how, howWords: t.how_words, score: t.score, more: t.more, moreWords: t.more_words, opened: t.opened,
            collection: t.collection, count: t.count, busy: t.busy, setAside: t.set_aside, health: t.health, sessions: t.sessions,
            chain: t.chain, name: t.name, when: t.when === null ? 0 : t.when
        }
    }

    // The chain brought: the list made to match it row by row, so that what
    // stays keeps its item (and slides to its new place), the keys kept on
    // the same thing.
    function apply(json) {
        const was = page.at(page.current)
        const wasUri = was !== null ? was.uri : ""
        page.view = JSON.parse(json)
        const rows = page.view.things.map(page.rowOf)
        let i = 0
        for (; i < rows.length; i++) {
            if (i < things.count && things.get(i).uri === rows[i].uri) {
                things.set(i, rows[i])
                continue
            }
            let later = -1
            for (let j = i + 1; j < things.count; j++) {
                if (things.get(j).uri === rows[i].uri) {
                    later = j
                    break
                }
            }
            if (later >= 0) {
                things.remove(i, later - i)
                things.set(i, rows[i])
            } else {
                things.insert(i, rows[i])
            }
        }
        if (things.count > rows.length)
            things.remove(rows.length, things.count - rows.length)
        const found = rows.findIndex(r => r.uri === wasUri)
        page.current = found >= 0 ? found : Math.min(page.current, rows.length - 1)
        page.reveal()
    }

    // "+" on a thing: what is left out under it, eight more.
    function plus(place) {
        const t = page.at(place)
        if (t === null || t.more === 0)
            return
        page.current = place
        page.opened = page.opened.concat([t.uri])
        page.ask()
    }

    // "−": what "+" opened under it folded back.
    function minus(place) {
        const t = page.at(place)
        if (t === null || t.opened === 0)
            return
        page.current = place
        page.opened = page.opened.filter(uri => uri !== t.uri)
        page.ask()
    }

    // A thing opened where it lives: its card beside the page (over it on a phone).
    function openAt(place) {
        const t = page.at(place)
        if (t === null || !t.found || t.busy)
            return
        page.current = place
        page.window.openThing({ uri: t.uri, kind: t.kind, key: t.key, title: t.title, start: t.when }, false)
    }

    // The chain from another thing of it.
    function recentre(place) {
        const t = page.at(place)
        if (t !== null)
            page.show(t.uri)
    }

    function showMenu(place, where) {
        const t = page.at(place)
        if (t === null)
            return
        page.current = place
        if (where !== null)
            page.window.menuAt = where
        nodeMenu.place = place
        nodeMenu.popup()
    }

    // The thing in view, in the drawing.
    function reveal() {
        if (page.outlineShown) {
            outline.positionViewAtIndex(page.current, ListView.Contain)
            return
        }
        const t = page.at(page.current)
        if (t === null)
            return
        const x = (16 + t.column * page.columnWidth) * page.zoom, y = (16 + t.row * page.rowHeight) * page.zoom
        if (x < drawing.contentX || x + 240 * page.zoom > drawing.contentX + drawing.width)
            drawing.contentX = Math.max(0, Math.min(x - 24, drawing.contentWidth - drawing.width))
        if (y < drawing.contentY || y + 60 * page.zoom > drawing.contentY + drawing.height)
            drawing.contentY = Math.max(0, Math.min(y - 24, drawing.contentHeight - drawing.height))
    }

    // The drawing whole in view.
    function fit() {
        page.zoom = Math.max(0.3, Math.min(1, drawing.width / Math.max(1, canvas.width), drawing.height / Math.max(1, canvas.height)))
        drawing.contentX = 0
        drawing.contentY = 0
    }

    // The tree's edges and the cross-ties, each a stretch of one path.
    function edges(cross) {
        if (page.view === null)
            return ""
        const t = page.view.things
        const box = page.boxWidth
        let path = ""
        const left = i => 16 + t[i].column * page.columnWidth, mid = i => 16 + t[i].row * page.rowHeight + 22
        if (!cross) {
            for (let i = 1; i < t.length; i++) {
                if (t[i].parent === null)
                    continue
                const p = t[i].parent, from = left(p) + box, to = left(i), half = from + (to - from) / 3
                path += "M " + from + " " + mid(p) + " H " + half + " V " + mid(i) + " H " + to + " "
            }
        } else {
            for (const c of page.view.cross) {
                const a = c.from, b = c.to
                const ax = left(a) + box / 2, ay = mid(a) + 22, bx = left(b) + box / 2, by = mid(b) - 22
                path += "M " + ax + " " + ay + " C " + ax + " " + (ay + 40) + " " + bx + " " + (by - 40) + " " + bx + " " + by + " "
            }
        }
        return path
    }

    // Keys of the tree view pattern, in both views.
    function key(event) {
        const t = page.at(page.current)
        if (t === null)
            return
        const firstChild = page.current + 1 < things.count && things.get(page.current + 1).parentPlace === page.current ? page.current + 1 : -1
        switch (event.key) {
        case Qt.Key_Down:
            page.current = Math.min(things.count - 1, page.current + 1)
            break
        case Qt.Key_Up:
            page.current = Math.max(0, page.current - 1)
            break
        case Qt.Key_Right:
            if (t.more > 0)
                page.plus(page.current)
            else if (firstChild >= 0)
                page.current = firstChild
            break
        case Qt.Key_Left:
            if (t.opened > 0)
                page.minus(page.current)
            else if (t.parentPlace >= 0)
                page.current = t.parentPlace
            break
        case Qt.Key_Home:
            page.current = 0
            break
        case Qt.Key_End:
            page.current = things.count - 1
            break
        case Qt.Key_Return:
        case Qt.Key_Enter:
            page.openAt(page.current)
            break
        case Qt.Key_Menu:
            page.showMenu(page.current, null)
            break
        default:
            if (event.key === Qt.Key_F10 && (event.modifiers & Qt.ShiftModifier)) {
                page.showMenu(page.current, null)
                break
            }
            // Type-ahead: the next title starting with what was typed.
            if (event.text.length !== 1 || event.text.trim() === "" || (event.modifiers & (Qt.ControlModifier | Qt.AltModifier)))
                return
            page.typed += event.text.toLowerCase()
            typedForgotten.restart()
            for (let k = 1; k <= things.count; k++) {
                const at = (page.current + k - (page.typed.length > 1 ? 1 : 0)) % things.count
                if (things.get(at).title.toLowerCase().startsWith(page.typed)) {
                    page.current = at
                    break
                }
            }
        }
        event.accepted = true
        page.reveal()
    }

    Keys.onPressed: event => page.key(event)
    onOutlineShownChanged: Qt.callLater(page.reveal)

    Timer {
        id: typedForgotten

        interval: 1000
        onTriggered: page.typed = ""
    }

    ListModel {
        id: things
    }

    Connections {
        target: page.sioul

        function onChainReady(uri, opened, json) {
            if (uri === page.entry && opened === JSON.stringify(page.opened))
                page.apply(json)
        }

        // A tie made or undone, a payment, a paper, a letter, a person changed: the same chain read again.
        function onLinksChanged() {
            if (page.visible)
                page.ask()
        }

        function onMoneyChanged() {
            if (page.visible)
                page.ask()
        }

        function onPapersChanged() {
            if (page.visible)
                page.ask()
        }

        function onLettersChanged() {
            if (page.visible)
                page.ask()
        }

        function onPersonsChanged() {
            if (page.visible)
                page.ask()
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: 8

        // On a phone the title takes its own line, the switches the next.
        Label {
            visible: page.theme.touch
            Layout.fillWidth: true
            Layout.preferredWidth: 0
            text: page.title
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: 20
            color: page.theme.text
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 8

            Label {
                visible: !page.theme.touch
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: page.title
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: 22
                color: page.theme.text
            }
            Item {
                visible: page.theme.touch
                Layout.fillWidth: true
            }
            // The drawing or the outline: the same things, the keys on the same one.
            Button {
                objectName: "chainDrawing"
                flat: true
                checkable: true
                checked: !page.outlineShown
                focusPolicy: Qt.NoFocus
                text: page.sioul.text("chain-drawing")
                onClicked: page.outlineShown = false
            }
            Button {
                objectName: "chainOutline"
                flat: true
                checkable: true
                checked: page.outlineShown
                focusPolicy: Qt.NoFocus
                text: page.sioul.text("chain-outline")
                onClicked: page.outlineShown = true
            }
            Button {
                visible: !page.outlineShown
                flat: true
                focusPolicy: Qt.NoFocus
                text: page.sioul.text("chain-fit")
                onClicked: page.fit()
            }
        }

        // The drawing: one column per step from the entry point, the tie's words on each edge.
        Flickable {
            id: drawing

            objectName: "chainDrawingView"
            visible: !page.outlineShown
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            contentWidth: canvas.width * page.zoom
            contentHeight: canvas.height * page.zoom
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            ScrollBar.horizontal: ScrollBar {}
            // A phone's swipe ends on a column.
            onMovementEnded: {
                if (!page.theme.touch)
                    return
                const step = page.columnWidth * page.zoom
                snap.to = Math.max(0, Math.min(drawing.contentWidth - drawing.width, Math.round(drawing.contentX / step) * step))
                snap.start()
            }

            NumberAnimation {
                id: snap

                target: drawing
                property: "contentX"
                duration: 150
                easing.type: Easing.OutCubic
            }

            // Ctrl and the wheel, or a pinch: closer or farther.
            WheelHandler {
                acceptedModifiers: Qt.ControlModifier
                onWheel: event => page.zoom = Math.max(0.3, Math.min(2, page.zoom * (event.angleDelta.y > 0 ? 1.1 : 1 / 1.1)))
            }
            PinchHandler {
                target: null
                property real began: 1
                onActiveChanged: if (active) began = page.zoom
                onActiveScaleChanged: page.zoom = Math.max(0.3, Math.min(2, began * activeScale))
            }

            Item {
                id: canvas

                width: page.view === null ? 0 : 32 + page.view.columns * page.columnWidth
                height: page.view === null ? 0 : 32 + page.view.rows * page.rowHeight
                scale: page.zoom
                transformOrigin: Item.TopLeft

                // Every edge in one Shape: the tree's, then the cross-ties, thinner.
                Shape {
                    anchors.fill: parent
                    preferredRendererType: Shape.CurveRenderer

                    ShapePath {
                        strokeWidth: 1.5
                        strokeColor: page.theme.muted
                        fillColor: "transparent"
                        PathSvg {
                            path: page.edges(false)
                        }
                    }
                    ShapePath {
                        strokeWidth: 1
                        strokeColor: page.theme.line
                        fillColor: "transparent"
                        PathSvg {
                            path: page.edges(true)
                        }
                    }
                }

                Repeater {
                    model: things

                    delegate: Item {
                        id: placed

                        required property int index
                        // Its roles: kept as they were while a removed row's item goes.
                        required property string uri
                        required property string kind
                        required property string family
                        required property string kindWords
                        required property string howWords
                        required property string title
                        required property string detail
                        required property string date
                        required property bool found
                        required property int more
                        required property string moreWords
                        required property int opened
                        required property string name
                        required property int column
                        required property int row
                        required property int parentPlace

                        x: 16 + placed.column * page.columnWidth
                        y: 16 + placed.row * page.rowHeight
                        width: node.width
                        height: node.height
                        // Slides to its new place on "+" and "−", never as the chain first shows.
                        Behavior on y {
                            enabled: page.view !== null
                            NumberAnimation {
                                duration: 200
                                easing.type: Easing.OutCubic
                            }
                        }

                        // How it is tied to the thing it hangs from, on its edge (on the thing's second line on a phone).
                        Label {
                            visible: !page.theme.touch && placed.parentPlace >= 0 && placed.howWords !== ""
                            x: -page.columnWidth + page.boxWidth + 6
                            y: 4
                            width: page.columnWidth - page.boxWidth - 12
                            text: placed.howWords
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            horizontalAlignment: Text.AlignRight
                            font.pixelSize: 10
                            color: page.theme.muted
                        }
                        ChainNode {
                            id: node

                            width: page.boxWidth
                            howOnLine: page.theme.touch
                            sioul: page.sioul
                            theme: page.theme
                            thing: ({ uri: placed.uri, kind: placed.kind, family: placed.family, kindWords: placed.kindWords, howWords: placed.howWords, title: placed.title, detail: placed.detail, date: placed.date, found: placed.found, more: placed.more, moreWords: placed.moreWords, opened: placed.opened, name: placed.name })
                            place: placed.index
                            entry: placed.index === 0
                            current: placed.index === page.current && page.activeFocus
                            onChosen: place => page.current = place
                            onOpened: place => page.openAt(place)
                            onPlus: place => page.plus(place)
                            onMinus: place => page.minus(place)
                            onMenu: (place, where) => page.showMenu(place, where)
                        }
                    }
                }
            }
        }

        ChainOutline {
            id: outline

            objectName: "chainOutlineView"
            visible: page.outlineShown
            Layout.fillWidth: true
            Layout.fillHeight: true
            sioul: page.sioul
            theme: page.theme
            things: things
            current: page.current
            focus: page.outlineShown
            onChosen: place => page.current = place
            onOpened: place => page.openAt(place)
            onPlus: place => page.plus(place)
            onMinus: place => page.minus(place)
            onMenu: (place, where) => page.showMenu(place, where)
        }
    }

    // A thing's menu: open it, its own ties, "+" or "−", the tie undone.
    SioulMenu {
        id: nodeMenu

        property int place: -1
        readonly property var target: page.at(nodeMenu.place)

        MenuItem {
            enabled: nodeMenu.target !== null && nodeMenu.target.found && !nodeMenu.target.busy
            text: page.sioul.text("ui-open")
            onTriggered: page.openAt(nodeMenu.place)
        }
        MenuItem {
            enabled: nodeMenu.target !== null && nodeMenu.place > 0 && !nodeMenu.target.busy
            text: page.sioul.text("chain-its-ties")
            onTriggered: page.recentre(nodeMenu.place)
        }
        MenuItem {
            visible: nodeMenu.target !== null && nodeMenu.target.more > 0
            height: visible ? implicitHeight : 0
            text: page.theme.plain(nodeMenu.target !== null ? nodeMenu.target.moreWords : "")
            onTriggered: page.plus(nodeMenu.place)
        }
        MenuItem {
            visible: nodeMenu.target !== null && nodeMenu.target.opened > 0
            height: visible ? implicitHeight : 0
            text: page.sioul.text("chain-minus")
            onTriggered: page.minus(nodeMenu.place)
        }
        MenuItem {
            enabled: nodeMenu.target !== null && nodeMenu.target.parentPlace >= 0
            text: page.sioul.text("link-undo")
            onTriggered: {
                const above = page.at(nodeMenu.target.parentPlace)
                if (above !== null)
                    page.sioul.unlinkThings(above.uri, nodeMenu.target.uri)
            }
        }
    }
}
