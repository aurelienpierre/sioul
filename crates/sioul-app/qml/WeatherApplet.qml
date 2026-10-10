// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The weather in the status line, in one colour, a button of the line
// (LineButton.qml): now, then hour by hour for the next two hours, each as an
// icon and a temperature (now alone when the window is narrow), the line's
// room kept for the rest; a tip says what they are. On a click, the next four
// hours, then the parts of the days to come, and where the data comes from.
// Without a place, a quiet icon that offers to choose one.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

LineButton {
    id: applet

    required property var sioul
    // Its pop-up under it, the line at the window's top (a computer's title bar); else above it.
    property bool below: false
    readonly property var shown: applet.sioul.forecast ? JSON.parse(applet.sioul.forecast) : null
    readonly property var now: applet.shown && applet.shown.view.now ? applet.shown.view.now : null
    // The next two hours beside now, when the window has room for them (a
    // title bar shares its width with the window's buttons); the rest on a click.
    readonly property var hours: applet.now && !applet.compact && applet.Window.width >= (applet.below ? 1200 : 900) ? applet.shown.view.hours.slice(0, 2) : []
    property bool choosing: false
    readonly property var places: applet.sioul.placesFound ? JSON.parse(applet.sioul.placesFound) : []

    // For the window's images: the forecast open, then as an image.
    function openForecast() {
        applet.choosing = false
        applet.openPopup()
    }

    function grab(path) {
        popupLoader.item.contentItem.grabToImage(result => result.saveToFile(path))
    }

    function openPopup() {
        popupLoader.active = true
        popupLoader.item.open()
    }

    // Its icon and the temperature, narrow too.
    display: AbstractButton.TextBesideIcon
    icon.name: applet.now ? applet.now.icon : "weather-none-available-symbolic"
    text: applet.now ? applet.now.temperature : ""
    name: applet.now ? [applet.now].concat(applet.hours).map(s => s.label + ": " + s.words + ", " + s.temperature).join("; ") : applet.sioul.text("weather-choose")
    tip: applet.now ? applet.sioul.textWith(applet.hours.length > 0 ? "weather-tip-hours" : "weather-tip", "place", applet.shown.place) + "\n" + applet.now.words : applet.sioul.text("weather-choose")
    menuOpen: popupLoader.item !== null && (popupLoader.item as Popup).opened
    onChosen: {
        applet.choosing = applet.shown === null
        applet.openPopup()
    }

    contentItem: RowLayout {
        spacing: 4

        Label {
            visible: applet.hours.length > 0
            text: applet.now ? applet.now.label : ""
            textFormat: Text.PlainText
            font.pixelSize: 11
            color: applet.theme.muted
        }
        Icon {
            iconName: applet.icon.name
            color: applet.theme.muted
            size: 16
        }
        Label {
            visible: applet.text !== ""
            text: applet.text
            textFormat: Text.PlainText
            color: applet.theme.muted
            font.features: { "tnum": 1 }
        }
        // Hour by hour: its time, small, then the same icon and temperature.
        Repeater {
            model: applet.hours

            delegate: RowLayout {
                id: hour

                required property var modelData

                Layout.leftMargin: 8
                spacing: 3

                Label {
                    text: hour.modelData.label
                    textFormat: Text.PlainText
                    font.pixelSize: 11
                    font.features: { "tnum": 1 }
                    color: applet.theme.muted
                }
                Icon {
                    iconName: hour.modelData.icon
                    color: applet.theme.muted
                    size: 14
                }
                Label {
                    text: hour.modelData.temperature
                    textFormat: Text.PlainText
                    color: applet.theme.muted
                    font.features: { "tnum": 1 }
                }
            }
        }
    }

    // Made the first time it opens, its rows with it.
    Loader {
        id: popupLoader

        active: false
        sourceComponent: Popup {
            id: popup

            y: applet.below ? applet.height + 6 : -height - 6
            x: Math.min(0, applet.parent ? applet.parent.width - applet.x - width : 0)
            width: 360
            padding: 12
            modal: false
            closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
            onClosed: applet.choosing = false

            background: Rectangle {
                color: applet.theme.surface
                radius: applet.theme.radius
                border.color: applet.theme.line
            }

            contentItem: ColumnLayout {
                spacing: 6

                // The forecast.
                ColumnLayout {
                    visible: !applet.choosing && applet.shown !== null
                    Layout.fillWidth: true
                    spacing: 4

                    Label {
                        Layout.fillWidth: true
                        text: applet.shown ? applet.shown.place : ""
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.weight: Font.DemiBold
                        color: applet.theme.text
                    }
                    Repeater {
                        model: applet.shown ? (applet.now ? [applet.now] : []).concat(applet.shown.view.hours).concat(applet.shown.view.parts) : []

                        delegate: RowLayout {
                            id: slot

                            required property var modelData

                            Layout.fillWidth: true
                            spacing: 8

                            Label {
                                Layout.preferredWidth: 150
                                text: slot.modelData.label
                                textFormat: Text.PlainText
                                elide: Text.ElideRight
                                color: applet.theme.muted
                            }
                            Icon {
                                iconName: slot.modelData.icon
                                color: applet.theme.text
                                size: 16
                                tip: slot.modelData.words
                            }
                            Label {
                                Layout.fillWidth: true
                                text: slot.modelData.temperature
                                textFormat: Text.PlainText
                                color: applet.theme.text
                                font.features: { "tnum": 1 }
                            }
                            Label {
                                text: slot.modelData.rain
                                textFormat: Text.PlainText
                                color: applet.theme.muted
                                font.features: { "tnum": 1 }
                            }
                        }
                    }
                    Label {
                        visible: applet.shown !== null && applet.now === null
                        text: applet.sioul.text("weather-none")
                        color: applet.theme.muted
                    }
                    // Credited where shown, as its licence asks.
                    Label {
                        Layout.topMargin: 4
                        text: "<a href=\"https://open-meteo.com/\">" + applet.theme.escaped(applet.shown ? applet.shown.credit : "") + "</a>"
                        textFormat: Text.RichText
                        font.pixelSize: 11
                        color: applet.theme.muted
                        linkColor: applet.theme.muted
                        onLinkActivated: link => Qt.openUrlExternally(link)
                    }
                    Button {
                        flat: true
                        text: applet.sioul.text("weather-change")
                        onClicked: applet.choosing = true
                    }
                }

                // Choosing a place: a name, then the place among its namesakes.
                ColumnLayout {
                    visible: applet.choosing || applet.shown === null
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        text: applet.sioul.text("weather-choose")
                        font.weight: Font.DemiBold
                        color: applet.theme.text
                    }
                    TextField {
                        id: placeName

                        Layout.fillWidth: true
                        placeholderText: applet.sioul.text("weather-search")
                        onAccepted: {
                            if (placeName.text.trim() !== "")
                                applet.sioul.findPlaces(placeName.text.trim())
                        }
                    }
                    Repeater {
                        model: Array.isArray(applet.places) ? applet.places : []

                        delegate: ItemDelegate {
                            id: found

                            required property var modelData

                            Layout.fillWidth: true
                            text: applet.theme.plain(found.modelData.name).replace(/&/g, "&&")
                            onClicked: {
                                applet.sioul.setWeatherPlace(found.modelData.name, found.modelData.latitude, found.modelData.longitude)
                                applet.choosing = false
                                popup.close()
                            }
                        }
                    }
                    Label {
                        visible: !Array.isArray(applet.places) && applet.places.error !== undefined
                        Layout.fillWidth: true
                        text: Array.isArray(applet.places) ? "" : applet.places.error || ""
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: applet.theme.warm
                    }
                    Label {
                        Layout.fillWidth: true
                        text: applet.sioul.text("weather-privacy")
                        wrapMode: Text.Wrap
                        font.pixelSize: 11
                        color: applet.theme.muted
                    }
                }
            }
        }
    }
}
