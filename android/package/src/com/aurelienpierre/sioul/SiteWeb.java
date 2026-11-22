// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.webkit.CookieManager;
import android.webkit.WebStorage;
import android.webkit.WebView;

import androidx.webkit.CookieManagerCompat;
import androidx.webkit.WebStorageCompat;
import androidx.webkit.WebViewCompat;
import androidx.webkit.WebViewFeature;

import org.json.JSONException;
import org.json.JSONObject;

import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;

/**
 * The rules of a site's screen ({@link SiteScreen}), apart from the screen
 * so that they are checked on a computer (android/jvm-checks/SiteScreenCheck):
 * what Rust may hand the screen ({@link Opening#of}), where each navigation
 * goes ({@link #where}), the host shown above the page ({@link #shownHost}),
 * a downloaded file's name ({@link #fileName}), the line saying what a site
 * asked for and did not get ({@link #askedLine}), whether a login's fill
 * still meets the page it was asked for ({@link #sameOrigin}); and Rust's
 * questions (verbs "site-…", {@link #call}): what this device's web view
 * is, a site opened, and the site brought back once the window answered a
 * fill. Rust decides where a site opens (crates/sioul-app/src/siteview.rs)
 * and what a fill does (crates/sioul-app/src/sitefill.rs).
 */
final class SiteWeb
{
    private SiteWeb()
    {
    }

    /**
     * What the web view is asked it can do, by androidx.webkit's names
     * (their values are their names): what a site's news needs (a script
     * before the page's own, and the channel back), a site silenced, pages
     * darkened, separate profiles. Rust's {@code siteview::FEATURES} names
     * the same, in the same order (SiteScreenCheck).
     */
    static final String[] FEATURES = { "DOCUMENT_START_SCRIPT", "WEB_MESSAGE_LISTENER", "MUTE_AUDIO", "ALGORITHMIC_DARKENING", "MULTI_PROFILE" };

    /** The features found, by the web view's package and version, asked once per process. */
    private static String featuresOf = "";
    private static JSONObject featuresFound;

    // ---------------------------------------------------------------- Rust's questions

    /**
     * Rust's question ({@link StepService#call}): "site-webview" {features,
     * process}, this device's web view; "site-open" {id, name, url, domain, look,
     * words, places, vault}, the site's screen shown, answered true or false;
     * "site-forget" {url, domain}, a removed site's sign-in forgotten
     * ({@link #forget}); "site-fill-back", the screen that asked the window
     * for a login brought back forward ({@link SiteScreen#comeBack}),
     * answered true or false. Any thread.
     *
     * @param context the application's context
     * @param verb what Rust asks
     * @param asked its JSON
     * @return the answer, as JSON text; null for a verb not known here
     * @throws JSONException never in practice: the answer's own keys
     */
    static String call(Context context, String verb, JSONObject asked) throws JSONException
    {
        switch (verb) {
        case "site-webview":
            return webView(context, asked.optBoolean("features", false), asked.optBoolean("process", false)).toString();
        case "site-open":
            return open(context, asked) ? "true" : "false";
        case "site-forget":
            return forget(asked.optString("url", ""), asked.optString("domain", "")).toString();
        case "site-fill-back":
            return SiteScreen.comeBack(context) ? "true" : "false";
        default:
            return null;
        }
    }

    /**
     * This device's web view, as Rust's {@code siteview::WebView} reads it:
     * {package, version}; with {@code process} or {@code features}, whether
     * it draws pages in a process of its own ("apart") or in Sioul's
     * ("one"; review V1) and its engine's own version from its user agent
     * ("138"; review V4), else what an earlier question found; with
     * {@code features}, its label and features too. Package "" when the
     * device has none (removed, or turned off). Asking loads the web view
     * into this process, once.
     */
    static JSONObject webView(Context context, boolean features, boolean process) throws JSONException
    {
        JSONObject said = new JSONObject();
        PackageInfo info;
        try {
            info = WebView.getCurrentWebViewPackage();
        } catch (RuntimeException e) {
            info = null;
        }
        if (info == null) {
            said.put("package", "");
            return said;
        }
        said.put("package", info.packageName);
        said.put("version", info.versionName == null ? "" : info.versionName);
        String of = info.packageName + " " + info.getLongVersionCode();
        String[] found = features || process ? drawing(context, of) : known(of);
        said.put("process", found[0]);
        said.put("engine", found[1]);
        if (!features)
            return said;
        // Its name and what it can do, for this device's card alone: the list needs neither.
        String label = "";
        try {
            if (info.applicationInfo != null)
                label = String.valueOf(info.applicationInfo.loadLabel(context.getPackageManager()));
        } catch (RuntimeException e) {
            label = "";
        }
        said.put("label", label);
        said.put("features", features(of));
        return said;
    }

    /** Where the web view draws and its engine, by its package and version: found once, a failure never kept (review V3). */
    private static String drawingOf = "";
    private static String[] drawingFound;

    /** What {@link #drawing} found for this web view, without asking it; "" for each when not yet. */
    private static synchronized String[] known(String of)
    {
        return drawingFound != null && drawingOf.equals(of) ? drawingFound : new String[] { "", "" };
    }

    /**
     * Whether the web view draws pages apart and its engine's version,
     * asked on Android's main thread (which the web view's start needs)
     * and waited for three seconds at most; each "" when it could not say,
     * then asked again the next time.
     */
    private static String[] drawing(Context context, String of)
    {
        String[] before = known(of);
        if (!before[0].isEmpty() && !before[1].isEmpty())
            return before;
        String[] found = { "", "" };
        java.util.concurrent.CountDownLatch done = new java.util.concurrent.CountDownLatch(1);
        Runnable work = () -> {
            try {
                if (WebViewFeature.isFeatureSupported(WebViewFeature.MULTI_PROCESS))
                    found[0] = WebViewCompat.isMultiProcessEnabled() ? "apart" : "one";
            } catch (RuntimeException e) {
                found[0] = "";
            }
            try {
                found[1] = engineOf(android.webkit.WebSettings.getDefaultUserAgent(context));
            } catch (RuntimeException e) {
                found[1] = "";
            }
            done.countDown();
        };
        if (Looper.myLooper() == Looper.getMainLooper())
            work.run();
        else
            new Handler(Looper.getMainLooper()).post(work);
        try {
            if (!done.await(3, java.util.concurrent.TimeUnit.SECONDS))
                return before;
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            return before;
        }
        synchronized (SiteWeb.class) {
            if (!found[0].isEmpty() && !found[1].isEmpty()) {
                drawingOf = of;
                drawingFound = found;
            }
        }
        return found;
    }

