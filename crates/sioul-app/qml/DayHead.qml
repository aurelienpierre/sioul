// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A day's whole-day things on one line, as a phone's Day draws them above its
// hours (`AgendaPage`'s head line): the first one by its title and "2 more"; a
// date asked first in its day (`timeline::days`), with its diamond, filled
// within seven days, and its words ("CAF form · Date asked · four days left",
// `WeekPlanning.datedWords`), as the computer's Day says it (the unified
// agenda, part 4, later 2). A tap opens the first one (`opened`).

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: head

    required property var sioul
    required property var theme
    // The agenda's timeline, whose words a date asked takes (`datedWords`).
    required property var planning
    // The day's head, as `days` gives it: each {item, when, …}.
    property var chips: []
    // The first, when it is a date asked.
    readonly property var firstDated: head.chips.length > 0 && head.chips[0].item.kind === "dated" ? head.chips[0] : null

    signal opened()

    visible: head.chips.length > 0
    spacing: 8

    Rectangle {
        objectName: "agendaDayDiamond"
        visible: head.firstDated !== null
        Layout.preferredWidth: 7
        Layout.preferredHeight: 7
        Layout.alignment: Qt.AlignVCenter
        rotation: 45
        color: head.firstDated && head.firstDated.item.tone !== "quiet" ? head.theme.text : "transparent"
        border.color: head.theme.text
        border.width: 1
    }
    Label {
        objectName: "agendaDayChips"
        Layout.fillWidth: true
        text: head.chips.length === 0 ? "" : (head.firstDated ? head.planning.datedWords({ title: head.firstDated.item.title, when: head.firstDated.item.time && head.firstDated.item.time.type === "at" ? head.firstDated.when || "" : "", line: head.firstDated.item.line || "" }) : head.chips[0].item.title) + (head.chips.length > 1 ? "  " + head.sioul.textArgs("agenda-chips-more", JSON.stringify({ n: head.chips.length - 1 })) : "")
        textFormat: Text.PlainText
        elide: Text.ElideRight
        color: head.theme.muted

        TapHandler {
            onTapped: head.opened()
        }
    }
}
