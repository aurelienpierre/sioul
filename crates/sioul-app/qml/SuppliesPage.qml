// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Supplies (docs/supplies.md): what is used up or sold, and what is left of
// it. Its calm view first: the supplies of what now is for, by category, one
// line each (its name, what is free, "4 planned" when some is, and the quiet
// words "low" or "none left"); every supply one tab away, whatever the hour,
// with its stock, its threshold and its prices; the archived folded under
// them. No colour of alarm, no count of what is low. A press opens the
// supply's card (SupplyCard.qml); ⚙ says where its tasks to resupply go.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme
    required property var window

    property var shown: ({ store: false, problem: "", keeps: false, calm: [], all: [], archived: [], elsewhere: 0, categories: [], projects: [] })
    // The calm view ("now", the default) or every supply ("all").
    property string tab: "now"
    // The archived, unfolded under the full view.
    property bool archivedShown: false
    // A narrow screen (a phone): a supply's numbers under its name.
    readonly property bool narrow: page.width < 560
    // The supplies of the tab shown, by category.
    readonly property var groups: page.tab === "now" ? page.shown.calm : page.shown.all
    // The supply's card, made the first time.
    property alias card: supplyCard

    function reload() {
        suppliesPlace.keep(() => {
            page.shown = JSON.parse(page.sioul.suppliesPage())
        })
    }

    // A new supply, its card empty.
    function startNew() {
        page.reload()
        supplyCard.now().begin()
    }

    // A supply's card, by its id.
    function open(id) {
        supplyCard.now().show(id)
    }

    onVisibleChanged: if (visible) page.reload()
    Component.onCompleted: page.reload()

    // A supply changed here or on another device, a task done or planned: shown at once.
    Connections {
        target: page.sioul

        function onSuppliesChanged() {
            if (page.visible)
                page.reload()
        }

        function onTasksChanged() {
            if (page.visible)
                page.reload()
        }
    }

    // A supply changed: the page keeps its place (ListPlace.qml).
    ListPlace {
        id: suppliesPlace

        view: scroll
    }

    ScrollView {
        id: scroll

        anchors.fill: parent
        anchors.margins: page.theme.gap
        contentWidth: availableWidth
        clip: true

        PageColumn {
            theme: page.theme
            room: scroll.availableWidth
            spacing: 8

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    text: page.sioul.text("ui-supplies")
                    font.pixelSize: 22
                    elide: Text.ElideRight
                    color: page.theme.text
                }
                Button {
                    objectName: "suppliesNew"
                    enabled: page.shown.store
                    text: page.sioul.text("supplies-new")
                    icon.name: "list-add"
                    onClicked: page.startNew()
                }
                // Where the tasks to resupply go (`supplies.list`).
                SettingsButton {
                    sioul: page.sioul
                    theme: page.theme
                    view: "supplies"
                }
            }
            // What the page is for, while nothing is kept yet.
            Label {
                visible: !page.shown.keeps
                Layout.fillWidth: true
                text: page.shown.store ? page.sioul.text("supplies-help") : page.sioul.text("error-no-store")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }
            Tabs {
                id: suppliesTabs

                objectName: "suppliesTabs"
                visible: page.shown.keeps
                Layout.fillWidth: true
                theme: page.theme
                name: page.sioul.text("ui-supplies")
                model: [{ id: "now", text: page.sioul.text("supplies-tab-now") }, { id: "all", text: page.sioul.text("supplies-tab-all") }]
                current: page.tab
                onChosen: id => page.tab = id
            }
            Label {
                visible: page.shown.problem !== ""
                Layout.fillWidth: true
                text: page.shown.problem
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: page.theme.warm
            }
            // The calm view with nothing for now: said, the full view one tab away.
            Label {
                objectName: "suppliesNoneNow"
                visible: page.shown.keeps && page.tab === "now" && page.shown.calm.length === 0
                Layout.fillWidth: true
                Layout.topMargin: 12
                text: page.sioul.text("supplies-none-now")
                wrapMode: Text.Wrap
                color: page.theme.muted
            }

            Repeater {
                model: page.groups

                delegate: ColumnLayout {
                    id: group

                    required property var modelData
                    readonly property string placeKey: "category:" + group.modelData.category

                    Layout.fillWidth: true
                    Layout.topMargin: 12
                    spacing: 6

                    Label {
                        text: group.modelData.category
                        textFormat: Text.PlainText
                        font.pixelSize: 17
                        font.weight: Font.DemiBold
                        color: page.theme.accent
                    }
                    Repeater {
                        model: group.modelData.supplies

                        delegate: Rectangle {
                            id: row

                            required property var modelData
                            readonly property string placeKey: row.modelData.id
                            // The full view says more: its stock, its threshold, its prices.
                            readonly property string more: page.tab === "all" ? [row.modelData.stock_text !== "" ? page.sioul.textWith("supplies-in-stock", "quantity", row.modelData.stock_text) : "", row.modelData.low_at !== "" ? page.sioul.textWith("supplies-low-at", "quantity", row.modelData.low_at) : "", row.modelData.buy !== "" ? page.sioul.textWith("supplies-buy-at", "price", row.modelData.buy) : "", row.modelData.sell !== "" ? page.sioul.textWith("supplies-sell-at", "price", row.modelData.sell) : ""].filter(t => t !== "").join(" · ") : ""

                            objectName: "supplyRow:" + row.modelData.id
                            Layout.fillWidth: true
                            implicitHeight: rowLayout.implicitHeight + 16
                            color: area.containsMouse ? page.theme.hover : page.theme.surface
                            border.color: page.theme.line
                            radius: 4
                            Accessible.role: Accessible.Button
                            Accessible.name: row.modelData.name + ", " + row.modelData.line

                            MouseArea {
                                id: area

                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: page.open(row.modelData.id)
                            }
                            // Its name, and what is free beside it; under it on a phone.
                            GridLayout {
                                id: rowLayout

                                anchors.fill: parent
                                anchors.margins: 8
                                columns: page.narrow ? 1 : 2
                                columnSpacing: 10
                                rowSpacing: 2

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 2

                                    Label {
                                        Layout.fillWidth: true
                                        text: row.modelData.name
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: page.theme.text
                                    }
                                    Label {
                                        visible: row.more !== ""
                                        Layout.fillWidth: true
                                        text: row.more
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        font.pixelSize: 12
                                        color: page.theme.muted
                                    }
                                }
                                Label {
                                    objectName: "supplyLine:" + row.modelData.id
                                    Layout.alignment: page.narrow ? Qt.AlignLeft : Qt.AlignRight
                                    text: row.modelData.line
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 13
                                    color: page.theme.muted
                                }
                            }
                        }
                    }
                }
            }
            // The calm view: those for other times, one line, the full view one tab away.
            Label {
                visible: page.tab === "now" && page.shown.elsewhere > 0
                Layout.fillWidth: true
                Layout.topMargin: 12
                text: page.sioul.textCounted("supplies-elsewhere", page.shown.elsewhere)
                wrapMode: Text.Wrap
                font.pixelSize: 13
                color: page.theme.muted
            }
            // The archived, folded under the full view, each with its card.
            BareButton {
                visible: page.tab === "all" && page.shown.archived.length > 0
                Layout.topMargin: 12
                flat: true
                text: (page.archivedShown ? "▾  " : "▸  ") + page.sioul.textCounted("supplies-archived", page.shown.archived.length)
                onClicked: page.archivedShown = !page.archivedShown
            }
            Repeater {
                model: page.tab === "all" && page.archivedShown ? page.shown.archived : []

                delegate: Button {
                    required property var modelData

                    objectName: "supplyArchived:" + modelData.id
                    Layout.fillWidth: true
                    flat: true
                    text: page.theme.plain(modelData.name)
                    onClicked: page.open(modelData.id)
                }
            }
        }
    }

    Later {
        id: supplyCard

        sourceComponent: Component {
            SupplyCard {
                sioul: page.sioul
                theme: page.theme
                window: page.window
                categories: page.shown.categories
                projects: page.shown.projects
                onSaved: page.reload()
            }
        }
    }
}
