// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.os.Bundle;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.view.WindowManager;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.TextClock;

import java.lang.ref.WeakReference;

/**
 * The alarm at waking, over the lock screen (WakeRinger's full-screen
 * notification): the time, Stop, 10 min later (a try's last ring: Stop
 * alone). Nothing else. Back does not end it: the alarm rings until one of
 * the two. Dark, the screen kept on while it shows. Opened once the ring is
 * over (from an old notification), it closes at once.
 */
public final class WakeScreen extends Activity
{
    private static WeakReference<WakeScreen> shown = new WeakReference<>(null);

    /** The ring is over (here, from its notification, or 10 min later): the screen goes. */
    static void close()
    {
        WakeScreen screen = shown.get();
        if (screen != null)
            screen.runOnUiThread(screen::finish);
    }

    /** Another ring over the one shown (a waking over a try): its buttons as they are now. */
    static void refresh()
    {
        WakeScreen screen = shown.get();
        if (screen != null)
            screen.runOnUiThread(() -> screen.setContentView(screen.layout()));
    }

    @Override
    protected void onCreate(Bundle state)
    {
        super.onCreate(state);
        if (!WakeRinger.ringing()) {
            finish();
            return;
        }
        shown = new WeakReference<>(this);
        setShowWhenLocked(true);
        setTurnScreenOn(true);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        setContentView(layout());
    }

    private View layout()
    {
        final float dp = getResources().getDisplayMetrics().density;
        LinearLayout column = new LinearLayout(this);
        column.setOrientation(LinearLayout.VERTICAL);
        column.setGravity(Gravity.CENTER);
        column.setBackgroundColor(0xFF101412);
        int margin = (int) (24 * dp);
        column.setPadding(margin, margin, margin, margin);
        // The time now, as the phone writes it, kept up to date.
        TextClock time = new TextClock(this);
        time.setTextSize(TypedValue.COMPLEX_UNIT_SP, 64);
        time.setTextColor(0xFFE6EAE7);
        time.setGravity(Gravity.CENTER);
        column.addView(time, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        column.addView(button(WakeAlarms.word(this, "stop"), false, dp), spaced(48, dp));
        if (WakeRinger.laterOffered())
            column.addView(button(WakeAlarms.word(this, "later"), true, dp), spaced(16, dp));
        return column;
    }

    private Button button(String text, boolean later, float dp)
    {
        Button button = new Button(this);
        button.setText(text);
        button.setAllCaps(false);
        button.setTextSize(TypedValue.COMPLEX_UNIT_SP, 20);
        button.setMinHeight((int) (64 * dp));
        button.setOnClickListener(v -> {
            WakeRinger.stop(getApplicationContext(), later, WakeRinger.trying());
            finish();
        });
        return button;
    }

    private static LinearLayout.LayoutParams spaced(int above, float dp)
    {
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        params.topMargin = (int) (above * dp);
        return params;
    }

    @Override
    public void onBackPressed()
    {
        // The alarm rings until Stop or 10 min later: Back does not end it.
    }

    @Override
    protected void onDestroy()
    {
        if (shown.get() == this)
            shown.clear();
        super.onDestroy();
    }
}
