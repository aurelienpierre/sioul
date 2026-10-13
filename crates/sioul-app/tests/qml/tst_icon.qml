// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// An icon in its own colours (Icon.qml) is read from the set of the bundled
// icons drawn for its window's colours, light or dark (desktop.rs, `follow`):
// it reads its name once as it shows, then again each time the window turns
// dark or light, and only then. Its window is a Window of its own, which has
// a theme as Sioul's windows have. Run by tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

Item {
    id: root

    width: 100
    height: 100

    Window {
        id: shown

        property var theme: QtObject {
            property bool dark: true
        }

        width: 120
        height: 120
        visible: true

        Column {
            Icon {
                id: one

                property var names: []

                iconName: "list-add"
                onNameChanged: one.names = one.names.concat([one.name])
            }
            Repeater {
                id: many

                model: 2

                delegate: Icon {
                    property var names: []

                    iconName: "go-next"
                    onNameChanged: names = names.concat([name])
                }
            }
            Loader {
                id: later

                active: false

                sourceComponent: Icon {
                    property var names: []

                    iconName: "go-up"
                    onNameChanged: names = names.concat([name])
                }
            }
        }
    }

    TestCase {
        name: "Icon"
        when: windowShown

        function test_read_once_as_it_shows_then_again_at_each_change() {
            compare(one.names, ["list-add"], "read once, in a window dark from the start")
            compare(many.itemAt(1).names, ["go-next"], "a delegate too")
            later.active = true
            tryVerify(() => later.item !== null)
            wait(50)
            compare(later.item.names, ["go-up"], "and an icon made later, in that window")
            shown.theme.dark = false
            tryCompare(one, "names", ["list-add", "", "list-add"])
            tryCompare(many.itemAt(0), "names", ["go-next", "", "go-next"])
            tryCompare(later.item, "names", ["go-up", "", "go-up"])
            shown.theme.dark = true
            tryCompare(one, "names", ["list-add", "", "list-add", "", "list-add"])
            wait(50)
            compare(later.item.names, ["go-up", "", "go-up", "", "go-up"], "read again at each change, once")
        }
    }
}
