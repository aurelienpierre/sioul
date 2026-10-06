// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.AlarmManager;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.graphics.drawable.Icon;
import android.net.Uri;
import android.os.UserManager;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.time.ZoneId;
import java.util.HashSet;
import java.util.Locale;
import java.util.Set;

/**
 * Reminders before events while Sioul is not on the screen (docs/android.md,
 * "Events"): Android freezes an app it does not show, so Rust hands the
 * coming reminders of events ahead (crates/sioul-app/src/eventalarms.rs:
 * their own alarms, Sioul's before them, the working day before), and each
 * is given to Android's alarm clock, as the doses are (DoseAlarms). At its
 * time EventReceiver asks Rust what to say: the event read again, sleep or a
 * pause holding it or not, the time left said as it is then. Rust shows it
 * through `show`, in the "Events" channel, at low importance: no sound, no
 * banner, yours to change in Android's settings; a tap opens the event in
 * Sioul (ReminderOpener). Rust silent, the words given with the list are
 * shown instead: never nothing. Apart from the doses: its own list, alarms,
 * channel and receiver.
 */
final class EventAlarms
{
    static final String RING = "com.aurelienpierre.sioul.action.EVENT_REMINDER";
    static final String LOOK = "com.aurelienpierre.sioul.action.EVENT_LOOK";
    /** The "Events" channel: the reminders, quiet. */
    static final String CHANNEL = "events";

    private static final String SCHEME = "sioul-event";
    private static final String KEPT = "sioul-events";
    private static final String LIST = "list";
    private static final String WORDS = "words";
    private static final String OPENED = "opened";
    /** A reminder is known by its key (its tag) and this number. */
    private static final int NOTE = 2;
    /** The question asked again once a day, nothing having rung meanwhile. */
    private static final int LOOK_CODE = 0x5801;
    /** A reminder stays in the shade until an hour after its event began. */
    private static final long KEPT_AFTER_MS = 60 * 60 * 1000L;

    private EventAlarms() {}

    /** At a reminder's time (eventalarms.rs, `sioul_event_decide`): {"shown"}, {"again_at"}, {}; null when Rust failed. */
    static native String nativeDecide(String key);
    /** The coming reminders, in the phone's `zone` (`sioul_event_coming`); "" when Rust failed. */
    static native String nativeComing(String zone);

    // ---------------------------------------------------------------- the list

