// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.AlarmManager;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import android.graphics.drawable.Icon;
import android.net.Uri;
import android.os.Build;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.HashSet;
import java.util.Locale;
import java.util.Set;

/**
 * Doses reminded while Sioul is not on the screen (docs/android.md, "Doses
 * while Sioul is away"). Android freezes an app it does not show: each coming
 * dose is given to Android's alarm clock ahead, and at its time DoseReceiver
 * asks Rust's side what to say (crates/sioul-app/src/alarms.rs), which reads
 * first what your other devices marked. Here: the alarms, their list kept for
 * Android's restarts, the reminders, the dose of a tapped one, and Sioul's
 * library, loaded without its window when Android starts Sioul for an alarm
 * alone (android/main.cpp).
 */
final class DoseAlarms
{
    static final String TAG = "sioul";
    static final String ALARM = "com.aurelienpierre.sioul.action.DOSE_ALARM";
    static final String TAKEN = "com.aurelienpierre.sioul.action.DOSE_TAKEN";
    /** The dose as the list names it: "Levothyroxine · 75 µg". */
    static final String NAME = "name";
    /** The reminder's title and text, as Rust said them. */
    static final String TITLE = "title";
    static final String BODY = "body";

    private static final String SCHEME = "sioul-dose";
    private static final String CHANNEL = "doses";
    private static final String KEPT = "sioul-doses";
    private static final String LIST = "alarms";
    private static final String OPENED = "opened";
    /** A reminder is known by its dose's key (its tag) and this number. */
    private static final int REMINDER = 1;
    /** "Taken at 08:03" stays half a day in the shade. */
    private static final long CONFIRMED_MS = 12 * 60 * 60 * 1000L;

    private static boolean loaded;

    private DoseAlarms() {}

    // ---------------------------------------------------------------- Sioul's library

    static native void nativeInit(Context context);
    static native String nativeDecide(String key);
    static native String nativeTaken(String key);

    /**
     * Sioul's library, loaded and started once per process. Android may start
     * Sioul for an alarm alone, without its window: then without Qt's Java
     * side, which Qt's loader starts when the window opens. The library alone
     * is loaded (Qt's libraries come as its own), by its name in the manifest
     * and the processor, as Qt names it: libsioul_arm64-v8a.so.
     */
    static synchronized void load(Context context)
    {
        if (loaded)
            return;
        String name = "sioul";
        try {
            PackageManager packages = context.getPackageManager();
            Intent launcher = packages.getLaunchIntentForPackage(context.getPackageName());
            ComponentName window = launcher == null ? null : launcher.getComponent();
            ActivityInfo info = window == null ? null : packages.getActivityInfo(window, PackageManager.GET_META_DATA);
            if (info != null && info.metaData != null)
                name = info.metaData.getString("android.app.lib_name", name);
        } catch (PackageManager.NameNotFoundException e) {
            Log.w(TAG, "Doses: the library's name is not in the manifest, \"sioul\" taken.");
        }
        UnsatisfiedLinkError first = null;
        for (String abi : Build.SUPPORTED_ABIS) {
            try {
                System.loadLibrary(name + "_" + abi);
                nativeInit(context.getApplicationContext());
                loaded = true;
                return;
            } catch (UnsatisfiedLinkError e) {
                if (first == null)
                    first = e;
            }
        }
        throw first != null ? first : new UnsatisfiedLinkError("Doses: no library " + name);
    }

    // ---------------------------------------------------------------- the alarms

    /**
     * Rust's coming doses (android/main.cpp): JSON [{key, at (Unix
     * milliseconds), title}]. Their alarms replace those given before, and
     * the list is kept for Android's restarts. Any thread.
     */
    static synchronized void set(Context context, String json)
    {
        final JSONArray doses;
        try {
            doses = new JSONArray(json);
        } catch (JSONException e) {
            Log.e(TAG, "Doses: the list of alarms does not read; those given before stay. " + e);
            return;
        }
        Set<String> coming = new HashSet<>();
        for (int i = 0; i < doses.length(); i++)
            coming.add(key(doses.optJSONObject(i)));
        JSONArray before = list(context);
        for (int i = 0; i < before.length(); i++) {
            String key = key(before.optJSONObject(i));
            if (!key.isEmpty() && !coming.contains(key))
                cancel(context, key);
        }
        schedule(context, doses);
        keep(context, doses);
        channel(context);
        Log.i(TAG, "Doses: " + doses.length() + " alarms given, " + (exact(context) ? "on time." : "inexact (no \"Alarms & reminders\")."));
    }

