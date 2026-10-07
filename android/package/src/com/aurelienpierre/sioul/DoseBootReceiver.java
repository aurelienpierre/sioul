// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright © 2026 Aurélien Pierre

package com.aurelienpierre.sioul;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;

/**
 * The doses' alarms given again from the list kept (DoseAlarms): Android
 * forgets them when it restarts; an update keeps them, given again all the
 * same; allowed "Alarms &amp; reminders" again, they ring on time again. After a
 * restart the phone is unlocked once first (Sioul's files cannot be read
 * before): a dose due meanwhile rings then.
 */
public final class DoseBootReceiver extends BroadcastReceiver
{
    @Override
    public void onReceive(Context context, Intent intent)
    {
        DoseAlarms.restore(context.getApplicationContext(), intent.getAction());
    }
}
