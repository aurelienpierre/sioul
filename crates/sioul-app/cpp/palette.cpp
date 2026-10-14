// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Whether Sioul's windows draw dark colours when none are chosen in Sioul, as
// main.qml decides it (Theme.qml, `darkFrom`): with high contrast on, the
// system's own colours, the application's palette's window colour darker
// than mid-grey (SystemPalette.window.hslLightness < 0.5); else the system's
// colour scheme when it says one; else the palette again. Read as Sioul
// starts (desktop.rs, `icons`), so that the bundled icons are drawn for the
// colours the window then takes. The palette alone would not do: on Windows
// 10, Qt's Vista style draws a light palette in the system's dark mode, while
// the window follows the colour scheme, dark.

#include <QAccessibilityHints>
#include <QGuiApplication>
#include <QPalette>
#include <QStyleHints>

extern "C" bool sioul_window_dark()
{
    const QStyleHints *hints = QGuiApplication::styleHints();
    const bool highContrast = hints->accessibility()->contrastPreference() == Qt::ContrastPreference::HighContrast;
    if (!highContrast && hints->colorScheme() == Qt::ColorScheme::Dark)
        return true;
    if (!highContrast && hints->colorScheme() == Qt::ColorScheme::Light)
        return false;
    // QColor's lightness is its HSL lightness, what QML calls hslLightness.
    return QGuiApplication::palette().color(QPalette::Active, QPalette::Window).lightnessF() < 0.5;
}
