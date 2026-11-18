// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Fields and buttons that stand out (Theme.qml's `field`, `edge` and
// `focusEdge`, and Sioul's overrides of the Basic style in qml/style/, which
// cpp/style.cpp gives in place of Basic's own; BareButton.qml). In both
// themes: the text on a field reads at 4.5:1 or better, its placeholder too;
// the field's fill is not the cards'; the edge stands at 3:1 or more against
// the field, the cards, the page and a filled button; the focused field's
// edge at 3:1 or more against the edge at rest and the fill. In a window of
// Sioul's (a `theme`): a flat button at rest that holds words keeps an outline
// in the edge, its icon tinted as its words; a flat button of an icon alone,
// or of an arrow alone, stays bare, as does a BareButton; a filled, checked or
// pressed one is drawn as Basic draws it; a text field, a text area one can
// type in, a number box and the boxes to tick have the field's fill and the
// edge, a read-only text area none; with the focus, a field's edge is 2 points
// of the focused edge; every size is Basic's; a field's menu (cut, copy,
// paste) comes at a right click, made then. Run by tools/qml-test.sh.

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

        width: 700
        height: 900
        visible: true
        palette.base: light.field

        Column {
            spacing: 8

            StyleButton {
                id: flat

                text: "Forget it"
                flat: true
                icon.name: "edit-delete"
            }
            StyleButton {
                id: iconAlone

                text: "Delete"
                flat: true
                icon.name: "edit-delete"
                display: T.AbstractButton.IconOnly
            }
            StyleButton {
                id: arrowAlone

                text: "▸"
                flat: true
            }
            BareButton {
                id: bare

                text: "▸  Server and folders"
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
            StyleCheckBox {
                id: box

                text: "Done too"
            }
            StyleRadioButton {
                id: radio

                text: "Once"
            }
            StyleSpinBox {
                id: spin

                value: 3
            }
            // Basic's own, to measure against.
            Basic.Button {
                id: basicButton

                text: "Forget it"
                flat: true
                icon.name: "edit-delete"
            }
            Basic.Button {
                id: basicBare

                text: "▸  Server and folders"
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
            Basic.CheckBox {
                id: basicBox

                text: "Done too"
            }
            Basic.RadioButton {
                id: basicRadio

                text: "Once"
            }
            Basic.SpinBox {
                id: basicSpin

                value: 3
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
            verify(contrast(t.focusEdge, t.edge) >= 3, "the focused edge against the edge at rest: " + contrast(t.focusEdge, t.edge).toFixed(2))
            verify(contrast(t.focusEdge, t.field) >= 3, "the focused edge against the fill: " + contrast(t.focusEdge, t.field).toFixed(2))
        }

        function test_a_flat_button_with_words_keeps_an_outline() {
            verify(flat.background.visible, "drawn at rest")
            compare(flat.background.color.a, 0, "no fill")
            compare(flat.background.border.width, 1)
            verify(Qt.colorEqual(flat.background.border.color, light.edge), "in the theme's edge")
            // Its icon tinted as its words: Qt 6.9 and 6.10's way, which 6.11 reads too.
            verify(Qt.colorEqual(flat.icon.color, flat.contentItem.color), "the icon tinted as the words")
            verify(Qt.colorEqual(flat.icon.color, basicButton.contentItem.color), "as Basic tints them")
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

        function test_an_icon_an_arrow_and_a_bare_button_stay_bare() {
            verify(!iconAlone.background.visible, "an icon alone: no outline")
            verify(!arrowAlone.background.visible, "an arrow alone: no outline")
            verify(!bare.background.visible, "a fold line: no outline")
            compare(bare.implicitWidth, basicBare.implicitWidth, "a bare button's size is a button's")
            compare(bare.implicitHeight, basicBare.implicitHeight)
        }

        function test_fields_and_boxes_have_a_fill_and_an_edge() {
            // At rest: the focus elsewhere (the other tests give it to the fields).
            filled.forceActiveFocus()
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
            for (const ticked of [box, radio]) {
                verify(Qt.colorEqual(ticked.indicator.color, light.field), ticked.text + ": the field's fill")
                verify(Qt.colorEqual(ticked.indicator.border.color, light.edge), ticked.text + ": the edge")
            }
            compare(box.implicitWidth, basicBox.implicitWidth)
            compare(box.implicitHeight, basicBox.implicitHeight)
            compare(radio.implicitWidth, basicRadio.implicitWidth)
            verify(Qt.colorEqual(spin.background.color, light.field), "a number box: the field's fill")
            verify(Qt.colorEqual(spin.background.border.color, light.edge), "and the edge")
            compare(spin.implicitWidth, basicSpin.implicitWidth)
            compare(spin.implicitHeight, basicSpin.implicitHeight)
        }

        function test_a_focused_field_shows_it() {
            sioulWindow.requestActivate()
            tryVerify(() => sioulWindow.active, 2000, "the window active")
            field.forceActiveFocus()
            tryVerify(() => field.activeFocus, 2000)
            compare(field.background.border.width, 2)
            verify(Qt.colorEqual(field.background.border.color, light.focusEdge), "the focused edge")
            area.forceActiveFocus()
            tryVerify(() => area.activeFocus, 2000)
            compare(area.background.border.width, 2)
            verify(Qt.colorEqual(area.background.border.color, light.focusEdge), "the text area's too")
        }

        // Cut, copy and paste, made at the first right click (Qt 6.11's own menu).
        function test_a_field_s_menu_comes_at_a_right_click() {
            tryVerify(() => light.fieldMenu, 3000, "Qt's menu for fields found")
            compare(area.T.ContextMenu.menu, null, "none made before it is asked")
            mouseClick(area, 20, 10, Qt.RightButton)
            tryVerify(() => area.T.ContextMenu.menu !== null, 1000, "made")
            tryVerify(() => area.T.ContextMenu.menu.visible, 1000, "open")
            area.T.ContextMenu.menu.close()
        }
    }
}
