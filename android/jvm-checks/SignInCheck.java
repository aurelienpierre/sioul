// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

/**
 * An extension's sign-in screen (SignIn.java): which navigations load, which
 * is given back, which are refused, and the site named above the page; the
 * same rule as the computer's SignInWindow.qml and the address the engine
 * gives back (extensions/engine.rs: only one that begins with the address watched for).
 */
public final class SignInCheck
{
    public static void main(String[] args)
    {
        String watch = "skoapp-prod://sign-in-callback";
        Object[][] cases = {
            { "https://sso.example.org/oidc/authorize?client_id=x", SignIn.Decision.ALLOWED },
            { "https://educonnect.example.org/login", SignIn.Decision.ALLOWED },
            { "skoapp-prod://sign-in-callback?code=OC-1&state=s", SignIn.Decision.WATCHED },
            { "skoapp-prod://sign-in-callback", SignIn.Decision.WATCHED },
            { "skoapp-prod://elsewhere?code=OC-1", SignIn.Decision.REFUSED },
            { "http://sso.example.org/login", SignIn.Decision.REFUSED },
            { "file:///sdcard/Download/page.html", SignIn.Decision.REFUSED },
            { "content://com.example.provider/page", SignIn.Decision.REFUSED },
            { "javascript:alert(1)", SignIn.Decision.REFUSED },
            { "data:text/html,<p>hi</p>", SignIn.Decision.REFUSED },
            { "intent://scan#Intent;scheme=zxing;end", SignIn.Decision.REFUSED },
            { "about:blank", SignIn.Decision.REFUSED },
            { null, SignIn.Decision.REFUSED },
        };
        int failed = 0;
        for (Object[] c : cases) {
            SignIn.Decision got = SignIn.decide((String) c[0], watch);
            if (got != c[1]) {
                failed++;
                System.out.println("FAIL " + c[0] + " -> " + got + " (expected " + c[1] + ")");
            }
        }
        // Nothing is watched for with no address: a page alone, https only.
        if (SignIn.decide("skoapp-prod://sign-in-callback?code=x", "") != SignIn.Decision.REFUSED) {
            failed++;
            System.out.println("FAIL an empty watch gives nothing back");
        }
        String[][] hosts = {
            { "https://SSO.Example.org/oidc/authorize?x=1", "sso.example.org" },
            { "https://user@sso.example.org:443/x", "sso.example.org:443" },
            { "https://sso.example.org#frag", "sso.example.org" },
            { "skoapp-prod://sign-in-callback?code=x", "sign-in-callback" },
        };
        for (String[] h : hosts) {
            if (!SignIn.host(h[0]).equals(h[1])) {
                failed++;
                System.out.println("FAIL host " + h[0] + " -> " + SignIn.host(h[0]));
            }
        }
        if (!SignIn.said("sso.example.org", true).equals("Vous vous connectez sur\u202f: sso.example.org") || !SignIn.said("sso.example.org", false).equals("You sign in on: sso.example.org")) {
            failed++;
            System.out.println("FAIL the line above the page");
        }
        int total = cases.length + hosts.length + 2;
        System.out.println(failed == 0 ? "all " + total + " sign-in cases agree" : failed + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
