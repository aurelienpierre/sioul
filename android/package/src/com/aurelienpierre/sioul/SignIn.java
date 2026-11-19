// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.Context;
import android.content.Intent;
import android.graphics.Typeface;
import android.os.Bundle;
import android.util.Log;
import android.view.ViewGroup;
import android.webkit.CookieManager;
import android.webkit.RenderProcessGoneDetail;
import android.webkit.WebResourceError;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebStorage;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.widget.Toast;

import org.json.JSONObject;

/**
 * An extension's sign-in page, on a phone or a tablet (docs/extensions.md,
 * "A sign-in"): a screen of Sioul's own with the system's web view, the
 * site's name always in sight above the page, so that you see where a
 * password is typed. Only https pages load; when a navigation reaches the
 * address the extension watches for (an application's own scheme, such as
 * {@code skoapp-prod://…}), it is stopped there, never loaded, and given back
 * to Rust whole (nativeReturned, android/main.cpp; extensions.rs). Closing
 * the screen gives back nothing. JavaScript is on, since sign-in pages need
 * it; files, content and other schemes are off; cookies and the web's
 * storage are cleared before and after (Sioul has no other web view on a
 * phone). No scheme is registered with the system: the manifest names
 * nothing for this activity to answer, and it is not exported.
 *
 * <p>The screen is part of Sioul's own task, seen in Recents: leaving it to
 * read a code in another app and coming back (Recents, or Sioul's icon)
 * finds it as it was. Back goes back one page of the sign-in, and from the
 * first page ends it. A screen the system makes again after Sioul's process
 * was ended is no longer waited for: it ends at once and says so. A page
 * whose renderer ends ends the sign-in, never Sioul; "Sign in…" is free
 * again at once.
 */
public final class SignIn extends Activity
{
    static final String TAG = "Sioul";
    /** The page to open (https). */
    static final String OPEN = "com.aurelienpierre.sioul.extra.OPEN";
    /** The beginning of the address returned to after the sign-in. */
    static final String WATCH = "com.aurelienpierre.sioul.extra.WATCH";
    /** The site's name, shown above the page until it moves on. */
    static final String HOST = "com.aurelienpierre.sioul.extra.HOST";
    /** Whether Sioul speaks French now: the screen's words follow Sioul's, not the phone's. */
    static final String FRENCH = "com.aurelienpierre.sioul.extra.FRENCH";

    /** What a navigation is, for this screen. */
    enum Decision { WATCHED, ALLOWED, REFUSED }

    /** Rust told: {returned} or {cancelled} (crates/sioul-app/src/extensions.rs). */
    static native void nativeReturned(String json);

    /** The screen shown, for Rust's "sign-in-close" (the time given up). */
    private static SignIn shown;
    /**
     * The page Rust asked to show and waits for, in this process; none once
     * the screen answered. A screen made again after the process ended finds none.
     */
    private static String asked;
    /** Whether a web view was made in this process: until then its folders may be emptied. */
    private static boolean loaded;

    private WebView view;
    private TextView site;
    private String watch = "";
    private boolean answered;

    /**
     * A navigation to {@code url}: the address watched for (given back, never
     * loaded), an https page (loaded), or anything else (refused).
     */
    static Decision decide(String url, String watch)
    {
        if (url == null)
            return Decision.REFUSED;
        if (watched(url, watch))
            return Decision.WATCHED;
        return url.startsWith("https://") ? Decision.ALLOWED : Decision.REFUSED;
    }

    /**
     * Whether {@code url} is the address watched for: it begins with
     * {@code watch}, and what follows ends its scheme and host (nothing, a
     * path, a query, a fragment), as Rust and the computer's window check it.
     */
    static boolean watched(String url, String watch)
    {
        if (url == null || watch == null || watch.isEmpty() || !url.startsWith(watch))
            return false;
        return url.length() == watch.length() || "/?#".indexOf(url.charAt(watch.length())) >= 0;
    }

    /** The host of an address, for the line above the page: no scheme, no user, no path. */
    static String host(String url)
    {
        if (url == null)
            return "";
        String rest = url.replaceFirst("^[A-Za-z][A-Za-z0-9+.-]*://", "");
        int end = rest.length();
        for (char stop : new char[] { '/', '?', '#' }) {
            int at = rest.indexOf(stop);
            if (at >= 0 && at < end)
                end = at;
        }
        String authority = rest.substring(0, end);
        int user = authority.lastIndexOf('@');
        return (user >= 0 ? authority.substring(user + 1) : authority).toLowerCase(java.util.Locale.ROOT);
    }

