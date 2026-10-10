// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.AlarmManager;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * The events' reminders (EventAlarms): at a reminder's time, Rust is asked
 * what to say (crates/sioul-app/src/eventalarms.rs) on a thread of its own,
 * the broadcast held open meanwhile (goAsync), as for the doses
 * (DoseReceiver). Past 45 seconds the broadcast is let go before Android
 * takes Sioul for hung, and the words given with the list are said: never
 * nothing; Rust's answer, if it comes, replaces them or takes them away.
 * Also the phone's own moments: started, Sioul updated, the time or the zone
 * changed, "Alarms &amp; reminders" allowed again: the alarms given again from
 * the list kept, then Rust asked for the list as it is now; and once a day.
 */
public final class EventReceiver extends BroadcastReceiver
{
    private static final long DEADLINE_MS = 45_000;

    @Override
    public void onReceive(Context context, Intent intent)
    {
        final String action = intent.getAction();
        if (action == null)
            return;
        final Context app = context.getApplicationContext() == null ? context : context.getApplicationContext();
        switch (action) {
        case EventAlarms.RING: {
            final String key = EventAlarms.key(intent);
            if (!key.isEmpty())
                ring(app, key);
            return;
        }
        case EventAlarms.LOOK:
            ask(app, "looked");
            return;
        case Intent.ACTION_BOOT_COMPLETED:
        case Intent.ACTION_MY_PACKAGE_REPLACED:
        case Intent.ACTION_TIME_CHANGED:
        case Intent.ACTION_TIMEZONE_CHANGED:
        case AlarmManager.ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED:
            EventAlarms.restore(app, action);
            ask(app, action);
            return;
        default:
            // Nothing else is sent here.
        }
    }

    /** A reminder's time: Rust's decision, or the words kept at the deadline. */
    private void ring(Context app, String key)
    {
        final Asked asked = new Asked(goAsync());
        final Handler main = new Handler(Looper.getMainLooper());
        final Runnable deadline = () -> asked.letGo(app, key);
        main.postDelayed(deadline, DEADLINE_MS);
        new Thread(() -> {
            String answer = null;
            try {
                DoseAlarms.load(app);
                answer = EventAlarms.nativeDecide(key);
            } catch (Throwable e) {
                // Sioul's library that does not load, among others: said all the same.
                Log.e(DoseAlarms.TAG, "Events: no answer about " + key, e);
            }
            main.removeCallbacks(deadline);
            asked.answer(app, key, answer);
        }, "sioul-event").start();
    }

    /** Rust asked for the list as it is now, on a thread of its own, the broadcast held open. */
    private void ask(Context app, String why)
    {
        final Asked asked = new Asked(goAsync());
        final Handler main = new Handler(Looper.getMainLooper());
        final Runnable deadline = () -> asked.letGo(null, "");
        main.postDelayed(deadline, DEADLINE_MS);
        new Thread(() -> {
            EventAlarms.ask(app, why);
            main.removeCallbacks(deadline);
            asked.finish();
        }, "sioul-events").start();
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

        /** The deadline: a reminder not answered yet is said with the words kept; the broadcast goes. */
        synchronized void letGo(Context context, String key)
        {
            if (finished)
                return;
            late = true;
            if (context != null)
                EventAlarms.fallback(context, key);
            finish();
        }

        synchronized void answer(Context context, String key, String answer)
        {
            try {
                decided(context, key, answer, late);
            } catch (RuntimeException e) {
                Log.e(DoseAlarms.TAG, "Events: the answer about " + key + " could not be shown", e);
            }
            finish();
        }

        synchronized void finish()
        {
            if (finished)
                return;
            finished = true;
            broadcast.finish();
        }
    }

    /**
     * Rust's decision: shown already (by Rust, through {@link EventAlarms#show});
     * held now, asked again at `again_at`; or nothing (moved, cancelled, told,
     * too late). No answer at all: the words kept, rather than nothing.
     */
    private static void decided(Context context, String key, String answer, boolean late)
    {
        JSONObject said = null;
        if (answer != null && !answer.equals("null")) {
            try {
                said = new JSONObject(answer);
            } catch (JSONException e) {
                Log.e(DoseAlarms.TAG, "Events: an answer that does not read: " + answer);
            }
        }
        if (said == null) {
            if (!late)
                EventAlarms.fallback(context, key);
            EventAlarms.done(context, key);
            return;
        }
        if (said.optBoolean("shown"))
            return;
        // Said for nothing at the deadline: taken away.
        if (late)
            EventAlarms.dismiss(context, key);
        long again = said.optLong("again_at");
        if (again > 0)
            EventAlarms.again(context, key, again);
        else
            EventAlarms.done(context, key);
    }
}
