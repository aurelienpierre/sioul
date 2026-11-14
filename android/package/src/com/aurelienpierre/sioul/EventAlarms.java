// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.AlarmManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.SharedPreferences;
import android.net.Uri;
import android.os.UserManager;
import android.util.Log;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.time.ZoneId;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/**
 * The time to leave for an event while Sioul is not on the screen
 * (docs/android.md, "Events"): Android freezes an app it does not show, so
 * Rust hands the coming ones ahead (crates/sioul-app/src/eventalarms.rs:
 * Sioul's reminder before each event), and each is given to Android's alarm
 * clock, as the doses are (DoseAlarms). At its time EventReceiver asks Rust
 * what to say: the event read again, sleep or a pause holding it or not, the
 * time left said as it is then. Rust shows it through `show`, a notice to
 * know (Notices) that lasts until its event starts; a tap opens the event in
 * Sioul (ReminderOpener). Rust silent, the words given with the list are
 * shown instead, with the event's address: never nothing. An event's own
 * alarm is never rung here: the phone's calendar app rings it. Apart from
 * the doses: its own list, alarms and receiver. The list also holds when
 * the end of work's notice, the night's and the meals' may come (kind
 * "day"): Rust decides those at their alarm and shows them through
 * DayNotes, or nothing; no words are kept for them. Also the taps on
 * Sioul's notices, kept until Sioul takes them (`opened`).
 */
final class EventAlarms
{
    static final String RING = "com.aurelienpierre.sioul.action.EVENT_REMINDER";
    static final String LOOK = "com.aurelienpierre.sioul.action.EVENT_LOOK";
    private static final String SCHEME = "sioul-event";
    private static final String KEPT = "sioul-events";
    private static final String LIST = "list";
    private static final String WORDS = "words";
    private static final String OPENED = "opened";
    /** The taps kept (`opened`), a list since 14 November 2026: an older Sioul's single one is dropped. */
    private static final String TAPS = "taps";
    /** How long a notice to know lasts once seen, as Rust gave it with the list (ms). */
    private static final String LASTS = "lasts_ms";
    /** The question asked again once a day, nothing having rung meanwhile. */
    private static final int LOOK_CODE = 0x5801;
    /** A tap older than this opens nothing: Sioul did not come up meanwhile, and would open it out of the blue. */
    static final long TAP_KEPT_MS = 5 * 60 * 1000L;
    /** The taps kept at most, the oldest let go first. */
    static final int TAPS_KEPT = 8;
    /** A notice to know lasts this long once seen when Rust has said nothing yet (ms): Sioul's own lifetime unsaid. */
    static final long LASTS_UNSAID_MS = 5 * 60 * 1000L;

    private EventAlarms() {}

    /** At a reminder's time (eventalarms.rs, `sioul_event_decide`): {"shown"}, {"again_at"}, {}; null when Rust failed. */
    static native String nativeDecide(String key);
    /** The coming reminders, in the phone's `zone` (`sioul_event_coming`); "" when Rust failed. */
    static native String nativeComing(String zone);

    // ---------------------------------------------------------------- the list

    /**
     * Rust's coming reminders: JSON {reminders: [{key, kind, at (Unix ms),
     * title, body, about, ends}], look (Unix ms), lasts_ms, words}. Their
     * alarms replace those given before, as `merged` says; the list is kept
     * for Android's restarts. Any thread.
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
        List<Arm> armed = new ArrayList<>();
        JSONArray list = merged(before, coming, System.currentTimeMillis(), armed);
        JSONObject words = given.optJSONObject("words");
        SharedPreferences.Editor kept = kept(context).edit().putString(LIST, list.toString());
        if (words != null)
            kept.putString(WORDS, words.toString());
        if (given.optLong(LASTS) > 0)
            kept.putLong(LASTS, given.optLong(LASTS));
        kept.commit();
        // The channels of the notices made, an older Sioul's deleted.
        Notices.channels(context, words);
        for (Arm arm : armed)
            alarm(context, arm.key, arm.at);
        look(context, given.optLong("look"));
        Log.i(DoseAlarms.TAG, "Events: " + coming.length() + " reminders given, " + armed.size() + " alarms set, " + (DoseAlarms.exact(context) ? "on time." : "inexact (no \"Alarms & reminders\")."));
    }

    /** An alarm to set: its reminder's key, and when (Unix ms). */
    static final class Arm
    {
        final String key;
        final long at;

        Arm(String key, long at)
        {
            this.key = key;
            this.at = at;
        }
    }

    /** Whether a reminder of the list rang and was answered (`done`): kept while Rust lists it, never set again. */
    static final String DONE = "done";

    /** The time Rust gave a reminder last (Unix ms), kept apart from its "at", the time its alarm is set for, which `again` moves. */
    static final String GIVEN = "given";

