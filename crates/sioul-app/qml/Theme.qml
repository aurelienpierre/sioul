// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The colours, spacing and reading type of every window: muted, with even
// steps of lightness, no pure white or black, no red for lateness
// (docs/design.md, "Keyboard and senses"). Light or dark follows the desktop.
//
// Each pair that is drawn together keeps its contrast: text on surfaces,
// light text on the accent, text on the hover tint and on the selection. Qt's
// Basic style draws a hovered list entry in `highlightedText` on `light`, and
// selected text in `highlightedText` on `highlight`: both are given colours
// that read on each other here (SioulWindow.qml).

import QtQuick

QtObject {
    id: theme

    // Dark or light: the desktop's, unless forced (for the window's images).
    property bool dark: false

    readonly property color background: dark ? "#1e1f1c" : "#f5f2ec"
    readonly property color surface: dark ? "#272824" : "#fbfaf7"
    readonly property color text: dark ? "#e6e2da" : "#2d2a26"
    readonly property color muted: dark ? "#a8a297" : "#6b655c"
    readonly property color line: dark ? "#3b3b36" : "#e2dccf"
    readonly property color button: dark ? "#34352f" : "#ebe5d9"
    // Under the pointer, or chosen in a list: text keeps its own colour on it.
    readonly property color hover: dark ? "#3a3b35" : "#e5ded1"
    // A pressed or toggled tool button, a scroll bar's handle: one step past the line.
    readonly property color pressed: dark ? "#55574f" : "#c9bfac"
    // Selected text and keyboard focus rings: a mid tone, readable under text and around buttons.
    readonly property color selection: dark ? "#5d8471" : "#5f8572"
    readonly property color accent: dark ? "#93b5a2" : "#4c6b5c"
    readonly property color accentText: dark ? "#1e1f1c" : "#fbfaf7"
    readonly property color warm: dark ? "#d4a56c" : "#8f6330"
    readonly property color forged: dark ? "#d38e70" : "#97573a"
    readonly property color focus: dark ? "#c9d9cf" : "#2f4a3d"
    readonly property int gap: 16
    readonly property int radius: 6

    // Long text (notes, mail, a task's notes): the family, size and spacing chosen in "Aa".
    property string readingFamily: ""
    property int readingSize: 16
    // Line height, as a multiple of the font's (1.5 reads more easily than Qt's 1.0).
    property real readingSpacing: 1.5

    // Each kind of thing by its icon, wherever things are listed.
    readonly property var kindIcons: ({
            "task": "view-task",
            "event": "view-calendar-day",
            "mail": "mail-message",
            "draft": "document-edit",
            "note": "view-pim-notes",
            "contact": "contact-new",
            "budget": "view-filter",
            "case": "folder-documents",
            "web": "insert-link",
            "file": "document-new",
            "other": "insert-link"
        })

    // The projects in the Time page's bars: calm, and apart from each other.
    readonly property var chartColors: dark ? ["#93b5a2", "#d4a56c", "#8fa9c9", "#c99393", "#b3a1cf", "#a8b86c", "#7fb8b8"] : ["#4c6b5c", "#b07d3f", "#4f6d8f", "#9a5a5a", "#6f5f93", "#6e7d32", "#3f7f7f"]

    // Each kind of task by its icon.
    readonly property var taskKindIcons: ({
            "call": "call-start",
            "write": "mail-message-new",
            "online": "internet-services",
            "out": "mark-location",
            "read": "view-readermode",
            "think": "games-hint",
            "make": "run-build"
        })

    // Rich text with its lines spaced: Qt's text documents take line-height from a style sheet.
    function spaced(html) {
        const height = Math.round(theme.readingSpacing * 100)
        return "<style>p, li, blockquote, pre, td, h1, h2, h3, h4, div { line-height: " + height + "%; }</style>" + html
    }

    // Fixed-width type: "monospace" names a font on Linux only; Windows and macOS have their own.
    readonly property string mono: Qt.platform.os === "windows" ? "Consolas" : (Qt.platform.os === "osx" || Qt.platform.os === "macos") ? "Menlo" : "monospace"

    // Words from a message, a site or a server, where Qt guesses HTML from a
    // "<" and has no textFormat to say otherwise (buttons, menu lines,
    // tooltips, dialog titles): a word joiner after each "<" keeps them plain.
    // Else a name holding <img src=…> loads an image from the network, or
    // crashes a button. A Label takes `textFormat: Text.PlainText` instead.
    function plain(text) {
        return text === undefined || text === null ? "" : String(text).replace(/</g, "<\u2060")
    }

    // A file's path as a file:// address, each part escaped: "#", "?" or "%"
    // in a name stay in it; a Windows path ("C:\…") becomes file:///C:/…, a
    // network folder ("\\server\share") file://server/share, as the backend's
    // `file_url` writes them and its `local_path` reads them back.
    function fileUrl(path) {
        const joined = String(path).replace(/\\/g, "/").split("/").map(part => encodeURIComponent(part).replace(/%3A/gi, ":")).join("/")
        return joined.startsWith("//") ? "file:" + joined : "file://" + (joined.startsWith("/") ? "" : "/") + joined
    }

    // A folder chosen in a dialog (a file:// address) as a path: on Windows
    // "file:///C:/Users" is "C:/Users", not "/C:/Users", and
    // "file://server/share" a network folder, "//server/share".
    function localPath(url) {
        const text = String(url)
        const path = /^file:\/\/[^\/]/.test(text) ? "//" + text.slice(7) : text.replace(/^file:\/\//, "")
        return decodeURIComponent(path).replace(/^\/([A-Za-z]:)/, "$1")
    }
}
