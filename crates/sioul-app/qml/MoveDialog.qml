// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Where messages go: every account with its folders, one button each, the
// folder they are in left out. Another account's folder takes a copy, and the
// original leaves its own once the copy is there.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: dialog

    required property var sioul
    required property var theme
    // The account and folder the messages are in, left out of the choices.
    property string fromAccount: ""
    property string fromFolder: ""
    property var accounts: []

    signal chosen(string account, string folder)

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(460, (parent ? parent.width : 460) - 2 * dialog.theme.gap)
    height: Math.min(implicitHeight, (parent ? parent.height : 600) - 2 * dialog.theme.gap)
    title: dialog.sioul.text("ui-move-title")
    onAboutToShow: dialog.accounts = dialog.sioul.mailAccounts ? JSON.parse(dialog.sioul.mailAccounts) : []

    contentItem: ScrollView {
        id: scroll

        contentWidth: availableWidth
        clip: true

        ColumnLayout {
            width: scroll.availableWidth
            spacing: 4

            Repeater {
                model: dialog.accounts

                delegate: ColumnLayout {
                    id: block

                    required property var modelData

                    Layout.fillWidth: true
                    spacing: 2

                    Label {
                        Layout.fillWidth: true
                        Layout.topMargin: 6
                        text: block.modelData.title
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                        color: dialog.theme.text
                    }
                    Repeater {
                        model: block.modelData.folders.concat(block.modelData.more).filter(f => block.modelData.id !== dialog.fromAccount || f.name !== dialog.fromFolder)

                        delegate: ItemDelegate {
                            id: choice

                            required property var modelData

                            Layout.fillWidth: true
                            leftPadding: 16
                            Accessible.name: choice.modelData.title
                            onClicked: {
                                dialog.close()
                                dialog.chosen(block.modelData.id, choice.modelData.name)
                            }

                            contentItem: RowLayout {
                                spacing: 8

                                Icon {
                                    iconName: choice.modelData.icon
                                    size: 16
                                }
                                // A folder's name, from the server: plain text.
                                Label {
                                    Layout.fillWidth: true
                                    text: choice.modelData.title
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    color: dialog.theme.text
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Sioul's own button, in your language: Qt's standard ones are not translated.
    footer: DialogButtonBox {
        Button {
            text: dialog.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: dialog.close()
    }
}
