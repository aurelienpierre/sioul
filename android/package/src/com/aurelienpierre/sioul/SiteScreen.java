// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.app.DownloadManager;
import android.content.ActivityNotFoundException;
import android.content.Intent;
import android.content.res.ColorStateList;
import android.graphics.Insets;
import android.graphics.Typeface;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.os.Message;
import android.text.Editable;
import android.text.TextUtils;
import android.text.TextWatcher;
import android.util.Log;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.KeyEvent;
import android.view.Menu;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
import android.webkit.ConsoleMessage;
import android.webkit.CookieManager;
import android.webkit.GeolocationPermissions;
import android.webkit.PermissionRequest;
import android.webkit.RenderProcessGoneDetail;
import android.webkit.URLUtil;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.ImageButton;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.TextView;
import android.widget.Toast;

/**
 * A site of Sioul's, inside Sioul on Android (docs/android.md, "Sites"): a
 * screen of Sioul's own around the system's web view, one site at a time,
 * over Sioul's window, in its task (one entry in Recents). Rust opens it
 * where this device's web view can carry the site (crates/sioul-app/src/
 * siteview.rs, through {@link SiteWeb#call}) and gives it the site, Sioul's
 * colours and its words in Sioul's language: Java keeps none of its own but
 * an English fallback.
 *
 * <p>A slim bar always in sight: back, the site's name and the host of the
 * page shown (the committed page's, never one only asked for), reload, and a
 * menu: what the site asked for and did not get, Open in the browser, Share
 * the page's address, Find in the page, Close this window (a pop-up's),
 * Close. Back goes back in the site, then closes the screen. A rotation, a
 * split screen or a keyboard attached keeps the page (the manifest's
 * {@code configChanges}); the bar and the page keep out of the system's bars
 * and above the soft keyboard.
 *
 * <p>Every site shares the web view's one store of this process, on this
 * device alone (no backup, no transfer: res/xml/sioul_data_extraction_rules.xml):
 * a site left and opened again is still signed in, after Android ended the
 * process too, and the page shown then is loaded again ({@link SiteWeb#kept}).
 * An upload goes through Android's file chooser; a download into Downloads,
 * said by Android's one notice that opens the file; a pop-up a site opens
 * at a tap shows over its page and goes when the page closes it; a link the
 * person taps to another site opens in the browser ({@link SiteWeb#where}).
 * The camera, the microphone and the location are refused in this phase,
 * the menu saying the site asked. A page whose drawing process ends closes
 * the screen; Sioul goes on.
 *
 * <p>Safety: https pages only, mixed content never; no file or content
 * access from a page; no JavaScript interface; not exported, its one extra
 * checked ({@link SiteWeb.Opening#of}); nothing of a page in the log (no
 * address, no cookie, not its console).
 */
public final class SiteScreen extends Activity
{
    /** The one extra Rust gives: the site, Sioul's colours and words, as JSON ({@link SiteWeb.Opening}). */
    static final String SITE = "com.aurelienpierre.sioul.extra.SITE";
    /** The page shown, kept in the screen's saved state. */
    private static final String SHOWN = "shown";
    /** Android's file chooser, for a page's upload: its requests' codes, one per request, from here. */
    private static final int FILES = 4301;
    private static final int FILES_SPAN = 64;

    private SiteWeb.Opening site;
    private TextView host;
    private FrameLayout pages;
    private LinearLayout finding;
    private EditText findField;
    private TextView findCount;
    private ImageButton more;
    /** The site's page. */
    private WebView view;
    /** A window the page opened, shown over it; null when none. */
    private WebView popup;
    /** The address of the page each shows, as committed: never a navigation only asked for. */
    private String committed = "";
    private String popupCommitted = "";
    /** A page's upload waiting for the chooser, and the code of the one request it answers. */
    private ValueCallback<Uri[]> files;
    private int filesAsked = FILES;
    /** What the site asked for and did not get. */
    private boolean askedCamera;
    private boolean askedMicrophone;
    private boolean askedLocation;
    /** The page's drawing process ended: the screen goes. */
    private boolean ended;

