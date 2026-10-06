// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul in the system tray, on a desktop (main.qml): a click shows the window
// or hides it, Sioul going on behind it (reminders, medicines, mail at its
// hours, the sharing); its menu shows or hides the window, or quits. Without
// a tray (`available` false: GNOME without its extension, a bare window
// manager), closing the window quits, as before. Kept out of qml/: the phone's
// build reads that folder for the modules to carry, and this one would bring
// Qt Widgets along for a tray Android has not (build.rs).

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
        Platform.MenuItem {
            text: tray.window.visible ? tray.sioul.text("tray-hide") : tray.sioul.text("tray-show")
            onTriggered: tray.window.toggleShown()
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
}
