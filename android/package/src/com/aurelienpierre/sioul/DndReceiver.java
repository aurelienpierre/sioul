// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.AlarmManager;
import android.app.NotificationManager;
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
 *
 * Both ways (docs/do-not-disturb.md): the phone's own do-not-disturb heard as
 * it changes, Sioul closed or not. Android sends a package with Do Not
 * Disturb access ACTION_INTERRUPTION_FILTER_CHANGED (NotificationManagerService,
 * "explicitly send the broadcast to all DND packages, even if they aren't
 * currently running"), and the owner of a mode its
 * ACTION_AUTOMATIC_ZEN_RULE_STATUS_CHANGED; both are protected broadcasts,
 * which only the system sends, so this receiver stays not exported. Decided
 * in Java (DndHeard), the library loaded only for a press; a change heard
 * while Sioul's own settles is looked at again once it did. Also Sioul's
 * switch pressed on its quick-settings tile (DndTile).
 */
public final class DndReceiver extends BroadcastReceiver
{
    static final String APPLY = "com.aurelienpierre.sioul.action.DND_APPLY";
    /** Sioul's switch pressed on its quick-settings tile (DndTile). */
    static final String TOGGLE = "com.aurelienpierre.sioul.action.DND_TOGGLE";
    private static final int CODE = 0x5138;
    /** A look again waits this long at most, the broadcast held open meanwhile. */
    private static final long WAIT_MS = 5_000;

    /** Do-not-disturb as the reasons say now, applied here (Rust); its answer, "" when none. */
    static native String nativeApply();

    /** A change of the phone's own do-not-disturb, heard as it happened: {on}. A press when it disagrees (Rust). */
    static native String nativeHeard(String json);

    /** Sioul's switch pressed on its tile: on, or off, as it is not now (Rust). */
    static native String nativeToggle();

    @Override
    public void onReceive(Context context, Intent intent)
    {
        String action = intent.getAction();
        if (action == null)
            return;
        final Context app = context.getApplicationContext();
        switch (action) {
        case Intent.ACTION_BOOT_COMPLETED:
        case Intent.ACTION_MY_PACKAGE_REPLACED:
            // The phone's state found, not heard: the next change counts from what holds then.
            PauseMode.forgetSeen(app);
            later(goAsync(), "applied", () -> {
                DoseAlarms.load(app);
                nativeApply();
            });
            return;
        case APPLY:
            later(goAsync(), "applied", () -> {
                DoseAlarms.load(app);
                nativeApply();
            });
            return;
        case TOGGLE:
            later(goAsync(), "pressed", () -> {
                DoseAlarms.load(app);
                nativeToggle();
            });
            return;
        case NotificationManager.ACTION_INTERRUPTION_FILTER_CHANGED:
        case NotificationManager.ACTION_AUTOMATIC_ZEN_RULE_STATUS_CHANGED:
            heard(app, intent, goAsync());
            return;
        default:
            return;
        }
    }

    /** A change of the phone's own do-not-disturb: decided, looked at again once Sioul's own change settled, pressed (Rust). */
    private static void heard(Context app, Intent intent, PendingResult held)
    {
        final String action = intent.getAction();
        final String id = intent.getStringExtra(NotificationManager.EXTRA_AUTOMATIC_ZEN_RULE_ID);
        final int status = intent.getIntExtra(NotificationManager.EXTRA_AUTOMATIC_ZEN_RULE_STATUS, NotificationManager.AUTOMATIC_RULE_STATUS_UNKNOWN);
        later(held, "heard", () -> {
            String kind = PauseMode.kindOfRule(app, id);
            String said = PauseMode.decide(app, action, status, kind, id);
            if (said.startsWith(DndHeard.LATER)) {
                long wait = Math.min(WAIT_MS, Long.parseLong(said.substring(DndHeard.LATER.length())));
                Thread.sleep(Math.max(0, wait));
                said = PauseMode.decide(app, action, status, kind, id);
            }
            if (!"on".equals(said) && !"off".equals(said))
                return;
            Log.i(DoseAlarms.TAG, "Do not disturb: the phone's own turned " + said + ", heard.");
            DoseAlarms.load(app);
            nativeHeard("{\"on\":" + "on".equals(said) + "}");
        });
    }

    /** Work that may wait, on a thread of its own, the broadcast held open meanwhile. */
    private interface Work
    {
        void run() throws Exception;
    }

    private static void later(PendingResult held, String what, Work work)
    {
        new Thread(() -> {
            try {
                work.run();
            } catch (Throwable e) {
                Log.e(DoseAlarms.TAG, "Do not disturb: not " + what, e);
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
