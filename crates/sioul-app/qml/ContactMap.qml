// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// OpenStreetMap, with a pin for each place given; a click on a pin says
// whose it is. Tiles come from the server set in the Contacts settings
// (OpenStreetMap's by default), asked by name, sparingly, and kept by Qt's
// cache; Qt's own lookup of map providers is turned off.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtLocation
import QtPositioning

MapView {
    id: view

    required property var theme
    // "https://tile.openstreetmap.org/".
    property string tiles: "https://tile.openstreetmap.org/"
    // [{ key, name, address, lat, lon }].
    property var pins: []
    property string chosen: ""

    signal pinClicked(string key)

    // Every pin in view: the map centred on them, as close as they allow.
    function fit() {
        if (view.pins.length === 0)
            return
        if (view.pins.length === 1) {
            view.map.center = QtPositioning.coordinate(view.pins[0].lat, view.pins[0].lon)
            view.map.zoomLevel = 15
            return
        }
        let shape = QtPositioning.rectangle()
        const lats = view.pins.map(p => p.lat)
        const lons = view.pins.map(p => p.lon)
        shape = QtPositioning.rectangle(QtPositioning.coordinate(Math.max(...lats), Math.min(...lons)), QtPositioning.coordinate(Math.min(...lats), Math.max(...lons)))
        view.map.visibleRegion = shape
        view.map.zoomLevel = Math.min(view.map.zoomLevel - 0.3, 16)
    }

    map.plugin: Plugin {
        name: "osm"

        PluginParameter {
            name: "osm.useragent"
            value: "Sioul (desktop mail and contacts client)"
        }
        PluginParameter {
            name: "osm.mapping.providersrepository.disabled"
            value: true
        }
        PluginParameter {
            name: "osm.mapping.custom.host"
            value: view.tiles
        }
    }
    // The map of the tile server set: the plugin's custom one.
    map.activeMapType: {
        const types = view.map.supportedMapTypes
        for (let i = 0; i < types.length; ++i)
            if (types[i].style === MapType.CustomMap)
                return types[i]
        return types.length > 0 ? types[0] : view.map.activeMapType
    }
    map.zoomLevel: 5
    map.center: QtPositioning.coordinate(46.6, 2.4)
    onPinsChanged: Qt.callLater(view.fit)
    onWidthChanged: Qt.callLater(view.fit)
    Component.onCompleted: Qt.callLater(view.fit)

    MapItemView {
        parent: view.map
        model: view.pins

        delegate: MapQuickItem {
            id: marker

            required property var modelData

            coordinate: QtPositioning.coordinate(marker.modelData.lat, marker.modelData.lon)
            anchorPoint.x: pin.width / 2
            anchorPoint.y: pin.height

            sourceItem: Item {
                id: pin

                width: 22
                height: 30

                Rectangle {
                    width: 22
                    height: 22
                    radius: 11
                    color: view.chosen === marker.modelData.key ? view.theme.warm : view.theme.accent
                    border.color: view.theme.surface
                    border.width: 2
                }
                Rectangle {
                    x: 9
                    y: 20
                    width: 4
                    height: 10
                    color: view.chosen === marker.modelData.key ? view.theme.warm : view.theme.accent
                }
                ToolTip.visible: hover.hovered
                ToolTip.text: view.theme.plain(marker.modelData.name + "\n" + marker.modelData.address)
                ToolTip.delay: 200

                HoverHandler {
                    id: hover
                }
                TapHandler {
                    onTapped: view.pinClicked(marker.modelData.key)
                }
            }
        }
    }

    // OpenStreetMap's contributors, as their licence asks.
    Label {
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 2
        padding: 2
        text: "© OpenStreetMap"
        font.pixelSize: 10
        color: "#333333"
        background: Rectangle {
            color: "#ccffffff"
        }
    }
}
