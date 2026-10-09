// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Notification;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.graphics.drawable.Icon;
import android.os.Build;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * The time running, in Android's notifications while a focus session runs
 * (docs/android.md, "The time running"): its task, a chronometer counting
 * from its start, the time paused left out, and Pause (Go on, while paused)
 * and Stop. Rust says what to show (crates/sioul-app/src/timenote.rs, through
 * android/main.cpp) whenever the session changes, here or on another
 * device; a button reaches Rust through TimeReceiver, Sioul's window running
 * or not; a tap opens Sioul. An ongoing notification the app posts stays
 * while Sioul is away, or stopped by Android: no foreground service, which
 * would keep Sioul running all day for a notification that needs nothing
 * running.
 */
final class TimeNote
{
    static final String PAUSE = "com.aurelienpierre.sioul.action.TIME_PAUSE";
    static final String RESUME = "com.aurelienpierre.sioul.action.TIME_RESUME";
    static final String STOP = "com.aurelienpierre.sioul.action.TIME_STOP";

    private static final String CHANNEL = "time";
    /** The one note, by its number alone: the doses' reminders are known by their key and 1. */
    private static final int NOTE = 2;

    private TimeNote() {}

    /** A button's action ("pause", "resume", "stop") done by Rust: the note it then showed, "" when none is left. */
    static native String nativeAction(String action);

    /**
     * Rust's note, JSON as timenote.rs writes it: put up, or changed in
     * place; "" takes it away. Any thread.
     */
    static synchronized void show(Context context, String json)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        if (json == null || json.isEmpty()) {
            notifications.cancel(NOTE);
            return;
        }
        final JSONObject note;
        try {
            note = new JSONObject(json);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "Time: a note that does not read; the one shown stays. " + e);
            return;
        }
        // Low: in the shade and the status bar, without sound or banner.
        notifications.createNotificationChannel(Channels.quiet(CHANNEL, note.optString("channel", "Focus timer"), NotificationManager.IMPORTANCE_LOW));
        boolean paused = note.optBoolean("paused");
        Notification.Builder built = new Notification.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(note.optString("title"))
            .setContentText(note.optString("text"))
            .setCategory(Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ? Notification.CATEGORY_STOPWATCH : Notification.CATEGORY_STATUS)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setContentIntent(open(context))
            .addAction(button(context, paused ? RESUME : PAUSE, note.optString("pause")))
            .addAction(button(context, STOP, note.optString("stop")));
        // Running, a chronometer from the start, the time paused left out; paused, none.
        if (paused)
            built.setShowWhen(false).setUsesChronometer(false);
        else
            built.setWhen(note.optLong("since")).setShowWhen(true).setUsesChronometer(true);
        notifications.notify(NOTE, built.build());
    }

    /** A button: its broadcast to TimeReceiver, by no other app. */
    private static Notification.Action button(Context context, String action, String label)
    {
        PendingIntent press = PendingIntent.getBroadcast(context, action.hashCode(), new Intent(context, TimeReceiver.class).setAction(action),
                                                         PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        return new Notification.Action.Builder(Icon.createWithResource(context, R.drawable.sioul_notification), label, press).build();
    }

    /** A tap: Sioul, as by its icon. */
    private static PendingIntent open(Context context)
    {
        Intent sioul = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        if (sioul == null)
            return null;
        return PendingIntent.getActivity(context, NOTE, sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED),
                                         PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }
}
