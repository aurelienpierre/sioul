// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Notes: your folder of Markdown files, read as Obsidian reads a vault.
// Search first, folders folded: no filing asked. A note opens read, its
// links followed with a click, wherever they lead (another note, a task, a
// message); editing is one switch away. On the side: what is tied to it,
// and its unticked lines, each one click from becoming a task.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import com.aurelienpierre.sioul

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // A narrow screen (a phone): what is tied to a note goes under it.
    readonly property bool narrow: page.width < 640
    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string notesText: ""
    readonly property var shown: page.notesText ? JSON.parse(page.notesText) : ({ missing: false, notes: [], recent: [], tree: false })

    function takeShown() {
        if (page.visible)
            page.notesText = page.sioul.notes
    }

    Connections {
        target: page.sioul

        function onNotesChanged() {
            page.takeShown()
        }
    }
    onVisibleChanged: page.takeShown()
    Component.onCompleted: page.takeShown()
    property string path: ""
    property var note: null
    // The note open, as what new things are tied to.
    readonly property var source: page.note && page.path !== "" ? { uri: page.sioul.uriOf("note", page.path), kind: "note", key: page.path, title: page.note.title } : null
    property bool editing: false
    property bool dirty: false
    // On a phone, the note open takes the page; Back keeps it (saved) and
    // closes it (main.qml).
    readonly property bool canGoBack: page.note !== null
    function back() {
        page.open("")
    }
    // A picture, a PDF, a sound: read or played, never edited here.
    readonly property string kind: page.note ? page.note.kind : "text"
    property var openFolders: ({})
    // What changed lately: folded until opened, as you left it.
    property bool recentOpen: page.sioul.viewFlag("notes-recent")
    property string query: ""
    // A memo just recorded, opened once the notes are read again.
    property string pending: ""
    // What the left column lists: when searching, what was found; else what
    // changed lately, then every note, as one list or as a tree of folders.
    readonly property var rows: {
        const note = (n, depth, showPath) => ({ type: "note", note: n, depth: depth, showPath: showPath })
        if (page.query !== "")
            return page.shown.notes.map(n => note(n, 0, true))
        const recent = page.shown.recent.length > 0 ? [{ type: "recent", title: page.sioul.text("note-recent"), open: page.recentOpen, depth: 0 }].concat(page.recentOpen ? page.shown.recent.map(n => note(n, 1, true)) : []) : []
        const all = [{ type: "heading", title: page.sioul.text("note-all") }]
        return recent.concat(all).concat(page.shown.tree ? page.tree() : page.shown.notes.map(n => note(n, 0, true)))
    }

    // Folders first, then the notes in them, each folder folded until opened.
    function tree() {
        const root = { folders: {}, notes: [] }
        // Every folder, empty ones too.
        for (const folder of page.shown.folders || []) {
            let at = root
            for (const part of folder.split("/")) {
                at.folders[part] = at.folders[part] || { folders: {}, notes: [] }
                at = at.folders[part]
            }
        }
        for (const n of page.shown.notes) {
            let at = root
            for (const part of n.folder === "" ? [] : n.folder.split("/")) {
                at.folders[part] = at.folders[part] || { folders: {}, notes: [] }
                at = at.folders[part]
            }
            at.notes.push(n)
        }
        const rows = []
        const walk = (node, prefix, depth) => {
            for (const name of Object.keys(node.folders).sort((a, b) => a.toLowerCase().localeCompare(b.toLowerCase()))) {
                const path = prefix === "" ? name : prefix + "/" + name
                const open = page.openFolders[path] === true
                rows.push({ type: "folder", path: path, title: name, depth: depth, open: open })
                if (open)
                    walk(node.folders[name], path, depth + 1)
            }
            for (const n of node.notes)
                rows.push({ type: "note", note: n, depth: depth, showPath: false })
        }
        walk(root, "", 0)
        return rows
    }

    function open(path) {
        if (page.dirty)
            page.save()
        page.path = path
        page.editing = false
        page.reload()
    }

    // Pictures in a note: never wider than the column, their line not spaced.
    function fitted(html, room) {
        return html.replace(/<img src="([^"]*)" width="(\d+)"(?: height="(\d+)")?>/g, (all, src, width, height) => {
            const shown = Math.max(40, Math.min(Number(width), Math.floor(room)))
            const tall = height ? ' height="' + Math.round(Number(height) * shown / Number(width)) + '"' : ""
            return '<p style="line-height: 100%"><img src="' + src + '" width="' + shown + '"' + tall + '></p>'
        })
    }

    // A folder folded or unfolded: the list stays where it was; unfolded, its
    // notes come into view, its own line kept on screen.
    function toggleFolder(row) {
        const y = list.contentY
        const index = page.rows.findIndex(r => r.type === row.type && r.path === row.path)
        if (row.type === "recent") {
            page.recentOpen = !page.recentOpen
            page.sioul.setViewFlag("notes-recent", page.recentOpen)
        } else {
            const next = Object.assign({}, page.openFolders)
            next[row.path] = !row.open
            page.openFolders = next
        }
        Qt.callLater(() => {
            list.contentY = y
            if (row.open || index < 0)
                return
            let last = index
            while (last + 1 < page.rows.length && (page.rows[last + 1].depth || 0) > (page.rows[index].depth || 0))
                last += 1
            list.positionViewAtIndex(last, ListView.Contain)
            if (list.indexAt(10, list.contentY + 2) > index)
                list.positionViewAtIndex(index, ListView.Beginning)
        })
    }

    // A folder of the tree opened, for the window's images.
    function unfold(path) {
        const row = page.rows.find(r => r.type === "folder" && r.path === path)
        if (row && !row.open)
            page.toggleFolder(row)
    }

    // The list scrolled down by `pixels` (for the window's images).
    function scrollList(pixels) {
        list.contentY = list.contentY + pixels
    }

    // The note changed last, for the window's images.
    // A new note: its name asked first.
    function startNew() {
        newNote.open()
    }

    function openFirst() {
        if (page.shown.recent.length > 0)
            page.open(page.shown.recent[0].path)
    }

    function reload() {
        const text = page.path === "" ? "" : page.sioul.note(page.path)
        page.note = text === "" ? null : JSON.parse(text)
        page.dirty = false
    }

    function save() {
        if (!page.note || !page.dirty)
            return
        const problem = page.sioul.saveNote(page.path, editor.text)
        if (problem === "")
            page.dirty = false
        else
            page.sioul.status = problem
    }

    // "../plan.md#october" from "admin/letters.md" → "plan.md".
    function resolve(href) {
        let target = href.split("#")[0]
        // "100%.md" is no percent-encoding: kept as written rather than thrown on.
        try {
            target = decodeURIComponent(target)
        } catch (malformed) {}
        const parts = page.path.split("/").slice(0, -1).concat(target.split("/"))
        const out = []
        for (const part of parts) {
            if (part === "" || part === ".")
                continue
            if (part === "..")
                out.pop()
            else
                out.push(part)
        }
        return out.join("/")
    }

    // A link of the note: another note, something of Sioul, a file of the notes
    // folder by its place from this note, or a page elsewhere. Elsewhere is web
    // pages, addresses, numbers and this computer's own files only: a link in a
    // note made from a mail must not reach a shared folder (file://server/…,
    // smb:) nor start a program through one of the system's protocols.
    function follow(href) {
        if (href.startsWith("sioul:note/")) {
            page.open(decodeURIComponent(href.slice(11).split("#")[0]))
            return
        }
        if (href.startsWith("sioul:") || href.startsWith("mid:")) {
            // Ties are written with the note's address as the core spells it (spaces encoded).
            const found = JSON.parse(page.sioul.related(page.source ? page.source.uri : "")).find(r => r.uri === href)
            page.window.openThing(found || { uri: href, kind: href.startsWith("mid:") ? "mail" : href.split(":")[1].split("/")[0], key: "" })
            return
        }
        const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(href)
        if (scheme === null) {
            // Another note, a picture, a PDF, a sound: shown here when the notes folder has it.
            const path = page.resolve(href)
            if (path !== "" && page.sioul.note(path) !== "")
                page.open(path)
            else
                page.sioul.status = page.sioul.textWith("note-link-not-found", "path", path)
            return
        }
        const kind = scheme[1].toLowerCase()
        // A program, a script or an installer is not started from a note: its folder opens instead.
        if (kind === "file" && href.toLowerCase().startsWith("file:///") && page.sioul.isProgram(href)) {
            Qt.openUrlExternally(href.slice(0, href.lastIndexOf("/") + 1))
            page.sioul.status = page.sioul.text("link-program")
        } else if (["http", "https", "mailto", "tel"].indexOf(kind) >= 0 || (kind === "file" && href.toLowerCase().startsWith("file:///")))
            Qt.openUrlExternally(href)
        else
            page.sioul.status = page.sioul.textWith("note-link-kept", "url", href)
    }

    Shortcut {
        sequence: "Ctrl+S"
        enabled: page.visible && page.editing
        onActivated: page.save()
    }

    Connections {
        target: page.sioul
        function onNotesChanged() {
            if (page.pending !== "") {
                page.open(page.pending)
                page.pending = ""
            } else if (!page.dirty && page.path !== "")
                page.reload()
        }
    }

    // An audio memo: recorded while this is loaded, the microphone closed otherwise.
    Loader {
        id: memo

        active: false
        sourceComponent: MemoRecorder {
            location: page.sioul.memoUrl()
            onFinished: url => {
                page.pending = page.sioul.memoRecorded(url)
                memo.active = false
            }
            // Qt's own words, in English, after a sentence in yours; no notes folder said as such.
            onFailed: problem => {
                page.sioul.status = problem === "" ? page.sioul.text("note-no-store") : page.sioul.textWith("note-memo-failed", "why", problem)
                memo.active = false
            }
            // The system keeps the microphone closed to Sioul: said, with where it opens.
            onRefused: {
                page.sioul.status = page.sioul.text("note-memo-no-microphone")
                memo.active = false
            }
        }
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // Search, then folders.
        ColumnLayout {
            visible: !(page.window.compact && page.note !== null)
            Layout.fillHeight: true
            Layout.fillWidth: true
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.3)
            Layout.minimumWidth: page.window.compact ? 0 : 200
            spacing: 8

            // The search on a line of its own: beside an open note the column is
            // narrow, and the buttons left it no width.
            TextField {
                Layout.fillWidth: true
                placeholderText: page.sioul.text("note-search")
                onTextEdited: {
                    page.query = text
                    page.sioul.searchNotes(text)
                }
            }
            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    text: page.sioul.text("note-new")
                    icon.name: "document-new"
                    icon.color: page.theme.text
                    onClicked: newNote.open()
                }
                ToolButton {
                    visible: !page.shown.missing
                    icon.name: "folder-new"
                    icon.color: page.theme.text
                    Accessible.name: page.sioul.text("note-folder-new")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("note-folder-new")
                    ToolTip.delay: 400
                    onClicked: folderName.ask(page.sioul.text("note-folder-new"), "", name => page.sioul.makeFolder("", name), page.sioul.text("note-folder-make"))
                }
                // One click starts a memo, one stops it; it opens as a note.
                // As wide as what it shows: a button's background asks for 100 pixels.
                Button {
                    visible: !page.shown.missing
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: memo.item ? page.sioul.textWith("note-memo-stop", "time", Math.floor(memo.item.seconds / 60) + ":" + String(memo.item.seconds % 60).padStart(2, "0")) : ""
                    icon.name: "media-record"
                    icon.color: memo.item ? page.theme.warm : page.theme.text
                    display: memo.item ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
                    Accessible.name: page.sioul.text("note-memo")
                    ToolTip.visible: hovered && !memo.item
                    ToolTip.text: page.sioul.text("note-memo")
                    ToolTip.delay: 400
                    onClicked: {
                        // Stopped before it started (the system still asking for the
                        // microphone): nothing to keep, the recorder goes.
                        if (memo.item && memo.item.started)
                            memo.item.stop()
                        else if (memo.item)
                            memo.active = false
                        else
                            memo.active = true
                    }
                }
                Item {
                    Layout.fillWidth: true
                }
                SettingsButton {
                    sioul: page.sioul
                    theme: page.theme
                    view: "notes"
                }
            }
            Label {
                visible: page.shown.missing
                Layout.fillWidth: true
                text: page.sioul.text("note-no-store")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            // One list, or a tree of folders: your choice, kept.
            RowLayout {
                visible: !page.shown.missing
                spacing: 4

                Repeater {
                    model: [{ tree: false, label: "note-as-list", icon: "view-list-text" }, { tree: true, label: "note-as-tree", icon: "view-list-tree" }]

                    delegate: Button {
                        id: shape

                        required property var modelData

                        text: page.sioul.text(shape.modelData.label)
                        icon.name: shape.modelData.icon
                        icon.color: page.theme.text
                        flat: page.shown.tree !== shape.modelData.tree
                        checkable: true
                        checked: page.shown.tree === shape.modelData.tree
                        onClicked: {
                            const already = page.shown.tree === shape.modelData.tree
                            page.sioul.setNotesTree(shape.modelData.tree)
                            // A click on the shape shown unticks it: its binding ticks it again.
                            if (already)
                                shape.checked = Qt.binding(() => page.shown.tree === shape.modelData.tree)
                        }
                    }
                }
            }
            ListView {
                id: list

                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 2
                ScrollBar.vertical: ScrollBar {}
                model: page.rows

                delegate: ItemDelegate {
                    id: row

                    required property var modelData
                    readonly property bool heading: row.modelData.type === "heading"
                    // A folder of the tree, or what changed lately: both fold.
                    readonly property bool folder: row.modelData.type === "folder" || row.modelData.type === "recent"

                    width: list.width - 10
                    enabled: !row.heading
                    leftPadding: 8 + 16 * (row.modelData.depth || 0)
                    highlighted: !row.heading && !row.folder && page.path === row.modelData.note.path
                    onClicked: {
                        if (row.folder)
                            page.toggleFolder(row.modelData)
                        else
                            page.open(row.modelData.note.path)
                    }

                    TapHandler {
                        enabled: !row.heading && row.modelData.type !== "recent"
                        acceptedButtons: Qt.RightButton
                        onTapped: {
                            if (row.folder)
                                folderMenu.show(row.modelData.path)
                            else
                                noteMenu.show(row.modelData.note)
                        }
                    }

                    background: Rectangle {
                        color: !row.heading && (row.highlighted || row.hovered) ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                        border.color: row.visualFocus ? page.theme.focus : "transparent"
                    }
                    contentItem: RowLayout {
                        spacing: 6

                        Label {
                            visible: row.folder
                            text: row.modelData.open ? "▾" : "▸"
                            textFormat: Text.PlainText
                            color: page.theme.muted
                        }
                        Icon {
                            visible: !row.heading
                            iconName: row.modelData.type === "recent" ? "document-open-recent" : row.folder ? "inode-directory" : ({ "image": "image-x-generic", "pdf": "application-pdf", "audio": "audio-x-generic" })[row.heading ? "" : row.modelData.note.kind] || "text-x-generic"
                            size: 16
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Label {
                                Layout.fillWidth: true
                                text: row.heading || row.folder ? row.modelData.title : row.modelData.note.title
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.weight: row.heading || row.folder ? Font.DemiBold : Font.Normal
                                color: row.heading ? page.theme.muted : page.theme.text
                            }
                            // Where it is, in the list and in what was found.
                            Label {
                                visible: !row.heading && !row.folder && row.modelData.showPath && row.modelData.note.folder !== ""
                                Layout.fillWidth: true
                                text: row.heading || row.folder ? "" : row.modelData.note.folder
                                textFormat: Text.PlainText
                                elide: Text.ElideMiddle
                                font.pixelSize: 11
                                color: page.theme.muted
                            }
                        }
                    }
                }
            }
        }

        // The note; on a phone, only once one is open.
        Panel {
            visible: !page.window.compact || page.note !== null
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.7)
            theme: page.theme

            Label {
                visible: page.note === null
                anchors.centerIn: parent
                text: page.sioul.text("note-none-open")
                color: page.theme.muted
            }

            GridLayout {
                visible: page.note !== null
                anchors.fill: parent
                columns: page.narrow ? 1 : 2
                columnSpacing: page.theme.gap
                rowSpacing: page.theme.gap

                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: 8

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 6

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 0

                            Label {
                                Layout.fillWidth: true
                                text: page.note ? page.note.title : ""
                                textFormat: Text.PlainText
                                font.pixelSize: 20
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: page.note ? page.note.path + (page.note.tags.length > 0 ? "  ·  #" + page.note.tags.join("  #") : "") : ""
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                        }
                        // A picture, a PDF, a sound: also with the desktop's own application.
                        Button {
                            visible: page.kind !== "text"
                            flat: true
                            text: page.sioul.text("note-open-elsewhere")
                            icon.name: "document-open"
                            icon.color: page.theme.text
                            onClicked: Qt.openUrlExternally(page.note.url)
                        }
                        Button {
                            visible: page.kind === "text"
                            text: page.editing ? page.sioul.text("note-read") : page.sioul.text("ui-edit")
                            icon.name: page.editing ? "view-hidden" : "document-edit"
                            icon.color: page.theme.text
                            onClicked: {
                                if (page.editing) {
                                    page.save()
                                    page.reload()
                                }
                                page.editing = !page.editing
                            }
                        }
                        Button {
                            visible: page.editing
                            enabled: page.dirty
                            text: page.sioul.text("note-save")
                            highlighted: true
                            onClicked: page.save()
                        }
                        SettingsButton {
                            visible: page.kind === "text"
                            sioul: page.sioul
                            theme: page.theme
                            reading: true
                        }
                    }

                    // A picture, fitted to the page, at most its own size.
                    Flickable {
                        visible: page.kind === "image"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        contentWidth: picture.width
                        contentHeight: picture.height
                        ScrollBar.vertical: ScrollBar {}

                        Image {
                            id: picture

                            width: Math.min(parent.parent.width, sourceSize.width > 0 ? sourceSize.width : parent.parent.width)
                            source: page.kind === "image" ? page.note.url : ""
                            fillMode: Image.PreserveAspectFit
                            asynchronous: true
                        }
                    }
                    Loader {
                        active: page.kind === "pdf"
                        visible: active
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        sourceComponent: PdfView {
                            source: page.note.url
                        }
                    }
                    Loader {
                        active: page.kind === "audio"
                        visible: active
                        Layout.fillWidth: true
                        sourceComponent: AudioPlayer {
                            source: page.note.url
                            sioul: page.sioul
                            theme: page.theme
                        }
                    }
                    Item {
                        visible: page.kind === "audio"
                        Layout.fillHeight: true
                    }

                    ScrollView {
                        visible: !page.editing && page.kind === "text"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        contentWidth: availableWidth
                        clip: true

                        TextEdit {
                            width: parent.width
                            readOnly: true
                            selectByMouse: true
                            // Pictures written the usual Markdown way, relative to the note.
                            baseUrl: page.note && page.note.base ? page.note.base : ""
                            textFormat: TextEdit.RichText
                            wrapMode: TextEdit.Wrap
                            // Links in the theme's colour: rich text would draw them pure blue. Lines get air.
                            text: page.note && page.kind === "text" ? page.theme.spaced(page.fitted(page.note.html, width - 8).replace(/<a href=/g, '<a style="color:' + page.theme.accent + '" href=')) : ""
                            color: page.theme.text
                            font.family: page.theme.readingFamily || font.family
                            font.pixelSize: page.theme.readingSize
                            onLinkActivated: link => page.follow(link)

                            HoverHandler {
                                cursorShape: parent.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.IBeamCursor
                            }
                        }
                    }
                    ScrollView {
                        visible: page.editing
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        TextArea {
                            id: editor

                            text: page.note ? page.note.text : ""
                            wrapMode: TextArea.Wrap

                            // A field shows where it is: a border, darker when it has the focus.
                            background: Rectangle {
                                color: page.theme.surface
                                radius: page.theme.radius
                                border.color: editor.activeFocus ? page.theme.focus : page.theme.line
                            }
                            // "monospace" is a family on Linux only: the theme names each system's own.
                            font.family: page.theme.readingFamily || page.theme.mono
                            font.pixelSize: page.theme.readingSize

                            TextSpacing {
                                document: editor.textDocument
                                spacing: page.theme.readingSpacing
                            }
                            onTextChanged: {
                                if (page.editing && page.note && editor.text !== page.note.text)
                                    page.dirty = true
                            }
                        }
                    }
                }

                // What is tied to it, and its unticked lines: beside the note, or under
                // it on a narrow screen, a third of the height at most.
                ScrollView {
                    Layout.fillHeight: !page.narrow
                    Layout.fillWidth: page.narrow
                    Layout.maximumHeight: page.narrow ? Math.round(page.height / 3) : Number.POSITIVE_INFINITY
                    Layout.preferredWidth: page.narrow ? -1 : 260
                    contentWidth: availableWidth
                    clip: true

                    ColumnLayout {
                        width: parent.width
                        spacing: 10

                        Button {
                            visible: page.note !== null && page.note.related.some(r => r.kind === "event")
                            Layout.fillWidth: true
                            text: page.sioul.text("note-send-guests")
                            icon.name: "mail-message-new"
                            icon.color: page.theme.text
                            onClicked: {
                                const id = page.sioul.mailNote(page.path)
                                if (id !== "")
                                    page.window.openDraft(id)
                            }
                        }
                        ThingActions {
                            visible: page.source !== null
                            sioul: page.sioul
                            theme: page.theme
                            window: page.window
                            source: page.source
                        }
                        RelatedList {
                            Layout.fillWidth: true
                            sioul: page.sioul
                            theme: page.theme
                            title: page.sioul.text("related-title")
                            uri: page.source ? page.source.uri : ""
                            onOpenThing: item => page.window.openThing(item)
                        }
                        Label {
                            visible: page.note !== null && page.note.checkboxes.some(c => !c.done)
                            text: page.sioul.text("note-open-lines")
                            font.weight: Font.DemiBold
                            color: page.theme.text
                        }
                        Repeater {
                            model: page.note ? page.note.checkboxes.filter(c => !c.done) : []

                            delegate: RowLayout {
                                id: line

                                required property var modelData

                                Layout.fillWidth: true
                                spacing: 6

                                Label {
                                    Layout.fillWidth: true
                                    text: line.modelData.text
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                                ToolButton {
                                    icon.name: "task-new"
                                    icon.color: page.theme.text
                                    Accessible.name: page.sioul.text("note-make-task")
                                    ToolTip.visible: hovered
                                    ToolTip.text: page.sioul.text("note-make-task")
                                    onClicked: {
                                        const answer = JSON.parse(page.sioul.taskFromLine(page.path, line.modelData.line))
                                        if (answer.uid)
                                            page.window.openTask(answer.uid)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Dialog {
        id: newNote

        anchors.centerIn: parent
        modal: true
        width: Math.min(440, page.width - 2 * page.theme.gap)
        title: page.sioul.text("note-new")
        onAboutToShow: noteTitle.clear()
        // Sioul's own buttons: Qt's standard ones ("OK", "Cancel") are not translated here.
        footer: DialogButtonBox {
            Button {
                text: page.sioul.text("note-make")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: page.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }
        onAccepted: {
            const made = page.sioul.createNote(noteTitle.text, "[]")
            if (made !== "") {
                page.open(made)
                page.editing = true
            }
        }

        TextField {
            id: noteTitle

            width: parent.width
            placeholderText: page.sioul.text("note-title")
            onAccepted: newNote.accept()
        }
    }

    // Right click on a note: open it, or tie it to something new or something that exists.
    SioulMenu {
        id: noteMenu

        property var target: null
        property var source: null

        function show(note) {
            noteMenu.target = note
            noteMenu.source = { uri: page.sioul.uriOf("note", note.path), kind: "note", key: note.path, title: note.title }
            noteMenu.popup()
        }

        MenuItem {
            text: page.sioul.text("ui-open")
            onTriggered: page.open(noteMenu.target.path)
        }
        MenuSeparator {}
        AddMenu {
            sioul: page.sioul
            window: page.window
            source: noteMenu.source
        }
        MenuItem {
            text: page.sioul.text("ui-link-existing")
            onTriggered: page.window.linkFrom(noteMenu.source)
        }
        MenuSeparator {}
        MenuItem {
            text: page.sioul.text("note-rename")
            onTriggered: renameNote.begin(noteMenu.target)
        }
        // To the vault's trash, out of sight, "Undo" offered.
        MenuItem {
            text: page.sioul.text("note-trash")
            onTriggered: {
                const problem = page.sioul.trashNote(noteMenu.target.path)
                if (problem !== "")
                    page.sioul.status = problem
                else if (page.path === noteMenu.target.path) {
                    page.path = ""
                    page.note = null
                }
            }
        }
    }

    // Right click on a folder: a note or a folder in it, a new name, or out when empty.
    SioulMenu {
        id: folderMenu

        property string path: ""
        readonly property bool empty: !page.shown.notes.some(n => n.path.startsWith(folderMenu.path + "/")) && !(page.shown.folders || []).some(f => f.startsWith(folderMenu.path + "/"))

        function show(path) {
            folderMenu.path = path
            folderMenu.popup()
        }

        MenuItem {
            text: page.sioul.text("note-new-here")
            onTriggered: folderName.ask(page.sioul.text("note-new"), "", title => {
                const made = page.sioul.createNoteIn(folderMenu.path, title)
                if (made !== "") {
                    page.open(made)
                    page.editing = true
                }
                return ""
            }, page.sioul.text("note-make"))
        }
        MenuItem {
            text: page.sioul.text("note-folder-new-inside")
            onTriggered: folderName.ask(page.sioul.text("note-folder-new"), "", name => page.sioul.makeFolder(folderMenu.path, name), page.sioul.text("note-folder-make"))
        }
        MenuSeparator {}
        MenuItem {
            text: page.sioul.text("note-folder-rename")
            onTriggered: folderName.ask(page.sioul.text("note-folder-rename"), folderMenu.path.split("/").pop(), name => {
                const answer = JSON.parse(page.sioul.renameFolder(folderMenu.path, name))
                return answer.error || ""
            }, page.sioul.text("ui-rename"))
        }
        MenuItem {
            enabled: folderMenu.empty
            text: folderMenu.empty ? page.sioul.text("note-folder-remove") : page.sioul.text("note-folder-remove-full")
            onTriggered: {
                const problem = page.sioul.removeFolder(folderMenu.path)
                if (problem !== "")
                    page.sioul.status = problem
            }
        }
    }

    // A name asked for: a new folder, a folder renamed, a note in a folder.
    Dialog {
        id: folderName

        property var action: null
        property string problem: ""
        // What its button does, in words: "Make the folder", "Rename".
        property string verb: ""

        function ask(title, current, action, verb) {
            folderName.title = title
            folderName.action = action
            folderName.verb = verb
            folderName.problem = ""
            folderField.text = current
            folderName.open()
            folderField.selectAll()
            folderField.forceActiveFocus()
        }

        anchors.centerIn: parent
        modal: true
        width: Math.min(440, page.width - 2 * page.theme.gap)
        // Sioul's own buttons: Qt's standard ones ("OK", "Cancel") are not translated here.
        footer: DialogButtonBox {
            Button {
                text: folderName.verb
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: page.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }
        onAccepted: {
            const problem = folderName.action ? folderName.action(folderField.text) : ""
            if (problem)
                page.sioul.status = problem
        }

        TextField {
            id: folderField

            width: parent.width
            onAccepted: folderName.accept()
        }
    }

    // A new name: the notes, tasks and ties naming it follow.
    Dialog {
        id: renameNote

        property string path: ""

        function begin(note) {
            renameNote.path = note.path
            const name = note.path.split("/").pop()
            newName.text = note.kind === "text" || name.endsWith(".md") ? name.replace(/\.md$/, "") : name.replace(/\.[^.]*$/, "")
            renameNote.open()
            newName.selectAll()
            newName.forceActiveFocus()
        }

        anchors.centerIn: parent
        modal: true
        width: Math.min(440, page.width - 2 * page.theme.gap)
        title: page.sioul.text("note-rename")
        footer: DialogButtonBox {
            Button {
                text: page.sioul.text("ui-rename")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: page.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
        }
        onAccepted: {
            const answer = JSON.parse(page.sioul.renameNote(renameNote.path, newName.text))
            if (answer.error)
                page.sioul.status = answer.error
            else if (page.path === renameNote.path)
                page.pending = answer.path
        }

        TextField {
            id: newName

            width: parent.width
            onAccepted: renameNote.accept()
        }
    }
}
