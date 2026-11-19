// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.ContentResolver;
import android.content.ContentValues;
import android.content.Context;
import android.content.pm.PackageManager;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.DocumentsContract;
import android.provider.MediaStore;
import android.util.Log;
import android.webkit.CookieManager;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URI;
import java.net.URISyntaxException;
import java.net.URL;
import java.util.Locale;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * A file a site offers, fetched by Sioul itself (review F3, F2): in the
 * site's own session (the web view's cookies for that address, sent to its
 * host alone), never through Android's download service, which would keep
 * them in its own database and send them on to every host the address
 * redirects to. A redirect is followed on the same host only; a page where a
 * file was expected (an error, a sign-in asked again: a link that works
 * once), a file a form made (asked again, it would not be the same), a file
 * larger than {@link #LIMIT} or than the room left: each is said, nothing is
 * saved, and the screen offers the page in the browser. Nothing is written
 * before the answer is known to be the file; a file cut short is deleted.
 *
 * <p>The pure parts are checked on a JVM (android/jvm-checks/SiteScreenCheck):
 * {@link #sameHost}, {@link #verdict}, {@link #roomFor}, {@link #isProgram}.
 */
final class SiteDownload
{
    private SiteDownload()
    {
    }

    /** The largest file a site's screen saves: 1 GiB. */
    static final long LIMIT = 1L << 30;
    /** What is left on the device's storage after a file, at the least: 200 MiB. */
    static final long MARGIN = 200L << 20;
    /** Redirects followed on the same host, at most. */
    static final int REDIRECTS = 5;

    /** Why a file was not saved: each has its words (SiteWeb.ENGLISH, "why_…"). */
    enum Why { FORM, ELSEWHERE, PAGE, LARGE, ROOM, NETWORK, WRITE, CANCELLED }

    /** The places a file may go, each with its words ("place_…"), in the order the question shows them. */
    static final String DOWNLOADS = "downloads";
    static final String CHOOSE = "choose";
    /** Where this Java can save, in the question's order (Notes, Papers, Letters come with Rust's help: docs/sites.md). */
    static final String[] PLACES = { DOWNLOADS, CHOOSE };

    // ---------------------------------------------------------------- Pure rules

    /**
     * Where a redirect may go: the address it names, resolved against the
     * one it answered, when it is https on the same host and port; else
     * null, and the file is not fetched there (the site's cookies would go
     * to another host).
     *
     * @param from the address that answered with a redirect
     * @param location its Location header, or null
     * @return the next address, or null
     */
    static String sameHost(String from, String location)
    {
        if (from == null || location == null || location.isEmpty())
            return null;
        try {
            URI base = new URI(from);
            URI next = base.resolve(new URI(location.trim()));
            if (!"https".equalsIgnoreCase(next.getScheme()) || next.getHost() == null || base.getHost() == null)
                return null;
            if (!next.getHost().equalsIgnoreCase(base.getHost()) || port(next) != port(base) || next.getRawUserInfo() != null)
                return null;
            return next.toString();
        } catch (URISyntaxException | IllegalArgumentException e) {
            return null;
        }
    }

    private static int port(URI address)
    {
        return address.getPort() < 0 ? 443 : address.getPort();
    }

    /**
     * Whether an answer is the file the page offered: status 200, and not a
     * page (HTML) where the web view had a file of another type, unless the
     * site says it is an attachment. Null when it is; else why not.
     *
     * @param status the HTTP status
     * @param expected the type the web view saw, or null
     * @param got the type of this answer, or null
     * @param disposition this answer's Content-Disposition, or null
     * @return null, or {@link Why#PAGE}
     */
    static Why verdict(int status, String expected, String got, String disposition)
    {
        if (status != HttpURLConnection.HTTP_OK)
            return Why.PAGE;
        String exp = base(expected);
        String answer = base(got);
        boolean page = answer.equals("text/html") || answer.equals("application/xhtml+xml");
        boolean attachment = disposition != null && disposition.toLowerCase(Locale.ROOT).trim().startsWith("attachment");
        if (page && !exp.equals("text/html") && !exp.equals("application/xhtml+xml") && !attachment)
            return Why.PAGE;
        return null;
    }

    /** A type without its parameters, in lower case; "" for none. */
    static String base(String type)
    {
        if (type == null)
            return "";
        int semi = type.indexOf(';');
        return (semi >= 0 ? type.substring(0, semi) : type).trim().toLowerCase(Locale.ROOT);
    }

    /**
     * Whether a file of {@code length} bytes (-1: not said) may be fetched
     * with {@code usable} bytes free: never above {@link #LIMIT}, and
     * {@link #MARGIN} left after it. Null when it may; else why not.
     *
     * @param length the file's length as the site says it; -1 when unknown
     * @param usable the room free on the device's storage
     * @return null, or {@link Why#LARGE} or {@link Why#ROOM}
     */
    static Why roomFor(long length, long usable)
    {
        if (length > LIMIT)
            return Why.LARGE;
        if (usable - Math.max(0, length) < MARGIN)
            return Why.ROOM;
        return null;
    }

    /**
     * Whether a file is a program (an app, an installer, a script): the
     * question then says one line of caution.
     *
     * @param name its name
     * @param type its type, or null
     * @return whether it is one
     */
    static boolean isProgram(String name, String type)
    {
        String lower = name == null ? "" : name.toLowerCase(Locale.ROOT);
        String[] endings = { ".apk", ".apks", ".xapk", ".aab", ".exe", ".msi", ".bat", ".cmd", ".com", ".scr", ".ps1", ".vbs", ".jar", ".sh", ".run", ".bin", ".deb", ".rpm", ".appimage", ".dmg", ".pkg", ".app" };
        for (String ending : endings) {
            if (lower.endsWith(ending))
                return true;
        }
        String kind = base(type);
        return kind.equals("application/vnd.android.package-archive") || kind.equals("application/x-msdownload") || kind.equals("application/x-msdos-program")
            || kind.equals("application/x-executable") || kind.equals("application/x-sh") || kind.equals("application/java-archive");
    }

    // ---------------------------------------------------------------- The fetch

    /** One file at a time, off Android's main thread. */
    private static final ExecutorService FETCHING = Executors.newSingleThreadExecutor(work -> {
        Thread thread = new Thread(work, "sioul-site-download");
        thread.setDaemon(true);
        return thread;
    });

    /** What the screen hears of a fetch, on Android's main thread. */
    interface Heard
    {
        /** So far: bytes fetched, and the length when the site said it (-1 else). */
        void progress(long fetched, long length);

        /** Saved: the address of the file, and whether it went to Sioul's own folder instead of Downloads. */
        void saved(Uri file, boolean own);

        /** Not saved, and why. */
        void failed(Why why);
    }

    /** A file to fetch: what the page offered, where it goes, how to tell the screen. */
    static final class Job
    {
        final String url;
        final String agent;
        final String type;
        final String referrer;
        final String name;
        final String place;
        /** The document Android's "Save as" made (place CHOOSE); null for Downloads. */
        final Uri chosen;
        volatile boolean cancelled;

        Job(String url, String agent, String type, String referrer, String name, String place, Uri chosen)
        {
            this.url = url;
            this.agent = agent;
            this.type = type;
            this.referrer = referrer;
            this.name = name;
            this.place = place;
            this.chosen = chosen;
        }
    }

    /**
     * The file fetched and saved, off the main thread; what happens said
     * through {@code heard}, posted by {@code main}.
     *
     * @param context the application's context
     * @param job the file
     * @param main runs a report on Android's main thread
     * @param heard the screen's ears
     */
    static void fetch(Context context, Job job, java.util.function.Consumer<Runnable> main, Heard heard)
    {
        FETCHING.execute(() -> {
            Why why;
            try {
                why = run(context, job, main, heard);
            } catch (RuntimeException e) {
                Log.w(DoseAlarms.TAG, "Sites: a download failed: " + e.getClass().getSimpleName());
                why = Why.NETWORK;
            }
            if (why != null) {
                Why said = job.cancelled ? Why.CANCELLED : why;
                if (job.chosen != null)
                    forget(context, job.chosen);
                main.accept(() -> heard.failed(said));
            }
        });
    }

    /** The fetch itself: null once saved (said), else why not. */
    private static Why run(Context context, Job job, java.util.function.Consumer<Runnable> main, Heard heard)
    {
        String address = job.url;
        HttpURLConnection answer = null;
        try {
            for (int hop = 0;; hop++) {
                if (job.cancelled)
                    return Why.CANCELLED;
                answer = (HttpURLConnection) new URL(address).openConnection();
                answer.setInstanceFollowRedirects(false);
                answer.setConnectTimeout(20_000);
                answer.setReadTimeout(30_000);
                if (job.agent != null && !job.agent.isEmpty())
                    answer.setRequestProperty("User-Agent", job.agent);
                if (job.referrer != null && !job.referrer.isEmpty())
                    answer.setRequestProperty("Referer", job.referrer);
                // The site's session for this address, as the web view would send it: to this host alone.
                String cookies = CookieManager.getInstance().getCookie(address);
                if (cookies != null && !cookies.isEmpty())
                    answer.setRequestProperty("Cookie", cookies);
                int status = answer.getResponseCode();
                if (status == 301 || status == 302 || status == 303 || status == 307 || status == 308) {
                    String next = sameHost(address, answer.getHeaderField("Location"));
                    answer.disconnect();
                    answer = null;
                    if (next == null)
                        return Why.ELSEWHERE;
                    if (hop + 1 >= REDIRECTS)
                        return Why.PAGE;
                    address = next;
                    continue;
                }
                Why not = verdict(status, job.type, answer.getContentType(), answer.getHeaderField("Content-Disposition"));
                if (not != null)
                    return not;
                long length = answer.getContentLengthLong();
                Why room = roomFor(length, Environment.getDataDirectory().getUsableSpace());
                if (room != null)
                    return room;
                break;
            }
            Target target = target(context, job);
            if (target == null)
                return Why.WRITE;
            long length = answer.getContentLengthLong();
            long fetched = 0;
            long told = 0;
            boolean done = false;
            try (InputStream in = answer.getInputStream(); OutputStream out = target.out) {
                byte[] buffer = new byte[64 * 1024];
                main.accept(() -> heard.progress(0, length));
                for (int read; (read = in.read(buffer)) >= 0;) {
                    if (job.cancelled)
                        return Why.CANCELLED;
                    fetched += read;
                    if (fetched > LIMIT)
                        return Why.LARGE;
                    if ((fetched & ~0x7FFFFF) != ((fetched - read) & ~0x7FFFFF) && Environment.getDataDirectory().getUsableSpace() < MARGIN)
                        return Why.ROOM;
                    out.write(buffer, 0, read);
                    long now = System.currentTimeMillis();
                    if (now - told >= 250) {
                        told = now;
                        long so = fetched;
                        main.accept(() -> heard.progress(so, length));
                    }
                }
                if (length >= 0 && fetched != length)
                    return Why.NETWORK;
                done = true;
            } finally {
                if (!done)
                    target.undo(context);
            }
            Uri file = target.finish(context);
            if (file == null)
                return Why.WRITE;
            main.accept(() -> heard.saved(file, target.own));
            Log.i(DoseAlarms.TAG, "Sites: a file saved, " + (target.own ? "in Sioul's own folder." : "where it was asked."));
            return null;
        } catch (IOException e) {
            Log.i(DoseAlarms.TAG, "Sites: a download stopped: " + e.getClass().getSimpleName());
            return job.cancelled ? Why.CANCELLED : Why.NETWORK;
        } finally {
            if (answer != null)
                answer.disconnect();
        }
    }

    /** Where the bytes go, and how the file is made visible or taken back. */
    private static final class Target
    {
        OutputStream out;
        /** Downloads on Android 10 and later: the row in MediaStore, pending until done. */
        Uri row;
        /** Android 9: the file itself. */
        File file;
        /** Android 9 without the storage permission: Sioul's own downloads folder. */
        boolean own;
        /** The document Android's "Save as" made. */
        Uri chosen;

        /** The file made visible; its address to open it, or null. */
        Uri finish(Context context)
        {
            try {
                if (row != null) {
                    ContentValues done = new ContentValues();
                    done.put(MediaStore.MediaColumns.IS_PENDING, 0);
                    context.getContentResolver().update(row, done, null, null);
                    return row;
                }
                if (chosen != null)
                    return chosen;
                if (file != null) {
                    android.media.MediaScannerConnection.scanFile(context, new String[] { file.getPath() }, null, null);
                    String root = own ? "external_files_path" : "external_path";
                    File folder = own ? context.getExternalFilesDir(null) : Environment.getExternalStorageDirectory();
                    String address = folder == null ? null : Attachments.contentUri(context.getPackageName(), folder.getCanonicalPath(), file.getCanonicalPath(), root);
                    return address == null ? null : Uri.parse(address);
                }
            } catch (IOException | RuntimeException e) {
                Log.w(DoseAlarms.TAG, "Sites: a file saved could not be shown: " + e.getClass().getSimpleName());
            }
            return null;
        }

        /** Nothing half-written stays. */
        void undo(Context context)
        {
            try {
                if (row != null)
                    context.getContentResolver().delete(row, null, null);
                if (file != null && !file.delete())
                    Log.w(DoseAlarms.TAG, "Sites: a file cut short could not be deleted.");
            } catch (RuntimeException e) {
                Log.w(DoseAlarms.TAG, "Sites: a file cut short could not be taken back: " + e.getClass().getSimpleName());
            }
        }
    }

    /** Where the file is written: the place asked; null when it cannot be opened. */
    private static Target target(Context context, Job job)
    {
        Target target = new Target();
        ContentResolver resolver = context.getContentResolver();
        try {
            if (job.chosen != null) {
                target.chosen = job.chosen;
                target.out = resolver.openOutputStream(job.chosen, "wt");
                return target.out == null ? null : target;
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                ContentValues file = new ContentValues();
                file.put(MediaStore.MediaColumns.DISPLAY_NAME, job.name);
                if (job.type != null && !base(job.type).isEmpty())
                    file.put(MediaStore.MediaColumns.MIME_TYPE, base(job.type));
                file.put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS);
                file.put(MediaStore.MediaColumns.IS_PENDING, 1);
                target.row = resolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, file);
                if (target.row == null)
                    return null;
                target.out = resolver.openOutputStream(target.row, "w");
                if (target.out == null) {
                    target.undo(context);
                    return null;
                }
                return target;
            }
            // Android 9: the public Downloads with the storage permission, else Sioul's own folder, said.
            boolean allowed = context.checkSelfPermission(android.Manifest.permission.WRITE_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED;
            File folder = allowed ? Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS) : context.getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS);
            if (folder == null || (!folder.isDirectory() && !folder.mkdirs()))
                return null;
            target.own = !allowed;
            target.file = unique(folder, job.name);
            target.out = new FileOutputStream(target.file);
            return target;
        } catch (IOException | RuntimeException e) {
            Log.w(DoseAlarms.TAG, "Sites: where to save a file could not be opened: " + e.getClass().getSimpleName());
            target.undo(context);
            return null;
        }
    }

    /** A name not taken in {@code folder}: "name.pdf", else "name (2).pdf"…; never a file replaced. */
    static File unique(File folder, String name)
    {
        File file = new File(folder, name);
        int dot = name.lastIndexOf('.');
        String stem = dot > 0 ? name.substring(0, dot) : name;
        String ext = dot > 0 ? name.substring(dot) : "";
        for (int n = 2; file.exists() && n < 1000; n++)
            file = new File(folder, stem + " (" + n + ")" + ext);
        return file;
    }

    /** The document Android's "Save as" made, deleted when nothing was saved in it. */
    private static void forget(Context context, Uri chosen)
    {
        try {
            DocumentsContract.deleteDocument(context.getContentResolver(), chosen);
        } catch (IOException | RuntimeException e) {
            Log.i(DoseAlarms.TAG, "Sites: the empty file asked for could not be deleted: " + e.getClass().getSimpleName());
        }
    }
}
