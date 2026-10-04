// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Text and images, made ready as the window starts. Sioul Symbols comes first
// in line after the text's own font for the few symbols Sioul writes (→ ▸ ▾ ✓),
// where Qt would otherwise look through the system's fonts for each
// (tools/make-symbols-font.py). Then, on a thread of its own, what Qt reads
// once before it draws text and images, while the QML engine reads the
// window's files, which the window waits for anyway: the list of the system's
// fonts (on a phone, a tenth of the start), and the image formats it can
// read, which Qt Quick asks for at the first icon, opening every image plugin
// of the system (KDE's, libheif's: 50 ms on a desktop).

#include <QChar>
#include <QFontDatabase>
#include <QImageReader>
#include <QString>

#include <thread>

extern "C" void sioul_warm_up()
{
    // Before the list of fonts is read: a font added afterwards has Qt read it all again.
    if (QFontDatabase::addApplicationFont(QStringLiteral(":/sioul/fonts/SioulSymbols.ttf")) != -1)
        QFontDatabase::addApplicationFallbackFontFamily(QChar::Script_Common, QStringLiteral("Sioul Symbols"));
    std::thread([] {
        QFontDatabase::families();
        QImageReader::supportedImageFormats();
    }).detach();
}
