// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;

/**
 * An event's reminder, new mail's notification, or a button of the end of
 * work's or the night's notice pressed (EventAlarms, MailNotes, DayNotes):
 * what it opens, the event's file, the Porch, a closing sheet or a block's
 * question, kept for Sioul ({@link EventAlarms#takeOpened}, which Rust reads
 * when the window comes up), the day's notice taken away, then Sioul brought
 * up as by its icon. A window of its own, never shown, as DoseOpener: no
 * intent naming a file stays with Sioul's window.
 */
public final class ReminderOpener extends Activity
{
    /** The intent's address: "sioul-open:event", "sioul-open:porch", "sioul-open:review", "sioul-open:need". */
    static final String SCHEME = "sioul-open";
    /** The event's file, on an event's reminder; "work" or "night" and its day for a closing sheet ("work:2026-10-14"); a block's key for its question. */
    static final String KEY = "key";
    /** The day's notice pressed, to take away (DayNotes: its key). */
    static final String NOTE = "note";

    /**
     * Whether a notice of Sioul's may open `kind` with `key`: an event, the
     * Porch, a block's question ("need"), or a closing sheet ("review") of
     * the work day ("work") or of the whole day ("night"), with the day it
     * is for ("work:2026-10-14"). Rust reads the same (eventalarms.rs,
     * `opened_of`). Pure, checked on a JVM (android/jvm-checks/DayNotesCheck.java).
     *
     * @param kind what the notice is about, or null
     * @param key what in it, or null
     * @return whether it is opened
     */
    static boolean opens(String kind, String key)
    {
        if ("event".equals(kind) || "porch".equals(kind) || "need".equals(kind))
            return true;
        return "review".equals(kind) && key != null && key.matches("(work|night)(:\\d{4}-\\d{2}-\\d{2})?");
    }

    @Override
    protected void onCreate(Bundle state)
    {
        super.onCreate(state);
        Uri address = getIntent().getData();
        String kind = address != null && SCHEME.equals(address.getScheme()) ? address.getSchemeSpecificPart() : null;
        String key = getIntent().getStringExtra(KEY);
        if (state == null && opens(kind, key)) {
            EventAlarms.opened(this, kind, key);
            DayNotes.dismiss(this, getIntent().getStringExtra(NOTE));
            Log.i(DoseAlarms.TAG, "Events: a notification tapped, kept for Sioul (" + kind + ").");
        }
        Intent sioul = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (sioul != null)
            startActivity(sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED));
        finish();
    }
}
