// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The notes of a task or an event, read (docs/tasks.md): their Markdown
// drawn in Rust (description::description_html), whoever wrote them, an
// invitation's sender too. HTML in them shows as
// words, no picture is drawn and nothing is fetched; a link opens in the
// browser (an address, in a new message), never in Sioul. Where a link
// leads is said under the notes before it opens: under the pointer on a
// computer, after a first tap on a touch screen, as in a message.
// Your own notes' checklist is ticked here (docs/tasks.md, "Checklists"): a
// tap on a box ticks it or takes the tick away; its menu (a right click, a
// long press) says done, to do, or not needed this time.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: notes

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The window (main.qml): an address opens a new message there.
    required property var window
    // The notes as written, in Markdown.
    property string markdown: ""
    // How a web link opens: in the browser, never fetched by Sioul (a test puts its own).
    property var opener: url => Qt.openUrlExternally(url)
    // The link under the pointer, or the one tapped once: said before it opens.
    property string pointed: ""
    // What is drawn, for the window's tests.
    readonly property alias drawn: words.text
    // The notes are yours to tick: each box of their checklist a link (`sioul-check:<line>`).
    property bool tickable: false
    // The line of the box whose menu is open.
    property int menuLine: -1
    // An item can be made a task of its own; a step of the task the notes are of.
    property bool converts: false
    property bool steps: false

    // An item of the checklist, by its line in the notes: "toggle", "done", "open" or "skipped".
    signal check(int line, string state)
    // An item made a step ("step") or a task ("task"), by its line.
    signal convert(int line, string to)

    spacing: 4

    // A link of the notes: the web, an address or a number, and nothing else.
    // The first activation of a link not yet said says it; the next opens it.
    function follow(link) {
        const url = String(link)
        // A box: ticked at once, nothing opened.
        if (notes.tickable && /^sioul-check:\d+$/.test(url)) {
            notes.check(Number(url.slice("sioul-check:".length)), "toggle")
            return
        }
        if (!/^(https?|mailto|tel):/i.test(url))
            return
        if (notes.pointed !== url) {
            notes.pointed = url
            return
        }
        if (/^mailto:/i.test(url))
            notes.window.writeTo(notes.address(url))
        else
            notes.opener(url)
    }

    // The address of a mailto: link, without what it asks to write.
    function address(url) {
        const raw = url.slice("mailto:".length).split("?")[0]
        try {
            return decodeURIComponent(raw)
        } catch (malformed) {
            return raw
        }
    }

    // Whether a link is a box of your own checklist.
    function isBox(link) {
        return notes.tickable && /^sioul-check:\d+$/.test(String(link))
    }

    // A box's menu, when the press is on one.
    function boxMenu(link) {
        if (!notes.isBox(link))
            return false
        notes.menuLine = Number(String(link).slice("sioul-check:".length))
        boxActions.popup()
        return true
    }

    onMarkdownChanged: notes.pointed = ""

    TextEdit {
        id: words

        Layout.fillWidth: true
        // Its own width, not its text's: it wraps on a phone.
        Layout.preferredWidth: 120
        readOnly: true
        selectByMouse: true
        textFormat: TextEdit.RichText
        wrapMode: TextEdit.Wrap
        // Links in the theme's colour: rich text would draw them pure blue. Lines get air.
        // Boxes in the words' colour, without a line under them; links in the theme's.
        // rich on purpose: Markdown made HTML in Rust (description::description_html): HTML written in it shows as words, no picture, links to the web, an address or a number only, boxes that tick.
        text: notes.markdown.trim() === "" ? "" : notes.theme.spaced(notes.sioul.descriptionHtml(notes.markdown, notes.tickable).replace(/<a href="sioul-check:/g, '<a style="text-decoration:none; color:' + notes.theme.text + '" href="sioul-check:').replace(/<a href=/g, '<a style="color:' + notes.theme.accent + '" href='))
        color: notes.theme.text
        selectionColor: notes.theme.accent
        selectedTextColor: notes.theme.accentText
        font.family: notes.theme.readingFamily || font.family
        font.pixelSize: notes.theme.readingSize
        onLinkActivated: link => notes.follow(link)
        onLinkHovered: link => notes.pointed = link

        HoverHandler {
            cursorShape: words.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.IBeamCursor
        }
        TapHandler {
            acceptedButtons: Qt.RightButton
            // A touch has no buttons: on a touch screen, the long press below.
            acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
            onTapped: eventPoint => notes.boxMenu(words.linkAt(eventPoint.position.x, eventPoint.position.y))
        }
        TapHandler {
            id: boxHold

            acceptedDevices: PointerDevice.TouchScreen
            onLongPressed: {
                const link = words.linkAt(boxHold.point.position.x, boxHold.point.position.y)
                // Where the finger is (main.qml's menuAt), before the menu opens.
                const window = notes.Window.window
                if (notes.isBox(link) && window && "menuAt" in window)
                    window.menuAt = boxHold.point.scenePosition
                notes.boxMenu(link)
            }
        }
    }
    // A box's menu: done, to do, not needed this time.
    SioulMenu {
        id: boxActions

        MenuItem {
            text: notes.sioul.text("checklist-done")
            onTriggered: notes.check(notes.menuLine, "done")
        }
        MenuItem {
            text: notes.sioul.text("checklist-open")
            onTriggered: notes.check(notes.menuLine, "open")
        }
        MenuItem {
            text: notes.sioul.text("checklist-skipped")
            onTriggered: notes.check(notes.menuLine, "skipped")
        }
        MenuSeparator {
            visible: notes.converts
        }
        MenuItem {
            visible: notes.converts && notes.steps
            height: visible ? implicitHeight : 0
            text: notes.sioul.text("checklist-to-step")
            onTriggered: notes.convert(notes.menuLine, "step")
        }
        MenuItem {
            visible: notes.converts
            height: visible ? implicitHeight : 0
            text: notes.sioul.text("checklist-to-task")
            onTriggered: notes.convert(notes.menuLine, "task")
        }
    }
    Label {
        objectName: "descriptionLink"
        visible: notes.pointed !== ""
        Layout.fillWidth: true
        text: notes.sioul.textWith("ui-link", "url", notes.pointed)
        textFormat: Text.PlainText
        elide: Text.ElideMiddle
        font.pixelSize: 12
        color: notes.theme.muted
    }
}
