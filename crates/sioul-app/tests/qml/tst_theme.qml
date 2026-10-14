// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Whether Sioul's window is dark (Theme.qml, `darkFrom`): the colours chosen
// in Settings first; else, with high contrast on, the system's own colours,
// by their lightness; else the system's colour scheme when it says one,
// which Windows 10 says dark while its style's palette is light; else the
// palette's lightness. A Theme bound as main.qml binds it follows each of
// them as they change while Sioul runs. Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

TestCase {
    id: root

    name: "Theme"

    Theme {
        id: theme
    }

    // What main.qml binds the window's Theme to, standing in for the system:
    // the setting, Qt.styleHints.colorScheme, the palette's window lightness,
    // and whether high contrast is on.
    QtObject {
        id: system

        property string chosen: ""
        property int scheme: Qt.ColorScheme.Unknown
        property real lightness: 0.94
        property bool highContrast: false
    }

    Theme {
        id: bound

        property var seen: []

        dark: bound.darkFrom(system.chosen, system.scheme, system.lightness, system.highContrast)
        onDarkChanged: bound.seen = bound.seen.concat([bound.dark])
    }

    function test_the_colours_chosen_come_first() {
        for (const scheme of [Qt.ColorScheme.Unknown, Qt.ColorScheme.Light, Qt.ColorScheme.Dark]) {
            for (const lightness of [0.05, 0.94]) {
                for (const highContrast of [false, true]) {
                    compare(theme.darkFrom("dark", scheme, lightness, highContrast), true)
                    compare(theme.darkFrom("light", scheme, lightness, highContrast), false)
                }
            }
        }
    }

    function test_the_systems_colour_scheme_before_its_palette() {
        // Windows 10 in dark mode: the scheme dark, the Vista style's palette light.
        compare(theme.darkFrom("", Qt.ColorScheme.Dark, 0.94, false), true)
        compare(theme.darkFrom("", Qt.ColorScheme.Light, 0.12, false), false)
        // No scheme said (a bare window manager, offscreen): the palette.
        compare(theme.darkFrom("", Qt.ColorScheme.Unknown, 0.12, false), true)
        compare(theme.darkFrom("", Qt.ColorScheme.Unknown, 0.94, false), false)
        // Mid-grey is light, as before.
        compare(theme.darkFrom("", Qt.ColorScheme.Unknown, 0.5, false), false)
    }

    function test_high_contrast_keeps_the_systems_own_colours() {
        // Its black and its white, by their lightness, whatever the scheme says
        // (Windows says none then; macOS's more contrast keeps its own).
        compare(theme.darkFrom("", Qt.ColorScheme.Unknown, 0.0, true), true)
        compare(theme.darkFrom("", Qt.ColorScheme.Unknown, 1.0, true), false)
        compare(theme.darkFrom("", Qt.ColorScheme.Light, 0.0, true), true)
        compare(theme.darkFrom("", Qt.ColorScheme.Dark, 1.0, true), false)
    }

    function test_a_change_while_sioul_runs_is_followed() {
        compare(bound.dark, false)
        bound.seen = []
        // The system turns dark at night, its palette left light (Windows 10).
        system.scheme = Qt.ColorScheme.Dark
        compare(bound.dark, true)
        // Light again.
        system.scheme = Qt.ColorScheme.Light
        compare(bound.dark, false)
        // High contrast on, its colours black: dark, the scheme no longer read.
        system.lightness = 0.0
        system.highContrast = true
        compare(bound.dark, true)
        // Light colours chosen in Settings.
        system.chosen = "light"
        compare(bound.dark, false)
        // Back to the system's.
        system.chosen = ""
        compare(bound.dark, true)
        compare(bound.seen, [true, false, true, false, true])
    }
}
