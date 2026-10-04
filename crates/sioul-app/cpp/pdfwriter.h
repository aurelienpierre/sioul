// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Prints a page of HTML into a PDF file, A4, as an invoice is sent.

#pragma once

#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

class PdfWriter : public QObject
{
    Q_OBJECT
    QML_ELEMENT

public:
    explicit PdfWriter(QObject *parent = nullptr);

    // Writes `html` into `path`, its folder made when missing; returns the
    // file or folder that could not be written, else "".
    Q_INVOKABLE QString write(const QString &html, const QString &path) const;
};