    /**
     * The list kept once Rust gives `coming` (`set`), and the alarms to set
     * for it, put in `armed`: a reminder new to the list, at its time (one
     * past rings at once: Rust says whether it still makes sense); one kept
     * already, at a time Rust moved into the future, or moved earlier than
     * it said before while it is not done, though that be past (the hours
     * edited so that today's end of work was a few minutes ago; a hold that
     * ended early): it rings at once; never one kept already again in the
     * past at the time Rust said before: it rang already, or rings at the
     * time `again` gave it, and keeps that time and whether it is done. So a
     * list given again and again with a past reminder in it (the end of
     * work's, while its ten minutes last) never rings it again and again:
     * Rust's times for it stay put, or only grow ("a second from now"). Pure,
     * checked on a JVM (android/jvm-checks/DayNotesCheck.java).
     *
     * @param kept the list kept until now
     * @param coming the list Rust gives
     * @param now the time now (Unix ms)
     * @param armed the alarms to set, added to
     * @return the list to keep
     */
    static JSONArray merged(JSONArray kept, JSONArray coming, long now, List<Arm> armed)
    {
        Map<String, JSONObject> before = new HashMap<>();
        for (int i = 0; i < kept.length(); i++) {
            JSONObject reminder = kept.optJSONObject(i);
            if (!key(reminder).isEmpty())
                before.put(key(reminder), reminder);
        }
        JSONArray out = new JSONArray();
        for (int i = 0; i < coming.length(); i++) {
            JSONObject reminder = coming.optJSONObject(i);
            if (reminder == null)
                continue;
            String key = key(reminder);
            long at = reminder.optLong("at");
            JSONObject old = before.get(key);
            JSONObject listed = copied(reminder);
            put(listed, GIVEN, at);
            // Moved earlier than Rust said last; an entry an older Sioul kept says nothing of it.
            boolean moved = old != null && old.has(GIVEN) && !old.optBoolean(DONE) && at < old.optLong(GIVEN);
            if (old != null && at <= now && !moved) {
                put(listed, "at", old.optLong("at"));
                if (old.optBoolean(DONE))
                    put(listed, DONE, true);
                out.put(listed);
                continue;
            }
            out.put(listed);
            if (!key.isEmpty() && at > 0)
                armed.add(new Arm(key, at));
        }
        return out;
    }

    /**
     * The list with `key` asked about again at `at` (`again`), set again,
     * not done. Pure (DayNotesCheck).
     *
     * @param kept the list kept
     * @param key the reminder's key
     * @param at when (Unix ms)
     * @return the list to keep
     */
    static JSONArray asked(JSONArray kept, String key, long at)
    {
        JSONArray out = new JSONArray();
        for (int i = 0; i < kept.length(); i++) {
            JSONObject reminder = copied(kept.optJSONObject(i));
            if (key.equals(key(reminder))) {
                put(reminder, "at", at);
                reminder.remove(DONE);
            }
            out.put(reminder);
        }
        return out;
    }

    /**
     * The list with `key` done with (`done`): kept, marked, while Rust lists
     * it, so that it is never set again; gone with the next list without it.
     * Pure (DayNotesCheck).
     *
     * @param kept the list kept
     * @param key the reminder's key
     * @return the list to keep
     */
    static JSONArray answered(JSONArray kept, String key)
    {
        JSONArray out = new JSONArray();
        for (int i = 0; i < kept.length(); i++) {
            JSONObject reminder = copied(kept.optJSONObject(i));
            if (key.equals(key(reminder)))
                put(reminder, DONE, true);
            out.put(reminder);
        }
        return out;
    }

    /** A copy of `reminder`, or an empty one. */
    private static JSONObject copied(JSONObject reminder)
    {
        try {
            return reminder == null ? new JSONObject() : new JSONObject(reminder.toString());
        } catch (JSONException e) {
            return new JSONObject();
        }
    }

