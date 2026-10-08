# Qt Quick and QML, as Sioul uses them

This page is for a developer who has not written QML. It shows how Sioul's window is written, with excerpts from its pages, and names the pitfalls the code's comments point at. Qt's own documentation teaches the language: [the QML reference](https://doc.qt.io/qt-6/qmlreference.html). How QML reaches Rust is the next guide, [the bridge](cxx-qt.md).

## What Qt, Qt Quick and QML are
- **Qt** is a C++ framework for programs with windows, on Linux, Windows, macOS and Android. Sioul uses Qt 6 (6.9 or newer, [building.md](building.md)).
- **QML** is Qt's language for interfaces. A `.qml` file describes a tree of objects, each with *properties* (its values), *signals* (what it says happened) and *functions*, written in JavaScript.
- **Qt Quick** is the library of visual objects QML is drawn with (`Item`, `Rectangle`, `Text`, `Image`, `ListView`…); **Qt Quick Controls** adds buttons, fields, menus and dialogs; **Qt Quick Layouts** places them in rows, columns and grids.

The window's QML is in `crates/sioul-app/qml/`. It holds no rule about your data: Rust gives it text and lists, already decided and worded, and QML lays them out ([start-here.md](start-here.md#the-rule-of-the-layers)).

## A QML file is a type
Each file defines one type, named after the file. `crates/sioul-app/qml/StoppedCard.qml`, the card "Where you stopped", begins:

```qml
pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: card

    required property var sioul
    required property var window
    property var stopped: null

    function reload() {
        card.stopped = JSON.parse(card.sioul.stopped() || "null")
    }

    visible: card.stopped !== null
    Component.onCompleted: card.reload()
```

- **The imports**: Qt Quick, its controls in the *Basic* style (below), its layouts. A file that names Sioul's own types written in Rust (`Sioul`, `Desktop`) also imports Sioul's module, `import com.aurelienpierre.sioul`, as `main.qml` does.
- **`Panel { … }`**: the card is a `Panel`, Sioul's own type from `Panel.qml` in the same folder: a soft surface with a thin border, itself a Qt `Pane`. Another file uses the card the same way, as `StoppedCard { … }` (`PorchPage.qml`, `TasksPage.qml`).
- **`id: card`** names the object inside this file. Ids are not properties: other files cannot see them.
- **`property var stopped: null`** declares a property. `var` holds any JavaScript value; the code also uses `string`, `bool`, `int`, `real` and `color`.
- **`function reload()`** is JavaScript. Here it asks the bridge object for "where you stopped" (`card.sioul.stopped()`, a JSON text) and keeps it parsed.
- **`Component.onCompleted`** runs once the object is made.

Each object in the file is written the same way: its type, then its properties, its handlers and its children between braces.

## Properties and bindings
`visible: card.stopped !== null` is not assigned once: it is a *binding*. QML notes which properties the expression read (`card.stopped`) and computes it again whenever one of them changes. Most of a page is bindings: a label's `text`, a button's `enabled`, a row's `visible`, all follow the data they read. This is what makes a page show new data without code that updates it.

- **A binding reads properties, never the disk.** When a property of the bridge object changes (Rust sets `porch`, `tasks`…), every binding that reads it runs again ([cxx-qt.md](cxx-qt.md)).
- **`readonly property`** can only be bound, never assigned: `readonly property var rows: JSON.parse(page.sioul.overlaps || "[]").filter(o => o.today)` in `PorchPage.qml`.
- **Assigning breaks a binding.** `card.stopped = …` in `reload` is an assignment, which is fine for a property that had none. But assigning to a property that had a binding replaces the binding for good. Checkable buttons meet this: a click on a checked button unchecks it, and its `checked: …` binding is gone. `BudgetDetail.qml` binds it again with `Qt.binding`:

    ```qml
    // A click on the step shown unticks it: its binding ticks it again.
    onClicked: {
        detail.step = stepButton.modelData
        stepButton.checked = Qt.binding(() => detail.step === stepButton.modelData)
    }
    ```

- **A `var` holding an object or an array tells nobody when its inside changes**, only when the property itself is given a new value. `main.qml` keeps which pages are made in `made`, an object; to mark one, it copies it, changes the copy, and assigns it, so that `onMadeChanged` runs:

    ```qml
    const made = Object.assign({}, window.made)
    made[window.page] = true
    window.made = made
    ```

Qt's documentation: [property binding](https://doc.qt.io/qt-6/qtqml-syntax-propertybinding.html).

