// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A PDF of the notes folder, read in place: pages one under the other, its
// links followed, its text selectable. Loaded only when a PDF opens.

import QtQuick
import QtQuick.Pdf

PdfMultiPageView {
    id: view

    required property url source

    document: PdfDocument {
        source: view.source
    }
}
