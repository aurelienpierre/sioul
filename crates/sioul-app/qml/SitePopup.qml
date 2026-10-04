// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A site's pop-up (a sign-in, a call): a window of its own for each, on the
// sites' profile, so it stays tied to the page that opened it and a sign-in
// answers it there. Closed, it goes.

import QtQuick
import QtWebEngine

SioulWindow {
    id: popup

    required property WebEngineProfile profile
    required property var webFixes
    // The site that opened it, as it was then: what a call's window may use.
    property var site: null
    property alias view: popupView

    signal webAuthAsked(var request)
    signal popupAsked(var request)
    // A call shares a screen: you choose which, or none, in Sioul's dialog.
    // Unanswered, Qt WebEngine would share the whole first screen by itself.
    signal screenAsked(var request)

    width: 520
    height: 680
    title: popupView.title
    onClosing: Qt.callLater(() => popup.destroy())

    WebEngineView {
        id: popupView

        anchors.fill: parent
        profile: popup.profile
        Component.onCompleted: userScripts.collection = popup.webFixes
        onWindowCloseRequested: popup.close()
        onWebAuthUxRequested: request => popup.webAuthAsked(request)
        // A pop-up of its own (a provider's second window): a window of its own too.
        onNewWindowRequested: request => popup.popupAsked(request)
        onDesktopMediaRequested: request => popup.screenAsked(request)
        onPermissionRequested: permission => {
            // A call's window: as the site that opened it allows; notifications
            // and the clipboard (a page's "Copy" asks it) as its page; nothing else.
            const site = popup.site
            const kinds = WebEnginePermission.PermissionType
            const type = permission.permissionType
            const allowed = type === kinds.Notifications || type === kinds.ClipboardReadWrite ? true
                : type === kinds.MediaAudioCapture ? site !== null && site.microphone
                : type === kinds.MediaVideoCapture ? site !== null && site.camera
                : type === kinds.MediaAudioVideoCapture ? site !== null && site.microphone && site.camera
                : (type === kinds.DesktopVideoCapture || type === kinds.DesktopAudioVideoCapture) ? site !== null && site.screen
                : false
            if (allowed)
                permission.grant()
            else
                permission.deny()
        }
    }
}
