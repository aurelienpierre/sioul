// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * A site's screen (SiteScreen.java, SiteWeb.java; docs/android.md, "Sites"),
 * on the JVM (android/jvm-checks/run.sh). What its one extra may carry
 * (SiteWeb.Opening.of): an https address with a host, an id and a name
 * without control characters and of reasonable length, a domain that is a
 * host's name, Sioul's colours or its light ones, only the words the screen
 * shows (each of Rust's among them, read from siteview.rs). Which addresses
 * load (SiteWeb.where): https pages and blank ones; a link tapped to another
 * site in the browser, a sign-in's redirects and a pop-up's pages kept; mail
 * and telephone addresses to their apps at a tap; http, files, content,
 * intents and other schemes refused. The host shown: the committed page's,
 * in its ASCII form, "…" before any. The rough domain as Rust's domain_of
 * reads it, the address kept for after a stop, the user agent without a
 * web view's marks, a downloaded file's name, the line of what a site asked
 * for, the features asked as Rust names them. Read from the sources: the
 * manifest's screen not exported, without an intent filter, kept through
 * rotations and resizes, above the keyboard, in Sioul's task and process;
 * the web view's diagnostics off; its folder out of a transfer to another
 * device; no JavaScript interface, no file or content access, no mixed
 * content, windows only at a tap, no debugging, no certificate error passed
 * over, the console never logged, no address nor cookie in a log line; the
 * camera, the microphone and the location refused; a drawing process that
 * ends handled; one extra read; StepService's door; the library in Gradle.
 * No Java of Sioul's empties every site's store: only the extensions'
 * sign-in screen empties one, its own, in a process apart. A login from
 * Bitwarden (SiteScreen's fill): the menu's two lines only with an account
 * set, the same-origin rule on addresses (scheme, host and port; http and
 * another host refused), the question Rust is asked; from the sources, one
 * evaluateJavascript in all of Sioul's Java, run after the address is
 * checked again, no JavaScript interface nor message channel anywhere, no
 * log line that could carry the call, the window brought forward and the
 * site back by reordering Sioul's task, Rust loaded before it is asked, the
 * answer taken as the screen resumes, main.cpp's bridge wiping it.
 */
public final class SiteScreenCheck
{
    static int failed = 0;
    static int checked = 0;

    static void expect(String what, boolean holds)
    {
        checked++;
        if (!holds) {
            failed++;
            System.out.println("FAIL " + what);
        }
    }

    /** The text with its comments taken out, so that only code is searched. */
    static String code(String text)
    {
        return text.replaceAll("(?s)/\\*.*?\\*/", "").replaceAll("(?m)//.*$", "").replaceAll("(?s)<!--.*?-->", "");
    }

    /** The body of `signature` in `text`, to its closing brace at its indentation; "" when it is not there. */
    static String body(String text, String signature, String indent)
    {
        int start = text.indexOf(signature);
        int end = start < 0 ? -1 : text.indexOf("\n" + indent + "}\n", start);
        return start < 0 || end < 0 ? "" : text.substring(start, end);
    }

    static String read(Path path) throws java.io.IOException
    {
        return new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
    }

    static String site(String change) throws org.json.JSONException
    {
        org.json.JSONObject asked = new org.json.JSONObject()
            .put("id", "bank")
            .put("name", "My bank")
            .put("url", "https://secure.bank.example.co.uk/login")
            .put("domain", "example.co.uk")
            .put("look", new org.json.JSONObject().put("dark", true).put("background", "#1e1f1c").put("surface", "#272824").put("text", "#E6E2DA").put("muted", "#a8a297").put("line", "#803b3b36"))
            .put("words", new org.json.JSONObject().put("browser", "Ouvrir dans le navigateur").put("asked", "Ce site a demandé {what}\u202f: pas dans cette version de Sioul.")
                .put("asked_camera", "la caméra").put("asked_location", "votre position").put("unknown", "never read").put("close", "x".repeat(601)));
        if (change != null) {
            org.json.JSONObject changed = new org.json.JSONObject(change);
            for (java.util.Iterator<String> keys = changed.keys(); keys.hasNext();) {
                String key = keys.next();
                asked.put(key, changed.get(key));
            }
        }
        return asked.toString();
    }

    public static void main(String[] args) throws Exception
    {
        // ------------------------------------------------ What the screen is given
        SiteWeb.Opening bank = SiteWeb.Opening.of(site(null));
        expect("a site Rust gives is taken", bank != null);
        expect("its id, name, address, domain", bank.id.equals("bank") && bank.name.equals("My bank") && bank.url.equals("https://secure.bank.example.co.uk/login") && bank.domain.equals("example.co.uk"));
        expect("Sioul's colours, dark", bank.dark && bank.background == 0xFF1E1F1C && bank.text == 0xFFE6E2DA && bank.line == 0x803B3B36);
        expect("Sioul's words, in its language", bank.word("browser").equals("Ouvrir dans le navigateur"));
        expect("a word longer than allowed: English instead", bank.word("close").equals("Close"));
        expect("a word not given: English", bank.word("reload").equals("Reload"));
        expect("a key the screen does not show: nothing", bank.word("unknown").isEmpty());
        SiteWeb.Opening plain = SiteWeb.Opening.of(site("{\"look\": {\"background\": \"red\", \"text\": \"#fff\"}}"));
        expect("colours that are not #RRGGBB: Sioul's light ones", plain != null && !plain.dark && plain.background == 0xFFF5F2EC && plain.text == 0xFF2D2A26);
        String[] refused = {
            "{\"id\": \"\"}", "{\"id\": \"a\\nb\"}", "{\"id\": \"" + "a".repeat(201) + "\"}", "{\"name\": \" \"}", "{\"name\": \"a\\u0007b\"}", "{\"name\": \"" + "n".repeat(201) + "\"}",
            "{\"url\": \"http://bank.example.org\"}", "{\"url\": \"https://\"}", "{\"url\": \"file:///sdcard/a.html\"}", "{\"url\": \"javascript:alert(1)\"}", "{\"url\": \"https://example.org/" + "a".repeat(4100) + "\"}",
            "{\"domain\": \"\"}", "{\"domain\": \"ex ample.org\"}", "{\"domain\": \"example.org/x\"}",
        };
        for (String change : refused)
            expect("refused: " + (change.length() > 60 ? change.substring(0, 60) + "…" : change), SiteWeb.Opening.of(site(change)) == null);
        SiteWeb.Opening other = SiteWeb.Opening.of(site("{\"url\": \"https://b\u00e4nk.example/\", \"domain\": \"b\u00e4nk.example\"}"));
        expect("a domain in other alphabets, in its ASCII form", other != null && other.domain.equals("xn--bnk-qla.example"));
        expect("no extra, not JSON, not an object: refused", SiteWeb.Opening.of(null) == null && SiteWeb.Opening.of("not json") == null && SiteWeb.Opening.of("[1]") == null);

        // ------------------------------------------------ A login from Bitwarden: the menu's lines, the address
        expect("no account said: the vault's lines hidden", !bank.vault);
        expect("an account set: the vault's lines shown", SiteWeb.Opening.of(site("{\"vault\": true}")).vault);
        expect("only a true that is JSON's own", !SiteWeb.Opening.of(site("{\"vault\": \"true\"}")).vault && !SiteWeb.Opening.of(site("{\"vault\": 1}")).vault && !SiteWeb.Opening.of(site("{\"vault\": false}")).vault);
        String signInPage = "https://secure.bank.example.co.uk/login?next=%2F";
        String[] sameOrigin = { "https://secure.bank.example.co.uk/", "https://secure.bank.example.co.uk/account#top", "HTTPS://Secure.Bank.Example.co.uk:443/x", "https://user@secure.bank.example.co.uk/" };
        for (String now : sameOrigin)
            expect("the same scheme, host and port: filled at " + now, SiteWeb.sameOrigin(signInPage, now));
        String[] otherOrigin = { "http://secure.bank.example.co.uk/login", "https://secure.bank.example.co.uk:8443/login", "https://bank.example.co.uk/login", "https://secure.bank.example.co.uk.example.net/", "https://look-alike.example.net/login", "https://look-alike.example.net/@secure.bank.example.co.uk", "about:blank", "", "javascript:alert(1)", "file:///sdcard/a.html", "https://secure.bank.example.co.uk:99999/", "https://secure.bank.example.co.uk:x/" };
        for (String now : otherOrigin)
            expect("another scheme, host or port: nothing filled at " + now, !SiteWeb.sameOrigin(signInPage, now));
        expect("asked on http: nothing, whatever follows", !SiteWeb.sameOrigin("http://secure.bank.example.co.uk/", "http://secure.bank.example.co.uk/") && !SiteWeb.sameOrigin(null, signInPage) && !SiteWeb.sameOrigin(signInPage, null));
        expect("a port read, 443 unsaid", SiteWeb.port("https://a.example.org/") == 443 && SiteWeb.port("https://a.example.org:8443/x") == 8443 && SiteWeb.port("https://[::1]:8080/") == 8080 && SiteWeb.port("https://[::1]/") == 443 && SiteWeb.port("http://a.example.org/") == -1 && SiteWeb.port("https://a.example.org:70000/") == -1);
        org.json.JSONObject askedFill = new org.json.JSONObject(SiteWeb.fillAsked(signInPage, "choose"));
        expect("Rust is asked the committed address and the mode, nothing else", askedFill.length() == 2 && askedFill.getString("url").equals(signInPage) && askedFill.getString("mode").equals("choose"));
        org.json.JSONObject takenFill = new org.json.JSONObject(SiteWeb.fillAsked(signInPage, null));
        expect("and for the window's answer, the address alone", takenFill.length() == 1 && takenFill.getString("url").equals(signInPage));
        expect("the vault's words, in English when Rust gives none", bank.word("fill").equals("Fill from Bitwarden") && bank.word("choose_login").equals("Choose a login\u2026") && !bank.word("fill_changed").isEmpty() && !bank.word("fill_https").isEmpty());
        String sitefill = read(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-app", "src", "sitefill.rs"));
        expect("Rust's same-origin test holds the same rule (sitefill.rs)", sitefill.contains("fn the_same_origin_is_the_scheme_the_host_and_the_port()") && sitefill.contains("\"https://bank.example.org:8443/login\"") && sitefill.contains("\"http://bank.example.org/login\""));

        // ------------------------------------------------ The host shown
        expect("the host alone, no user, no port, lower case", SiteWeb.host("https://user:pw@Secure.Bank.Example.org:8443/a?b#c").equals("secure.bank.example.org"));
        expect("a backslash ends the host", SiteWeb.host("https://bank.example.org\\@evil.example.net/").equals("bank.example.org"));
        expect("an address in numbers", SiteWeb.host("https://[::1]:8080/").equals("[::1]"));
        expect("before a page has committed: …", SiteWeb.shownHost("").equals("…") && SiteWeb.shownHost(null).equals("…"));
        expect("a blank page committed says so (review N3)", SiteWeb.shownHost("about:blank").equals("about:blank") && SiteWeb.shownHost("About:Blank#x").equals("about:blank") && SiteWeb.shownHost("about:srcdoc").equals("about:srcdoc"));
        expect("the committed page's host", SiteWeb.shownHost("https://accounts.example.org/sign-in").equals("accounts.example.org"));
        String unicode = SiteWeb.shownHost("https://b\u00e4nk.example.org/");
        expect("a name in other alphabets in its ASCII form: " + unicode, unicode.startsWith("xn--") && unicode.endsWith(".example.org"));

        // ------------------------------------------------ The bar's two lines, always true (review N2)
        String from = "from {name}";
        SiteWeb.Bar onSite = SiteWeb.bar("My bank", "example.co.uk", "https://secure.bank.example.co.uk/home", from);
        expect("on the site: its name, then the host", onSite.first.equals("My bank") && onSite.second.equals("secure.bank.example.co.uk") && !onSite.hostFirst);
        SiteWeb.Bar elsewhere = SiteWeb.bar("My bank", "example.co.uk", "https://look-alike.example.net/login", from);
        expect("elsewhere: the host first, the site's name never above it", elsewhere.first.equals("look-alike.example.net") && elsewhere.second.equals("from My bank") && elsewhere.hostFirst);
        SiteWeb.Bar lookAlike = SiteWeb.bar("My bank", "example.co.uk", "https://secure.bank.example.co.uk.example.net/", from);
        expect("a host that only starts like the site's is elsewhere", lookAlike.hostFirst && lookAlike.first.equals("secure.bank.example.co.uk.example.net"));
        SiteWeb.Bar sameCountry = SiteWeb.bar("My bank", "example.co.uk", "https://bank.example.uk/", from);
        expect("another registrable domain under the same country is elsewhere", sameCountry.hostFirst);
        SiteWeb.Bar blank = SiteWeb.bar("My bank", "example.co.uk", "about:blank", from);
        expect("a blank page: said as such, from the site", blank.first.equals("about:blank") && blank.second.equals("from My bank") && blank.hostFirst);
        SiteWeb.Bar none = SiteWeb.bar("My bank", "example.co.uk", "", from);
        expect("before any page: the name, then …", none.first.equals("My bank") && none.second.equals("…") && !none.hostFirst);
        SiteWeb.Bar idn = SiteWeb.bar("Ma banque", "xn--bnk-qla.example", "https://www.b\u00e4nk.example/", "depuis {name}");
        expect("a domain in other alphabets compared in its ASCII form", !idn.hostFirst && idn.second.equals("www.xn--bnk-qla.example"));
        SiteWeb.Bar spoof = SiteWeb.bar("Ma banque", "xn--bnk-qla.example", "https://www.bank.example/", "depuis {name}");
        expect("a look-alike in plain letters is elsewhere", spoof.hostFirst && spoof.second.equals("depuis Ma banque"));
        expect("the word given by Rust: from", bank.word("from").equals("from {name}"));

        // ------------------------------------------------ The domain, as Rust's domain_of
        String[][] domains = { { "secure.bank.example.co.uk", "example.co.uk" }, { "id.example.gouv.fr", "example.gouv.fr" }, { "mail.example.me", "example.me" }, { "example.me", "example.me" }, { "localhost", "localhost" }, { "a.b.c.example.org", "example.org" }, { "www.example-public.fr", "example-public.fr" } };
        for (String[] pair : domains)
            expect("the domain of " + pair[0], SiteWeb.domainOf(pair[0]).equals(pair[1]));
        String rust = read(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-app", "src", "siteview.rs"));
        for (String[] pair : domains)
            expect("Rust's test holds the same domain: " + pair[0], rust.contains("(\"" + pair[0] + "\", \"" + pair[1] + "\")"));

        // ------------------------------------------------ Where each navigation goes
        String bankPage = "https://secure.bank.example.co.uk/home";
        String domain = "example.co.uk";
        Object[][] ways = {
            // url, main frame, gesture, redirect, current, window, way (the site's page as the opener)
            { "https://www.example.co.uk/help", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "https://news.example.net/article", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.BROWSER },
            { "https://idp.example.net/authorize", true, false, true, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "https://idp.example.net/authorize", true, false, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "https://idp.example.net/choose", true, true, false, "https://idp.example.net/authorize", SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "https://secure.bank.example.co.uk/back", true, true, false, "https://idp.example.net/done", SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "https://news.example.net/article", true, true, false, "https://accounts.example.com/x", SiteWeb.Window.POPUP, SiteWeb.Way.LOAD },
            { "https://frame.example.net/pay", false, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "HTTPS://News.Example.net/", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.BROWSER },
            { "http://old.example.net/", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.BROWSER },
            { "http://old.example.net/", true, false, true, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "http://old.example.net/", false, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "mailto:help@example.co.uk", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.OTHER_APP },
            { "tel:+33100000000", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.OTHER_APP },
            { "mailto:help@example.co.uk", true, false, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "tel:+33100000000", false, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "intent://scan/#Intent;scheme=zxing;end", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "file:///sdcard/Download/a.pdf", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "content://com.example.provider/a", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "javascript:alert(1)", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "sioul-open:notice", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "market://details?id=x", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "about:blank", true, false, false, "", SiteWeb.Window.POPUP, SiteWeb.Way.LOAD },
            { "about:srcdoc", false, false, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "data:text/html,<p>x</p>", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "data:text/html,<p>x</p>", false, false, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "blob:https://secure.bank.example.co.uk/1", false, false, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.LOAD },
            { "blob:https://secure.bank.example.co.uk/1", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { "https://", true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            { null, true, true, false, bankPage, SiteWeb.Window.SITE, SiteWeb.Way.REFUSE },
            // A new tab (review N1): its first page on another site leaves as a tapped link does, redirects included.
            { "https://news.example.net/article", true, false, false, "", SiteWeb.Window.TAB, SiteWeb.Way.BROWSER },
            { "https://news.example.net/article", true, true, false, "about:blank", SiteWeb.Window.TAB, SiteWeb.Way.BROWSER },
            { "https://news.example.net/article", true, false, true, "", SiteWeb.Window.TAB, SiteWeb.Way.BROWSER },
            { "https://www.example.co.uk/help", true, false, false, "", SiteWeb.Window.TAB, SiteWeb.Way.LOAD },
            { "https://www.example.co.uk/statement", true, false, true, "", SiteWeb.Window.TAB, SiteWeb.Way.LOAD },
            { "http://old.example.net/", true, false, false, "", SiteWeb.Window.TAB, SiteWeb.Way.BROWSER },
            { "mailto:help@example.co.uk", true, false, false, "", SiteWeb.Window.TAB, SiteWeb.Way.OTHER_APP },
            { "intent://scan/#Intent;scheme=zxing;end", true, false, false, "", SiteWeb.Window.TAB, SiteWeb.Way.REFUSE },
            { "about:blank", true, false, false, "", SiteWeb.Window.TAB, SiteWeb.Way.LOAD },
            { "https://frame.example.net/ad", false, false, false, "", SiteWeb.Window.TAB, SiteWeb.Way.LOAD },
            // A tab with a page of its own: as the site's page.
            { "https://news.example.net/article", true, true, false, "https://www.example.co.uk/help", SiteWeb.Window.TAB, SiteWeb.Way.BROWSER },
            { "https://idp.example.net/authorize", true, false, true, "https://www.example.co.uk/help", SiteWeb.Window.TAB, SiteWeb.Way.LOAD },
            // A pop-up with a size (a sign-in's) keeps every https page.
            { "https://idp.example.net/authorize", true, false, false, "", SiteWeb.Window.POPUP, SiteWeb.Way.LOAD },
            { "https://idp.example.net/choose", true, true, false, "https://idp.example.net/authorize", SiteWeb.Window.POPUP, SiteWeb.Way.LOAD },
        };
        for (Object[] way : ways) {
            SiteWeb.Way found = SiteWeb.where((String) way[0], (Boolean) way[1], (Boolean) way[2], (Boolean) way[3], (String) way[4], bankPage, domain, (SiteWeb.Window) way[5]);
            expect(way[0] + " (main " + way[1] + ", tap " + way[2] + ", redirect " + way[3] + ", from " + way[4] + ", " + way[5] + "): " + way[6] + ", not " + found, found == way[6]);
        }

        expect("a new tab's first page on the opener's own site stays", SiteWeb.where("https://idp.example.net/next", true, false, false, "", "https://idp.example.net/authorize", domain, SiteWeb.Window.TAB) == SiteWeb.Way.LOAD);
        expect("a window still empty: nothing https committed", SiteWeb.isEmptyWindow("") && SiteWeb.isEmptyWindow("about:blank") && SiteWeb.isEmptyWindow(null) && !SiteWeb.isEmptyWindow("https://www.example.co.uk/help"));

        // ------------------------------------------------ Kept, the user agent, a file's name, what was asked
        expect("the page shown kept for after a stop", "https://secure.bank.example.co.uk/home?x=1".equals(SiteWeb.kept("https://secure.bank.example.co.uk/home?x=1")));
        expect("never another scheme, nor a long one", SiteWeb.kept("http://example.org") == null && SiteWeb.kept("about:blank") == null && SiteWeb.kept(null) == null && SiteWeb.kept("https://example.org/" + "a".repeat(8200)) == null);
        String agent = "Mozilla/5.0 (Linux; Android 14; Tablet Build/AP2A; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/142.0.7444.90 Safari/537.36";
        String cleaned = SiteWeb.userAgent(agent);
        expect("the user agent says it is a web view, without the device's model (review S1): " + cleaned, cleaned.equals("Mozilla/5.0 (Linux; Android 14; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/142.0.7444.90 Safari/537.36"));
        expect("a tablet's agent without wv keeps its Android, drops its model", SiteWeb.userAgent("Mozilla/5.0 (Linux; Android 9; Tablet-9 Build/PPR1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36").startsWith("Mozilla/5.0 (Linux; Android 9) AppleWebKit"));
        expect("an agent that names no Android: unchanged", SiteWeb.userAgent("Mozilla/5.0 (X11; Linux x86_64)").equals("Mozilla/5.0 (X11; Linux x86_64)") && SiteWeb.userAgent(null) == null);
        String[][] names = { { "../../etc/passwd", "passwd" }, { "..\\..\\x.pdf", "x.pdf" }, { ".hidden", "hidden" }, { "a:b*c?.pdf", "a_b_c_.pdf" }, { "report\n.pdf", "report_.pdf" }, { "", "download" }, { null, "download" }, { "...", "download" }, { "Relevé de compte.pdf", "Relevé de compte.pdf" } };
        for (String[] pair : names)
            expect("a file's name: " + pair[0] + " → " + SiteWeb.fileName(pair[0]), SiteWeb.fileName(pair[0]).equals(pair[1]));
        String long_ = SiteWeb.fileName("a".repeat(300) + ".pdf");
        expect("a long name cut, its extension kept", long_.length() == 120 && long_.endsWith(".pdf"));
        String wide = SiteWeb.fileName("\u00e9".repeat(119) + ".pdf");
        expect("a name cut to 240 bytes in UTF-8, between characters (section 10): " + wide.length(), wide.getBytes(StandardCharsets.UTF_8).length <= 240 && wide.endsWith(".pdf") && wide.startsWith("\u00e9\u00e9"));
        String emoji = SiteWeb.fileName("\ud83d\udcc4".repeat(70) + ".pdf");
        expect("never half a character", emoji.getBytes(StandardCharsets.UTF_8).length <= 240 && !Character.isHighSurrogate(emoji.charAt(emoji.length() - 5)) && emoji.endsWith(".pdf"));
        // The chooser's types (review F5): extensions as their types, none for any.
        java.util.function.Function<String, String> map = ext -> ext.equals("pdf") ? "application/pdf" : ext.equals("jpg") ? "image/jpeg" : null;
        expect("extensions as their types", java.util.Arrays.equals(SiteWeb.acceptTypes(new String[] { ".pdf,.JPG", "image/jpeg" }, map), new String[] { "application/pdf", "image/jpeg" }));
        expect("types kept, unknown left out", java.util.Arrays.equals(SiteWeb.acceptTypes(new String[] { "image/*", ".xyz", "", null }, map), new String[] { "image/*" }));
        expect("anything: no type", SiteWeb.acceptTypes(new String[] { "" }, map).length == 0 && SiteWeb.acceptTypes(null, map).length == 0 && SiteWeb.acceptTypes(new String[] { ".pdf", "*" + '/' + "*" }, map).length == 0);
        expect("nothing asked: no line", SiteWeb.askedLine(bank, false, false, false, false) == null);
        expect("what was asked, in Sioul's words", "Ce site a demandé la caméra, votre position\u202f: pas dans cette version de Sioul.".equals(SiteWeb.askedLine(bank, true, false, true, false)));
        expect("the microphone in English when not given", SiteWeb.askedLine(bank, false, true, false, false).contains("the microphone"));
        expect("protected videos said too (review S4)", SiteWeb.askedLine(bank, false, false, false, true).contains("protected videos"));

        // ------------------------------------------------ What the file chooser hands back (review F1)
        String own = "com.aurelienpierre.sioul";
        expect("a document of another app goes to the page", SiteWeb.mayUpload("content://com.android.providers.media.documents/document/image%3A12", own));
        expect("a downloaded file another app lends", SiteWeb.mayUpload("content://com.android.providers.downloads.documents/document/msf%3A31", own));
        expect("never a file's path into Sioul's data", !SiteWeb.mayUpload("file:///data/user/0/com.aurelienpierre.sioul/files/mail/home/cur/1", own));
        expect("never the sites' own cookies", !SiteWeb.mayUpload("file:///data/data/com.aurelienpierre.sioul/app_webview/Default/Cookies", own));
        expect("never a file's path at all", !SiteWeb.mayUpload("file:///sdcard/Download/a.pdf", own) && !SiteWeb.mayUpload("FILE:///sdcard/a.pdf", own));
        expect("never Sioul's own provider", !SiteWeb.mayUpload("content://com.aurelienpierre.sioul.qtprovider/attachments/a.pdf", own));
        expect("never Sioul's start provider", !SiteWeb.mayUpload("content://com.aurelienpierre.sioul.start/x", own));
        expect("never Sioul's provider named by a user's number", !SiteWeb.mayUpload("content://0@com.aurelienpierre.sioul.qtprovider/files_path/notes/a.md", own));
        expect("never Sioul's provider in capitals or with a port", !SiteWeb.mayUpload("content://COM.AurelienPierre.Sioul.QtProvider/a", own) && !SiteWeb.mayUpload("content://com.aurelienpierre.sioul.qtprovider:1/a", own));
        expect("never Sioul's package itself as a provider", !SiteWeb.mayUpload("content://com.aurelienpierre.sioul/a", own));
        expect("a provider whose name only starts like Sioul's is another app's", SiteWeb.mayUpload("content://com.aurelienpierre.sioulother.files/a", own));
        expect("no other scheme, no address, no provider", !SiteWeb.mayUpload("https://example.org/a", own) && !SiteWeb.mayUpload((String) null, own) && !SiteWeb.mayUpload("content:///a", own) && !SiteWeb.mayUpload("content:a", own));
        expect("as Android's Uri parts", SiteWeb.mayUpload("content", "com.android.externalstorage.documents", own) && !SiteWeb.mayUpload("content", "10@com.aurelienpierre.sioul.qtprovider", own) && !SiteWeb.mayUpload("file", "", own) && !SiteWeb.mayUpload("content", null, own));
        expect("a file to open, or several", SiteWeb.choosesFiles(android.webkit.WebChromeClient.FileChooserParams.MODE_OPEN) && SiteWeb.choosesFiles(android.webkit.WebChromeClient.FileChooserParams.MODE_OPEN_MULTIPLE));
        expect("never a file to save nor a folder", !SiteWeb.choosesFiles(android.webkit.WebChromeClient.FileChooserParams.MODE_SAVE) && !SiteWeb.choosesFiles(2));

        // ------------------------------------------------ A file fetched by Sioul itself (review F3, F2)
        String file = "https://secure.bank.example.co.uk/statements/7";
        expect("a redirect on the same host is followed", "https://secure.bank.example.co.uk/files/7.pdf".equals(SiteDownload.sameHost(file, "/files/7.pdf")) && "https://secure.bank.example.co.uk/x".equalsIgnoreCase(SiteDownload.sameHost(file, "https://SECURE.bank.example.co.uk/x")));
        expect("a relative one too", "https://secure.bank.example.co.uk/statements/7.pdf".equals(SiteDownload.sameHost(file, "7.pdf")));
        expect("never to another host, even of the same site", SiteDownload.sameHost(file, "https://files.example.co.uk/7.pdf") == null && SiteDownload.sameHost(file, "https://storage.example.net/7.pdf") == null);
        expect("never to another scheme, port, a user, or a host given without a scheme", SiteDownload.sameHost(file, "http://secure.bank.example.co.uk/7.pdf") == null && SiteDownload.sameHost(file, "https://secure.bank.example.co.uk:8443/7.pdf") == null
            && SiteDownload.sameHost(file, "https://u@secure.bank.example.co.uk/7.pdf") == null && SiteDownload.sameHost(file, "//storage.example.net/7.pdf") == null);
        expect("nothing to follow: nothing", SiteDownload.sameHost(file, null) == null && SiteDownload.sameHost(file, "") == null && SiteDownload.sameHost(null, "/x") == null && SiteDownload.sameHost(file, "https://exa mple.net/") == null);
        expect("the file: kept", SiteDownload.verdict(200, "application/pdf", "application/pdf", null) == null && SiteDownload.verdict(200, null, "application/octet-stream", "attachment; filename=a.pdf") == null);
        expect("a page where a file was expected: never saved under its name", SiteDownload.verdict(200, "application/pdf", "text/html; charset=utf-8", null) == SiteDownload.Why.PAGE && SiteDownload.verdict(200, "", "application/xhtml+xml", "inline") == SiteDownload.Why.PAGE);
        expect("a page the site gives as an attachment, or a page expected: kept", SiteDownload.verdict(200, "application/pdf", "text/html", "Attachment; filename=a.html") == null && SiteDownload.verdict(200, "text/html", "text/html", null) == null);
        expect("any other status: not the file", SiteDownload.verdict(404, "application/pdf", "application/pdf", null) == SiteDownload.Why.PAGE && SiteDownload.verdict(206, "application/pdf", "application/pdf", null) == SiteDownload.Why.PAGE && SiteDownload.verdict(403, null, null, null) == SiteDownload.Why.PAGE);
        expect("larger than the limit: not fetched", SiteDownload.roomFor(SiteDownload.LIMIT + 1, Long.MAX_VALUE / 2) == SiteDownload.Why.LARGE);
        expect("no room left after it: not fetched", SiteDownload.roomFor(500L << 20, 600L << 20) == SiteDownload.Why.ROOM && SiteDownload.roomFor(-1, 100L << 20) == SiteDownload.Why.ROOM);
        expect("room enough, length said or not: fetched", SiteDownload.roomFor(5L << 20, 8L << 30) == null && SiteDownload.roomFor(-1, 8L << 30) == null);
        expect("a program is said as one", SiteDownload.isProgram("setup.APK", null) && SiteDownload.isProgram("update", "application/vnd.android.package-archive") && SiteDownload.isProgram("tool.exe", "application/octet-stream"));
        expect("a document is not", !SiteDownload.isProgram("Relev\u00e9.pdf", "application/pdf") && !SiteDownload.isProgram("photo.jpg", "image/jpeg") && !SiteDownload.isProgram(null, null));
        long t = 1_800_000_000_000L;
        SiteWeb.posted("https://secure.bank.example.co.uk/export", t);
        expect("a form's address remembered: its file cannot be asked again", SiteWeb.wasPosted("https://secure.bank.example.co.uk/export", t + 60_000));
        expect("for five minutes, that address alone", !SiteWeb.wasPosted("https://secure.bank.example.co.uk/export", t + SiteWeb.POSTED_FOR + 1) && !SiteWeb.wasPosted("https://secure.bank.example.co.uk/export?x", t) && !SiteWeb.wasPosted(null, t));
        for (int i = 0; i < 16; i++)
            SiteWeb.posted("https://example.org/form/" + i, t + i);
        expect("the last sixteen kept", !SiteWeb.wasPosted("https://secure.bank.example.co.uk/export", t + 20) && SiteWeb.wasPosted("https://example.org/form/0", t + 20) && SiteWeb.wasPosted("https://example.org/form/15", t + 20));
        SiteWeb.posted("http://example.org/form", t);
        expect("never an address that is not https", !SiteWeb.wasPosted("http://example.org/form", t));
        expect("the request names the page's origin alone", SiteWeb.origin("https://user@Secure.Bank.example.co.uk:8443/home?id=7#x").equals("https://secure.bank.example.co.uk:8443/") && SiteWeb.origin("about:blank").isEmpty() && SiteWeb.origin("").isEmpty());
        expect("progress in English", SiteWeb.progress(42, 100, java.util.Locale.ENGLISH).equals("42%") && SiteWeb.progress(3_100_000, -1, java.util.Locale.ENGLISH).equals("3.1\u00a0MB") && SiteWeb.size(820_000, java.util.Locale.ENGLISH).equals("820\u00a0KB") && SiteWeb.size(999, java.util.Locale.ENGLISH).equals("999\u00a0B"));
        expect("and in French", SiteWeb.progress(42, 100, java.util.Locale.FRENCH).equals("42\u202f%") && SiteWeb.size(3_100_000, java.util.Locale.FRENCH).equals("3,1\u00a0Mo") && SiteWeb.size(1_200_000_000, java.util.Locale.FRENCH).equals("1,2\u00a0Go"));
        expect("the browser offered where it can save what Sioul could not", SiteWeb.browserHelps(SiteDownload.Why.FORM) && SiteWeb.browserHelps(SiteDownload.Why.PAGE) && SiteWeb.browserHelps(SiteDownload.Why.ELSEWHERE) && !SiteWeb.browserHelps(SiteDownload.Why.ROOM) && !SiteWeb.browserHelps(SiteDownload.Why.LARGE));
        for (SiteDownload.Why why : SiteDownload.Why.values())
            expect("each reason has its words: " + why, SiteWeb.ENGLISH.containsKey("why_" + why.name().toLowerCase(java.util.Locale.ROOT)));
        java.io.File folder = Files.createTempDirectory("sioul-site-download").toFile();
        try {
            new java.io.File(folder, "a.pdf").createNewFile();
            new java.io.File(folder, "a (2).pdf").createNewFile();
            expect("a name taken gets a number, never a file replaced", SiteDownload.unique(folder, "a.pdf").getName().equals("a (3).pdf") && SiteDownload.unique(folder, "b.pdf").getName().equals("b.pdf"));
        } finally {
            for (java.io.File f : folder.listFiles())
                f.delete();
            folder.delete();
        }

        // ------------------------------------------------ Where a file may go (review F4)
        expect("the places in Rust's order", SiteWeb.places(new org.json.JSONArray("[\"choose\", \"downloads\"]")).equals(java.util.Arrays.asList("choose", "downloads")));
        expect("a place this Java cannot save to is not offered, nor one twice", SiteWeb.places(new org.json.JSONArray("[\"papers\", \"downloads\", \"downloads\"]")).equals(java.util.Arrays.asList("downloads")));
        expect("none listed: Downloads, then a folder chosen", SiteWeb.places(null).equals(java.util.Arrays.asList("downloads", "choose")) && SiteWeb.places(new org.json.JSONArray("[\"notes\"]")).equals(java.util.Arrays.asList("downloads", "choose")));
        expect("the site handed with its places", SiteWeb.Opening.of(site("{\"places\": [\"choose\"]}")).places.equals(java.util.Arrays.asList("choose")) && bank.places.equals(java.util.Arrays.asList("downloads", "choose")));
        Matcher rustPlaces = Pattern.compile("pub\\(crate\\) const PLACES: \\[&str; \\d+\\] = \\[([^\\]]*)\\]").matcher(rust);
        expect("Rust's places found", rustPlaces.find());
        List<String> placesNamed = new ArrayList<>();
        Matcher placeQuoted = Pattern.compile("\"([a-z]+)\"").matcher(rustPlaces.group(1));
        while (placeQuoted.find())
            placesNamed.add(placeQuoted.group(1));
        expect("Rust lists the places this Java saves to, in its order: " + placesNamed, placesNamed.equals(java.util.Arrays.asList(SiteDownload.PLACES)));
        for (String place : SiteDownload.PLACES)
            expect("each place has its words: " + place, SiteWeb.ENGLISH.containsKey("place_" + place));

        // ------------------------------------------------ A removed site's sign-in forgotten (review K1)
        List<String> expired = SiteWeb.expiries(java.util.Arrays.asList("sid=abc; domain=.example.org; path=/; Secure; HttpOnly", "pref=dark; path=/account", "__Host-token=x; path=/; Secure", "=nameless", "lang=fr; Domain=app.example.org"));
        expect("each cookie expired with its own domain and path: " + expired, expired.size() == 4
            && expired.get(0).equals("sid=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/; Secure; Domain=.example.org")
            && expired.get(1).equals("pref=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/account; Secure")
            && expired.get(2).equals("__Host-token=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/; Secure")
            && expired.get(3).endsWith("; Domain=app.example.org"));
        expect("no cookie, nothing to expire", SiteWeb.expiries(null).isEmpty() && SiteWeb.expiriesOf(null, "app.example.org", "example.org").isEmpty());
        List<String> byName = SiteWeb.expiriesOf("sid=abc; pref=dark; __Host-t=1", "app.example.org", "example.org");
        expect("without their attributes: each name expired on its host and under the site's domain: " + byName, byName.size() == 7 && byName.contains("sid=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/; Secure; Domain=example.org")
            && byName.contains("pref=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/; Secure") && byName.contains("__Host-t=; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/; Secure"));

        // ------------------------------------------------ Loaded again, refused (review L3, R2)
        expect("a page of the site loaded again as it was", "https://secure.bank.example.co.uk/statements?y=2026".equals(SiteWeb.again("https://secure.bank.example.co.uk/statements?y=2026", "example.co.uk")));
        expect("a page of another domain never brought back under the site", SiteWeb.again("https://look-alike.example.net/login", "example.co.uk") == null && SiteWeb.again("https://idp.example.net/authorize?state=1", "example.co.uk") == null);
        expect("nothing kept, or not https: the site's own", SiteWeb.again(null, "example.co.uk") == null && SiteWeb.again("about:blank", "example.co.uk") == null);
        expect("an http page refused: encrypted pages only", SiteWeb.refusedWord("http://old.example.net/").equals("not_https") && SiteWeb.refusedWord("HTTP://old.example.net/").equals("not_https"));
        expect("another app's address refused: no other application", SiteWeb.refusedWord("intent://scan/#Intent;end").equals("no_app_link") && SiteWeb.refusedWord("market://details?id=x").equals("no_app_link") && SiteWeb.refusedWord(null).equals("no_app_link"));

        // ------------------------------------------------ The web view's own engine (review V4)
        expect("the engine's version from its user agent", SiteWeb.engineOf("Mozilla/5.0 (Linux; Android 14; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/138.0.7204.179 Mobile Safari/537.36").equals("138"));
        expect("none said: nothing", SiteWeb.engineOf(null).isEmpty() && SiteWeb.engineOf("Mozilla/5.0 (Linux; Android 9)").isEmpty() && SiteWeb.engineOf("HeadlessChrome/120.0").isEmpty());

        // ------------------------------------------------ As Rust names them
        Matcher features = Pattern.compile("pub\\(crate\\) const FEATURES: \\[&str; \\d+\\] = \\[([^\\]]*)\\]").matcher(rust);
        expect("Rust's features found", features.find());
        List<String> named = new ArrayList<>();
        Matcher quoted = Pattern.compile("\"([A-Z_]+)\"").matcher(features.group(1));
        while (quoted.find())
            named.add(quoted.group(1));
        expect("the features asked are Rust's, in its order: " + named, named.equals(java.util.Arrays.asList(SiteWeb.FEATURES)));
        String[] library = { androidx.webkit.WebViewFeature.DOCUMENT_START_SCRIPT, androidx.webkit.WebViewFeature.WEB_MESSAGE_LISTENER, androidx.webkit.WebViewFeature.MUTE_AUDIO, androidx.webkit.WebViewFeature.ALGORITHMIC_DARKENING, androidx.webkit.WebViewFeature.MULTI_PROFILE };
        expect("each the library's own name", java.util.Arrays.equals(library, SiteWeb.FEATURES));
        String words = body(rust, "pub(crate) fn words(", "");
        Matcher keys = Pattern.compile("\\(\"([a-z_]+)\", \"(site|ui)-[a-z-]+\"\\)|insert\\(\"([a-z_]+)\"\\.into\\(\\)").matcher(words);
        List<String> given = new ArrayList<>();
        while (keys.find())
            given.add(keys.group(1) != null ? keys.group(1) : keys.group(3));
        expect("Rust's words found: " + given, given.size() >= 20);
        for (String key : given)
            expect("a word Rust gives is one the screen keeps: " + key, SiteWeb.ENGLISH.containsKey(key));
        expect("every word the screen shows comes from Rust", given.containsAll(SiteWeb.ENGLISH.keySet()));

        // ------------------------------------------------ Read from the sources
        Path root = Paths.get(System.getProperty("sioul.root"), "android", "package");
        String manifest = code(read(root.resolve("AndroidManifest.xml")));
        Matcher activity = Pattern.compile("(?s)<activity\\s+android:name=\"com\\.aurelienpierre\\.sioul\\.SiteScreen\"(.*?)(/>|</activity>)").matcher(manifest);
        expect("the screen is in the manifest", activity.find());
        String declared = activity.group(1);
        expect("not exported", declared.contains("android:exported=\"false\""));
        expect("no intent filter", activity.group(2).equals("/>") && !declared.contains("intent-filter"));
        expect("kept through rotations, resizes and keyboards", declared.contains("orientation") && declared.contains("screenSize") && declared.contains("smallestScreenSize") && declared.contains("screenLayout") && declared.contains("keyboard"));
        expect("above the soft keyboard", declared.contains("android:windowSoftInputMode=\"adjustResize\""));
        expect("in Sioul's task and process", !declared.contains("android:process") && !declared.contains("taskAffinity") && !declared.contains("excludeFromRecents") && !declared.contains("launchMode"));
        expect("the web view's diagnostics off", Pattern.compile("(?s)android:name=\"android\\.webkit\\.WebView\\.MetricsOptOut\"\\s+android:value=\"true\"").matcher(manifest).find());
        String rules = code(read(root.resolve("res/xml/sioul_data_extraction_rules.xml")));
        expect("the web view's folder out of a transfer", body(rules, "<device-transfer>", "    ").contains("<exclude domain=\"root\" path=\"app_webview/\" />") || rules.matches("(?s).*<device-transfer>.*<exclude domain=\"root\" path=\"app_webview/\" />.*</device-transfer>.*"));
        expect("and of a cloud backup", rules.matches("(?s).*<cloud-backup>.*<exclude domain=\"root\" />.*</cloud-backup>.*"));
        expect("no cloud backup before Android 12", manifest.contains("android:allowBackup=\"false\""));

        Path java = root.resolve("src/com/aurelienpierre/sioul");
        String screen = code(read(java.resolve("SiteScreen.java")));
        String web = code(read(java.resolve("SiteWeb.java")));
        expect("no JavaScript interface", !screen.contains("addJavascriptInterface") && !web.contains("addJavascriptInterface"));
        expect("no file or content access", screen.contains("setAllowFileAccess(false)") && screen.contains("setAllowContentAccess(false)") && !screen.contains("setAllowFileAccess(true)") && !screen.contains("AllowUniversalAccessFromFileURLs(true)") && !screen.contains("AllowFileAccessFromFileURLs(true)"));
        expect("no mixed content", screen.contains("MIXED_CONTENT_NEVER_ALLOW"));
        expect("a window only at a tap", screen.contains("setJavaScriptCanOpenWindowsAutomatically(false)") && body(screen, "public boolean onCreateWindow(", "        ").contains("!gesture"));
        expect("no debugging", !screen.contains("setWebContentsDebuggingEnabled"));
        expect("no certificate error passed over", !screen.contains("onReceivedSslError") && !screen.contains(".proceed("));
        expect("the console never logged", body(screen, "public boolean onConsoleMessage(", "        ").contains("return true;"));
        expect("the camera and the microphone refused", body(screen, "public void onPermissionRequest(", "        ").contains("request.deny();") && !screen.contains(".grant("));
        expect("the location refused", body(screen, "public void onGeolocationPermissionsShowPrompt(", "        ").contains("callback.invoke(origin, false, false);"));
        expect("a drawing process that ends: the screen goes, Sioul stays", body(screen, "public boolean onRenderProcessGone(", "        ").contains("end();") && body(screen, "public boolean onRenderProcessGone(", "        ").contains("return true;"));
        Matcher extras = Pattern.compile("getIntent\\(\\)\\.(get\\w+)\\(([^)]*)\\)").matcher(screen);
        int read = 0;
        while (extras.find()) {
            read++;
            expect("only the site's one extra is read: " + extras.group(), extras.group(1).equals("getStringExtra") && extras.group(2).equals("SITE"));
        }
        expect("the extra read once", read == 1);
        expect("the extra checked before anything", screen.indexOf("SiteWeb.Opening.of(getIntent().getStringExtra(SITE))") < screen.indexOf("setContentView"));
        for (String source : new String[] { screen, web }) {
            Matcher log = Pattern.compile("Log\\.\\w\\(([^;]*)\\);").matcher(source);
            while (log.find()) {
                String said = log.group(1);
                expect("nothing of a page in a log line: " + said, !said.matches("(?is).*\\b(url|address|committed|popupCommitted|cookies?|agent|name|disposition|asked)\\b.*") && !said.contains("getUrl") && !said.contains("getCookie"));
            }
        }
        expect("one screen at a time, in Sioul's task", body(web, "private static boolean open(", "    ").contains("Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP"));
        String shown = body(screen, "private void shownHost()", "    ");
        expect("the bar's lines from the rule, for the page in front", shown.contains("SiteWeb.bar(site.name, site.domain, popup != null ? popupCommitted : committed, site.word(\"from\"))") && shown.contains("title.setText(bar.first);") && shown.contains("host.setText(bar.second);"));
        expect("the site's name set nowhere else after the bar is made", screen.split("setText\\(site\\.name\\)", -1).length - 1 == 1);
        expect("the line holding a host is cut on the left, read left to right", body(screen, "private void lines(", "    ").contains("hostLine.setEllipsize(TextUtils.TruncateAt.START);") && body(screen, "private void lines(", "    ").contains("hostLine.setTextDirection(View.TEXT_DIRECTION_LTR);"));
        String fetchSource = code(read(java.resolve("SiteDownload.java")));
        boolean downloadService = false;
        try (java.util.stream.Stream<Path> all = Files.list(java)) {
            for (Path source : (Iterable<Path>) all::iterator)
                downloadService |= code(read(source)).contains("DownloadManager");
        }
        expect("no site's file goes through Android's download service, nor its cookies", !downloadService);
        expect("the fetch follows no redirect by itself", fetchSource.contains("answer.setInstanceFollowRedirects(false);") && !fetchSource.contains("setInstanceFollowRedirects(true)") && !fetchSource.contains("setFollowRedirects"));
        expect("a redirect followed on the same host alone", fetchSource.contains("String next = sameHost(address, answer.getHeaderField(\"Location\"));") && fetchSource.contains("return Why.ELSEWHERE;"));
        expect("the cookies read for each address asked, sent to it alone", body(fetchSource, "private static Why run(", "    ").indexOf("CookieManager.getInstance().getCookie(address)") > body(fetchSource, "private static Why run(", "    ").indexOf("new URL(address).openConnection()"));
        expect("nothing written before the answer is the file", fetchSource.indexOf("Why not = verdict(") < fetchSource.indexOf("Target target = target(context, job);") && fetchSource.indexOf("roomFor(length,") < fetchSource.indexOf("Target target = target(context, job);"));
        expect("a file cut short is taken back", fetchSource.contains("if (!done)") && fetchSource.contains("target.undo(context);") && fetchSource.contains("forget(context, job);"));
        expect("a file chosen to be replaced stays when the download fails before writing it", !SiteDownload.takenBack(false, 4096) && !SiteDownload.takenBack(false, -1));
        expect("the empty file made for a download, or one begun, is taken back", SiteDownload.takenBack(false, 0) && SiteDownload.takenBack(true, 4096) && SiteDownload.takenBack(true, -1));
        expect("a file under way stops with its site", body(screen, "protected void onDestroy()", "    ").contains("fetching.cancelled = true;"));
        expect("a form's address remembered, nothing answered in the page's place", body(screen, "public WebResourceResponse shouldInterceptRequest(", "        ").contains("SiteWeb.posted(") && body(screen, "public WebResourceResponse shouldInterceptRequest(", "        ").contains("return null;"));
        String offered = body(screen, "private void download(WebView page,", "    ");
        expect("a form's file said at once, never fetched", offered.indexOf("SiteWeb.wasPosted(url") >= 0 && offered.indexOf("SiteWeb.wasPosted(url") < offered.indexOf("offers.add("));
        // Every download asks first, even after a tap (review F4, the owner's word of 19 November 2026).
        expect("a file offered waits for its question, never fetched straight away", offered.contains("offers.add(") && offered.contains("askNext();") && !offered.contains("fetch("));
        expect("three offers wait at most: a page cannot start several unasked", screen.contains("private static final int OFFERS = 3;") && offered.indexOf("offers.size() >= OFFERS") < offered.indexOf("offers.add("));
        expect("one question at a time, none while a file is chosen or fetched", body(screen, "private void askNext()", "    ").contains("if (asking != null || choosing != null || fetching != null || offers.isEmpty()"));
        String question = body(screen, "private void ask(SiteDownload.Offer offer)", "    ");
        expect("the question: the file's name, which can be changed", question.contains("EditText name = new EditText(this);") && question.contains("name.setText(offer.name);") && question.contains("offer.named(SiteWeb.fileName(name.getText().toString()))"));
        expect("its size when known, and the host it comes from", question.contains("offer.length > 0") && question.contains("replace(\"{host}\", offer.host)"));
        expect("a line of caution for a program", question.contains("if (SiteDownload.isProgram(offer.name, offer.type))"));
        expect("each place offered, Rust's order, the first chosen", question.contains("for (int i = 0; i < ids.length; i++)") && question.contains("site.word(\"place_\" + site.places.get(i))") && question.contains("places.check(ids[0]);"));
        expect("Cancel saves nothing", question.contains(".setNegativeButton(site.word(\"cancel\"), null)"));
        expect("a file is fetched only once said where: Downloads, or the document \"Save as\" made", screen.split("[^.\\w]fetch\\(", -1).length - 1 == 3 && body(screen, "private void save(", "    ").contains("fetch(offer.job(SiteDownload.DOWNLOADS, null));")
            && body(screen, "protected void onActivityResult(", "    ").contains("fetch(offer.job(SiteDownload.CHOOSE, data.getData()));"));
        expect("\"Save as\" left: nothing saved", body(screen, "protected void onActivityResult(", "    ").contains("if (offer != null && result == RESULT_OK && data != null && data.getData() != null)"));
        expect("the question goes with its site", body(screen, "protected void onDestroy()", "    ").contains("offers.clear();") && body(screen, "protected void onDestroy()", "    ").contains("asking.dismiss();"));
        Matcher fetchLog = Pattern.compile("Log\\.\\w\\(DoseAlarms\\.TAG, ([^;]*)\\);").matcher(fetchSource);
        int fetchLogs = 0;
        while (fetchLog.find()) {
            fetchLogs++;
            expect("nothing of a file in a log line: " + fetchLog.group(1), fetchLog.group(1).matches("\"[^\"]*\"(\\s*\\+\\s*(e\\.getClass\\(\\)\\.getSimpleName\\(\\)|\\(target\\.own \\? \"[^\"]*\" : \"[^\"]*\"\\)))?"));
        }
        expect("the fetch's log lines read: " + fetchLogs, fetchLogs >= 6);
        // The fallback where the web view does not load ends cleanly (review L5).
        String create = body(screen, "protected void onCreate(Bundle saved)", "    ");
        expect("a web view made is noted once made, and only then", create.indexOf("view = page();") >= 0 && create.indexOf("madeView = true;") == create.indexOf("view = page();") + "view = page();".length() + "\n            ".length());
        expect("no web view: the browser, then the screen closes, nothing else asked", create.contains("browse(site.url);\n            finish();\n            return;"));
        expect("the cookie manager asked in one place, behind that note, its failure caught", screen.split("CookieManager\\.getInstance\\(\\)", -1).length - 1 == 2 && body(screen, "private void flushCookies()", "    ").contains("if (!madeView)") && body(screen, "private void flushCookies()", "    ").contains("catch (RuntimeException e)"));
        expect("pausing and closing write the cookies through it", body(screen, "protected void onPause()", "    ").contains("flushCookies();") && body(screen, "protected void onDestroy()", "    ").contains("flushCookies();") && !body(screen, "protected void onDestroy()", "    ").contains("CookieManager"));
        String forgetting = body(web, "private static String forgetHere(", "    ");
        expect("a site's data deleted alone where the web view can", forgetting.indexOf("WebViewFeature.isFeatureSupported(WebViewFeature.DELETE_BROWSING_DATA)") >= 0 && forgetting.indexOf("WebViewFeature.isFeatureSupported(WebViewFeature.DELETE_BROWSING_DATA)") < forgetting.indexOf("WebStorageCompat.deleteBrowsingDataForSite(WebStorage.getInstance(), url,"));
        expect("else its cookies and its address's storage", forgetting.contains("cookies.setCookie(address, cookie);") && forgetting.contains("WebStorage.getInstance().deleteOrigin(") && forgetting.contains("return \"cookies\";"));
        expect("on Android's main thread, waited for a while", body(web, "static JSONObject forget(", "    ").contains("new Handler(Looper.getMainLooper()).post(work);") && body(web, "static JSONObject forget(", "    ").contains("done.await(8, java.util.concurrent.TimeUnit.SECONDS)"));
        expect("StepService's door hands it on", web.contains("case \"site-forget\":"));
        // Nothing of Sioul's Java empties every site's store (review K2's rule, kept here): the
        // extensions' sign-in screen alone empties a web store, and it is its own, in a process
        // apart whose web view keeps other folders (SignInCheck holds the rest of its rule).
        List<String> wiping = new ArrayList<>();
        try (java.util.stream.Stream<Path> all = Files.list(java)) {
            for (Path source : (Iterable<Path>) all::iterator) {
                String text = code(read(source));
                if (text.contains("removeAllCookies") || text.contains("deleteAllData") || text.contains("getDir(\"webview") || text.contains("clearCache(true)") || text.contains("deleteBrowsingData(") || text.contains("setDataDirectorySuffix("))
                    wiping.add(source.getFileName().toString());
            }
        }
        expect("nothing of Sioul's Java empties a web store but the sign-in's screen: " + wiping, wiping.equals(List.of("SignIn.java")));
        Matcher signIn = Pattern.compile("(?s)<activity\\s+android:name=\"com\\.aurelienpierre\\.sioul\\.SignIn\"(.*?)(/>|</activity>)").matcher(manifest);
        Matcher signInProcess = Pattern.compile("android:process=\"([^\"]+)\"").matcher(signIn.find() ? signIn.group(1) : "");
        String apart = signInProcess.find() ? signInProcess.group(1) : "";
        expect("and the sign-in's screen runs in a process apart, its own, where no site is", SignIn.ownProcess("com.aurelienpierre.sioul" + apart, "com.aurelienpierre.sioul") && !SignIn.ownProcess("com.aurelienpierre.sioul", "com.aurelienpierre.sioul") && !declared.contains("android:process"));
        java.io.File[] itsFolders = SignIn.folders(new java.io.File("data"), new java.io.File("cache"));
        expect("so the folders it empties are its own, never the sites' (app_webview, cache/WebView)", !itsFolders[0].equals(new java.io.File("data", "app_webview")) && !itsFolders[1].equals(new java.io.File("cache", "WebView")) && itsFolders[0].getParentFile().equals(new java.io.File("data")) && itsFolders[1].getParentFile().equals(new java.io.File("cache")));
        // A web view drawing inside Sioul's process sends the site to the browser (review V1).
        expect("the screen asks where the web view draws before making one", create.indexOf("if (SiteWeb.drawsInside())") >= 0 && create.indexOf("if (SiteWeb.drawsInside())") < create.indexOf("view = page();") && create.indexOf("if (SiteWeb.drawsInside())") > create.indexOf("try {"));
        expect("and then opens the browser, saying why", create.contains("say(site.word(\"one_process\"));\n                browse(site.url);\n                finish();"));
        expect("where it draws, asked of androidx.webkit behind its feature", body(web, "static boolean drawsInside()", "    ").contains("WebViewFeature.isFeatureSupported(WebViewFeature.MULTI_PROCESS) && !WebViewCompat.isMultiProcessEnabled()"));
        String drawing = body(web, "private static String[] drawing(", "    ");
        expect("Rust's question asked on Android's main thread, three seconds at most", drawing.contains("new Handler(Looper.getMainLooper()).post(work);") && drawing.contains("done.await(3, java.util.concurrent.TimeUnit.SECONDS)"));
        expect("an answer kept only when whole (review V3)", drawing.contains("if (!found[0].isEmpty() && !found[1].isEmpty())") && body(web, "private static synchronized JSONObject features(", "    ").contains("if (whole) {"));
        expect("third-party cookies refused (review S2)", screen.contains("cookies.setAcceptThirdPartyCookies(page, false);") && !screen.contains("setAcceptThirdPartyCookies(page, true)"));
        expect("the service workers' own file and content access off (review S3)", body(screen, "private static void workers()", "    ").contains("workers.setAllowFileAccess(false);") && body(screen, "private static void workers()", "    ").contains("workers.setAllowContentAccess(false);") && body(screen, "private WebView page()", "    ").contains("workers();"));
        expect("an HTTP sign-in refused and said (review S4)", body(screen, "public void onReceivedHttpAuthRequest(", "        ").contains("handler.cancel();") && !screen.contains("handler.proceed("));
        expect("protected media refused and said", body(screen, "public void onPermissionRequest(", "        ").contains("RESOURCE_PROTECTED_MEDIA_ID") && body(screen, "public void onPermissionRequest(", "        ").contains("request.deny();"));
        // A page's dialogs and refusals cannot hold the screen (review N5).
        String dialogs = body(screen, "private boolean pageDialog(", "    ");
        expect("each of a page's dialogs is Sioul's own", screen.contains("return pageDialog(page, message, result, null, null, false);") && screen.contains("return pageDialog(page, message, result, result, defaultValue, true);") && screen.contains("return pageDialog(page, null, result, null, null, true);") && screen.contains("return pageDialog(page, message, result, null, null, true);"));
        expect("a second within a second refused at once", dialogs.contains("if (dialogShown || android.os.SystemClock.elapsedRealtime() - dialogGone < 1000 || isFinishing())") && dialogs.indexOf("result.cancel();") < dialogs.indexOf("dialogShown = true;"));
        expect("Close this site always offered", dialogs.contains("dialog.setNeutralButton(site.word(\"close_site\"), (d, w) -> {") && dialogs.contains("finish();"));
        expect("the page's answer given exactly once", dialogs.contains("if (answered[0])") && dialogs.contains("shown.setOnDismissListener(d -> {\n            answer.accept(false);"));
        expect("a refused navigation said once a page, in its own words", body(screen, "public boolean shouldOverrideUrlLoading(", "        ").contains("!current.equals(toldRefused)") && body(screen, "public boolean shouldOverrideUrlLoading(", "        ").contains("say(site.word(SiteWeb.refusedWord(url)));"));
        // A drawing process that ends behind another app: nothing said over it, the page back at the return (review L2, L3).
        String ending = body(screen, "private void end()", "    ");
        expect("behind: nothing said, loaded again at the return", ending.indexOf("if (!inFront) {") >= 0 && ending.indexOf("if (!inFront) {") < ending.indexOf("say(site.word(\"ended\"));") && ending.contains("againAtReturn = true;"));
        expect("in front: said, and the screen closes", ending.contains("say(site.word(\"ended\"));\n        finish();"));
        expect("the return loads it again, said", body(screen, "protected void onResume()", "    ").contains("if (againAtReturn) {") && body(screen, "private void again()", "    ").contains("SiteWeb.again(committed, site.domain)") && body(screen, "private void again()", "    ").contains("fetchSaid(site.word(\"loaded_again\"), false);"));
        expect("a screen made again loads a page of the site alone, and says so", create.contains("String again = SiteWeb.again(kept, site.domain);") && create.contains("fetchSaid(site.word(\"loaded_again\"), false);"));
        String windows = body(screen, "public boolean onCreateWindow(", "        ");
        expect("a new tab told from a pop-up with a size", windows.contains("popupTab = !dialog;"));
        String override = body(screen, "public boolean shouldOverrideUrlLoading(", "        ");
        expect("each navigation's window said to the rule", override.contains("SiteWeb.Window.SITE : popupTab ? SiteWeb.Window.TAB : SiteWeb.Window.POPUP") && override.contains("current, committed, site.domain, window"));
        expect("an empty window whose first page went elsewhere goes, after the call", override.contains("if (inPopup && request.isForMainFrame() && SiteWeb.isEmptyWindow(popupCommitted))") && body(screen, "private void closeEmpty(", "    ").contains("pages.post("));
        String chooser = body(screen, "public boolean onShowFileChooser(", "        ");
        expect("the chooser's intent from the field's types, never the web view's guess (review F5)", chooser.contains("SiteWeb.acceptTypes(params.getAcceptTypes(),") && !chooser.contains("params.createIntent()") && chooser.contains("say(site.word(\"no_chooser\"));"));
        expect("the system's bars in Sioul's colour on Android 11 to 14 (section 10)", body(screen, "private void insets(", "    ").contains("if (Build.VERSION.SDK_INT < Build.VERSION_CODES.VANILLA_ICE_CREAM) {"));
        expect("the chooser: another mode answered by nothing", chooser.contains("if (!SiteWeb.choosesFiles(params.getMode()))") && chooser.indexOf("choosesFiles") < chooser.indexOf("startActivityForResult"));
        String result = body(screen, "protected void onActivityResult(", "    ");
        expect("the chooser's answer checked, for the request waiting alone", result.contains("request == filesAsked && files != null") && result.contains("SiteWeb.chosen(result == RESULT_OK, data, getPackageName())"));
        expect("nothing else hands the page a file", screen.split("onReceiveValue\\(", -1).length - 1 == screen.split("onReceiveValue\\(null\\)", -1).length - 1 + 1);
        expect("opening a site never resets Sioul's task (it would close itself)", !body(web, "private static boolean open(", "    ").contains("RESET_TASK_IF_NEEDED"));

        // ------------------------------------------------ Whatever is tapped outside the site shows at once (review L1)
        // Android's activity element: finishOnTaskLaunch shuts an activity down, the task's root
        // excepted, when its task is launched again with FLAG_ACTIVITY_RESET_TASK_IF_NEEDED, and
        // only then. Every way into Sioul starts the launcher's intent with that flag; Recents
        // brings the task back without it, and the site with it.
        expect("the site's screen closes when Sioul's task is launched again", declared.contains("android:finishOnTaskLaunch=\"true\""));
        expect("and is not moved to another task instead", !declared.contains("allowTaskReparenting"));
        Matcher windowActivity = Pattern.compile("(?s)<activity\\s+android:name=\"org\\.qtproject\\.qt\\.android\\.bindings\\.QtActivity\"(.*?)</activity>").matcher(manifest);
        expect("the window is in the manifest", windowActivity.find());
        expect("the window is the launcher's, the task's root, brought up and never made twice", windowActivity.group(1).contains("android.intent.category.LAUNCHER") && windowActivity.group(1).contains("android:launchMode=\"singleTop\"") && !windowActivity.group(1).contains("taskAffinity"));
        String[][] openers = {
            { "ReminderOpener", "a notice" }, { "DoseOpener", "a dose's reminder" }, { "PauseOpener", "Pause, on the tile or the shortcut" }, { "HomeCardOpener", "a line of the home screen's card" },
            { "ShareActivity", "a share, a mail address, a calendar or contacts file" }, { "HomeCard", "the home screen's card" }, { "WakeAlarms", "the alarm at waking" }, { "StepService", "the background service's notice" }, { "TimeNote", "the time running's notice" },
        };
        for (String[] opener : openers) {
            String source = code(read(java.resolve(opener[0] + ".java")));
            Matcher launch = Pattern.compile("(?s)getLaunchIntentForPackage\\(.{0,300}?\\.addFlags\\(([^)]*)\\)").matcher(source);
            boolean found = false;
            boolean reset = true;
            while (launch.find()) {
                found = true;
                reset &= launch.group(1).contains("Intent.FLAG_ACTIVITY_NEW_TASK") && launch.group(1).contains("Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED");
            }
            expect(opener[1] + " (" + opener[0] + ") brings Sioul up as its icon does, closing a site's screen", found && reset);
        }
        // A dose's reminder tapped with a site open: the dose is kept for Sioul first, then the
        // window comes up over the site's screen it closes, and takes the dose as it becomes active.
        String dose = code(read(java.resolve("DoseOpener.java")));
        expect("a dose tapped is kept before Sioul comes up", dose.indexOf("DoseAlarms.opened(this, key)") >= 0 && dose.indexOf("DoseAlarms.opened(this, key)") < dose.indexOf("getLaunchIntentForPackage"));
        expect("a dose's reminder opens DoseOpener", code(read(java.resolve("DoseAlarms.java"))).contains("new Intent(context, DoseOpener.class)"));
        String windowQml = read(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-app", "qml", "main.qml"));
        expect("the window takes what was tapped as it becomes active", Pattern.compile("(?s)Qt\\.application\\.state === Qt\\.ApplicationActive\\)\\s*\\{\\s*sioul\\.backHere\\(\\)").matcher(windowQml).find());
        String backend = read(Paths.get(System.getProperty("sioul.root"), "crates", "sioul-app", "src", "backend.rs"));
        expect("back here, Rust takes the taps, a dose's first", body(backend, "    fn back_here(mut self: Pin<&mut Self>) {", "    ").contains("self.as_mut().take_taps();") && body(backend, "    fn take_taps(mut self: Pin<&mut Self>) {", "    ").contains("crate::alarms::opened()"));
        expect("the site's screen keeps nothing from the window coming up", !screen.contains("moveTaskToBack"));

        // ------------------------------------------------ A login from Bitwarden, read from the sources
        int evaluated = 0;
        List<String> bridges = new ArrayList<>();
        try (java.util.stream.Stream<Path> all = Files.list(java)) {
            for (Path source : (Iterable<Path>) all::iterator) {
                String text = code(read(source));
                evaluated += text.split("\\.evaluateJavascript\\(", -1).length - 1;
                for (String bridge : new String[] { "addJavascriptInterface", "addWebMessageListener", "postWebMessage", "createWebMessageChannel", "WebMessagePort", "WebMessageCompat", "JavascriptInterface" }) {
                    if (text.contains(bridge))
                        bridges.add(source.getFileName() + ": " + bridge);
                }
            }
        }
        expect("one evaluateJavascript in all of Sioul's Java: " + evaluated, evaluated == 1);
        String running = body(screen, "private void run(WebView page, String address, String call)", "    ");
        expect("it is the fill's, run in the page asked, after its address is checked again", running.contains("page.evaluateJavascript(call, null);") && running.indexOf("SiteWeb.sameOrigin(address, front == popup ? popupCommitted : committed)") >= 0
            && running.indexOf("SiteWeb.sameOrigin(address, front == popup ? popupCommitted : committed)") < running.indexOf("page.evaluateJavascript(") && running.contains("front != page") && running.contains("say(site.word(\"fill_changed\"));"));
        // Review of 22 November 2026, finding 1: the record is a copy; the web view's own address is asked too.
        expect("and against the web view's own address, right before the call", running.indexOf("!SiteWeb.sameOrigin(address, page.getUrl())") >= 0 && running.indexOf("!SiteWeb.sameOrigin(address, page.getUrl())") < running.indexOf("page.evaluateJavascript("));
        // The committed address is written by doUpdateVisitedHistory alone; onPageCommitVisible comes later and
        // may name an older navigation, so it only draws the bar again. Elsewhere a page's address is only emptied.
        expect("the committed address written from doUpdateVisitedHistory alone", screen.split("committed\\(page, url\\);", -1).length - 1 == 1
            && body(screen, "public void doUpdateVisitedHistory(", "        ").contains("committed(page, url);")
            && !body(screen, "public void onPageCommitVisible(", "        ").contains("committed") && body(screen, "public void onPageCommitVisible(", "        ").contains("shownHost();"));
        Matcher writes = Pattern.compile("\\b(popupCommitted|committed) = ([^;]*);").matcher(screen);
        List<String> written = new ArrayList<>();
        while (writes.find()) {
            if (!writes.group(2).equals("\"\"") && !writes.group(2).equals("url == null ? \"\" : url"))
                written.add(writes.group());
        }
        expect("nothing else writes a page's address, only empties it: " + written, written.isEmpty() && body(screen, "private void committed(WebView page, String url)", "    ").split(" = url == null \\? \"\" : url;", -1).length - 1 == 2);
        expect("no JavaScript interface, no message channel from a page, anywhere: " + bridges, bridges.isEmpty());
        String menuBody = body(screen, "private void menu()", "    ");
        Matcher vaultLines = Pattern.compile("(?s)if \\(site\\.vault\\) \\{\\s*items\\.add\\(Menu\\.NONE, 6, 0, site\\.word\\(\"fill\"\\)\\);\\s*items\\.add\\(Menu\\.NONE, 7, 0, site\\.word\\(\"choose_login\"\\)\\);\\s*\\}").matcher(menuBody);
        expect("the menu's two lines of the vault only with an account set", vaultLines.find() && menuBody.split("site\\.word\\(\"fill\"\\)", -1).length == 2 && menuBody.split("site\\.word\\(\"choose_login\"\\)", -1).length == 2);
        expect("each line asks its own mode", menuBody.contains("case 6:\n                fill(\"fill\");") && menuBody.contains("case 7:\n                fill(\"choose\");"));
        String asking = body(screen, "private void fill(String mode)", "    ");
        expect("asked with the address the web view committed for the page in front", asking.contains("String address = page == popup ? popupCommitted : committed;") && asking.contains("SiteWeb.fillAsked(address, mode)") && asking.indexOf("if (!SiteWeb.isHttps(address))") < asking.indexOf("fillQuestion(\"ask\""));
        String reaching = body(screen, "private static String rust(", "    ");
        expect("Rust loaded before it is asked, as the receivers load it", reaching.indexOf("DoseAlarms.load(context);") >= 0 && reaching.indexOf("DoseAlarms.load(context);") < reaching.indexOf("nativeFill(verb, json)"));
        expect("Rust asked in one place", screen.split("nativeFill\\(", -1).length - 1 == 2);
        expect("off Android's main thread, the answer back on it", body(screen, "private void fillQuestion(String verb,", "    ").contains("new Thread(") && body(screen, "private void fillQuestion(String verb,", "    ").contains("runOnUiThread("));
        List<String> logged = new ArrayList<>();
        int logLines = 0;
        for (String source : new String[] { screen, web }) {
            Matcher line = Pattern.compile("Log\\.\\w\\((.*)\\);").matcher(source);
            while (line.find()) {
                logLines++;
                String arguments = line.group(1).replaceAll("\"(?:[^\"\\\\]|\\\\.)*\"", "\"\"");
                if (!arguments.matches("DoseAlarms\\.TAG, \"\"(\\s*\\+\\s*e\\.getClass\\(\\)\\.getSimpleName\\(\\))?"))
                    logged.add(line.group(1));
            }
        }
        expect("every log line of the screen fixed words and an exception's class, nothing it was handed: " + logged + ", of " + logLines, logged.isEmpty() && logLines >= 10);
        expect("the call never in an intent, a saved state or the clipboard", !running.contains("putExtra") && !screen.contains("ClipboardManager") && !screen.contains("setPrimaryClip") && !body(screen, "protected void onSaveInstanceState(", "    ").contains("fill"));
        expect("the call kept in no field: a local of the answer, then run", !Pattern.compile("(?m)^    (private |static |final )*String \\w*([Cc]all|[Aa]nswer|[Pp]assword)\\w*( =[^;]*)?;").matcher(screen).find()
            && body(screen, "private void answered(", "    ").contains("String call = said.optString(\"call\", \"\");"));
        String forward = body(screen, "private void toWindow(", "    ");
        expect("the window brought forward in Sioul's task, the launcher's own activity reordered, never made twice", forward.contains("getLaunchIntentForPackage(getPackageName())") && forward.contains("new Intent().setComponent(window).addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT)") && !forward.contains("NEW_TASK") && !forward.contains("CLEAR_TOP"));
        String back = body(screen, "static boolean comeBack(Context context)", "    ");
        expect("the screen that asked brought back by reordering, never a new one, nothing when it is gone", back.contains("if (askingWindow() == null)") && back.contains("Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_REORDER_TO_FRONT") && !back.contains("putExtra") && !back.contains("CLEAR_TOP"));
        expect("reordering in those two places alone", screen.split("FLAG_ACTIVITY_REORDER_TO_FRONT", -1).length - 1 == 2 && !web.contains("FLAG_ACTIVITY_REORDER_TO_FRONT"));
        expect("the window's answer taken as the screen resumes", body(screen, "protected void onResume()", "    ").contains("if (fillPage != null)\n            takeFill();"));
        expect("a fill forgotten with its screen", body(screen, "protected void onDestroy()", "    ").contains("forgetFill();"));
        String opening = body(web, "private static boolean open(", "    ");
        expect("a site opened while a screen waits below the window closes that screen, never clearing the window above it", opening.contains("SiteScreen below = SiteScreen.askingWindow();") && opening.contains("below.runOnUiThread(below::finish);") && opening.contains("below != null ? Intent.FLAG_ACTIVITY_NEW_TASK : Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP"));
        expect("StepService's door brings the site back", web.contains("case \"site-fill-back\":\n            return SiteScreen.comeBack(context) ? \"true\" : \"false\";"));
        String bridge = read(Paths.get(System.getProperty("sioul.root"), "android", "main.cpp"));
        String nativeFill = body(bridge, "extern \"C\" JNIEXPORT jstring JNICALL Java_com_aurelienpierre_sioul_SiteScreen_nativeFill(", "");
        expect("main.cpp's bridge: Rust's answer wiped as it is given back, nothing logged", nativeFill.contains("sioul_site_fill(") && nativeFill.contains("sioul_site_fill_free(answer);") && nativeFill.contains("utf16.fill(QChar(u'\\0'));") && !nativeFill.contains("__android_log") && !nativeFill.contains("printf"));
        expect("and Rust's side logs nothing of a fill", !sitefill.contains("eprintln!") && !sitefill.contains("println!") && !sitefill.contains("dbg!"));
        expect("a site given is checked before the screen opens", body(web, "private static boolean open(", "    ").contains("Opening.of(asked.toString()) == null"));
        String steps = code(read(java.resolve("StepService.java")));
        expect("StepService's door to the sites' verbs", steps.contains("if (verb.startsWith(\"site-\"))") && steps.contains("return SiteWeb.call(context, verb, asked);"));
        expect("androidx.webkit in Gradle", code(read(root.resolve("build.gradle"))).contains("implementation 'androidx.webkit:webkit:"));

        System.out.println("SiteScreenCheck: " + (checked - failed) + " of " + checked + " held");
        if (failed > 0)
            System.exit(1);
    }
}
