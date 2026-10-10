// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Contacts, names first: the list is names with one line under each, of one
// category when chosen; a contact opens on the right with its categories, its
// addresses and numbers, "Write" and "Call" next to each; the rest (postal
// addresses, birthday, notes, web sites) folded under "More". Editing happens
// in place; deleting waits ten seconds with "Undo". "Duplicates" opens in the
// place of a card. Right click on a name for what is not in view.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string contactsText: ""
    readonly property var shown: page.contactsText ? JSON.parse(page.contactsText) : ({ contacts: [], sentence: "", can_add: false, categories: [], category: "" })

    function takeShown() {
        if (page.visible)
            page.contactsText = page.sioul.contacts
    }

    Connections {
        target: page.sioul

        function onContactsChanged() {
            page.takeShown()
        }
    }
    onVisibleChanged: page.takeShown()
    Component.onCompleted: page.takeShown()
    property string openKey: ""
    property var person: null
    // The contact open, as what new things are tied to.
    readonly property var source: page.person && page.person.uid ? { uri: "sioul:contact/" + encodeURIComponent(page.person.uid), kind: "contact", key: page.person.key || "", title: page.person.name, name: page.person.name, address: page.person.emails.length > 0 ? page.person.emails[0].value : "" } : null
    property bool editing: false
    // The duplicates, in the place of a card: made the first time they are looked at.
    property bool duplicatesShown: false
    property bool duplicatesMade: false
    // On a phone, the contact open (or its form, or the duplicates) takes the
    // page; Back leaves the form as Cancel does, then closes what is open (main.qml).
    readonly property bool canGoBack: page.person !== null || page.editing || page.duplicatesShown
    function back() {
        if (page.duplicatesShown) {
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

    // Into another address book: asked first when it would not keep everything.
    function moveTo(book, confirmed) {
        if (!page.person || book.name === page.person.book)
            return
        const answer = JSON.parse(page.sioul.moveContact(page.openKey, book.id, confirmed))
        if (answer.losses) {
            bookMove.book = book
            bookMove.ask(book.name, page.sioul.textArgs("contact-book-move-ask", JSON.stringify({ place: book.name, list: answer.losses.join(", ") })), page.sioul.text("move-anyway"))
            return
        }
        if (answer.error)
            page.sioul.status = answer.error
        else
            page.open(answer.key)
    }

    function open(key) {
        if (key)
            page.duplicatesShown = false
        page.openKey = key
        page.editing = false
        page.moreShown = false
        page.problem = ""
        page.person = key ? JSON.parse(page.sioul.contact(key) || "null") : null
        page.readMail()
    }

    function readMail() {
        page.mail = page.person ? JSON.parse(page.sioul.personOf(page.openKey) || "null") || page.noMail : page.noMail
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
        enabled: page.visible && (page.person !== null || page.duplicatesShown)
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
            readonly property bool alone: page.person === null && !page.duplicatesShown

            visible: !(page.window.compact && (page.person !== null || page.editing || page.duplicatesShown))
            Layout.fillHeight: true
            Layout.fillWidth: alone
            Layout.minimumWidth: 0
            Layout.preferredWidth: alone ? columns.width : Math.round((columns.width - columns.spacing) * 0.38)
            spacing: 8

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
                    display: page.person === null && !page.duplicatesShown && !page.window.compact ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
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
                    display: page.person === null && !page.duplicatesShown && !page.window.compact ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
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
                    display: page.person === null && !page.duplicatesShown && !page.window.compact ? AbstractButton.TextBesideIcon : AbstractButton.IconOnly
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
                SettingsButton {
                    sioul: page.sioul
                    theme: page.theme
                    view: "contacts"
                }
            }

            // One category only, or all of them: what Nextcloud Contacts shows as groups.
            PlainComboBox {
                readonly property var names: page.shown.categories || []

                visible: names.length > 0 && !page.mapShown
                Layout.fillWidth: true
                Layout.maximumWidth: 320
                model: [page.sioul.text("contacts-category-all")].concat(names.map(n => page.theme.plain(n)))
                currentIndex: page.shown.category ? Math.max(0, names.indexOf(page.shown.category) + 1) : 0
                Accessible.name: page.sioul.text("contact-categories")
                onActivated: index => page.sioul.showContactsCategory(index === 0 ? "" : names[index - 1])
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
                    highlighted: page.openKey === row.modelData.key
                    onClicked: page.open(row.modelData.key)
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

                        Avatar {
                            theme: page.theme
                            source: row.modelData.photo
                            name: row.modelData.name
                            size: 32
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            Label {
                                Layout.fillWidth: true
                                text: row.modelData.name
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: page.theme.text
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
                                Button {
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

                                    Repeater {
                                        model: page.person && page.moreShown ? page.person.addresses.reduce((all, a) => all.concat([page.sioul.text("contact-address") + (a.label ? " · " + page.labelText(a.label) : ""), a.value]), []) : []

                                        delegate: Label {
                                            id: addressCell

                                            required property string modelData
                                            required property int index

                                            Layout.fillWidth: addressCell.index % 2 === 1
                                            Layout.alignment: Qt.AlignTop
                                            text: addressCell.modelData
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: addressCell.index % 2 === 0 ? page.theme.muted : page.theme.text
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
                                        currentIndex: page.person ? Math.max(0, bookChoice.books.findIndex(b => b.name === page.person.book)) : 0
                                        Component.onCompleted: bookChoice.books = JSON.parse(page.sioul.addressBooks() || "[]")
                                        onActivated: index => page.moveTo(bookChoice.books[index], false)
                                    }
                                }
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
    }

    // Right click on a name.
    Later {
        id: rowMenu

        sourceComponent: Component {
            SioulMenu {
                id: rowMenuForm

                property var target: null
                readonly property var source: rowMenuForm.target ? { uri: rowMenuForm.target.uri, kind: "contact", key: rowMenuForm.target.key, title: rowMenuForm.target.name, name: rowMenuForm.target.name, address: rowMenuForm.target.email } : null

                MenuItem {
                    text: page.sioul.text("ui-edit")
                    onTriggered: {
                        page.open(rowMenuForm.target.key)
                        page.formNow().load(page.person)
                        page.editing = true
                    }
                }
                MenuItem {
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

    ConfirmDialog {
        id: bookMove

        property var book: null

        sioul: page.sioul
        theme: page.theme
        onConfirmed: page.moveTo(bookMove.book, true)
    }
}
