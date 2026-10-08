// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The web engine of the Sites page (Chromium, through Qt WebEngine) draws with
// what Qt Quick draws with, chosen before the application is made: OpenGL
// unless the system's own is set (Vulkan, Metal, Direct3D). That is all
// QtWebEngineQuick::initialize() does in Qt 6.11, done here without linking
// Qt WebEngine: its libraries (Chromium's, a thousand files) are loaded when
// the Sites page imports it, not each time Sioul starts. As Sioul quits, the
// sites' pages are closed as a browser closes its tabs (sioul_close_sites).

#include <QColorSpace>
#include <QCoreApplication>
#include <QEventLoop>
#include <QJSValue>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QSGRendererInterface>
#include <QSurfaceFormat>
#include <QTimer>

extern "C" void sioul_start_web_engine()
{
    QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
    const auto api = QQuickWindow::graphicsApi();
    if (api != QSGRendererInterface::OpenGL && api != QSGRendererInterface::Vulkan && api != QSGRendererInterface::Metal
        && api != QSGRendererInterface::Direct3D11)
        QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
#ifdef Q_OS_MACOS
    // colour: Sioul's windows say that their numbers are sRGB, so that macOS
    // converts them to the screen, sites included; without it, Qt hands them
    // to the screen as they are, too saturated on a wide-gamut one
    // (docs/colour.md). Untested: no Mac was at hand.
    QSurfaceFormat format = QSurfaceFormat::defaultFormat();
    format.setColorSpace(QColorSpace::SRgb);
    QSurfaceFormat::setDefaultFormat(format);
#endif
}

// Sioul has quit, its windows gone: each site's page is closed as a browser
// closes its tabs, before Qt WebEngine shuts down with the application. The
// page's beforeunload, pagehide and unload run; some sites keep a login only
// in the open page and write it back to their storage then (Discord's token):
// destroyed without them, the page loses it. SitesPage.qml's closeAll()
// closes them and says how many are still closing; this waits for its
// sitesClosed(), two seconds at most (Chromium gives a page that does not
// answer half a second for each step). Nothing to do when the Sites page was
// never made. docs/sites.md, "Closing".
extern "C" void sioul_close_sites(QQmlApplicationEngine *engine)
{
    const auto roots = engine ? engine->rootObjects() : QList<QObject *>();
    // main.qml's `sitesPage`, null until the page is made.
    const QVariant page = roots.isEmpty() ? QVariant() : roots.first()->property("sitesPage");
    QObject *sites = page.metaType() == QMetaType::fromType<QJSValue>() ? page.value<QJSValue>().toQObject() : page.value<QObject *>();
    if (!sites)
        return;
    QEventLoop loop;
    QObject::connect(sites, SIGNAL(sitesClosed()), &loop, SLOT(quit()));
    QVariant closing;
    if (!QMetaObject::invokeMethod(sites, "closeAll", Q_RETURN_ARG(QVariant, closing)) || closing.toInt() <= 0)
        return;
    QTimer::singleShot(2000, &loop, &QEventLoop::quit);
    loop.exec();
}
