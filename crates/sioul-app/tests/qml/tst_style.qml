// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Fields and buttons that stand out (Theme.qml's `field` and `edge`, and
// Sioul's overrides of the Basic style in qml/style/, which cpp/style.cpp
// gives in place of Basic's own). In both themes: the text on a field reads
// at 4.5:1 or better, its placeholder too; the field's fill is not the
// cards'; the edge stands at 3:1 or more against the field, the cards, the
// page and a filled button. In a window of Sioul's (a `theme`): a flat
// button at rest keeps an outline in the edge, while a filled, checked or
// pressed one is drawn as Basic draws it; a text field and a text area one
// can type in have the field's fill and the edge, a read-only text area
// none; every size is Basic's; a field's menu (cut, copy, paste) comes at
// a right click. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic as Basic
import QtQuick.Templates as T
import QtTest
import "../../qml"
import "../../qml/style"

Item {
    id: root

    width: 600
    height: 400

    Theme {
        id: light
    }
    Theme {
        id: dark

        dark: true
    }

    // A window of Sioul's stands in: its `theme` is what the overrides read.
    Window {
        id: sioulWindow

        readonly property var theme: light

        width: 600
        height: 400
        visible: true
        palette.base: light.field

        Column {
            spacing: 8

            StyleButton {
                id: flat

                text: "Forget it"
                flat: true
            }
            StyleButton {
                id: flatChecked

                text: "Week"
                flat: true
                checkable: true
                checked: true
            }
            StyleButton {
                id: filled

                text: "Save"
            }
            StyleTextField {
                id: field

                placeholderText: "A name"
            }
            StyleTextArea {
                id: area

                text: "Some notes"
            }
            StyleTextArea {
                id: readOnlyArea

                text: "A link to copy"
                readOnly: true
            }
            // Basic's own, to measure against.
            Basic.Button {
                id: basicButton

                text: "Forget it"
                flat: true
            }
            Basic.TextField {
                id: basicField

                placeholderText: "A name"
            }
            Basic.TextArea {
                id: basicArea

                text: "Some notes"
            }
        }
    }

    TestCase {
        name: "Style"
        when: windowShown

        function lin(c) {
            return c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
        }
        function luminance(c) {
            return 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
        }
        function contrast(a, b) {
            const la = luminance(a), lb = luminance(b)
            return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05)
        }

        function test_contrast_in_both_themes_data() {
            return [{ tag: "light", theme: light }, { tag: "dark", theme: dark }]
        }

        function test_contrast_in_both_themes(data) {
            const t = data.theme
            verify(contrast(t.text, t.field) >= 4.5, "text on a field: " + contrast(t.text, t.field).toFixed(2))
            verify(contrast(t.muted, t.field) >= 4.5, "a placeholder on a field: " + contrast(t.muted, t.field).toFixed(2))
            verify(!Qt.colorEqual(t.field, t.surface), "the field's fill is not the cards'")
            verify(contrast(t.field, t.surface) >= 1.1, "the field apart from the cards: " + contrast(t.field, t.surface).toFixed(2))
            for (const ground of ["field", "surface", "background", "button"])
                verify(contrast(t.edge, t[ground]) >= 3, "the edge against the " + ground + ": " + contrast(t.edge, t[ground]).toFixed(2))
        }

        function test_a_flat_button_keeps_an_outline() {
            verify(flat.background.visible, "drawn at rest")
            compare(flat.background.color.a, 0, "no fill")
            compare(flat.background.border.width, 1)
            verify(Qt.colorEqual(flat.background.border.color, light.edge), "in the theme's edge")
            // Checked or filled: Basic's fill, no outline.
            verify(flatChecked.background.visible)
            compare(flatChecked.background.border.width, 0)
            verify(flatChecked.background.color.a > 0, "the checked one filled")
            compare(filled.background.border.width, 0)
            verify(filled.background.color.a > 0, "a plain button filled")
            // Basic's size.
            compare(flat.implicitWidth, basicButton.implicitWidth)
            compare(flat.implicitHeight, basicButton.implicitHeight)
        }

        function test_fields_have_a_fill_and_an_edge() {
            verify(Qt.colorEqual(field.background.color, light.field), "a text field: the field's fill")
            verify(Qt.colorEqual(field.background.border.color, light.edge), "and the edge")
            compare(field.background.border.width, 1)
            compare(field.implicitWidth, basicField.implicitWidth)
            compare(field.implicitHeight, basicField.implicitHeight)
            verify(area.background.visible, "a text area one types in: drawn")
            verify(Qt.colorEqual(area.background.color, light.field), "its fill")
            verify(Qt.colorEqual(area.background.border.color, light.edge), "its edge")
            compare(area.implicitWidth, basicArea.implicitWidth)
            compare(area.implicitHeight, basicArea.implicitHeight)
            verify(!readOnlyArea.background.visible, "a read-only one: bare words")
        }

        // Cut, copy and paste, made at the first right click (Qt 6.11's own menu).
        function test_a_field_s_menu_comes_at_a_right_click() {
            compare(field.T.ContextMenu.menu, null, "none made before it is asked")
            mouseClick(field, 20, 10, Qt.RightButton)
            tryVerify(() => field.T.ContextMenu.menu !== null, 1000, "made")
            tryVerify(() => field.T.ContextMenu.menu.visible, 1000, "open")
            field.T.ContextMenu.menu.close()
        }
    }
}
