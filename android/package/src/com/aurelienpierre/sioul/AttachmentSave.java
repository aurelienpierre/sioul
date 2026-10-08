// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;

import org.json.JSONObject;

/**
 * "Save…" beside a mail attachment, on a phone (docs/client.md, "Antivirus";
 * Attachments): Android's own question where to save it
 * ({@code ACTION_CREATE_DOCUMENT}: Downloads, a folder, a cloud's app), then
 * the copy there, on a thread of its own, and the copy in Sioul's cache taken
 * out, saved or not. Android answers only an activity that asks for a result,
 * so this one, never seen, asks; it stays behind Sioul's window while the
 * file is copied, since Android's leave to write there goes with it. Rust is
 * told what became of it (nativeSaved, android/main.cpp), and says it in the
 * status line.
 */
public final class AttachmentSave extends Activity
{
    /** The attachment's copy in Sioul's cache (a canonical path). */
    static final String PATH = "com.aurelienpierre.sioul.extra.PATH";
    /** The name offered: the attachment's. */
    static final String NAME = "com.aurelienpierre.sioul.extra.NAME";
    /** Its type, from its name's extension. */
    static final String TYPE = "com.aurelienpierre.sioul.extra.TYPE";

    private static final int WHERE = 0x5135;

    /** Rust told: {path, name, saved, shown} or {path, name, saved, problem} (crates/sioul-app/src/attachments.rs). */
    static native void nativeSaved(String json);

    @Override
    protected void onCreate(Bundle saved)
    {
        super.onCreate(saved);
        // Made again (the screen turned): Android's question is open already.
        if (saved != null)
            return;
        Attachments.SaveAs ask = Attachments.saveAs(getIntent().getStringExtra(NAME), getIntent().getStringExtra(TYPE));
        Intent where = new Intent(ask.action).addCategory(ask.category).setType(ask.type).putExtra(Intent.EXTRA_TITLE, ask.title);
        try {
            startActivityForResult(where, WHERE);
        } catch (ActivityNotFoundException | SecurityException e) {
            Log.w(Attachments.TAG, "Attachments: Android's question where to save could not be asked: " + e.getClass().getSimpleName());
            done(null);
        }
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data)
    {
        super.onActivityResult(request, result, data);
        if (request == WHERE)
            done(result == RESULT_OK && data != null ? data.getData() : null);
    }

    /** The copy made (or not) on a thread of its own, Rust told, then this activity ends. */
    private void done(Uri target)
    {
        final Context app = getApplicationContext();
        final String path = getIntent().getStringExtra(PATH);
        final String name = getIntent().getStringExtra(NAME);
        new Thread(() -> {
            tell(Attachments.write(app, path, target, name));
            runOnUiThread(this::finish);
        }, "sioul-save").start();
    }

    /** Rust told, when Sioul's library is loaded; else (its process made anew meanwhile) the copy is there all the same. */
    private static void tell(JSONObject said)
    {
        try {
            nativeSaved(said.toString());
        } catch (UnsatisfiedLinkError e) {
            Log.i(Attachments.TAG, "Attachments: Sioul's window closed meanwhile; nothing said.");
        }
    }
}
