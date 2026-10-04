// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

#include "textspacing.h"

#include <QTextBlock>
#include <QTextBlockFormat>
#include <QTextCursor>
#include <QTextDocument>

TextSpacing::TextSpacing(QObject *parent)
    : QObject(parent)
{
}

QQuickTextDocument *TextSpacing::document() const
{
    return m_document;
}

void TextSpacing::setDocument(QQuickTextDocument *document)
{
    if (m_document == document)
        return;
    if (m_document && m_document->textDocument())
        disconnect(m_document->textDocument(), nullptr, this, nullptr);
    m_document = document;
    if (m_document && m_document->textDocument())
        connect(m_document->textDocument(), &QTextDocument::contentsChanged, this, &TextSpacing::apply);
    apply();
    Q_EMIT documentChanged();
}

qreal TextSpacing::spacing() const
{
    return m_spacing;
}

void TextSpacing::setSpacing(qreal spacing)
{
    if (qFuzzyCompare(m_spacing, spacing))
        return;
    m_spacing = spacing;
    apply();
    Q_EMIT spacingChanged();
}

void TextSpacing::apply()
{
    if (m_applying || !m_document)
        return;
    QTextDocument *text = m_document->textDocument();
    if (!text)
        return;
    const qreal wanted = m_spacing * 100.0;
    m_applying = true;
    for (QTextBlock block = text->begin(); block.isValid(); block = block.next()) {
        const QTextBlockFormat current = block.blockFormat();
        if (current.lineHeightType() == QTextBlockFormat::ProportionalHeight && qFuzzyCompare(current.lineHeight(), wanted))
            continue;
        QTextBlockFormat format;
        format.setLineHeight(wanted, QTextBlockFormat::ProportionalHeight);
        QTextCursor cursor(block);
        cursor.mergeBlockFormat(format);
    }
    m_applying = false;
}
