// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's overrides of the Basic style (docs/qt-quick.md, "Sioul's overrides
// of the Basic style"). Wherever the QML engine would load the Basic style's
// own Button.qml, TextField.qml or TextArea.qml, it loads Sioul's from
// qml/style/ instead: a flat button keeps an outline, a field its own fill
// and a firmer border, in every window and dialog of Sioul's, Qt's own
// included. Every page imports QtQuick.Controls.Basic, chosen as the program
// is built; a style of Sioul's own would need each page to import it, and
// every new page to remember. Given to the engine once, before the window's
// first file is read.

#include <QQmlAbstractUrlInterceptor>
#include <QQmlApplicationEngine>
#include <QString>
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
        if (name == u"Button.qml")
            return QUrl(QStringLiteral("qrc:/qt/qml/com/aurelienpierre/sioul/qml/style/StyleButton.qml"));
        if (name == u"TextField.qml")
            return QUrl(QStringLiteral("qrc:/qt/qml/com/aurelienpierre/sioul/qml/style/StyleTextField.qml"));
        if (name == u"TextArea.qml")
            return QUrl(QStringLiteral("qrc:/qt/qml/com/aurelienpierre/sioul/qml/style/StyleTextArea.qml"));
        return url;
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
