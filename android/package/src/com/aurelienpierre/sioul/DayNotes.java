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

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.List;

/**
 * The end of work's notice and the night's (docs/reviews.md), as on a
 * computer: "Work hours are over" with Close the work day; as the night's
 * wind-down begins, its own notice with Close the whole day while that
 * night's review is to give, and Options…. Rust decides what and when, at
 * their alarms, given with the events' (crates/sioul-app/src/eventalarms.rs,
 * reviews.rs, health.rs), and shows them here. Each button opens Sioul on
 * its sheet or its question (ReminderOpener), the notice then taken away; a
 * tap on the notice is its first button. The "End of the day" channel, at
 * low importance: no sound, no banner, yours to change in Android's
 * settings, made as Sioul's other quiet ones (Channels.quiet). Never said
 * from words kept with the list: Rust alone says them, or nothing
 * (EventAlarms.fallback).
 */
final class DayNotes
{
    /** The "End of the day" channel. */
    static final String CHANNEL = "day";
    /** A notice is known by its key (its tag: "day:work:2026-10-14") and this number. */
    static final int NOTE = 5;
    /** Android shows three buttons at most. */
    static final int AT_MOST = 3;

    private DayNotes() {}

    /** A button: its words, and what it opens (ReminderOpener.opens). */
    static final class Button
    {
        final String label;
        final String kind;
        final String key;

        Button(String label, String kind, String key)
        {
            this.label = label;
            this.kind = kind;
            this.key = key;
        }
    }

    /**
     * The buttons Rust gave ({buttons: [{label, kind, key}]}), as Android
     * shows them: in their order, three at most, each with its words and
     * opening what a notice of Sioul's may open (ReminderOpener.opens).
     * Pure, checked on a JVM (android/jvm-checks/DayNotesCheck.java).
     *
     * @param said Rust's notice, or null
     * @return the buttons to show, perhaps none
     */
    static List<Button> buttons(JSONObject said)
    {
        List<Button> out = new ArrayList<>();
        JSONArray given = said == null ? null : said.optJSONArray("buttons");
        if (given == null)
            return out;
        for (int i = 0; i < given.length() && out.size() < AT_MOST; i++) {
            JSONObject button = given.optJSONObject(i);
            if (button == null)
                continue;
            String label = button.optString("label");
            String kind = button.optString("kind");
            String key = button.optString("key");
            if (!label.isEmpty() && ReminderOpener.opens(kind, key))
                out.add(new Button(label, kind, key));
        }
        return out;
    }

    /**
     * Each press's own request code, so that Android keeps their intents
     * apart: the tap on the notice (`index` -1) and each button.
     *
     * @param note the notice's key
     * @param index the button's place, -1 for the tap on the notice
     * @return the request code
     */
    static int requestCode(String note, int index)
    {
        return (note + "#" + index).hashCode();
    }

    /**
     * Rust's notice (eventalarms.rs, `day_note`): JSON {key, title, body,
     * buttons, until (Unix ms), words {day}}, shown in its place by its key;
     * gone from the shade at `until`. Any thread.
     *
     * @param context the application's
     * @param json Rust's words
     */
    static void show(Context context, String json)
    {
        final JSONObject said;
        try {
            said = new JSONObject(json);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "End of the day: a notice that does not read. " + e);
            return;
        }
        String note = said.optString("key");
        List<Button> buttons = buttons(said);
        if (note.isEmpty() || buttons.isEmpty())
            return;
        JSONObject words = said.optJSONObject("words");
        String name = words == null ? "" : words.optString("day");
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        notifications.createNotificationChannel(Channels.quiet(CHANNEL, name.isEmpty() ? EventAlarms.word(context, "day") : name, NotificationManager.IMPORTANCE_LOW));
        String title = said.optString("title");
        String body = said.optString("body");
        Notification.Builder builder = new Notification.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(title.isEmpty() ? "Sioul" : title)
            .setContentText(body)
            .setStyle(new Notification.BigTextStyle().bigText(body))
            .setCategory(Notification.CATEGORY_REMINDER)
            .setContentIntent(open(context, note, -1, buttons.get(0)))
            .setAutoCancel(true)
            .setOnlyAlertOnce(true);
        for (int i = 0; i < buttons.size(); i++) {
            Button button = buttons.get(i);
            builder.addAction(new Notification.Action.Builder(Icon.createWithResource(context, R.drawable.sioul_notification), button.label, open(context, note, i, button)).build());
        }
        long left = said.optLong("until") - System.currentTimeMillis();
        if (left > 0)
            builder.setTimeoutAfter(left);
        notifications.notify(note, NOTE, builder.build());
        // Said: its alarm has nothing left to do, after a restart neither.
        EventAlarms.done(context, note);
    }

    /** A press: Sioul brought up on what the button offers (ReminderOpener), this notice named to go. */
    private static PendingIntent open(Context context, String note, int index, Button button)
    {
        Intent sioul = new Intent(context, ReminderOpener.class).setData(Uri.fromParts(ReminderOpener.SCHEME, button.kind, null))
                           .putExtra(ReminderOpener.KEY, button.key).putExtra(ReminderOpener.NOTE, note).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        return PendingIntent.getActivity(context, requestCode(note, index), sioul, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /**
     * A notice pressed, taken away from the shade: a button's press leaves
     * it there by itself.
     *
     * @param context any of Sioul's
     * @param note the notice's key; nothing when null or empty
     */
    static void dismiss(Context context, String note)
    {
        if (note != null && !note.isEmpty())
            context.getSystemService(NotificationManager.class).cancel(note, NOTE);
    }
}
