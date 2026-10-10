// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A check box whose sentence wraps. In a layout, an item that does not fill
// the width keeps its own width, and the layout is then at least that wide:
// one long sentence (in French, on a phone) laid a whole settings panel wider
// than the screen. Give it Layout.fillWidth (or a width) where it sits.

import QtQuick
import QtQuick.Controls.Basic

CheckBox {
    id: box

    contentItem: Text {
        leftPadding: box.indicator && !box.mirrored ? box.indicator.width + box.spacing : 0
        rightPadding: box.indicator && box.mirrored ? box.indicator.width + box.spacing : 0
        text: box.text
        textFormat: Text.PlainText
        font: box.font
        color: box.palette.windowText
        wrapMode: Text.Wrap
        verticalAlignment: Text.AlignVCenter
    }
}
