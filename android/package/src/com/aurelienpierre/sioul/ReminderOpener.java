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
 * what it opens, the event by its address, the Porch, a closing sheet or a block's
 * question, kept for Sioul ({@link EventAlarms#takeOpened}, which Rust reads
 * when the window comes up), the day's notice taken away, then Sioul brought
 * up as by its icon. A window of its own, never shown, as DoseOpener: no
 * intent naming a file stays with Sioul's window.
 */
public final class ReminderOpener extends Activity
{
    /** The intent's address: "sioul-open:event", "sioul-open:porch", "sioul-open:review", "sioul-open:need", "sioul-open:mail", "sioul-open:mailbox". */
    static final String SCHEME = "sioul-open";
    /** The event's address and the time it reminds of ({@code sioul:event/<UID>#1793520000}, never its file; an older Sioul's notice: its file), on an event's reminder; "work" or "night" and its day for a closing sheet ("work:2026-10-14"); a block's key for its question; a message's address (never its file) for new mail's notification of one message. */
    static final String KEY = "key";
    /** The day's notice pressed, to take away (DayNotes: its key). */
    static final String NOTE = "note";

    /**
     * Whether a notice of Sioul's may open `kind` with `key`: an event, the
     * Porch, the mailbox (new mail of several), a block's question ("need"), a closing sheet ("review") of the
     * work day ("work") or of the whole day ("night"), with the day it is for
     * ("work:2026-10-14"), or a message ("mail") by its address alone, never
     * a file. Rust reads the same (eventalarms.rs, `opened_of`). Pure,
     * checked on a JVM (android/jvm-checks/DayNotesCheck.java, MailOpenCheck.java).
     *
     * @param kind what the notice is about, or null
     * @param key what in it, or null
     * @return whether it is opened
     */
    static boolean opens(String kind, String key)
    {
        if ("event".equals(kind) || "porch".equals(kind) || "need".equals(kind) || "mailbox".equals(kind))
            return true;
        if ("mail".equals(kind))
            return isMailAddress(key);
        return "review".equals(kind) && key != null && key.matches("(work|night)(:\\d{4}-\\d{2}-\\d{2})?");
    }

    /**
     * An event's address as an event's reminder carries it (eventalarms.rs,
     * {@code shown}): {@code sioul:event/<UID>}, the UID as links.rs encodes
     * it (a slash, a "#" encoded), then "#" and the time it reminds of, Unix
     * seconds, when known; never a file's path.
     *
     * @param key what a reminder carries, or null
     * @return whether it is an event's address
     */
    static boolean isEventAddress(String key)
    {
        if (key == null || !key.matches("sioul:event/[^/\\\\#]+(#\\d+)?"))
            return false;
        String uid = key.substring("sioul:event/".length()).replaceFirst("#\\d+$", "");
        return !uid.equals(".") && !uid.equals("..");
    }

    /**
     * A message's address as Sioul writes it (links.rs): "mid:" and an id in
     * which a slash is encoded, or {@code sioul:mail/<account>/<digest>}, the
     * digest 16 hexadecimal digits; never a file's path. Rust's
     * {@code is_mail_address} (eventalarms.rs).
     *
     * @param key what a notice or a card carries, or null
     * @return whether it is a message's address
     */
    static boolean isMailAddress(String key)
    {
        if (key == null)
            return false;
        if (key.startsWith("mid:"))
            return key.length() > 4 && key.indexOf('/') < 0;
        if (!key.matches("sioul:mail/[^/\\\\]+/[0-9a-fA-F]{16}"))
            return false;
        String account = key.substring("sioul:mail/".length(), key.lastIndexOf('/'));
        return !account.equals(".") && !account.equals("..");
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
