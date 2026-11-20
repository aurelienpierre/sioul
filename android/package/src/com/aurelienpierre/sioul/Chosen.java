// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.ContentResolver;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;
import android.util.Log;

import org.json.JSONObject;

import java.io.File;
import java.io.IOException;

/**
 * A file chosen in one of Sioul's dialogs on a phone (docs/android.md,
 * "Files chosen in a dialog"; crates/sioul-app/src/chosen.rs). Android's
 * picker answers with a {@code content://} address, a document of some
 * provider (the phone's storage, Downloads, a cloud's app), never a path,
 * and Qt keeps the leave to read it. Rust asks for a copy of it in a folder
 * of Sioul's cache made for it, then hands the copy's path to what asked
 * for the file, and removes it once taken.
 *
 * <p>The reading is the share sheet's (ShareActivity: its name, its type,
 * its copy written beside then put in place); here are only what Rust asks
 * and the bound. A file larger than Rust's bound, or one that would leave
 * the phone with less room than Rust's margin, is not copied, and the answer
 * says which: Rust says it in words in the dialog.
 *
 * <p>A dialog that saves a file is answered the same way, a document made
 * where you chose: Rust writes the bytes in its cache, and the copy goes
 * there through the attachments' own writer (Attachments.write), which
 * removes the document again when the copy fails.
 *
 * <p>The pure part ({@link #room}) is checked on a computer: android/jvm-checks,
 * ChosenCheck.
 */
final class Chosen
{
    private Chosen()
    {
    }

    /**
     * Rust's question ({@link StepService#call}, verbs "chosen-…"):
     * "chosen-copy" {uri, folder, most, margin}, answered {path, name} or
     * {problem: "refused" | "large" | "room" | "failed", name, detail};
     * "chosen-name" {uri}, answered {name};
     * "chosen-save" {path, uri, name}, answered as Attachments.write says.
     */
    static String call(Context context, String verb, JSONObject asked)
    {
        String uri = asked.optString("uri", "");
        switch (verb) {
        case "chosen-copy":
            return copy(context, uri, new File(asked.optString("folder", "")), asked.optLong("most", 0), asked.optLong("margin", 0)).toString();
        case "chosen-name":
            return Attachments.said("name", content(uri) ? ShareActivity.name(context.getContentResolver(), Uri.parse(uri)) : "").toString();
        case "chosen-save":
            if (!content(uri))
                return Attachments.said("saved", false, "problem", "refused").toString();
            return Attachments.write(context, asked.optString("path", ""), Uri.parse(uri), asked.optString("name", "")).toString();
        default:
            return null;
        }
    }

    /** Whether an address is a content:// one: the only kind Rust hands here. */
    static boolean content(String uri)
    {
        return uri != null && uri.regionMatches(true, 0, "content://", 0, "content://".length());
    }

    /**
     * Whether a file of {@code size} bytes (-1: the provider does not say)
     * may be copied with {@code usable} bytes free (-1: unknown): never above
     * {@code most}, and {@code margin} left after it. Null when it may; else
     * "large" or "room".
     */
    static String room(long size, long usable, long most, long margin)
    {
        if (size > most)
            return "large";
        if (usable >= 0 && usable - Math.max(0, size) < margin)
            return "room";
        return null;
    }

    /** The document's size as its provider says it, or -1. */
    private static long size(ContentResolver resolver, Uri uri)
    {
        try (Cursor row = resolver.query(uri, new String[] { OpenableColumns.SIZE }, null, null, null)) {
            if (row != null && row.moveToFirst() && !row.isNull(0))
                return row.getLong(0);
        } catch (RuntimeException e) {
            // Not said: the copy's own bound holds.
        }
        return -1;
    }

    /**
     * The document at {@code uri} copied into {@code folder} (Rust's, made for
     * it alone), under its own name cleaned (ShareActivity.cleaned): whole, or
     * nothing left there. Nothing of the file's name goes to the log.
     */
    static JSONObject copy(Context context, String uri, File folder, long most, long margin)
    {
        if (!content(uri) || folder.getPath().isEmpty() || most <= 0 || most >= Integer.MAX_VALUE)
            return Attachments.said("problem", "refused");
        Uri address = Uri.parse(uri);
        ContentResolver resolver = context.getContentResolver();
        String name = ShareActivity.name(resolver, address);
        if (!folder.isDirectory() && !folder.mkdirs())
            return Attachments.said("problem", "failed", "name", name, "detail", "cannot make the folder");
        String why = room(size(resolver, address), folder.getUsableSpace(), most, margin);
        if (why != null)
            return Attachments.said("problem", why, "name", name);
        String clean = ShareActivity.cleaned(name, ShareActivity.typeOf(context, address));
        File file = new File(folder, clean);
        try {
            // One byte past the bound tells a larger file, whatever its provider said.
            ShareActivity.copy(context, address, (int) most + 1, file);
        } catch (IOException | RuntimeException e) {
            Log.w(Attachments.TAG, "A file chosen: not copied: " + e.getClass().getSimpleName());
            return Attachments.said("problem", "failed", "name", name, "detail", ShareActivity.why(e));
        }
        if (file.length() > most) {
            file.delete();
            return Attachments.said("problem", "large", "name", name);
        }
        return Attachments.said("path", file.getPath(), "name", clean);
    }
}
