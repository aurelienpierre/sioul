// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A date asked's card, where you are (the unified agenda, part 4): a
// letter's (its date asked, its appointment), a contract's last day to give
// notice, a paper's last valid day, a planned payment, an invoice you
// issued, a prescription's dates. Read by its kind and key
// (`datedCard(kind, key)`), in the words its own page says; its mark filled
// within seven days. Edit opens that thing's own form: a letter's dates while
// it has neither its task nor its event (then theirs), the paper's, the
// contract's, the budget line's; an invoice is its own record, said so; a
// prescription's form is Health's. Its own actions beside: See the scan, its
// task, Into the agenda, Done; Plan the renewal; Paid; Fetched today.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CardFrame {
    id: card

    required property var window
    // Its kind as `dated::Kind` writes it ("letter", "notice"…) and its key.
    property string kind: ""
    property string key: ""
    // What `datedCard` read: {there, kind, key, title, mark, when, filled, lines, edit, item, …}.
    property var shown: null
    readonly property bool there: card.shown !== null && card.shown.there === true
    property string problem: ""

    function read() {
        card.problem = ""
        card.shown = card.kind !== "" && card.key !== "" ? JSON.parse(card.sioul.datedCard(card.kind, card.key) || "{}") : null
        if (card.there)
            card.host.named("", card.shown.title)
    }

    // Written: read again, and the agenda's marks follow.
    function written(problem) {
        card.problem = problem || ""
        if (card.problem === "") {
            card.read()
            card.window.agendaAgain()
        }
    }

    onKindChanged: Qt.callLater(card.read)
    onKeyChanged: Qt.callLater(card.read)
    Component.onCompleted: card.read()

    Connections {
        target: card.sioul

        function onAgendaChanged() {
            if (card.visible)
                card.read()
        }
    }

    function edit() {
        if (!card.there)
            return
        const s = card.shown
        if (s.edit === "letter-dates")
            letterForm.now().edit(s.key, s.item)
        else if (s.edit === "task" && s.task)
            card.host.follow({ kind: "task", owner: s.task })
        else if (s.edit === "event" && s.event)
            card.host.follow({ kind: "event", srcKey: s.event.key, srcStart: s.event.start })
        else if (s.edit === "paper")
            paperForm.now().edit(s.item)
        else if (s.edit === "contract")
            contractForm.now().edit(s.item)
        else if (s.edit === "line")
            lineForm.now().begin(s.item)
        else if (s.edit === "health")
            card.window.openThing({ kind: "health", uri: "", key: "" })
    }

    iconName: "milestone"
    dot: card.theme.accent
    title: card.there ? card.shown.title : card.sioul.text("card-gone")
    when: card.there ? card.shown.when : ""
    editable: card.there && card.shown.edit !== "none"
    why: card.there ? card.shown.why || "" : ""
    onEditAsked: card.edit()

    // Its mark and its words: filled within seven days, outlined beyond.
    RowLayout {
        visible: card.there && card.shown.mark !== ""
        Layout.fillWidth: true
        spacing: 8

        Rectangle {
            objectName: "datedMark"
            Layout.alignment: Qt.AlignVCenter
            implicitWidth: 9
            implicitHeight: 9
            rotation: 45
            color: card.there && card.shown.filled ? card.theme.text : "transparent"
            border.color: card.theme.text
            border.width: 1
        }
        Label {
            Layout.fillWidth: true
            text: card.there ? card.shown.mark : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: card.theme.text
        }
    }
    Repeater {
        model: card.there ? card.shown.lines || [] : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: card.theme.text
        }
    }
    // A letter's dates are read from its scan: they count until you correct them.
    Label {
        visible: card.there && card.shown.read === true && card.shown.edit === "letter-dates"
        Layout.fillWidth: true
        text: card.sioul.text("dated-letter-dates-why")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: card.theme.muted
    }
    Label {
        visible: card.problem !== ""
        Layout.fillWidth: true
        text: card.problem
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: card.theme.text
    }

    actions: [
        // A letter: its scan, its task, its appointment, done.
        Button {
            visible: card.there && (card.kind === "letter" || card.kind === "letter-appointment") && (card.shown.file || "") !== ""
            flat: true
            text: card.sioul.text("letters-scan")
            onClicked: Qt.openUrlExternally(card.theme.fileUrl(card.shown.file))
        },
        Button {
            objectName: "datedLetterTask"
            visible: card.there && (card.kind === "letter" || card.kind === "letter-appointment") && (card.shown.can_task || (card.shown.task || "") !== "")
            flat: true
            text: card.sioul.text(card.there && (card.shown.task || "") !== "" ? "letters-task-made" : "letters-task")
            onClicked: {
                if ((card.shown.task || "") !== "")
                    card.host.follow({ kind: "task", owner: card.shown.task })
                else
                    card.written(card.sioul.letterTask(card.key))
            }
        },
        Button {
            objectName: "datedLetterEvent"
            visible: card.there && (card.kind === "letter" || card.kind === "letter-appointment") && card.shown.into_agenda === true
            flat: true
            text: card.sioul.text("letters-event")
            onClicked: card.written(card.sioul.letterEvent(card.key))
        },
        Button {
            visible: card.there && (card.kind === "letter" || card.kind === "letter-appointment")
            flat: true
            text: card.sioul.text("letters-done")
            onClicked: {
                const problem = card.sioul.letterDone(card.key, card.shown.project || "")
                card.problem = problem
                if (problem === "") {
                    card.window.agendaAgain()
                    card.host.close()
                }
            }
        },
        // A paper: its renewal, its file.
        Button {
            visible: card.there && card.kind === "paper" && (card.shown.can_renew === true || (card.shown.renewal || "") !== "")
            enabled: card.there && card.shown.can_renew === true
            flat: true
            text: card.sioul.text(card.there && (card.shown.renewal || "") !== "" ? "papers-renewal-planned" : "papers-plan-renewal")
            onClicked: card.written(card.sioul.planRenewal(card.key))
        },
        Button {
            visible: card.there && card.kind === "paper" && card.shown.file_there === true
            flat: true
            text: card.sioul.text("ui-open")
            onClicked: Qt.openUrlExternally(card.theme.fileUrl(card.shown.file))
        },
        // An invoice: paid, with Undo in the status line.
        Button {
            visible: card.there && card.kind === "invoice" && card.shown.paid !== true
            flat: true
            text: card.sioul.text("dated-paid")
            onClicked: card.written(card.sioul.setInvoicePaid(card.shown.number, true))
        },
        // A prescription: fetched today; its form is Health's.
        Button {
            visible: card.there && (card.kind === "prescription-ends" || card.kind === "runs-out") && card.shown.fetched_today !== true
            flat: true
            text: card.sioul.text("health-refilled")
            onClicked: card.written(card.sioul.refilled(card.key))
        },
        // A contract's or a payment's page: Stop it…, its budget.
        Button {
            visible: card.there && (card.kind === "notice" || card.kind === "payment")
            flat: true
            text: card.sioul.text("dated-open-page")
            onClicked: card.window.openPage(card.kind === "notice" ? "contract" : "budget", card.key)
        }
    ]

    Later {
        id: letterForm

        sourceComponent: Component {
            LetterDatesForm {
                sioul: card.sioul
                theme: card.theme
                onSaved: card.written("")
            }
        }
    }
    Later {
        id: paperForm

        sourceComponent: Component {
            PaperDialog {
                sioul: card.sioul
                theme: card.theme
                window: card.window
                kinds: card.there ? card.shown.kinds || [] : []
                onSaved: card.written("")
            }
        }
    }
    Later {
        id: contractForm

        sourceComponent: Component {
            ContractDialog {
                sioul: card.sioul
                theme: card.theme
                window: card.window
                kinds: card.there ? card.shown.kinds || [] : []
                presets: card.there ? card.shown.presets || [] : []
                onSaved: card.written("")
            }
        }
    }
    Later {
        id: lineForm

        sourceComponent: Component {
            LineDialog {
                sioul: card.sioul
                theme: card.theme
                window: card.window
                onChanged: card.written("")
            }
        }
    }
}