    @Override
    protected void onCreate(Bundle saved)
    {
        site = SiteWeb.Opening.of(getIntent().getStringExtra(SITE));
        // Sioul's own theme for the menu and the find field, light or dark as Sioul is.
        setTheme(site != null && site.dark ? android.R.style.Theme_DeviceDefault_NoActionBar : android.R.style.Theme_DeviceDefault_Light_NoActionBar);
        super.onCreate(saved);
        if (site == null) {
            Log.w(DoseAlarms.TAG, "Sites: a screen asked without a site it can show; closed.");
            finish();
            return;
        }
        setContentView(layout());
        try {
            view = page(false);
        } catch (RuntimeException e) {
            // No web view after all (turned off meanwhile, or not loading): the browser, as before.
            Log.w(DoseAlarms.TAG, "Sites: no web view to show the site: " + e.getClass().getSimpleName());
            browse(site.url);
            finish();
            return;
        }
        pages.addView(view, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        String again = SiteWeb.restored(saved, SHOWN);
        view.loadUrl(again != null ? again : site.url);
        shownHost();
    }

    // ---------------------------------------------------------------- The screen

    private int dp(int value)
    {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    /** The bar, the find row, a line, the pages: kept out of the system's bars and above the keyboard. */
    private View layout()
    {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(site.surface);

        LinearLayout bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        bar.setGravity(Gravity.CENTER_VERTICAL);
        bar.setMinimumHeight(dp(52));
        bar.addView(button(R.drawable.sioul_site_back, site.word("back"), v -> back()));
        LinearLayout titles = new LinearLayout(this);
        titles.setOrientation(LinearLayout.VERTICAL);
        titles.setPadding(dp(4), 0, dp(4), 0);
        TextView name = new TextView(this);
        name.setText(site.name);
        name.setTypeface(Typeface.DEFAULT_BOLD);
        name.setTextSize(16);
        name.setTextColor(site.text);
        name.setSingleLine(true);
        name.setEllipsize(TextUtils.TruncateAt.END);
        titles.addView(name);
        host = new TextView(this);
        host.setTextSize(13);
        host.setTextColor(site.muted);
        host.setSingleLine(true);
        // Cut on the left: the registrable part, at its end, stays in sight.
        host.setEllipsize(TextUtils.TruncateAt.START);
        host.setTextDirection(View.TEXT_DIRECTION_LTR);
        titles.addView(host);
        bar.addView(titles, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
        bar.addView(button(R.drawable.sioul_site_reload, site.word("reload"), v -> {
            WebView front = front();
            if (front != null)
                front.reload();
        }));
        more = button(R.drawable.sioul_site_more, site.word("more"), v -> menu());
        bar.addView(more);
        root.addView(bar, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        root.addView(findRow(), new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        View line = new View(this);
        line.setBackgroundColor(site.line);
        root.addView(line, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, Math.max(1, dp(1) / 2)));
        pages = new FrameLayout(this);
        pages.setBackgroundColor(site.background);
        root.addView(pages, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1));
        insets(root);
        return root;
    }

    /**
     * The system's bars and the keyboard: from Android 11, the window drawn
     * under them (Android 15 does so for every app built for it) and the
     * screen padded by their room, the keyboard's included; before, the
     * window kept within them and resized above the keyboard
     * ({@code adjustResize}). Their icons dark over a light bar.
     */
    @SuppressWarnings("deprecation")
    private void insets(LinearLayout root)
    {
        boolean light = SiteWeb.isLight(site.surface);
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            getWindow().setDecorFitsSystemWindows(false);
            root.setOnApplyWindowInsetsListener((v, given) -> {
                Insets room = given.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout() | WindowInsets.Type.ime());
                v.setPadding(room.left, room.top, room.right, room.bottom);
                return WindowInsets.CONSUMED;
            });
            WindowInsetsController bars = getWindow().getInsetsController();
            if (bars != null) {
                int both = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                bars.setSystemBarsAppearance(light ? both : 0, both);
            }
        } else {
            getWindow().setStatusBarColor(site.surface);
            getWindow().setNavigationBarColor(site.surface);
            if (light)
                getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR | View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR);
        }
    }

