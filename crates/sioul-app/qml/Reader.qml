// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// One message, read: what you can do with it on top (reply, forward, file it
// away), then who wrote to whom apart from the text, its attachments folded,
// its text with quoted messages and repeated headers set off. Everything else
// is in the "…" menu. Moving, deleting and junking happen at once, with
// "Undo" in the status line for ten seconds; nothing asks "are you sure?".

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Panel {
    id: reader

    required property var sioul
    required property var window
    // The message, by its file; empty when none is open.
    property string key: ""
    // Its entry in the list: trust, and on the Porch the reasons and the screener.
    property var item: null
    property var reading: null
    // The invitation the message carries, if any.
    property var invite: null
    // The sender's contact, by file; empty when the sender is not one.
    property string senderContact: ""
    // The site this message announces ("a new item in your secure mailbox"), when it is one of yours.
    readonly property var announces: reader.reading && reader.reading.from_address ? JSON.parse(reader.sioul.siteForSender(reader.reading.from_address) || "null") : null
    // The message, as what new things are tied to.
    readonly property var source: ({
            uri: reader.reading && reader.reading.uri ? reader.reading.uri : "",
            kind: "mail",
            key: reader.key,
            title: reader.reading ? reader.reading.subject : "",
            name: reader.reading ? reader.reading.from_name : "",
            address: reader.reading && reader.reading.from_address ? reader.reading.from_address : "",
            known: reader.senderContact !== ""
        })
    // Read and flagged, as last done here: the server follows.
    property bool unread: false
    property bool flagged: false
    property bool reasonsShown: false
    // Attachments stay folded until asked for.
    property bool attachmentsShown: false
    // The quoted history of an HTML message, folded.
    property bool historyShown: false
    // The link under the pointer, said in full before anyone clicks it.
    property string hoveredLink: ""
    // What the message is tied to (tasks made from it, notes), read when it opens.
    // A message set aside as hostile shows nothing of itself until you choose to read it.
    property bool revealed: false
    readonly property bool gated: reader.item !== null && reader.item.hidden === true && !reader.revealed
    readonly property string role: reading && reading.role ? reading.role : ""
    // Narrow: the actions show their icons only, their names on hover, as
    // soon as their names would not fit in the row (French ones are longer).
    // Narrower still (a phone): "junk" goes in the menu under ⋮.
    readonly property bool compact: reader.actionsWidth(b => b.wideWidth) > reader.availableWidth
    readonly property bool tight: reader.actionsWidth(b => b.narrowWidth) > reader.availableWidth

    signal closeRequested

    // The row's width, each action as wide as `width` says. "Junk" counts
    // whether it shows or not: it hides when the row is too narrow.
    function actionsWidth(width) {
        const named = [replyButton, replyAllButton, forwardButton, addButton, linkButton, archiveButton, trashButton, unsubscribeButton].filter(b => b.visible).concat([junkButton])
        return named.reduce((sum, b) => sum + width(b), 0) + moreButton.implicitWidth + (named.length + 1) * actionRow.spacing
    }

    function trustColor(level) {
        return level === "verified" ? reader.theme.accent : level === "forged" ? reader.theme.forged : reader.theme.muted
    }

    function trustIcon(level) {
        return level === "verified" ? "security-high" : level === "forged" ? "security-low" : "security-medium"
    }

    // Links in the theme's colour: rich text would draw them pure blue, unreadable on a dark theme.
    function withLinks(html) {
        return html.replace(/<a href=/g, '<a style="color:' + reader.theme.accent + '" href=')
    }

    // A link of the message is followed only to the web or to an address, and
    // only once its address shows below: under the pointer, or after a first tap.
    // An address is written to here, in Sioul, without what the link would fill in.
    function follow(link) {
        if (!/^(https?|mailto):/i.test(link))
            return
        if (reader.hoveredLink !== link) {
            reader.hoveredLink = link
            return
        }
        if (/^mailto:/i.test(link))
            reader.window.writeTo(reader.linkHost(link))
        else
            Qt.openUrlExternally(link)
    }

    // Where a link goes: its host, after any "name@" put before it to mislead.
    function linkHost(link) {
        const web = /^[a-z][a-z0-9+.-]*:\/\/(?:[^\/?#]*@)?(\[[^\]\/?#]*\]|[^\/?#:]*)/i.exec(link)
        if (web)
            return web[1]
        const mail = /^mailto:([^?]*)/i.exec(link)
        if (!mail)
            return ""
        try {
            return decodeURIComponent(mail[1])
        } catch (malformed) {
            return mail[1]
        }
    }

    // The attachments unfolded, and the reading scrolled to its end, where they are (the window's images).
    function showAttachments() {
        reader.attachmentsShown = true
        const flick = readingScroll.contentItem as Flickable
        if (flick !== null)
            flick.contentY = Math.max(0, flick.contentHeight - flick.height)
    }

    function attachmentIcon(kind) {
        return { "pdf": "application-pdf", "image": "image-x-generic", "text": "text-x-generic", "document": "x-office-document",
                 "archive": "package-x-generic", "calendar": "view-calendar-day" }[kind] || "mail-attachment"
    }

    // Filed away, the message leaves the list: the reader closes on it.
    function act(action, target) {
        reader.sioul.act(reader.key, action, target || "")
        if (["read", "unread", "flag", "unflag"].indexOf(action) < 0)
            reader.closeRequested()
        else if (action === "read" || action === "unread")
            reader.unread = action === "unread"
        else
            reader.flagged = action === "flag"
    }

    onKeyChanged: {
        keyBand.reset()
        reader.revealed = false
        reader.reasonsShown = false
        reader.attachmentsShown = false
        reader.historyShown = false
        reader.hoveredLink = ""
        reader.reading = reader.key ? JSON.parse(reader.sioul.message(reader.key) || "null") : null
        reader.invite = reader.key ? JSON.parse(reader.sioul.invitation(reader.key) || "null") : null
        reader.senderContact = reader.reading && reader.reading.from_address ? reader.sioul.contactFor(reader.reading.from_address) : ""
        reader.unread = false
        reader.flagged = reader.reading ? reader.reading.flagged : false
        if (reader.reading)
            reader.sioul.opened(reader.key)
    }

    Shortcut {
        sequence: "Escape"
        enabled: reader.visible && reader.key !== ""
        onActivated: reader.closeRequested()
    }
    Shortcut {
        sequence: "Ctrl+R"
        enabled: reader.visible && reader.reading !== null
        onActivated: reader.window.compose("reply", reader.key)
    }
    Shortcut {
        sequence: "Ctrl+Shift+R"
        enabled: reader.visible && reader.reading !== null && reader.reading.others
        onActivated: reader.window.compose("reply-all", reader.key)
    }
    Shortcut {
        sequence: "Ctrl+L"
        enabled: reader.visible && reader.reading !== null
        onActivated: reader.window.compose("forward", reader.key)
    }
    Shortcut {
        sequence: "Delete"
        enabled: reader.visible && reader.reading !== null
        onActivated: reader.act("trash")
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 8

        // What you can do with it, always in the same place. Its width never
        // widens the column: the text below keeps the panel's width.
        RowLayout {
            id: actionRow

            Layout.fillWidth: true
            Layout.minimumWidth: 0
            spacing: 4

            ActionButton {
                id: replyButton

                theme: reader.theme
                compact: reader.compact
                iconName: "mail-reply-sender"
                label: reader.sioul.text("ui-reply")
                onClicked: reader.window.compose("reply", reader.key)
            }
            ActionButton {
                id: replyAllButton

                theme: reader.theme
                compact: reader.compact
                visible: reader.reading !== null && reader.reading.others
                iconName: "mail-reply-all"
                label: reader.sioul.text("ui-reply-all")
                onClicked: reader.window.compose("reply-all", reader.key)
            }
            ActionButton {
                id: forwardButton

                theme: reader.theme
                compact: reader.compact
                iconName: "mail-forward"
                label: reader.sioul.text("ui-forward")
                onClicked: reader.window.compose("forward", reader.key)
            }
            // Something new tied to it: a task for what it asks, an event, a note, a contact.
            ActionButton {
                id: addButton

                theme: reader.theme
                compact: reader.compact
                iconName: "list-add"
                label: reader.sioul.text("ui-add-new")
                onClicked: addMenu.now().popup(addButton, 0, addButton.height)
            }
            // Tied to what is there already: a task, an event, a project, anything.
            ActionButton {
                id: linkButton

                theme: reader.theme
                compact: reader.compact
                enabled: reader.source.uri !== ""
                iconName: "insert-link"
                label: reader.sioul.text("ui-link-existing")
                onClicked: linkMenu.now().popup(linkButton, 0, linkButton.height)
            }
            Item {
                Layout.fillWidth: true
            }
            ActionButton {
                id: archiveButton

                theme: reader.theme
                compact: reader.compact
                visible: reader.role !== "archive" && reader.role !== "all"
                iconName: "archive-insert"
                label: reader.sioul.text("ui-archive")
                onClicked: reader.act("archive")
            }
            ActionButton {
                id: trashButton

                theme: reader.theme
                compact: reader.compact
                iconName: "user-trash"
                label: reader.role === "trash" ? reader.sioul.text("ui-delete-for-good") : reader.sioul.text("ui-trash")
                onClicked: reader.act("trash")
            }
            ActionButton {
                id: junkButton

                theme: reader.theme
                compact: reader.compact
                visible: !reader.tight
                iconName: reader.role === "junk" ? "mail-mark-notjunk" : "mail-mark-junk"
                label: reader.role === "junk" ? reader.sioul.text("ui-not-junk") : reader.sioul.text("ui-junk")
                onClicked: reader.act(reader.role === "junk" ? "not-junk" : "junk")
            }
            // Leaving the list this message comes from, in one click (docs/client.md,
            // "Unsubscribing"): only for mail that names a way out. When nothing
            // may be contacted (forged, spam, nothing proven) it rests dimmed,
            // and its tip (a tap, on a phone) says why. How text reads is in
            // the page's ⚙.
            ActionButton {
                id: unsubscribeButton

                readonly property var offer: reader.reading && reader.reading.unsubscribe ? reader.reading.unsubscribe : null

                theme: reader.theme
                compact: reader.compact
                visible: unsubscribeButton.offer !== null
                opacity: unsubscribeButton.offer !== null && unsubscribeButton.offer.offered ? 1 : 0.5
                iconName: "dialog-cancel"
                label: unsubscribeButton.offer !== null ? unsubscribeButton.offer.label : ""
                ToolTip.visible: unsubscribeButton.hovered
                ToolTip.text: unsubscribeButton.offer !== null ? unsubscribeButton.offer.tip : ""
                Accessible.description: unsubscribeButton.offer !== null ? unsubscribeButton.offer.tip : ""
                onClicked: {
                    const page = reader.sioul.unsubscribe(reader.key)
                    if (page !== "")
                        Qt.openUrlExternally(page)
                }
            }
            ActionButton {
                id: moreButton

                theme: reader.theme
                compact: reader.compact
                iconName: "overflow-menu"
                label: reader.sioul.text("ui-more")
                display: AbstractButton.IconOnly
                ToolTip.visible: moreButton.hovered
                onClicked: more.popup(moreButton, 0, moreButton.height)
            }
        }

        Later {
            id: addMenu

            sourceComponent: Component {
                AddMenu {
                    id: addMenuForm

                    sioul: reader.sioul
                    window: reader.window
                    source: reader.source
                }
            }
        }

        Later {
            id: linkMenu

            sourceComponent: Component {
                SioulMenu {
                    id: linkMenuForm

                    Repeater {
                        model: ["task", "event", "project", ""]

                        delegate: MenuItem {
                            required property string modelData

                            text: reader.sioul.text(modelData === "" ? "link-kind-all" : "link-kind-" + modelData)
                            onTriggered: reader.window.linkFrom(reader.source, modelData)
                        }
                    }
                }
            }
        }

        SioulMenu {
            id: more

            // On a phone, where the row has no room for it.
            MenuItem {
                visible: reader.tight
                height: visible ? implicitHeight : 0
                text: junkButton.label
                onTriggered: reader.act(reader.role === "junk" ? "not-junk" : "junk")
            }
            MenuItem {
                text: reader.unread ? reader.sioul.text("ui-mark-read") : reader.sioul.text("ui-mark-unread")
                onTriggered: reader.act(reader.unread ? "read" : "unread")
            }
            MenuItem {
                text: reader.flagged ? reader.sioul.text("ui-unflag") : reader.sioul.text("ui-flag")
                onTriggered: reader.act(reader.flagged ? "unflag" : "flag")
            }
            MenuItem {
                text: reader.sioul.text("ui-move-to")
                onTriggered: moveDialog.now().open()
            }
            MenuSeparator {}
            MenuItem {
                visible: reader.reading !== null
                height: visible ? implicitHeight : 0
                text: reader.sioul.text("contracts-keep")
                onTriggered: reader.window.contractFromMail(reader.reading.subject, reader.reading.from_name || reader.reading.from_address)
            }
            MenuSeparator {}
            MenuItem {
                visible: reader.reading !== null && !!reader.reading.from_address
                height: visible ? implicitHeight : 0
                text: reader.senderContact ? reader.sioul.text("ui-open-contact") : reader.sioul.text("ui-add-contact")
                onTriggered: {
                    if (reader.senderContact) {
                        reader.window.openContact(reader.senderContact)
                    } else {
                        reader.sioul.addSender(reader.reading.from_name, reader.reading.from_address)
                        reader.senderContact = reader.sioul.contactFor(reader.reading.from_address)
                    }
                }
            }
            MenuSeparator {}
            // How the sender reaches you (PersonSheet.qml): their list and why, the
            // list chosen for them, Always through, each channel at each time. A
            // sender nothing proves is theirs is weighed as a stranger: said, not offered.
            MenuItem {
                enabled: reader.reading !== null && !!reader.reading.block_address && reader.reading.sender_judgeable
                text: reader.reading && !reader.reading.sender_judgeable ? reader.sioul.text("attention-sheet-sender") + " · " + reader.sioul.text("sender-unverified-short") : reader.sioul.text("attention-sheet-sender")
                onTriggered: reader.window.openPersonSheet(reader.senderContact || "", JSON.stringify([reader.reading.from_address]))
            }
            MenuItem {
                text: reader.sioul.text("ui-show-source")
                onTriggered: sourceDialog.now().open()
            }
        }

        // Hostile: what it is, and what you can do, without a word of it.
        ColumnLayout {
            visible: reader.gated
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: reader.sioul.text("hostile-gate")
                wrapMode: Text.Wrap
                font.pixelSize: 17
                color: reader.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: reader.sioul.text("hostile-gate-help")
                wrapMode: Text.Wrap
                lineHeight: 1.3
                color: reader.theme.muted
            }
            Flow {
                Layout.fillWidth: true
                spacing: 8

                Button {
                    text: reader.sioul.text("hostile-forward")
                    icon.name: "mail-forward"
                    icon.color: reader.theme.text
                    onClicked: reader.window.compose("forward", reader.key)
                }
                Button {
                    text: reader.sioul.text("ui-block")
                    onClicked: blockDialog.now().open()
                }
                Button {
                    text: reader.sioul.text("ui-trash")
                    onClicked: reader.act("trash")
                }
                Button {
                    flat: true
                    text: reader.sioul.text("hostile-read")
                    onClicked: reader.revealed = true
                }
            }
            Item {
                Layout.fillHeight: true
            }
        }

        ScrollView {
            id: readingScroll

            visible: !reader.gated
            Layout.fillWidth: true
            Layout.fillHeight: true
            contentWidth: availableWidth

            ColumnLayout {
                width: readingScroll.availableWidth
                spacing: 12

                // A secure mailbox's notice: the mailbox itself, one click away.
                Button {
                    visible: reader.announces !== null
                    text: reader.announces ? reader.sioul.textWith("site-open", "site", reader.announces.name) : ""
                    icon.name: "internet-web-browser"
                    icon.color: reader.theme.accentText
                    highlighted: true
                    onClicked: reader.window.openSite(reader.announces.id)
                }

                // Who wrote to whom, apart from the text.
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: headerColumn.implicitHeight + 24
                    color: reader.theme.background
                    radius: reader.theme.radius

                    ColumnLayout {
                        id: headerColumn

                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 4

                        RowLayout {
                            spacing: 8

                            Icon {
                                visible: reader.item === null || reader.item.trust_level !== "own"
                                iconName: reader.item ? reader.trustIcon(reader.item.trust_level) : "security-medium"
                                tip: reader.reading ? reader.reading.checks : ""
                            }
                            // What the message says of itself is plain text: a "<" opens no tag.
                            Label {
                                Layout.fillWidth: true
                                text: reader.reading ? reader.reading.from_name : ""
                                textFormat: Text.PlainText
                                font.pixelSize: 18
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                                color: reader.theme.text
                            }
                            Icon {
                                visible: reader.flagged
                                iconName: "mail-flag"
                                size: 16
                                tip: reader.sioul.text("ui-flagged")
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: reader.reading ? reader.reading.from_address + (reader.item && reader.item.trust ? "  ·  " + reader.item.trust : "") : ""
                            textFormat: Text.PlainText
                            color: reader.item ? reader.trustColor(reader.item.trust_level) : reader.theme.muted
                            elide: Text.ElideRight
                        }
                        // Encrypted or signed with OpenPGP, and what came of it.
                        RowLayout {
                            visible: reader.reading !== null && !!reader.reading.protection
                            Layout.fillWidth: true
                            spacing: 6

                            Icon {
                                iconName: reader.reading && reader.reading.protection && reader.reading.protection.encrypted ? "document-encrypted" : "document-sign"
                                size: 16
                            }
                            Label {
                                Layout.fillWidth: true
                                text: reader.reading && reader.reading.protection ? reader.reading.protection.line : ""
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: reader.reading && reader.reading.protection && reader.reading.protection.fine ? reader.theme.accent : reader.theme.warm
                            }
                            // Encrypted to your security key: opened only when you ask, never because it is shown.
                            Button {
                                visible: reader.reading !== null && !!reader.reading.protection && reader.reading.protection.security_key && !keyBand.active
                                text: reader.sioul.text("seckey-open-with")
                                onClicked: keyBand.start()
                            }
                        }
                        SecurityKeyBand {
                            id: keyBand

                            Layout.fillWidth: true
                            sioul: reader.sioul
                            theme: reader.theme
                            context: reader.key
                            purpose: "open"
                            onGo: pin => reader.sioul.openWithSecurityKey(reader.key, pin)
                            // Opened: its session key stays in memory, the message reads as any other.
                            onDone: {
                                reader.reading = JSON.parse(reader.sioul.message(reader.key) || "null")
                                keyBand.reset()
                            }
                        }
                        GridLayout {
                            Layout.fillWidth: true
                            Layout.topMargin: 4
                            columns: 2
                            columnSpacing: 12
                            rowSpacing: 2

                            Label {
                                text: reader.sioul.text("ui-to")
                                color: reader.theme.muted
                            }
                            Label {
                                Layout.fillWidth: true
                                text: reader.reading ? reader.reading.to.join(", ") : ""
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: reader.theme.text
                            }
                            Label {
                                visible: reader.reading !== null && reader.reading.cc.length > 0
                                text: reader.sioul.text("ui-cc")
                                color: reader.theme.muted
                            }
                            Label {
                                visible: reader.reading !== null && reader.reading.cc.length > 0
                                Layout.fillWidth: true
                                text: reader.reading ? reader.reading.cc.join(", ") : ""
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: reader.theme.text
                            }
                            Label {
                                text: reader.sioul.text("compose-date")
                                color: reader.theme.muted
                            }
                            Label {
                                Layout.fillWidth: true
                                text: reader.reading ? reader.reading.date : ""
                                color: reader.theme.text
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            Layout.topMargin: 6
                            text: reader.reading ? reader.reading.subject : ""
                            textFormat: Text.PlainText
                            font.pixelSize: 17
                            wrapMode: Text.Wrap
                            color: reader.theme.text
                        }
                    }
                }

                // Why it is where it is, on the Porch: folded, on request.
                Button {
                    visible: reader.item !== null && !!reader.item.reasons
                    flat: true
                    text: (reader.reasonsShown ? "▾  " : "▸  ") + reader.sioul.text("ui-reasons")
                    onClicked: reader.reasonsShown = !reader.reasonsShown
                }
                Repeater {
                    model: reader.reasonsShown && reader.item && reader.item.reasons ? reader.item.reasons : []

                    delegate: Label {
                        id: reason

                        required property string modelData

                        Layout.fillWidth: true
                        Layout.leftMargin: 24
                        text: "· " + reason.modelData
                        textFormat: Text.PlainText
                        wrapMode: Text.Wrap
                        color: reader.theme.muted
                        font.pixelSize: 13
                    }
                }

                // The attachments, apart.
                Rectangle {
                    visible: reader.reading !== null && reader.reading.attachments.length > 0
                    Layout.fillWidth: true
                    implicitHeight: attachmentColumn.implicitHeight + 20
                    color: "transparent"
                    radius: reader.theme.radius
                    border.color: reader.theme.line

                    ColumnLayout {
                        id: attachmentColumn

                        anchors.fill: parent
                        anchors.margins: 10
                        spacing: 6

                        Button {
                            id: attachmentsTitle

                            Layout.fillWidth: true
                            flat: true
                            text: (reader.attachmentsShown ? "▾  " : "▸  ") + reader.sioul.text("ui-attachments-title") + (reader.reading ? "  (" + reader.reading.attachments.length + ")" : "")
                            onClicked: reader.attachmentsShown = !reader.attachmentsShown

                            contentItem: RowLayout {
                                spacing: 8

                                Icon {
                                    iconName: "mail-attachment"
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: attachmentsTitle.text
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                    color: reader.theme.text
                                }
                            }
                        }
                        Label {
                            visible: reader.attachmentsShown && reader.reading !== null && reader.reading.can_open
                            Layout.fillWidth: true
                            // A phone has no antivirus Sioul can call: said, never a scan claimed.
                            text: reader.sioul.attachmentsUnscanned() ? reader.sioul.text("ui-attachments-phone") : reader.sioul.text("ui-attachments-scan")
                            wrapMode: Text.Wrap
                            color: reader.theme.muted
                            font.pixelSize: 13
                        }
                        Repeater {
                            model: reader.attachmentsShown && reader.reading ? reader.reading.attachments : []

                            delegate: RowLayout {
                                id: attached

                                required property var modelData

                                Layout.fillWidth: true
                                spacing: 8

                                Icon {
                                    iconName: reader.attachmentIcon(attached.modelData.kind)
                                    size: 22
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: attached.modelData.name
                                    textFormat: Text.PlainText
                                    elide: Text.ElideMiddle
                                    color: reader.theme.text
                                }
                                Label {
                                    text: attached.modelData.size
                                    color: reader.theme.muted
                                    font.pixelSize: 13
                                }
                                Button {
                                    visible: reader.reading !== null && reader.reading.can_open
                                    text: reader.sioul.text("ui-open")
                                    onClicked: reader.sioul.openAttachment(reader.key, attached.modelData.index)
                                }
                                Button {
                                    visible: reader.reading !== null && reader.reading.can_open
                                    // On a phone, Android asks where.
                                    text: reader.sioul.attachmentsUnscanned() ? reader.sioul.text("ui-save-as") : reader.sioul.text("ui-save")
                                    onClicked: reader.sioul.saveAttachment(reader.key, attached.modelData.index)
                                }
                                Button {
                                    visible: reader.reading !== null && reader.reading.can_open
                                    text: reader.sioul.text("papers-keep")
                                    Accessible.description: reader.sioul.attachmentsUnscanned() ? reader.sioul.text("papers-keep-help-phone") : reader.sioul.text("papers-keep-help")
                                    onClicked: reader.sioul.keepAttachmentAsPaper(reader.key, attached.modelData.index)
                                }
                            }
                        }
                        Label {
                            visible: reader.attachmentsShown && reader.reading !== null && !reader.reading.can_open
                            Layout.fillWidth: true
                            text: reader.sioul.text("ui-attachments-closed")
                            wrapMode: Text.Wrap
                            color: reader.theme.forged
                        }
                    }
                }

                // An invitation: what, when, where, and the answers in view.
                Rectangle {
                    visible: reader.invite !== null
                    Layout.fillWidth: true
                    implicitHeight: inviteColumn.implicitHeight + 20
                    color: reader.theme.background
                    radius: reader.theme.radius
                    border.color: reader.theme.accent

                    ColumnLayout {
                        id: inviteColumn

                        anchors.fill: parent
                        anchors.margins: 10
                        spacing: 6

                        RowLayout {
                            spacing: 8

                            Icon {
                                iconName: "view-calendar-day"
                            }
                            Label {
                                Layout.fillWidth: true
                                text: reader.invite ? (reader.invite.method === "CANCEL" ? reader.sioul.textWith("invite-cancelled", "summary", reader.invite.summary) : reader.invite.summary) : ""
                                textFormat: Text.PlainText
                                font.weight: Font.DemiBold
                                wrapMode: Text.Wrap
                                color: reader.theme.text
                            }
                        }
                        Label {
                            Layout.fillWidth: true
                            text: reader.invite ? [reader.invite.when, reader.invite.location].filter(t => t).join("  ·  ") : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: reader.theme.text
                        }
                        Label {
                            visible: reader.invite !== null && reader.invite.organizer !== ""
                            Layout.fillWidth: true
                            text: reader.invite ? reader.sioul.textWith("agenda-organizer", "name", reader.invite.organizer) : ""
                            textFormat: Text.PlainText
                            color: reader.theme.muted
                        }
                        Label {
                            visible: reader.invite !== null && reader.invite.known && reader.invite.method !== "CANCEL"
                            text: reader.sioul.text("invite-known")
                            color: reader.theme.muted
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 6

                            Button {
                                visible: reader.invite !== null && reader.invite.method === "REQUEST"
                                text: reader.sioul.text("invite-accept")
                                onClicked: reader.sioul.answerInvitation(reader.key, "accepted")
                            }
                            Button {
                                visible: reader.invite !== null && reader.invite.method === "REQUEST"
                                text: reader.sioul.text("invite-maybe")
                                onClicked: reader.sioul.answerInvitation(reader.key, "tentative")
                            }
                            Button {
                                visible: reader.invite !== null && reader.invite.method === "REQUEST"
                                text: reader.sioul.text("invite-decline")
                                onClicked: reader.sioul.answerInvitation(reader.key, "declined")
                            }
                            Button {
                                visible: reader.invite !== null && reader.invite.method === "PUBLISH" && !reader.invite.known
                                text: reader.sioul.text("invite-add")
                                onClicked: reader.sioul.answerInvitation(reader.key, "add")
                            }
                            Button {
                                visible: reader.invite !== null && reader.invite.method === "CANCEL" && reader.invite.known
                                text: reader.sioul.text("invite-remove")
                                onClicked: reader.sioul.answerInvitation(reader.key, "remove")
                            }
                        }
                    }
                }

                // HTML mail, made safe: formatted, its history folded.
                TextEdit {
                    visible: reader.reading !== null && reader.reading.html !== null
                    Layout.fillWidth: true
                    Layout.topMargin: 4
                    text: reader.reading && reader.reading.html ? reader.theme.spaced(reader.withLinks(reader.reading.html.main)) : ""
                    textFormat: TextEdit.RichText
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    color: reader.theme.text
                    font.family: reader.theme.readingFamily || reader.window.font.family
                    font.pixelSize: reader.theme.readingSize
                    onLinkActivated: link => reader.follow(link)
                    onLinkHovered: link => reader.hoveredLink = link
                }
                RowLayout {
                    visible: reader.reading !== null && reader.reading.html !== null && reader.reading.html.images_hidden
                    Layout.fillWidth: true
                    spacing: 8

                    Icon {
                        iconName: "image-x-generic"
                        size: 16
                    }
                    Label {
                        Layout.fillWidth: true
                        text: reader.sioul.text("ui-images-hidden")
                        wrapMode: Text.Wrap
                        color: reader.theme.muted
                        font.pixelSize: 13
                    }
                }
                Button {
                    visible: reader.reading !== null && reader.reading.html !== null && !!reader.reading.html.quoted
                    flat: true
                    text: (reader.historyShown ? "▾  " : "▸  ") + (reader.historyShown ? reader.sioul.text("ui-quote-hide") : reader.sioul.text("ui-quote-history"))
                    onClicked: reader.historyShown = !reader.historyShown
                }
                RowLayout {
                    visible: reader.historyShown && reader.reading !== null && reader.reading.html !== null && !!reader.reading.html.quoted
                    Layout.fillWidth: true
                    spacing: 10

                    Rectangle {
                        Layout.fillHeight: true
                        Layout.preferredWidth: 3
                        radius: 1
                        color: reader.theme.accent
                    }
                    TextEdit {
                        Layout.fillWidth: true
                        text: reader.historyShown && reader.reading && reader.reading.html && reader.reading.html.quoted ? reader.theme.spaced(reader.withLinks(reader.reading.html.quoted)) : ""
                        textFormat: TextEdit.RichText
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.Wrap
                        color: reader.theme.muted
                        font.family: reader.theme.readingFamily || reader.window.font.family
                        font.pixelSize: reader.theme.readingSize
                        onLinkActivated: link => reader.follow(link)
                        onLinkHovered: link => reader.hoveredLink = link
                    }
                }

                // Plain text: what this message says, then what it quotes or forwards.
                Repeater {
                    model: reader.reading && reader.reading.html === null ? reader.reading.parts : []

                    delegate: ColumnLayout {
                        id: part

                        required property var modelData
                        readonly property int lines: part.modelData.text ? part.modelData.text.split("\n").length : 0
                        property bool unfolded: part.lines <= 8

                        Layout.fillWidth: true
                        spacing: 4

                        TextEdit {
                            visible: part.modelData.kind === "text"
                            Layout.fillWidth: true
                            text: part.modelData.kind === "text" ? reader.theme.spaced(reader.withLinks(part.modelData.rich)) : ""
                            readOnly: true
                            selectByMouse: true
                            wrapMode: TextEdit.Wrap
                            textFormat: TextEdit.RichText
                            color: reader.theme.text
                            font.family: reader.theme.readingFamily || reader.window.font.family
                            font.pixelSize: reader.theme.readingSize
                            onLinkActivated: link => reader.follow(link)
                            onLinkHovered: link => reader.hoveredLink = link
                        }

                        RowLayout {
                            visible: part.modelData.kind === "attribution"
                            Layout.fillWidth: true
                            Layout.topMargin: 6
                            spacing: 8

                            Icon {
                                iconName: "mail-reply-sender"
                                size: 16
                            }
                            Label {
                                Layout.fillWidth: true
                                text: part.modelData.kind === "attribution" ? part.modelData.text : ""
                                textFormat: Text.PlainText
                                wrapMode: Text.Wrap
                                color: reader.theme.muted
                                font.pixelSize: 13
                            }
                        }

                        // A quoted message: set off by a bar, folded when long.
                        RowLayout {
                            visible: part.modelData.kind === "quote"
                            Layout.fillWidth: true
                            Layout.leftMargin: Math.max(0, (part.modelData.depth || 1) - 1) * 12
                            spacing: 10

                            Rectangle {
                                Layout.fillHeight: true
                                Layout.preferredWidth: 3
                                radius: 1
                                color: (part.modelData.depth || 1) % 2 === 1 ? reader.theme.accent : reader.theme.warm
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2

                                TextEdit {
                                    Layout.fillWidth: true
                                    text: part.modelData.kind !== "quote" ? "" : reader.theme.spaced(reader.withLinks(part.unfolded ? part.modelData.rich : part.modelData.rich.split("<br>").slice(0, 4).join("<br>") + " …"))
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.Wrap
                                    textFormat: TextEdit.RichText
                                    color: reader.theme.muted
                                    font.family: reader.theme.readingFamily || reader.window.font.family
                                    font.pixelSize: reader.theme.readingSize
                                    onLinkActivated: link => reader.follow(link)
                                    onLinkHovered: link => reader.hoveredLink = link
                                }
                                Button {
                                    visible: part.lines > 8
                                    flat: true
                                    text: part.unfolded ? reader.sioul.text("ui-quote-hide") : reader.sioul.textWith("ui-quote-show", "n", String(part.lines))
                                    onClicked: part.unfolded = !part.unfolded
                                }
                            }
                        }

                        // The headers of a forwarded or answered message.
                        Rectangle {
                            visible: part.modelData.kind === "headers"
                            Layout.fillWidth: true
                            Layout.topMargin: 8
                            implicitHeight: fieldsGrid.implicitHeight + 16
                            color: reader.theme.background
                            radius: reader.theme.radius
                            border.color: reader.theme.line

                            RowLayout {
                                anchors.fill: parent
                                anchors.margins: 8
                                spacing: 10

                                Icon {
                                    Layout.alignment: Qt.AlignTop
                                    iconName: "mail-forward"
                                    size: 16
                                }
                                GridLayout {
                                    id: fieldsGrid

                                    Layout.fillWidth: true
                                    columns: 2
                                    columnSpacing: 10
                                    rowSpacing: 1

                                    // Name, value, name, value: one row per header.
                                    Repeater {
                                        model: part.modelData.kind === "headers" ? part.modelData.fields : []

                                        delegate: Label {
                                            id: field

                                            required property var modelData
                                            required property int index

                                            Layout.fillWidth: field.index % 2 === 1
                                            text: field.modelData
                                            textFormat: Text.PlainText
                                            wrapMode: Text.Wrap
                                            color: field.index % 2 === 0 ? reader.theme.muted : reader.theme.text
                                            font.pixelSize: 13
                                        }
                                    }
                                }
                            }
                        }

                        Label {
                            visible: part.modelData.kind === "signature"
                            Layout.fillWidth: true
                            text: part.modelData.kind === "signature" ? part.modelData.text : ""
                            textFormat: Text.PlainText
                            wrapMode: Text.Wrap
                            color: reader.theme.muted
                            font.pixelSize: 13
                        }
                    }
                }

                Label {
                    visible: reader.reading !== null && reader.reading.html === null && reader.reading.parts.length === 0
                    text: reader.sioul.text("ui-no-text")
                    color: reader.theme.muted
                }

                // What this message is tied to: the task it became, a note, an event.
                RelatedList {
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    sioul: reader.sioul
                    theme: reader.theme
                    title: reader.sioul.text("related-title")
                    uri: reader.reading && reader.reading.uri ? reader.reading.uri : ""
                    onOpenThing: item => reader.window.openThing(item)
                }
            }
        }

        // The link under the pointer, in full; else how the message is shown.
        // Its host first, cut from the left if it must be: the end of a host
        // says whose it is, and a long address cut in its middle could hide it.
        RowLayout {
            id: linkLine

            Layout.fillWidth: true
            spacing: 6

            Label {
                visible: reader.hoveredLink !== "" && text !== ""
                Layout.maximumWidth: Math.round(linkLine.width * 0.45)
                text: reader.linkHost(reader.hoveredLink)
                textFormat: Text.PlainText
                elide: Text.ElideLeft
                font.pixelSize: 12
                font.weight: Font.DemiBold
                color: reader.theme.text
            }
            Label {
                Layout.fillWidth: true
                text: reader.hoveredLink ? reader.sioul.textWith("ui-link", "url", reader.hoveredLink) : reader.sioul.text("ui-text-safe")
                textFormat: Text.PlainText
                elide: Text.ElideMiddle
                color: reader.hoveredLink ? reader.theme.text : reader.theme.muted
                font.pixelSize: 12
            }
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: reader.theme.gap

            // On more lines when the screen is narrow (a phone, in French): never wider than it.
            Flow {
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                spacing: reader.theme.gap

                Button {
                    visible: reader.item !== null && !!reader.item.screener && reader.item.address !== ""
                    text: reader.sioul.text("ui-let-in")
                    onClicked: reader.sioul.letIn(reader.item.address)
                }
                // In the review queue, your word on your own filter's: Spam (into
                // the Junk folder, or kept there), or Not spam (back in its lane,
                // from the Junk folder into the inbox); for good, with ten seconds
                // to undo; `$Junk` or `$NotJunk` told to the server, the act kept in
                // this device's label log, which every device and the next training read.
                // Set aside as spam by your provider: Not spam alone.
                Button {
                    visible: reader.item !== null && !!reader.item.review
                    text: reader.sioul.text("ui-spam")
                    onClicked: reader.act("spam")
                }
                Button {
                    visible: reader.item !== null && !!reader.item.spam
                    text: reader.sioul.text("ui-not-spam")
                    onClicked: reader.act("not-spam")
                }
            }
            Button {
                Layout.alignment: Qt.AlignTop
                text: reader.sioul.text("ui-close")
                onClicked: reader.closeRequested()
            }
        }
    }

    // Blocking: the address, or everyone at its domain; said before it is done.
    Later {
        id: blockDialog

        sourceComponent: Component {
            Dialog {
                id: blockDialogForm

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: Math.min(520, reader.window.width - 2 * reader.theme.gap)
                title: reader.sioul.text("ui-block-title")

                ColumnLayout {
                    width: parent.width
                    spacing: 10

                    Label {
                        Layout.fillWidth: true
                        text: reader.sioul.text("ui-block-text")
                        wrapMode: Text.Wrap
                        color: reader.theme.text
                    }
                    Button {
                        Layout.fillWidth: true
                        text: reader.reading && reader.reading.block_address ? reader.theme.plain(reader.sioul.textWith("ui-block-address", "address", reader.reading.block_address)) : ""
                        onClicked: {
                            reader.sioul.blockFrom(reader.reading.block_address, reader.key)
                            blockDialogForm.close()
                            reader.closeRequested()
                        }
                    }
                    Button {
                        Layout.fillWidth: true
                        visible: reader.reading !== null && !!reader.reading.block_domain
                        text: reader.reading && reader.reading.block_domain ? reader.theme.plain(reader.sioul.textWith("ui-block-domain", "domain", reader.reading.block_domain)) : ""
                        onClicked: {
                            reader.sioul.blockFrom(reader.reading.block_domain, reader.key)
                            blockDialogForm.close()
                            reader.closeRequested()
                        }
                    }
                    Button {
                        Layout.alignment: Qt.AlignRight
                        text: reader.sioul.text("ui-cancel")
                        onClicked: blockDialogForm.close()
                    }
                }
            }
        }
    }

    // Moving: every account's folders, this one's included.
    Later {
        id: moveDialog

        sourceComponent: Component {
            MoveDialog {
                id: moveDialogForm

                sioul: reader.sioul
                theme: reader.theme
                onAboutToShow: {
                    const place = JSON.parse(reader.sioul.place(reader.key) || "{}")
                    moveDialogForm.fromAccount = place.account || ""
                    moveDialogForm.fromFolder = place.folder || ""
                }
                onChosen: (account, folder) => reader.sioul.moveMessages(JSON.stringify([reader.key]), account, folder)
            }
        }
    }

    // The message as it came: headers, parts, encodings.
    Later {
        id: sourceDialog

        sourceComponent: Component {
            Dialog {
                id: sourceDialogForm

                parent: Overlay.overlay
                anchors.centerIn: parent
                modal: true
                width: reader.window.width * 0.8
                height: reader.window.height * 0.8
                title: reader.sioul.text("ui-show-source")

                // Sioul's own button, in your language: Qt's standard ones are not translated.
                footer: DialogButtonBox {
                    Button {
                        text: reader.sioul.text("ui-close")
                        DialogButtonBox.buttonRole: DialogButtonBox.RejectRole
                    }
                }

                ScrollView {
                    anchors.fill: parent

                    TextArea {
                        text: sourceDialogForm.visible ? reader.sioul.source(reader.key) : ""
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextArea.WrapAnywhere
                        font.family: reader.theme.mono
                        font.pixelSize: 12
                        color: reader.theme.text
                    }
                }
            }
        }
    }
}