    /** The line above the page, in Sioul's language (French or English). */
    static String said(String host, boolean french)
    {
        return (french ? "Vous vous connectez sur\u202f: " : "You sign in on: ") + host;
    }

    /** What is said when a screen made again is no longer waited for. */
    static String interrupted(boolean french)
    {
        return french ? "La connexion a été interrompue pendant que Sioul était fermé\u202f: appuyez à nouveau sur Se connecter…"
            : "The sign-in was interrupted while Sioul was closed: press Sign in… again.";
    }

    /**
     * Whether a screen being made is the one Rust waits for: never one the
     * system makes again from its saved state (the process ended meanwhile,
     * so nothing waits for its answer), nor one whose page is not the page asked.
     */
    static boolean waitedFor(boolean remade, String waited, String open)
    {
        return !remade && waited != null && waited.equals(open);
    }

    /**
     * Rust's question ({@link StepService#call}, verbs "sign-in-…"):
     * "sign-in-open" {open, watch, host}, answered {asked} or {problem};
     * "sign-in-close", the screen shown closed (Rust gave up waiting).
     */
    static String call(Context context, String verb, JSONObject asked)
    {
        switch (verb) {
        case "sign-in-open": {
            String open = asked.optString("open", "");
            if (decide(open, "") != Decision.ALLOWED)
                return Attachments.said("problem", "scheme").toString();
            // Into Sioul's own task (the manifest gives the screen no affinity of its own).
            Intent intent = new Intent(context, SignIn.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                .putExtra(OPEN, open).putExtra(WATCH, asked.optString("watch", "")).putExtra(HOST, asked.optString("host", ""))
                .putExtra(FRENCH, asked.optBoolean("french", false));
            SignIn.asked = open;
            try {
                context.startActivity(intent);
                return Attachments.said("asked", true).toString();
            } catch (RuntimeException e) {
                SignIn.asked = null;
                Log.w(TAG, "SignIn: the screen could not be shown: " + e.getClass().getSimpleName());
                return Attachments.said("problem", "refused").toString();
            }
        }
        case "sign-in-close": {
            final SignIn screen = shown;
            if (screen != null)
                screen.runOnUiThread(screen::finish);
            return "true";
        }
        default:
            return null;
        }
    }

    @Override
    protected void onCreate(Bundle saved)
    {
        super.onCreate(saved);
        watch = getIntent().getStringExtra(WATCH);
        String open = getIntent().getStringExtra(OPEN);
        boolean french = getIntent().getBooleanExtra(FRENCH, false);
        if (!waitedFor(saved != null, asked, open)) {
            // Nobody waits for this one: its sign-in ended with the process. Said, then gone.
            answered = true;
            Toast.makeText(this, interrupted(french), Toast.LENGTH_LONG).show();
            clean();
            finish();
            return;
        }
        shown = this;
        clean();

        LinearLayout column = new LinearLayout(this);
        column.setOrientation(LinearLayout.VERTICAL);
        column.setFitsSystemWindows(true);
        LinearLayout bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        int gap = (int) (8 * getResources().getDisplayMetrics().density);
        bar.setPadding(2 * gap, gap, gap, gap);
        site = new TextView(this);
        site.setTypeface(Typeface.DEFAULT_BOLD);
        site.setTextSize(16);
        site.setText(said(getIntent().getStringExtra(HOST), french));
        bar.addView(site, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
        Button close = new Button(this);
        close.setText(french ? "Annuler" : "Cancel");
        close.setOnClickListener(v -> finish());
        bar.addView(close);
        column.addView(bar, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        view = new WebView(this);
        WebSettings settings = view.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setGeolocationEnabled(false);
        settings.setSupportMultipleWindows(false);
        settings.setJavaScriptCanOpenWindowsAutomatically(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        settings.setSafeBrowsingEnabled(true);
        CookieManager.getInstance().setAcceptCookie(true);
        CookieManager.getInstance().setAcceptThirdPartyCookies(view, false);
        view.setWebViewClient(new WebViewClient() {
            @Override
            public boolean shouldOverrideUrlLoading(WebView page, WebResourceRequest request)
            {
                String url = request.getUrl().toString();
                // Inside a page, its blank frames load too (as on a computer, SignInWindow.qml).
                if (!request.isForMainFrame() && (url.equals("about:blank") || url.equals("about:srcdoc")))
                    return false;
                switch (decide(url, watch)) {
                case WATCHED:
                    // From the main frame only: a frame of another site cannot end the sign-in.
                    if (request.isForMainFrame())
                        returned(url);
                    return true;
                case ALLOWED:
                    return false;
                default:
                    Log.i(TAG, "SignIn: a page that is not https refused.");
                    return true;
                }
            }

            @Override
            public void onPageStarted(WebView page, String url, android.graphics.Bitmap icon)
            {
                if (decide(url, watch) == Decision.WATCHED) {
                    page.stopLoading();
                    returned(url);
                    return;
                }
                site.setText(said(host(url), french));
            }

            @Override
            public void onReceivedError(WebView page, WebResourceRequest request, WebResourceError error)
            {
                // A redirect to the watched address that ends as an error rather than a navigation.
                if (request.isForMainFrame() && decide(request.getUrl().toString(), watch) == Decision.WATCHED)
                    returned(request.getUrl().toString());
            }

            @Override
            public boolean onRenderProcessGone(WebView page, RenderProcessGoneDetail detail)
            {
                // The page's renderer ended (its own fault, or the system took its memory back):
                // the sign-in ends here, Sioul stays, and "Sign in…" is free again at once.
                Log.i(TAG, "SignIn: the page's renderer ended; the sign-in is cancelled.");
                tell(Attachments.said("cancelled", true));
                if (page.getParent() instanceof ViewGroup)
                    ((ViewGroup) page.getParent()).removeView(page);
                page.destroy();
                if (view == page)
                    view = null;
                finish();
                return true;
            }
        });
        column.addView(view, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1));
        setContentView(column);
        if (decide(open, "") != Decision.ALLOWED) {
            finish();
            return;
        }
        view.loadUrl(open);
    }

    /** The address returned, given back once; the screen ends. */
    private void returned(String url)
    {
        tell(Attachments.said("returned", url));
        finish();
    }

    /** Back: one page back in the sign-in; from its first page, the sign-in ends. */
    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed()
    {
        if (view != null && view.canGoBack())
            view.goBack();
        else
            super.onBackPressed();
    }

    /** Rust told once, when Sioul's library is loaded. */
    private void tell(JSONObject said)
    {
        if (answered)
            return;
        answered = true;
        asked = null;
        try {
            nativeReturned(said.toString());
        } catch (UnsatisfiedLinkError e) {
            Log.i(TAG, "SignIn: Sioul's window closed meanwhile; nothing said.");
        }
    }

    /**
     * The web's cookies and storage forgotten: before the page, and after.
     * The HTTP cache goes with the view ({@code clearCache}); service workers
     * and the Cache API's stores, which no call of the web view empties, go
     * with its folders at the first screen of each process ({@link #wipe}).
     */
    private void forget()
    {
        CookieManager.getInstance().removeAllCookies(null);
        CookieManager.getInstance().flush();
        WebStorage.getInstance().deleteAllData();
    }

    /**
     * The web's data cleared before a page: at the first screen of the
     * process, its folders emptied while no web view holds them (what an
     * earlier screen left when its process ended before it could clear them,
     * service workers and caches included); then cookies and storage.
     */
    private void clean()
    {
        if (!loaded)
            wipe(new java.io.File[] { getDir("webview", MODE_PRIVATE), new java.io.File(getCacheDir(), "WebView") });
        loaded = true;
        forget();
    }

    /**
     * The web view's folders emptied, every file and folder under each, the
     * folders themselves kept: only before a web view is made in this process
     * (Sioul has no other), so that nothing holds them open.
     */
    static void wipe(java.io.File[] folders)
    {
        for (java.io.File folder : folders) {
            java.io.File[] inside = folder.listFiles();
            if (inside == null)
                continue;
            for (java.io.File file : inside) {
                // A link is taken out, never followed.
                if (file.isDirectory() && !java.nio.file.Files.isSymbolicLink(file.toPath()))
                    wipe(new java.io.File[] { file });
                if (!file.delete())
                    Log.i(TAG, "SignIn: a web view file stayed.");
            }
        }
    }

    @Override
    protected void onDestroy()
    {
        tell(Attachments.said("cancelled", true));
        if (view != null) {
            view.stopLoading();
            view.clearHistory();
            view.clearCache(true);
            view.clearFormData();
            view.destroy();
            view = null;
        }
        forget();
        if (shown == this)
            shown = null;
        super.onDestroy();
    }
}
