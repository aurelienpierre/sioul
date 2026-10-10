// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.util.Log;

/**
 * A reminder tapped: its dose kept for Sioul ({@link DoseAlarms#takeOpened}, which
 * Rust reads when the window comes up), then Sioul brought up as by its icon.
 * A window of its own, never shown, so that no intent with a dose in it stays
 * with Sioul's window, to be given again when Android restores it.
 */
public final class DoseOpener extends Activity
{
    @Override
    protected void onCreate(Bundle state)
    {
        super.onCreate(state);
        String key = DoseAlarms.key(getIntent());
        if (state == null && !key.isEmpty()) {
            DoseAlarms.opened(this, key);
            Log.i(DoseAlarms.TAG, "Doses: a reminder tapped, its dose kept for Sioul.");
        }
        Intent sioul = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (sioul != null)
            startActivity(sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED));
        finish();
    }
}
