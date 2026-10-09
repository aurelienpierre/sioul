// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.AlarmManager;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.net.Uri;
import android.os.Build;
import android.os.UserManager;
import android.provider.Settings;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.time.Instant;
import java.time.LocalDate;
import java.time.LocalDateTime;
import java.time.LocalTime;
import java.time.ZoneId;
import java.time.ZonedDateTime;
import java.util.Locale;

/**
 * The alarm at waking (docs/android.md, "Waking"): it rings when the night
 * ends, on the mornings ticked with the night in Sioul's Health settings.
 * Rust hands the coming wakings, eight days ahead, whenever they change
 * (crates/sioul-app/src/wake.rs, through android/main.cpp). Here they are
 * kept, in the storage a phone reads before its first unlock (a phone that
 * restarted in the night still rings), and the next is given to Android's
 * alarm clock: setAlarmClock, exact, through Doze, the alarm shown in the
 * status bar. At each ring, a little before each and as each night starts,
 * after a restart, a change of time or of zone, Rust is asked again
 * (WakeReceiver); when it cannot answer, the list kept goes on, then the
 * usual week, repeated: never nothing. Apart from the doses: its own list,
 * codes, channel and receiver.
 */
final class WakeAlarms
{
    static final String RING = "com.aurelienpierre.sioul.action.WAKE_RING";
    static final String LOOK = "com.aurelienpierre.sioul.action.WAKE_LOOK";
    static final String STOP = "com.aurelienpierre.sioul.action.WAKE_STOP";
    static final String LATER = "com.aurelienpierre.sioul.action.WAKE_LATER";
    /** "Try the alarm": rung as a waking, by its own action (told apart in `dumpsys alarm`). */
    static final String TRY = "com.aurelienpierre.sioul.action.WAKE_TRY";
    /** A ring's time (Unix milliseconds), on its intent. */
    static final String AT = "at";
    /** On a ring and its buttons: a try's; and a try's last ring, after its "10 min later" (no other). */
    static final String TRIAL = "trial";
    static final String LAST = "last";
    /** Seconds before a try rings (wake.rs, TRY_SECONDS). */
    static final int TRY_SECONDS = 10;
    /** A try's answers (wake.rs): it rings; exact alarms, notifications or the full screen refused. */
    static final int TRY_RINGS = 0;
    static final int TRY_NO_EXACT = 1;
    static final int TRY_NO_NOTIFICATIONS = 2;
    static final int TRY_NO_SCREEN = 3;
    /** Minutes "10 min later" puts the alarm off by (wake.rs, LATER_MINUTES). */
    static final int LATER_MINUTES = 10;
    /** The "Waking" channel: the ring's notification, and the one after "10 min later". */
    static final String CHANNEL = "waking";

    // Its alarms and buttons, by codes of their own (the doses' are their keys' hashes, to another receiver).
    private static final int RING_CODE = 0x5711;
    private static final int LOOK_CODE = 0x5712;
    private static final int AGAIN_CODE = 0x5713;
    private static final int SHOW_CODE = 0x5714;
    static final int STOP_CODE = 0x5715;
    static final int LATER_CODE = 0x5716;
    static final int SCREEN_CODE = 0x5717;
    // A try's own: its alarm, and its buttons (their extras say a try's: never a waking's buttons changed).
    private static final int TRY_CODE = 0x5718;
    static final int TRY_STOP_CODE = 0x5719;
    static final int TRY_LATER_CODE = 0x571A;

    private static final String KEPT = "sioul-waking";
    private static final String LIST = "list";
    /** The ring last given to Android, and the last that rang: a ring missed while the phone was off. */
    private static final String ARMED = "armed";
    private static final String RANG = "rang";
    /** After "10 min later": when it rings again. */
    private static final String AGAIN = "again";
    /** A try's own keys: when it rings, and its words (before any list). Never the waking's. */
    private static final String TRY_AT = "try_at";
    private static final String TRY_WORDS = "try_words";
    /** A ring missed while the phone was off rings as it starts again, when this late at most. */
    private static final long MISSED_MS = 30 * 60 * 1000L;
    /** A ring due within a minute is the one ringing now: never given again. */
    private static final long DUE_MS = 60 * 1000L;

    private WakeAlarms() {}

    /** The coming wakings (wake.rs, `sioul_wake_next`), in the phone's `zone`; JSON, or {"unknown": true}. */
    static native String nativeNext(String zone, boolean fetch);

