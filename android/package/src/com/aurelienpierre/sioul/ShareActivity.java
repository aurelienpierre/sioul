// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ProviderInfo;
import android.content.res.AssetFileDescriptor;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.Parcelable;
import android.os.Process;
import android.provider.OpenableColumns;
import android.content.pm.ShortcutManager;
import android.util.Log;
import android.webkit.MimeTypeMap;
import android.widget.Toast;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;

/**
 * What other apps hand Sioul to write (docs/android.md, "Sharing"): files and
 * text shared to it, or to one of your addresses straight from the share
 * sheet (MailShortcuts), and mailto: links. Each becomes a request in Sioul's
 * data folder (handed/incoming/<id>.json), which Rust makes a draft of as
 * its window comes up (crates/sioul-app/src/outside.rs); Sioul's window is
 * brought up at once.
 *
 * Shared files come as content:// addresses that Sioul may read only while
 * this activity lives. They are opened on a thread of their own (a cloud's
 * app may fetch a file before it opens), then this activity ends, and the copy
 * goes on from the open files into handed/files/<id>/, under their own names,
 * while you write. When all are copied, handed/incoming/<id>.done.json says
 * where, and Rust is told (nativeHanded) to attach them. Never shown itself.
 *
 * Not taken: a file named by its place (file://), and Sioul's own files,
 * which any app could otherwise ask Sioul to attach. Nothing is sent until
 * you press Send.
 */
public final class ShareActivity extends Activity
{
    /** The account a launcher's shortcut writes from (MailShortcuts). */
    static final String ACCOUNT = "com.aurelienpierre.sioul.extra.ACCOUNT";
    /** Sioul's shortcuts for your addresses: "mail:" and the account's id. */
    static final String PREFIX = "mail:";
    /** More files than this in one share are not taken. */
    private static final int MOST = 100;
    /** Names kept under this many bytes: Android's file names hold 255. */
    private static final int LONGEST = 200;

    /** Rust told that something was handed (android/main.cpp, outside.rs). */
    static native void nativeHanded();

    /** A shared file, open, or why not. */
    private static final class Opened
    {
        final String name;
        final String type;
        AssetFileDescriptor file;
        String error;

        Opened(String name, String type)
        {
            this.name = name;
            this.type = type;
        }
    }

    @Override
    protected void onCreate(Bundle state)
    {
        super.onCreate(state);
        // Made again (the manifest keeps it through a turn of the screen): the
        // share was taken the first time.
        if (state != null) {
            finish();
            return;
        }
        final Context app = getApplicationContext();
        final Intent intent = getIntent();
        final File handed = handed(app);
        final String id = Long.toHexString(System.currentTimeMillis()) + "-" + Long.toHexString(System.nanoTime() & 0xffffffL) + "-" + Integer.toHexString(Process.myPid());
        final List<Uri> files = new ArrayList<>();
        final JSONArray refused = new JSONArray();
        try {
            for (Uri uri : streams(intent)) {
                if (files.size() < MOST && taken(uri))
                    files.add(uri);
                else
                    refused.put(lastPart(uri));
            }
            JSONObject request = request(intent, id, files.size(), refused);
            write(new File(handed, "incoming/" + id + ".json"), request.toString());
        } catch (JSONException | IOException | RuntimeException e) {
            Log.e(DoseAlarms.TAG, "Sharing: what was shared could not be kept for Sioul: " + e);
            Toast.makeText(app, failed(e), Toast.LENGTH_LONG).show();
            finish();
            return;
        }
        used(app, intent);
        open();
        if (files.isEmpty()) {
            // Sioul's window, already up (split screen), takes it now; else when it comes up.
            new Thread(ShareActivity::tell, "sioul-share").start();
            finish();
            return;
        }
        new Thread(() -> copy(app, handed, id, files), "sioul-share").start();
    }

    /**
     * Where Rust takes what is handed (sioul_core::handed): XDG_DATA_HOME/sioul/handed
     * (android/main.cpp). Not the drafts' folder: the sharing carries that one to
     * your other devices, and a request is this phone's.
     */
    static File handed(Context context)
    {
        return new File(context.getFilesDir(), "data/sioul/handed");
    }

