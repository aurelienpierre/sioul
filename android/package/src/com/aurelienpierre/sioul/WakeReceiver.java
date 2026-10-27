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

import java.util.concurrent.atomic.AtomicBoolean;

/**
 * The alarm at waking's moments (WakeAlarms): its ring, which starts the
 * ringer at once and gives the next from the list kept, then asks Rust for
 * the wakings as they are now; its look, a little before each ring and as
 * each night starts, which asks Rust after the sync app had its twenty
 * seconds (Sioul may be frozen while another device changed the night); a
 * try ("Try the alarm"), rung the same way and nothing else; Stop and 10 min
 * later; and the phone's own moments: started (before its first
 * unlock, then after), Sioul updated, the time or the zone changed, "Alarms &amp;
 * reminders" allowed again, each giving the alarms again. Rust is asked on a
 * thread of its own, the broadcast held open meanwhile (goAsync) and let go
 * before Android's minute. Read before the first unlock (directBootAware):
 * then the list kept rings, and Rust waits for the unlock.
 */
public final class WakeReceiver extends BroadcastReceiver
{
    private static final long DEADLINE_MS = 50_000;

    @Override
    public void onReceive(Context context, Intent intent)
    {
        final String action = intent.getAction();
        if (action == null)
            return;
        final Context app = context.getApplicationContext() == null ? context : context.getApplicationContext();
        switch (action) {
        case WakeAlarms.RING: {
            long at = intent.getLongExtra(WakeAlarms.AT, System.currentTimeMillis());
            WakeRinger.ring(app, at, false, false);
            WakeAlarms.rang(app, at);
            WakeAlarms.arm(app, "rang");
            ask(app, false, "rang");
            return;
        }
        case WakeAlarms.TRY: {
            // "Try the alarm": rung as a waking, and nothing else: the next waking, its list and Rust are left alone.
            long at = intent.getLongExtra(WakeAlarms.AT, System.currentTimeMillis());
            boolean last = intent.getBooleanExtra(WakeAlarms.LAST, false);
            Log.i(DoseAlarms.TAG, "Waking: the try rings" + (last ? ", its last time." : "."));
            WakeRinger.ring(app, at, true, last);
            return;
        }
        case WakeAlarms.LOOK:
            WakeAlarms.arm(app, "looked");
            ask(app, true, "looked");
            return;
        case WakeAlarms.STOP:
            WakeRinger.stop(app, false, intent.getBooleanExtra(WakeAlarms.TRIAL, false));
            return;
        case WakeAlarms.LATER:
            WakeRinger.stop(app, true, intent.getBooleanExtra(WakeAlarms.TRIAL, false));
            return;
        case Intent.ACTION_LOCKED_BOOT_COMPLETED:
        case Intent.ACTION_BOOT_COMPLETED: {
            // A ring missed while the phone was off: now, if not half an hour late (a try is not given again).
            long missed = WakeAlarms.missed(app);
            if (missed > 0) {
                WakeRinger.ring(app, missed, false, false);
                WakeAlarms.rang(app, missed);
            }
            WakeAlarms.arm(app, action);
            ask(app, false, action);
            return;
        }
        case Intent.ACTION_MY_PACKAGE_REPLACED:
        case Intent.ACTION_TIME_CHANGED:
        case Intent.ACTION_TIMEZONE_CHANGED:
        case AlarmManager.ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED:
            WakeAlarms.arm(app, action);
            // The doses' alarms too, keyed in the new zone or for the clock set, in the same
            // broadcast held open, once the phone is unlocked (the A4 check, #2).
            ask(app, false, action, Intent.ACTION_TIME_CHANGED.equals(action) || Intent.ACTION_TIMEZONE_CHANGED.equals(action));
            return;
        default:
            // Nothing else is sent here.
        }
    }

    /** Rust asked, on a thread of its own, the broadcast held open until it answers or Android's deadline nears. */
    private void ask(Context app, boolean fetch, String why)
    {
        ask(app, fetch, why, false);
    }

    /** The same, the doses handed first when `doses` (the zone or the clock changed: DoseAlarms.zoneChanged). */
    private void ask(Context app, boolean fetch, String why, boolean doses)
    {
        if (!WakeAlarms.unlocked(app))
            return;
        final PendingResult broadcast = goAsync();
        final AtomicBoolean finished = new AtomicBoolean();
        final Runnable letGo = () -> {
            if (finished.compareAndSet(false, true))
                broadcast.finish();
        };
        final Handler main = new Handler(Looper.getMainLooper());
        main.postDelayed(letGo, DEADLINE_MS);
        new Thread(() -> {
            if (doses)
                DoseAlarms.zoneChanged(app);
            WakeAlarms.ask(app, fetch, why);
            main.removeCallbacks(letGo);
            letGo.run();
        }, "sioul-waking").start();
    }
}
