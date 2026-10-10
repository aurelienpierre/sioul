// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Accounts, in three tabs. Yours: one card per address, each of its services
// (mail, calendars and contacts, Google) with its server, what its watcher
// last said, a switch to turn it off (kept, neither synced nor shown), and,
// for mail, what it is for, its shield, its rank, how far back and how often,
// your name and signature; "What this server offers" asks the server what
// else it has (mail beside calendars, a Nextcloud's apps). Adding one: mail
// (address, then the server found, then the password, tested before
// anything is kept), contacts and calendars on a CalDAV/CardDAV server, or
// Google (signed in on Google's page). Encryption: your keys. Who may reach
// you, and when, is Settings ▸ What reaches you's (ReachesTab.qml). Sites are
// not accounts: they are on the Sites page.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Item {
    id: page

    required property var sioul
    required property var theme

    readonly property var accounts: page.sioul.accounts ? JSON.parse(page.sioul.accounts) : []
    readonly property var found: page.sioul.found ? JSON.parse(page.sioul.found) : null
    readonly property var scouted: page.sioul.scouted ? JSON.parse(page.sioul.scouted) : null
    property string removing: ""
    // One card per address: its services together.
    readonly property var identities: {
        const out = []
        for (const account of page.accounts) {
            let identity = out.find(i => i.identity === account.identity)
            if (!identity) {
                identity = { identity: account.identity, services: [] }
                out.push(identity)
            }
            identity.services.push(account)
        }
        return out
    }

    // For the window's images: your accounts in view, or another tab.
    function openFirstSettings() {
        tabs.currentIndex = 0
    }

    function showTab(index) {
        tabs.currentIndex = index
    }

    // The Google panel, its steps unfolded (for captures).
    function showGoogle() {
        tabs.currentIndex = 1
        page.googleStepsShown = true
        addTab.item.showGoogle()
    }

    // Android: an account the phone has, chosen in its own list. Its address
    // fills the forms; Google's goes to Google's sign-in.
    Connections {
        target: page.sioul

        function onPhoneAccountChosen(name, kind) {
            if (name.indexOf("@") < 0)
                return
            tabs.currentIndex = 1
            if (kind === "com.google") {
                // Its calendars' form and its mail's, whose server is Google's:
                // an app password or Google's page, never the account's password.
                addTab.item.googleFor(name)
                page.addMailFor(name)
            } else {
                addTab.item.davFor(name, false)
                page.addMailFor(name)
            }
        }
    }

    // Adding the mail of an address whose calendars are here, with another password: the form, filled.
    function addMailFor(address) {
        tabs.currentIndex = 1
        addTab.item.mailFor(address)
        page.lastForm = "mail"
        page.sioul.discover(address)
    }

    function addDavFor(address) {
        tabs.currentIndex = 1
        addTab.item.davFor(address, true)
    }

    // A mail account signed in with Google, signed in again (or given an app
    // password instead): the mail form, its address, Google's two ways.
    function googleMailAgain(account) {
        tabs.currentIndex = 1
        addTab.item.mailFor(account.address)
        page.againId = account.id
        page.lastForm = "mail"
        page.sioul.discover(account.address)
    }

    // An account's password, given on this device (AccountPassword.qml): made the first time.
    function askPassword(account) {
        if (passwordDialog.item === null)
            passwordDialog.setSource("AccountPassword.qml", { sioul: page.sioul, theme: page.theme })
        passwordDialog.item.ask(account)
    }

    Loader {
        id: passwordDialog
    }

    // A key's file: imported once its passphrase, if it has one, is typed.
    FileDialog {
        id: keyPicker

        title: page.sioul.text("ui-import-key")
        nameFilters: ["OpenPGP (*.asc *.gpg *.pgp *.key)", page.sioul.text("ui-all-files") + " (*)"]
        onAccepted: {
            page.importing = keyPicker.selectedFile.toString()
            page.passphraseWanted()
        }
    }

    // A security key's certificate, from `gpg --export`: imported for the key
    // the setup read ("" ) or for a key Sioul knows (its identifier).
    FileDialog {
        id: certificatePicker

        property string forKey: ""

        title: page.sioul.text("seckey-import-file")
        nameFilters: ["OpenPGP (*.asc *.gpg *.pgp *.key)", page.sioul.text("ui-all-files") + " (*)"]
        onAccepted: page.sioul.importSecurityKeyCertificate(certificatePicker.forKey, certificatePicker.selectedFile.toString())
    }

    // Your security keys, read again when keys change; what the setup says now;
    // the key a lookup is for ("" for the one the setup read).
    property var securityKeys: ({ cards: [] })
    property var keySetup: ({ state: "", line: "", tries: "", warm: false, action: "", fingerprint: "", ident: "", commands: [] })
    property string lookingFor: ""
    // The key "Send it to keys.openpgp.org" is about, once its sentence is shown.
    property string publishing: ""

    function lookFor(ident) {
        page.lookingFor = ident
        page.sioul.findSecurityKeyCertificate(ident)
    }

    // "Send it to keys.openpgp.org": first what goes public, in a sentence; sent on "Send it" only.
    function askToSend(ident) {
        page.publishing = ident
        page.keySetup = { state: "publish", line: page.sioul.text("seckey-send-tells"), tries: "", warm: false, action: "", fingerprint: "", ident: ident, commands: [] }
    }

    // Your OpenPGP keys and others', read again when they change.
    property var keys: ({ keys: [], without_key: [] })
    property bool othersShown: false
    property string importing: ""
    // The key's passphrase field, in its tab, takes the focus.
    signal passphraseWanted

    // Which form a message from the backend is about: "mail", "dav" or "google".
    property string lastForm: "mail"
    // The mail form signs this account in again (Google's mail), rather than adding one.
    property string againId: ""
    property bool davServerShown: false
    property bool googleStepsShown: false
    // Sioul's own Google key in this build, and whether you would rather use yours.
    readonly property bool googleBuiltIn: page.sioul.googleBuiltIn()
    property bool ownGoogleKey: false

    // A new discovery fills the fields again, even those edited by hand.
    Connections {
        target: page.sioul

        function onKeysChanged() {
            page.keys = JSON.parse(page.sioul.pgpKeys() || "{\"keys\": [], \"without_key\": []}")
            page.securityKeys = JSON.parse(page.sioul.securityKeys() || "{\"cards\": []}")
        }

        // What the security key's setup says now.
        function onSecurityKeyChanged(context, state) {
            if (context === "setup")
                page.keySetup = JSON.parse(state)
        }

        function onFoundChanged() {
            if (page.found && addTab.item)
                addTab.item.fillFound(page.found)
        }

        function onAccountAdded() {
            if (addTab.item)
                addTab.item.clear()
            tabs.currentIndex = 0
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: page.theme.gap
        spacing: page.theme.gap

        Label {
            Layout.fillWidth: true
            text: page.sioul.text("ui-accounts")
            font.pixelSize: 22
            color: page.theme.text
        }
        // The tabs, as the task page's views: the one shown filled.
        Flow {
            id: tabs

            property int currentIndex: 0
            // The tabs made so far: each the first time it is shown.
            property var made: ({ 0: true })

            onCurrentIndexChanged: {
                const made = Object.assign({}, tabs.made)
                made[tabs.currentIndex] = true
                tabs.made = made
            }

            Layout.fillWidth: true
            spacing: 6

            Repeater {
                model: ["accounts-tab-yours", "accounts-tab-add", "accounts-tab-keys"]

                delegate: Button {
                    id: tab

                    required property string modelData
                    required property int index

                    text: page.sioul.text(modelData)
                    checkable: true
                    checked: tabs.currentIndex === index
                    flat: tabs.currentIndex !== index
                    onClicked: {
                        tabs.currentIndex = index
                        // A click on the tab shown would untick it: it stays the one shown.
                        tab.checked = Qt.binding(() => tabs.currentIndex === tab.index)
                    }
                }
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: tabs.currentIndex

            // Your accounts: one card per address.
            // One card per address, made as it comes into view (a phone builds one at a time).
            ListView {
                id: yoursScroll

                clip: true
                spacing: page.theme.gap
                model: page.identities
                // No card chosen: the list opens on its top, the header in view.
                currentIndex: -1
                // Cards made once are kept (made again at each pass, they cost a stutter).
                cacheBuffer: 4000
                ScrollBar.vertical: ScrollBar {}

                header: ColumnLayout {
                    width: yoursScroll.width
                    spacing: page.theme.gap

                    Label {
                        visible: page.accounts.length === 0
                        Layout.fillWidth: true
                        text: page.sioul.text("ui-no-accounts")
                        wrapMode: Text.Wrap
                        color: page.theme.muted
                    }

                    // How far back mail and the agenda reach: the choice below the words
                    // when the screen is narrow (a phone), what it means under both.
                    Flow {
                        Layout.fillWidth: true
                            spacing: page.theme.gap

                        Label {
                            height: history.height
                            verticalAlignment: Text.AlignVCenter
                            text: page.sioul.text("ui-history")
                            color: page.theme.text
                        }
                        PlainComboBox {
                            id: history

                            readonly property var weeks: [1, 2, 4, 13, 26, 52, 0]

                            width: 200
                            model: weeks.map(w => page.sioul.text("history-" + w))
                            currentIndex: Math.max(0, weeks.indexOf(page.sioul.historyWeeks()))
                            onActivated: index => page.sioul.setHistory(weeks[index])
                        }
                    }
                    Label {
                        Layout.fillWidth: true
                        text: page.sioul.text("ui-history-note")
                        wrapMode: Text.Wrap
                        color: page.theme.muted
                        font.pixelSize: 13
                    }

                    Item {
                    Layout.preferredHeight: 1
                    }
                }

                delegate: Panel {
                    id: card

                    required property var modelData
                    readonly property var scout: page.scouted !== null && page.scouted.address === card.modelData.identity ? page.scouted : null

                    width: yoursScroll.width
                    theme: page.theme

                    ColumnLayout {
                        anchors.fill: parent
                        spacing: 10

                        // Who it is, whole; what its server offers beside it, or
                        // below it on a narrow screen (a phone).
                        GridLayout {
                            readonly property bool narrow: card.width < 520

                            Layout.fillWidth: true
                            columns: narrow ? 2 : 3
                            columnSpacing: page.theme.gap
                            rowSpacing: 4

                            Icon {
                                iconName: "user-identity"
                            }
                            Label {
                                Layout.fillWidth: true
                                text: card.modelData.identity
                                textFormat: Text.PlainText
                                font.pixelSize: 17
                                font.weight: Font.DemiBold
                                // An address has no space to break at: anywhere, rather than cut.
                                wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                                color: page.theme.text
                            }
                            Button {
                                Layout.columnSpan: parent.narrow ? 2 : 1
                                visible: card.modelData.identity.indexOf("@") > 0
                                flat: true
                                enabled: !page.sioul.scouting
                                icon.name: "system-search"
                                text: page.sioul.scouting && card.scout === null ? page.sioul.text("scout-asking") : page.sioul.text("scout-button")
                                onClicked: page.sioul.scoutAccount(card.modelData.identity)
                            }
                        }

                        Repeater {
                            model: card.modelData.services

                            delegate: ServiceBlock {
                                Layout.fillWidth: true
                            }
                        }

                        // What the server says it offers besides.
                        ColumnLayout {
                            visible: card.scout !== null
                            Layout.fillWidth: true
                            spacing: 6

                            Label {
                                Layout.fillWidth: true
                                text: card.scout === null ? "" : page.sioul.textWith("scout-title", "server", card.scout.nextcloud ? card.scout.nextcloud.server.replace("https://", "") : card.modelData.identity.split("@")[1])
                                textFormat: Text.PlainText
                                font.weight: Font.DemiBold
                                wrapMode: Text.Wrap
                                color: page.theme.text
                            }
                            // Mail.
                            RowLayout {
                                visible: card.scout !== null && card.scout.mail !== null
                                Layout.fillWidth: true
                                spacing: 8

                                // What a server says of itself is plain text: a "<" opens no tag.
                                Label {
                                    Layout.fillWidth: true
                                    text: card.scout && card.scout.mail ? page.sioul.textWith("scout-mail", "server", card.scout.mail.line) + (card.scout.mail.have ? "  ·  " + page.sioul.text("scout-have") : "") : ""
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                            }
                            Flow {
                                visible: card.scout !== null && card.scout.mail !== null && !card.scout.mail.have
                                Layout.fillWidth: true
                                spacing: 8

                                Button {
                                    visible: card.scout !== null && card.scout.dav_password
                                    enabled: !page.sioul.formBusy
                                    highlighted: true
                                    text: page.sioul.text("scout-mail-add")
                                    onClicked: {
                                        page.lastForm = "scout"
                                        page.sioul.addMailLike(card.modelData.identity, card.scout.mail.host, card.scout.mail.port, card.scout.mail.security, card.scout.mail.login)
                                    }
                                }
                                Button {
                                    flat: true
                                    text: page.sioul.text("scout-mail-add-other")
                                    onClicked: page.addMailFor(card.modelData.identity)
                                }
                            }
                            // Calendars, tasks and contacts.
                            RowLayout {
                                visible: card.scout !== null && card.scout.dav !== null
                                Layout.fillWidth: true
                                spacing: 8

                                Label {
                                    Layout.fillWidth: true
                                    text: card.scout && card.scout.dav ? page.sioul.textWith("scout-dav", "server", card.scout.dav.url.replace("https://", "")) + (card.scout.dav.have ? "  ·  " + page.sioul.text("scout-have") : "") : ""
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: page.theme.text
                                }
                                Button {
                                    visible: card.scout !== null && card.scout.dav !== null && !card.scout.dav.have
                                    flat: true
                                    text: page.sioul.text("scout-dav-add")
                                    onClicked: page.addDavFor(card.modelData.identity)
                                }
                            }
                            // A Nextcloud, its apps.
                            Label {
                                visible: card.scout !== null && card.scout.nextcloud !== null
                                Layout.fillWidth: true
                                text: card.scout && card.scout.nextcloud ? page.sioul.textArgs("scout-nextcloud", JSON.stringify({ product: card.scout.nextcloud.product, version: card.scout.nextcloud.version })) + "  ·  " + (card.scout.nextcloud.asked ? page.sioul.text("scout-apps") : page.sioul.text("scout-apps-unasked")) : ""
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: page.theme.text
                            }
                            Repeater {
                                model: card.scout && card.scout.nextcloud ? card.scout.nextcloud.apps : []

                                delegate: Label {
                                    required property var modelData

                                    Layout.fillWidth: true
                                    Layout.leftMargin: 12
                                    text: "·  " + modelData.title + "  —  " + page.sioul.text(modelData.in_sioul ? "scout-in-sioul" : "scout-not-in-sioul")
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    color: modelData.in_sioul ? page.theme.text : page.theme.muted
                                }
                            }
                            Label {
                                visible: card.scout !== null && card.scout.mail === null && card.scout.dav === null && card.scout.nextcloud === null
                                Layout.fillWidth: true
                                text: page.sioul.text("scout-nothing")
                                color: page.theme.muted
                            }
                            Label {
                                visible: page.sioul.formError !== "" && page.lastForm === "scout"
                                Layout.fillWidth: true
                                text: page.sioul.formError
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: page.theme.forged
                            }
                        }
                    }
                }

                footer: ColumnLayout {
                    width: yoursScroll.width
                    spacing: page.theme.gap

                    // What all addresses share: the key for the AI that reads hostile mail, when one allows it.
                    Repeater {
                        id: shared

                        property var rows: JSON.parse(page.sioul.settings("accounts") || "[]")

                        model: shared.rows

                        delegate: SettingRow {
                            required property var modelData

                            Layout.fillWidth: true
                            Layout.topMargin: page.theme.gap
                            setting: modelData
                            sioul: page.sioul
                            theme: page.theme
                            onSave: (key, value, shown) => {
                                const problem = page.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
                                if (problem === "")
                                    shared.rows = JSON.parse(page.sioul.settings("accounts") || "[]")
                                else
                                    page.sioul.status = problem
                            }
                        }
                    }
                    Item {
                        Layout.preferredHeight: page.theme.gap
                    }
                }
            }

            // Adding one.
            // Made the first time it is shown.
            Loader {
                id: addTab

                active: tabs.made[1] === true
                sourceComponent: Component {
                    ScrollView {
                        id: addScroll

                        contentWidth: availableWidth
                        clip: true

                            // What the page asks of this tab once it is made (`addTab.item`).
                            function fillFound(found) {
                                host.text = found.host
                                port.text = String(found.port)
                                security.currentIndex = found.security === "starttls" ? 1 : 0
                                login.text = found.login
                                password.text = ""
                                // Once the found address's own bindings (its key kept) have followed.
                                Qt.callLater(() => {
                                    if (googleMail.item)
                                        googleMail.item.reset()
                                })
                                // Google's mail asks no account password: its own form instead.
                                if (!found.google)
                                    password.forceActiveFocus()
                            }
                            function clear() {
                                addressField.text = ""
                                password.text = ""
                                page.againId = ""
                                if (googleMail.item)
                                    googleMail.item.reset()
                                davAddress.text = ""
                                davPassword.text = ""
                                davUrl.text = ""
                                davLogin.text = ""
                                googleAddress.text = ""
                                googleId.text = ""
                                googleSecret.text = ""
                            }
                            function mailFor(address) {
                                addressField.text = address
                            }
                            function davFor(address, focus) {
                                davAddress.text = address
                                if (focus)
                                    davPassword.forceActiveFocus()
                            }
                            function googleFor(address) {
                                googleAddress.text = address
                            }
                            function showGoogle() {
                                Qt.callLater(() => addScroll.contentItem.contentY = Math.max(0, googlePanel.y - page.theme.gap))
                            }

                        ColumnLayout {
                            width: addScroll.availableWidth
                            spacing: page.theme.gap

                            // Android: the phone's own accounts, its addresses without their passwords.
                            ColumnLayout {
                                visible: page.sioul.phoneAccounts()
                                Layout.fillWidth: true
                                spacing: 4

                                Button {
                                    text: page.sioul.text("ui-phone-account")
                                    icon.name: "user-identity"
                                    icon.color: page.theme.text
                                    onClicked: page.sioul.choosePhoneAccount()
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: page.sioul.text("ui-phone-account-note")
                                    wrapMode: Text.Wrap
                                    font.pixelSize: 13
                                    color: page.theme.muted
                                }
                            }

                            // Adding a mail account.
                            Panel {
                                Layout.fillWidth: true
                                                        theme: page.theme

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 10

                                    Label {
                                        text: page.sioul.text("ui-add-account")
                                        font.pixelSize: 18
                                        color: page.theme.text
                                    }
                                    RowLayout {
                                        spacing: page.theme.gap

                                        TextField {
                                            id: addressField

                                            Layout.fillWidth: true
                                            placeholderText: page.sioul.text("ui-address")
                                            inputMethodHints: Qt.ImhEmailCharactersOnly | Qt.ImhNoAutoUppercase
                                            onAccepted: if (find.enabled) find.clicked()
                                        }
                                        Button {
                                            id: find

                                            text: page.sioul.formBusy && page.found === null ? page.sioul.text("ui-finding") : page.sioul.text("ui-find-server")
                                            enabled: addressField.text.indexOf("@") > 0 && !page.sioul.formBusy
                                            onClicked: {
                                                page.lastForm = "mail"
                                                page.againId = ""
                                                page.sioul.discover(addressField.text)
                                            }
                                        }
                                    }

                                    // What was found, said before any password is typed.
                                    ColumnLayout {
                                        visible: page.found !== null
                                        Layout.fillWidth: true
                                        spacing: 10

                                        Label {
                                            Layout.fillWidth: true
                                            text: page.found ? page.found.by : ""
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: page.theme.text
                                        }
                                        Label {
                                            visible: !page.found || !page.found.google
                                            Layout.fillWidth: true
                                            text: page.found ? page.found.hint : ""
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: page.theme.warm
                                        }
                                        Button {
                                            visible: page.found !== null && !!page.found.help_url && !page.found.google
                                            text: page.sioul.text("ui-app-passwords")
                                            onClicked: Qt.openUrlExternally(page.found.help_url)
                                        }
                                        // Google's mail: an app password or Google's page (GoogleMail.qml),
                                        // made the first time an address is Google's.
                                        Loader {
                                            id: googleMail

                                            active: page.found !== null && page.found.google === true
                                            visible: active
                                            Layout.fillWidth: true
                                            sourceComponent: GoogleMail {
                                                sioul: page.sioul
                                                theme: page.theme
                                                address: addressField.text
                                                found: page.found
                                                againId: page.againId
                                                onStarted: page.lastForm = "mail"
                                                onDone: {
                                                    addTab.item.clear()
                                                    page.sioul.found = ""
                                                    tabs.currentIndex = 0
                                                }
                                            }
                                        }
                                        GridLayout {
                                            visible: !page.found || !page.found.google
                                            Layout.fillWidth: true
                                            columns: 2
                                            columnSpacing: page.theme.gap
                                            rowSpacing: 8

                                            Label {
                                                text: page.sioul.text("ui-host")
                                                color: page.theme.muted
                                            }
                                            TextField {
                                                id: host
                                                Layout.fillWidth: true
                                            }
                                            Label {
                                                text: page.sioul.text("ui-port")
                                                color: page.theme.muted
                                            }
                                            TextField {
                                                id: port
                                                Layout.preferredWidth: 100
                                                validator: IntValidator {
                                                    bottom: 1
                                                    top: 65535
                                                }
                                            }
                                            Label {
                                                text: page.sioul.text("ui-security")
                                                color: page.theme.muted
                                            }
                                            PlainComboBox {
                                                id: security
                                                Layout.fillWidth: true
                                                model: [page.sioul.text("security-tls"), page.sioul.text("security-starttls")]
                                            }
                                            Label {
                                                text: page.sioul.text("ui-login")
                                                color: page.theme.muted
                                            }
                                            TextField {
                                                id: login
                                                Layout.fillWidth: true
                                            }
                                            Label {
                                                text: page.sioul.text("ui-password")
                                                color: page.theme.muted
                                            }
                                            PasswordField {
                                                id: password
                                                Layout.fillWidth: true
                                                sioul: page.sioul
                                                onAccepted: if (connect.enabled) connect.clicked()
                                            }
                                        }
                                        Label {
                                            visible: !page.found || !page.found.google
                                            Layout.fillWidth: true
                                            text: page.sioul.text("ui-password-note")
                                            wrapMode: Text.Wrap
                                            color: page.theme.muted
                                        }
                                        Button {
                                            id: connect

                                            visible: !page.found || !page.found.google
                                            text: page.sioul.formBusy ? page.sioul.text("ui-connecting") : page.sioul.text("ui-connect")
                                            enabled: !page.sioul.formBusy && password.text.length > 0 && host.text.length > 0
                                            onClicked: {
                                                page.lastForm = "mail"
                                                page.sioul.addAccount(addressField.text, host.text, parseInt(port.text), security.currentIndex === 1 ? "starttls" : "tls", login.text, password.text)
                                            }
                                        }
                                    }

                                    Label {
                                        visible: page.sioul.formError !== "" && page.lastForm === "mail"
                                        Layout.fillWidth: true
                                        text: page.sioul.formError
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.forged
                                    }
                                }
                            }

                            // Adding contacts and calendars (CardDAV, CalDAV).
                            Panel {
                                Layout.fillWidth: true
                                theme: page.theme

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 10

                                    Label {
                                        text: page.sioul.text("ui-add-dav")
                                        font.pixelSize: 18
                                        color: page.theme.text
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: page.sioul.text("ui-add-dav-note")
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                    }
                                    GridLayout {
                                        Layout.fillWidth: true
                                        columns: 2
                                        columnSpacing: page.theme.gap
                                        rowSpacing: 8

                                        Label {
                                            text: page.sioul.text("ui-address")
                                            color: page.theme.muted
                                        }
                                        TextField {
                                            id: davAddress

                                            Layout.fillWidth: true
                                            inputMethodHints: Qt.ImhEmailCharactersOnly | Qt.ImhNoAutoUppercase
                                        }
                                        Label {
                                            text: page.sioul.text("ui-password")
                                            color: page.theme.muted
                                        }
                                        PasswordField {
                                            id: davPassword

                                            Layout.fillWidth: true
                                            sioul: page.sioul
                                        }
                                    }
                                    // Where the server is, when it cannot be found from the address.
                                    Button {
                                        flat: true
                                        text: (page.davServerShown ? "▾  " : "▸  ") + page.sioul.text("ui-dav-server")
                                        onClicked: page.davServerShown = !page.davServerShown
                                    }
                                    GridLayout {
                                        visible: page.davServerShown
                                        Layout.fillWidth: true
                                        columns: 2
                                        columnSpacing: page.theme.gap
                                        rowSpacing: 8

                                        Label {
                                            text: page.sioul.text("ui-host")
                                            color: page.theme.muted
                                        }
                                        TextField {
                                            id: davUrl

                                            Layout.fillWidth: true
                                            placeholderText: "https://…"
                                        }
                                        Label {
                                            text: page.sioul.text("ui-login")
                                            color: page.theme.muted
                                        }
                                        TextField {
                                            id: davLogin

                                            Layout.fillWidth: true
                                        }
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: page.sioul.text("ui-password-note")
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                    }
                                    Button {
                                        text: page.sioul.formBusy && page.lastForm === "dav" ? page.sioul.text("ui-connecting") : page.sioul.text("ui-connect")
                                        enabled: !page.sioul.formBusy && davAddress.text.indexOf("@") > 0 && davPassword.text.length > 0
                                        onClicked: {
                                            page.lastForm = "dav"
                                            page.sioul.addDav(davAddress.text, davUrl.text, davLogin.text, davPassword.text)
                                        }
                                    }
                                    Label {
                                        visible: page.sioul.formError !== "" && page.lastForm === "dav"
                                        Layout.fillWidth: true
                                        text: page.sioul.formError
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.forged
                                    }
                                }
                            }

                            // Google's calendars and contacts: signed in on Google's page, with your own key.
                            Panel {
                                id: googlePanel

                                Layout.fillWidth: true
                                theme: page.theme

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 10

                                    Label {
                                        text: page.sioul.text("ui-add-google")
                                        font.pixelSize: 18
                                        color: page.theme.text
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: page.sioul.text(page.googleBuiltIn && !page.ownGoogleKey ? "ui-add-google-built-in" : "ui-add-google-note")
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                    }
                                    // Sioul's own key in this build: your own only if you want it.
                                    CheckBox {
                                        visible: page.googleBuiltIn
                                        text: page.sioul.text("ui-google-own-key")
                                        checked: page.ownGoogleKey
                                        onToggled: page.ownGoogleKey = checked
                                    }
                                    // Its sentence wraps: on one line, it was wider than a phone.
                                    Button {
                                        id: googleStepsButton

                                        visible: !page.googleBuiltIn || page.ownGoogleKey
                                        Layout.fillWidth: true
                                        flat: true
                                        text: (page.googleStepsShown ? "▾  " : "▸  ") + page.sioul.text("ui-google-steps")
                                        contentItem: Label {
                                            text: googleStepsButton.text
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: page.theme.text
                                        }
                                        onClicked: page.googleStepsShown = !page.googleStepsShown
                                    }
                                    Label {
                                        visible: page.googleStepsShown
                                        Layout.fillWidth: true
                                        text: page.sioul.text("ui-google-steps-text")
                                        textFormat: Text.MarkdownText
                                        wrapMode: Text.Wrap
                                        color: page.theme.text
                                        onLinkActivated: link => Qt.openUrlExternally(link)
                                    }
                                    GridLayout {
                                        Layout.fillWidth: true
                                        columns: 2
                                        columnSpacing: page.theme.gap
                                        rowSpacing: 8

                                        Label {
                                            text: page.sioul.text("ui-address")
                                            color: page.theme.muted
                                        }
                                        TextField {
                                            id: googleAddress

                                            Layout.fillWidth: true
                                            inputMethodHints: Qt.ImhEmailCharactersOnly | Qt.ImhNoAutoUppercase
                                        }
                                        Label {
                                            visible: !page.googleBuiltIn || page.ownGoogleKey
                                            text: page.sioul.text("ui-google-client-id")
                                            color: page.theme.muted
                                        }
                                        TextField {
                                            id: googleId

                                            visible: !page.googleBuiltIn || page.ownGoogleKey

                                            Layout.fillWidth: true
                                            placeholderText: "….apps.googleusercontent.com"
                                        }
                                        Label {
                                            visible: !page.googleBuiltIn || page.ownGoogleKey
                                            text: page.sioul.text("ui-google-client-secret")
                                            color: page.theme.muted
                                        }
                                        PasswordField {
                                            id: googleSecret

                                            visible: !page.googleBuiltIn || page.ownGoogleKey

                                            Layout.fillWidth: true
                                            sioul: page.sioul
                                        }
                                    }
                                    RowLayout {
                                        spacing: 8

                                        Button {
                                            text: page.sioul.text("ui-google-sign-in")
                                            enabled: !page.sioul.formBusy && googleAddress.text.indexOf("@") > 0 && ((page.googleBuiltIn && !page.ownGoogleKey) || (googleId.text.trim().length > 0 && googleSecret.text.trim().length > 0))
                                            onClicked: {
                                                page.lastForm = "google"
                                                page.sioul.addGoogle(googleAddress.text, page.googleBuiltIn && !page.ownGoogleKey ? "" : googleId.text, page.googleBuiltIn && !page.ownGoogleKey ? "" : googleSecret.text, "")
                                            }
                                        }
                                        Button {
                                            visible: page.sioul.formBusy && page.lastForm === "google"
                                            flat: true
                                            text: page.sioul.text("ui-cancel")
                                            onClicked: page.sioul.cancelGoogle()
                                        }
                                    }
                                    Label {
                                        visible: page.sioul.formBusy && page.lastForm === "google"
                                        Layout.fillWidth: true
                                        text: page.sioul.text("ui-google-waiting")
                                        wrapMode: Text.Wrap
                                        color: page.theme.text
                                    }
                                    Label {
                                        visible: page.sioul.formError !== "" && page.lastForm === "google"
                                        Layout.fillWidth: true
                                        text: page.sioul.formError
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.forged
                                    }
                                }
                            }

                            Item {
                                Layout.preferredHeight: page.theme.gap
                            }
                        }
                    }
                }
            }

            // Encryption.
            // Made the first time it is shown: its rows cost seconds on a phone.
            Loader {
                active: tabs.made[2] === true
                sourceComponent: Component {
                    ScrollView {
                        id: keysScroll

                        contentWidth: availableWidth
                        clip: true
                        Component.onCompleted: {
                            page.keys = JSON.parse(page.sioul.pgpKeys() || "{\"keys\": [], \"without_key\": []}")
                            page.securityKeys = JSON.parse(page.sioul.securityKeys() || "{\"cards\": []}")
                        }

                            Connections {
                                target: page

                                function onPassphraseWanted() {
                                    keyPassphrase.forceActiveFocus()
                                }
                            }

                        ColumnLayout {
                            width: keysScroll.availableWidth
                            spacing: page.theme.gap

                            // Your security key: its private keys stay on it; Sioul keeps their public part.
                            Panel {
                                Layout.fillWidth: true
                                theme: page.theme

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 10

                                    Label {
                                        text: page.sioul.text("seckey-section")
                                        font.pixelSize: 18
                                        color: page.theme.text
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: page.sioul.text("seckey-intro")
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                    }
                                    // The keys Sioul knows: what each signs for, its touch, its certificate.
                                    Repeater {
                                        model: page.securityKeys.cards

                                        delegate: ColumnLayout {
                                            id: knownKey

                                            required property var modelData

                                            Layout.fillWidth: true
                                            spacing: 2

                                            RowLayout {
                                                Layout.fillWidth: true
                                                spacing: 8

                                                Icon {
                                                    iconName: "dialog-password"
                                                }
                                                Label {
                                                    Layout.fillWidth: true
                                                    text: knownKey.modelData.title
                                                    textFormat: Text.PlainText
                                                    elide: Text.ElideRight
                                                    color: page.theme.text
                                                }
                                            }
                                            Repeater {
                                                model: [knownKey.modelData.signs_for, knownKey.modelData.touch, knownKey.modelData.source]

                                                delegate: Label {
                                                    required property string modelData

                                                    Layout.fillWidth: true
                                                    text: modelData
                                                    textFormat: Text.PlainText
                                                    wrapMode: Text.Wrap
                                                    color: page.theme.muted
                                                    font.pixelSize: 13
                                                }
                                            }
                                            Label {
                                                visible: knownKey.modelData.expiry !== ""
                                                Layout.fillWidth: true
                                                text: knownKey.modelData.expiry
                                                textFormat: Text.PlainText
                                                wrapMode: Text.Wrap
                                                color: knownKey.modelData.late ? page.theme.warm : page.theme.muted
                                                font.pixelSize: 13
                                            }
                                            Flow {
                                                Layout.fillWidth: true
                                                Layout.topMargin: 4
                                                spacing: 8

                                                // Expired or expiring, GnuPG here able to: its own pinentry asks the PIN.
                                                Button {
                                                    visible: knownKey.modelData.renewable === true
                                                    highlighted: true
                                                    text: page.sioul.text("seckey-renew")
                                                    onClicked: page.sioul.renewSecurityKey(knownKey.modelData.ident)
                                                }
                                                Button {
                                                    text: page.sioul.text("seckey-newer")
                                                    onClicked: page.lookFor(knownKey.modelData.ident)
                                                }
                                                Button {
                                                    flat: true
                                                    text: page.sioul.text("seckey-import-gnupg")
                                                    onClicked: page.sioul.importSecurityKeyFromGnupg(knownKey.modelData.ident)
                                                }
                                                Button {
                                                    flat: true
                                                    text: page.sioul.text("seckey-import-file")
                                                    onClicked: {
                                                        certificatePicker.forKey = knownKey.modelData.ident
                                                        certificatePicker.open()
                                                    }
                                                }
                                                Button {
                                                    flat: true
                                                    text: page.sioul.text("seckey-send")
                                                    onClicked: page.askToSend(knownKey.modelData.ident)
                                                }
                                                Button {
                                                    visible: knownKey.modelData.pin_held
                                                    flat: true
                                                    text: page.sioul.text("seckey-forget-pin")
                                                    onClicked: page.sioul.forgetSecurityKeyPin()
                                                }
                                                Button {
                                                    flat: true
                                                    text: page.sioul.text("seckey-stop")
                                                    onClicked: page.sioul.stopUsingSecurityKey(knownKey.modelData.ident)
                                                }
                                            }
                                            // The same by hand, each command to copy.
                                            CommandText {
                                                visible: (knownKey.modelData.by_hand || []).length > 0
                                                Layout.fillWidth: true
                                                Layout.topMargin: 2
                                                sioul: page.sioul
                                                theme: page.theme
                                                text: page.sioul.text("seckey-by-hand") + " " + (knownKey.modelData.by_hand || []).map(c => "`" + c + "`").join(" ")
                                                color: page.theme.muted
                                                pixelSize: 13
                                            }
                                        }
                                    }
                                    // The setup: what the key says, then where its public part comes from.
                                    RowLayout {
                                        visible: page.keySetup.state !== ""
                                        Layout.fillWidth: true
                                        spacing: 8

                                        BusyIndicator {
                                            visible: page.keySetup.state === "working"
                                            running: visible
                                            implicitWidth: 18
                                            implicitHeight: 18
                                        }
                                        // Its commands, if any, to copy (no smart card service, a locked key).
                                        CommandText {
                                            Layout.fillWidth: true
                                            sioul: page.sioul
                                            theme: page.theme
                                            text: page.keySetup.line || ""
                                            color: page.keySetup.warm ? page.theme.warm : page.theme.text
                                        }
                                    }
                                    Label {
                                        visible: page.keySetup.state === "found"
                                        Layout.fillWidth: true
                                        text: page.sioul.text("seckey-lookup-tells")
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                        font.pixelSize: 13
                                    }
                                    Flow {
                                        visible: page.keySetup.state === "found" || page.keySetup.state === "missing" || page.keySetup.state === "problem" || page.keySetup.state === "publish" || page.keySetup.action === "publish" || page.keySetup.action === "renew-offer"
                                        Layout.fillWidth: true
                                        spacing: 8

                                        Button {
                                            visible: page.keySetup.state === "found"
                                            highlighted: true
                                            text: page.sioul.text("seckey-look-for-it")
                                            onClicked: page.lookFor("")
                                        }
                                        // Also after GnuPG could not: once it can, or from a file written by hand.
                                        Button {
                                            visible: page.keySetup.state === "found" || page.keySetup.state === "missing" || (page.keySetup.state === "problem" && !!page.keySetup.fingerprint)
                                            text: page.sioul.text("seckey-import-gnupg")
                                            onClicked: page.sioul.importSecurityKeyFromGnupg(page.lookingFor)
                                        }
                                        Button {
                                            visible: page.keySetup.state === "found" || page.keySetup.state === "missing" || (page.keySetup.state === "problem" && !!page.keySetup.fingerprint)
                                            text: page.sioul.text("seckey-import-file")
                                            onClicked: {
                                                certificatePicker.forKey = page.lookingFor
                                                certificatePicker.open()
                                            }
                                        }
                                        // What goes public was said: sent on this press only.
                                        Button {
                                            visible: page.keySetup.state === "publish"
                                            highlighted: true
                                            text: page.sioul.text("seckey-send-confirm")
                                            onClicked: page.sioul.sendSecurityKeyToKeysOpenpgp(page.publishing)
                                        }
                                        // A part of the key expired: renewed from here, each subkey named.
                                        Button {
                                            visible: page.keySetup.action === "renew-offer"
                                            highlighted: true
                                            text: page.sioul.text("seckey-renew")
                                            onClicked: page.sioul.renewSecurityKey(page.keySetup.ident)
                                        }
                                        // Renewed: others have the new date once it is sent.
                                        Button {
                                            visible: page.keySetup.action === "publish"
                                            text: page.sioul.text("seckey-send")
                                            onClicked: page.askToSend(page.keySetup.ident)
                                        }
                                        Button {
                                            visible: page.keySetup.action === "release"
                                            highlighted: true
                                            text: page.sioul.text("seckey-release")
                                            onClicked: {
                                                const answer = JSON.parse(page.sioul.letGnupgRelease() || "{}")
                                                page.keySetup = { state: answer.done ? "working" : "problem", line: answer.line || "", tries: "", warm: !answer.done, action: answer.done ? "" : "retry", fingerprint: "", ident: "", commands: [] }
                                                if (answer.done)
                                                    page.sioul.readSecurityKey()
                                            }
                                        }
                                        // A touch missed while GnuPG renewed the key: renewed again on this press.
                                        Button {
                                            visible: page.keySetup.action === "retry" || page.keySetup.action === "release" || page.keySetup.action === "lookup" || page.keySetup.action === "renew"
                                            text: page.sioul.text("seckey-try-again")
                                            onClicked: {
                                                if (page.keySetup.action === "lookup")
                                                    page.lookFor(page.lookingFor)
                                                else if (page.keySetup.action === "renew")
                                                    page.sioul.renewSecurityKey(page.keySetup.ident)
                                                else
                                                    page.sioul.readSecurityKey()
                                            }
                                        }
                                        Button {
                                            flat: true
                                            text: page.sioul.text("seckey-not-now")
                                            onClicked: page.keySetup = { state: "", line: "", tries: "", warm: false, action: "", fingerprint: "", ident: "", commands: [] }
                                        }
                                    }
                                    // What the buttons do, by hand, each command to copy.
                                    CommandText {
                                        visible: (page.keySetup.commands || []).length > 0 && page.keySetup.state !== "working"
                                        Layout.fillWidth: true
                                        sioul: page.sioul
                                        theme: page.theme
                                        text: page.sioul.text("seckey-by-hand") + " " + (page.keySetup.commands || []).map(c => "`" + c + "`").join(" ")
                                        color: page.theme.muted
                                        pixelSize: 13
                                    }
                                    Button {
                                        visible: page.keySetup.state === "" || page.keySetup.state === "done"
                                        text: page.sioul.text("seckey-use")
                                        onClicked: {
                                            page.lookingFor = ""
                                            page.sioul.readSecurityKey()
                                        }
                                    }
                                }
                            }

                            // Encryption: your keys, made here or imported; others' keys folded.
                            Panel {
                                Layout.fillWidth: true
                                theme: page.theme

                                ColumnLayout {
                                    anchors.fill: parent
                                    spacing: 10

                                    Label {
                                        text: page.sioul.text("ui-encryption")
                                        font.pixelSize: 18
                                        color: page.theme.text
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        text: page.sioul.text("ui-encryption-note")
                                        wrapMode: Text.Wrap
                                        color: page.theme.muted
                                    }
                                    Repeater {
                                        model: page.keys.keys.filter(k => k.own)

                                        delegate: RowLayout {
                                            id: ownKey

                                            required property var modelData

                                            Layout.fillWidth: true
                                            spacing: 8

                                            Icon {
                                                iconName: "document-encrypt"
                                            }
                                            ColumnLayout {
                                                Layout.fillWidth: true
                                                spacing: 0

                                                Label {
                                                    Layout.fillWidth: true
                                                    text: ownKey.modelData.names.join(", ")
                                                    textFormat: Text.PlainText
                                                    elide: Text.ElideRight
                                                    color: page.theme.text
                                                }
                                                Label {
                                                    Layout.fillWidth: true
                                                    text: ownKey.modelData.fingerprint.match(/.{1,4}/g).join(" ") + (ownKey.modelData.expires ? "  ·  " + page.sioul.textWith("pgp-expires", "date", ownKey.modelData.expires) : "")
                                                    textFormat: Text.PlainText
                                                    font.family: page.theme.mono
                                                    font.pixelSize: 12
                                                    elide: Text.ElideRight
                                                    color: page.theme.muted
                                                }
                                            }
                                            Button {
                                                text: page.sioul.text("ui-export-key")
                                                onClicked: page.sioul.pgpExport(ownKey.modelData.fingerprint)
                                            }
                                            // Made here: its revocation certificate, kept beside it, to copy somewhere safe.
                                            Button {
                                                visible: ownKey.modelData.revocation === true
                                                text: page.sioul.text("ui-save-revocation")
                                                onClicked: page.sioul.pgpSaveRevocation(ownKey.modelData.fingerprint)
                                            }
                                        }
                                    }
                                    Repeater {
                                        model: page.keys.without_key

                                        delegate: Button {
                                            id: makeKey

                                            required property string modelData

                                            text: page.theme.plain(page.sioul.textWith("ui-make-key", "address", makeKey.modelData))
                                            onClicked: page.sioul.pgpGenerate(makeKey.modelData)
                                        }
                                    }
                                    RowLayout {
                                        Layout.fillWidth: true
                                        spacing: 8

                                        Button {
                                            text: page.sioul.text("ui-import-key")
                                            onClicked: keyPicker.open()
                                        }
                                        PasswordField {
                                            id: keyPassphrase

                                            visible: page.importing !== ""
                                            Layout.fillWidth: true
                                            sioul: page.sioul
                                            placeholderText: page.sioul.text("ui-key-passphrase")
                                            onAccepted: importNow.clicked()
                                        }
                                        Button {
                                            id: importNow

                                            visible: page.importing !== ""
                                            text: page.sioul.text("ui-import")
                                            onClicked: {
                                                const problem = page.sioul.pgpImport(page.importing, keyPassphrase.text)
                                                page.lastForm = "pgp"
                                                if (problem) {
                                                    keyError.text = problem
                                                    return
                                                }
                                                keyError.text = ""
                                                page.importing = ""
                                                keyPassphrase.text = ""
                                            }
                                        }
                                    }
                                    Label {
                                        id: keyError

                                        visible: text !== ""
                                        Layout.fillWidth: true
                                        textFormat: Text.PlainText
                                        wrapMode: Text.Wrap
                                        color: page.theme.forged
                                    }
                                    Button {
                                        visible: page.keys.keys.some(k => !k.own)
                                        flat: true
                                        text: (page.othersShown ? "▾  " : "▸  ") + page.sioul.text("ui-others-keys")
                                        onClicked: page.othersShown = !page.othersShown
                                    }
                                    Repeater {
                                        model: page.othersShown ? page.keys.keys.filter(k => !k.own) : []

                                        delegate: RowLayout {
                                            id: otherKey

                                            required property var modelData

                                            Layout.fillWidth: true
                                            Layout.leftMargin: 12
                                            spacing: 8

                                            // Their names come with their mail (Autocrypt): plain text.
                                            Label {
                                                Layout.fillWidth: true
                                                text: otherKey.modelData.names.join(", ")
                                                textFormat: Text.PlainText
                                                elide: Text.ElideRight
                                                color: page.theme.text
                                            }
                                            Button {
                                                flat: true
                                                text: page.sioul.text("ui-remove")
                                                onClicked: page.sioul.pgpRemove(otherKey.modelData.fingerprint)
                                            }
                                        }
                                    }
                                }
                            }

                        }
                    }
                }
            }
        }
    }

    // One service of an address: its server, its switch, what its watcher
    // said; for mail, its own settings in full.
    component ServiceBlock: ColumnLayout {
        id: service

        required property var modelData
        // Mail's own settings, read when unfolded (`settingsShown`).
        readonly property bool hasSettings: service.modelData.service === "mail" && service.modelData.enabled
        property var settings: []
        property bool settingsShown: false
        property bool detailsShown: false

        spacing: 6

        RowLayout {
            Layout.fillWidth: true
            spacing: page.theme.gap

            Icon {
                iconName: service.modelData.service === "mail" ? "internet-mail" : "view-calendar"
            }
            Label {
                Layout.fillWidth: true
                text: page.sioul.text("account-service-" + service.modelData.service) + (service.modelData.rows.length > 0 ? "  ·  " + service.modelData.rows[0].value : "")
                textFormat: Text.PlainText
                font.weight: Font.DemiBold
                // Its server, on a second line rather than cut, on a narrow screen.
                wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                color: service.modelData.enabled ? page.theme.text : page.theme.muted
            }
            Switch {
                id: serviceSwitch

                checked: service.modelData.enabled
                Accessible.name: page.sioul.text("account-service-" + service.modelData.service)
                ToolTip.visible: hovered
                ToolTip.text: page.sioul.text("account-switch-help")
                ToolTip.delay: 500
                onToggled: {
                    const problem = page.sioul.setAccountEnabled(service.modelData.id, checked)
                    if (problem !== "") {
                        page.sioul.status = problem
                        // Not changed: the switch says so again.
                        serviceSwitch.checked = Qt.binding(() => service.modelData.enabled)
                    }
                }
            }
            Button {
                flat: true
                text: page.sioul.text("ui-remove")
                onClicked: {
                    page.removing = service.modelData.id
                    confirm.now().open()
                }
            }
        }
        RowLayout {
            visible: service.modelData.status !== ""
            Layout.fillWidth: true
            Layout.leftMargin: 28
            spacing: 8

            // A problem in the warm colour of its words, not Breeze's orange alarm.
            Icon {
                iconName: service.modelData.status_error ? "dialog-warning" : "dialog-ok-apply"
                color: service.modelData.status_error ? page.theme.warm : page.theme.text
                size: 16
            }
            Label {
                Layout.fillWidth: true
                text: service.modelData.status
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                color: service.modelData.status_error ? page.theme.warm : page.theme.text
            }
            // No password on this device (an account come from another one), or refused.
            Button {
                visible: service.modelData.password_wanted
                enabled: !page.sioul.formBusy
                text: page.sioul.text("account-password")
                onClicked: page.askPassword(service.modelData)
            }
            // Google ended the access: its page again, with the key kept.
            Button {
                visible: service.modelData.google && service.modelData.service !== "mail" && service.modelData.status_error
                enabled: !page.sioul.formBusy
                text: page.sioul.text("ui-google-again")
                onClicked: {
                    page.lastForm = "google"
                    page.sioul.addGoogle(service.modelData.address, "", "", service.modelData.id)
                }
            }
        }
        // Google's mail signed in with Google, its access ended (or never given on
        // this device): Google's page again, or an app password instead; never
        // the account's password. Below the line, to fit a phone.
        Flow {
            visible: service.modelData.google && service.modelData.service === "mail" && service.modelData.status_error
            Layout.fillWidth: true
            Layout.leftMargin: 28
            spacing: 8

            Button {
                enabled: !page.sioul.formBusy
                text: page.sioul.text("ui-google-again")
                onClicked: page.googleMailAgain(service.modelData)
            }
            Button {
                enabled: !page.sioul.formBusy
                text: page.sioul.text("ui-gmail-again-app-password")
                onClicked: page.askPassword(service.modelData)
            }
        }
        // Mail: what it is for, its shield, how far back, how often: folded, made
        // when unfolded (a card with them all took half a second on a phone).
        Button {
            visible: service.hasSettings
            Layout.leftMargin: 20
            flat: true
            text: (service.settingsShown ? "▾  " : "▸  ") + page.sioul.text("account-settings")
            onClicked: {
                if (!service.settingsShown)
                    service.settings = JSON.parse(page.sioul.settings("account:" + service.modelData.id) || "[]")
                service.settingsShown = !service.settingsShown
            }
        }
        Loader {
            active: service.settingsShown
            visible: active
            Layout.fillWidth: true
            Layout.leftMargin: 28

            sourceComponent: ColumnLayout {
                spacing: 6

                Repeater {
                    model: service.settings

                    delegate: SettingRow {
                        required property var modelData

                        Layout.fillWidth: true
                        setting: modelData
                        sioul: page.sioul
                        theme: page.theme
                        onSave: (key, value, shown) => {
                            const problem = page.sioul.changeSetting(key, JSON.stringify(shown), JSON.stringify(value))
                            if (problem === "")
                                service.settings = JSON.parse(page.sioul.settings("account:" + service.modelData.id) || "[]")
                            else
                                page.sioul.status = problem
                        }
                    }
                }
            }
        }
        // Its rank, then your name and signature: below one another when the screen is narrow.
        Flow {
            visible: service.modelData.service === "mail" && service.modelData.enabled
            Layout.fillWidth: true
            Layout.leftMargin: 28
            spacing: page.theme.gap

            Label {
                height: priority.height
                verticalAlignment: Text.AlignVCenter
                text: page.sioul.text("account-row-priority")
                color: page.theme.muted
            }
            PlainComboBox {
                id: priority

                readonly property var levels: ["above", "average", "below"]

                width: 220
                model: levels.map(level => page.sioul.text("priority-" + level))
                currentIndex: levels.indexOf(service.modelData.priority)
                onActivated: index => page.sioul.setPriority(service.modelData.id, levels[index])
            }
            Button {
                flat: true
                text: page.sioul.text("ui-writing")
                onClicked: writing.now().edit(service.modelData)
            }
        }
        // Server, sender checks, where things are kept: folded.
        Button {
            visible: service.modelData.rows.length > 0
            Layout.leftMargin: 20
            flat: true
            text: (service.detailsShown ? "▾  " : "▸  ") + page.sioul.text("account-details")
            onClicked: service.detailsShown = !service.detailsShown
        }
        // Made when unfolded.
        Loader {
            active: service.detailsShown
            visible: active
            Layout.fillWidth: true
            Layout.leftMargin: 28

            sourceComponent: GridLayout {
                columns: 2
                columnSpacing: page.theme.gap
                rowSpacing: 2

                Repeater {
                    model: service.modelData.rows.reduce((all, r) => all.concat([r.label, r.value]), [])

                    delegate: Label {
                        id: cell

                        required property string modelData
                        required property int index

                        Layout.fillWidth: cell.index % 2 === 1
                        text: cell.modelData
                        textFormat: Text.PlainText
                        // A folder's path has no space to break at: anywhere, then.
                        wrapMode: Text.WrapAtWordBoundaryOrAnywhere
                        font.pixelSize: 13
                        color: cell.index % 2 === 0 ? page.theme.muted : page.theme.text
                    }
                }
            }
        }
    }

    // Your name as recipients see it, and your signature, for one account.
    Later {
        id: writing

        sourceComponent: Component {
            Dialog {
                id: writingForm

                property string account: ""

                function edit(account) {
                    writingForm.account = account.id
                    writingName.text = account.name
                    writingSignature.text = account.signature
                    writingForm.open()
                }

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(560, page.width - 2 * page.theme.gap)
                title: page.theme.plain(writingForm.account)
                onAccepted: page.sioul.setWriting(writingForm.account, writingName.text, writingSignature.text)

                ColumnLayout {
                    width: parent.width
                    spacing: 8

                    Label {
                        text: page.sioul.text("ui-your-name")
                        color: page.theme.muted
                    }
                    TextField {
                        id: writingName

                        Layout.fillWidth: true
                    }
                    Label {
                        text: page.sioul.text("ui-signature")
                        color: page.theme.muted
                    }
                    TextArea {
                        id: writingSignature

                        // A field shows where it is: a border, darker when it has the focus.
                        background: Rectangle {
                            color: page.theme.surface
                            radius: page.theme.radius
                            border.color: writingSignature.activeFocus ? page.theme.focus : page.theme.line
                        }
                        Layout.fillWidth: true
                        Layout.preferredHeight: 110
                        wrapMode: TextArea.Wrap
                        placeholderText: page.sioul.text("ui-signature-note")
                    }
                }

                footer: DialogButtonBox {
                    Button {
                        text: page.sioul.text("ui-save")
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }
            }
        }
    }

    Later {
        id: confirm

        sourceComponent: Component {
            Dialog {
                id: confirmForm

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(480, page.width - 2 * page.theme.gap)
                onAccepted: page.sioul.removeAccount(page.removing)

                Label {
                    width: parent.width
                    text: page.sioul.textWith("ui-remove-confirmForm", "id", page.removing)
                    textFormat: Text.PlainText
                    wrapMode: Text.Wrap
                    color: page.theme.text
                }

                footer: DialogButtonBox {
                    Button {
                        text: page.sioul.text("ui-remove")
                        DialogButtonBox.buttonRole: DialogButtonBox.AcceptRole
                    }
                    Button {
                        text: page.sioul.text("ui-cancel")
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }
            }
        }
    }
}
