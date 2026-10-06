// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// SIGTERM (the system ending the session, `kill`), SIGINT (Ctrl+C in the
// terminal Sioul was started from) and SIGHUP (that terminal closed) end
// Sioul as its window's close does: Qt's quit, on Qt's thread, the same as
// an X11 session manager's "Die". So what waits for "Undo" goes out, the
// sites' pages close as a browser closes its tabs (cpp/webengine.cpp), and Qt
// WebEngine writes what the sites keep before the program ends. Before, any of
// them ended Sioul on the spot, nothing written. A second signal ends it at
// once, and so does the end of thirty seconds (systemd waits 45 before its
// SIGKILL). A signal handler may only write: one byte into a pipe, which Qt's
// event loop reads. Nothing on Windows, which has none of these signals.

#include <QCoreApplication>
#include <QSocketNotifier>

#ifdef Q_OS_UNIX
#include <csignal>
#include <fcntl.h>
#include <unistd.h>

namespace {
int ends[2] = {-1, -1};

void heard(int)
{
    const char byte = 1;
    [[maybe_unused]] const auto written = ::write(ends[1], &byte, 1);
}
}

extern "C" void sioul_quit_on_signals()
{
    if (ends[0] >= 0 || ::pipe(ends) != 0)
        return;
    for (const int end : ends) {
        ::fcntl(end, F_SETFD, FD_CLOEXEC);
        ::fcntl(end, F_SETFL, ::fcntl(end, F_GETFL) | O_NONBLOCK);
    }
    auto *notifier = new QSocketNotifier(ends[0], QSocketNotifier::Read, QCoreApplication::instance());
    QObject::connect(notifier, &QSocketNotifier::activated, notifier, [notifier] {
        char bytes[16];
        while (::read(ends[0], bytes, sizeof bytes) > 0) {
        }
        notifier->setEnabled(false);
        std::signal(SIGTERM, SIG_DFL);
        std::signal(SIGINT, SIG_DFL);
        std::signal(SIGHUP, SIG_DFL);
        ::alarm(30);
        QCoreApplication::quit();
    });
    struct sigaction action = {};
    action.sa_handler = heard;
    sigemptyset(&action.sa_mask);
    action.sa_flags = SA_RESTART;
    for (const int signal : {SIGTERM, SIGINT, SIGHUP})
        ::sigaction(signal, &action, nullptr);
}
#else
extern "C" void sioul_quit_on_signals() { }
#endif
