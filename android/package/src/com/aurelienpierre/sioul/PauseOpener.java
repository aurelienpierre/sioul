// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;

/**
 * "Pause" pressed on the quick-settings tile (PauseTile) or the home screen's
 * shortcut (res/xml/sioul_shortcuts.xml): the press kept for Sioul
 * ({@link PauseMode#pressed}, which Rust takes when the window comes up), then Sioul
 * brought up as by its icon, where the pause starts, or shows when it is on
 * already. Never shown itself. Not exported: no other app can press it.
 */
public final class PauseOpener extends Activity
{
    @Override
    protected void onCreate(Bundle state)
    {
        super.onCreate(state);
        if (state == null)
            PauseMode.pressed(this);
        Intent sioul = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (sioul != null)
            startActivity(sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED));
        finish();
    }
}
