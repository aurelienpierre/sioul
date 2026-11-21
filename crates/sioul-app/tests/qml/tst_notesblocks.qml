// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A long note read on the Notes page (NotesPage.qml) as a list of its
// blocks: only a few of them are made, whatever the note's length, and no
// item holds the whole note's text while it is read; the editor takes it
// only when Edit is pressed; a change elsewhere in the notes folder (the
// same stamp) sets nothing and leaves the view where it was, while a change
// of the note itself (another stamp) reads it again, the block at the top
// staying there; the outline's heading lands on its block and marks the
// heading being read; Ctrl+A then copying gives the whole Markdown text; a
// link is followed. And the blocks stand as the whole note stood in one
// rich text: each block's first line where Qt drew it there. On a stand-in
// for Sioul (made-up words). Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1280
    height: 860

    Theme {
        id: testTheme
    }

    QtObject {
        id: fakeWindow

        property bool compact: false
        property var opened: null

        function openThing(item) {
            fakeWindow.opened = item
        }
        function openChain(uri) {}
        function linkFrom(source) {}
        function openTask(uid) {}
        function openDraft(id) {}
    }

    QtObject {
        id: mock

        signal sharedIn(var stores)
        signal linksChanged
        signal tasksChanged

        readonly property string path: "Long note.md"
        // 1: as first read; 2: a paragraph written at its top from elsewhere.
        property int version: 1
        // How many times the note was read whole (`note`), and what was copied.
        property int reads: 0
        property string copied: ""
        property string status: ""
        property string notes: JSON.stringify({ missing: false, show: "folder", type: "all", query: "", question: 1, looking: false, namesOnly: false, folders: [], recent: [], groups: [], notes: [{ path: mock.path, title: "Long note", folder: "", kind: "text", tags: [], modified: 1, created: null }] })

        // Words of no one: a long paragraph, the same each time.
        readonly property string sentence: "The orchard ledger lists each basket of pears by the day it was picked, the weight read on the old copper scale by the barn door, and the stall at the market that took it, so that nothing is counted twice when the season ends. "
        // A paragraph of about a kilobyte: six hundred blocks, some 600 KB of text.
        readonly property string words: mock.sentence.repeat(5).trim()
        readonly property int length: 600
        // A heading every ten blocks, a link in block 5, a tick box in block 7.
        function blocksOf(version) {
            const out = []
            for (let i = 0; i < mock.length; ++i) {
                if (i % 10 === 0)
                    out.push({ html: "<h2>Part " + (i / 10) + "</h2>\n", kind: "h2", end: "h2", picture: false, key: "k" + i })
                else if (i === 5)
                    out.push({ html: "<p>See <a href=\"sioul:task/t1\">the leaflet task</a> before the market.</p>\n", kind: "p", end: "p", picture: false, key: "k" + i })
                else if (i === 7)
                    out.push({ html: "<ul>\n<li><input disabled=\"\" type=\"checkbox\"/>\nWeigh the last basket</li>\n</ul>\n", kind: "ul", end: "li", picture: false, key: "k" + i })
                else
                    out.push({ html: "<p>Block " + i + ". " + mock.words + "</p>\n", kind: "p", end: "p", picture: false, key: "k" + i })
            }
            if (version === 2)
                out.unshift({ html: "<p>Written at the top from another device.</p>\n", kind: "p", end: "p", picture: false, key: "new" })
            return out
        }
        function textOf(version) {
            return (version === 2 ? "Written at the top from another device.\n\n" : "") + mock.blocksOf(1).map(b => b.html.replace(/<[^>]+>/g, "").trim()).join("\n\n") + "\n"
        }
        function outlineOf(version) {
            const out = []
            for (let i = 0; i < mock.length; i += 10)
                out.push({ level: 2, text: "Part " + (i / 10), line: 1 + i * 2, drawn: true, block: i + (version === 2 ? 1 : 0) })
            return out
        }

        // The blocks of every kind, cut as `compose::markdown_blocks` cuts them, for the layout's
        // test; `end` said where it is not an item's own words, nor a block's own element.
        readonly property var corpus: [
            { html: "<p>W0 a first paragraph of words.</p>\n", kind: "p" },
            { html: "<h1>W1 a title</h1>\n", kind: "h1" },
            { html: "<p>W2 a paragraph under it.</p>\n", kind: "p" },
            { html: "<h2>W3 a part</h2>\n", kind: "h2" },
            { html: "<h3>W4 a section</h3>\n", kind: "h3" },
            { html: "<h4>W5 a smaller one</h4>\n", kind: "h4" },
            { html: "<p>W6 words, then a list drawn tight.</p>\n", kind: "p" },
            { html: "<ul>\n<li>W7 one</li>\n</ul>\n", kind: "ul" },
            { html: "<ul>\n<li>W8 two\n<ul>\n<li>inside\n<ul>\n<li>deeper</li>\n</ul>\n</li>\n</ul>\n</li>\n</ul>\n", kind: "li", end: "ul" },
            { html: "<ul>\n<li>W9 three</li>\n</ul>\n", kind: "li" },
            { html: "<ol start=\"3\">\n<li>W10 third</li>\n</ol>\n", kind: "ol" },
            { html: "<ol start=\"4\">\n<li>W11 fourth</li>\n</ol>\n", kind: "li" },
            { html: "<ul>\n<li>\n<p>W12 apart one</p>\n</li>\n</ul>\n", kind: "ul", end: "p" },
            { html: "<ul>\n<li>\n<p>W13 apart two</p>\n<p>its second paragraph</p>\n</li>\n</ul>\n", kind: "li p", end: "p" },
            { html: "<ul>\n<li><input disabled=\"\" type=\"checkbox\"/>\nW14 a tick box</li>\n</ul>\n", kind: "ul" },
            { html: "<ul>\n<li><input disabled=\"\" type=\"checkbox\" checked=\"\"/>\nW15 ticked</li>\n</ul>\n", kind: "li" },
            { html: "<pre><code>W16 code\n\nafter a blank line\n</code></pre>\n", kind: "pre" },
            { html: "<blockquote>\n<p>W17 quoted</p>\n<ul>\n<li>a list in it</li>\n</ul>\n</blockquote>\n", kind: "blockquote" },
            { html: "<p>W18 before the rule</p>\n", kind: "p" },
            { html: "<hr />\n", kind: "hr" },
            { html: "<p>W20 after the rule, with <a href=\"https://example.org\">a link</a>.</p>\n", kind: "p" },
            { html: "<table><thead><tr><th>W21</th><th>b</th></tr></thead><tbody>\n<tr><td>1</td><td>2</td></tr>\n</tbody></table>\n", kind: "table" },
            { html: "<p>W22 after the table</p>\n", kind: "p" },
            { html: "<h2>W23 a part after words</h2>\n", kind: "h2" },
            { html: "<ul>\n<li>W24 a list under a heading</li>\n</ul>\n", kind: "ul" },
            { html: "<h3>W25 a section after a list</h3>\n", kind: "h3" },
            { html: "<pre><code>W26 code under a heading\n</code></pre>\n", kind: "pre" },
            { html: "<blockquote>\n<p>W27 a quote</p>\n</blockquote>\n", kind: "blockquote" },
            { html: "<p>W28 the end.</p>\n", kind: "p" }
        ]

        function note(path) {
            if (path === "Corpus.md")
                return JSON.stringify({ path: path, title: "Corpus", kind: "text", text: "corpus", blocks: mock.corpus.map((b, i) => ({ html: b.html, kind: b.kind, end: b.end || (/^(ul|ol|li)/.test(b.kind) ? "li" : b.kind), picture: false, key: "c" + i })), outline: [], tags: [], checkboxes: [], related: [], base: "", stamp: "c" })
            if (path !== mock.path)
                return ""
            mock.reads += 1
            return JSON.stringify({
                path: path,
                title: "Long note",
                kind: "text",
                text: mock.textOf(mock.version),
                blocks: mock.blocksOf(mock.version),
                outline: mock.outlineOf(mock.version),
                tags: [],
                checkboxes: [{ text: "Weigh the last basket", done: false, line: 15 }],
                related: [],
                base: "",
                stamp: String(mock.version)
            })
        }
        function noteStamp(path) {
            return path === mock.path ? String(mock.version) : path === "Corpus.md" ? "c" : ""
        }
        function copyText(text) {
            mock.copied = text
            return true
        }
        function linkTarget(source, href) {
            return JSON.stringify({ uri: href, kind: "task", key: "t1" })
        }
        function noteOutline(path, text) {
            return JSON.stringify(mock.outlineOf(mock.version))
        }
        function saveNote(path, text, stamp) {
            return JSON.stringify({ problem: "", stamp: "s", path: path, kept: "" })
        }
        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + ": " + value
        }
        function viewFlag(name) {
            return false
        }
        function setViewFlag(name, on) {}
        function searchNotes(query) {}
        function searchNoteTexts() {}
        function setNotesView(show, kind) {}
        function uriOf(kind, key) {
            return "sioul:" + kind + "/" + encodeURIComponent(key)
        }
        function related(uri) {
            return "[]"
        }
        function isProgram(href) {
            return false
        }
    }

    Item {
        id: room

        width: root.width
        height: root.height

        NotesPage {
            id: page

            anchors.fill: parent
            sioul: mock
            theme: testTheme
            window: fakeWindow
        }
    }

    // The whole note in one rich text, as the page drew it before: where Qt puts each block.
    TextEdit {
        id: whole

        visible: false
        readOnly: true
        textFormat: TextEdit.RichText
        wrapMode: TextEdit.Wrap
        color: testTheme.text
        font.family: testTheme.readingFamily || font.family
        font.pixelSize: testTheme.readingSize
    }

    // What the page sets: its blocks, its note.
    QtObject {
        id: sets

        property int blocks: 0
        property int note: 0

        function clear() {
            sets.blocks = 0
            sets.note = 0
        }
    }
    Connections {
        target: page

        function onBlocksChanged() {
            sets.blocks += 1
        }
        function onNoteChanged() {
            sets.note += 1
        }
    }

    TestCase {
        name: "NotesBlocks"
        when: windowShown

        function named(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = named(kids[i], name)
                if (found)
                    return found
            }
            if (item.contentItem && kids.indexOf(item.contentItem) < 0)
                return named(item.contentItem, name)
            return null
        }
        // Every item of the tree, hidden ones too.
        function every(item, found) {
            if (!item)
                return found
            found.push(item)
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i)
                every(kids[i], found)
            if (item.contentItem && kids.indexOf(item.contentItem) < 0)
                every(item.contentItem, found)
            return found
        }
        function list() {
            return named(page, "noteReadList")
        }
        // The blocks' delegates made, those kept aside for later included.
        function made() {
            return every(list().contentItem, []).filter(i => i.objectName === "noteBlock")
        }
        // The block at the top of the reading view, and how far the view is into it.
        function topNow() {
            const view = list()
            view.forceLayout()
            const index = view.indexAt(1, view.contentY + 1)
            const item = view.itemAtIndex(index)
            return { index: index, key: page.blocks[index].key, offset: view.contentY - item.y }
        }
        function openNote(path) {
            room.height = root.height
            page.open("")
            tryVerify(() => list().count === 0)
            mock.version = 1
            page.open(path)
            tryVerify(() => list().count === page.blocks.length && list().count > 0)
            waitForRendering(page)
        }

        function cleanup() {
            page.editing = false
            page.dirty = false
            page.wholeSelected = false
            mock.version = 1
            mock.copied = ""
            fakeWindow.opened = null
        }

        // Six hundred blocks: a screen of them made, and as many again around; scrolled
        // through, the same delegates used again.
        function test_a_long_note_makes_few_delegates() {
            openNote(mock.path)
            const first = made().length
            verify(first > 3 && first <= 40, first + " delegates made for 600 blocks")
            const view = list()
            for (const at of [150, 300, 599, 0, 450]) {
                view.positionViewAtIndex(at, ListView.Beginning)
                view.forceLayout()
                wait(0)
            }
            view.positionViewAtEnd()
            waitForRendering(page)
            verify(made().length <= 60, made().length + " delegates after a scroll through")
            compare(view.itemAtIndex(599).getText(0, 20).indexOf("Block 599"), 0, "the last block drawn")
        }

        // Read, no item holds the note's whole text (nor its Markdown), the editor none.
        function test_no_item_holds_the_whole_text_while_reading() {
            openNote(mock.path)
            const text = mock.textOf(1)
            verify(text.length > 550000, "a long note: " + text.length)
            const editor = named(page, "noteEditor")
            compare(editor.text, "")
            for (const item of every(page, [])) {
                const shown = item.text
                if (typeof shown === "string")
                    verify(shown.length < 5000, (item.objectName || item) + " holds " + shown.length + " characters")
            }
        }

        // The editor takes the text when Edit is pressed, and lets it go back to reading.
        function test_the_editor_takes_the_text_only_when_edit_is_pressed() {
            openNote(mock.path)
            const editor = named(page, "noteEditor")
            compare(editor.text, "")
            mouseClick(named(page, "noteEdit"))
            verify(page.editing)
            compare(editor.text, mock.textOf(1))
            verify(!page.dirty, "given its text, nothing typed")
            mouseClick(named(page, "noteEdit"))
            verify(!page.editing)
            compare(editor.text, "")
        }

        // A change elsewhere in the notes folder: the note open is not read again, nothing set,
        // the view where it was.
        function test_a_change_elsewhere_sets_nothing() {
            openNote(mock.path)
            list().positionViewAtIndex(300, ListView.Beginning)
            list().contentY += 7
            waitForRendering(page)
            const before = topNow()
            const reads = mock.reads
            sets.clear()
            mock.notesChanged()
            mock.sharedIn(["files/notes/"])
            waitForRendering(page)
            compare(mock.reads, reads, "not read again")
            compare(sets.blocks, 0, "no block set")
            compare(sets.note, 0, "the note left as it is")
            const after = topNow()
            compare(after.key, before.key)
            verify(Math.abs(after.offset - before.offset) < 0.5, "the same place: " + before.offset + " then " + after.offset)
        }

        // The note itself changed (another stamp, a paragraph written above): read again, the
        // block at the top and how far into it kept.
        function test_its_own_change_keeps_the_top_block() {
            openNote(mock.path)
            list().positionViewAtIndex(300, ListView.Beginning)
            list().contentY += 11
            waitForRendering(page)
            const before = topNow()
            const reads = mock.reads
            mock.version = 2
            mock.notesChanged()
            compare(mock.reads, reads + 1, "read again")
            tryVerify(() => list().count === 601, 2000)
            waitForRendering(page)
            tryVerify(() => topNow().key === before.key, 2000, "the same block at the top: " + JSON.stringify(topNow()) + " for " + JSON.stringify(before))
            verify(Math.abs(topNow().offset - before.offset) < 0.5, "as far into it: " + topNow().offset)
            compare(topNow().index, before.index + 1, "one block more above it")
        }

        // A heading of the outline pressed: its block at the top; the heading read marked.
        function test_the_outline_lands_on_the_heading_block() {
            openNote(mock.path)
            const lines = every(named(page, "outlineSide"), []).filter(i => i.objectName === "outlineHeading" && i.visible)
            compare(lines.length, 60)
            mouseClick(lines[5])
            tryVerify(() => topNow().index === 50 && Math.abs(topNow().offset) < 1, 2000, "at block 50: " + JSON.stringify(topNow()))
            compare(page.readHeading, 5)
            verify(lines[5].current && !lines[4].current, "the heading read marked")
            // Far down, beyond the outline's own view: from the page, as a link to it does.
            page.goToHeading(page.outline[42])
            tryVerify(() => topNow().index === 420, 2000)
            compare(page.readHeading, 42)
            // A few lines into part 42: still under its heading.
            list().contentY += 300
            tryCompare(page, "readHeading", 42)
            page.follow("#part-7")
            tryVerify(() => topNow().index === 70, 2000, "a link to a heading, as a web page writes it")
        }

        // Ctrl+A, then copying: the note's whole Markdown text on the clipboard; a click ends it.
        function test_select_all_then_copy_gives_the_markdown_text() {
            openNote(mock.path)
            list().positionViewAtIndex(3, ListView.Beginning)
            waitForRendering(page)
            const block = list().itemAtIndex(3)
            mouseClick(block, 40, block.height / 2)
            verify(block.activeFocus, "the block clicked has the keys")
            keyClick(Qt.Key_A, Qt.ControlModifier)
            verify(page.wholeSelected)
            verify(Qt.colorEqual(named(page, "noteReadView").background.color, Qt.alpha(testTheme.selection, 0.3)), "its ground in the selection's colour")
            keyClick(Qt.Key_C, Qt.ControlModifier)
            compare(mock.copied, mock.textOf(1))
            mouseClick(list().itemAtIndex(4), 60, 20)
            verify(!page.wholeSelected, "a click in the text ends it")
            // A selection made with the mouse copies its own words, as before.
            mock.copied = ""
            keyClick(Qt.Key_C, Qt.ControlModifier)
            compare(mock.copied, "", "not the whole note")
        }

        // A link of a block followed through the page, as the whole text's were.
        function test_a_link_is_followed() {
            openNote(mock.path)
            list().positionViewAtIndex(5, ListView.Beginning)
            waitForRendering(page)
            const block = list().itemAtIndex(5)
            const at = block.getText(0, block.length).indexOf("leaflet")
            const where = block.positionToRectangle(at)
            mouseClick(block, where.x + 4, where.y + where.height / 2)
            tryVerify(() => fakeWindow.opened !== null)
            compare(fakeWindow.opened.uri, "sioul:task/t1")
            // The tick box's line drawn, its line not ticked listed beside the note.
            list().positionViewAtIndex(7, ListView.Beginning)
            const box = list().itemAtIndex(7)
            verify(box.getText(0, box.length).indexOf("Weigh the last basket") >= 0)
            verify(every(named(page, "noteSide"), []).some(i => i.objectName === "noteLineTask" && i.visible))
        }

        // Each block's first line where Qt drew it in the whole note, to the pixel.
        function test_the_blocks_stand_as_the_whole_note_stood() {
            room.height = 2400
            page.open("")
            tryVerify(() => list().count === 0)
            page.open("Corpus.md")
            tryVerify(() => list().count === mock.corpus.length)
            waitForRendering(page)
            list().forceLayout()
            // The whole note: the blocks joined, each list said once around its items.
            let joined = ""
            for (const b of mock.corpus) {
                let html = b.html
                if (b.kind.startsWith("li")) {
                    joined = joined.replace(/<\/(ul|ol)>\n$/, "")
                    html = html.slice(html.indexOf("\n") + 1)
                }
                joined += html
            }
            whole.width = list().width
            whole.text = testTheme.spaced(joined.replace(/<a href=/g, '<a style="color:' + testTheme.accent + '" href='))
            const plain = whole.getText(0, whole.length)
            const firstWhole = whole.positionToRectangle(plain.indexOf("W0")).y
            const first = list().itemAtIndex(0)
            const report = []
            for (let i = 1; i < mock.corpus.length; ++i) {
                const word = "W" + i
                const at = plain.indexOf(word)
                if (at < 0)
                    continue
                const item = list().itemAtIndex(i)
                verify(item !== null, "block " + i + " made")
                const own = item.getText(0, item.length).indexOf(word)
                const y = item.y + item.positionToRectangle(own).y - first.y
                const expected = whole.positionToRectangle(at).y - firstWhole
                if (Math.abs(y - expected) > 0.5)
                    report.push(word + " at " + y + ", in the whole note at " + expected)
            }
            compare(report.join("; "), "")
        }
    }
}