    private ImageButton button(int icon, String said, View.OnClickListener pressed)
    {
        ImageButton button = new ImageButton(this);
        button.setImageResource(icon);
        button.setImageTintList(ColorStateList.valueOf(site.text));
        // The theme's own round ripple at a press.
        TypedValue ripple = new TypedValue();
        if (getTheme().resolveAttribute(android.R.attr.selectableItemBackgroundBorderless, ripple, true))
            button.setBackgroundResource(ripple.resourceId);
        else
            button.setBackgroundResource(0);
        button.setContentDescription(said);
        button.setTooltipText(said);
        button.setMinimumWidth(dp(48));
        button.setMinimumHeight(dp(48));
        button.setOnClickListener(pressed);
        return button;
    }

    /** Find in the page: the words, how many found and which, previous, next, close. */
    private View findRow()
    {
        finding = new LinearLayout(this);
        finding.setOrientation(LinearLayout.HORIZONTAL);
        finding.setGravity(Gravity.CENTER_VERTICAL);
        finding.setPadding(dp(8), 0, 0, 0);
        finding.setVisibility(View.GONE);
        findField = new EditText(this);
        findField.setSingleLine(true);
        findField.setHint(site.word("find"));
        findField.setImeOptions(EditorInfo.IME_ACTION_SEARCH);
        findField.setTextColor(site.text);
        findField.addTextChangedListener(new TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int start, int count, int after)
            {
            }

            @Override
            public void onTextChanged(CharSequence s, int start, int before, int count)
            {
            }

            @Override
            public void afterTextChanged(Editable s)
            {
                WebView front = front();
                if (front != null)
                    front.findAllAsync(s.toString());
            }
        });
        findField.setOnEditorActionListener((v, action, key) -> {
            WebView front = front();
            if (front != null)
                front.findNext(true);
            return true;
        });
        finding.addView(findField, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
        findCount = new TextView(this);
        findCount.setTextColor(site.muted);
        findCount.setPadding(dp(8), 0, dp(4), 0);
        finding.addView(findCount);
        finding.addView(textButton("‹", site.word("previous"), v -> {
            WebView front = front();
            if (front != null)
                front.findNext(false);
        }));
        finding.addView(textButton("›", site.word("next"), v -> {
            WebView front = front();
            if (front != null)
                front.findNext(true);
        }));
        finding.addView(textButton("×", site.word("close"), v -> closeFind()));
        return finding;
    }

    private TextView textButton(String glyph, String said, View.OnClickListener pressed)
    {
        TextView button = new TextView(this);
        button.setText(glyph);
        button.setTextSize(22);
        button.setTextColor(site.text);
        button.setGravity(Gravity.CENTER);
        button.setContentDescription(said);
        button.setTooltipText(said);
        button.setMinimumWidth(dp(48));
        button.setMinimumHeight(dp(48));
        button.setClickable(true);
        button.setFocusable(true);
        button.setOnClickListener(pressed);
        return button;
    }

    private void openFind()
    {
        finding.setVisibility(View.VISIBLE);
        findField.requestFocus();
        InputMethodManager keyboard = getSystemService(InputMethodManager.class);
        if (keyboard != null)
            keyboard.showSoftInput(findField, InputMethodManager.SHOW_IMPLICIT);
        if (findField.length() > 0) {
            WebView front = front();
            if (front != null)
                front.findAllAsync(findField.getText().toString());
        }
    }

    private void closeFind()
    {
        finding.setVisibility(View.GONE);
        findCount.setText("");
        for (WebView page : new WebView[] { view, popup }) {
            if (page != null)
                page.clearMatches();
        }
        InputMethodManager keyboard = getSystemService(InputMethodManager.class);
        if (keyboard != null)
            keyboard.hideSoftInputFromWindow(findField.getWindowToken(), 0);
    }

    /** The bar's menu: what the site asked for, then the browser, sharing, finding, the pop-up, closing. */
    private void menu()
    {
        PopupMenu menu = new PopupMenu(this, more);
        Menu items = menu.getMenu();
        String asked = SiteWeb.askedLine(site, askedCamera, askedMicrophone, askedLocation);
        if (asked != null)
            items.add(Menu.NONE, 0, 0, asked).setEnabled(false);
        items.add(Menu.NONE, 1, 1, site.word("browser"));
        items.add(Menu.NONE, 2, 2, site.word("share"));
        items.add(Menu.NONE, 3, 3, site.word("find"));
        if (popup != null)
            items.add(Menu.NONE, 4, 4, site.word("close_window"));
        items.add(Menu.NONE, 5, 5, site.word("close"));
        menu.setOnMenuItemClickListener(item -> {
            switch (item.getItemId()) {
            case 1:
                browse(shown());
                return true;
            case 2:
                share(shown());
                return true;
            case 3:
                openFind();
                return true;
            case 4:
                closePopup();
                return true;
            case 5:
                finish();
                return true;
            default:
                return false;
            }
        });
        menu.show();
    }

    /** The page in front: the pop-up while it shows, else the site's. */
    private WebView front()
    {
        return popup != null ? popup : view;
    }

    /** The address of the page in front, as committed; the site's own before any. */
    private String shown()
    {
        String address = popup != null ? popupCommitted : committed;
        return SiteWeb.isHttps(address) ? address : site.url;
    }

    /** The host above the page: the committed page's in front, "…" before it has one. */
    private void shownHost()
    {
        if (host != null)
            host.setText(SiteWeb.shownHost(popup != null ? popupCommitted : committed));
    }

    /** An address in the browser, or another app that opens it; said when none does. */
    private void browse(String address)
    {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(address)).addCategory(Intent.CATEGORY_BROWSABLE));
        } catch (ActivityNotFoundException e) {
            say(site.word("no_browser"));
        }
    }

    /** A mail or telephone address, tapped in a page: the app for it, never another scheme. */
    private void hand(String address)
    {
        try {
            startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(address)));
        } catch (ActivityNotFoundException e) {
            say(site.word("no_browser"));
        }
    }

    /** The page's address shared, through Android's share sheet. */
    private void share(String address)
    {
        Intent send = new Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_TEXT, address);
        try {
            startActivity(Intent.createChooser(send, null));
        } catch (ActivityNotFoundException e) {
            say(site.word("no_browser"));
        }
    }

    private void say(String words)
    {
        Toast.makeText(getApplicationContext(), words, Toast.LENGTH_LONG).show();
    }

    // ---------------------------------------------------------------- The pages

    /** A web view for the site's page, or for a window it opened: the same settings, the same store. */
    private WebView page(boolean opened)
    {
        WebView page = new WebView(this);
        WebSettings settings = page.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        // Nothing of this device's files reaches a page (file:// and content:// are off;
        // a page's file:// reaching others is off by default for apps built after 2012).
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setMixedContentMode(WebSettings.MIXED_CONTENT_NEVER_ALLOW);
        // On, so that a request is heard (and said in the menu), then refused.
        settings.setGeolocationEnabled(true);
        // A window opens only at a tap, through onCreateWindow.
        settings.setSupportMultipleWindows(true);
        settings.setJavaScriptCanOpenWindowsAutomatically(false);
        settings.setSupportZoom(true);
        settings.setBuiltInZoomControls(true);
        settings.setDisplayZoomControls(false);
        settings.setUseWideViewPort(true);
        settings.setLoadWithOverviewMode(true);
        settings.setMediaPlaybackRequiresUserGesture(true);
        settings.setUserAgentString(SiteWeb.userAgent(settings.getUserAgentString()));
        CookieManager cookies = CookieManager.getInstance();
        cookies.setAcceptCookie(true);
        // As a computer's Sioul: a sign-in in a frame of another site keeps its cookie.
        cookies.setAcceptThirdPartyCookies(page, true);
        page.setBackgroundColor(site.background);
        page.setWebViewClient(new Client(opened));
        page.setWebChromeClient(new Chrome());
        page.setDownloadListener(this::download);
        page.setFindListener((at, count, done) -> {
            if (done)
                findCount.setText(count == 0 ? site.word("find_none") : (at + 1) + "/" + count);
        });
        return page;
    }

    /** Where each page goes, the host shown, a drawing process that ends. */
    private final class Client extends WebViewClient
    {
        private final boolean opened;

        Client(boolean opened)
        {
            this.opened = opened;
        }

        @Override
        public boolean shouldOverrideUrlLoading(WebView page, WebResourceRequest request)
        {
            String url = request.getUrl().toString();
            String current = page == popup ? popupCommitted : committed;
            switch (SiteWeb.where(url, request.isForMainFrame(), request.hasGesture(), request.isRedirect(), current, site.domain, opened)) {
            case LOAD:
                return false;
            case BROWSER:
                browse(url);
                return true;
            case OTHER_APP:
                hand(url);
                return true;
            default:
                if (request.isForMainFrame())
                    say(site.word("not_https"));
                return true;
            }
        }

        // The page committed, its own or a same-document change: its address is the one shown.
        @Override
        public void doUpdateVisitedHistory(WebView page, String url, boolean reload)
        {
            committed(page, url);
        }

        @Override
        public void onPageCommitVisible(WebView page, String url)
        {
            committed(page, url);
        }

        @Override
        public boolean onRenderProcessGone(WebView page, RenderProcessGoneDetail detail)
        {
            // Its drawing process ended (the page's fault, or Android took its memory back):
            // the web views of this screen cannot be used again; the screen goes, Sioul stays.
            end();
            return true;
        }
    }

    private void committed(WebView page, String url)
    {
        if (page == view)
            committed = url == null ? "" : url;
        else if (page == popup && popup != null)
            popupCommitted = url == null ? "" : url;
        shownHost();
    }

    /** What a page asks of the screen: permissions, files, windows; its console is never logged. */
    private final class Chrome extends WebChromeClient
    {
        @Override
        public void onPermissionRequest(PermissionRequest request)
        {
            for (String resource : request.getResources()) {
                if (PermissionRequest.RESOURCE_VIDEO_CAPTURE.equals(resource))
                    askedCamera = true;
                else if (PermissionRequest.RESOURCE_AUDIO_CAPTURE.equals(resource))
                    askedMicrophone = true;
            }
            // Refused in this phase: the next asks you, site by site.
            request.deny();
        }

        @Override
        public void onGeolocationPermissionsShowPrompt(String origin, GeolocationPermissions.Callback callback)
        {
            askedLocation = true;
            callback.invoke(origin, false, false);
        }

        @Override
        public boolean onShowFileChooser(WebView page, ValueCallback<Uri[]> callback, FileChooserParams params)
        {
            if (files != null)
                files.onReceiveValue(null);
            files = null;
            // A file to save or a folder (a recent web view's other modes): not answered.
            if (!SiteWeb.choosesFiles(params.getMode())) {
                callback.onReceiveValue(null);
                return true;
            }
            files = callback;
            Intent pick = params.createIntent();
            if (params.getMode() == FileChooserParams.MODE_OPEN_MULTIPLE)
                pick.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
            // Each request its own code: a late answer to an earlier one never reaches this one.
            filesAsked = FILES + (filesAsked - FILES + 1) % FILES_SPAN;
            try {
                startActivityForResult(pick, filesAsked);
            } catch (ActivityNotFoundException e) {
                files = null;
                callback.onReceiveValue(null);
            }
            return true;
        }

        @Override
        public boolean onCreateWindow(WebView page, boolean dialog, boolean gesture, Message result)
        {
            // At a tap only, from the site's page, one at a time.
            if (!gesture || page != view || popup != null || ended)
                return false;
            try {
                popup = page(true);
            } catch (RuntimeException e) {
                return false;
            }
            popupCommitted = "";
            pages.addView(popup, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
            ((WebView.WebViewTransport) result.obj).setWebView(popup);
            result.sendToTarget();
            shownHost();
            return true;
        }

        @Override
        public void onCloseWindow(WebView window)
        {
            if (window == popup)
                closePopup();
        }

        @Override
        public boolean onConsoleMessage(ConsoleMessage message)
        {
            // A page's console never reaches Android's log.
            return true;
        }
    }

    /** The window the page opened goes; the site's page is in front again. */
    private void closePopup()
    {
        if (popup == null)
            return;
        WebView gone = popup;
        popup = null;
        popupCommitted = "";
        pages.removeView(gone);
        gone.destroy();
        shownHost();
    }

    /** The drawing process ended: every web view of the screen goes, then the screen; said once. */
    private void end()
    {
        if (ended)
            return;
        ended = true;
        Log.i(DoseAlarms.TAG, "Sites: a page's drawing process ended; its screen closes.");
        say(site.word("ended"));
        for (WebView page : new WebView[] { popup, view }) {
            if (page != null) {
                pages.removeView(page);
                page.destroy();
            }
        }
        popup = null;
        view = null;
        finish();
    }

    /**
     * A download: an https file is fetched by Android's download service
     * with the site's cookies, into Downloads, Android's one notice saying
     * when it is there and opening it; on Android 9 without the storage
     * permission, into Sioul's own downloads folder. A file the page made
     * itself cannot be fetched again: said.
     */
    private void download(String url, String agent, String disposition, String type, long length)
    {
        if (url == null || !SiteWeb.isHttps(url)) {
            say(url != null && (url.startsWith("blob:") || url.startsWith("data:")) ? site.word("download_page") : site.word("not_https"));
            return;
        }
        String name = SiteWeb.fileName(URLUtil.guessFileName(url, disposition, type));
        DownloadManager service = getSystemService(DownloadManager.class);
        if (service == null) {
            say(site.word("download_failed"));
            return;
        }
        for (boolean shared : new boolean[] { true, false }) {
            try {
                service.enqueue(fetch(url, agent, type, name, shared));
                say(site.word("downloading").replace("{file}", name));
                return;
            } catch (SecurityException | IllegalStateException | IllegalArgumentException e) {
                Log.i(DoseAlarms.TAG, "Sites: a download refused into " + (shared ? "Downloads" : "Sioul's own folder") + ": " + e.getClass().getSimpleName());
            }
        }
        say(site.word("download_failed"));
    }

    private DownloadManager.Request fetch(String url, String agent, String type, String name, boolean shared)
    {
        DownloadManager.Request request = new DownloadManager.Request(Uri.parse(url));
        String cookies = CookieManager.getInstance().getCookie(url);
        if (cookies != null && !cookies.isEmpty())
            request.addRequestHeader("Cookie", cookies);
        if (agent != null && !agent.isEmpty())
            request.addRequestHeader("User-Agent", agent);
        if (type != null && !type.isEmpty())
            request.setMimeType(type);
        request.setTitle(name);
        request.setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_ONLY_COMPLETION);
        if (shared)
            request.setDestinationInExternalPublicDir(Environment.DIRECTORY_DOWNLOADS, name);
        else
            request.setDestinationInExternalFilesDir(this, Environment.DIRECTORY_DOWNLOADS, name);
        return request;
    }

    // ---------------------------------------------------------------- Android's calls

    /** Back: out of finding, then back in the page in front, then the pop-up closed, then the screen. */
    private void back()
    {
        if (finding.getVisibility() == View.VISIBLE) {
            closeFind();
            return;
        }
        WebView front = front();
        if (front != null && front.canGoBack()) {
            front.goBack();
            return;
        }
        if (popup != null) {
            closePopup();
            return;
        }
        finish();
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed()
    {
        back();
    }

    @Override
    public boolean onKeyDown(int code, KeyEvent event)
    {
        // A hardware keyboard's Ctrl+F finds in the page, as in a browser.
        if (code == KeyEvent.KEYCODE_F && event.isCtrlPressed() && view != null) {
            openFind();
            return true;
        }
        return super.onKeyDown(code, event);
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data)
    {
        if (request >= FILES && request < FILES + FILES_SPAN) {
            // Only the answer to the request waiting, checked before the page has it (review F1).
            if (request == filesAsked && files != null) {
                files.onReceiveValue(SiteWeb.chosen(result == RESULT_OK, data, getPackageName()));
                files = null;
            }
            return;
        }
        super.onActivityResult(request, result, data);
    }

    @Override
    protected void onPause()
    {
        // What the sites keep, written now: Android may end the process while it is away.
        CookieManager.getInstance().flush();
        for (WebView page : new WebView[] { view, popup }) {
            if (page != null)
                page.onPause();
        }
        super.onPause();
    }

    @Override
    protected void onResume()
    {
        super.onResume();
        for (WebView page : new WebView[] { view, popup }) {
            if (page != null)
                page.onResume();
        }
    }

    @Override
    protected void onSaveInstanceState(Bundle out)
    {
        super.onSaveInstanceState(out);
        String kept = SiteWeb.kept(committed);
        if (kept != null)
            out.putString(SHOWN, kept);
    }

    @Override
    protected void onDestroy()
    {
        if (files != null) {
            files.onReceiveValue(null);
            files = null;
        }
        for (WebView page : new WebView[] { popup, view }) {
            if (page != null) {
                if (page.getParent() instanceof ViewGroup)
                    ((ViewGroup) page.getParent()).removeView(page);
                page.destroy();
            }
        }
        popup = null;
        view = null;
        CookieManager.getInstance().flush();
        super.onDestroy();
    }
}
