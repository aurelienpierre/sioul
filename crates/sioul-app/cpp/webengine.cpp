// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The web engine of the Sites page (Chromium, through Qt WebEngine) draws with
// what Qt Quick draws with, chosen before the application is made: OpenGL
// unless the system's own is set (Vulkan, Metal, Direct3D). That is all
// QtWebEngineQuick::initialize() does in Qt 6.11, done here without linking
// Qt WebEngine: its libraries (Chromium's, a thousand files) are loaded when
// the Sites page imports it, not each time Sioul starts.

#include <QCoreApplication>
#include <QQuickWindow>
#include <QSGRendererInterface>

extern "C" void sioul_start_web_engine()
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    const auto api = QQuickWindow::graphicsApi();
    if (api != QSGRendererInterface::OpenGL && api != QSGRendererInterface::Vulkan && api != QSGRendererInterface::Metal
        && api != QSGRendererInterface::Direct3D11)
        QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
}
