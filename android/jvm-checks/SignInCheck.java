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
        // A1, A2: only the screen Rust waits for, in this process, shows a page.
        Object[][] waited = {
            { false, "https://sso.example.org/a", "https://sso.example.org/a", true },
            { true, "https://sso.example.org/a", "https://sso.example.org/a", false },
            { false, null, "https://sso.example.org/a", false },
            { false, "https://sso.example.org/b", "https://sso.example.org/a", false },
        };
        for (Object[] w : waited) {
            if (SignIn.waitedFor((Boolean) w[0], (String) w[1], (String) w[2]) != (Boolean) w[3]) {
                failed++;
                System.out.println("FAIL waited for: made again " + w[0] + ", asked " + w[1] + ", page " + w[2]);
            }
        }
        if (!SignIn.interrupted(true).contains("Se connecter…") || !SignIn.interrupted(false).contains("Sign in…")) {
            failed++;
            System.out.println("FAIL what an interrupted sign-in says");
        }
        // Read from the repository: the screen in Sioul's own task, seen in
        // Recents; a page's renderer that ends ends the sign-in, never Sioul.
        int read = 0;
        try {
            java.nio.file.Path root = java.nio.file.Path.of(System.getProperty("sioul.root"));
            String manifest = java.nio.file.Files.readString(root.resolve("android/package/AndroidManifest.xml"));
            int at = manifest.indexOf("android:name=\"com.aurelienpierre.sioul.SignIn\"");
            String activity = at < 0 ? "" : manifest.substring(manifest.lastIndexOf("<activity", at), manifest.indexOf("/>", at));
            String source = java.nio.file.Files.readString(root.resolve("android/package/src/com/aurelienpierre/sioul/SignIn.java"));
            String[][] expected = {
                { "the screen is declared", String.valueOf(!activity.isEmpty()) },
                { "seen in Recents", String.valueOf(!activity.contains("excludeFromRecents")) },
                { "in Sioul's own task", String.valueOf(!activity.contains("taskAffinity")) },
                { "never exported", String.valueOf(activity.contains("android:exported=\"false\"")) },
                { "a renderer that ends is handled", String.valueOf(source.contains("public boolean onRenderProcessGone(") && source.contains("return true;")) },
            };
            for (String[] e : expected) {
                read++;
                if (!e[1].equals("true")) {
                    failed++;
                    System.out.println("FAIL " + e[0]);
                }
            }
        } catch (java.io.IOException | RuntimeException e) {
            failed++;
            System.out.println("FAIL the repository's files: " + e);
        }
        int total = cases.length + hosts.length + 2 + waited.length + 1 + read;
        System.out.println(failed == 0 ? "all " + total + " sign-in cases agree" : failed + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }
}
