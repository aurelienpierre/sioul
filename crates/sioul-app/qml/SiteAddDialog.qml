// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A site added, on a computer and on Android alike (SitesPage.qml,
// android/SitesPage.qml): from the sites people usually keep, by country
// (and state or province) and type, found by a word; or by hand. Only
// encrypted addresses. Pinned, it closes on the site (`added`); refused, it
// stays open with why, what you typed kept.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

SafeDialog {
    id: adding

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The width and height it keeps within: the page's.
    property real room: adding.roomWidth
    property real roomTall: adding.roomHeight
    // The word looked for, for the window's images.
    property alias findText: find.text

    // Pinned: the new site's id.
    signal added(string id)

    readonly property var kinds: ["mailbox", "chat", "video", "social", "dating", "other"]
    readonly property var kindIcons: ({ "mailbox": "mail-folder-inbox", "chat": "im-user", "video": "camera-video", "social": "system-users", "dating": "emblem-favorite" })
    property var offered: ({ country: "", region: "", countries: [], types: [], categories: [], sites: [] })
    // "type:chat", "category:Banque", or "" for all.
    property string category: ""
    // From the list: what it is for and which mail announces it; by hand, as its kind goes.
    property string area: ""
    property var announced: []
    property string chosen: ""
    // What was refused (an address without https…): said here, the dialog kept open.
    property string problem: ""
    readonly property var country: adding.offered.countries.find(c => c.code === adding.offered.country) || null
    readonly property var shownPresets: {
        const words = find.text.toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").split(/\s+/).filter(w => w !== "")
        const folded = t => String(t).toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "")
        return adding.offered.sites
            .filter(p => adding.category === "" || (adding.category.startsWith("type:") ? p.kind === adding.category.slice(5) : p.categories.indexOf(adding.category.slice(9)) >= 0))
            .filter(p => words.every(w => folded(p.name + " " + p.about + " " + p.host + " " + p.categories.join(" ")).indexOf(w) >= 0))
            .sort((a, b) => adding.kinds.indexOf(a.kind) - adding.kinds.indexOf(b.kind) || a.name.localeCompare(b.name))
    }

    function load(country, region) {
        adding.offered = JSON.parse(adding.sioul.sitePresets(country, region))
    }

    // Pinned, it closes on the site; refused, it stays open with why, what you typed kept.
    function save() {
        const categories = siteCategories.text.split(",").map(c => c.trim()).filter(c => c !== "")
        const made = JSON.parse(adding.sioul.addSite(JSON.stringify({ name: siteName.text.trim(), url: siteUrl.text.trim(), kind: adding.kinds[siteKind.currentIndex], area: adding.area, announced_by: adding.announced, categories: categories })))
        if (made.error) {
            adding.problem = made.error
            return
        }
        adding.close()
        adding.added(made.id)
    }

    function take(preset) {
        adding.chosen = preset.url
        siteName.text = preset.name
        siteUrl.text = preset.url
        siteKind.currentIndex = Math.max(0, adding.kinds.indexOf(preset.kind))
        siteCategories.text = preset.categories.join(", ")
        adding.area = preset.area
        adding.announced = preset.announced_by
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, Math.min(adding.room, adding.roomWidth) - 2 * adding.theme.gap)
    height: Math.min(660, Math.min(adding.roomTall, adding.roomHeight) - 2 * adding.theme.gap)
    title: adding.sioul.text("site-add")
    onAboutToShow: {
        siteName.text = ""
        siteUrl.text = "https://"
        siteKind.currentIndex = 0
        siteCategories.text = ""
        find.text = ""
        adding.category = ""
        adding.area = ""
        adding.announced = []
        adding.chosen = ""
        adding.problem = ""
        adding.load("", "")
    }
    // On a phone the keyboard would cover the list it filters: the field is tapped when wanted.
    onOpened: {
        if (!adding.theme.touch)
            find.forceActiveFocus()
    }

    contentItem: ColumnLayout {
        spacing: 8

        // Side by side; on a phone, one under the other.
        GridLayout {
            Layout.fillWidth: true
            columns: adding.width < 480 ? 1 : 3
            columnSpacing: 6
            rowSpacing: 6

            PlainComboBox {
                id: countryChoice

                readonly property var choices: [{ code: "-", name: adding.sioul.text("site-presets-everyone") }].concat(adding.offered.countries)

                Layout.fillWidth: true
                model: choices.map(c => adding.theme.plain(c.name))
                currentIndex: Math.max(0, choices.findIndex(c => c.code === (adding.offered.country || "-")))
                onActivated: index => adding.load(choices[index].code, "")
            }
            PlainComboBox {
                id: regionChoice

                readonly property var choices: [{ code: "", name: adding.sioul.text("site-presets-whole-country") }].concat(adding.country ? adding.country.regions : [])

                visible: adding.country !== null && adding.country.regions.length > 0
                Layout.fillWidth: true
                model: choices.map(r => adding.theme.plain(r.name))
                currentIndex: Math.max(0, choices.findIndex(r => r.code === adding.offered.region))
                onActivated: index => adding.load(adding.offered.country, choices[index].code)
            }
            PlainComboBox {
                readonly property var choices: [{ id: "", name: adding.sioul.text("site-presets-all") }].concat(adding.offered.types.map(t => ({ id: "type:" + t, name: adding.sioul.text("site-kind-" + t) }))).concat(adding.offered.categories.map(c => ({ id: "category:" + c, name: c })))

                Layout.fillWidth: true
                model: choices.map(c => adding.theme.plain(c.name))
                currentIndex: Math.max(0, choices.findIndex(c => c.id === adding.category))
                onActivated: index => adding.category = choices[index].id
            }
        }
        TextField {
            id: find

            Layout.fillWidth: true
            placeholderText: adding.sioul.text("site-presets-find")
            inputMethodHints: Qt.ImhNoPredictiveText
        }
        ListView {
            id: presetList

            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.minimumHeight: 120
            clip: true
            model: adding.shownPresets
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}

            delegate: ItemDelegate {
                id: preset

                required property var modelData

                width: ListView.view.width
                highlighted: adding.chosen === preset.modelData.url
                onClicked: adding.take(preset.modelData)
                onDoubleClicked: {
                    adding.take(preset.modelData)
                    adding.save()
                }

                contentItem: RowLayout {
                    spacing: 8

                    Icon {
                        iconName: adding.kindIcons[preset.modelData.kind] || "internet-web-browser"
                        size: 16
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0

                        Label {
                            Layout.fillWidth: true
                            text: preset.modelData.name
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.weight: Font.DemiBold
                            color: adding.theme.text
                        }
                        Label {
                            Layout.fillWidth: true
                            text: [preset.modelData.about, preset.modelData.categories.join(", "), preset.modelData.host, preset.modelData.calls === true ? adding.sioul.text("site-presets-calls") : "", preset.modelData.kept ? adding.sioul.text("site-presets-kept") : ""].filter(t => t !== "").join(" · ")
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            font.pixelSize: 12
                            color: adding.theme.muted
                        }
                    }
                }
            }
        }
        Label {
            visible: adding.shownPresets.length === 0
            Layout.fillWidth: true
            text: adding.sioul.text("site-presets-none")
            wrapMode: Text.Wrap
            color: adding.theme.muted
        }
        Label {
            text: adding.sioul.text("site-by-hand")
            color: adding.theme.muted
        }
        GridLayout {
            Layout.fillWidth: true
            columns: 2
            columnSpacing: 10
            rowSpacing: 6

            Label {
                text: adding.sioul.text("site-field-name")
                color: adding.theme.muted
            }
            TextField {
                id: siteName

                Layout.fillWidth: true
            }
            Label {
                text: adding.sioul.text("site-field-url")
                color: adding.theme.muted
            }
            TextField {
                id: siteUrl

                Layout.fillWidth: true
                inputMethodHints: Qt.ImhUrlCharactersOnly | Qt.ImhNoPredictiveText
                onAccepted: adding.save()
            }
            Label {
                text: adding.sioul.text("site-field-kind")
                color: adding.theme.muted
            }
            PlainComboBox {
                id: siteKind

                Layout.fillWidth: true
                model: adding.kinds.map(k => adding.sioul.text("site-kind-" + k))
                // Chosen by hand: what it is for follows its type.
                onActivated: adding.area = ""
            }
            Label {
                text: adding.sioul.text("site-field-categories")
                color: adding.theme.muted
            }
            TextField {
                id: siteCategories

                Layout.fillWidth: true
                placeholderText: adding.sioul.text("site-categories-help")
            }
        }
        Label {
            visible: adding.problem !== ""
            Layout.fillWidth: true
            text: adding.problem
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: adding.theme.warm
        }
    }

    // Out of the footer's own box: a box there closes the dialog on its
    // first button before adding could be refused. Sioul's own words, translated.
    footer: Item {
        implicitWidth: addButtons.implicitWidth
        implicitHeight: addButtons.implicitHeight

        DialogButtonBox {
            id: addButtons

            anchors.fill: parent

            Button {
                text: adding.sioul.text("ui-add")
                highlighted: true
                DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
            }
            Button {
                text: adding.sioul.text("ui-cancel")
                DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
            }
            onAccepted: adding.save()
            onRejected: adding.close()
        }
    }
}
