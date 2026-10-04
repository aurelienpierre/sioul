// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's own icon on its windows: the taskbar, the window switcher, a title
// bar where there is one. Linux desktops find it through sioul.desktop (its
// Icon, matched by the desktop file name); Windows and macOS, and a window run
// from the build folder, from here. PNGs drawn for each size: no SVG plugin needed.

#include <QGuiApplication>
#include <QIcon>
#include <QString>

extern "C" void sioul_set_window_icon()
{
    QIcon icon;
    for (const int size : { 16, 22, 24, 32, 48, 64, 128, 256 })
        icon.addFile(QStringLiteral(":/sioul/icon/%1.png").arg(size), QSize(size, size));
    QGuiApplication::setWindowIcon(icon);
}