    /**
     * The alarms given again from the list kept: Android forgets them when it
     * restarts (DoseBootReceiver). A dose whose time went by rings at once:
     * Rust says whether it is still to remind.
     */
    static synchronized void restore(Context context, String why)
    {
        JSONArray doses = list(context);
        schedule(context, doses);
        Log.i(TAG, "Doses: " + doses.length() + " alarms given again (" + why + ").");
    }

    /** A dose asked about again at `at`, Rust waiting for news from your other devices. */
    static synchronized void again(Context context, String key, long at, String name)
    {
        alarm(context, key, at, name);
        JSONArray doses = list(context);
        boolean found = false;
        for (int i = 0; i < doses.length(); i++) {
            JSONObject dose = doses.optJSONObject(i);
            if (key.equals(key(dose))) {
                put(dose, "at", at);
                found = true;
            }
        }
        if (!found) {
            JSONObject dose = new JSONObject();
            put(dose, "key", key);
            put(dose, "at", at);
            put(dose, "title", name);
            doses.put(dose);
        }
        keep(context, doses);
    }

    /** A dose reminded: out of the list, not to ring again after a restart. */
    static synchronized void reminded(Context context, String key)
    {
        JSONArray doses = list(context);
        JSONArray left = new JSONArray();
        for (int i = 0; i < doses.length(); i++)
            if (!key.equals(key(doses.optJSONObject(i))))
                left.put(doses.opt(i));
        keep(context, left);
    }

    /**
     * Whether the alarms ring on time: "Alarms &amp; reminders" allowed, which
     * Android 12 lets you take back and gives from Android 13 (USE_EXACT_ALARM).
     */
    static boolean exact(Context context)
    {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.S || context.getSystemService(AlarmManager.class).canScheduleExactAlarms();
    }

    private static void schedule(Context context, JSONArray doses)
    {
        for (int i = 0; i < doses.length(); i++) {
            JSONObject dose = doses.optJSONObject(i);
            String key = key(dose);
            long at = dose == null ? 0 : dose.optLong("at");
            if (!key.isEmpty() && at > 0)
                alarm(context, key, at, dose.optString("title"));
        }
    }

