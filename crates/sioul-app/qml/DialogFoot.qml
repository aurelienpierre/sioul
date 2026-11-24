// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The one foot of a dialog (docs/qt-quick.md, "The look of each kind of
// thing", rule 15): what removes or sets aside at the left (Remove, Not
// today), written inside it; Cancel, then the main action, at the right, in
// that order on every system (Qt's DialogButtonBox orders them as each
// system does, and the dialogs changed order from one to the next). The main
// action is the one button filled with the accent, and its words say what it
// does (Save, Send, Add a calendar), never "OK". Too narrow for one line (a
// phone), what is at the left goes on a line above, at the left still.
//
//     footer: DialogFoot {
//         sioul: form.sioul
//         theme: form.theme
//         mainText: form.sioul.text("ui-save")
//         onAccepted: form.save()      // the page closes the dialog when it is done
//         onRejected: form.close()
//
//         Button { text: form.sioul.text("health-remove"); onClicked: … }
//     }
//
// Unlike a DialogButtonBox given as the footer, it never closes the dialog by
// itself: a save that fails keeps the form, and says why, open.

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Pane {
    id: foot

    // Sioul's backend (backend.rs): its words in your language.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The main action's words: what it does. None: no main action (a dialog that only informs).
    property string mainText: ""
    // The main action can be taken now.
    property bool mainEnabled: true
    // Cancel's words: "Cancel" unless a dialog has better ones ("Close", once nothing is left to cancel).
    property string cancelText: foot.sioul.text("ui-cancel")
    // A Cancel at all.
    property bool cancels: true
    // What removes or sets aside, at the left.
    default property alias aside: asideRow.data
    // The two at the right, for a page's tests and pictures.
    readonly property alias mainButton: mainButton
    readonly property alias cancelButton: cancelButton
    // Too narrow for one line: what is at the left goes above.
    readonly property bool stacked: asideRow.implicitWidth > 0 && asideRow.implicitWidth + foot.theme.gap + rightRow.implicitWidth > foot.availableWidth

    // The main action.
    signal accepted
    // Cancel.
    signal rejected

    padding: foot.theme.cardPadding
    topPadding: foot.theme.rowGap

    background: Item {}

    contentItem: Item {
        implicitWidth: asideRow.implicitWidth + (asideRow.implicitWidth > 0 ? foot.theme.gap : 0) + rightRow.implicitWidth
        implicitHeight: foot.stacked ? asideRow.implicitHeight + foot.theme.rowGap + rightRow.implicitHeight : Math.max(asideRow.implicitHeight, rightRow.implicitHeight)

        Row {
            id: asideRow

            objectName: "dialogAside"
            x: 0
            y: foot.stacked ? 0 : Math.round((parent.height - height) / 2)
            spacing: foot.theme.labelGap
        }
        Row {
            id: rightRow

            x: parent.width - width
            y: foot.stacked ? asideRow.height + foot.theme.rowGap : Math.round((parent.height - height) / 2)
            spacing: foot.theme.labelGap
            layoutDirection: Qt.LeftToRight

            Button {
                id: cancelButton

                objectName: "dialogCancel"
                visible: foot.cancels
                text: foot.theme.plain(foot.cancelText)
                onClicked: foot.rejected()
            }
            Button {
                id: mainButton

                objectName: "dialogMain"
                visible: foot.mainText !== ""
                enabled: foot.mainEnabled
                highlighted: true
                text: foot.theme.plain(foot.mainText)
                onClicked: foot.accepted()
            }
        }
    }
}
