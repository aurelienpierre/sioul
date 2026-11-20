// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An office's opening hours looked up (hourslookup.rs, offices.rs): first
// what would be sent, the office's name and where it is, and to whom (the
// administration's directory for an office in France, the map's search
// service unless it is set to None or asked Sioul to wait), with what goes
// along (Sioul's name and version; the window's language to the map), which
// the person may correct; nothing leaves before they press "Look up", and
// then one request to each, both at once. Each press has its number: an
// answer to an earlier press never fills the list of a later one. The
// answer is a short list, "Is it this one?", each office with its address
// and hours as plain text, its source named ("© OpenStreetMap
// contributors"); the one picked has its hours written on the card with
// their source and the day ("kept on the card" said only when something
// was written). A number or an address the card lacks is offered after,
// never written by itself. Nothing found changes nothing; a source that did
// not answer is said.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: lookup

    required property var sioul
    required property var theme
    property string key: ""
    // The card's section as it was (`contactHours`): its name, what it would send, to whom.
    property var shown: null
    // "ask", "asking", "answered", "picked".
    property string step: "ask"
    // The number of the last press of Look up: its answer alone fills the list.
    property int press: 0
    // The pick wrote something on the card (else it said those hours already).
    property bool changed: false
    // Who would be asked, in words; "" when nobody can be.
    readonly property string sources: lookup.shown ? lookup.shown.sources || "" : ""
    property var found: []
    property var failed: []
    property bool examples: false
    // What the card lacks that the office picked has: {phone, address}.
    property var offers: ({ phone: "", address: "" })
    property string problem: ""

    signal picked

    function begin(key, shown) {
        lookup.key = key
        lookup.shown = shown
        lookup.step = "ask"
        lookup.found = []
        lookup.failed = []
        lookup.offers = { phone: "", address: "" }
        lookup.problem = ""
        words.text = shown && shown.asked ? shown.asked.name : ""
        place.text = shown && shown.asked ? shown.asked.place : ""
        lookup.open()
    }

    // The person pressed Look up: the words they see, sent once, with this press's number.
    function ask() {
        // A word still being composed by a phone's keyboard, committed as its field gives up the focus.
        scroll.forceActiveFocus()
        if (words.text.trim() === "" || lookup.sources === "" || lookup.step !== "ask")
            return
        lookup.press += 1
        lookup.step = "asking"
        const asked = { name: words.text.trim(), place: place.text.trim(), france: lookup.shown && lookup.shown.asked ? lookup.shown.asked.france === true : false, press: lookup.press }
        lookup.sioul.lookUpHours(lookup.key, JSON.stringify(asked))
    }

    function take(office) {
        const answer = JSON.parse(lookup.sioul.pickHours(lookup.key, JSON.stringify(office)) || "{}")
        if (answer.error) {
            lookup.problem = answer.error
            return
        }
        lookup.offers = answer.offers || { phone: "", address: "" }
        lookup.changed = answer.changed !== false
        lookup.step = "picked"
        lookup.picked()
    }

    function add(what, value) {
        const answer = JSON.parse(lookup.sioul.addToCard(lookup.key, what, value) || "{}")
        if (answer.error) {
            lookup.problem = answer.error
            return
        }
        const next = Object.assign({}, lookup.offers)
        next[what] = ""
        lookup.offers = next
    }

    Connections {
        target: lookup.sioul

        // The answer to this press, for this card, while it is awaited; a late one, for a card
        // closed since or an earlier press, says nothing.
        function onHoursFound(key, found) {
            if (key !== lookup.key || lookup.step !== "asking")
                return
            const answer = JSON.parse(found || "{}")
            if (answer.press !== lookup.press)
                return
            lookup.found = answer.found || []
            lookup.failed = answer.failed || []
            lookup.examples = answer.offline === true
            lookup.step = "answered"
        }
    }

    objectName: "openingHoursLookup"
    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(560, (parent ? lookup.roomWidth : 560) - 2 * lookup.theme.gap)
    height: Math.min(implicitHeight, (parent ? lookup.roomHeight : 800) - 2 * lookup.theme.gap)
    modal: true
    title: lookup.theme.plain(lookup.sioul.textWith("hours-lookup-title", "name", lookup.shown ? lookup.shown.name : ""))

    ScrollView {
        id: scroll

        anchors.fill: parent
        contentWidth: availableWidth

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 12

            // What is sent, and to whom, before anything is.
            Section {
                visible: lookup.step === "ask" || lookup.step === "asking"
                Layout.fillWidth: true
                theme: lookup.theme
                title: lookup.sioul.text("hours-lookup-words")

                TextField {
                    id: words

                    objectName: "openingHoursLookupWords"
                    Layout.fillWidth: true
                    enabled: lookup.step === "ask"
                    Accessible.name: lookup.sioul.text("hours-lookup-words")
                    onAccepted: lookup.ask()
                }
                Label {
                    Layout.fillWidth: true
                    text: lookup.sioul.text("hours-lookup-place")
                    color: lookup.theme.muted
                }
                TextArea {
                    id: place

                    objectName: "openingHoursLookupPlace"
                    Layout.fillWidth: true
                    enabled: lookup.step === "ask"
                    wrapMode: TextEdit.Wrap
                    Accessible.name: lookup.sioul.text("hours-lookup-place")
                }
                // What goes, to whom, and with what: the map named, with the window's language, when it is asked.
                Label {
                    objectName: "openingHoursLookupSends"
                    Layout.fillWidth: true
                    text: !lookup.shown ? "" : lookup.sources === "" ? lookup.sioul.text("hours-lookup-nobody") : (lookup.shown.map || "") === "" ? lookup.sioul.textArgs("hours-lookup-sends", JSON.stringify({ sources: lookup.sources, map: lookup.shown.map_name || "" })) : lookup.sioul.textWith("hours-lookup-sends-directory", "sources", lookup.sources)
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: lookup.theme.text
                }
                // The map's search service not asked, and why.
                Label {
                    objectName: "openingHoursLookupMapStopped"
                    visible: lookup.shown !== null && (lookup.shown.map || "") !== ""
                    Layout.fillWidth: true
                    text: lookup.shown && lookup.shown.map === "off" ? lookup.sioul.text("hours-lookup-map-off") : lookup.sioul.text("hours-lookup-map-waiting")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: lookup.theme.muted
                }
                Label {
                    visible: lookup.shown !== null && lookup.shown.offline === true
                    Layout.fillWidth: true
                    text: lookup.sioul.text("hours-lookup-offline")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: lookup.theme.muted
                }
                RowLayout {
                    visible: lookup.step === "asking"
                    Layout.fillWidth: true
                    spacing: 8

                    BusyIndicator {
                        running: lookup.step === "asking"
                        implicitWidth: 24
                        implicitHeight: 24
                    }
                    Label {
                        Layout.fillWidth: true
                        text: lookup.shown ? lookup.sioul.textWith("hours-lookup-asking", "sources", lookup.shown.sources || "") : ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: lookup.theme.muted
                    }
                }
            }

            // "Is it this one?"
            Section {
                objectName: "openingHoursLookupList"
                visible: lookup.step === "answered"
                Layout.fillWidth: true
                theme: lookup.theme
                title: lookup.sioul.text("hours-lookup-which")

                Label {
                    visible: lookup.examples
                    Layout.fillWidth: true
                    text: lookup.sioul.text("hours-lookup-offline")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    font.pixelSize: 12
                    color: lookup.theme.muted
                }
                Label {
                    objectName: "openingHoursLookupNone"
                    visible: lookup.found.length === 0
                    Layout.fillWidth: true
                    text: lookup.sioul.text("hours-lookup-none")
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: lookup.theme.text
                }
                Repeater {
                    model: lookup.found

                    delegate: ColumnLayout {
                        id: office

                        required property var modelData
                        required property int index
                        readonly property bool takes: office.modelData.hours !== "" || office.modelData.note !== ""

                        Layout.fillWidth: true
                        Layout.topMargin: office.index > 0 ? 8 : 0
                        spacing: 3

                        Rectangle {
                            visible: office.index > 0
                            Layout.fillWidth: true
                            implicitHeight: 1
                            color: lookup.theme.line
                        }
                        Label {
                            Layout.fillWidth: true
                            text: office.modelData.name
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.weight: Font.DemiBold
                            color: lookup.theme.text
                        }
                        Label {
                            visible: office.modelData.address !== ""
                            Layout.fillWidth: true
                            text: office.modelData.address
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: lookup.theme.muted
                        }
                        Repeater {
                            model: office.modelData.lines

                            delegate: Label {
                                required property string modelData

                                Layout.fillWidth: true
                                text: modelData
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: lookup.theme.text
                            }
                        }
                        Label {
                            visible: office.modelData.hours === ""
                            Layout.fillWidth: true
                            text: lookup.sioul.text("hours-lookup-no-hours")
                            textFormat: Text.PlainText
                            color: lookup.theme.muted
                        }
                        Label {
                            visible: office.modelData.note !== ""
                            Layout.fillWidth: true
                            text: lookup.sioul.textWith("hours-lookup-free", "text", office.modelData.note)
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: lookup.theme.muted
                        }
                        Label {
                            Layout.fillWidth: true
                            text: office.modelData.source === "openstreetmap.org" ? office.modelData.source_name + " · " + lookup.sioul.text("hours-map-credit") : office.modelData.source_name
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            font.pixelSize: 12
                            color: lookup.theme.muted
                        }
                        Button {
                            objectName: "openingHoursLookupPick" + office.index
                            visible: office.takes
                            text: lookup.sioul.text("hours-lookup-pick")
                            Accessible.name: lookup.sioul.textWith("hours-lookup-pick-name", "name", office.modelData.name)
                            onClicked: lookup.take(office.modelData)
                        }
                    }
                }
                Repeater {
                    model: lookup.failed

                    delegate: Label {
                        required property string modelData

                        Layout.fillWidth: true
                        text: modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: lookup.theme.warm
                    }
                }
            }

            // Picked: the hours are on the card; what it lacks, offered.
            Section {
                objectName: "openingHoursLookupPicked"
                visible: lookup.step === "picked"
                Layout.fillWidth: true
                theme: lookup.theme
                title: lookup.sioul.text(lookup.changed ? "hours-saved" : "hours-saved-same")

                Button {
                    objectName: "openingHoursLookupAddPhone"
                    visible: lookup.offers.phone !== ""
                    Layout.fillWidth: true
                    text: lookup.theme.plain(lookup.sioul.textWith("hours-lookup-add-number", "number", lookup.offers.phone))
                    onClicked: lookup.add("phone", lookup.offers.phone)
                }
                Button {
                    objectName: "openingHoursLookupAddAddress"
                    visible: lookup.offers.address !== ""
                    Layout.fillWidth: true
                    text: lookup.sioul.text("hours-lookup-add-address")
                    onClicked: lookup.add("address", lookup.offers.address)
                }
                Label {
                    visible: lookup.offers.address !== ""
                    Layout.fillWidth: true
                    text: lookup.offers.address
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: lookup.theme.muted
                }
            }

            Label {
                visible: lookup.problem !== ""
                Layout.fillWidth: true
                text: lookup.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: lookup.theme.warm
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            objectName: "openingHoursLookupGo"
            visible: lookup.step === "ask"
            enabled: words.text.trim() !== "" && lookup.sources !== ""
            text: lookup.sioul.text("hours-lookup-go")
            highlighted: true
            onClicked: lookup.ask()
        }
        Button {
            text: lookup.sioul.text(lookup.step === "picked" ? "ui-close" : "ui-cancel")
            onClicked: lookup.close()
        }
    }
}
