// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Notification;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.graphics.drawable.Icon;
import android.net.Uri;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * New mail at the times it may come (docs/porch.md, "Notifications"; Rust
 * decides what and when: crates/sioul-app/src/mailnote.rs): one notification
 * for a batch, "Two letters", the first senders with their subjects below,
 * the next batch in its place. The "New mail" channel, at low importance: no
 * sound, no banner, yours to change in Android's settings. A batch with
 * someone Always through in it (Rust's `through`), while one of Sioul's modes
 * silences the phone: its twin that passes do-not-disturb, at Android's
 * default importance (its sound, no banner), made then (PauseMode). A tap,
 * or Open, brings Sioul up on the Porch (ReminderOpener). On the lock
 * screen, the count alone. From any process: the window's, or the one that
 * fetches mail while Sioul is away.
 */
final class MailNotes
{
    /** The "New mail" channel. */
    static final String CHANNEL = "mail";
    private static final String TAG = "mail";
    private static final int NOTE = 1;
    /** Gone from the shade after half a day: the Porch keeps the mail. */
    private static final long KEPT_MS = 12 * 60 * 60 * 1000L;

    private MailNotes() {}

    /** Rust's notification: JSON {title, body, through, words {mail, mail-through, open}}. Any thread. */
    static void show(Context context, String json)
    {
        final JSONObject said;
        try {
            said = new JSONObject(json);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "Mail: a notification that does not read. " + e);
            return;
        }
        JSONObject words = said.optJSONObject("words");
        String usual = words == null ? "" : words.optString("mail");
        String passing = words == null ? "" : words.optString("mail-through");
        String openWord = words == null ? "" : words.optString("open");
        context.getSystemService(NotificationManager.class).createNotificationChannel(
            Channels.quiet(CHANNEL, usual.isEmpty() ? EventAlarms.word(context, "mail") : usual, NotificationManager.IMPORTANCE_LOW));
        String channel = PauseMode.mailChannelNow(context, said.optBoolean("through", false), passing.isEmpty() ? EventAlarms.word(context, "mail-through") : passing);
        String title = said.optString("title");
        String body = said.optString("body");
        Intent porch = new Intent(context, ReminderOpener.class).setData(Uri.fromParts(ReminderOpener.SCHEME, "porch", null)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        PendingIntent open = PendingIntent.getActivity(context, NOTE, porch, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        // On the lock screen: how many, never who.
        Notification count = new Notification.Builder(context, channel)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(title)
            .build();
        Notification note = new Notification.Builder(context, channel)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(new Notification.BigTextStyle().bigText(body))
            .setCategory(Notification.CATEGORY_EMAIL)
            .setVisibility(Notification.VISIBILITY_PRIVATE)
            .setPublicVersion(count)
            .setContentIntent(open)
            .setAutoCancel(true)
            .setTimeoutAfter(KEPT_MS)
            .addAction(new Notification.Action.Builder(Icon.createWithResource(context, R.drawable.sioul_notification), openWord.isEmpty() ? EventAlarms.word(context, "open") : openWord, open).build())
            .build();
        context.getSystemService(NotificationManager.class).notify(TAG, NOTE, note);
    }
}
