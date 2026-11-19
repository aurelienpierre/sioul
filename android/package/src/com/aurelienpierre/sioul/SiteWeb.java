// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInfo;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;
import android.webkit.WebView;

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
 * asked for and did not get ({@link #askedLine}); and Rust's questions
 * (verbs "site-…", {@link #call}): what this device's web view is, and a
 * site opened. Rust decides where a site opens (crates/sioul-app/src/siteview.rs).
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
     * Rust's question ({@link StepService#call}): "site-webview" {features},
     * this device's web view; "site-open" {id, name, url, domain, look,
     * words}, the site's screen shown, answered true or false. Any thread.
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
            return webView(context, asked.optBoolean("features", false)).toString();
        case "site-open":
            return open(context, asked) ? "true" : "false";
        default:
            return null;
        }
    }

    /**
     * This device's web view, as Rust's {@code siteview::WebView} reads it:
     * {package, version}, and with {@code features} its label and features
     * too; package "" when the device has none (removed, or turned off).
     * Asking its features loads the web view into this process, once.
     */
    static JSONObject webView(Context context, boolean features) throws JSONException
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
        said.put("features", features(info.packageName + " " + info.getLongVersionCode()));
        return said;
    }

    /** Each feature of {@link #FEATURES}, asked of the web view once for its package and version. */
    private static synchronized JSONObject features(String of) throws JSONException
    {
        if (featuresFound != null && featuresOf.equals(of))
            return featuresFound;
        JSONObject found = new JSONObject();
        for (String name : FEATURES) {
            boolean on;
            try {
                on = WebViewFeature.isFeatureSupported(name);
            } catch (RuntimeException e) {
                on = false;
            }
            found.put(name, on);
        }
        featuresOf = of;
        featuresFound = found;
        return found;
    }

    /**
     * The site's screen shown, over Sioul's window, in Sioul's own task
     * (seen in Recents, one entry): a screen already shown, of this site or
     * another, is replaced. What Rust gave is checked as the screen will
     * check it ({@link Opening#of}).
     */
    private static boolean open(Context context, JSONObject asked)
    {
        if (Opening.of(asked.toString()) == null)
            return false;
        Intent intent = new Intent(context, SiteScreen.class)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP)
            .putExtra(SiteScreen.SITE, asked.toString());
        try {
            context.startActivity(intent);
            return true;
        } catch (RuntimeException e) {
            Log.w(DoseAlarms.TAG, "Sites: the site's screen could not be shown: " + e.getClass().getSimpleName());
            return false;
        }
    }

    // ---------------------------------------------------------------- What the screen is given

    /**
     * A site as the screen shows it, from the one extra Rust gives
     * ({@link SiteScreen#SITE}): its id and name, its address (https, with a
     * host), the registrable domain its links stay in, Sioul's colours and
     * the words in Sioul's language. Anything else in the intent is never read.
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
        private final Map<String, String> words;

        private Opening(String id, String name, String url, String domain, boolean dark, int[] colours, Map<String, String> words)
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
         * the screen shows, each a text of 600 characters at most.
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
            return new Opening(id, name, url, domain, dark, colours, words);
        }
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
        ENGLISH.put("asked", "This site asked for {what}: not in this version of Sioul.");
        ENGLISH.put("asked_camera", "the camera");
        ENGLISH.put("asked_microphone", "the microphone");
        ENGLISH.put("asked_location", "your location");
        ENGLISH.put("downloading", "Downloading {file} into Downloads: a notice says when it is there.");
        ENGLISH.put("download_page", "This file is made by the page itself: Sioul cannot save it yet. Open the page in your browser to save it.");
        ENGLISH.put("download_failed", "This file could not be saved here: open the page in your browser to save it.");
        ENGLISH.put("ended", "The page stopped: Android ended what drew it. Sioul goes on; open the site again.");
        ENGLISH.put("no_browser", "No application on this device opens this address.");
        ENGLISH.put("not_https", "Sioul opens encrypted pages only (https): this one was not opened.");
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
     * its ASCII form, a name in other alphabets as its "xn--" form (D2);
     * "…" before a page has committed. The label keeps its end in sight
     * when it is cut, so the registrable part always shows.
     *
     * @param committed the address of the page shown, as the web view committed it; "" before any
     * @return the host to show
     */
    static String shownHost(String committed)
    {
        String host = host(committed);
        if (host.isEmpty())
            return "…";
        try {
            return java.net.IDN.toASCII(host, java.net.IDN.ALLOW_UNASSIGNED).toLowerCase(Locale.ROOT);
        } catch (IllegalArgumentException e) {
            return host;
        }
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
     * The web view's own user agent, without the marks of a web view
     * ("; wv" and "Version/4.0"), as a computer's Sioul leaves Qt's name
     * out: some chats and one large provider refuse browsers they take for
     * embedded ones (docs/sites.md, "The browser").
     */
    static String userAgent(String agent)
    {
        if (agent == null)
            return null;
        return agent.replace("; wv)", ")").replace("Version/4.0 ", "").trim();
    }

    // ---------------------------------------------------------------- Downloads, requests

    /**
     * A downloaded file's name, as the web view guessed it, made safe for
     * Downloads: no folder in it, no control character, never "." or ".."
     * nor hidden, 120 characters at most (its extension kept); "download"
     * when nothing is left.
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
        if (name.length() > 120) {
            int dot = name.lastIndexOf('.');
            String ext = dot > 0 && name.length() - dot <= 10 ? name.substring(dot) : "";
            name = name.substring(0, 120 - ext.length()) + ext;
        }
        return name.isEmpty() ? "download" : name;
    }

    /**
     * The line in the bar's menu saying what the site asked for and did
     * not get in this phase (phase 2 asks you): the camera, the
     * microphone, your location, in that order, joined by commas; null
     * when it asked for none.
     *
     * @param site the site's words
     * @param camera whether it asked for the camera
     * @param microphone whether it asked for the microphone
     * @param location whether it asked for your location
     * @return the line, or null
     */
    static String askedLine(Opening site, boolean camera, boolean microphone, boolean location)
    {
        java.util.List<String> what = new java.util.ArrayList<>();
        if (camera)
            what.add(site.word("asked_camera"));
        if (microphone)
            what.add(site.word("asked_microphone"));
        if (location)
            what.add(site.word("asked_location"));
        return what.isEmpty() ? null : site.word("asked").replace("{what}", String.join(", ", what));
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
