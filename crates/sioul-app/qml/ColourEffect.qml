// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// colour: a site's picture in the screen's own colours, and calmer when asked
// (docs/colour.md). A WebEngineView's layer.effect, while
// ScreenColours.active(Screen.name) says its tables change something: each
// pixel is looked up in the two tables ScreenColours made for the screen the
// window is on (cpp/screencolours.cpp, colour.rs), by shaders/colour.frag.

import QtQuick
import com.aurelienpierre.sioul

ShaderEffect {
    id: effect

    // The picture, given by the layer.
    property var source
    // The tables, as textures (shaders/colour.frag): the cube, 33 tiles of
    // 33 × 33 side by side, and the curve.
    readonly property Image cube: cubeImage
    readonly property Image curve: curveImage
    readonly property real tableSize: ScreenColours.cubeSize

    fragmentShader: "qrc:/sioul/shaders/colour.frag.qsb"

    // Hidden: only their textures are read, filtered linearly (smooth). Their
    // address changes with the tables (ScreenColours.revision).
    Image {
        id: cubeImage

        visible: false
        smooth: true
        cache: false
        asynchronous: false
        source: ScreenColours.revision >= 0 ? ScreenColours.source("cube", Screen.name) : ""
    }
    Image {
        id: curveImage

        visible: false
        smooth: true
        cache: false
        asynchronous: false
        source: ScreenColours.revision >= 0 ? ScreenColours.source("curve", Screen.name) : ""
    }
}
