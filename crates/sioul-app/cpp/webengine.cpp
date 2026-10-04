// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The web engine of the Sites page (Chromium, through Qt WebEngine) must be
// started before the application is made.

#include <QCoreApplication>
#include <QtWebEngineQuick/qtwebenginequickglobal.h>

extern "C" void sioul_start_web_engine()
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    QtWebEngineQuick::initialize();
}
