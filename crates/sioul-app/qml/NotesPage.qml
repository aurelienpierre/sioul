// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Notes: your folder of Markdown files, read as Obsidian reads a vault.
// Search first, folders folded: no filing asked. The search finds a note by
// its title, path and tags at once, and by its text after a pause in typing
// (notesview.rs). One line under it chooses what the list shows (by folder,
// last changed, created) and of which type, kept on this device. A note
// opens read, its links followed with a click, wherever they lead (another
// note, a task, a message); editing is one switch away. On the side: its
// outline, what is tied to it, and its unticked lines, each one click from
// becoming a task; on a narrow screen, the outline folds at the top of the note.
//
// A note is read as a list of its blocks (Rust cuts them: compose::markdown_blocks),
// only those in sight laid out, so that a long note shows at once; the editor
// takes the note's text only when Edit is pressed; a change elsewhere in the
// notes folder leaves the note open as it is (docs/tasks.md, "Notes: a long note opens at once").

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
    readonly property var shown: page.notesText ? JSON.parse(page.notesText) : ({ missing: false, notes: [], recent: [], folders: [], groups: [], show: "folder", type: "all", query: "", looking: false, namesOnly: false })
    // What the list shows and of which type, as Rust keeps them on this device; whether it shows what a search found.
    readonly property string showing: page.shown.show || "folder"
    readonly property string typed: page.shown.type || "all"
    readonly property bool searching: (page.shown.query || "") !== ""

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
    // The note as read: its blocks, each { html, kind, end, picture, key } (`notesview::blocks_json`),
    // and the stamp of the text they were made from. Its file unchanged, the note is not read again.
    property var blocks: []
    property string shownStamp: ""
    // The whole note selected in the reading view (Ctrl+A): copying takes its Markdown text.
    property bool wholeSelected: false
    // The note open, as what new things are tied to.
    readonly property var source: page.note && page.path !== "" ? { uri: page.sioul.uriOf("note", page.path), kind: "note", key: page.path, title: page.note.title } : null
    property bool editing: false
    property bool dirty: false
    // A newer version of the note open came through the sharing while you typed in it: said, never put over your text.
    property bool newer: false
    // On a phone, the note open takes the page; Back keeps it (saved) and
    // closes it (main.qml).
    readonly property bool canGoBack: page.note !== null
    // The page's ⚙, open at one of its settings ("case_store", the notes folder, from a
    // link by key: SettingDoor.qml, the sharing's line), or closed; with no folder too.
    function openSettings(open, key) {
        notesSettings.show(open)
        if (open && key)
            Qt.callLater(() => notesSettings.scrollTo(key))
    }

    function back() {
        page.open("")
    }
    // A picture, a PDF, a sound: read or played, never edited here; an office document ("document"): named, opened elsewhere.
    readonly property string kind: page.note ? page.note.kind : "text"
    property var openFolders: ({})
    // What changed lately: folded until opened, as you left it.
    property bool recentOpen: page.sioul.viewFlag("notes-recent")
    property string query: ""
    // A memo just recorded, opened once the notes are read again.
    property string pending: ""
    // What the left column lists: when searching, what was found (by name,
    // then by text); by folder, what changed lately, then the tree of
    // folders; last changed or created, one list under quiet headings.
    readonly property var rows: {
        const note = (n, depth, showPath) => ({ type: "note", note: n, depth: depth, showPath: showPath })
        if (page.searching)
            return page.shown.notes.map(n => note(n, 0, true))
        if (page.showing !== "folder") {
            let dated = []
            for (const group of page.shown.groups || [])
                dated = dated.concat([{ type: "heading", title: page.whenWords(group.when), path: group.when }], group.notes.map(n => note(n, 0, true)))
            return dated
        }
        const recent = page.shown.recent.length > 0 ? [{ type: "recent", title: page.sioul.text("note-recent"), open: page.recentOpen, depth: 0 }].concat(page.recentOpen ? page.shown.recent.map(n => note(n, 1, true)) : []) : []
        const all = [{ type: "heading", title: page.sioul.text("note-all") }]
        return recent.concat(all).concat(page.tree())
    }

    // A dated list's heading: "today", "week"… in words; a year as it is.
    function whenWords(when) {
        switch (when) {
        case "after":
            return page.sioul.text("note-when-after")
        case "today":
            return page.sioul.text("note-when-today")
        case "week":
            return page.sioul.text("note-when-week")
        case "month":
            return page.sioul.text("note-when-month")
        case "year":
            return page.sioul.text("note-when-year")
        case "unknown":
            return page.sioul.text("note-when-unknown")
        default:
            return when
        }
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
        page.wholeSelected = false
        page.reload()
    }

    // A note opened at one of its headings ("#Money" in a link): its words,
    // case aside, or as a web page writes them ("#money-first"); none found,
    // the note opens at its top.
    function openAt(path, anchor) {
        const same = page.note !== null && path === page.path
        page.open(path)
        if (anchor === "")
            return
        // Its text drawn already, else once the window has laid it out.
        if (same)
            page.goToAnchor(anchor)
        else
            page.wanted = { anchor: anchor }
    }

    // A heading asked for while its note opens ({ anchor }), or chosen in the
    // fold that closes ({ heading }): gone to once the window has laid the
    // note out anew, after Qt's polish and before the frame is drawn.
    property var wanted: null

    Connections {
        target: page.wanted !== null ? page.Window.window : null
        ignoreUnknownSignals: true

        function onAfterAnimating() {
            const wanted = page.wanted
            page.wanted = null
            if (wanted.heading)
                page.goToHeading(wanted.heading)
            else
                page.goToAnchor(wanted.anchor)
        }
    }

    function goToAnchor(anchor) {
        const slug = words => words.toLowerCase().trim().replace(/\s+/g, "-").replace(/[^\p{L}\p{N}_-]/gu, "")
        const found = page.outline.find(h => h.text.toLowerCase() === anchor.toLowerCase()) || page.outline.find(h => slug(h.text) === slug(anchor))
        if (found)
            page.goToHeading(found)
    }

    // The note's headings: as read, from Rust with the note; while it is
    // edited, from its text after a pause in typing (`outlinePause`).
    property var editedOutline: []
    readonly property var outline: page.note === null || page.kind !== "text" ? [] : page.editing ? page.editedOutline : (page.note.outline || [])
    // Shown only for two headings or more.
    readonly property bool outlined: page.outline.length >= 2
    // The editor holds the note's text only while it is edited: read, no item holds the whole text.
    onEditingChanged: {
        page.wholeSelected = false
        page.editedOutline = page.editing && page.note ? page.note.outline || [] : []
        editor.text = page.editing && page.note ? page.note.text : ""
    }

    // The block at the top of the reading view, and the heading of the
    // outline it is under (its line marked there): -1 before the first.
    readonly property int topBlock: readList.count > 0 ? readList.indexAt(1, readList.contentY + 1) : -1
    readonly property int readHeading: {
        if (page.editing || page.topBlock < 0)
            return -1
        let found = -1
        page.outline.forEach((h, i) => {
            if (h.block !== undefined && h.block <= page.topBlock)
                found = i
        })
        return found
    }

    // A heading of the outline pressed: its place in the note brought to the
    // top of the view. Read, its block (Rust says which: `notesview::place_headings`);
    // edited, the cursor goes to its line.
    function goToHeading(heading) {
        if (page.editing) {
            const text = editor.text
            let at = 0
            for (let line = 1; line < heading.line; ++line) {
                const next = text.indexOf("\n", at)
                if (next < 0)
                    break
                at = next + 1
            }
            editor.cursorPosition = at
            // A phone's keyboard opens only on a tap in the text.
            if (!page.window.compact)
                editor.forceActiveFocus()
            page.scrollTo(editView, editor.positionToRectangle(at).y)
            return
        }
        // The note just read: its blocks given to the list first.
        if (readPlace.rows !== page.blocks)
            readPlace.apply()
        // Moved on purpose: the place noted before is not put back over it.
        readPlace.leave()
        if (!heading.drawn || heading.block === undefined || heading.block < 0 || heading.block >= readList.count)
            readList.positionViewAtBeginning()
        else
            readList.positionViewAtIndex(heading.block, ListView.Beginning)
    }

    // A note's view scrolled so that `y` of its text is at its top, as far as it goes.
    function scrollTo(view, y) {
        const flick = view.contentItem
        flick.contentY = Math.max(0, Math.min(y, flick.contentHeight - flick.height))
    }

    // The space under block `index` of the note as read: the one Qt's rich
    // text leaves between two blocks of one document, the larger of the
    // margins that meet. Above: a heading's own, none for a table nor for an
    // item of a list drawn tight, else a paragraph's; under: none for an item
    // its own words or a nested list end, else a paragraph's. None under the
    // last. `kind` and `end`: the elements a block's first and last lines are
    // drawn in (`compose::Block`). tst_notesblocks.qml holds it to the whole
    // note's layout.
    function gapBelow(index) {
        const next = readPlace.rows[index + 1]
        if (next === undefined || next.kind === "table")
            return 0
        const above = ({ "h1": 18, "h2": 16, "h3": 14, "li": 0 })[next.kind]
        return Math.max(above === undefined ? 12 : above, ["li", "ul", "ol"].indexOf(readPlace.rows[index].end) >= 0 ? 0 : 12)
    }

    // A block's rich text as drawn: links in the theme's colour (rich text
    // would draw them pure blue), lines spaced, pictures no wider than `room`
    // (only a block holding one reads the width). An item with another of its
    // list after it (`more`) has no space under its list: Qt gives that space
    // to a list's last item, before a list nested in it.
    function blockText(block, more, room) {
        const html = (block.picture ? page.fitted(block.html, room) : block.html).replace(/<a href=/g, '<a style="color:' + page.theme.accent + '" href=')
        return page.theme.spaced(more ? "<style>ul, ol { margin-bottom: 0px; }</style>" + html : html)
    }

    // Whether block `index` is an item of a list, another item of it under it.
    function itemFollowed(index) {
        const next = readPlace.rows[index + 1]
        return next !== undefined && next.kind.startsWith("li")
    }

    // A key pressed in the reading view: Ctrl+A selects the whole note, then
    // copying puts its Markdown text on the clipboard; anything else leaves
    // the selection to the block (a selection made with the mouse stays in one).
    function readingKey(event) {
        if (event.matches(StandardKey.SelectAll)) {
            page.wholeSelected = true
            event.accepted = true
        } else if (page.wholeSelected && event.matches(StandardKey.Copy)) {
            if (page.note)
                page.sioul.copyText(page.note.text)
            event.accepted = true
        } else if (page.wholeSelected && event.key !== Qt.Key_Control && event.key !== Qt.Key_Shift && event.key !== Qt.Key_Meta && event.key !== Qt.Key_Alt)
            page.wholeSelected = false
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
            listPlace.leave()
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

    // Words searched, as if typed and left a moment, for the window's images.
    function grabSearch(words) {
        searchField.text = words
        page.query = words
        page.sioul.searchNotes(words)
        page.sioul.searchNoteTexts()
    }

    // The outline's fold opened, where the note shows it (a phone), for the window's images.
    function grabOutline() {
        if (page.narrow)
            outlineFold.open = true
    }

    // Words typed at the end of the note being edited, for the window's images.
    function typeAtEnd(words) {
        editor.insert(editor.length, words)
    }

    // The note changed last, for the window's images.
    // A new note: its name asked first.
    function startNew() {
        newNote.now().open()
    }

    function openFirst() {
        const groups = page.shown.groups || []
        const first = page.shown.recent.length > 0 ? page.shown.recent[0] : groups.length > 0 ? groups[0].notes[0] : null
        if (first)
            page.open(first.path)
    }

    // The note open read again, only when its own file changed (its stamp): a
    // change elsewhere in the folder, a tie made, the plan made again set
    // nothing, and the view stays where it is. Changed, its blocks go to the
    // list, which keeps the block at its top (`readPlace`); the editor, while
    // nothing is typed in it, takes the new text.
    function reload() {
        page.dirty = false
        page.newer = false
        if (page.path !== "" && page.note !== null && page.note.path === page.path && page.shownStamp !== "" && page.sioul.noteStamp(page.path) === page.shownStamp)
            return
        const text = page.path === "" ? "" : page.sioul.note(page.path)
        const note = text === "" ? null : JSON.parse(text)
        page.wholeSelected = false
        page.note = note
        page.shownStamp = note !== null && note.stamp ? note.stamp : ""
        page.blocks = note !== null && note.blocks ? note.blocks : []
        if (page.editing && note !== null && editor.text !== note.text)
            editor.text = note.text
    }

    function save() {
        if (!page.note || !page.dirty)
            return
        const saved = JSON.parse(page.sioul.saveNote(page.path, editor.text, page.note.stamp || ""))
        if (saved.problem !== "") {
            page.sioul.status = saved.problem
            return
        }
        page.dirty = false
        page.newer = false
        page.note.stamp = saved.stamp
        // Changed elsewhere meanwhile: this version went beside it, and is the one open.
        if (saved.path !== page.path) {
            page.path = saved.path
            page.sioul.status = saved.kept
        }
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
        const decoded = words => {
            try {
                return decodeURIComponent(words)
            } catch (malformed) {
                return words
            }
        }
        // A heading of this note, or of another (`[[plan#Money]]`): the note opens there.
        if (href.startsWith("#")) {
            page.goToAnchor(decoded(href.slice(1)))
            return
        }
        if (href.startsWith("sioul:note/")) {
            const [path, anchor] = href.slice(11).split("#")
            page.openAt(decoded(path), decoded(anchor || ""))
            return
        }
        if (href.startsWith("sioul:") || href.startsWith("mid:")) {
            // However it is spelled ("mid:<…>", bare, encoded): one of the note's ties, or the thing itself, as Rust finds it.
            const found = JSON.parse(page.sioul.linkTarget(page.source ? page.source.uri : "", href) || "null")
            page.window.openThing(found || { uri: href, kind: href.startsWith("mid:") ? "mail" : href.split(":")[1].split("/")[0], key: "" })
            return
        }
        const scheme = /^([a-z][a-z0-9+.-]*):/i.exec(href)
        if (scheme === null) {
            // Another note, a picture, a PDF, a sound: shown here when the notes folder has it.
            const path = page.resolve(href)
            const mark = href.indexOf("#")
            if (path !== "" && page.sioul.note(path) !== "")
                page.openAt(path, mark < 0 ? "" : decoded(href.slice(mark + 1)))
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
        // A note changed on another device, come through the sharing: the list
        // and the note open read again, as Health, Papers and Time do; never
        // over what you are typing: that note stays as you have it, and a
        // line says a newer version came (saving then puts yours beside it).
        function onSharedIn(stores) {
            if (stores.indexOf("files/notes/") < 0)
                return
            page.takeShown()
            if (page.path === "" || page.note === null)
                return
            if (!page.dirty) {
                page.reload()
                return
            }
            const stamp = page.sioul.noteStamp(page.path)
            if (stamp === "" || stamp !== page.note.stamp)
                page.newer = true
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

    // No notes folder on this device, or one that does not read: asked for in
    // the page's place (FolderNeeded.qml), never an empty list.
    FolderNeeded {
        id: folderNeeded

        anchors.fill: parent
        sioul: page.sioul
        theme: page.theme
        title: page.sioul.text("ui-notes")
        onChosen: page.sioul.searchNotes(page.query)
    }

    RowLayout {
        id: columns

        visible: !folderNeeded.shown
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
            // narrow, and the buttons left it no width. The names answer as you
            // type; the texts after a pause (`textPause`).
            TextField {
                id: searchField

                objectName: "noteSearch"
                Layout.fillWidth: true
                placeholderText: page.sioul.text("note-search")
                onTextEdited: {
                    page.query = text
                    page.sioul.searchNotes(text)
                    textPause.restart()
                }
            }
            Timer {
                id: textPause

                interval: 300
                onTriggered: page.sioul.searchNoteTexts()
            }
            // The page's head (PageHead.qml): New, a folder, a memo at the left; what the
            // list shows and of which type after them, on a line of their own when the
            // column is narrow; the ⚙ last at the right; on a phone, the two folded
            // behind "Filters".
            PageHead {
                id: notesHead

                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                foldsFilters: page.window.compact === true
                offersFilters: !page.shown.missing
                filtersSet: (page.showing !== "folder" ? 1 : 0) + (page.typed !== "all" ? 1 : 0)
                filtersName: "noteFilters"
                navigation: [
                    Button {
                        text: page.sioul.text("note-new")
                        icon.name: "document-new"
                        icon.color: page.theme.text
                        onClicked: newNote.now().open()
                    },
                    ToolButton {
                        visible: !page.shown.missing
                        icon.name: "folder-new"
                        icon.color: page.theme.text
                        Accessible.name: page.sioul.text("note-folder-new")
                        ToolTip.visible: hovered
                        ToolTip.text: page.sioul.text("note-folder-new")
                        ToolTip.delay: 400
                        onClicked: folderName.now().ask(page.sioul.text("note-folder-new"), "", name => page.sioul.makeFolder("", name), page.sioul.text("note-folder-make"))
                    },
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
                ]
                settings: SettingsButton {
                    id: notesSettings

                    sioul: page.sioul
                    theme: page.theme
                    view: "notes"
                }

                // What the list shows, and of which type, each with its name: one line
                // under the buttons when the column has room for both, else each on its
                // own line, its name beside it; each choice kept on this device as it was
                // left (notesview.rs).
                GridLayout {
                    id: choices

                    readonly property bool oneLine: choices.width >= showName.implicitWidth + typeName.implicitWidth + showBox.implicitWidth + typeBox.implicitWidth + 5 * choices.columnSpacing + 4

                    objectName: "noteChoices"
                    visible: !page.shown.missing && !notesHead.filtersFolded
                    width: notesHead.row.width
                    columns: choices.oneLine ? 6 : 2
                    columnSpacing: 6
                    rowSpacing: 6

                    Label {
                        id: showName

                        objectName: "noteShowName"
                        text: page.sioul.text("note-show")
                        textFormat: Text.PlainText
                        color: page.theme.muted
                    }
                    PlainComboBox {
                        id: showBox

                        objectName: "noteShow"
                        Layout.fillWidth: true
                        Layout.maximumWidth: showBox.implicitWidth
                        // One width whatever is chosen, its widest choice's; squeezed by a narrow
                        // column, its words end in "…" rather than being cut.
                        implicitWidth: Math.ceil(Math.max(...showBox.model.map(c => showBoxMetrics.advanceWidth(c.text)))) + 13 + showBox.leftPadding + showBox.rightPadding
                        spacing: 4
                        textRole: "text"
                        valueRole: "value"
                        contentItem: Label {
                            leftPadding: 12
                            rightPadding: 1
                            text: showBox.displayText
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                            font: showBox.font
                            color: showBox.palette.text
                        }

                        FontMetrics {
                            id: showBoxMetrics

                            font: showBox.font
                        }
                        model: [{ value: "folder", text: page.sioul.text("note-show-folder") }, { value: "changed", text: page.sioul.text("note-show-changed") }, { value: "created", text: page.sioul.text("note-show-created") }]
                        currentIndex: Math.max(0, showBox.indexOfValue(page.showing))
                        Accessible.name: page.sioul.text("note-show")
                        // Each box says its own choice: two chosen in a row both count, before Rust's answer.
                        onActivated: index => page.sioul.setNotesView(showBox.model[index].value, typeBox.model[Math.max(0, typeBox.currentIndex)].value)

                        // Rust's answer, or the page made again: the box follows what is kept.
                        Connections {
                            target: page

                            function onShowingChanged() {
                                showBox.currentIndex = Math.max(0, showBox.indexOfValue(page.showing))
                            }
                        }
                    }
                    Item {
                        visible: choices.oneLine
                        implicitWidth: 4
                    }
                    Label {
                        id: typeName

                        objectName: "noteTypeName"
                        text: page.sioul.text("note-type")
                        textFormat: Text.PlainText
                        color: page.theme.muted
                    }
                    PlainComboBox {
                        id: typeBox

                        objectName: "noteType"
                        Layout.fillWidth: true
                        Layout.maximumWidth: typeBox.implicitWidth
                        // One width whatever is chosen, its widest choice's; squeezed by a narrow
                        // column, its words end in "…" rather than being cut.
                        implicitWidth: Math.ceil(Math.max(...typeBox.model.map(c => typeBoxMetrics.advanceWidth(c.text)))) + 13 + typeBox.leftPadding + typeBox.rightPadding
                        spacing: 4
                        textRole: "text"
                        valueRole: "value"
                        contentItem: Label {
                            leftPadding: 12
                            rightPadding: 1
                            text: typeBox.displayText
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            verticalAlignment: Text.AlignVCenter
                            font: typeBox.font
                            color: typeBox.palette.text
                        }

                        FontMetrics {
                            id: typeBoxMetrics

                            font: typeBox.font
                        }
                        model: [{ value: "all", text: page.sioul.text("note-type-all") }, { value: "text", text: page.sioul.text("note-type-text") }, { value: "sound", text: page.sioul.text("note-type-sound") }, { value: "pdf", text: page.sioul.text("note-type-pdf") }, { value: "other", text: page.sioul.text("note-type-other") }]
                        currentIndex: Math.max(0, typeBox.indexOfValue(page.typed))
                        Accessible.name: page.sioul.text("note-type")
                        onActivated: index => page.sioul.setNotesView(showBox.model[Math.max(0, showBox.currentIndex)].value, typeBox.model[index].value)

                        Connections {
                            target: page

                            function onTypedChanged() {
                                typeBox.currentIndex = Math.max(0, typeBox.indexOfValue(page.typed))
                            }
                        }
                    }
                    Item {
                        visible: choices.oneLine
                        Layout.fillWidth: true
                    }
                }
            }
            // A note added, renamed, moved or deleted, a folder folded, the texts' answer
            // after the names': the list keeps its place (ListPlace.qml); a search, another
            // Show or Type, from its top. A note shows twice (in What changed lately and
            // under All): its key says where.
            ListPlace {
                id: listPlace

                view: list
                source: page.rows
                key: r => r.type === "note" ? ["note", r.depth, r.showPath, r.note.path].join("|") : r.type + "|" + (r.path || "")
                scope: [page.shown.query, page.showing, page.typed]
            }
            ListView {
                id: list

                Layout.fillWidth: true
                Layout.fillHeight: true
                objectName: "noteList"
                clip: true
                spacing: 2
                ScrollBar.vertical: ScrollBar {}
                model: listPlace.rows

                // Under what was found: still looking in the texts, nothing found, and
                // that PDFs and sounds are found by their names; under a list of one
                // type, that it holds nothing.
                footer: ColumnLayout {
                    width: list.width - 10
                    spacing: 4

                    Item {
                        implicitHeight: 4
                    }
                    Repeater {
                        model: [
                            { name: "noteLooking", shown: page.searching && page.shown.looking, words: page.sioul.text("note-search-looking") },
                            { name: "noteNoneFound", shown: page.searching && !page.shown.looking && page.shown.notes.length === 0, words: page.sioul.text("note-search-none") },
                            { name: "noteNamesOnly", shown: page.searching && page.shown.namesOnly, words: page.sioul.text("note-search-names-only") },
                            { name: "noteNoneOfType", shown: !page.searching && page.typed !== "all" && !page.rows.some(r => r.type === "note"), words: page.sioul.text("note-type-none") }
                        ]

                        delegate: Label {
                            required property var modelData

                            objectName: modelData.name
                            visible: modelData.shown
                            Layout.fillWidth: true
                            leftPadding: 8
                            text: modelData.words
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: page.theme.muted
                        }
                    }
                }

                delegate: ItemDelegate {
                    id: row

                    required property var modelData
                    readonly property bool heading: row.modelData.type === "heading"
                    // A folder of the tree, or what changed lately: both fold.
                    readonly property bool folder: row.modelData.type === "folder" || row.modelData.type === "recent"
                    // What changed lately is a heading of the list as All notes is: drawn alike
                    // (no icon, the headings' weight and colour), its arrow because it folds.
                    readonly property bool headed: row.heading || row.modelData.type === "recent"

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

                    // On a touch screen, the menu comes at a long press; letting go then opens nothing.
                    onPressAndHold: {
                        row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
                        if (row.heading || row.modelData.type === "recent")
                            return
                        if (row.folder)
                            folderMenu.now().show(row.modelData.path)
                        else
                            noteMenu.now().show(row.modelData.note)
                    }
                    TapHandler {
                        enabled: !row.heading && row.modelData.type !== "recent"
                        acceptedButtons: Qt.RightButton
                        // A touch has no buttons: on a touch screen, the row's long press.
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onTapped: {
                            if (row.folder)
                                folderMenu.now().show(row.modelData.path)
                            else
                                noteMenu.now().show(row.modelData.note)
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
                            visible: !row.headed
                            iconName: row.folder ? "inode-directory" : ({ "image": "image-x-generic", "pdf": "application-pdf", "audio": "audio-x-generic", "document": "x-office-document" })[row.heading ? "" : row.modelData.note.kind] || "text-x-generic"
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
                                color: row.headed ? page.theme.muted : page.theme.text
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
                            // Found by its text: the line that holds the words, as written.
                            Label {
                                objectName: "noteFoundLine"
                                visible: text !== ""
                                Layout.fillWidth: true
                                // It wraps in the row's width: its own, unwrapped, asks for none.
                                Layout.preferredWidth: 0
                                text: row.heading || row.folder ? "" : row.modelData.note.line || ""
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                font.pixelSize: 12
                                color: page.theme.text
                            }
                        }
                    }
                }
            }
        }

        // The note; on a phone, only once one is open.
        Panel {
            objectName: "notePanel"
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

                        // Its title and path, cut in the room left: never asking their one-line width.
                        ColumnLayout {
                            Layout.fillWidth: true
                            Layout.preferredWidth: 0
                            spacing: 0

                            Label {
                                objectName: "noteTitle"
                                Layout.fillWidth: true
                                text: page.note ? page.note.title : ""
                                textFormat: Text.PlainText
                                font.pixelSize: 20
                                elide: Text.ElideRight
                                color: page.theme.text
                            }
                            // Its path and its tags: a long path loses its middle, never its
                            // file's name nor its tags, and never widens the panel.
                            Label {
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: page.note ? page.note.path + (page.note.tags.length > 0 ? "  ·  #" + page.note.tags.join("  #") : "") : ""
                                textFormat: Text.PlainText
                                elide: Text.ElideMiddle
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                        }
                        // A picture, a PDF, a sound, an office document: also with the desktop's own application.
                        Button {
                            objectName: "noteOpenElsewhere"
                            visible: page.kind !== "text"
                            flat: true
                            text: page.sioul.text("note-open-elsewhere")
                            icon.name: "document-open"
                            icon.color: page.theme.text
                            onClicked: Qt.openUrlExternally(page.note.url)
                        }
                        Button {
                            objectName: "noteEdit"
                            visible: page.kind === "text"
                            text: page.editing ? page.sioul.text("note-read") : page.sioul.text("ui-edit")
                            icon.name: page.editing ? "view-hidden" : "document-edit"
                            icon.color: page.theme.text
                            // Back to reading once saved, where the reading view was; a text that
                            // could not be saved stays in the editor, the line under the page says why.
                            onClicked: {
                                if (!page.editing) {
                                    page.editing = true
                                    return
                                }
                                page.save()
                                if (page.dirty)
                                    return
                                page.editing = false
                                page.reload()
                            }
                        }
                        Button {
                            visible: page.editing
                            enabled: page.dirty
                            text: page.sioul.text("note-save")
                            highlighted: true
                            onClicked: page.save()
                        }
                    }

                    // On a phone and in a narrow window, the outline folds at the top of
                    // the note, closed until opened; a long one scrolls in a third of the
                    // page. A heading chosen there, it closes, as a menu does, and the
                    // note's text, given back its room, shows that heading.
                    Fold {
                        id: outlineFold

                        objectName: "outlineFold"
                        visible: page.outlined && page.narrow
                        Layout.fillWidth: true
                        theme: page.theme
                        title: page.sioul.text("note-outline")
                        titleSize: 14
                        framed: false

                        ScrollView {
                            Layout.fillWidth: true
                            Layout.maximumHeight: Math.round(page.height / 3)
                            implicitHeight: foldOutline.implicitHeight
                            contentWidth: availableWidth
                            clip: true

                            NoteOutline {
                                id: foldOutline

                                width: parent.width
                                theme: page.theme
                                headings: page.narrow ? page.outline : []
                                current: page.readHeading
                                onChosen: heading => {
                                    outlineFold.open = false
                                    page.wanted = { heading: heading }
                                }
                            }
                        }
                    }

                    // An office document: what Sioul does with it, in one line.
                    Label {
                        objectName: "noteFileSaid"
                        visible: page.kind === "document"
                        Layout.fillWidth: true
                        text: page.sioul.text("note-file-elsewhere")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: page.theme.muted
                    }
                    Item {
                        visible: page.kind === "document"
                        Layout.fillHeight: true
                    }

                    // A newer version came while you typed: said calmly, your text left as it is.
                    Label {
                        visible: page.newer && page.dirty
                        Layout.fillWidth: true
                        text: page.sioul.text("note-newer-came")
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 13
                        color: page.theme.muted
                    }

                    // A picture, fitted to the page, at most its own size; read at
                    // the screen's width at most (a photo of 12 megapixels read
                    // whole held 48 MB), not again as the window is resized.
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

                            width: Math.min(parent.parent.width, picture.implicitWidth > 0 ? picture.implicitWidth : parent.parent.width)
                            sourceSize.width: Math.ceil(Screen.width * Screen.devicePixelRatio)
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
                            sioul: page.sioul
                            theme: page.theme
                            window: page.window
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

                    // The note's blocks given to the list: read again (its file changed),
                    // the block at the top stays where it was; another note, from its top.
                    ListPlace {
                        id: readPlace

                        view: readList
                        source: page.blocks
                        key: "key"
                        scope: page.path
                    }
                    // The note as read: a list of its blocks, only those in sight (and a
                    // little around) laid out, each delegate used again for the next
                    // block scrolled to. The whole note selected (Ctrl+A): its ground in
                    // the selection's colour.
                    ScrollView {
                        id: readView

                        objectName: "noteReadView"
                        visible: !page.editing && page.kind === "text"
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        background: Rectangle {
                            color: page.wholeSelected ? Qt.alpha(page.theme.selection, 0.3) : "transparent"
                            radius: page.theme.radius
                        }

                        ListView {
                            id: readList

                            objectName: "noteReadList"
                            model: readPlace.rows
                            reuseItems: true
                            cacheBuffer: 600

                            // A block: one text item, nothing else at work in it. Its text
                            // depends on the width only when it holds a picture.
                            delegate: TextEdit {
                                id: block

                                required property var modelData
                                required property int index

                                objectName: "noteBlock"
                                width: readList.width
                                // The space before the next block under it: a block brought to
                                // the top shows its first line there.
                                bottomPadding: page.gapBelow(block.index)
                                readOnly: true
                                selectByMouse: true
                                // Pictures written the usual Markdown way, relative to the note.
                                baseUrl: page.note && page.note.base ? page.note.base : ""
                                textFormat: TextEdit.RichText
                                wrapMode: TextEdit.Wrap
                                // rich on purpose: Markdown made HTML in Rust (compose::markdown_blocks): HTML written in it shows as words, a picture from elsewhere as a link.
                                text: page.blockText(block.modelData, page.itemFollowed(block.index), block.modelData.picture ? block.width - 8 : 0)
                                color: page.theme.text
                                font.family: page.theme.readingFamily || font.family
                                font.pixelSize: page.theme.readingSize
                                onLinkActivated: link => page.follow(link)
                                Keys.onPressed: event => page.readingKey(event)
                                // A click in a block ends the whole note's selection.
                                onActiveFocusChanged: {
                                    if (block.activeFocus)
                                        page.wholeSelected = false
                                }
                                onCursorPositionChanged: {
                                    if (block.activeFocus)
                                        page.wholeSelected = false
                                }

                                HoverHandler {
                                    cursorShape: parent.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.IBeamCursor
                                }
                            }
                        }
                    }
                    ScrollView {
                        id: editView

                        objectName: "noteEditView"
                        visible: page.editing
                        Layout.fillWidth: true
                        Layout.fillHeight: true

                        TextArea {
                            id: editor

                            objectName: "noteEditor"
                            // Its text given when Edit is pressed, taken back after (`onEditingChanged`).
                            wrapMode: TextArea.Wrap

                            // A field shows where it is: a border, darker when it has the focus.
                            background: Rectangle {
                                color: page.theme.field
                                radius: page.theme.radius
                                border.color: editor.activeFocus ? page.theme.focusEdge : page.theme.edge
                                border.width: editor.activeFocus ? 2 : 1
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
                                // The outline follows the text after a pause, never at each key.
                                if (page.editing)
                                    outlinePause.restart()
                            }

                            Timer {
                                id: outlinePause

                                interval: 600
                                onTriggered: page.editedOutline = JSON.parse(page.sioul.noteOutline(page.path, editor.text) || "[]")
                            }
                        }
                    }
                }

                // What is tied to it, and its unticked lines: beside the note, or under
                // it on a narrow screen, a third of the height at most.
                ScrollView {
                    objectName: "noteSide"
                    Layout.fillHeight: !page.narrow
                    Layout.fillWidth: page.narrow
                    Layout.maximumHeight: page.narrow ? Math.round(page.height / 3) : Number.POSITIVE_INFINITY
                    Layout.preferredWidth: page.narrow ? -1 : 260
                    contentWidth: availableWidth
                    clip: true

                    ColumnLayout {
                        width: parent.width
                        spacing: 10

                        // The note's outline first, where the column shows beside it.
                        ColumnLayout {
                            objectName: "outlineSide"
                            visible: page.outlined && !page.narrow
                            Layout.fillWidth: true
                            spacing: 2

                            Label {
                                Layout.fillWidth: true
                                text: page.sioul.text("note-outline")
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.weight: Font.DemiBold
                                color: page.theme.text
                                Accessible.role: Accessible.Heading
                            }
                            NoteOutline {
                                Layout.fillWidth: true
                                theme: page.theme
                                headings: page.narrow ? [] : page.outline
                                current: page.readHeading
                                onChosen: heading => page.goToHeading(heading)
                            }
                        }
                        Button {
                            // Tied to an event: what is tied follows the ties as they change, the note's text unchanged.
                            visible: page.note !== null && relatedList.items.some(r => r.kind === "event")
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
                            id: relatedList

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
                                    objectName: "noteLineTask"
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

    Later {
        id: newNote

        sourceComponent: Component {
            SafeDialog {
                id: newNoteForm

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, Math.min(page.width, newNoteForm.roomWidth) - 2 * page.theme.gap)
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
                    onAccepted: newNoteForm.accept()
                }
            }
        }
    }

    // Right click on a note: open it, or tie it to something new or something that exists.
    Later {
        id: noteMenu

        sourceComponent: Component {
            SioulMenu {
                id: noteMenuForm

                property var target: null
                property var source: null

                function show(note) {
                    noteMenuForm.target = note
                    noteMenuForm.source = { uri: page.sioul.uriOf("note", note.path), kind: "note", key: note.path, title: note.title }
                    noteMenuForm.popup()
                }

                MenuItem {
                    text: page.sioul.text("ui-open")
                    onTriggered: page.open(noteMenuForm.target.path)
                }
                MenuSeparator {}
                AddMenu {
                    sioul: page.sioul
                    window: page.window
                    source: noteMenuForm.source
                }
                MenuItem {
                    text: page.sioul.text("ui-link-existing")
                    onTriggered: page.window.linkFrom(noteMenuForm.source)
                }
                MenuSeparator {}
                MenuItem {
                    text: page.sioul.text("note-rename")
                    onTriggered: renameNote.now().begin(noteMenuForm.target)
                }
                // To the vault's trash, out of sight, "Undo" offered.
                MenuItem {
                    text: page.sioul.text("note-trash")
                    onTriggered: {
                        const problem = page.sioul.trashNote(noteMenuForm.target.path)
                        if (problem !== "")
                            page.sioul.status = problem
                        else if (page.path === noteMenuForm.target.path) {
                            page.path = ""
                            page.note = null
                            page.blocks = []
                            page.shownStamp = ""
                            page.dirty = false
                            page.editing = false
                        }
                    }
                }
            }
        }
    }

    // Right click on a folder: a note or a folder in it, a new name, or out when empty.
    Later {
        id: folderMenu

        sourceComponent: Component {
            SioulMenu {
                id: folderMenuForm

                property string path: ""
                readonly property bool empty: !page.shown.notes.some(n => n.path.startsWith(folderMenuForm.path + "/")) && !(page.shown.folders || []).some(f => f.startsWith(folderMenuForm.path + "/"))

                function show(path) {
                    folderMenuForm.path = path
                    folderMenuForm.popup()
                }

                MenuItem {
                    text: page.sioul.text("note-new-here")
                    onTriggered: folderName.now().ask(page.sioul.text("note-new"), "", title => {
                        const made = page.sioul.createNoteIn(folderMenuForm.path, title)
                        if (made !== "") {
                            page.open(made)
                            page.editing = true
                        }
                        return ""
                    }, page.sioul.text("note-make"))
                }
                MenuItem {
                    text: page.sioul.text("note-folder-new-inside")
                    onTriggered: folderName.now().ask(page.sioul.text("note-folder-new"), "", name => page.sioul.makeFolder(folderMenuForm.path, name), page.sioul.text("note-folder-make"))
                }
                MenuSeparator {}
                MenuItem {
                    text: page.sioul.text("note-folder-rename")
                    onTriggered: folderName.now().ask(page.sioul.text("note-folder-rename"), folderMenuForm.path.split("/").pop(), name => {
                        const answer = JSON.parse(page.sioul.renameFolder(folderMenuForm.path, name))
                        return answer.error || ""
                    }, page.sioul.text("ui-rename"))
                }
                MenuItem {
                    enabled: folderMenuForm.empty
                    text: folderMenuForm.empty ? page.sioul.text("note-folder-remove") : page.sioul.text("note-folder-remove-full")
                    onTriggered: {
                        const problem = page.sioul.removeFolder(folderMenuForm.path)
                        if (problem !== "")
                            page.sioul.status = problem
                    }
                }
            }
        }
    }

    // A name asked for: a new folder, a folder renamed, a note in a folder.
    Later {
        id: folderName

        sourceComponent: Component {
            SafeDialog {
                id: folderNameForm

                property var action: null
                property string problem: ""
                // What its button does, in words: "Make the folder", "Rename".
                property string verb: ""

                function ask(title, current, action, verb) {
                    folderNameForm.title = page.theme.plain(title)
                    folderNameForm.action = action
                    folderNameForm.verb = verb
                    folderNameForm.problem = ""
                    folderField.text = current
                    folderNameForm.open()
                    folderField.selectAll()
                    folderField.forceActiveFocus()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, Math.min(page.width, folderNameForm.roomWidth) - 2 * page.theme.gap)
                // Sioul's own buttons: Qt's standard ones ("OK", "Cancel") are not translated here.
                footer: DialogButtonBox {
                    Button {
                        text: page.theme.plain(folderNameForm.verb)
                        highlighted: true
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }
                onAccepted: {
                    const problem = folderNameForm.action ? folderNameForm.action(folderField.text) : ""
                    if (problem)
                        page.sioul.status = problem
                }

                TextField {
                    id: folderField

                    width: parent.width
                    onAccepted: folderNameForm.accept()
                }
            }
        }
    }

    // A new name: the notes, tasks and ties naming it follow.
    Later {
        id: renameNote

        sourceComponent: Component {
            SafeDialog {
                id: renameNoteForm

                property string path: ""

                function begin(note) {
                    renameNoteForm.path = note.path
                    const name = note.path.split("/").pop()
                    newName.text = note.kind === "text" || /\.(md|txt)$/i.test(name) ? name.replace(/\.(md|txt)$/i, "") : name.replace(/\.[^.]*$/, "")
                    renameNoteForm.open()
                    newName.selectAll()
                    newName.forceActiveFocus()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(440, Math.min(page.width, renameNoteForm.roomWidth) - 2 * page.theme.gap)
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
                    const answer = JSON.parse(page.sioul.renameNoteForm(renameNoteForm.path, newName.text))
                    if (answer.error)
                        page.sioul.status = answer.error
                    else if (page.path === renameNoteForm.path)
                        page.pending = answer.path
                }

                TextField {
                    id: newName

                    width: parent.width
                    onAccepted: renameNoteForm.accept()
                }
            }
        }
    }
}
