// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.app.Application;
import android.content.Context;
import android.content.Intent;
import android.graphics.Typeface;
import android.os.Binder;
import android.os.Bundle;
import android.os.Handler;
import android.os.IBinder;
import android.os.Looper;
import android.os.Parcel;
import android.os.Process;
import android.os.RemoteException;
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
 * whole. Closing the screen gives back nothing. JavaScript is on, since
 * sign-in pages need it; files, content and other schemes are off. No scheme
 * is registered with the system: the manifest names nothing for this
 * activity to answer, and it is not exported.
 *
 * <p>A store of its own: the screen runs in a process apart from Sioul's
 * window ({@link #PROCESS}, the manifest's {@code android:process}), whose
 * web view keeps its folders apart ({@code WebView.setDataDirectorySuffix},
 * set once per process before any web view is made there: {@link #store}).
 * Its cookies, storage and caches are emptied before the page and after it;
 * the sites' store (SiteScreen.java), in the window's process, is never
 * touched by it, and neither sees the other's cookies. A screen that cannot
 * be sure of its own store shows no page.
 *
 * <p>Its answer goes back by a door ({@link Waited}): a binder of the
 * window's process, made for this one sign-in and handed to the screen in
 * the intent that starts it, which reaches this screen alone (not
 * exported). The screen tells it that it is shown, handing a door of its own
 * by which the window closes it ("sign-in-close"), then the address returned
 * or nothing; the window keeps only an address that begins with what it
 * watches for, and gives it to Rust (nativeReturned, android/main.cpp;
 * extensions.rs). Each door hears only Sioul's own processes, every call
 * goes one way without waiting (a process frozen in the back is never
 * asked to answer), and each side hears the other's process end.
 *
 * <p>The screen is part of Sioul's own task, seen in Recents: leaving it to
 * read a code in another app and coming back (Recents, or Sioul's icon)
 * finds it as it was. Back goes back one page of the sign-in, and from the
 * first page ends it. A screen the system makes again after its process was
 * ended is no longer waited for: it ends at once and says so; a screen whose
 * window's process ended ends too, as soon as it is in front. A page whose
 * renderer ends ends the sign-in, never Sioul; "Sign in…" is free again at
 * once.
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
    /** The window's door ({@link Waited}), in a bundle of its own: the screen's one way back. */
    static final String DOOR = "com.aurelienpierre.sioul.extra.DOOR";
    /**
     * The screen's process, after Sioul's package and a colon (the
     * manifest's {@code android:process=":signin"}), and the name its web
     * view's folders carry ({@code app_webview_signin}, {@code cache/webview_signin}).
     */
    static final String PROCESS = "signin";

    /** The screen shown, to the window: its own door follows. */
    static final int SHOWN = IBinder.FIRST_CALL_TRANSACTION;
    /** The screen's answer, to the window: the address returned, or none (cancelled). */
    static final int ANSWER = IBinder.FIRST_CALL_TRANSACTION + 1;
    /** The window gave up waiting, to the screen: it closes. */
    static final int CLOSE = IBinder.FIRST_CALL_TRANSACTION + 2;

    /** What a navigation is, for this screen. */
    enum Decision { WATCHED, ALLOWED, REFUSED }

    /** Rust told: {returned} or {cancelled} (crates/sioul-app/src/extensions.rs). In the window's process alone. */
    static native void nativeReturned(String json);

    /** In the window's process: the sign-in Rust waits for, none once it is answered or given up. */
    private static Waited waited;
    /** In the screen's process: whether its web view's folders are the sign-in's own, set once ({@link #store}). */
    private static boolean apart;
    /** In the screen's process: whether a web view was made here; until then its folders may be emptied. */
    private static boolean loaded;

    private WebView view;
    private TextView site;
    private String watch = "";
    private boolean french;
    /** This screen's store is the sign-in's own ({@link #store}): only then is anything emptied. */
    private boolean own;
    /** The window's door, from the intent; null when none came. */
    private IBinder window;
    /** The screen's own door, handed to the window. */
    private final Door door = new Door(this);
    /** The window's process ending, heard; unlinked as the screen goes. */
    private final IBinder.DeathRecipient ended = () -> runOnUiThread(this::windowEnded);
    private boolean answered;
    private boolean inFront;
    /** The window's process ended: the screen ends as soon as it is in front. */
    private boolean windowGone;

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

    /**
     * What the window gives Rust for a screen's answer: the address returned
     * when it is the one watched for, else that the sign-in was cancelled.
     * Nothing else a screen says reaches Rust.
     */
    static String answer(String address, String watch)
    {
        return (watched(address, watch) ? Attachments.said("returned", address) : Attachments.said("cancelled", true)).toString();
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
        return (french ? "Vous vous connectez sur : " : "You sign in on: ") + host;
    }

    /** What is said when a screen made again is no longer waited for: Android ended its process meanwhile. */
    static String interrupted(boolean french)
    {
        return french ? "Android a fermé la connexion pendant votre absence : appuyez à nouveau sur Se connecter…"
            : "Android closed the sign-in while you were away: press Sign in… again.";
    }

    /** What is said when Sioul's window ended while the screen was shown. */
    static String windowClosed(boolean french)
    {
        return french ? "Sioul a été fermé entre-temps : la connexion s’arrête. Appuyez à nouveau sur Se connecter…"
            : "Sioul was closed meanwhile: the sign-in stops. Press Sign in… again.";
    }

    /** What is said when the screen could not be sure of a store of its own: no page shown. */
    static String noStore(boolean french)
    {
        return french ? "La page de connexion n’a pas pu s’ouvrir à l’écart de vos sites : rien n’a été chargé."
            : "The sign-in page could not open apart from your sites: nothing was loaded.";
    }

    /**
     * Whether a screen being made is the one Rust waits for: never one the
     * system makes again from its saved state (its process ended meanwhile,
     * and the page with it), nor one whose window no longer waits (no door
     * came, or the window's process ended).
     */
    static boolean waitedFor(boolean remade, boolean windowWaits)
    {
        return !remade && windowWaits;
    }

    /**
     * Whether {@code process} is the screen's own process of the package
     * {@code pkg} ({@link #PROCESS}): never the window's, where the sites'
     * store is, nor another of Sioul's.
     */
    static boolean ownProcess(String process, String pkg)
    {
        return process != null && pkg != null && !pkg.isEmpty() && process.equals(pkg + ":" + PROCESS);
    }

    /**
     * The web view's folders in the screen's process, under the app's data
     * folder and its cache folder: as the web view names them for a suffix
     * ({@code webview_} and the suffix, the data folder's under
     * {@code app_}, as {@code Context.getDir} names it), never the sites'
     * ({@code app_webview}, {@code cache/WebView}).
     */
    static java.io.File[] folders(java.io.File data, java.io.File cache)
    {
        return new java.io.File[] { new java.io.File(data, "app_webview_" + PROCESS), new java.io.File(cache, "webview_" + PROCESS) };
    }

    /**
     * Rust's question ({@link StepService#call}, verbs "sign-in-…"), in the
     * window's process: "sign-in-open" {open, watch, host}, answered {asked}
     * or {problem}; "sign-in-close", the screen shown closed (Rust gave up
     * waiting, or has its answer).
     */
    static String call(Context context, String verb, JSONObject asked)
    {
        switch (verb) {
        case "sign-in-open": {
            String open = asked.optString("open", "");
            if (decide(open, "") != Decision.ALLOWED)
                return Attachments.said("problem", "scheme").toString();
            Waited door = new Waited(asked.optString("watch", ""));
            Bundle doors = new Bundle();
            doors.putBinder(DOOR, door);
            // Into Sioul's own task (the manifest gives the screen no affinity of its own).
            Intent intent = new Intent(context, SignIn.class).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                .putExtra(OPEN, open).putExtra(WATCH, door.watch).putExtra(HOST, asked.optString("host", ""))
                .putExtra(FRENCH, asked.optBoolean("french", false)).putExtra(DOOR, doors);
            synchronized (SignIn.class) {
                waited = door;
            }
            try {
                context.startActivity(intent);
                return Attachments.said("asked", true).toString();
            } catch (RuntimeException e) {
                synchronized (SignIn.class) {
                    if (waited == door)
                        waited = null;
                }
                Log.w(TAG, "SignIn: the screen could not be shown: " + e.getClass().getSimpleName());
                return Attachments.said("problem", "refused").toString();
            }
        }
        case "sign-in-close":
            Waited.close();
            return "true";
        default:
            return null;
        }
    }

    /**
     * Said to a door, one way: never waiting for its process, which may be
     * frozen in the back (Android ends an app that a call waits on while it
     * is frozen); nothing when it is gone. A door of the screen's, an
     * address, either, or none.
     */
    static void send(IBinder to, int code, IBinder door, String address)
    {
        if (to == null)
            return;
        Parcel data = Parcel.obtain();
        try {
            data.writeStrongBinder(door);
            data.writeString(address);
            to.transact(code, data, null, IBinder.FLAG_ONEWAY);
        } catch (RemoteException | RuntimeException e) {
            Log.i(TAG, "SignIn: the other side is gone; nothing said.");
        } finally {
            data.recycle();
        }
    }

    /**
     * The window's door, in its process: one per sign-in Rust waits for.
     * Heard only from Sioul's own processes (this app's user), and only
     * while it is the sign-in waited for; the screen's process ending before
     * it answered is a cancelled sign-in.
     */
    static final class Waited extends Binder implements IBinder.DeathRecipient
    {
        /** What the page returns to, as Rust asked. */
        final String watch;
        /** The screen's door, once it said it is shown. */
        private IBinder screen;
        /** Rust gave up waiting: a screen that says it is shown is closed at once. */
        private boolean closed;
        private boolean answered;

        Waited(String watch)
        {
            this.watch = watch;
        }

        @Override
        protected boolean onTransact(int code, Parcel data, Parcel reply, int flags)
        {
            if (Binder.getCallingUid() != Process.myUid())
                return false;
            switch (code) {
            case SHOWN:
                shown(data.readStrongBinder());
                return true;
            case ANSWER:
                data.readStrongBinder();
                answer(SignIn.answer(data.readString(), watch));
                return true;
            default:
                return false;
            }
        }

        /** The screen shown: its door kept, to close it; closed at once when Rust no longer waits. */
        private void shown(IBinder door)
        {
            if (door == null)
                return;
            boolean close;
            synchronized (SignIn.class) {
                close = closed || waited != this || screen != null;
                if (!close)
                    screen = door;
            }
            if (close) {
                send(door, CLOSE, null, null);
                return;
            }
            try {
                door.linkToDeath(this, 0);
            } catch (RemoteException e) {
                binderDied();
            }
        }

        /** The screen's process ended before it answered: the sign-in is cancelled. */
        @Override
        public void binderDied()
        {
            answer(SignIn.answer(null, watch));
        }

        /** Rust told once, while this is the sign-in it waits for, on Android's main thread. */
        private void answer(String json)
        {
            synchronized (SignIn.class) {
                if (answered || waited != this)
                    return;
                answered = true;
            }
            new Handler(Looper.getMainLooper()).post(() -> {
                try {
                    nativeReturned(json);
                } catch (UnsatisfiedLinkError e) {
                    Log.i(TAG, "SignIn: Sioul's window closed meanwhile; nothing said.");
                }
            });
        }

        /** Rust no longer waits: the screen shown is closed, one shown later too. */
        static void close()
        {
            Waited now;
            IBinder door;
            synchronized (SignIn.class) {
                now = waited;
                waited = null;
                if (now == null)
                    return;
                now.closed = true;
                door = now.screen;
            }
            if (door == null)
                return;
            send(door, CLOSE, null, null);
            try {
                door.unlinkToDeath(now, 0);
            } catch (RuntimeException e) {
                // Already gone.
            }
        }
    }

    /**
     * The screen's door, in its process: the window closes the screen by it.
     * Heard only from Sioul's own processes; it holds the screen weakly, so
     * that a window that keeps it keeps no screen alive.
     */
    static final class Door extends Binder
    {
        private final java.lang.ref.WeakReference<SignIn> screen;

        Door(SignIn screen)
        {
            this.screen = new java.lang.ref.WeakReference<>(screen);
        }

        @Override
        protected boolean onTransact(int code, Parcel data, Parcel reply, int flags)
        {
            if (Binder.getCallingUid() != Process.myUid() || code != CLOSE)
                return false;
            SignIn shown = screen.get();
            if (shown != null)
                shown.runOnUiThread(shown::finish);
            return true;
        }
    }

    @Override
    protected void onCreate(Bundle saved)
    {
        super.onCreate(saved);
        watch = getIntent().getStringExtra(WATCH);
        String open = getIntent().getStringExtra(OPEN);
        french = getIntent().getBooleanExtra(FRENCH, false);
        Bundle doors = getIntent().getBundleExtra(DOOR);
        window = doors == null ? null : doors.getBinder(DOOR);
        own = store();
        if (!own) {
            // Never a page in a store that may be the sites': nothing loaded, said, gone.
            Log.w(TAG, "SignIn: no web store of its own in this process; the sign-in is cancelled.");
            end(noStore(french));
            return;
        }
        if (!waitedFor(saved != null, window != null && window.isBinderAlive())) {
            // Its page ended with its process: a window still waiting hears "cancelled" at once. Said, then gone.
            clean();
            end(interrupted(french));
            return;
        }
        try {
            window.linkToDeath(ended, 0);
        } catch (RemoteException e) {
            clean();
            end(windowClosed(french));
            return;
        }
        send(window, SHOWN, door, null);
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
                tell(null);
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

    /**
     * Whether this process's web view keeps the sign-in's own store: the
     * screen runs in its own process ({@link #ownProcess}), and the web
     * view's folders there carry {@link #PROCESS}, set before any web view is
     * made in it. Android takes that once a process, and refuses it once a
     * web view is loaded: a second screen of the same process finds it set.
     */
    private boolean store()
    {
        if (!ownProcess(Application.getProcessName(), getPackageName()))
            return false;
        synchronized (SignIn.class) {
            if (!apart) {
                try {
                    WebView.setDataDirectorySuffix(PROCESS);
                    apart = true;
                } catch (IllegalStateException | IllegalArgumentException e) {
                    // A web view loaded here before, with folders that may not be the sign-in's.
                    return false;
                }
            }
        }
        return true;
    }

    /** The address returned, given back once; the screen ends. */
    private void returned(String url)
    {
        tell(url);
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

    /** The window told once: the address returned, or none (cancelled). */
    private void tell(String address)
    {
        if (answered)
            return;
        answered = true;
        send(window, ANSWER, null, address);
    }

    /** The screen ends without a page: the window, if it waits, hears "cancelled"; said in one line. */
    private void end(String text)
    {
        tell(null);
        Toast.makeText(this, text, Toast.LENGTH_LONG).show();
        finish();
    }

    /** Sioul's window ended: nobody waits; the screen ends now if in front, else as it comes back. */
    private void windowEnded()
    {
        answered = true;
        windowGone = true;
        if (inFront && !isFinishing())
            end(windowClosed(french));
    }

    @Override
    protected void onResume()
    {
        super.onResume();
        inFront = true;
        if (windowGone && !isFinishing())
            end(windowClosed(french));
    }

    @Override
    protected void onPause()
    {
        inFront = false;
        super.onPause();
    }

    /**
     * The sign-in's own store emptied: its cookies and the web's storage,
     * and the view's cache when there is one. Only when this screen's store
     * is the sign-in's ({@link #store}): this process's web view keeps no
     * other, and the sites' is in the window's process. Service workers and
     * the Cache API's stores, which no call of the web view empties, go
     * with its folders at the first screen of each process ({@link #clean}).
     */
    private void forget()
    {
        if (!own)
            return;
        if (view != null)
            view.clearCache(true);
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
        if (!own)
            return;
        if (!loaded)
            wipe(folders(getDataDir(), getCacheDir()));
        loaded = true;
        forget();
    }

    /**
     * The web view's folders emptied, every file and folder under each, the
     * folders themselves kept: only the sign-in's own ({@link #folders}),
     * before a web view is made in its process, so that nothing holds them open.
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
        tell(null);
        if (view != null) {
            view.stopLoading();
            view.clearHistory();
            view.clearFormData();
        }
        forget();
        if (view != null) {
            view.destroy();
            view = null;
        }
        if (window != null) {
            try {
                window.unlinkToDeath(ended, 0);
            } catch (RuntimeException e) {
                // Never linked, or already gone.
            }
        }
        super.onDestroy();
    }
}