## Signals and handlers
A *signal* says that something happened; a *handler* is the code that runs then. Each property has its own: `stopped` changing emits `stoppedChanged`, handled by `onStoppedChanged`. Controls have theirs: a `Button` emits `clicked`, handled by `onClicked`. A file declares its own with `signal changed` (`Interruption.qml`) and emits it by calling it, `dialog.changed()`.

To handle the signals of another object, such as the bridge object every page shares, a page uses `Connections`. `StoppedCard.qml` reads itself again when the tasks or the Porch change, since the line may have been left or cleared elsewhere (the focus window, another page, another device):

```qml
Connections {
    target: card.sioul

    function onTasksChanged() {
        card.reload()
    }

    function onPorchChanged() {
        card.reload()
    }
}
```

The bridge's own signals arrive the same way: `main.qml` handles `onCopyRequested(text)`, `onTaskDone(uid)` and others in a `Connections` whose target is `sioul`. Qt's documentation: [signal and handler event system](https://doc.qt.io/qt-6/qtqml-syntax-signals.html).

## Required properties, and how a page gets `sioul`
`required property var sioul` says that whoever makes the card must give it `sioul`; QML refuses to make it otherwise. This is how every page and every part of a page reaches the one bridge object:

1. **`main.qml` makes it once**, with the theme:

    ```qml
    Theme {
        id: theme
        …
    }

    Sioul {
        id: sioul
    }
    ```

2. **Each page receives it** when it is made, among its required properties: `loaders[i].setSource(window.pageFiles[i], given)` (`makePages` in `main.qml`), where `given` holds `sioul`, `theme` and, for most pages, `window`.
3. **Each part of a page receives it from the page**. `PorchPage.qml` makes the card so:

    ```qml
    StoppedCard {
        Layout.fillWidth: true
        sioul: page.sioul
        theme: page.theme
        window: page.window
    }
    ```

Three objects travel this way everywhere: `sioul` (the bridge to Rust), `theme` (colours and sizes) and `window` (the main window, which opens things: `card.window.openTask(card.stopped.task)`). Qt's documentation: [required properties](https://doc.qt.io/qt-6/qtqml-syntax-objectattributes.html#required-properties).

## `pragma ComponentBehavior: Bound`
Most of the window's files start with this line. A *component* is a piece of QML written inside a file to be made later, perhaps many times: a list's `delegate`, a `Loader`'s `sourceComponent`. With the pragma, each component of the file stays bound to the file it is written in: it may use the file's ids, and Qt's tools can check those uses. Its delegates then take their data as required properties, never as names the model would put in their scope:

```qml
Repeater {
    model: overlapsCard.rows

    delegate: ColumnLayout {
        id: overlap

        required property var modelData
        …
    }
}
```

Qt's documentation: [QML document structure, pragmas](https://doc.qt.io/qt-6/qtqml-documents-structure.html).

## Lists from JSON
Rust hands most lists to QML as JSON text. QML parses them, and a `Repeater` or a `ListView` makes one *delegate* (one copy of a component) per entry, given the entry as `modelData`. The Porch's card of overlapping events (`PorchPage.qml`):

```qml
Panel {
    id: overlapsCard

    // Made with the agenda, off the window's thread.
    readonly property var rows: JSON.parse(page.sioul.overlaps || "[]").filter(o => o.today)

    visible: overlapsCard.rows.length > 0
    …
        Repeater {
            model: overlapsCard.rows

            delegate: ColumnLayout {
                id: overlap

                required property var modelData
                …
                Label {
                    text: page.sioul.textArgs("overlap-today", JSON.stringify({ first: …, second: … }))
                }
```

- **`Repeater`** makes every delegate at once: for short lists (a card's lines, a menu's entries).
- **`ListView`** makes only the rows in sight, and with `reuseItems: true` reuses rows that scroll away (`ContactsPage.qml`): for long lists.
- **The `|| "[]"`** covers the first moment, before Rust has set the property.

Qt's documentation: [Repeater](https://doc.qt.io/qt-6/qml-qtquick-repeater.html), [ListView](https://doc.qt.io/qt-6/qml-qtquick-listview.html).

## Made when first needed
Making QML objects costs time. On a 2019 phone, the Accounts page took five seconds to open while it made every form it holds; with its parts made when first needed, each page now opens in 0.1 to 0.55 seconds the first time ([android.md](android.md#pages-and-scrolling)). So the window makes little at first, and the rest when it is first shown.
- **Pages**: `main.qml` holds one empty `Loader` per page. `made` lists the pages made; the Porch is the only one at the start. Showing a page for the first time marks it, and `makePages` gives its `Loader` the page's file with `setSource(file, { sioul: …, theme: …, window: … })`. A page made is then kept.
- **Forms, dialogs and menus**: `Later.qml`, a `Loader` not active until `now()` is called:

    ```qml
    Loader {
        // Out of the layouts it sits in: a form opens over the window.
        visible: false
        active: false

        function now() {
            active = true
            return item
        }

        function close() {
            if (item)
                item.close()
        }
    }
    ```

    A page writes `form.now().open()`, never `form.open()`.

- **Windows that open with a session**: a `Loader` whose `active` is bound to the state, such as the focus window's (`active: sioul.focusSession !== ""` in `main.qml`).
- **What follows from it**: an object made later does not exist before. A `Loader`'s `item` is `null` until then, so code that reads it checks first (`titleBar.item ? … : null` in `main.qml`); and an id inside a `Later` or a `Loader` is not seen outside it: the page reaches the object through `now()` or `item`.

Qt's documentation: [Loader](https://doc.qt.io/qt-6/qml-qtquick-loader.html).

## The Basic style and the theme
- **One style, Basic.** Qt Quick Controls come in several *styles* (Material, Fusion, the system's…). Every file that uses controls imports `QtQuick.Controls.Basic`, and `crates/sioul-app/src/lib.rs` sets `QT_QUICK_CONTROLS_STYLE=Basic` unless you chose another, so that even Qt's own tooltips are drawn in it. Qt's documentation: [styles](https://doc.qt.io/qt-6/qtquickcontrols-styles.html).
- **`Theme.qml`** is a `QtObject` (an object that draws nothing) with Sioul's colours, light or dark (`background`, `surface`, `text`, `muted`, `accent`…), its spacing (`gap`, narrower on a phone) and radius, and the reading settings (`readingFamily`, `readingSize`, `readingSpacing`). `main.qml` makes it once, and it travels as `theme`. Pages take their colours from it (`color: card.theme.text`); the one exception is the map's credit, drawn over the map's own colours (`ContactMap.qml`).
- **`SioulWindow.qml`**, the type of every window of Sioul, gives the theme's colours to Qt's controls through the window's `palette`.
- **`Panel.qml`**, the card; **`Icon.qml`**, an icon; **`SioulMenu.qml`**, every menu (as wide as its widest line, kept inside the window). Use them rather than Qt's types directly, so that every page looks the same.

## Icons, fonts and resources
A *resource file* (`.qrc`) lists files that the build compiles into the program; QML and C++ reach them by addresses that begin with `qrc:/` or `:/`.
- **The pages themselves** are resources of Sioul's QML module: `crates/sioul-app/src/lib.rs` loads `qrc:/qt/qml/com/aurelienpierre/sioul/qml/main.qml`.
- **Icons** are named as the desktop's icon themes name them (`view-task`, `mail-message`: `Theme.qml`'s `kindIcons`), and drawn by `Icon.qml`, an `IconImage` read on a thread. The Breeze icons Sioul uses are bundled, light and dark, in `crates/sioul-app/icons/`; `crates/sioul-app/src/desktop.rs` (`icons`) makes them stand in for whatever the desktop's icon theme lacks. A new icon: name it in the QML or the Rust, then run `tools/bundle-icons.py` again (it needs the Breeze icon theme installed), which copies every Breeze icon the sources name and writes `icons.qrc`.
- **Sioul's own icon** (`app.qrc`) and **Sioul Symbols**, a small font of the symbols system fonts may lack (`fonts/fonts.qrc`, made by `tools/make-symbols-font.py`).

`build.rs` names the three `.qrc` files ([cxx-qt.md](cxx-qt.md#buildrs-what-the-module-holds)). Qt's documentation: [the Qt resource system](https://doc.qt.io/qt-6/resources.html).

## Words
A page holds no sentence of its own (proper names and credits aside, such as `© OpenStreetMap`): every text is asked of Rust by its message's name, `sioul.text("stopped-done")`, `sioul.textWith("stopped-at", "when", card.stopped.when)`, `sioul.textArgs(…)`. The Qt way, `qsTr()`, is not used. How the messages are written: [fluent.md](fluent.md).

## A computer and a phone
The same files serve both. What differs:
- **The width**: `main.qml`'s `compact` is true below 720 pixels (a phone, or a narrow window). The places then come over the pages from the left (☰), and each page shows one pane at a time; a page that opened something (a message, a task) says so with `canGoBack`, and its `back()` closes it, which Android's Back calls. `theme.compact` narrows the margins.
- **The system**: `Qt.platform.os === "android"` where only a phone differs (the window has no title bar of its own there, nor a tray).
- **Files of their own**: Android has no Qt WebEngine and no Qt PDF, so `qml/android/SitesPage.qml` and `qml/android/PdfView.qml` replace the computer's (`build.rs` lists one or the other, and `main.qml`'s `pageFiles` picks the Sites page). The tray, `qml-desktop/Tray.qml`, is loaded on computers only, in a folder apart so that the phone's build does not carry it.
- **Touch**: a right click opens a menu with a mouse; a finger has no buttons. Each such place has two handlers, one for the mouse and the touchpad, one for the touch screen's long press (`DndApplet.qml`):

    ```qml
    TapHandler {
        acceptedButtons: Qt.RightButton
        // A touch has no buttons: on a touch screen, the long press below.
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onTapped: applet.openMenu()
    }
    TapHandler {
        id: hold

        acceptedDevices: PointerDevice.TouchScreen
        onLongPressed: {
            …
            applet.openMenu()
        }
    }
    ```

- **Checking a phone's layout on a computer**: `SIOUL_GRAB_PHONE` lays the window out at a phone's size (412 × 891) when it saves images of the pages ([android.md](android.md#what-differs-on-android)).

## Pitfalls the code's comments name
- **Binding loops.** A binding that, through others, changes what it reads, runs again and again; Qt stops it and warns ("Binding loop detected"). `TaskPanel.qml` gives the title field a fixed `implicitWidth: 120`, since "the text's width would follow the width it sets, a binding loop". `MailPage.qml` opens the first folder in `Qt.callLater(…)`, after the bindings reading the accounts are done, because opening it at once "changes what they read (a binding loop when done at once)". `Qt.callLater` runs a function once the current changes are over.
- **Assigning over a binding** ends it (above): bind it again with `Qt.binding`.
- **What is made later is `null` before** (above): check a `Loader`'s `item`, reach a form through `now()`.
- **Changing an object inside a `var`** tells nobody (above): assign a new one.
- **A page out of sight** keeps what it showed: `PorchPage.qml` copies `sioul.porch` into its own `porchText` only while it is visible, and again when it comes back, so that results landing meanwhile cost nothing.
- **A window shown too early**: `main.qml` sets `visible: true` and maximizes the window in `Component.onCompleted`; its comment says that a `visibility` set in the declaration "left a phone's window unshown: a black screen".
- **A tray menu shown by itself**: `Qt.labs.platform`'s `Menu` is visible by default, and a visible menu is a menu on the screen: Plasma's tray menu, and the widgets' fallback, then opened it at the screen's top left corner, unasked, as Sioul started. `qml-desktop/Tray.qml` sets `visible: false` on it; a right click on the tray icon still opens it.
- **Every object made costs something later**: with Qt 6.11, each item shown, hidden or restacked makes Qt walk the window's whole tree at the next frame ([android.md](android.md#pages-and-scrolling)). Make as little as the page needs.

## Checking QML
- **qmllint**, Qt's linter: `tools/lint-qml.sh`, after `cargo build -p sioul-app`. It lints the pages as one module, with the `qmldir` and the type description (`plugin.qmltypes`) the build wrote, so that qmllint knows Sioul's own types. Qt's documentation: [qmllint](https://doc.qt.io/qt-6/qtqml-tooling-qmllint.html).
- **Tests**: `tools/qml-test.sh` runs the QML tests of `crates/sioul-app/tests/qml/`, each component on a stand-in for Sioul, with Qt's QtTest; no build needed ([building.md](building.md#the-checks-before-a-commit)).
- **Images of every page**: `tools/demo/run.sh <folder> en pages` starts the window on the demo profile, in a sandbox, shows each page in turn, saves it and quits (`SIOUL_GRAB`); another step list in place of `pages` acts in the window. It is the one way to start the window for a check ([building.md](building.md#running-the-window-for-a-check)).
- **Qt's messages** (QML errors, warnings, `console.info`): Fedora's Qt sends them to the system journal; `QT_FORCE_STDERR_LOGGING=1` brings them back to the terminal. Each page's making time is logged as `sioul-perf: … made in … ms`.
