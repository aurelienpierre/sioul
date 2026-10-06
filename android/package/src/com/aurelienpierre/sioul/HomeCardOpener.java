// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;

/**
 * A line of the home screen's card tapped (HomeCard): the Porch, or Now on
 * its step, kept for Rust (HomeCard.opened; homecard.rs takes it when the
 * window comes up), then Sioul brought up as by its icon. A window of its
 * own, never shown, so that no intent naming a step stays with Sioul's
 * window, to be given again when Android restores it. Not exported.
 */
public final class HomeCardOpener extends Activity
{
    @Override
    protected void onCreate(Bundle state)
    {
        super.onCreate(state);
        if (state == null)
            HomeCard.opened(this, getIntent());
        Intent sioul = getPackageManager().getLaunchIntentForPackage(getPackageName());
        if (sioul != null)
            startActivity(sioul.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_RESET_TASK_IF_NEEDED));
        finish();
    }
}
