// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Line spacing for editable text. Qt Quick gives a TextArea no line height,
// and tight lines make a block of text a wall: each paragraph of its document
// gets the height asked here, again when text is loaded. A new line typed
// inherits its paragraph's spacing.

#pragma once

#include <QObject>
#include <QPointer>
#include <QQuickTextDocument>
#include <QtQml/qqmlregistration.h>

class TextSpacing : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QQuickTextDocument *document READ document WRITE setDocument NOTIFY documentChanged)
    Q_PROPERTY(double spacing READ spacing WRITE setSpacing NOTIFY spacingChanged)

public:
    explicit TextSpacing(QObject *parent = nullptr);

    QQuickTextDocument *document() const;
    void setDocument(QQuickTextDocument *document);
    qreal spacing() const;
    void setSpacing(qreal spacing);

Q_SIGNALS:
    void documentChanged();
    void spacingChanged();

private:
    // Every paragraph without the asked height gets it; the others are left alone.
    void apply();

    QPointer<QQuickTextDocument> m_document;
    qreal m_spacing = 1.5;
    bool m_applying = false;
};
