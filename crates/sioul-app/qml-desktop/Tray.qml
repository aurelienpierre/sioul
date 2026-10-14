// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul in the system tray, on a desktop (main.qml): a click shows the window
// or hides it, Sioul going on behind it (reminders, medicines, mail at its
// hours, the sharing); its menu shows or hides the window, closes the work
// day or the whole day on its sheet in the window (docs/reviews.md), or
// quits. On macOS it is in the menu bar. On Windows the end of work's notice
// and the night's come as its messages, and a click on one opens what it
// offers (main.qml's `onTrayNotice`). Without
// a tray (`available` false: GNOME without its extension, a bare window
// manager), closing the window quits, as before. Plasma's tray menu is made of
// widgets: on a computer Sioul is a widgets application (cpp/application.cpp),
// and this is loaded once the window shows (main.qml's loadTray). Kept out of
// qml/: the phone's build reads that folder for the modules to carry, and this
// one would bring Qt Widgets along for a tray Android has not (build.rs).

import QtQuick
import Qt.labs.platform as Platform

Platform.SystemTrayIcon {
    id: tray

    required property var window
    required property var sioul

    visible: true
    // Sioul's own icon (app.qrc), as on its windows.
    icon.source: "qrc:/sioul/icon/64.png"
    tooltip: "Sioul"
    menu: Platform.Menu {
        // Never shown by itself: the tray shows it at a right click. A menu
        // "visible", as Qt.labs.platform has it by default, is a menu on the
        // screen: Plasma's tray menu (plasma-integration's SystemTrayMenu) and
        // the widgets' (QWidgetPlatformMenu) pass it to their QMenu, which
        // then opened at the screen's top left corner (0, 0), unasked, once
        // Sioul started, until a click elsewhere (seen 7 October 2026).
        visible: false

        Platform.MenuItem {
            text: tray.window.visible ? tray.sioul.text("tray-hide") : tray.sioul.text("tray-show")
            onTriggered: tray.window.toggleShown()
        }
        Platform.MenuSeparator {}
        // At any hour, as the places' two buttons: each on its sheet, the window brought back.
        Platform.MenuItem {
            objectName: "closeWorkDay"
            text: tray.sioul.text("review-close-work")
            onTriggered: tray.window.reviewFromTray("work")
        }
        Platform.MenuItem {
            objectName: "closeWholeDay"
            text: tray.sioul.text("review-close-night")
            onTriggered: tray.window.reviewFromTray("night")
        }
        Platform.MenuSeparator {}
        Platform.MenuItem {
            text: tray.sioul.text("tray-quit")
            onTriggered: tray.window.quitSioul()
        }
    }
    // A click: hidden when it is in front, else shown and brought forward.
    onActivated: reason => {
        if (reason === Platform.SystemTrayIcon.Trigger)
            tray.window.toggleShown()
    }
    // One of its messages clicked: what the last notice offered (Windows),
    // else the window (main.qml's `trayMessageClicked`).
    onMessageClicked: tray.window.trayMessageClicked()
}
