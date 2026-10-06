// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/**
 * The background service's alarm, and the phone's restart or Sioul's update
 * (StepService, in the same process ":steps"): a step for the service
 * running, else the service started again, if it was on. An exact alarm
 * lets an app start its foreground service from the background; so do the
 * restart and the update.
 */
public final class StepReceiver extends BroadcastReceiver
{
    @Override
    public void onReceive(Context context, Intent intent)
    {
        String action = intent.getAction();
        if (action == null)
            return;
        Context app = context.getApplicationContext();
        switch (action) {
        case StepService.STEP:
            StepService.alarm(app);
            break;
        case Intent.ACTION_BOOT_COMPLETED:
        case Intent.ACTION_MY_PACKAGE_REPLACED:
            StepService.restart(app, "restart");
            break;
        default:
            break;
        }
    }
}
