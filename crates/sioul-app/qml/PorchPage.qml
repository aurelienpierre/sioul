// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The Porch: verified codes on top, whatever the time; then, inside an admin
// window, a few sentences and the lanes; outside, only when it opens. A
// message opens on the right, in the Reader.

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
    property string porchText: ""
    readonly property var view: page.porchText ? JSON.parse(page.porchText) : ({ open: false, closed: null, right_now: [], summary: "", status: "", lanes: [] })

    function takeShown() {
        if (page.visible)
            page.porchText = page.sioul.porch
    }

    Connections {
        target: page.sioul

        function onPorchChanged() {
            page.takeShown()
        }
    }
    // Lanes whose "?" is open, by key.
    property var helpShown: ({})
    // What sites notified, kept for the Porch.
    property var siteNotices: JSON.parse(page.sioul.siteNotices() || "[]")
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
        if (page.visible)
            page.reloadDoses()
    }
    onViewChanged: page.reloadDoses()
    Component.onCompleted: {
        page.takeShown()
        page.reloadDoses()
    }

    Connections {
        target: page.window

        function onNowChanged() {
            if (page.visible)
                page.reloadDoses()
        }
    }

    Connections {
        target: page.sioul

        function onSitesChanged() {
            page.siteNotices = JSON.parse(page.sioul.siteNotices() || "[]")
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
    // Lanes folded or unfolded by hand, by key; the others keep their default.
    property var unfolded: ({})

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

    function grabSettings(path) {
        settingsButton.grab(path)
    }

    // The first message of the first lane, for the window's images.
    function openFirst() {
        if (view.lanes.length > 0 && view.lanes[0].items.length > 0)
            page.openKey = view.lanes[0].items[0].key
    }

    function isFolded(lane) {
        return page.unfolded[lane.key] === undefined ? lane.folded : !page.unfolded[lane.key]
    }

    function toggle(lane) {
        const next = Object.assign({}, page.unfolded)
        next[lane.key] = isFolded(lane)
        page.unfolded = next
    }

    function trustIcon(level) {
        return level === "verified" ? "security-high" : level === "forged" ? "security-low" : "security-medium"
    }

    function toggleHelp(key) {
        const next = Object.assign({}, page.helpShown)
        next[key] = !next[key]
        page.helpShown = next
    }

    // A small number in words ("three"), as the translations write them,
    // digits beyond twelve: no counts that read as badges (docs/design.md).
    function countWords(n) {
        const id = "count-" + n
        const words = page.sioul.text(id)
        return words === id ? String(n) : words
    }

    function laneIcon(key) {
        if (key.startsWith("case:"))
            return "folder-documents"
        if (key.startsWith("public:"))
            return "mail-message"
        return { "people": "user-identity", "screener": "contact-new", "filed": "folder-mail", "low": "go-bottom", "set-aside": "mail-mark-junk", "hostile": "dialog-cancel" }[key] || "mail-message"
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

                // Where the Porch stands, in a line; real time; its settings.
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
                    CheckBox {
                        id: realtime

                        text: page.sioul.text("ui-realtime")
                        checked: page.sioul.realtime
                        ToolTip.visible: hovered
                        ToolTip.text: page.sioul.text("ui-realtime-help")
                        ToolTip.delay: 400
                        onToggled: {
                            page.sioul.setRealtimeMode(checked)
                            realtime.checked = Qt.binding(() => page.sioul.realtime)
                        }
                    }
                    // Work shown whatever the hours: while work rests, or once ticked.
                    // Not during a meal or sleep: they come first, whatever is ticked.
                    CheckBox {
                        id: workNow

                        visible: (page.window.moment.quiet && page.window.moment.time !== "meals" && page.window.moment.time !== "sleep") || page.window.moment.work_now === true
                        text: page.sioul.text("mode-work-now")
                        checked: page.window.moment.work_now === true
                        ToolTip.visible: hovered
                        ToolTip.text: page.sioul.text("mode-work-now-help")
                        ToolTip.delay: 400
                        onToggled: {
                            page.sioul.setWorkNow(checked)
                            workNow.checked = Qt.binding(() => page.window.moment.work_now === true)
                        }
                    }
                    SettingsButton {
                        id: settingsButton

                        sioul: page.sioul
                        theme: page.theme
                        view: "porch"
                    }
                }

                // Opened: what came, in a few sentences, before the cards (docs/design.md, "Opening").
                Label {
                    visible: page.view.open && page.view.summary !== ""
                    Layout.fillWidth: true
                    text: page.view.summary
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 17
                    lineHeight: 1.3
                    color: page.theme.text
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

                // Where you stopped, when something interrupted you: first, until it is done.
                StoppedCard {
                    Layout.fillWidth: true
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                }

                // Two events at once today, the time to get there and back counted:
                // said here until you open one to move it, or say to leave it.
                Panel {
                    id: overlapsCard

                    // Made with the agenda, off the window's thread.
                    readonly property var rows: JSON.parse(page.sioul.overlaps || "[]").filter(o => o.today)

                    visible: overlapsCard.rows.length > 0
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        Repeater {
                            model: overlapsCard.rows

                            delegate: ColumnLayout {
                                id: overlap

                                required property var modelData

                                Layout.fillWidth: true
                                spacing: 4

                                Label {
                                    Layout.fillWidth: true
                                    text: page.sioul.textArgs("overlap-today", JSON.stringify({ first: overlap.modelData.first.title + " (" + overlap.modelData.first.from + "–" + overlap.modelData.first.to + ")", second: overlap.modelData.second.title + " (" + overlap.modelData.second.from + "–" + overlap.modelData.second.to + ")" }))
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                                Flow {
                                    Layout.fillWidth: true
                                    spacing: 6

                                    Repeater {
                                        model: [overlap.modelData.first, overlap.modelData.second]

                                        delegate: Button {
                                            id: openOne

                                            required property var modelData

                                            text: page.sioul.textWith("overlap-open", "title", openOne.modelData.title)
                                            onClicked: page.window.openThing({ kind: "event", key: openOne.modelData.key, uri: "" })
                                        }
                                    }
                                    Button {
                                        flat: true
                                        text: page.sioul.text("overlap-set-aside")
                                        onClicked: page.sioul.setOverlapAside(overlap.modelData.key)
                                    }
                                }
                            }
                        }
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
                                            if (dueDose.modelData.choose) {
                                                page.sioul.doseChoose(dueDose.modelData.key, true)
                                                page.reloadDoses()
                                                return
                                            }
                                            if (dueDose.modelData.late) {
                                                page.window.askDose(dueDose.modelData.key)
                                                return
                                            }
                                            page.sioul.setDoseTaken(dueDose.modelData.key, true)
                                            page.reloadDoses()
                                        }
                                    }
                                    Button {
                                        visible: dueDose.modelData.choose === true
                                        flat: true
                                        text: page.sioul.text("health-not-taken")
                                        onClicked: {
                                            page.sioul.doseChoose(dueDose.modelData.key, false)
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

                // Doses due while Sioul was closed on all your computers: a question on the
                // past, answered once (when it was taken, in DoseTaken.qml); never a reminder.
                Panel {
                    visible: page.missedDoses.length > 0
                    Layout.fillWidth: true
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 6

                        Label {
                            Layout.fillWidth: true
                            text: page.sioul.text("health-missed-question")
                            wrapMode: Text.Wrap
                            font.weight: Font.DemiBold
                            color: page.theme.text
                        }
                        Repeater {
                            model: page.missedDoses

                            // The buttons beside the dose, or under it on a phone.
                            delegate: GridLayout {
                                id: missedDose

                                required property var modelData

                                Layout.fillWidth: true
                                columns: page.width < 600 ? 2 : 3
                                columnSpacing: 12
                                rowSpacing: 2

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

                // The calls Sioul declined on this phone, each at a time its caller may
                // reach you, whatever the Porch's own hours (CallsSection.qml, docs/porch.md).
                CallsSection {
                    Layout.fillWidth: true
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                }

                // Money, in the window: the week's payments, and whether the account holds them.
                RowLayout {
                    id: money

                    property var bank: null

                    // Asleep, none: no money at night.
                    visible: page.view.open && page.window.moment.sleep !== true && money.bank !== null && (money.bank.week !== "" || money.bank.attention > 0)
                    Layout.fillWidth: true
                    spacing: 8

                    Component.onCompleted: money.bank = JSON.parse(page.sioul.bank())
                    Connections {
                        target: page

                        function onVisibleChanged() {
                            if (page.visible)
                                money.bank = JSON.parse(page.sioul.bank())
                        }
                    }

                    Label {
                        Layout.fillWidth: true
                        text: money.bank === null ? "" : [money.bank.week, money.bank.attention > 0 ? page.sioul.textArgs("bank-attention", JSON.stringify({ count: money.bank.attention })) : ""].filter(t => t !== "").join(" ")
                        wrapMode: Text.Wrap
                        color: page.theme.muted
                    }
                    Button {
                        visible: money.bank !== null && money.bank.attention > 0
                        flat: true
                        text: page.sioul.text("ui-budgets")
                        onClicked: page.window.page = 9
                    }
                }

                // Paper letters, read from their scans: in the window, like mail.
                LettersSection {
                    id: letters

                    visible: page.view.open && page.window.moment.sleep !== true && (letters.shown.letters.length > 0 || letters.shown.missing !== "")
                    Layout.fillWidth: true
                    sioul: page.sioul
                    theme: page.theme
                    window: page.window
                }

                // Right now: the one exception to the windows.
                Repeater {
                    model: page.view.right_now

                    delegate: Panel {
                        id: codeCard

                        required property var modelData

                        Layout.fillWidth: true
                        theme: page.theme
                        accent: true

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 8

                            RowLayout {
                                spacing: 8

                                Icon {
                                    iconName: "dialog-password"
                                }
                                // What mail and sites say is plain text: a "<" opens no tag, loads nothing.
                                Label {
                                    Layout.fillWidth: true
                                    text: codeCard.modelData.title
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                            }
                            RowLayout {
                                visible: !!codeCard.modelData.code
                                spacing: page.theme.gap

                                Label {
                                    text: codeCard.modelData.code || ""
                                    textFormat: Text.PlainText
                                    font.family: page.theme.mono
                                    font.pixelSize: 30
                                    font.letterSpacing: 3
                                    color: page.theme.text
                                }
                                Button {
                                    id: copyButton

                                    property bool done: false

                                    text: copyButton.done ? page.sioul.text("ui-copied") : page.sioul.text("ui-copy")
                                    onClicked: {
                                        page.window.copy(codeCard.modelData.code)
                                        copyButton.done = true
                                    }
                                }
                            }
                            Label {
                                visible: !!codeCard.modelData.validity
                                text: codeCard.modelData.validity || ""
                                color: page.theme.muted
                            }
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
                        text: page.sioul.text("ui-open-anyway")
                        onClicked: page.sioul.openAnyway()
                    }
                }

                // What your sites notified while you were away, each site with its lines.
                Repeater {
                    model: page.view.open ? page.siteNotices : []

                    delegate: Panel {
                        id: siteNotice

                        required property var modelData

                        Layout.fillWidth: true
                        theme: page.theme

                        ColumnLayout {
                            anchors.fill: parent
                            spacing: 4

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                Icon {
                                    iconName: "internet-web-browser"
                                    size: 16
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: page.sioul.textWith("site-notified", "site", siteNotice.modelData.name)
                                    textFormat: Text.PlainText
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                    color: page.theme.text
                                }
                                Button {
                                    text: page.sioul.text("ui-open")
                                    onClicked: page.window.openSite(siteNotice.modelData.id)
                                }
                            }
                            Repeater {
                                model: siteNotice.modelData.items.slice(-4)

                                delegate: Label {
                                    required property string modelData

                                    Layout.fillWidth: true
                                    Layout.leftMargin: 24
                                    text: modelData
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    color: page.theme.text
                                }
                            }
                            Label {
                                visible: siteNotice.modelData.items.length > 4
                                Layout.leftMargin: 24
                                text: page.sioul.textWith("site-notified-more", "n", String(siteNotice.modelData.items.length - 4))
                                font.pixelSize: 12
                                color: page.theme.muted
                            }
                        }
                    }
                }

                // Open: the lanes, each saying what it holds.
                Repeater {
                    model: page.view.open ? page.view.lanes : []

                    delegate: ColumnLayout {
                        id: lane

                        required property var modelData
                        readonly property bool folded: page.isFolded(modelData)

                        Layout.fillWidth: true
                        spacing: 2

                        // The lane's title folds and unfolds it; its "?" says how mail lands
                        // here and holds what changes it. The title fills the row and elides.
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 2

                            ItemDelegate {
                                id: laneTitle

                                Layout.fillWidth: true
                                leftPadding: 8
                                rightPadding: 8
                                onClicked: page.toggle(lane.modelData)
                                Accessible.name: lane.modelData.title

                                background: Rectangle {
                                    color: laneTitle.hovered ? page.theme.surface : "transparent"
                                    radius: page.theme.radius
                                    border.color: laneTitle.visualFocus ? page.theme.focus : "transparent"
                                    border.width: 2
                                }
                                contentItem: RowLayout {
                                    spacing: 8

                                    // The caret, as wide as the shields below it, which line up under it.
                                    Label {
                                        Layout.preferredWidth: 16
                                        horizontalAlignment: Text.AlignHCenter
                                        text: lane.folded ? "▸" : "▾"
                                        color: page.theme.text
                                    }
                                    // In the text's colour: Breeze draws "set aside" with a red sign.
                                    Icon {
                                        iconName: page.laneIcon(lane.modelData.key)
                                        color: page.theme.text
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: lane.modelData.title
                                        textFormat: Text.PlainText
                                        font.pixelSize: 17
                                        font.weight: Font.DemiBold
                                        elide: Text.ElideRight
                                        color: page.theme.text
                                    }
                                    // How many, in words while few.
                                    Label {
                                        text: page.countWords(lane.modelData.items.length)
                                        color: page.theme.muted
                                    }
                                }
                            }
                            ToolButton {
                                text: "?"
                                checkable: true
                                checked: page.helpShown[lane.modelData.key] === true
                                Accessible.name: page.sioul.text("ui-lane-help")
                                ToolTip.visible: hovered
                                ToolTip.text: page.sioul.text("ui-lane-help")
                                ToolTip.delay: 300
                                onClicked: page.toggleHelp(lane.modelData.key)
                            }
                        }
                        // What it holds, under its title.
                        Label {
                            Layout.fillWidth: true
                            Layout.leftMargin: 8 + 16 + 8
                            text: lane.modelData.about
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 13
                            color: page.theme.muted
                        }
                        // How mail lands here, and what changes it: made while shown, its
                        // settings read again each time. Each setting (SettingRow.qml) is
                        // read by its file, which the Porch then does not load at its start.
                        Loader {
                            id: help

                            readonly property bool shown: page.helpShown[lane.modelData.key] === true

                            visible: help.shown
                            active: help.shown
                            Layout.fillWidth: true
                            Layout.leftMargin: 8
                            Layout.topMargin: 4
                            Layout.bottomMargin: 4

                            sourceComponent: Panel {
                                id: rules

                                property var rows: JSON.parse(page.sioul.settings(lane.modelData.settings))

                                theme: page.theme

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 8

                                    Repeater {
                                        model: lane.modelData.rules

                                        delegate: Label {
                                            required property string modelData

                                            Layout.fillWidth: true
                                            text: modelData
                                            wrapMode: Text.Wrap
                                            lineHeight: 1.25
                                            color: page.theme.text
                                        }
                                    }
                                    Repeater {
                                        model: rules.rows

                                        delegate: Loader {
                                            id: ruleRow

                                            required property var modelData

                                            Layout.fillWidth: true
                                            Layout.topMargin: 6
                                            Component.onCompleted: ruleRow.setSource("SettingRow.qml", { setting: ruleRow.modelData, sioul: page.sioul, theme: page.theme })

                                            Connections {
                                                target: ruleRow.item

                                                function onSave(key, value) {
                                                    const problem = page.sioul.setSetting(key, JSON.stringify(value))
                                                    if (problem === "")
                                                        rules.rows = JSON.parse(page.sioul.settings(lane.modelData.settings))
                                                    else
                                                        page.sioul.status = problem
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        Repeater {
                            model: lane.folded ? [] : lane.modelData.items

                            delegate: ItemDelegate {
                                id: row

                                required property var modelData

                                Layout.fillWidth: true
                                // Under its lane: the shield beneath the lane's icon, the sender beneath its title.
                                Layout.leftMargin: 16 + 8
                                highlighted: page.sameMessage(page.openKey, modelData.key)
                                // Its words for screen readers: the row has no text of its own.
                                Accessible.name: [row.modelData.sender, row.modelData.subject, row.modelData.date].filter(t => !!t).join(", ")
                                leftPadding: 8
                                rightPadding: 10
                                topPadding: 8
                                bottomPadding: 8
                                onClicked: page.openKey = modelData.key
                                Keys.onReturnPressed: page.openKey = modelData.key
                                Keys.onEnterPressed: page.openKey = modelData.key

                                // On a touch screen, the menu comes at a long press; letting go then opens nothing.
                                onPressAndHold: {
                                    row.Window.window.menuAt = row.mapToItem(null, row.pressX, row.pressY)
                                    itemMenu.show(row.modelData)
                                }
                                TapHandler {
                                    acceptedButtons: Qt.RightButton
                                    // A touch has no buttons: on a touch screen, the row's long press.
                                    acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                                    onTapped: itemMenu.show(row.modelData)
                                }

                                background: Rectangle {
                                    color: row.highlighted || row.hovered ? page.theme.surface : "transparent"
                                    radius: page.theme.radius
                                    border.color: row.visualFocus ? page.theme.focus : row.highlighted ? page.theme.line : "transparent"
                                    border.width: row.visualFocus ? 2 : 1
                                }

                                contentItem: ColumnLayout {
                                    spacing: 3

                                    RowLayout {
                                        spacing: 8

                                        Icon {
                                            iconName: page.trustIcon(row.modelData.trust_level)
                                            size: 16
                                            tip: row.modelData.checks
                                        }
                                        Label {
                                            Layout.fillWidth: true
                                            text: row.modelData.sender
                                            textFormat: Text.PlainText
                                            font.weight: Font.DemiBold
                                            elide: Text.ElideRight
                                            color: page.theme.text
                                        }
                                        Icon {
                                            visible: row.modelData.attachments.length > 0
                                            iconName: "mail-attachment"
                                            size: 16
                                        }
                                        Label {
                                            text: row.modelData.date
                                            color: page.theme.muted
                                            font.pixelSize: 13
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        Layout.leftMargin: 16 + 8
                                        spacing: 6

                                        Label {
                                            Layout.fillWidth: true
                                            text: row.modelData.subject
                                            textFormat: Text.PlainText
                                            elide: Text.ElideRight
                                            font.italic: row.modelData.hidden
                                            color: row.modelData.hidden ? page.theme.muted : page.theme.text
                                        }
                                        // In words, quiet: an account ranked more important (no
                                        // orange mark); a public address's topic, and a rude tone.
                                        Repeater {
                                            model: [row.modelData.important ? page.sioul.text("priority-above") : "", row.modelData.topic, row.modelData.tone === "rude" ? page.sioul.text("tone-rude") : ""].filter(t => !!t)

                                            delegate: Rectangle {
                                                id: chip

                                                required property string modelData

                                                implicitWidth: chipText.implicitWidth + 12
                                                implicitHeight: chipText.implicitHeight + 4
                                                radius: height / 2
                                                color: "transparent"
                                                border.color: page.theme.line

                                                Label {
                                                    id: chipText

                                                    anchors.centerIn: parent
                                                    text: chip.modelData
                                                    textFormat: Text.PlainText
                                                    font.pixelSize: 11
                                                    color: page.theme.muted
                                                }
                                            }
                                        }
                                    }
                                    Label {
                                        visible: row.modelData.summary !== ""
                                        Layout.fillWidth: true
                                        Layout.leftMargin: 16 + 8
                                        text: row.modelData.summary
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        font.pixelSize: 13
                                        color: page.theme.muted
                                    }
                                }
                            }
                        }
                    }
                }

                Button {
                    visible: page.view.open && page.view.lanes.length > 0
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

            visible: page.opened !== null && reader.item !== null && reader.item.reading !== null
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

    // Right click on a message: something new tied to it, or a tie to something
    // that exists. Made the first time it is asked for.
    Loader {
        id: itemMenu

        function show(item) {
            itemMenu.active = true
            itemMenu.item.show(item)
        }

        active: false
        sourceComponent: SioulMenu {
            id: menu

            property var source: null

            function show(item) {
                menu.source = { uri: page.sioul.uriOf("mail", item.key), kind: "mail", key: item.key, title: item.subject, name: item.sender, address: item.hidden ? "" : item.address, known: !item.screener }
                menu.popup()
            }

            AddMenu {
                sioul: page.sioul
                window: page.window
                source: menu.source
            }
            MenuItem {
                enabled: menu.source !== null && menu.source.uri !== ""
                text: page.sioul.text("ui-link-existing")
                onTriggered: page.window.linkFrom(menu.source)
            }
        }
    }
}
