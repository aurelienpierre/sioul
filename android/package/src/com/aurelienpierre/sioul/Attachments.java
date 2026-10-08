// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;
import android.util.Log;
import android.webkit.MimeTypeMap;

import org.json.JSONException;
import org.json.JSONObject;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

/**
 * A mail attachment opened in the app you choose, or saved where you choose
 * (docs/client.md, "Antivirus"; crates/sioul-app/src/attachments.rs). Rust
 * writes the attachment in Sioul's cache,
 * {@code cache/sioul/attachments/<message>/<name>}, after Sioul's own
 * checks: mail set aside keeps its attachments closed, and programs and
 * installers are never opened. A phone has no antivirus Sioul can call, and
 * nothing here claims one.
 *
 * <p>Since Android 7 a file goes to another app by a {@code content://}
 * address, never by its path. Here it is an address of Qt's FileProvider
 * (authority {@code <package>.qtprovider}), whose root "attachments"
 * (res/xml/qtprovider_paths.xml) is that folder of the cache alone. Android
 * lends the address, to read only, to the one app that opens it: that app
 * reads this file and nothing else of Sioul's. When no app is set for its
 * type, Android asks which one ("Open with").
 *
 * <p>Saving asks Android where ({@code ACTION_CREATE_DOCUMENT}, in
 * AttachmentSave), then copies the file there; the copy in the cache goes,
 * saved or not.
 *
 * <p>The pure parts (the address, the type, what an app is handed, what
 * Android is asked) are checked on a computer: android/jvm-checks,
 * AttachmentsCheck.
 */
public final class Attachments
{
    static final String TAG = DoseAlarms.TAG;
    /** Qt's FileProvider, the one Sioul declares: its authority, after the package's name. */
    static final String AUTHORITY = ".qtprovider";
    /** The provider's root for attachments, by its name in res/xml/qtprovider_paths.xml. */
    static final String ROOT = "attachments";
    /** That root's folder, in the app's cache, where Rust writes attachments (XDG_CACHE_HOME/sioul/attachments). */
    static final String FOLDER = "sioul/attachments";
    /** Android's installer's type: never handed to open, whatever the name says. */
    static final String INSTALLER = "application/vnd.android.package-archive";
    /** A type nothing more is known of. */
    static final String BYTES = "application/octet-stream";
    /**
     * A file handed to open: to read, and nothing more, in a task of its own
     * (Rust asks from Sioul's application, not from its window).
     */
    static final int VIEW_FLAGS = Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_ACTIVITY_NEW_TASK;

    private static final char[] HEX = "0123456789ABCDEF".toCharArray();

    private Attachments()
    {
    }

    /** What another app is handed to open a file: an action, an address, a type, flags. */
    static final class View
    {
        final String action;
        final String uri;
        final String type;
        final int flags;

        View(String action, String uri, String type, int flags)
        {
            this.action = action;
            this.uri = uri;
            this.type = type;
            this.flags = flags;
        }
    }

    /** Android's question "where?" for a file to save: an action, a category, a type, the name offered. */
    static final class SaveAs
    {
        final String action;
        final String category;
        final String type;
        final String title;

        SaveAs(String action, String category, String type, String title)
        {
            this.action = action;
            this.category = category;
            this.type = type;
            this.title = title;
        }
    }

