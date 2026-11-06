// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Agenda ⚙ ▸ Your availability (design-availability-sharing §4; docs/client.md,
// "Sharing your availability"): what you give of your time, and to whom.
// First, for each Nextcloud, what its users see without being asked, and
// whether your working hours go there. Then one card per availability: its
// name, its agendas (with their words), one line per share saying who sees
// what and who keeps it up to date, its link with Copy, Open for a booking
// page, and Stop (with Undo). "Share with…" asks only who: a person, a group,
// anyone with a link, a booking tool; Sioul names the way it will use and
// why (its server's own first), and Share makes it. "New availability…"
// gives a name and picks the agendas, none ticked. Every change is the
// window's (`servers`); the answers of the servers come as
// `agendaShared("availability", …)`.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: avail

    required property var sioul
    required property var theme
    // The row's sentence, said once above.
    property string about: ""
    property var view: ({ head: [], items: [], agendas: [], accounts: [] })
    property string viewText: ""
    property string problem: ""
    // The card whose "Share with…" is open, by id; "" none.
    property string asking: ""
    // "New availability…" open.
    property bool making: false

    readonly property var audiences: ["person", "group", "link", "booking"]

    // Read again; `kept`, the same answer changes nothing (the open chooser stays).
    function read(kept) {
        const text = avail.sioul.availability() || "{}"
        if (kept === true && text === avail.viewText)
            return
        avail.viewText = text
        try {
            avail.view = JSON.parse(text)
        } catch (e) {
            avail.view = { head: [], items: [], agendas: [], accounts: [] }
        }
    }

    // A server's own page for a way you set up there: Nextcloud's Calendar
    // (Appointments), Google's calendar (appointment schedules), Fastmail's.
    function pageOf(way, account) {
        if (way === "google-schedule")
            return "https://calendar.google.com/calendar/"
        if (way === "fastmail-link")
            return "https://app.fastmail.com/calendar/"
        const server = (avail.view.head || []).find(h => h.account === account)
        return way === "appointments" && server ? "https://" + server.server + "/index.php/apps/calendar/" : ""
    }

    Connections {
        target: avail.sioul

        function onAgendasChanged() {
            avail.read(true)
        }

        function onAgendaShared(id, answer) {
            if (id === "availability")
                avail.read(true)
        }
    }

    objectName: "availabilitySetup"
    spacing: 6
    Component.onCompleted: avail.read()

    Label {
        visible: avail.about !== ""
        Layout.fillWidth: true
        text: avail.about
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: avail.theme.muted
    }

    // What each Nextcloud shows without being asked, and your working hours there.
    Repeater {
        model: avail.view.head || []

        delegate: ColumnLayout {
            id: server

            required property var modelData

            Layout.fillWidth: true
            spacing: 2

            Label {
                objectName: "availabilityServerLine"
                Layout.fillWidth: true
                text: server.modelData.line
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: avail.theme.text
            }
            WrapCheckBox {
                objectName: "availabilityHours"
                Layout.fillWidth: true
                text: avail.sioul.textWith("availability-hours", "server", server.modelData.server)
                checked: server.modelData.hours === true
                ToolTip.visible: hovered && server.modelData.propose === true && !checked
                ToolTip.text: avail.sioul.text("availability-hours-proposed")
                onToggled: {
                    avail.problem = avail.sioul.availabilityHours(server.modelData.account, checked)
                    avail.read()
                }
            }
        }
    }

    // One card per availability.
    Repeater {
        model: avail.view.items || []

        delegate: ColumnLayout {
            id: card

            required property var modelData
            readonly property bool open: avail.asking === card.modelData.id

            objectName: "availabilityCard"
            Layout.fillWidth: true
            Layout.topMargin: 6
            spacing: 4

            Label {
                objectName: "availabilityName"
                Layout.fillWidth: true
                text: avail.theme.plain(card.modelData.name)
                font.bold: true
                elide: Text.ElideRight
                color: avail.theme.text
            }
            Flow {
                Layout.fillWidth: true
                spacing: 4

                Repeater {
                    model: card.modelData.agendas || []

                    delegate: Label {
                        id: chip

                        required property var modelData
                        // A word that is the agenda's own name is said once.
                        readonly property string word: {
                            const said = (card.modelData.words || {})[chip.modelData.id] || ""
                            return said.trim().toLowerCase() === String(chip.modelData.name).trim().toLowerCase() ? "" : said
                        }

                        objectName: "availabilityChip"
                        leftPadding: 8
                        rightPadding: 8
                        topPadding: 2
                        bottomPadding: 2
                        text: avail.theme.plain(chip.modelData.name) + (chip.word !== "" ? " · " + avail.theme.plain(chip.word) : "")
                        textFormat: Text.PlainText
                        color: avail.theme.text
                        background: Rectangle {
                            radius: avail.theme.radius
                            color: avail.theme.button
                        }
                    }
                }
            }
            Repeater {
                model: card.modelData.shares || []

                delegate: ColumnLayout {
                    id: share

                    required property var modelData
                    readonly property bool page: ["appointments", "google-schedule"].indexOf(share.modelData.way) >= 0

                    objectName: "availabilityShareRow"
                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        objectName: "availabilityShareLine"
                        Layout.fillWidth: true
                        text: share.modelData.line
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: avail.theme.text
                    }
                    Label {
                        objectName: "availabilityKeeper"
                        Layout.fillWidth: true
                        text: share.modelData.keeper
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: avail.theme.muted
                    }
                    LinkLine {
                        objectName: "availabilityLink"
                        visible: share.modelData.link !== ""
                        Layout.fillWidth: true
                        sioul: avail.sioul
                        theme: avail.theme
                        link: share.modelData.link
                    }
                    RowLayout {
                        spacing: 6

                        Button {
                            objectName: "availabilityOpen"
                            visible: share.page && share.modelData.link !== ""
                            text: avail.sioul.text("ui-open")
                            onClicked: Qt.openUrlExternally(share.modelData.link)
                        }
                        Button {
                            objectName: "availabilityStop"
                            text: avail.sioul.text("availability-stop")
                            onClicked: {
                                avail.problem = avail.sioul.stopAvailability(JSON.stringify({ id: card.modelData.id, way: share.modelData.way, account: share.modelData.account, who: share.modelData.who }))
                            }
                        }
                    }
                }
            }
            Button {
                objectName: "availabilityShareWith"
                visible: !card.open
                text: avail.sioul.text("availability-share-with")
                onClicked: avail.asking = card.modelData.id
            }
            Loader {
                active: card.open
                visible: active
                Layout.fillWidth: true

                sourceComponent: Component {
                    AvailabilityChooser {
                        sioul: avail.sioul
                        theme: avail.theme
                        availability: card.modelData.id
                        pageOf: avail.pageOf
                        onDone: shared => {
                            avail.asking = ""
                            avail.read()
                        }
                    }
                }
            }
        }
    }

    // A new availability: a name, and the agendas of yours, none ticked.
    Button {
        objectName: "availabilityNew"
        visible: !avail.making
        Layout.topMargin: 6
        text: avail.sioul.text("availability-new")
        onClicked: avail.making = true
    }
    ColumnLayout {
        id: form

        property var picked: ({})
        property var words: ({})

        visible: avail.making
        Layout.fillWidth: true
        spacing: 4

        TextField {
            id: formName

            objectName: "availabilityNewName"
            Layout.fillWidth: true
            placeholderText: avail.sioul.text("availability-name")
            Accessible.name: placeholderText
        }
        Repeater {
            model: avail.view.agendas || []

            delegate: RowLayout {
                id: source

                required property var modelData

                Layout.fillWidth: true
                spacing: 6

                WrapCheckBox {
                    objectName: "availabilityAgenda"
                    Layout.fillWidth: true
                    Layout.preferredWidth: 0
                    text: avail.theme.plain(source.modelData.name)
                    checked: form.picked[source.modelData.id] === true
                    onToggled: {
                        const next = Object.assign({}, form.picked)
                        next[source.modelData.id] = checked
                        form.picked = next
                    }
                }
                TextField {
                    objectName: "availabilityWord"
                    visible: form.picked[source.modelData.id] === true
                    Layout.preferredWidth: 120
                    placeholderText: avail.sioul.text("availability-word")
                    Accessible.name: placeholderText
                    onTextEdited: {
                        const next = Object.assign({}, form.words)
                        next[source.modelData.id] = text
                        form.words = next
                    }
                }
            }
        }
        RowLayout {
            spacing: 8

            Button {
                objectName: "availabilitySave"
                text: avail.sioul.text("ui-save")
                onClicked: {
                    const agendas = Object.keys(form.picked).filter(k => form.picked[k])
                    const answer = JSON.parse(avail.sioul.saveAvailability(JSON.stringify({ name: formName.text.trim(), agendas: agendas, words: form.words })) || "{}")
                    avail.problem = answer.problem || ""
                    if (avail.problem === "") {
                        avail.making = false
                        form.picked = {}
                        form.words = {}
                        formName.text = ""
                        avail.read()
                        avail.asking = answer.id || ""
                    }
                }
            }
            Button {
                flat: true
                text: avail.sioul.text("ui-cancel")
                onClicked: avail.making = false
            }
        }
    }
    Label {
        objectName: "availabilityProblem"
        visible: avail.problem !== ""
        Layout.fillWidth: true
        text: avail.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: avail.theme.warm
    }
}
