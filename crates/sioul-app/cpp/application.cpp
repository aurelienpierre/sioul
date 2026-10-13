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
//
// A press in any of Sioul's windows (a click, a key, a touch) tells Rust that
// a person is at this computer (devices.rs, `pressed`), five seconds apart at
// most: its entry among your devices says so, and a phone's background steps
// follow it. The press itself goes on as it came; nothing is taken from it.

#include <QApplication>
#include <QByteArray>
#include <QElapsedTimer>
#include <QEvent>
#include <QList>
#include <QObject>

#include <vector>

extern "C" void sioul_window_pressed();

namespace {
// Presses in Sioul's windows, watched for the whole application.
class Presses : public QObject
{
public:
    using QObject::QObject;

protected:
    bool eventFilter(QObject *watched, QEvent *event) override
    {
        switch (event->type()) {
        case QEvent::MouseButtonPress:
        case QEvent::KeyPress:
        case QEvent::TouchBegin:
        case QEvent::TabletPress:
            if (!said.isValid() || said.hasExpired(5000)) {
                said.start();
                sioul_window_pressed();
            }
            break;
        default:
            break;
        }
        return QObject::eventFilter(watched, event);
    }

private:
    QElapsedTimer said;
};

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
    application->installEventFilter(new Presses(application));
    return application;
}

// The token a second Sioul handed on with its links (outside.rs,
// `activate_with_token`): Qt's Wayland client gives XDG_ACTIVATION_TOKEN to
// the compositor at the next window's activation (requestActivate), then
// unsets it, so that the draft it opens comes to the front. qputenv takes Qt's
// own lock on the environment, which Qt reads under the same lock.
extern "C" void sioul_set_activation_token(const char *token)
{
    qputenv("XDG_ACTIVATION_TOKEN", QByteArray(token));
}
