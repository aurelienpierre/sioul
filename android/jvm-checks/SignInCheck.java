// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

/**
 * An extension's sign-in screen (SignIn.java): which navigations load, which
 * is given back, which are refused, and the site named above the page; the
 * same rule as the computer's SignInWindow.qml and the address the engine
 * gives back (extensions/engine.rs: only one that begins with the address
 * watched for). Its store of its own: the process it accepts as its own, the
 * folders it empties (its own, never the sites'), the answer the window
 * keeps; read from the repository, the manifest's process, the transfer's
 * rule, and the order that makes the store the sign-in's before anything is
 * emptied or loaded.
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
            { "skoapp-prod://sign-in-callback/x?code=OC-1", SignIn.Decision.WATCHED },
            { "skoapp-prod://sign-in-callback#code=OC-1", SignIn.Decision.WATCHED },
            { "skoapp-prod://sign-in-callback.other.example/?code=OC-1", SignIn.Decision.REFUSED },
            { "skoapp-prod://sign-in-callback-x?code=OC-1", SignIn.Decision.REFUSED },
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
        // A1, A2: only the screen Rust waits for shows a page: never one made again
        // after its process ended, nor one whose window no longer waits.
        Object[][] waited = {
            { false, true, true },
            { true, true, false },
            { false, false, false },
            { true, false, false },
        };
        for (Object[] w : waited) {
            if (SignIn.waitedFor((Boolean) w[0], (Boolean) w[1]) != (Boolean) w[2]) {
                failed++;
                System.out.println("FAIL waited for: made again " + w[0] + ", the window waits " + w[1]);
            }
        }
        for (boolean french : new boolean[] { true, false }) {
            String again = french ? "Se connecter…" : "Sign in…";
            if (!SignIn.interrupted(french).contains(again) || !SignIn.windowClosed(french).contains(again) || SignIn.noStore(french).isEmpty()) {
                failed++;
                System.out.println("FAIL what an interrupted sign-in says, French " + french);
            }
        }
        // What the window gives Rust: the address returned when it is the one
        // watched for, else "cancelled"; nothing else a screen says goes through.
        String[][] answers = {
            { "skoapp-prod://sign-in-callback?code=OC-1&state=s", watch, "returned" },
            { "skoapp-prod://sign-in-callback#code=OC-1", watch, "returned" },
            { null, watch, "cancelled" },
            { "", watch, "cancelled" },
            { "https://sso.example.org/after", watch, "cancelled" },
            { "skoapp-prod://sign-in-callback.other.example/?code=OC-1", watch, "cancelled" },
            { "skoapp-prod://sign-in-callback?code=OC-1", "", "cancelled" },
            { "skoapp-prod://sign-in-callback?code=OC-1", null, "cancelled" },
        };
        for (String[] a : answers) {
            String said = SignIn.answer(a[0], a[1]);
            boolean right;
            try {
                org.json.JSONObject got = new org.json.JSONObject(said);
                right = a[2].equals("returned") ? got.length() == 1 && a[0].equals(got.optString("returned", null)) : got.length() == 1 && got.optBoolean("cancelled", false);
            } catch (org.json.JSONException e) {
                right = false;
            }
            if (!right) {
                failed++;
                System.out.println("FAIL the window's answer for " + a[0] + " -> " + said);
            }
        }
        // The store of its own: only the screen's process is taken as its own, never the window's
        // (where the sites' store is) nor another of Sioul's.
        String pkg = "com.aurelienpierre.sioul";
        Object[][] processes = {
            { pkg + ":" + SignIn.PROCESS, pkg, true },
            { pkg, pkg, false },
            { pkg + ":steps", pkg, false },
            { pkg + ":listener", pkg, false },
            { pkg + ":" + SignIn.PROCESS + "2", pkg, false },
            { "org.example.other:" + SignIn.PROCESS, pkg, false },
            { ":" + SignIn.PROCESS, "", false },
            { null, pkg, false },
            { pkg + ":" + SignIn.PROCESS, null, false },
        };
        for (Object[] p : processes) {
            if (SignIn.ownProcess((String) p[0], (String) p[1]) != (Boolean) p[2]) {
                failed++;
                System.out.println("FAIL own process: " + p[0] + " of " + p[1]);
            }
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
            String code = code(source);
            String rules = code(java.nio.file.Files.readString(root.resolve("android/package/res/xml/sioul_data_extraction_rules.xml")));
            java.util.regex.Matcher declared = java.util.regex.Pattern.compile("android:process=\"([^\"]*)\"").matcher(activity);
            String process = declared.find() ? declared.group(1) : "";
            java.util.regex.Matcher named = java.util.regex.Pattern.compile("<manifest[^>]*\\spackage=\"([^\"]+)\"").matcher(manifest);
            String manifestPackage = named.find() ? named.group(1) : "";
            String transfer = between(rules, "<device-transfer>", "</device-transfer>");
            String backup = between(rules, "<cloud-backup>", "</cloud-backup>");
            String own = SignIn.folders(new java.io.File("data"), new java.io.File("cache"))[0].getName() + "/";
            String create = body(code, "protected void onCreate(Bundle saved)", "    ");
            String forget = body(code, "private void forget()", "    ");
            String clean = body(code, "private void clean()", "    ");
            String store = body(code, "private boolean store()", "    ");
            int stored = create.indexOf("own = store();");
            boolean emptiedInForget = true;
            for (String empties : new String[] { "removeAllCookies", "deleteAllData", "clearCache(true)", "deleteBrowsingData" })
                emptiedInForget &= count(code, empties) == count(forget, empties);
            boolean oneWay = count(code, ".transact(") == count(code, ", IBinder.FLAG_ONEWAY);") && count(code, ".transact(") >= 1;
            String[][] expected = {
                { "the screen is declared", String.valueOf(!activity.isEmpty()) },
                { "seen in Recents", String.valueOf(!activity.contains("excludeFromRecents")) },
                { "in Sioul's own task", String.valueOf(!activity.contains("taskAffinity")) },
                { "never exported", String.valueOf(activity.contains("android:exported=\"false\"")) },
                { "a renderer that ends is handled", String.valueOf(source.contains("public boolean onRenderProcessGone(") && source.contains("return true;")) },
                { "resized by the keyboard: the site's name in sight", String.valueOf(activity.contains("android:windowSoftInputMode=\"adjustResize\"")) },
                { "the watched address from the main frame only", String.valueOf(source.contains("case WATCHED:\n                    // From the main frame only") && source.contains("if (request.isForMainFrame())\n                        returned(url);")) },
                { "Sioul's language, not the phone's", String.valueOf(source.contains("getIntent().getBooleanExtra(FRENCH, false)") && !source.contains("getLocales()")) },
                // A store of its own: the process the manifest gives the screen is the one it takes as its own, and it is alone there.
                { "in the process the screen takes as its own", String.valueOf(manifestPackage.equals(pkg) && SignIn.ownProcess(manifestPackage + process, manifestPackage)) },
                { "alone in that process", String.valueOf(count(code(manifest), "android:process=\"" + process + "\"") == 1) },
                { "its folder out of a transfer", String.valueOf(transfer.contains("<exclude domain=\"root\" path=\"" + own + "\" />")) },
                { "and out of a cloud backup", String.valueOf(backup.contains("<exclude domain=\"root\" />")) },
                { "the store made its own before anything else in onCreate", String.valueOf(stored > 0 && stored < create.indexOf("clean();") && stored < create.indexOf("new WebView(") && stored < create.indexOf("CookieManager.") && stored < create.indexOf("send(window")) },
                { "nothing emptied nor shown unless it is its own", String.valueOf(create.indexOf("if (!own) {") > stored && create.indexOf("if (!own) {") < create.indexOf("clean();") && forget.contains("if (!own)\n            return;") && clean.contains("if (!own)\n            return;")) },
                { "the web view's own emptying only in forget()", String.valueOf(emptiedInForget && count(code, "wipe(folders(") == count(clean, "wipe(folders(") && count(code, "getDir(") == 0) },
                { "the folders named once a process, before any web view, a second screen finding them named", String.valueOf(count(code, "setDataDirectorySuffix(") == 1 && store.indexOf("if (!apart) {") >= 0 && store.indexOf("if (!apart) {") < store.indexOf("WebView.setDataDirectorySuffix(PROCESS);") && store.contains("catch (IllegalStateException") && store.indexOf("ownProcess(") < store.indexOf("setDataDirectorySuffix(")) },
                { "each door hears Sioul's own processes alone", String.valueOf(count(code, "protected boolean onTransact(") == 2 && count(code, "if (Binder.getCallingUid() != Process.myUid()") == 2) },
                { "every call one way, never waiting on a frozen process", String.valueOf(oneWay && !code.contains("pingBinder")) },
                { "Rust told from the window's door alone", String.valueOf(count(code, "nativeReturned(json)") == 1 && body(code, "private void answer(String json)", "        ").contains("nativeReturned(json)")) },
                { "each side hears the other's process end", String.valueOf(code.contains("door.linkToDeath(this, 0);") && code.contains("window.linkToDeath(ended, 0);") && code.contains("window.unlinkToDeath(ended, 0);")) },
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
        // A5: the sign-in's own folders emptied (its data folder and its cache), themselves kept,
        // a link taken out and never followed; the sites' folders beside them untouched.
        try {
            java.nio.file.Path base = java.nio.file.Files.createTempDirectory("sioul-signin-wipe");
            java.nio.file.Path data = java.nio.file.Files.createDirectories(base.resolve("data"));
            java.nio.file.Path cache = java.nio.file.Files.createDirectories(base.resolve("cache"));
            java.io.File[] folders = SignIn.folders(data.toFile(), cache.toFile());
            java.nio.file.Path mine = folders[0].toPath();
            java.nio.file.Path mineCache = folders[1].toPath();
            java.nio.file.Files.writeString(java.nio.file.Files.createDirectories(mine.resolve("Default/Service Worker/CacheStorage")).resolve("entry"), "kept by a sign-in");
            java.nio.file.Files.writeString(java.nio.file.Files.createDirectories(mine.resolve("Default")).resolve("Cookies"), "a sign-in's cookie");
            java.nio.file.Files.writeString(java.nio.file.Files.createDirectories(mineCache.resolve("Default/HTTP Cache")).resolve("entry"), "a sign-in's page");
            // The sites' store, the web view's own in the window's process (SiteScreen.java).
            java.nio.file.Path sites = java.nio.file.Files.createDirectories(data.resolve("app_webview/Default"));
            java.nio.file.Files.writeString(sites.resolve("Cookies"), "a site's cookie");
            java.nio.file.Path sitesCache = java.nio.file.Files.createDirectories(cache.resolve("WebView/Default/HTTP Cache"));
            java.nio.file.Files.writeString(sitesCache.resolve("entry"), "a site's page");
            java.nio.file.Path outside = java.nio.file.Files.createDirectories(base.resolve("outside"));
            java.nio.file.Files.writeString(outside.resolve("mine"), "not the web view's");
            java.nio.file.Files.createSymbolicLink(mine.resolve("link"), outside);
            SignIn.wipe(folders);
            SignIn.wipe(SignIn.folders(base.resolve("absent").toFile(), base.resolve("absent").toFile()));
            read += 4;
            if (mine.toFile().list().length != 0 || mineCache.toFile().list().length != 0 || !java.nio.file.Files.isDirectory(mine) || !java.nio.file.Files.isDirectory(mineCache)) {
                failed++;
                System.out.println("FAIL the sign-in's folders emptied, themselves kept");
            }
            if (!java.nio.file.Files.exists(outside.resolve("mine"))) {
                failed++;
                System.out.println("FAIL a link followed out of the web view's folder");
            }
            if (!java.nio.file.Files.readString(sites.resolve("Cookies")).equals("a site's cookie") || !java.nio.file.Files.exists(sitesCache.resolve("entry"))) {
                failed++;
                System.out.println("FAIL the sites' store touched by the sign-in");
            }
            if (!mine.getParent().equals(data) || !mineCache.getParent().equals(cache) || mine.endsWith("app_webview") || mineCache.endsWith("WebView")) {
                failed++;
                System.out.println("FAIL the sign-in's folders: " + mine + ", " + mineCache);
            }
            SignIn.wipe(new java.io.File[] { base.toFile() });
            java.nio.file.Files.deleteIfExists(base);
        } catch (java.io.IOException | UnsupportedOperationException e) {
            System.out.println("the wipe not checked here: " + e);
        }
        int total = cases.length + hosts.length + 2 + waited.length + 2 + answers.length + processes.length + read;
        System.out.println(failed == 0 ? "all " + total + " sign-in cases agree" : failed + " failed");
        System.exit(failed == 0 ? 0 : 1);
    }

    /** The text with its comments taken out, so that only code is searched. */
    static String code(String text)
    {
        return text.replaceAll("(?s)/\\*.*?\\*/", "").replaceAll("(?m)^\\s*//.*$", "").replaceAll("(?s)<!--.*?-->", "");
    }

    /** The body of `signature` in `text`, to its closing brace at its indentation; "" when it is not there. */
    static String body(String text, String signature, String indent)
    {
        int start = text.indexOf(signature);
        int end = start < 0 ? -1 : text.indexOf("\n" + indent + "}\n", start);
        return start < 0 || end < 0 ? "" : text.substring(start, end);
    }

    /** What lies between `open` and `close` in `text`; "" when either is missing. */
    static String between(String text, String open, String close)
    {
        int start = text.indexOf(open);
        int end = start < 0 ? -1 : text.indexOf(close, start);
        return start < 0 || end < 0 ? "" : text.substring(start, end);
    }

    /** How many times `what` is in `text`. */
    static int count(String text, String what)
    {
        return text.split(java.util.regex.Pattern.quote(what), -1).length - 1;
    }
}
