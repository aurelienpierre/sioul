// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts, names first: the list is names with one line under each, of one
// category when chosen; a contact opens on the right with its categories, its
// addresses and numbers, "Write" and "Call" next to each; the rest (postal
// addresses, birthday, notes, web sites) folded under "More". Editing happens
// in place; deleting waits ten seconds with "Undo". "Duplicates" opens in the
// place of a card. Right click on a name for what is not in view. At the
// head, the household (« Proches », docs/persons.md): a person's page opens
// in the place of a card too. A contact you bill shows its client profile
// (what its invoices print, ClientProfileDialog.qml) and "Bill this client".
// Above the list, the filters combined (ContactFilters.qml: books, group, has
// or lacks, how many show); "Select" gives each row a box, and the selection
// bar acts on many at once (ContactSelection.qml); "Import…" reads a vCard or
// a CSV file (ContactImport.qml, in the window). A card of an address book
// you only read says so wherever it would be written (wave D).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts
import com.aurelienpierre.sioul

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string contactsText: ""
    readonly property var shown: page.contactsText ? JSON.parse(page.contactsText) : ({ contacts: [], sentence: "", can_add: false, categories: [], category: "", filter: {}, books: [], counted: "", shown: 0, total: 0 })

    function takeShown() {
        if (page.visible) {
            page.contactsText = page.sioul.contacts
            // The search as this device kept it, once, while it is not being typed.
            if (!page.searchTaken && page.shown.filter && search.text === "" && !search.activeFocus) {
                search.text = page.shown.filter.name || ""
                page.searchTaken = true
            }
        }
    }
    property bool searchTaken: false

    // "Select": each row a box; the keys ticked, by file.
    property bool selecting: false
    property var selected: ({})
    readonly property var selectedKeys: Object.keys(page.selected)

    function select(key, on) {
        const next = Object.assign({}, page.selected)
        if (on)
            next[key] = true
        else
            delete next[key]
        page.selected = next
    }

    // Contacts ⚙ opened (or closed), scrolled to a setting when one is named (the pictures' steps).
    function openSettings(open, key) {
        contactsSettings.show(open)
        if (open && key)
            Qt.callLater(() => contactsSettings.scrollTo(key))
    }

    // One of Contacts ⚙'s parts, by its objectName (the pictures' steps), or null.
    function settingsPart(name) {
        return contactsSettings.part(name, true)
    }

    // Every contact the filters show, ticked.
    function selectShown() {
        const next = {}
        for (const c of page.shown.contacts)
            next[c.key] = true
        page.selected = next
    }

    // The selection follows what is shown: another filter or search leaves
    // ticked only the cards still in sight, so "With them…" acts on what the
    // count says and the rows show (review F9).
    onShownChanged: {
        const ticked = Object.keys(page.selected)
        if (ticked.length === 0)
            return
        const inSight = {}
        for (const c of page.shown.contacts)
            inSight[c.key] = true
        const kept = ticked.filter(key => inSight[key] === true)
        if (kept.length === ticked.length)
            return
        const next = {}
        for (const key of kept)
            next[key] = true
        page.selected = next
    }

    // The address books a selection can move to: those that can be written.
    readonly property var writableBooks: (page.shown.books || []).filter(b => !b.read_only)

    Connections {
        target: page.sioul

        function onContactsChanged() {
            page.takeShown()
        }
    }
    onVisibleChanged: {
        page.takeShown()
        // The household as it is now: persons come from your other devices too.
        if (page.visible)
            household.reload()
    }
    Component.onCompleted: page.takeShown()
    property string openKey: ""
    property var person: null
    // The open contact as a client: its profile ({exists, summary, …}), read with it.
    property var clientProfile: null
    readonly property string clientRef: page.person && page.person.uid ? "sioul:contact/" + encodeURIComponent(page.person.uid) : ""
    // The contact open, as what new things are tied to.
    readonly property var source: page.person && page.person.uid ? { uri: "sioul:contact/" + encodeURIComponent(page.person.uid), kind: "contact", key: page.person.key || "", title: page.person.name, name: page.person.name, address: page.person.emails.length > 0 ? page.person.emails[0].value : "" } : null
    property bool editing: false
    // The duplicates, in the place of a card: made the first time they are looked at.
    property bool duplicatesShown: false
    property bool duplicatesMade: false
    // A person of the household whose page is open, by id; "" none.
    property string personId: ""
    property bool personMade: false
    // On a phone, the contact open (or its form, or the duplicates, or a
    // person's page) takes the page; Back leaves the form as Cancel does,
    // then closes what is open (main.qml).
    readonly property bool canGoBack: page.person !== null || page.editing || page.duplicatesShown || page.personId !== ""
    function back() {
        if (page.personId !== "") {
            page.openPerson("")
        } else if (page.duplicatesShown) {
            page.duplicatesShown = false
        } else if (page.editing) {
            page.editing = false
            if (!page.openKey)
                page.person = null
        } else {
            page.open("")
        }
    }
    property bool moreShown: false
    property string problem: ""
    // Their list, who they are to you and why (`personOf`): {who, from, choice
    // ("" when their categories decide), said, own [an address or a number of
    // theirs with a list of its own], choices [{value, label}]}.
    readonly property var noMail: ({ who: "neutral", choice: "", said: "", own: [], choices: [] })
    property var mail: page.noMail

    // Into another address book: asked first when it would not keep everything,
    // or when it changes who may reach you (a book whose groups are held back).
    function moveTo(book, confirmed) {
        if (!page.person || book.id === page.person.place)
            return
        const answer = JSON.parse(page.sioul.moveContact(page.openKey, book.id, confirmed))
        if ((answer.losses || []).length > 0 || (answer.reach || []).length > 0) {
            bookMove.book = book
            const losses = (answer.losses || []).length > 0 ? [page.sioul.textArgs("contact-book-move-ask", JSON.stringify({ place: book.name, list: answer.losses.join(", ") }))] : []
            bookMove.ask(book.name, (answer.reach || []).concat(losses).join(" "), page.sioul.text("move-anyway"))
            return
        }
        if (answer.error)
            page.sioul.status = answer.error
        else
            page.open(answer.key)
    }

    // A person's page, in the place of a card ("" closes it).
    function openPerson(id) {
        if (id !== "") {
            page.open("")
            page.duplicatesShown = false
            page.personMade = true
        }
        page.personId = id
        if (id !== "" && personLoader.item)
            (personLoader.item as PersonPage).reload()
    }

    // The person's form: new (a name and a card perhaps), or theirs.
    function personFormNow() {
        const dialog = personForm.now()
        const household = JSON.parse(page.sioul.household() || "{}")
        dialog.relations = household.relations || []
        dialog.children = (household.persons || []).filter(p => p.relation === "child").map(p => ({ id: p.id, name: p.name }))
        return dialog
    }

    function open(key) {
        if (key) {
            page.duplicatesShown = false
            page.personId = ""
        }
        page.openKey = key
        page.editing = false
        page.moreShown = false
        page.problem = ""
        page.person = key ? JSON.parse(page.sioul.contact(key) || "null") : null
        page.clientProfile = page.clientRef !== "" ? JSON.parse(page.sioul.clientProfile(page.clientRef) || "null") : null
        page.readMail()
        page.cardPerson = page.person && page.person.uid ? JSON.parse(page.sioul.personOfCard(page.person.uid) || "null") : null
    }
    // The person of the household the open card is, {id, name}, or null.
    property var cardPerson: null

    function readMail() {
        page.mail = page.person ? JSON.parse(page.sioul.personOf(page.openKey) || "null") || page.noMail : page.noMail
    }

    // The server a photo's address names, as its button says it: "example.org".
    function photoHost(url) {
        const found = /^https:\/\/([^\/?#@:]+)/i.exec(url || "")
        return found ? found[1] : ""
    }

    // What an address or a number is for ("work", "cell"), in your language; a
    // kind the card writes and Sioul does not know ("other", "car") as written.
    function labelText(label) {
        const kind = label.split(",")[0].trim()
        const said = page.sioul.text("label-" + kind)
        return said === "label-" + kind ? kind : said
    }

    // The card and the form are made when first needed: a list of names alone
    // on a phone, most of the time.
    property bool cardMade: false
    property bool formMade: false
    onPersonChanged: {
        if (page.person !== null)
            page.cardMade = true
    }

    // The contact form, made now if it was not.
    function formNow() {
        page.formMade = true
        return formLoader.item.form
    }

    // A card's text inside rich text: shown as written, never read as markup.
    function escaped(text) {
        return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;")
    }

    // For the window's tests: the open contact gets another number, and is saved.
    function addNumber(number) {
        page.formNow().load(page.person)
        page.editing = true
        const edited = page.formNow().edit()
        edited.phones.push({ label: "home", value: number })
        const answer = JSON.parse(page.sioul.saveContact(page.openKey, JSON.stringify(edited)))
        page.open(answer.key || page.openKey)
    }

    // The card's details unfolded and scrolled to (`revealed`), for the window's images:
    // on a phone the card's scroll is short, under what it keeps in sight.
    property int revealed: 0
    function revealDetails() {
        page.moreShown = true
        page.revealed += 1
    }

    // The first contact, for the window's images.
    function openFirst() {
        if (page.shown.contacts.length > 0)
            page.open(page.shown.contacts[0].key)
    }

    // The duplicates in the place of a card, the card closed.
    function showDuplicates() {
        page.open("")
        page.duplicatesMade = true
        page.duplicatesShown = true
    }

    // The duplicates' view, made now if it was not (the window's tests).
    function duplicatesNow() {
        page.duplicatesMade = true
        return duplicatesLoader.item
    }

    // A new card; `number` filled in when given (a call's line on the Porch, on a computer).
    function startNew(number) {
        page.duplicatesShown = false
        page.openKey = ""
        page.person = { name: "", emails: [{ label: "", value: "" }], phones: [{ label: "", value: number || "" }], org: "", title: "", addresses: [], birthday: "", notes: "", urls: [], categories: [], book: "", read_only: false }
        page.problem = ""
        page.editing = true
        page.formNow().load(page.person)
    }

    // The map instead of the list: every contact placed, a pin opening its card.
    property bool mapShown: false
    property var places: ({ allowed: false, tiles: "", pins: [], waiting: 0, locating: false })
    // The open contact's first address, when placed.
    readonly property var personPin: {
        if (page.person === null || !page.person.addresses)
            return null
        for (const a of page.person.addresses) {
            const at = page.sioul.placeOf(a.value)
            if (at !== "") {
                const p = JSON.parse(at)
                return { key: page.openKey, name: page.person.name, address: a.value, lat: p.lat, lon: p.lon }
            }
        }
        return null
    }

    function reloadPlaces() {
        page.places = JSON.parse(page.sioul.mapView() || "{}")
    }

    onMapShownChanged: if (page.mapShown) page.reloadPlaces()

    // The list changed (a sync, a save): the open contact is read again.
    Connections {
        target: page.sioul

        function onContactsChanged() {
            if (page.openKey && !page.editing) {
                page.person = JSON.parse(page.sioul.contact(page.openKey) || "null")
                page.readMail()
            }
        }

        function onPlacesChanged() {
            page.reloadPlaces()
            if (page.openKey && !page.editing)
                page.person = JSON.parse(page.sioul.contact(page.openKey) || "null")
        }
    }

    Shortcut {
        sequence: "Escape"
        // A card open over the page takes Escape first (main.qml).
        enabled: page.visible && !page.window.cardRef && (page.person !== null || page.duplicatesShown)
        onActivated: {
            page.editing = false
            page.duplicatesShown = false
            page.open("")
        }
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        ColumnLayout {
            // Alone, the list takes the page; beside a card or the duplicates, its left part.
            readonly property bool alone: page.person === null && !page.duplicatesShown && page.personId === ""

            visible: !(page.window.compact && (page.person !== null || page.editing || page.duplicatesShown || page.personId !== ""))
            Layout.fillHeight: true
            Layout.fillWidth: alone
            Layout.minimumWidth: 0
            Layout.preferredWidth: alone ? columns.width : Math.round((columns.width - columns.spacing) * 0.38)
            spacing: 8

            // The household, at the head: you, the others, a person to add.
            Household {
                id: household

                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                chosen: page.personId
                onOpenPerson: id => page.openPerson(id)
                onAddPerson: page.personFormNow().begin("", "", "")
                onNotAPerson: id => {
                    if (page.personId === id)
                        page.openPerson("")
                }
                onRenamePerson: id => {
                    const person = JSON.parse(page.sioul.personPage(id) || "null")
                    if (person)
                        page.personFormNow().edit(person)
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                TextField {
                    id: search

                    Layout.fillWidth: true
                    placeholderText: page.sioul.text("ui-search")
                    onTextEdited: searching.restart()

                    Timer {
                        id: searching

                        interval: 250
                        onTriggered: page.sioul.searchContacts(search.text)
                    }
                }
                Button {
                    visible: page.shown.can_add
                    // As wide as what it shows: the style's buttons are 100 pixels at least.
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: page.sioul.text("ui-new-contact")
                    icon.name: "contact-new"
                    icon.color: page.theme.text
                    // Narrow beside an open card, or on a phone: the icon alone, its name on hover.
                    display: page.person === null && !page.duplicatesShown && page.personId === "" && !page.window.compact ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
                    ToolTip.visible: hovered && display === AbstractButton.IconOnly
                    ToolTip.text: page.theme.plain(text)
                    Accessible.name: text
                    onClicked: page.startNew()
                }
                // The list, or the map.
                Button {
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: page.mapShown ? page.sioul.text("map-list") : page.sioul.text("map-show")
                    icon.name: page.mapShown ? "view-list-text" : "mark-location"
                    icon.color: page.theme.text
                    display: page.person === null && !page.duplicatesShown && page.personId === "" && !page.window.compact ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
                    Accessible.name: text
                    ToolTip.visible: hovered && display === AbstractButton.IconOnly
                    ToolTip.text: page.theme.plain(text)
                    onClicked: page.mapShown = !page.mapShown
                }
                // Duplicates, in the place of a card.
                Button {
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: page.sioul.text("dup-open")
                    icon.name: "edit-copy"
                    icon.color: page.theme.text
                    highlighted: page.duplicatesShown
                    display: page.person === null && !page.duplicatesShown && page.personId === "" && !page.window.compact ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
                    Accessible.name: text
                    ToolTip.visible: hovered && display === AbstractButton.IconOnly
                    ToolTip.text: page.theme.plain(text)
                    onClicked: {
                        if (page.duplicatesShown)
                            page.duplicatesShown = false
                        else
                            page.showDuplicates()
                    }
                }
                // Many at once: each row a box (ContactSelection.qml).
                Button {
                    objectName: "contactsSelect"
                    visible: !page.mapShown && page.shown.contacts.length > 0
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: page.sioul.text("contacts-select")
                    icon.name: "edit-select-all"
                    icon.color: page.theme.text
                    highlighted: page.selecting
                    display: page.person === null && !page.duplicatesShown && page.personId === "" && !page.window.compact ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
                    Accessible.name: text
                    ToolTip.visible: hovered && display === AbstractButton.IconOnly
                    ToolTip.text: page.theme.plain(text)
                    onClicked: {
                        page.selecting = !page.selecting
                        if (!page.selecting)
                            page.selected = {}
                    }
                }
                // A vCard or a CSV file read, its contacts imported (ContactImport.qml).
                Button {
                    objectName: "contactsImport"
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    text: page.sioul.text("books-import")
                    icon.name: "document-import"
                    icon.color: page.theme.text
                    display: AbstractButton.IconOnly
                    Accessible.name: text
                    ToolTip.visible: hovered
                    ToolTip.text: page.theme.plain(text)
                    onClicked: importFile.open()
                }
                SettingsButton {
                    id: contactsSettings

                    sioul: page.sioul
                    theme: page.theme
                    view: "contacts"
                }
            }

            // The filters, combined: books, a group or none, has or lacks, how many show.
            ContactFilters {
                visible: !page.mapShown && page.shown.total > 0
                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                view: page.shown
                onAsked: filter => page.sioul.setContactsFilter(JSON.stringify(Object.assign({}, filter, { name: search.text })))
            }

            // The selection: how many, all shown, what to do with them.
            ContactSelection {
                visible: page.selecting && !page.mapShown
                Layout.fillWidth: true
                sioul: page.sioul
                theme: page.theme
                keys: page.selectedKeys
                shownCount: page.shown.contacts.length
                books: page.writableBooks
                groups: page.shown.categories || []
                onSelectShown: page.selectShown()
                onSelectNone: page.selected = {}
                onEnded: page.selected = {}
                onFinished: {
                    page.selecting = false
                    page.selected = {}
                }
            }

            Label {
                visible: page.shown.sentence !== ""
                Layout.fillWidth: true
                text: page.shown.sentence
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            // The map: asked first whether addresses may be placed.
            ColumnLayout {
                visible: page.mapShown
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 8

                Label {
                    visible: !page.places.allowed
                    Layout.fillWidth: true
                    text: page.sioul.text("map-allow")
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }
                RowLayout {
                    visible: !page.places.allowed || page.places.waiting > 0 || page.places.locating
                    Layout.fillWidth: true

                    Label {
                        visible: page.places.allowed
                        Layout.fillWidth: true
                        text: page.places.locating ? page.sioul.text("map-locating") : page.sioul.textArgs("map-waiting", JSON.stringify({ n: page.places.waiting }))
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: page.theme.muted
                    }
                    Button {
                        visible: !page.places.locating
                        text: page.places.allowed ? page.sioul.text("map-place") : page.sioul.text("map-allow-button")
                        icon.name: "mark-location"
                        icon.color: page.theme.text
                        onClicked: page.sioul.locateAddresses(true)
                    }
                }
                Label {
                    visible: page.places.allowed && page.places.pins.length === 0 && !page.places.locating && page.places.waiting === 0
                    Layout.fillWidth: true
                    text: page.sioul.text("map-none")
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }
                Loader {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    active: page.mapShown && page.places.pins.length > 0

                    sourceComponent: ContactMap {
                        theme: page.theme
                        tiles: page.places.tiles
                        pins: page.places.pins
                        chosen: page.openKey
                        onPinClicked: key => page.open(key)
                    }
                }
            }

            ListView {
                id: list

                visible: !page.mapShown
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                spacing: 2
                model: page.shown.contacts
                ScrollBar.vertical: ScrollBar {}
                // Rows given again to the contacts scrolled in, rather than made anew: a
                // phone made and shaped each one at every pass.
                reuseItems: true
                cacheBuffer: 600

                delegate: ItemDelegate {
                    id: row

                    required property var modelData

                    width: list.width - 12
                    padding: 10
                    highlighted: page.openKey === row.modelData.key || (page.selecting && page.selected[row.modelData.key] === true)
                    onClicked: {
                        if (page.selecting)
                            page.select(row.modelData.key, page.selected[row.modelData.key] !== true)
                        else
                            page.open(row.modelData.key)
                    }
                    Keys.onReturnPressed: page.open(row.modelData.key)

                    background: Rectangle {
                        color: row.highlighted || row.hovered ? page.theme.surface : "transparent"
                        radius: page.theme.radius
                        border.color: row.visualFocus ? page.theme.focus : row.highlighted ? page.theme.line : "transparent"
                    }

                    // On a touch screen, the menu comes at a long press; letting go then opens nothing.
                    onPressAndHold: {
                        row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
                        const menu = rowMenu.now()
                        menu.target = row.modelData
                        menu.popup()
                    }
                    TapHandler {
                        acceptedButtons: Qt.RightButton
                        // A touch has no buttons: on a touch screen, the row's long press.
                        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                        onTapped: {
                            const menu = rowMenu.now()
                            menu.target = row.modelData
                            menu.popup()
                        }
                    }

                    contentItem: RowLayout {
                        spacing: 10

                        // Selecting: its box.
                        CheckBox {
                            objectName: "contactRowBox"
                            visible: page.selecting
                            checked: page.selected[row.modelData.key] === true
                            Accessible.name: page.theme.plain(row.modelData.name)
                            onToggled: page.select(row.modelData.key, checked)
                        }
                        Avatar {
                            theme: page.theme
                            source: row.modelData.photo
                            name: row.modelData.name
                            size: 32
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 5

                                // Its title, greyed, before the name (its field, `N`'s prefix).
                                Label {
                                    objectName: "contactRowTitle"
                                    visible: (row.modelData.title || "") !== ""
                                    text: row.modelData.title || ""
                                    textFormat: Text.PlainText
                                    color: page.theme.muted
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: row.modelData.name
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    color: page.theme.text
                                }
                            }
                            Label {
                                visible: row.modelData.detail !== ""
                                Layout.fillWidth: true
                                text: row.modelData.detail
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.muted
                                font.pixelSize: 13
                            }
                        }
                    }
                }
            }
        }

        // The contact, read: made with the first one opened.
        Loader {
            id: cardLoader

            active: page.cardMade
            visible: page.person !== null && !page.editing
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.62)

            sourceComponent: Component {
                Panel {
                    id: cardPanel

                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 10

                        ScrollView {
                            id: cardScroll

                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            contentWidth: availableWidth

                            // Its end in sight once the details are unfolded for a picture (`revealDetails`).
                            Connections {
                                target: page
                                function onRevealedChanged() {
                                    Qt.callLater(() => {
                                        const flick = cardScroll.contentItem as Flickable
                                        if (flick)
                                            flick.contentY = Math.max(0, flick.contentHeight - flick.height)
                                    })
                                }
                            }

                            ColumnLayout {
                                width: cardScroll.availableWidth
                                spacing: 10

                                // Their picture, their name, what they do.
                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 14

                                    Avatar {
                                        Layout.alignment: Qt.AlignTop
                                        theme: page.theme
                                        source: page.person ? page.person.photo : ""
                                        name: page.person ? page.person.name : ""
                                        size: 72
                                    }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 4

                                        Label {
                                            Layout.fillWidth: true
                                            text: page.person ? page.person.name : ""
                                            textFormat: Text.PlainText
                                            font.pixelSize: 22
                                            wrapMode: Text.Wrap
                                            color: page.theme.text
                                        }
                                        Label {
                                            visible: page.person !== null && (page.person.org !== "" || page.person.title !== "")
                                            Layout.fillWidth: true
                                            text: page.person ? [page.person.title, page.person.org].filter(t => t).join(" · ") : ""
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: page.theme.muted
                                        }
                                        // Its categories, quiet; one shows the list of that category.
                                        Flow {
                                            visible: page.person !== null && (page.person.categories || []).length > 0
                                            Layout.fillWidth: true
                                            spacing: 6

                                            Repeater {
                                                model: page.person ? (page.person.categories || []) : []

                                                delegate: Rectangle {
                                                    id: chip

                                                    required property string modelData

                                                    implicitWidth: chipText.implicitWidth + 14
                                                    implicitHeight: chipText.implicitHeight + 4
                                                    radius: height / 2
                                                    color: "transparent"
                                                    border.color: page.theme.line

                                                    Label {
                                                        id: chipText

                                                        anchors.centerIn: parent
                                                        text: chip.modelData
                                                        textFormat: Text.PlainText
                                                        font.pixelSize: 12
                                                        color: page.theme.muted
                                                    }
                                                    TapHandler {
                                                        onTapped: {
                                                            page.sioul.showContactsCategory(chip.modelData)
                                                            // On a phone the list is out of sight behind the card.
                                                            if (page.window.compact)
                                                                page.open("")
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // A photo the card names on another server: loaded only when you ask,
                                // then kept here (decided 3 November 2026); nothing remote loads by itself.
                                Button {
                                    objectName: "contactPhotoLoad"
                                    visible: page.person !== null && (page.person.photo_web || "") !== ""
                                    flat: true
                                    icon.name: "image-x-generic"
                                    icon.color: page.theme.text
                                    text: page.person && page.person.photo_web ? page.theme.plain(page.sioul.textWith("contact-photo-load", "host", page.photoHost(page.person.photo_web))) : ""
                                    ToolTip.visible: hovered
                                    ToolTip.text: page.sioul.text("contact-photo-load-help")
                                    ToolTip.delay: 400
                                    onClicked: page.sioul.loadContactPhoto(page.person.photo_web)
                                }

                                // Addresses, each with "Write".
                                Repeater {
                                    model: page.person ? page.person.emails : []

                                    delegate: RowLayout {
                                        id: email

                                        required property var modelData

                                        Layout.fillWidth: true
                                        spacing: 8

                                        Icon {
                                            iconName: "mail-message"
                                            size: 16
                                        }
                                        Label {
                                            Layout.fillWidth: true
                                            text: email.modelData.value + (email.modelData.label ? "  ·  " + page.labelText(email.modelData.label) : "")
                                            textFormat: Text.PlainText
                                            elide: Text.ElideRight
                                            color: page.theme.text
                                        }
                                        Button {
                                            // The name without the commas and semicolons that would cut the
                                            // recipient in two ("Doe, Jane" → "Doe" and "Jane <…>").
                                            readonly property string name: page.person ? page.person.name.replace(/[,;<>"]+/g, " ").replace(/\s+/g, " ").trim() : ""

                                            text: page.sioul.text("ui-write")
                                            onClicked: page.window.writeTo(name !== "" ? name + " <" + email.modelData.value + ">" : email.modelData.value)
                                        }
                                    }
                                }
                                // Numbers, each with "Call" (the desktop's telephone application).
                                Repeater {
                                    model: page.person ? page.person.phones : []

                                    delegate: RowLayout {
                                        id: phone

                                        required property var modelData

                                        Layout.fillWidth: true
                                        spacing: 8

                                        Icon {
                                            iconName: "call-start"
                                            size: 16
                                        }
                                        Label {
                                            Layout.fillWidth: true
                                            text: phone.modelData.value + (phone.modelData.label ? "  ·  " + page.labelText(phone.modelData.label) : "")
                                            textFormat: Text.PlainText
                                            elide: Text.ElideRight
                                            color: page.theme.text
                                        }
                                        Button {
                                            text: page.sioul.text("ui-call")
                                            onClicked: Qt.openUrlExternally("tel:" + phone.modelData.value.replace(/[^+0-9]/g, ""))
                                        }
                                    }
                                }

                                // How they reach you: their list and why, in a sentence; their
                                // sheet (PersonSheet.qml) chooses their list, Always through, and
                                // says what each channel does with them at each time. Any card,
                                // with an address or only a number.
                                RowLayout {
                                    visible: page.person !== null
                                    Layout.fillWidth: true
                                    spacing: 8

                                    // Names that are bundled (tools/bundle-icons.py): Windows and macOS have no theme.
                                    Icon {
                                        iconName: page.mail.who === "safe" ? "security-high" : page.mail.who === "blocked" ? "dialog-cancel" : "view-calendar-day"
                                        size: 16
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: page.mail.said
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.text
                                    }
                                }
                                // The person of your household this card is, or one to make of it.
                                Button {
                                    objectName: "cardPersonButton"
                                    visible: page.person !== null && (page.person.uid || "") !== ""
                                    flat: true
                                    icon.name: page.cardPerson ? "go-next" : "list-add"
                                    icon.color: page.theme.text
                                    text: page.cardPerson ? page.theme.plain(page.sioul.textWith("person-of-card", "name", page.cardPerson.name)) : page.sioul.text("person-card-link")
                                    onClicked: {
                                        if (page.cardPerson) {
                                            page.openPerson(page.cardPerson.id)
                                            return
                                        }
                                        cardLink.target = page.person
                                        cardLink.choices = (JSON.parse(page.sioul.household() || "{}").persons || []).filter(p => p.id !== "me")
                                        cardLink.popup()
                                    }
                                }
                                Button {
                                    visible: page.person !== null
                                    Layout.leftMargin: 18
                                    flat: true
                                    objectName: "personSheetButton"
                                    text: page.theme.plain(page.sioul.textWith("attention-sheet-title", "name", page.person ? (page.person.name || page.sioul.text("attention-sheet-them")) : ""))
                                    icon.name: "go-next"
                                    icon.color: page.theme.text
                                    onClicked: page.window.openPersonSheet(page.openKey, "")
                                }

                                // Where they live, when the address is placed.
                                Loader {
                                    visible: page.personPin !== null
                                    Layout.fillWidth: true
                                    Layout.preferredHeight: 300
                                    active: page.personPin !== null

                                    sourceComponent: ContactMap {
                                        theme: page.theme
                                        tiles: page.places.tiles !== "" ? page.places.tiles : "https://tile.openstreetmap.org/"
                                        pins: page.personPin ? [page.personPin] : []
                                    }
                                }

                                // The rest, folded.
                                BareButton {
                                    readonly property bool any: page.person !== null && (page.person.addresses.length > 0 || page.person.birthday !== "" || page.person.notes !== "" || page.person.urls.length > 0)

                                    visible: any
                                    flat: true
                                    text: (page.moreShown ? "▾  " : "▸  ") + page.sioul.text("ui-more-details")
                                    onClicked: page.moreShown = !page.moreShown
                                }
                                GridLayout {
                                    visible: page.moreShown && page.person !== null
                                    Layout.fillWidth: true
                                    Layout.leftMargin: 12
                                    columns: 2
                                    columnSpacing: 12
                                    rowSpacing: 6

                                    // Each postal address, its label, then the address with Directions (AddressLine.qml).
                                    Repeater {
                                        model: page.person && page.moreShown ? page.person.addresses.reduce((all, a) => all.concat([{ text: page.sioul.text("contact-address") + (a.label ? " · " + page.labelText(a.label) : ""), address: "" }, { text: a.value, address: a.value }]), []) : []

                                        delegate: AddressLine {
                                            id: addressCell

                                            required property var modelData

                                            Layout.fillWidth: addressCell.address !== ""
                                            Layout.alignment: Qt.AlignTop
                                            theme: page.theme
                                            sioul: page.sioul
                                            text: addressCell.modelData.text
                                            address: addressCell.modelData.address
                                        }
                                    }
                                    Label {
                                        visible: page.person !== null && page.person.birthday !== ""
                                        text: page.sioul.text("contact-birthday")
                                        color: page.theme.muted
                                    }
                                    Label {
                                        visible: page.person !== null && page.person.birthday !== ""
                                        Layout.fillWidth: true
                                        text: page.person ? page.person.birthday : ""
                                        textFormat: Text.PlainText
                                        color: page.theme.text
                                    }
                                    Label {
                                        visible: page.person !== null && page.person.notes !== ""
                                        Layout.alignment: Qt.AlignTop
                                        text: page.sioul.text("contact-notes")
                                        color: page.theme.muted
                                    }
                                    Label {
                                        visible: page.person !== null && page.person.notes !== ""
                                        Layout.fillWidth: true
                                        text: page.person ? page.person.notes : ""
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.text
                                    }
                                    Repeater {
                                        model: page.person && page.moreShown ? page.person.urls.reduce((all, u) => all.concat([page.sioul.text("contact-web"), u]), []) : []

                                        // A web address of the card (which comes from a server, or from
                                        // anyone's vCard): a link only when it is a web page, its text
                                        // escaped, so that it cannot bring markup or remote pictures in.
                                        delegate: Label {
                                            id: urlCell

                                            required property string modelData
                                            required property int index
                                            readonly property bool link: urlCell.index % 2 === 1 && /^https?:\/\//i.test(urlCell.modelData)

                                            Layout.fillWidth: urlCell.index % 2 === 1
                                            // rich on purpose: a link, its address escaped; any other line is plain text.
                                            text: urlCell.link ? '<a style="color:' + page.theme.accent + '" href="' + page.escaped(urlCell.modelData) + '">' + page.escaped(urlCell.modelData) + '</a>' : urlCell.modelData
                                            textFormat: urlCell.link ? Text.RichText : Text.PlainText
                                            elide: Text.ElideRight
                                            color: urlCell.index % 2 === 0 ? page.theme.muted : page.theme.text
                                            onLinkActivated: link => {
                                                if (/^https?:\/\//i.test(link))
                                                    Qt.openUrlExternally(link)
                                            }
                                        }
                                    }
                                    Label {
                                        text: page.sioul.text("contact-book")
                                        color: page.theme.muted
                                    }
                                    // Another address book moves the contact there, asked first
                                    // when it would not keep everything (Google's keeps less).
                                    PlainComboBox {
                                        id: bookChoice

                                        property var books: []

                                        Layout.fillWidth: true
                                        enabled: page.person !== null && !page.person.read_only
                                        model: bookChoice.books.map(b => page.theme.plain(b.name))
                                        // The card's own book, by its place ("account/id"), never by a name two books may share.
                                        currentIndex: page.person ? Math.max(0, bookChoice.books.findIndex(b => b.id === page.person.place)) : 0
                                        Component.onCompleted: bookChoice.books = JSON.parse(page.sioul.addressBooks() || "[]")
                                        onActivated: index => page.moveTo(bookChoice.books[index], false)

                                        // Read again for each card opened: a book made or removed since shows.
                                        Connections {
                                            target: page

                                            function onPersonChanged() {
                                                bookChoice.books = JSON.parse(page.sioul.addressBooks() || "[]")
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // As a client: its profile, what its invoices print; a draft invoice for it.
                        Label {
                            objectName: "contactClientProfile"
                            visible: page.clientProfile !== null && page.clientProfile.exists
                            Layout.fillWidth: true
                            text: page.clientProfile && page.clientProfile.exists ? page.sioul.textWith("clients-profile-line", "profile", page.clientProfile.summary) : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                        Flow {
                            visible: page.clientRef !== ""
                            Layout.fillWidth: true
                            spacing: 6

                            Button {
                                objectName: "contactBill"
                                text: page.sioul.text("clients-bill")
                                icon.name: "document-new"
                                icon.color: page.theme.text
                                onClicked: {
                                    const made = JSON.parse(page.sioul.newInvoiceDraft(JSON.stringify({ client: page.clientRef })) || "{}")
                                    if (made.error)
                                        page.problem = made.error
                                    else
                                        page.window.openInvoiceDraft(made.draft)
                                }
                            }
                            Button {
                                objectName: "contactProfile"
                                flat: true
                                text: page.sioul.text(page.clientProfile && page.clientProfile.exists ? "clients-profile-change" : "clients-profile-make")
                                onClicked: profileForm.now().edit(page.clientRef)
                            }
                        }
                        // What they are part of: tasks, notes, events.
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
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: page.theme.gap

                            Button {
                                visible: page.person !== null && !page.person.read_only
                                text: page.sioul.text("ui-edit")
                                onClicked: {
                                    page.formNow().load(page.person)
                                    page.editing = true
                                }
                            }
                            Item {
                                Layout.fillWidth: true
                            }
                            Button {
                                text: page.sioul.text("ui-close")
                                onClicked: page.open("")
                            }
                        }
                    }
                }
            }
        }

        // The contact, edited in place: made the first time one is.
        Loader {
            id: formLoader

            active: page.formMade
            visible: page.editing
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.62)

            sourceComponent: Component {
                Panel {
                    id: formPanel

                    // The form, for the page (`formNow`).
                    property alias form: form

                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 10

                        ScrollView {
                            id: formScroll

                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            contentWidth: availableWidth

                            ContactForm {
                                id: form

                                width: formScroll.availableWidth
                                sioul: page.sioul
                                theme: page.theme
                                known: page.shown.categories || []
                                // The card's book (a new one's: the first that can be written), when its groups are held back.
                                heldBook: {
                                    const place = page.person ? page.person.place : (page.writableBooks.length > 0 ? page.writableBooks[0].place : "")
                                    const book = (page.shown.books || []).find(b => b.place === place)
                                    return book && book.groups === false ? book.name : ""
                                }
                                onAskedSwitch: page.openSettings(true, "addressbooks")
                            }
                        }
                        Label {
                            visible: page.problem !== ""
                            Layout.fillWidth: true
                            text: page.problem
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.warm
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: page.theme.gap

                            Item {
                                Layout.fillWidth: true
                            }
                            Button {
                                text: page.sioul.text("ui-cancel")
                                onClicked: {
                                    page.editing = false
                                    if (!page.openKey)
                                        page.person = null
                                }
                            }
                            Button {
                                text: page.sioul.text("ui-save")
                                highlighted: true
                                onClicked: {
                                    const answer = JSON.parse(page.sioul.saveContact(page.openKey, JSON.stringify(form.edit())))
                                    if (answer.error) {
                                        page.problem = answer.error
                                        return
                                    }
                                    page.open(answer.key)
                                }
                            }
                        }
                    }
                }
            }
        }

        // The duplicates, in the place of a card: made the first time they are looked at.
        Loader {
            id: duplicatesLoader

            active: page.duplicatesMade
            visible: page.duplicatesShown
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.62)

            sourceComponent: Component {
                ContactDuplicates {
                    sioul: page.sioul
                    theme: page.theme
                    compact: duplicatesLoader.width < 560
                    onOpenContact: key => page.open(key)
                    onCloseAsked: page.duplicatesShown = false
                }
            }
        }

        // A person's page, in the place of a card: made the first time one is opened.
        Loader {
            id: personLoader

            active: page.personMade
            visible: page.personId !== ""
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.62)

            sourceComponent: Component {
                PersonPage {
                    theme: page.theme
                    sioul: page.sioul
                    window: page.window
                    personId: page.personId
                    onCloseAsked: page.openPerson("")
                    onEditAsked: person => page.personFormNow().edit(person)
                    onOpenContact: key => page.open(key)
                    onChanged: household.reload()
                    onKeepsAsked: person => {
                        keepsFolder.person = person
                        keepsFolder.open()
                    }
                    onEndAsked: person => endForm.now().ask(person)
                }
            }
        }
    }

    // A contact's client profile, made or changed.
    Later {
        id: profileForm

        sourceComponent: Component {
            ClientProfileDialog {
                sioul: page.sioul
                theme: page.theme
                onSaved: page.clientProfile = page.clientRef !== "" ? JSON.parse(page.sioul.clientProfile(page.clientRef) || "null") : null
            }
        }
    }

    // Right click on a name.
    Later {
        id: rowMenu

        sourceComponent: Component {
            SioulMenu {
                id: rowMenuForm

                property var target: null
                readonly property var source: rowMenuForm.target ? { uri: rowMenuForm.target.uri, kind: "contact", key: rowMenuForm.target.key, title: rowMenuForm.target.name, name: rowMenuForm.target.name, address: rowMenuForm.target.email } : null

                // A card of a book you only read: said, and neither Edit nor Delete offered.
                MenuItem {
                    objectName: "contactRowReadOnly"
                    visible: rowMenuForm.target !== null && rowMenuForm.target.read_only === true
                    height: visible ? implicitHeight : 0
                    enabled: false
                    text: page.sioul.text("contact-read-only")
                }
                MenuItem {
                    objectName: "contactRowEdit"
                    enabled: rowMenuForm.target !== null && rowMenuForm.target.read_only !== true
                    text: page.sioul.text("ui-edit")
                    onTriggered: {
                        page.open(rowMenuForm.target.key)
                        page.formNow().load(page.person)
                        page.editing = true
                    }
                }
                MenuItem {
                    objectName: "contactRowDelete"
                    enabled: rowMenuForm.target !== null && rowMenuForm.target.read_only !== true
                    text: page.sioul.text("ui-delete")
                    onTriggered: {
                        if (page.openKey === rowMenuForm.target.key)
                            page.open("")
                        page.sioul.deleteContact(rowMenuForm.target.key)
                    }
                }
                MenuSeparator {}
                AddMenu {
                    sioul: page.sioul
                    window: page.window
                    source: rowMenuForm.source
                }
                MenuItem {
                    text: page.sioul.text("ui-link-existing")
                    onTriggered: page.window.linkFrom(rowMenuForm.source)
                }
            }
        }
    }

    // A person's form: new from Household or a card, or theirs.
    Later {
        id: personForm

        sourceComponent: Component {
            PersonDialog {
                sioul: page.sioul
                theme: page.theme
                window: page.window
                onSaved: id => {
                    household.reload()
                    if (page.person && page.person.uid)
                        page.cardPerson = JSON.parse(page.sioul.personOfCard(page.person.uid) || "null")
                    if (id !== "" && page.person === null)
                        page.openPerson(id)
                    else if (personLoader.item)
                        (personLoader.item as PersonPage).reload()
                }
            }
        }
    }

    // "What Sioul keeps about…": where to save it; its papers' files copied there, its page written as a PDF.
    PdfWriter {
        id: pdf
    }
    FolderDialog {
        id: keepsFolder

        property var person: null

        onAccepted: {
            // On Windows the address is file:///C:/…, on Android a folder of the phone's storage (Theme.localPath).
            const folder = page.theme.localPath(keepsFolder.selectedFolder)
            if (folder === "") {
                page.sioul.status = page.sioul.text("folder-not-on-device")
                return
            }
            const made = JSON.parse(page.sioul.personKeeps(keepsFolder.person.id, folder) || "{}")
            if (made.error) {
                page.sioul.status = made.error
                return
            }
            const failed = pdf.write(made.html, made.pdf)
            page.sioul.status = failed !== "" ? page.sioul.textWith("invoice-not-written", "path", failed) : page.sioul.textWith("person-keeps-saved", "path", made.folder)
        }
    }

    // "Ends…": the day it ended; for a partner, their own papers saved to hand back, then taken out if you say so.
    Later {
        id: endForm

        sourceComponent: Component {
            SafeDialog {
                id: ending

                property var person: null
                property string folder: ""
                property string problem: ""

                function ask(person) {
                    ending.person = person
                    ending.folder = ""
                    ending.problem = ""
                    endDay.date = ""
                    takeOut.checked = false
                    ending.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(460, ending.roomWidth - 2 * page.theme.gap)
                title: ending.person ? page.theme.plain(page.sioul.textWith("person-ends-title", "name", ending.person.name)) : ""

                contentItem: ColumnLayout {
                    spacing: 8

                    Label {
                        text: page.sioul.text("person-ends-on")
                        color: page.theme.muted
                    }
                    DateField {
                        id: endDay

                        theme: page.theme
                        sioul: page.sioul
                        locale: page.window.sioulLocale
                        pickLabel: page.sioul.text("event-pick-day")
                    }
                    // A partner's own papers go back with them; those of two stay, the household's past.
                    Label {
                        visible: ending.person !== null && ending.person.partner === true
                        Layout.fillWidth: true
                        text: page.sioul.text("person-ends-hand-back")
                        wrapMode: Text.Wrap
                        color: page.theme.text
                    }
                    RowLayout {
                        visible: ending.person !== null && ending.person.partner === true
                        Layout.fillWidth: true
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: ending.folder === "" ? "—" : ending.folder
                            textFormat: Text.PlainText
                            elide: Text.ElideMiddle
                            color: page.theme.text
                        }
                        Button {
                            text: page.sioul.text("papers-choose")
                            onClicked: handBack.open()
                        }
                    }
                    CheckBox {
                        id: takeOut

                        visible: ending.person !== null && ending.person.partner === true
                        enabled: ending.folder !== ""
                        Layout.fillWidth: true
                        text: page.sioul.text("person-ends-take-out")
                    }
                    Label {
                        visible: ending.problem !== ""
                        Layout.fillWidth: true
                        text: ending.problem
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: page.theme.warm
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 8

                        Item {
                            Layout.fillWidth: true
                        }
                        Button {
                            text: page.sioul.text("ui-cancel")
                            onClicked: ending.close()
                        }
                        Button {
                            objectName: "personEndSave"
                            highlighted: true
                            text: page.sioul.text("person-ends")
                            onClicked: {
                                const answer = JSON.parse(page.sioul.endPerson(ending.person.id, JSON.stringify({ until: endDay.date, folder: ending.folder, take_out: takeOut.checked })) || "{}")
                                if (answer.error) {
                                    ending.problem = answer.error
                                    return
                                }
                                if (answer.saved > 0)
                                    page.sioul.status = page.sioul.textArgs("person-ends-saved", JSON.stringify({ n: answer.saved, path: ending.folder }))
                                ending.close()
                                household.reload()
                                if (personLoader.item)
                                    (personLoader.item as PersonPage).reload()
                            }
                        }
                    }
                }

                FolderDialog {
                    id: handBack

                    onAccepted: ending.folder = page.theme.localPath(handBack.selectedFolder)
                }
            }
        }
    }

    // "This is a person in your household…": one of them, or a new person, with this card.
    SioulMenu {
        id: cardLink

        property var target: null
        property var choices: []

        Instantiator {
            model: cardLink.choices

            delegate: MenuItem {
                required property var modelData

                text: page.theme.plain(modelData.name)
                onTriggered: {
                    page.problem = page.sioul.linkPersonCard(modelData.id, cardLink.target.uid, true)
                    page.cardPerson = JSON.parse(page.sioul.personOfCard(cardLink.target.uid) || "null")
                    household.reload()
                }
            }
            onObjectAdded: (index, object) => cardLink.insertItem(index, object)
            onObjectRemoved: (index, object) => cardLink.removeItem(object)
        }
        MenuSeparator {
            visible: cardLink.choices.length > 0
        }
        MenuItem {
            text: page.sioul.text("person-card-new")
            onTriggered: page.personFormNow().begin(cardLink.target.name, cardLink.target.uid, "")
        }
    }

    // "Import…": a vCard or a CSV file, read on this device (contactimport.rs).
    FileDialog {
        id: importFile

        nameFilters: [page.sioul.text("import-file-kinds") + " (*.vcf *.vcard *.csv)"]
        // Its address as chosen: Rust reads a file's (file:///C:/… on Windows too), and says
        // how a phone hands one over when the picker gives a content: address.
        onAccepted: page.sioul.importContacts(importFile.selectedFile.toString())
    }

    ConfirmDialog {
        id: bookMove

        property var book: null

        sioul: page.sioul
        theme: page.theme
        onConfirmed: page.moveTo(bookMove.book, true)
    }
}