    /**
     * Whether this device's web view draws pages inside Sioul's own
     * process, on Android's main thread: where it does, a page's crash
     * would end Sioul, and the site opens in the browser (review V1).
     * False when it cannot say.
     *
     * @return whether it draws inside Sioul
     */
    static boolean drawsInside()
    {
        return WebViewFeature.isFeatureSupported(WebViewFeature.MULTI_PROCESS) && !WebViewCompat.isMultiProcessEnabled();
    }

    /**
     * The engine's own version, its first number, from the web view's user
     * agent ("… Chrome/138.0.7204.179 …" → "138"); "" when it says none.
     *
     * @param agent the web view's default user agent, or null
     * @return the version's first number, or ""
     */
    static String engineOf(String agent)
    {
        if (agent == null)
            return "";
        java.util.regex.Matcher chrome = java.util.regex.Pattern.compile("\\bChrome/(\\d{1,5})\\.").matcher(agent);
        return chrome.find() ? chrome.group(1) : "";
    }

    /** Each feature of {@link #FEATURES}, asked of the web view once for its package and version. */
    private static synchronized JSONObject features(String of) throws JSONException
    {
        if (featuresFound != null && featuresOf.equals(of))
            return featuresFound;
        JSONObject found = new JSONObject();
        boolean whole = true;
        for (String name : FEATURES) {
            boolean on;
            try {
                on = WebViewFeature.isFeatureSupported(name);
            } catch (RuntimeException e) {
                on = false;
                whole = false;
            }
            found.put(name, on);
        }
        // Kept only when every answer came (review V3): a failure is asked again next time.
        if (whole) {
            featuresOf = of;
            featuresFound = found;
        }
        return found;
    }

    /**
     * The site's screen shown, over Sioul's window, in Sioul's own task
     * (seen in Recents, one entry): a screen already shown, of this site or
     * another, is replaced. What Rust gave is checked as the screen will
     * check it ({@link Opening#of}). Decided from the live screen of this
     * process ({@link SiteScreen#live}), never from a fill waiting (review
     * of 22 November 2026, finding 3): one over the window is cleared
     * ({@code FLAG_ACTIVITY_CLEAR_TOP}); one under the window, which it
     * brought forward for a login, is closed, and the new one opens over
     * the window, since clearing down to it would close the window too;
     * with none known (none made, or Android ended the process since), the
     * new one opens over the window and nothing is cleared, so that a
     * record Android kept under the window is never cleared down to.
     */
    private static boolean open(Context context, JSONObject asked)
    {
        if (Opening.of(asked.toString()) == null)
            return false;
        SiteScreen live = SiteScreen.live();
        boolean under = live != null && live.isUnderWindow();
        if (under)
            live.runOnUiThread(live::finish);
        Intent intent = new Intent(context, SiteScreen.class)
            .addFlags(live != null && !under ? Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP : Intent.FLAG_ACTIVITY_NEW_TASK)
            .putExtra(SiteScreen.SITE, asked.toString());
        try {
            context.startActivity(intent);
            return true;
        } catch (RuntimeException e) {
            Log.w(DoseAlarms.TAG, "Sites: the site's screen could not be shown: " + e.getClass().getSimpleName());
            return false;
        }
    }

