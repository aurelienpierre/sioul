// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A contact's card on a phone (ContactsPage.qml), on a stand-in for Sioul:
// an office's card with its opening hours, tied to one thing or to twelve,
// opens with its hours in sight, whole; what it is tied to scrolls with the
// card and never takes its room; Close stays in sight. Delete beside Edit,
// pressed, does what the row menu's Delete does and closes the card, and a
// card of a book you only read offers none. With two address books, the
// card's own is in its main part, in sight, a move asked of the backend and
// the card's book shown until it has moved, a key opening its list and moving
// nothing; with one, under More. A new
// card's form asks its book when two can be written, the one Sioul chose
// first already chosen, and its edit carries the place; a card that exists
// carries none. Run by tools/qml-test.sh. Fiction only.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 412
    height: 891

    Theme {
        id: testTheme

        compact: true
        touch: true
    }

    // An invented office: two ways to reach it, its hours, its ties.
    readonly property var office: ({
        key: "/books/office.vcf", uid: "office-1", name: "Health cover office", org: "Health cover office", title: "", card: "org",
        emails: [{ label: "work", value: "members@healthcover.example.org" }],
        phones: [{ label: "work", value: "+33 1 23 45 67 89" }],
        addresses: [], birthday: "", notes: "", urls: [], categories: [], book: "Contacts", place: "", read_only: false, photo: "", photo_web: "",
        hours: { open: { value: "Mo-Fr 09:00-12:00,14:00-16:30; PH off" }, phone: null, note: "" }
    })
    // An invented person with a postal address (its details under More), and a card of a book read only.
    readonly property var jeanne: ({
        key: "/books/jeanne.vcf", uid: "jeanne-1", name: "Jeanne Exemple", org: "", title: "", card: "",
        emails: [{ label: "home", value: "jeanne@example.org" }],
        phones: [{ label: "cell", value: "+33 6 12 34 56 78" }],
        addresses: [{ label: "home", value: "1 rue de l'Exemple\n99999 Exempleville" }], birthday: "", notes: "", urls: [], categories: [], book: "Contacts", place: "cloud/contacts", read_only: false, photo: "", photo_web: "",
        hours: { open: null, phone: null, note: "" }
    })
    readonly property var shared: ({
        key: "/books/shared.vcf", uid: "shared-1", name: "Sam Exemple", org: "", title: "", card: "",
        emails: [{ label: "work", value: "sam@example.org" }], phones: [],
        addresses: [], birthday: "", notes: "", urls: [], categories: [], book: "Shared with you", place: "cloud/shared", read_only: true, photo: "", photo_web: "",
        hours: { open: null, phone: null, note: "" }
    })
    readonly property var cards: [root.office, root.jeanne, root.shared]
    // The address books, as the view lists them (`ContactsView::books`).
    readonly property var contactsBook: ({ place: "cloud/contacts", name: "Contacts", read_only: false, groups: true })
    readonly property var teamBook: ({ place: "cloud/team", name: "Team", read_only: false, groups: false })
    readonly property var sharedBook: ({ place: "cloud/shared", name: "Shared with you", read_only: true, groups: false })
    readonly property var hours: ({
        name: "Health cover office", card: "org", read_only: false, mark: false,
        open: { value: "Mo-Fr 09:00-12:00,14:00-16:30; PH off", short: "", lines: ["Monday to Friday: 9:00–12:00, 14:00–16:30"], holidays_off: true, unread: false, doubt: "", rest: [], source: "Typed by you on 14 September 2026", stale: false, now: "Open until 16:30" },
        phone: { value: "Mo-Th 09:00-12:00; PH off", short: "", lines: ["Monday to Thursday: 9:00–12:00"], holidays_off: true, unread: false, doubt: "", rest: [], source: "Typed by you on 14 September 2026", stale: false, now: "Closed: opens Monday at 9:00" },
        note: "Closed between Christmas and New Year.", numbers: ["+33 1 23 45 67 89"],
        asked: { name: "Health cover office", place: "", france: true },
        sources: "the directory and OpenStreetMap", map: "", map_name: "OpenStreetMap", holidays_known: true, offline: false
    })

    QtObject {
        id: mock

        signal linksChanged
        signal tasksChanged
        signal notesChanged
        signal eventsChanged
        signal householdChanged
        signal personsChanged
        signal placesChanged
        signal hoursFound(string key, string found)

        property string status: ""
        // How many things the card is tied to.
        property int ties: 1
        // The address books, read only ones too, as the view lists them.
        property var books: [root.contactsBook]
        // Where each card is now, by its file, once moved ("account/id").
        property var moved: ({})
        // What a move answers: null moves it; {losses: […]} asks first, as the backend does.
        property var moveAnswer: null
        // The book the backend proposes for a new card (`newContactBook`).
        property string proposed: "cloud/contacts"
        // What was asked: the cards deleted, the moves, the forms saved.
        property var deleted: []
        property var moves: []
        property var saved: []
        // The list, as the backend's property is (its change is `contactsChanged`).
        property string contacts: JSON.stringify({ contacts: root.cards.map(c => ({ key: c.key, name: c.name, detail: "", title: "", photo: "", place: c.place, read_only: c.read_only })), sentence: "", can_add: true, categories: [], category: "", filter: {}, books: mock.books, counted: "", shown: root.cards.length, total: root.cards.length })

        function text(key) { return key }
        function textWith(key, name, value) { return key + ":" + value }
        function textArgs(key, args) { return key + ":" + args }
        function textCounted(key, count) { return key + ":" + count }
        function contact(key) {
            const card = root.cards.find(c => c.key === key)
            if (!card)
                return "null"
            const place = mock.moved[key] || card.place
            const book = mock.books.find(b => b.place === place)
            return JSON.stringify(Object.assign({}, card, { place: place, book: book ? book.name : card.book }))
        }
        function contactHours(key) { return key === root.office.key ? JSON.stringify(root.hours) : "" }
        function clientProfile(ref) { return JSON.stringify({ exists: false, summary: "" }) }
        function personOf(key) { return "null" }
        function personOfCard(uid) { return "null" }
        function household() { return JSON.stringify({ persons: [], others: [], relations: [] }) }
        // As the backend: only the books a contact can go into, those that can be written.
        function addressBooks() { return JSON.stringify(mock.books.filter(b => !b.read_only).map(b => ({ id: b.place, name: b.name }))) }
        function newContactBook() { return mock.proposed }
        // Scheduled with Undo by the backend; the stand-in keeps what was asked.
        function deleteContact(key) { mock.deleted = mock.deleted.concat([key]) }
        function moveContact(key, book, confirmed) {
            mock.moves = mock.moves.concat([{ key: key, book: book, confirmed: confirmed }])
            if (mock.moveAnswer !== null && !confirmed)
                return JSON.stringify(mock.moveAnswer)
            const next = Object.assign({}, mock.moved)
            next[key] = book
            mock.moved = next
            return JSON.stringify({ key: key })
        }
        function saveContact(key, edit) {
            mock.saved = mock.saved.concat([{ key: key, edit: JSON.parse(edit) }])
            return JSON.stringify({ key: key || "/books/new.vcf" })
        }
        function placeOf(address) { return "" }
        function related(uri) {
            const out = []
            for (let i = 0; i < mock.ties; i++)
                out.push({ uri: "sioul:task/call-" + i, how: "Involves them", title: "Call the office about refund " + (i + 1), detail: "", when: "", kind: "task", found: true })
            return JSON.stringify(out)
        }
    }

    QtObject {
        id: shell

        property bool compact: true
        property string sioulLocale: "en"

        function openThing(item) {}
        function writeTo(address) {}
        function openPersonSheet(key, which) {}
        function openInvoiceDraft(draft) {}
        function linkFrom(source) {}
        function cardRef(person) { return "" }
    }

    Component {
        id: pageComponent

        ContactsPage {
            width: root.width
            height: root.height
            sioul: mock
            theme: testTheme
            window: shell
        }
    }

    // Every object under `item` that passes `test`, a scroll view's and a loader's own included.
    function all(item, test, out) {
        out = out || []
        if (!item)
            return out
        const kids = (item.children || [])
        for (let i = 0; i < kids.length; ++i) {
            const kid = kids[i]
            if (test(kid))
                out.push(kid)
            root.all(kid, test, out)
            if (kid.contentItem !== undefined && kid.contentItem !== null && kid.contentItem !== kid && kids.indexOf(kid.contentItem) < 0)
                root.all(kid.contentItem, test, out)
        }
        return out
    }

    function named(item, name) {
        return root.all(item, c => c.objectName === name)[0]
    }

    TestCase {
        name: "ContactCard"
        when: windowShown

        // Each test from the same start: one book, nothing asked yet.
        function init() {
            mock.ties = 1
            mock.books = [root.contactsBook]
            mock.moved = {}
            mock.moveAnswer = null
            mock.proposed = "cloud/contacts"
            mock.deleted = []
            mock.moves = []
            mock.saved = []
        }

        // The page with a card open, laid out.
        function opened(card) {
            const page = createTemporaryObject(pageComponent, root)
            page.open(card.key)
            waitForRendering(page)
            return page
        }

        // What shows by its objectName, or by its words: visible ones only.
        function shown(page, name) {
            return root.all(page, c => c.visible && c.objectName === name)[0]
        }
        function says(page, words) {
            return root.all(page, c => c.visible && c.text === words)[0]
        }

        // A choice in a box, as a person makes it: the box opened, the line pressed.
        function choose(box, index) {
            mouseClick(box)
            tryVerify(() => box.popup.opened, 2000)
            const list = box.popup.contentItem
            tryVerify(() => list.itemAtIndex(index) !== null, 2000)
            waitForRendering(list)
            mouseClick(list.itemAtIndex(index))
            tryVerify(() => !box.popup.visible, 2000)
        }

        // Within the card's width on a phone: never past the screen's edge.
        function within(item) {
            const x = item.mapToItem(root, 0, 0).x
            verify(x >= 0 && x + item.width <= root.width + 0.5, item.objectName + ": " + x + " + " + item.width + " within " + root.width)
        }

        // A card open in a computer's window narrower than 860 points (24 November 2026:
        // the list's ⋮ and ⚙ ended under the card, every list action out of reach): the
        // search, ⚙ and each list action stay inside the list's column, each with its
        // word, the actions on a line of their own under the search when they do not fit
        // beside it; none in ⋮ (a computer folds nothing).
        function test_a_computer_s_list_head_beside_a_card() {
            shell.compact = false
            const page = createTemporaryObject(pageComponent, root, { width: 800, height: 700 })
            page.open(root.jeanne.key)
            waitForRendering(page)
            const head = shown(page, "contactsHead")
            verify(head !== undefined, "the list's head")
            const right = head.mapToItem(root, head.width, 0).x
            const column = head.parent
            verify(right <= column.mapToItem(root, column.width, 0).x + 0.5, "the head within its column")
            for (const name of ["contactsSettings", "contactsNew"]) {
                const item = shown(page, name)
                verify(item !== undefined, name + " shown")
                const edge = item.mapToItem(root, item.width, 0).x
                verify(edge <= right + 0.5, name + " within the column: " + edge + " > " + right)
            }
            const plus = shown(page, "contactsNew")
            compare(plus.display, AbstractButton.TextBesideIcon, "New contact in words on a computer")
            verify(shown(page, "contactsMore") === undefined, "nothing in ⋮ on a computer")
            verify(plus.mapToItem(root, 0, 0).y > shown(page, "contactsSettings").mapToItem(root, 0, 0).y + 1, "the actions on their own line, under the search")
            shell.compact = true
        }

        // On a phone: "+" for New contact stays on the row, one tap; the others in ⋮ with their words.
        function test_a_phone_s_list_head() {
            testTheme.touch = true
            const page = createTemporaryObject(pageComponent, root)
            waitForRendering(page)
            const plus = shown(page, "contactsNew")
            verify(plus !== undefined, "+ on the row")
            compare(plus.display, AbstractButton.IconOnly, "+ alone, as the rules allow")
            compare(plus.iconName, "list-add")
            verify(shown(page, "contactsMore") !== undefined, "the others in ⋮")
            within(shown(page, "contactsSettings"))
            within(shown(page, "contactsMore"))
            testTheme.touch = false
        }

        // Delete beside Edit, pressed: the card closed, its deletion asked of the
        // backend (which waits ten seconds with Undo in the status line, as the row
        // menu's Delete does). A card of a book you only read offers neither.
        function test_delete_on_the_card() {
            const page = opened(root.jeanne)
            const remove = shown(page, "contactCardDelete")
            verify(remove, "Delete under the card")
            verify(says(page, "ui-edit"), "beside Edit")
            compare(remove.text, "ui-delete")
            verify(!remove.highlighted, "a colour that does not shout")
            within(remove)
            mouseClick(remove)
            compare(mock.deleted, [root.jeanne.key], "deleted as the row menu deletes")
            compare(page.person, null, "the card closed")
            compare(page.openKey, "")
            const readOnly = opened(root.shared)
            verify(readOnly.person !== null)
            verify(!shown(readOnly, "contactCardDelete"), "no Delete on a card of a book read only")
            verify(!says(readOnly, "ui-edit"), "nor Edit")
            verify(says(readOnly, "ui-close"), "Close stays")
        }

        // Two address books: the card's own in its main part, in sight at its top on a
        // phone, More folded; another chosen asks the backend to move it there, and once
        // moved the chooser says where it is. A move the backend asks about first leaves
        // the card's book shown while it asks; a card of a book read only says its book.
        function test_two_books_the_book_on_the_card() {
            mock.books = [root.contactsBook, root.teamBook, root.sharedBook]
            const page = opened(root.jeanne)
            verify(!page.moreShown, "More folded")
            const box = shown(page, "contactBookMain")
            verify(box, "the book in the card's main part")
            verify(!shown(page, "contactBookMore"), "not under More")
            compare(box.currentText, "Contacts", "the card's own book")
            compare(box.count, 2, "the books that can be written")
            const row = shown(page, "contactBookRow")
            within(row)
            within(box)
            // On a phone the chooser is as wide as its book's name needs, 360 points at most, and
            // gives way only at the card's edge.
            const room = Math.min(360, box.implicitWidth, scrollOf(box).width - row.children[0].width - row.spacing)
            verify(box.width >= room - 1, "the chooser " + box.width + " of " + room + " points")
            const flick = scrollOf(box)
            compare(flick.contentY, 0)
            verify(box.mapToItem(flick, 0, box.height).y <= flick.height, "in sight without scrolling")
            choose(box, 1)
            compare(mock.moves.length, 1)
            compare(mock.moves[0].book, "cloud/team")
            compare(mock.moves[0].confirmed, false, "asked first whether it would keep everything")
            tryCompare(box, "currentText", "Team", 2000, "moved, it says where it is")
            // A move the backend asks about: the card's book stays shown until it moves.
            mock.moveAnswer = { losses: ["its photo"] }
            choose(box, 0)
            compare(mock.moves.length, 2)
            compare(mock.moves[1].book, "cloud/contacts")
            compare(box.currentText, "Team", "asked, nothing moved yet: its book shown")
            compare(page.person.place, "cloud/team")
            // A key on the box opens its list; nothing more is asked until a book is chosen.
            box.forceActiveFocus()
            keyClick(Qt.Key_Down)
            tryVerify(() => box.popup.opened, 2000)
            compare(mock.moves.length, 2, "an arrow moves nothing")
            keyClick(Qt.Key_Escape)
            tryVerify(() => !box.popup.visible, 2000)
            compare(box.currentText, "Team")
            // On a computer's width too, the chooser right beside its label: the room left goes after it.
            const wide = createTemporaryObject(pageComponent, root, { width: 1400 })
            wide.open(root.jeanne.key)
            waitForRendering(wide)
            const wideRow = shown(wide, "contactBookRow")
            const wideBox = shown(wide, "contactBookMain")
            const label = wideRow.children[0]
            const gap = wideBox.mapToItem(wideRow, 0, 0).x - (label.x + label.width)
            verify(gap <= wideRow.spacing + 1, "the chooser " + gap + " points after its label")
            // A long name: the row as wide as the card, the chooser giving way at its edge.
            mock.books = [Object.assign({}, root.contactsBook, { name: "Contacts · a-rather-long-server-name.example.org" }), root.teamBook]
            const long = opened(root.jeanne)
            const longRow = shown(long, "contactBookRow")
            const longBox = shown(long, "contactBookMain")
            within(longRow)
            within(longBox)
            const edge = scrollOf(longBox).width
            const right = longBox.mapToItem(scrollOf(longBox), longBox.width, 0).x
            const wanted = Math.min(edge, longRow.children[0].width + longRow.spacing + 360)
            verify(Math.abs(right - wanted) <= 1, "the chooser up to the card's edge: its right " + right + ", wanted " + wanted)
            const readOnly = opened(root.shared)
            verify(!shown(readOnly, "contactBookMain"), "no chooser on a card of a book read only")
            compare(shown(readOnly, "contactBookReadOnly").text, "Shared with you", "its book said")
        }

        // One address book: the card's main part shows none; More, pressed, shows it.
        function test_one_book_the_book_under_more() {
            const page = opened(root.jeanne)
            verify(!shown(page, "contactBookRow"), "nothing in the main part")
            const more = says(page, "▸  ui-more-details")
            verify(more, "More")
            mouseClick(more)
            const box = shown(page, "contactBookMore")
            verify(box, "under More")
            compare(box.currentText, "Contacts")
        }

        // A new card with two books that can be written: its form asks the book, the one
        // Sioul chose first chosen; another chosen goes with the edit, by place. Untouched,
        // Sioul's choice goes. With one book that can be written (another read only), the
        // form asks nothing and the edit names that one. A card that exists asks nothing
        // and names none: its card's chooser moves it.
        function test_a_new_card_s_book() {
            mock.books = [root.contactsBook, root.teamBook]
            mock.proposed = "cloud/team"
            const page = createTemporaryObject(pageComponent, root)
            page.startNew("")
            waitForRendering(page)
            const box = shown(page, "contactFormBook")
            verify(box, "the form asks the book")
            compare(box.currentText, "Team", "Sioul's choice, already chosen")
            within(box)
            choose(box, 0)
            compare(box.currentText, "Contacts")
            shown(page, "contactFormName").text = "Ada Exemple"
            mouseClick(says(page, "ui-save"))
            compare(mock.saved.length, 1)
            compare(mock.saved[0].key, "")
            compare(mock.saved[0].edit.book, "cloud/contacts", "the book chosen, by place")
            // Untouched: Sioul's choice.
            page.startNew("")
            waitForRendering(page)
            compare(shown(page, "contactFormBook").currentText, "Team")
            shown(page, "contactFormName").text = "Ben Exemple"
            mouseClick(says(page, "ui-save"))
            compare(mock.saved[1].edit.book, "cloud/team")
            // One book that can be written, one read only: nothing asked, that one named.
            mock.books = [root.contactsBook, root.sharedBook]
            mock.proposed = "cloud/contacts"
            page.startNew("")
            waitForRendering(page)
            verify(!shown(page, "contactFormBook"), "one book takes it: nothing asked")
            shown(page, "contactFormName").text = "Cleo Exemple"
            mouseClick(says(page, "ui-save"))
            compare(mock.saved[2].edit.book, "cloud/contacts")
            // A card that exists: Edit, no book asked, none named.
            mock.books = [root.contactsBook, root.teamBook]
            page.open(root.jeanne.key)
            waitForRendering(page)
            mouseClick(says(page, "ui-edit"))
            waitForRendering(page)
            verify(page.editing)
            verify(!shown(page, "contactFormBook"), "a card that exists: its card moves it")
            mouseClick(says(page, "ui-save"))
            compare(mock.saved[3].key, root.jeanne.key)
            compare(mock.saved[3].edit.book, undefined, "no book in its edit")
        }

        // The card's scroll view, the one that holds the opening hours.
        function scrollOf(item) {
            for (let p = item; p; p = p.parent) {
                if (p.contentY !== undefined && p.contentHeight !== undefined)
                    return p
            }
            return null
        }

        // On a phone, an office's card opens with its opening hours in sight, whole, however many
        // things it is tied to: what is tied to it scrolls with the card, never takes its room
        // (with twelve ties kept under the card, its scroll view was left 219 points of 891, the
        // opening hours' buttons ending at 458); Close stays in sight.
        function test_an_office_s_hours_in_sight_on_a_phone() {
            let room = 0
            for (const ties of [1, 12]) {
                mock.ties = ties
                const page = createTemporaryObject(pageComponent, root)
                page.open(root.office.key)
                tryVerify(() => root.named(page, "openingHoursCallBack") !== undefined && root.named(page, "openingHoursCallBack").visible, 2000)
                waitForRendering(page)
                const call = root.named(page, "openingHoursCallBack")
                const flick = scrollOf(call)
                verify(flick, "the card scrolls")
                compare(flick.contentY, 0, "opened at its top")
                const bottom = call.mapToItem(flick, 0, call.height).y
                verify(bottom <= flick.height, ties + " ties: Call back… ends at " + bottom + ", the card's scroll view at " + flick.height)
                // The ties in the card's own scroll view, which they leave as tall as with one.
                const tie = root.all(page, c => c.visible && c.text === "Call the office about refund " + ties)[0]
                verify(tie, "the last tie")
                verify(scrollOf(tie) === flick, "the ties scroll with the card")
                if (room === 0)
                    room = flick.height
                compare(flick.height, room, ties + " ties: the card keeps its room")
                const close = root.all(page, c => c.visible && c.text === "ui-close")[0]
                verify(close, "Close")
                verify(close.mapToItem(root, 0, close.height).y <= root.height + 0.5, "Close in sight")
            }
        }
    }
}
