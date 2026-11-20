// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The site named above an extension's sign-in page (SignInWindow.qml,
// docs/extensions.md, "A sign-in"): the host of the last page drawn, so that
// you see where a password is typed. Never the host of a navigation still
// under way: Chromium shows the address of a navigation the window itself
// starts (a pop-up turned into the window's own page) before its page comes,
// and the page drawn meanwhile is still the old one. So the name follows a
// load of the main frame that succeeded, and while a navigation is under way
// the page is covered and its name is "…". A load that failed stays covered
// (the page drawn may be the old one or an error page); one stopped leaves
// the old page, and its name, as they were.
//
// The name is said in its ASCII form (`xn--…` for a label written in other
// letters: Qt prints an internationalized name in Unicode, where a look-alike
// of a platform's name would read as the real one), and the window cuts it on
// the left, so that its end, the registrable part, stays in sight. The
// address watched for counts only from the main frame, and only when what
// follows it ends its scheme and host (`watched`).

pragma ComponentBehavior: Bound

import QtQml

QtObject {
    id: site

    // The host of the last page that came in the main frame; "" before the first.
    property string host: ""
    // A navigation under way: the page is covered, its name not said.
    property bool pending: true
    // The last navigation failed: the page stays covered.
    property bool failed: false
    // The page is covered, and nothing can be typed into it.
    readonly property bool covered: site.pending || site.failed || site.host === ""
    // What is said above the page.
    readonly property string shown: site.covered ? "…" : site.host

    // The host of an address, with its port when it names one: no scheme, no
    // user, lower case, each label in ASCII.
    function hostOf(address) {
        const rest = String(address).replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
        const authority = rest.split(/[\/?#]/)[0].replace(/^.*@/, "").toLowerCase()
        const colon = authority.lastIndexOf(":")
        const port = colon >= 0 && /^:[0-9]*$/.test(authority.slice(colon)) ? authority.slice(colon) : ""
        const name = authority.slice(0, authority.length - port.length)
        return name.split(".").map(label => /^[\x00-\x7f]*$/.test(label) ? label : "xn--" + site.punycode(label)).join(".") + port
    }

    // A label in Punycode (RFC 3492, section 6.3), as a name's ASCII form writes it after "xn--".
    function punycode(label) {
        const base = 36, tMin = 1, tMax = 26
        const codes = Array.from(label).map(c => c.codePointAt(0))
        const digit = d => String.fromCharCode(d < 26 ? d + 97 : d + 22)
        const adapt = (delta, points, first) => {
            delta = first ? Math.floor(delta / 700) : Math.floor(delta / 2)
            delta += Math.floor(delta / points)
            let k = 0
            while (delta > ((base - tMin) * tMax) / 2) {
                delta = Math.floor(delta / (base - tMin))
                k += base
            }
            return k + Math.floor(((base - tMin + 1) * delta) / (delta + 38))
        }
        let out = codes.filter(c => c < 0x80).map(c => String.fromCharCode(c)).join("")
        const basic = out.length
        if (basic > 0)
            out += "-"
        let handled = basic, n = 128, delta = 0, bias = 72
        while (handled < codes.length) {
            const m = Math.min(...codes.filter(c => c >= n))
            delta += (m - n) * (handled + 1)
            n = m
            for (const c of codes) {
                if (c < n)
                    delta++
                if (c !== n)
                    continue
                let q = delta
                for (let k = base; ; k += base) {
                    const t = k <= bias ? tMin : (k >= bias + tMax ? tMax : k - bias)
                    if (q < t)
                        break
                    out += digit(t + ((q - t) % (base - t)))
                    q = Math.floor((q - t) / (base - t))
                }
                out += digit(q)
                bias = adapt(delta, handled + 1, handled === basic)
                delta = 0
                handled++
            }
            delta++
            n++
        }
        return out
    }

    // Whether `address` is the one watched for: it begins with `watch`, and
    // what follows ends its scheme and host (nothing, a path, a query, a fragment).
    function watched(address, watch) {
        const text = String(address)
        if (!watch || !text.startsWith(watch))
            return false
        return text.length === watch.length || "/?#".includes(text.charAt(watch.length))
    }

    // A load of the main frame: "started", "succeeded", "failed" or "stopped", at `address`.
    function loading(status, address) {
        switch (status) {
        case "started":
            site.pending = true
            site.failed = false
            break
        case "succeeded":
            site.host = site.hostOf(address)
            site.pending = false
            site.failed = false
            break
        case "failed":
            site.pending = false
            site.failed = true
            break
        case "stopped":
            // Stopped before its page came: the old page is still the one drawn.
            site.pending = false
            break
        }
    }
}
