// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// Sioul's own SiteDownloads.qml and DownloadCard.qml on real Qt WebEngine
// requests (tools/web-downloads-probe/run.sh, by hand, never in CI): a stand-in
// site on 127.0.0.1 offers files to a page in a profile made as Sioul makes its
// own (SitesWeb.qml), and this page hands each request over as the Sites page
// does (SitesPage.qml: its view read once, then never). The backend is a
// stand-in that hands out the held folders run.sh made (0700) and says each
// call. Scenarios: "held" and "trickle" (accepted at once, Save after 3 s);
// "view-cancel", "view-save", "view-next" (the page's view destroyed while the
// card waits, then Cancel or Save, Qt's crash of review 3.2); "many" (thirty
// files offered by script, no click).

import QtQuick
import QtWebEngine
import "../../crates/sioul-app/qml"

Window {
    id: win

    width: 700
    height: 860
    visible: true

    property string scenario: Qt.application.arguments[Qt.application.arguments.length - 3]
    property string base: Qt.application.arguments[Qt.application.arguments.length - 2]
    property string held: Qt.application.arguments[Qt.application.arguments.length - 1]
    property WebEngineProfile profile: null
    property int offered: 0

    function log(what) {
        console.warn("probe " + (Date.now() / 1000).toFixed(6) + " " + what)
    }

    Theme {
        id: probeTheme
    }

    // Sioul's backend, standing in: the held folders run.sh made, one per file, in turn.
    QtObject {
        id: backend

        property int holds: 0
        property string status: ""

        signal downloadFiled(string hold, string place, string id)
        signal downloadRefused(string hold, string problem)

        onStatusChanged: win.log("status: " + backend.status)

        function text(key) {
            return key
        }
        function textWith(key, name, value) {
            return key + " " + value
        }
        function isProgram(name) {
            return false
        }
        function papers() {
            return "{}"
        }
        function downloadHold(asked) {
            backend.holds += 1
            const name = JSON.parse(asked).name.replace(/[\/\\]/g, "_") || "download"
            win.log("hold #" + backend.holds + " for " + name)
            return JSON.stringify({ hold: win.held + "/held-" + backend.holds, held: name, problem: "" })
        }
        function downloadOffer(asked) {
            const place = id => ({ id: id, label: id, detail: "about " + id, enabled: true })
            return JSON.stringify({ name: JSON.parse(asked).name, host: "127.0.0.1", from: "from 127.0.0.1", waits: "download-waits", program: false, caution: "", places: [place("downloads"), place("folder"), place("notes"), place("papers"), place("letters")], first: "downloads", folder: "" })
        }
        function downloadStart(asked) {
            win.log("start " + asked)
            return JSON.stringify({ name: JSON.parse(asked).name, folder: "", problem: "" })
        }
        function downloadRoom(hold) {
            return -1
        }
        // As Rust does once it placed the file: the window hears it is over.
        function downloadDone(asked) {
            win.log("done " + asked)
            const hold = JSON.parse(asked).hold
            Qt.callLater(() => backend.downloadFiled(hold, "downloads", ""))
        }
        function downloadFailed(asked) {
            win.log("failed " + asked)
        }
        function downloadDrop(asked) {
            win.log("drop " + asked)
        }
        function downloadSaid(what, asked) {
            win.log("said " + what)
        }
    }

    Component {
        id: profileKind

        WebEngineProfilePrototype {
            storageName: "probe-sites"
            httpCacheType: WebEngineProfile.DiskHttpCache
            persistentCookiesPolicy: WebEngineProfile.ForcePersistentCookies
        }
    }

    Component.onCompleted: {
        win.profile = (profileKind.createObject(win) as WebEngineProfilePrototype).instance()
        // As the Sites page does: its view read here, once (SitesPage.qml).
        win.profile.downloadRequested.connect(download => {
            win.offered += 1
            const view = download.view
            win.log("requested #" + win.offered + " state=" + download.state + " name=" + download.suggestedFileName)
            downloads.offer(download, { id: "probe", name: "Probe" }, view && view.url ? String(view.url) : "", null)
            win.log("#" + win.offered + " after the offer: state=" + download.state + " folder=" + download.downloadDirectory)
        })
        holder.active = true
    }

    SiteDownloads {
        id: downloads

        anchors.fill: parent
        sioul: backend
        theme: probeTheme
    }

    Loader {
        id: holder

        anchors.fill: parent
        active: false
        z: -1
        sourceComponent: WebEngineView {
            profile: win.profile
            url: win.base + ({ "held": "/held.html", "trickle": "/trickle.html", "view-next": "/two.html", "many": "/many.html" }[win.scenario] || "/held.html")
        }
    }

    Timer {
        interval: 3000
        running: true
        onTriggered: {
            win.log("after 3 s: the card open " + (downloads.card !== null && downloads.card.opened) + ", files held " + downloads.entries.length)
            if (win.scenario.startsWith("view-")) {
                holder.active = false
                win.log("the view destroyed")
            } else if ((win.scenario === "held" || win.scenario === "trickle") && downloads.card) {
                win.log("Save pressed")
                downloads.card.save()
            }
        }
    }
    Timer {
        interval: 5000
        running: win.scenario.startsWith("view-")
        onTriggered: {
            if (win.scenario === "view-cancel") {
                win.log("Cancel pressed")
                downloads.card.cancel()
            } else {
                win.log("Save pressed")
                downloads.card.save()
            }
            win.log("still alive after the press")
        }
    }
    Timer {
        interval: 6000
        running: win.scenario === "view-next"
        onTriggered: {
            win.log("the second asked: " + (downloads.card !== null && downloads.card.opened) + "; Save pressed")
            downloads.card.save()
            win.log("still alive after the second press")
        }
    }
    Timer {
        interval: 5000
        running: win.scenario === "many"
        onTriggered: win.log("many: offered " + win.offered + ", held " + downloads.entries.length + ", the card open " + (downloads.card !== null && downloads.card.opened))
    }
    Timer {
        interval: 8000
        running: true
        onTriggered: {
            win.log("end: files held " + downloads.entries.length)
            Qt.quit()
        }
    }
}