    /**
     * "Sioul could not take what you shared: …", in the phone's language.
     * Kept here, not among Android's strings: Qt's build keeps English ones
     * only (its template's resConfig), and Rust's words are not loaded yet.
     */
    private static String failed(Exception e)
    {
        String why = why(e);
        boolean french = "fr".equals(Locale.getDefault().getLanguage());
        return french ? "Sioul n\u2019a pas pu prendre ce que vous avez partag\u00e9\u202f: " + why : "Sioul could not take what you shared: " + why;
    }

    /** Sioul's window brought up, as by its icon. */
    private void open()
    {
        Intent sioul = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (sioul != null)
            startActivity(sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED));
    }

    /** Rust told, when Sioul's library is loaded (its window up); else it takes the request as the window comes up. */
    private static void tell()
    {
        try {
            nativeHanded();
        } catch (UnsatisfiedLinkError e) {
            // Not loaded yet: the window's start takes it.
        }
    }

    /** An address of yours chosen in the share sheet: Android ranks it higher next time. */
    private static void used(Context app, Intent intent)
    {
        try {
            String shortcut = intent.getStringExtra(Intent.EXTRA_SHORTCUT_ID);
            if (shortcut == null || !shortcut.startsWith(PREFIX))
                return;
            ShortcutManager manager = app.getSystemService(ShortcutManager.class);
            if (manager != null)
                manager.reportShortcutUsed(shortcut);
        } catch (RuntimeException e) {
            // Only a ranking lost.
        }
    }

    // ---------------------------------------------------------------- the request

    /** What Rust reads (sioul_core::handed::Handed). */
    private static JSONObject request(Intent intent, String id, int files, JSONArray refused) throws JSONException
    {
        JSONObject request = new JSONObject();
        request.put("id", id);
        request.put("account", account(intent));
        Uri data = intent.getData();
        if (data != null && "mailto".equalsIgnoreCase(data.getScheme()))
            request.put("mailto", data.toString());
        Bundle extras = intent.getExtras();
        request.put("to", strings(extras, Intent.EXTRA_EMAIL));
        request.put("cc", strings(extras, Intent.EXTRA_CC));
        request.put("bcc", strings(extras, Intent.EXTRA_BCC));
        request.put("subject", text(extras, Intent.EXTRA_SUBJECT));
        request.put("text", text(extras, Intent.EXTRA_TEXT));
        request.put("files", files);
        request.put("copier", Process.myPid());
        request.put("refused", refused);
        return request;
    }

    /** The account chosen: an address of yours in the share sheet, or a launcher's shortcut; "" for Sioul's usual one. */
    private static String account(Intent intent)
    {
        String shortcut = intent.getStringExtra(Intent.EXTRA_SHORTCUT_ID);
        if (shortcut != null && shortcut.startsWith(PREFIX))
            return shortcut.substring(PREFIX.length());
        String chosen = intent.getStringExtra(ACCOUNT);
        return chosen == null ? "" : chosen;
    }

    /** Addresses, however the app gave them: one, an array, a list. */
    @SuppressWarnings("deprecation")
    private static JSONArray strings(Bundle extras, String key)
    {
        JSONArray out = new JSONArray();
        Object value = extras == null ? null : extras.get(key);
        if (value instanceof CharSequence) {
            out.put(value.toString());
        } else if (value instanceof String[]) {
            for (String one : (String[]) value) {
                if (one != null)
                    out.put(one);
            }
        } else if (value instanceof Collection) {
            for (Object one : (Collection<?>) value) {
                if (one != null)
                    out.put(one.toString());
            }
        }
        return out;
    }

    /** A text, however the app gave it: a string, styled text, a list of them (one per paragraph). */
    @SuppressWarnings("deprecation")
    private static String text(Bundle extras, String key)
    {
        Object value = extras == null ? null : extras.get(key);
        if (value instanceof CharSequence)
            return value.toString();
        if (value instanceof Collection) {
            StringBuilder joined = new StringBuilder();
            for (Object one : (Collection<?>) value) {
                if (one == null)
                    continue;
                if (joined.length() > 0)
                    joined.append("\n\n");
                joined.append(one);
            }
            return joined.toString();
        }
        return "";
    }

    // ---------------------------------------------------------------- the files

    /**
     * The files shared: EXTRA_STREAM, one or a list, each once. Not the
     * clip's addresses: beside a text, an app may put there the picture the
     * share sheet shows, which is no attachment.
     */
    @SuppressWarnings("deprecation")
    private static List<Uri> streams(Intent intent)
    {
        Set<Uri> found = new LinkedHashSet<>();
        String action = intent.getAction();
        if (Intent.ACTION_SEND.equals(action)) {
            Object one = intent.getParcelableExtra(Intent.EXTRA_STREAM);
            if (one instanceof Uri)
                found.add((Uri) one);
        } else if (Intent.ACTION_SEND_MULTIPLE.equals(action)) {
            ArrayList<Parcelable> many = intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM);
            if (many != null) {
                for (Object one : many) {
                    if (one instanceof Uri)
                        found.add((Uri) one);
                }
            }
        }
        return new ArrayList<>(found);
    }

    /**
     * Whether a shared address is taken: a content:// one, of another app's
     * provider. Not a file:// one, nor one of Sioul's own providers: any app
     * could name Sioul's private files that way, to have them attached.
     */
    private boolean taken(Uri uri)
    {
        if (!ContentResolver.SCHEME_CONTENT.equals(uri.getScheme()) || uri.getAuthority() == null)
            return false;
        ProviderInfo provider = getPackageManager().resolveContentProvider(uri.getAuthority(), 0);
        return provider == null || !getPackageName().equals(provider.packageName);
    }

    private static String lastPart(Uri uri)
    {
        String last = uri.getLastPathSegment();
        return last == null || last.isEmpty() ? "?" : last;
    }

    /**
     * On a thread of its own: each file opened, then this activity let go (its
     * permission with it: the files stay open), then each copied, then Rust told.
     */
    private void copy(Context app, File handed, String id, List<Uri> uris)
    {
        ContentResolver resolver = app.getContentResolver();
        List<Opened> opened = new ArrayList<>();
        for (Uri uri : uris) {
            String type = null;
            try {
                type = resolver.getType(uri);
            } catch (RuntimeException e) {
                // No type said: the name's extension decides.
            }
            Opened file = new Opened(name(resolver, uri), type);
            try {
                file.file = resolver.openAssetFileDescriptor(uri, "r");
                if (file.file == null)
                    file.error = "no file";
            } catch (FileNotFoundException | RuntimeException e) {
                file.error = why(e);
            }
            opened.add(file);
        }
        runOnUiThread(this::finish);
        File folder = new File(handed, "files/" + id);
        JSONArray done = new JSONArray();
        Set<String> names = new HashSet<>();
        for (Opened file : opened) {
            JSONObject said = new JSONObject();
            try {
                said.put("name", file.name);
                if (file.error == null) {
                    String name = unique(names, cleaned(file.name, file.type));
                    File copy = new File(folder, name);
                    try {
                        store(file.file, folder, copy);
                        said.put("name", name);
                        said.put("path", copy.getAbsolutePath());
                    } catch (IOException | RuntimeException e) {
                        said.put("error", why(e));
                    }
                } else {
                    said.put("error", file.error);
                }
            } catch (JSONException e) {
                Log.e(DoseAlarms.TAG, "Sharing: " + e);
            }
            done.put(said);
        }
        try {
            write(new File(handed, "incoming/" + id + ".done.json"), new JSONObject().put("files", done).toString());
        } catch (JSONException | IOException e) {
            Log.e(DoseAlarms.TAG, "Sharing: the copied files could not be told to Sioul: " + e);
        }
        Log.i(DoseAlarms.TAG, "Sharing: " + done.length() + " files copied for a draft.");
        tell();
    }

    /** A file copied whole, or not at all: under ".part" until it is. The open file closed after. */
    private static void store(AssetFileDescriptor file, File folder, File copy) throws IOException
    {
        if (!folder.isDirectory() && !folder.mkdirs())
            throw new IOException("cannot make " + folder);
        File part = new File(folder, copy.getName() + ".part");
        try (InputStream in = file.createInputStream(); OutputStream out = new FileOutputStream(part)) {
            byte[] buffer = new byte[64 * 1024];
            int read;
            while ((read = in.read(buffer)) > 0)
                out.write(buffer, 0, read);
        } catch (IOException e) {
            part.delete();
            throw e;
        } finally {
            try {
                file.close();
            } catch (IOException e) {
                // Closed with its stream already.
            }
        }
        if (!part.renameTo(copy)) {
            part.delete();
            throw new IOException("cannot name " + copy.getName());
        }
    }

    /** A file's name as its app gives it, else the end of its address. */
    private static String name(ContentResolver resolver, Uri uri)
    {
        try (Cursor row = resolver.query(uri, new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null)) {
            if (row != null && row.moveToFirst()) {
                int column = row.getColumnIndex(OpenableColumns.DISPLAY_NAME);
                if (column >= 0 && !row.isNull(column)) {
                    String name = row.getString(column);
                    if (name != null && !name.trim().isEmpty())
                        return name.trim();
                }
            }
        } catch (RuntimeException e) {
            // No name said: the address's.
        }
        return lastPart(uri);
    }

    /**
     * A name a file can have in Sioul's folder, as close to the app's as
     * can be: no "/" nor hidden dot, not too long, and an extension from its
     * type when it has none (Sioul tells the recipient a file's type by it).
     */
    static String cleaned(String name, String type)
    {
        String clean = name.replace('/', '_').replace('\\', '_').replace('\0', '_').trim();
        while (clean.startsWith("."))
            clean = clean.substring(1);
        if (clean.isEmpty() || clean.endsWith(".part"))
            clean = clean.isEmpty() ? "attachment" : clean + "_";
        int dot = clean.lastIndexOf('.');
        if (dot <= 0 && type != null) {
            String extension = MimeTypeMap.getSingleton().getExtensionFromMimeType(type.toLowerCase(Locale.ROOT));
            if (extension != null)
                clean = clean + "." + extension;
        }
        while (clean.getBytes(StandardCharsets.UTF_8).length > LONGEST) {
            dot = clean.lastIndexOf('.');
            String extension = dot > 0 && clean.length() - dot <= 10 ? clean.substring(dot) : "";
            String stem = clean.substring(0, clean.length() - extension.length());
            clean = stem.substring(0, stem.offsetByCodePoints(stem.length(), -1)) + extension;
        }
        return clean;
    }

    /** "photo.jpg", then "photo (2).jpg" for a second of the same name. */
    static String unique(Set<String> taken, String name)
    {
        String candidate = name;
        int dot = name.lastIndexOf('.');
        String stem = dot > 0 ? name.substring(0, dot) : name;
        String extension = dot > 0 ? name.substring(dot) : "";
        for (int n = 2; taken.contains(candidate.toLowerCase(Locale.ROOT)); n++)
            candidate = stem + " (" + n + ")" + extension;
        taken.add(candidate.toLowerCase(Locale.ROOT));
        return candidate;
    }

    /** Why it failed, in the system's words, without Java's class names. */
    private static String why(Exception e)
    {
        String message = e.getMessage();
        return message == null || message.trim().isEmpty() ? e.getClass().getSimpleName() : message.trim();
    }

    /** A text written whole: under another name, then renamed. */
    static void write(File file, String text) throws IOException
    {
        File folder = file.getParentFile();
        if (folder != null && !folder.isDirectory() && !folder.mkdirs())
            throw new IOException("cannot make " + folder);
        File temporary = new File(folder, file.getName() + ".new");
        try (FileOutputStream out = new FileOutputStream(temporary)) {
            out.write(text.getBytes(StandardCharsets.UTF_8));
            out.getFD().sync();
        }
        if (!temporary.renameTo(file)) {
            temporary.delete();
            throw new IOException("cannot write " + file);
        }
    }
}