    /**
     * A part of an address as Android's Uri.encode writes it, as FileProvider
     * does: letters, digits and {@code _-!.~'()*} kept, every other byte of
     * its UTF-8 as {@code %XX}.
     */
    static String encode(String text)
    {
        StringBuilder out = new StringBuilder();
        for (byte b : text.getBytes(StandardCharsets.UTF_8)) {
            int c = b & 0xff;
            if ((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || "_-!.~'()*".indexOf(c) >= 0)
                out.append((char) c);
            else
                out.append('%').append(HEX[c >> 4]).append(HEX[c & 15]);
        }
        return out.toString();
    }

    /** Uri.decode's way back: each {@code %XX} a byte, the bytes read as UTF-8. */
    static String decode(String text)
    {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream();
        int i = 0;
        while (i < text.length()) {
            int high = i + 2 < text.length() ? Character.digit(text.charAt(i + 1), 16) : -1;
            int low = i + 2 < text.length() ? Character.digit(text.charAt(i + 2), 16) : -1;
            if (text.charAt(i) == '%' && high >= 0 && low >= 0) {
                bytes.write(high * 16 + low);
                i += 3;
            } else {
                int point = text.codePointAt(i);
                byte[] own = new String(Character.toChars(point)).getBytes(StandardCharsets.UTF_8);
                bytes.write(own, 0, own.length);
                i += Character.charCount(point);
            }
        }
        return new String(bytes.toByteArray(), StandardCharsets.UTF_8);
    }

    /**
     * The address FileProvider gives {@code file} in the root "attachments",
     * whose folder is {@code folder} (both canonical paths, as FileProvider
     * compares them): {@code content://<package>.qtprovider/attachments/<the
     * rest, encoded part by part>}. Null for a file outside the folder, the
     * folder itself, or a path with an empty, "." or ".." part.
     */
    static String contentUri(String packageName, String folder, String file)
    {
        if (packageName == null || packageName.isEmpty() || folder == null || folder.isEmpty() || file == null)
            return null;
        String root = folder.endsWith("/") ? folder : folder + "/";
        if (!file.startsWith(root) || file.length() == root.length())
            return null;
        StringBuilder path = new StringBuilder();
        for (String part : file.substring(root.length()).split("/", -1)) {
            if (part.isEmpty() || part.equals(".") || part.equals(".."))
                return null;
            if (path.length() > 0)
                path.append('/');
            path.append(encode(part));
        }
        return "content://" + packageName + AUTHORITY + "/" + encode(ROOT) + "/" + path;
    }

    /**
     * FileProvider's way back (its getFileForUri), for the checks: the file
     * an address names in the root "attachments", whose folder is
     * {@code folder}; null when it names no file there.
     */
    static String fileOf(String packageName, String folder, String uri)
    {
        String start = "content://" + packageName + AUTHORITY + "/";
        if (uri == null || !uri.startsWith(start))
            return null;
        String path = uri.substring(start.length());
        int split = path.indexOf('/');
        if (split <= 0 || !decode(path.substring(0, split)).equals(ROOT))
            return null;
        String inside = decode(path.substring(split + 1));
        for (String part : inside.split("/", -1)) {
            if (part.isEmpty() || part.equals(".") || part.equals(".."))
                return null;
        }
        return (folder.endsWith("/") ? folder : folder + "/") + inside;
    }

    /**
     * The type an app is asked to open: Rust's, from the name's extension
     * (sioul_core::compose::mime_of), else Android's own for that extension,
     * else plain bytes; in lower case. Null for Android's installer, whatever
     * gave it: an installer is never handed to open.
     */
    static String type(String given, String android)
    {
        String type = given == null || given.isEmpty() || given.equalsIgnoreCase(BYTES) ? android : given;
        if (type == null || type.isEmpty())
            type = BYTES;
        type = type.toLowerCase(Locale.ROOT);
        return type.equals(INSTALLER) ? null : type;
    }

    /**
     * What is handed to open {@code file} (a canonical path) of type
     * {@code type} (from type()): ACTION_VIEW, its content address, the type,
     * read only; null when the file is not in the attachments' folder, or for
     * an installer.
     */
    static View view(String packageName, String folder, String file, String type)
    {
        String uri = contentUri(packageName, folder, file);
        if (uri == null || type == null || type.equals(INSTALLER))
            return null;
        return new View(Intent.ACTION_VIEW, uri, type, VIEW_FLAGS);
    }

    /**
     * What Android is asked to save a file: ACTION_CREATE_DOCUMENT, among
     * the files that open (CATEGORY_OPENABLE), of its type (plain bytes when
     * none), under its name (the attachment's when none).
     */
    static SaveAs saveAs(String name, String type)
    {
        String title = name == null || name.isEmpty() ? "attachment" : name;
        String kind = type == null || type.isEmpty() ? BYTES : type.toLowerCase(Locale.ROOT);
        return new SaveAs(Intent.ACTION_CREATE_DOCUMENT, Intent.CATEGORY_OPENABLE, kind, title);
    }

    // ---------------------------------------------------------------- on the phone

    /**
     * Rust's question (StepService.call, verbs "attachment-…"):
     * "attachment-open" {path, type}, answered {opened} or {problem};
     * "attachment-save" {path, name, type}, answered {asked} or {problem},
     * then AttachmentSave tells Rust what became of it.
     */
    static String call(Context context, String verb, JSONObject asked)
    {
        switch (verb) {
        case "attachment-open":
            return open(context, asked.optString("path", ""), asked.optString("type", "")).toString();
        case "attachment-save":
            return save(context, asked.optString("path", ""), asked.optString("name", ""), asked.optString("type", "")).toString();
        default:
            return null;
        }
    }

    /** A JSON answer from pairs of names and values; what cannot be written is left out. */
    static JSONObject said(Object... pairs)
    {
        JSONObject said = new JSONObject();
        for (int i = 0; i + 1 < pairs.length; i += 2) {
            try {
                said.put((String) pairs[i], pairs[i + 1]);
            } catch (JSONException e) {
                // A name or a value JSON cannot hold: left out.
            }
        }
        return said;
    }

    /** Android's own type for a file's extension; null when it has none. */
    private static String androidType(String file)
    {
        int dot = file.lastIndexOf('.');
        if (dot < 0 || dot < file.lastIndexOf('/'))
            return null;
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.substring(dot + 1).toLowerCase(Locale.ROOT));
    }

