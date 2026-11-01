// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A popup drawn over the whole window (Overlay.overlay), kept inside its
// safe area: never under Android's navigation buttons or its status bar,
// nor a notch, whichever side they are on (a phone held sideways has its
// buttons beside), the usual small gap within it (review of 4c77d0c, L9).
// The overlay covers the system bars too; Qt pushes a popup inside its
// margins and shrinks it when it is taller or wider (QQuickPopupPositioner),
// so a long form keeps its buttons in view and scrolls. Every dialog over
// the window is one of these, or a SafeDialog: a check in Rust holds it
// (tests/safe_area.rs).

import QtQuick
import QtQuick.Controls.Basic

Popup {
    id: safe

    // The window's safe area, as the overlay knows it (Qt 6.9 and later).
    readonly property var safeArea: safe.parent ? safe.parent.SafeArea.margins : null
    // Kept between the dialog and the window's edges, inside that area.
    property real safeGap: 8
    // The window less its safe area: what a dialog's own width and height
    // are kept within (Qt pushes a popup inside its margins, but shrinks only
    // a size it did not set).
    readonly property real roomWidth: safe.parent ? safe.parent.width - (safe.safeArea ? safe.safeArea.left + safe.safeArea.right : 0) : 800
    readonly property real roomHeight: safe.parent ? safe.parent.height - (safe.safeArea ? safe.safeArea.top + safe.safeArea.bottom : 0) : 800

    topMargin: (safe.safeArea ? safe.safeArea.top : 0) + safe.safeGap
    bottomMargin: (safe.safeArea ? safe.safeArea.bottom : 0) + safe.safeGap
    leftMargin: (safe.safeArea ? safe.safeArea.left : 0) + safe.safeGap
    rightMargin: (safe.safeArea ? safe.safeArea.right : 0) + safe.safeGap
}
