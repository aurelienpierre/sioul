// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// colour: the sites in the screen's own colours, and calmer colours
// (docs/colour.md). ScreenColours, a QML singleton, finds each screen's
// profile (on X11, the root window's _ICC_PROFILE atoms, matched to screens
// through colord), has colour.rs make the two tables of each screen, and
// hands them to ColourEffect.qml as images ("image://sioul-colour/…"). It
// converts to the screen only where nothing else does: on X11, outside a
// Wayland session, without a colour-managing compositor. Calmer colours work
// everywhere Sites exists.

#pragma once

#include <QByteArray>
#include <QHash>
#include <QImage>
#include <QList>
#include <QMutex>
#include <QObject>
#include <QStringList>
#include <QTimer>
#include <QtQml/qqmlregistration.h>

#include <memory>

class QJSEngine;
class QQmlEngine;
class ProfileWatcher;

class ScreenColours : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    // Settings ▸ Display, "Colours for this screen": on unless turned off.
    Q_PROPERTY(bool screenConversion READ screenConversion WRITE setScreenConversion NOTIFY changed)
    // "Calmer colours on sites": 0 off, 1 a little, 2 more.
    Q_PROPERTY(int calmer READ calmer WRITE setCalmer NOTIFY changed)
    // Grows each time the tables change: what the effects bind to.
    Q_PROPERTY(int revision READ revision NOTIFY changed)
    // Entries of the cube on each side (colour.rs, CUBE).
    Q_PROPERTY(int cubeSize READ cubeSize CONSTANT)

public:
    // Made by the QML engine, once; registers the tables' image provider.
    static ScreenColours *create(QQmlEngine *engine, QJSEngine *);
    ~ScreenColours() override;

    bool screenConversion() const;
    void setScreenConversion(bool on);
    int calmer() const;
    void setCalmer(int calmer);
    int revision() const;
    static int cubeSize();

    // Whether the tables change anything on this screen (its QScreen name; "": the primary).
    Q_INVOKABLE bool active(const QString &screen) const;
    // The address of one table ("cube" or "curve") for this screen.
    Q_INVOKABLE QString source(const QString &which, const QString &screen) const;

    // The table an address names ("cube/<screen number>/<revision>"), for the image provider.
    QImage image(const QString &id) const;

    // X11: whether a native event says a profile atom changed on the root window.
    bool profileChanged(const QByteArray &eventType, const void *message) const;
    // Everything read and made again, a moment after the last change.
    void later();

Q_SIGNALS:
    void changed();

private:
    explicit ScreenColours(QObject *parent = nullptr);
    void refresh();
    // The profile of each screen, by name; empty for none. What converts: 0
    // Sioul (X11), 1 the desktop, 2 nothing.
    QHash<QString, QByteArray> profiles(int *who);

    struct Tables {
        bool identity = true;
        QImage cube;
        QImage curve;
    };

    bool m_screenConversion = true;
    int m_calmer = 0;
    int m_revision = 0;
    QTimer m_later;
    // The screens' names, by number in the tables' addresses.
    QStringList m_screens;
    QHash<QString, Tables> m_tables;
    mutable QMutex m_mutex;
    // X11: the root window and the atoms watched, and what watches them
    // (a native event filter of its own, so that QML sees a plain QObject).
    quint32 m_root = 0;
    QList<quint32> m_atoms;
    std::unique_ptr<ProfileWatcher> m_watcher;
};
