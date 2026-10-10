// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Which of the vault's logins to take, found by two fields, each optional,
// both matching when both are filled: Site (the logins' sites, else their
// names; never their user names) and User name (the user name only).
// Opened for a site, Site holds its domain, or that of the login chosen
// there last; for a mail account, User name holds the name it logs in with
// (its address, unless it names another). Names, user names and sites only
// here: a password leaves the vault for the one login chosen. A login made
// for another domain than the page's says which.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Dialog {
    id: chooser

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // The address the logins are for: a site's page, or a mail account's server.
    property string url: ""
    // A login made for another domain than the page's said in the warm colour,
    // so a look-alike site shows; not for a mail account, whose server is
    // seldom where its provider's login is kept (imap.gmail.com, accounts.google.com).
    property bool warnElsewhere: true
    // What the last search found, and what it asked.
    property var shown: ({ found: [], more: 0, error: "", asked: false, both: false })
    // A large vault is searched off the window's thread: the search waited
    // for (its ticket, else 0, and what it asked), and whether Enter came first.
    property int waiting: 0
    // What the fields held when the search waited for: an answer for older fields is
    // not shown.
    property var waitingFor: ({ site: "", user: "" })
    // Enter was pressed while the search waited: its first login is taken once found.
    property bool takeWhenFound: false
    // For the window's tests: what the fields hold.
    property alias site: siteField.text
    // For the window's tests: what the User name field holds.
    property alias user: userField.text
    // What taking a login does: "Fill" in a site, "Use" for an account's password.
    property string takeText: chooser.sioul.text("bitwarden-choose-fill")

    // A login taken, by its id in the vault.
    signal chosen(string item)

    // Opened for a site's page: Site holds its domain, or the domain of the
    // login chosen there last (one made for another site: a sign-in with Google).
    function begin(url) {
        const first = chooser.first(url)
        chooser.start(url, first.chosen || first.domain || "", "")
    }

    // Opened for a mail account: User name holds the name it logs in with
    // (its address, unless the account names another: AccountPassword.qml);
    // Site the domain of the login chosen for it last, else nothing: a mail
    // server's domain is seldom where its provider's login is kept
    // (imap.gmail.com and accounts.google.com, mail.ecloud.global and
    // murena.io), and with both fields matching it would hide the very login
    // looked for. The server's own logins still come first.
    function beginForAccount(server, login) {
        chooser.start(server, chooser.first(server).chosen || "", login || "")
    }

    // What the vault suggests for an address: its domain, and the login chosen there
    // last.
    function first(url) {
        const first = JSON.parse(chooser.sioul.bitwardenSite(url))
        return first.error ? { domain: "", chosen: "" } : first
    }

    // Opened for `url`, the fields holding `site` and `user`, searched at once.
    function start(url, site, user) {
        chooser.url = url
        chooser.takeWhenFound = false
        chooser.shown = { found: [], more: 0, error: "", asked: true, both: false }
        siteField.text = site
        userField.text = user
        chooser.search()
        chooser.open()
        siteField.forceActiveFocus()
        siteField.selectAll()
    }

    // The search, for what the fields hold now: answered at once, or for a
    // large vault later (`bitwardenFound`).
    function search() {
        typing.stop()
        const site = siteField.text.trim()
        const user = userField.text.trim()
        const answer = JSON.parse(chooser.sioul.bitwardenSearch(chooser.url, site, user))
        if (answer.later !== undefined) {
            chooser.waiting = answer.later
            chooser.waitingFor = { site: site, user: user }
            return
        }
        chooser.waiting = 0
        chooser.show(answer, site, user)
    }

    // A search's answer shown: the logins found, how many more, or why none.
    function show(answer, site, user) {
        chooser.shown = { found: answer.found || [], more: answer.more || 0, error: answer.error || "", asked: site !== "" || user !== "", both: site !== "" && user !== "" }
        list.currentIndex = 0
    }

    // The login at `index` taken: the chooser closes and says which (`chosen`).
    function take(index) {
        const choice = chooser.shown.found[index]
        if (!choice)
            return
        chooser.close()
        chooser.chosen(choice.id)
    }

    // Enter: the login in the list's highlight, the first unless moved; what
    // was typed last searched first, should the pause not have come yet, and
    // its first login taken once found.
    function takeCurrent() {
        if (typing.running)
            chooser.search()
        if (chooser.waiting !== 0)
            chooser.takeWhenFound = true
        else
            chooser.take(list.currentIndex)
    }

    // The highlight moved by `by` logins, within the list.
    function move(by) {
        list.currentIndex = Math.max(0, Math.min(list.count - 1, list.currentIndex + by))
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    modal: true
    width: Math.min(560, (parent ? parent.width : 560) - 2 * chooser.theme.gap)
    title: chooser.sioul.text("bitwarden-choose-title")
    // Closed before a large vault answered: nothing taken when it does.
    onClosed: {
        chooser.waiting = 0
        chooser.takeWhenFound = false
    }

    // Searched as you type, once typing pauses.
    Timer {
        id: typing

        interval: 150
        onTriggered: chooser.search()
    }

    // A large vault's answer: only the one waited for, an older one dropped.
    Connections {
        target: chooser.sioul

        function onBitwardenFound(ticket, found) {
            if (ticket !== chooser.waiting)
                return
            chooser.waiting = 0
            chooser.show(JSON.parse(found), chooser.waitingFor.site, chooser.waitingFor.user)
            if (chooser.takeWhenFound) {
                chooser.takeWhenFound = false
                chooser.take(0)
            }
        }
    }

    // A field with a button at its end that empties it, as PasswordField's eye
    // sits in its field; not a Tab stop: Tab goes from one field to the other.
    component EmptiableField: TextField {
        id: input

        rightPadding: empty.visible ? empty.width + 6 : input.leftPadding
        inputMethodHints: Qt.ImhNoPredictiveText | Qt.ImhNoAutoUppercase
        onTextChanged: typing.restart()
        onAccepted: chooser.takeCurrent()
        Keys.onDownPressed: chooser.move(1)
        Keys.onUpPressed: chooser.move(-1)

        Button {
            id: empty

            anchors.right: parent.right
            anchors.rightMargin: 2
            anchors.verticalCenter: parent.verticalCenter
            visible: input.text !== ""
            width: height
            height: parent.height - 6
            flat: true
            padding: 4
            focusPolicy: Qt.NoFocus
            icon.name: "edit-clear"
            icon.color: chooser.theme.text
            display: AbstractButton.IconOnly
            text: chooser.sioul.text("bitwarden-choose-empty")
            Accessible.name: empty.text
            ToolTip.visible: empty.hovered
            ToolTip.text: chooser.theme.plain(empty.text)
            ToolTip.delay: 400
            onClicked: {
                input.clear()
                input.forceActiveFocus()
            }
        }
    }

    contentItem: ColumnLayout {
        spacing: 8

        // Side by side, one Tab apart; one above the other on a phone.
        GridLayout {
            Layout.fillWidth: true
            columns: chooser.availableWidth >= 400 ? 2 : 1
            columnSpacing: 8
            rowSpacing: 6

            ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredWidth: 100
                spacing: 2

                Label {
                    Layout.fillWidth: true
                    text: chooser.sioul.text("bitwarden-choose-by-site")
                    font.pixelSize: 12
                    elide: Text.ElideRight
                    color: chooser.theme.muted
                }
                EmptiableField {
                    id: siteField

                    Layout.fillWidth: true
                    placeholderText: chooser.sioul.text("bitwarden-choose-by-site-hint")
                    Accessible.name: chooser.sioul.text("bitwarden-choose-by-site")
                    KeyNavigation.tab: userField
                    KeyNavigation.backtab: userField
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.preferredWidth: 100
                spacing: 2

                Label {
                    Layout.fillWidth: true
                    text: chooser.sioul.text("bitwarden-choose-by-user")
                    font.pixelSize: 12
                    elide: Text.ElideRight
                    color: chooser.theme.muted
                }
                EmptiableField {
                    id: userField

                    Layout.fillWidth: true
                    placeholderText: chooser.sioul.text("bitwarden-choose-by-user-hint")
                    Accessible.name: chooser.sioul.text("bitwarden-choose-by-user")
                    KeyNavigation.tab: siteField
                    KeyNavigation.backtab: siteField
                }
            }
        }
        // Nothing asked, nothing found, the vault locked; or how many more the list leaves out.
        Label {
            visible: text !== ""
            Layout.fillWidth: true
            text: chooser.waiting !== 0 && chooser.shown.found.length === 0 ? ""
                  : chooser.shown.error !== "" ? chooser.shown.error
                  : !chooser.shown.asked ? chooser.sioul.text("bitwarden-choose-ask")
                  : chooser.shown.found.length === 0 && chooser.shown.both ? chooser.sioul.text("bitwarden-choose-nothing-both")
                  : chooser.shown.found.length === 0 ? chooser.sioul.text("bitwarden-choose-nothing")
                  : chooser.shown.more > 0 ? chooser.sioul.textWith("bitwarden-choose-more", "count", String(chooser.shown.more))
                  : ""
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            color: chooser.theme.muted
        }
        ListView {
            id: list

            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(list.contentHeight, 320)
            clip: true
            model: chooser.shown.found
            boundsBehavior: Flickable.StopAtBounds

            delegate: ItemDelegate {
                id: row

                required property var modelData
                required property int index
                readonly property bool warned: chooser.warnElsewhere && row.modelData.elsewhere && row.modelData.site !== ""

                width: ListView.view.width
                highlighted: ListView.isCurrentItem
                Accessible.name: [row.modelData.name, row.modelData.username, siteLine.text].filter(t => t !== "").join(", ")
                onClicked: chooser.take(row.index)

                contentItem: ColumnLayout {
                    spacing: 2

                    // A vault's items may be shared with you by others: plain text.
                    Label {
                        Layout.fillWidth: true
                        text: row.modelData.name
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.weight: Font.DemiBold
                        color: chooser.theme.text
                    }
                    RowLayout {
                        visible: row.modelData.username !== "" || row.modelData.site !== ""
                        Layout.fillWidth: true
                        spacing: 8

                        Label {
                            visible: text !== ""
                            Layout.fillWidth: true
                            text: row.modelData.username
                            textFormat: Text.PlainText
                            elide: Text.ElideRight
                            color: chooser.theme.muted
                        }
                        // Its site; made for another domain than the page's, said so, in the warm colour.
                        Label {
                            id: siteLine

                            visible: text !== ""
                            Layout.fillWidth: row.modelData.username === ""
                            Layout.maximumWidth: row.width * 0.6
                            text: row.warned ? chooser.sioul.textWith("bitwarden-choose-other-site", "site", row.modelData.site) : row.modelData.site
                            textFormat: Text.PlainText
                            elide: Text.ElideMiddle
                            horizontalAlignment: row.modelData.username === "" ? Text.AlignLeft : Text.AlignRight
                            color: row.warned ? chooser.theme.warm : chooser.theme.muted
                        }
                    }
                }
            }
        }
    }

    footer: DialogButtonBox {
        Button {
            text: chooser.theme.plain(chooser.takeText)
            highlighted: true
            enabled: chooser.shown.found.length > 0
            onClicked: chooser.takeCurrent()
        }
        Button {
            text: chooser.sioul.text("ui-cancel")
            DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
        }
        onRejected: chooser.close()
    }
}