    /**
     * A dose's alarm, at its time even while the phone sleeps (Doze). Not
     * Android's alarm clock (setAlarmClock): it would show an alarm in the
     * status bar for good, a dose always coming within two days, and put the
     * doses before your own alarm on the lock screen. Without "Alarms &amp;
     * reminders", as close as Android allows: up to an hour late.
     */
    private static void alarm(Context context, String key, long at, String name)
    {
        AlarmManager alarms = context.getSystemService(AlarmManager.class);
        PendingIntent ring = PendingIntent.getBroadcast(context, key.hashCode(), alarmIntent(context, key).putExtra(NAME, name),
                                                        PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        try {
            if (exact(context)) {
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
        PendingIntent ring = PendingIntent.getBroadcast(context, key.hashCode(), alarmIntent(context, key),
                                                        PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_NO_CREATE);
        if (ring != null) {
            context.getSystemService(AlarmManager.class).cancel(ring);
            ring.cancel();
        }
    }

    private static Intent alarmIntent(Context context, String key)
    {
        return new Intent(context, DoseReceiver.class).setAction(ALARM).setData(address(key));
    }

    /** A dose's intents differ by their address, its key: one alarm per dose, whatever the keys' hashes. */
    static Uri address(String key)
    {
        return Uri.fromParts(SCHEME, key, null);
    }

    /** The dose an intent is about; "" when none. */
    static String key(Intent intent)
    {
        Uri address = intent == null ? null : intent.getData();
        if (address == null || !SCHEME.equals(address.getScheme()))
            return "";
        String key = address.getSchemeSpecificPart();
        return key == null ? "" : key;
    }

    private static String key(JSONObject dose)
    {
        return dose == null ? "" : dose.optString("key");
    }

    private static void put(JSONObject dose, String name, Object value)
    {
        try {
            dose.put(name, value);
        } catch (JSONException e) {
            // Only for a name that is null or a number that is not one.
        }
    }

    // ---------------------------------------------------------------- the list kept

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

    private static void keep(Context context, JSONArray doses)
    {
        kept(context).edit().putString(LIST, doses.toString()).commit();
    }

    /** A tapped reminder's dose, kept until Sioul takes it (DoseOpener). */
    static synchronized void opened(Context context, String key)
    {
        kept(context).edit().putString(OPENED, key).apply();
    }

    /** The dose of a tapped reminder, once; null when there is none (android/main.cpp, for Rust). */
    static synchronized String takeOpened(Context context)
    {
        SharedPreferences kept = kept(context);
        String key = kept.getString(OPENED, null);
        if (key != null) {
            kept.edit().remove(OPENED).apply();
            Log.i(TAG, "Doses: a tapped reminder's dose handed to Sioul.");
        }
        return key;
    }

    // ---------------------------------------------------------------- the reminders

    /**
     * The "Doses" channel: Android's default importance (its sound, no banner
     * over what you do), yours to change in Android's settings.
     */
    static void channel(Context context)
    {
        context.getSystemService(NotificationManager.class).createNotificationChannel(
            Channels.quiet(CHANNEL, "Doses", NotificationManager.IMPORTANCE_DEFAULT));
    }

    /** Whether reminders can show: Sioul's notifications on (from Android 13, your yes), the "Doses" channel too. */
    static boolean notificationsAllowed(Context context)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        if (!notifications.areNotificationsEnabled())
            return false;
        NotificationChannel doses = notifications.getNotificationChannel(CHANNEL);
        return doses == null || doses.getImportance() != NotificationManager.IMPORTANCE_NONE;
    }

    /** Rust's reminder, as Rust said it, with "Taken"; a tap opens Sioul on the dose's question. */
    static void remind(Context context, String key, String title, String body, String name, String taken)
    {
        Intent press = new Intent(context, DoseReceiver.class).setAction(TAKEN).setData(address(key))
                           .putExtra(NAME, name).putExtra(TITLE, title).putExtra(BODY, body);
        PendingIntent button = PendingIntent.getBroadcast(context, key.hashCode(), press,
                                                          PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Notification.Action action = new Notification.Action.Builder(
            Icon.createWithResource(context, R.drawable.sioul_notification),
            taken == null || taken.isEmpty() ? words(TAKEN_WORD) : taken, button).build();
        show(context, key, reminder(context, key, title, body).addAction(action));
    }

    /** "Taken" recorded: the reminder becomes a quiet line, "Taken at 08:03", for half a day. */
    static void confirm(Context context, String key, String title, String line)
    {
        show(context, key, reminder(context, key, title, line).setTimeoutAfter(CONFIRMED_MS));
    }

    /**
     * "Taken" more than half an hour late: when, asked in Sioul, which a tap
     * opens. Android opens no window from a button's broadcast (Android 12).
     */
    static void askWhen(Context context, String key, String title, String line)
    {
        show(context, key, reminder(context, key, title, line == null || line.isEmpty() ? words(LATE_WORDS) : line));
    }

    /**
     * No answer from Rust, or none in time: never silence, never a claim. A
     * dose to check in Sioul, as Rust says it then ("dose-alarm-fallback").
     */
    static void fallback(Context context, String key)
    {
        show(context, key, reminder(context, key, "Sioul", words(FALLBACK_WORDS)));
    }

    /** A dose's reminder taken away: marked meanwhile, or said for nothing. */
    static void dismiss(Context context, String key)
    {
        context.getSystemService(NotificationManager.class).cancel(key, REMINDER);
    }

    private static Notification.Builder reminder(Context context, String key, String title, String text)
    {
        // Sioul's window opened by DoseOpener, never with the dose in its own intent.
        PendingIntent open = PendingIntent.getActivity(context, key.hashCode(),
                                                       new Intent(context, DoseOpener.class).setData(address(key)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
                                                       PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        // During a pause that lets doses through, their own channel, which
        // passes the pause's do-not-disturb (PauseMode); "Doses" otherwise.
        return new Notification.Builder(context, PauseMode.dosesChannelNow(context))
            .setSmallIcon(R.drawable.sioul_notification)
            .setColor(context.getColor(R.color.sioul_green))
            .setContentTitle(title)
            .setContentText(text)
            .setStyle(new Notification.BigTextStyle().bigText(text))
            .setCategory(Notification.CATEGORY_REMINDER)
            .setContentIntent(open)
            .setAutoCancel(true)
            // Replaced (by "Taken", by Rust's answer after the fallback): no second sound.
            .setOnlyAlertOnce(true);
    }

    private static void show(Context context, String key, Notification.Builder reminder)
    {
        channel(context);
        context.getSystemService(NotificationManager.class).notify(key, REMINDER, reminder.build());
    }

    // ---------------------------------------------------------------- the words said here

    private static final int TAKEN_WORD = 0;
    private static final int LATE_WORDS = 1;
    private static final int FALLBACK_WORDS = 2;
    private static final String[][] WORDS = {
        { "Taken", "Pris" },
        { "Taken late: tap to say when.", "Pris en retard : touchez pour dire à quelle heure." },
        { "A dose is due: open Sioul to check it.", "Une prise est prévue : ouvrez Sioul pour vérifier." },
    };

    /**
     * The few words said here, in Sioul's two languages (crates/sioul-core/
     * locales), by the phone's language as Sioul takes it (android/main.cpp);
     * all the others are Rust's.
     */
    private static String words(int which)
    {
        return WORDS[which]["fr".equals(Locale.getDefault().getLanguage()) ? 1 : 0];
    }
}
