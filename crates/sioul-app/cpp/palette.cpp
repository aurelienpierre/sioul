// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Whether Sioul's windows draw dark colours when none are chosen in Sioul:
// the application's palette, its window colour darker than mid-grey, as
// main.qml reads it (SystemPalette.window.hslLightness < 0.5). Read as Sioul
// starts (desktop.rs, `icons`), so that the bundled icons are drawn for the
// colours the window then takes. The system's own setting would not do: on
// Windows 10, Qt's style draws light in the system's dark mode, and so does
// the window.

#include <QGuiApplication>
#include <QPalette>

extern "C" bool sioul_window_dark()
{
    // QColor's lightness is its HSL lightness, what QML calls hslLightness.
    return QGuiApplication::palette().color(QPalette::Active, QPalette::Window).lightnessF() < 0.5;
}
