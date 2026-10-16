// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.Manifest;
import android.app.Activity;
import android.os.Bundle;
import android.os.SystemClock;
import android.util.Log;

/**
 * Android's question for the texts' permissions (docs/texts.md), asked from
 * Settings ▸ This phone ▸ Texts: reading the texts (READ_SMS), hearing a new
 * one (RECEIVE_SMS), sending one written on a computer (SEND_SMS), and the
 * SIMs' names (READ_PHONE_STATE). Android answers only an activity, so this
 * one, never seen, asks and goes. Sioul never asks to be the SMS app.
 */
public final class TextsAsk extends Activity
{
    private static final int ASK = 0x5140;
    /** When the question was asked (elapsed time, ms): its answer at once means none was shown (`Texts.atOnce`). */
    private long asked = -1;

    @Override
    protected void onCreate(Bundle saved)
    {
        super.onCreate(saved);
        if (saved != null)
            return;
        asked = SystemClock.elapsedRealtime();
        requestPermissions(new String[] { Manifest.permission.READ_SMS, Manifest.permission.RECEIVE_SMS, Manifest.permission.SEND_SMS, Manifest.permission.READ_PHONE_STATE }, ASK);
    }

    @Override
    public void onRequestPermissionsResult(int request, String[] permissions, int[] results)
    {
        super.onRequestPermissionsResult(request, permissions, results);
        int given = 0;
        for (int result : results)
            if (result == android.content.pm.PackageManager.PERMISSION_GRANTED)
                given++;
        long took = asked < 0 ? -1 : SystemClock.elapsedRealtime() - asked;
        Log.i(Texts.TAG, "Texts: " + given + " of " + results.length + " permissions given" + (Texts.atOnce(given, took) ? ", at once: no question was shown." : "."));
        Texts.remember(this, given, results.length, took);
        finish();
    }
}
