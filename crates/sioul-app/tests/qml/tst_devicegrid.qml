// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// What comes forward on each device at each time (DeviceGrid.qml;
// docs/areas.md, "What each device is for"), on a stand-in for Sioul: in a
// wide window the cells line up as one matrix, devices by times, under the
// times' names, each cell saying its areas in words; a cell you changed is
// framed; a cell opens three boxes, and a box says the device, the time and
// the areas as they then stand ("none" for nothing); "As its areas say" only
// on a changed cell, and it asks for the cell back; a device whose Sioul is
// older says one sentence in its cells' place; at a phone's 412 points each
// device says its cells one line per time, and nothing passes the window's
// edge in the longest words of both languages. The cells themselves are
// made and tested in sioul-sync (device_table.rs). Run by tools/qml-test.sh.

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

Item {
    id: root

    width: 1100
    height: 900

    Theme {
        id: testTheme
    }

    QtObject {
        id: mock

        // The longest of each language's words.
        readonly property var words: ({ "area-filter-work": "Travail", "area-filter-admin": "Démarches", "area-filter-leisure": "Loisirs", "area-filter-how": "Comment ce filtre fonctionne", "devices-cell-as-areas": "Selon ce à quoi il sert", "devices-grid-device": "Appareil" })

        function text(key) {
            return mock.words[key] || key
        }
        function textWith(key, name, value) {
            return key + ":" + value
        }
    }

    // Made up: no real device.
    readonly property var times: ["Travail", "Démarches", "Loisirs", "Repas", "Sommeil"]

    function cell(device, time, title, areas, own) {
        const on = areas === "" ? [] : areas.split("+")
        const words = { work: "Travail", admin: "Démarches", leisure: "Loisirs" }
        const label = on.length === 0 ? "Rien" : on.map(a => words[a]).join(" · ")
        return { time: time, title: title, work: on.indexOf("work") >= 0, admin: on.indexOf("admin") >= 0, leisure: on.indexOf("leisure") >= 0, label: label, own: own === true, name: device + ", " + title + " : " + label }
    }

    function cells(device, areas, changed) {
        const ids = ["work", "admin", "leisure", "meals", "sleep"]
        return ids.map((id, i) => root.cell(device, id, root.times[i], areas[i], (changed || []).indexOf(id) >= 0))
    }

    readonly property var rows: [
        { id: "desk", here: true, name: "Bureau de travail", follows: true, cells: root.cells("Bureau de travail", ["work", "", "", "", ""]) },
        { id: "phone", here: false, name: "Le téléphone (Moss)", follows: true, cells: root.cells("Le téléphone (Moss)", ["work+leisure", "admin", "leisure", "", "leisure"], ["work", "meals"]) },
        { id: "tablet", here: false, name: "La tablette (Fern)", follows: false, cells: [], cells_older: "Son Sioul est plus ancien : tout y arrive encore." }
    ]

    Component {
        id: gridComponent

        DeviceGrid {
            sioul: mock
            theme: testTheme
            times: root.times
            help: "Ce qui arrive sur cet appareil à ce moment. Cochez ce qui doit arriver ; sans rien de coché, rien n’arrive alors. « Selon ce à quoi il sert » revient à ce que le moment amène, gardé à ce pour quoi l’appareil est fait."
        }
    }

    Component {
        id: spyComponent

        SignalSpy {}
    }

    TestCase {
        name: "DeviceGrid"
        when: windowShown

        function all(item, test, into) {
            const found = into || []
            if (!item)
                return found
            if (test(item))
                found.push(item)
            for (let i = 0; i < (item.children || []).length; ++i)
                all(item.children[i], test, found)
            return found
        }

        function made(wide, width, rows) {
            return createTemporaryObject(gridComponent, root, { rows: rows || root.rows, wide: wide, width: width, x: 0, y: 20 })
        }

        // A cell of the grid that is drawn, by its device and time.
        function cellOf(grid, device, time) {
            return all(grid, i => i.objectName === "cell-" + device + "-" + time && i.visible && i.width > 0)[0] || null
        }

        // An item by its name, in a cell or in its list over the window.
        function named(cell, name) {
            return all(cell, i => i.objectName === name)[0] || all(cell.Overlay.overlay, i => i.objectName === name && i.visible)[0] || null
        }

        function buttonOf(cell) {
            return all(cell, i => i.objectName === "areaFilter")[0]
        }

        // In a wide window: one matrix, the times' names above, each cell in
        // words, a changed cell framed, an older Sioul said in one sentence.
        function test_the_cells_line_up_as_one_matrix() {
            const grid = made(true, 1000)
            waitForRendering(grid)
            const titles = all(grid, i => i.visible && root.times.indexOf(i.text) >= 0 && i.font && i.font.pixelSize === 12)
            compare(titles.map(t => t.text), root.times)
            const work = cellOf(grid, "desk", "work")
            verify(work !== null)
            compare(buttonOf(work).text, "Travail  ▾")
            compare(buttonOf(work).Accessible.name, "Bureau de travail, Travail : Travail")
            compare(buttonOf(cellOf(grid, "desk", "leisure")).text, "Rien  ▾")
            compare(buttonOf(cellOf(grid, "phone", "work")).text, "Travail · Loisirs  ▾")
            // Each time's cells under its name: one column.
            for (let t = 0; t < 5; ++t) {
                const id = ["work", "admin", "leisure", "meals", "sleep"][t]
                const x = [cellOf(grid, "desk", id), cellOf(grid, "phone", id)].map(c => Math.round(c.mapToItem(grid, 0, 0).x))
                compare(x[0], x[1], id)
                compare(Math.round(titles[t].mapToItem(grid, 0, 0).x), x[0], id)
            }
            // Drawn apart: the cells you changed, and only they.
            compare([cellOf(grid, "phone", "work").border.width, cellOf(grid, "phone", "meals").border.width, cellOf(grid, "phone", "admin").border.width, cellOf(grid, "desk", "work").border.width], [1, 1, 0, 0])
            // An older Sioul: no cell, one sentence.
            verify(cellOf(grid, "tablet", "work") === null)
            const older = all(grid, i => i.objectName === "cellsOlder" && i.visible)
            compare(older.length, 1)
            compare(older[0].text, "Son Sioul est plus ancien : tout y arrive encore.")
        }

        // A cell opens three boxes; a box says the device, the time and the
        // three as they then stand, "none" when nothing is left ticked.
        function test_a_box_says_the_device_the_time_and_the_areas() {
            const grid = made(true, 1000)
            const spy = createTemporaryObject(spyComponent, root, { target: grid, signalName: "picked" })
            const work = cellOf(grid, "desk", "work")
            mouseClick(buttonOf(work))
            tryVerify(() => named(work, "area:leisure") !== null)
            compare([named(work, "area:work").checked, named(work, "area:admin").checked, named(work, "area:leisure").checked], [true, false, false])
            verify(!named(work, "areaFilterAsHours"), "as its areas say already: nothing to put back")
            mouseClick(named(work, "area:leisure"))
            compare(spy.count, 1)
            compare([spy.signalArguments[0][0], spy.signalArguments[0][1], spy.signalArguments[0][2]], ["desk", "work", "work+leisure"])
            mouseClick(named(work, "area:work"))
            compare([spy.signalArguments[1][0], spy.signalArguments[1][1], spy.signalArguments[1][2]], ["desk", "work", "none"], "Rust's word until it answers: only work was ticked, and it is unticked")
        }

        // "As its areas say": on a changed cell only, in its own words, and
        // it asks for the cell back ("").
        function test_as_its_areas_say_puts_a_changed_cell_back() {
            const grid = made(true, 1000)
            const spy = createTemporaryObject(spyComponent, root, { target: grid, signalName: "picked" })
            const meals = cellOf(grid, "phone", "meals")
            mouseClick(buttonOf(meals))
            tryVerify(() => named(meals, "areaFilterAsHours") !== null)
            const back = named(meals, "areaFilterAsHours")
            compare(back.text, "Selon ce à quoi il sert")
            mouseClick(back)
            compare(spy.count, 1)
            compare([spy.signalArguments[0][0], spy.signalArguments[0][1], spy.signalArguments[0][2]], ["phone", "meals", ""])
        }

        // At a phone's 412 points: each device says its cells in words, one
        // line per time under the time's name; no matrix; nothing past the edge,
        // the boxes and their help neither.
        function test_on_a_phone_each_device_says_its_cells_in_lines() {
            root.width = 412
            const grid = made(false, 380, [root.rows[1]])
            waitForRendering(grid)
            const lines = ["work", "admin", "leisure", "meals", "sleep"].map(id => cellOf(grid, "phone", id))
            verify(lines.every(c => c !== null))
            // One under the other, in the times' order.
            const tops = lines.map(c => Math.round(c.mapToItem(grid, 0, 0).y))
            for (let i = 1; i < tops.length; ++i)
                verify(tops[i] > tops[i - 1], "line " + i + ": " + tops)
            const titles = all(grid, i => i.visible && root.times.indexOf(i.text) >= 0)
            compare(titles.map(t => t.text), root.times)
            compare(buttonOf(lines[0]).text, "Travail · Loisirs  ▾")
            compare(buttonOf(lines[3]).text, "Rien  ▾")
            mouseClick(buttonOf(lines[0]))
            tryVerify(() => named(lines[0], "areaFilterHelp") !== null)
            mouseClick(named(lines[0], "areaFilterHelp"))
            tryVerify(() => named(lines[0], "areaFilterHelpText") !== null && named(lines[0], "areaFilterHelpText").lineCount > 1)
            const drawn = all(grid, i => i.visible && i.width > 0).concat(all(grid.Overlay.overlay, i => i.visible && i.width > 0 && (i.objectName || "").indexOf("area") === 0))
            verify(drawn.length > 10, drawn.length + " items")
            for (const item of drawn) {
                const at = item.mapToItem(null, 0, 0)
                verify(at.x >= -0.5 && at.x + item.width <= root.width + 0.5, (item.objectName || item.text || item) + ": " + at.x + " + " + item.width)
            }
            root.width = 1100
        }
    }
}
