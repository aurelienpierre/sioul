// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

// The site named above an extension's sign-in page on a computer
// (SignInSite.qml, which SignInWindow.qml follows): the host of the last page
// that came in the main frame, never of a navigation still under way (the
// review's D1: a pop-up turned into the window's own navigation, toward a host
// that never answers, named that host while the old page stayed drawn).
// SignInWindow.qml itself needs Qt WebEngine, which this runner does not
// start; the window is tried by hand (docs/extensions.md). Run by
// tools/qml-test.sh.

import QtQuick
import QtTest
import "../../qml"

TestCase {
    id: test

    name: "SignInSite"

    Component {
        id: siteKind

        SignInSite {}
    }

    function test_nothing_is_named_before_a_page_came() {
        const site = createTemporaryObject(siteKind, test)
        compare(site.shown, "…")
        verify(site.covered)
        site.loading("started", "https://sso.example.org/oidc/authorize?client_id=x")
        compare(site.shown, "…", "the first page on its way")
        site.loading("succeeded", "https://SSO.example.org/oidc/authorize?client_id=x")
        compare(site.shown, "sso.example.org")
        verify(!site.covered)
    }

    function test_a_navigation_under_way_never_names_its_host() {
        const site = createTemporaryObject(siteKind, test)
        site.loading("started", "https://pages.example.net/")
        site.loading("succeeded", "https://pages.example.net/")
        compare(site.shown, "pages.example.net", "a page that came is named, whatever it is")
        // The page asks a pop-up toward the trusted host, on a port that never answers.
        site.loading("started", "https://sso.example.org:81/")
        compare(site.shown, "…", "never the host of a page not yet drawn")
        verify(site.covered, "the old page covered meanwhile")
        compare(site.host, "pages.example.net")
        // It never came: stopped, the old page and its name are back.
        site.loading("stopped", "https://sso.example.org:81/")
        compare(site.shown, "pages.example.net")
        verify(!site.covered)
    }

    // D2: a name in other letters is said in its ASCII form, never as a look-alike.
    function test_the_name_is_said_in_ascii() {
        const site = createTemporaryObject(siteKind, test)
        compare(site.punycode("bücher"), "bcher-kva")
        compare(site.punycode("münchen"), "mnchen-3ya")
        compare(site.hostOf("https://аpple.com/login"), "xn--pple-43d.com", "a Cyrillic а is not a Latin a")
        compare(site.hostOf("https://user@sso.bücher.example:8443/x"), "sso.xn--bcher-kva.example:8443")
        compare(site.hostOf("https://sso.example.org/a?b"), "sso.example.org")
        site.loading("succeeded", "https://аpple.com/login")
        compare(site.shown, "xn--pple-43d.com")
    }

    // D3: the address watched for is given back only when what follows it
    // ends its scheme and host.
    function test_the_watched_address_is_matched_exactly() {
        const site = createTemporaryObject(siteKind, test)
        const watch = "skoapp-prod://sign-in-callback"
        verify(site.watched(watch, watch))
        verify(site.watched(watch + "?code=x&state=y", watch))
        verify(site.watched(watch + "/x", watch))
        verify(site.watched(watch + "#a", watch))
        verify(!site.watched(watch + ".other.example/?code=x", watch), "another host that begins alike")
        verify(!site.watched(watch + "-x?code=x", watch))
        verify(!site.watched("https://sso.example.org/", watch))
        verify(!site.watched(watch, ""), "nothing watched, nothing given back")
    }

    function test_a_failed_page_stays_covered() {
        const site = createTemporaryObject(siteKind, test)
        site.loading("succeeded", "https://sso.example.org/login")
        site.loading("started", "https://user@idp.example.org:8443/x")
        site.loading("failed", "https://user@idp.example.org:8443/x")
        verify(site.failed && site.covered)
        compare(site.shown, "…")
        site.loading("started", "https://idp.example.org/again")
        verify(!site.failed && site.covered)
        site.loading("succeeded", "https://user@idp.example.org:8443/again")
        compare(site.shown, "idp.example.org:8443", "no user, the port kept")
    }
}
