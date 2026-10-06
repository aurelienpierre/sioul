// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// On a computer, Sioul is a widgets application (QApplication): the system
// tray's menu is made of widgets on Plasma (its platform theme's QMenu), and
// Qt.labs.platform draws its tray with widgets where the system has no tray of
// its own; without one, Plasma's tray aborted Sioul at start ("QWidget: Cannot
// create a QWidget without QApplication"). A phone has no tray, and Android's
// build carries no Qt Widgets: there, CXX-Qt's QGuiApplication (lib.rs).
//
// The program's arguments are copied and kept with the application, which
// keeps a reference to their count and their pointers as long as it lives, as
// CXX-Qt keeps its own.
//
// Sioul goes on while windows other than its own close: a draft, the focus
// window, the routine player, its window hidden in the tray. Its window's
// closing quits it (main.qml's onClosing), not the last window's.

#include <QApplication>
#include <QByteArray>
#include <QList>
#include <QObject>

#include <vector>

namespace {
// The arguments, alive with the application, which may take some out (Qt's own).
class Arguments : public QObject
{
public:
    Arguments(int count, const char *const *values)
    {
        for (int i = 0; i < count; ++i)
            owned.append(QByteArray(values[i]));
        for (QByteArray &value : owned)
            pointers.push_back(value.data());
        pointers.push_back(nullptr);
        size = count;
    }

    int size = 0;
    QList<QByteArray> owned;
    std::vector<char *> pointers;
};
}

extern "C" QGuiApplication *sioul_new_application(int count, const char *const *values)
{
    auto *arguments = new Arguments(count, values);
    auto *application = new QApplication(arguments->size, arguments->pointers.data());
    arguments->setParent(application);
    QGuiApplication::setQuitOnLastWindowClosed(false);
    return application;
}