    /**
     * A site removed from Sioul forgets its sign-in on this device (review
     * K1), on Android's main thread, waited for a few seconds: where the web
     * view can delete a site's data alone (androidx.webkit's
     * DELETE_BROWSING_DATA), everything it stored for the site of
     * {@code url} (cookies, the cache, the pages' own storage), answered
     * {forgot: "all"}; else its cookies for its address and its domain,
     * each set expired with its own attributes where the web view gives
     * them (GET_COOKIE_INFO), and its address's own storage
     * ({@code WebStorage.deleteOrigin}), answered {forgot: "cookies"}: the
     * cache, and what it kept under its other addresses, stay. {forgot:
     * "none"} when nothing could be asked. Loads the web view into this
     * process.
     *
     * @param url the site's address
     * @param domain its domain, as Rust reads it
     * @return the answer
     * @throws JSONException never in practice: the answer's own keys
     */
    static JSONObject forget(String url, String domain) throws JSONException
    {
        JSONObject said = new JSONObject();
        said.put("forgot", "none");
        if (!isHttps(url))
            return said;
        String[] forgot = { "none" };
        java.util.concurrent.CountDownLatch done = new java.util.concurrent.CountDownLatch(1);
        Runnable work = () -> {
            try {
                forgot[0] = forgetHere(url, domain, done);
            } catch (RuntimeException e) {
                Log.w(DoseAlarms.TAG, "Sites: a removed site's sign-in could not be forgotten: " + e.getClass().getSimpleName());
                done.countDown();
            }
        };
        if (Looper.myLooper() == Looper.getMainLooper())
            work.run();
        else
            new Handler(Looper.getMainLooper()).post(work);
        try {
            if (!done.await(8, java.util.concurrent.TimeUnit.SECONDS))
                Log.w(DoseAlarms.TAG, "Sites: a removed site's sign-in still being forgotten after 8 s.");
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        said.put("forgot", forgot[0]);
        return said;
    }

    /** {@link #forget}'s work, on Android's main thread: what was forgotten. */
    private static String forgetHere(String url, String domain, java.util.concurrent.CountDownLatch done)
    {
        CookieManager cookies = CookieManager.getInstance();
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DELETE_BROWSING_DATA)) {
            WebStorageCompat.deleteBrowsingDataForSite(WebStorage.getInstance(), url, () -> {
                cookies.flush();
                done.countDown();
            });
            return "all";
        }
        java.util.List<String> addresses = new java.util.ArrayList<>(java.util.Arrays.asList(url));
        if (domain != null && domain.matches("[a-z0-9.-]{1,253}") && !host(url).equals(domain))
            addresses.add("https://" + domain + "/");
        boolean info = WebViewFeature.isFeatureSupported(WebViewFeature.GET_COOKIE_INFO);
        for (String address : addresses) {
            java.util.List<String> expired = info ? expiries(CookieManagerCompat.getCookieInfo(cookies, address)) : expiriesOf(cookies.getCookie(address), host(address), domain);
            for (String cookie : expired)
                cookies.setCookie(address, cookie);
            WebStorage.getInstance().deleteOrigin("https://" + host(address));
        }
        cookies.flush();
        done.countDown();
        return "cookies";
    }

    /** The date every expired cookie is set to. */
    private static final String EPOCH = "Thu, 01 Jan 1970 00:00:00 GMT";

    /**
     * Each cookie the web view gave with its attributes ("name=value;
     * domain=.example.org; path=/; Secure"), as a Set-Cookie that expires it:
     * the same name, domain and path, Secure kept (a "__Host-" name keeps
     * no domain, as it must).
     *
     * @param infos the cookies, as CookieManagerCompat.getCookieInfo gives them; null for none
     * @return the Set-Cookie lines that expire them
     */
    static java.util.List<String> expiries(java.util.List<String> infos)
    {
        java.util.List<String> out = new java.util.ArrayList<>();
        if (infos == null)
            return out;
        for (String info : infos) {
            if (info == null)
                continue;
            String[] parts = info.split(";");
            int eq = parts[0].indexOf('=');
            String name = (eq >= 0 ? parts[0].substring(0, eq) : parts[0]).trim();
            if (name.isEmpty())
                continue;
            String domain = null;
            String path = "/";
            for (int i = 1; i < parts.length; i++) {
                String part = parts[i].trim();
                int at = part.indexOf('=');
                String key = (at >= 0 ? part.substring(0, at) : part).trim().toLowerCase(Locale.ROOT);
                String value = at >= 0 ? part.substring(at + 1).trim() : "";
                if (key.equals("domain") && !value.isEmpty())
                    domain = value;
                else if (key.equals("path") && value.startsWith("/"))
                    path = value;
            }
            StringBuilder line = new StringBuilder(name).append("=; Max-Age=0; Expires=").append(EPOCH).append("; Path=").append(path).append("; Secure");
            if (domain != null && !name.startsWith("__Host-"))
                line.append("; Domain=").append(domain);
            out.add(line.toString());
        }
        return out;
    }

    /**
     * A Cookie header's names ("a=1; b=2"), each as the Set-Cookie lines
     * that expire it where the web view does not say its attributes: kept
     * by its host alone, and under the site's domain, at the root.
     *
     * @param header the web view's Cookie header for an address, or null
     * @param host the address's host
     * @param domain the site's domain
     * @return the Set-Cookie lines
     */
    static java.util.List<String> expiriesOf(String header, String host, String domain)
    {
        java.util.List<String> out = new java.util.ArrayList<>();
        if (header == null)
            return out;
        for (String pair : header.split(";")) {
            int eq = pair.indexOf('=');
            String name = (eq >= 0 ? pair.substring(0, eq) : pair).trim();
            if (name.isEmpty())
                continue;
            String base = name + "=; Max-Age=0; Expires=" + EPOCH + "; Path=/; Secure";
            out.add(base);
            if (!name.startsWith("__Host-")) {
                for (String under : new String[] { host, domain }) {
                    String line = base + "; Domain=" + under;
                    if (under != null && !under.isEmpty() && !out.contains(line))
                        out.add(line);
                }
            }
        }
        return out;
    }

    // ---------------------------------------------------------------- What the screen is given

    /**
     * A site as the screen shows it, from the one extra Rust gives
     * ({@link SiteScreen#SITE}): its id and name, its address (https, with a
     * host), the registrable domain its links stay in, Sioul's colours, the
     * words in Sioul's language, and whether a Bitwarden account is set on
     * this device (the menu's two lines of the vault). Anything else in the
     * intent is never read.
     */
    static final class Opening
    {
        final String id;
        final String name;
        final String url;
        final String domain;
        final boolean dark;
        final int background;
        final int surface;
        final int text;
        final int muted;
        final int line;
        /** Where a file may be saved, in the order the question offers them (SiteDownload.PLACES, as Rust lists them). */
        final java.util.List<String> places;
        /** A Bitwarden account is set on this device: Fill from Bitwarden and Choose a login… show in the menu. */
        final boolean vault;
        private final Map<String, String> words;

        private Opening(String id, String name, String url, String domain, boolean dark, int[] colours, Map<String, String> words, java.util.List<String> places, boolean vault)
        {
            this.id = id;
            this.name = name;
            this.url = url;
            this.domain = domain;
            this.dark = dark;
            this.background = colours[0];
            this.surface = colours[1];
            this.text = colours[2];
            this.muted = colours[3];
            this.line = colours[4];
            this.words = words;
            this.places = places;
            this.vault = vault;
        }

        /** The words Rust gave for {@code key}, else English, so that nothing shows a key. */
        String word(String key)
        {
            String said = words.get(key);
            if (said != null && !said.isEmpty())
                return said;
            String english = ENGLISH.get(key);
            return english == null ? "" : english;
        }

        /**
         * The site Rust handed, checked: null when the JSON is not an
         * object, the id is empty, too long or holds a control character,
         * the name is empty or too long, the address is not https with a
         * host or is too long, or the domain is not a host's name. Sioul's
         * colours: a colour that is not {@code #RRGGBB} or
         * {@code #AARRGGBB} takes Sioul's light one. Words: only the keys
         * the screen shows, each a text of 600 characters at most. The
         * vault's lines: only for {@code "vault": true}.
         *
         * @param json the extra's text, or null
         * @return the site, or null
         */
        static Opening of(String json)
        {
            if (json == null || json.length() > 64_000)
                return null;
            JSONObject asked;
            try {
                asked = new JSONObject(json);
            } catch (JSONException e) {
                return null;
            }
            String id = asked.optString("id", "");
            String name = asked.optString("name", "").trim();
            String url = asked.optString("url", "");
            String domain;
            try {
                // In its ASCII form, as the web view writes the hosts it compares with it.
                domain = java.net.IDN.toASCII(asked.optString("domain", ""), java.net.IDN.ALLOW_UNASSIGNED).toLowerCase(Locale.ROOT);
            } catch (IllegalArgumentException e) {
                return null;
            }
            if (id.isEmpty() || id.length() > 200 || hasControl(id) || name.isEmpty() || name.length() > 200 || hasControl(name))
                return null;
            if (url.length() > 4096 || !isHttps(url) || !domain.matches("[a-z0-9.-]{1,253}"))
                return null;
            JSONObject look = asked.optJSONObject("look");
            if (look == null)
                look = new JSONObject();
            boolean dark = look.optBoolean("dark", false);
            int[] colours = {
                colour(look.optString("background", ""), 0xFFF5F2EC),
                colour(look.optString("surface", ""), 0xFFFBFAF7),
                colour(look.optString("text", ""), 0xFF2D2A26),
                colour(look.optString("muted", ""), 0xFF6B655C),
                colour(look.optString("line", ""), 0xFFE2DCCF),
            };
            Map<String, String> words = new TreeMap<>();
            JSONObject given = asked.optJSONObject("words");
            if (given != null) {
                for (String key : ENGLISH.keySet()) {
                    String said = given.optString(key, "");
                    if (!said.isEmpty() && said.length() <= 600)
                        words.put(key, said);
                }
            }
            return new Opening(id, name, url, domain, dark, colours, words, places(asked.optJSONArray("places")), Boolean.TRUE.equals(asked.opt("vault")));
        }
    }

    /**
     * The places a file may be saved, in the order Rust lists them, each one
     * this Java can save to (SiteDownload.PLACES) and named once; this
     * Java's own list when Rust lists none it knows. A place Rust adds later
     * (Notes, Papers, Letters) is offered once this Java can save there.
     *
     * @param listed Rust's list, or null
     * @return the places, never empty
     */
    static java.util.List<String> places(org.json.JSONArray listed)
    {
        java.util.List<String> known = java.util.Arrays.asList(SiteDownload.PLACES);
        java.util.List<String> places = new java.util.ArrayList<>();
        for (int i = 0; listed != null && i < listed.length(); i++) {
            String place = listed.optString(i, "");
            if (known.contains(place) && !places.contains(place))
                places.add(place);
        }
        return places.isEmpty() ? known : places;
    }

    /** Sioul's words for the screen in English, for a word Rust did not give (siteview.rs, `words`). */
    static final Map<String, String> ENGLISH = new TreeMap<>();

    static {
        ENGLISH.put("back", "Back");
        ENGLISH.put("reload", "Reload");
        ENGLISH.put("more", "More");
        ENGLISH.put("close", "Close");
        ENGLISH.put("browser", "Open in the browser");
        ENGLISH.put("share", "Share the page's address");
        ENGLISH.put("find", "Find in the page");
        ENGLISH.put("find_none", "Not found");
        ENGLISH.put("previous", "Previous");
        ENGLISH.put("next", "Next");
        ENGLISH.put("close_window", "Close this window");
        ENGLISH.put("from", "from {name}");
        ENGLISH.put("asked", "This site asked for {what}: not in this version of Sioul.");
        ENGLISH.put("asked_camera", "the camera");
        ENGLISH.put("asked_microphone", "the microphone");
        ENGLISH.put("asked_location", "your location");
        ENGLISH.put("asked_media", "protected videos");
        ENGLISH.put("http_auth", "This site asks for a sign-in of its own that Sioul does not show (HTTP authentication): open it in your browser.");
        ENGLISH.put("download_page", "This file is made by the page itself: Sioul cannot save it yet. Open the page in your browser to save it.");
        ENGLISH.put("save_title", "Save this file?");
        ENGLISH.put("save_name", "Name");
        ENGLISH.put("save_from", "From {host}");
        ENGLISH.put("save_size_from", "{size}, from {host}");
        ENGLISH.put("save_program", "This file is a program: open it only if you expected one from this site.");
        ENGLISH.put("save_where", "Where");
        ENGLISH.put("place_choose", "A folder I choose…");
        ENGLISH.put("save", "Save");
        ENGLISH.put("cancel", "Cancel");
        ENGLISH.put("too_many", "This page offered more files at once than Sioul asks about: the others were not saved.");
        ENGLISH.put("saving", "Downloading {file}");
        ENGLISH.put("saved", "{file} is in {place}.");
        ENGLISH.put("saved_choose", "{file} is saved where you chose.");
        ENGLISH.put("saved_own", "{file} is in Sioul's own Downloads folder on this device (Android, data, Sioul): Android did not let Sioul write to Downloads.");
        ENGLISH.put("place_downloads", "Downloads");
        ENGLISH.put("open", "Open");
        ENGLISH.put("stop", "Stop");
        ENGLISH.put("no_app", "No application on this device opens this file.");
        ENGLISH.put("no_chooser", "No application on this device lets you choose a file to send.");
        ENGLISH.put("save_failed", "{file} was not saved: {why}");
        ENGLISH.put("why_form", "the site made it for a form, which Sioul cannot send again. Open the page in your browser to save it.");
        ENGLISH.put("why_elsewhere", "the site sends it from another address, where Sioul does not take its sign-in. Open the page in your browser to save it.");
        ENGLISH.put("why_page", "asked again, the site answered with a page, not the file (a link that works once, or a sign-in asked again). Open the page in your browser to save it.");
        ENGLISH.put("why_large", "it is larger than 1 GB, the most Sioul saves from a site.");
        ENGLISH.put("why_room", "this device has not enough room left for it.");
        ENGLISH.put("why_network", "the connection stopped before the end.");
        ENGLISH.put("why_write", "it could not be written there.");
        ENGLISH.put("why_cancelled", "you stopped it.");
        ENGLISH.put("ended", "The page stopped: Android ended what drew it. Sioul goes on; open the site again.");
        ENGLISH.put("no_browser", "No application on this device opens this address.");
        ENGLISH.put("not_https", "Sioul opens encrypted pages only (https): this one was not opened.");
        ENGLISH.put("no_app_link", "Sioul opens no other application from a page: this link was not opened.");
        ENGLISH.put("loaded_again", "Android had closed this page: it was loaded again.");
        ENGLISH.put("page_says", "{host} says");
        ENGLISH.put("ok", "OK");
        ENGLISH.put("close_site", "Close this site");
        ENGLISH.put("leave_ask", "Leave this page? What you typed in it may be lost.");
        ENGLISH.put("leave", "Leave");
        ENGLISH.put("stay", "Stay");
        ENGLISH.put("one_process", "This device's web view draws pages inside Sioul, where a page's crash would end Sioul: the site opens in your browser.");
        ENGLISH.put("fill", "Fill from Bitwarden");
        ENGLISH.put("choose_login", "Choose a login\u2026");
        ENGLISH.put("fill_changed", "The page changed meanwhile: nothing was filled.");
        ENGLISH.put("fill_https", "Sioul fills a login only into a page loaded over https: nothing was filled.");
    }

    /** Whether a text holds a control character (a line break among them). */
    static boolean hasControl(String text)
    {
        for (int i = 0; i < text.length(); i++) {
            if (Character.isISOControl(text.charAt(i)))
                return true;
        }
        return false;
    }

    /** "#RRGGBB" or "#AARRGGBB" as a colour, else `otherwise`. */
    static int colour(String said, int otherwise)
    {
        if (said == null || !said.matches("#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})"))
            return otherwise;
        long value = Long.parseLong(said.substring(1), 16);
        return said.length() == 7 ? (int) (0xFF000000L | value) : (int) value;
    }

    /** Whether a colour is light: the system's bars over it then take dark icons. */
    static boolean isLight(int colour)
    {
        double luminance = 0.2126 * ((colour >> 16) & 0xFF) + 0.7152 * ((colour >> 8) & 0xFF) + 0.0722 * (colour & 0xFF);
        return luminance > 140;
    }

    // ---------------------------------------------------------------- Addresses

    /** Whether an address is https with a host: the only pages a site's screen loads. */
    static boolean isHttps(String url)
    {
        return url != null && url.regionMatches(true, 0, "https://", 0, 8) && !host(url).isEmpty();
    }

    /**
     * An address's host: no scheme, no user, no port, no path, lower case;
     * "" when it has none.
     */
    static String host(String url)
    {
        if (url == null)
            return "";
        int scheme = url.indexOf("://");
        if (scheme < 0)
            return "";
        String rest = url.substring(scheme + 3);
        int end = rest.length();
        for (char stop : new char[] { '/', '?', '#', '\\' }) {
            int at = rest.indexOf(stop);
            if (at >= 0 && at < end)
                end = at;
        }
        String authority = rest.substring(0, end);
        authority = authority.substring(authority.lastIndexOf('@') + 1);
        if (authority.startsWith("[")) {
            int close = authority.indexOf(']');
            return close > 0 ? authority.substring(0, close + 1).toLowerCase(Locale.ROOT) : "";
        }
        int port = authority.indexOf(':');
        return (port >= 0 ? authority.substring(0, port) : authority).toLowerCase(Locale.ROOT);
    }

    /**
     * The host shown above the page: the committed page's, never a
     * navigation only asked for (review of the extensions' sign-in, D1); in
     * its ASCII form, a name in other alphabets as its "xn--" form (D2); a
     * blank page committed as such ("about:blank": a window the page wrote
     * into itself, review N3); "…" before a page has committed. The label
     * keeps its end in sight when it is cut, so the registrable part
     * always shows.
     *
     * @param committed the address of the page shown, as the web view committed it; "" before any
     * @return the host to show
     */
    static String shownHost(String committed)
    {
        if (committed != null && committed.regionMatches(true, 0, "about:", 0, 6)) {
            String page = committed.toLowerCase(Locale.ROOT);
            int end = page.length();
            for (char stop : new char[] { '?', '#' }) {
                int at = page.indexOf(stop);
                if (at >= 0 && at < end)
                    end = at;
            }
            return page.substring(0, Math.min(end, 40));
        }
        String host = host(committed);
        if (host.isEmpty())
            return "…";
        try {
            return java.net.IDN.toASCII(host, java.net.IDN.ALLOW_UNASSIGNED).toLowerCase(Locale.ROOT);
        } catch (IllegalArgumentException e) {
            return host;
        }
    }

    /** The bar's two lines, and whether the first is the host. */
    static final class Bar
    {
        final String first;
        final String second;
        final boolean hostFirst;

        Bar(String first, String second, boolean hostFirst)
        {
            this.first = first;
            this.second = second;
            this.hostFirst = hostFirst;
        }
    }

    /**
     * The bar's two lines, always true (review N2): while the page shown is
     * on the site's own domain, the site's name, its host under it; on any
     * other host (a redirect, a sign-in provider, a link wrapper, a tab or
     * a pop-up, a blank page), that host first and "from" the site's name
     * under it, so that Sioul never vouches for a page of another domain
     * under the site's name; before a page has committed, the name and "…".
     *
     * @param name the site's name
     * @param siteDomain the site's domain, in ASCII (Rust's)
     * @param committed the address of the page shown, as committed; "" before any
     * @param from Sioul's words for the second line, with {@code {name}} in them
     * @return the two lines
     */
    static Bar bar(String name, String siteDomain, String committed, String from)
    {
        String host = shownHost(committed);
        if (host.equals("…"))
            return new Bar(name, host, false);
        if (!host.startsWith("about:") && domainOf(host).equals(siteDomain))
            return new Bar(name, host, false);
        return new Bar(host, from.replace("{name}", name), true);
    }

    /**
     * The domain under which a host lives, as Rust's
     * {@code sioul_core::sites::domain_of} reads it: its last two labels,
     * three for the few countries that register under a second level
     * ("co.uk", "gouv.fr"). Rough on purpose: two sites taken as one keep a
     * link inside, as a computer keeps every link.
     */
    static String domainOf(String host)
    {
        String[] labels = java.util.Arrays.stream(host.split("\\.")).filter(l -> !l.isEmpty()).toArray(String[]::new);
        boolean second = labels.length >= 3 && java.util.Arrays.asList("co", "com", "gov", "org", "ac", "net", "gouv").contains(labels[labels.length - 2]);
        int keep = Math.min(labels.length, second ? 3 : 2);
        return String.join(".", java.util.Arrays.copyOfRange(labels, labels.length - keep, labels.length));
    }

    /** Where a navigation goes. */
    enum Way { LOAD, BROWSER, OTHER_APP, REFUSE }

    /**
     * Which window a navigation is in: the site's page; a new tab the page
     * opened (a link with {@code target="_blank"}, a {@code window.open}
     * without a size: the web view's {@code isDialog} false); a pop-up it
     * opened with a size, as a sign-in does ({@code isDialog} true).
     */
    enum Window { SITE, TAB, POPUP }

    /**
     * Where a navigation of a site's screen goes (WebViewClient's
     * shouldOverrideUrlLoading, which a POST and the page's first load never
     * reach). An https page loads, in the page or a frame; a blank page
     * too, and in a frame a page-made or inline one. A link the person taps
     * in the site's page (a gesture, not a server's redirect) to another
     * site than both the site's own domain and the page's opens in the
     * browser, and so does a new tab's first page on another site than the
     * site's and its opener's, redirects included (review N1): a link that
     * opens a new tab leaves as a tapped link does. A sign-in that moves
     * through other sites by redirects and forms stays here, and a pop-up
     * the site opened with a size (a sign-in's) keeps every https page. A
     * tab with a page of its own is as the site's page. A tapped mail or
     * telephone address goes to the app for it. Anything else is refused:
     * http, files, content, intents, other apps' schemes.
     *
     * @param url where it goes
     * @param mainFrame whether it is the page's own, not a frame's
     * @param gesture whether the person's tap started it
     * @param redirect whether a server redirected to it
     * @param current the address of the page it leaves, as committed; "" before any
     * @param opener the address of the site's page, as committed (for a tab's first page)
     * @param siteDomain the site's own domain (Rust's)
     * @param window the window it is in
     * @return where it goes
     */
    static Way where(String url, boolean mainFrame, boolean gesture, boolean redirect, String current, String opener, String siteDomain, Window window)
    {
        if (url == null)
            return Way.REFUSE;
        String lower = url.toLowerCase(Locale.ROOT);
        // A blank page holds nothing (a window opened empty, then sent somewhere).
        if (lower.equals("about:blank") || lower.equals("about:srcdoc"))
            return Way.LOAD;
        if (!mainFrame && (lower.startsWith("blob:https://") || lower.startsWith("data:")))
            return Way.LOAD;
        // A new tab's first page came from a tap: the window opens at a tap only.
        boolean firstOfTab = window == Window.TAB && mainFrame && !isHttps(current);
        boolean tapped = gesture || firstOfTab;
        if (lower.startsWith("mailto:") || lower.startsWith("tel:"))
            return mainFrame && tapped ? Way.OTHER_APP : Way.REFUSE;
        if (!isHttps(url))
            return mainFrame && tapped && lower.startsWith("http://") ? Way.BROWSER : Way.REFUSE;
        String to = domainOf(host(url));
        if (firstOfTab)
            return !to.equals(siteDomain) && !to.equals(domainOf(host(opener))) ? Way.BROWSER : Way.LOAD;
        if (!mainFrame || window == Window.POPUP || !gesture || redirect)
            return Way.LOAD;
        boolean elsewhere = !to.equals(siteDomain) && !to.equals(domainOf(host(current)));
        return elsewhere ? Way.BROWSER : Way.LOAD;
    }

    /**
     * Whether a window the page opened is still empty: nothing https
     * committed yet. Its first navigation ending elsewhere (the browser,
     * another app, refused, a download) closes it, so that no blank page
     * stays over the site (review N4).
     *
     * @param committed the window's committed address; "" before any
     * @return whether it holds no page of its own
     */
    static boolean isEmptyWindow(String committed)
    {
        return !isHttps(committed);
    }

    /**
     * An https address's port: the one it names, 443 when it names none; -1
     * when it is not https with a host, or its port does not read.
     *
     * @param url the address, or null
     * @return its port, or -1
     */
    static int port(String url)
    {
        if (!isHttps(url))
            return -1;
        String rest = url.substring(8);
        int end = rest.length();
        for (char stop : new char[] { '/', '?', '#', '\\' }) {
            int at = rest.indexOf(stop);
            if (at >= 0 && at < end)
                end = at;
        }
        String authority = rest.substring(0, end);
        authority = authority.substring(authority.lastIndexOf('@') + 1);
        int colon = authority.startsWith("[") ? authority.indexOf(':', Math.max(0, authority.indexOf(']'))) : authority.indexOf(':');
        if (colon < 0)
            return 443;
        String digits = authority.substring(colon + 1);
        if (!digits.matches("[0-9]{1,5}"))
            return -1;
        int port = Integer.parseInt(digits);
        return port <= 65535 ? port : -1;
    }

    /**
     * Whether a login's fill still meets the page it was asked for: both
     * addresses https, with the same host and the same port (the scheme,
     * host and port of the web view's committed addresses, as Rust's
     * {@code sitefill::same_origin} checks the ticket's). The path may
     * differ: a sign-in's page moves within its site. Anything else, the
     * fill is not run.
     *
     * @param asked the committed address of the page when the fill was asked
     * @param now the committed address of the page in front now
     * @return whether the fill may run
     */
    static boolean sameOrigin(String asked, String now)
    {
        return isHttps(asked) && isHttps(now) && host(asked).equals(host(now)) && port(asked) >= 0 && port(asked) == port(now);
    }

    /**
     * What the screen asks Rust for a fill ({@code sitefill::answer}): the
     * page's committed address, and "fill" or "choose"; for the window's
     * answer, the address alone (mode null). Nothing else.
     *
     * @param url the committed address of the page in front
     * @param mode "fill", "choose", or null
     * @return the JSON text
     */
    static String fillAsked(String url, String mode)
    {
        JSONObject asked = new JSONObject();
        try {
            asked.put("url", url == null ? "" : url);
            if (mode != null)
                asked.put("mode", mode);
        } catch (JSONException e) {
            return "{}";
        }
        return asked.toString();
    }

    /**
     * The address kept when the screen is put away, to load again if
     * Android ends the process meanwhile: the committed page's, https and
     * of reasonable length; else none (the site's own address then).
     */
    static String kept(String committed)
    {
        return committed != null && committed.length() <= 8192 && isHttps(committed) ? committed : null;
    }

    /** The address saved in a screen's state ({@link #kept}), read back: the same rule. */
    static String restored(Bundle saved, String key)
    {
        return saved == null ? null : kept(saved.getString(key));
    }

    /**
     * The page loaded again after Android ended it (a process, or a page's
     * drawing process): the kept address when it is on the site's own
     * domain, else none, and the site's own address is loaded (review L3):
     * a page of another domain is never brought back under the site.
     *
     * @param kept the address kept, or null
     * @param siteDomain the site's domain
     * @return the address to load again, or null
     */
    static String again(String kept, String siteDomain)
    {
        String address = kept(kept);
        return address != null && domainOf(shownHost(address)).equals(siteDomain) ? address : null;
    }

    /**
     * What a refused navigation says (review R2, N5): an http page, "Sioul
     * opens encrypted pages only"; anything else (another app's address,
     * a file), "Sioul opens no other application from a page".
     *
     * @param url the address refused, or null
     * @return the words' key
     */
    static String refusedWord(String url)
    {
        return url != null && url.regionMatches(true, 0, "http:", 0, 5) ? "not_https" : "no_app_link";
    }

    /**
     * The web view's own user agent, saying what it is (a web view, "; wv",
     * as Android writes it), without the device's model and build (review
     * S1): of the first parentheses, only "Linux", "Android" and its version, and
     * "wv" stay. Unchanged when it holds no "Android" there. A site that
     * refuses browsers inside apps then goes through Open in the browser.
     */
    static String userAgent(String agent)
    {
        if (agent == null)
            return null;
        int open = agent.indexOf('(');
        int close = open < 0 ? -1 : agent.indexOf(')', open);
        if (close < 0)
            return agent.trim();
        java.util.List<String> kept = new java.util.ArrayList<>();
        boolean android = false;
        for (String part : agent.substring(open + 1, close).split(";")) {
            String token = part.trim();
            if (token.matches("Android [0-9.]{1,12}"))
                android = true;
            if (token.equals("Linux") || token.equals("wv") || token.matches("Android [0-9.]{1,12}"))
                kept.add(token);
        }
        if (!android)
            return agent.trim();
        return (agent.substring(0, open + 1) + String.join("; ", kept) + agent.substring(close)).trim();
    }

    // ---------------------------------------------------------------- Downloads, requests

    /** The addresses a page sent a form's body to (POST), and when: a few, for a few minutes. */
    private static final java.util.LinkedHashMap<String, Long> POSTED = new java.util.LinkedHashMap<>();
    /** How long a form's address is remembered. */
    static final long POSTED_FOR = 5 * 60_000L;

    /**
     * A request with a form's body (POST) seen by the screen, by its
     * address: a file it answers cannot be fetched again (review F2). The
     * last sixteen, for {@link #POSTED_FOR}. Any thread.
     *
     * @param url its address
     * @param now the time now (Unix ms)
     */
    static void posted(String url, long now)
    {
        if (url == null || !isHttps(url))
            return;
        synchronized (POSTED) {
            POSTED.remove(url);
            POSTED.put(url, now);
            while (POSTED.size() > 16)
                POSTED.remove(POSTED.keySet().iterator().next());
        }
    }

    /**
     * Whether a file's address was sent a form's body lately ({@link #posted}).
     *
     * @param url the file's address
     * @param now the time now (Unix ms)
     * @return whether a form made it
     */
    static boolean wasPosted(String url, long now)
    {
        synchronized (POSTED) {
            Long at = url == null ? null : POSTED.get(url);
            return at != null && now - at <= POSTED_FOR && now >= at;
        }
    }

    /**
     * What a download's request names as where it comes from: the origin of
     * the page shown alone ("https://host/"), as a browser sends it to
     * another site; "" before a page.
     *
     * @param committed the page shown, as committed
     * @return its origin, or ""
     */
    static String origin(String committed)
    {
        if (!isHttps(committed))
            return "";
        String rest = committed.substring(8);
        int end = rest.length();
        for (char stop : new char[] { '/', '?', '#' }) {
            int at = rest.indexOf(stop);
            if (at >= 0 && at < end)
                end = at;
        }
        String authority = rest.substring(0, end);
        authority = authority.substring(authority.lastIndexOf('@') + 1);
        return "https://" + authority.toLowerCase(Locale.ROOT) + "/";
    }

    /**
     * A file's progress in a few characters: "42 %" when its length is
     * known, else what came so far ("3.1 MB").
     *
     * @param fetched bytes so far
     * @param length its length; -1 when not said
     * @param locale the words' language, for the decimal mark
     * @return the progress
     */
    static String progress(long fetched, long length, Locale locale)
    {
        if (length > 0)
            return Math.min(100, fetched * 100 / length) + (locale != null && "fr".equals(locale.getLanguage()) ? "\u202f%" : "%");
        return size(fetched, locale);
    }

    /**
     * A size as people read it, in powers of 1000 as Android's own settings
     * say it: "820 KB", "3.1 MB", "1.2 GB" ("Ko", "Mo", "Go" in French).
     *
     * @param bytes the size
     * @param locale the words' language
     * @return the size
     */
    static String size(long bytes, Locale locale)
    {
        boolean french = locale != null && "fr".equals(locale.getLanguage());
        String[] units = french ? new String[] { "o", "Ko", "Mo", "Go" } : new String[] { "B", "KB", "MB", "GB" };
        double value = Math.max(0, bytes);
        int unit = 0;
        while (value >= 1000 && unit < units.length - 1) {
            value /= 1000;
            unit++;
        }
        String number = unit == 0 || value >= 100 ? String.valueOf(Math.round(value)) : String.format(Locale.ROOT, "%.1f", value);
        if (french)
            number = number.replace('.', ',');
        return number + "\u00a0" + units[unit];
    }

    /**
     * Whether the page in the browser can save a file Sioul could not: a
     * form's, another address's, a page answered in its place, a file made
     * by the page; not a file too large, no room, the connection, Sioul's
     * own writing, nor one stopped.
     *
     * @param why why it was not saved
     * @return whether "Open in the browser" is offered
     */
    static boolean browserHelps(SiteDownload.Why why)
    {
        return why == SiteDownload.Why.FORM || why == SiteDownload.Why.ELSEWHERE || why == SiteDownload.Why.PAGE;
    }

    /**
     * A downloaded file's name, as the web view guessed it, made safe for
     * Downloads: no folder in it, no control character, never "." or ".."
     * nor hidden, 120 characters and 240 bytes at most (its extension
     * kept); "download" when nothing is left.
     */
    static String fileName(String guessed)
    {
        String name = guessed == null ? "" : guessed;
        name = name.substring(Math.max(name.lastIndexOf('/'), name.lastIndexOf('\\')) + 1);
        StringBuilder out = new StringBuilder();
        for (int i = 0; i < name.length(); i++) {
            char c = name.charAt(i);
            out.append(Character.isISOControl(c) || ":*?\"<>|".indexOf(c) >= 0 ? '_' : c);
        }
        name = out.toString().trim();
        while (name.startsWith("."))
            name = name.substring(1);
        int dot = name.lastIndexOf('.');
        String ext = dot > 0 && name.length() - dot <= 10 ? name.substring(dot) : "";
        String stem = ext.isEmpty() ? name : name.substring(0, dot);
        if (name.length() > 120)
            stem = stem.substring(0, Math.max(0, 120 - ext.length()));
        // A file system's names hold 255 bytes: 240 at most in UTF-8, cut between characters (section 10).
        while (!stem.isEmpty() && (stem + ext).getBytes(java.nio.charset.StandardCharsets.UTF_8).length > 240)
            stem = stem.substring(0, Character.isLowSurrogate(stem.charAt(stem.length() - 1)) && stem.length() > 1 ? stem.length() - 2 : stem.length() - 1);
        if (stem.trim().isEmpty())
            return "download" + ext;
        return (stem + ext).trim();
    }

    /**
     * The line in the bar's menu saying what the site asked for and did
     * not get in this phase (phase 2 asks you): the camera, the
     * microphone, your location, protected videos (review S4), in that
     * order, joined by commas; null when it asked for none.
     *
     * @param site the site's words
     * @param camera whether it asked for the camera
     * @param microphone whether it asked for the microphone
     * @param location whether it asked for your location
     * @param media whether it asked to play protected videos
     * @return the line, or null
     */
    static String askedLine(Opening site, boolean camera, boolean microphone, boolean location, boolean media)
    {
        java.util.List<String> what = new java.util.ArrayList<>();
        if (camera)
            what.add(site.word("asked_camera"));
        if (microphone)
            what.add(site.word("asked_microphone"));
        if (location)
            what.add(site.word("asked_location"));
        if (media)
            what.add(site.word("asked_media"));
        return what.isEmpty() ? null : site.word("asked").replace("{what}", String.join(", ", what));
    }

    /**
     * The types Android's file chooser offers for a page's field: each type
     * it names, and each extension (".pdf") as its type, through
     * {@code byExtension} (Android's MimeTypeMap); doubles and unknowns
     * left out; none when the field takes anything. The web view's own
     * intent takes the field's first entry as a type, which is none for an
     * extension (review F5).
     *
     * @param accepted the field's accept entries, or null
     * @param byExtension a type for an extension without its dot, or null
     * @return the types, none for any
     */
    static String[] acceptTypes(String[] accepted, java.util.function.Function<String, String> byExtension)
    {
        java.util.List<String> types = new java.util.ArrayList<>();
        for (String entry : accepted == null ? new String[0] : accepted) {
            for (String part : entry == null ? new String[0] : entry.split(",")) {
                String given = part.trim().toLowerCase(Locale.ROOT);
                String type = given.startsWith(".") ? byExtension.apply(given.substring(1)) : given;
                if (type != null && type.equals("*" + '/' + "*"))
                    return new String[0];
                if (type == null || !type.matches("[a-z0-9.+-]+/[a-z0-9.+*-]+"))
                    continue;
                if (!types.contains(type))
                    types.add(type);
            }
        }
        return types.toArray(new String[0]);
    }

    /**
     * Whether the chooser asked for is one Sioul answers: a file to open, or
     * several; never a file to save nor a folder (FileChooserParams' other
     * modes, which a recent web view may ask for), answered by nothing.
     *
     * @param mode FileChooserParams' mode
     * @return whether Android's file chooser opens for it
     */
    static boolean choosesFiles(int mode)
    {
        return mode == android.webkit.WebChromeClient.FileChooserParams.MODE_OPEN || mode == android.webkit.WebChromeClient.FileChooserParams.MODE_OPEN_MULTIPLE;
    }

    /**
     * Whether a file the chooser handed back may go to a page (review F1;
     * Android's reference for FileChooserParams: "The received result must
     * be treated as untrusted as it can contain Uris pointing to your own
     * app's sensitive data files"): a document another app lends
     * ({@code content:}), never one of Sioul's own providers (its package's
     * name, or a name under it, a user's number before {@code @} taken
     * off, as Android reads it), never a {@code file:} path, which the web
     * view would read as Sioul (its data, its web store, its cookies), and
     * nothing else.
     *
     * @param scheme the address's scheme, as Android's Uri reads it, or null
     * @param authority its authority, as Android's Uri reads it, or null
     * @param own Sioul's package
     * @return whether it may be uploaded
     */
    static boolean mayUpload(String scheme, String authority, String own)
    {
        if (scheme == null || !scheme.equalsIgnoreCase("content") || authority == null || own == null || own.isEmpty())
            return false;
        String provider = authority.substring(authority.lastIndexOf('@') + 1);
        int port = provider.indexOf(':');
        if (port >= 0)
            provider = provider.substring(0, port);
        provider = provider.trim().toLowerCase(Locale.ROOT);
        String sioul = own.toLowerCase(Locale.ROOT);
        return !provider.isEmpty() && !provider.equals(sioul) && !provider.startsWith(sioul + ".");
    }

    /**
     * {@link #mayUpload(String, String, String)} for an address as text: its
     * scheme, then its authority up to the path.
     *
     * @param address the address, or null
     * @param own Sioul's package
     * @return whether it may be uploaded
     */
    static boolean mayUpload(String address, String own)
    {
        if (address == null)
            return false;
        int colon = address.indexOf(':');
        if (colon <= 0)
            return false;
        String scheme = address.substring(0, colon);
        if (!address.startsWith("//", colon + 1))
            return mayUpload(scheme, null, own);
        String rest = address.substring(colon + 3);
        int end = rest.length();
        for (char stop : new char[] { '/', '?', '#' }) {
            int at = rest.indexOf(stop);
            if (at >= 0 && at < end)
                end = at;
        }
        return mayUpload(scheme, rest.substring(0, end), own);
    }

    /**
     * The files chosen in Android's picker for a page's upload: each one
     * picked (several, where the page took several), or the one, kept only
     * when it may be uploaded ({@link #mayUpload(String, String, String)});
     * none (the upload cancelled) when the picker was left or nothing is left.
     *
     * @param ok whether the picker answered with a choice
     * @param data its answer, or null
     * @param own Sioul's package
     * @return the files, or null
     */
    static Uri[] chosen(boolean ok, Intent data, String own)
    {
        if (!ok || data == null)
            return null;
        java.util.List<Uri> given = new java.util.ArrayList<>();
        if (data.getClipData() != null && data.getClipData().getItemCount() > 0) {
            for (int i = 0; i < data.getClipData().getItemCount(); i++)
                given.add(data.getClipData().getItemAt(i).getUri());
        } else {
            given.add(data.getData());
        }
        java.util.List<Uri> kept = new java.util.ArrayList<>();
        for (Uri file : given) {
            if (file != null && mayUpload(file.getScheme(), file.getAuthority(), own))
                kept.add(file);
            else if (file != null)
                Log.w(DoseAlarms.TAG, "Sites: a file the chooser handed back was not given to the page (not another app's document).");
        }
        return kept.isEmpty() ? null : kept.toArray(new Uri[0]);
    }
}
