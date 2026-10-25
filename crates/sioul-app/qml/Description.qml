// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The notes of a task or an event, read (docs/tasks.md): their Markdown
// drawn in Rust (description::description_html), whoever wrote them, an
// invitation's sender too. HTML in them shows as
// words, no picture is drawn and nothing is fetched; a link opens in the
// browser (an address, in a new message), never in Sioul. Where a link
// leads is said under the notes before it opens: under the pointer on a
// computer, after a first tap on a touch screen, as in a message.

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

    spacing: 4

    // A link of the notes: the web, an address or a number, and nothing else.
    // The first activation of a link not yet said says it; the next opens it.
    function follow(link) {
        const url = String(link)
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
        // rich on purpose: Markdown made HTML in Rust (description::description_html): HTML written in it shows as words, no picture, links to the web, an address or a number only.
        text: notes.markdown.trim() === "" ? "" : notes.theme.spaced(notes.sioul.descriptionHtml(notes.markdown).replace(/<a href=/g, '<a style="color:' + notes.theme.accent + '" href='))
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
