// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Text put on the system's clipboard, for Copy buttons (backend.rs,
// `copy_text`): Qt Quick has no clipboard of its own. Read back after, so
// that the window says "Copied." only when the clipboard holds it.

#include <QClipboard>
#include <QGuiApplication>
#include <QString>

#include <cstddef>

extern "C" bool sioul_copy_text(const char *text, std::size_t length)
{
    QClipboard *clipboard = QGuiApplication::clipboard();
    if (clipboard == nullptr)
        return false;
    const QString wanted = QString::fromUtf8(text, static_cast<qsizetype>(length));
    clipboard->setText(wanted);
    return clipboard->text() == wanted;
}
