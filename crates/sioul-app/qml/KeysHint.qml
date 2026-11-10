// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The keys, in the status line (StatusLine.qml), when nothing else needs the
// room: wherever a hardware keyboard is attached (`sioul.keyboard`): a
// computer, a tablet with its keyboard, Android saying so and each change
// followed (devicekind.rs). Never decided by the width, which a phone held
// sideways passes (the case of a phone seen on 16 October 2026), nor by
// touch, which a tablet with a keyboard has too.

import QtQuick
import QtQuick.Controls.Basic

Label {
    id: hint

    required property var sioul
    required property var theme
    // What the status line leaves for it: nothing else said, wide enough.
    property bool room: true

    visible: hint.room && hint.sioul.keyboard === true
    text: hint.sioul.text("ui-keys")
    elide: Text.ElideRight
    color: hint.theme.muted
    font.pixelSize: 12
}
