// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A phone's own sections in its card in Settings ▸ Devices (DevicesTab.qml;
// design of 18 November 2026: the tab "This phone" merges into it). Every
// phone: its calls (CallsLine.qml), set from any device. This phone, on the
// phone itself: what Android must allow (AllowList.qml), Android's
// restricted settings for an install from a file (RestrictedCard.qml),
// other apps' notifications and each app, conversation and site by time
// (AppNotesSetup.qml, its two parts), messages on your computers
// (PhoneMessagesSetup.qml), texts (TextsSetup.qml), the call screening's own
// settings and where a declined call goes (CallsSetup.qml), and
// do-not-disturb on this phone (DndSetup.qml), and its web view in one line
// (where sites open: siteview.rs). Another phone's card says that
// Android's accesses are set on that phone; on a computer, it lists each app,
// conversation and site with times of its own, read and changed there too
// (AppNotesSetup.qml's part "lists"). At a phone's width every section folds
// (CardFold.qml), and is made only once opened. A door lands in one card's
// part (`reveal`; DevicesTab.qml chooses the card): Settings ▸ Devices ▸ Calls
// ("settings:devices.calls") and each app by time ("settings:devices.apps").

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: card

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The device's id in the sharing.
    property string deviceId: ""
    // This device's own card.
    property bool here: false
    // What the core says of its calls (`calls_line::view`); none for a device that is not a phone.
    property var calls: null
    // What this phone says of itself (`phonecard::view`): {phone, allow, restricted, webview}; {phone: false} elsewhere.
    property var own: ({ phone: false })
    // A phone's width: the sections fold.
    property bool folds: false
    // How many apps, conversations and sites have times of their own.
    property int lists: 0

    // A field of the phone's line said (DevicesTab.qml writes it, `deviceSay`).
    signal say(string field, string value)
    // What this phone says of itself, again (an access asked, a step pressed).
    signal ownSaid(var own)

    readonly property bool phoneHere: card.here && card.own.phone === true
    // Each app, conversation and site by time: on this phone; on a computer, in every phone's card once some have times.
    readonly property bool listsHere: card.phoneHere || (!card.here && card.own.phone !== true && card.calls !== null && card.lists > 0)

    // The blocks shown, in words: changed only when one comes or goes, so that a
    // reload (what this phone says of itself, read again) keeps them and what is unfolded.
    readonly property string blocks: card.phoneHere ? ["apps", "lists", "messages", "texts"].concat(card.sioul.telephony === true ? ["screening"] : []).concat(["dnd"]).join(" ") : card.listsHere ? "lists" : ""

    // A door's part in view: its section unfolded, and returned to be scrolled to.
    function reveal(part) {
        const name = part === "calls" ? "foldCalls" : part === "restricted" ? "foldRestricted" : "fold-lists"
        for (let i = 0; i < card.children.length; i++) {
            const fold = card.children[i] as CardFold
            if (fold && fold.objectName === name && fold.visible) {
                fold.open = true
                return fold
            }
        }
        return null
    }

    spacing: 10

    CardFold {
        objectName: "foldCalls"
        visible: card.calls !== null
        Layout.fillWidth: true
        theme: card.theme
        title: card.sioul.text("phone-calls-title")
        folds: card.folds

        CallsLine {
            Layout.fillWidth: true
            sioul: card.sioul
            theme: card.theme
            view: card.calls || ({ rows: [], columns: [], profiles: [], switches: [], groups: [] })
            here: card.phoneHere
            narrow: card.folds
            onSay: (field, value) => card.say(field, value)
            onSimsAsked: card.ownSaid(JSON.parse(card.sioul.phoneAllow("sims") || "{}"))
        }
    }
    // Another phone: what Android allows is set on it.
    Label {
        visible: !card.here && card.calls !== null
        Layout.fillWidth: true
        text: card.sioul.text("phone-allow-elsewhere")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: card.theme.muted
    }

    // This device's web view, in one line: its version, where sites open, what it cannot carry (siteview.rs).
    Label {
        objectName: "webviewLine"
        visible: card.phoneHere && (card.own.webview || "") !== ""
        Layout.fillWidth: true
        text: card.own.webview || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: card.theme.muted
    }

    CardFold {
        id: allowFold

        objectName: "foldAllow"
        visible: card.phoneHere
        Layout.fillWidth: true
        theme: card.theme
        title: card.sioul.text("phone-allow-title")
        folds: card.folds

        AllowList {
            Layout.fillWidth: true
            sioul: card.sioul
            theme: card.theme
            lines: card.own.allow || []
            narrow: card.folds
            onAsk: what => {
                card.ownSaid(JSON.parse(card.sioul.phoneAllow(what) || "{}"))
                // "Android greys the switch?": the restricted settings' card, unfolded.
                if (what === "restricted")
                    card.reveal("restricted")
            }
        }
    }
    CardFold {
        objectName: "foldRestricted"
        visible: card.phoneHere && card.own.restricted !== null && card.own.restricted !== undefined
        Layout.fillWidth: true
        theme: card.theme
        title: card.sioul.text("restricted-title")
        folds: card.folds

        RestrictedCard {
            Layout.fillWidth: true
            sioul: card.sioul
            theme: card.theme
            view: card.own.restricted || ({ steps: [] })
            onStep: id => card.ownSaid(JSON.parse(card.sioul.restrictedStep(id) || "{}"))
        }
    }

    // The tab "This phone"'s blocks, each made once shown.
    Repeater {
        model: card.blocks === "" ? [] : card.blocks.split(" ")

        delegate: CardFold {
            id: block

            required property string modelData

            objectName: "fold-" + block.modelData
            Layout.fillWidth: true
            theme: card.theme
            title: card.sioul.text("phone-card-" + block.modelData)
            folds: card.folds

            Loader {
                objectName: "block-" + block.modelData
                active: !block.folds || block.open
                Layout.fillWidth: true
                sourceComponent: block.modelData === "apps" ? apps : block.modelData === "lists" ? lists : block.modelData === "messages" ? messages : block.modelData === "texts" ? texts : block.modelData === "screening" ? screening : dnd
            }
        }
    }

    Component {
        id: apps

        AppNotesSetup {
            sioul: card.sioul
            theme: card.theme
            part: "phone"
        }
    }
    // Each app, conversation and browser site, and each by time (from What reaches you's old Exceptions view).
    Component {
        id: lists

        AppNotesSetup {
            sioul: card.sioul
            theme: card.theme
            part: "lists"
        }
    }
    Component {
        id: messages

        PhoneMessagesSetup {
            sioul: card.sioul
            theme: card.theme
        }
    }
    Component {
        id: texts

        TextsSetup {
            sioul: card.sioul
            theme: card.theme
        }
    }
    Component {
        id: screening

        CallsSetup {
            sioul: card.sioul
            theme: card.theme
            inCard: true
        }
    }
    Component {
        id: dnd

        DndSetup {
            sioul: card.sioul
            theme: card.theme
            part: "phone"
        }
    }
}