    /** The attachments' folder in this app's cache, as FileProvider reads it (canonical). */
    private static String folder(Context context) throws IOException
    {
        return new File(context.getCacheDir(), FOLDER).getCanonicalPath();
    }

    /**
     * The attachment at {@code path} handed to the app you choose:
     * {opened: true}, else {problem: "refused" | "gone" | "no-app" | "failed"}.
     * Nothing of the file's name goes to the log.
     */
    static JSONObject open(Context context, String path, String given)
    {
        try {
            String file = new File(path).getCanonicalPath();
            View view = view(context.getPackageName(), folder(context), file, type(given, androidType(file)));
            if (view == null)
                return said("problem", "refused");
            if (!new File(file).isFile())
                return said("problem", "gone");
            context.startActivity(new Intent(view.action).setDataAndType(Uri.parse(view.uri), view.type).addFlags(view.flags));
            Log.i(TAG, "Attachments: one handed to open, as " + view.type + ".");
            return said("opened", true);
        } catch (ActivityNotFoundException e) {
            Log.i(TAG, "Attachments: no app opens this type.");
            return said("problem", "no-app");
        } catch (IOException | RuntimeException e) {
            Log.w(TAG, "Attachments: not handed to open: " + e.getClass().getSimpleName());
            return said("problem", "failed", "detail", e.getClass().getSimpleName());
        }
    }

    /**
     * Android asked where to save the attachment at {@code path}
     * (AttachmentSave, never shown itself): {asked: true}, else
     * {problem: "refused" | "gone" | "failed"}.
     */
    static JSONObject save(Context context, String path, String name, String type)
    {
        try {
            String file = new File(path).getCanonicalPath();
            if (contentUri(context.getPackageName(), folder(context), file) == null)
                return said("problem", "refused");
            if (!new File(file).isFile())
                return said("problem", "gone");
            context.startActivity(new Intent(context, AttachmentSave.class).putExtra(AttachmentSave.PATH, file)
                .putExtra(AttachmentSave.NAME, name).putExtra(AttachmentSave.TYPE, type).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
            return said("asked", true);
        } catch (IOException | RuntimeException e) {
            Log.w(TAG, "Attachments: Android's question where to save not asked: " + e.getClass().getSimpleName());
            return said("problem", "failed", "detail", e.getClass().getSimpleName());
        }
    }

    /**
     * The attachment at {@code path} copied where Android's question
     * answered ({@code target}, null when you closed it), then taken out of
     * the cache, saved or not: {path, name, saved, shown (the name it has
     * there)} or {path, name, saved: false, problem}. A copy that fails
     * midway is removed there. On a thread of its own.
     */
    static JSONObject write(Context context, String path, Uri target, String name)
    {
        File source = path == null || path.isEmpty() ? null : new File(path);
        try {
            if (target == null)
                return said("path", path, "name", name, "saved", false);
            if (source == null || !source.isFile())
                return said("path", path, "name", name, "saved", false, "problem", "gone");
            try (InputStream in = new FileInputStream(source); OutputStream out = context.getContentResolver().openOutputStream(target, "w")) {
                if (out == null)
                    throw new IOException("no stream");
                byte[] buffer = new byte[64 * 1024];
                for (int n = in.read(buffer); n > 0; n = in.read(buffer))
                    out.write(buffer, 0, n);
            }
            return said("path", path, "name", name, "saved", true, "shown", shownName(context, target, name));
        } catch (IOException | RuntimeException e) {
            Log.w(TAG, "Attachments: not saved: " + e.getClass().getSimpleName());
            try {
                DocumentsContract.deleteDocument(context.getContentResolver(), target);
            } catch (Exception gone) {
                // Nothing written there, or the place keeps it: left.
            }
            return said("path", path, "name", name, "saved", false, "problem", e.getClass().getSimpleName());
        } finally {
            if (source != null && !source.delete() && source.exists())
                Log.w(TAG, "Attachments: the copy in the cache stays until tomorrow.");
        }
    }

    /** The name the saved file has where it went (Android may add " (1)"); {@code name} when it does not say. */
    private static String shownName(Context context, Uri target, String name)
    {
        try (Cursor row = context.getContentResolver().query(target, new String[] {OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (row != null && row.moveToFirst() && !row.isNull(0))
                return row.getString(0);
        } catch (RuntimeException e) {
            // The place does not say: the name asked.
        }
        return name;
    }
}
