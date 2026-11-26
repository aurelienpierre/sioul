// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A PDF of the notes folder, on Android: Qt PDF has no Android version, so
// nothing is shown here; the page's "Open with…" hands the PDF to the reader
// Android has for it. Same name and same properties as qml/PdfView.qml, every
// one of them: QML refuses a whole page that gives this view a property it
// lacks, and on the phone alone (from 3 to 26 November 2026 the Notes page
// gave it `sioul`, `theme` and `window`, which it did not declare, and the
// phone's Notes page showed nothing at all). tst_pdf.qml holds both files to
// the same properties; tst_androidset.qml makes the pages from the phone's
// own set of files.

import QtQuick

Item {
    required property url source
    // Given by the pages as to qml/PdfView.qml, which uses them; unused here.
    property var sioul: null
    property var theme: null
    property var window: null
    property var openExternally: url => Qt.openUrlExternally(url)
    property real zoom: 1
    property var asked: null
    // No document nor pages here: nothing is drawn.
    readonly property var document: null
    readonly property var pages: null
}
