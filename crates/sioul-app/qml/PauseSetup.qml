// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The pauses' setup, under their settings (Settings ▸ Pauses, docs/pauses.md):
// what this device's do-not-disturb can do, said plainly, with the system's
// pages it needs (access, starred contacts, Plasma's notifications) and, on
// GNOME, the consent for Sioul to switch its Do Not Disturb; a try-out of
// the pause's screen, which holds nothing; the last pause forgotten.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: setup

    required property var sioul
    required property var theme
    // {on, line, consent, offers: [{key, label}], gnome, forgettable}, read when shown.
    property var report: ({ on: false, line: "", consent: "", offers: [], gnome: false, forgettable: false })

    // The pause's screen as it will be, nothing held.
    signal tried

    function reload() {
        setup.report = JSON.parse(setup.sioul.pauseSetup() || "null") || setup.report
    }

    spacing: 6
    Component.onCompleted: setup.reload()
    onVisibleChanged: if (visible) setup.reload()

    Label {
        Layout.fillWidth: true
        Layout.topMargin: 18
        text: setup.sioul.text("pause-setup-dnd")
        font.pixelSize: 17
        font.weight: Font.DemiBold
        elide: Text.ElideRight
        color: setup.theme.accent
    }
    Label {
        visible: setup.report.line !== ""
        Layout.fillWidth: true
        text: setup.report.line
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.text
    }
    // The system's own pages it needs: each a button.
    Flow {
        Layout.fillWidth: true
        spacing: 6

        Repeater {
            model: setup.report.offers.filter(o => o.key !== "desktop")

            delegate: Button {
                id: offer

                required property var modelData

                width: Math.min(implicitWidth, parent ? parent.width : implicitWidth)
                text: offer.modelData.label
                onClicked: {
                    setup.sioul.openDnd(offer.modelData.key)
                    setup.reload()
                }
            }
        }
    }
    // GNOME: Sioul switches its Do Not Disturb only with this yes, and back after.
    WrapCheckBox {
        visible: setup.report.offers.some(o => o.key === "desktop")
        Layout.fillWidth: true
        text: setup.report.consent
        checked: setup.report.gnome
        onToggled: {
            setup.sioul.setSetting("pause.gnome", JSON.stringify(checked))
            setup.reload()
        }
    }

    // The screen, tried on a calm day.
    Button {
        Layout.topMargin: 12
        text: setup.sioul.text("pause-setup-try")
        onClicked: setup.tried()
    }
    Label {
        Layout.fillWidth: true
        text: setup.sioul.text("pause-setup-try-help")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }

    // What Sioul keeps of the last pause, and forgets.
    Button {
        visible: setup.report.forgettable
        Layout.topMargin: 12
        flat: true
        text: setup.sioul.text("pause-setup-forget")
        onClicked: {
            setup.sioul.forgetPause()
            setup.reload()
        }
    }
    Label {
        visible: setup.report.forgettable
        Layout.fillWidth: true
        Layout.bottomMargin: 24
        text: setup.sioul.text("pause-setup-forget-help")
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: setup.theme.muted
    }
}
