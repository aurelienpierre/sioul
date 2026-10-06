// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What a hand does on a vertical timeline, said once for the three of them
// (Health's day and week, the agenda's day and week, the Tasks page's day):
// one set of handlers over the whole timeline, which finds nothing itself
// and asks its timeline what lies under the pointer. A timeline opens no
// slower for its blocks: none of them carries a handler of its own.
//
// With a mouse or a touchpad: a press and a drag moves (`started`, `moved`,
// `released`); a click is a tap; a right click asks for the menu. With a
// finger: a plain swipe scrolls the timeline, as before; a long press is
// `held`, and the timeline says whether it picks something up (`pickUp`):
// then the finger drags it, the timeline holding still, and lifting the
// finger without moving asks for the menu, as a long press always did.
// Dragging near the top or the bottom of the view scrolls it. Nothing moves
// but under the hand: the timeline draws where the thing would go, and
// changes it only when it is let go.

pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: area

    // The timeline's view: still while a finger drags, scrolled at its edges.
    required property Flickable flick
    // The pointer's look where it hovers, and while it drags (the timeline says).
    property int cursorShape: Qt.ArrowCursor
    property int dragCursor: Qt.ClosedHandCursor
    // A finger held this long picks up (seconds); it goes once moved this far (pixels).
    property real holdSeconds: 0.45
    property real threshold: 10
    // Picked up by a finger, or dragged with a mouse.
    readonly property bool dragging: area.going
    // A finger held: the timeline picked something up and the finger has it.
    readonly property bool holding: area.armed

    // The pointer over the timeline, in this item's coordinates; off it: (-1, -1).
    signal hovered(point position)
    signal tapped(point position, point scenePosition)
    signal doubleTapped(point position)
    // A right click; a long press let go without moving, after `pickUp(true)`.
    signal menuAsked(point position, point scenePosition)
    // A finger held still: call `pickUp` to take what is under it.
    signal held(point position, point scenePosition)
    // A drag begins at `position` (where it was pressed), goes on, ends.
    signal started(point position)
    signal moved(point position)
    signal released(point position)
    signal canceled

    // A finger's long press took something: the finger drags it now. With
    // `menuOnRelease`, let go without moving it asks for the menu.
    function pickUp(menuOnRelease) {
        area.armed = true
        area.menuOnRelease = menuOnRelease
        area.flick.interactive = false
    }

    property bool armed: false
    property bool going: false
    property bool menuOnRelease: false
    property point pressed: Qt.point(0, 0)
    property point pressedScene: Qt.point(0, 0)
    // Where the pointer was last, on the screen: the view may scroll under a still hand.
    property point lastScene: Qt.point(0, 0)

    // The hand let go: dropped where it is, or nothing (a long press let go in place, a grab taken away).
    function finish(position, dropped) {
        const was = area.going, held = area.armed
        area.going = false
        area.armed = false
        area.flick.interactive = true
        edgeScroll.stop()
        if (was && dropped)
            area.released(position)
        else if (was || held)
            area.canceled()
    }

    function follow(scene) {
        area.lastScene = scene
        const inView = area.flick.mapFromItem(null, scene.x, scene.y)
        // Within 40 px of the view's top or bottom, and more to see that way: it scrolls.
        const room = 40
        const more = (inView.y < room && area.flick.contentY > 0) || (inView.y > area.flick.height - room && area.flick.contentY < area.flick.contentHeight + area.flick.bottomMargin - area.flick.height)
        if (more && !edgeScroll.running)
            edgeScroll.start()
        area.moved(area.mapFromItem(null, scene.x, scene.y))
    }

    Timer {
        id: edgeScroll

        interval: 16
        repeat: true
        onTriggered: {
            const inView = area.flick.mapFromItem(null, area.lastScene.x, area.lastScene.y)
            const room = 40
            let step = 0
            if (inView.y < room)
                step = -Math.ceil((room - inView.y) / 4)
            else if (inView.y > area.flick.height - room)
                step = Math.ceil((inView.y - area.flick.height + room) / 4)
            const top = Math.max(0, area.flick.contentHeight + area.flick.bottomMargin - area.flick.height)
            const next = Math.max(0, Math.min(top, area.flick.contentY + step))
            if (step === 0 || next === area.flick.contentY) {
                edgeScroll.stop()
                return
            }
            area.flick.contentY = next
            area.moved(area.mapFromItem(null, area.lastScene.x, area.lastScene.y))
        }
    }

    HoverHandler {
        id: hover

        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        cursorShape: area.going ? area.dragCursor : area.cursorShape
        onPointChanged: {
            if (hover.hovered && !area.going)
                area.hovered(hover.point.position)
        }
        onHoveredChanged: {
            if (!hover.hovered)
                area.hovered(Qt.point(-1, -1))
        }
    }

    // A click, a double click; a finger's taps are the long press's handler's.
    TapHandler {
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad | PointerDevice.Stylus
        onTapped: eventPoint => area.tapped(eventPoint.position, eventPoint.scenePosition)
        onDoubleTapped: eventPoint => area.doubleTapped(eventPoint.position)
    }

    TapHandler {
        acceptedButtons: Qt.RightButton
        // A touch has no buttons: on a touch screen, the long press.
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: eventPoint => area.menuAsked(eventPoint.position, eventPoint.scenePosition)
    }

    // A mouse or a touchpad: press, drag. The view does not scroll by dragging
    // with a mouse (its Flickable takes no button): the wheel scrolls it.
    DragHandler {
        id: mouseDrag

        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad | PointerDevice.Stylus
        acceptedButtons: Qt.LeftButton
        target: null
        cursorShape: area.dragCursor
        onActiveChanged: {
            if (mouseDrag.active) {
                area.pressed = mouseDrag.centroid.pressPosition
                area.lastScene = mouseDrag.centroid.scenePosition
                area.going = true
                area.started(mouseDrag.centroid.pressPosition)
            } else if (area.going) {
                area.finish(area.mapFromItem(null, area.lastScene.x, area.lastScene.y), true)
            }
        }
        onCentroidChanged: {
            if (mouseDrag.active)
                area.follow(mouseDrag.centroid.scenePosition)
        }
        onCanceled: area.finish(area.pressed, false)
    }

    // A finger: a tap, a double tap; held, it picks up; a plain swipe scrolls the view as before.
    TapHandler {
        id: hold

        acceptedDevices: PointerDevice.TouchScreen
        longPressThreshold: area.holdSeconds
        onTapped: eventPoint => area.tapped(eventPoint.position, eventPoint.scenePosition)
        onDoubleTapped: eventPoint => area.doubleTapped(eventPoint.position)
        onLongPressed: {
            area.pressed = hold.point.position
            area.pressedScene = hold.point.scenePosition
            area.lastScene = hold.point.scenePosition
            area.held(hold.point.position, hold.point.scenePosition)
        }
    }
    PointHandler {
        id: finger

        acceptedDevices: PointerDevice.TouchScreen
        onPointChanged: {
            if (!area.armed || !finger.active)
                return
            const scene = finger.point.scenePosition
            if (!area.going) {
                // Moved past the drag threshold, from where it was held: it goes.
                const dx = scene.x - area.pressedScene.x, dy = scene.y - area.pressedScene.y
                if (Math.sqrt(dx * dx + dy * dy) < area.threshold)
                    return
                area.going = true
                area.started(area.pressed)
            }
            area.follow(scene)
        }
        onActiveChanged: {
            if (finger.active || !area.armed)
                return
            if (area.going) {
                area.finish(area.mapFromItem(null, area.lastScene.x, area.lastScene.y), true)
                return
            }
            // Held and let go where it was: its menu, as a long press always gave.
            const menu = area.menuOnRelease
            area.finish(area.pressed, false)
            if (menu)
                area.menuAsked(area.pressed, area.pressedScene)
        }
    }
}
