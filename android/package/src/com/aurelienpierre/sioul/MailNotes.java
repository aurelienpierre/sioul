// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.Context;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.time.Instant;
import java.time.ZoneId;

/**
 * New mail at the times it may come (docs/porch.md, "Notifications"; Rust
 * decides what and when: crates/sioul-app/src/mailnote.rs): one notification
 * for a batch, "Two letters", the first senders with their subjects below,
 * the next batch in its place. A notice to know (Notices): the "Notices"
 * channel, its minutes once seen, never past the end of its day, the Porch
 * keeping the mail. A batch with someone Always through in it (Rust's
 * `through`), while one of Sioul's modes silences the phone: new mail's own
 * channel that passes do-not-disturb, at Android's default importance (its
 * sound, no banner), made then (PauseMode), as before; it rings each time,
 * though a batch shows already (Notices.alerts). Each message it names opens from a
 * button of its own (Rust's `opens`: one per letter, three at most), a tap
 * on the notice the first; one message alone, it; several without their
 * own, the mailbox at its top (ReminderOpener; the unified mailbox's parts
 * 2 and 5). On the lock screen, the count alone. From any process: the
 * window's, or the one that fetches mail while Sioul is away.
 */
final class MailNotes
{
    /** Its tag: one at a time, the next batch in its place. */
    static final String TAG = "mail";

    private MailNotes() {}

    /**
     * Rust's notification (eventalarms.rs, `mail_note`): JSON {title, body,
     * through, opens: [{label, address}] (one per message named, phase 2 of
     * the notifications), open (one message's address, or ""), words}, and,
     * when Rust says them, {about, lasts_ms, ends}; as a notice to know, its
     * minutes `lasts` (as Rust gave them with the events' list) when unsaid,
     * and gone at the end of its day (`dayEnds`) at the latest, as new mail
     * is worth telling until then. Each button opens its message by its
     * address alone, never a file; a tap the first; without any, `open`'s
     * message, else the mailbox. Pure, checked on a JVM
     * (android/jvm-checks/MailOpenCheck.java).
     *
     * @param said Rust's notification
     * @param lasts how long a notice to know lasts once seen (ms), when unsaid
     * @param dayEnds when the day it comes on ends (Unix ms), when Rust says no end
     * @return the notice to post (Notices.post)
     */
    static Notices.Said said(JSONObject said, long lasts, long dayEnds)
    {
        Notices.Said out = new Notices.Said();
        out.tag = TAG;
        out.kind = "mail";
        out.group = Notices.INFORMATIVE;
        out.lastsMs = said.optLong("lasts_ms", 0) > 0 ? said.optLong("lasts_ms") : lasts;
        out.endsMs = said.optLong("ends", 0) > 0 ? said.optLong("ends") : Math.max(0, dayEnds);
        out.title = said.optString("title");
        out.body = said.optString("body");
        // On the lock screen: how many, never who.
        out.publicTitle = out.title;
        out.publicBody = "";
        JSONObject words = said.optJSONObject("words");
        out.words = words;
        JSONArray opens = said.optJSONArray("opens");
        for (int i = 0; opens != null && i < opens.length() && out.buttons.size() < Notices.AT_MOST; i++) {
            JSONObject letter = opens.optJSONObject(i);
            String address = letter == null ? "" : letter.optString("address");
            String label = letter == null ? "" : letter.optString("label");
            if (ReminderOpener.isMailAddress(address) && !label.isEmpty())
                out.buttons.add(new Notices.Button(label, address, ""));
        }
        String open = words == null ? "" : words.optString("open");
        if (open.isEmpty())
            open = EventAlarms.word(null, "open");
        if (out.buttons.isEmpty()) {
            String one = said.optString("open");
            out.buttons.add(new Notices.Button(open, ReminderOpener.isMailAddress(one) ? one : "sioul:mailbox", ""));
        }
        String about = said.optString("about");
        out.about = ReminderOpener.isNoticeAddress(about) ? about : out.buttons.get(0).open;
        return out;
    }

    /** Rust's notification, posted as a notice to know (`said`, Notices.post). Any thread. */
    static void show(Context context, String json)
    {
        final JSONObject given;
        try {
            given = new JSONObject(json);
        } catch (JSONException e) {
            // Its words only: org.json's message quotes the text, which may name a message.
            Log.e(DoseAlarms.TAG, "Mail: a notification that does not read (" + e.getClass().getSimpleName() + ").");
            return;
        }
        long now = System.currentTimeMillis();
        Notices.Said said = said(given, EventAlarms.lasts(context), dayEnds(now, ZoneId.systemDefault()));
        String passing = Notices.word(context, said.words, "mail-through");
        String channel = PauseMode.mailChannelNow(context, given.optBoolean("through", false), passing);
        // Someone Always through during one of Sioul's modes: new mail's own channel that passes, as before.
        if (PauseMode.MAIL_CHANNEL.equals(channel))
            said.channel = channel;
        Notices.post(context, said);
    }

    /**
     * When the local day of `now` ends (Unix ms): new mail's notice goes then
     * at the latest. Pure (MailOpenCheck).
     *
     * @param now the time now (Unix ms)
     * @param zone the phone's zone
     * @return the next midnight there (Unix ms)
     */
    static long dayEnds(long now, ZoneId zone)
    {
        return Instant.ofEpochMilli(now).atZone(zone).toLocalDate().plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli();
    }

    /**
     * The notification taken away, each message it told dismissed, read or
     * done on one of your devices (Rust's `notices::withdraw_if_settled`).
     * Any thread; nothing when none shows.
     */
    static void withdraw(Context context)
    {
        Notices.dismiss(context, TAG);
    }
}
