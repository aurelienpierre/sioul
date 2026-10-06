// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A site's pop-up (a sign-in, a call): a window of its own for each, on the
// sites' profile, so it stays tied to the page that opened it and a sign-in
// answers it there. Closed, it goes: its page closed first, as a browser
// closes a tab (its beforeunload, pagehide and unload run), the window gone
// at once all the same.

import QtQuick
import QtWebEngine

SioulWindow {
    id: popup

    required property WebEngineProfile profile
    required property var webFixes
    // The site that opened it, as it was then: what a call's window may use.
    property var site: null
    property alias view: popupView
    // How long its page may take to close (SitesPage.qml).
    property int closeWait: 2000
    // Its page closed: by `closePage`, or by itself (a sign-in done).
    property bool pageClosed: false
    // Who waits for its page to close.
    property var whenClosed: []

    signal webAuthAsked(var request)
    signal popupAsked(var request)
    // A call shares a screen: you choose which, or none, in Sioul's dialog.
    // Unanswered, Qt WebEngine would share the whole first screen by itself.
    signal screenAsked(var request)

    width: 520
    height: 680
    title: popupView.title
    // Closed by you, or as Sioul quits: the window goes, its page closes behind it.
    onClosing: {
        if (popup.pageClosed)
            Qt.callLater(() => popup.destroy())
        else
            popup.closePage(() => {})
    }

    // Its page closed as a browser closes a tab, then `done`, at most `closeWait` later.
    function closePage(done) {
        if (popup.pageClosed) {
            done()
            return
        }
        popup.whenClosed = popup.whenClosed.concat([done])
        if (closeDeadline.running)
            return
        closeDeadline.start()
        popupView.triggerWebAction(WebEngineView.RequestClose)
    }
    function pageGone() {
        if (popup.pageClosed)
            return
        closeDeadline.stop()
        popup.pageClosed = true
        const waiting = popup.whenClosed
        popup.whenClosed = []
        for (const done of waiting)
            done()
        // Once this answer is over: it may come while the window is closing.
        Qt.callLater(popup.leave)
    }
    // Its window closed already: it goes; else it closes now (a sign-in done), and goes.
    function leave() {
        if (popup.visible)
            popup.close()
        else
            popup.destroy()
    }

    Timer {
        id: closeDeadline

        interval: popup.closeWait
        onTriggered: popup.pageGone()
    }

    WebEngineView {
        id: popupView

        anchors.fill: parent
        profile: popup.profile
        // A call's window shares the screen as the site's page does (SitesPage.qml).
        settings.screenCaptureEnabled: true
        Component.onCompleted: userScripts.collection = popup.webFixes
        // Closed, by `closePage` or by its own script (which ran its unload first).
        onWindowCloseRequested: popup.pageGone()
        // Called by Qt WebEngine when the page refused to close: the window goes all the same.
        function windowCloseRejected() {
            popup.pageGone()
        }
        // While it closes, its "Leave the site?" is answered yes: you closed it.
        onJavaScriptDialogRequested: request => {
            if (closeDeadline.running && request.type === JavaScriptDialogRequest.DialogTypeBeforeUnload) {
                request.accepted = true
                request.dialogAccept()
            }
        }
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
