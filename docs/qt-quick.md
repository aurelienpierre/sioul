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
                    text: page.sioul.textArgs("moved-ask", JSON.stringify({ payee: …, budget: … }))
                }
```

- **`Repeater`** makes every delegate at once: for short lists (a card's lines, a menu's entries).
- **`ListView`** makes only the rows in sight, and with `reuseItems: true` reuses rows that scroll away (`ContactsPage.qml`): for long lists.
- **The `|| "[]"`** covers the first moment, before Rust has set the property.

Qt's documentation: [Repeater](https://doc.qt.io/qt-6/qml-qtquick-repeater.html), [ListView](https://doc.qt.io/qt-6/qml-qtquick-listview.html).

## A list keeps its place
**The rule.** When an item of a list is added, changed or deleted, here or on another device, the list does not move under the eye: the row at the top of its view stays where it was, to the pixel; when that row is the one deleted, the next one takes its place there, and the rows above it do not move. A list goes to its top only when the person changes what it lists (another filter, search, folder, day) or asks for it. A row the person has just made from the list comes into view by the smallest move, and only when it would be out of sight. At its very top, a list stays at its top, so that what comes first shows (as a browser keeps a page). While the person drags or flicks a list, nothing moves under the finger: new rows wait until the movement ends.

**Why Qt does not do it alone.** A list from JSON is a new array each time Rust says something changed, and Qt treats a new array as a new model: a `ListView` given one goes back to its top at once (Contacts, scrolled to its 22nd card, went to 0 when one card above was deleted), unless the array is equal to the last one, entry for entry. A `Repeater` keeps its `ScrollView`'s position, but makes its rows anew, and every row under a change moves by the change's height (the Health page's day moved by one row of 52 points for a meal deleted above; the Porch, by one row of 46). A model changed in place row by row (Mail's mailbox, Movements) keeps its rows, but each row then shows the next one's words.

**One mechanism**, `ListPlace.qml`, beside each list that scrolls. It notes the place before the rows change (the row at the top of the view by its key, with how far the view's top is into it, and the rows around it), lets the view take the new rows, and puts the place back: that row where it was; gone, the first row under it still there, in its place; none, the nearest row above it, where it was. This is what Qt itself does for a `ListModel` changed row by row by key (the Week's list, `WeekPlanning.qml`'s `diff`), and what the mechanism does for every other list. It puts it back once the window has laid the new rows out (`Window.afterAnimating`, which Qt sends after its polish and before the frame is drawn): a row whose words wrap learns its height only in that layout, and a place put back before it is off by every such row above. A `ListView`, which Qt has sent to its top at once, is also put back at once: the rows it makes there are laid out on the spot (`ensurePolished` on each, then on the view), so that nothing under them moves a frame later. `tst_listplace.qml` checks every case on each kind of list, and shows the place put back only once, before the layout, off by several points.

**How a list joins it.** Three shapes, one component:

- A `ListView` given an array: the `ListPlace` holds the array, the view reads its `rows`; `key` names the field that tells a row (`"key"` by default, or a function of the row); `scope` is what the list lists. Contacts:

    ```qml
    ListPlace {
        id: listPlace

        view: list
        source: page.shown.contacts
        scope: [page.shown.filter, page.shown.category]
    }
    ListView {
        id: list

        model: listPlace.rows
        …
    }
    ```

- Rows that are a `Repeater`'s in a `ScrollView` or a `Flickable`, and the rows inside a `ListView`'s own rows (a group's tasks, a day's events): each such row's delegate says its key, `readonly property string placeKey: modelData.uid`, and the `ListPlace`'s `view` is the `ScrollView` (or the `ListView`). Several `Repeater`s may share one view: each row is found by its `placeKey`, wherever it is, and a key is read within the keys of the rows that hold it (a lane's, then a message's).
- A `ListModel` the page changes in place: `rows: theModel`, and the change runs inside `place.keep(() => …)`, which holds it while the person drags.
- A page whose whole answer feeds a scrolling view with many parts (Tasks ▸ Now): the `ListPlace`'s `view` is that view, its rows say their `placeKey`, and the page takes its answer inside `place.keep(() => …)`.
- A `Repeater` given an array equal to the last, entry for entry, does not make its rows anew. The Tasks board gives its columns without their cards (`{ id, title }`), so that a card moved leaves the columns, and each column's own scroll and `ListPlace`, as they are.

A row made here: `place.reveal(key)` before or after asking Rust to make it; the row comes into view when a later reading holds it. The view's own resets stay as they were: a page that goes to its top on purpose (a new search in Mail) still does, and `place.toTop()` does it for the person's own ask. A page that brings a row into view itself right after its rows change (Health's row chosen in the agenda, Notes' folder unfolded, the letter just read) calls `place.leave()` first, so that the place is not put back over it. A view at its top stays there; `fresh()` says that another scope's rows are shown, for a page that opens such a list elsewhere than its top (the Agenda's What comes on today).

**What changes for code and tests.** The view takes its rows once the change's bindings have settled (`Qt.callLater`): a scope that comes from the same answer as the rows is read settled. Code that reads the view's rows right after changing the data reads the view's model (`place.rows`), or waits a turn; a test does the same (`tryVerify`, `tryCompare`). Rows held while the person drags come when the movement ends.

**What does not need it**: a list that never scrolls on its own (a card's lines, a menu, chips), a form's rows the person is typing in (they change only as typed), a list read once when its dialog opens.

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
- **Light or dark** (`Theme.darkFrom`, tried by `crates/sioul-app/tests/qml/tst_theme.qml`): the colours chosen in Settings; else, with high contrast on (`Qt.styleHints.accessibility.contrastPreference`), the system's own colours by their lightness (`SystemPalette`'s window colour); else the system's colour scheme (`Qt.styleHints.colorScheme`) when it says one; else that lightness again. `main.qml` binds the Theme to them, so a change made while Sioul runs is followed at once. The scheme comes before the palette since 14 October 2026: Qt 6.11's Windows Vista style (Windows 10; Windows 11 has a style of its own) draws a light palette while the system is dark (`QWindowsVistaStyle::polish`), and the window stayed light. In high contrast Sioul still draws its own colours: the system's decide light or dark, not the colours themselves.
- **`SioulWindow.qml`**, the type of every window of Sioul, gives the theme's colours to Qt's controls through the window's `palette`. Its `base`, Qt's colour for what you type in, is the theme's `field`: text fields, spin boxes and boxes to tick have a fill of their own.
- **`Panel.qml`**, the card; **`Icon.qml`**, an icon; **`SioulMenu.qml`**, every menu (as wide as its widest line, kept inside the window); **`Tabs.qml`**, every row of tabs; **`PageColumn.qml`**, a page's column; **`Section.qml`**, a heading and a frame around a group of fields (below). Use them rather than Qt's types directly, so that every page looks the same.

## Sioul's overrides of the Basic style {#basic-style-overrides}
Fields and buttons stand out from the words around them: a text field, a text area, a combo box, a number box and the boxes to tick have a fill of their own, the theme's `field`, and a firmer border, its `edge`, which turns to 2 points of its `focusEdge` with the focus; a flat button that holds words keeps an outline in the `edge`, so that no action is drawn as plain words. Basic draws none of it: a flat button has no background until it is pressed or checked, a text area is bare words, a field has the cards' own colour with a faint border, and its focused border is as light as the firmer one at rest.

- **Where it is done**: once, for the whole program. Every page imports `QtQuick.Controls.Basic`, chosen as the program is built (`qmlcachegen` compiles the pages against it), so a style of Sioul's own would need each page to import it, and every new page to remember to. Instead `cpp/style.cpp` gives the QML engine an interceptor (`QQmlAbstractUrlInterceptor`) before the window's first file is read: wherever the engine would load the Basic style's own `Button.qml`, `TextField.qml`, `TextArea.qml`, `CheckBox.qml`, `RadioButton.qml` or `SpinBox.qml`, it loads Sioul's from `qml/style/` (`StyleButton.qml`…). Every window and dialog gets them, Qt's own included. Qt's `ToolButton` and the other controls stay Basic's.
- **Which Qt they are written for**: every Qt the build accepts, 6.9 and newer. Each file is Basic's as Qt 6.9 and 6.10 write it, which Qt 6.11 loads too, under the licences Qt gives it (`NOTICES.md`), with nothing newer than 6.9: a button's icon tinted by `icon.color`, not by 6.11's `IconLabel.defaultIconColor`; the system's high contrast (`Qt.styleHints.accessibility`, 6.10) read only where Qt has it. A property a Qt does not know stops the file, and with it every window: Qt 6.11's `Button.qml` copied as it was left the Flatpak (Qt 6.10) with no window (review of 18 November 2026). The release's Flatpak job starts the built program once on that Qt, offscreen, through the `shell` picture steps, and stops when its window does not load or when the log's `style:` line says the overrides are not in place (`.github/workflows/release.yml`).
- **What they change**, each its background or its border alone; every size is Basic's:
  - `StyleButton`: a flat button at rest (not pressed, checked nor highlighted) that holds words (a letter or a digit) has no fill and a 1-point outline in the `edge`, fainter when disabled. A flat button of an icon alone, or of an arrow or a cross alone, stays bare, as Basic draws it, with a ring while the keyboard's focus is on it. Every other button is drawn as Basic draws it.
  - `StyleTextField`: its border in the `edge`; with the focus, 2 points of the `focusEdge`.
  - `StyleTextArea`: a text area you can type in is a field, the `field`, the `edge` and the `focusEdge`; a read-only one (a text to select and copy) stays bare.
  - `StyleCheckBox`, `StyleRadioButton`: the box's or the circle's border in the `edge`, the `focusEdge` with the keyboard's focus; `StyleSpinBox`: a number box's border in the `edge`, its focused frame in the `focusEdge`.
  - A control that brings its own `background:` keeps it; the pages that draw their own field (a note, a task's title and notes, a contact's, an event's, a paper's, a contract's, the message being written, the day's review…) draw it in the `field`, the `edge` and, with the focus, 2 points of the `focusEdge` too. Every combo box is a `PlainComboBox`, drawn as a field there.
- **What is not an action of the page**: `BareButton.qml`, Basic's flat button with a focus ring, never outlined, for a row of a list or a panel (Mail's addresses among their folders), a fold line ("▸ Server and folders", "▸ More details"), a line of a menu made of buttons (topics, the people of do-not-disturb, the pause's numbers), a dose already taken. A button inside a field (a password's eye) shows its icon alone, and stays bare by the rule above.
- **Their colours**: the overrides read the window's theme (`Window.window.theme`, as `Icon.qml` does: every window of Sioul's is a `SioulWindow`); a window of Qt's own has none, and gets the palette's `mid` and `highlight` instead. In both themes (WCAG's contrasts):

  | | light | dark |
  |---|---|---|
  | text on the `field` | 12.1:1 | 13.5:1 |
  | a placeholder (`muted`) on the `field` | 4.9:1 | 6.9:1 |
  | the `edge` against the field / the cards / the page / a filled button | 3.4 / 3.9 / 3.6 / 3.2 | 4.2 / 3.6 / 4.0 / 3.0 |
  | the `focusEdge` (the accent's hue) against the `edge` / the `field` | 3.1 / 10.6 | 3.1 / 13.0 |
  | the `field` against the cards (`surface`) | 1.13 | 1.18 |

  `tst_style.qml` checks them, and the overrides' drawing and sizes against Basic's.
- **The fields' menu** (cut, copy, paste on a right click or a long press): Qt 6.11's Basic gives its fields one (`TextEditingContextMenu`); Qt 6.10, the Flatpak's runtime, has none. The theme looks for it once, off the window's start (`Theme.fieldMenu`); where it exists, each field makes it at its first right click; where it does not, a field hears no request at all, and the click goes on to what is under it, as with Basic's own.
- **Checking**: `qmltestrunner` does not run `cpp/style.cpp`, so the QML tests draw Basic's own controls, except `tst_style.qml`, which makes the overrides directly. The window's pictures show them (`tools/demo/run.sh`), and the `shell` steps' log says whether they are in place.

## A row of actions

An action's button carries its icon and its word (`ActionButton.qml`): an icon alone only for the signs that need no word, ⚙, ✕, ⋮, + and ◂ ▸ (`compact`), named on hover and to screen readers. A row of them (a message's in the reader, the contacts' list's head) gives each an `ActionFolds.qml`, which knows the room the row has for them (`room`, measured on what its column is given, never on the row's own width, which its minimums would hold wide), each action's width with its word (`wideWidth`), whether it applies now (`offered`) and its rank (`rank`, the lowest staying the longest). On a phone (`folding`, the theme's `touch`) the actions show in their rank while their words fit, and the others wait in the row's ⋮ menu with their words (`folded`, listed first in the menu by an Instantiator whose items press the action itself: `clicked()`); a dimmed action that still says why when pressed (Unsubscribe while nothing may be contacted) stays pressable there, dimmed. On a computer nothing folds, so that what took one press still takes one: a row too narrow for its actions (`wraps`) lays them out on a second line, with their words (the reader's row is a `Flow`, the space between its two groups taking the rest of a line they fit on; the contacts' actions move under the search). `tst_actionrow.qml` checks both.

## Tabs
A page that has several views switches between them with `Tabs.qml`, one row of tabs: Settings' sections on a computer, Tasks' six views, Accounts, Money, the Agenda's views (What comes, Day, Week, Month, Search) and Time's period (Week, Month, Year).

```qml
Tabs {
    objectName: "moneyTabs"
    Layout.fillWidth: true
    theme: page.theme
    model: [{ id: "overview", text: page.sioul.text("money-tab-overview") }, …]
    current: page.tabId
    onChosen: id => page.tabId = id
}
```

- **What it shows**: a thin rule under the whole row; the tab shown marked by a thick line in the accent and strong words; the others in muted words, darker with a fine line under the pointer; no fill. Qt's own `TabButton` is not used: under Sioul's palette its Basic look fills the tabs not chosen with the accent.
- **How wide**: it spans the width it is given (`Layout.fillWidth`). Each tab takes an equal share while every name fits in one; else each takes its name's width (measured bold, so that the row never moves as you choose, and measured again when the font changes) and the room left is shared among them. A tab's words are never shortened with "…".
- **When the names are wider than the row**, it scrolls sideways, by a finger or the wheel, and says so: the side that has more fades into the page (`ground`); the tab at that edge shows cut, never whole (the tabs are widened alike so that the row's edge falls in a tab's middle, and a tab brought into view keeps part of its neighbour beside it); on a computer, a chevron at that end brings the next tab into view. The tab shown is kept in sight. Settings at a window of 1100 points scrolls (11 sections), so does Tasks on a phone in French.
- **Keys and screen readers**: the keyboard's focus is on the tab shown: Tab reaches it (or a click), then leaves the row; Left and Right choose the tab before or after, around the ends, Home and End the first and the last, and the focus follows. A screen reader hears a list of page tabs named after the page (`name`), each tab by its words, checkable, the one shown checked and selected: the tabs are checkable buttons, whose state Qt itself tells an assistive tool. `tst_tabs.qml` runs with one listening (`qml-test: accessibility`: `tools/qml-test.sh` starts it on a headless Weston, where Qt's AT-SPI bridge works; offscreen has none).
- **The page decides**: `model` holds `{ id, text }`, `text` fixed words (a translation) or made plain; `current` is the id shown; `chosen(id)` says which was chosen, and the page shows it.
- **Tests find a tab** by its name, `<objectName>:<id>` (`moneyTabs:movements`, `agendaModes:week`, `settingsTabs:devices`, `taskModes:list`, `accountTabs:add`, `timePeriods:month`). `tst_tabs.qml` checks the shares, the scrolling and what says it, the chevrons, the tab kept in sight, a font changed later, the focus and the keys, and what a screen reader is told.
- **What stays as it is**: filters are not views, and keep their own look: Mail's chips, Contacts' category, a budget's Days, Weeks, Months and Years, Notes' List and Folders, the two- and three-way choices inside forms.
- **On Tasks**, the tabs take their own row; Routines, the project, kind and category filters, the agendas' filter and ⚙ go on the row under them. On the Agenda and Time, the tabs are above the row of Today, ◂ ▸ and the title, and stay as the page scrolls.

## A page's column
Text and forms keep a legible width, centred in the page; tables, grids and cards in several columns take the whole width. `PageColumn.qml` is that column: a `ColumnLayout` given the room the page has (its `ScrollView`'s `availableWidth`), as wide as the theme's `readable` (720 points) at most, centred, or as wide as the room with `wide`.

```qml
ScrollView {
    id: scroll

    contentWidth: availableWidth

    PageColumn {
        theme: page.theme
        room: scroll.availableWidth
        wide: page.section === "devices"
        …
    }
}
```

Settings' sections (What reaches you and Devices `wide`), Papers, Letters and Tasks ▸ Now lay their content in one. The title and the tabs of a page stay outside it, across the page.

## Settings on a phone
On a computer, Settings' sections are tabs across the page. On a phone (`compact`), the page opens on the list of its sections, each with its name and one line saying what it holds (`settings-about-<section>`); nothing of a section is made behind the list, nor the computer's tabs. A tap opens a section, with its name as the page's title, and Back (the system's, or the arrow in the page's bar: `canGoBack` and `back()`, as other pages) returns to the list. A link to a setting from elsewhere (`window.showParameters(key)`, `sioul:settings/<key>`, `settings:<key>`, a notice, the Porch) opens its section there as on a computer, and Back then leaves Settings, as from any page; so does setting `section` (the pictures' steps), with Back to the list. Settings left, the next visit opens on the list. `tst_settingstabs.qml` checks both widths, every kind of link, and that each line of the list says what it holds within its row.

## Sections
`Section.qml` is a heading and a frame around a group of fields, for cards and dialogs: the heading in strong words in the accent, said as a heading to screen readers; under it the group on a card's surface with its thin border, so that the fields on it stand out. What it holds goes in its column. Without its frame (`framed: false`), the group lies flat on what holds it: a device's Details on its card in Settings ▸ Devices, which `CardFold.qml` folds behind a line with its arrow (the same line folds each section of a phone's card, at every width, what it holds made once opened).

```qml
Section {
    Layout.fillWidth: true
    theme: page.theme
    title: page.sioul.text("…")

    TextField {
        Layout.fillWidth: true
    }
}
```

Settings' rows are shown this way, one frame per heading (`group` in `settings::for_view`).

## Icons, fonts and resources
A *resource file* (`.qrc`) lists files that the build compiles into the program; QML and C++ reach them by addresses that begin with `qrc:/` or `:/`.
- **The pages themselves** are resources of Sioul's QML module: `crates/sioul-app/src/lib.rs` loads `qrc:/qt/qml/com/aurelienpierre/sioul/qml/main.qml`.
- **Icons** are named as the desktop's icon themes name them (`view-task`, `mail-message`: `Theme.qml`'s `kindIcons`), and drawn by `Icon.qml`, an `IconImage` read on a thread. The Breeze icons Sioul uses are bundled, light and dark, in `crates/sioul-app/icons/`; `crates/sioul-app/src/desktop.rs` (`icons`) makes them stand in for whatever the desktop's icon theme lacks, and is the theme itself where the desktop names none (Windows, macOS, a phone). A new icon: name it in the QML or the Rust, then run `tools/bundle-icons.py` again (it needs the Breeze icon theme installed), which copies every Breeze icon the sources name and writes `icons.qrc`. It never empties the folder: Sioul's own icons there (`sioul-web.svg` for Sites, `sioul-texts.svg` for Texts: its `OWN` list) are kept and listed, a Breeze icon no longer named goes, and nothing else is touched.
- **The light or dark set follows the window's colours**, not the system's setting: as Sioul starts, the colours chosen in Settings, else the system's as `main.qml` decides them (`cpp/palette.cpp`, as `Theme.darkFrom`); then `main.qml`'s Theme calls `followIcons` whenever its colours change (the system turning dark at night, a choice in Settings), and `Icon.qml` reads its name again from the set now in use. Buttons and menus tint their icons with the palette's colour, so only `Icon.qml`, which draws an icon in its own colours, needs it. Until 13 October 2026 the set was chosen once, by the system's setting (Windows' registry, macOS's appearance, light elsewhere). On Windows 10 in dark mode, Qt's Windows Vista style draws a light palette, so the window was light and its icons light too; a phone in night mode drew dark icons on its dark window; and neither followed a change while Sioul ran.
- **The test of the bundled icons** (`cargo test --release -p sioul-app --test icons`, on the three systems of the CI): every name the window gives (`icon.name`, `iconName`, an object's `icon`, an `IconImage`'s `name`, the tables and functions named for icons, the weather's and the mail folders' from Rust) is read by Qt in both sets with no theme of the system's, and every bundled SVG draws something. A name given any other way is not seen: give it one of these ways.
- **Sioul's own icon** (`app.qrc`) and **Sioul Symbols**, a small font of the symbols system fonts may lack (`fonts/fonts.qrc`, made by `tools/make-symbols-font.py`).

`build.rs` names the three `.qrc` files ([cxx-qt.md](cxx-qt.md#buildrs-what-the-module-holds)). Qt's documentation: [the Qt resource system](https://doc.qt.io/qt-6/resources.html).

## Words
A page holds no sentence of its own (proper names and credits aside, such as `© OpenStreetMap`): every text is asked of Rust by its message's name, `sioul.text("stopped-done")`, `sioul.textWith("stopped-at", "when", card.stopped.when)`, `sioul.textArgs(…)`. The Qt way, `qsTr()`, is not used. How the messages are written: [fluent.md](fluent.md).

## Words from outside, as plain text
Qt guesses rich text. A `Text` or a `Label` left at `Text.AutoText`, the default, draws as HTML any string whose first tag looks like one; the controls (`Button`, `MenuItem`, `CheckBox`, `RadioButton`, `ItemDelegate`, `TabButton`, an `Action`, a `ToolTip`), the title of a `Dialog` or a `Menu`, a text field's `placeholderText` and a `ComboBox`'s list draw their words through such an item, with no `textFormat` to say otherwise. A name, a subject or an event's title holding `<img src="http://…">` then loads that picture from the network, which tells whoever chose the address that it was seen, when and from where; and a button holding it crashed the window (Qt 6.11). Words from outside are everywhere: a sender's name, a contact's or a list's, an event's title from a shared calendar, a folder's name from a server, a site's, a caller's. So every binding that draws words shows one of:
- **fixed words**: a string, a number, a translation (`sioul.text(…)`), or `sioul.textWith(…)`/`sioul.textArgs(…)` whose values are fixed words themselves;
- **words made plain**: `theme.plain(…)` (`Theme.qml`) puts a word joiner, U+2060, after each "<", so that Qt never takes one for a tag, and nothing shows of it; where a file has no theme, the same replacement is written out (`Icon.qml`, the menus of usual sites). In rich text on purpose (a link), words are escaped instead: `theme.escaped(…)`;
- **a `Text` or a `Label` told `textFormat: Text.PlainText`**, the simplest for them, whatever its binding.

Examples, as the pages write them:

```qml
Label {
    text: row.modelData.name
    textFormat: Text.PlainText
}
Button {
    text: page.theme.plain(page.sioul.textWith("overlap-open", "title", openOne.modelData.title))
}
PlainComboBox {
    textRole: "label"
    valueRole: "id"
    model: column.words.fields.map(f => Object.assign({}, f, { label: column.theme.plain(f.label) }))
}
```

What a person types (a `TextField`, a `TextArea`) is plain text already, and kept as typed. Code that saves or sends reads the data, never the words a control draws (a `ComboBox`'s `currentText`): the word joiner would go with them (the form that makes a task list takes its account by its place in the list, `TasksPage.qml`). The few views that are rich on purpose (a message's HTML, made safe in Rust by `reading::safe_html`; Markdown made HTML by `compose::markdown_html`, which shows HTML written in it as words and a picture from elsewhere as a link) say why in a comment holding "rich on purpose:", on their line or the line above. `tools/check-plain-text.py` holds every page to this ("Checking QML", below), and `tst_plaintext.qml` shows Sioul's components such words while a stand-in web server notes what is asked of it: nothing is fetched, nothing crashes.

**"&" in a list or a menu.** A button and a menu line read "&" as the mark of their shortcut key and leave it out ("R&D" drew "RD"), and a `ComboBox`'s list is made of such lines. Menus double each "&" of words from outside (`.replace(/&/g, "&&")`, "&&" being one). Every ComboBox of the window is a `PlainComboBox` (`PlainComboBox.qml`), whose list does it, its closed box being a text field, which shows the text as it is; a ComboBox with a list of its own doubles the "&" itself (the task lists' in `TaskPanel.qml`). `tst_plaincombobox.qml` measures what a line draws against the same words in a plain `Text`, Qt's own list included, which leaves the "&" out.

## A computer, a tablet and a phone
The same files serve all three, and three questions decide what differs, never "is it Android?" alone ([android.md](android.md#tablets)):
- **The width**: `main.qml`'s `compact` is true below 720 pixels (a phone, a tablet in split screen, or any narrow window): the room decides the layout, on every system. The places then come over the pages from the left (☰), and each page shows one pane at a time; a page that opened something (a message, a task) says so with `canGoBack`, and its `back()` closes it, which Android's Back calls (`backTaken`). A popup or a dialog takes the keyboard's focus as it opens (`SafePopup.qml`, `SafeDialog.qml`: `focus: true`), so that Escape and Android's Back close it first: Qt's popups take Back only while they hold the focus, and while one that closes on Escape is open Qt holds the window's shortcuts back. A popup beside a field still being typed into keeps `focus: false`; a Back that nothing took reaches the frame's keys, which go back as the shortcut does. `theme.compact` narrows the margins.
- **The system**: `Qt.platform.os === "android"` only where Android's own ways differ (the window has no title bar of its own there, nor a tray, nor a minimum size; a site opens in a screen of Sioul's own, PDFs in other apps); `sioul.telephony` where calls and texts need a SIM (the Texts place, an `sms:` link, the call screening's settings).
- **The device's kind**: `sioul.deviceKind`, "phone", "tablet" or "computer" (`devicekind.rs`), for what follows the person: the Settings tab's name, where Mail's topics look for groups. Never the layout.
- **An editing card's width** (the task panel, the event form, "How was it?"): no cap of its own. Its natural width is its fields at about 70 characters of the body font (a readable line: Bringhurst's 45 to 75 characters), or its tiles side by side (`CostTiles.naturalWidth`), whichever is wider, measured with a `FontMetrics` of the card's own font. The task panel takes it while the list beside it keeps 480 pixels, or half the room in a narrower window (`TasksPage.panelWidth`); the event form and "How was it?" take it up to the window less the usual margins (`theme.gap`). On a phone, a card takes the whole width less those margins. Over the whole window, it keeps to the window's safe area (`SafeDialog.qml`: never under a phone's system bars; [android.md](android.md#pages-and-scrolling)).
- **Fingers**: `theme.touch` (Android, a phone's or a tablet's, and the runner's phone and tablet sizes) keeps what is tapped a fingertip wide: the tiles' cells are 30 pixels at least, and the tiles stand in one column wherever two would make them narrower.
- **Files of their own**: Android has no Qt WebEngine and no Qt PDF, so `qml/android/SitesPage.qml` and `qml/android/PdfView.qml` replace the computer's (`build.rs` lists one or the other, and `main.qml`'s `pageFiles` picks the Sites page). Both Sites pages share their list, menu and forms (`SitesList.qml`, `SiteRow.qml`, `SiteMenu.qml`, `SiteEditDialog.qml`, `SiteAddDialog.qml`); Android's imports the window's folder (`import ".."`) to find them. The tray, `qml-desktop/Tray.qml`, is loaded on computers only, in a folder apart so that the phone's build does not carry it.
- **Touch**: a right click opens a menu with a mouse; a finger has no buttons. Each such place has two handlers, one for the mouse and the touchpad, one for the touch screen's long press (`ReachButton.qml`):

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

- **Checking a phone's layout on a computer**: `SIOUL_GRAB_PHONE` lays the window out at a phone's size (412 × 891) when it saves images of the pages ([android.md](android.md#what-differs-on-android)). A tablet's: `tools/demo/run.sh --release OUT en tablet`, each page at 1280 × 800, the kind a tablet, no title bar of Sioul's and touch, as on Android ([android.md](android.md#tablets)).

## Pitfalls the code's comments name
- **Binding loops.** A binding that, through others, changes what it reads, runs again and again; Qt stops it and warns ("Binding loop detected"). `TaskPanel.qml` gives the title field a fixed `implicitWidth: 120`, since "the text's width would follow the width it sets, a binding loop". `MailPage.qml` opens the first folder in `Qt.callLater(…)`, after the bindings reading the accounts are done, because opening it at once "changes what they read (a binding loop when done at once)". `Qt.callLater` runs a function once the current changes are over.
- **Assigning over a binding** ends it (above): bind it again with `Qt.binding`.
- **What is made later is `null` before** (above): check a `Loader`'s `item`, reach a form through `now()`.
- **Changing an object inside a `var`** tells nobody (above): assign a new one.
- **A page out of sight** keeps what it showed: `PorchPage.qml` copies `sioul.porch` into its own `porchText` only while it is visible, and again when it comes back, so that results landing meanwhile cost nothing.
- **A window shown too early**: `main.qml` sets `visible: true` and maximizes the window in `Component.onCompleted`; its comment says that a `visibility` set in the declaration "left a phone's window unshown: a black screen".
- **A tray menu shown by itself**: `Qt.labs.platform`'s `Menu` is visible by default, and a visible menu is a menu on the screen: Plasma's tray menu, and the widgets' fallback, then opened it at the screen's top left corner, unasked, as Sioul started. `qml-desktop/Tray.qml` sets `visible: false` on it; a right click on the tray icon still opens it.
- **A field with an input mask, half typed**: Qt says neither `accepted` nor `editingFinished` while the mask is not filled, and a Tab into it leaves the cursor at the end of the mask, where digits go nowhere. `ClockField.qml` selects the whole text on the way in, starts over on a digit when the mask is full, reads what was typed on the way out, and says `editingFinished` itself when that changed the text.
- **Esc and a popup without the focus**: a popup hears Esc only while it holds the focus, so a form's dialog behind it closes instead. A field that keeps the focus beside its popup takes Esc with `Keys.onShortcutOverride` (`ClockField.qml`, `DateField.qml`).
- **The focus and a phone's keyboard**: a `TextInput` given the focus raises the soft keyboard, and it gets the focus back from its dialog when a sheet over it closes. `ClockField.qml` sets `focus: false` on the field before it opens its sheet.
- **A press on a popup's opener**: with `CloseOnPressOutside`, the press closes the popup and the click opens it again. `Popup.CloseOnPressOutsideParent`, with the opener inside the popup's parent, lets the button close what it opened.
- **Every object made costs something later**: with Qt 6.11, each item shown, hidden or restacked makes Qt walk the window's whole tree at the next frame ([android.md](android.md#pages-and-scrolling)). Make as little as the page needs.

## Checking QML
- **qmllint**, Qt's linter: `tools/lint-qml.sh`, after `cargo build --release -p sioul-app`. It lints the pages as one module, with the `qmldir` and the type description (`plugin.qmltypes`) the build wrote, so that qmllint knows Sioul's own types. Qt's documentation: [qmllint](https://doc.qt.io/qt-6/qtqml-tooling-qmllint.html).
- **Words from outside drawn as plain text**: `tools/check-plain-text.py`, which `tools/lint-qml.sh` and `tools/final-pass.sh` run, needs no build. It reads each binding that draws words (`text:`, `title:`, `ToolTip.text:`, `placeholderText:`, a `ComboBox`'s `model:` by its `textRole`, `ToolTip.show(…)`, and words given in JavaScript, `label.text = …`, to an item named by its id) with a small parser of the JavaScript the pages use: a ternary is safe when both its branches are, `a + b` when both are, `x.map(v => …)` when what it maps to is, a comparison always; any other name or call is someone's words until made plain. A custom component is followed to its root type: a `Button` with a `contentItem` of its own draws its words itself, and is checked inside. It prints each binding that fails, `file:line: Type.property: expression`, and exits with 1 when there is one ("Words from outside, as plain text", above).
- **A line that starts with a parenthesis**: JavaScript inserts no semicolon before it, so `f(x)` followed by a line `(a as B).go()` calls what `f` gave back, and qmllint says nothing (the folder chooser of the Notes page's prompt did nothing on Android that way). `tools/lint-qml.sh` refuses such a line unless a condition (`if`, `while`, `for`) or a comment comes before it: put each step in braces, and name what a cast gives (`const own = loader.item as FolderBrowser`) before calling it.
- **Tests**: `tools/qml-test.sh` runs the QML tests of `crates/sioul-app/tests/qml/`, each component on a stand-in for Sioul, with Qt's QtTest; no build needed ([building.md](building.md#the-checks-before-a-commit)).
- **Images of every page**: `tools/demo/run.sh <folder> en pages` starts the window on the demo profile, in a sandbox, shows each page in turn, saves it and quits (`SIOUL_GRAB`); another step list in place of `pages` acts in the window. It is the one way to start the window for a check ([building.md](building.md#running-the-window-for-a-check)).
- **Qt's messages** (QML errors, warnings, `console.info`): Fedora's Qt sends them to the system journal; `QT_FORCE_STDERR_LOGGING=1` brings them back to the terminal. Each page's making time is logged as `sioul-perf: … made in … ms`.
