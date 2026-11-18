// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// "From Bitwarden…", under a password field of the forms that add an
// account or give one its password again (AccountsPage.qml,
// GoogleMail.qml, AccountPassword.qml): the vault opened first when it is
// locked (VaultUnlock.qml, the security key where the window can ask one),
// then its logins, found by the name the account logs in with as the user
// name (LoginChooser.qml); the one chosen fills the field, and only its
// password leaves the vault. The field is then the page's VaultHeld.qml's
// for good, which purges it (and its undo) when the vault locks or the form
// is left; the button says which login filled it, until the hand changes it. Shown once a Bitwarden account is
// set (Accounts ▸ Your accounts ▸ Password manager). Another domain than the
// server's is no warning here: the password goes to the account's own
// server.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic

Button {
    id: fill

    // Sioul's backend (backend.rs): its words in your language and what it does.
    required property var sioul
    // The window's colours, sizes and fonts (Theme.qml).
    required property var theme
    // What asks a security key (VaultUnlock.qml's `keys`): the window's on a computer, none on a phone.
    property var keys: null
    // "missing" (no account set), "locked" or "unlocked": the page says it, and reads it again.
    property string vault: "missing"
    // The account's server, as a site ("https://imap.example.org"), its logins first; "" for none.
    property string site: ""
    // The name the account logs in with: its own, else its address.
    property string user: ""
    // The field a login chosen fills (a PasswordField), and the page's keeper of
    // such fields (VaultHeld.qml), which empties it when the vault locks.
    property PasswordField field: null
    property var held: null
    // The login chosen is remembered for `site`, proposed first there next time,
    // the Sites page's fills included: only for an account's own mail server
    // (AccountPassword.qml), never for a form's, whose host may be a site's.
    property bool remember: false
    // The name of the login that filled the field, while it holds its password.
    property string from: ""

    // A login chosen and its password in the field: the login's name, to say where it came from.
    signal taken(string name)
    // What went wrong, in words (the vault's answer).
    signal problem(string text)
    // The vault opened here: the page reads its state again.
    signal opened

    // The vault opened first when it is locked; then its logins.
    function choose() {
        if (fill.sioul.bitwardenState() === "locked")
            unlock.begin()
        else
            chooser.now().beginForAccount(fill.site, fill.user)
    }

    function take(item) {
        const login = JSON.parse(fill.sioul.bitwardenLogin(fill.remember ? fill.site : "", item))
        if (login.error) {
            fill.problem(login.error)
            return
        }
        fill.field.text = login.password
        if (fill.held)
            fill.held.took(fill.field, login.password)
        fill.from = login.name
        fill.taken(login.name)
    }

    // The field emptied or typed in by hand: no longer that login's password as it gave it.
    Connections {
        target: fill.field

        function onTextChanged() {
            if (fill.field.text === "")
                fill.from = ""
        }
        function onTextEdited() {
            fill.from = ""
        }
    }
    // Let go by the keeper (the vault locked, the form left): no login named.
    Connections {
        target: fill.held

        function onPurged(field) {
            if (field === fill.field)
                fill.from = ""
        }
    }

    visible: fill.vault !== "missing"
    implicitWidth: implicitContentWidth + leftPadding + rightPadding
    flat: true
    enabled: !fill.sioul.formBusy
    icon.name: "dialog-password"
    icon.color: fill.theme.text
    // Which login filled the field, once one did: "From Bitwarden: Cloud." (a press chooses another).
    text: fill.from !== "" ? fill.theme.plain(fill.sioul.textWith("account-password-from", "name", fill.from)) : fill.sioul.text("account-password-vault")
    onClicked: fill.choose()

    // Bitwarden, opened here for the session.
    VaultUnlock {
        id: unlock

        sioul: fill.sioul
        theme: fill.theme
        keys: fill.keys
        onUnlocked: {
            fill.opened()
            chooser.now().beginForAccount(fill.site, fill.user)
        }
    }

    // Made the first time.
    Later {
        id: chooser

        sourceComponent: Component {
            LoginChooser {
                sioul: fill.sioul
                theme: fill.theme
                warnElsewhere: false
                takeText: fill.sioul.text("account-password-use")
                onChosen: item => fill.take(item)
            }
        }
    }
}
