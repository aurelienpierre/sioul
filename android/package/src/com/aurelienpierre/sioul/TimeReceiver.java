// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;

import java.util.concurrent.atomic.AtomicBoolean;

/**
 * A button of the time running (TimeNote): Pause, Go on or Stop, done by
 * Rust's side (crates/sioul-app/src/timenote.rs) on a thread of its own, the
 * broadcast held open meanwhile (goAsync), Sioul's window running or not.
 * Rust reads first what your other devices did, so that a button a moment
 * behind changes nothing of it, then shows the note as it follows itself:
 * two buttons pressed in a row show in their order. Android gives a button's
 * broadcast ten seconds: past eight it is let go, and Rust's note comes when
 * it is done.
 */
public final class TimeReceiver extends BroadcastReceiver
{
    private static final long DEADLINE_MS = 8_000;

    @Override
    public void onReceive(Context context, Intent intent)
    {
        final String action = word(intent.getAction());
        if (action.isEmpty())
            return;
        final Context app = context.getApplicationContext();
        final PendingResult broadcast = goAsync();
        final AtomicBoolean finished = new AtomicBoolean();
        final Runnable letGo = () -> {
            if (finished.compareAndSet(false, true))
                broadcast.finish();
        };
        final Handler main = new Handler(Looper.getMainLooper());
        main.postDelayed(letGo, DEADLINE_MS);
        new Thread(() -> {
            try {
                DoseAlarms.load(app);
                String note = TimeNote.nativeAction(action);
                Log.i(DoseAlarms.TAG, "Time: " + action + (note == null || note.isEmpty() ? ", no session left." : ", done."));
            } catch (Throwable e) {
                // Sioul's library that does not load, among others: the note stays as it was.
                Log.e(DoseAlarms.TAG, "Time: " + action + " not done", e);
            }
            main.removeCallbacks(letGo);
            letGo.run();
        }, "sioul-time").start();
    }

    /** The button's action as Rust reads it: "pause", "resume", "stop"; "" for any other. */
    private static String word(String action)
    {
        if (TimeNote.PAUSE.equals(action))
            return "pause";
        if (TimeNote.RESUME.equals(action))
            return "resume";
        if (TimeNote.STOP.equals(action))
            return "stop";
        return "";
    }
}
