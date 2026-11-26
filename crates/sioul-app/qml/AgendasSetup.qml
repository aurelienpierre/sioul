// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Agenda ⚙'s block of the agendas (several agendas), at a glance (26
// November 2026): one table, one row per calendar and task list, every
// property in a column, no fold to open: its colour and name (its account
// under it), whose it is, what it is for, when it reaches you, what you may
// do with it, whether it counts as your busy time, who sees it, when it was
// synced. Each value whole, in a few words (the full words in its menu's
// list, a sentence as its tooltip), each column as wide as its widest value;
// wider than the page, the table scrolls sideways rather than cut one. Its
// colour, whose it is, what for, when and what you may do are changed on the
// row (its dot, its menus), busy time by its switch; the rest, which needs
// more room, in the agenda's card, Details… (AgendaCard.qml, headed
// sections), never a setting the row changes. On a phone, one block per
// agenda, each value under its label. What Sioul saw decides until you say;
// each change is saved at once, into the configuration, and travels with the
// settings (`agendas::set_field`). Nothing is asked of a server before the
// page shows: what this device knows comes at once, and who sees each agenda
// arrives into its row when its server answers, a quiet mark meanwhile
// (`readAgendaSharing`, asked once the page is drawn). "Counts as busy time"
// is the agenda's one switch (`busytime`): its server's own mark where it
// keeps one (Nextcloud's "transparent"). Below the agendas, where your
// stand-ins of each kind go and the follow-up tasks (`children`). Your
// availability is its own block (AvailabilitySetup.qml).

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme
    // The row's sentence, said once above the agendas.
    property string about: ""
    // `agenda::setup`: {agendas: [{id, name, colour, place, events, tasks, whose, person,
    // area, reaches, access, server_read_only, for_work, said}], pairs}.
    property var view: ({ agendas: [], pairs: [], children: { kinds: [], yours: [], follow_ups: true, any: false } })
    property string viewText: ""
    property string problem: ""
    // Each agenda's sharing as its server last answered (`agendaShared`, else the row's
    // `shared_with`, its last answer on this device), by place.
    property var sharing: ({})
    // The agendas whose sharing is being asked of their server now, by place: a quiet mark on their row.
    property var reading: ({})
    // The removal asked of an agenda: its place, and what it would do (`agendaRemoval`).
    property string removing: ""
    property var removal: ({})
    // "New agenda…" open.
    property bool making: false
    // "Subscribe to a calendar…" open (SubscribeForm.qml).
    property bool subscribing: false
    readonly property var colours: ["#4c6b5c", "#3f6f8f", "#7a5c99", "#b07d3f", "#a3504f", "#5f7f3a", "#5b6470", "#c06c8d"]
    // The table at a glance where there is room for it; else one block per agenda (a phone, a narrow window).
    // The table where a computer gives room for it; on a phone, one block per agenda.
    readonly property bool wide: setup.width >= 760
    readonly property int gap: 10
    // A block's labels, on a phone.
    readonly property int labelWidth: 120
    // The table's columns, by their heading's words, each as wide as its widest value
    // (`measure`), the room left shared among them; wider than the page, the table scrolls
    // sideways rather than cut a word (the coordinator's notes of 26 November 2026).
    readonly property var columns: ["agenda-col-name", "agenda-whose", "agenda-for", "agenda-reaches", "agenda-access", "agenda-col-busy", "agenda-sharing", "agenda-col-synced", ""]
    // Each column's widest value now (`measure`), in pixels.
    property var need: []
    readonly property int needed: setup.need.reduce((a, b) => a + b, 0) + setup.gap * (setup.columns.length - 1)
    // The table's own width: its columns', or the page's when it is wider.
    readonly property int tableWidth: Math.max(setup.width, setup.needed)

    // A calendar of busy blocks pressed: Agenda ⚙ ▸ Your availability, where it is kept, shown.
    signal availabilityAsked

    readonly property var areas: ["work", "admin", "leisure"]

    // A value of the table changed in place: its words and a ▾, its choices in a menu; one
    // chosen (`chosen`), or, for several (what it is for), each ticked or not (`toggledOne`).
    component MenuCell: Button {
        id: cell

        property var theme
        property var choices: []
        property var labels: []
        property string current: ""
        property bool several: false
        property var ticked: []
        // The words shown; the chosen one's by default.
        property string shown: cell.labels[Math.max(0, cell.choices.indexOf(cell.current))] || ""

        signal chosen(string value)
        signal toggledOne(string value, bool on)

        // The choice at `index` taken, as its menu item does.
        function choose(index) {
            if (cell.several)
                cell.toggledOne(cell.choices[index], cell.ticked.indexOf(cell.choices[index]) < 0)
            else if (cell.choices[index] !== cell.current)
                cell.chosen(cell.choices[index])
        }

        flat: true
        leftPadding: 8
        rightPadding: 6
        text: cell.shown
        onClicked: cellMenu.popup(cell, 0, cell.height)

        contentItem: RowLayout {
            spacing: 4

            Label {
                Layout.fillWidth: true
                text: cell.text
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: cell.theme ? cell.theme.text : "black"
            }
            Label {
                text: "▾"
                color: cell.theme ? cell.theme.muted : "gray"
            }
        }

        Menu {
            id: cellMenu

            Repeater {
                model: cell.choices.length

                delegate: MenuItem {
                    id: item

                    required property int index

                    checkable: true
                    checked: cell.several ? cell.ticked.indexOf(cell.choices[item.index]) >= 0 : cell.choices[item.index] === cell.current
                    text: cell.theme ? cell.theme.plain(cell.labels[item.index] || "") : ""
                    onTriggered: {
                        cell.choose(item.index)
                        // What the settings say once read again, never the tick's own toggle.
                        item.checked = Qt.binding(() => cell.several ? cell.ticked.indexOf(cell.choices[item.index]) >= 0 : cell.choices[item.index] === cell.current)
                    }
                }
            }
        }
    }

    // A column's width in the table: its widest value, and its share of the room left (but the Details… column's).
    function columnWidth(index) {
        const own = setup.need[index] || 0
        const spare = setup.width - setup.needed
        if (spare <= 0 || index === setup.columns.length - 1)
            return own
        const growing = setup.needed - (setup.need[setup.columns.length - 1] || 0) - setup.gap * (setup.columns.length - 1)
        return own + Math.floor(spare * own / Math.max(1, growing))
    }

    // A text's width in a cell's font: `kind` "bold" (a name, a heading), "small" (under a name), else plain.
    function measure(text, kind) {
        const metrics = kind === "bold" ? boldMetrics : kind === "small" ? smallMetrics : plainMetrics
        metrics.text = text
        return Math.ceil(metrics.advanceWidth)
    }

    // Each column's widest value, its heading's among them: read again with the agendas and their sharing.
    function measureColumns() {
        const rows = setup.view.agendas || []
        const cell = text => setup.measure(text, "") + setup.measure("▾", "") + 22
        const widest = (values, at) => values.reduce((most, v) => Math.max(most, v), at)
        const heading = i => setup.columns[i] === "" ? 0 : setup.measure(setup.sioul.text(setup.columns[i]), "bold")
        const named = rows.map(a => Math.max(setup.measure(a.name, "bold"), setup.measure(a.place || "", "small")))
        setup.need = [
            Math.max(heading(0), Math.min(220, widest(named, 80)) + 24),
            widest(rows.map(a => a.mirror === true ? setup.measure(setup.whoseWords(a), "") : cell(setup.whoseWords(a))), heading(1)),
            widest(rows.filter(a => a.mirror !== true).map(a => cell(setup.areaWords(String(a.area).split("+")))), heading(2)),
            widest(rows.filter(a => a.mirror !== true).map(a => cell(setup.reachesWords(a))), heading(3)),
            widest(rows.filter(a => a.mirror !== true).map(a => cell(setup.accessWords(a))), heading(4)),
            widest(rows.map(a => setup.measure((a.busy_time || {}).short || "", "small")), Math.max(heading(5), switchProbe.implicitWidth)),
            Math.min(260, widest(rows.map(a => setup.measure(setup.sharedWords(a, setup.sharing[a.id] || null), "") + 16), heading(6))),
            Math.min(200, widest(rows.map(a => setup.measure(a.synced || "", "")), heading(7))),
            detailsProbe.implicitWidth
        ]
    }

    // Read again; `kept`, the same answer changes nothing (its rows, their open menus, stay).
    function read(kept) {
        const text = setup.sioul.agendasSetup()
        if (kept === true && text === setup.viewText)
            return
        setup.viewText = text
        try {
            setup.view = JSON.parse(text)
        } catch (e) {
            setup.view = { agendas: [], pairs: [], children: { kinds: [], yours: [], follow_ups: true, any: false } }
        }
        // Who sees each one, as its server last answered on this device, until it answers anew.
        const next = Object.assign({}, setup.sharing)
        let added = false
        for (const agenda of setup.view.agendas || []) {
            if (next[agenda.id] === undefined && agenda.shared_with) {
                next[agenda.id] = agenda.shared_with
                added = true
            }
        }
        if (added)
            setup.sharing = next
        setup.measureColumns()
    }

    // Who sees each agenda on a server, asked once the page is drawn, each answer into its row.
    function askSharing() {
        const reading = Object.assign({}, setup.reading)
        const asked = []
        for (const agenda of setup.view.agendas || []) {
            if (agenda.mirror === true || !agenda.server || agenda.server.server === "local" || agenda.subscription)
                continue
            reading[agenda.id] = true
            asked.push(agenda.id)
        }
        setup.reading = reading
        for (const id of asked)
            setup.sioul.readAgendaSharing(id)
    }

    // One field written ("" back to what Sioul saw); the agendas read again.
    function save(id, field, value) {
        setup.problem = setup.sioul.setAgendaField(id, field, value)
        setup.read()
    }

    // "Counts as busy time" turned: kept where it is kept, and on its server where it keeps the mark.
    function setBusy(id, counts) {
        setup.problem = setup.sioul.setAgendaBusy(id, counts)
        setup.read()
    }

    // One of what it is for ticked or not, the others as they are.
    function toggleArea(agenda, area, on) {
        const now = String(agenda.area).split("+")
        setup.save(agenda.id, "area", setup.areas.filter(a => a === area ? on : now.indexOf(a) >= 0).join("+"))
    }

    // A row's values in a few words, whole in their cell; the full words stay in each menu's list.
    // What it is for: "Admin + Leisure".
    function areaWords(on) {
        return setup.areas.filter(a => on.indexOf(a) >= 0).map(a => setup.sioul.text("agenda-area-short-" + a)).join(" + ")
    }

    // Whose it is: "Yours", "Child · Léa"; a calendar of busy blocks, what it is kept for.
    function whoseWords(agenda) {
        if (agenda.mirror === true)
            return agenda.kept_for ? setup.sioul.textWith("agenda-mirror-kept-for", "name", agenda.kept_for) : setup.sioul.text("agenda-mirror-kept")
        const role = setup.sioul.text("agenda-whose-short-" + agenda.whose)
        return (agenda.person || "") !== "" ? setup.sioul.textArgs("agenda-whose-short-person", JSON.stringify({ role: role, name: agenda.person })) : role
    }

    // When it reaches you: "All day", "Outside work"; one for work alone, "Work hours".
    function reachesWords(agenda) {
        return setup.sioul.text("agenda-reaches-short-" + (String(agenda.area).split("+").indexOf("work") >= 0 ? "work" : agenda.reaches))
    }

    // What you may do: "Read, write", "Read only", "Busy only".
    function accessWords(agenda) {
        return setup.sioul.text("agenda-access-short-" + agenda.access)
    }

    // Who sees it, in its row: the names its server gave, "Only you", a link; this device's own
    // sharing for one here; by its address for a calendar read by it; "" before any answer (its
    // quiet mark shows). The server's sentence is its tooltip, and its card's.
    function sharedWords(agenda, share) {
        if (agenda.subscription)
            return setup.sioul.text("agenda-shared-address")
        if (!agenda.server || agenda.server.server === "local")
            return setup.sioul.text("agenda-shared-here")
        if (share === null)
            return ""
        if ((share.problem || "") !== "" || share.sharer === undefined || share.sharer === "none")
            return setup.sioul.text("agenda-shared-unknown")
        const names = (share.people || []).map(p => p.name).join(", ")
        if (share.link)
            return names === "" ? setup.sioul.text("agenda-shared-link-alone") : setup.sioul.textWith("agenda-shared-link", "who", names)
        return names === "" ? setup.sioul.text("agenda-shared-only-you") : names
    }

    // A colour chosen on its row: a calendar read by its address keeps it in its settings; yours, on its server.
    function recolour(agenda, colour) {
        if (agenda.subscription)
            setup.saveSubscription(agenda.subscription.id, "colour", colour)
        else {
            setup.problem = setup.sioul.recolourAgenda(agenda.id, colour)
            setup.read()
        }
    }

    // Its card, over the page.
    function openCard(id) {
        const card = agendaCard.now()
        card.show(id)
    }

    // The card open now, or null (the window's pictures, the tests).
    function card() {
        return agendaCard.item
    }

    // Sioul's window, with its own properties (its locale), for the person's form.
    function sioulWindow(): var {
        return setup.Window.window
    }

    // A person chosen for an agenda ("person", "parent_person"): the key and the name beside it.
    function setPerson(id, field, person) {
        setup.problem = setup.sioul.setAgendaPerson(id, field, person)
        setup.read()
    }

    // "New person…" from an agenda's chooser: the person's form, its relation the agenda's; chosen once saved.
    function newPerson(agenda, field) {
        const household = JSON.parse(setup.sioul.household() || "{}")
        const dialog = personForm.now()
        dialog.relations = household.relations || []
        dialog.children = (household.persons || []).filter(p => p.relation === "child").map(p => ({ id: p.id, name: p.name }))
        dialog.place = agenda.id
        dialog.field = field
        const relation = field === "parent_person" ? "other-parent" : ({ child: "child", partner: "partner", parent: "other-parent" })[agenda.whose] || ""
        dialog.begin("", "", relation, field === "parent_person" && agenda.person_id ? [agenda.person_id] : [])
    }

    // The removal asked first, in one sentence; done on the second press.
    function askRemoval(id) {
        setup.removal = JSON.parse(setup.sioul.agendaRemoval(id) || "{}")
        setup.removing = id
    }
    function remove(id) {
        setup.problem = setup.sioul.removeAgenda(id)
        setup.removing = ""
        setup.read()
    }

    Connections {
        target: setup.sioul

        // An agenda's role or person, or a person, changed here or on another device:
        // the agendas and their choosers read again.
        function onAgendasChanged() {
            setup.read(true)
        }

        function onPersonsChanged() {
            setup.read(true)
        }

        function onAgendaShared(id, answer) {
            if (id === "availability")
                return
            const next = Object.assign({}, setup.sharing)
            next[id] = JSON.parse(answer || "{}")
            setup.sharing = next
            const reading = Object.assign({}, setup.reading)
            delete reading[id]
            setup.reading = reading
            setup.measureColumns()
        }
    }

    // "New agenda…" opened afresh, its role said (the window's pictures: someone else's, a name typed).
    function askNew(whose, name) {
        setup.subscribing = false
        making.begin("", "")
        making.whose = whose
        making.name = name
        setup.making = true
    }

    // "Subscribe to a calendar…": its form, opened afresh (also from Settings ▸ Hours, through AgendaPage's `askSubscribe`).
    function askSubscribe() {
        setup.making = false
        setup.subscribing = true
        subscribeForm.begin()
    }

    // One field of a subscribed calendar written; the agendas read again.
    function saveSubscription(id, field, value) {
        setup.problem = setup.sioul.setSubscriptionField(id, field, value)
        setup.read()
    }

    // A list of tasks alone (it takes no events): renamed and removed in Tasks ⚙ (TaskListsSetup.qml), not here.
    function tasksOnly(agenda) {
        return agenda.tasks === true && agenda.events !== true
    }

    // What may be chosen of what you may do with it: never writing where its server keeps it read only.
    function accessChoices(agenda) {
        return agenda.server_read_only ? ["read", "availability"] : ["write", "read", "availability"]
    }

    objectName: "agendasSetup"
    spacing: 4
    Component.onCompleted: {
        setup.read()
        asking.start()
    }

    // Who sees each one asked once the page is drawn, never before it shows.
    Timer {
        id: asking

        interval: 60
        onTriggered: setup.askSharing()
    }

    Label {
        visible: setup.about !== ""
        Layout.fillWidth: true
        text: setup.about
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
    Label {
        visible: setup.problem !== ""
        Layout.fillWidth: true
        text: setup.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.warm
    }

    // A new agenda, with its role (part 5): made here, and on its server by the next sync;
    // or a calendar subscribed by its address, read by each device (SubscribeForm.qml).
    Flow {
        visible: !setup.making && !setup.subscribing
        Layout.fillWidth: true
        spacing: 8

        Button {
            objectName: "agendaNew"
            visible: (setup.view.accounts || []).length > 0
            text: setup.sioul.text("agenda-new")
            onClicked: {
                making.begin("", "")
                setup.making = true
            }
        }
        Button {
            objectName: "agendaSubscribe"
            text: setup.sioul.text("subscribe-open")
            onClicked: setup.askSubscribe()
        }
    }
    SubscribeForm {
        id: subscribeForm

        persons: setup.view.persons || ({})
        visible: setup.subscribing
        Layout.fillWidth: true
        // A form keeps a readable width beside the table.
        Layout.maximumWidth: setup.theme.readable
        sioul: setup.sioul
        theme: setup.theme
        onDone: id => {
            setup.subscribing = false
            setup.read()
        }
        onCancelled: setup.subscribing = false
    }
    NewAgendaForm {
        id: making

        visible: setup.making
        Layout.fillWidth: true
        Layout.maximumWidth: setup.theme.readable
        sioul: setup.sioul
        theme: setup.theme
        accounts: setup.view.accounts || []
        persons: setup.view.persons || ({})
        onMade: place => {
            setup.making = false
            setup.read()
        }
        onKept: setup.making = false
    }

    // What the columns are measured with (`measure`): a cell's fonts, a switch, Details….
    Label {
        id: probe

        visible: false
    }
    TextMetrics {
        id: plainMetrics

        font: probe.font
    }
    TextMetrics {
        id: boldMetrics

        font.family: probe.font.family
        font.pixelSize: probe.font.pixelSize
        font.weight: Font.DemiBold
    }
    TextMetrics {
        id: smallMetrics

        font.family: probe.font.family
        font.pixelSize: 13
    }
    Switch {
        id: switchProbe

        visible: false
    }
    Button {
        id: detailsProbe

        visible: false
        flat: true
        text: setup.sioul.text("agenda-details")
    }

    // "Counts as busy time", said once for every row's switch.
    Label {
        objectName: "agendaBusyLegend"
        visible: (setup.view.agendas || []).length > 0
        Layout.fillWidth: true
        Layout.maximumWidth: setup.theme.readable * 1.4
        text: setup.sioul.text("agenda-busy-legend")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }

    // The table, its own width: wider than the page, it scrolls sideways rather than cut a value.
    Flickable {
        id: table

        objectName: "agendaTable"
        Layout.fillWidth: true
        Layout.preferredHeight: rowsColumn.implicitHeight + (sideways.visible ? sideways.height : 0)
        contentWidth: setup.wide ? setup.tableWidth : width
        contentHeight: rowsColumn.implicitHeight
        flickableDirection: Flickable.HorizontalFlick
        interactive: contentWidth > width + 1
        boundsBehavior: Flickable.StopAtBounds
        clip: true

        ScrollBar.horizontal: ScrollBar {
            id: sideways

            objectName: "agendaTableScroll"
            visible: table.contentWidth > table.width + 1
            policy: ScrollBar.AlwaysOn
        }

        ColumnLayout {
            id: rowsColumn

            width: table.contentWidth
            spacing: 4

            // The columns' headings, over the table alone.
            RowLayout {
                objectName: "agendaHeadings"
                visible: setup.wide && (setup.view.agendas || []).length > 0
                Layout.fillWidth: true
                Layout.topMargin: 6
                spacing: setup.gap

                Repeater {
                    model: setup.columns

                    delegate: Label {
                        id: heading

                        required property var modelData
                        required property int index

                        Layout.preferredWidth: setup.columnWidth(heading.index)
                        Layout.maximumWidth: setup.columnWidth(heading.index)
                        text: heading.modelData === "" ? "" : setup.sioul.text(heading.modelData)
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        color: setup.theme.muted
                    }
                }
            }

            Repeater {
                model: setup.view.agendas

                delegate: ColumnLayout {
                    id: one

                    required property var modelData
                    readonly property var on: String(one.modelData.area).split("+")
                    readonly property bool mirror: one.modelData.mirror === true
                    readonly property var busy: one.modelData.busy_time || ({ counts: false, changeable: false, short: "", line: "" })
                    readonly property var share: setup.sharing[one.modelData.id] || null
                    // Its colour may be changed here: yours on its server, or a calendar read by its address.
                    readonly property bool recolourable: !one.mirror && ((!!one.modelData.server && one.modelData.server.own === true) || (!!one.modelData.subscription && one.modelData.subscription.unread !== true))

                    objectName: "agendaSetup"
                    Layout.fillWidth: true
                    spacing: 2

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 1
                        color: setup.theme.line
                    }

                    GridLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: setup.wide ? 2 : 6
                        columns: setup.wide ? setup.columns.length : 2
                        columnSpacing: setup.gap
                        rowSpacing: setup.wide ? 0 : 2

                        // Its colour (changed in place) and name, on two lines at most, its account under it;
                        // on a phone its Details… beside it.
                        RowLayout {
                            Layout.columnSpan: setup.wide ? 1 : 2
                            Layout.preferredWidth: setup.wide ? setup.columnWidth(0) : -1
                            Layout.maximumWidth: setup.wide ? setup.columnWidth(0) : -1
                            Layout.fillWidth: !setup.wide
                            spacing: 6

                            Rectangle {
                                id: dot

                                objectName: "agendaRowColour"
                                Layout.preferredWidth: 16
                                Layout.preferredHeight: 16
                                Layout.alignment: Qt.AlignTop
                                Layout.topMargin: 3
                                radius: 8
                                color: one.modelData.colour || setup.theme.accent
                                border.width: one.recolourable ? 1 : 0
                                border.color: setup.theme.line
                                Accessible.role: one.recolourable ? Accessible.Button : Accessible.Graphic
                                Accessible.name: setup.sioul.text("agenda-colour") + " · " + one.modelData.name

                                TapHandler {
                                    enabled: one.recolourable
                                    onTapped: colourMenu.open()
                                }

                                // Its colour chosen in place, among the agendas' eight.
                                Popup {
                                    id: colourMenu

                                    objectName: "agendaColourMenu"
                                    y: dot.height + 4
                                    padding: 8

                                    contentItem: Row {
                                        spacing: 6

                                        Repeater {
                                            model: setup.colours

                                            delegate: Rectangle {
                                                id: choice

                                                required property string modelData

                                                objectName: "agendaColour"
                                                width: 26
                                                height: 26
                                                radius: 13
                                                color: choice.modelData
                                                border.width: (one.modelData.colour || "").toLowerCase().startsWith(choice.modelData) ? 3 : 0
                                                border.color: setup.theme.text
                                                Accessible.role: Accessible.Button
                                                Accessible.name: setup.sioul.text("agenda-colour") + " " + choice.modelData

                                                TapHandler {
                                                    onTapped: {
                                                        colourMenu.close()
                                                        setup.recolour(one.modelData, choice.modelData)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                spacing: 0

                                Label {
                                    objectName: "agendaLineLabel"
                                    Layout.fillWidth: true
                                    text: one.modelData.name + (one.modelData.kept_hidden === true ? " · " + setup.sioul.text("agendas-kept-hidden") : "")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    maximumLineCount: 2
                                    elide: Text.ElideRight
                                    font.weight: Font.DemiBold
                                    color: setup.theme.text

                                    TapHandler {
                                        onTapped: setup.openCard(one.modelData.id)
                                    }
                                }
                                Label {
                                    objectName: "agendaAccount"
                                    Layout.fillWidth: true
                                    text: one.modelData.place
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: 13
                                    color: setup.theme.muted
                                }
                            }
                            Button {
                                objectName: "agendaOpenNarrow"
                                visible: !setup.wide
                                text: setup.sioul.text("agenda-details")
                                Accessible.name: setup.sioul.textWith("agenda-details-of", "name", one.modelData.name)
                                onClicked: setup.openCard(one.modelData.id)
                            }
                        }

                        // Whose it is: the role in place; whose name in its card. A mirror: what it is kept for.
                        Label {
                            visible: !setup.wide
                            Layout.preferredWidth: setup.labelWidth
                            text: setup.sioul.text("agenda-whose")
                            wrapMode: Text.Wrap
                            color: setup.theme.muted
                        }
                        Item {
                            Layout.preferredWidth: setup.wide ? setup.columnWidth(1) : -1
                            Layout.maximumWidth: setup.wide ? setup.columnWidth(1) : -1
                            Layout.fillWidth: !setup.wide
                            implicitHeight: one.mirror ? keptFor.implicitHeight : whoseChoice.implicitHeight

                            Label {
                                id: keptFor

                                objectName: "agendaKeptFor"
                                visible: one.mirror
                                width: parent.width
                                text: setup.whoseWords(one.modelData)
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: setup.theme.muted
                            }
                            MenuCell {
                                id: whoseChoice

                                objectName: "agendaRowWhose"
                                visible: !one.mirror
                                width: parent.width
                                theme: setup.theme
                                choices: ["yours", "child", "parent", "partner", "someone", "shared"]
                                labels: choices.map(w => setup.sioul.text("agenda-whose-" + w))
                                current: one.modelData.whose
                                // Whose name, beside the role: chosen in its card.
                                shown: setup.whoseWords(one.modelData)
                                Accessible.name: setup.sioul.text("agenda-whose") + " · " + one.modelData.name
                                onChosen: value => setup.save(one.modelData.id, "whose", value)
                            }
                        }

                        // What it is for: work, your admin, leisure, ticked in a menu.
                        Label {
                            visible: !setup.wide && !one.mirror
                            Layout.preferredWidth: setup.labelWidth
                            text: setup.sioul.text("agenda-for")
                            wrapMode: Text.Wrap
                            color: setup.theme.muted
                        }
                        MenuCell {
                            objectName: "agendaRowFor"
                            visible: setup.wide || !one.mirror
                            opacity: one.mirror ? 0 : 1
                            enabled: !one.mirror
                            Layout.preferredWidth: setup.wide ? setup.columnWidth(2) : -1
                            Layout.maximumWidth: setup.wide ? setup.columnWidth(2) : -1
                            Layout.fillWidth: !setup.wide
                            theme: setup.theme
                            several: true
                            choices: setup.areas
                            labels: setup.areas.map(a => setup.sioul.text("area-" + a))
                            ticked: one.on
                            shown: setup.areaWords(one.on)
                            Accessible.name: setup.sioul.text("agenda-for") + " · " + one.modelData.name
                            onToggledOne: (value, on) => setup.toggleArea(one.modelData, value, on)
                        }

                        // When it reaches you: a personal one all day or outside work hours; one for work in its hours.
                        Label {
                            visible: !setup.wide && !one.mirror
                            Layout.preferredWidth: setup.labelWidth
                            text: setup.sioul.text("agenda-reaches")
                            wrapMode: Text.Wrap
                            color: setup.theme.muted
                        }
                        Item {
                            visible: setup.wide || !one.mirror
                            Layout.preferredWidth: setup.wide ? setup.columnWidth(3) : -1
                            Layout.maximumWidth: setup.wide ? setup.columnWidth(3) : -1
                            Layout.fillWidth: !setup.wide
                            implicitHeight: reachesChoice.implicitHeight

                            MenuCell {
                                id: reachesChoice

                                objectName: "agendaRowReaches"
                                visible: !one.mirror && one.on.indexOf("work") < 0
                                width: parent.width
                                theme: setup.theme
                                choices: ["all-day", "outside-work"]
                                labels: choices.map(r => setup.sioul.text("agenda-reaches-" + r))
                                current: one.modelData.reaches
                                shown: setup.reachesWords(one.modelData)
                                Accessible.name: setup.sioul.text("agenda-reaches") + " · " + one.modelData.name
                                onChosen: value => setup.save(one.modelData.id, "reaches", value)
                            }
                            Label {
                                visible: !one.mirror && one.on.indexOf("work") >= 0
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width
                                leftPadding: 8
                                text: setup.reachesWords(one.modelData)
                                textFormat: Text.PlainText
                                color: setup.theme.text
                            }
                        }

                        // What you may do with it: never writing where its server keeps it read only.
                        Label {
                            visible: !setup.wide && !one.mirror
                            Layout.preferredWidth: setup.labelWidth
                            text: setup.sioul.text("agenda-access")
                            wrapMode: Text.Wrap
                            color: setup.theme.muted
                        }
                        MenuCell {
                            objectName: "agendaRowAccess"
                            visible: setup.wide || !one.mirror
                            opacity: one.mirror ? 0 : 1
                            enabled: !one.mirror
                            Layout.preferredWidth: setup.wide ? setup.columnWidth(4) : -1
                            Layout.maximumWidth: setup.wide ? setup.columnWidth(4) : -1
                            Layout.fillWidth: !setup.wide
                            theme: setup.theme
                            choices: setup.accessChoices(one.modelData)
                            labels: choices.map(a => setup.sioul.text("agenda-access-" + a))
                            current: one.modelData.access
                            shown: setup.accessWords(one.modelData)
                            Accessible.name: setup.sioul.text("agenda-access") + " · " + one.modelData.name
                            onChosen: value => setup.save(one.modelData.id, "access", value)
                        }

                        // Counts as busy time: its one switch, and in a few words why it cannot be turned (another
                        // person's, holidays, a list of tasks, Sioul's copy), that its server has not got it yet,
                        // or that its server counts what Sioul does not; the sentence on a phone and as its tooltip.
                        Label {
                            visible: !setup.wide
                            Layout.preferredWidth: setup.labelWidth
                            text: setup.sioul.text("agenda-col-busy")
                            wrapMode: Text.Wrap
                            color: setup.theme.muted
                        }
                        ColumnLayout {
                            Layout.preferredWidth: setup.wide ? setup.columnWidth(5) : -1
                            Layout.maximumWidth: setup.wide ? setup.columnWidth(5) : -1
                            Layout.fillWidth: !setup.wide
                            spacing: 0

                            Switch {
                                id: busySwitch

                                objectName: "agendaBusy"
                                visible: one.busy.changeable === true
                                checked: one.busy.counts === true
                                Accessible.name: setup.sioul.textWith("agenda-busy-of", "name", one.modelData.name)
                                Accessible.description: one.busy.line || ""
                                onToggled: {
                                    setup.setBusy(one.modelData.id, checked)
                                    busySwitch.checked = Qt.binding(() => one.busy.counts === true)
                                }
                            }
                            Label {
                                objectName: "agendaBusyLine"
                                visible: text !== ""
                                Layout.fillWidth: true
                                text: setup.wide ? (one.busy.short || "") : (one.busy.line || "")
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: setup.theme.muted
                                Accessible.description: one.busy.line || ""

                                ToolTip.visible: busyHover.hovered && setup.wide
                                ToolTip.text: setup.theme.plain(one.busy.line || "")

                                HoverHandler {
                                    id: busyHover
                                }
                            }
                        }

                        // Who sees it, as its server last said, in names; a quiet mark while it is being asked.
                        Label {
                            visible: !setup.wide && !one.mirror
                            Layout.preferredWidth: setup.labelWidth
                            text: setup.sioul.text("agenda-sharing")
                            wrapMode: Text.Wrap
                            color: setup.theme.muted
                        }
                        RowLayout {
                            visible: setup.wide || !one.mirror
                            Layout.preferredWidth: setup.wide ? setup.columnWidth(6) : -1
                            Layout.maximumWidth: setup.wide ? setup.columnWidth(6) : -1
                            Layout.fillWidth: !setup.wide
                            spacing: 4

                            Label {
                                objectName: "agendaRowShared"
                                visible: !one.mirror
                                Layout.fillWidth: true
                                Layout.preferredWidth: 0
                                text: setup.sharedWords(one.modelData, one.share)
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: one.share !== null && (one.share.problem || "") !== "" ? setup.theme.warm : setup.theme.text

                                ToolTip.visible: sharedHover.hovered && one.share !== null
                                ToolTip.text: one.share === null ? "" : setup.theme.plain(one.share.problem || one.share.line || "")

                                HoverHandler {
                                    id: sharedHover
                                }
                            }
                            // Being asked of its server: a quiet mark, gone when it answers.
                            Label {
                                objectName: "agendaRowReading"
                                visible: setup.reading[one.modelData.id] === true
                                text: "…"
                                color: setup.theme.muted
                                Accessible.name: setup.sioul.text("agenda-sharing-reading")
                            }
                        }

                        // When it was synced, in one form for every agenda; warm when its last sync went wrong.
                        Label {
                            visible: !setup.wide && !one.mirror
                            Layout.preferredWidth: setup.labelWidth
                            text: setup.sioul.text("agenda-col-synced")
                            wrapMode: Text.Wrap
                            color: setup.theme.muted
                        }
                        Label {
                            objectName: "agendaRowSynced"
                            visible: setup.wide || !one.mirror
                            Layout.preferredWidth: setup.wide ? setup.columnWidth(7) : -1
                            Layout.maximumWidth: setup.wide ? setup.columnWidth(7) : -1
                            Layout.fillWidth: !setup.wide
                            text: one.modelData.synced || ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: one.modelData.synced_warn === true ? setup.theme.warm : setup.theme.muted
                            Accessible.description: one.modelData.synced_full || ""

                            ToolTip.visible: syncedHover.hovered && (one.modelData.synced_full || "") !== text
                            ToolTip.text: setup.theme.plain(one.modelData.synced_full || "")

                            HoverHandler {
                                id: syncedHover
                            }
                        }

                        // Its card: what needs more room.
                        Button {
                            objectName: "agendaOpen"
                            visible: setup.wide
                            Layout.preferredWidth: setup.columnWidth(8)
                            Layout.maximumWidth: setup.columnWidth(8)
                            flat: true
                            text: setup.sioul.text("agenda-details")
                            Accessible.name: setup.sioul.textWith("agenda-details-of", "name", one.modelData.name)
                            onClicked: setup.openCard(one.modelData.id)
                        }
                    }
                }
            }
        }
    }
    // Children's events (part 3): where your stand-in of each kind goes, the follow-up tasks.
    ColumnLayout {
        visible: setup.view.children.any === true
        Layout.fillWidth: true
        Layout.maximumWidth: setup.theme.readable
        Layout.topMargin: 8
        spacing: 4

        Label {
            text: setup.sioul.text("children-events")
            font.bold: true
            color: setup.theme.text
        }
        Label {
            text: setup.sioul.text("children-stand-ins")
            color: setup.theme.text
        }
        Repeater {
            model: setup.view.children.kinds

            delegate: RowLayout {
                id: kind

                required property var modelData
                readonly property var choices: [""].concat(setup.view.children.yours.map(a => a.id))

                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.preferredWidth: 110
                    text: kind.modelData.label
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: setup.theme.text
                }
                PlainComboBox {
                    objectName: "childrenStandIn"
                    Layout.fillWidth: true
                    model: [setup.sioul.text("children-stand-in-usual")].concat(setup.view.children.yours.map(a => setup.theme.plain(a.name)))
                    currentIndex: Math.max(0, kind.choices.indexOf(kind.modelData.stand_in))
                    Accessible.name: setup.sioul.text("children-stand-ins") + " · " + kind.modelData.label
                    onActivated: index => {
                        setup.problem = setup.sioul.setStandInAgenda(kind.modelData.id, kind.choices[index])
                        setup.read()
                    }
                }
                SpinBox {
                    visible: kind.modelData.usual_months > 0 || kind.modelData.months > 0
                    from: 0
                    to: 60
                    value: kind.modelData.months
                    Accessible.name: setup.sioul.text("children-months") + " · " + kind.modelData.label
                    onValueModified: {
                        setup.problem = setup.sioul.setRhythm(kind.modelData.id, value === kind.modelData.usual_months ? -1 : value)
                        setup.read()
                    }
                }
            }
        }
        CheckBox {
            objectName: "childrenFollowUps"
            Layout.fillWidth: true
            text: setup.sioul.text("children-follow-ups")
            checked: setup.view.children.follow_ups === true
            onToggled: {
                setup.problem = setup.sioul.setFollowUps(checked)
                setup.read()
            }
        }
    }

    // An agenda's card (AgendaCard.qml), made the first time one is opened.
    Later {
        id: agendaCard

        sourceComponent: Component {
            AgendaCard {
                setup: setup
            }
        }
    }

    // A person's form, from an agenda's "New person…": chosen for that agenda once saved.
    Later {
        id: personForm

        sourceComponent: Component {
            PersonDialog {
                id: agendaPersonForm

                property string place: ""
                property string field: ""

                sioul: setup.sioul
                theme: setup.theme
                window: setup.sioulWindow()
                onSaved: id => {
                    if (id !== "" && agendaPersonForm.place !== "")
                        setup.setPerson(agendaPersonForm.place, agendaPersonForm.field, id)
                }
            }
        }
    }
}
