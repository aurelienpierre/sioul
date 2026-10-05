// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * A dose's alarm, and the "Taken" of its reminder (DoseAlarms): each asks
 * Rust's side (crates/sioul-app/src/alarms.rs) on a thread of its own, the
 * broadcast held open meanwhile (goAsync). At an alarm, Rust first gives the
 * sync app twenty seconds to bring your other devices' marks; Android gives
 * the alarm's broadcast a minute (it comes from the background), the
 * button's ten seconds (from the foreground). An expedited JobScheduler job
 * would get longer, but waits for its turn, its quota and the phone's sleep,
 * where a broadcast runs at once. Past the deadline the broadcast is let go,
 * before Android takes Sioul for hung and stops it: an alarm's reminder then
 * says to check in Sioul rather than nothing, and Rust's answer, if it comes,
 * replaces it.
 */
public final class DoseReceiver extends BroadcastReceiver
{
    private static final long DEADLINE_MS = 45_000;
    private static final long FOREGROUND_DEADLINE_MS = 8_000;

    @Override
    public void onReceive(Context context, Intent intent)
    {
        final String action = intent.getAction();
        final String key = DoseAlarms.key(intent);
        final boolean alarm = DoseAlarms.ALARM.equals(action);
        if (key.isEmpty() || !(alarm || DoseAlarms.TAKEN.equals(action)))
            return;
        final Context app = context.getApplicationContext();
        final Asked asked = new Asked(goAsync());
        final Handler main = new Handler(Looper.getMainLooper());
        final Runnable deadline = () -> asked.letGo(alarm ? app : null, key);
        final boolean foreground = (intent.getFlags() & Intent.FLAG_RECEIVER_FOREGROUND) != 0;
        main.postDelayed(deadline, foreground ? FOREGROUND_DEADLINE_MS : DEADLINE_MS);
        new Thread(() -> {
            String answer = null;
            try {
                DoseAlarms.load(app);
                answer = alarm ? DoseAlarms.nativeDecide(key) : DoseAlarms.nativeTaken(key);
            } catch (Throwable e) {
                // Sioul's library that does not load, among others: said all the same.
                Log.e(DoseAlarms.TAG, "Doses: no answer about " + key, e);
            }
            main.removeCallbacks(deadline);
            asked.answer(app, intent, key, alarm, answer);
        }, "sioul-dose").start();
    }

    /** One broadcast: answered in time, or let go at the deadline and answered after. */
    private static final class Asked
    {
        private final PendingResult broadcast;
        private boolean late;
        private boolean finished;

        Asked(PendingResult broadcast)
        {
            this.broadcast = broadcast;
        }

        /** The deadline: an alarm not answered yet says to check in Sioul; the broadcast goes. */
        synchronized void letGo(Context alarm, String key)
        {
            if (finished)
                return;
            late = true;
            if (alarm != null)
                DoseAlarms.fallback(alarm, key);
            finish();
        }

        synchronized void answer(Context context, Intent intent, String key, boolean alarm, String answer)
        {
            try {
                if (alarm)
                    decided(context, intent, key, answer, late);
                else
                    taken(context, intent, key, answer);
            } catch (RuntimeException e) {
                Log.e(DoseAlarms.TAG, "Doses: the answer about " + key + " could not be shown", e);
            }
            finish();
        }

        private void finish()
        {
            if (finished)
                return;
            finished = true;
            broadcast.finish();
        }
    }

    /**
     * Rust's decision at a dose's time: the reminder, as Rust says it; or
     * nothing, maybe asked again later (news from another device awaited).
     */
    private static void decided(Context context, Intent intent, String key, String answer, boolean late)
    {
        String name = text(intent.getStringExtra(DoseAlarms.NAME));
        JSONObject said = parse(answer);
        if (said == null) {
            DoseAlarms.fallback(context, key);
            return;
        }
        if (said.optBoolean("show")) {
            String title = said.optString("title");
            DoseAlarms.remind(context, key, title.isEmpty() ? "Sioul" : title, said.optString("body"), name, said.optString("taken"));
            DoseAlarms.reminded(context, key);
        } else if (late) {
            // The reminder to check in Sioul, said for nothing.
            DoseAlarms.dismiss(context, key);
        }
        long again = said.optLong("again_at");
        if (again > 0)
            DoseAlarms.again(context, key, again, name);
    }

    /**
     * Rust's answer to "Taken": recorded, said quietly; more than half an hour
     * late, when it was taken is asked in Sioul. Else the reminder stays as it
     * was, its button still there.
     */
    private static void taken(Context context, Intent intent, String key, String answer)
    {
        String title = text(intent.getStringExtra(DoseAlarms.TITLE));
        String name = text(intent.getStringExtra(DoseAlarms.NAME));
        JSONObject said = parse(answer);
        if (said != null && said.optBoolean("done"))
            DoseAlarms.confirm(context, key, name.isEmpty() ? title : name, said.optString("line"));
        else if (said != null && said.optBoolean("open"))
            DoseAlarms.askWhen(context, key, title, said.optString("line"));
        else
            Log.w(DoseAlarms.TAG, "Doses: \"Taken\" not recorded for " + key + ": " + answer);
    }

    private static JSONObject parse(String answer)
    {
        if (answer == null)
            return null;
        try {
            return new JSONObject(answer);
        } catch (JSONException e) {
            Log.e(DoseAlarms.TAG, "Doses: an answer that does not read: " + answer);
            return null;
        }
    }

    private static String text(String text)
    {
        return text == null ? "" : text;
    }
}
