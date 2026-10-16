// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The keys, in the status line (StatusLine.qml), when nothing else needs the
// room: on a computer only. A phone has no keyboard to name, held upright or
// sideways: decided by the platform (`theme.touch`, a phone or the runner's
// phone size), never by the width, which a phone held sideways passes (the
// case of a phone seen on 16 October 2026).

import QtQuick
import QtQuick.Controls.Basic

Label {
    id: hint

    required property var sioul
    required property var theme
    // What the status line leaves for it: nothing else said, wide enough.
    property bool room: true

    visible: hint.room && hint.theme.touch !== true
    text: hint.sioul.text("ui-keys")
    elide: Text.ElideRight
    color: hint.theme.muted
    font.pixelSize: 12
}
