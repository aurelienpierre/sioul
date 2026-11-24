// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's overrides of the Basic style (docs/qt-quick.md, "Sioul's overrides
// of the Basic style"). Wherever the QML engine would load the Basic style's
// own Button.qml, ToolButton.qml, TextField.qml, TextArea.qml, CheckBox.qml,
// RadioButton.qml or SpinBox.qml, it loads Sioul's from qml/style/ instead
// (StyleButton.qml…): a button that holds words is outlined, an icon alone
// bare, the main action filled with the accent, a selected one framed in it;
// what you type in or tick has its own fill and a firmer border, in every window and dialog of Sioul's,
// Qt's own included. Each is written as Qt 6.9 and 6.10 write Basic's, which
// Qt 6.11 loads too: a property newer than the Qt that runs stops the file,
// and with it every window (the Flatpak runs Qt 6.10). Every page imports QtQuick.Controls.Basic, chosen as the program
// is built; a style of Sioul's own would need each page to import it, and
// every new page to remember. Given to the engine once, before the window's
// first file is read.

#include <QQmlAbstractUrlInterceptor>
#include <QQmlApplicationEngine>
#include <QString>
#include <QStringList>
#include <QStringView>
#include <QUrl>

namespace {

class StyleOverrides final : public QQmlAbstractUrlInterceptor
{
public:
    QUrl intercept(const QUrl &url, DataType type) override
    {
        if (type != QmlFile)
            return url;
        // From Qt's resources (qrc:/qt-project.org/imports/QtQuick/Controls/Basic/…)
        // or its folder of QML modules (…/qml/QtQuick/Controls/Basic/…).
        static const QString basic = QStringLiteral("/QtQuick/Controls/Basic/");
        const QString path = url.path();
        const qsizetype at = path.lastIndexOf(basic);
        if (at < 0)
            return url;
        const QStringView name = QStringView(path).mid(at + basic.size());
        static const QStringList overridden = { QStringLiteral("Button.qml"), QStringLiteral("ToolButton.qml"), QStringLiteral("TextField.qml"), QStringLiteral("TextArea.qml"),
                                                QStringLiteral("CheckBox.qml"), QStringLiteral("RadioButton.qml"), QStringLiteral("SpinBox.qml") };
        if (!overridden.contains(name))
            return url;
        return QUrl(QStringLiteral("qrc:/qt/qml/com/aurelienpierre/sioul/qml/style/Style") + name.toString());
    }
};

} // namespace

extern "C" void sioul_style_overrides(QQmlApplicationEngine *engine)
{
    // The engine does not own its interceptors: this one lives as long as the program.
    static StyleOverrides overrides;
    if (engine)
        engine->addUrlInterceptor(&overrides);
}
