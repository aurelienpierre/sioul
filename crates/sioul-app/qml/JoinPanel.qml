// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Joining your other devices through your Nextcloud's own sign-in
// (docs/database.md, "Joining through your Nextcloud's sign-in"; join.rs):
// Murena's, or another Nextcloud by its address. Its page opens in the
// browser and Sioul waits for it (Cancel stops it); the sharing folder found
// there is proposed, or a place is typed; the passphrase typed once joins it
// (twice for a new sharing, offered only once every place was looked at and
// none found); a look that could not go through says so, with "Look again";
// then what came is said, and what still needs a sign-in here. In Settings ▸ Your folder and sharing (SharePanel.qml), which
// the Porch's card opens on a device that does not share yet.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: join

    required property var sioul
    required property var theme

    // Where it stands (join.rs's `Stand`): {step, host, signed, places, problem, lines, needs}.
    property var stand: ({ step: "" })
    readonly property string step: join.stand.step || ""
    readonly property var places: join.stand.places || []
    // Something under way, or its result to read: the panel's other ways wait meanwhile.
    readonly property bool busy: join.step !== ""
    // "Sign in to another Nextcloud" pressed: its address asked.
    property bool other: false
    // The place chosen among those found; "Another place" pressed: one typed.
    property string place: ""
    property bool otherPlace: false
    // A place typed: none found, the look incomplete, or "Another place".
    readonly property bool typed: join.step === "none" || join.step === "unchecked" || join.otherPlace
    // A new sharing may start there (every place looked at, none found): the passphrase twice.
    readonly property bool starting: join.step === "none"
    // Joined, on a phone: what Android has not allowed Sioul yet that it needs here (access.rs).
    property var withheld: []

    function readWithheld() {
        // A stand-in for Sioul without the texts (an older test's), or a computer: nothing.
        const answer = typeof join.sioul.texts === "function" ? JSON.parse(join.sioul.texts("access", "{}") || "null") : null
        join.withheld = answer && answer.lines ? answer.lines : []
    }

    // Android's question, its page, or Sioul's app info: then read again for half a minute.
    function askAndroid(verb, what) {
        join.sioul.texts(verb, JSON.stringify({ what: what }))
        asking.left = 15
        asking.restart()
    }

    function reload() {
        // A stand-in for Sioul without the join (an older test's) says nothing under way.
        join.stand = JSON.parse(typeof join.sioul.joinStatus === "function" ? join.sioul.joinStatus() || "{}" : "{}")
        if (join.places.length > 0 && !join.places.includes(join.place))
            join.place = join.places[0]
        if (join.step === "" || join.step === "joined") {
            passphrase.clear()
            again.clear()
        }
        if (join.step === "joined")
            join.readWithheld()
    }

    // From the Porch's card: Murena's sign-in at once, or another's address asked.
    function begin(how) {
        if (join.step !== "")
            return
        if (how === "murena") {
            join.sioul.joinSignIn("murena")
        } else {
            join.other = true
            address.forceActiveFocus()
        }
    }

    function signIn() {
        if (address.text.trim() !== "")
            join.sioul.joinSignIn(address.text)
    }

    function joinNow() {
        join.sioul.joinFolder(join.typed ? placeField.text : join.place, passphrase.text, join.starting ? again.text : "")
    }

    spacing: 6
    Component.onCompleted: join.reload()
    onVisibleChanged: if (visible) join.reload()

    Timer {
        id: asking

        property int left: 0

        interval: 2000
        repeat: true
        onTriggered: {
            join.readWithheld()
            asking.left -= 1
            if (asking.left <= 0 || join.withheld.length === 0)
                asking.stop()
        }
    }

    Connections {
        target: join.sioul
        ignoreUnknownSignals: true

        function onJoinChanged() {
            join.reload()
        }
    }

    // Nothing under way: the two ways to sign in.
    Label {
        visible: join.step === ""
        Layout.fillWidth: true
        text: join.sioul.text("join-title")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: join.theme.text
    }
    Label {
        visible: join.step === ""
        Layout.fillWidth: true
        text: join.sioul.text("join-help")
        wrapMode: Text.Wrap
        font.pixelSize: 13
        color: join.theme.muted
    }
    Flow {
        visible: join.step === ""
        Layout.fillWidth: true
        spacing: 8

        Button {
            highlighted: true
            text: join.sioul.text("join-murena")
            onClicked: join.sioul.joinSignIn("murena")
        }
        Button {
            flat: true
            text: join.sioul.text("join-other")
            onClicked: {
                join.other = true
                address.forceActiveFocus()
            }
        }
    }
    RowLayout {
        visible: join.step === "" && join.other
        Layout.fillWidth: true
        spacing: 8

        Label {
            text: join.sioul.text("join-other-address")
            color: join.theme.muted
        }
        TextField {
            id: address

            Layout.fillWidth: true
            placeholderText: "cloud.example.org"
            inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
            Accessible.name: join.sioul.text("join-other-address")
            onAccepted: join.signIn()
        }
        Button {
            enabled: address.text.trim() !== ""
            text: join.sioul.text("join-sign-in")
            onClicked: join.signIn()
        }
    }

    // Signing in, in the browser: Sioul waits.
    Label {
        visible: join.step === "signing"
        Layout.fillWidth: true
        text: join.step === "signing" ? join.sioul.textWith("join-signing", "host", join.stand.host || "") : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: join.theme.text
    }
    // Signed in: as whom, where.
    Label {
        visible: (join.stand.signed || "") !== "" && ["looking", "found", "none", "unchecked", "joining"].includes(join.step)
        Layout.fillWidth: true
        text: join.stand.signed || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: join.theme.text
    }
    Label {
        visible: join.step === "looking"
        Layout.fillWidth: true
        text: join.sioul.text("join-looking")
        wrapMode: Text.Wrap
        color: join.theme.muted
    }
    // The folder found there: one, said; several, one to choose.
    Label {
        visible: join.step === "found" && join.places.length === 1 && !join.otherPlace
        Layout.fillWidth: true
        text: join.step === "found" && join.places.length > 0 ? join.sioul.textWith("join-found", "place", join.places[0]) : ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: join.theme.text
    }
    Label {
        visible: join.step === "found" && join.places.length > 1 && !join.otherPlace
        Layout.fillWidth: true
        text: join.sioul.text("join-found-some")
        wrapMode: Text.Wrap
        color: join.theme.text
    }
    Repeater {
        model: join.step === "found" && join.places.length > 1 && !join.otherPlace ? join.places : []

        delegate: RadioButton {
            required property string modelData

            Layout.fillWidth: true
            text: join.theme.plain(modelData)
            checked: join.place === modelData
            onToggled: join.place = modelData
        }
    }
    // None found there: a place typed, or a new sharing started there.
    Label {
        visible: join.step === "none"
        Layout.fillWidth: true
        text: join.sioul.text("join-none")
        wrapMode: Text.Wrap
        color: join.theme.text
    }
    GridLayout {
        visible: ["found", "none", "unchecked"].includes(join.step)
        Layout.fillWidth: true
        columns: 2
        columnSpacing: 8
        rowSpacing: 6

        Label {
            visible: join.typed
            text: join.sioul.text("join-place")
            color: join.theme.muted
        }
        TextField {
            id: placeField

            visible: join.typed
            Layout.fillWidth: true
            text: "Documents/Sioul"
            inputMethodHints: Qt.ImhNoAutoUppercase | Qt.ImhNoPredictiveText
            Accessible.name: join.sioul.text("join-place")
        }
        Label {
            text: join.sioul.text("share-passphrase")
            color: join.theme.muted
        }
        PasswordField {
            id: passphrase

            Layout.fillWidth: true
            sioul: join.sioul
            Accessible.name: join.sioul.text("share-passphrase")
            onAccepted: if (!join.starting) join.joinNow()
        }
        Label {
            visible: join.starting
            text: join.sioul.text("share-again")
            color: join.theme.muted
        }
        PasswordField {
            id: again

            visible: join.starting
            Layout.fillWidth: true
            sioul: join.sioul
            Accessible.name: join.sioul.text("share-again")
        }
        Label {
            Layout.columnSpan: 2
            Layout.fillWidth: true
            text: join.sioul.text(join.starting ? "share-server-passphrase-hint" : "share-passphrase-known")
            wrapMode: Text.Wrap
            font.pixelSize: 13
            color: join.theme.muted
        }
    }
    // Why not, in words: a sign-in refused, a wrong passphrase.
    Label {
        visible: (join.stand.problem || "") !== ""
        Layout.fillWidth: true
        text: join.stand.problem || ""
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: join.theme.warm
    }
    Label {
        visible: join.step === "joining"
        Layout.fillWidth: true
        text: join.sioul.text("join-joining")
        wrapMode: Text.Wrap
        color: join.theme.muted
    }
    Flow {
        visible: ["signing", "looking", "found", "none", "unchecked"].includes(join.step)
        Layout.fillWidth: true
        spacing: 8

        Button {
            visible: join.step === "found" || join.step === "unchecked"
            highlighted: true
            enabled: passphrase.text.length > 0
            text: join.sioul.text("join-join")
            onClicked: join.joinNow()
        }
        Button {
            visible: join.step === "none"
            highlighted: true
            enabled: passphrase.text.length > 0
            text: join.sioul.text("join-start")
            onClicked: join.joinNow()
        }
        Button {
            visible: join.step === "unchecked"
            flat: true
            text: join.sioul.text("join-look-again")
            onClicked: join.sioul.joinLookAgain()
        }
        Button {
            visible: join.step === "found" && !join.otherPlace
            flat: true
            text: join.sioul.text("join-other-place")
            onClicked: {
                join.otherPlace = true
                placeField.forceActiveFocus()
            }
        }
        Button {
            visible: join.step === "signing"
            flat: true
            text: join.sioul.text("join-open-again")
            onClicked: join.sioul.joinOpenAgain()
        }
        Button {
            flat: true
            text: join.sioul.text("ui-cancel")
            onClicked: {
                join.otherPlace = false
                join.sioul.joinCancel()
            }
        }
    }
    // Joined: who it shares with now, what came; what still needs a sign-in here.
    Repeater {
        model: join.step === "joined" ? (join.stand.lines || []) : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: join.theme.text
        }
    }
    Label {
        visible: join.step === "joined" && (join.stand.needs || []).length > 0
        Layout.topMargin: 6
        Layout.fillWidth: true
        text: join.sioul.text("join-needs-title")
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: join.theme.text
    }
    Repeater {
        model: join.step === "joined" ? (join.stand.needs || []) : []

        delegate: Label {
            required property string modelData

            Layout.fillWidth: true
            text: modelData
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: join.theme.text
        }
    }
    // What Android has not allowed Sioul yet here, each with what Android wants (a phone's).
    Repeater {
        model: join.step === "joined" ? join.withheld : []

        delegate: ColumnLayout {
            id: withheldLine

            required property var modelData

            Layout.fillWidth: true
            Layout.topMargin: 4
            spacing: 4

            Label {
                Layout.fillWidth: true
                text: withheldLine.modelData.said
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: join.theme.text
            }
            Flow {
                Layout.fillWidth: true
                spacing: 6

                Button {
                    visible: withheldLine.modelData.restricted === true
                    width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                    text: join.sioul.text("appnotes-open-info")
                    onClicked: join.askAndroid("access-info", "")
                }
                Button {
                    width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                    text: join.sioul.text("texts-setup-ask")
                    onClicked: join.askAndroid("access-ask", withheldLine.modelData.what)
                }
            }
        }
    }
    Button {
        visible: join.step === "joined" || join.step === "failed"
        text: join.sioul.text(join.step === "joined" ? "join-done" : "join-try-again")
        onClicked: {
            join.other = false
            join.otherPlace = false
            join.sioul.joinDone()
        }
    }
}
