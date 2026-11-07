// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The hour's panel and the date's month on a touch screen (ClockField.qml,
// DateField.qml), a window a phone wide: a bottom sheet over the whole window,
// cells 44 points tall or more, the field left without the focus so that no soft
// keyboard comes until the field itself is tapped, a tap outside keeps the value
// and Back gives back the earlier one; the two grids side by side where the
// sheet is wide enough, a phone held sideways. A day picked opens the hour's
// sheet. TalkBack itself is not run here. Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 360
    height: 640

    Theme {
        id: testTheme

        touch: true
    }

    QtObject {
        id: words

        function text(key) {
            return key === "qt-locale" ? "en_GB" : key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
    }

    Component {
        id: fieldComponent

        ClockField {
            x: 20
            y: 20
            theme: testTheme
            sioul: words
        }
    }

    Component {
        id: dateComponent

        DateField {
            x: 20
            y: 60
            theme: testTheme
            sioul: words
            locale: Qt.locale("en_GB")
            date: "2026-11-05"
        }
    }

    // A form over the window, as a phone's dialogs are.
    Component {
        id: dialogComponent

        SafeDialog {
            property alias hour: inside

            parent: Overlay.overlay
            modal: true
            width: 300
            height: 200

            contentItem: Item {
                ClockField {
                    id: inside

                    theme: testTheme
                    sioul: words
                    text: "08:15"
                }
            }
        }
    }

    TestCase {
        name: "ClockSheet"
        when: windowShown

        function find(item, name) {
            if (!item)
                return null
            if (item.objectName === name)
                return item
            const kids = item.children || []
            for (let i = 0; i < kids.length; ++i) {
                const found = find(kids[i], name)
                if (found)
                    return found
            }
            return null
        }

        function cell(field, name) {
            return find(field.panel.contentItem, name)
        }

        function openField(text) {
            const field = createTemporaryObject(fieldComponent, root, { text: text })
            mouseClick(find(field, "clockButton"))
            verify(field.panelOpen, "the button opens the sheet")
            return field
        }

        // A day's cell of the month, by its number in the month shown.
        function dayCell(month, day) {
            const kids = month.contentItem.children
            for (let i = 0; i < kids.length; ++i)
                if (kids[i].model && kids[i].model.day === day && kids[i].model.month === month.month)
                    return kids[i]
            return null
        }

        // A bottom sheet: as wide as the window, down at its foot, the form dimmed under it.
        function test_a_bottom_sheet() {
            const field = openField("08:15")
            const sheet = field.panel
            compare(sheet.parent, sheet.Overlay.overlay, "over the whole window")
            compare(sheet.width, root.width)
            compare(sheet.y + sheet.height, root.height, "at the window's foot")
            verify(sheet.modal)
            verify(sheet.height < root.height, "the grids fit a phone upright: " + sheet.height)
        }

        // Cells a fingertip wide, all of them; 6 columns within the phone's width.
        function test_cells_a_fingertip_wide() {
            const field = openField("08:15")
            for (const name of ["clockHour0", "clockHour23", "clockMinute0", "clockMinute55", "clockMinus", "clockPlus"]) {
                const it = cell(field, name)
                verify(it.height >= 44, name + " " + it.height)
                verify(it.width >= 44, name + " " + it.width)
            }
            const hours = find(field.panel.contentItem, "clockHours")
            verify(hours.width <= root.width, "within the phone: " + hours.width)
        }

        // No soft keyboard: the sheet opens without the focus in the field. The field had
        // it, the keyboard goes, and it does not come back as the sheet closes.
        function test_no_keyboard_until_the_field_is_tapped() {
            const field = openField("08:15")
            verify(!field.activeFocus, "no focus, no keyboard")
            mouseClick(cell(field, "clockHour14"))
            verify(!field.activeFocus)
            compare(field.text, "14:15")
            field.panel.close()
            verify(!field.activeFocus, "not when it closes either")

            const tapped = createTemporaryObject(fieldComponent, root, { text: "09:00" })
            mouseClick(tapped, 10, tapped.height / 2)
            verify(tapped.activeFocus, "the field tapped takes the focus: the keyboard's turn")
            verify(!tapped.panelOpen, "and opens no sheet")
            mouseClick(find(tapped, "clockButton"))
            verify(tapped.panelOpen)
            verify(!tapped.activeFocus, "the sheet sends the keyboard away")
            tapped.panel.close()
            verify(!tapped.activeFocus, "and it does not come back")
        }

        function test_a_tap_outside_keeps_and_back_gives_back() {
            const field = openField("08:15")
            mouseClick(cell(field, "clockHour14"))
            compare(field.text, "14:15")
            verify(field.panelOpen, "applied, still open")
            mouseClick(root, root.width / 2, 5)
            verify(!field.panelOpen, "a tap above the sheet closes it")
            compare(field.text, "14:15", "kept")

            mouseClick(find(field, "clockButton"))
            mouseClick(cell(field, "clockHour9"))
            compare(field.text, "09:15")
            keyClick(Qt.Key_Back)
            verify(!field.panelOpen, "Back closes it")
            compare(field.text, "14:15", "the earlier one")

            mouseClick(find(field, "clockButton"))
            mouseClick(cell(field, "clockMinute30"))
            keyClick(Qt.Key_Escape)
            verify(!field.panelOpen, "Esc too")
            compare(field.text, "14:15")
        }

        // Over a form: a tap outside the sheet closes the sheet alone, Back too.
        function test_over_a_form_it_closes_alone() {
            const dialog = createTemporaryObject(dialogComponent, root)
            dialog.open()
            tryCompare(dialog, "opened", true)
            const field = dialog.hour
            mouseClick(find(field, "clockButton"))
            verify(field.panelOpen)
            mouseClick(cell(field, "clockHour14"))
            compare(field.text, "14:15")
            mouseClick(root, root.width / 2, 5)
            verify(!field.panelOpen, "the sheet closes")
            verify(dialog.opened, "the form stays")
            compare(field.text, "14:15")
            mouseClick(find(field, "clockButton"))
            verify(field.panelOpen)
            keyClick(Qt.Key_Back)
            verify(!field.panelOpen, "Back closes the sheet")
            verify(dialog.opened, "the form stays")
            dialog.close()
        }

        // Saved as it closes: a phone's field has no focus to lose.
        function test_editing_finished_when_it_closes() {
            const field = openField("08:15")
            let finished = 0
            field.editingFinished.connect(() => finished++)
            mouseClick(cell(field, "clockHour14"))
            compare(finished, 0)
            mouseClick(root, root.width / 2, 5)
            compare(finished, 1)
        }

        // A phone held sideways: the grids side by side, the sheet within the screen's height.
        function test_side_by_side_on_a_wide_sheet() {
            const window = root.Window.window
            const width = window.width
            const height = window.height
            window.width = 780
            window.height = 360
            root.width = 780
            root.height = 360
            tryCompare(root.Overlay.overlay, "width", 780)
            const field = openField("08:15")
            const sheet = field.panel
            compare(sheet.sideBySide, true)
            verify(sheet.height <= 360, "within the screen: " + sheet.height)
            const hours = find(sheet.contentItem, "clockHours")
            const minutes = find(sheet.contentItem, "clockMinutes")
            verify(minutes.mapToItem(null, 0, 0).x >= hours.mapToItem(null, 0, 0).x + hours.width, "minutes beside the hours")
            verify(cell(field, "clockHour0").height >= 44)
            verify(cell(field, "clockHour0").width >= 44)
            sheet.close()
            window.width = width
            window.height = height
            root.width = 360
            root.height = 640
        }

        // The month too is a sheet with no keyboard, its days a fingertip tall; a day closes it.
        function test_the_month_is_a_sheet() {
            const date = createTemporaryObject(dateComponent, root)
            mouseClick(find(date, "dateButton"))
            verify(date.monthOpen)
            const popup = date.monthPopup
            compare(popup.parent, popup.Overlay.overlay)
            compare(popup.width, root.width)
            compare(popup.y + popup.height, root.height)
            verify(popup.modal)
            verify(popup.height < root.height, "a month fits a phone upright: " + popup.height)
            const month = find(popup.contentItem, "dateMonth")
            tryVerify(() => dayCell(month, 10) !== null)
            verify(dayCell(month, 10).height >= 44, "a fingertip tall: " + dayCell(month, 10).height + ", the month " + month.height)
            verify(dayCell(month, 10).width >= 44, "a fingertip wide: " + dayCell(month, 10).width)
            mouseClick(dayCell(month, 10))
            compare(date.date, "2026-11-10")
            verify(!date.monthOpen)
        }

        // A phone's next step after a day: the hour's sheet, never the keyboard.
        function test_a_day_opens_the_hour_s_sheet() {
            const field = createTemporaryObject(fieldComponent, root, { text: "08:15" })
            const date = createTemporaryObject(dateComponent, root, { hourField: field })
            mouseClick(find(date, "dateButton"))
            const month = find(date.monthPopup.contentItem, "dateMonth")
            tryVerify(() => dayCell(month, 10) !== null)
            mouseClick(dayCell(month, 10))
            verify(!date.monthOpen)
            verify(field.panelOpen, "the hour's sheet")
            verify(!field.activeFocus, "with no keyboard")
            mouseClick(cell(field, "clockHour14"))
            compare(field.text, "14:15")
            field.panel.close()
        }
    }
}