    /**
     * Rust's coming reminders: JSON {reminders: [{key, at (Unix ms), title,
     * body}], look (Unix ms), words}. Their alarms replace those given
     * before; the list is kept for Android's restarts. Any thread.
     */
    static synchronized void set(Context context, String json)
    {
        final JSONObject given;
        try {
            given = new JSONObject(json);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "Events: the list does not read; the one kept stays. " + e);
            return;
        }
        JSONArray coming = given.optJSONArray("reminders");
        if (coming == null)
            coming = new JSONArray();
        Set<String> keys = new HashSet<>();
        for (int i = 0; i < coming.length(); i++)
            keys.add(key(coming.optJSONObject(i)));
        JSONArray before = list(context);
        for (int i = 0; i < before.length(); i++) {
            String key = key(before.optJSONObject(i));
            if (!key.isEmpty() && !keys.contains(key))
                cancel(context, key);
        }
        JSONObject words = given.optJSONObject("words");
        SharedPreferences.Editor kept = kept(context).edit().putString(LIST, coming.toString());
        if (words != null)
            kept.putString(WORDS, words.toString());
        kept.commit();
        channel(context);
        schedule(context, coming);
        look(context, given.optLong("look"));
        Log.i(DoseAlarms.TAG, "Events: " + coming.length() + " reminders given, " + (DoseAlarms.exact(context) ? "on time." : "inexact (no \"Alarms & reminders\")."));
    }

    /** The alarms given again from the list kept: Android forgets them when it restarts. One whose time went by rings at once: Rust says whether it still makes sense. */
    static synchronized void restore(Context context, String why)
    {
        JSONArray coming = list(context);
        schedule(context, coming);
        Log.i(DoseAlarms.TAG, "Events: " + coming.length() + " reminders given again (" + why + ").");
    }

    /**
     * Rust asked for the coming reminders, then given them (EventReceiver, on
     * a thread of its own): after a restart, an update, a change of time or
     * of zone, once a day. Not before the phone's first unlock: Sioul's files
     * cannot be read then. No answer: the list kept goes on.
     */
    static void ask(Context context, String why)
    {
        if (!context.getSystemService(UserManager.class).isUserUnlocked())
            return;
        String answer = null;
        try {
            DoseAlarms.load(context);
            answer = nativeComing(ZoneId.systemDefault().getId());
        } catch (Throwable e) {
            Log.e(DoseAlarms.TAG, "Events: Sioul did not answer (" + why + "); the list kept goes on.", e);
        }
        if (answer != null && !answer.isEmpty())
            set(context, answer);
    }

    /** A reminder asked about again at `at`: held now (you sleep), still worth telling then. */
    static synchronized void again(Context context, String key, long at)
    {
        JSONArray coming = list(context);
        for (int i = 0; i < coming.length(); i++) {
            JSONObject reminder = coming.optJSONObject(i);
            if (key.equals(key(reminder)))
                put(reminder, "at", at);
        }
        kept(context).edit().putString(LIST, coming.toString()).commit();
        alarm(context, key, at);
    }

    /** A reminder done with (told, gone): out of the list, not to ring again after a restart. */
    static synchronized void done(Context context, String key)
    {
        JSONArray coming = list(context);
        JSONArray left = new JSONArray();
        for (int i = 0; i < coming.length(); i++)
            if (!key.equals(key(coming.optJSONObject(i))))
                left.put(coming.opt(i));
        kept(context).edit().putString(LIST, left.toString()).commit();
    }

    private static void schedule(Context context, JSONArray coming)
    {
        for (int i = 0; i < coming.length(); i++) {
            JSONObject reminder = coming.optJSONObject(i);
            String key = key(reminder);
            long at = reminder == null ? 0 : reminder.optLong("at");
            if (!key.isEmpty() && at > 0)
                alarm(context, key, at);
        }
    }

    /**
     * A reminder's alarm, at its time even while the phone sleeps (Doze);
     * without "Alarms & reminders", as close as Android allows: up to an hour
     * late. Never Android's alarm clock: no alarm icon for an appointment.
     */
    private static void alarm(Context context, String key, long at)
    {
        AlarmManager alarms = context.getSystemService(AlarmManager.class);
        PendingIntent ring = PendingIntent.getBroadcast(context, key.hashCode(), ringIntent(context, key),
                                                        PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        try {
            if (DoseAlarms.exact(context)) {
                alarms.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, ring);
                return;
            }
        } catch (SecurityException e) {
            // Taken back between the question and the alarm.
        }
        alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, ring);
    }

    private static void cancel(Context context, String key)
    {
        PendingIntent ring = PendingIntent.getBroadcast(context, key.hashCode(), ringIntent(context, key),
                                                        PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_NO_CREATE);
        if (ring != null) {
            context.getSystemService(AlarmManager.class).cancel(ring);
            ring.cancel();
        }
    }

    /** The list asked for again at `at` (inexact: nothing hangs on its minute). */
    private static void look(Context context, long at)
    {
        PendingIntent look = PendingIntent.getBroadcast(context, LOOK_CODE, new Intent(context, EventReceiver.class).setAction(LOOK),
                                                        PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        if (at > System.currentTimeMillis())
            context.getSystemService(AlarmManager.class).setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, at, look);
    }

    private static Intent ringIntent(Context context, String key)
    {
        return new Intent(context, EventReceiver.class).setAction(RING).setData(Uri.fromParts(SCHEME, key, null));
    }

    /** The reminder an intent is about; "" when none. */
    static String key(Intent intent)
    {
        Uri address = intent == null ? null : intent.getData();
        if (address == null || !SCHEME.equals(address.getScheme()))
            return "";
        String key = address.getSchemeSpecificPart();
        return key == null ? "" : key;
    }

    private static String key(JSONObject reminder)
    {
        return reminder == null ? "" : reminder.optString("key");
    }

    /** The words given with a reminder in the list, shown when Rust cannot be asked; null when not kept. */
    static synchronized JSONObject kept(Context context, String key)
    {
        JSONArray coming = list(context);
        for (int i = 0; i < coming.length(); i++) {
            JSONObject reminder = coming.optJSONObject(i);
            if (key.equals(key(reminder)))
                return reminder;
        }
        return null;
    }

    private static void put(JSONObject object, String name, Object value)
    {
        try {
            object.put(name, value);
        } catch (JSONException e) {
            // Only for a name that is null or a number that is not one.
        }
    }

    // ---------------------------------------------------------------- kept

    private static SharedPreferences kept(Context context)
    {
        return context.getSharedPreferences(KEPT, Context.MODE_PRIVATE);
    }

    private static JSONArray list(Context context)
    {
        try {
            return new JSONArray(kept(context).getString(LIST, "[]"));
        } catch (JSONException e) {
            return new JSONArray();
        }
    }

    /** A tapped notification's thing, kept until Sioul takes it (ReminderOpener): "event" and its file, "porch". */
    static synchronized void opened(Context context, String kind, String key)
    {
        JSONObject thing = new JSONObject();
        put(thing, "kind", kind);
        put(thing, "key", key == null ? "" : key);
        kept(context).edit().putString(OPENED, thing.toString()).apply();
    }

    /** What a tapped notification opens, once, as JSON {kind, key}; null when nothing (android/main.cpp, for Rust). */
    static synchronized String takeOpened(Context context)
    {
        SharedPreferences kept = kept(context);
        String thing = kept.getString(OPENED, null);
        if (thing != null)
            kept.edit().remove(OPENED).apply();
        return thing;
    }

    // ---------------------------------------------------------------- the reminders

    /** The "Events" channel: low importance, without sound or banner; yours to change. */
    static void channel(Context context)
    {
        context.getSystemService(NotificationManager.class).createNotificationChannel(
            new NotificationChannel(CHANNEL, word(context, "channel"), NotificationManager.IMPORTANCE_LOW));
    }

    /**
     * Rust's reminder (eventalarms.rs, `show`): JSON {key, title, body, event
     * (its file), until (Unix ms, when the event begins), words}. A tap, or
     * Open, brings Sioul up on the event. Any thread.
     */
    static void show(Context context, String json)
    {
        final JSONObject said;
        try {
            said = new JSONObject(json);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "Events: a reminder that does not read. " + e);
            return;
        }
        JSONObject words = said.optJSONObject("words");
        if (words != null)
            kept(context).edit().putString(WORDS, words.toString()).apply();
        String key = said.optString("key");
        post(context, key, said.optString("title"), said.optString("body"), said.optString("event"), said.optLong("until"));
        // Told (from the window's minute, maybe): its alarm has nothing left to do.
        cancel(context, key);
        done(context, key);
    }

    /** The words given with the list, said when Rust cannot be asked at a reminder's time: never nothing. */
    static void fallback(Context context, String key)
    {
        JSONObject reminder = kept(context, key);
        if (reminder == null)
            return;
        post(context, key, reminder.optString("title"), reminder.optString("body"), "", 0);
    }

    /** A reminder said for nothing (by the fallback) taken away. */
    static void dismiss(Context context, String key)
    {
        context.getSystemService(NotificationManager.class).cancel(key, NOTE);
    }

    private static void post(Context context, String key, String title, String body, String event, long until)
    {
        channel(context);
        // Without its event's file (said from the words kept): Sioul as by its icon.
        Intent sioul = new Intent(context, ReminderOpener.class).setData(Uri.fromParts(ReminderOpener.SCHEME, event.isEmpty() ? "app" : "event", null))
                           .putExtra(ReminderOpener.KEY, event).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        PendingIntent open = PendingIntent.getActivity(context, key.hashCode(), sioul, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification.Builder reminder = new Notification.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(title.isEmpty() ? "Sioul" : title)
            .setContentText(body)
            .setStyle(new Notification.BigTextStyle().bigText(body))
            .setCategory(Notification.CATEGORY_REMINDER)
            .setContentIntent(open)
            .setAutoCancel(true)
            .setOnlyAlertOnce(true)
            .addAction(new Notification.Action.Builder(Icon.createWithResource(context, R.drawable.sioul_notification), word(context, "open"), open).build());
        long left = until - System.currentTimeMillis();
        if (until > 0 && left > -KEPT_AFTER_MS)
            reminder.setTimeoutAfter(left + KEPT_AFTER_MS);
        context.getSystemService(NotificationManager.class).notify(key, NOTE, reminder.build());
    }

    // ---------------------------------------------------------------- the words said here

    private static final String[][] FALLBACK_WORDS = {
        { "channel", "Events", "Événements" },
        { "mail", "New mail", "Nouveau courrier" },
        { "open", "Open", "Ouvrir" },
    };

    /**
     * The words said here, as Rust gave them with the list (in the person's
     * language, crates/sioul-core/locales); before any list, these, by the
     * phone's language.
     */
    static String word(Context context, String which)
    {
        try {
            String said = new JSONObject(kept(context).getString(WORDS, "{}")).optString(which);
            if (!said.isEmpty())
                return said;
        } catch (JSONException e) {
            // The phone's own words below.
        }
        boolean french = "fr".equals(Locale.getDefault().getLanguage());
        for (String[] word : FALLBACK_WORDS)
            if (word[0].equals(which))
                return word[french ? 2 : 1];
        return which;
    }
}
