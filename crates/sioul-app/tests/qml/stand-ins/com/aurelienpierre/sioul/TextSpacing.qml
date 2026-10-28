// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The window's TextSpacing (cpp/textspacing.h), stood in for the QML tests:
// its two properties, and nothing done with them. With it, a test loads a
// file that imports the window's module for that type alone (TaskPanel.qml).

import QtQml

QtObject {
    property var document: null
    property real spacing: 1
}
