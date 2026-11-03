// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// A PDF of the notes folder, read in place: pages one under the other, the
// width of the view (Ctrl and the wheel, or two fingers, zoom), its text
// selectable (Ctrl+C copies it). Loaded only when a PDF opens.
//
// Its links (decided 3 November 2026), as a link in mail: one to another
// page of the document goes there; one to the web or to an address shows
// where it goes first, in a bar under the pages ("Opens example.org:
// https://…"), and opens only with Open (an address: Sioul's writing
// window); any other kind (file:, javascript:…) is not followed, and the bar
// says so. Built from QtQuick.Pdf's own parts (PdfDocument, PdfPageImage,
// PdfLinkModel, PdfSelection): Qt's PdfMultiPageView opens every link itself.

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Pdf

Item {
    id: view

    required property url source
    // Sioul's backend (backend.rs): its words in your language.
    property var sioul: null
    // The window's colours, sizes and fonts (Theme.qml).
    property var theme: null
    // The window (main.qml): its writing window, for an address.
    property var window: null
    // How a web link is opened, once you said Open: the system's browser (a test puts its own).
    property var openExternally: url => Qt.openUrlExternally(url)
    // The pages' width against the view's: 1, the view's width.
    property real zoom: 1
    // The link shown in the bar: {url, host, followed}; null for none.
    property var asked: null

    readonly property alias document: pdf
    readonly property alias pages: list

    PdfDocument {
        id: pdf

        source: view.source
    }

    // A link tapped: another page at once; the web or an address, its address
    // shown and asked about once; anything else refused, and said.
    function follow(url, page, location) {
        if (page >= 0) {
            list.positionViewAtIndex(page, ListView.Beginning)
            return
        }
        const text = String(url)
        const followed = /^(https?:\/\/|mailto:)/i.test(text)
        view.asked = { url: text, host: followed ? view.hostOf(text) : "", followed: followed }
    }

    // Open pressed: an address goes to the writing window, a web link to the browser.
    function open() {
        const asked = view.asked
        view.asked = null
        if (asked === null || !asked.followed)
            return
        if (/^mailto:/i.test(asked.url)) {
            if (view.window)
                view.window.writeTo(view.hostOf(asked.url))
        } else {
            view.openExternally(asked.url)
        }
    }

    // Where a link goes: its host, after any "name@" put before it to mislead;
    // an address's own, decoded (as the Reader says it, Reader.qml `linkHost`).
    function hostOf(link) {
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

    function words(id, args) {
        if (!view.sioul)
            return id
        return args ? view.sioul.textArgs(id, JSON.stringify(args)) : view.sioul.text(id)
    }

    // A button's words as plain text: a button draws rich text otherwise (Theme.qml `plain`).
    function plain(text) {
        return view.theme ? view.theme.plain(text) : text
    }

    ListView {
        id: list

        anchors.fill: parent
        anchors.bottomMargin: bar.visible ? bar.height : 0
        clip: true
        spacing: 8
        model: pdf.status === PdfDocument.Ready ? pdf.pageCount : 0
        contentWidth: width * Math.max(1, view.zoom)
        flickableDirection: view.zoom > 1 ? Flickable.HorizontalAndVerticalFlick : Flickable.VerticalFlick
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        delegate: Item {
            id: sheet

            required property int index
            readonly property size points: pdf.pagePointSize(sheet.index)
            // Pixels per point: the view's width, times the zoom.
            readonly property real ratio: list.width * view.zoom / Math.max(1, sheet.points.width)

            width: Math.max(1, sheet.points.width * sheet.ratio)
            height: Math.max(1, sheet.points.height * sheet.ratio)

            Rectangle {
                anchors.fill: parent
                color: "white"
            }
            PdfPageImage {
                anchors.fill: parent
                document: pdf
                currentFrame: sheet.index
                asynchronous: true
                fillMode: Image.PreserveAspectFit
                sourceSize.width: Math.ceil(sheet.width * Screen.devicePixelRatio)
            }
            PdfSelection {
                id: selection

                anchors.fill: parent
                document: pdf
                page: sheet.index
                renderScale: Math.max(0.01, sheet.ratio)
                from: textDrag.centroid.pressPosition
                to: textDrag.centroid.position
                hold: !textDrag.active
            }
            DragHandler {
                id: textDrag

                acceptedDevices: PointerDevice.Mouse | PointerDevice.Stylus
                target: null
            }
            Shortcut {
                sequences: [StandardKey.Copy]
                enabled: selection.text !== ""
                onActivated: selection.copyToClipboard()
            }
            Repeater {
                model: PdfLinkModel {
                    document: pdf
                    page: sheet.index
                }

                delegate: Item {
                    id: link

                    required property rect rectangle
                    required property url url
                    required property int page
                    required property point location

                    objectName: "pdfLink"
                    x: link.rectangle.x * sheet.ratio
                    y: link.rectangle.y * sheet.ratio
                    width: link.rectangle.width * sheet.ratio
                    height: link.rectangle.height * sheet.ratio

                    HoverHandler {
                        cursorShape: Qt.PointingHandCursor
                    }
                    TapHandler {
                        gesturePolicy: TapHandler.ReleaseWithinBounds
                        onTapped: view.follow(link.url, link.page, link.location)
                    }
                }
            }
        }

        // Ctrl and the wheel, or two fingers: the pages larger or smaller, from half to four times.
        WheelHandler {
            acceptedModifiers: Qt.ControlModifier
            onWheel: event => view.zoom = Math.min(4, Math.max(0.5, view.zoom * (event.angleDelta.y > 0 ? 1.1 : 1 / 1.1)))
        }
        PinchHandler {
            target: null
            onScaleChanged: delta => view.zoom = Math.min(4, Math.max(0.5, view.zoom * delta))
        }
    }

    // Where the link tapped goes, asked once: Open or Cancel; one not followed, said.
    Rectangle {
        id: bar

        objectName: "pdfLinkBar"
        visible: view.asked !== null
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: row.implicitHeight + 16
        color: view.theme ? view.theme.surface : "#f4f1ea"
        border.color: view.theme ? view.theme.line : "#d0ccc4"

        RowLayout {
            id: row

            anchors.fill: parent
            anchors.margins: 8
            spacing: 8

            Label {
                objectName: "pdfLinkWords"
                Layout.fillWidth: true
                Layout.preferredWidth: 0
                text: view.asked === null ? "" : view.asked.followed ? view.words("pdf-link-ask", { host: view.asked.host, url: view.asked.url }) : view.words("pdf-link-refused", { url: view.asked.url })
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                maximumLineCount: 3
                elide: Text.ElideRight
                color: view.theme ? view.theme.text : "black"
            }
            Button {
                objectName: "pdfLinkOpen"
                visible: view.asked !== null && view.asked.followed
                text: view.plain(view.words("pdf-link-open"))
                onClicked: view.open()
            }
            Button {
                objectName: "pdfLinkCancel"
                flat: true
                text: view.plain(view.words(view.asked !== null && view.asked.followed ? "ui-cancel" : "ui-close"))
                onClicked: view.asked = null
            }
        }
    }
}