    /** The alarms given again from the list kept: Android forgets them when it restarts. One whose time went by rings at once: Rust says whether it still makes sense. Those done with stay unset. */
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
        kept(context).edit().putString(LIST, asked(list(context), key, at).toString()).commit();
        alarm(context, key, at);
    }

    /** A reminder done with (told, gone): marked, not to ring again, after a restart neither (`answered`). */
    static synchronized void done(Context context, String key)
    {
        kept(context).edit().putString(LIST, answered(list(context), key).toString()).commit();
    }

    private static void schedule(Context context, JSONArray coming)
    {
        for (int i = 0; i < coming.length(); i++) {
            JSONObject reminder = coming.optJSONObject(i);
            String key = key(reminder);
            long at = reminder == null ? 0 : reminder.optLong("at");
            if (!key.isEmpty() && at > 0 && !reminder.optBoolean(DONE))
                alarm(context, key, at);
        }
    }

    /**
     * A reminder's alarm, at its time even while the phone sleeps (Doze);
     * without "Alarms &amp; reminders", as close as Android allows: up to an hour
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

    /**
     * The times of the reminders to come (ms), not done, read fresh from
     * another of Sioul's processes, never written there: the listener's
     * quiet window keeps off them (AppNotes), since under its hint they
     * would come without a sound.
     */
    @SuppressWarnings("deprecation")
    static long[] coming(Context context)
    {
        JSONArray coming;
        try {
            coming = new JSONArray(context.getSharedPreferences(KEPT, Context.MODE_PRIVATE | Context.MODE_MULTI_PROCESS).getString(LIST, "[]"));
        } catch (JSONException e) {
            return new long[0];
        }
        long[] times = new long[coming.length()];
        int n = 0;
        for (int i = 0; i < coming.length(); i++) {
            JSONObject reminder = coming.optJSONObject(i);
            if (reminder != null && reminder.optLong("at", 0) > 0 && !reminder.optBoolean(DONE))
                times[n++] = reminder.optLong("at");
        }
        return Arrays.copyOf(times, n);
    }

    private static JSONArray list(Context context)
    {
        try {
            return new JSONArray(kept(context).getString(LIST, "[]"));
        } catch (JSONException e) {
            return new JSONArray();
        }
    }

    /**
     * A tapped notification's thing, kept until Sioul takes it
     * (ReminderOpener): "notice" and its address; an older notice's
     * "event", "porch"… Each tap kept apart, never in another's place
     * (`tapped`).
     */
    static synchronized void opened(Context context, String kind, String key)
    {
        JSONArray taps;
        try {
            taps = new JSONArray(kept(context).getString(TAPS, "[]"));
        } catch (JSONException e) {
            taps = new JSONArray();
        }
        kept(context).edit().putString(TAPS, tapped(taps, kind, key, System.currentTimeMillis()).toString()).remove(OPENED).commit();
    }

    /**
     * What tapped notifications open, each once, as a JSON list
     * [{kind, key}] in the order they were tapped; null when nothing
     * (android/main.cpp, for Rust: as Sioul comes back, and as it starts).
     * A tap older than five minutes is let go (`fresh`); an older Sioul's
     * single tap, without its time, too.
     */
    static synchronized String takeOpened(Context context)
    {
        SharedPreferences kept = kept(context);
        String taps = kept.getString(TAPS, null);
        if (taps == null && !kept.contains(OPENED))
            return null;
        kept.edit().remove(TAPS).remove(OPENED).commit();
        JSONArray fresh;
        try {
            fresh = fresh(new JSONArray(taps == null ? "[]" : taps), System.currentTimeMillis());
        } catch (JSONException e) {
            return null;
        }
        return fresh.length() == 0 ? null : fresh.toString();
    }

    /**
     * The taps kept with `kind` and `key` tapped at `now` (Unix ms): last,
     * the same thing tapped before taken out (one tap per thing), the oldest
     * let go beyond eight. Pure, checked on a JVM
     * (android/jvm-checks/NoticesCheck.java).
     *
     * @param taps the taps kept: [{kind, key, at}]
     * @param kind what was tapped
     * @param key what in it
     * @param now the time now (Unix ms)
     * @return the taps to keep
     */
    static JSONArray tapped(JSONArray taps, String kind, String key, long now)
    {
        String said = key == null ? "" : key;
        List<JSONObject> kept = new ArrayList<>();
        for (int i = 0; i < taps.length(); i++) {
            JSONObject tap = taps.optJSONObject(i);
            if (tap != null && !(tap.optString("kind").equals(kind) && tap.optString("key").equals(said)))
                kept.add(tap);
        }
        JSONObject tap = new JSONObject();
        put(tap, "kind", kind);
        put(tap, "key", said);
        put(tap, "at", now);
        kept.add(tap);
        JSONArray out = new JSONArray();
        for (JSONObject one : kept.subList(Math.max(0, kept.size() - TAPS_KEPT), kept.size()))
            out.put(one);
        return out;
    }

    /**
     * The taps still worth opening at `now` (Unix ms), {kind, key} each, in
     * their order: tapped within five minutes, never one from the future (a
     * clock set back) nor one without its time. Pure (NoticesCheck).
     *
     * @param taps the taps kept: [{kind, key, at}]
     * @param now the time now (Unix ms)
     * @return those to open
     */
    static JSONArray fresh(JSONArray taps, long now)
    {
        JSONArray out = new JSONArray();
        for (int i = 0; i < taps.length(); i++) {
            JSONObject tap = taps.optJSONObject(i);
            long at = tap == null ? 0 : tap.optLong("at");
            if (at <= 0 || at > now || now - at > TAP_KEPT_MS)
                continue;
            JSONObject thing = new JSONObject();
            put(thing, "kind", tap.optString("kind"));
            put(thing, "key", tap.optString("key"));
            out.put(thing);
        }
        return out;
    }

    // ---------------------------------------------------------------- the reminders

    /**
     * Rust's reminder (eventalarms.rs, `shown`): a notice (Notices.Said) with
     * {key}, the reminder's key, its tag too; a tap, or Open, brings Sioul up
     * on the event. Its alarm then has nothing left to do. Any thread.
     */
    static void show(Context context, String json)
    {
        final JSONObject said;
        try {
            said = new JSONObject(json);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "Events: a reminder that does not read (" + e.getClass().getSimpleName() + ").");
            return;
        }
        String key = said.optString("key");
        Notices.post(context, Notices.Said.of(said));
        // Told (from the window's minute, maybe): its alarm has nothing left to do.
        cancel(context, key);
        done(context, key);
    }

    /**
     * The words given with the list, said when Rust cannot be asked at a
     * reminder's time: never nothing. Not the end of work's notice, the
     * night's nor a meal's (kind "day"): said by Rust alone, or not at all,
     * as on a computer (Rust's notice may not come: the day closed, a
     * meeting, someone at a computer).
     */
    static void fallback(Context context, String key)
    {
        JSONObject reminder = kept(context, key);
        if (reminder == null || isDay(reminder.optString("kind")))
            return;
        Notices.post(context, fallen(reminder, key, lasts(context), System.currentTimeMillis(), word(context, "open")));
    }

    /**
     * A reminder of the list said from its words (`fallback`): a notice to
     * know tagged by its key, about its event (its address and time, as the
     * list gives it: a tap opens it too), lasting until the event starts
     * (`ends`), else the minutes Rust last gave (`lasts`). Pure, checked on
     * a JVM (android/jvm-checks/NoticesCheck.java).
     *
     * @param reminder the list's entry: {key, kind, title, body, about, ends}
     * @param key its key
     * @param lasts how long a notice to know lasts once seen (ms)
     * @param now the time now (Unix ms)
     * @param open the words of its button, "Open"
     * @return the notice to post
     */
    static Notices.Said fallen(JSONObject reminder, String key, long lasts, long now, String open)
    {
        Notices.Said said = new Notices.Said();
        said.tag = key;
        String about = reminder.optString("about");
        said.about = ReminderOpener.isNoticeAddress(about) ? about : "";
        said.kind = reminder.optString("kind");
        said.endsMs = Math.max(0, reminder.optLong("ends"));
        said.lastsMs = said.endsMs > now ? said.endsMs - now : lasts;
        said.title = reminder.optString("title");
        said.body = reminder.optString("body");
        if (!said.about.isEmpty())
            said.buttons.add(new Notices.Button(open, said.about, ""));
        return said;
    }

    /**
     * Whether an entry of the list is the end of work's notice, the night's
     * or a meal's (DayNotes), which have no words kept. Pure, checked on a JVM
     * (android/jvm-checks/DayNotesCheck.java).
     *
     * @param kind the entry's kind, as Rust gave it
     * @return whether it is one of the day's
     */
    static boolean isDay(String kind)
    {
        return "day".equals(kind);
    }

    /** A reminder said for nothing (by the fallback) taken away. */
    static void dismiss(Context context, String key)
    {
        Notices.dismiss(context, key);
    }

    /** How long a notice to know lasts once seen (ms), as Rust last gave it with the list; five minutes before any. */
    static long lasts(Context context)
    {
        long given = kept(context).getLong(LASTS, 0);
        return given > 0 ? given : LASTS_UNSAID_MS;
    }

    /** The words Rust gave with a notice, kept for those said while it cannot be asked. */
    static void keepWords(Context context, JSONObject words)
    {
        if (words != null && words.length() > 0)
            kept(context).edit().putString(WORDS, words.toString()).apply();
    }

    // ---------------------------------------------------------------- the words said here

    private static final String[][] FALLBACK_WORDS = {
        { "notices", "Notices", "À savoir" },
        { "act", "To act on", "À faire" },
        { "act-pause", "To act on during a pause", "À faire pendant une pause" },
        { "mail-through", "New mail from people always let through", "Courriels des personnes qui passent toujours" },
        { "open", "Open", "Ouvrir" },
        { "public", "Sioul has a reminder for you.", "Sioul a un rappel pour vous." },
    };

    /**
     * The words said here, as Rust gave them with the list or a notice (in
     * the person's language, crates/sioul-core/locales); before any, these,
     * by the phone's language. Without a context (a check on a JVM), these.
     */
    static String word(Context context, String which)
    {
        try {
            String said = context == null ? "" : new JSONObject(kept(context).getString(WORDS, "{}")).optString(which);
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
