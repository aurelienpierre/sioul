// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A call's card (docs/porch.md, "A call's card"): a call of one of your
// phones as an object of its own, read by its address (`callCard(address)`,
// calls.rs `card`). Its head says what it was ("Missed call from Noa
// Linden") and when; its first section the call itself, the most useful
// first: who, their number, when, how long, which way, answered or not, on
// which phone, and what the screening did when it screened it; its second
// what it is tied to, as every card lists its ties (RelatedList.qml): the
// card of whom it was with, the task it made, the call that closed it.
// Opened from the Porch's row of calls, a person's sheet, and any tie. Read
// only: a call is what the phone says it was, never written here.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

CardFrame {
    id: card

    objectName: "callCard"

    required property var window
    // The call's address, `sioul:call/<phone>:<start>-<in|out>`.
    property string address: ""
    // What `callCard` read: {there, address, title, when, fields: [label, value, label, value, …]}; {there: false, gone}.
    property var shown: null
    readonly property bool there: card.shown !== null && card.shown.there === true

    // Read from what was just given, never from the card's properties: on a phone the card may be
    // let go (another card opened) between the call and this read, which comes a moment later.
    function read() {
        const shown = card.address !== "" ? JSON.parse(card.sioul.callCard(card.address) || "{}") : null
        card.shown = shown
        tiesAgain.tries = 0
        if (shown && shown.there === true && card.host)
            card.host.named("", shown.title || "")
    }

    onAddressChanged: Qt.callLater(card.read)
    Component.onCompleted: card.read()

    // A call's ties are the tie index's alone, which the window makes on a thread of its own the
    // first time it is asked: none yet, they are read again a moment later, three times at most.
    Timer {
        id: tiesAgain

        property int tries: 0

        interval: 1500
        repeat: true
        running: card.there && ties.items.length === 0 && tiesAgain.tries < 3
        onTriggered: {
            tiesAgain.tries++
            ties.reload()
        }
    }

    iconName: "call-start"
    title: card.there ? card.shown.title : card.sioul.text("call-card-gone-title")
    when: card.there ? card.shown.when : ""
    editable: false
    why: card.there ? card.sioul.text("call-card-why") : ""

    // Gone: its phone keeps its calls a month.
    Label {
        objectName: "callGone"
        visible: card.shown !== null && !card.there
        Layout.fillWidth: true
        text: card.shown && card.shown.gone ? card.shown.gone : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: card.theme.muted
    }

    // The call: each field's label, then its value standing out.
    Section {
        objectName: "callFields"
        visible: card.there
        Layout.fillWidth: true
        theme: card.theme
        title: card.sioul.text("call-card-section-call")

        GridLayout {
            Layout.fillWidth: true
            columns: card.host.window.compact ? 1 : 2
            columnSpacing: card.theme.gap
            rowSpacing: card.host.window.compact ? 2 : 6

            Repeater {
                model: card.there ? card.shown.fields : []

                delegate: Label {
                    required property var modelData
                    required property int index

                    // Even places the labels, odd ones the values (two cells per field).
                    Layout.fillWidth: index % 2 === 1
                    Layout.topMargin: card.host.window.compact && index % 2 === 0 && index > 0 ? 6 : 0
                    text: modelData
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: index % 2 === 0 ? 13 : 15
                    font.weight: index % 2 === 0 ? Font.Normal : Font.DemiBold
                    color: index % 2 === 0 ? card.theme.muted : card.theme.text
                }
            }
        }
    }

    // What it is tied to: whom it was with, the task it made.
    Section {
        objectName: "callTies"
        visible: card.there
        Layout.fillWidth: true
        theme: card.theme
        title: card.sioul.text("call-card-section-ties")

        RelatedList {
            id: ties

            Layout.fillWidth: true
            sioul: card.sioul
            theme: card.theme
            uri: card.there ? card.shown.address : ""
            onOpenThing: item => card.window.openThing(item, true)
        }
        Label {
            visible: ties.items.length === 0
            Layout.fillWidth: true
            text: card.sioul.text("call-card-no-ties")
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: card.theme.muted
        }
    }
}
