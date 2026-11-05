// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Stands in for the window's PdfWriter (cpp/pdfwriter.cpp) in the QML
// tests: it writes nothing, and says each page it was given.

import QtQuick

QtObject {
    property var written: []

    function write(html, path) {
        written = written.concat([path])
        return ""
    }
}