    // ---------------------------------------------------------------- the list

    /**
     * Rust's wakings (wake.rs): {rings, looks (Unix ms), week (Monday first,
     * "07:00" or ""), zone, words}. Kept, then the next ring and look given to
     * Android. Rust not knowing ({"unknown": true}: the night's file does not
     * read) changes nothing kept. Any thread.
     */
    static synchronized void set(Context context, String json)
    {
        final JSONObject list;
        try {
            list = new JSONObject(json);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "Waking: a list that does not read; the one kept stays. " + e);
            return;
        }
        if (list.optBoolean("unknown")) {
            Log.w(DoseAlarms.TAG, "Waking: Sioul cannot read the night now; the alarms kept stay.");
            arm(context, "not known");
            return;
        }
        kept(context).edit().putString(LIST, list.toString()).commit();
        channel(context);
        arm(context, "given");
    }

    /**
     * The next ring and look given to Android's alarm clock, from the list
     * kept: Android forgets them at a restart, a change of zone moves them.
     * The list run out (Sioul not asked for eight days), the usual week. A
     * ring after "10 min later" given again too. Never inexact: without
     * "Alarms &amp; reminders", nothing, and the Health settings say so. Any thread.
     */
    static synchronized void arm(Context context, String why)
    {
        JSONObject list = list(context);
        long now = System.currentTimeMillis();
        ZoneId here = ZoneId.systemDefault();
        ZoneId made = zone(list.optString("zone"));
        long ring = first(list.optJSONArray("rings"), made, here, now + DUE_MS);
        boolean usual = ring == 0;
        if (usual)
            ring = usual(list.optJSONArray("week"), here, now + DUE_MS);
        long look = first(list.optJSONArray("looks"), made, here, now);
        long again = kept(context).getLong(AGAIN, 0);
        if (!exact(context)) {
            Log.w(DoseAlarms.TAG, "Waking: no \"Alarms & reminders\": no alarm given (" + why + ").");
            return;
        }
        AlarmManager alarms = context.getSystemService(AlarmManager.class);
        try {
            if (ring > 0)
                alarms.setAlarmClock(new AlarmManager.AlarmClockInfo(ring, show(context)), ring(context, RING_CODE, ring));
            else
                alarms.cancel(ring(context, RING_CODE, 0));
            if (again > now)
                alarms.setAlarmClock(new AlarmManager.AlarmClockInfo(again, show(context)), ring(context, AGAIN_CODE, again));
            if (look > 0)
                alarms.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, look, look(context));
            else
                alarms.cancel(look(context));
        } catch (SecurityException e) {
            // "Alarms & reminders" taken back meanwhile.
            Log.w(DoseAlarms.TAG, "Waking: Android refused the alarm (" + why + "). " + e);
            return;
        }
        kept(context).edit().putLong(ARMED, ring).apply();
        Log.i(DoseAlarms.TAG, "Waking: " + (ring > 0 ? "next at " + Instant.ofEpochMilli(ring) + (usual ? " (the usual week)" : "") : "none") + ", " + why + ".");
    }

    /**
     * Rust asked for the coming wakings, then given them (WakeReceiver, on a
     * thread of its own: Rust reads the sharing first, `fetch` the sync app
     * given twenty seconds). Not before the phone's first unlock: Sioul's
     * files cannot be read then, and the list kept rings. No answer: the
     * list kept goes on.
     */
    static void ask(Context context, boolean fetch, String why)
    {
        if (!unlocked(context)) {
            Log.i(DoseAlarms.TAG, "Waking: not unlocked since the phone started; the list kept rings (" + why + ").");
            return;
        }
        String answer = null;
        try {
            DoseAlarms.load(context);
            answer = nativeNext(ZoneId.systemDefault().getId(), fetch);
        } catch (Throwable e) {
            // Sioul's library that does not load, among others.
            Log.e(DoseAlarms.TAG, "Waking: Sioul did not answer (" + why + "); the list kept goes on.", e);
        }
        if (answer != null && !answer.isEmpty())
            set(context, answer);
    }

    /** A ring rang (or rings now): known, so that a restart does not ring it again. */
    static synchronized void rang(Context context, long at)
    {
        SharedPreferences.Editor edit = kept(context).edit().putLong(RANG, at);
        if (kept(context).getLong(AGAIN, 0) <= at + DUE_MS)
            edit.remove(AGAIN);
        edit.commit();
    }

    /** A ring the phone missed while it was off, no more than half an hour ago; 0 when none. */
    static synchronized long missed(Context context)
    {
        SharedPreferences kept = kept(context);
        long now = System.currentTimeMillis();
        long rang = kept.getLong(RANG, 0);
        for (long at : new long[] { kept.getLong(ARMED, 0), kept.getLong(AGAIN, 0) })
            if (at > rang && at <= now && now - at < MISSED_MS)
                return at;
        return 0;
    }

    /** "10 min later": the alarm again in ten minutes, kept for a restart; when. */
    static synchronized long later(Context context)
    {
        long again = System.currentTimeMillis() + LATER_MINUTES * 60_000L;
        kept(context).edit().putLong(AGAIN, again).commit();
        try {
            if (exact(context))
                context.getSystemService(AlarmManager.class).setAlarmClock(new AlarmManager.AlarmClockInfo(again, show(context)), ring(context, AGAIN_CODE, again));
        } catch (SecurityException e) {
            Log.w(DoseAlarms.TAG, "Waking: Android refused the alarm 10 min later. " + e);
        }
        return again;
    }

    /** Stop: nothing rings again before the next waking. */
    static synchronized void stopped(Context context)
    {
        kept(context).edit().remove(AGAIN).commit();
        context.getSystemService(AlarmManager.class).cancel(ring(context, AGAIN_CODE, 0));
    }

    // ---------------------------------------------------------------- a try

    /**
     * "Try the alarm" (Health's settings; wake.rs hands {at (Unix ms), words}):
     * the alarm at `at`, ten seconds on, through the path a waking takes:
     * Android's alarm clock, WakeReceiver, WakeRinger (the rising sound, in
     * the foreground), WakeScreen over the lock screen. By its own action,
     * code and keys: the next waking, its list and its "10 min later" are left
     * as they are, and nothing is written of the night. When Android refuses
     * exact alarms, notifications or the full screen, nothing is set: the
     * answer says which (TRY_NO_…), for the setting to say why, rather than a
     * try that rings unseen or not at all. Not given again after a restart.
     */
    static synchronized int tryNow(Context context, String json)
    {
        int refused = !exact(context) ? TRY_NO_EXACT : !notificationsAllowed(context) ? TRY_NO_NOTIFICATIONS : !fullScreen(context) ? TRY_NO_SCREEN : TRY_RINGS;
        if (refused != TRY_RINGS) {
            Log.w(DoseAlarms.TAG, "Waking: no try: " + (refused == TRY_NO_EXACT ? "no \"Alarms & reminders\"." : refused == TRY_NO_NOTIFICATIONS ? "notifications off." : "no full screen."));
            return refused;
        }
        long now = System.currentTimeMillis();
        long at = now + TRY_SECONDS * 1000L;
        JSONObject words = null;
        try {
            JSONObject asked = new JSONObject(json);
            long given = asked.optLong("at");
            if (given > now && given < now + 60_000L)
                at = given;
            words = asked.optJSONObject("words");
        } catch (JSONException e) {
            // Ten seconds on, the words kept.
        }
        SharedPreferences.Editor edit = kept(context).edit().putLong(TRY_AT, at);
        if (words != null)
            edit.putString(TRY_WORDS, words.toString());
        edit.commit();
        // A try's "rings again" line, from a try before: no longer true.
        context.getSystemService(NotificationManager.class).cancel(WakeRinger.TRY_AGAIN_NOTE);
        channel(context);
        try {
            context.getSystemService(AlarmManager.class).setAlarmClock(new AlarmManager.AlarmClockInfo(at, show(context)), trial(context, at, false));
        } catch (SecurityException e) {
            Log.w(DoseAlarms.TAG, "Waking: no try: Android refused the alarm. " + e);
            return TRY_NO_EXACT;
        }
        Log.i(DoseAlarms.TAG, "Waking: a try at " + Instant.ofEpochMilli(at) + "; the next waking left as it is.");
        return TRY_RINGS;
    }

    /** A try's "10 min later": rung once more, as a try, its last time (without "10 min later"); when. */
    static synchronized long tryLater(Context context)
    {
        long at = System.currentTimeMillis() + LATER_MINUTES * 60_000L;
        kept(context).edit().putLong(TRY_AT, at).commit();
        try {
            if (exact(context))
                context.getSystemService(AlarmManager.class).setAlarmClock(new AlarmManager.AlarmClockInfo(at, show(context)), trial(context, at, true));
        } catch (SecurityException e) {
            Log.w(DoseAlarms.TAG, "Waking: Android refused the try 10 min later. " + e);
        }
        Log.i(DoseAlarms.TAG, "Waking: the try again at " + Instant.ofEpochMilli(at) + ", its last time.");
        return at;
    }

    /** A try over (Stop, or its last ring): its alarm, if one is left, taken back; nothing of the waking's. */
    static synchronized void tryStopped(Context context)
    {
        kept(context).edit().remove(TRY_AT).commit();
        context.getSystemService(AlarmManager.class).cancel(trial(context, 0, false));
        Log.i(DoseAlarms.TAG, "Waking: the try is over; the next waking left as it is.");
    }

    /** A try's alarm: to WakeReceiver as a waking's, by its own action and code. */
    private static PendingIntent trial(Context context, long at, boolean last)
    {
        Intent ring = new Intent(context, WakeReceiver.class).setAction(TRY).putExtra(AT, at).putExtra(TRIAL, true).putExtra(LAST, last);
        return PendingIntent.getBroadcast(context, TRY_CODE, ring, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /** The first of `times` after `after`, moved to the same time on the clock here; 0 when none. */
    private static long first(JSONArray times, ZoneId made, ZoneId here, long after)
    {
        long first = 0;
        for (int i = 0; times != null && i < times.length(); i++) {
            long at = rezoned(times.optLong(i), made, here);
            if (at > after && (first == 0 || at < first))
                first = at;
        }
        return first;
    }

    /**
     * A time made in another zone (the phone taken elsewhere, or Sioul's
     * zone a few minutes behind a change), at the same time on the clock
     * here: a waking at 07:00 stays at 07:00.
     */
    static long rezoned(long at, ZoneId made, ZoneId here)
    {
        if (at <= 0 || made == null || made.equals(here))
            return at;
        LocalDateTime clock = LocalDateTime.ofInstant(Instant.ofEpochMilli(at), made);
        return clock.atZone(here).toInstant().toEpochMilli();
    }

    /**
     * The next ring of the usual `week` (Monday first: "07:00", or "" when
     * that morning does not ring) after `after`, here; 0 when none is ticked.
     * What rings when Sioul cannot be asked: the usual mornings, repeated.
     */
    static long usual(JSONArray week, ZoneId here, long after)
    {
        if (week == null || week.length() != 7)
            return 0;
        LocalDate day = Instant.ofEpochMilli(after).atZone(here).toLocalDate();
        for (int i = 0; i < 9; i++, day = day.plusDays(1)) {
            String time = week.optString(day.getDayOfWeek().getValue() - 1);
            if (time.isEmpty())
                continue;
            try {
                // A time the change of hour skips: just after it, as Rust says it.
                long at = ZonedDateTime.of(day, LocalTime.parse(time), here).toInstant().toEpochMilli();
                if (at > after)
                    return at;
            } catch (RuntimeException e) {
                Log.w(DoseAlarms.TAG, "Waking: a usual time that does not read: " + time);
            }
        }
        return 0;
    }

    private static ZoneId zone(String id)
    {
        try {
            return id == null || id.isEmpty() ? null : ZoneId.of(id);
        } catch (RuntimeException e) {
            return null;
        }
    }

    /** A ring's alarm (the next waking, or the one after "10 min later"), to WakeReceiver, by no other app. */
    private static PendingIntent ring(Context context, int code, long at)
    {
        Intent ring = new Intent(context, WakeReceiver.class).setAction(RING).putExtra(AT, at);
        return PendingIntent.getBroadcast(context, code, ring, PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /** The question before a ring and as a night starts. */
    private static PendingIntent look(Context context)
    {
        return PendingIntent.getBroadcast(context, LOOK_CODE, new Intent(context, WakeReceiver.class).setAction(LOOK),
                                          PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    /** The alarm tapped in the status bar or on the lock screen: Sioul, as by its icon. */
    private static PendingIntent show(Context context)
    {
        Intent sioul = context.getPackageManager().getLaunchIntentForPackage(context.getPackageName());
        if (sioul == null)
            return null;
        return PendingIntent.getActivity(context, SHOW_CODE, sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED),
                                         PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
    }

    // ---------------------------------------------------------------- what Android allows

    /** "Alarms &amp; reminders": Android 12 gives it at install and lets you take it back; 13 gives it for good (USE_EXACT_ALARM). */
    static boolean exact(Context context)
    {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.S || context.getSystemService(AlarmManager.class).canScheduleExactAlarms();
    }

    /** The screen lit over the lock screen as it rings: yours to refuse from Android 14. */
    static boolean fullScreen(Context context)
    {
        return Build.VERSION.SDK_INT < Build.VERSION_CODES.UPSIDE_DOWN_CAKE || context.getSystemService(NotificationManager.class).canUseFullScreenIntent();
    }

    /** Sioul's notifications shown, and the "Waking" channel not turned off: else the ring has no Stop to show. */
    static boolean notificationsAllowed(Context context)
    {
        NotificationManager notifications = context.getSystemService(NotificationManager.class);
        if (!notifications.areNotificationsEnabled())
            return false;
        NotificationChannel waking = notifications.getNotificationChannel(CHANNEL);
        return waking == null || waking.getImportance() != NotificationManager.IMPORTANCE_NONE;
    }

    /** For Rust (android/main.cpp): 1 exact alarms, 2 the full screen, 4 notifications. */
    static int state(Context context)
    {
        return (exact(context) ? 1 : 0) | (fullScreen(context) ? 2 : 0) | (notificationsAllowed(context) ? 4 : 0);
    }

    /** Android's page where one is allowed: "exact", "screen", "notifications"; Sioul's own page else. */
    static void settings(Context context, String which)
    {
        String app = context.getPackageName();
        Intent page;
        if ("exact".equals(which) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S)
            page = new Intent(Settings.ACTION_REQUEST_SCHEDULE_EXACT_ALARM, Uri.parse("package:" + app));
        else if ("screen".equals(which) && Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
            page = new Intent(Settings.ACTION_MANAGE_APP_USE_FULL_SCREEN_INTENT, Uri.parse("package:" + app));
        else if ("notifications".equals(which))
            page = new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, app);
        else
            page = new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + app));
        try {
            context.startActivity(page.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        } catch (ActivityNotFoundException e) {
            context.startActivity(new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.parse("package:" + app)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK));
        }
    }

    /** Whether the phone was unlocked since it started: Sioul's own files can be read. */
    static boolean unlocked(Context context)
    {
        return context.getSystemService(UserManager.class).isUserUnlocked();
    }

    // ---------------------------------------------------------------- kept, and said

    /**
     * The "Waking" channel: high importance (over the lock screen, the
     * screen lit). Without a sound of its own: WakeRinger plays it, rising,
     * where a channel's would be at one volume and stop as the shade opens.
     */
    static void channel(Context context)
    {
        NotificationChannel waking = Channels.quiet(CHANNEL, word(context, "channel"), NotificationManager.IMPORTANCE_HIGH);
        waking.setSound(null, null);
        waking.enableVibration(false);
        waking.setLockscreenVisibility(Notification.VISIBILITY_PUBLIC);
        context.getSystemService(NotificationManager.class).createNotificationChannel(waking);
    }

    /**
     * Kept where the phone reads it before its first unlock (device-protected
     * storage): times and words only, nothing of what Sioul keeps.
     */
    private static SharedPreferences kept(Context context)
    {
        return context.createDeviceProtectedStorageContext().getSharedPreferences(KEPT, Context.MODE_PRIVATE);
    }

    private static JSONObject list(Context context)
    {
        try {
            return new JSONObject(kept(context).getString(LIST, "{}"));
        } catch (JSONException e) {
            return new JSONObject();
        }
    }

    private static final String[][] WORDS = {
        { "channel", "Waking", "Réveil" },
        { "title", "Waking", "Réveil" },
        { "stop", "Stop", "Arrêter" },
        { "later", "10 min later", "10 min plus tard" },
        { "again", "Rings again at {time}", "Sonne de nouveau à {time}" },
    };

    /**
     * The words said here, as Rust gave them with the wakings (or with a
     * try), in the person's language (crates/sioul-core/locales); before
     * either, these, by the phone's language.
     */
    static String word(Context context, String which)
    {
        JSONObject words = list(context).optJSONObject("words");
        String said = words == null ? "" : words.optString(which);
        if (said.isEmpty()) {
            try {
                said = new JSONObject(kept(context).getString(TRY_WORDS, "{}")).optString(which);
            } catch (JSONException e) {
                said = "";
            }
        }
        if (!said.isEmpty())
            return said;
        boolean french = "fr".equals(Locale.getDefault().getLanguage());
        for (String[] word : WORDS)
            if (word[0].equals(which))
                return word[french ? 2 : 1];
        return which;
    }
}
