// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

#include "pdfwriter.h"

#include <QDir>
#include <QFileInfo>
#include <QMarginsF>
#include <QPageLayout>
#include <QPageSize>
#include <QPdfWriter>
#include <QTextDocument>

PdfWriter::PdfWriter(QObject *parent)
    : QObject(parent)
{
}

QString PdfWriter::write(const QString &html, const QString &path) const
{
    const QFileInfo file(path);
    if (!QDir().mkpath(file.absolutePath()))
        return file.absolutePath();
    {
        QPdfWriter writer(path);
        writer.setPageSize(QPageSize(QPageSize::A4));
        writer.setPageMargins(QMarginsF(18, 18, 18, 18), QPageLayout::Millimeter);
        writer.setResolution(300);
        writer.setTitle(file.completeBaseName());
        writer.setCreator(QStringLiteral("Sioul"));
        QTextDocument document;
        document.setHtml(html);
        document.print(&writer);
    }
    return QFileInfo::exists(path) && QFileInfo(path).size() > 0 ? QString() : path;
}
