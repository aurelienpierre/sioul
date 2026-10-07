// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One setting: its name, the control that changes it, and a sentence on what
// it does. Used in a page's settings panel and inside a Porch lane's "?".

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

ColumnLayout {
    id: field

    required property var setting
    required property var sioul
    required property var theme
    readonly property var weekdays: ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"]

    signal save(string key, var value)

    // A folder chosen in Sioul's own browser (FolderBrowser.qml), on Android.
    function browse() {
        if (browser.item === null)
            browser.setSource("FolderBrowser.qml", { sioul: field.sioul, theme: field.theme })
        browser.item.begin(folderField.text)
    }

    function same(a, b) {
        return JSON.stringify(a) === JSON.stringify(b)
    }

    spacing: 3

    // A grid's name is its channel's, said by the buttons above it (Accounts).
    Label {
        visible: field.setting.kind !== "note" && field.setting.kind !== "link" && field.setting.kind !== "matrix"
        Layout.fillWidth: true
        text: field.setting.label
        font.weight: Font.DemiBold
        wrapMode: Text.Wrap
        color: field.theme.text
    }

    // Nothing to change: what a lane holds, then how mail lands there.
    Label {
        visible: field.setting.kind === "note"
        Layout.fillWidth: true
        text: field.setting.label
        wrapMode: Text.Wrap
        lineHeight: 1.25
        color: field.theme.text
    }

    // Nothing to change here: a sentence, and a button to where it is
    // changed (the Health page's meals and sleep, from the hours).
    ColumnLayout {
        visible: field.setting.kind === "link"
        Layout.fillWidth: true
        spacing: 2

        Label {
            Layout.fillWidth: true
            text: field.setting.kind === "link" ? field.setting.help : ""
            wrapMode: Text.Wrap
            lineHeight: 1.25
            color: field.theme.text
        }
        Button {
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            flat: true
            text: field.setting.label
            icon.name: "go-next"
            icon.color: field.theme.text
            // The window shows it, as a reminder's "Open" does (main.qml, openThing).
            onClicked: field.sioul.reminderOpened(String(field.setting.value), "", "")
        }
    }

    // Boxes in a grid: the states down, the times and the pause across (who
    // may reach you when, on one channel). Each row ticked or unticked whole
    // in one click (Always, Never); the blocked, a row never ticked. On a
    // screen too narrow for a row, its name stands above its boxes; sideways
    // when even the boxes are too wide.
    Loader {
        active: field.setting.kind === "matrix"
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            Flickable {
                id: grid

                readonly property var rows: field.setting.rows || []
                readonly property var columns: field.setting.choices || []
                readonly property var ticked: field.setting.value || []
                // Each column as wide as its heading, a box at least; the
                // rows' names as wide as the widest: the grid fits a phone when it can.
                readonly property var widths: {
                    const out = []
                    for (let i = 0; i < heads.count; i++) {
                        const head = heads.itemAt(i)
                        out.push(head ? Math.max(40, head.implicitWidth + 8) : 40)
                    }
                    return out
                }
                readonly property real nameWidth: {
                    let width = 40
                    for (let i = 0; i < names.count; i++) {
                        const name = names.itemAt(i)
                        if (name)
                            width = Math.max(width, name.implicitWidth + 12)
                    }
                    return width
                }
                readonly property real boxesWidth: grid.widths.reduce((sum, w) => sum + w, 0)
                readonly property real buttonsWidth: alwaysWidth.implicitWidth + neverWidth.implicitWidth + 8
                // A row's name above its boxes when name, boxes and buttons do not fit side by side.
                readonly property bool narrow: grid.width > 0 && grid.nameWidth + grid.boxesWidth + grid.buttonsWidth > grid.width

                // A whole row at once: every column, or none.
                function whole(row, on) {
                    field.save(field.setting.key + "." + row, on ? grid.columns.map(c => c.value) : [])
                }

                implicitHeight: table.implicitHeight + (wide.visible ? wide.height : 0)
                contentWidth: table.implicitWidth
                contentHeight: table.implicitHeight
                flickableDirection: Flickable.HorizontalFlick
                boundsBehavior: Flickable.StopAtBounds
                clip: true
                ScrollBar.horizontal: ScrollBar {
                    id: wide

                    visible: grid.contentWidth > grid.width
                    policy: ScrollBar.AsNeeded
                }

                // Measured once each, for the widths above.
                Repeater {
                    id: names

                    model: grid.rows

                    delegate: Label {
                        required property var modelData

                        visible: false
                        text: modelData.label
                    }
                }
                Button {
                    id: alwaysWidth

                    visible: false
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    flat: true
                    font.pixelSize: 13
                    text: field.sioul.text("reach-row-always")
                }
                Button {
                    id: neverWidth

                    visible: false
                    implicitWidth: implicitContentWidth + leftPadding + rightPadding
                    flat: true
                    font.pixelSize: 13
                    text: field.sioul.text("reach-row-never")
                }

                ColumnLayout {
                    id: table

                    spacing: 0

                    RowLayout {
                        spacing: 0

                        Item {
                            visible: !grid.narrow
                            Layout.preferredWidth: grid.nameWidth
                            Layout.preferredHeight: 1
                        }
                        Repeater {
                            id: heads

                            model: grid.columns

                            delegate: Label {
                                required property var modelData
                                required property int index

                                Layout.preferredWidth: grid.widths[index] || 40
                                text: modelData.label
                                horizontalAlignment: Text.AlignHCenter
                                font.pixelSize: 13
                                color: field.theme.muted
                            }
                        }
                    }
                    Repeater {
                        model: grid.rows

                        delegate: ColumnLayout {
                            id: list

                            required property var modelData
                            // The blocked: never, a row nothing ticks.
                            readonly property bool fixed: list.modelData.value === "blocked"

                            spacing: 0

                            // On a narrow screen: the name on its own line, Always and Never at its end.
                            RowLayout {
                                visible: grid.narrow
                                Layout.preferredWidth: grid.boxesWidth
                                Layout.topMargin: 6
                                spacing: 0

                                Label {
                                    Layout.fillWidth: true
                                    text: list.modelData.label
                                    elide: Text.ElideRight
                                    color: list.fixed ? field.theme.muted : field.theme.text
                                }
                                Repeater {
                                    model: list.fixed ? [] : [true, false]

                                    delegate: Button {
                                        required property bool modelData

                                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                                        implicitHeight: implicitContentHeight + topPadding + bottomPadding
                                        flat: true
                                        font.pixelSize: 13
                                        text: field.sioul.text(modelData ? "reach-row-always" : "reach-row-never")
                                        Accessible.name: list.modelData.label + ", " + text
                                        onClicked: grid.whole(list.modelData.value, modelData)
                                    }
                                }
                            }
                            RowLayout {
                                spacing: 0

                                Label {
                                    visible: !grid.narrow
                                    Layout.preferredWidth: grid.nameWidth
                                    text: list.modelData.label
                                    color: list.fixed ? field.theme.muted : field.theme.text
                                }
                                Repeater {
                                    model: grid.columns

                                    delegate: Item {
                                        id: cell

                                        required property var modelData
                                        required property int index
                                        readonly property string cellKey: list.modelData.value + ":" + cell.modelData.value

                                        Layout.preferredWidth: grid.widths[cell.index] || 40
                                        Layout.preferredHeight: box.implicitHeight

                                        CheckBox {
                                            id: box

                                            anchors.horizontalCenter: parent.horizontalCenter
                                            enabled: !list.fixed
                                            checked: !list.fixed && grid.ticked.indexOf(cell.cellKey) >= 0
                                            Accessible.name: list.modelData.label + ", " + cell.modelData.label
                                            ToolTip.visible: hovered
                                            ToolTip.text: list.fixed ? field.sioul.text("reach-row-blocked") : list.modelData.label + " · " + cell.modelData.label
                                            ToolTip.delay: 600
                                            onToggled: {
                                                // The row's times, this one as ticked now.
                                                const times = grid.columns.map(c => c.value).filter(v => v === cell.modelData.value ? box.checked : grid.ticked.indexOf(list.modelData.value + ":" + v) >= 0)
                                                field.save(field.setting.key + "." + list.modelData.value, times)
                                                box.checked = Qt.binding(() => !list.fixed && grid.ticked.indexOf(cell.cellKey) >= 0)
                                            }
                                        }
                                    }
                                }
                                // On a wide screen: Always and Never after the boxes.
                                Repeater {
                                    model: grid.narrow || list.fixed ? [] : [true, false]

                                    delegate: Button {
                                        required property bool modelData

                                        implicitWidth: implicitContentWidth + leftPadding + rightPadding
                                        implicitHeight: implicitContentHeight + topPadding + bottomPadding
                                        flat: true
                                        font.pixelSize: 13
                                        text: field.sioul.text(modelData ? "reach-row-always" : "reach-row-never")
                                        Accessible.name: list.modelData.label + ", " + text
                                        onClicked: grid.whole(list.modelData.value, modelData)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sioul's own spam filter: its training on a computer, where its table
    // comes from on a phone (SpamFilter.qml); its sentence said first there.
    Loader {
        active: field.setting.kind === "spam"
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            SpamFilter {
                sioul: field.sioul
                theme: field.theme
                about: field.setting.help
            }
        }
    }

    // What each kind of notification does at each time: a mark in each cell,
    // its choices on a press (NotifyGrid.qml), saved a row at a time.
    Loader {
        active: field.setting.kind === "notify"
        visible: active
        Layout.fillWidth: true

        sourceComponent: Component {
            NotifyGrid {
                setting: field.setting
                sioul: field.sioul
                theme: field.theme
                onSave: (key, value) => field.save(key, value)
            }
        }
    }

    // On or off. Saved, the rows are read again; refused, the switch says the
    // setting as it stayed (a click leaves its own tick otherwise).
    Switch {
        id: onOff

        visible: field.setting.kind === "bool"
        checked: field.setting.value === true
        Accessible.name: field.setting.label
        onToggled: {
            field.save(field.setting.key, checked)
            onOff.checked = Qt.binding(() => field.setting.value === true)
        }
    }

    // A number.
    RowLayout {
        visible: field.setting.kind === "int"
        spacing: 6

        SpinBox {
            from: field.setting.min
            to: field.setting.max
            stepSize: field.setting.step
            editable: true
            value: field.setting.kind === "int" ? field.setting.value : 0
            Accessible.name: field.setting.label
            onValueModified: field.save(field.setting.key, value)
        }
        Label {
            text: field.setting.unit
            color: field.theme.muted
        }
    }

    // A number with decimals.
    RowLayout {
        visible: field.setting.kind === "float"
        Layout.fillWidth: true
        spacing: 8

        Slider {
            id: slider

            Layout.fillWidth: true
            from: field.setting.min
            to: field.setting.max
            stepSize: field.setting.step
            snapMode: Slider.SnapAlways
            value: field.setting.kind === "float" ? field.setting.value : 0
            Accessible.name: field.setting.label
            onPressedChanged: {
                if (!pressed)
                    field.save(field.setting.key, Math.round(value * 100) / 100)
            }
            Keys.onReleased: field.save(field.setting.key, Math.round(value * 100) / 100)
        }
        // A share as a percentage ("95%", "95 %"), else the number and its unit.
        Label {
            text: field.setting.unit === "%" ? field.sioul.textWith("spam-percent", "n", String(Math.round(slider.value * 100))) : slider.value.toLocaleString(Qt.locale(field.sioul.text("qt-locale")), "f", field.setting.step < 1 ? 1 : 0) + " " + field.setting.unit
            color: field.theme.text
        }
    }

    // What a source is for: work, your admin, leisure; any of them together,
    // on more lines when the screen is narrow.
    Flow {
        id: areaRow

        readonly property var on: field.setting.kind === "areas" ? String(field.setting.value).split("+") : []

        visible: field.setting.kind === "areas"
        Layout.fillWidth: true
        spacing: 12

        Repeater {
            model: field.setting.kind === "areas" ? ["work", "admin", "leisure"] : []

            delegate: CheckBox {
                id: area

                required property string modelData

                text: field.sioul.text("area-" + modelData)
                checked: areaRow.on.indexOf(modelData) >= 0
                onToggled: {
                    const was = areaRow.on
                    const now = ["work", "admin", "leisure"].filter(a => a === modelData ? checked : was.indexOf(a) >= 0)
                    field.save(field.setting.key, now.join("+"))
                    area.checked = Qt.binding(() => areaRow.on.indexOf(area.modelData) >= 0)
                }
            }
        }
    }

    // Some of a list, each ticked or not: the projects the Porch shows.
    Flow {
        id: picks

        readonly property var on: field.setting.kind === "picks" ? field.setting.value : []

        visible: field.setting.kind === "picks"
        Layout.fillWidth: true
        spacing: 12

        Repeater {
            model: field.setting.kind === "picks" ? field.setting.choices : []

            delegate: CheckBox {
                id: pick

                required property var modelData

                text: pick.modelData.label
                checked: picks.on.indexOf(pick.modelData.value) >= 0
                onToggled: {
                    field.save(field.setting.key, field.setting.choices.map(c => c.value).filter(v => v === pick.modelData.value ? pick.checked : picks.on.indexOf(v) >= 0))
                    pick.checked = Qt.binding(() => picks.on.indexOf(pick.modelData.value) >= 0)
                }
            }
        }
    }

    // One of a few.
    ComboBox {
        visible: field.setting.kind === "choice"
        Layout.fillWidth: true
        model: field.setting.kind === "choice" ? field.setting.choices.map(c => c.label) : []
        currentIndex: field.setting.kind === "choice" ? Math.max(0, field.setting.choices.findIndex(c => field.same(c.value, field.setting.value))) : -1
        Accessible.name: field.setting.label
        onActivated: index => field.save(field.setting.key, field.setting.choices[index].value)
    }

    // Text, a line or several.
    TextField {
        visible: field.setting.kind === "text"
        Layout.fillWidth: true
        text: field.setting.kind === "text" ? field.setting.value : ""
        Accessible.name: field.setting.label
        onEditingFinished: {
            if (text !== field.setting.value)
                field.save(field.setting.key, text)
        }
    }
    ScrollView {
        visible: field.setting.kind === "long"
        Layout.fillWidth: true
        Layout.preferredHeight: 90

        TextArea {
            id: longText

            text: field.setting.kind === "long" ? field.setting.value : ""
            wrapMode: TextArea.Wrap
            Accessible.name: field.setting.label

            // A field shows where it is: a border, darker when it has the focus.
            background: Rectangle {
                color: field.theme.surface
                radius: field.theme.radius
                border.color: longText.activeFocus ? field.theme.focus : field.theme.line
            }
            onEditingFinished: {
                if (text !== field.setting.value)
                    field.save(field.setting.key, text)
            }
        }
    }

    // A folder.
    RowLayout {
        visible: field.setting.kind === "folder"
        Layout.fillWidth: true
        spacing: 6

        TextField {
            id: folderField

            Layout.fillWidth: true
            text: field.setting.kind === "folder" ? field.setting.value : ""
            Accessible.name: field.setting.label
            onEditingFinished: {
                if (text !== field.setting.value)
                    field.save(field.setting.key, text)
            }
        }
        Button {
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            text: field.sioul.text("ui-choose")
            onClicked: Qt.platform.os === "android" ? field.browse() : folderDialog.open()

            // On Android, Sioul's own browser (FolderBrowser.qml): the system's picker refuses the phone's storage.
            Loader {
                id: browser
            }
            Connections {
                target: browser.item

                function onChosen(path) {
                    field.save(field.setting.key, path)
                }
            }

            FolderDialog {
                id: folderDialog

                // On Windows the address is file:///C:/…: its path is C:/…, not /C:/…;
                // on Android, a folder of the phone's storage (Theme.localPath).
                onAccepted: {
                    const path = field.theme.localPath(selectedFolder)
                    if (path === "")
                        field.sioul.status = field.sioul.text("folder-not-on-device")
                    else
                        field.save(field.setting.key, path)
                }
            }
        }
    }

    // A font family: the desktop's first.
    ComboBox {
        readonly property var families: [""].concat(Qt.fontFamilies())

        visible: field.setting.kind === "font"
        Layout.fillWidth: true
        model: field.setting.kind === "font" ? families.map(f => f === "" ? field.sioul.text("set-font-desktop") : f) : []
        currentIndex: field.setting.kind === "font" ? Math.max(0, families.indexOf(field.setting.value)) : -1
        Accessible.name: field.setting.label
        onActivated: index => field.save(field.setting.key, families[index])
    }

    // Minutes per weekday.
    GridLayout {
        visible: field.setting.kind === "week"
        Layout.fillWidth: true
        columns: 2
        columnSpacing: 10
        rowSpacing: 2

        Repeater {
            model: field.setting.kind === "week" ? 7 : 0

            delegate: RowLayout {
                id: day

                required property int index

                spacing: 6

                Label {
                    Layout.preferredWidth: 46
                    text: field.sioul.text("weekday-" + (day.index + 1))
                    elide: Text.ElideRight
                    color: field.theme.muted
                }
                SpinBox {
                    from: 0
                    to: 600
                    stepSize: 15
                    editable: true
                    value: field.setting.value[day.index]
                    Accessible.name: field.sioul.text("weekday-" + (day.index + 1))
                    onValueModified: {
                        const week = field.setting.value.slice()
                        week[day.index] = value
                        field.save(field.setting.key, week)
                    }
                }
            }
        }
    }

    // A week of hours: each day on or off, with one range of hours or
    // several (09:00–12:00 and 14:00–17:00, a lunch between), each changed or
    // taken away on its own; the other days and ranges are always kept.
    GridLayout {
        id: week

        // The day's ranges as saved, in order, each with its place in the list.
        function rangesOf(day) {
            if (field.setting.kind !== "windows")
                return []
            return field.setting.value.map((w, at) => ({ at: at, day: w.day.toLowerCase(), start: w.start, end: w.end || "" })).filter(w => w.day === day).sort((a, b) => a.start.localeCompare(b.start))
        }

        function sorted(all) {
            return all.sort((a, b) => field.weekdays.indexOf(a.day.toLowerCase()) - field.weekdays.indexOf(b.day.toLowerCase()) || a.start.localeCompare(b.start))
        }

        // A day switched on (09:00–17:00) or off (every range of it goes).
        function setDay(day, on) {
            const rest = field.setting.value.filter(w => w.day.toLowerCase() !== day)
            field.save(field.setting.key, week.sorted(on ? rest.concat([{ day: day, start: "09:00", end: "17:00", minutes: 0 }]) : rest))
        }

        // One range changed, the day's others kept.
        function setRange(at, start, end) {
            const all = field.setting.value.slice()
            all[at] = { day: all[at].day, start: start, end: end, minutes: 0 }
            field.save(field.setting.key, week.sorted(all))
        }

        // Another range, an hour after the day's last one ends.
        function addRange(day) {
            const ranges = week.rangesOf(day)
            const last = ranges.length > 0 && ranges[ranges.length - 1].end.length === 5 ? ranges[ranges.length - 1].end : "12:00"
            const hour = Math.min(Number(last.slice(0, 2)) + 1, 21)
            const pad = n => (n < 10 ? "0" : "") + n
            field.save(field.setting.key, week.sorted(field.setting.value.concat([{ day: day, start: pad(hour) + last.slice(2), end: pad(hour + 2) + last.slice(2), minutes: 0 }])))
        }

        function removeRange(at) {
            field.save(field.setting.key, field.setting.value.filter((w, i) => i !== at))
        }

        visible: field.setting.kind === "windows"
        Layout.fillWidth: true
        columns: 4
        columnSpacing: 8
        rowSpacing: 2

        Repeater {
            model: field.setting.kind === "windows" ? field.weekdays : []

            delegate: RowLayout {
                id: weekday

                required property string modelData
                required property int index
                readonly property var ranges: week.rangesOf(weekday.modelData)

                Layout.columnSpan: 4
                spacing: 8

                CheckBox {
                    Layout.preferredWidth: 130
                    Layout.alignment: Qt.AlignTop
                    text: field.sioul.text("weekday-" + (weekday.index + 1))
                    checked: weekday.ranges.length > 0
                    onToggled: week.setDay(weekday.modelData, checked)
                }
                Flow {
                    Layout.fillWidth: true
                    spacing: 12

                    Repeater {
                        model: weekday.ranges

                        delegate: Row {
                            id: range

                            required property var modelData

                            spacing: 4

                            TextField {
                                id: from

                                width: 70
                                text: range.modelData.start
                                inputMask: "99:99"
                                Accessible.name: field.sioul.text("weekday-" + (weekday.index + 1))
                                onEditingFinished: if (from.text !== range.modelData.start) week.setRange(range.modelData.at, from.text, to.text)
                            }
                            Label {
                                anchors.verticalCenter: parent.verticalCenter
                                text: "–"
                                color: field.theme.muted
                            }
                            TextField {
                                id: to

                                width: 70
                                text: range.modelData.end
                                inputMask: "99:99"
                                Accessible.name: field.sioul.text("weekday-" + (weekday.index + 1))
                                onEditingFinished: if (to.text !== range.modelData.end) week.setRange(range.modelData.at, from.text, to.text)
                            }
                            ToolButton {
                                visible: weekday.ranges.length > 1
                                text: "×"
                                Accessible.name: field.sioul.text("hours-range-remove")
                                ToolTip.visible: hovered
                                ToolTip.text: field.sioul.text("hours-range-remove")
                                onClicked: week.removeRange(range.modelData.at)
                            }
                        }
                    }
                    ToolButton {
                        visible: weekday.ranges.length > 0
                        text: "+"
                        Accessible.name: field.sioul.text("hours-range-add")
                        ToolTip.visible: hovered
                        ToolTip.text: field.sioul.text("hours-range-add")
                        onClicked: week.addRange(weekday.modelData)
                    }
                }
            }
        }
    }

    // Names, each renamed in place or taken away: kinds of task (new ones
    // added below), categories (taken off every task, after a second word).
    ColumnLayout {
        visible: field.setting.kind === "kinds" || field.setting.kind === "categories" || field.setting.kind === "collections"
        Layout.fillWidth: true
        spacing: 2

        Repeater {
            model: field.setting.kind === "kinds" || field.setting.kind === "categories" || field.setting.kind === "collections" ? field.setting.value : []

            delegate: RowLayout {
                id: named

                required property var modelData
                property bool asking: false

                Layout.fillWidth: true
                spacing: 6

                TextField {
                    Layout.fillWidth: true
                    text: named.modelData.label
                    // Google's calendars change on its own pages only: greyed, with why.
                    enabled: !named.modelData.locked
                    ToolTip.visible: !enabled && hovered
                    ToolTip.text: named.modelData.locked || ""
                    Accessible.name: field.setting.label
                    onEditingFinished: {
                        const name = text.trim()
                        if (name !== "" && name !== named.modelData.label)
                            field.save(field.setting.key, { from: named.modelData.id, to: name })
                    }
                }
                Label {
                    visible: (named.modelData.detail || "") !== ""
                    text: named.modelData.detail || ""
                    font.pixelSize: 12
                    color: field.theme.muted
                }
                ToolButton {
                    visible: !named.asking
                    enabled: !named.modelData.locked
                    text: "×"
                    Accessible.name: field.sioul.text("ui-delete")
                    onClicked: {
                        if (field.setting.kind === "kinds")
                            field.save(field.setting.key, { from: named.modelData.id, to: "" })
                        else
                            named.asking = true
                    }
                }
                Button {
                    visible: named.asking
                    text: field.sioul.text(field.setting.kind === "collections" ? "set-collections-remove" : "set-task-categories-remove")
                    onClicked: field.save(field.setting.key, { from: named.modelData.id, to: "" })
                }
                Button {
                    visible: named.asking
                    flat: true
                    text: field.sioul.text("ui-cancel")
                    onClicked: named.asking = false
                }
            }
        }
        RowLayout {
            visible: field.setting.kind === "kinds"
            Layout.fillWidth: true
            spacing: 6

            TextField {
                id: newName

                Layout.fillWidth: true
                placeholderText: field.sioul.text("set-task-kinds-new")
                onAccepted: addName.clicked()
            }
            Button {
                id: addName

                enabled: newName.text.trim() !== ""
                text: field.sioul.text("ui-add")
                onClicked: {
                    field.save(field.setting.key, { from: "", to: newName.text.trim() })
                    newName.clear()
                }
            }
        }
    }

    // Days off: from a day to another, and a word on them.
    ColumnLayout {
        visible: field.setting.kind === "timeoff"
        Layout.fillWidth: true
        spacing: 4

        Repeater {
            model: field.setting.kind === "timeoff" ? field.setting.value : []

            delegate: RowLayout {
                id: off

                required property var modelData
                required property int index

                spacing: 6

                Label {
                    Layout.fillWidth: true
                    text: off.modelData.from + " → " + off.modelData.until + (off.modelData.label ? "  ·  " + off.modelData.label : "")
                    elide: Text.ElideRight
                    color: field.theme.text
                }
                ToolButton {
                    text: "×"
                    Accessible.name: field.sioul.text("ui-delete")
                    onClicked: field.save(field.setting.key, field.setting.value.filter((w, i) => i !== off.index))
                }
            }
        }
        RowLayout {
            spacing: 6

            TextField {
                id: offFrom

                Layout.preferredWidth: 110
                placeholderText: "2026-12-24"
                inputMask: "9999-99-99;_"
            }
            Label {
                text: "→"
                color: field.theme.muted
            }
            TextField {
                id: offUntil

                Layout.preferredWidth: 110
                placeholderText: "2027-01-02"
                inputMask: "9999-99-99;_"
            }
            TextField {
                id: offLabel

                Layout.fillWidth: true
                placeholderText: field.sioul.text("set-time-off-label")
            }
            Button {
                text: field.sioul.text("set-time-off-add")
                enabled: /^\d{4}-\d\d-\d\d$/.test(offFrom.text) && /^\d{4}-\d\d-\d\d$/.test(offUntil.text)
                onClicked: {
                    field.save(field.setting.key, field.setting.value.concat([{ from: offFrom.text, until: offUntil.text, label: offLabel.text }]))
                    offFrom.text = ""
                    offUntil.text = ""
                    offLabel.text = ""
                }
            }
        }
    }

    // Addresses and @domains, or words: side by side, each with its ×.
    ColumnLayout {
        visible: field.setting.kind === "senders" || field.setting.kind === "words"
        Layout.fillWidth: true
        spacing: 6

        Flow {
            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: field.setting.kind === "senders" || field.setting.kind === "words" ? field.setting.value : []

                delegate: Rectangle {
                    id: entry

                    required property string modelData

                    implicitWidth: Math.min(entryRow.implicitWidth + 10, field.width)
                    implicitHeight: entryRow.implicitHeight
                    radius: field.theme.radius
                    color: field.theme.button

                    RowLayout {
                        id: entryRow

                        anchors.fill: parent
                        anchors.leftMargin: 10
                        spacing: 0

                        // An address or a domain, as a message gave it: plain text.
                        Label {
                            Layout.fillWidth: true
                            Layout.maximumWidth: field.width - 50
                            text: entry.modelData
                            textFormat: Text.PlainText
                            elide: Text.ElideMiddle
                            color: field.theme.text
                        }
                        ToolButton {
                            implicitHeight: 28
                            implicitWidth: 28
                            text: "×"
                            Accessible.name: field.sioul.text("ui-delete") + " " + entry.modelData
                            onClicked: field.save(field.setting.key, field.setting.value.filter(e => e !== entry.modelData))
                        }
                    }
                }
            }
        }
        TextField {
            Layout.fillWidth: true
            // The four lists take numbers too; the senders you let in, addresses only.
            placeholderText: field.sioul.text(field.setting.kind === "words" ? "set-words-add" : field.setting.key === "known" ? "set-senders-add" : "set-senders-add-number")
            onAccepted: {
                if (text.trim() === "")
                    return
                field.save(field.setting.key, field.setting.value.concat([text.trim()]))
                clear()
            }
        }
    }

    // A secret for the keyring: typed, never shown again.
    ColumnLayout {
        id: secretBox

        // Its words: the AI's key, GitHub's token.
        readonly property string words: field.setting.key === "github_token" ? "set-github-token" : "set-ai-key"

        visible: field.setting.kind === "secret"
        Layout.fillWidth: true
        spacing: 4

        Label {
            Layout.fillWidth: true
            text: field.setting.value === true ? field.sioul.text(secretBox.words + "-kept") : field.sioul.text(secretBox.words + "-none")
            wrapMode: Text.Wrap
            color: field.setting.value === true ? field.theme.accent : field.theme.muted
        }
        RowLayout {
            Layout.fillWidth: true

            PasswordField {
                id: secretField

                Layout.fillWidth: true
                sioul: field.sioul
                placeholderText: field.setting.label
                Accessible.name: field.setting.label
                onAccepted: if (keepSecret.enabled) keepSecret.clicked()
            }
            Button {
                id: keepSecret

                text: field.sioul.text("set-ai-key-keep")
                enabled: secretField.text.trim() !== ""
                onClicked: {
                    field.save(field.setting.key, secretField.text)
                    secretField.clear()
                }
            }
        }
        Button {
            visible: field.setting.value === true
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            flat: true
            text: field.sioul.text(secretBox.words + "-forget")
            onClicked: field.save(field.setting.key, "")
        }
        // GitHub's page for a token, its rights filled in: read issues and pull requests.
        Button {
            visible: field.setting.key === "github_token" && field.setting.value !== true
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            flat: true
            text: field.sioul.text("set-github-token-make")
            onClicked: Qt.openUrlExternally("https://github.com/settings/personal-access-tokens/new?name=Sioul&description=Read+issues+and+pull+requests&expires_in=366&issues=read&pull_requests=read&metadata=read")
        }
    }

    // A case's routes: each a few lines, comma-separated words.
    ColumnLayout {
        visible: field.setting.kind === "routes"
        Layout.fillWidth: true
        spacing: 8

        Repeater {
            model: field.setting.kind === "routes" ? field.setting.value : []

            delegate: Rectangle {
                id: route

                required property var modelData
                required property int index

                function changed(part, text) {
                    const all = JSON.parse(JSON.stringify(field.setting.value))
                    all[route.index][part] = text.split(",").map(t => t.trim()).filter(t => t !== "")
                    field.save(field.setting.key, all)
                }

                Layout.fillWidth: true
                implicitHeight: routeColumn.implicitHeight + 16
                radius: field.theme.radius
                color: "transparent"
                border.color: field.theme.line

                ColumnLayout {
                    id: routeColumn

                    anchors.fill: parent
                    anchors.margins: 8
                    spacing: 4

                    RowLayout {
                        Layout.fillWidth: true

                        Label {
                            Layout.fillWidth: true
                            text: field.sioul.text("set-route") + " " + (route.index + 1)
                            color: field.theme.muted
                        }
                        ToolButton {
                            text: "×"
                            Accessible.name: field.sioul.text("ui-delete")
                            onClicked: field.save(field.setting.key, field.setting.value.filter((r, i) => i !== route.index))
                        }
                    }
                    Repeater {
                        model: [["from_domains", "set-route-domains"], ["from_addresses", "set-route-addresses"], ["subject_contains", "set-route-subject"], ["text_contains", "set-route-text"], ["attachment_contains", "set-route-attachments"]]

                        delegate: ColumnLayout {
                            id: part

                            required property var modelData

                            Layout.fillWidth: true
                            spacing: 1

                            Label {
                                text: field.sioul.text(part.modelData[1])
                                font.pixelSize: 12
                                color: field.theme.muted
                            }
                            TextField {
                                Layout.fillWidth: true
                                text: (route.modelData[part.modelData[0]] || []).join(", ")
                                placeholderText: field.sioul.text("set-route-comma")
                                onEditingFinished: {
                                    if (text !== (route.modelData[part.modelData[0]] || []).join(", "))
                                        route.changed(part.modelData[0], text)
                                }
                            }
                        }
                    }
                }
            }
        }
        Button {
            implicitWidth: implicitContentWidth + leftPadding + rightPadding
            flat: true
            text: "+  " + field.sioul.text("set-route-add")
            onClicked: field.save(field.setting.key, field.setting.value.concat([{ from_domains: [], from_addresses: [], subject_contains: [], text_contains: [], attachment_contains: [] }]))
        }
    }

    Label {
        visible: text !== "" && field.setting.kind !== "link" && field.setting.kind !== "spam"
        Layout.fillWidth: true
        text: field.setting.kind === "note" ? field.setting.help.split("\n").filter(line => line !== "").map(line => "•  " + line).join("\n") : field.setting.help
        wrapMode: Text.Wrap
        font.pixelSize: 13
        lineHeight: 1.25
        color: field.theme.muted
    }
}
