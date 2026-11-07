// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Porch (docs/porch.md): what came, as notices, in sections every one
// alike (PorchSection.qml), each notice a row (NoticeRow.qml): codes first,
// whatever the time; then, inside an admin window, each project, what you
// sent yourself, the people you know, what your phone brought, the sites'
// notifications, the week's money, the paper letters, then what is filed,
// someone new, less important accounts, your filter's catches, what is set
// aside; outside the window, only when it opens. Dismiss and Mark read act
// at once, with ten seconds to undo. A message opens on the right, in the Reader.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    // Read while shown: a page out of sight keeps what it showed, and reads
    // again when it comes back; results landing meanwhile cost nothing.
    property string porchText: ""
    readonly property var view: page.porchText ? JSON.parse(page.porchText) : ({ open: false, closed: null, right_now: [], summary: "", status: "", lanes: [] })

    function takeShown() {
        if (page.visible) {
            page.porchText = page.sioul.porch
            page.readNotices()
        }
    }

    Connections {
        target: page.sioul

        function onPorchChanged() {
            page.takeShown()
        }
        function onLettersChanged() {
            page.readNotices()
        }
    }
    // The Porch's sections that are not mail, as notices (`notices::view`):
    // {sites, money, letters}, each a list of rows; read while shown.
    property var notices: ({ sites: [], money: [], letters: [] })
    function readNotices() {
        if (page.visible)
            page.notices = JSON.parse(page.sioul.notices("view", "{}") || "{}")
    }
    // The notices just dismissed or read here, by id: gone from the rows at
    // once, before the Porch is read again; back with Undo.
    property var gone: ({})
    // The last word said here, for its ten seconds: {verb, row, at, said}.
    property var lastSaid: null

    // The rows still new (not dismissed, read or done on any device), and
    // the message open in the Reader, which stays while it is read.
    function newOnes(rows) {
        return (rows || []).filter(row => {
            const notice = row.notice || {}
            if (row.key && page.sameMessage(row.key, page.openKey))
                return true
            if (page.gone[notice.id] === true)
                return false
            return !notice.state || notice.state.mark === "new"
        })
    }

    // A word said of a row's notices (`notices::call`): "dismiss", "read",
    // "done"; gone from the Porch at once, ten seconds to undo.
    function say(verb, row) {
        const asked = { kind: row.kind || (row.notice ? row.notice.kind : ""), ids: row.ids || [row.notice.id], targets: row.targets || [] }
        const answer = JSON.parse(page.sioul.notices(verb, JSON.stringify(asked)) || "{}")
        if (!answer.at) {
            if (answer.said)
                page.sioul.status = answer.said
            return
        }
        const next = Object.assign({}, page.gone)
        for (const id of asked.ids)
            next[id] = true
        page.gone = next
        page.lastSaid = { asked: asked, mark: answer.mark, at: answer.at, said: answer.said }
        undoTimer.restart()
    }

    // The agenda's conflicts read again each minute: a time's area comes into view, or leaves it.
    property int agendaStamp: 0

    // An event of a conflict opened at that time of it (a series' Thursday, not its next).
    function openEvent(e) {
        if (e && e.key)
            page.window.openThing({ kind: "event", key: e.key, start: e.start || 0, uri: "" })
    }

    // A conflict answered (`pim::answer_conflict`): "keep" says it no more, both events kept;
    // "move" opens your own event to move it. Either settles its notice on every device (Done),
    // gone at once; what went wrong is said.
    function answerConflict(row, answer) {
        const conflict = row.conflict || {}
        let problem = ""
        let open = ""
        if (conflict.kind === "during") {
            // A long event's warning: the event whole, and the pairs it holds, which a Sioul before reads.
            problem = page.sioul.setOverlapAside([conflict.key].concat(conflict.keys || []).join("\n"))
        } else {
            const answered = JSON.parse(page.sioul.answerConflict(conflict.key, answer) || "{}")
            problem = answered.problem || ""
            open = answered.open || ""
        }
        if (problem !== "") {
            page.sioul.status = problem
            return
        }
        const asked = { kind: "agenda", ids: row.ids || [row.notice.id], targets: [] }
        page.sioul.notices("done", JSON.stringify(asked))
        const next = Object.assign({}, page.gone)
        for (const id of asked.ids)
            next[id] = true
        page.gone = next
        if (open !== "") {
            const events = [conflict.first, conflict.second].filter(e => e && e.key === open)
            page.window.openThing({ kind: "event", key: open, start: events.length > 0 ? events[0].start || 0 : 0, uri: "" })
        }
    }

    function undo() {
        const last = page.lastSaid
        if (last === null)
            return
        page.sioul.notices("undo", JSON.stringify(Object.assign({ mark: last.mark, at: last.at }, last.asked)))
        const next = Object.assign({}, page.gone)
        for (const id of last.asked.ids)
            delete next[id]
        page.gone = next
        page.lastSaid = null
        undoTimer.stop()
    }

    Timer {
        id: undoTimer

        interval: 10000
        onTriggered: page.lastSaid = null
    }

    // Rows on two lines, their quiet actions in their menu: a phone, or the
    // list narrowed beside a message open in the Reader.
    readonly property bool narrowRows: page.window.compact || list.availableWidth < 640

    // Sections after the phone's, the sites', the money's and the letters'.
    function isLate(key) {
        return ["filed", "screener", "low", "review", "set-aside", "hostile"].indexOf(key) >= 0
    }
    // Notes or papers gone many at once here and held, not said yet
    // (`share::withheld`): read when the Porch shows and at each minute.
    property var withheld: []
    property string withheldProblem: ""
    // "Put them back" pressed, its answer not come yet (off the window's thread: thousands take a while), and for which card.
    property string puttingBack: ""

    function readWithheld() {
        page.withheld = JSON.parse(page.sioul.shareWithheld() || "[]").filter(w => !page.sioul.viewFlag(w.key))
    }
    // Said: not again on the Porch for that bulk of files (a later one is said anew).
    function withheldSaid(key) {
        page.sioul.setViewFlag(key, true)
        page.readWithheld()
    }
    Connections {
        target: page.sioul

        function onShareWithheldBack(problem) {
            page.withheldProblem = problem
            if (problem === "" && page.puttingBack !== "")
                page.withheldSaid(page.puttingBack)
            page.puttingBack = ""
        }
    }
    // The persons made from older texts on this device (docs/persons.md, "Made from
    // older texts"), in an admin window: said once, nothing to answer; once shown a
    // few seconds, not shown again when the Porch is left.
    property var personsMade: []
    property bool personsMadeRead: false

    function readPersonsMade() {
        if (!page.personsMadeRead)
            page.personsMade = JSON.parse(page.sioul.personsMade() || "[]")
    }
    Timer {
        interval: 3000
        running: page.visible && page.personsMade.length > 0 && !page.personsMadeRead
        onTriggered: {
            page.personsMadeRead = true
            page.sioul.personsMadeSaid()
        }
    }
    // Shown a few seconds while the Porch is in view: said once, put away when the Porch is left.
    property bool withheldRead: false
    Timer {
        interval: 3000
        running: page.visible && page.withheld.length > 0
        onTriggered: page.withheldRead = true
    }
    // The doses (docs/health.md, "On the Porch"), made off the window's thread
    // (`missedView`, `health::missed`): today's from their time on, not marked
    // yet, reminded or not; and those due while Sioul was closed, asked about.
    // Whatever the hours: doses are not mail.
    readonly property var doses: JSON.parse(page.sioul.missedView || "{}")
    readonly property var dueDoses: page.doses.due || []
    readonly property var missedDoses: page.doses.closed || []

    function reloadDoses() {
        page.sioul.refreshHealth()
    }

    // Read again when the Porch shows, when its mail is sorted again, and at
    // each minute's turn of the window's clock: a dose comes due, passes its
    // half hour, or is marked on another computer (through the sharing).
    onVisibleChanged: {
        page.takeShown()
        if (page.visible) {
            page.reloadDoses()
            page.readWithheld()
            page.readPersonsMade()
        } else if (page.personsMadeRead) {
            page.personsMade = []
        }
        if (!page.visible && page.withheldRead) {
            for (const w of page.withheld)
                page.sioul.setViewFlag(w.key, true)
            page.withheldRead = false
            page.withheld = []
            page.withheldProblem = ""
        }
    }
    // A card's message looked for once the view is parsed: on the text's own change, `view`
    // could still be the last one (a card tapped before the Porch was read went to Mail).
    onViewChanged: {
        page.reloadDoses()
        page.lookForCard()
    }
    Component.onCompleted: {
        page.takeShown()
        page.reloadDoses()
        page.readWithheld()
        page.readPersonsMade()
    }

    Connections {
        target: page.window

        function onNowChanged() {
            if (page.visible) {
                page.reloadDoses()
                page.readNotices()
                page.agendaStamp += 1
                if (!page.withheldRead)
                    page.readWithheld()
                // An admin window begins: the card waits for it.
                page.readPersonsMade()
            }
        }
    }

    Connections {
        target: page.sioul

        function onSitesChanged() {
            page.readNotices()
        }
    }
    // The message being read, by its file; it stays open across refreshes.
    property string openKey: ""
    readonly property var opened: findItem(openKey)
    // The reader is made the first time a message is opened, then kept.
    onOpenedChanged: {
        if (page.opened !== null && reader.item === null)
            reader.setSource("Reader.qml", { sioul: page.sioul, theme: page.theme, window: page.window })
    }
    // On a phone, the message open takes the page; Back closes it (main.qml).
    readonly property bool canGoBack: page.openKey !== ""
    function back() {
        page.openKey = ""
    }

    // A message tapped on the phone's home screen card (homecard.rs, main.qml's
    // "card-mail"), by its address (`address`: this version's card) or its file
    // (an older card's): opened here, in the Reader, once the Porch is read,
    // while the Porch shows it; else through the window's one opener (filed,
    // moved, read on another device).
    property string cardKey: ""
    property string cardAddress: ""
    function openCardMessage(address, key) {
        page.cardAddress = address
        page.cardKey = key
        page.openKey = key
        page.lookForCard()
    }
    function openMessage(key) {
        page.openCardMessage("", key)
    }
    function lookForCard() {
        if ((page.cardKey === "" && page.cardAddress === "") || page.porchText === "")
            return
        const key = page.cardKey
        const address = page.cardAddress
        page.cardKey = ""
        page.cardAddress = ""
        const item = page.findItem(key) || page.findByAddress(address)
        if (item !== null) {
            page.openKey = item.key
            return
        }
        page.openKey = ""
        page.window.openMail({ address: address, file: key })
    }

    // A message the Porch shows, by its address (`ItemView::uri`).
    function findByAddress(address) {
        if (!address)
            return null
        for (const lane of view.lanes)
            for (const item of lane.items)
                if (item.uri === address)
                    return item
        return null
    }
    // A message keeps the start of its file name when reading or flagging it renames it.
    function sameMessage(a, b) {
        return a !== "" && b !== "" && a.split(/[\/\\]/).pop().split(/[:!]/)[0] === b.split(/[\/\\]/).pop().split(/[:!]/)[0]
    }

    function findItem(key) {
        if (!key)
            return null
        for (const lane of view.lanes)
            for (const item of lane.items)
                if (page.sameMessage(item.key, key))
                    return item
        return null
    }

    // How mail is sorted, every lane: for the window's images.
    function openSettings(open) {
        settingsButton.show(open)
    }

    // A scan of a paper letter, chosen and copied into the folder scans are
    // read from (the New menu's "A paper letter (scan)…"); without a folder,
    // where one is chosen: Settings.
    function addScan() {
        if (page.sioul.lettersFolder() === "")
            page.window.showParameters("case_store")
        else
            scanPicker.open()
    }

    function grabSettings(path) {
        settingsButton.grab(path)
    }

    // A message Sioul's own spam filter put a word beside ("" any), opened
    // with why it is here unfolded: for the window's images.
    function openSpam(word) {
        for (const lane of view.lanes)
            for (const item of lane.items)
                if (item.spam_word !== "" && (word === "" || item.spam_word === word)) {
                    page.openKey = item.key
                    return
                }
    }

    function showReasons() {
        if (reader.item !== null)
            reader.item.reasonsShown = true
    }

    // A lane's section, by its key, among the Porch's (the window's images).
    function laneSectionOf(key) {
        for (const lanes of [earlyLanes, lateLanes])
            for (let i = 0; i < lanes.count; ++i) {
                const section = lanes.itemAt(i)
                if (section !== null && section.key === key)
                    return section
            }
        return null
    }

    // A lane at the top of the list (the window's images).
    function scrollToLane(key) {
        const section = page.laneSectionOf(key)
        const flick = list.contentItem as Flickable
        if (section === null || flick === null)
            return
        const top = section.mapToItem(flick.contentItem, 0, 0).y
        flick.contentY = Math.max(0, Math.min(top - 8, flick.contentHeight - flick.height))
    }

    // The first message of the first lane, for the window's images.
    function openFirst() {
        if (view.lanes.length > 0 && view.lanes[0].items.length > 0)
            page.openKey = view.lanes[0].items[0].key
    }

    // A lane opened, as its title opens it (the window's images).
    function openLane(key) {
        const section = page.laneSectionOf(key)
        if (section !== null)
            section.unfold()
    }

    function trustIcon(level) {
        return level === "verified" ? "security-high" : level === "forged" ? "security-low" : "security-medium"
    }

    // The first new message of the first lane dismissed, as its row's Dismiss does (the window's images).
    function dismissFirst() {
        for (const lane of view.lanes) {
            const shown = page.newOnes(lane.items)
            if (shown.length > 0) {
                page.say("dismiss", page.mailRow(shown[0]))
                return
            }
        }
    }

    // A section's help, once, and what changes it: in a popup (`helpPopup`).
    function showHelp(lane) {
        helpPopup.now().show(lane)
    }

    // The help's popup, when made (the window's pictures), and closed.
    function helpItem() {
        return helpPopup.item
    }
    function closeHelp() {
        if (helpPopup.item !== null)
            helpPopup.item.close()
    }

    // A lane's help as its "?" says it: what it holds, how mail lands there.
    function helpOf(lane) {
        return [lane.about].concat(lane.rules || []).filter(t => !!t).join("\n\n")
    }

    // A message as AddMenu ties things to it.
    function sourceOf(item) {
        return { uri: page.sioul.uriOf("mail", item.key), kind: "mail", key: item.key, title: item.subject, name: item.sender, address: item.hidden ? "" : item.address, known: !item.screener }
    }

    // A message's notice's row: its kind, its notice, the file Read marks.
    function mailRow(item) {
        return { kind: "mail", ids: [item.notice.id], targets: [item.key], notice: item.notice }
    }

    // A letter opened: on its scan, as a letter opens until the Letters page opens it.
    // A letter opened on the Letters page (main.qml's `openLetter`); its scan alone by "See the scan".
    function openLetter(letter) {
        if (letter.id)
            page.window.openLetter(letter.id)
    }

    function seeScan(letter) {
        if (letter.file)
            Qt.openUrlExternally(page.theme.fileUrl(letter.file))
    }

    // A letter's own actions (its row's menu): a task for its date, its
    // appointment in the agenda, done (filed with its project).
    function letterAct(id, letter) {
        let problem = ""
        if (id === "task")
            problem = page.sioul.letterTask(letter.id)
        else if (id === "open-task")
            page.window.openTask(letter.task)
        else if (id === "event")
            problem = page.sioul.letterEvent(letter.id)
        else if (id === "done")
            problem = page.sioul.letterDone(letter.id, letter.project || "")
        if (problem !== "")
            page.sioul.status = problem
        page.readNotices()
    }

    RowLayout {
        id: columns

        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        // Widths are given, not left to the text: a long line would otherwise
        // widen its column under the other one.
        ScrollView {
            id: list

            visible: !(page.window.compact && page.opened !== null)
            Layout.fillHeight: true
            Layout.fillWidth: page.opened === null
            Layout.minimumWidth: 0
            Layout.preferredWidth: page.opened === null ? columns.width : Math.round((columns.width - columns.spacing) * 0.38)
            contentWidth: availableWidth

            ColumnLayout {
                width: list.availableWidth
                spacing: page.theme.gap

                // Where the Porch stands, in a line; its settings. Real time went to the card
                // What reaches you now, as Mail every minute (part 5).
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        Layout.fillWidth: true
                        text: page.view.status
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: page.theme.muted
                    }
                    SettingsButton {
                        id: settingsButton

                        sioul: page.sioul
                        theme: page.theme
                        view: "porch"
                    }
                }
                // "Show all mail now" (decision 12): a look at every lane and address, at
                // any hour, on this device alone, which tells nothing ("Work now" went:
                // Reachable and "Work until tonight" have its two jobs, part 3). Under the
                // first row, which it widened past a phone's edge in French (part 5).
                Button {
                    objectName: "show-all-mail"
                    visible: page.view.open === true && page.view.all !== true
                    Layout.maximumWidth: parent.width
                    flat: true
                    text: page.sioul.text("ui-show-all-mail")
                    Accessible.name: page.sioul.text("ui-show-all-mail")
                    Accessible.description: page.sioul.text("ui-show-all-mail-help")
                    ToolTip.visible: hovered
                    ToolTip.text: page.sioul.text("ui-show-all-mail-help")
                    ToolTip.delay: 400
                    onClicked: page.sioul.openAnyway()
                }

                // On a phone: other apps' notifications held until their time, and those back (docs/android.md).
                Label {
                    visible: page.view.open && (page.view.apps_line || "") !== ""
                    Layout.fillWidth: true
                    text: page.view.apps_line || ""
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.muted
                }

                // A device that does not share yet: joining your other devices,
                // through your Nextcloud's own sign-in or a sync app's folder
                // (docs/database.md, "Joining through your Nextcloud's sign-in");
                // first, since what they bring sets the hours and the night below.
                // Asked here until it shares, or until you leave it as it is.
                Panel {
                    id: joinCard

                    property bool leftAsIs: page.sioul.viewFlag("porch-join-left")

                    visible: page.window.moment.shares === false && !joinCard.leftAsIs
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("join-title")
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("join-card-why")
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 8

                            Button {
                                text: page.sioul.text("join-murena")
                                onClicked: page.window.joinDevices("murena")
                            }
                            Button {
                                text: page.sioul.text("join-other")
                                onClicked: page.window.joinDevices("nextcloud")
                            }
                            Button {
                                text: page.sioul.text("join-folder")
                                onClicked: page.window.joinDevices("folder")
                            }
                            Button {
                                flat: true
                                text: page.sioul.text("porch-hours-leave")
                                onClicked: {
                                    page.sioul.setViewFlag("porch-join-left", true)
                                    joinCard.leftAsIs = true
                                }
                            }
                        }
                    }
                }

                // The hours not set yet: work, your admin (docs/areas.md); every
                // other time is leisure. Asked here until they are, or until you
                // leave them as they are.
                Panel {
                    id: hoursCard

                    readonly property var missing: [["window", "work_hours", "set-windows"], ["window.admin", "admin_hours", "set-windows-admin"]].filter(k => page.window.moment[k[1]] === false)
                    property bool leftAsIs: page.sioul.viewFlag("porch-hours-left")

                    visible: hoursCard.missing.length > 0 && !hoursCard.leftAsIs
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: hoursCard.missing.length === 2 ? page.sioul.text("porch-hours-none") : page.sioul.textWith("porch-hours-some", "which", hoursCard.missing.map(k => page.sioul.text(k[2])).join(", "))
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("porch-hours-why")
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                        RowLayout {
                            spacing: 8

                            Button {
                                text: page.sioul.text("porch-hours-set")
                                icon.name: "chronometer"
                                icon.color: page.theme.text
                                onClicked: page.window.showParameters(hoursCard.missing[0][0])
                            }
                            Button {
                                flat: true
                                text: page.sioul.text("porch-hours-leave")
                                onClicked: {
                                    page.sioul.setViewFlag("porch-hours-left", true)
                                    hoursCard.leftAsIs = true
                                }
                            }
                        }
                    }
                }

                // The night not set (Health): nothing keeps notifications away while
                // you sleep (docs/health.md). Asked until it is, or left as it is.
                Panel {
                    id: nightCard

                    property bool leftAsIs: page.sioul.viewFlag("porch-night-left")

                    visible: page.window.moment.night === false && !nightCard.leftAsIs
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("porch-night-none")
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("porch-night-why")
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                        RowLayout {
                            spacing: 8

                            Button {
                                text: page.sioul.text("porch-night-set")
                                icon.name: "chronometer"
                                icon.color: page.theme.text
                                onClicked: page.window.openThing({ kind: "needs", uri: "", key: "" })
                            }
                            Button {
                                flat: true
                                text: page.sioul.text("porch-hours-leave")
                                onClicked: {
                                    page.sioul.setViewFlag("porch-night-left", true)
                                    nightCard.leftAsIs = true
                                }
                            }
                        }
                    }
                }

                // The persons made from older texts: who, and what named each.
                Panel {
                    objectName: "porchPersonsMade"
                    visible: page.personsMade.length > 0
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 4

                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("persons-made-title")
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                        Repeater {
                            model: page.personsMade

                            delegate: Label {
                                required property var modelData

                                Layout.fillWidth: true
                                text: modelData.from !== "" ? page.sioul.textArgs("persons-made-row", JSON.stringify({ name: modelData.name, from: modelData.from })) : modelData.name
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: page.theme.muted
                            }
                        }
                    }
                }

                // Notes or papers gone many at once here, held until they leave
                // your other devices by themselves (docs/database.md, "Removed
                // notes"): said once, calmly. Nothing has to be pressed: once
                // shown a few seconds, it is not said again when the Porch is
                // left; Settings ▸ Your folder and sharing says it as long as it lasts.
                Repeater {
                    model: page.withheld

                    delegate: Panel {
                        id: withheldCard

                        required property var modelData

                        Layout.fillWidth: true
                        theme: page.theme

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 6

                            Label {
                                Layout.fillWidth: true
                                text: withheldCard.modelData.text
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: page.theme.text
                            }
                            Label {
                                Layout.fillWidth: true
                                text: withheldCard.modelData.leaves
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: page.theme.muted
                            }
                            Label {
                                visible: text !== ""
                                Layout.fillWidth: true
                                text: page.withheldProblem
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                font.pixelSize: 13
                                color: page.theme.muted
                            }
                            Flow {
                                Layout.fillWidth: true
                                spacing: 8

                                Button {
                                    enabled: page.puttingBack === ""
                                    text: page.sioul.text(page.puttingBack === withheldCard.modelData.key ? "porch-withheld-putting-back" : "porch-withheld-put-back")
                                    onClicked: {
                                        page.withheldProblem = ""
                                        page.puttingBack = withheldCard.modelData.key
                                        page.sioul.sharePutBackWithheld(withheldCard.modelData.store)
                                    }
                                }
                                Button {
                                    enabled: page.puttingBack === ""
                                    text: page.sioul.text("porch-withheld-take-out")
                                    onClicked: {
                                        page.withheldProblem = page.sioul.shareTakeOutWithheld(withheldCard.modelData.store)
                                        if (page.withheldProblem === "")
                                            page.withheldSaid(withheldCard.modelData.key)
                                    }
                                }
                                Button {
                                    flat: true
                                    text: page.sioul.text("porch-hours-leave")
                                    onClicked: page.withheldSaid(withheldCard.modelData.key)
                                }
                            }
                        }
                    }
                }

                // Day one of mail's topics: the topics Sioul started from the senders it
                // knows, said once, until you put it away (docs/topics.md, "Topic presets").
                Panel {
                    id: topicsCard

                    property string text: page.sioul.viewFlag("topics-day-one-left") ? "" : page.sioul.topicsCard()

                    objectName: "topicsDayOne"
                    visible: topicsCard.text !== ""
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: topicsCard.text
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: page.theme.text
                        }
                        Button {
                            flat: true
                            text: page.sioul.text("porch-hours-leave")
                            onClicked: {
                                page.sioul.setViewFlag("topics-day-one-left", true)
                                topicsCard.text = ""
                            }
                        }
                    }
                }

                // On a phone: what Android has not allowed Sioul yet that it needs
                // here, said once, until you put it away (docs/android.md, "What Android withholds").
                AccessCard {
                    Layout.fillWidth: true
                    active: page.visible
                    sioul: page.sioul
                    theme: page.theme
                }

                // Where you stopped, when something interrupted you: first, until it is done.
                StoppedCard {
                    Layout.fillWidth: true
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                }

                // Nothing read here, the health file nor its last read: said,
                // never an empty list read as no doses (review of 36002ea's check, low 19).
                Panel {
                    visible: (page.doses.note || "") !== ""
                    Layout.fillWidth: true
                    theme: page.theme

                    Label {
                        anchors.fill: parent
                        text: page.doses.note || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: page.theme.text
                    }
                }

                // Today's doses from their time on, not marked yet: reminded or
                // not, a notification can go unseen. "Taken" marks one now; more
                // than half an hour late, when it was taken is asked (DoseTaken.qml).
                // Under one, the doubt when another device may know: never "not taken".
                Panel {
                    visible: page.dueDoses.length > 0
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        Repeater {
                            model: page.dueDoses

                            delegate: GridLayout {
                                id: dueDose

                                required property var modelData

                                Layout.fillWidth: true
                                columns: 3
                                columnSpacing: 12
                                rowSpacing: 2

                                // As wide as the question's below: the names line up.
                                Label {
                                    Layout.preferredWidth: 90
                                    text: dueDose.modelData.time
                                    textFormat: Text.PlainText
                                    font.features: { "tnum": 1 }
                                    color: page.theme.text
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: dueDose.modelData.name + (dueDose.modelData.dose !== "" ? "  ·  " + dueDose.modelData.dose : "")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                                // Answers that differ on your devices (`choose`): both said
                                // under it, you choose; "Taken" then keeps the time it was taken.
                                RowLayout {
                                    Layout.alignment: Qt.AlignRight
                                    spacing: 6

                                    Button {
                                        text: page.sioul.text(dueDose.modelData.late && !dueDose.modelData.choose ? "health-taken-when" : "health-taken")
                                        onClicked: {
                                            // The writer's answer said, as Health's list says it.
                                            if (dueDose.modelData.choose) {
                                                const chosen = page.sioul.doseChoose(dueDose.modelData.key, true)
                                                if (chosen !== "")
                                                    page.sioul.status = chosen
                                                page.reloadDoses()
                                                return
                                            }
                                            if (dueDose.modelData.late) {
                                                page.window.askDose(dueDose.modelData.key)
                                                return
                                            }
                                            const problem = page.sioul.setDoseTaken(dueDose.modelData.key, true)
                                            if (problem !== "")
                                                page.sioul.status = problem
                                            page.reloadDoses()
                                        }
                                    }
                                    Button {
                                        visible: dueDose.modelData.choose === true
                                        flat: true
                                        text: page.sioul.text("health-not-taken")
                                        onClicked: {
                                            const problem = page.sioul.doseChoose(dueDose.modelData.key, false)
                                            if (problem !== "")
                                                page.sioul.status = problem
                                            page.reloadDoses()
                                        }
                                    }
                                }
                                // Whether it was taken on another device is not known here: said, never
                                // guessed; "This device is off" for each device it names.
                                DoubtLine {
                                    Layout.columnSpan: 3
                                    Layout.fillWidth: true
                                    sioul: page.sioul
                                    theme: page.theme
                                    doubt: dueDose.modelData.doubt
                                    offs: dueDose.modelData.doubt_off || []
                                    onSaidOff: page.reloadDoses()
                                }
                            }
                        }
                    }
                }

                // Doses neither marked nor reminded: due while Sioul was closed on all your
                // computers, or while it ran but their reminder could not be shown. A question
                // on the past, answered once (when it was taken, in DoseTaken.qml); never a reminder.
                Panel {
                    visible: page.missedDoses.length > 0
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        Repeater {
                            model: page.missedDoses

                            // The buttons beside the dose, or under it on a phone.
                            delegate: GridLayout {
                                id: missedDose

                                required property var modelData
                                required property int index

                                Layout.fillWidth: true
                                columns: page.width < 600 ? 2 : 3
                                columnSpacing: 12
                                rowSpacing: 2

                                // The question, above the first dose of its kind: Sioul closed,
                                // or its reminder not shown (health.rs, `missed_rows`).
                                Label {
                                    visible: missedDose.index === 0 || page.missedDoses[missedDose.index - 1].question !== missedDose.modelData.question
                                    Layout.columnSpan: missedDose.columns
                                    Layout.fillWidth: true
                                    Layout.topMargin: missedDose.index === 0 ? 0 : 6
                                    Layout.bottomMargin: 4
                                    text: missedDose.modelData.question
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.weight: Font.DemiBold
                                    color: page.theme.text
                                }
                                Label {
                                    Layout.preferredWidth: 90
                                    text: missedDose.modelData.time
                                    textFormat: Text.PlainText
                                    font.features: { "tnum": 1 }
                                    color: page.theme.text
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: missedDose.modelData.name + (missedDose.modelData.dose !== "" ? "  ·  " + missedDose.modelData.dose : "")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                                RowLayout {
                                    Layout.columnSpan: page.width < 600 ? 2 : 1
                                    Layout.alignment: Qt.AlignRight
                                    spacing: 6

                                    Button {
                                        text: page.sioul.text("health-taken-when")
                                        onClicked: page.window.askDose(missedDose.modelData.key)
                                    }
                                    Button {
                                        flat: true
                                        text: page.sioul.text("health-not-taken")
                                        onClicked: {
                                            page.sioul.doseNotTaken(missedDose.modelData.key)
                                            page.reloadDoses()
                                        }
                                    }
                                }
                                // Whether it was taken on another device is not known here: said, never
                                // guessed; "This device is off" for each device it names.
                                DoubtLine {
                                    Layout.columnSpan: missedDose.columns
                                    Layout.fillWidth: true
                                    sioul: page.sioul
                                    theme: page.theme
                                    doubt: missedDose.modelData.doubt
                                    offs: missedDose.modelData.doubt_off || []
                                    onSaidOff: page.reloadDoses()
                                }
                            }
                        }
                    }
                }

                // Right now: the one exception to the hours, a code's message at
                // once, whatever the time (docs/porch.md, "Right now").
                PorchSection {
                    sioul: page.sioul
                    theme: page.theme
                    compact: page.window.compact
                    key: "right-now"
                    title: page.sioul.text("right-now-title")
                    help: page.sioul.text("porch-about-right-now")
                    count: page.newOnes(page.view.right_now).length
                    onHelpAsked: page.showHelp({ key: "right-now", title: page.sioul.text("right-now-title"), about: page.sioul.text("porch-about-right-now"), rules: [], settings: "" })

                    Repeater {
                        model: page.newOnes(page.view.right_now)

                        delegate: NoticeRow {
                            id: codeRow

                            required property var modelData

                            sioul: page.sioul
                            theme: page.theme
                            window: page.window
                            compact: page.narrowRows
                            notice: codeRow.modelData.notice
                            iconName: codeRow.modelData.warning ? "security-medium" : "security-high"
                            note: codeRow.modelData.warning || ""
                            mainText: codeRow.modelData.code ? page.sioul.text("ui-copy") : ""
                            mainIcon: "edit-copy"
                            onMainPressed: page.window.copy(codeRow.modelData.code)
                            // Codes are in no lane: their message through the window's one opener, by its address.
                            onOpened: page.window.openMail({ address: codeRow.modelData.address || "", file: codeRow.modelData.key })
                            onReadPressed: page.say("read", { kind: "mail", ids: [codeRow.modelData.notice.id], targets: [codeRow.modelData.key] })
                            onDismissPressed: page.say("dismiss", { kind: "mail", ids: [codeRow.modelData.notice.id], targets: [codeRow.modelData.key] })
                        }
                    }
                }

                // Closed: when it opens, and nothing else.
                ColumnLayout {
                    visible: !page.view.open
                    Layout.fillWidth: true
                    Layout.topMargin: page.theme.gap
                    spacing: page.theme.gap

                    Label {
                        Layout.fillWidth: true
                        text: page.view.closed || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        font.pixelSize: 19
                        lineHeight: 1.3
                        color: page.theme.text
                    }
                    Label {
                        Layout.fillWidth: true
                        text: page.sioul.text("ui-not-open")
                        wrapMode: Text.Wrap
                        color: page.theme.muted
                    }
                    Button {
                        text: page.sioul.text("ui-show-all-mail")
                        onClicked: page.sioul.openAnyway()
                    }
                }

                // Each project, your public addresses, what you sent yourself, the people you know.
                Repeater {
                    id: earlyLanes

                    model: page.view.open ? page.view.lanes.filter(lane => !page.isLate(lane.key)) : []
                    delegate: laneSection
                }

                // From your phone: its calls, texts and other apps' notifications, each at a
                // time its sender may reach you, whatever the Porch's own hours
                // (CallsSection.qml, PhoneMessagesSection.qml, docs/porch.md, "From your phone").
                PorchSection {
                    id: phoneSection

                    sioul: page.sioul
                    theme: page.theme
                    compact: page.window.compact
                    key: "phone"
                    title: page.sioul.text("phonemsgs-title")
                    help: page.sioul.text("porch-about-phone")
                    count: calls.count + phoneMessages.count
                    onHelpAsked: page.showHelp({ key: "phone", title: page.sioul.text("phonemsgs-title"), about: page.sioul.text("porch-about-phone"), rules: [], settings: "" })

                    CallsSection {
                        id: calls

                        Layout.fillWidth: true
                        sioul: page.sioul
                        theme: page.theme
                        window: page.window
                        porch: page
                    }
                    PhoneMessagesSection {
                        id: phoneMessages

                        Layout.fillWidth: true
                        sioul: page.sioul
                        theme: page.theme
                        window: page.window
                        porch: page
                    }
                }

                // The agenda's conflicts waiting for an answer (`notices::agenda_rows`, from the
                // agenda's `overlaps`): only those someone else's change made, each once, at its
                // time, as the agenda says them; Move mine opens your own event, Keep both says it
                // no more; either settles its notice on every device (docs/porch.md, "Agenda").
                PorchSection {
                    id: agendaSection

                    readonly property var conflicts: page.view.open ? page.newOnes(JSON.parse(page.sioul.notices("agenda", page.agendaStamp >= 0 ? (page.sioul.overlaps || "[]") : "[]") || "[]")) : []

                    sioul: page.sioul
                    theme: page.theme
                    compact: page.window.compact
                    key: "agenda"
                    title: page.sioul.text("porch-agenda")
                    help: page.sioul.text("porch-about-agenda")
                    count: agendaSection.conflicts.length
                    onHelpAsked: page.showHelp({ key: "agenda", title: page.sioul.text("porch-agenda"), about: page.sioul.text("porch-about-agenda"), rules: [], settings: "" })

                    Repeater {
                        model: agendaSection.conflicts

                        delegate: NoticeRow {
                            id: conflictRow

                            required property var modelData
                            readonly property var conflict: conflictRow.modelData.conflict || ({})
                            readonly property bool movable: (conflictRow.conflict.answers || []).indexOf("move") >= 0
                            readonly property var events: conflictRow.conflict.kind === "during" ? [conflictRow.conflict.event] : [conflictRow.conflict.first, conflictRow.conflict.second]

                            sioul: page.sioul
                            theme: page.theme
                            compact: page.narrowRows
                            notice: conflictRow.modelData.notice
                            iconName: "view-calendar-day"
                            mainText: page.sioul.text(conflictRow.movable ? "conflict-move-mine" : "conflict-keep-both")
                            mainIcon: ""
                            more: {
                                const more = conflictRow.movable ? [{ id: "keep", text: page.sioul.text("conflict-keep-both"), icon: "" }] : []
                                for (let i = 0; i < conflictRow.events.length; ++i) {
                                    const e = conflictRow.events[i] || {}
                                    more.push({ id: "open-" + i, text: page.theme.plain(page.sioul.textWith("overlap-open", "title", e.title || "")), icon: "" })
                                }
                                return more
                            }
                            onOpened: page.openEvent(conflictRow.events[0])
                            onMainPressed: page.answerConflict(conflictRow.modelData, conflictRow.movable ? "move" : "keep")
                            onDismissPressed: page.say("dismiss", conflictRow.modelData)
                            onChosen: id => {
                                if (id === "keep")
                                    page.answerConflict(conflictRow.modelData, "keep")
                                else if (id.indexOf("open-") === 0)
                                    page.openEvent(conflictRow.events[Number(id.slice(5))])
                            }
                        }
                    }
                }

                // The sites' notifications kept for the Porch, a row per site.
                PorchSection {
                    sioul: page.sioul
                    theme: page.theme
                    compact: page.window.compact
                    key: "sites"
                    title: page.sioul.text("porch-sites")
                    help: page.sioul.text("porch-about-sites")
                    count: page.view.open ? page.newOnes(page.notices.sites || []).length : 0
                    onHelpAsked: page.showHelp({ key: "sites", title: page.sioul.text("porch-sites"), about: page.sioul.text("porch-about-sites"), rules: [], settings: "" })

                    Repeater {
                        model: page.view.open ? page.newOnes(page.notices.sites || []) : []

                        delegate: NoticeRow {
                            id: siteRow

                            required property var modelData

                            sioul: page.sioul
                            theme: page.theme
                            compact: page.narrowRows
                            notice: siteRow.modelData.notice
                            iconName: "internet-services"
                            mainText: page.sioul.text("ui-open")
                            mainIcon: "internet-services"
                            onOpened: page.window.openSite(siteRow.modelData.site)
                            onMainPressed: page.window.openSite(siteRow.modelData.site)
                            onDismissPressed: page.say("dismiss", siteRow.modelData)

                            // Its earlier lines, under the newest.
                            Repeater {
                                model: (siteRow.modelData.lines || []).slice(0, -1).slice(-3)

                                delegate: Label {
                                    required property string modelData

                                    Layout.fillWidth: true
                                    text: modelData
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: 13
                                    color: page.theme.muted
                                }
                            }
                        }
                    }
                }

                // Money: what the week holds, as Budgets reads it; none at night.
                PorchSection {
                    sioul: page.sioul
                    theme: page.theme
                    compact: page.window.compact
                    key: "money"
                    title: page.sioul.text("porch-money")
                    help: page.sioul.text("porch-about-money")
                    count: page.view.open && page.window.moment.sleep !== true ? page.newOnes(page.notices.money || []).length : 0
                    onHelpAsked: page.showHelp({ key: "money", title: page.sioul.text("porch-money"), about: page.sioul.text("porch-about-money"), rules: [], settings: "" })

                    Repeater {
                        model: page.view.open && page.window.moment.sleep !== true ? page.newOnes(page.notices.money || []) : []

                        delegate: NoticeRow {
                            id: moneyRow

                            required property var modelData

                            sioul: page.sioul
                            theme: page.theme
                            compact: page.narrowRows
                            notice: moneyRow.modelData.notice
                            iconName: "view-financial-budget"
                            amount: moneyRow.modelData.amount || ""
                            // A shortfall's sentence whole: the payment, what is missing, what covers it.
                            note: moneyRow.modelData.note || ""
                            mainText: page.sioul.text("porch-open-budgets")
                            mainIcon: "view-financial-budget"
                            onOpened: page.window.page = 9
                            onMainPressed: page.window.page = 9
                            onDismissPressed: page.say("dismiss", moneyRow.modelData)
                        }
                    }
                }

                // Paper letters waiting, read from their scans: in the window, like mail; only while one waits.
                PorchSection {
                    sioul: page.sioul
                    theme: page.theme
                    compact: page.window.compact
                    key: "letters"
                    title: page.sioul.text("letters")
                    help: page.sioul.text("porch-about-letters")
                    count: page.view.open && page.window.moment.sleep !== true ? page.newOnes(page.notices.letters || []).length : 0
                    onHelpAsked: page.showHelp({ key: "letters", title: page.sioul.text("letters"), about: page.sioul.text("porch-about-letters"), rules: [], settings: "" })

                    Repeater {
                        model: page.view.open && page.window.moment.sleep !== true ? page.newOnes(page.notices.letters || []) : []

                        delegate: NoticeRow {
                            id: letterRow

                            required property var modelData
                            readonly property var letter: letterRow.modelData.letter || ({})

                            sioul: page.sioul
                            theme: page.theme
                            compact: page.narrowRows
                            notice: letterRow.modelData.notice
                            iconName: "mail-read"
                            // A scan not read: why, said calmly.
                            note: (letterRow.letter.problem || "") !== "" ? page.sioul.textWith("letters-unread", "problem", letterRow.letter.problem) : ""
                            // The row opens the letter on the Letters page; its main action, the scan.
                            mainText: page.sioul.text("letters-scan")
                            mainIcon: "document-open"
                            more: [letterRow.letter.deadline !== "" && letterRow.letter.task === "" ? { id: "task", text: page.sioul.text("letters-task") } : null,
                                   letterRow.letter.task !== "" ? { id: "open-task", text: page.sioul.text("letters-task-made") } : null,
                                   letterRow.letter.appointment ? { id: "event", text: page.sioul.text("letters-event") } : null,
                                   { id: "done", text: page.sioul.text("letters-done") }].filter(m => m !== null)
                            onOpened: page.openLetter(letterRow.letter)
                            onMainPressed: page.seeScan(letterRow.letter)
                            onDismissPressed: page.say("dismiss", letterRow.modelData)
                            onChosen: id => page.letterAct(id, letterRow.letter)

                            // What it says: what it asks, by when.
                            Repeater {
                                model: letterRow.letter.lines || []

                                delegate: Label {
                                    required property string modelData

                                    Layout.fillWidth: true
                                    text: modelData
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 13
                                    color: page.theme.muted
                                }
                            }
                        }
                    }
                }

                // Filed mail, someone new, less important accounts, your filter's catches, what is set aside, hostile mail.
                Repeater {
                    id: lateLanes

                    model: page.view.open ? page.view.lanes.filter(lane => page.isLate(lane.key)) : []
                    delegate: laneSection
                }

                // Closes the Porch on the mail it holds: only while some is shown.
                Button {
                    visible: page.view.open && page.view.lanes.some(lane => page.newOnes(lane.items).length > 0)
                    Layout.topMargin: page.theme.gap
                    text: page.sioul.text("ui-done")
                    onClicked: {
                        page.openKey = ""
                        page.sioul.done()
                    }
                }

                Item {
                    Layout.preferredHeight: page.theme.gap
                }
            }
        }

        // The message, in the Reader (Reader.qml): made the first time one is
        // opened (onOpenedChanged), since it takes long to make.
        Loader {
            id: reader

            visible: page.opened !== null && reader.item !== null && reader.item.showing
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumWidth: 0
            Layout.preferredWidth: Math.round((columns.width - columns.spacing) * 0.62)

            Binding {
                target: reader.item
                when: reader.item !== null
                property: "key"
                value: page.opened !== null ? page.openKey : ""
            }
            Binding {
                target: reader.item
                when: reader.item !== null
                property: "item"
                value: page.opened
            }
            Connections {
                target: reader.item

                function onCloseRequested() {
                    page.openKey = ""
                }
            }
        }
    }

    // A lane of mail, as a section (each project, your public addresses, what
    // you sent yourself, the people you know; filed, someone new, less
    // important, your filter's catches, set aside, hostile): its messages
    // still new, the one open in the Reader too.
    Component {
        id: laneSection

        PorchSection {
            id: lane

            required property var modelData
            readonly property var shown: page.newOnes(lane.modelData.items)

            sioul: page.sioul
            theme: page.theme
            compact: page.window.compact
            key: lane.modelData.key
            title: lane.modelData.title
            help: page.helpOf(lane.modelData)
            foldedByDefault: lane.modelData.folded
            count: lane.shown.length
            onHelpAsked: page.showHelp(lane.modelData)

            // The review queue, opened: your word on all of it at once, ten seconds to undo.
            Flow {
                visible: lane.modelData.key === "review"
                Layout.fillWidth: true
                Layout.leftMargin: 8
                spacing: 6

                Button {
                    flat: true
                    text: page.sioul.text("ui-not-spam-all")
                    onClicked: page.sioul.actMany(JSON.stringify(lane.shown.map(item => item.key)), "not-spam")
                }
                Button {
                    flat: true
                    text: page.sioul.text("ui-spam-all")
                    onClicked: page.sioul.actMany(JSON.stringify(lane.shown.map(item => item.key)), "spam")
                }
            }
            Repeater {
                model: lane.folded ? [] : lane.shown

                delegate: NoticeRow {
                    id: message

                    required property var modelData

                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                    compact: page.narrowRows
                    notice: message.modelData.notice
                    highlighted: page.sameMessage(page.openKey, message.modelData.key)
                    iconName: page.trustIcon(message.modelData.trust_level)
                    iconTip: message.modelData.checks || ""
                    veiled: message.modelData.hidden === true
                    // In words, quiet: an account ranked more important (no orange mark); a
                    // public address's topic, a rude tone; your own filter's word ("maybe spam"), never why.
                    chips: [message.modelData.important ? page.sioul.text("priority-above") : "", message.modelData.topic, message.modelData.tone === "rude" ? page.sioul.text("tone-rude") : "",
                            message.modelData.spam_word || "", message.modelData.attachments.length > 0 ? page.sioul.text("notice-attached") : ""].filter(t => !!t)
                    note: message.modelData.summary || ""
                    source: page.sourceOf(message.modelData)
                    onOpened: page.openKey = message.modelData.key
                    onReadPressed: page.say("read", page.mailRow(message.modelData))
                    onDismissPressed: page.say("dismiss", page.mailRow(message.modelData))

                    // In the review queue: your word, Spam or Not spam, ten seconds to undo.
                    Flow {
                        visible: !!message.modelData.review
                        Layout.fillWidth: true
                        spacing: 4

                        Button {
                            flat: true
                            font.pixelSize: 13
                            text: page.sioul.text("ui-not-spam")
                            Accessible.name: message.modelData.subject + ", " + text
                            onClicked: page.sioul.act(message.modelData.key, "not-spam", "")
                        }
                        Button {
                            flat: true
                            font.pixelSize: 13
                            text: page.sioul.text("ui-spam")
                            Accessible.name: message.modelData.subject + ", " + text
                            onClicked: page.sioul.act(message.modelData.key, "spam", "")
                        }
                    }
                }
            }
        }
    }

    // What was just said, for ten seconds: its line and Undo, over the list's foot.
    Panel {
        objectName: "noticeUndo"
        visible: page.lastSaid !== null
        anchors.bottom: parent.bottom
        anchors.horizontalCenter: columns.horizontalCenter
        anchors.bottomMargin: page.theme.gap
        width: Math.min(page.width - 2 * page.theme.gap, implicitWidth)
        theme: page.theme

        RowLayout {
            id: undoRow

            anchors.fill: parent
            spacing: 8

            Label {
                Layout.fillWidth: true
                text: page.lastSaid ? page.lastSaid.said : ""
                textFormat: Text.PlainText
                elide: Text.ElideRight
                color: page.theme.text
            }
            Button {
                objectName: "noticeUndoButton"
                flat: true
                text: page.sioul.text("ui-undo")
                onClicked: page.undo()
            }
        }
    }

    // A section's help, once, and what changes it (each setting read by its
    // file, SettingRow.qml, which the Porch then does not load at its start).
    Later {
        id: helpPopup

        sourceComponent: Component {
            SafePopup {
                id: help

                property var lane: null
                property var rows: []

                function show(lane) {
                    help.lane = lane
                    help.rows = lane.settings ? JSON.parse(page.sioul.settings(lane.settings)) : []
                    help.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(560, Math.min(page.width, help.roomWidth) - 2 * page.theme.gap)
                height: Math.min(implicitHeight, help.roomHeight - 2 * page.theme.gap)
                padding: 16

                contentItem: ScrollView {
                    contentWidth: availableWidth

                    ColumnLayout {
                        width: help.availableWidth
                        spacing: 8

                        Label {
                            Layout.fillWidth: true
                            text: help.lane ? help.lane.title : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 17
                            font.weight: Font.DemiBold
                            color: page.theme.text
                        }
                        Repeater {
                            model: help.lane ? [help.lane.about].concat(help.lane.rules || []).filter(t => !!t) : []

                            delegate: Label {
                                required property string modelData

                                Layout.fillWidth: true
                                text: modelData
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                lineHeight: 1.25
                                color: page.theme.text
                            }
                        }
                        Repeater {
                            model: help.rows

                            delegate: Loader {
                                id: ruleRow

                                required property var modelData

                                Layout.fillWidth: true
                                Layout.topMargin: 6
                                Component.onCompleted: ruleRow.setSource("SettingRow.qml", { setting: ruleRow.modelData, sioul: page.sioul, theme: page.theme })

                                Connections {
                                    target: ruleRow.item

                                    function onSave(key, value, shown) {
                                        const problem = page.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
                                        if (problem === "")
                                            help.rows = JSON.parse(page.sioul.settings(help.lane.settings))
                                        else
                                            page.sioul.status = problem
                                    }
                                }
                            }
                        }
                        Button {
                            Layout.alignment: Qt.AlignRight
                            text: page.sioul.text("ui-close")
                            onClicked: help.close()
                        }
                    }
                }
            }
        }
    }

    // PDF and pictures, the kinds Sioul reads; several at once. Each copied,
    // the chosen file left where it was; what happened said on the status line.
    FileDialog {
        id: scanPicker

        title: page.sioul.text("letters-add-scan-title")
        fileMode: FileDialog.OpenFiles
        nameFilters: [page.sioul.text("letters-add-scan-filter") + " (*.pdf *.png *.jpg *.jpeg *.tif *.tiff *.webp)"]
        onAccepted: page.sioul.status = page.sioul.lettersAdd(JSON.stringify(Array.from(scanPicker.selectedFiles, f => f.toString())))
    }
}
