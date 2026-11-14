// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Log;

/**
 * A notice of Sioul's tapped, or one of its buttons pressed (Notices): what
 * it opens, its thing by its address ("notice": an event, a message, a
 * code's message, the mailbox, a need, the end of the work day or of the
 * whole day), kept for Sioul ({@link EventAlarms#takeOpened}, which Rust
 * reads as the window comes up or starts), the notice taken away, then Sioul
 * brought up as by its icon. A notice an older Sioul posted opens as it
 * said: an event, the Porch, the mailbox, a closing sheet, a block's
 * question, a message. A window of its own, never shown, as DoseOpener: no
 * intent naming a file stays with Sioul's window.
 */
public final class ReminderOpener extends Activity
{
    /** The intent's address: "sioul-open:notice"; an older Sioul's "sioul-open:event", "sioul-open:porch", "sioul-open:review", "sioul-open:need", "sioul-open:mail", "sioul-open:mailbox". */
    static final String SCHEME = "sioul-open";
    /** What every notice of this Sioul opens: a thing by its address (`isNoticeAddress`). */
    static final String NOTICE = "notice";
    /** The thing's address ("notice": {@code sioul:event/<UID>#1793520000}, {@code mid:…}, {@code sioul:need/meal:0}, {@code sioul:day/work:2026-10-14}…), never a file; an older Sioul's notice: an event's address or file, a closing sheet's "work:2026-10-14", a block's key, a message's address. */
    static final String KEY = "key";
    /** The notice pressed, to take away (Notices: its tag). */
    static final String NOTE = "note";

    /**
     * Whether a notice of Sioul's may open `kind` with `key`: a thing by its
     * address ("notice", `isNoticeAddress`); as an older Sioul's notice said
     * it, an event, the Porch, the mailbox (new mail of several), a block's
     * question ("need"), a closing sheet ("review") of the work day ("work")
     * or of the whole day ("night"), with the day it is for
     * ("work:2026-10-14"), or a message ("mail") by its address alone, never
     * a file. Rust reads the same (eventalarms.rs, `opened_of`). Pure,
     * checked on a JVM (android/jvm-checks/: DayNotesCheck, MailOpenCheck,
     * EventOpenCheck).
     *
     * @param kind what the notice is about, or null
     * @param key what in it, or null
     * @return whether it is opened
     */
    static boolean opens(String kind, String key)
    {
        if (NOTICE.equals(kind))
            return isNoticeAddress(key);
        if ("event".equals(kind) || "porch".equals(kind) || "need".equals(kind) || "mailbox".equals(kind))
            return true;
        if ("mail".equals(kind))
            return isMailAddress(key);
        return "review".equals(kind) && key != null && key.matches("(work|night)(:\\d{4}-\\d{2}-\\d{2})?");
    }

    /**
     * A thing's address as a notice of Sioul's carries it (Rust's
     * notice::address, the links'), never a file: an event's
     * (`isEventAddress`), a message's (`isMailAddress`; a code's is its
     * message's), the mailbox, the Porch, a need ({@code sioul:need/meal:0}),
     * the end of the work day or of the whole day
     * ({@code sioul:day/work:2026-10-14}), or another thing of Sioul's by its
     * kind and id (a task, a paper, a contract, a budget's line: a tablet's
     * dates), its parts never "." nor "..". Rust's {@code is_notice_address}
     * (eventalarms.rs), on the same addresses (EventOpenCheck).
     *
     * @param key what a notice carries, or null
     * @return whether it is a thing's address
     */
    static boolean isNoticeAddress(String key)
    {
        if (key == null || key.length() > 1024)
            return false;
        if (isEventAddress(key) || isMailAddress(key) || key.equals("sioul:mailbox") || key.equals("sioul:porch"))
            return true;
        if (key.startsWith("sioul:day/"))
            return key.matches("sioul:day/(work|night):\\d{4}-\\d{2}-\\d{2}") && validDate(key.substring(key.indexOf(':', "sioul:day/".length()) + 1));
        java.util.regex.Matcher thing = java.util.regex.Pattern.compile("sioul:(need|task|paper|contract|budget|movement|letter)/(.+)").matcher(key);
        if (!thing.matches())
            return false;
        String rest = thing.group(2);
        if (thing.group(1).equals("need") && rest.contains("/"))
            return false;
        for (String part : rest.split("/", -1))
            if (part.isEmpty() || part.equals(".") || part.equals("..") || !part.matches("[\\x21-\\x5b\\x5d-\\x7e]+"))
                return false;
        return true;
    }

    /** Whether "2026-10-14" is a day of the calendar. */
    private static boolean validDate(String day)
    {
        try {
            java.time.LocalDate.parse(day);
            return true;
        } catch (java.time.format.DateTimeParseException e) {
            return false;
        }
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
        if (state == null) {
            if (opens(kind, key)) {
                EventAlarms.opened(this, kind, key);
                Log.i(DoseAlarms.TAG, "Notices: one tapped, kept for Sioul (" + kind + ").");
            }
            // A button leaves its notice there by itself; a tap took it away already: its group follows.
            Notices.dismiss(this, getIntent().getStringExtra(NOTE));
        }
        Intent sioul = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (sioul != null)
            startActivity(sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED));
        finish();
    }
}
