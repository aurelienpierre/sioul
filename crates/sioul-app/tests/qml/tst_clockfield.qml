// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Every form's one time field (ClockField.qml, the unified agenda, part 5), on
// a computer: "HH:MM" behind its mask; Up and Down by five minutes to the next
// whole five, Page Up and Page Down by an hour, around midnight, said as typed;
// what does not read left as it is; what is half typed read when the field is
// left. The panel, by pointer alone: an hour with no minutes yet gives :00, each
// click applies at once, a click outside keeps, Esc gives back, the keyboard
// never enters it. tst_clocksheet.qml holds the phone's sheet. Run by
// tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtTest
import "../../qml"

Item {
    id: root

    width: 600
    height: 500

    Theme {
        id: testTheme
    }

    // Sioul's words, in English; made up where Qt's names decide.
    QtObject {
        id: words

        function text(key) {
            return key === "qt-locale" ? "en_GB" : key
        }
        function textCounted(key, n) {
            return n + (key === "clock-hour-name" ? (n === 1 ? " hour" : " hours") : (n === 1 ? " minute" : " minutes"))
        }
    }

    Component {
        id: fieldComponent

        ClockField {}
    }

    // With its panel, under the top of the window, and a field after it for Tab.
    Component {
        id: panelComponent

        ClockField {
            x: 20
            y: 20
            theme: testTheme
            sioul: words
        }
    }

    Component {
        id: nextComponent

        TextField {
            x: 300
            y: 20
            width: 100
        }
    }

    // A field bound to its model, as the settings' rows are (`text: …`).
    QtObject {
        id: model

        property string value: "09:00"
    }

    Component {
        id: boundComponent

        ClockField {
            text: model.value
        }
    }

    // A field rebuilt when its value is saved, as a settings row's ranges are.
    Component {
        id: rebuiltComponent

        Item {
            id: rows

            property var ranges: ["09:00"]
            property Item row: repeater.count > 0 ? repeater.itemAt(0) : null

            Repeater {
                id: repeater

                model: rows.ranges

                delegate: ClockField {
                    required property string modelData

                    x: 20
                    y: 20
                    theme: testTheme
                    sioul: words
                    text: modelData
                    onEditingFinished: if (text !== modelData) rows.ranges = [text]
                }
            }
        }
    }

    TestCase {
        name: "ClockField"
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

        // A cell of the panel by its name ("clockHour14", "clockMinute25", "clockPlus").
        function cell(field, name) {
            return find(field.panel.contentItem, name)
        }

        function clickCell(field, name) {
            const it = cell(field, name)
            verify(it, name)
            mouseClick(it)
        }

        function type(keys) {
            for (const key of keys)
                keyClick(key)
        }

        function openField(text) {
            const field = createTemporaryObject(panelComponent, root, { text: text })
            field.forceActiveFocus(Qt.TabFocusReason)
            mouseClick(find(field, "clockButton"))
            verify(field.panelOpen, "the button opens the panel")
            return field
        }

        function test_up_and_down_by_five_minutes() {
            const field = createTemporaryObject(fieldComponent, root, { text: "09:03" })
            let edited = 0
            field.textEdited.connect(() => edited++)
            field.forceActiveFocus()
            keyClick(Qt.Key_Up)
            compare(field.text, "09:05", "to the next whole five")
            keyClick(Qt.Key_Up)
            compare(field.text, "09:10")
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Down)
            compare(field.text, "08:55")
            compare(edited, 5, "said as typed")
            field.text = "23:58"
            keyClick(Qt.Key_Up)
            compare(field.text, "00:00", "around midnight")
            keyClick(Qt.Key_Down)
            compare(field.text, "23:55")
            field.text = "25:00"
            keyClick(Qt.Key_Up)
            compare(field.text, "25:00", "what does not read stays as typed")
            field.text = "9:"
            keyClick(Qt.Key_Up)
            compare(field.text.replace(/ /g, ""), "9:", "half typed stays as typed")
            verify(field.inputMask.indexOf("99:99") === 0, field.inputMask)
        }

        function test_page_up_and_down_by_an_hour() {
            const field = createTemporaryObject(fieldComponent, root, { text: "14:32" })
            let edited = 0
            field.textEdited.connect(() => edited++)
            field.forceActiveFocus()
            keyClick(Qt.Key_PageUp)
            compare(field.text, "15:32", "the same minute")
            keyClick(Qt.Key_PageDown)
            keyClick(Qt.Key_PageDown)
            compare(field.text, "13:32")
            compare(edited, 3, "said as typed")
            field.text = "23:30"
            keyClick(Qt.Key_PageUp)
            compare(field.text, "00:30", "around midnight")
            keyClick(Qt.Key_PageDown)
            compare(field.text, "23:30")
            field.text = "25:00"
            keyClick(Qt.Key_PageUp)
            compare(field.text, "25:00", "what does not read stays as typed")
        }

        // An end field stops at 23:55, never wrapping to the day's start (review of
        // 21436bf, later 10).
        function test_an_end_stops_at_the_day_s_end() {
            const field = createTemporaryObject(fieldComponent, root, { text: "23:50", end: true })
            field.forceActiveFocus()
            keyClick(Qt.Key_Up)
            keyClick(Qt.Key_Up)
            compare(field.text, "23:55")
            field.text = "00:05"
            keyClick(Qt.Key_Down)
            keyClick(Qt.Key_Down)
            compare(field.text, "00:00")
            field.text = "23:30"
            keyClick(Qt.Key_PageUp)
            compare(field.text, "23:55", "an hour later stops there too")
            keyClick(Qt.Key_PageUp)
            compare(field.text, "23:55")
            field.text = "23:58"
            keyClick(Qt.Key_Up)
            compare(field.text, "23:58", "typed past the stop, it never steps back")
            field.text = "00:30"
            keyClick(Qt.Key_PageDown)
            compare(field.text, "00:00", "an hour earlier stops at the day's start")
        }

        // A field without a mask is stepped whole, never added to (review of 679f96a, later 11).
        function test_a_field_without_a_mask_steps_whole() {
            const field = createTemporaryObject(fieldComponent, root, { inputMask: "", text: "09:00" })
            field.forceActiveFocus()
            keyClick(Qt.Key_Up)
            compare(field.text, "09:05")
        }

        // A step keeps the field's binding: its model changed after, it shows the model's.
        function test_a_step_keeps_the_binding() {
            const field = createTemporaryObject(boundComponent, root)
            field.forceActiveFocus()
            keyClick(Qt.Key_Up)
            compare(field.text, "09:05")
            model.value = "14:30"
            compare(field.text, "14:30", "the model's, after a step")
            model.value = "09:00"
        }

        function test_what_is_typed_reads_data() {
            return [
                { tag: "four digits", typed: "1430", read: "14:30" },
                { tag: "a colon", typed: "14:30", read: "14:30" },
                { tag: "h", typed: "14h30", read: "14:30" },
                { tag: "H", typed: "14H30", read: "14:30" },
                { tag: "a dot", typed: "14.30", read: "14:30" },
                { tag: "three digits", typed: "930", read: "09:30" },
                { tag: "three digits behind the mask", typed: "93:0", read: "09:30" },
                { tag: "three digits, a valid hour first", typed: "143", read: "01:43" },
                { tag: "the hour", typed: "14", read: "14:00" },
                { tag: "the hour behind the mask", typed: "14:", read: "14:00" },
                { tag: "the hour, h", typed: "14h", read: "14:00" },
                { tag: "one digit", typed: "9", read: "09:00" },
                { tag: "one digit behind the mask", typed: "9:", read: "09:00" },
                { tag: "one hour digit and minutes", typed: "9:30", read: "09:30" },
                { tag: "the mask's blanks", typed: "14:__", read: "14:00" },
                { tag: "the mask's spaces", typed: "9 :30", read: "09:30" },
                { tag: "midnight", typed: "0", read: "00:00" },
                { tag: "the last minute", typed: "23:59", read: "23:59" },
                { tag: "no hour 24", typed: "24", read: "" },
                { tag: "no 24:00", typed: "24:00", read: "" },
                { tag: "no hour 25", typed: "25:00", read: "" },
                { tag: "no minute 60", typed: "12:60", read: "" },
                { tag: "no minute 60, digits", typed: "1260", read: "" },
                { tag: "no minute 60, three digits", typed: "160", read: "" },
                { tag: "one minute digit could be two things", typed: "9:5", read: "" },
                { tag: "empty", typed: "", read: "" },
                { tag: "the mask alone", typed: ":", read: "" },
                { tag: "minutes alone", typed: ":30", read: "" },
                { tag: "words", typed: "noon", read: "" },
                { tag: "too long", typed: "12345", read: "" }
            ]
        }

        function test_what_is_typed_reads(data) {
            const field = createTemporaryObject(fieldComponent, root)
            compare(field.readClock(data.typed), data.read)
        }

        // Each form of typing, by keys alone, Tab into the field and Tab out of it.
        function test_typed_from_tab_to_tab_data() {
            return [
                { tag: "1430", keys: "1430", read: "14:30", finished: 1 },
                { tag: "14:30", keys: "14:30", read: "14:30", finished: 1 },
                { tag: "14h30", keys: "14h30", read: "14:30", finished: 1 },
                { tag: "930", keys: "930", read: "09:30", finished: 1 },
                { tag: "14", keys: "14", read: "14:00", finished: 1 },
                { tag: "9", keys: "9", read: "09:00", finished: 1 },
                { tag: "9:30", keys: "9:30", read: "09:30", finished: 1 },
                { tag: "25 stays for its form", keys: "2500", read: "25:00", finished: 1 },
                { tag: "half typed nonsense stays", keys: "99", read: "99:", finished: 0 }
            ]
        }

        function test_typed_from_tab_to_tab(data) {
            const before = createTemporaryObject(nextComponent, root, { y: 60 })
            const field = createTemporaryObject(panelComponent, root, { text: "08:15" })
            const after = createTemporaryObject(nextComponent, root)
            before.KeyNavigation.tab = field
            field.KeyNavigation.tab = after
            let finished = 0
            field.editingFinished.connect(() => finished++)
            before.forceActiveFocus()
            keyClick(Qt.Key_Tab)
            verify(field.activeFocus, "Tab comes to the field")
            compare(field.selectedText.length > 0, true, "the whole text selected, the first digit starts over")
            type(data.keys)
            keyClick(Qt.Key_Tab)
            verify(after.activeFocus, "Tab goes on, past the button")
            compare(field.text, data.read)
            compare(finished, data.finished, "its form hears it, half typed too")
        }

        // Typed into an empty field: the mask's cursor at its end would swallow every digit.
        function test_typed_into_an_empty_field() {
            const field = createTemporaryObject(panelComponent, root)
            field.forceActiveFocus(Qt.TabFocusReason)
            type("1430")
            compare(field.text, "14:30")
        }

        // A click into the field selects the time once it is done, digits start over; a
        // cursor at the mask's end would swallow them. A second click places the cursor.
        function test_a_click_into_the_field_is_ready_for_digits() {
            const field = createTemporaryObject(panelComponent, root, { text: "08:15" })
            mouseClick(field, field.width - find(field, "clockButton").width - 2, field.height / 2)
            verify(field.activeFocus)
            verify(!field.panelOpen, "the field's text is for typing, the button for the panel")
            wait(0)
            compare(field.selectedText.length, 5)
            type("1430")
            compare(field.text, "14:30")
        }

        // A digit with the mask full starts over: after a click, a step, a time typed.
        function test_typing_again_starts_over() {
            const field = openField("08:15")
            clickCell(field, "clockHour14")
            type("1630")
            compare(field.text, "16:30", "after a click")
            keyClick(Qt.Key_Up)
            type("930")
            keyClick(Qt.Key_Return)
            compare(field.text, "09:30", "after a step")
            type("0800")
            compare(field.text, "08:00", "after a time typed")
        }

        // Enter reads what was typed first: `accepted` says it whole, as the forms save on it.
        function test_enter_reads_what_was_typed() {
            const field = createTemporaryObject(panelComponent, root, { text: "08:15" })
            let accepted = 0
            field.accepted.connect(() => accepted++)
            field.forceActiveFocus(Qt.TabFocusReason)
            type("14")
            keyClick(Qt.Key_Return)
            compare(field.text, "14:00")
            compare(accepted, 1)
        }

        // Alt+Down opens the panel, the focus stays in the field, typing goes on beside
        // it and the cells follow; Tab goes past the grids and closes it, the value kept.
        function test_alt_down_and_tab_never_enters_the_grids() {
            const field = createTemporaryObject(panelComponent, root, { text: "08:15" })
            const after = createTemporaryObject(nextComponent, root)
            field.KeyNavigation.tab = after
            field.forceActiveFocus(Qt.TabFocusReason)
            keyClick(Qt.Key_Down, Qt.AltModifier)
            verify(field.panelOpen, "Alt+Down opens it")
            verify(field.activeFocus, "the focus stays in the field")
            compare(field.text, "08:15", "and it steps nothing")
            verify(cell(field, "clockHour8").chosen)
            verify(cell(field, "clockMinute15").chosen)
            type("1430")
            compare(field.text, "14:30")
            verify(cell(field, "clockHour14").chosen)
            verify(!cell(field, "clockHour8").chosen)
            keyClick(Qt.Key_Tab)
            verify(after.activeFocus, "Tab goes past the grids")
            verify(!field.panelOpen, "and leaves the panel closed")
            compare(field.text, "14:30")
        }

        function test_no_cell_takes_the_focus() {
            const field = openField("08:15")
            clickCell(field, "clockHour9")
            clickCell(field, "clockMinute30")
            clickCell(field, "clockPlus")
            verify(field.activeFocus, "the focus stays in the field")
            compare(find(field, "clockButton").activeFocusOnTab, false, "the button is no Tab stop")
            compare(cell(field, "clockHour9").activeFocusOnTab, false, "nor a cell")
        }

        // Each click applies at once and leaves the panel open: an hour on the minutes
        // there are, a minute on the hour, -1 and +1. No OK anywhere.
        function test_each_click_applies_at_once() {
            const field = openField("08:15")
            let edited = 0
            field.textEdited.connect(() => edited++)
            clickCell(field, "clockHour14")
            compare(field.text, "14:15", "the minutes kept")
            verify(field.panelOpen, "still open")
            clickCell(field, "clockMinute30")
            compare(field.text, "14:30")
            clickCell(field, "clockPlus")
            compare(field.text, "14:31")
            clickCell(field, "clockMinus")
            clickCell(field, "clockMinus")
            compare(field.text, "14:29")
            verify(field.panelOpen, "still open, nothing to confirm")
            compare(edited, 5, "said as typed")
            verify(!cell(field, "clockMinute25").chosen, "14:29 is on no cell")
            compare(find(field.panel.contentItem, "clockNow").text, "14:29", "said beside the steppers")
        }

        // An hour with no minutes yet gives :00; the minutes wait for an hour, none is made up.
        function test_an_hour_with_no_minutes_gives_00() {
            const field = openField("")
            verify(!cell(field, "clockMinute30").enabled, "no hour to put minutes on")
            verify(!cell(field, "clockPlus").enabled)
            verify(!cell(field, "clockMinus").enabled)
            mouseClick(cell(field, "clockMinute30"))
            compare(field.clock, "", "a click on it changes nothing")
            clickCell(field, "clockHour9")
            compare(field.text, "09:00")
            clickCell(field, "clockMinute30")
            compare(field.text, "09:30")
        }

        // Half typed, the panel reads it: the same as Tab would.
        function test_the_panel_reads_what_is_half_typed() {
            const field = createTemporaryObject(panelComponent, root, { text: "08:15" })
            field.forceActiveFocus(Qt.TabFocusReason)
            type("14")
            mouseClick(find(field, "clockButton"))
            compare(field.text, "14:00")
            clickCell(field, "clockHour9")
            compare(field.text, "09:00")
        }

        function test_a_click_outside_keeps_the_value() {
            const field = openField("08:15")
            clickCell(field, "clockHour14")
            mouseClick(root, root.width - 10, root.height - 10)
            verify(!field.panelOpen, "closed")
            compare(field.text, "14:15", "kept")
        }

        function test_the_button_closes_what_it_opened() {
            const field = openField("08:15")
            clickCell(field, "clockHour14")
            mouseClick(find(field, "clockButton"))
            verify(!field.panelOpen, "closed, not reopened")
            compare(field.text, "14:15", "kept")
        }

        function test_esc_gives_back_the_earlier_one() {
            const field = openField("08:15")
            clickCell(field, "clockHour14")
            clickCell(field, "clockMinute30")
            let edited = 0
            field.textEdited.connect(() => edited++)
            keyClick(Qt.Key_Escape)
            verify(!field.panelOpen, "closed")
            compare(field.text, "08:15", "as it was")
            compare(edited, 1, "said as typed: a form following the text follows back")
            // An empty field, empty again.
            const empty = openField("")
            clickCell(empty, "clockHour9")
            compare(empty.text, "09:00")
            keyClick(Qt.Key_Escape)
            compare(empty.clock, "", "empty again")
            type("1430")
            compare(empty.text, "14:30", "and typing still works")
        }

        // What does not read is given back as it was, half typed included.
        function test_esc_gives_back_what_does_not_read() {
            const field = createTemporaryObject(panelComponent, root, { text: "08:15" })
            field.forceActiveFocus(Qt.TabFocusReason)
            type("9:5")
            compare(field.clock, "")
            mouseClick(find(field, "clockButton"))
            compare(field.text, "9:5", "left as typed")
            clickCell(field, "clockHour14")
            compare(field.text, "14:00", "the minutes did not read: :00")
            keyClick(Qt.Key_Escape)
            compare(field.text, "9:5", "as it was")
        }

        // A form that saves on `editingFinished` hears the value kept once, as the panel
        // closes; a click does not save half a choice, and Esc leaves nothing to say.
        function test_editing_finished_once_when_it_closes() {
            const field = openField("08:15")
            let finished = 0
            field.editingFinished.connect(() => finished++)
            clickCell(field, "clockHour14")
            clickCell(field, "clockMinute30")
            compare(finished, 0, "no save under the pointer")
            mouseClick(root, root.width - 10, root.height - 10)
            compare(finished, 1, "the value kept, said")

            mouseClick(find(field, "clockButton"))
            clickCell(field, "clockHour9")
            keyClick(Qt.Key_Escape)
            compare(field.text, "14:30")
            compare(finished, 1, "Esc says nothing")

            mouseClick(find(field, "clockButton"))
            mouseClick(find(field, "clockButton"))
            compare(finished, 1, "nothing changed, nothing said")
        }

        // Saved as it closes, a settings row's list is rebuilt under the panel: nothing breaks.
        function test_a_row_rebuilt_as_it_closes() {
            const rows = createTemporaryObject(rebuiltComponent, root)
            const field = rows.row
            mouseClick(find(field, "clockButton"))
            clickCell(field, "clockHour14")
            mouseClick(root, root.width - 10, root.height - 10)
            compare(rows.ranges[0], "14:00")
            wait(0)
            compare(rows.row.text, "14:00", "the new row says it")
            mouseClick(find(rows.row, "clockButton"))
            verify(rows.row.panelOpen, "and opens its own panel")
            keyClick(Qt.Key_Escape)
        }

        // The end's limits hold for -1 and +1 too: an end field reaches 23:59, never around.
        function test_an_end_stays_in_its_day_by_pointer() {
            const field = openField("23:58")
            field.end = true
            clickCell(field, "clockPlus")
            clickCell(field, "clockPlus")
            compare(field.text, "23:59", "stops there")
            field.text = "00:01"
            clickCell(field, "clockMinus")
            clickCell(field, "clockMinus")
            compare(field.text, "00:00", "stops there")
            field.end = false
            clickCell(field, "clockMinus")
            compare(field.text, "23:59", "any other field goes around midnight")
        }

        // Beside the field where the window has room, the next row in reach; else under it.
        function test_beside_or_under_the_field() {
            const field = openField("08:15")
            compare(field.panel.beside, true)
            compare(field.panel.x, field.width + 4)
            field.closePanel()
            field.x = root.width - field.width - 20
            mouseClick(find(field, "clockButton"))
            compare(field.panel.beside, false, "no room on the right")
            verify(field.panel.y >= field.height, "under the field")
            const left = field.panel.parent.mapToItem(null, field.panel.x, field.panel.y).x
            verify(left + field.panel.width <= root.width, "inside the window")
            field.closePanel()
        }

        // The cells: 24 hours in six columns, twelve minutes in six columns, 00 to 55.
        function test_the_grids() {
            const field = openField("08:15")
            const hours = find(field.panel.contentItem, "clockHours")
            const minutes = find(field.panel.contentItem, "clockMinutes")
            compare(hours.columns, 6)
            compare(minutes.columns, 6)
            compare(cell(field, "clockHour0").text, "00")
            compare(cell(field, "clockHour23").text, "23")
            compare(cell(field, "clockMinute0").text, ":00")
            compare(cell(field, "clockMinute55").text, ":55")
            verify(!cell(field, "clockHour24"))
            compare(cell(field, "clockHour6").y, cell(field, "clockHour0").y + cell(field, "clockHour0").height + hours.spacing, "06 starts the second row")
            compare(cell(field, "clockMinute30").y, cell(field, "clockMinute0").y + cell(field, "clockMinute0").height + minutes.spacing, ":30 starts the second row")
        }

        // A 12-hour locale keeps the 24 cells, labelled 12 AM to 11 PM; the field stays on 14:30.
        function test_a_twelve_hour_locale() {
            const field = createTemporaryObject(panelComponent, root, { text: "14:30", timeLocale: Qt.locale("en_US") })
            verify(field.twelveHours)
            mouseClick(find(field, "clockButton"))
            compare(cell(field, "clockHour0").text, "12 " + field.timeLocale.amText)
            compare(cell(field, "clockHour1").text, "1 " + field.timeLocale.amText)
            compare(cell(field, "clockHour12").text, "12 " + field.timeLocale.pmText)
            compare(cell(field, "clockHour23").text, "11 " + field.timeLocale.pmText)
            verify(cell(field, "clockHour14").chosen)
            clickCell(field, "clockHour15")
            compare(field.text, "15:30", "the form reads 24 hours")
            compare(field.value, "3:30 " + field.timeLocale.pmText)
            verify(!createTemporaryObject(panelComponent, root, { timeLocale: Qt.locale("fr_FR") }).twelveHours)
        }

        // A spin button with a readable value; radio buttons in two named groups.
        function test_what_a_screen_reader_finds() {
            const field = openField("14:30")
            compare(field.Accessible.role, Accessible.SpinBox)
            compare(field.value, "14 hours 30")
            field.text = "01:00"
            compare(field.value, "1 hour", "a whole hour, said alone")
            field.text = "25:00"
            compare(field.value, "", "no time, no value")
            field.text = "14:30"
            const hour = cell(field, "clockHour14")
            compare(hour.Accessible.role, Accessible.RadioButton)
            compare(hour.Accessible.name, "14 hours")
            compare(hour.Accessible.checked, true)
            compare(cell(field, "clockHour15").Accessible.checked, false)
            const minute = cell(field, "clockMinute30")
            compare(minute.Accessible.role, Accessible.RadioButton)
            compare(minute.Accessible.name, "30 minutes")
            compare(minute.Accessible.checked, true)
            compare(cell(field, "clockPlus").Accessible.role, Accessible.Button)
            compare(cell(field, "clockPlus").Accessible.name, "clock-later")
            compare(find(field.panel.contentItem, "clockHours").Accessible.name, "clock-hours")
            compare(find(field.panel.contentItem, "clockMinutes").Accessible.name, "clock-minutes")
            compare(find(field, "clockButton").Accessible.name, "clock-pick")
        }

        // A screen reader's actions do what the pointer and the keys do.
        function test_a_screen_reader_s_actions() {
            const field = openField("14:30")
            cell(field, "clockHour15").Accessible.toggleAction()
            compare(field.text, "15:30", "a radio button toggled is a click")
            cell(field, "clockPlus").Accessible.pressAction()
            compare(field.text, "15:31", "a button pressed too")
            field.Accessible.increaseAction()
            compare(field.text, "15:35", "the field increased: Up")
            field.Accessible.decreaseAction()
            compare(field.text, "15:30", "decreased: Down")
            keyClick(Qt.Key_Escape)
            const empty = openField("")
            cell(empty, "clockMinute30").Accessible.toggleAction()
            compare(empty.clock, "", "a cell disabled stays so for a screen reader")
            keyClick(Qt.Key_Escape)
        }

        // The field hidden (a form closed, a whole day ticked), the panel goes with it.
        function test_hidden_it_closes() {
            const field = openField("08:15")
            field.visible = false
            verify(!field.panelOpen)
        }

        // Without the window's colours, a field is typed only, as a form may still ask.
        function test_without_a_theme_it_is_typed_only() {
            const field = createTemporaryObject(fieldComponent, root, { text: "09:00" })
            verify(!find(field, "clockButton").visible)
            field.forceActiveFocus()
            keyClick(Qt.Key_Down, Qt.AltModifier)
            verify(!field.panelOpen)
            compare(field.text, "09:00")
        }

        // "23:59" is never cut, whatever width a form gave it.
        function test_as_wide_as_a_time_and_its_button() {
            const field = createTemporaryObject(panelComponent, root, { text: "23:59" })
            verify(field.Layout.minimumWidth >= field.contentWidth + field.leftPadding + field.rightPadding, "room for the text: " + field.Layout.minimumWidth)
            verify(field.rightPadding >= find(field, "clockButton").width, "the button beside the text, not over it")
        }

        // Nor once its font changes after it is made: a dialog's fields take the
        // window's font only when the dialog is placed in the window, after their
        // width was measured (10 November 2026: "10:10" read "0:10" in the Time
        // dialog and Health's change of a meal, on a phone).
        function test_as_wide_as_a_time_in_the_font_it_comes_to_have() {
            const field = createTemporaryObject(panelComponent, root, { text: "10:10" })
            const before = field.implicitWidth
            field.font.pixelSize = 26
            waitForRendering(field)
            verify(field.implicitWidth > before, "wider in a larger font: " + before + " then " + field.implicitWidth)
            verify(field.width >= field.contentWidth + field.leftPadding + field.rightPadding, "room for the text: " + field.width + " for " + field.contentWidth)
        }
    }
}
