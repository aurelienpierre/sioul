// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.AlarmManager;
import android.app.PendingIntent;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.util.Log;

/**
 * Do-not-disturb applied in Sioul's own process (crates/sioul-app/src/
 * everywhere.rs, `apply`), where Android's modes of Sioul's are kept
 * (PauseMode): asked by the background service when your other devices
 * changed it, at the alarm of its next end (the switch's "until", waking,
 * the night), and after the phone's restart. Sioul's window open or not:
 * the library is loaded alone, as for a dose (DoseAlarms.load), and Rust
 * answers on a thread of its own, the broadcast held open meanwhile.
 */
public final class DndReceiver extends BroadcastReceiver
{
    static final String APPLY = "com.aurelienpierre.sioul.action.DND_APPLY";
    private static final int CODE = 0x5138;

    /** Do-not-disturb as the reasons say now, applied here (Rust); its answer, "" when none. */
    static native String nativeApply();

    @Override
    public void onReceive(Context context, Intent intent)
    {
        String action = intent.getAction();
        if (!APPLY.equals(action) && !Intent.ACTION_BOOT_COMPLETED.equals(action) && !Intent.ACTION_MY_PACKAGE_REPLACED.equals(action))
            return;
        final Context app = context.getApplicationContext();
        final PendingResult held = goAsync();
        new Thread(() -> {
            try {
                DoseAlarms.load(app);
                nativeApply();
            } catch (Throwable e) {
                Log.e(DoseAlarms.TAG, "Do not disturb: not applied", e);
            } finally {
                held.finish();
            }
        }, "sioul-dnd").start();
    }

    /** Asked from the background service's process: applied in Sioul's own. */
    static void poke(Context context)
    {
        context.sendBroadcast(new Intent(context, DndReceiver.class).setAction(APPLY));
    }

    /**
     * Applied again at `atMillis` (Unix milliseconds), the next end Rust
     * gave; 0 takes the alarm away. Exact through Doze while allowed, else
     * within Android's inexact window.
     */
    static void schedule(Context context, long atMillis)
    {
        AlarmManager alarms = context.getSystemService(AlarmManager.class);
        PendingIntent apply = PendingIntent.getBroadcast(context, CODE, new Intent(context, DndReceiver.class).setAction(APPLY),
                                                         PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        if (atMillis <= 0) {
            alarms.cancel(apply);
            return;
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S || alarms.canScheduleExactAlarms())
            alarms.setExactAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, atMillis, apply);
        else
            alarms.setAndAllowWhileIdle(AlarmManager.RTC_WAKEUP, atMillis, apply);
    }
}
